// sv_world.rs -- world query functions
// Converted from: myq2-original/server/sv_world.c
//
// Entity area checking: spatial partitioning via area nodes,
// linking/unlinking entities, box queries, point contents, and tracing.

use myq2_common::common::{com_printf, com_dprintf};
use myq2_common::q_shared::*;
use myq2_common::qfiles::MAX_MAP_AREAS;
use std::sync::Mutex;

static SV_WORLD_CTX: Mutex<Option<SvWorldContext>> = Mutex::new(None);

/// Access the global SvWorldContext via a closure.
pub fn with_sv_world_ctx<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut SvWorldContext) -> R,
{
    let mut guard = SV_WORLD_CTX.lock().unwrap();
    guard.as_mut().map(f)
}

/// Initialize the global world context.
pub fn init_sv_world_ctx() {
    *SV_WORLD_CTX.lock().unwrap() = Some(SvWorldContext::new());
}

// ===============================================================================
// ENTITY AREA CHECKING
// ===============================================================================

const AREA_DEPTH: i32 = 4;
const AREA_NODES: usize = 32;
const MAX_TOTAL_ENT_LEAFS: usize = 128;
// MAX_ENT_CLUSTERS comes from myq2_common::q_shared::* (imported above)

// ============================================================
// Server Edict and solid/SVF types (from sv_game, canonical location)
// ============================================================

use crate::sv_game::{Edict, Solid};
pub use myq2_game::game::{SVF_NOCLIENT, SVF_DEADMONSTER, SVF_MONSTER};

// ============================================================
// Area node (spatial partitioning BSP for entities)
// ============================================================

#[derive(Debug, Clone)]
pub struct AreaNode {
    pub axis: i32,       // -1 = leaf node
    pub dist: f32,
    pub children: [usize; 2], // indices into SvWorldContext::areanodes
    pub trigger_edicts: Vec<usize>, // edict indices
    pub solid_edicts: Vec<usize>,   // edict indices
}

impl Default for AreaNode {
    fn default() -> Self {
        Self {
            axis: -1,
            dist: 0.0,
            children: [usize::MAX; 2],
            trigger_edicts: Vec::new(),
            solid_edicts: Vec::new(),
        }
    }
}

use crate::server::ServerState;

// ============================================================
// MoveClip — internal trace structure
// ============================================================

struct MoveClip {
    boxmins: Vec3,
    boxmaxs: Vec3,
    mins: Vec3,
    maxs: Vec3,
    mins2: Vec3,
    maxs2: Vec3,
    start: Vec3,
    end: Vec3,
    trace: Trace,
    passedict: i32,     // edict index, -1 = none
    contentmask: i32,
}

impl Default for MoveClip {
    fn default() -> Self {
        Self {
            boxmins: [0.0; 3],
            boxmaxs: [0.0; 3],
            mins: [0.0; 3],
            maxs: [0.0; 3],
            mins2: [0.0; 3],
            maxs2: [0.0; 3],
            start: [0.0; 3],
            end: [0.0; 3],
            trace: Trace::default(),
            passedict: -1,
            contentmask: 0,
        }
    }
}

// ============================================================
// Collision model callbacks trait
// ============================================================

/// Trait abstracting the collision model (CM) functions that sv_world needs.
/// Allows testing and decoupling from the actual CM implementation.
///
/// The `Sync` supertrait requirement enables parallel entity visibility checks
/// in sv_build_client_frame, where CM methods are called from multiple threads.
pub trait CollisionModel: Sync {
    fn box_leafnums(
        &self,
        mins: &Vec3,
        maxs: &Vec3,
        list: &mut [i32],
        list_size: usize,
        topnode: &mut i32,
    ) -> i32;
    fn leaf_cluster(&self, leafnum: i32) -> i32;
    fn leaf_area(&self, leafnum: i32) -> i32;
    fn point_contents(&self, p: &Vec3, headnode: i32) -> i32;
    fn transformed_point_contents(
        &self,
        p: &Vec3,
        headnode: i32,
        origin: &Vec3,
        angles: &Vec3,
    ) -> i32;
    fn headnode_for_box(&self, mins: &Vec3, maxs: &Vec3) -> i32;
    fn box_trace(
        &self,
        start: &Vec3,
        end: &Vec3,
        mins: &Vec3,
        maxs: &Vec3,
        headnode: i32,
        brushmask: i32,
    ) -> Trace;
    fn transformed_box_trace(
        &self,
        start: &Vec3,
        end: &Vec3,
        mins: &Vec3,
        maxs: &Vec3,
        headnode: i32,
        brushmask: i32,
        origin: &Vec3,
        angles: &Vec3,
    ) -> Trace;

    // --- Additional methods needed by sv_ents.rs ---

    fn num_clusters(&self) -> i32;
    fn cluster_pvs(&self, cluster: i32) -> &[u8];
    fn cluster_phs(&self, cluster: i32) -> &[u8];
    fn point_leafnum(&self, p: &Vec3) -> i32;
    fn write_area_bits(&self, area: i32) -> (i32, [u8; MAX_MAP_AREAS / 8]);
    fn areas_connected(&self, area1: i32, area2: i32) -> bool;
    fn headnode_visible(&self, headnode: i32, bitvector: &[u8]) -> bool;
}

// ============================================================
// SvWorldContext — holds all former C globals
// ============================================================

pub struct SvWorldContext {
    pub areanodes: Vec<AreaNode>,
    pub numareanodes: usize,
}

impl Default for SvWorldContext {
    fn default() -> Self {
        Self::new()
    }
}

impl SvWorldContext {
    pub fn new() -> Self {
        Self {
            areanodes: Vec::new(),
            numareanodes: 0,
        }
    }

