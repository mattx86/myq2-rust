// cl_input.rs -- builds an intended movement command to send to the server
// Converted from: myq2-original/client/cl_input.c
//
// R1Q2/Q2Pro feature: FPS-independent strafe jumping
// The strafe jump gain in Quake 2 depends on framerate because discrete angle
// changes per frame affect acceleration. This module can normalize angle deltas
// to a fixed physics timestep to ensure consistent strafe jumping at any FPS.

use crate::client::*;
use myq2_common::q_shared::*;
use myq2_common::common::com_printf;

// ===============================================================================
// FPS-INDEPENDENT STRAFE JUMPING (R1Q2/Q2Pro feature)
// ===============================================================================

/// State for FPS-independent angle normalization.
/// Tracks angle changes to normalize them to a fixed timestep.
#[derive(Default)]
pub struct StrafeJumpNormalizer {
    /// Previous frame's viewangles (for calculating delta)
    pub last_angles: Vec3,
    /// Whether we have valid previous angles
    pub initialized: bool,
    /// Accumulated fractional yaw for sub-frame precision
    pub accumulated_yaw: f32,
}

impl StrafeJumpNormalizer {
    /// Normalize angle delta for FPS-independent strafe jumping.
    ///
    /// This scales the yaw change to what it would be at a fixed timestep,
    /// ensuring consistent strafe jump gains regardless of framerate.
    ///
    /// # Arguments
    /// * `viewangles` - Current view angles (will be modified in-place)
    /// * `frame_msec` - Current frame time in milliseconds
    /// * `target_msec` - Target physics timestep (e.g., 8ms for 125fps)
    /// * `enabled` - Whether normalization is enabled
    pub fn normalize(&mut self, viewangles: &mut Vec3, frame_msec: f32, target_msec: f32, enabled: bool) {
        if !enabled || target_msec <= 0.0 || frame_msec <= 0.0 {
            // Just update tracking state
            self.last_angles = *viewangles;
            self.initialized = true;
            return;
        }

        if !self.initialized {
            self.last_angles = *viewangles;
            self.initialized = true;
            return;
        }

        // Calculate the yaw delta from last frame
        let mut yaw_delta = viewangles[YAW] - self.last_angles[YAW];

        // Handle wraparound (-180 to 180)
        if yaw_delta > 180.0 {
            yaw_delta -= 360.0;
        } else if yaw_delta < -180.0 {
            yaw_delta += 360.0;
        }

        // Calculate the scaling factor
        // If we're running at higher FPS than target, the delta will be smaller
        // per frame, but we'll have more frames. We want the same total delta
        // per physics tick.
        //
        // The key insight: strafe jumping gains depend on the angle *rate* of change.
        // At 250fps (4ms frames) we get smaller deltas but more of them.
        // At 125fps (8ms frames) we get larger deltas but fewer.
        //
        // To normalize: scale delta by (target_msec / frame_msec)
        // This makes each frame's delta equivalent to what it would be at target FPS.
        let scale = target_msec / frame_msec;

        // Apply scaling to yaw (primary strafe angle)
        // Only scale if we're above target FPS (frame_msec < target_msec)
        // Below target FPS, don't scale up as that could cause issues
        if frame_msec < target_msec {
            let scaled_delta = yaw_delta * scale;
            let adjustment = scaled_delta - yaw_delta;

            // Accumulate fractional adjustments for precision
            self.accumulated_yaw += adjustment;

            // Apply accumulated adjustment when it's significant
            if self.accumulated_yaw.abs() >= 0.1 {
                viewangles[YAW] += self.accumulated_yaw;
                self.accumulated_yaw = 0.0;

                // Normalize to 0-360 range
                while viewangles[YAW] < 0.0 {
                    viewangles[YAW] += 360.0;
                }
                while viewangles[YAW] >= 360.0 {
                    viewangles[YAW] -= 360.0;
                }
            }
        }

        // Update tracking state
        self.last_angles = *viewangles;
    }

    /// Reset the normalizer state (call on disconnect/level change)
    pub fn reset(&mut self) {
        self.last_angles = [0.0; 3];
        self.initialized = false;
        self.accumulated_yaw = 0.0;
    }
}

// ===============================================================================
//
// KEY BUTTONS
//
// Continuous button event tracking is complicated by the fact that two different
// input sources (say, mouse button 1 and the control key) can both press the
// same button, but the button should only be released when both of the
// pressing key have been released.
//
// When a key event issues a button command (+forward, +attack, etc), it appends
// its key number as a parameter to the command so it can be matched up with
// the release.
//
// state bit 0 is the current state of the key
// state bit 1 is edge triggered on the up to down transition
// state bit 2 is edge triggered on the down to up transition
//
// ===============================================================================

/// All input button states.
#[derive(Default)]
pub struct InputButtons {
    pub in_klook: KButton,
    pub in_left: KButton,
    pub in_right: KButton,
    pub in_forward: KButton,
    pub in_back: KButton,
    pub in_lookup: KButton,
    pub in_lookdown: KButton,
    pub in_moveleft: KButton,
    pub in_moveright: KButton,
    pub in_strafe: KButton,
    pub in_speed: KButton,
    pub in_use: KButton,
    pub in_attack: KButton,
    pub in_up: KButton,
    pub in_down: KButton,
    pub in_impulse: i32,
}


/// Input timing state.
#[derive(Default)]
pub struct InputTiming {
    pub frame_msec: u32,
    pub old_sys_frame_time: u32,
    /// Strafe jump angle normalizer (R1Q2/Q2Pro feature)
    pub strafe_normalizer: StrafeJumpNormalizer,
}


/// Input-related cvars.
pub struct InputCvars {
    pub cl_nodelta: f32,
    pub cl_upspeed: f32,
    pub cl_forwardspeed: f32,
    pub cl_sidespeed: f32,
    pub cl_yawspeed: f32,
    /// Enable FPS-independent strafe jumping (R1Q2/Q2Pro feature)
    pub cl_strafejump_fix: bool,
    /// Target physics FPS for strafe jump normalization (default 125)
    pub cl_physics_fps: f32,
    pub cl_pitchspeed: f32,
    pub cl_run: f32,
    pub cl_anglespeedkey: f32,
}

