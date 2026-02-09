// sv_lag_compensation.rs -- Server-side lag compensation for fair hit detection
//
// When a high-ping client fires a weapon, this module allows the server to
// "rewind" entity positions to where they were when the client actually saw
// the shot, making hit detection fair regardless of ping.
//
// Key concepts:
// 1. Store historical snapshots of entity positions
// 2. When processing a hit, rewind to client's perceived time
// 3. Perform hit detection against rewound positions
// 4. Apply results in current time

use myq2_common::q_shared::{Vec3, MAX_EDICTS};

/// Number of historical snapshots to keep per entity
/// At 10Hz server frame rate, 16 frames = 1.6 seconds of history
pub const LAG_COMPENSATION_FRAMES: usize = 16;

/// Maximum ping we'll compensate for (in ms)
pub const MAX_LAG_COMPENSATION_MS: i32 = 200;

/// A snapshot of entity position at a specific time
#[derive(Clone, Debug, Default)]
pub struct EntitySnapshot {
    /// Server time when this snapshot was taken (ms)
    pub time: i32,
    /// Entity origin
    pub origin: Vec3,
    /// Entity mins (bounding box)
    pub mins: Vec3,
    /// Entity maxs (bounding box)
    pub maxs: Vec3,
    /// Whether this snapshot is valid
    pub valid: bool,
    /// Entity was solid at this time
    pub solid: bool,
}

/// Historical position data for a single entity
#[derive(Clone)]
pub struct EntityHistory {
    /// Ring buffer of snapshots
    pub snapshots: [EntitySnapshot; LAG_COMPENSATION_FRAMES],
    /// Current write index in the ring buffer
    pub write_index: usize,
    /// Entity number
    pub entity_num: i32,
}

impl Default for EntityHistory {
    fn default() -> Self {
        Self {
            snapshots: std::array::from_fn(|_| EntitySnapshot::default()),
            write_index: 0,
            entity_num: 0,
        }
    }
}

impl EntityHistory {
    /// Record a new snapshot for this entity
    pub fn record(&mut self, time: i32, origin: &Vec3, mins: &Vec3, maxs: &Vec3, solid: bool) {
        let snapshot = &mut self.snapshots[self.write_index];
        snapshot.time = time;
        snapshot.origin = *origin;
        snapshot.mins = *mins;
        snapshot.maxs = *maxs;
        snapshot.solid = solid;
        snapshot.valid = true;

        self.write_index = (self.write_index + 1) % LAG_COMPENSATION_FRAMES;
    }

    /// Find the snapshot closest to the given time
    /// Returns None if no valid snapshot is found within reasonable bounds
    pub fn get_snapshot_at_time(&self, target_time: i32) -> Option<&EntitySnapshot> {
        let mut best_snapshot: Option<&EntitySnapshot> = None;
        let mut best_diff = i32::MAX;

        for snapshot in &self.snapshots {
            if !snapshot.valid {
                continue;
            }

            let diff = (snapshot.time - target_time).abs();
            if diff < best_diff {
                best_diff = diff;
                best_snapshot = Some(snapshot);
            }
        }

        // Only return if we found something reasonably close (within 200ms)
        if best_diff <= MAX_LAG_COMPENSATION_MS {
            best_snapshot
        } else {
            None
        }
    }

    /// Interpolate between two snapshots to get position at exact time
    pub fn interpolate_at_time(&self, target_time: i32) -> Option<EntitySnapshot> {
        // Find the two snapshots bracketing the target time
        let mut before: Option<&EntitySnapshot> = None;
        let mut after: Option<&EntitySnapshot> = None;

        for snapshot in &self.snapshots {
            if !snapshot.valid {
                continue;
            }

            if snapshot.time <= target_time {
                if before.is_none() || snapshot.time > before.unwrap().time {
                    before = Some(snapshot);
                }
            }
            if snapshot.time >= target_time {
                if after.is_none() || snapshot.time < after.unwrap().time {
                    after = Some(snapshot);
                }
            }
        }

        match (before, after) {
            (Some(b), Some(a)) if b.time != a.time => {
                // Interpolate between before and after
                let total_time = (a.time - b.time) as f32;
                let lerp = (target_time - b.time) as f32 / total_time;

                let mut result = EntitySnapshot {
                    time: target_time,
                    valid: true,
                    solid: b.solid || a.solid,
                    ..Default::default()
                };

                for i in 0..3 {
                    result.origin[i] = b.origin[i] + lerp * (a.origin[i] - b.origin[i]);
                    result.mins[i] = b.mins[i] + lerp * (a.mins[i] - b.mins[i]);
                    result.maxs[i] = b.maxs[i] + lerp * (a.maxs[i] - b.maxs[i]);
                }

                Some(result)
            }
            (Some(s), _) | (_, Some(s)) => {
                // Only have one snapshot - use it directly
                Some(s.clone())
            }
            _ => None,
        }
    }

