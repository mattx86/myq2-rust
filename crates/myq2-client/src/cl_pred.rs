// cl_pred.rs -- client-side movement prediction
// Converted from: myq2-original/client/cl_pred.c

use crate::client::*;
use myq2_common::q_shared::*;
use myq2_common::common::{com_printf, com_dprintf};

// ============================================================
// CL_CheckPredictionError
// ============================================================

/// Compare the server's reported player origin with our predicted origin.
/// If the error is small, save it for interpolation smoothing.
/// If it's large (teleport), clear the prediction error.
pub fn cl_check_prediction_error(
    cl: &mut ClientState,
    incoming_acknowledged: i32,
    cl_predict: f32,
    cl_showmiss: f32,
) {
    cl_check_prediction_error_with_time(cl, incoming_acknowledged, cl_predict, cl_showmiss, 0);
}

/// Check prediction error with current time for smoothed error application.
pub fn cl_check_prediction_error_with_time(
    cl: &mut ClientState,
    incoming_acknowledged: i32,
    cl_predict: f32,
    cl_showmiss: f32,
    current_time: i32,
) {
    if cl_predict == 0.0 || (cl.frame.playerstate.pmove.pm_flags & PMF_NO_PREDICTION) != 0 {
        return;
    }

    // calculate the last usercmd_t we sent that the server has processed
    let frame = (incoming_acknowledged as usize) & (CMD_BACKUP - 1);

    // compare what the server returned with what we had predicted it to be
    let mut delta = [0i32; 3];
    for i in 0..3 {
        delta[i] = cl.frame.playerstate.pmove.origin[i] as i32
            - cl.predicted_origins[frame][i] as i32;
    }

    // save the prediction error for interpolation
    let len = delta[0].abs() + delta[1].abs() + delta[2].abs();
    if len > 640 {
        // 80 world units -- a teleport or something
        vector_clear(&mut cl.prediction_error);
        // Also clear smoothed error on teleport
        cl.smoothing.prediction_error.clear();
    } else {
        if cl_showmiss != 0.0 && (delta[0] != 0 || delta[1] != 0 || delta[2] != 0) {
            com_dprintf(&format!(
                "prediction miss on {}: {}\n",
                cl.frame.serverframe,
                delta[0] + delta[1] + delta[2]
            ));
        }

        // copy corrected origin back
        for i in 0..3 {
            cl.predicted_origins[frame][i] = cl.frame.playerstate.pmove.origin[i];
        }

        // Calculate the raw error
        let mut raw_error = [0.0f32; 3];
        for i in 0..3 {
            raw_error[i] = delta[i] as f32 * 0.125;
        }

        // Use smoothed error if enabled, otherwise use raw
        if cl.smoothing.prediction_error.enabled && current_time > 0 {
            cl.smoothing.prediction_error.set_error(raw_error, current_time);
            cl.prediction_error = cl.smoothing.prediction_error.get_smoothed_error(current_time);
        } else {
            // Legacy behavior - apply error immediately
            cl.prediction_error = raw_error;
        }
    }
}

/// Get the current smoothed prediction error.
/// Call this each frame to get the interpolated error value.
pub fn cl_get_smoothed_prediction_error(cl: &mut ClientState, current_time: i32) -> [f32; 3] {
    if cl.smoothing.prediction_error.enabled {
        cl.smoothing.prediction_error.get_smoothed_error(current_time)
    } else {
        cl.prediction_error
    }
}

// ============================================================
// CL_ClipMoveToEntities
// ============================================================

/// Clip a player movement against all solid entities in the current frame.
pub fn cl_clip_move_to_entities(
    start: &Vec3,
    mins: &Vec3,
    maxs: &Vec3,
    end: &Vec3,
    tr: &mut Trace,
    cl: &ClientState,
    cl_parse_entities: &[EntityState; MAX_PARSE_ENTITIES],
    cm_headnode_for_box: &dyn Fn(&Vec3, &Vec3) -> i32,
    cm_transformed_box_trace: &dyn Fn(&Vec3, &Vec3, &Vec3, &Vec3, i32, i32, &Vec3, &Vec3) -> Trace,
) {
    for i in 0..cl.frame.num_entities as usize {
        let num = (cl.frame.parse_entities as usize + i) & (MAX_PARSE_ENTITIES - 1);
        let ent = &cl_parse_entities[num];

        if ent.solid == 0 {
            continue;
        }

        if ent.number == cl.playernum + 1 {
            continue;
        }

        let (headnode, angles): (i32, Vec3);

        if ent.solid == 31 {
            // special value for bmodel
            let cmodel_idx = ent.modelindex as usize;
            if cmodel_idx < cl.model_clip.len() && cl.model_clip[cmodel_idx] != 0 {
                // In the full engine, model_clip[idx] is a cmodel index;
                // we'd look up its headnode. For now we use the index as headnode.
                headnode = cl.model_clip[cmodel_idx];
                angles = ent.angles;
            } else {
                continue;
            }
        } else {
            // encoded bbox
            let x = 8 * (ent.solid & 31);
            let zd = 8 * ((ent.solid >> 5) & 31);
            let zu = 8 * ((ent.solid >> 10) & 63) - 32;

            let bmins = [-(x as f32), -(x as f32), -(zd as f32)];
            let bmaxs = [x as f32, x as f32, zu as f32];

            headnode = cm_headnode_for_box(&bmins, &bmaxs);
            angles = vec3_origin; // boxes don't rotate
        }

        if tr.allsolid {
            return;
        }

        let trace = cm_transformed_box_trace(
            start,
            end,
            mins,
            maxs,
            headnode,
            MASK_PLAYERSOLID,
            &ent.origin,
            &angles,
        );

        if trace.allsolid || trace.startsolid || trace.fraction < tr.fraction {
            let mut new_trace = trace;
            new_trace.ent_index = ent.number;
            if tr.startsolid {
                *tr = new_trace;
                tr.startsolid = true;
            } else {
                *tr = new_trace;
            }
        } else if trace.startsolid {
            tr.startsolid = true;
        }
    }
}