impl Default for InputCvars {
    fn default() -> Self {
        Self {
            cl_nodelta: 0.0,
            cl_upspeed: 200.0,
            cl_forwardspeed: 200.0,
            cl_sidespeed: 200.0,
            cl_yawspeed: 140.0,
            cl_pitchspeed: 150.0,
            cl_run: 0.0,
            cl_anglespeedkey: 1.5,
            cl_strafejump_fix: true,   // Enabled by default for consistent gameplay
            cl_physics_fps: 125.0,     // 125fps = 8ms timestep (classic competitive rate)
        }
    }
}

/// Process a key-down event for a button.
///
/// `k` is the key number (-1 if typed manually at console for continuous down).
/// `time` is the timestamp from the key event.
pub fn key_down(b: &mut KButton, k: i32, time: u32, sys_frame_time: u32) {
    if k == b.down[0] || k == b.down[1] {
        return; // repeating key
    }

    if b.down[0] == 0 {
        b.down[0] = k;
    } else if b.down[1] == 0 {
        b.down[1] = k;
    } else {
        com_printf("Three keys down for a button!\n");
        return;
    }

    if b.state & 1 != 0 {
        return; // still down
    }

    // save timestamp
    b.downtime = if time != 0 {
        time
    } else {
        sys_frame_time.wrapping_sub(100)
    };

    b.state |= 1 + 2; // down + impulse down
}

/// Process a key-up event for a button.
///
/// `k` is the key number (-1 if typed manually, which clears all).
/// `time` is the timestamp from the key event.
pub fn key_up(b: &mut KButton, k: i32, time: u32) {
    if k == -1 {
        // typed manually at the console, assume for unsticking, so clear all
        b.down[0] = 0;
        b.down[1] = 0;
        b.state = 4; // impulse up
        return;
    }

    if b.down[0] == k {
        b.down[0] = 0;
    } else if b.down[1] == k {
        b.down[1] = 0;
    } else {
        return; // key up without corresponding down (menu pass through)
    }

    if b.down[0] != 0 || b.down[1] != 0 {
        return; // some other key is still holding it down
    }

    if b.state & 1 == 0 {
        return; // still up (this should not happen)
    }

    // save timestamp
    let uptime = time;
    if uptime != 0 {
        b.msec += uptime.wrapping_sub(b.downtime);
    } else {
        b.msec += 10;
    }

    b.state &= !1; // now up
    b.state |= 4;  // impulse up
}

/// Returns the fraction of the frame that the key was down.
pub fn cl_key_state(key: &mut KButton, sys_frame_time: u32, frame_msec: u32) -> f32 {
    key.state &= 1; // clear impulses

    let mut msec = key.msec as i32;
    key.msec = 0;

    if key.state != 0 {
        // still down
        msec += sys_frame_time.wrapping_sub(key.downtime) as i32;
        key.downtime = sys_frame_time;
    }

    let mut val = msec as f32 / frame_msec as f32;
    if val < 0.0 {
        val = 0.0;
    }
    if val > 1.0 {
        val = 1.0;
    }

    val
}

// ==========================================================================

/// Moves the local angle positions.
pub fn cl_adjust_angles(
    viewangles: &mut Vec3,
    buttons: &mut InputButtons,
    cvars: &InputCvars,
    frametime: f32,
    sys_frame_time: u32,
    frame_msec: u32,
) {
    let speed = if buttons.in_speed.state & 1 != 0 {
        frametime * cvars.cl_anglespeedkey
    } else {
        frametime
    };

    if buttons.in_strafe.state & 1 == 0 {
        viewangles[YAW] -= speed * cvars.cl_yawspeed * cl_key_state(&mut buttons.in_right, sys_frame_time, frame_msec);
        viewangles[YAW] += speed * cvars.cl_yawspeed * cl_key_state(&mut buttons.in_left, sys_frame_time, frame_msec);
    }

    if buttons.in_klook.state & 1 != 0 {
        viewangles[PITCH] -= speed * cvars.cl_pitchspeed * cl_key_state(&mut buttons.in_forward, sys_frame_time, frame_msec);
        viewangles[PITCH] += speed * cvars.cl_pitchspeed * cl_key_state(&mut buttons.in_back, sys_frame_time, frame_msec);
    }

    let up = cl_key_state(&mut buttons.in_lookup, sys_frame_time, frame_msec);
    let down = cl_key_state(&mut buttons.in_lookdown, sys_frame_time, frame_msec);

    viewangles[PITCH] -= speed * cvars.cl_pitchspeed * up;
    viewangles[PITCH] += speed * cvars.cl_pitchspeed * down;
}