    // ================================================================
    // SV_CreateAreaNode
    //
    // Builds a uniformly subdivided tree for the given world size.
    // Returns the index of the created node.
    // ================================================================
    fn create_area_node(&mut self, depth: i32, mins: &Vec3, maxs: &Vec3) -> usize {
        let anode_idx = self.numareanodes;
        self.numareanodes += 1;

        if anode_idx >= self.areanodes.len() {
            self.areanodes.push(AreaNode::default());
        } else {
            self.areanodes[anode_idx] = AreaNode::default();
        }

        if depth == AREA_DEPTH {
            self.areanodes[anode_idx].axis = -1;
            self.areanodes[anode_idx].children = [usize::MAX; 2];
            return anode_idx;
        }

        let size = vector_subtract(maxs, mins);
        if size[0] > size[1] {
            self.areanodes[anode_idx].axis = 0;
        } else {
            self.areanodes[anode_idx].axis = 1;
        }

        let axis = self.areanodes[anode_idx].axis as usize;
        let dist = 0.5 * (maxs[axis] + mins[axis]);
        self.areanodes[anode_idx].dist = dist;

        let mins1 = *mins;
        let mut mins2 = *mins;
        let mut maxs1 = *maxs;
        let maxs2 = *maxs;

        maxs1[axis] = dist;
        mins2[axis] = dist;

        let child0 = self.create_area_node(depth + 1, &mins2, &maxs2);
        let child1 = self.create_area_node(depth + 1, &mins1, &maxs1);

        self.areanodes[anode_idx].children[0] = child0;
        self.areanodes[anode_idx].children[1] = child1;

        anode_idx
    }

    // ================================================================
    // SV_ClearWorld
    // ================================================================
    pub fn clear_world(&mut self, world_mins: &Vec3, world_maxs: &Vec3) {
        self.areanodes.clear();
        self.areanodes.resize(AREA_NODES, AreaNode::default());
        self.numareanodes = 0;
        self.create_area_node(0, world_mins, world_maxs);
    }

    // ================================================================
    // SV_UnlinkEdict
    // ================================================================
    pub fn unlink_edict(&mut self, edicts: &mut [Edict], ent_idx: usize) {
        if !edicts[ent_idx].area_linked {
            return; // not linked in anywhere
        }

        let node_idx = edicts[ent_idx].area_node as usize;
        if node_idx < self.areanodes.len() {
            let node = &mut self.areanodes[node_idx];
            node.trigger_edicts.retain(|&e| e != ent_idx);
            node.solid_edicts.retain(|&e| e != ent_idx);
        }

        edicts[ent_idx].area_linked = false;
        edicts[ent_idx].area_node = -1;
    }

    // ================================================================
    // SV_LinkEdict
    // ================================================================
    pub fn link_edict(
        &mut self,
        edicts: &mut [Edict],
        ent_idx: usize,
        server_state: ServerState,
        cm: &dyn CollisionModel,
    ) {
        if edicts[ent_idx].area_linked {
            self.unlink_edict(edicts, ent_idx); // unlink from old position
        }

        if ent_idx == 0 {
            return; // don't add the world
        }

        if !edicts[ent_idx].inuse {
            return;
        }

        // set the size
        let ent = &mut edicts[ent_idx];
        ent.size = vector_subtract(&ent.maxs, &ent.mins);

        // encode the size into the entity_state for client prediction
        if ent.solid == Solid::Bbox && (ent.svflags & SVF_DEADMONSTER) == 0 {
            // assume that x/y are equal and symmetric
            let mut i = (ent.maxs[0] / 8.0) as i32;
            if i < 1 { i = 1; }
            if i > 31 { i = 31; }

            // z is not symmetric
            let mut j = ((-ent.mins[2]) / 8.0) as i32;
            if j < 1 { j = 1; }
            if j > 31 { j = 31; }

            // and z maxs can be negative...
            let mut k = ((ent.maxs[2] + 32.0) / 8.0) as i32;
            if k < 1 { k = 1; }
            if k > 63 { k = 63; }

            ent.s.solid = (k << 10) | (j << 5) | i;
        } else if ent.solid == Solid::Bsp {
            ent.s.solid = 31; // a solid_bbox will never create this value
        } else {
            ent.s.solid = 0;
        }

        // set the abs box
        if ent.solid == Solid::Bsp
            && (ent.s.angles[0] != 0.0 || ent.s.angles[1] != 0.0 || ent.s.angles[2] != 0.0)
        {
            // expand for rotation
            let mut max: f32 = 0.0;
            for i in 0..3 {
                let v = ent.mins[i].abs();
                if v > max {
                    max = v;
                }
                let v = ent.maxs[i].abs();
                if v > max {
                    max = v;
                }
            }
            for i in 0..3 {
                ent.absmin[i] = ent.s.origin[i] - max;
                ent.absmax[i] = ent.s.origin[i] + max;
            }
        } else {
            // normal
            ent.absmin = vector_add(&ent.s.origin, &ent.mins);
            ent.absmax = vector_add(&ent.s.origin, &ent.maxs);
        }

        // because movement is clipped an epsilon away from an actual edge,
        // we must fully check even when bounding boxes don't quite touch
        ent.absmin[0] -= 1.0;
        ent.absmin[1] -= 1.0;
        ent.absmin[2] -= 1.0;
        ent.absmax[0] += 1.0;
        ent.absmax[1] += 1.0;
        ent.absmax[2] += 1.0;

        // link to PVS leafs
        ent.num_clusters = 0;
        ent.areanum = 0;
        ent.areanum2 = 0;

        let absmin = ent.absmin;
        let absmax = ent.absmax;

        // get all leafs, including solids
        let mut leafs = [0i32; MAX_TOTAL_ENT_LEAFS];
        let mut topnode: i32 = 0;
        let num_leafs =
            cm.box_leafnums(&absmin, &absmax, &mut leafs, MAX_TOTAL_ENT_LEAFS, &mut topnode);

        // set areas
        let mut clusters = [0i32; MAX_TOTAL_ENT_LEAFS];
        for i in 0..num_leafs as usize {
            clusters[i] = cm.leaf_cluster(leafs[i]);
            let area = cm.leaf_area(leafs[i]);
            if area != 0 {
                let ent = &mut edicts[ent_idx];
                // doors may legally straddle two areas,
                // but nothing should ever need more than that
                if ent.areanum != 0 && ent.areanum != area {
                    if ent.areanum2 != 0
                        && ent.areanum2 != area
                        && server_state == ServerState::Loading
                    {
                        com_dprintf(&format!(
                            "Object touching 3 areas at {} {} {}\n",
                            ent.absmin[0], ent.absmin[1], ent.absmin[2]
                        ));
                    }
                    ent.areanum2 = area;
                } else {
                    ent.areanum = area;
                }
            }
        }

        let ent = &mut edicts[ent_idx];
        if num_leafs >= MAX_TOTAL_ENT_LEAFS as i32 {
            // assume we missed some leafs, and mark by headnode
            ent.num_clusters = -1;
            ent.headnode = topnode;
        } else {
            ent.num_clusters = 0;
            let mut done = false;
            for i in 0..num_leafs as usize {
                if clusters[i] == -1 {
                    continue; // not a visible leaf
                }
                let mut duplicate = false;
                for j in 0..i {
                    if clusters[j] == clusters[i] {
                        duplicate = true;
                        break;
                    }
                }
                if !duplicate {
                    if ent.num_clusters == MAX_ENT_CLUSTERS as i32 {
                        // assume we missed some leafs, and mark by headnode
                        ent.num_clusters = -1;
                        ent.headnode = topnode;
                        done = true;
                        break;
                    }
                    ent.clusternums[ent.num_clusters as usize] = clusters[i];
                    ent.num_clusters += 1;
                }
            }
            if done {
                // already handled above
            }
        }

        // if first time, make sure old_origin is valid
        let ent = &mut edicts[ent_idx];
        if ent.linkcount == 0 {
            ent.s.old_origin = ent.s.origin;
        }
        ent.linkcount += 1;

        if ent.solid == Solid::Not {
            return;
        }

        // find the first node that the ent's box crosses
        let ent_absmin = ent.absmin;
        let ent_absmax = ent.absmax;
        let ent_solid = ent.solid;

        let mut node_idx: usize = 0;
        loop {
            let node = &self.areanodes[node_idx];
            if node.axis == -1 {
                break;
            }
            if ent_absmin[node.axis as usize] > node.dist {
                node_idx = node.children[0];
            } else if ent_absmax[node.axis as usize] < node.dist {
                node_idx = node.children[1];
            } else {
                break; // crosses the node
            }
        }

        // link it in
        if ent_solid == Solid::Trigger {
            self.areanodes[node_idx].trigger_edicts.push(ent_idx);
        } else {
            self.areanodes[node_idx].solid_edicts.push(ent_idx);
        }

        edicts[ent_idx].area_linked = true;
        edicts[ent_idx].area_node = node_idx as i32;
    }