// ============================================================
// CL_PMTrace
// ============================================================

/// Trace against the world and all solid entities for player movement.
pub fn cl_pm_trace(
    start: &Vec3,
    mins: &Vec3,
    maxs: &Vec3,
    end: &Vec3,
    cl: &ClientState,
    cl_parse_entities: &[EntityState; MAX_PARSE_ENTITIES],
    cm_box_trace: &dyn Fn(&Vec3, &Vec3, &Vec3, &Vec3, i32, i32) -> Trace,
    cm_headnode_for_box: &dyn Fn(&Vec3, &Vec3) -> i32,
    cm_transformed_box_trace: &dyn Fn(&Vec3, &Vec3, &Vec3, &Vec3, i32, i32, &Vec3, &Vec3) -> Trace,
) -> Trace {
    // check against world
    let mut t = cm_box_trace(start, end, mins, maxs, 0, MASK_PLAYERSOLID);
    if t.fraction < 1.0 {
        t.ent_index = 1; // world entity
    }

    // check all other solid models
    cl_clip_move_to_entities(
        start,
        mins,
        maxs,
        end,
        &mut t,
        cl,
        cl_parse_entities,
        cm_headnode_for_box,
        cm_transformed_box_trace,
    );

    t
}

// ============================================================
// CL_PMpointcontents
// ============================================================

/// Get combined point contents at a position, checking world and bmodel entities.
pub fn cl_pm_point_contents(
    point: &Vec3,
    cl: &ClientState,
    cl_parse_entities: &[EntityState; MAX_PARSE_ENTITIES],
    cm_point_contents: &dyn Fn(&Vec3, i32) -> i32,
    cm_transformed_point_contents: &dyn Fn(&Vec3, i32, &Vec3, &Vec3) -> i32,
) -> i32 {
    let mut contents = cm_point_contents(point, 0);

    for i in 0..cl.frame.num_entities as usize {
        let num = (cl.frame.parse_entities as usize + i) & (MAX_PARSE_ENTITIES - 1);
        let ent = &cl_parse_entities[num];

        if ent.solid != 31 {
            // special value for bmodel
            continue;
        }

        let cmodel_idx = ent.modelindex as usize;
        if cmodel_idx < cl.model_clip.len() && cl.model_clip[cmodel_idx] != 0 {
            contents |= cm_transformed_point_contents(
                point,
                cl.model_clip[cmodel_idx],
                &ent.origin,
                &ent.angles,
            );
        }
    }

    contents
}

// ============================================================
// PmoveClView — lightweight view of ClientState for pmove callbacks
// ============================================================

/// Lightweight view of the ClientState fields needed by pmove trace callbacks.
/// Used to avoid borrowing the full ClientState while it's mutably borrowed.
pub struct PmoveClView<'a> {
    pub num_entities: i32,
    pub parse_entities: i32,
    pub playernum: i32,
    pub model_clip: &'a [i32],
}

/// CL_PMTrace using a PmoveClView instead of &ClientState.
pub fn cl_pm_trace_with_view(
    start: &Vec3,
    mins: &Vec3,
    maxs: &Vec3,
    end: &Vec3,
    view: &PmoveClView,
    cl_parse_entities: &[EntityState; MAX_PARSE_ENTITIES],
    cm_box_trace_fn: &dyn Fn(&Vec3, &Vec3, &Vec3, &Vec3, i32, i32) -> Trace,
    cm_headnode_for_box_fn: &dyn Fn(&Vec3, &Vec3) -> i32,
    cm_transformed_box_trace_fn: &dyn Fn(&Vec3, &Vec3, &Vec3, &Vec3, i32, i32, &Vec3, &Vec3) -> Trace,
) -> Trace {
    let mut t = cm_box_trace_fn(start, end, mins, maxs, 0, MASK_PLAYERSOLID);
    if t.fraction < 1.0 {
        t.ent_index = 1; // world entity
    }

    // clip against entities
    for i in 0..view.num_entities as usize {
        let num = (view.parse_entities as usize + i) & (MAX_PARSE_ENTITIES - 1);
        let ent = &cl_parse_entities[num];

        if ent.solid == 0 {
            continue;
        }
        if ent.number == view.playernum + 1 {
            continue;
        }

        let (headnode, angles): (i32, Vec3);

        if ent.solid == 31 {
            let cmodel_idx = ent.modelindex as usize;
            if cmodel_idx < view.model_clip.len() && view.model_clip[cmodel_idx] != 0 {
                headnode = view.model_clip[cmodel_idx];
                angles = ent.angles;
            } else {
                continue;
            }
        } else {
            let x = 8 * (ent.solid & 31);
            let zd = 8 * ((ent.solid >> 5) & 31);
            let zu = 8 * ((ent.solid >> 10) & 63) - 32;
            let bmins = [-(x as f32), -(x as f32), -(zd as f32)];
            let bmaxs = [x as f32, x as f32, zu as f32];
            headnode = cm_headnode_for_box_fn(&bmins, &bmaxs);
            angles = VEC3_ORIGIN;
        }

        if t.allsolid {
            return t;
        }

        let trace = cm_transformed_box_trace_fn(
            start, end, mins, maxs, headnode, MASK_PLAYERSOLID, &ent.origin, &angles,
        );

        if trace.allsolid || trace.startsolid || trace.fraction < t.fraction {
            let mut new_trace = trace;
            new_trace.ent_index = ent.number;
            if t.startsolid {
                t = new_trace;
                t.startsolid = true;
            } else {
                t = new_trace;
            }
        } else if trace.startsolid {
            t.startsolid = true;
        }
    }

    t
}