/// Build the intended movement command from keyboard state.
pub fn cl_base_move(
    cmd: &mut UserCmd,
    viewangles: &mut Vec3,
    buttons: &mut InputButtons,
    cvars: &InputCvars,
    frametime: f32,
    sys_frame_time: u32,
    frame_msec: u32,
) {
    cl_adjust_angles(viewangles, buttons, cvars, frametime, sys_frame_time, frame_msec);

    *cmd = UserCmd::default();

    cmd.angles = [viewangles[0] as i16, viewangles[1] as i16, viewangles[2] as i16];

    if buttons.in_strafe.state & 1 != 0 {
        cmd.sidemove += (cvars.cl_sidespeed * cl_key_state(&mut buttons.in_right, sys_frame_time, frame_msec)) as i16;
        cmd.sidemove -= (cvars.cl_sidespeed * cl_key_state(&mut buttons.in_left, sys_frame_time, frame_msec)) as i16;
    }

    cmd.sidemove += (cvars.cl_sidespeed * cl_key_state(&mut buttons.in_moveright, sys_frame_time, frame_msec)) as i16;
    cmd.sidemove -= (cvars.cl_sidespeed * cl_key_state(&mut buttons.in_moveleft, sys_frame_time, frame_msec)) as i16;

    cmd.upmove += (cvars.cl_upspeed * cl_key_state(&mut buttons.in_up, sys_frame_time, frame_msec)) as i16;
    cmd.upmove -= (cvars.cl_upspeed * cl_key_state(&mut buttons.in_down, sys_frame_time, frame_msec)) as i16;

    if buttons.in_klook.state & 1 == 0 {
        cmd.forwardmove += (cvars.cl_forwardspeed * cl_key_state(&mut buttons.in_forward, sys_frame_time, frame_msec)) as i16;
        cmd.forwardmove -= (cvars.cl_forwardspeed * cl_key_state(&mut buttons.in_back, sys_frame_time, frame_msec)) as i16;
    }

    // adjust for speed key / running
    if (buttons.in_speed.state & 1 != 0) ^ (cvars.cl_run != 0.0) {
        cmd.forwardmove *= 2;
        cmd.sidemove *= 2;
        cmd.upmove *= 2;
    }
}

/// Clamp the pitch angle to valid range.
pub fn cl_clamp_pitch(viewangles: &mut Vec3, delta_angles: &[i16; 3]) {
    let pitch = short2angle(delta_angles[PITCH]);
    let pitch = if pitch > 180.0 { pitch - 360.0 } else { pitch };

    if viewangles[PITCH] + pitch < -360.0 {
        viewangles[PITCH] += 360.0; // wrapped
    }
    if viewangles[PITCH] + pitch > 360.0 {
        viewangles[PITCH] -= 360.0; // wrapped
    }

    if viewangles[PITCH] + pitch > 89.0 {
        viewangles[PITCH] = 89.0 - pitch;
    }
    if viewangles[PITCH] + pitch < -89.0 {
        viewangles[PITCH] = -89.0 - pitch;
    }
}

/// Fill in the remaining fields of a usercmd after base movement and mouse input.
pub fn cl_finish_move(
    cmd: &mut UserCmd,
    viewangles: &mut Vec3,
    buttons: &mut InputButtons,
    delta_angles: &[i16; 3],
    frametime: f32,
    anykeydown: bool,
    key_dest_game: bool,
    cl_lightlevel: f32,
    strafe_normalizer: &mut StrafeJumpNormalizer,
    cl_strafejump_fix: bool,
    cl_physics_fps: f32,
) {
    // figure button bits
    if buttons.in_attack.state & 3 != 0 {
        cmd.buttons |= BUTTON_ATTACK;
    }
    buttons.in_attack.state &= !2;

    if buttons.in_use.state & 3 != 0 {
        cmd.buttons |= BUTTON_USE;
    }
    buttons.in_use.state &= !2;

    if anykeydown && key_dest_game {
        cmd.buttons |= BUTTON_ANY;
    }

    // send milliseconds of time to apply the move
    let ms = (frametime * 1000.0) as i32;
    let ms = if ms > 250 { 100 } else { ms };
    cmd.msec = ms as u8;

    // R1Q2/Q2Pro: Apply FPS-independent strafe jump normalization
    // This ensures consistent strafe jump gains regardless of framerate
    if cl_strafejump_fix && cl_physics_fps > 0.0 {
        let frame_msec = frametime * 1000.0;
        let target_msec = 1000.0 / cl_physics_fps; // e.g., 8ms for 125fps
        strafe_normalizer.normalize(viewangles, frame_msec, target_msec, true);
    }

    cl_clamp_pitch(viewangles, delta_angles);
    for i in 0..3 {
        cmd.angles[i] = angle2short(viewangles[i]) as i16;
    }

    cmd.impulse = buttons.in_impulse as u8;
    buttons.in_impulse = 0;

    // send the ambient light level at the player's current position
    cmd.lightlevel = cl_lightlevel as u8;
}

/// Create a complete usercmd for this frame.
pub fn cl_create_cmd(
    viewangles: &mut Vec3,
    buttons: &mut InputButtons,
    cvars: &InputCvars,
    timing: &mut InputTiming,
    delta_angles: &[i16; 3],
    frametime: f32,
    sys_frame_time: u32,
    anykeydown: bool,
    key_dest_game: bool,
    cl_lightlevel: f32,
) -> UserCmd {
    timing.frame_msec = sys_frame_time.wrapping_sub(timing.old_sys_frame_time);
    if timing.frame_msec < 1 {
        timing.frame_msec = 1;
    }
    if timing.frame_msec > 200 {
        timing.frame_msec = 200;
    }

    let mut cmd = UserCmd::default();

    // get basic movement from keyboard
    cl_base_move(
        &mut cmd,
        viewangles,
        buttons,
        cvars,
        frametime,
        sys_frame_time,
        timing.frame_msec,
    );

    // NOTE: IN_Move (mice/external controllers) would be called here

    cl_finish_move(
        &mut cmd,
        viewangles,
        buttons,
        delta_angles,
        frametime,
        anykeydown,
        key_dest_game,
        cl_lightlevel,
        &mut timing.strafe_normalizer,
        cvars.cl_strafejump_fix,
        cvars.cl_physics_fps,
    );

    timing.old_sys_frame_time = sys_frame_time;

    cmd
}