    // ================================================================
    // SV_AreaEdicts_r
    // ================================================================
    fn area_edicts_r(
        &self,
        node_idx: usize,
        edicts: &[Edict],
        area_mins: &Vec3,
        area_maxs: &Vec3,
        area_type: i32,
        area_list: &mut Vec<usize>,
        area_maxcount: usize,
    ) {
        let node = &self.areanodes[node_idx];

        // touch linked edicts
        let start = if area_type == AREA_SOLID {
            &node.solid_edicts
        } else {
            &node.trigger_edicts
        };

        for &check_idx in start.iter() {
            let check = &edicts[check_idx];

            if check.solid == Solid::Not {
                continue; // deactivated
            }
            if check.absmin[0] > area_maxs[0]
                || check.absmin[1] > area_maxs[1]
                || check.absmin[2] > area_maxs[2]
                || check.absmax[0] < area_mins[0]
                || check.absmax[1] < area_mins[1]
                || check.absmax[2] < area_mins[2]
            {
                continue; // not touching
            }

            if area_list.len() == area_maxcount {
                com_printf("SV_AreaEdicts: MAXCOUNT\n");
                return;
            }

            area_list.push(check_idx);
        }

        if node.axis == -1 {
            return; // terminal node
        }

        // recurse down both sides
        if area_maxs[node.axis as usize] > node.dist {
            self.area_edicts_r(
                node.children[0],
                edicts,
                area_mins,
                area_maxs,
                area_type,
                area_list,
                area_maxcount,
            );
        }
        if area_mins[node.axis as usize] < node.dist {
            self.area_edicts_r(
                node.children[1],
                edicts,
                area_mins,
                area_maxs,
                area_type,
                area_list,
                area_maxcount,
            );
        }
    }

    // ================================================================
    // SV_AreaEdicts
    // ================================================================
    pub fn area_edicts(
        &self,
        mins: &Vec3,
        maxs: &Vec3,
        edicts: &[Edict],
        maxcount: usize,
        areatype: i32,
    ) -> Vec<usize> {
        let mut list = Vec::new();
        if self.numareanodes > 0 {
            self.area_edicts_r(0, edicts, mins, maxs, areatype, &mut list, maxcount);
        }
        list
    }

    // ================================================================
    // SV_HullForEntity
    //
    // Returns a headnode that can be used for testing or clipping an
    // object of mins/maxs size.
    // ================================================================
    pub fn hull_for_entity(
        edicts: &[Edict],
        ent_idx: usize,
        models: &[Option<CModel>],
        cm: &dyn CollisionModel,
    ) -> i32 {
        let ent = &edicts[ent_idx];

        // decide which clipping hull to use, based on the size
        if ent.solid == Solid::Bsp {
            // explicit hulls in the BSP model
            let model_idx = ent.s.modelindex as usize;
            let model = models.get(model_idx).and_then(|m| m.as_ref());

            match model {
                Some(m) => return m.headnode,
                None => panic!("MOVETYPE_PUSH with a non bsp model"),
            }
        }

        // create a temp hull from bounding box sizes
        cm.headnode_for_box(&ent.mins, &ent.maxs)
    }

