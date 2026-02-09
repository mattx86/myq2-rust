// cl_smooth.rs -- Advanced network smoothing and interpolation
//
// This module provides comprehensive smoothing features for online gameplay:
// - Adaptive interpolation buffer (adjusts to network conditions)
// - Dead reckoning for other players
// - View/camera smoothing
// - Weapon fire prediction
// - Cubic/spline interpolation
// - Network statistics
// - Entity priority system
// - Input buffering

use myq2_common::q_shared::*;
use std::collections::VecDeque;

// ============================================================
// Adaptive Interpolation Buffer
// ============================================================

/// Adaptive interpolation configuration
#[derive(Debug, Clone)]
pub struct AdaptiveInterpolation {
    /// Current target buffer in milliseconds
    pub target_buffer_ms: i32,
    /// Minimum buffer size
    pub min_buffer_ms: i32,
    /// Maximum buffer size
    pub max_buffer_ms: i32,
    /// Recent jitter samples for calculation
    jitter_history: VecDeque<i32>,
    /// Last packet arrival time
    last_arrival_time: i32,
    /// Expected packet interval (typically 100ms for 10Hz)
    expected_interval_ms: i32,
    /// Smoothing factor for buffer adjustments (0.0-1.0)
    pub smoothing_factor: f32,
    /// Whether adaptive mode is enabled
    pub enabled: bool,
}

impl Default for AdaptiveInterpolation {
    fn default() -> Self {
        Self {
            target_buffer_ms: 100,      // Start with standard 100ms
            min_buffer_ms: 50,          // Never go below 50ms
            max_buffer_ms: 200,         // Never exceed 200ms
            jitter_history: VecDeque::with_capacity(32),
            last_arrival_time: 0,
            expected_interval_ms: 100,  // 10Hz server
            smoothing_factor: 0.1,      // Slow adjustment
            enabled: true,
        }
    }
}

impl AdaptiveInterpolation {
    /// Record a packet arrival and update jitter statistics
    pub fn record_packet(&mut self, arrival_time: i32) {
        if !self.enabled {
            return;
        }

        if self.last_arrival_time > 0 {
            let interval = arrival_time - self.last_arrival_time;
            let jitter = (interval - self.expected_interval_ms).abs();

            self.jitter_history.push_back(jitter);
            if self.jitter_history.len() > 32 {
                self.jitter_history.pop_front();
            }

            // Update expected interval with exponential moving average
            self.expected_interval_ms =
                ((self.expected_interval_ms as f32 * 0.9) + (interval as f32 * 0.1)) as i32;
        }
        self.last_arrival_time = arrival_time;

        // Recalculate target buffer based on jitter
        self.update_target_buffer();
    }

    /// Update the target buffer based on measured jitter
    fn update_target_buffer(&mut self) {
        if self.jitter_history.is_empty() {
            return;
        }

        // Calculate average and max jitter
        let avg_jitter: i32 = self.jitter_history.iter().sum::<i32>()
            / self.jitter_history.len() as i32;
        let max_jitter = *self.jitter_history.iter().max().unwrap_or(&0);

        // Target buffer = expected interval + 2x average jitter + some headroom
        // Use max jitter for extra safety margin
        let ideal_buffer = self.expected_interval_ms + (avg_jitter * 2) + (max_jitter / 2);

        // Smooth transition to new target
        let new_target = (self.target_buffer_ms as f32 * (1.0 - self.smoothing_factor))
            + (ideal_buffer as f32 * self.smoothing_factor);

        self.target_buffer_ms = (new_target as i32).clamp(self.min_buffer_ms, self.max_buffer_ms);
    }

    /// Get the current interpolation delay to use
    pub fn get_lerp_delay(&self) -> i32 {
        if self.enabled {
            self.target_buffer_ms
        } else {
            100 // Default 100ms
        }
    }

    /// Get current jitter estimate in ms
    pub fn get_jitter(&self) -> i32 {
        if self.jitter_history.is_empty() {
            0
        } else {
            self.jitter_history.iter().sum::<i32>() / self.jitter_history.len() as i32
        }
    }

    /// Reset adaptive state (call on disconnect)
    pub fn reset(&mut self) {
        self.jitter_history.clear();
        self.last_arrival_time = 0;
        self.target_buffer_ms = 100;
        self.expected_interval_ms = 100;
    }
}

// ============================================================
// Dead Reckoning for Other Players
// ============================================================

/// Movement pattern for prediction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MovementPattern {
    #[default]
    Unknown,
    Standing,
    Walking,
    Running,
    Strafing,
    Jumping,
    Falling,
    Swimming,
}

/// Dead reckoning state for a player entity
#[derive(Debug, Clone, Default)]
pub struct DeadReckoningState {
    /// Last known position
    pub position: Vec3,
    /// Last known velocity
    pub velocity: Vec3,
    /// Last known acceleration (for more accurate prediction)
    pub acceleration: Vec3,
    /// Current movement pattern
    pub pattern: MovementPattern,
    /// Time of last server update
    pub last_update_time: i32,
    /// Confidence in prediction (0.0 to 1.0, decreases over time)
    pub confidence: f32,
    /// Maximum prediction time before giving up (ms)
    pub max_prediction_ms: i32,
    /// Whether this player's input pattern is predictable
    pub predictable: bool,
    /// Recent position history for pattern detection
    position_history: VecDeque<(i32, Vec3)>,
}

impl DeadReckoningState {
    pub fn new() -> Self {
        Self {
            max_prediction_ms: 200,
            confidence: 1.0,
            position_history: VecDeque::with_capacity(16),
            ..Default::default()
        }
    }

    /// Update with new server data
    pub fn update(&mut self, position: Vec3, time: i32) {
        // Calculate velocity from position delta
        if !self.position_history.is_empty() {
            let dt = (time - self.last_update_time) as f32 / 1000.0;
            if dt > 0.0 && dt < 1.0 {
                let old_velocity = self.velocity;
                for i in 0..3 {
                    self.velocity[i] = (position[i] - self.position[i]) / dt;
                    // Calculate acceleration
                    self.acceleration[i] = (self.velocity[i] - old_velocity[i]) / dt;
                }
            }
        }

        // Store in history
        self.position_history.push_back((time, position));
        if self.position_history.len() > 16 {
            self.position_history.pop_front();
        }

        // Detect movement pattern
        self.detect_pattern();

        self.position = position;
        self.last_update_time = time;
        self.confidence = 1.0;
    }

    /// Detect movement pattern from history
    fn detect_pattern(&mut self) {
        let speed = (self.velocity[0].powi(2) + self.velocity[1].powi(2)).sqrt();
        let vertical_speed = self.velocity[2];

        self.pattern = if vertical_speed > 200.0 {
            MovementPattern::Jumping
        } else if vertical_speed < -200.0 {
            MovementPattern::Falling
        } else if speed < 10.0 {
            MovementPattern::Standing
        } else if speed < 200.0 {
            MovementPattern::Walking
        } else {
            MovementPattern::Running
        };

        // Check for strafing (rapid direction changes)
        if self.position_history.len() >= 4 {
            // Simplified strafe detection - check for lateral velocity changes
            self.predictable = speed > 50.0;
        }
    }

    /// Predict position at given time
    pub fn predict(&self, current_time: i32, gravity: f32) -> Vec3 {
        let dt = (current_time - self.last_update_time) as f32 / 1000.0;

        // Clamp prediction time
        if dt <= 0.0 || dt > (self.max_prediction_ms as f32 / 1000.0) {
            return self.position;
        }

        let mut predicted = self.position;

        // Apply velocity
        for i in 0..3 {
            predicted[i] += self.velocity[i] * dt;
        }

        // Apply acceleration (with damping for stability)
        let accel_factor = (1.0 - dt).max(0.0); // Dampen acceleration over time
        for i in 0..2 { // Only X and Y acceleration
            predicted[i] += 0.5 * self.acceleration[i] * accel_factor * dt * dt;
        }

        // Apply gravity for jumping/falling
        if matches!(self.pattern, MovementPattern::Jumping | MovementPattern::Falling) {
            predicted[2] -= 0.5 * gravity * dt * dt;
        }

        predicted
    }

    /// Get confidence-adjusted predicted position (blends with last known)
    pub fn predict_with_confidence(&mut self, current_time: i32, gravity: f32) -> Vec3 {
        let predicted = self.predict(current_time, gravity);

        // Reduce confidence over time
        let dt = (current_time - self.last_update_time) as f32 / 1000.0;
        self.confidence = (1.0 - dt * 2.0).max(0.0);

        // Blend between prediction and last known position based on confidence
        let mut result = [0.0f32; 3];
        for i in 0..3 {
            result[i] = predicted[i] * self.confidence + self.position[i] * (1.0 - self.confidence);
        }
        result
    }
}

// ============================================================
// View Interpolation Smoothing
// ============================================================

/// View smoothing state to prevent camera snapping
#[derive(Debug, Clone, Default)]
pub struct ViewSmoothing {
    /// Current smoothed origin
    pub smoothed_origin: Vec3,
    /// Current smoothed angles
    pub smoothed_angles: Vec3,
    /// Origin velocity for smooth transitions
    pub origin_velocity: Vec3,
    /// Angle velocity for smooth transitions
    pub angle_velocity: Vec3,
    /// Maximum correction per second (units)
    pub max_origin_speed: f32,
    /// Maximum angle correction per second (degrees)
    pub max_angle_speed: f32,
    /// Whether smoothing is active
    pub enabled: bool,
    /// Initialized flag
    initialized: bool,
}

impl ViewSmoothing {
    pub fn new() -> Self {
        Self {
            max_origin_speed: 500.0,   // 500 units/sec max correction
            max_angle_speed: 180.0,    // 180 deg/sec max correction
            enabled: true,
            ..Default::default()
        }
    }

    /// Update view with smoothing applied
    pub fn update(
        &mut self,
        target_origin: &Vec3,
        target_angles: &Vec3,
        delta_time: f32,
    ) -> (Vec3, Vec3) {
        if !self.enabled {
            return (*target_origin, *target_angles);
        }

        if !self.initialized {
            self.smoothed_origin = *target_origin;
            self.smoothed_angles = *target_angles;
            self.initialized = true;
            return (*target_origin, *target_angles);
        }

        // Smooth origin
        let max_origin_delta = self.max_origin_speed * delta_time;
        for i in 0..3 {
            let diff = target_origin[i] - self.smoothed_origin[i];
            let clamped_diff = diff.clamp(-max_origin_delta, max_origin_delta);
            self.smoothed_origin[i] += clamped_diff;
            self.origin_velocity[i] = clamped_diff / delta_time;
        }

        // Smooth angles (with wrap-around handling)
        let max_angle_delta = self.max_angle_speed * delta_time;
        for i in 0..3 {
            let mut diff = target_angles[i] - self.smoothed_angles[i];
            // Normalize to -180..180
            while diff > 180.0 { diff -= 360.0; }
            while diff < -180.0 { diff += 360.0; }

            let clamped_diff = diff.clamp(-max_angle_delta, max_angle_delta);
            self.smoothed_angles[i] += clamped_diff;
            self.angle_velocity[i] = clamped_diff / delta_time;

            // Normalize result
            while self.smoothed_angles[i] > 180.0 { self.smoothed_angles[i] -= 360.0; }
            while self.smoothed_angles[i] < -180.0 { self.smoothed_angles[i] += 360.0; }
        }

        (self.smoothed_origin, self.smoothed_angles)
    }

    /// Force immediate snap to target (for teleports, etc.)
    pub fn snap_to(&mut self, origin: &Vec3, angles: &Vec3) {
        self.smoothed_origin = *origin;
        self.smoothed_angles = *angles;
        self.origin_velocity = [0.0; 3];
        self.angle_velocity = [0.0; 3];
        self.initialized = true;
    }

    /// Reset smoothing state
    pub fn reset(&mut self) {
        self.initialized = false;
        self.origin_velocity = [0.0; 3];
        self.angle_velocity = [0.0; 3];
    }
}

// ============================================================
// Weapon Fire Prediction
// ============================================================