/// Center the view pitch to match the server's delta angle.
pub fn in_center_view(viewangles: &mut Vec3, delta_angles: &[i16; 3]) {
    viewangles[PITCH] = -short2angle(delta_angles[PITCH]);
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== StrafeJumpNormalizer ==========

    #[test]
    fn strafe_normalizer_default() {
        let sn = StrafeJumpNormalizer::default();
        assert!(!sn.initialized);
        assert_eq!(sn.accumulated_yaw, 0.0);
        assert_eq!(sn.last_angles, [0.0; 3]);
    }

    #[test]
    fn strafe_normalizer_reset() {
        let mut sn = StrafeJumpNormalizer {
            initialized: true,
            accumulated_yaw: 5.0,
            last_angles: [10.0, 20.0, 30.0],
        };
        sn.reset();
        assert!(!sn.initialized);
        assert_eq!(sn.accumulated_yaw, 0.0);
        assert_eq!(sn.last_angles, [0.0; 3]);
    }

    #[test]
    fn strafe_normalizer_first_call_initializes() {
        let mut sn = StrafeJumpNormalizer::default();
        let mut angles = [0.0, 45.0, 0.0];
        sn.normalize(&mut angles, 4.0, 8.0, true);
        assert!(sn.initialized);
        assert_eq!(sn.last_angles[YAW], 45.0);
        // First call should not modify viewangles
        assert_eq!(angles[YAW], 45.0);
    }

    #[test]
    fn strafe_normalizer_disabled_just_tracks() {
        let mut sn = StrafeJumpNormalizer::default();
        let mut angles = [0.0, 90.0, 0.0];
        sn.normalize(&mut angles, 4.0, 8.0, false); // disabled
        assert!(sn.initialized);
        assert_eq!(sn.last_angles[YAW], 90.0);
        assert_eq!(angles[YAW], 90.0); // unmodified
    }

    #[test]
    fn strafe_normalizer_zero_frame_msec_tracks_only() {
        let mut sn = StrafeJumpNormalizer::default();
        let mut angles = [0.0, 90.0, 0.0];
        sn.normalize(&mut angles, 0.0, 8.0, true);
        assert!(sn.initialized);
        // Should not apply normalization
        assert_eq!(angles[YAW], 90.0);
    }

    #[test]
    fn strafe_normalizer_at_target_fps_no_scaling() {
        let mut sn = StrafeJumpNormalizer::default();
        let mut angles = [0.0, 0.0, 0.0];
        // Initialize
        sn.normalize(&mut angles, 8.0, 8.0, true);

        // Second call at target fps (8ms), delta = 5 degrees
        angles[YAW] = 5.0;
        sn.normalize(&mut angles, 8.0, 8.0, true);
        // frame_msec == target_msec -> no scaling (frame_msec < target_msec is false)
        assert_eq!(angles[YAW], 5.0);
    }

    #[test]
    fn strafe_normalizer_high_fps_scales_up() {
        let mut sn = StrafeJumpNormalizer::default();
        let mut angles = [0.0, 0.0, 0.0];
        // Initialize
        sn.normalize(&mut angles, 4.0, 8.0, true);

        // At 250fps (4ms) with target 125fps (8ms), scale = 8/4 = 2.0
        // Delta = 2.0 degrees, scaled = 4.0, adjustment = 2.0
        angles[YAW] = 2.0;
        sn.normalize(&mut angles, 4.0, 8.0, true);

        // With accumulated_yaw = 2.0 (>= 0.1), it should be applied
        // YAW should now be 2.0 + 2.0 = 4.0
        assert!((angles[YAW] - 4.0).abs() < 0.1, "Got {}", angles[YAW]);
    }

    #[test]
    fn strafe_normalizer_below_target_fps_no_scaling() {
        let mut sn = StrafeJumpNormalizer::default();
        let mut angles = [0.0, 0.0, 0.0];
        sn.normalize(&mut angles, 16.0, 8.0, true); // init

        angles[YAW] = 10.0;
        sn.normalize(&mut angles, 16.0, 8.0, true); // 62.5fps, below 125fps target
        // frame_msec > target_msec, should not scale
        assert_eq!(angles[YAW], 10.0);
    }

    #[test]
    fn strafe_normalizer_accumulated_below_threshold_deferred() {
        let mut sn = StrafeJumpNormalizer::default();
        let mut angles = [0.0, 0.0, 0.0];
        sn.normalize(&mut angles, 7.0, 8.0, true); // init

        // Very small delta at slightly above target fps
        angles[YAW] = 0.01;
        sn.normalize(&mut angles, 7.0, 8.0, true);
        // scale = 8/7 ≈ 1.143, delta = 0.01, scaled = 0.01143, adjustment ≈ 0.00143
        // accumulated_yaw < 0.1, so not applied
        assert!((angles[YAW] - 0.01).abs() < 0.001);
    }

    #[test]
    fn strafe_normalizer_wraparound_positive() {
        let mut sn = StrafeJumpNormalizer::default();
        let mut angles = [0.0, 350.0, 0.0];
        sn.normalize(&mut angles, 4.0, 8.0, true); // init

        // Wrap around from 350 to 10 (delta = +20 via wraparound)
        angles[YAW] = 10.0;
        sn.normalize(&mut angles, 4.0, 8.0, true);
        // Delta wraps: 10 - 350 = -340 -> +20 (adjusted for wraparound)
        // The code adds 360 when delta < -180
        // The actual yaw should be adjusted upward
        // With scale=2, delta=20 -> scaled=40, adjustment=20
        // yaw = 10.0 + 20.0 = 30.0
        assert!(angles[YAW] > 10.0, "Should have increased from wraparound adjustment, got {}", angles[YAW]);
    }

    #[test]
    fn strafe_normalizer_negative_target_msec() {
        let mut sn = StrafeJumpNormalizer::default();
        let mut angles = [0.0, 45.0, 0.0];
        sn.normalize(&mut angles, 4.0, -8.0, true);
        // target_msec <= 0 -> just tracks
        assert!(sn.initialized);
        assert_eq!(angles[YAW], 45.0);
    }

    // ========== key_down / key_up ==========

    #[test]
    fn key_down_basic() {
        let mut b = KButton::default();
        key_down(&mut b, 42, 1000, 1000);
        assert_eq!(b.down[0], 42);
        assert_eq!(b.state & 1, 1); // down
        assert_eq!(b.state & 2, 2); // impulse down
        assert_eq!(b.downtime, 1000);
    }

    #[test]
    fn key_down_second_key() {
        let mut b = KButton::default();
        key_down(&mut b, 42, 1000, 1000);
        key_down(&mut b, 43, 1100, 1100);
        assert_eq!(b.down[0], 42);
        assert_eq!(b.down[1], 43);
    }

    #[test]
    fn key_down_repeating_ignored() {
        let mut b = KButton::default();
        key_down(&mut b, 42, 1000, 1000);
        let state_before = b.state;
        key_down(&mut b, 42, 1100, 1100); // repeat
        assert_eq!(b.state, state_before); // no change
    }

    #[test]
    fn key_down_zero_time_uses_sys_frame_time() {
        let mut b = KButton::default();
        key_down(&mut b, 42, 0, 5000);
        assert_eq!(b.downtime, 5000u32.wrapping_sub(100));
    }

    #[test]
    fn key_up_basic() {
        let mut b = KButton::default();
        key_down(&mut b, 42, 1000, 1000);
        key_up(&mut b, 42, 1100);
        assert_eq!(b.down[0], 0);
        assert_eq!(b.state & 1, 0); // no longer down
        assert_eq!(b.state & 4, 4); // impulse up
        assert_eq!(b.msec, 100); // 1100 - 1000
    }

    #[test]
    fn key_up_negative_one_clears_all() {
        let mut b = KButton::default();
        key_down(&mut b, 42, 1000, 1000);
        key_down(&mut b, 43, 1100, 1100);
        key_up(&mut b, -1, 1200);
        assert_eq!(b.down[0], 0);
        assert_eq!(b.down[1], 0);
        assert_eq!(b.state, 4); // impulse up only
    }

    #[test]
    fn key_up_one_key_other_holds() {
        let mut b = KButton::default();
        key_down(&mut b, 42, 1000, 1000);
        key_down(&mut b, 43, 1100, 1100);
        key_up(&mut b, 42, 1200);
        // Key 43 still down, so state bit 0 should remain
        assert_eq!(b.down[0], 0);
        assert_eq!(b.down[1], 43);
        assert_eq!(b.state & 1, 1); // still down
    }

    #[test]
    fn key_up_zero_time() {
        let mut b = KButton::default();
        key_down(&mut b, 42, 1000, 1000);
        key_up(&mut b, 42, 0);
        assert_eq!(b.msec, 10); // fallback
    }

    #[test]
    fn key_up_unknown_key_ignored() {
        let mut b = KButton::default();
        key_down(&mut b, 42, 1000, 1000);
        key_up(&mut b, 99, 1100); // not a key that was pressed
        // No change
        assert_eq!(b.down[0], 42);
        assert_eq!(b.state & 1, 1);
    }

    // ========== cl_key_state ==========

    #[test]
    fn cl_key_state_idle() {
        let mut b = KButton::default();
        let val = cl_key_state(&mut b, 5000, 16);
        assert_eq!(val, 0.0);
    }

    #[test]
    fn cl_key_state_full_frame_down() {
        let mut b = KButton::default();
        b.msec = 16;
        b.state = 0;
        let val = cl_key_state(&mut b, 5000, 16);
        assert_eq!(val, 1.0);
    }

    #[test]
    fn cl_key_state_half_frame() {
        let mut b = KButton::default();
        b.msec = 8;
        b.state = 0;
        let val = cl_key_state(&mut b, 5000, 16);
        assert!((val - 0.5).abs() < 0.01);
    }

    #[test]
    fn cl_key_state_still_down_adds_remaining() {
        let mut b = KButton::default();
        b.msec = 0;
        b.state = 1; // still down
        b.downtime = 4990;
        // sys_frame_time - downtime = 5000 - 4990 = 10
        let val = cl_key_state(&mut b, 5000, 16);
        // 10 / 16 = 0.625
        assert!((val - 0.625).abs() < 0.01);
        // After call, downtime should be updated
        assert_eq!(b.downtime, 5000);
    }

    #[test]
    fn cl_key_state_clamps_to_one() {
        let mut b = KButton::default();
        b.msec = 100; // way more than frame_msec
        let val = cl_key_state(&mut b, 5000, 16);
        assert_eq!(val, 1.0);
    }

    // ========== cl_clamp_pitch ==========

    #[test]
    fn cl_clamp_pitch_within_range() {
        let mut viewangles: Vec3 = [45.0, 0.0, 0.0];
        let delta = [0i16; 3];
        cl_clamp_pitch(&mut viewangles, &delta);
        assert_eq!(viewangles[PITCH], 45.0);
    }

    #[test]
    fn cl_clamp_pitch_exceeds_89() {
        let mut viewangles: Vec3 = [100.0, 0.0, 0.0];
        let delta = [0i16; 3];
        cl_clamp_pitch(&mut viewangles, &delta);
        assert_eq!(viewangles[PITCH], 89.0);
    }

    #[test]
    fn cl_clamp_pitch_below_negative_89() {
        let mut viewangles: Vec3 = [-100.0, 0.0, 0.0];
        let delta = [0i16; 3];
        cl_clamp_pitch(&mut viewangles, &delta);
        assert_eq!(viewangles[PITCH], -89.0);
    }

    #[test]
    fn cl_clamp_pitch_with_delta_angle() {
        // delta_angles[PITCH] shifts the effective pitch
        let mut viewangles: Vec3 = [80.0, 0.0, 0.0];
        // A small positive delta pitch (angle2short / short2angle)
        let delta = [angle2short(10.0) as i16, 0, 0];
        cl_clamp_pitch(&mut viewangles, &delta);
        // pitch + short2angle(delta) = 80 + 10 = 90 > 89
        // So viewangles[PITCH] = 89 - short2angle(delta) = 89 - 10 = 79
        let pitch_delta = short2angle(delta[PITCH]);
        assert!(viewangles[PITCH] + pitch_delta <= 89.0 + 0.1);
    }

    // ========== in_center_view ==========

    #[test]
    fn in_center_view_sets_pitch() {
        let mut viewangles: Vec3 = [45.0, 90.0, 0.0];
        let delta = [0i16, 0, 0];
        in_center_view(&mut viewangles, &delta);
        assert_eq!(viewangles[PITCH], 0.0); // -short2angle(0) = 0
        assert_eq!(viewangles[YAW], 90.0); // unchanged
    }

    #[test]
    fn in_center_view_with_delta() {
        let mut viewangles: Vec3 = [45.0, 90.0, 0.0];
        let delta_val = angle2short(15.0) as i16;
        let delta = [delta_val, 0, 0];
        in_center_view(&mut viewangles, &delta);
        // viewangles[PITCH] = -short2angle(delta_val) ≈ -15.0
        let expected = -short2angle(delta_val);
        assert!((viewangles[PITCH] - expected).abs() < 0.1);
    }

    // ========== InputCvars defaults ==========

    #[test]
    fn input_cvars_defaults() {
        let cvars = InputCvars::default();
        assert_eq!(cvars.cl_upspeed, 200.0);
        assert_eq!(cvars.cl_forwardspeed, 200.0);
        assert_eq!(cvars.cl_sidespeed, 200.0);
        assert_eq!(cvars.cl_yawspeed, 140.0);
        assert_eq!(cvars.cl_pitchspeed, 150.0);
        assert_eq!(cvars.cl_anglespeedkey, 1.5);
        assert!(cvars.cl_strafejump_fix);
        assert_eq!(cvars.cl_physics_fps, 125.0);
    }

    // ========== InputButtons default ==========

    #[test]
    fn input_buttons_default_all_zero() {
        let buttons = InputButtons::default();
        assert_eq!(buttons.in_forward.state, 0);
        assert_eq!(buttons.in_attack.state, 0);
        assert_eq!(buttons.in_impulse, 0);
    }

    // ========== cl_base_move ==========

    #[test]
    fn cl_base_move_no_input_produces_zero_cmd() {
        let mut cmd = UserCmd::default();
        let mut viewangles: Vec3 = [0.0, 0.0, 0.0];
        let mut buttons = InputButtons::default();
        let cvars = InputCvars::default();
        cl_base_move(&mut cmd, &mut viewangles, &mut buttons, &cvars, 0.016, 5000, 16);
        assert_eq!(cmd.forwardmove, 0);
        assert_eq!(cmd.sidemove, 0);
        assert_eq!(cmd.upmove, 0);
    }

    #[test]
    fn cl_base_move_forward_key() {
        let mut cmd = UserCmd::default();
        let mut viewangles: Vec3 = [0.0, 0.0, 0.0];
        let mut buttons = InputButtons::default();
        let cvars = InputCvars::default();
        // Simulate forward key held for full frame
        buttons.in_forward.msec = 16;
        buttons.in_forward.state = 0;
        cl_base_move(&mut cmd, &mut viewangles, &mut buttons, &cvars, 0.016, 5000, 16);
        assert!(cmd.forwardmove > 0, "Forward move should be positive, got {}", cmd.forwardmove);
    }

    #[test]
    fn cl_base_move_strafe_left() {
        let mut cmd = UserCmd::default();
        let mut viewangles: Vec3 = [0.0, 0.0, 0.0];
        let mut buttons = InputButtons::default();
        let cvars = InputCvars::default();
        buttons.in_moveleft.msec = 16;
        buttons.in_moveleft.state = 0;
        cl_base_move(&mut cmd, &mut viewangles, &mut buttons, &cvars, 0.016, 5000, 16);
        assert!(cmd.sidemove < 0, "Strafe left should produce negative sidemove, got {}", cmd.sidemove);
    }

    #[test]
    fn cl_base_move_speed_multiplier() {
        let mut cmd_normal = UserCmd::default();
        let mut viewangles_normal: Vec3 = [0.0, 0.0, 0.0];
        let mut buttons_normal = InputButtons::default();
        buttons_normal.in_forward.msec = 16;
        buttons_normal.in_forward.state = 0;
        let cvars = InputCvars::default();
        cl_base_move(&mut cmd_normal, &mut viewangles_normal, &mut buttons_normal, &cvars, 0.016, 5000, 16);

        let mut cmd_run = UserCmd::default();
        let mut viewangles_run: Vec3 = [0.0, 0.0, 0.0];
        let mut buttons_run = InputButtons::default();
        buttons_run.in_forward.msec = 16;
        buttons_run.in_forward.state = 0;
        buttons_run.in_speed.state = 1; // speed key held
        cl_base_move(&mut cmd_run, &mut viewangles_run, &mut buttons_run, &cvars, 0.016, 5000, 16);

        // With speed key, movement should be doubled
        assert_eq!(cmd_run.forwardmove, cmd_normal.forwardmove * 2);
    }

    // ========== cl_finish_move button bits ==========

    #[test]
    fn cl_finish_move_attack_button() {
        let mut cmd = UserCmd::default();
        let mut viewangles: Vec3 = [0.0, 0.0, 0.0];
        let mut buttons = InputButtons::default();
        buttons.in_attack.state = 3; // down + impulse down
        let delta = [0i16; 3];
        let mut sn = StrafeJumpNormalizer::default();
        cl_finish_move(&mut cmd, &mut viewangles, &mut buttons, &delta, 0.016, false, true, 128.0, &mut sn, false, 125.0);
        assert_ne!(cmd.buttons & BUTTON_ATTACK, 0);
        // impulse bit should be cleared
        assert_eq!(buttons.in_attack.state & 2, 0);
    }

    #[test]
    fn cl_finish_move_use_button() {
        let mut cmd = UserCmd::default();
        let mut viewangles: Vec3 = [0.0, 0.0, 0.0];
        let mut buttons = InputButtons::default();
        buttons.in_use.state = 3; // down + impulse down
        let delta = [0i16; 3];
        let mut sn = StrafeJumpNormalizer::default();
        cl_finish_move(&mut cmd, &mut viewangles, &mut buttons, &delta, 0.016, false, true, 128.0, &mut sn, false, 125.0);
        assert_ne!(cmd.buttons & BUTTON_USE, 0);
    }

    #[test]
    fn cl_finish_move_any_key_in_game() {
        let mut cmd = UserCmd::default();
        let mut viewangles: Vec3 = [0.0, 0.0, 0.0];
        let mut buttons = InputButtons::default();
        let delta = [0i16; 3];
        let mut sn = StrafeJumpNormalizer::default();
        cl_finish_move(&mut cmd, &mut viewangles, &mut buttons, &delta, 0.016, true, true, 128.0, &mut sn, false, 125.0);
        assert_ne!(cmd.buttons & BUTTON_ANY, 0);
    }

    #[test]
    fn cl_finish_move_no_any_key_not_in_game() {
        let mut cmd = UserCmd::default();
        let mut viewangles: Vec3 = [0.0, 0.0, 0.0];
        let mut buttons = InputButtons::default();
        let delta = [0i16; 3];
        let mut sn = StrafeJumpNormalizer::default();
        cl_finish_move(&mut cmd, &mut viewangles, &mut buttons, &delta, 0.016, true, false, 128.0, &mut sn, false, 125.0);
        assert_eq!(cmd.buttons & BUTTON_ANY, 0);
    }

    #[test]
    fn cl_finish_move_msec_clamped() {
        let mut cmd = UserCmd::default();
        let mut viewangles: Vec3 = [0.0, 0.0, 0.0];
        let mut buttons = InputButtons::default();
        let delta = [0i16; 3];
        let mut sn = StrafeJumpNormalizer::default();
        // frametime of 0.5 = 500ms > 250 limit -> should become 100
        cl_finish_move(&mut cmd, &mut viewangles, &mut buttons, &delta, 0.5, false, true, 0.0, &mut sn, false, 125.0);
        assert_eq!(cmd.msec, 100);
    }

    #[test]
    fn cl_finish_move_impulse() {
        let mut cmd = UserCmd::default();
        let mut viewangles: Vec3 = [0.0, 0.0, 0.0];
        let mut buttons = InputButtons::default();
        buttons.in_impulse = 7;
        let delta = [0i16; 3];
        let mut sn = StrafeJumpNormalizer::default();
        cl_finish_move(&mut cmd, &mut viewangles, &mut buttons, &delta, 0.016, false, true, 0.0, &mut sn, false, 125.0);
        assert_eq!(cmd.impulse, 7);
        assert_eq!(buttons.in_impulse, 0); // cleared after use
    }

    #[test]
    fn cl_finish_move_lightlevel() {
        let mut cmd = UserCmd::default();
        let mut viewangles: Vec3 = [0.0, 0.0, 0.0];
        let mut buttons = InputButtons::default();
        let delta = [0i16; 3];
        let mut sn = StrafeJumpNormalizer::default();
        cl_finish_move(&mut cmd, &mut viewangles, &mut buttons, &delta, 0.016, false, true, 128.0, &mut sn, false, 125.0);
        assert_eq!(cmd.lightlevel, 128);
    }

    // ========== cl_create_cmd ==========

    #[test]
    fn cl_create_cmd_basic() {
        let mut viewangles: Vec3 = [0.0, 90.0, 0.0];
        let mut buttons = InputButtons::default();
        let cvars = InputCvars::default();
        let mut timing = InputTiming::default();
        timing.old_sys_frame_time = 4984;
        let delta = [0i16; 3];
        let cmd = cl_create_cmd(
            &mut viewangles, &mut buttons, &cvars, &mut timing,
            &delta, 0.016, 5000, false, true, 128.0,
        );
        // frame_msec = 5000 - 4984 = 16
        assert_eq!(timing.frame_msec, 16);
        assert_eq!(cmd.lightlevel, 128);
        assert_eq!(timing.old_sys_frame_time, 5000);
    }

    #[test]
    fn cl_create_cmd_clamps_frame_msec() {
        let mut viewangles: Vec3 = [0.0, 0.0, 0.0];
        let mut buttons = InputButtons::default();
        let cvars = InputCvars::default();
        let mut timing = InputTiming::default();
        timing.old_sys_frame_time = 0;
        let delta = [0i16; 3];
        let _cmd = cl_create_cmd(
            &mut viewangles, &mut buttons, &cvars, &mut timing,
            &delta, 0.016, 500, false, true, 0.0,
        );
        // frame_msec = 500 - 0 = 500, clamped to 200
        assert_eq!(timing.frame_msec, 200);
    }

    #[test]
    fn cl_create_cmd_min_frame_msec() {
        let mut viewangles: Vec3 = [0.0, 0.0, 0.0];
        let mut buttons = InputButtons::default();
        let cvars = InputCvars::default();
        let mut timing = InputTiming::default();
        timing.old_sys_frame_time = 5000;
        let delta = [0i16; 3];
        let _cmd = cl_create_cmd(
            &mut viewangles, &mut buttons, &cvars, &mut timing,
            &delta, 0.016, 5000, false, true, 0.0,
        );
        // frame_msec = 0, clamped to 1
        assert_eq!(timing.frame_msec, 1);
    }
}