    // ================================================================
    // SV_PointContents
    // ================================================================
    pub fn point_contents(
        &self,
        p: &Vec3,
        edicts: &[Edict],
        models: &[Option<CModel>],
        cm: &dyn CollisionModel,
    ) -> i32 {
        // get base contents from world
        let world_headnode = models
            .get(1)
            .and_then(|m| m.as_ref())
            .map(|m| m.headnode)
            .unwrap_or(0);
        let mut contents = cm.point_contents(p, world_headnode);

        // or in contents from all the other entities
        let touch = self.area_edicts(p, p, edicts, MAX_EDICTS, AREA_SOLID);

        for &touch_idx in touch.iter() {
            let hit = &edicts[touch_idx];

            // might intersect, so do an exact clip
            let headnode = Self::hull_for_entity(edicts, touch_idx, models, cm);
            let angles = if hit.solid != Solid::Bsp {
                &vec3_origin // boxes don't rotate
            } else {
                &hit.s.angles
            };

            let c2 = cm.transformed_point_contents(p, headnode, &hit.s.origin, angles);
            contents |= c2;
        }

        contents
    }

    // ================================================================
    // SV_ClipMoveToEntities
    // ================================================================
    fn clip_move_to_entities(
        &self,
        clip: &mut MoveClip,
        edicts: &[Edict],
        models: &[Option<CModel>],
        cm: &dyn CollisionModel,
    ) {
        let touchlist =
            self.area_edicts(&clip.boxmins, &clip.boxmaxs, edicts, MAX_EDICTS, AREA_SOLID);

        // be careful, it is possible to have an entity in this
        // list removed before we get to it (killtriggered)
        for &touch_idx in touchlist.iter() {
            let touch = &edicts[touch_idx];

            if touch.solid == Solid::Not {
                continue;
            }
            if touch_idx as i32 == clip.passedict {
                continue;
            }
            if clip.trace.allsolid {
                return;
            }
            if clip.passedict >= 0 {
                let passedict = &edicts[clip.passedict as usize];
                if touch.owner_index == clip.passedict {
                    continue; // don't clip against own missiles
                }
                if passedict.owner_index == touch_idx as i32 {
                    continue; // don't clip against owner
                }
            }

            if (clip.contentmask & CONTENTS_DEADMONSTER) == 0
                && (touch.svflags & SVF_DEADMONSTER) != 0
            {
                continue;
            }

            // might intersect, so do an exact clip
            let headnode = Self::hull_for_entity(edicts, touch_idx, models, cm);
            let angles = if touch.solid != Solid::Bsp {
                &vec3_origin // boxes don't rotate
            } else {
                &touch.s.angles
            };

            let trace = if (touch.svflags & SVF_MONSTER) != 0 {
                cm.transformed_box_trace(
                    &clip.start,
                    &clip.end,
                    &clip.mins2,
                    &clip.maxs2,
                    headnode,
                    clip.contentmask,
                    &touch.s.origin,
                    angles,
                )
            } else {
                cm.transformed_box_trace(
                    &clip.start,
                    &clip.end,
                    &clip.mins,
                    &clip.maxs,
                    headnode,
                    clip.contentmask,
                    &touch.s.origin,
                    angles,
                )
            };

            if trace.allsolid || trace.startsolid || trace.fraction < clip.trace.fraction {
                let mut new_trace = trace;
                new_trace.ent_index = touch_idx as i32;
                if clip.trace.startsolid {
                    clip.trace = new_trace;
                    clip.trace.startsolid = true;
                } else {
                    clip.trace = new_trace;
                }
            } else if trace.startsolid {
                clip.trace.startsolid = true;
            }
        }
    }

    // ================================================================
    // SV_TraceBounds
    // ================================================================
    fn trace_bounds(
        start: &Vec3,
        mins: &Vec3,
        maxs: &Vec3,
        end: &Vec3,
        boxmins: &mut Vec3,
        boxmaxs: &mut Vec3,
    ) {
        for i in 0..3 {
            if end[i] > start[i] {
                boxmins[i] = start[i] + mins[i] - 1.0;
                boxmaxs[i] = end[i] + maxs[i] + 1.0;
            } else {
                boxmins[i] = end[i] + mins[i] - 1.0;
                boxmaxs[i] = start[i] + maxs[i] + 1.0;
            }
        }
    }

    // ================================================================
    // SV_Trace
    //
    // Moves the given mins/maxs volume through the world from start to end.
    // Passedict and edicts owned by passedict are explicitly not checked.
    // ================================================================
    pub fn trace(
        &self,
        start: &Vec3,
        mins: Option<&Vec3>,
        maxs: Option<&Vec3>,
        end: &Vec3,
        passedict: i32, // edict index, -1 = none
        contentmask: i32,
        edicts: &[Edict],
        models: &[Option<CModel>],
        cm: &dyn CollisionModel,
    ) -> Trace {
        let mins = mins.unwrap_or(&vec3_origin);
        let maxs = maxs.unwrap_or(&vec3_origin);

        let mut clip = MoveClip::default();

        // clip to world
        clip.trace = cm.box_trace(start, end, mins, maxs, 0, contentmask);
        clip.trace.ent_index = 0; // world entity
        if clip.trace.fraction == 0.0 {
            return clip.trace; // blocked by the world
        }

        clip.contentmask = contentmask;
        clip.start = *start;
        clip.end = *end;
        clip.mins = *mins;
        clip.maxs = *maxs;
        clip.passedict = passedict;

        clip.mins2 = *mins;
        clip.maxs2 = *maxs;

        // create the bounding box of the entire move
        Self::trace_bounds(
            start,
            &clip.mins2,
            &clip.maxs2,
            end,
            &mut clip.boxmins,
            &mut clip.boxmaxs,
        );

        // clip to other solid entities
        self.clip_move_to_entities(&mut clip, edicts, models, cm);

        clip.trace
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sv_game::Edict;
    use myq2_common::qfiles::MAX_MAP_AREAS;

    fn make_edicts(n: usize) -> Vec<Edict> {
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            v.push(Edict::default());
        }
        v
    }