/// Predicted weapon effect for immediate visual feedback
#[derive(Debug, Clone)]
pub struct PredictedWeaponEffect {
    /// Effect type (muzzle flash, tracer, impact)
    pub effect_type: WeaponEffectType,
    /// Origin of effect
    pub origin: Vec3,
    /// Direction/end point
    pub direction: Vec3,
    /// Time effect was created
    pub create_time: i32,
    /// How long effect lasts (ms)
    pub duration_ms: i32,
    /// Whether confirmed by server
    pub confirmed: bool,
    /// Server sequence to match for confirmation
    pub sequence: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponEffectType {
    MuzzleFlash,
    Tracer,
    BulletImpact,
    RocketTrail,
    RailTrail,
}

/// Weapon fire prediction state
#[derive(Debug, Clone)]
pub struct WeaponPrediction {
    /// Pending predicted effects
    pub effects: VecDeque<PredictedWeaponEffect>,
    /// Maximum pending effects
    max_effects: usize,
    /// Whether prediction is enabled
    pub enabled: bool,
    /// Current sequence number
    sequence: i32,
    /// Timeout for unconfirmed predictions (ms)
    pub timeout_ms: i32,
}

impl Default for WeaponPrediction {
    fn default() -> Self {
        Self {
            effects: VecDeque::with_capacity(16),
            max_effects: 16,
            enabled: true,
            sequence: 0,
            timeout_ms: 200,
        }
    }
}

impl WeaponPrediction {
    /// Predict a weapon fire effect
    pub fn predict_fire(
        &mut self,
        effect_type: WeaponEffectType,
        origin: Vec3,
        direction: Vec3,
        current_time: i32,
    ) -> i32 {
        if !self.enabled {
            return -1;
        }

        self.sequence += 1;

        let effect = PredictedWeaponEffect {
            effect_type,
            origin,
            direction,
            create_time: current_time,
            duration_ms: match effect_type {
                WeaponEffectType::MuzzleFlash => 50,
                WeaponEffectType::Tracer => 100,
                WeaponEffectType::BulletImpact => 100,
                WeaponEffectType::RocketTrail => 200,
                WeaponEffectType::RailTrail => 500,
            },
            confirmed: false,
            sequence: self.sequence,
        };

        self.effects.push_back(effect);

        // Limit queue size
        while self.effects.len() > self.max_effects {
            self.effects.pop_front();
        }

        self.sequence
    }

    /// Confirm a predicted effect (server acknowledged)
    pub fn confirm(&mut self, sequence: i32) {
        for effect in self.effects.iter_mut() {
            if effect.sequence == sequence {
                effect.confirmed = true;
                break;
            }
        }
    }

    /// Get active effects for rendering
    pub fn get_active_effects(&self, current_time: i32) -> Vec<&PredictedWeaponEffect> {
        self.effects.iter()
            .filter(|e| {
                let age = current_time - e.create_time;
                age >= 0 && age < e.duration_ms
            })
            .collect()
    }

    /// Clean up old effects
    pub fn cleanup(&mut self, current_time: i32) {
        self.effects.retain(|e| {
            let age = current_time - e.create_time;
            // Keep if still visible or recently created (waiting for confirm)
            age < e.duration_ms || (!e.confirmed && age < self.timeout_ms)
        });
    }