    /// Clear all history for this entity
    pub fn clear(&mut self) {
        for snapshot in &mut self.snapshots {
            snapshot.valid = false;
        }
        self.write_index = 0;
    }
}

/// Lag compensation state for the entire server
pub struct LagCompensation {
    /// Historical data for all entities
    pub entities: Vec<EntityHistory>,
    /// Whether lag compensation is enabled
    pub enabled: bool,
    /// Maximum compensation time in ms
    pub max_compensation_ms: i32,
    /// Debug mode - log compensation actions
    pub debug: bool,
}

impl Default for LagCompensation {
    fn default() -> Self {
        let mut entities = Vec::with_capacity(MAX_EDICTS);
        for i in 0..MAX_EDICTS {
            let mut history = EntityHistory::default();
            history.entity_num = i as i32;
            entities.push(history);
        }

        Self {
            entities,
            enabled: true,
            max_compensation_ms: MAX_LAG_COMPENSATION_MS,
            debug: false,
        }
    }
}

impl LagCompensation {
    /// Create a new lag compensation system
    pub fn new() -> Self {
        Self::default()
    }

    /// Record current entity positions at the given server time
    pub fn record_frame(&mut self, time: i32, entity_data: &[(i32, Vec3, Vec3, Vec3, bool)]) {
        if !self.enabled {
            return;
        }

        for (entity_num, origin, mins, maxs, solid) in entity_data {
            if *entity_num >= 0 && (*entity_num as usize) < self.entities.len() {
                self.entities[*entity_num as usize].record(time, origin, mins, maxs, *solid);
            }
        }
    }

    /// Get entity position at a specific time (for lag-compensated hit detection)
    ///
    /// # Arguments
    /// * `entity_num` - The entity to look up
    /// * `target_time` - The time to look up (usually: server_time - client_ping)
    /// * `interpolate` - Whether to interpolate between snapshots
    ///
    /// # Returns
    /// The entity snapshot at the target time, or None if not available
    pub fn get_entity_at_time(
        &self,
        entity_num: i32,
        target_time: i32,
        interpolate: bool,
    ) -> Option<EntitySnapshot> {
        if !self.enabled {
            return None;
        }

        if entity_num < 0 || (entity_num as usize) >= self.entities.len() {
            return None;
        }

        let history = &self.entities[entity_num as usize];

        if interpolate {
            history.interpolate_at_time(target_time)
        } else {
            history.get_snapshot_at_time(target_time).cloned()
        }
    }

    /// Calculate the rewind time for a client based on their ping
    ///
    /// # Arguments
    /// * `server_time` - Current server time
    /// * `client_ping` - Client's current ping in ms
    ///
    /// # Returns
    /// The time to rewind to for hit detection
    pub fn calculate_rewind_time(&self, server_time: i32, client_ping: i32) -> i32 {
        // Clamp ping to maximum compensation
        let compensated_ping = client_ping.min(self.max_compensation_ms);
        server_time - compensated_ping
    }

    /// Perform lag-compensated hit test
    ///
    /// This is a convenience function that:
    /// 1. Calculates rewind time based on client ping
    /// 2. Gets entity position at that time
    /// 3. Performs bounding box hit test
    ///
    /// # Arguments
    /// * `entity_num` - Target entity
    /// * `server_time` - Current server time
    /// * `client_ping` - Attacking client's ping
    /// * `start` - Start of trace line
    /// * `end` - End of trace line
    ///
    /// # Returns
    /// (hit, hit_point) - Whether there was a hit and where
    pub fn test_hit(
        &self,
        entity_num: i32,
        server_time: i32,
        client_ping: i32,
        start: &Vec3,
        end: &Vec3,
    ) -> (bool, Vec3) {
        if !self.enabled {
            return (false, [0.0; 3]);
        }

        let rewind_time = self.calculate_rewind_time(server_time, client_ping);

        if let Some(snapshot) = self.get_entity_at_time(entity_num, rewind_time, true) {
            if !snapshot.solid {
                return (false, [0.0; 3]);
            }

            // Simple AABB line intersection test
            let hit = line_intersects_aabb(start, end, &snapshot.origin, &snapshot.mins, &snapshot.maxs);
            if let Some(point) = hit {
                if self.debug {
                    myq2_common::common::com_dprintf(&format!(
                        "Lag comp hit: entity {} rewound {}ms (ping {})\n",
                        entity_num, server_time - rewind_time, client_ping
                    ));
                }
                return (true, point);
            }
        }

        (false, [0.0; 3])
    }