    // =========================================================================
    // Mock CollisionModel for sv_world tests
    // =========================================================================

    struct MockCM {
        /// Simulated leafnums returned by box_leafnums
        leafnums: Vec<i32>,
        /// Cluster for each leaf (indexed by leafnum)
        clusters: Vec<i32>,
        /// Area for each leaf (indexed by leafnum)
        areas: Vec<i32>,
        /// Whether areas are always connected
        areas_always_connected: bool,
    }

    impl MockCM {
        fn simple() -> Self {
            Self {
                leafnums: vec![0, 1],
                clusters: vec![0, 1, 2, 3],
                areas: vec![1, 1, 1, 1],
                areas_always_connected: true,
            }
        }
    }

    impl CollisionModel for MockCM {
        fn box_leafnums(
            &self,
            _mins: &Vec3,
            _maxs: &Vec3,
            list: &mut [i32],
            list_size: usize,
            topnode: &mut i32,
        ) -> i32 {
            let count = self.leafnums.len().min(list_size);
            for i in 0..count {
                list[i] = self.leafnums[i];
            }
            *topnode = 0;
            count as i32
        }

        fn leaf_cluster(&self, leafnum: i32) -> i32 {
            self.clusters.get(leafnum as usize).copied().unwrap_or(-1)
        }

        fn leaf_area(&self, leafnum: i32) -> i32 {
            self.areas.get(leafnum as usize).copied().unwrap_or(0)
        }

        fn point_contents(&self, _p: &Vec3, _headnode: i32) -> i32 {
            0
        }

        fn transformed_point_contents(
            &self,
            _p: &Vec3,
            _headnode: i32,
            _origin: &Vec3,
            _angles: &Vec3,
        ) -> i32 {
            0
        }

        fn headnode_for_box(&self, _mins: &Vec3, _maxs: &Vec3) -> i32 {
            0
        }

        fn box_trace(
            &self,
            _start: &Vec3,
            end: &Vec3,
            _mins: &Vec3,
            _maxs: &Vec3,
            _headnode: i32,
            _brushmask: i32,
        ) -> Trace {
            // Default: no collision, fraction=1.0, endpos=end
            let mut t = Trace::default();
            t.endpos = *end;
            t
        }

        fn transformed_box_trace(
            &self,
            _start: &Vec3,
            end: &Vec3,
            _mins: &Vec3,
            _maxs: &Vec3,
            _headnode: i32,
            _brushmask: i32,
            _origin: &Vec3,
            _angles: &Vec3,
        ) -> Trace {
            let mut t = Trace::default();
            t.endpos = *end;
            t
        }

        fn num_clusters(&self) -> i32 {
            self.clusters.len() as i32
        }

        fn cluster_pvs(&self, _cluster: i32) -> &[u8] {
            &[0xFF, 0xFF, 0xFF, 0xFF]
        }

        fn cluster_phs(&self, _cluster: i32) -> &[u8] {
            &[0xFF, 0xFF, 0xFF, 0xFF]
        }

        fn point_leafnum(&self, _p: &Vec3) -> i32 {
            0
        }

        fn write_area_bits(&self, _area: i32) -> (i32, [u8; MAX_MAP_AREAS / 8]) {
            (0, [0u8; MAX_MAP_AREAS / 8])
        }

        fn areas_connected(&self, _area1: i32, _area2: i32) -> bool {
            self.areas_always_connected
        }

        fn headnode_visible(&self, _headnode: i32, _bitvector: &[u8]) -> bool {
            true
        }
    }

    // =========================================================================
    // AreaNode tests
    // =========================================================================

    #[test]
    fn area_node_default() {
        let node = AreaNode::default();
        assert_eq!(node.axis, -1);
        assert_eq!(node.dist, 0.0);
        assert_eq!(node.children, [usize::MAX; 2]);
        assert!(node.trigger_edicts.is_empty());
        assert!(node.solid_edicts.is_empty());
    }

    // =========================================================================
    // SvWorldContext basic tests
    // =========================================================================

    #[test]
    fn world_context_new() {
        let ctx = SvWorldContext::new();
        assert_eq!(ctx.numareanodes, 0);
        assert!(ctx.areanodes.is_empty());
    }

    #[test]
    fn world_context_clear_world() {
        let mut ctx = SvWorldContext::new();
        let mins = [-4096.0, -4096.0, -4096.0];
        let maxs = [4096.0, 4096.0, 4096.0];

        ctx.clear_world(&mins, &maxs);

        assert!(ctx.numareanodes > 0, "clear_world should create area nodes");
        assert!(ctx.areanodes.len() >= ctx.numareanodes);
    }

    #[test]
    fn world_context_clear_world_creates_tree() {
        let mut ctx = SvWorldContext::new();
        let mins = [-4096.0, -4096.0, -4096.0];
        let maxs = [4096.0, 4096.0, 4096.0];

        ctx.clear_world(&mins, &maxs);

        // With AREA_DEPTH=4, the tree should have multiple nodes
        // A full binary tree of depth 4 has 2^5 - 1 = 31 nodes
        assert!(ctx.numareanodes > 1, "Should have multiple area nodes in tree");

        // Root node (index 0) should have a valid axis
        let root = &ctx.areanodes[0];
        assert!(root.axis == 0 || root.axis == 1, "Root axis should be 0 or 1");
    }

    #[test]
    fn world_context_clear_world_symmetric() {
        let mut ctx = SvWorldContext::new();
        let mins = [-1024.0, -1024.0, -512.0];
        let maxs = [1024.0, 1024.0, 512.0];

        ctx.clear_world(&mins, &maxs);

        // Root node should split on the axis with the largest extent
        // X and Y are equal (2048 each), Z is 1024
        // So root should split on X (axis 0) since size[0] > size[1] is false for equal,
        // it picks axis 1 when sizes are equal
        let root = &ctx.areanodes[0];
        // With equal X and Y, axis will be 1 (size[0] is NOT > size[1])
        assert_eq!(root.axis, 1, "With equal X/Y extents, should split on axis 1");
        assert_eq!(root.dist, 0.0, "Midpoint of [-1024, 1024] should be 0");
    }