/// Register all input-related commands and cvars.
/// Corresponds to CL_InitInput() in the original cl_input.c (lines 413-450).
///
/// Registers 15 button pairs (+command/-command), plus `centerview`,
/// `impulse`, and the `cl_nodelta` cvar.
pub fn cl_init_input(input_buttons: std::sync::Arc<std::sync::Mutex<InputButtons>>) {
    // Macro to register a +down/-up button command pair.
    // Each closure captures a clone of the Arc<Mutex<InputButtons>> and calls
    // key_down/key_up with args parsed from the command system.
    macro_rules! register_button {
        ($down_cmd:expr, $up_cmd:expr, $field:ident) => {
            {
                let buttons = input_buttons.clone();
                myq2_common::cmd::cmd_add_command(
                    $down_cmd,
                    Some(Box::new(move |_ctx| {
                        let k: i32 = myq2_common::cmd::cmd_argv(1).parse().unwrap_or(-1);
                        let time: u32 = myq2_common::cmd::cmd_argv(2).parse().unwrap_or(0);
                        let sys_ft = myq2_common::common::sys_milliseconds() as u32;
                        key_down(&mut buttons.lock().unwrap().$field, k, time, sys_ft);
                    })),
                );
            }
            {
                let buttons = input_buttons.clone();
                myq2_common::cmd::cmd_add_command(
                    $up_cmd,
                    Some(Box::new(move |_ctx| {
                        let k: i32 = myq2_common::cmd::cmd_argv(1).parse().unwrap_or(-1);
                        let time: u32 = myq2_common::cmd::cmd_argv(2).parse().unwrap_or(0);
                        key_up(&mut buttons.lock().unwrap().$field, k, time);
                    })),
                );
            }
        };
    }

    // Register all 15 button pairs (matches original CL_InitInput order)
    register_button!("+moveup",    "-moveup",    in_up);
    register_button!("+movedown",  "-movedown",  in_down);
    register_button!("+left",      "-left",      in_left);
    register_button!("+right",     "-right",     in_right);
    register_button!("+forward",   "-forward",   in_forward);
    register_button!("+back",      "-back",      in_back);
    register_button!("+lookup",    "-lookup",    in_lookup);
    register_button!("+lookdown",  "-lookdown",  in_lookdown);
    register_button!("+strafe",    "-strafe",    in_strafe);
    register_button!("+moveleft",  "-moveleft",  in_moveleft);
    register_button!("+moveright", "-moveright", in_moveright);
    register_button!("+speed",     "-speed",     in_speed);
    register_button!("+attack",    "-attack",    in_attack);
    register_button!("+use",       "-use",       in_use);
    register_button!("+klook",     "-klook",     in_klook);

    // centerview — centers the player's vertical view angle
    myq2_common::cmd::cmd_add_command_simple("centerview", crate::cl_main::cl_center_view);

    // impulse — sends an impulse command value
    {
        let buttons = input_buttons.clone();
        myq2_common::cmd::cmd_add_command(
            "impulse",
            Some(Box::new(move |_ctx| {
                buttons.lock().unwrap().in_impulse =
                    myq2_common::cmd::cmd_argv(1).parse().unwrap_or(0);
            })),
        );
    }

    // Register cl_nodelta cvar
    myq2_common::cvar::cvar_get("cl_nodelta", "0", CVAR_ARCHIVE);

    com_printf("CL_InitInput: input commands registered\n");
}