    /// Clear all history (call on map change)
    pub fn clear(&mut self) {
        for entity in &mut self.entities {
            entity.clear();
        }
    }

    /// Enable or disable lag compensation
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Set maximum compensation time
    pub fn set_max_compensation(&mut self, max_ms: i32) {
        self.max_compensation_ms = max_ms.clamp(0, 500);
    }
}

/// Simple line-AABB intersection test
fn line_intersects_aabb(
    start: &Vec3,
    end: &Vec3,
    origin: &Vec3,
    mins: &Vec3,
    maxs: &Vec3,
) -> Option<Vec3> {
    // Calculate AABB bounds in world space
    let mut box_mins = [0.0f32; 3];
    let mut box_maxs = [0.0f32; 3];
    for i in 0..3 {
        box_mins[i] = origin[i] + mins[i];
        box_maxs[i] = origin[i] + maxs[i];
    }

    // Ray direction
    let mut dir = [0.0f32; 3];
    for i in 0..3 {
        dir[i] = end[i] - start[i];
    }

    let mut t_min = 0.0f32;
    let mut t_max = 1.0f32;

    for i in 0..3 {
        if dir[i].abs() < 1e-6 {
            // Ray parallel to slab
            if start[i] < box_mins[i] || start[i] > box_maxs[i] {
                return None;
            }
        } else {
            let inv_d = 1.0 / dir[i];
            let mut t1 = (box_mins[i] - start[i]) * inv_d;
            let mut t2 = (box_maxs[i] - start[i]) * inv_d;

            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }

            t_min = t_min.max(t1);
            t_max = t_max.min(t2);

            if t_min > t_max {
                return None;
            }
        }
    }

    // Calculate hit point
    let mut hit_point = [0.0f32; 3];
    for i in 0..3 {
        hit_point[i] = start[i] + t_min * dir[i];
    }

    Some(hit_point)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_history() {
        let mut history = EntityHistory::default();

        // Record some positions
        history.record(0, &[0.0, 0.0, 0.0], &[-16.0; 3], &[16.0; 3], true);
        history.record(100, &[100.0, 0.0, 0.0], &[-16.0; 3], &[16.0; 3], true);
        history.record(200, &[200.0, 0.0, 0.0], &[-16.0; 3], &[16.0; 3], true);

        // Get snapshot at time 150 (should interpolate)
        let snapshot = history.interpolate_at_time(150);
        assert!(snapshot.is_some());
        let s = snapshot.unwrap();
        assert!((s.origin[0] - 150.0).abs() < 1.0);
    }

    #[test]
    fn test_lag_compensation() {
        let mut lag_comp = LagCompensation::new();

        // Record entity positions
        lag_comp.entities[1].record(0, &[0.0, 0.0, 0.0], &[-16.0; 3], &[16.0; 3], true);
        lag_comp.entities[1].record(100, &[100.0, 0.0, 0.0], &[-16.0; 3], &[16.0; 3], true);

        // Test rewinding
        let rewind_time = lag_comp.calculate_rewind_time(150, 100);
        assert_eq!(rewind_time, 50);

        // Get entity at rewind time
        let snapshot = lag_comp.get_entity_at_time(1, 50, true);
        assert!(snapshot.is_some());
    }

    #[test]
    fn test_line_aabb_intersection() {
        let origin = [0.0, 0.0, 0.0];
        let mins = [-16.0, -16.0, -16.0];
        let maxs = [16.0, 16.0, 16.0];

        // Line that hits the box
        let start = [-100.0, 0.0, 0.0];
        let end = [100.0, 0.0, 0.0];
        let hit = line_intersects_aabb(&start, &end, &origin, &mins, &maxs);
        assert!(hit.is_some());

        // Line that misses the box
        let start2 = [-100.0, 100.0, 0.0];
        let end2 = [100.0, 100.0, 0.0];
        let hit2 = line_intersects_aabb(&start2, &end2, &origin, &mins, &maxs);
        assert!(hit2.is_none());
    }
}