    // =========================================================================
    // Link/Unlink edict tests
    // =========================================================================

    fn make_solid_edict(origin: Vec3, mins: Vec3, maxs: Vec3) -> Edict {
        let mut e = Edict::default();
        e.inuse = true;
        e.s.origin = origin;
        e.mins = mins;
        e.maxs = maxs;
        e.solid = Solid::Bbox;
        e
    }

    #[test]
    fn unlink_edict_not_linked() {
        let mut ctx = SvWorldContext::new();
        ctx.clear_world(&[-4096.0; 3], &[4096.0; 3]);

        let mut edicts = make_edicts(2);
        edicts[1].area_linked = false;

        // Should not panic
        ctx.unlink_edict(&mut edicts, 1);
        assert!(!edicts[1].area_linked);
    }

    #[test]
    fn link_edict_not_inuse() {
        let mut ctx = SvWorldContext::new();
        ctx.clear_world(&[-4096.0; 3], &[4096.0; 3]);
        let cm = MockCM::simple();

        let mut edicts = make_edicts(2);
        edicts[1].inuse = false;

        ctx.link_edict(&mut edicts, 1, ServerState::Game, &cm);
        assert!(!edicts[1].area_linked, "Inactive edict should not be linked");
    }

    #[test]
    fn link_edict_world_entity_skip() {
        let mut ctx = SvWorldContext::new();
        ctx.clear_world(&[-4096.0; 3], &[4096.0; 3]);
        let cm = MockCM::simple();

        let mut edicts = make_edicts(2);
        edicts[0].inuse = true;

        ctx.link_edict(&mut edicts, 0, ServerState::Game, &cm);
        // Index 0 (world entity) should be skipped
        assert!(!edicts[0].area_linked, "World entity should not be linked");
    }

    #[test]
    fn link_edict_solid_bbox() {
        let mut ctx = SvWorldContext::new();
        ctx.clear_world(&[-4096.0; 3], &[4096.0; 3]);
        let cm = MockCM::simple();

        let mut edicts = make_edicts(2);
        edicts[1] = make_solid_edict([0.0, 0.0, 0.0], [-16.0, -16.0, -24.0], [16.0, 16.0, 32.0]);

        ctx.link_edict(&mut edicts, 1, ServerState::Game, &cm);

        assert!(edicts[1].area_linked, "Solid bbox edict should be linked");
        assert!(edicts[1].linkcount > 0, "linkcount should be incremented");
    }

    #[test]
    fn link_edict_sets_size() {
        let mut ctx = SvWorldContext::new();
        ctx.clear_world(&[-4096.0; 3], &[4096.0; 3]);
        let cm = MockCM::simple();

        let mut edicts = make_edicts(2);
        edicts[1] = make_solid_edict([0.0, 0.0, 0.0], [-16.0, -16.0, -24.0], [16.0, 16.0, 32.0]);

        ctx.link_edict(&mut edicts, 1, ServerState::Game, &cm);

        // size = maxs - mins
        assert_eq!(edicts[1].size[0], 32.0);
        assert_eq!(edicts[1].size[1], 32.0);
        assert_eq!(edicts[1].size[2], 56.0);
    }

    #[test]
    fn link_edict_sets_absmin_absmax() {
        let mut ctx = SvWorldContext::new();
        ctx.clear_world(&[-4096.0; 3], &[4096.0; 3]);
        let cm = MockCM::simple();

        let mut edicts = make_edicts(2);
        edicts[1] = make_solid_edict([100.0, 200.0, 0.0], [-16.0, -16.0, -24.0], [16.0, 16.0, 32.0]);

        ctx.link_edict(&mut edicts, 1, ServerState::Game, &cm);

        // absmin = origin + mins - 1 (epsilon)
        assert!((edicts[1].absmin[0] - (100.0 - 16.0 - 1.0)).abs() < 0.01);
        assert!((edicts[1].absmin[1] - (200.0 - 16.0 - 1.0)).abs() < 0.01);
        // absmax = origin + maxs + 1
        assert!((edicts[1].absmax[0] - (100.0 + 16.0 + 1.0)).abs() < 0.01);
        assert!((edicts[1].absmax[1] - (200.0 + 16.0 + 1.0)).abs() < 0.01);
    }

    #[test]
    fn link_edict_first_link_copies_old_origin() {
        let mut ctx = SvWorldContext::new();
        ctx.clear_world(&[-4096.0; 3], &[4096.0; 3]);
        let cm = MockCM::simple();

        let mut edicts = make_edicts(2);
        edicts[1] = make_solid_edict([100.0, 200.0, 300.0], [-8.0; 3], [8.0; 3]);
        edicts[1].linkcount = 0; // first time linking

        ctx.link_edict(&mut edicts, 1, ServerState::Game, &cm);

        assert_eq!(edicts[1].s.old_origin, edicts[1].s.origin,
            "First link should copy origin to old_origin");
    }

    #[test]
    fn link_then_unlink_edict() {
        let mut ctx = SvWorldContext::new();
        ctx.clear_world(&[-4096.0; 3], &[4096.0; 3]);
        let cm = MockCM::simple();

        let mut edicts = make_edicts(2);
        edicts[1] = make_solid_edict([0.0, 0.0, 0.0], [-16.0; 3], [16.0; 3]);

        ctx.link_edict(&mut edicts, 1, ServerState::Game, &cm);
        assert!(edicts[1].area_linked);

        ctx.unlink_edict(&mut edicts, 1);
        assert!(!edicts[1].area_linked);
        assert_eq!(edicts[1].area_node, -1);
    }