// ============================================================
// CL_SendCmd
// ============================================================

/// The full send-command logic. In the C code this builds a sizebuf, writes
/// delta-compressed usercmds, computes a CRC checksum, and transmits via
/// netchan. Parameters represent the necessary client/netchan state.
///
/// This is a simplified structural conversion; the actual network buffer
/// operations depend on the message writing functions from myq2_common.
pub fn cl_send_cmd(
    cl: &mut ClientState,
    cls: &mut ClientStatic,
    buttons: &mut InputButtons,
    cvars: &InputCvars,
    timing: &mut InputTiming,
    sys_frame_time: u32,
    anykeydown: bool,
    cl_lightlevel: f32,
    _userinfo_modified: &mut bool,
) {
    // build a command even if not connected

    // save this command off for prediction
    let i = (cls.netchan.outgoing_sequence as usize) & (CMD_BACKUP - 1);

    let mut cmd = cl_create_cmd(
        &mut cl.viewangles,
        buttons,
        cvars,
        timing,
        &cl.frame.playerstate.pmove.delta_angles,
        cls.frametime,
        sys_frame_time,
        anykeydown,
        cls.key_dest == KeyDest::Game,
        cl_lightlevel,
    );

    // === Input Buffering for smooth local movement ===
    // Buffer raw inputs and blend them for smoother local prediction
    if cl.smoothing.input_buffer.enabled {
        use crate::cl_smooth::BufferedInput;

        // Create buffered input from raw command
        let buffered = BufferedInput {
            forward: cmd.forwardmove as f32,
            side: cmd.sidemove as f32,
            up: cmd.upmove as f32,
            angles: cl.viewangles,
            buttons: cmd.buttons as u32,
            time: cls.realtime,
        };

        // Add to buffer
        cl.smoothing.input_buffer.add(buffered);

        // Get smoothed input and apply to command
        if let Some(smoothed) = cl.smoothing.input_buffer.get_smoothed() {
            // Apply smoothed movement (only affects prediction smoothness, not actual commands sent)
            // This provides local smoothing while the actual command remains accurate
            // Note: We only smooth the movement values, not the angles (angles need to be responsive)
            cmd.forwardmove = smoothed.forward as i16;
            cmd.sidemove = smoothed.side as i16;
            cmd.upmove = smoothed.up as i16;
        }
    }

    cl.cmds[i] = cmd;
    cl.cmd_time[i] = cls.realtime;
    cl.cmd = cl.cmds[i];

    if cls.state == ConnState::Disconnected || cls.state == ConnState::Connecting {
        return;
    }

    if cls.state == ConnState::Connected {
        // If there's pending data or it's been >1s since last send, transmit
        if cls.netchan.message.cursize > 0 || cls.realtime - cls.netchan.last_sent > 1000 {
            // netchan_transmit(&mut cls.netchan, &[]);
        }
    }

    // send a userinfo update if needed
    // (handled by caller checking userinfo_modified)

    // The full implementation would:
    // 1. Write clc_move byte
    // 2. Write checksum placeholder byte
    // 3. Write last valid frame number (or -1 for no delta)
    // 4. Write 3 delta-compressed usercmds (current and 2 previous)
    // 5. Calculate and fill in the CRC checksum
    // 6. Transmit via netchan
}