/// CL_PMpointcontents using a PmoveClView instead of &ClientState.
pub fn cl_pm_point_contents_with_view(
    point: &Vec3,
    view: &PmoveClView,
    cl_parse_entities: &[EntityState; MAX_PARSE_ENTITIES],
    cm_point_contents_fn: &dyn Fn(&Vec3, i32) -> i32,
    cm_transformed_point_contents_fn: &dyn Fn(&Vec3, i32, &Vec3, &Vec3) -> i32,
) -> i32 {
    let mut contents = cm_point_contents_fn(point, 0);

    for i in 0..view.num_entities as usize {
        let num = (view.parse_entities as usize + i) & (MAX_PARSE_ENTITIES - 1);
        let ent = &cl_parse_entities[num];

        if ent.solid != 31 {
            continue;
        }

        let cmodel_idx = ent.modelindex as usize;
        if cmodel_idx < view.model_clip.len() && view.model_clip[cmodel_idx] != 0 {
            contents |= cm_transformed_point_contents_fn(
                point,
                view.model_clip[cmodel_idx],
                &ent.origin,
                &ent.angles,
            );
        }
    }

    contents
}

// ============================================================
// CL_PredictMovement
// ============================================================

/// Run client-side movement prediction.
///
/// Sets cl.predicted_origin and cl.predicted_angles by replaying
/// unacknowledged user commands through the player movement code.
pub fn cl_predict_movement(
    cl: &mut ClientState,
    cls: &ClientStatic,
    cl_predict_value: f32,
    cl_showmiss_value: f32,
    cl_paused_value: f32,
    pm_airaccelerate: &mut f32,
    pmove_fn: &dyn Fn(&mut PmoveData),
) {
    if cls.state != ConnState::Active {
        return;
    }

    if cl_paused_value != 0.0 {
        return;
    }

    if cl_predict_value == 0.0
        || (cl.frame.playerstate.pmove.pm_flags & PMF_NO_PREDICTION) != 0
    {
        // just set angles
        for i in 0..3 {
            cl.predicted_angles[i] = cl.viewangles[i]
                + short2angle(cl.frame.playerstate.pmove.delta_angles[i]);
        }
        return;
    }

    let mut ack = cls.netchan.incoming_acknowledged;
    let current = cls.netchan.outgoing_sequence;

    // if we are too far out of date, just freeze
    if current - ack >= CMD_BACKUP as i32
        && cl_showmiss_value != 0.0 {
            com_dprintf("exceeded CMD_BACKUP\n");
        }
        // PRED_OUT_OF_DATE_FREEZE: optionally return here

    // copy current state to pmove
    let mut pm = PmoveData::default();

    // parse air acceleration from config string
    *pm_airaccelerate = cl.configstrings[CS_AIRACCEL]
        .parse::<f32>()
        .unwrap_or(0.0);

    pm.s = cl.frame.playerstate.pmove;

    #[allow(unused_assignments)]
    let mut frame: usize = 0;

    // run frames
    while ack + 1 < current {
        ack += 1;
        frame = (ack as usize) & (CMD_BACKUP - 1);
        pm.cmd = cl.cmds[frame];

        pmove_fn(&mut pm);

        // save for debug checking
        for j in 0..3 {
            cl.predicted_origins[frame][j] = pm.s.origin[j];
        }
    }

    // R1Q2-style "detect" mode stair smoothing:
    // Instead of comparing 2 frames back (vanilla Q2), we compare against the previous
    // predicted origin and use specific height ranges that match typical Quake 2 stair
    // heights (8, 12, 16 world units = 64, 96, 128 pmove units). This provides smoother
    // stair climbing without incorrectly smoothing teleports or other vertical movements.
    let oldz = cl.predicted_origin[2];
    let newz = pm.s.origin[2] as f32 * 0.125;
    let step = newz - oldz;

    // Check if this looks like a stair step:
    // - Player is on ground
    // - Player has horizontal velocity (actively moving)
    // - Height change matches typical stair heights (7-17 units covers 8, 12, 16 unit steps with margin)
    let has_velocity = pm.s.velocity[0] != 0 || pm.s.velocity[1] != 0;
    if (pm.s.pm_flags & PMF_ON_GROUND) != 0
        && has_velocity
        && step > 7.0
        && step < 17.0
    {
        cl.predicted_step = step;
        cl.predicted_step_time = (cls.realtime as f32 - cls.frametime * 500.0) as u32;
    }

    // copy results out for rendering
    cl.predicted_origin[0] = pm.s.origin[0] as f32 * 0.125;
    cl.predicted_origin[1] = pm.s.origin[1] as f32 * 0.125;
    cl.predicted_origin[2] = pm.s.origin[2] as f32 * 0.125;

    // === Moving platform/brush velocity prediction ===
    // If standing on a mover (door, elevator, platform), add platform velocity
    // to predicted position for smoother riding
    if pm.groundentity > 0 && cl.smoothing.mover_prediction.enabled {
        // Get time delta for velocity application (frametime is in seconds)
        let delta_time = cls.frametime;
        let platform_offset = cl.smoothing.mover_prediction
            .get_platform_offset(pm.groundentity, delta_time);

        // Apply platform offset to predicted position
        cl.predicted_origin[0] += platform_offset[0];
        cl.predicted_origin[1] += platform_offset[1];
        cl.predicted_origin[2] += platform_offset[2];
    }

    cl.predicted_angles = pm.viewangles;
}