    #[test]
    fn link_edict_trigger() {
        let mut ctx = SvWorldContext::new();
        ctx.clear_world(&[-4096.0; 3], &[4096.0; 3]);
        let cm = MockCM::simple();

        let mut edicts = make_edicts(2);
        edicts[1].inuse = true;
        edicts[1].s.origin = [0.0, 0.0, 0.0];
        edicts[1].mins = [-16.0; 3];
        edicts[1].maxs = [16.0; 3];
        edicts[1].solid = Solid::Trigger;

        ctx.link_edict(&mut edicts, 1, ServerState::Game, &cm);

        assert!(edicts[1].area_linked, "Trigger edict should be linked");

        // Check that it's in the trigger list (not solid list)
        let node_idx = edicts[1].area_node as usize;
        assert!(
            ctx.areanodes[node_idx].trigger_edicts.contains(&1),
            "Trigger edict should be in trigger_edicts list"
        );
    }

    #[test]
    fn link_edict_encodes_solid_bbox() {
        let mut ctx = SvWorldContext::new();
        ctx.clear_world(&[-4096.0; 3], &[4096.0; 3]);
        let cm = MockCM::simple();

        let mut edicts = make_edicts(2);
        edicts[1] = make_solid_edict([0.0, 0.0, 0.0], [-16.0, -16.0, -24.0], [16.0, 16.0, 32.0]);

        ctx.link_edict(&mut edicts, 1, ServerState::Game, &cm);

        // For SOLID_BBOX:
        // i = maxs[0] / 8 = 16/8 = 2, clamped to [1,31]
        // j = -mins[2] / 8 = 24/8 = 3, clamped to [1,31]
        // k = (maxs[2]+32) / 8 = 64/8 = 8, clamped to [1,63]
        // solid = (k << 10) | (j << 5) | i = (8 << 10) | (3 << 5) | 2 = 8192 | 96 | 2 = 8290
        assert_eq!(edicts[1].s.solid, 8290, "Solid encoding should match packed format");
    }

    // =========================================================================
    // area_edicts tests
    // =========================================================================

    #[test]
    fn area_edicts_empty_world() {
        let ctx = SvWorldContext::new();
        let edicts = make_edicts(1);

        let result = ctx.area_edicts(&[-100.0; 3], &[100.0; 3], &edicts, 1024, AREA_SOLID);
        assert!(result.is_empty(), "Empty world should return no edicts");
    }

    #[test]
    fn area_edicts_finds_linked_entity() {
        let mut ctx = SvWorldContext::new();
        ctx.clear_world(&[-4096.0; 3], &[4096.0; 3]);
        let cm = MockCM::simple();

        let mut edicts = make_edicts(3);
        edicts[1] = make_solid_edict([0.0, 0.0, 0.0], [-16.0; 3], [16.0; 3]);

        ctx.link_edict(&mut edicts, 1, ServerState::Game, &cm);

        let result = ctx.area_edicts(
            &[-100.0; 3],
            &[100.0; 3],
            &edicts,
            1024,
            AREA_SOLID,
        );
        assert!(result.contains(&1), "area_edicts should find the linked entity");
    }

    #[test]
    fn area_edicts_excludes_out_of_range() {
        let mut ctx = SvWorldContext::new();
        ctx.clear_world(&[-4096.0; 3], &[4096.0; 3]);
        let cm = MockCM::simple();

        let mut edicts = make_edicts(3);
        edicts[1] = make_solid_edict([1000.0, 1000.0, 0.0], [-16.0; 3], [16.0; 3]);

        ctx.link_edict(&mut edicts, 1, ServerState::Game, &cm);

        // Search in a region that doesn't overlap with the entity
        let result = ctx.area_edicts(
            &[-100.0, -100.0, -100.0],
            &[100.0, 100.0, 100.0],
            &edicts,
            1024,
            AREA_SOLID,
        );
        assert!(!result.contains(&1), "Entity outside search box should not be found");
    }

    #[test]
    fn area_edicts_maxcount_limit() {
        let mut ctx = SvWorldContext::new();
        ctx.clear_world(&[-4096.0; 3], &[4096.0; 3]);
        let cm = MockCM::simple();

        // Link multiple entities at the same location
        let mut edicts = make_edicts(10);
        for i in 1..10 {
            edicts[i] = make_solid_edict([0.0, 0.0, 0.0], [-16.0; 3], [16.0; 3]);
            ctx.link_edict(&mut edicts, i, ServerState::Game, &cm);
        }

        // Search with maxcount=3
        let result = ctx.area_edicts(
            &[-100.0; 3],
            &[100.0; 3],
            &edicts,
            3,
            AREA_SOLID,
        );
        assert!(result.len() <= 3, "area_edicts should respect maxcount limit");
    }

    // =========================================================================
    // trace_bounds tests
    // =========================================================================

    #[test]
    fn trace_bounds_forward_movement() {
        let start = [0.0, 0.0, 0.0];
        let end = [100.0, 100.0, 100.0];
        let mins = [-16.0, -16.0, -24.0];
        let maxs = [16.0, 16.0, 32.0];
        let mut boxmins = [0.0; 3];
        let mut boxmaxs = [0.0; 3];

        SvWorldContext::trace_bounds(&start, &mins, &maxs, &end, &mut boxmins, &mut boxmaxs);

        // For each axis where end > start:
        // boxmins[i] = start[i] + mins[i] - 1
        // boxmaxs[i] = end[i] + maxs[i] + 1
        assert_eq!(boxmins[0], 0.0 + (-16.0) - 1.0); // -17
        assert_eq!(boxmaxs[0], 100.0 + 16.0 + 1.0);   // 117
    }