    /// Clear all predictions
    pub fn clear(&mut self) {
        self.effects.clear();
    }
}

// ============================================================
// Cubic/Spline Interpolation
// ============================================================

/// Catmull-Rom spline interpolation for smooth curves
pub fn catmull_rom_interpolate(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;

    0.5 * ((2.0 * p1) +
           (-p0 + p2) * t +
           (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2 +
           (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

/// Catmull-Rom interpolation for Vec3
pub fn catmull_rom_interpolate_vec3(p0: &Vec3, p1: &Vec3, p2: &Vec3, p3: &Vec3, t: f32) -> Vec3 {
    [
        catmull_rom_interpolate(p0[0], p1[0], p2[0], p3[0], t),
        catmull_rom_interpolate(p0[1], p1[1], p2[1], p3[1], t),
        catmull_rom_interpolate(p0[2], p1[2], p2[2], p3[2], t),
    ]
}

/// Frame history for spline interpolation
#[derive(Debug, Clone, Default)]
pub struct SplineHistory {
    /// Ring buffer of positions with timestamps
    positions: VecDeque<(i32, Vec3)>,
    /// Maximum history size
    max_history: usize,
}

impl SplineHistory {
    pub fn new(max_history: usize) -> Self {
        Self {
            positions: VecDeque::with_capacity(max_history),
            max_history,
        }
    }

    /// Add a new position sample
    pub fn add(&mut self, time: i32, position: Vec3) {
        self.positions.push_back((time, position));
        while self.positions.len() > self.max_history {
            self.positions.pop_front();
        }
    }

    /// Interpolate position at given time using Catmull-Rom spline
    pub fn interpolate(&self, target_time: i32) -> Option<Vec3> {
        if self.positions.len() < 4 {
            // Not enough points for spline, fall back to linear
            return self.interpolate_linear(target_time);
        }

        // Find the two middle control points bracketing target_time
        let mut p1_idx = None;
        for (i, (time, _)) in self.positions.iter().enumerate() {
            if *time >= target_time {
                p1_idx = Some(i.saturating_sub(1));
                break;
            }
        }

        let p1_idx = p1_idx.unwrap_or(self.positions.len().saturating_sub(2));

        if p1_idx == 0 || p1_idx + 2 >= self.positions.len() {
            return self.interpolate_linear(target_time);
        }

        let (t0, p0) = self.positions[p1_idx - 1];
        let (t1, p1) = self.positions[p1_idx];
        let (t2, p2) = self.positions[p1_idx + 1];
        let (t3, p3) = self.positions[p1_idx + 2];

        // Calculate interpolation parameter
        if t2 == t1 {
            return Some(p1);
        }
        let t = ((target_time - t1) as f32) / ((t2 - t1) as f32);
        let t = t.clamp(0.0, 1.0);

        Some(catmull_rom_interpolate_vec3(&p0, &p1, &p2, &p3, t))
    }

    /// Fallback linear interpolation
    fn interpolate_linear(&self, target_time: i32) -> Option<Vec3> {
        if self.positions.len() < 2 {
            return self.positions.back().map(|(_, p)| *p);
        }

        // Find bracketing points
        let mut p1_idx = 0;
        for (i, (time, _)) in self.positions.iter().enumerate() {
            if *time >= target_time {
                p1_idx = i.saturating_sub(1);
                break;
            }
        }

        if p1_idx + 1 >= self.positions.len() {
            return self.positions.back().map(|(_, p)| *p);
        }

        let (t1, p1) = self.positions[p1_idx];
        let (t2, p2) = self.positions[p1_idx + 1];

        if t2 == t1 {
            return Some(p1);
        }

        let t = ((target_time - t1) as f32) / ((t2 - t1) as f32);
        let t = t.clamp(0.0, 1.0);

        let mut result = [0.0f32; 3];
        for i in 0..3 {
            result[i] = p1[i] + t * (p2[i] - p1[i]);
        }
        Some(result)
    }

    /// Clear history
    pub fn clear(&mut self) {
        self.positions.clear();
    }
}

// ============================================================
// Network Statistics
// ============================================================

/// Network statistics for diagnostics
#[derive(Debug, Clone, Default)]
pub struct NetworkStats {
    /// Current ping in ms
    pub ping: i32,
    /// Average ping over recent samples
    pub avg_ping: i32,
    /// Minimum ping seen
    pub min_ping: i32,
    /// Maximum ping seen
    pub max_ping: i32,
    /// Current jitter (ping variance)
    pub jitter: i32,
    /// Packet loss percentage (0-100)
    pub packet_loss: f32,
    /// Packets received
    pub packets_received: u64,
    /// Packets lost (estimated)
    pub packets_lost: u64,
    /// Current interpolation buffer size
    pub interp_buffer_ms: i32,
    /// Extrapolation active
    pub extrapolating: bool,
    /// Incoming bandwidth (bytes/sec)
    pub incoming_bps: i32,
    /// Outgoing bandwidth (bytes/sec)
    pub outgoing_bps: i32,
    /// Last update time
    last_update: i32,
    /// Ping history for averaging
    ping_history: VecDeque<i32>,
    /// Bytes received this second
    bytes_this_second: i32,
    /// Second start time
    second_start: i32,
}

impl NetworkStats {
    pub fn new() -> Self {
        Self {
            min_ping: i32::MAX,
            ping_history: VecDeque::with_capacity(64),
            ..Default::default()
        }
    }

    /// Record a ping sample
    pub fn record_ping(&mut self, ping: i32, current_time: i32) {
        self.ping = ping;
        self.ping_history.push_back(ping);
        if self.ping_history.len() > 64 {
            self.ping_history.pop_front();
        }

        // Update statistics
        self.min_ping = self.min_ping.min(ping);
        self.max_ping = self.max_ping.max(ping);

        if !self.ping_history.is_empty() {
            self.avg_ping = self.ping_history.iter().sum::<i32>()
                / self.ping_history.len() as i32;

            // Calculate jitter (standard deviation approximation)
            let variance: i32 = self.ping_history.iter()
                .map(|p| (p - self.avg_ping).abs())
                .sum::<i32>() / self.ping_history.len() as i32;
            self.jitter = variance;
        }

        self.last_update = current_time;
    }

    /// Record packet received
    pub fn record_packet(&mut self, size: i32, current_time: i32) {
        self.packets_received += 1;

        // Track bandwidth
        if current_time - self.second_start >= 1000 {
            self.incoming_bps = self.bytes_this_second;
            self.bytes_this_second = 0;
            self.second_start = current_time;
        }
        self.bytes_this_second += size;
    }

    /// Record packet loss
    pub fn record_loss(&mut self, expected_seq: i32, received_seq: i32) {
        if received_seq > expected_seq {
            let lost = (received_seq - expected_seq) as u64;
            self.packets_lost += lost;
        }

        // Calculate loss percentage
        let total = self.packets_received + self.packets_lost;
        if total > 0 {
            self.packet_loss = (self.packets_lost as f32 / total as f32) * 100.0;
        }
    }

    /// Get formatted statistics string for display
    pub fn format_display(&self) -> String {
        format!(
            "Ping: {}ms (avg:{} min:{} max:{})\n\
             Jitter: {}ms | Loss: {:.1}%\n\
             Interp: {}ms | BW: {:.1}KB/s",
            self.ping, self.avg_ping,
            if self.min_ping == i32::MAX { 0 } else { self.min_ping },
            self.max_ping,
            self.jitter, self.packet_loss,
            self.interp_buffer_ms,
            self.incoming_bps as f32 / 1024.0
        )
    }

    /// Reset statistics
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

// ============================================================
// Bandwidth Adaptation
// ============================================================

/// Network quality level for bandwidth adaptation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkQuality {
    /// Excellent - low ping, low jitter, no loss
    Excellent,
    /// Good - acceptable conditions
    Good,
    /// Fair - some congestion detected
    Fair,
    /// Poor - high packet loss or jitter
    Poor,
    /// Critical - severe network issues
    Critical,
}

/// Bandwidth adaptation system that monitors network conditions
/// and provides recommendations for adjusting client send rate.
#[derive(Debug, Clone)]
pub struct BandwidthAdapter {
    /// Whether adaptation is enabled
    pub enabled: bool,
    /// Current network quality assessment
    pub quality: NetworkQuality,
    /// Recommended packets per second (client to server)
    pub recommended_rate: i32,
    /// Minimum rate (never go below)
    pub min_rate: i32,
    /// Maximum rate (never exceed)
    pub max_rate: i32,
    /// Current rate
    pub current_rate: i32,
    /// History of quality samples for trend detection
    quality_history: VecDeque<NetworkQuality>,
    /// Last adaptation time
    last_adapt_time: i32,
    /// Adaptation interval (ms) - don't adapt too frequently
    adapt_interval_ms: i32,
    /// Consecutive poor samples before reducing rate
    poor_threshold: i32,
    /// Consecutive good samples before increasing rate
    good_threshold: i32,
}

impl Default for BandwidthAdapter {
    fn default() -> Self {
        Self {
            enabled: true,
            quality: NetworkQuality::Good,
            recommended_rate: 80, // 80 packets/sec default (Quake 2 standard)
            min_rate: 20,         // Never below 20 pps
            max_rate: 125,        // Cap at 125 pps (8ms interval)
            current_rate: 80,
            quality_history: VecDeque::with_capacity(16),
            last_adapt_time: 0,
            adapt_interval_ms: 2000, // Adapt at most every 2 seconds
            poor_threshold: 4,       // 4 consecutive poor samples to reduce
            good_threshold: 8,       // 8 consecutive good samples to increase
        }
    }
}

impl BandwidthAdapter {
    /// Assess network quality from current statistics
    pub fn assess_quality(&self, stats: &NetworkStats) -> NetworkQuality {
        // Critical: very high packet loss
        if stats.packet_loss > 15.0 {
            return NetworkQuality::Critical;
        }

        // Poor: high jitter or moderate loss
        if stats.jitter > 100 || stats.packet_loss > 5.0 {
            return NetworkQuality::Poor;
        }

        // Fair: noticeable jitter or some loss
        if stats.jitter > 50 || stats.packet_loss > 2.0 {
            return NetworkQuality::Fair;
        }

        // Good: moderate conditions
        if stats.jitter > 20 || stats.packet_loss > 0.5 {
            return NetworkQuality::Good;
        }

        // Excellent: pristine conditions
        NetworkQuality::Excellent
    }

    /// Update bandwidth adaptation based on current network stats
    /// Returns true if rate recommendation changed
    pub fn update(&mut self, stats: &NetworkStats, current_time: i32) -> bool {
        if !self.enabled {
            return false;
        }

        // Assess current quality
        self.quality = self.assess_quality(stats);

        // Add to history
        self.quality_history.push_back(self.quality);
        if self.quality_history.len() > 16 {
            self.quality_history.pop_front();
        }

        // Don't adapt too frequently
        if current_time - self.last_adapt_time < self.adapt_interval_ms {
            return false;
        }

        // Count consecutive poor/good samples
        let mut consecutive_poor = 0;
        let mut consecutive_good = 0;

        for q in self.quality_history.iter().rev() {
            match q {
                NetworkQuality::Poor | NetworkQuality::Critical => {
                    consecutive_poor += 1;
                    consecutive_good = 0;
                }
                NetworkQuality::Excellent | NetworkQuality::Good => {
                    consecutive_good += 1;
                    consecutive_poor = 0;
                }
                NetworkQuality::Fair => {
                    // Fair doesn't break streak but doesn't add to it
                }
            }
        }

        let old_rate = self.recommended_rate;

        // Reduce rate if poor conditions persist
        if consecutive_poor >= self.poor_threshold {
            // Reduce by 20%, minimum min_rate
            self.recommended_rate = ((self.recommended_rate as f32 * 0.8) as i32).max(self.min_rate);
            self.last_adapt_time = current_time;
        }
        // Increase rate if good conditions persist
        else if consecutive_good >= self.good_threshold && self.recommended_rate < self.max_rate {
            // Increase by 10%, maximum max_rate
            self.recommended_rate = ((self.recommended_rate as f32 * 1.1) as i32).min(self.max_rate);
            self.last_adapt_time = current_time;
        }

        self.current_rate = self.recommended_rate;
        old_rate != self.recommended_rate
    }

    /// Get the recommended command interval in milliseconds
    pub fn get_cmd_interval_ms(&self) -> i32 {
        if self.current_rate <= 0 {
            return 100; // Fallback to 10 Hz
        }
        1000 / self.current_rate
    }

    /// Force a specific rate (for manual override via cvar)
    pub fn set_rate(&mut self, rate: i32) {
        self.current_rate = rate.clamp(self.min_rate, self.max_rate);
        self.recommended_rate = self.current_rate;
    }

    /// Reset adaptation state
    pub fn reset(&mut self) {
        self.quality = NetworkQuality::Good;
        self.recommended_rate = 80;
        self.current_rate = 80;
        self.quality_history.clear();
        self.last_adapt_time = 0;
    }

    /// Get quality as string for display
    pub fn quality_string(&self) -> &'static str {
        match self.quality {
            NetworkQuality::Excellent => "Excellent",
            NetworkQuality::Good => "Good",
            NetworkQuality::Fair => "Fair",
            NetworkQuality::Poor => "Poor",
            NetworkQuality::Critical => "Critical",
        }
    }
}

// ============================================================
// Entity Priority System
// ============================================================

/// Entity priority for network updates
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EntityPriority {
    /// Critical - always update (local player, attacker)
    Critical = 4,
    /// High - players, nearby projectiles
    High = 3,
    /// Medium - nearby entities, active items
    Medium = 2,
    /// Low - distant entities
    Low = 1,
    /// Minimal - far decorations
    Minimal = 0,
}

/// Entity priority calculation with view frustum awareness
#[derive(Debug, Clone)]
pub struct EntityPrioritySystem {
    /// Distance thresholds for priority levels
    pub high_distance: f32,
    pub medium_distance: f32,
    pub low_distance: f32,
    /// Whether priority system is enabled
    pub enabled: bool,
    /// Whether frustum-based boosting is enabled
    pub frustum_boost_enabled: bool,
    /// Field of view for frustum check (half-angle in degrees)
    pub frustum_half_fov: f32,
}

impl Default for EntityPrioritySystem {
    fn default() -> Self {
        Self {
            high_distance: 512.0,    // Within 512 units = high priority
            medium_distance: 1024.0, // Within 1024 = medium
            low_distance: 2048.0,    // Within 2048 = low
            enabled: true,
            frustum_boost_enabled: true,
            frustum_half_fov: 55.0,  // Slightly wider than typical 90 degree FOV
        }
    }
}

impl EntityPrioritySystem {
    /// Check if entity is in player's view frustum (simplified cone check)
    fn is_in_view_frustum(
        &self,
        viewer_origin: &Vec3,
        viewer_forward: &Vec3,
        entity_origin: &Vec3,
    ) -> bool {
        // Calculate direction to entity
        let dx = entity_origin[0] - viewer_origin[0];
        let dy = entity_origin[1] - viewer_origin[1];
        let dz = entity_origin[2] - viewer_origin[2];

        // Get distance (avoid zero division)
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
        if dist < 0.001 {
            return true; // Entity at viewer position
        }

        // Normalize direction
        let dir_x = dx / dist;
        let dir_y = dy / dist;
        let dir_z = dz / dist;

        // Dot product with view forward
        let dot = dir_x * viewer_forward[0] + dir_y * viewer_forward[1] + dir_z * viewer_forward[2];

        // Convert FOV to cosine threshold
        let fov_cos = (self.frustum_half_fov * std::f32::consts::PI / 180.0).cos();

        // Entity is in frustum if dot product is greater than threshold
        dot > fov_cos
    }

    /// Calculate priority for an entity
    pub fn calculate_priority(
        &self,
        viewer_origin: &Vec3,
        entity_origin: &Vec3,
        is_player: bool,
        is_projectile: bool,
        is_attacker: bool,
    ) -> EntityPriority {
        if !self.enabled {
            return EntityPriority::High;
        }

        // Attackers are always critical
        if is_attacker {
            return EntityPriority::Critical;
        }

        // Players are high priority regardless of distance
        if is_player {
            return EntityPriority::High;
        }

        // Calculate distance
        let dx = viewer_origin[0] - entity_origin[0];
        let dy = viewer_origin[1] - entity_origin[1];
        let dz = viewer_origin[2] - entity_origin[2];
        let distance = (dx * dx + dy * dy + dz * dz).sqrt();

        // Projectiles within medium range are high priority
        if is_projectile && distance < self.medium_distance {
            return EntityPriority::High;
        }

        // Distance-based priority
        if distance < self.high_distance {
            EntityPriority::High
        } else if distance < self.medium_distance {
            EntityPriority::Medium
        } else if distance < self.low_distance {
            EntityPriority::Low
        } else {
            EntityPriority::Minimal
        }
    }

    /// Calculate priority with view frustum awareness
    /// Entities in the player's view get boosted priority
    pub fn calculate_priority_with_frustum(
        &self,
        viewer_origin: &Vec3,
        viewer_forward: &Vec3,
        entity_origin: &Vec3,
        is_player: bool,
        is_projectile: bool,
        is_attacker: bool,
    ) -> EntityPriority {
        // Get base priority
        let base_priority = self.calculate_priority(
            viewer_origin, entity_origin, is_player, is_projectile, is_attacker
        );

        // If frustum boost is enabled and entity is in view, boost priority
        if self.frustum_boost_enabled
            && base_priority < EntityPriority::High
            && self.is_in_view_frustum(viewer_origin, viewer_forward, entity_origin)
        {
            // Boost by one level for entities in view
            match base_priority {
                EntityPriority::Medium => EntityPriority::High,
                EntityPriority::Low => EntityPriority::Medium,
                EntityPriority::Minimal => EntityPriority::Low,
                other => other,
            }
        } else {
            base_priority
        }
    }

    /// Get update interval based on priority (in ms)
    pub fn get_update_interval(&self, priority: EntityPriority) -> i32 {
        match priority {
            EntityPriority::Critical => 0,   // Every frame
            EntityPriority::High => 0,       // Every frame
            EntityPriority::Medium => 50,    // Every 50ms
            EntityPriority::Low => 100,      // Every 100ms
            EntityPriority::Minimal => 200,  // Every 200ms
        }
    }
}

// ============================================================
// Input Buffering
// ============================================================

/// Buffered input command
#[derive(Debug, Clone)]
pub struct BufferedInput {
    /// Forward/back movement
    pub forward: f32,
    /// Side movement
    pub side: f32,
    /// Up movement (jump/crouch)
    pub up: f32,
    /// View angles
    pub angles: Vec3,
    /// Button states
    pub buttons: u32,
    /// Time this input was recorded
    pub time: i32,
}

/// Input buffer for smooth local movement
#[derive(Debug, Clone)]
pub struct InputBuffer {
    /// Buffered inputs
    inputs: VecDeque<BufferedInput>,
    /// Buffer size in frames
    buffer_size: usize,
    /// Whether buffering is enabled
    pub enabled: bool,
    /// Smoothing factor for input blending
    pub smoothing: f32,
}

impl Default for InputBuffer {
    fn default() -> Self {
        Self {
            inputs: VecDeque::with_capacity(4),
            buffer_size: 2,  // 2 frame buffer
            enabled: true,
            smoothing: 0.5,
        }
    }
}

impl InputBuffer {
    /// Add input to buffer
    pub fn add(&mut self, input: BufferedInput) {
        if !self.enabled {
            return;
        }

        self.inputs.push_back(input);
        while self.inputs.len() > self.buffer_size {
            self.inputs.pop_front();
        }
    }

    /// Get smoothed input
    pub fn get_smoothed(&self) -> Option<BufferedInput> {
        if !self.enabled || self.inputs.is_empty() {
            return None;
        }

        if self.inputs.len() == 1 {
            return self.inputs.back().cloned();
        }

        // Blend recent inputs
        let mut result = BufferedInput {
            forward: 0.0,
            side: 0.0,
            up: 0.0,
            angles: [0.0; 3],
            buttons: 0,
            time: 0,
        };

        let weight_sum: f32 = (0..self.inputs.len())
            .map(|i| (i + 1) as f32)
            .sum();

        for (i, input) in self.inputs.iter().enumerate() {
            let weight = (i + 1) as f32 / weight_sum;
            result.forward += input.forward * weight;
            result.side += input.side * weight;
            result.up += input.up * weight;
            for j in 0..3 {
                result.angles[j] += input.angles[j] * weight;
            }
            // Use latest buttons
            result.buttons = input.buttons;
            result.time = input.time;
        }

        Some(result)
    }

    /// Clear buffer
    pub fn clear(&mut self) {
        self.inputs.clear();
    }
}

// ============================================================
// Prediction Error Smoothing
// ============================================================

/// Smooths prediction error corrections over multiple frames
/// instead of applying them instantly, reducing visual jarring
#[derive(Debug, Clone)]
pub struct PredictionErrorSmoothing {
    /// Current smoothed error (being applied)
    pub current_error: Vec3,
    /// Target error (from server correction)
    pub target_error: Vec3,
    /// Time when error was set
    pub error_time: i32,
    /// Duration to smooth over (ms)
    pub smooth_duration_ms: i32,
    /// Whether smoothing is enabled
    pub enabled: bool,
}

impl Default for PredictionErrorSmoothing {
    fn default() -> Self {
        Self {
            current_error: [0.0; 3],
            target_error: [0.0; 3],
            error_time: 0,
            smooth_duration_ms: 150, // Smooth over 150ms for better jitter tolerance
            enabled: true,
        }
    }
}

impl PredictionErrorSmoothing {
    /// Set a new prediction error to smooth
    pub fn set_error(&mut self, error: Vec3, current_time: i32) {
        if !self.enabled {
            self.current_error = error;
            return;
        }

        self.target_error = error;
        self.error_time = current_time;
    }

    /// Get the smoothed error for the current frame
    pub fn get_smoothed_error(&mut self, current_time: i32) -> Vec3 {
        if !self.enabled {
            return self.current_error;
        }

        let elapsed = current_time - self.error_time;
        if elapsed >= self.smooth_duration_ms {
            // Smoothing complete
            self.current_error = self.target_error;
            return self.current_error;
        }

        // Interpolate from current to target
        let t = elapsed as f32 / self.smooth_duration_ms as f32;
        let mut result = [0.0f32; 3];
        for i in 0..3 {
            result[i] = self.current_error[i] + t * (self.target_error[i] - self.current_error[i]);
        }

        result
    }

    /// Clear the error (on teleport, etc.)
    pub fn clear(&mut self) {
        self.current_error = [0.0; 3];
        self.target_error = [0.0; 3];
    }
}

// ============================================================
// Recoil/Kick Smoothing
// ============================================================

/// Smooths weapon recoil/kick angles with momentum decay
/// for more natural-feeling weapon feedback
#[derive(Debug, Clone)]
pub struct RecoilSmoothing {
    /// Current smoothed kick angles
    pub current_kick: Vec3,
    /// Target kick angles from server
    pub target_kick: Vec3,
    /// Angular velocity for momentum (degrees/sec)
    pub velocity: Vec3,
    /// Decay rate (how fast recoil settles) - higher = faster recovery
    pub decay_rate: f32,
    /// Approach rate (how fast we reach target) - higher = snappier recoil
    pub approach_rate: f32,
    /// Whether smoothing is enabled
    pub enabled: bool,
}

impl Default for RecoilSmoothing {
    fn default() -> Self {
        Self {
            current_kick: [0.0; 3],
            target_kick: [0.0; 3],
            velocity: [0.0; 3],
            decay_rate: 8.0,    // Recover from recoil in ~125ms
            approach_rate: 20.0, // Snap to new recoil in ~50ms
            enabled: true,
        }
    }
}

impl RecoilSmoothing {
    /// Update recoil smoothing with new target and delta time
    pub fn update(&mut self, target_kick: &Vec3, delta_time: f32) -> Vec3 {
        if !self.enabled {
            self.current_kick = *target_kick;
            return self.current_kick;
        }

        self.target_kick = *target_kick;

        for i in 0..3 {
            // Calculate difference to target
            let diff = self.target_kick[i] - self.current_kick[i];

            // Add velocity toward target (approach)
            self.velocity[i] += diff * self.approach_rate * delta_time;

            // Apply velocity decay (momentum dampening)
            self.velocity[i] *= (1.0 - self.decay_rate * delta_time).max(0.0);

            // Update current position
            self.current_kick[i] += self.velocity[i] * delta_time;

            // When target is zero and we're close, decay to zero faster
            if self.target_kick[i].abs() < 0.1 && self.current_kick[i].abs() < 1.0 {
                self.current_kick[i] *= 1.0 - (self.decay_rate * 2.0 * delta_time).min(1.0);
            }
        }

        self.current_kick
    }

    /// Continue recoil decay during packet loss
    /// This ensures the view punch continues to settle naturally
    /// even when we don't receive updates from the server
    pub fn continue_decay(&mut self, delta_time: f32) -> Vec3 {
        if !self.enabled {
            return self.current_kick;
        }

        for i in 0..3 {
            // During packet loss, decay toward zero (natural recovery)
            // Use a gentler decay rate for smoother continuation
            let decay_factor = (1.0 - self.decay_rate * 0.5 * delta_time).max(0.0);

            // Apply velocity decay
            self.velocity[i] *= decay_factor;

            // Update position with remaining velocity
            self.current_kick[i] += self.velocity[i] * delta_time;

            // Additional decay toward zero
            self.current_kick[i] *= decay_factor;

            // Clamp very small values to zero to prevent floating point creep
            if self.current_kick[i].abs() < 0.01 {
                self.current_kick[i] = 0.0;
            }
        }

        self.current_kick
    }

    /// Clear recoil state (on weapon switch, teleport, etc.)
    pub fn clear(&mut self) {
        self.current_kick = [0.0; 3];
        self.target_kick = [0.0; 3];
        self.velocity = [0.0; 3];
    }

    /// Predict weapon fire recoil locally before server confirms.
    /// This provides immediate feedback when the player fires,
    /// reducing perceived input lag.
    ///
    /// # Arguments
    /// * `weapon_type` - Weapon type (1=blaster, 2=shotgun, etc.)
    /// * `current_time` - Current client time in ms
    ///
    /// Returns true if prediction was applied.
    pub fn predict_fire(&mut self, weapon_type: i32, _current_time: i32) -> bool {
        if !self.enabled {
            return false;
        }

        // Weapon-specific recoil patterns (degrees)
        // These approximate the server's kick_angles
        let (pitch_kick, yaw_kick) = match weapon_type {
            1 => (-1.0, 0.0),              // Blaster - minimal recoil
            2 => (-3.0, 0.5),              // Shotgun - moderate kick up
            3 => (-5.0, 1.0),              // Super shotgun - stronger kick
            4 => (-0.5, 0.0),              // Machinegun - rapid small kicks
            5 => (-0.8, 0.2),              // Chaingun - similar to machinegun
            6 => (-1.0, 0.0),              // Grenades - minimal (throwing motion)
            7 => (-2.0, 0.0),              // Rocket launcher - moderate
            8 => (-0.3, 0.0),              // Hyperblaster - small rapid kicks
            9 => (-4.0, 0.0),              // Railgun - strong kick
            10 => (-3.0, 0.0),             // BFG - moderate sustained
            _ => (-1.0, 0.0),              // Default
        };

        // Add small random variation for more natural feel
        let pitch_var = (rand::random::<f32>() - 0.5) * 0.2;
        let yaw_var = (rand::random::<f32>() - 0.5) * 0.5;

        // Apply predicted kick with some velocity for smooth application
        self.velocity[0] += (pitch_kick + pitch_var) * self.approach_rate * 0.5;
        self.velocity[1] += (yaw_kick + yaw_var) * self.approach_rate * 0.5;

        true
    }
}

// ============================================================
// Frame Time Smoothing
// ============================================================

/// Smooths frame time deltas to reduce jitter from variable frame rates
#[derive(Debug, Clone)]
pub struct FrameTimeSmoothing {
    /// Recent frame time samples
    samples: VecDeque<f32>,
    /// Maximum samples to keep
    max_samples: usize,
    /// Whether smoothing is enabled
    pub enabled: bool,
}

impl Default for FrameTimeSmoothing {
    fn default() -> Self {
        Self {
            samples: VecDeque::with_capacity(8),
            max_samples: 8,
            enabled: true,
        }
    }
}

impl FrameTimeSmoothing {
    /// Add a frame time sample and get the smoothed value
    pub fn add_sample(&mut self, frame_time: f32) -> f32 {
        if !self.enabled {
            return frame_time;
        }

        self.samples.push_back(frame_time);
        while self.samples.len() > self.max_samples {
            self.samples.pop_front();
        }

        // Return weighted average (more recent = higher weight)
        if self.samples.is_empty() {
            return frame_time;
        }

        let mut weighted_sum = 0.0f32;
        let mut weight_total = 0.0f32;
        for (i, &sample) in self.samples.iter().enumerate() {
            let weight = (i + 1) as f32;
            weighted_sum += sample * weight;
            weight_total += weight;
        }

        weighted_sum / weight_total
    }

    /// Get the current average frame time
    pub fn get_average(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        self.samples.iter().sum::<f32>() / self.samples.len() as f32
    }

    /// Clear samples
    pub fn clear(&mut self) {
        self.samples.clear();
    }
}

// ============================================================
// Particle/Effect Continuation
// ============================================================

/// Tracked effect for continuation during packet loss
#[derive(Debug, Clone, Default)]
pub struct TrackedEffect {
    /// Effect type identifier
    pub effect_type: i32,
    /// Effect origin
    pub origin: Vec3,
    /// Effect velocity (for moving effects)
    pub velocity: Vec3,
    /// Time effect started
    pub start_time: i32,
    /// Duration of effect (ms)
    pub duration_ms: i32,
    /// Whether effect is active
    pub active: bool,
    /// Entity this effect is attached to (-1 if none)
    pub entity_num: i32,
}

/// Effect continuation state for particle systems
#[derive(Debug, Clone)]
pub struct EffectContinuation {
    /// Tracked effects
    pub effects: Vec<TrackedEffect>,
    /// Maximum tracked effects
    max_effects: usize,
    /// Continuation timeout (ms)
    pub timeout_ms: i32,
    /// Whether continuation is enabled
    pub enabled: bool,
}

impl Default for EffectContinuation {
    fn default() -> Self {
        Self {
            effects: Vec::with_capacity(64),
            max_effects: 64,
            timeout_ms: 500,
            enabled: true,
        }
    }
}

impl EffectContinuation {
    /// Register an effect for continuation
    pub fn register(
        &mut self,
        effect_type: i32,
        origin: Vec3,
        velocity: Vec3,
        duration_ms: i32,
        entity_num: i32,
        current_time: i32,
    ) {
        if !self.enabled {
            return;
        }

        // Find existing or empty slot
        let mut slot = None;
        for (i, effect) in self.effects.iter().enumerate() {
            if !effect.active {
                slot = Some(i);
                break;
            }
        }

        let effect = TrackedEffect {
            effect_type,
            origin,
            velocity,
            start_time: current_time,
            duration_ms,
            active: true,
            entity_num,
        };

        if let Some(idx) = slot {
            self.effects[idx] = effect;
        } else if self.effects.len() < self.max_effects {
            self.effects.push(effect);
        }
    }

    /// Get effects that should continue (with updated positions)
    pub fn get_continuing_effects(&self, current_time: i32) -> Vec<(Vec3, i32)> {
        if !self.enabled {
            return Vec::new();
        }

        self.effects.iter()
            .filter(|e| {
                e.active &&
                current_time - e.start_time < e.duration_ms + self.timeout_ms
            })
            .map(|e| {
                // Calculate current position based on velocity
                let elapsed = (current_time - e.start_time) as f32 / 1000.0;
                let mut pos = e.origin;
                for i in 0..3 {
                    pos[i] += e.velocity[i] * elapsed;
                }
                (pos, e.effect_type)
            })
            .collect()
    }

    /// Clean up expired effects
    pub fn cleanup(&mut self, current_time: i32) {
        for effect in self.effects.iter_mut() {
            if effect.active {
                let elapsed = current_time - effect.start_time;
                if elapsed > effect.duration_ms + self.timeout_ms {
                    effect.active = false;
                }
            }
        }
    }

    /// Clear all effects
    pub fn clear(&mut self) {
        self.effects.clear();
    }
}

// ============================================================
// Snapshot Buffering
// ============================================================

/// A buffered snapshot of game state for interpolation
#[derive(Debug, Clone, Default)]
pub struct GameSnapshot {
    /// Server time of this snapshot
    pub server_time: i32,
    /// Server frame number
    pub server_frame: i32,
    /// Whether this snapshot is valid
    pub valid: bool,
    /// Arrival time (client realtime)
    pub arrival_time: i32,
}

/// Buffers multiple snapshots for smoother interpolation
#[derive(Debug, Clone)]
pub struct SnapshotBuffer {
    /// Buffered snapshots
    snapshots: VecDeque<GameSnapshot>,
    /// Maximum snapshots to buffer
    max_snapshots: usize,
    /// Target buffer delay (ms)
    pub target_delay_ms: i32,
    /// Whether buffering is enabled
    pub enabled: bool,
}

impl Default for SnapshotBuffer {
    fn default() -> Self {
        Self {
            snapshots: VecDeque::with_capacity(8),
            max_snapshots: 8,
            target_delay_ms: 100, // 100ms buffer
            enabled: true,
        }
    }
}

impl SnapshotBuffer {
    /// Add a new snapshot
    pub fn add_snapshot(&mut self, server_time: i32, server_frame: i32, arrival_time: i32) {
        if !self.enabled {
            return;
        }

        let snapshot = GameSnapshot {
            server_time,
            server_frame,
            valid: true,
            arrival_time,
        };

        self.snapshots.push_back(snapshot);
        while self.snapshots.len() > self.max_snapshots {
            self.snapshots.pop_front();
        }
    }

    /// Get the best snapshot pair for interpolation at given render time
    pub fn get_interpolation_snapshots(&self, render_time: i32) -> Option<(&GameSnapshot, &GameSnapshot, f32)> {
        if self.snapshots.len() < 2 {
            return None;
        }

        // Find two snapshots bracketing the render time
        let target_time = render_time - self.target_delay_ms;

        let mut before: Option<&GameSnapshot> = None;
        let mut after: Option<&GameSnapshot> = None;

        for snapshot in &self.snapshots {
            if snapshot.server_time <= target_time {
                before = Some(snapshot);
            } else if after.is_none() {
                after = Some(snapshot);
            }
        }

        match (before, after) {
            (Some(b), Some(a)) => {
                let total = (a.server_time - b.server_time) as f32;
                let lerp = if total > 0.0 {
                    ((target_time - b.server_time) as f32 / total).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                Some((b, a, lerp))
            }
            _ => None,
        }
    }

    /// Get the newest snapshot
    pub fn get_newest(&self) -> Option<&GameSnapshot> {
        self.snapshots.back()
    }

    /// Clear all snapshots
    pub fn clear(&mut self) {
        self.snapshots.clear();
    }
}

// ============================================================
// Entity Removal Fadeout
// ============================================================

/// Configuration for entity removal fadeout
/// When entities disappear from server updates, fade them out instead of popping
#[derive(Debug, Clone)]
pub struct EntityFadeout {
    /// Whether fadeout is enabled
    pub enabled: bool,
    /// Duration of fadeout in milliseconds
    pub fadeout_duration_ms: i32,
    /// Maximum number of frames to render fading entities
    pub max_fadeout_frames: i32,
    /// Minimum alpha before entity is fully hidden
    pub min_alpha: f32,
}

impl Default for EntityFadeout {
    fn default() -> Self {
        Self {
            enabled: true,
            fadeout_duration_ms: 150, // 150ms fadeout for smooth disappearance
            max_fadeout_frames: 15,   // ~1.5 seconds at 10Hz server rate
            min_alpha: 0.05,          // Stop rendering below 5% alpha
        }
    }
}

impl EntityFadeout {
    /// Calculate the alpha for a fading entity
    /// Returns None if the entity should not be rendered at all
    pub fn calculate_alpha(
        &self,
        current_time: i32,
        last_seen_time: i32,
        current_serverframe: i32,
        entity_serverframe: i32,
    ) -> Option<f32> {
        if !self.enabled {
            return None;
        }

        // Check if entity was recently visible
        let frames_since_seen = current_serverframe - entity_serverframe;
        if frames_since_seen <= 0 || frames_since_seen > self.max_fadeout_frames {
            return None; // Entity is current or too old
        }

        // Calculate alpha based on time since last seen
        let time_since_seen = current_time - last_seen_time;
        if time_since_seen <= 0 {
            return Some(1.0); // Just disappeared, full alpha
        }

        let progress = (time_since_seen as f32) / (self.fadeout_duration_ms as f32);
        let alpha = 1.0 - progress.min(1.0);

        if alpha < self.min_alpha {
            return None; // Too faded, don't render
        }

        Some(alpha)
    }
}

// ============================================================
// Entity Spawn Fade-In
// ============================================================

/// Configuration for entity spawn fade-in
/// When entities first appear, fade them in instead of popping
#[derive(Debug, Clone)]
pub struct EntityFadein {
    /// Whether fade-in is enabled
    pub enabled: bool,
    /// Duration of fade-in in milliseconds
    pub fadein_duration_ms: i32,
}

impl Default for EntityFadein {
    fn default() -> Self {
        Self {
            enabled: true,
            fadein_duration_ms: 150, // 150ms fade-in for smooth appearance
        }
    }
}

impl EntityFadein {
    /// Calculate the alpha for a spawning entity
    /// Returns 1.0 if fully faded in or fade-in disabled
    pub fn calculate_alpha(&self, current_time: i32, spawn_time: i32) -> f32 {
        if !self.enabled || spawn_time <= 0 {
            return 1.0;
        }

        let time_since_spawn = current_time - spawn_time;
        if time_since_spawn <= 0 {
            return 0.1; // Just spawned, nearly invisible
        }

        if time_since_spawn >= self.fadein_duration_ms {
            return 1.0; // Fully faded in
        }

        // Linear fade from 0.1 to 1.0 over duration
        let progress = (time_since_spawn as f32) / (self.fadein_duration_ms as f32);
        0.1 + (progress * 0.9).min(0.9)
    }
}

// ============================================================
// Mover/Platform Velocity Prediction
// ============================================================

/// Tracks velocity of moving brush entities (doors, platforms, elevators)
/// for improved player prediction when standing on movers.
#[derive(Debug, Clone)]
pub struct MoverPrediction {
    /// Whether mover prediction is enabled
    pub enabled: bool,
    /// Per-entity mover velocity tracking (indexed by entity number)
    /// Stores (previous_origin, current_origin, calculated_velocity, last_update_time)
    mover_data: Vec<MoverEntityData>,
    /// Maximum entities to track
    max_entities: usize,
}

#[derive(Debug, Clone, Default)]
pub struct MoverEntityData {
    /// Previous frame origin
    pub prev_origin: Vec3,
    /// Current origin
    pub current_origin: Vec3,
    /// Calculated velocity (world units per second)
    pub velocity: Vec3,
    /// Last time this entity was updated
    pub last_update_time: i32,
    /// Whether this entity is a valid mover
    pub is_mover: bool,
}

impl Default for MoverPrediction {
    fn default() -> Self {
        Self {
            enabled: true,
            mover_data: Vec::new(),
            max_entities: 0,
        }
    }
}

impl MoverPrediction {
    /// Create with capacity for max_entities
    pub fn new(max_entities: usize) -> Self {
        let mut mover_data = Vec::with_capacity(max_entities);
        for _ in 0..max_entities {
            mover_data.push(MoverEntityData::default());
        }
        Self {
            enabled: true,
            mover_data,
            max_entities,
        }
    }

    /// Update a mover entity's position and calculate velocity
    pub fn update_entity(&mut self, entnum: usize, origin: &Vec3, current_time: i32, solid: i32) {
        if entnum >= self.mover_data.len() {
            return;
        }

        let data = &mut self.mover_data[entnum];

        // Detect if this is a brush model (solid type indicates SOLID_BSP = 31)
        // Brush entities have solid >= 31 (SOLID_BSP)
        data.is_mover = solid >= 31;

        if !data.is_mover {
            return;
        }

        // Calculate time delta
        let dt = if data.last_update_time > 0 {
            ((current_time - data.last_update_time) as f32) / 1000.0
        } else {
            0.0
        };

        // Store previous origin
        data.prev_origin = data.current_origin;
        data.current_origin = *origin;
        data.last_update_time = current_time;

        // Calculate velocity if we have a valid time delta
        if dt > 0.001 && dt < 1.0 {
            for i in 0..3 {
                data.velocity[i] = (data.current_origin[i] - data.prev_origin[i]) / dt;
            }
        } else if dt >= 1.0 {
            // Too much time passed, reset velocity
            data.velocity = [0.0; 3];
        }
    }

    /// Get the velocity of a mover entity
    /// Returns None if the entity is not a mover or not tracked
    pub fn get_mover_velocity(&self, entnum: usize) -> Option<Vec3> {
        if !self.enabled || entnum >= self.mover_data.len() {
            return None;
        }

        let data = &self.mover_data[entnum];
        if !data.is_mover {
            return None;
        }

        // Only return velocity if the mover has meaningful movement
        let speed_sq = data.velocity[0] * data.velocity[0]
            + data.velocity[1] * data.velocity[1]
            + data.velocity[2] * data.velocity[2];

        if speed_sq > 1.0 {
            // At least 1 unit/sec movement
            Some(data.velocity)
        } else {
            None
        }
    }

    /// Get the predicted position offset for a player standing on a mover
    pub fn get_platform_offset(&self, groundentity: i32, delta_time: f32) -> Vec3 {
        if !self.enabled || groundentity <= 0 {
            return [0.0; 3];
        }

        match self.get_mover_velocity(groundentity as usize) {
            Some(vel) => [
                vel[0] * delta_time,
                vel[1] * delta_time,
                vel[2] * delta_time,
            ],
            None => [0.0; 3],
        }
    }

    /// Clear all mover data
    pub fn clear(&mut self) {
        for data in &mut self.mover_data {
            *data = MoverEntityData::default();
        }
    }
}

// ============================================================
// Screen Blend Smoothing
// ============================================================

/// Smoothly interpolates screen blend colors (damage flash, powerups, etc.)
/// instead of hard-snapping between values. This creates smoother visual
/// transitions during packet loss or rapid state changes.
#[derive(Debug, Clone)]
pub struct ScreenBlendSmoothing {
    /// Whether blend smoothing is enabled
    pub enabled: bool,
    /// Current smoothed blend color [r, g, b, a]
    pub current_blend: [f32; 4],
    /// Target blend color from server
    pub target_blend: [f32; 4],
    /// Smoothing speed (higher = faster transition)
    pub smoothing_speed: f32,
    /// Last update time
    pub last_update_time: i32,
}

impl Default for ScreenBlendSmoothing {
    fn default() -> Self {
        Self {
            enabled: true,
            current_blend: [0.0; 4],
            target_blend: [0.0; 4],
            smoothing_speed: 10.0, // Fast enough to feel responsive, smooth enough to reduce jarring
            last_update_time: 0,
        }
    }
}

impl ScreenBlendSmoothing {
    /// Update with new blend values from server
    pub fn update(&mut self, blend: &[f32; 4], current_time: i32) {
        self.target_blend = *blend;

        if !self.enabled {
            self.current_blend = *blend;
            return;
        }

        let dt = if self.last_update_time > 0 {
            ((current_time - self.last_update_time) as f32) / 1000.0
        } else {
            0.0
        };

        // Exponential smoothing toward target
        let factor = (self.smoothing_speed * dt).min(1.0);
        for i in 0..4 {
            self.current_blend[i] += (self.target_blend[i] - self.current_blend[i]) * factor;
        }

        self.last_update_time = current_time;
    }

    /// Continue smoothing during packet loss (interpolate toward current target)
    pub fn continue_smoothing(&mut self, current_time: i32) {
        if !self.enabled {
            return;
        }

        let dt = if self.last_update_time > 0 {
            ((current_time - self.last_update_time) as f32) / 1000.0
        } else {
            0.0
        };

        // Continue interpolation during packet loss
        // Blend toward zero alpha slowly (fade out effect)
        let factor = (self.smoothing_speed * 0.5 * dt).min(1.0); // Slower during packet loss
        for i in 0..4 {
            self.current_blend[i] += (self.target_blend[i] - self.current_blend[i]) * factor;
        }

        self.last_update_time = current_time;
    }

    /// Get the smoothed blend values
    pub fn get_blend(&self) -> [f32; 4] {
        if self.enabled {
            self.current_blend
        } else {
            self.target_blend
        }
    }

    /// Clear state
    pub fn clear(&mut self) {
        self.current_blend = [0.0; 4];
        self.target_blend = [0.0; 4];
        self.last_update_time = 0;
    }
}

// ============================================================
// Item Rotation/Bobbing Continuation
// ============================================================

/// Tracks item rotation and bobbing state for smooth continuation during packet loss.
/// Items with EF_ROTATE use time-based autorotate, but we can track the phase
/// to continue smoothly when packets are dropped.
#[derive(Debug, Clone, Copy, Default)]
pub struct ItemBobState {
    /// Current rotation angle
    pub angle: f32,
    /// Rotation speed (degrees per second)
    pub rotation_speed: f32,
    /// Bob phase (0 to 2*PI)
    pub bob_phase: f32,
    /// Bob frequency for this item
    pub bob_frequency: f32,
    /// Last known bob position
    pub last_bob: f32,
    /// Last update time
    pub last_update_time: i32,
    /// Whether state is valid
    pub valid: bool,
}

/// Manages item rotation/bobbing continuation for multiple entities.
#[derive(Debug, Clone)]
pub struct ItemRotationSmoothing {
    /// Whether rotation smoothing is enabled
    pub enabled: bool,
    /// Per-entity item bob state (indexed by entity number)
    pub item_states: Vec<ItemBobState>,
    /// Default rotation speed (degrees per second)
    pub default_rotation_speed: f32,
}

impl Default for ItemRotationSmoothing {
    fn default() -> Self {
        Self {
            enabled: true,
            item_states: Vec::new(),
            default_rotation_speed: 40.0, // Approx same as original autorotate
        }
    }
}

impl ItemRotationSmoothing {
    /// Create with specified capacity
    pub fn new(max_entities: usize) -> Self {
        let mut item_states = Vec::with_capacity(max_entities);
        for _ in 0..max_entities {
            item_states.push(ItemBobState::default());
        }
        Self {
            enabled: true,
            item_states,
            default_rotation_speed: 40.0,
        }
    }

    /// Update item state from current values
    pub fn update(&mut self, entity_num: usize, angle: f32, bob: f32, current_time: i32) {
        if !self.enabled || entity_num >= self.item_states.len() {
            return;
        }

        let state = &mut self.item_states[entity_num];

        // Calculate rotation speed from angle change
        if state.valid && state.last_update_time > 0 {
            let dt = (current_time - state.last_update_time) as f32 / 1000.0;
            if dt > 0.001 && dt < 1.0 {
                let angle_diff = angle - state.angle;
                // Handle wraparound
                let angle_diff = if angle_diff > 180.0 {
                    angle_diff - 360.0
                } else if angle_diff < -180.0 {
                    angle_diff + 360.0
                } else {
                    angle_diff
                };
                // Smooth rotation speed estimate
                let estimated_speed = angle_diff / dt;
                state.rotation_speed = state.rotation_speed * 0.8 + estimated_speed * 0.2;
            }
        }

        state.angle = angle;
        state.last_bob = bob;
        state.last_update_time = current_time;
        state.valid = true;
    }

    /// Get extrapolated rotation angle during packet loss
    pub fn get_extrapolated_angle(&self, entity_num: usize, current_time: i32) -> Option<f32> {
        if !self.enabled || entity_num >= self.item_states.len() {
            return None;
        }

        let state = &self.item_states[entity_num];
        if !state.valid || state.last_update_time == 0 {
            return None;
        }

        let dt = (current_time - state.last_update_time) as f32 / 1000.0;
        if dt < 0.0 || dt > 0.5 {
            return None;
        }

        // Extrapolate angle using tracked rotation speed
        let speed = if state.rotation_speed.abs() > 1.0 {
            state.rotation_speed
        } else {
            self.default_rotation_speed
        };

        let extrapolated = state.angle + speed * dt;
        Some(extrapolated % 360.0)
    }

    /// Get extrapolated bob phase during packet loss
    /// Returns the bob offset to add to Z position
    pub fn get_extrapolated_bob(&self, entity_num: usize, current_time: i32, bob_scale: f32) -> Option<f32> {
        if !self.enabled || entity_num >= self.item_states.len() {
            return None;
        }

        let state = &self.item_states[entity_num];
        if !state.valid || state.last_update_time == 0 {
            return None;
        }

        let dt = (current_time - state.last_update_time) as f32 / 1000.0;
        if dt < 0.0 || dt > 0.5 {
            return None;
        }

        // Continue bob using same formula but with extrapolated time
        let extrapolated_time = (state.last_update_time as f32 / 1000.0) + dt;
        let bob = 5.0 + (extrapolated_time * bob_scale).cos() * 5.0;
        Some(bob)
    }

    /// Clear state for an entity
    pub fn clear_entity(&mut self, entity_num: usize) {
        if entity_num < self.item_states.len() {
            self.item_states[entity_num] = ItemBobState::default();
        }
    }

    /// Clear all state
    pub fn clear(&mut self) {
        for state in &mut self.item_states {
            *state = ItemBobState::default();
        }
    }
}

// ============================================================
// Dynamic Light Interpolation
// ============================================================

/// Tracks dynamic light state for smooth interpolation.
#[derive(Debug, Clone, Copy, Default)]
pub struct DynamicLightState {
    /// Light position
    pub origin: Vec3,
    /// Light radius
    pub radius: f32,
    /// Light color
    pub color: Vec3,
    /// Previous position for interpolation
    pub prev_origin: Vec3,
    /// Previous radius
    pub prev_radius: f32,
    /// Last update time
    pub last_update_time: i32,
    /// Whether state is valid
    pub valid: bool,
}

/// Manages dynamic light interpolation.
#[derive(Debug, Clone)]
pub struct DynamicLightSmoothing {
    /// Whether light smoothing is enabled
    pub enabled: bool,
    /// Per-entity light states (indexed by entity that creates the light)
    pub light_states: Vec<DynamicLightState>,
}

impl Default for DynamicLightSmoothing {
    fn default() -> Self {
        Self {
            enabled: true,
            light_states: Vec::new(),
        }
    }
}

impl DynamicLightSmoothing {
    /// Create with specified capacity
    pub fn new(max_entities: usize) -> Self {
        let mut light_states = Vec::with_capacity(max_entities);
        for _ in 0..max_entities {
            light_states.push(DynamicLightState::default());
        }
        Self {
            enabled: true,
            light_states,
        }
    }

    /// Update light state for an entity
    pub fn update(&mut self, entity_num: usize, origin: &Vec3, radius: f32, color: &Vec3, current_time: i32) {
        if !self.enabled || entity_num >= self.light_states.len() {
            return;
        }

        let state = &mut self.light_states[entity_num];
        state.prev_origin = state.origin;
        state.prev_radius = state.radius;
        state.origin = *origin;
        state.radius = radius;
        state.color = *color;
        state.last_update_time = current_time;
        state.valid = true;
    }

    /// Get interpolated light parameters
    pub fn get_interpolated(&self, entity_num: usize, lerp: f32) -> Option<(Vec3, f32, Vec3)> {
        if !self.enabled || entity_num >= self.light_states.len() {
            return None;
        }

        let state = &self.light_states[entity_num];
        if !state.valid {
            return None;
        }

        let mut origin = [0.0f32; 3];
        for i in 0..3 {
            origin[i] = state.prev_origin[i] + (state.origin[i] - state.prev_origin[i]) * lerp;
        }
        let radius = state.prev_radius + (state.radius - state.prev_radius) * lerp;

        Some((origin, radius, state.color))
    }

    /// Clear state for an entity
    pub fn clear_entity(&mut self, entity_num: usize) {
        if entity_num < self.light_states.len() {
            self.light_states[entity_num] = DynamicLightState::default();
        }
    }

    /// Clear all state
    pub fn clear(&mut self) {
        for state in &mut self.light_states {
            *state = DynamicLightState::default();
        }
    }
}

// ============================================================
// Weapon Animation Smoothing
// ============================================================

/// Tracks weapon animation state for smooth frame continuation during packet loss.
/// Weapon animations run at server rate (10Hz) but we render at 60+Hz,
/// so we need to smoothly interpolate between frames.
#[derive(Debug, Clone)]
pub struct WeaponAnimSmoothing {
    /// Whether weapon anim smoothing is enabled
    pub enabled: bool,
    /// Current interpolated frame (fractional)
    pub current_frame: f32,
    /// Target frame from server
    pub target_frame: i32,
    /// Previous frame
    pub prev_frame: i32,
    /// Animation direction (+1 forward, -1 backward, 0 static)
    pub direction: i32,
    /// Last time we received a frame update
    pub last_update_time: i32,
    /// Estimated animation speed (frames per second)
    pub anim_speed: f32,
    /// Whether we're actively animating
    pub animating: bool,
}

impl Default for WeaponAnimSmoothing {
    fn default() -> Self {
        Self {
            enabled: true,
            current_frame: 0.0,
            target_frame: 0,
            prev_frame: 0,
            direction: 0,
            last_update_time: 0,
            anim_speed: 10.0, // Default 10 fps weapon animation
            animating: false,
        }
    }
}

impl WeaponAnimSmoothing {
    /// Update with new frame from server
    pub fn update(&mut self, new_frame: i32, current_time: i32) {
        if !self.enabled {
            self.current_frame = new_frame as f32;
            self.target_frame = new_frame;
            return;
        }

        // Detect frame change
        if new_frame != self.target_frame {
            self.prev_frame = self.target_frame;
            self.target_frame = new_frame;

            // Determine animation direction
            if new_frame == 0 && self.prev_frame > 10 {
                // Weapon switch (frame reset to 0 from high frame)
                self.direction = 0;
                self.current_frame = 0.0;
                self.animating = false;
            } else if new_frame > self.prev_frame {
                self.direction = 1;
                self.animating = true;
            } else if new_frame < self.prev_frame {
                self.direction = -1;
                self.animating = true;
            } else {
                self.direction = 0;
                self.animating = false;
            }

            // Calculate animation speed from frame change timing
            let dt = if self.last_update_time > 0 {
                ((current_time - self.last_update_time) as f32) / 1000.0
            } else {
                0.1 // Default 100ms
            };

            if dt > 0.01 && dt < 0.5 {
                // Estimate frames per second
                let frame_diff = (new_frame - self.prev_frame).abs() as f32;
                self.anim_speed = (frame_diff / dt).clamp(5.0, 30.0);
            }
        }

        self.last_update_time = current_time;
    }

    /// Continue animation during packet loss
    pub fn continue_animation(&mut self, current_time: i32) {
        if !self.enabled || !self.animating {
            return;
        }

        let dt = if self.last_update_time > 0 {
            ((current_time - self.last_update_time) as f32) / 1000.0
        } else {
            0.0
        };

        // Limit extrapolation time to prevent runaway animation
        if dt > 0.5 {
            self.animating = false;
            return;
        }

        // Advance frame based on direction and speed
        self.current_frame += (self.direction as f32) * self.anim_speed * dt;

        // Don't go past target frame
        if self.direction > 0 && self.current_frame > self.target_frame as f32 {
            self.current_frame = self.target_frame as f32;
        } else if self.direction < 0 && self.current_frame < self.target_frame as f32 {
            self.current_frame = self.target_frame as f32;
        }

        self.last_update_time = current_time;
    }

    /// Get the smoothed backlerp value for rendering
    /// Returns (frame, oldframe, backlerp)
    pub fn get_smooth_frames(&self) -> (i32, i32, f32) {
        if !self.enabled {
            return (self.target_frame, self.prev_frame, 0.0);
        }

        let frame = self.current_frame.floor() as i32;
        let oldframe = if frame > 0 { frame - 1 } else { 0 };
        let backlerp = 1.0 - (self.current_frame - self.current_frame.floor());

        (frame.max(0), oldframe.max(0), backlerp.clamp(0.0, 1.0))
    }

    /// Clear state (weapon switch, etc)
    pub fn clear(&mut self) {
        self.current_frame = 0.0;
        self.target_frame = 0;
        self.prev_frame = 0;
        self.direction = 0;
        self.animating = false;
    }
}

// ============================================================
// Footstep Prediction
// ============================================================

/// Per-entity footstep prediction state
#[derive(Debug, Clone, Default)]
pub struct EntityFootstepState {
    /// Whether this entity is being tracked
    pub active: bool,
    /// Last footstep time (client time ms)
    pub last_footstep_time: i32,
    /// Last known origin
    pub last_origin: Vec3,
    /// Accumulated distance since last footstep
    pub distance_accumulator: f32,
    /// Whether a footstep was predicted but not yet confirmed
    pub pending_prediction: bool,
    /// Time of pending prediction (for timeout/cleanup)
    pub pending_time: i32,
}

/// Client-side footstep prediction for other players.
/// Predicts footstep sounds based on player movement to fill gaps during packet loss.
#[derive(Debug, Clone)]
pub struct FootstepPrediction {
    /// Whether prediction is enabled
    pub enabled: bool,
    /// Per-entity footstep states (indexed by entity number)
    pub entities: Vec<EntityFootstepState>,
    /// Distance between footsteps (units) - typically 64 for Q2
    pub step_distance: f32,
    /// Minimum speed to generate footsteps (units/sec)
    pub min_speed: f32,
    /// Maximum prediction window (ms) - don't predict too far ahead
    pub max_predict_ms: i32,
}

impl Default for FootstepPrediction {
    fn default() -> Self {
        Self {
            enabled: true,
            entities: Vec::new(),
            step_distance: 64.0,   // Distance between footsteps
            min_speed: 100.0,      // Minimum walking speed
            max_predict_ms: 500,   // Don't predict more than 500ms ahead
        }
    }
}

impl FootstepPrediction {
    /// Create with specified entity capacity
    pub fn new(max_entities: usize) -> Self {
        let mut entities = Vec::with_capacity(max_entities);
        for _ in 0..max_entities {
            entities.push(EntityFootstepState::default());
        }
        Self {
            enabled: true,
            entities,
            step_distance: 64.0,
            min_speed: 100.0,
            max_predict_ms: 500,
        }
    }

    /// Update entity position and check if footstep should be predicted.
    /// Returns true if a footstep sound should be played for this entity.
    pub fn update_entity(
        &mut self,
        entity_num: usize,
        origin: &Vec3,
        current_time: i32,
        is_on_ground: bool,
    ) -> bool {
        if !self.enabled || entity_num >= self.entities.len() {
            return false;
        }

        let state = &mut self.entities[entity_num];

        // If not previously active, initialize
        if !state.active {
            state.active = true;
            state.last_origin = *origin;
            state.last_footstep_time = current_time;
            state.distance_accumulator = 0.0;
            return false;
        }

        // Calculate horizontal distance moved (ignore vertical for footsteps)
        let dx = origin[0] - state.last_origin[0];
        let dy = origin[1] - state.last_origin[1];
        let distance = (dx * dx + dy * dy).sqrt();

        // Update last origin
        state.last_origin = *origin;

        // Only accumulate distance if on ground
        if !is_on_ground {
            return false;
        }

        // Accumulate distance
        state.distance_accumulator += distance;

        // Check if we've traveled enough for a footstep
        if state.distance_accumulator >= self.step_distance {
            state.distance_accumulator = state.distance_accumulator % self.step_distance;
            state.last_footstep_time = current_time;
            state.pending_prediction = true;
            state.pending_time = current_time;
            return true;
        }

        false
    }

    /// Check if we should predict footsteps for an entity during packet loss.
    /// Uses velocity and time since last update to estimate footsteps.
    pub fn predict_during_loss(
        &mut self,
        entity_num: usize,
        velocity: &Vec3,
        current_time: i32,
        time_since_update_ms: i32,
    ) -> bool {
        if !self.enabled || entity_num >= self.entities.len() {
            return false;
        }

        // Don't predict too far ahead
        if time_since_update_ms > self.max_predict_ms {
            return false;
        }

        let state = &mut self.entities[entity_num];

        // Calculate horizontal speed
        let speed = (velocity[0] * velocity[0] + velocity[1] * velocity[1]).sqrt();
        if speed < self.min_speed {
            return false;
        }

        // Estimate distance traveled since last footstep
        let time_since_step = current_time - state.last_footstep_time;
        if time_since_step <= 0 {
            return false;
        }

        // Calculate expected step interval based on speed
        let step_interval_ms = (self.step_distance / speed * 1000.0) as i32;
        if step_interval_ms <= 0 {
            return false;
        }

        // Check if it's time for a predicted footstep
        if time_since_step >= step_interval_ms {
            state.last_footstep_time = current_time;
            state.pending_prediction = true;
            state.pending_time = current_time;
            return true;
        }

        false
    }

    /// Confirm a server-sent footstep event, clearing any pending prediction
    pub fn confirm_footstep(&mut self, entity_num: usize, current_time: i32) {
        if entity_num >= self.entities.len() {
            return;
        }

        let state = &mut self.entities[entity_num];
        state.last_footstep_time = current_time;
        state.pending_prediction = false;
        state.distance_accumulator = 0.0;
    }

    /// Clear tracking for an entity (e.g., player disconnected)
    pub fn clear_entity(&mut self, entity_num: usize) {
        if entity_num < self.entities.len() {
            self.entities[entity_num] = EntityFootstepState::default();
        }
    }

    /// Clear all entity tracking
    pub fn clear(&mut self) {
        for state in self.entities.iter_mut() {
            *state = EntityFootstepState::default();
        }
    }
}

// ============================================================
// Player Animation Continuation
// ============================================================

/// Per-entity player animation state for smooth continuation during packet loss
#[derive(Debug, Clone, Default)]
pub struct PlayerAnimContinuation {
    /// Last known animation frame
    pub last_frame: i32,
    /// Estimated frames per second (from velocity)
    pub estimated_fps: f32,
    /// Last update time
    pub last_update_time: i32,
    /// Last known horizontal speed (for animation speed estimation)
    pub last_speed: f32,
    /// Whether we're actively continuing animation
    pub continuing: bool,
}

/// Manages player animation continuation for all entities
#[derive(Debug, Clone)]
pub struct PlayerAnimSmoothing {
    /// Whether animation continuation is enabled
    pub enabled: bool,
    /// Per-entity animation states
    pub entities: Vec<PlayerAnimContinuation>,
    /// Animation FPS for walking (around 10 fps)
    pub walk_fps: f32,
    /// Animation FPS for running (around 15 fps)
    pub run_fps: f32,
    /// Speed threshold for running (units/sec)
    pub run_threshold: f32,
}

impl Default for PlayerAnimSmoothing {
    fn default() -> Self {
        Self {
            enabled: true,
            entities: Vec::new(),
            walk_fps: 10.0,   // Walk cycle ~10 fps
            run_fps: 15.0,    // Run cycle ~15 fps
            run_threshold: 200.0, // Above 200 u/s is running
        }
    }
}

impl PlayerAnimSmoothing {
    /// Create with specified entity capacity
    pub fn new(max_entities: usize) -> Self {
        let mut entities = Vec::with_capacity(max_entities);
        for _ in 0..max_entities {
            entities.push(PlayerAnimContinuation::default());
        }
        Self {
            enabled: true,
            entities,
            walk_fps: 10.0,
            run_fps: 15.0,
            run_threshold: 200.0,
        }
    }

    /// Update animation state from server frame
    pub fn update_from_server(
        &mut self,
        entity_num: usize,
        frame: i32,
        velocity: &Vec3,
        current_time: i32,
    ) {
        if !self.enabled || entity_num >= self.entities.len() {
            return;
        }

        let state = &mut self.entities[entity_num];

        // Calculate horizontal speed
        let speed = (velocity[0] * velocity[0] + velocity[1] * velocity[1]).sqrt();

        // Estimate animation FPS based on speed
        state.estimated_fps = if speed < 50.0 {
            0.0 // Standing still
        } else if speed < self.run_threshold {
            // Linear interpolation between 0 and walk_fps
            self.walk_fps * (speed / self.run_threshold)
        } else {
            // Linear interpolation between walk_fps and run_fps
            let run_factor = ((speed - self.run_threshold) / 100.0).min(1.0);
            self.walk_fps + (self.run_fps - self.walk_fps) * run_factor
        };

        state.last_frame = frame;
        state.last_speed = speed;
        state.last_update_time = current_time;
        state.continuing = false;
    }

    /// Continue animation during packet loss.
    /// Returns the predicted frame to use.
    pub fn continue_animation(
        &mut self,
        entity_num: usize,
        current_time: i32,
    ) -> Option<i32> {
        if !self.enabled || entity_num >= self.entities.len() {
            return None;
        }

        let state = &mut self.entities[entity_num];

        // Don't continue if standing still
        if state.estimated_fps < 0.5 {
            return None;
        }

        // Calculate time since last update
        let time_delta_ms = current_time - state.last_update_time;
        if time_delta_ms <= 0 || time_delta_ms > 1000 {
            return None; // Don't extrapolate more than 1 second
        }

        // Calculate frames to advance based on estimated FPS
        let time_delta_sec = time_delta_ms as f32 / 1000.0;
        let frames_to_advance = (time_delta_sec * state.estimated_fps) as i32;

        // Cap the advancement to prevent huge jumps
        let capped_advance = frames_to_advance.min(15);

        state.continuing = true;

        // Return predicted frame (wrap within typical animation cycle 0-39)
        Some((state.last_frame + capped_advance) % 40)
    }

    /// Clear animation state for an entity
    pub fn clear_entity(&mut self, entity_num: usize) {
        if entity_num < self.entities.len() {
            self.entities[entity_num] = PlayerAnimContinuation::default();
        }
    }

    /// Clear all entity tracking
    pub fn clear(&mut self) {
        for state in self.entities.iter_mut() {
            *state = PlayerAnimContinuation::default();
        }
    }
}

// ============================================================
// Combined Smoothing State
// ============================================================

/// Combined state for all smoothing features
#[derive(Debug, Clone, Default)]
pub struct SmoothingState {
    pub adaptive_interp: AdaptiveInterpolation,
    pub view_smoothing: ViewSmoothing,
    /// Separate smoothing for weapon/gun model (faster settings for responsiveness)
    pub weapon_smoothing: ViewSmoothing,
    pub weapon_prediction: WeaponPrediction,
    pub network_stats: NetworkStats,
    pub priority_system: EntityPrioritySystem,
    pub input_buffer: InputBuffer,
    /// Per-entity dead reckoning states (indexed by entity number)
    pub dead_reckoning: Vec<DeadReckoningState>,
    /// Per-entity spline histories (indexed by entity number)
    pub spline_histories: Vec<SplineHistory>,
    /// Whether cubic interpolation is enabled
    pub cubic_interp_enabled: bool,
    /// Prediction error smoothing
    pub prediction_error: PredictionErrorSmoothing,
    /// Frame time smoothing
    pub frame_time: FrameTimeSmoothing,
    /// Effect/particle continuation
    pub effect_continuation: EffectContinuation,
    /// Snapshot buffering
    pub snapshot_buffer: SnapshotBuffer,
    /// Weapon recoil/kick smoothing
    pub recoil_smoothing: RecoilSmoothing,
    /// Entity removal fadeout configuration
    pub entity_fadeout: EntityFadeout,
    /// Entity spawn fade-in configuration
    pub entity_fadein: EntityFadein,
    /// Moving brush/platform velocity prediction
    pub mover_prediction: MoverPrediction,
    /// View bob continuation during packet loss
    pub view_bob: ViewBobContinuation,
    /// Weapon animation smoothing for sub-frame interpolation
    pub weapon_anim: WeaponAnimSmoothing,
    /// Screen blend smoothing (damage flash, powerups)
    pub screen_blend: ScreenBlendSmoothing,
    /// Item rotation/bobbing continuation
    pub item_rotation: ItemRotationSmoothing,
    /// Dynamic light interpolation
    pub dynamic_lights: DynamicLightSmoothing,
    /// Bandwidth adaptation based on network conditions
    pub bandwidth_adapter: BandwidthAdapter,
    /// Client-side footstep prediction for other players
    pub footstep_prediction: FootstepPrediction,
    /// Player animation continuation during packet loss
    pub player_anim: PlayerAnimSmoothing,
    /// Weapon sway continuation during packet loss
    pub weapon_sway: WeaponSway,
}

// ============================================================
// View Bob Continuation
// ============================================================

/// Continues view bob motion during packet loss for smoother feel
#[derive(Debug, Clone)]
pub struct ViewBobContinuation {
    /// Whether bob continuation is enabled
    pub enabled: bool,
    /// Current bob phase (0.0 to 2*PI)
    pub phase: f32,
    /// Bob frequency (cycles per second)
    pub frequency: f32,
    /// Vertical bob amplitude
    pub amplitude_y: f32,
    /// Roll bob amplitude
    pub amplitude_roll: f32,
    /// Last player velocity magnitude (for determining if moving)
    pub last_speed: f32,
    /// Last update time
    pub last_update_time: i32,
    /// Whether bob is currently active (player was moving)
    pub active: bool,
}

impl Default for ViewBobContinuation {
    fn default() -> Self {
        Self {
            enabled: true,
            phase: 0.0,
            frequency: 1.8,      // ~2 cycles per second (walking pace)
            amplitude_y: 0.5,    // Small vertical bob
            amplitude_roll: 0.3, // Slight roll
            last_speed: 0.0,
            last_update_time: 0,
            active: false,
        }
    }
}

impl ViewBobContinuation {
    /// Update bob state based on current velocity
    pub fn update(&mut self, velocity: &Vec3, current_time: i32) {
        if !self.enabled {
            return;
        }

        // Calculate horizontal speed
        let speed = (velocity[0] * velocity[0] + velocity[1] * velocity[1]).sqrt();

        // Determine if player is moving enough to bob
        let moving_threshold = 50.0; // units/sec
        self.active = speed > moving_threshold;

        if self.active {
            self.last_speed = speed;

            // Advance phase based on time delta
            let dt = if self.last_update_time > 0 {
                ((current_time - self.last_update_time) as f32) / 1000.0
            } else {
                0.0
            };

            // Speed affects frequency slightly
            let speed_factor = (speed / 200.0).clamp(0.8, 1.5);
            self.phase += 2.0 * std::f32::consts::PI * self.frequency * speed_factor * dt;

            // Keep phase in valid range
            if self.phase > 2.0 * std::f32::consts::PI {
                self.phase -= 2.0 * std::f32::consts::PI;
            }
        }

        self.last_update_time = current_time;
    }

    /// Continue bob during packet loss (extrapolate forward)
    pub fn continue_bob(&mut self, current_time: i32) {
        if !self.enabled || !self.active {
            return;
        }

        let dt = if self.last_update_time > 0 {
            ((current_time - self.last_update_time) as f32) / 1000.0
        } else {
            0.0
        };

        // Continue phase advancement at last known speed
        let speed_factor = (self.last_speed / 200.0).clamp(0.8, 1.5);
        self.phase += 2.0 * std::f32::consts::PI * self.frequency * speed_factor * dt;

        if self.phase > 2.0 * std::f32::consts::PI {
            self.phase -= 2.0 * std::f32::consts::PI;
        }

        self.last_update_time = current_time;
    }

    /// Get the current bob offset and roll
    /// Returns (vertical_offset, roll_angle)
    pub fn get_bob(&self) -> (f32, f32) {
        if !self.enabled || !self.active {
            return (0.0, 0.0);
        }

        let bob_y = self.amplitude_y * self.phase.sin();
        let bob_roll = self.amplitude_roll * (self.phase * 0.5).sin();

        (bob_y, bob_roll)
    }

    /// Clear bob state
    pub fn clear(&mut self) {
        self.phase = 0.0;
        self.last_speed = 0.0;
        self.active = false;
        self.last_update_time = 0;
    }
}

// ============================================================
// Weapon Sway Continuation
// ============================================================

/// Weapon sway continuation during packet loss.
/// Tracks the gun's inertial movement based on player movement and view rotation.
#[derive(Debug, Clone)]
pub struct WeaponSway {
    /// Whether sway is enabled
    pub enabled: bool,
    /// Current sway offset (x=lateral, y=vertical, z=forward)
    pub sway_offset: Vec3,
    /// Current sway velocity for momentum
    pub sway_velocity: Vec3,
    /// Last known player velocity
    pub last_velocity: Vec3,
    /// Last known view angles
    pub last_angles: Vec3,
    /// Last update time
    pub last_update_time: i32,
    /// Sway amount multiplier
    pub sway_scale: f32,
    /// Sway spring stiffness (return to center)
    pub spring_stiffness: f32,
    /// Sway damping (velocity decay)
    pub damping: f32,
    /// Maximum sway offset
    pub max_offset: f32,
}

impl Default for WeaponSway {
    fn default() -> Self {
        Self {
            enabled: true,
            sway_offset: [0.0; 3],
            sway_velocity: [0.0; 3],
            last_velocity: [0.0; 3],
            last_angles: [0.0; 3],
            last_update_time: 0,
            sway_scale: 0.003,       // How much velocity affects sway
            spring_stiffness: 15.0,  // Return-to-center spring
            damping: 0.85,           // Velocity decay per second
            max_offset: 2.0,         // Maximum sway offset in units
        }
    }
}

impl WeaponSway {
    /// Update sway from player movement and view rotation
    pub fn update(&mut self, velocity: &Vec3, angles: &Vec3, current_time: i32) {
        if !self.enabled {
            return;
        }

        let dt = if self.last_update_time > 0 {
            ((current_time - self.last_update_time) as f32 / 1000.0).clamp(0.0, 0.1)
        } else {
            0.0
        };

        if dt > 0.0 {
            // Calculate view rotation delta for sway impulse
            let yaw_delta = (angles[1] - self.last_angles[1]).clamp(-45.0, 45.0);
            let pitch_delta = (angles[0] - self.last_angles[0]).clamp(-45.0, 45.0);

            // Add impulse from velocity changes
            let vel_x_change = velocity[0] - self.last_velocity[0];
            let vel_y_change = velocity[1] - self.last_velocity[1];

            // Apply impulses: strafe -> lateral, forward -> vertical, rotation -> angular
            self.sway_velocity[0] += vel_x_change * self.sway_scale * 0.5;  // Lateral from strafe
            self.sway_velocity[1] += vel_y_change * self.sway_scale * 0.3;  // Vertical from move
            self.sway_velocity[0] += yaw_delta * 0.02;                       // Lateral from rotation
            self.sway_velocity[1] -= pitch_delta * 0.015;                    // Vertical from pitch

            // Apply spring force (return to center)
            for i in 0..3 {
                self.sway_velocity[i] -= self.sway_offset[i] * self.spring_stiffness * dt;
            }

            // Apply damping
            let damping_factor = self.damping.powf(dt * 60.0);
            for i in 0..3 {
                self.sway_velocity[i] *= damping_factor;
            }

            // Integrate velocity to position
            for i in 0..3 {
                self.sway_offset[i] += self.sway_velocity[i] * dt;
                // Clamp to max offset
                self.sway_offset[i] = self.sway_offset[i].clamp(-self.max_offset, self.max_offset);
            }
        }

        self.last_velocity = *velocity;
        self.last_angles = *angles;
        self.last_update_time = current_time;
    }

    /// Continue sway during packet loss - maintain momentum
    pub fn continue_during_packet_loss(&mut self, current_time: i32) {
        if !self.enabled {
            return;
        }

        let dt = if self.last_update_time > 0 {
            ((current_time - self.last_update_time) as f32 / 1000.0).clamp(0.0, 0.1)
        } else {
            0.0
        };

        if dt > 0.0 {
            // Apply spring force (gradually return to center)
            for i in 0..3 {
                self.sway_velocity[i] -= self.sway_offset[i] * self.spring_stiffness * dt;
            }

            // Apply stronger damping during packet loss (settle down)
            let damping_factor = (self.damping * 0.9).powf(dt * 60.0);
            for i in 0..3 {
                self.sway_velocity[i] *= damping_factor;
            }

            // Integrate velocity to position
            for i in 0..3 {
                self.sway_offset[i] += self.sway_velocity[i] * dt;
                self.sway_offset[i] = self.sway_offset[i].clamp(-self.max_offset, self.max_offset);
            }
        }

        self.last_update_time = current_time;
    }

    /// Get current sway offset to apply to gun position
    pub fn get_offset(&self) -> Vec3 {
        if self.enabled {
            self.sway_offset
        } else {
            [0.0; 3]
        }
    }

    /// Clear sway state
    pub fn clear(&mut self) {
        self.sway_offset = [0.0; 3];
        self.sway_velocity = [0.0; 3];
        self.last_velocity = [0.0; 3];
        self.last_angles = [0.0; 3];
        self.last_update_time = 0;
    }
}

impl SmoothingState {
    pub fn new(max_entities: usize) -> Self {
        let mut dead_reckoning = Vec::with_capacity(max_entities);
        let mut spline_histories = Vec::with_capacity(max_entities);

        for _ in 0..max_entities {
            dead_reckoning.push(DeadReckoningState::new());
            spline_histories.push(SplineHistory::new(8));
        }

        // Create weapon smoothing with faster settings for responsiveness
        let mut weapon_smoothing = ViewSmoothing::new();
        weapon_smoothing.max_origin_speed = 800.0;  // Faster than view (800 units/sec)
        weapon_smoothing.max_angle_speed = 360.0;   // Much faster rotation (360 deg/sec)

        Self {
            dead_reckoning,
            spline_histories,
            cubic_interp_enabled: true,
            view_smoothing: ViewSmoothing::new(),
            weapon_smoothing,
            network_stats: NetworkStats::new(),
            mover_prediction: MoverPrediction::new(max_entities),
            item_rotation: ItemRotationSmoothing::new(max_entities),
            dynamic_lights: DynamicLightSmoothing::new(max_entities),
            footstep_prediction: FootstepPrediction::new(max_entities),
            player_anim: PlayerAnimSmoothing::new(max_entities),
            ..Default::default()
        }
    }

    /// Reset all smoothing state (call on map change/disconnect)
    pub fn reset(&mut self) {
        self.adaptive_interp.reset();
        self.view_smoothing.reset();
        self.weapon_smoothing.reset();
        self.weapon_prediction.clear();
        self.network_stats.reset();
        self.input_buffer.clear();
        self.prediction_error.clear();
        self.frame_time.clear();
        self.effect_continuation.clear();
        self.snapshot_buffer.clear();
        self.recoil_smoothing.clear();
        self.mover_prediction.clear();
        self.view_bob.clear();
        self.weapon_anim.clear();
        self.entity_fadein = EntityFadein::default();
        self.screen_blend.clear();
        self.item_rotation.clear();
        self.dynamic_lights.clear();
        self.bandwidth_adapter.reset();
        self.footstep_prediction.clear();
        self.player_anim.clear();
        self.weapon_sway.clear();

        for dr in self.dead_reckoning.iter_mut() {
            *dr = DeadReckoningState::new();
        }
        for sh in self.spline_histories.iter_mut() {
            sh.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adaptive_interpolation() {
        let mut ai = AdaptiveInterpolation::default();

        // Simulate steady packets
        for i in 0..20 {
            ai.record_packet(i * 100);
        }

        // Should have stable buffer near 100ms
        assert!(ai.target_buffer_ms >= ai.min_buffer_ms);
        assert!(ai.target_buffer_ms <= ai.max_buffer_ms);
    }

    #[test]
    fn test_catmull_rom() {
        // Test that interpolation passes through control points
        let result = catmull_rom_interpolate(0.0, 1.0, 2.0, 3.0, 0.0);
        assert!((result - 1.0).abs() < 0.01);

        let result = catmull_rom_interpolate(0.0, 1.0, 2.0, 3.0, 1.0);
        assert!((result - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_dead_reckoning() {
        let mut dr = DeadReckoningState::new();

        dr.update([0.0, 0.0, 0.0], 0);
        dr.update([100.0, 0.0, 0.0], 100);

        // Should predict forward movement
        let predicted = dr.predict(150, 800.0);
        assert!(predicted[0] > 100.0);
    }

    #[test]
    fn test_view_smoothing() {
        let mut vs = ViewSmoothing::new();

        let (origin, angles) = vs.update(&[0.0, 0.0, 0.0], &[0.0, 0.0, 0.0], 0.016);

        // First update should initialize
        assert_eq!(origin, [0.0, 0.0, 0.0]);

        // Large jump should be clamped
        let (origin2, _) = vs.update(&[1000.0, 0.0, 0.0], &[0.0, 0.0, 0.0], 0.016);
        assert!(origin2[0] < 1000.0); // Should not snap immediately
    }
}