// ============================================================
// Unit tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use myq2_common::q_shared::*;

    // -------------------------------------------------------
    // Helper: create a minimal ClientState for prediction testing
    // -------------------------------------------------------
    fn make_cl_for_prediction(
        pm_origin: [i16; 3],
        pm_flags: u8,
        serverframe: i32,
    ) -> ClientState {
        let mut cl = ClientState::default();
        cl.frame.playerstate.pmove.origin = pm_origin;
        cl.frame.playerstate.pmove.pm_flags = pm_flags;
        cl.frame.serverframe = serverframe;
        cl
    }

    // -------------------------------------------------------
    // cl_check_prediction_error tests
    // -------------------------------------------------------

    #[test]
    fn test_prediction_error_skipped_when_predict_disabled() {
        let mut cl = make_cl_for_prediction([800, 0, 0], 0, 100);
        cl.prediction_error = [99.0, 99.0, 99.0]; // sentinel

        cl_check_prediction_error(&mut cl, 50, 0.0, 0.0);

        // When cl_predict == 0.0, the function returns early.
        // prediction_error should not be modified.
        assert_eq!(cl.prediction_error, [99.0, 99.0, 99.0]);
    }

    #[test]
    fn test_prediction_error_skipped_when_no_prediction_flag() {
        let mut cl = make_cl_for_prediction([800, 0, 0], PMF_NO_PREDICTION, 100);
        cl.prediction_error = [99.0, 99.0, 99.0];

        cl_check_prediction_error(&mut cl, 50, 1.0, 0.0);

        assert_eq!(cl.prediction_error, [99.0, 99.0, 99.0]);
    }

    #[test]
    fn test_prediction_error_zero_when_perfect_match() {
        let mut cl = make_cl_for_prediction([800, 400, -200], 0, 100);

        // Set predicted_origins to match exactly what the server reports
        let frame = 50usize & (CMD_BACKUP - 1);
        cl.predicted_origins[frame] = [800, 400, -200];

        cl_check_prediction_error(&mut cl, 50, 1.0, 0.0);

        // Delta is zero, so error should be zero
        for i in 0..3 {
            assert!((cl.prediction_error[i]).abs() < 0.001,
                "prediction_error[{}] should be 0, got {}", i, cl.prediction_error[i]);
        }
    }

    #[test]
    fn test_prediction_error_small_delta_applied() {
        let mut cl = make_cl_for_prediction([808, 400, -200], 0, 100);
        // Smoothing disabled so raw error is applied
        cl.smoothing.prediction_error.enabled = false;

        let frame = 50usize & (CMD_BACKUP - 1);
        // Predicted: [800, 400, -200], server says [808, 400, -200]
        // Delta = [8, 0, 0]
        cl.predicted_origins[frame] = [800, 400, -200];

        cl_check_prediction_error(&mut cl, 50, 1.0, 0.0);

        // Error = delta * 0.125 = [1.0, 0.0, 0.0]
        assert!((cl.prediction_error[0] - 1.0).abs() < 0.001,
            "error[0]={}", cl.prediction_error[0]);
        assert!((cl.prediction_error[1] - 0.0).abs() < 0.001);
        assert!((cl.prediction_error[2] - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_prediction_error_corrects_predicted_origin() {
        let mut cl = make_cl_for_prediction([808, 400, -200], 0, 100);
        cl.smoothing.prediction_error.enabled = false;

        let frame = 50usize & (CMD_BACKUP - 1);
        cl.predicted_origins[frame] = [800, 400, -200];

        cl_check_prediction_error(&mut cl, 50, 1.0, 0.0);

        // predicted_origins[frame] should be corrected to server value
        assert_eq!(cl.predicted_origins[frame], [808, 400, -200]);
    }

    #[test]
    fn test_prediction_error_teleport_clears_error() {
        let mut cl = make_cl_for_prediction([8000, 0, 0], 0, 100);
        cl.prediction_error = [5.0, 5.0, 5.0]; // pre-existing error

        let frame = 50usize & (CMD_BACKUP - 1);
        // Huge delta: server says 8000, predicted 0. delta=8000, len=8000 > 640
        cl.predicted_origins[frame] = [0, 0, 0];

        cl_check_prediction_error(&mut cl, 50, 1.0, 0.0);

        // Teleport: error cleared to zero
        assert_eq!(cl.prediction_error, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_prediction_error_just_under_teleport_threshold() {
        let mut cl = make_cl_for_prediction([640, 0, 0], 0, 100);
        cl.smoothing.prediction_error.enabled = false;

        let frame = 50usize & (CMD_BACKUP - 1);
        // Delta = [640, 0, 0]. len = 640 which is NOT > 640, so this is correction not teleport
        cl.predicted_origins[frame] = [0, 0, 0];

        cl_check_prediction_error(&mut cl, 50, 1.0, 0.0);

        // Error should be applied (not cleared as teleport)
        assert!((cl.prediction_error[0] - (640.0 * 0.125)).abs() < 0.01,
            "error={}", cl.prediction_error[0]);
    }

    #[test]
    fn test_prediction_error_just_over_teleport_threshold() {
        let mut cl = make_cl_for_prediction([641, 0, 0], 0, 100);

        let frame = 50usize & (CMD_BACKUP - 1);
        cl.predicted_origins[frame] = [0, 0, 0];

        cl_check_prediction_error(&mut cl, 50, 1.0, 0.0);

        // len=641 > 640 => teleport => error cleared
        assert_eq!(cl.prediction_error, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_prediction_error_negative_delta() {
        let mut cl = make_cl_for_prediction([100, 100, 100], 0, 100);
        cl.smoothing.prediction_error.enabled = false;

        let frame = 50usize & (CMD_BACKUP - 1);
        // Predicted overshot: [110, 110, 110], server says [100, 100, 100]
        cl.predicted_origins[frame] = [110, 110, 110];

        cl_check_prediction_error(&mut cl, 50, 1.0, 0.0);

        // Delta = [100-110, 100-110, 100-110] = [-10, -10, -10]
        // Error = [-10*0.125, -10*0.125, -10*0.125] = [-1.25, -1.25, -1.25]
        for i in 0..3 {
            assert!((cl.prediction_error[i] - (-1.25)).abs() < 0.01,
                "error[{}]={}", i, cl.prediction_error[i]);
        }
    }

    // -------------------------------------------------------
    // cl_check_prediction_error_with_time tests (smoothed path)
    // -------------------------------------------------------

    #[test]
    fn test_prediction_error_smoothed_path() {
        let mut cl = make_cl_for_prediction([808, 400, -200], 0, 100);
        cl.smoothing.prediction_error.enabled = true;

        let frame = 50usize & (CMD_BACKUP - 1);
        cl.predicted_origins[frame] = [800, 400, -200];

        cl_check_prediction_error_with_time(&mut cl, 50, 1.0, 0.0, 5000);

        // With smoothing enabled, get_smoothed_error is called.
        // At time=5000 with error_time=5000, elapsed=0, so the smoothed error
        // should be near the current_error (initial), not yet at target.
        // The exact value depends on the smoothing impl - just verify it doesn't panic
        // and produces a reasonable value.
        let mag = cl.prediction_error[0].abs() + cl.prediction_error[1].abs() + cl.prediction_error[2].abs();
        assert!(mag < 100.0, "error magnitude should be reasonable: {}", mag);
    }

    // -------------------------------------------------------
    // cl_get_smoothed_prediction_error tests
    // -------------------------------------------------------

    #[test]
    fn test_get_smoothed_error_disabled() {
        let mut cl = ClientState::default();
        cl.smoothing.prediction_error.enabled = false;
        cl.prediction_error = [1.0, 2.0, 3.0];

        let result = cl_get_smoothed_prediction_error(&mut cl, 5000);
        assert_eq!(result, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_get_smoothed_error_enabled_returns_smoothed() {
        let mut cl = ClientState::default();
        cl.smoothing.prediction_error.enabled = true;
        cl.smoothing.prediction_error.target_error = [10.0, 0.0, 0.0];
        cl.smoothing.prediction_error.current_error = [0.0, 0.0, 0.0];
        cl.smoothing.prediction_error.error_time = 1000;
        cl.smoothing.prediction_error.smooth_duration_ms = 100;

        // At time 1050 (halfway through smoothing)
        let result = cl_get_smoothed_prediction_error(&mut cl, 1050);
        // Should be ~halfway between 0 and 10 in X
        assert!(result[0] > 3.0 && result[0] < 7.0,
            "halfway smoothed error should be ~5.0, got {}", result[0]);
    }

    #[test]
    fn test_get_smoothed_error_after_duration() {
        let mut cl = ClientState::default();
        cl.smoothing.prediction_error.enabled = true;
        cl.smoothing.prediction_error.target_error = [10.0, 20.0, 30.0];
        cl.smoothing.prediction_error.current_error = [0.0, 0.0, 0.0];
        cl.smoothing.prediction_error.error_time = 1000;
        cl.smoothing.prediction_error.smooth_duration_ms = 100;

        // After smoothing duration (1200 > 1000 + 100)
        let result = cl_get_smoothed_prediction_error(&mut cl, 1200);
        assert!((result[0] - 10.0).abs() < 0.01, "x={}", result[0]);
        assert!((result[1] - 20.0).abs() < 0.01, "y={}", result[1]);
        assert!((result[2] - 30.0).abs() < 0.01, "z={}", result[2]);
    }

    // -------------------------------------------------------
    // cl_clip_move_to_entities tests
    // -------------------------------------------------------

    #[test]
    fn test_clip_move_skips_zero_solid_entities() {
        let mut cl = ClientState::default();
        cl.frame.num_entities = 1;
        cl.frame.parse_entities = 0;
        cl.playernum = 0;

        let mut parse_ents = std::array::from_fn(|_| EntityState::default());
        parse_ents[0].solid = 0; // should be skipped
        parse_ents[0].number = 5;

        let mut tr = Trace::default();
        tr.fraction = 1.0;

        let start = [0.0, 0.0, 0.0];
        let end = [100.0, 0.0, 0.0];
        let mins = [-16.0, -16.0, -24.0];
        let maxs = [16.0, 16.0, 32.0];

        // Dummy callbacks that should never be called since entity is skipped
        let headnode_fn = |_: &Vec3, _: &Vec3| -> i32 { 0 };
        let trace_fn = |_: &Vec3, _: &Vec3, _: &Vec3, _: &Vec3, _: i32, _: i32, _: &Vec3, _: &Vec3| -> Trace {
            panic!("should not be called for solid=0 entity");
        };

        cl_clip_move_to_entities(
            &start, &mins, &maxs, &end, &mut tr, &cl, &parse_ents,
            &headnode_fn, &trace_fn,
        );

        assert!((tr.fraction - 1.0).abs() < 0.001, "fraction should be unchanged");
    }

    #[test]
    fn test_clip_move_skips_player_entity() {
        let mut cl = ClientState::default();
        cl.frame.num_entities = 1;
        cl.frame.parse_entities = 0;
        cl.playernum = 4; // player entity number is playernum + 1 = 5

        let mut parse_ents = std::array::from_fn(|_| EntityState::default());
        parse_ents[0].solid = 31; // solid bmodel
        parse_ents[0].number = 5; // same as playernum + 1, should be skipped

        let mut tr = Trace::default();
        tr.fraction = 1.0;

        let start = [0.0; 3];
        let end = [100.0, 0.0, 0.0];
        let mins = [-16.0; 3];
        let maxs = [16.0; 3];

        let headnode_fn = |_: &Vec3, _: &Vec3| -> i32 { 0 };
        let trace_fn = |_: &Vec3, _: &Vec3, _: &Vec3, _: &Vec3, _: i32, _: i32, _: &Vec3, _: &Vec3| -> Trace {
            panic!("should not be called for player entity");
        };

        cl_clip_move_to_entities(
            &start, &mins, &maxs, &end, &mut tr, &cl, &parse_ents,
            &headnode_fn, &trace_fn,
        );

        assert!((tr.fraction - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_clip_move_encoded_bbox_decoding() {
        // Test that the encoded bbox solid value is decoded correctly
        // solid format: x=8*(solid&31), zd=8*((solid>>5)&31), zu=8*((solid>>10)&63)-32

        let mut cl = ClientState::default();
        cl.frame.num_entities = 1;
        cl.frame.parse_entities = 0;
        cl.playernum = 0;

        let mut parse_ents = std::array::from_fn(|_| EntityState::default());
        // Encode: x=2 (16 units), zd=3 (24 units down), zu=4 (0 units up: 8*4-32=0)
        let solid = 2 | (3 << 5) | (4 << 10);
        parse_ents[0].solid = solid;
        parse_ents[0].number = 10;
        parse_ents[0].origin = [50.0, 0.0, 0.0];

        let mut tr = Trace::default();
        tr.fraction = 1.0;

        let start = [0.0; 3];
        let end = [100.0, 0.0, 0.0];
        let mins = [-1.0; 3];
        let maxs = [1.0; 3];

        // Track what headnode_for_box receives
        let mut called_with_mins = [0.0f32; 3];
        let mut called_with_maxs = [0.0f32; 3];

        let headnode_fn = |bmins: &Vec3, bmaxs: &Vec3| -> i32 {
            // Verify the decoded bbox
            // x = 8 * 2 = 16
            assert!((bmins[0] - (-16.0)).abs() < 0.01, "bmins[0]={}", bmins[0]);
            assert!((bmins[1] - (-16.0)).abs() < 0.01, "bmins[1]={}", bmins[1]);
            // zd = 8 * 3 = 24
            assert!((bmins[2] - (-24.0)).abs() < 0.01, "bmins[2]={}", bmins[2]);
            assert!((bmaxs[0] - 16.0).abs() < 0.01, "bmaxs[0]={}", bmaxs[0]);
            assert!((bmaxs[1] - 16.0).abs() < 0.01, "bmaxs[1]={}", bmaxs[1]);
            // zu = 8 * 4 - 32 = 0
            assert!((bmaxs[2] - 0.0).abs() < 0.01, "bmaxs[2]={}", bmaxs[2]);
            42 // return dummy headnode
        };

        let trace_fn = |_: &Vec3, _: &Vec3, _: &Vec3, _: &Vec3, headnode: i32, _: i32, _: &Vec3, _: &Vec3| -> Trace {
            assert_eq!(headnode, 42, "headnode should match what headnode_for_box returned");
            Trace::default() // fraction=1.0, no hit
        };

        cl_clip_move_to_entities(
            &start, &mins, &maxs, &end, &mut tr, &cl, &parse_ents,
            &headnode_fn, &trace_fn,
        );
    }

    #[test]
    fn test_clip_move_uses_closer_trace() {
        let mut cl = ClientState::default();
        cl.frame.num_entities = 2;
        cl.frame.parse_entities = 0;
        cl.playernum = 0;

        let mut parse_ents = std::array::from_fn(|_| EntityState::default());
        // Two entities with encoded bboxes
        parse_ents[0].solid = 2 | (3 << 5) | (8 << 10);
        parse_ents[0].number = 10;
        parse_ents[0].origin = [80.0, 0.0, 0.0]; // farther

        parse_ents[1].solid = 2 | (3 << 5) | (8 << 10);
        parse_ents[1].number = 11;
        parse_ents[1].origin = [30.0, 0.0, 0.0]; // closer

        let mut tr = Trace::default();
        tr.fraction = 1.0;

        let start = [0.0; 3];
        let end = [100.0, 0.0, 0.0];
        let mins = [-1.0; 3];
        let maxs = [1.0; 3];

        let headnode_fn = |_: &Vec3, _: &Vec3| -> i32 { 0 };

        let mut call_count = 0;
        let trace_fn = |_: &Vec3, _: &Vec3, _: &Vec3, _: &Vec3, _: i32, _: i32, origin: &Vec3, _: &Vec3| -> Trace {
            let mut t = Trace::default();
            // Entity at [30,0,0] returns fraction 0.3
            // Entity at [80,0,0] returns fraction 0.8
            if origin[0] < 50.0 {
                t.fraction = 0.3;
            } else {
                t.fraction = 0.8;
            }
            t
        };

        cl_clip_move_to_entities(
            &start, &mins, &maxs, &end, &mut tr, &cl, &parse_ents,
            &headnode_fn, &trace_fn,
        );

        // The trace with the smaller fraction should win
        assert!((tr.fraction - 0.3).abs() < 0.01,
            "should use closer trace: fraction={}", tr.fraction);
    }

    #[test]
    fn test_clip_move_allsolid_early_return() {
        let mut cl = ClientState::default();
        cl.frame.num_entities = 2;
        cl.frame.parse_entities = 0;
        cl.playernum = 0;

        let mut parse_ents = std::array::from_fn(|_| EntityState::default());
        parse_ents[0].solid = 2 | (3 << 5) | (8 << 10);
        parse_ents[0].number = 10;
        parse_ents[1].solid = 2 | (3 << 5) | (8 << 10);
        parse_ents[1].number = 11;

        let mut tr = Trace::default();
        tr.fraction = 1.0;
        tr.allsolid = true; // Set allsolid before entering

        let start = [0.0; 3];
        let end = [100.0, 0.0, 0.0];
        let mins = [-1.0; 3];
        let maxs = [1.0; 3];

        let headnode_fn = |_: &Vec3, _: &Vec3| -> i32 { 0 };
        let trace_fn = |_: &Vec3, _: &Vec3, _: &Vec3, _: &Vec3, _: i32, _: i32, _: &Vec3, _: &Vec3| -> Trace {
            panic!("should not trace when already allsolid");
        };

        // When tr.allsolid is true, the function should return early
        // Note: the check happens AFTER the headnode computation but BEFORE the trace call
        // Actually, looking at the code, allsolid is checked inside the loop after computing headnode
        // but before calling cm_transformed_box_trace. Since we set allsolid before entering,
        // the first entity will compute headnode but then return before tracing.
        // This may panic on the headnode_fn call though. Let me adjust:

        let headnode_fn = |_: &Vec3, _: &Vec3| -> i32 { 0 };

        // Won't reach trace_fn because allsolid check is before it
        cl_clip_move_to_entities(
            &start, &mins, &maxs, &end, &mut tr, &cl, &parse_ents,
            &headnode_fn, &|_: &Vec3, _: &Vec3, _: &Vec3, _: &Vec3, _: i32, _: i32, _: &Vec3, _: &Vec3| -> Trace {
                panic!("should not be called when allsolid");
            },
        );

        assert!(tr.allsolid);
    }

    // -------------------------------------------------------
    // cl_pm_point_contents tests
    // -------------------------------------------------------

    #[test]
    fn test_pm_point_contents_world_only() {
        let cl = ClientState::default();
        let parse_ents = std::array::from_fn(|_| EntityState::default());

        let point = [100.0, 200.0, 300.0];

        let result = cl_pm_point_contents(
            &point,
            &cl,
            &parse_ents,
            &|_pt: &Vec3, _headnode: i32| -> i32 { 0x4 }, // CONTENTS_WATER
            &|_: &Vec3, _: i32, _: &Vec3, _: &Vec3| -> i32 { 0 },
        );

        assert_eq!(result, 0x4);
    }

    #[test]
    fn test_pm_point_contents_with_bmodel() {
        let mut cl = ClientState::default();
        cl.frame.num_entities = 1;
        cl.frame.parse_entities = 0;

        let mut parse_ents = std::array::from_fn(|_| EntityState::default());
        parse_ents[0].solid = 31; // bmodel
        parse_ents[0].modelindex = 1;

        // model_clip is a fixed-size array, just set the index
        cl.model_clip[1] = 5; // headnode 5

        let point = [0.0; 3];

        let result = cl_pm_point_contents(
            &point,
            &cl,
            &parse_ents,
            &|_: &Vec3, _: i32| -> i32 { 0x1 },  // CONTENTS_SOLID from world
            &|_: &Vec3, headnode: i32, _: &Vec3, _: &Vec3| -> i32 {
                assert_eq!(headnode, 5, "should use model_clip headnode");
                0x8 // CONTENTS_LAVA from bmodel
            },
        );

        // Combined: 0x1 | 0x8 = 0x9
        assert_eq!(result, 0x9);
    }

    // -------------------------------------------------------
    // cl_predict_movement - basic tests (requires pmove callback)
    // -------------------------------------------------------

    #[test]
    fn test_predict_movement_inactive_state_returns_early() {
        let mut cl = ClientState::default();
        let mut cls = ClientStatic::default();
        cls.state = ConnState::Disconnected; // not Active

        let mut pm_airaccel = 0.0f32;

        cl_predict_movement(
            &mut cl, &cls, 1.0, 0.0, 0.0, &mut pm_airaccel,
            &|_pm: &mut PmoveData| {},
        );

        // Should return early without modifying anything meaningful
        assert_eq!(cl.predicted_origin, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_predict_movement_paused_returns_early() {
        let mut cl = ClientState::default();
        let mut cls = ClientStatic::default();
        cls.state = ConnState::Active;

        let mut pm_airaccel = 0.0f32;

        cl_predict_movement(
            &mut cl, &cls, 1.0, 0.0, 1.0, /* paused */ &mut pm_airaccel,
            &|_pm: &mut PmoveData| {},
        );

        assert_eq!(cl.predicted_origin, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_predict_movement_no_prediction_sets_angles() {
        let mut cl = ClientState::default();
        let mut cls = ClientStatic::default();
        cls.state = ConnState::Active;

        cl.viewangles = [10.0, 20.0, 30.0];
        cl.frame.playerstate.pmove.delta_angles = [0, 0, 0];

        let mut pm_airaccel = 0.0f32;

        cl_predict_movement(
            &mut cl, &cls,
            0.0, // cl_predict disabled
            0.0, 0.0, &mut pm_airaccel,
            &|_pm: &mut PmoveData| {},
        );

        // When prediction is disabled, predicted_angles = viewangles + short2angle(delta_angles)
        for i in 0..3 {
            let expected = cl.viewangles[i] + short2angle(0);
            assert!((cl.predicted_angles[i] - expected).abs() < 0.01,
                "predicted_angles[{}]={} expected={}", i, cl.predicted_angles[i], expected);
        }
    }

    #[test]
    fn test_predict_movement_runs_pmove() {
        let mut cl = ClientState::default();
        let mut cls = ClientStatic::default();
        cls.state = ConnState::Active;
        cls.netchan.incoming_acknowledged = 10;
        cls.netchan.outgoing_sequence = 12;

        cl.frame.playerstate.pmove.origin = [800, 400, 200]; // 100.0, 50.0, 25.0

        let mut pmove_called_count = 0u32;
        let mut pm_airaccel = 0.0f32;

        // Simple pmove that just moves +8 in X each call (1.0 world units)
        cl_predict_movement(
            &mut cl, &cls, 1.0, 0.0, 0.0, &mut pm_airaccel,
            &|pm: &mut PmoveData| {
                pm.s.origin[0] += 8; // +1.0 world unit
            },
        );

        // From ack=10 to current=12, pmove runs for frames 11.
        // Starting at 800, after 1 call: 808
        // predicted_origin = 808 * 0.125 = 101.0
        assert!((cl.predicted_origin[0] - 101.0).abs() < 0.01,
            "predicted_origin[0]={}", cl.predicted_origin[0]);
    }

    // -------------------------------------------------------
    // PmoveClView tests
    // -------------------------------------------------------

    #[test]
    fn test_pmove_cl_view_trace_skips_player() {
        let view = PmoveClView {
            num_entities: 1,
            parse_entities: 0,
            playernum: 4,
            model_clip: &[0, 0],
        };

        let mut parse_ents = std::array::from_fn(|_| EntityState::default());
        parse_ents[0].solid = 31;
        parse_ents[0].number = 5; // playernum + 1

        let start = [0.0; 3];
        let end = [100.0, 0.0, 0.0];
        let mins = [-1.0; 3];
        let maxs = [1.0; 3];

        let box_trace = |_: &Vec3, _: &Vec3, _: &Vec3, _: &Vec3, _: i32, _: i32| -> Trace {
            Trace::default() // no world hit
        };
        let headnode_fn = |_: &Vec3, _: &Vec3| -> i32 { 0 };
        let trans_trace = |_: &Vec3, _: &Vec3, _: &Vec3, _: &Vec3, _: i32, _: i32, _: &Vec3, _: &Vec3| -> Trace {
            panic!("should not trace against player entity");
        };

        let result = cl_pm_trace_with_view(
            &start, &mins, &maxs, &end,
            &view, &parse_ents,
            &box_trace, &headnode_fn, &trans_trace,
        );

        assert!((result.fraction - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_pmove_cl_view_point_contents() {
        let view = PmoveClView {
            num_entities: 0,
            parse_entities: 0,
            playernum: 0,
            model_clip: &[],
        };
        let parse_ents = std::array::from_fn(|_| EntityState::default());
        let point = [0.0; 3];

        let result = cl_pm_point_contents_with_view(
            &point,
            &view,
            &parse_ents,
            &|_: &Vec3, _: i32| -> i32 { 0x2 }, // world contents
            &|_: &Vec3, _: i32, _: &Vec3, _: &Vec3| -> i32 { 0 },
        );

        assert_eq!(result, 0x2);
    }
}