    #[test]
    fn trace_bounds_backward_movement() {
        let start = [100.0, 100.0, 100.0];
        let end = [0.0, 0.0, 0.0];
        let mins = [-16.0, -16.0, -24.0];
        let maxs = [16.0, 16.0, 32.0];
        let mut boxmins = [0.0; 3];
        let mut boxmaxs = [0.0; 3];

        SvWorldContext::trace_bounds(&start, &mins, &maxs, &end, &mut boxmins, &mut boxmaxs);

        // For each axis where end <= start:
        // boxmins[i] = end[i] + mins[i] - 1
        // boxmaxs[i] = start[i] + maxs[i] + 1
        assert_eq!(boxmins[0], 0.0 + (-16.0) - 1.0);  // -17
        assert_eq!(boxmaxs[0], 100.0 + 16.0 + 1.0);    // 117
    }

    #[test]
    fn trace_bounds_stationary() {
        let pos = [50.0, 50.0, 50.0];
        let mins = [-8.0, -8.0, -8.0];
        let maxs = [8.0, 8.0, 8.0];
        let mut boxmins = [0.0; 3];
        let mut boxmaxs = [0.0; 3];

        SvWorldContext::trace_bounds(&pos, &mins, &maxs, &pos, &mut boxmins, &mut boxmaxs);

        // When start == end (end is not > start):
        // boxmins[i] = end[i] + mins[i] - 1 = 50 - 8 - 1 = 41
        // boxmaxs[i] = start[i] + maxs[i] + 1 = 50 + 8 + 1 = 59
        for i in 0..3 {
            assert_eq!(boxmins[i], 41.0);
            assert_eq!(boxmaxs[i], 59.0);
        }
    }

    // =========================================================================
    // hull_for_entity tests
    // =========================================================================

    #[test]
    fn hull_for_entity_bbox() {
        let cm = MockCM::simple();
        let mut edicts = make_edicts(2);
        edicts[1].solid = Solid::Bbox;
        edicts[1].mins = [-16.0; 3];
        edicts[1].maxs = [16.0; 3];

        let models: Vec<Option<CModel>> = vec![None; 4];

        let headnode = SvWorldContext::hull_for_entity(&edicts, 1, &models, &cm);
        assert_eq!(headnode, 0, "BBOX entity should use headnode_for_box from CM");
    }

    #[test]
    #[should_panic(expected = "MOVETYPE_PUSH with a non bsp model")]
    fn hull_for_entity_bsp_no_model_panics() {
        let cm = MockCM::simple();
        let mut edicts = make_edicts(2);
        edicts[1].solid = Solid::Bsp;
        edicts[1].s.modelindex = 0; // no model

        let models: Vec<Option<CModel>> = vec![None; 4];

        SvWorldContext::hull_for_entity(&edicts, 1, &models, &cm);
    }

    #[test]
    fn hull_for_entity_bsp_with_model() {
        let cm = MockCM::simple();
        let mut edicts = make_edicts(2);
        edicts[1].solid = Solid::Bsp;
        edicts[1].s.modelindex = 1;

        let mut models: Vec<Option<CModel>> = vec![None; 4];
        models[1] = Some(CModel {
            headnode: 42,
            mins: [-64.0; 3],
            maxs: [64.0; 3],
            origin: [0.0; 3],
        });

        let headnode = SvWorldContext::hull_for_entity(&edicts, 1, &models, &cm);
        assert_eq!(headnode, 42, "BSP entity should return model's headnode");
    }

    // =========================================================================
    // SV_Trace tests (uses world context + collision model)
    // =========================================================================

    #[test]
    fn trace_through_empty_world() {
        let ctx = SvWorldContext::new();
        let cm = MockCM::simple();

        let edicts = make_edicts(1);
        let models: Vec<Option<CModel>> = vec![None];

        let start = [0.0, 0.0, 0.0];
        let end = [100.0, 0.0, 0.0];
        let mins = [-1.0; 3];
        let maxs = [1.0; 3];

        let trace = ctx.trace(
            &start,
            Some(&mins),
            Some(&maxs),
            &end,
            -1, // no passedict
            CONTENTS_SOLID,
            &edicts,
            &models,
            &cm,
        );

        assert_eq!(trace.fraction, 1.0, "Trace through empty world should reach endpoint");
        assert!(!trace.allsolid);
        assert!(!trace.startsolid);
    }

    #[test]
    fn trace_with_no_mins_maxs() {
        let ctx = SvWorldContext::new();
        let cm = MockCM::simple();

        let edicts = make_edicts(1);
        let models: Vec<Option<CModel>> = vec![None];

        let start = [0.0, 0.0, 0.0];
        let end = [100.0, 0.0, 0.0];

        let trace = ctx.trace(
            &start,
            None, // no mins
            None, // no maxs
            &end,
            -1,
            CONTENTS_SOLID,
            &edicts,
            &models,
            &cm,
        );

        assert_eq!(trace.fraction, 1.0);
    }

    // =========================================================================
    // point_contents test
    // =========================================================================

    #[test]
    fn point_contents_empty_world() {
        let ctx = SvWorldContext::new();
        let cm = MockCM::simple();

        let edicts = make_edicts(1);
        let models: Vec<Option<CModel>> = vec![None, None]; // world model at index 1

        let point = [0.0, 0.0, 0.0];
        let contents = ctx.point_contents(&point, &edicts, &models, &cm);

        assert_eq!(contents, 0, "Empty world should have no contents");
    }

    // =========================================================================
    // Global world context tests
    // =========================================================================

    #[test]
    fn init_global_world_context() {
        init_sv_world_ctx();
        let result = with_sv_world_ctx(|ctx| {
            ctx.numareanodes
        });
        assert!(result.is_some());
        assert_eq!(result.unwrap(), 0);
    }

    // =========================================================================
    // MoveClip default tests
    // =========================================================================

    #[test]
    fn moveclip_default() {
        let clip = MoveClip::default();
        assert_eq!(clip.passedict, -1);
        assert_eq!(clip.contentmask, 0);
        assert_eq!(clip.trace.fraction, 1.0);
        assert!(!clip.trace.allsolid);
        assert!(!clip.trace.startsolid);
    }
}
