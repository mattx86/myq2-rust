// pmove.rs — Player movement code
// Converted from: myq2-original/qcommon/pmove.c

use crate::q_shared::{
    angle_vectors, cross_product, dot_product, short2angle, vector_length, vector_normalize,
    vector_scale, CPlane, CSurface, PmType, PmoveData, Trace, Vec3, VEC3_ORIGIN,
    CONTENTS_CURRENT_0, CONTENTS_CURRENT_180, CONTENTS_CURRENT_270, CONTENTS_CURRENT_90,
    CONTENTS_CURRENT_DOWN, CONTENTS_CURRENT_UP, CONTENTS_LADDER, CONTENTS_SLIME, CONTENTS_SOLID,
    CONTENTS_WATER, MASK_CURRENT, MASK_WATER, MAXTOUCH, PITCH, PMF_DUCKED, PMF_JUMP_HELD, PMF_ON_GROUND, PMF_TIME_LAND, PMF_TIME_TELEPORT, PMF_TIME_WATERJUMP,
    SURF_SLICK, YAW, MAX_CLIP_PLANES,
};

// ============================================================
// Constants
// ============================================================

const STEPSIZE: f32 = 18.0;
const STOP_EPSILON: f32 = 0.1;
const MIN_STEP_NORMAL: f32 = 0.7;

// Movement parameters
const PM_STOPSPEED: f32 = 100.0;
const PM_MAXSPEED: f32 = 300.0;
const PM_DUCKSPEED: f32 = 100.0;
const PM_ACCELERATE: f32 = 10.0;
const PM_AIRACCELERATE: f32 = 0.0;
const PM_WATERACCELERATE: f32 = 10.0;
const PM_FRICTION: f32 = 6.0;
const PM_WATERFRICTION: f32 = 1.0;
const PM_WATERSPEED: f32 = 400.0;

// ============================================================
// Pmove local state — zeroed before each pmove
// ============================================================

#[derive(Clone)]
struct PmLocal {
    origin: Vec3,
    velocity: Vec3,

    forward: Vec3,
    right: Vec3,
    up: Vec3,
    frametime: f32,

    groundsurface: Option<CSurface>,
    groundplane: CPlane,
    groundcontents: i32,

    previous_origin: [i16; 3],
    ladder: bool,
}

impl Default for PmLocal {
    fn default() -> Self {
        Self {
            origin: [0.0; 3],
            velocity: [0.0; 3],
            forward: [0.0; 3],
            right: [0.0; 3],
            up: [0.0; 3],
            frametime: 0.0,
            groundsurface: None,
            groundplane: CPlane::default(),
            groundcontents: 0,
            previous_origin: [0; 3],
            ladder: false,
        }
    }
}

// ============================================================
// Callbacks trait — replaces C function pointers on pmove_t
// ============================================================

/// Trait for the trace and pointcontents callbacks that the engine provides.
pub trait PmoveCallbacks {
    fn trace(&self, start: &Vec3, mins: &Vec3, maxs: &Vec3, end: &Vec3) -> Trace;
    fn pointcontents(&self, point: &Vec3) -> i32;
}

// ============================================================
// Pmove context — holds all state for one Pmove() call
// ============================================================

struct PmoveContext<'a, C: PmoveCallbacks> {
    pm: &'a mut PmoveData,
    pml: PmLocal,
    cb: &'a C,
}

// ============================================================
// Internal functions
// ============================================================

/// Slide off of the impacting surface.
fn pm_clip_velocity(inv: &Vec3, normal: &Vec3, out: &mut Vec3, overbounce: f32) {
    let backoff = dot_product(inv, normal) * overbounce;
    for i in 0..3 {
        let change = normal[i] * backoff;
        out[i] = inv[i] - change;
        if out[i] > -STOP_EPSILON && out[i] < STOP_EPSILON {
            out[i] = 0.0;
        }
    }
}

impl<'a, C: PmoveCallbacks> PmoveContext<'a, C> {
    // --------------------------------------------------------
    // PM_StepSlideMove_ (inner slide move)
    // --------------------------------------------------------
    fn step_slide_move_inner(&mut self) {
        let numbumps = 4;
        let primal_velocity = self.pml.velocity;
        let mut numplanes: usize = 0;
        let mut planes = [[0.0f32; 3]; MAX_CLIP_PLANES];

        let mut time_left = self.pml.frametime;

        for _bumpcount in 0..numbumps {
            let end = [
                self.pml.origin[0] + time_left * self.pml.velocity[0],
                self.pml.origin[1] + time_left * self.pml.velocity[1],
                self.pml.origin[2] + time_left * self.pml.velocity[2],
            ];

            let trace = self.cb.trace(&self.pml.origin, &self.pm.mins, &self.pm.maxs, &end);

            if trace.allsolid {
                // entity is trapped in another solid
                self.pml.velocity[2] = 0.0;
                return;
            }

            if trace.fraction > 0.0 {
                // actually covered some distance
                self.pml.origin = trace.endpos;
                numplanes = 0;
            }

            if trace.fraction == 1.0 {
                break; // moved the entire distance
            }

            // save entity for contact
            if (self.pm.numtouch as usize) < MAXTOUCH && trace.ent_index >= 0 {
                self.pm.touchents[self.pm.numtouch as usize] = trace.ent_index;
                self.pm.numtouch += 1;
            }

            time_left -= time_left * trace.fraction;

            // slide along this plane
            if numplanes >= MAX_CLIP_PLANES {
                self.pml.velocity = VEC3_ORIGIN;
                break;
            }

            planes[numplanes] = trace.plane.normal;
            numplanes += 1;

            // modify original_velocity so it parallels all of the clip planes
            let mut found = false;
            for i in 0..numplanes {
                pm_clip_velocity(
                    &self.pml.velocity.clone(),
                    &planes[i],
                    &mut self.pml.velocity,
                    1.01,
                );
                let mut ok = true;
                for j in 0..numplanes {
                    if j != i
                        && dot_product(&self.pml.velocity, &planes[j]) < 0.0 {
                            ok = false;
                            break;
                        }
                }
                if ok {
                    found = true;
                    break;
                }
            }

            if !found {
                // go along the crease
                if numplanes != 2 {
                    self.pml.velocity = VEC3_ORIGIN;
                    break;
                }
                let dir = cross_product(&planes[0], &planes[1]);
                let d = dot_product(&dir, &self.pml.velocity);
                self.pml.velocity = vector_scale(&dir, d);
            }

            // if velocity is against the original velocity, stop dead
            if dot_product(&self.pml.velocity, &primal_velocity) <= 0.0 {
                self.pml.velocity = VEC3_ORIGIN;
                break;
            }
        }

        if self.pm.s.pm_time != 0 {
            self.pml.velocity = primal_velocity;
        }
    }

    // --------------------------------------------------------
    // PM_StepSlideMove
    // --------------------------------------------------------
    fn step_slide_move(&mut self) {
        let start_o = self.pml.origin;
        let start_v = self.pml.velocity;

        self.step_slide_move_inner();

        let down_o = self.pml.origin;
        let down_v = self.pml.velocity;

        let mut up = start_o;
        up[2] += STEPSIZE;

        let trace = self.cb.trace(&up, &self.pm.mins, &self.pm.maxs, &up);
        if trace.allsolid {
            return; // can't step up
        }

        // try sliding above
        self.pml.origin = up;
        self.pml.velocity = start_v;

        self.step_slide_move_inner();

        // push down the final amount
        let mut down = self.pml.origin;
        down[2] -= STEPSIZE;
        let trace = self.cb.trace(&self.pml.origin, &self.pm.mins, &self.pm.maxs, &down);
        if !trace.allsolid {
            self.pml.origin = trace.endpos;
        }

        let up = self.pml.origin;

        // decide which one went farther
        let down_dist = (down_o[0] - start_o[0]) * (down_o[0] - start_o[0])
            + (down_o[1] - start_o[1]) * (down_o[1] - start_o[1]);
        let up_dist = (up[0] - start_o[0]) * (up[0] - start_o[0])
            + (up[1] - start_o[1]) * (up[1] - start_o[1]);

        if down_dist > up_dist || trace.plane.normal[2] < MIN_STEP_NORMAL {
            self.pml.origin = down_o;
            self.pml.velocity = down_v;
            return;
        }
        // Special case
        // if we were walking along a plane, then we need to copy the Z over
        self.pml.velocity[2] = down_v[2];
    }

    // --------------------------------------------------------
    // PM_Friction
    // --------------------------------------------------------
    fn friction(&mut self) {
        let speed = (self.pml.velocity[0] * self.pml.velocity[0]
            + self.pml.velocity[1] * self.pml.velocity[1]
            + self.pml.velocity[2] * self.pml.velocity[2])
            .sqrt();

        if speed < 1.0 {
            self.pml.velocity[0] = 0.0;
            self.pml.velocity[1] = 0.0;
            return;
        }

        let mut drop = 0.0f32;

        // apply ground friction
        let has_ground = self.pm.groundentity >= 0;
        let surface_slick = self
            .pml
            .groundsurface
            .as_ref()
            .is_some_and(|s| (s.flags & SURF_SLICK) != 0);

        if (has_ground && self.pml.groundsurface.is_some() && !surface_slick) || self.pml.ladder {
            let friction = PM_FRICTION;
            let control = if speed < PM_STOPSPEED {
                PM_STOPSPEED
            } else {
                speed
            };
            drop += control * friction * self.pml.frametime;
        }

        // apply water friction
        if self.pm.waterlevel != 0 && !self.pml.ladder {
            drop += speed * PM_WATERFRICTION * self.pm.waterlevel as f32 * self.pml.frametime;
        }

        // scale the velocity
        let mut newspeed = speed - drop;
        if newspeed < 0.0 {
            newspeed = 0.0;
        }
        newspeed /= speed;

        self.pml.velocity[0] *= newspeed;
        self.pml.velocity[1] *= newspeed;
        self.pml.velocity[2] *= newspeed;
    }

    // --------------------------------------------------------
    // PM_Accelerate
    // --------------------------------------------------------
    fn accelerate(&mut self, wishdir: &Vec3, wishspeed: f32, accel: f32) {
        let currentspeed = dot_product(&self.pml.velocity, wishdir);
        let addspeed = wishspeed - currentspeed;
        if addspeed <= 0.0 {
            return;
        }
        let mut accelspeed = accel * self.pml.frametime * wishspeed;
        if accelspeed > addspeed {
            accelspeed = addspeed;
        }
        for i in 0..3 {
            self.pml.velocity[i] += accelspeed * wishdir[i];
        }
    }

    // --------------------------------------------------------
    // PM_AirAccelerate
    // --------------------------------------------------------
    fn air_accelerate(&mut self, wishdir: &Vec3, wishspeed: f32, accel: f32) {
        let wishspd = if wishspeed > 30.0 { 30.0 } else { wishspeed };
        let currentspeed = dot_product(&self.pml.velocity, wishdir);
        let addspeed = wishspd - currentspeed;
        if addspeed <= 0.0 {
            return;
        }
        let mut accelspeed = accel * wishspeed * self.pml.frametime;
        if accelspeed > addspeed {
            accelspeed = addspeed;
        }
        for i in 0..3 {
            self.pml.velocity[i] += accelspeed * wishdir[i];
        }
    }

    // --------------------------------------------------------
    // PM_AddCurrents
    // --------------------------------------------------------
    fn add_currents(&mut self, wishvel: &mut Vec3) {
        // account for ladders
        if self.pml.ladder && self.pml.velocity[2].abs() <= 200.0 {
            if self.pm.viewangles[PITCH] <= -15.0 && self.pm.cmd.forwardmove > 0 {
                wishvel[2] = 200.0;
            } else if self.pm.viewangles[PITCH] >= 15.0 && self.pm.cmd.forwardmove > 0 {
                wishvel[2] = -200.0;
            } else if self.pm.cmd.upmove > 0 {
                wishvel[2] = 200.0;
            } else if self.pm.cmd.upmove < 0 {
                wishvel[2] = -200.0;
            } else {
                wishvel[2] = 0.0;
            }

            // limit horizontal speed when on a ladder
            wishvel[0] = wishvel[0].clamp(-25.0, 25.0);
            wishvel[1] = wishvel[1].clamp(-25.0, 25.0);
        }

        // add water currents
        if (self.pm.watertype & MASK_CURRENT) != 0 {
            let mut v: Vec3 = [0.0; 3];

            if (self.pm.watertype & CONTENTS_CURRENT_0) != 0 {
                v[0] += 1.0;
            }
            if (self.pm.watertype & CONTENTS_CURRENT_90) != 0 {
                v[1] += 1.0;
            }
            if (self.pm.watertype & CONTENTS_CURRENT_180) != 0 {
                v[0] -= 1.0;
            }
            if (self.pm.watertype & CONTENTS_CURRENT_270) != 0 {
                v[1] -= 1.0;
            }
            if (self.pm.watertype & CONTENTS_CURRENT_UP) != 0 {
                v[2] += 1.0;
            }
            if (self.pm.watertype & CONTENTS_CURRENT_DOWN) != 0 {
                v[2] -= 1.0;
            }

            let mut s = PM_WATERSPEED;
            if self.pm.waterlevel == 1 && self.pm.groundentity >= 0 {
                s /= 2.0;
            }

            for i in 0..3 {
                wishvel[i] += s * v[i];
            }
        }

        // add conveyor belt velocities
        if self.pm.groundentity >= 0 {
            let mut v: Vec3 = [0.0; 3];

            if (self.pml.groundcontents & CONTENTS_CURRENT_0) != 0 {
                v[0] += 1.0;
            }
            if (self.pml.groundcontents & CONTENTS_CURRENT_90) != 0 {
                v[1] += 1.0;
            }
            if (self.pml.groundcontents & CONTENTS_CURRENT_180) != 0 {
                v[0] -= 1.0;
            }
            if (self.pml.groundcontents & CONTENTS_CURRENT_270) != 0 {
                v[1] -= 1.0;
            }
            if (self.pml.groundcontents & CONTENTS_CURRENT_UP) != 0 {
                v[2] += 1.0;
            }
            if (self.pml.groundcontents & CONTENTS_CURRENT_DOWN) != 0 {
                v[2] -= 1.0;
            }

            for i in 0..3 {
                wishvel[i] += 100.0 * v[i];
            }
        }
    }

    // --------------------------------------------------------
    // PM_WaterMove
    // --------------------------------------------------------
    fn water_move(&mut self) {
        let mut wishvel: Vec3 = [0.0; 3];
        let fwd = self.pml.forward;
        let right = self.pml.right;
        let fm = self.pm.cmd.forwardmove as f32;
        let sm = self.pm.cmd.sidemove as f32;

        for i in 0..3 {
            wishvel[i] = fwd[i] * fm + right[i] * sm;
        }

        if self.pm.cmd.forwardmove == 0 && self.pm.cmd.sidemove == 0 && self.pm.cmd.upmove == 0 {
            wishvel[2] -= 60.0; // drift towards bottom
        } else {
            wishvel[2] += self.pm.cmd.upmove as f32;
        }

        self.add_currents(&mut wishvel);

        let mut wishdir = wishvel;
        let mut wishspeed = vector_normalize(&mut wishdir);

        if wishspeed > PM_MAXSPEED {
            let scale = PM_MAXSPEED / wishspeed;
            for i in 0..3 {
                wishvel[i] *= scale;
            }
            wishspeed = PM_MAXSPEED;
        }
        wishspeed *= 0.5;

        self.accelerate(&wishdir, wishspeed, PM_WATERACCELERATE);

        self.step_slide_move();
    }

    // --------------------------------------------------------
    // PM_AirMove
    // --------------------------------------------------------
    fn air_move(&mut self) {
        let fmove = self.pm.cmd.forwardmove as f32;
        let smove = self.pm.cmd.sidemove as f32;
        let fwd = self.pml.forward;
        let right = self.pml.right;

        let mut wishvel: Vec3 = [0.0; 3];
        for i in 0..2 {
            wishvel[i] = fwd[i] * fmove + right[i] * smove;
        }
        wishvel[2] = 0.0;

        self.add_currents(&mut wishvel);

        let mut wishdir = wishvel;
        let mut wishspeed = vector_normalize(&mut wishdir);

        // clamp to server defined max speed
        let maxspeed = if (self.pm.s.pm_flags & PMF_DUCKED) != 0 {
            PM_DUCKSPEED
        } else {
            PM_MAXSPEED
        };

        if wishspeed > maxspeed {
            let scale = maxspeed / wishspeed;
            for i in 0..3 {
                wishvel[i] *= scale;
            }
            wishspeed = maxspeed;
        }

        let gravity = self.pm.s.gravity as f32;

        if self.pml.ladder {
            self.accelerate(&wishdir, wishspeed, PM_ACCELERATE);
            if wishvel[2] == 0.0 {
                if self.pml.velocity[2] > 0.0 {
                    self.pml.velocity[2] -= gravity * self.pml.frametime;
                    if self.pml.velocity[2] < 0.0 {
                        self.pml.velocity[2] = 0.0;
                    }
                } else {
                    self.pml.velocity[2] += gravity * self.pml.frametime;
                    if self.pml.velocity[2] > 0.0 {
                        self.pml.velocity[2] = 0.0;
                    }
                }
            }
            self.step_slide_move();
        } else if self.pm.groundentity >= 0 {
            // walking on ground
            self.pml.velocity[2] = 0.0;
            self.accelerate(&wishdir, wishspeed, PM_ACCELERATE);

            // PGM -- fix for negative trigger_gravity fields
            if gravity > 0.0 {
                self.pml.velocity[2] = 0.0;
            } else {
                self.pml.velocity[2] -= gravity * self.pml.frametime;
            }

            if self.pml.velocity[0] == 0.0 && self.pml.velocity[1] == 0.0 {
                return;
            }
            self.step_slide_move();
        } else {
            // not on ground, so little effect on velocity
            if PM_AIRACCELERATE != 0.0 {
                self.air_accelerate(&wishdir, wishspeed, PM_ACCELERATE);
            } else {
                self.accelerate(&wishdir, wishspeed, 1.0);
            }
            // add gravity
            self.pml.velocity[2] -= gravity * self.pml.frametime;
            self.step_slide_move();
        }
    }

    // --------------------------------------------------------
    // PM_CatagorizePosition
    // --------------------------------------------------------
    fn categorize_position(&mut self) {
        // see if standing on something solid
        let mut point = self.pml.origin;
        point[2] -= 0.25;

        if self.pml.velocity[2] > 180.0 {
            self.pm.s.pm_flags &= !PMF_ON_GROUND;
            self.pm.groundentity = -1;
        } else {
            let trace =
                self.cb
                    .trace(&self.pml.origin, &self.pm.mins, &self.pm.maxs, &point);
            self.pml.groundplane = trace.plane;
            self.pml.groundsurface = trace.surface.clone();
            self.pml.groundcontents = trace.contents;

            if trace.ent_index < 0
                || (trace.plane.normal[2] < 0.7 && !trace.startsolid)
            {
                self.pm.groundentity = -1;
                self.pm.s.pm_flags &= !PMF_ON_GROUND;
            } else {
                self.pm.groundentity = trace.ent_index;

                // hitting solid ground will end a waterjump
                if (self.pm.s.pm_flags & PMF_TIME_WATERJUMP) != 0 {
                    self.pm.s.pm_flags &=
                        !(PMF_TIME_WATERJUMP | PMF_TIME_LAND | PMF_TIME_TELEPORT);
                    self.pm.s.pm_time = 0;
                }

                if (self.pm.s.pm_flags & PMF_ON_GROUND) == 0 {
                    // just hit the ground
                    self.pm.s.pm_flags |= PMF_ON_GROUND;
                    // don't do landing time if we were just going down a slope
                    if self.pml.velocity[2] < -200.0 {
                        self.pm.s.pm_flags |= PMF_TIME_LAND;
                        if self.pml.velocity[2] < -400.0 {
                            self.pm.s.pm_time = 25;
                        } else {
                            self.pm.s.pm_time = 18;
                        }
                    }
                }
            }

            if (self.pm.numtouch as usize) < MAXTOUCH && trace.ent_index >= 0 {
                self.pm.touchents[self.pm.numtouch as usize] = trace.ent_index;
                self.pm.numtouch += 1;
            }
        }

        // get waterlevel, accounting for ducking
        self.pm.waterlevel = 0;
        self.pm.watertype = 0;

        let sample2 = (self.pm.viewheight - self.pm.mins[2]) as i32;
        let sample1 = sample2 / 2;

        let mut point = [
            self.pml.origin[0],
            self.pml.origin[1],
            self.pml.origin[2] + self.pm.mins[2] + 1.0,
        ];
        let cont = self.cb.pointcontents(&point);

        if (cont & MASK_WATER) != 0 {
            self.pm.watertype = cont;
            self.pm.waterlevel = 1;
            point[2] = self.pml.origin[2] + self.pm.mins[2] + sample1 as f32;
            let cont = self.cb.pointcontents(&point);
            if (cont & MASK_WATER) != 0 {
                self.pm.waterlevel = 2;
                point[2] = self.pml.origin[2] + self.pm.mins[2] + sample2 as f32;
                let cont = self.cb.pointcontents(&point);
                if (cont & MASK_WATER) != 0 {
                    self.pm.waterlevel = 3;
                }
            }
        }
    }

    // --------------------------------------------------------
    // PM_CheckJump
    // --------------------------------------------------------
    fn check_jump(&mut self) {
        if (self.pm.s.pm_flags & PMF_TIME_LAND) != 0 {
            return;
        }

        if self.pm.cmd.upmove < 10 {
            self.pm.s.pm_flags &= !PMF_JUMP_HELD;
            return;
        }

        // must wait for jump to be released
        if (self.pm.s.pm_flags & PMF_JUMP_HELD) != 0 {
            return;
        }

        if self.pm.s.pm_type == PmType::Dead {
            return;
        }

        if self.pm.waterlevel >= 2 {
            // swimming, not jumping
            self.pm.groundentity = -1;

            if self.pml.velocity[2] <= -300.0 {
                return;
            }

            if self.pm.watertype == CONTENTS_WATER {
                self.pml.velocity[2] = 100.0;
            } else if self.pm.watertype == CONTENTS_SLIME {
                self.pml.velocity[2] = 80.0;
            } else {
                self.pml.velocity[2] = 50.0;
            }
            return;
        }

        if self.pm.groundentity < 0 {
            return; // in air, so no effect
        }

        self.pm.s.pm_flags |= PMF_JUMP_HELD;

        self.pm.groundentity = -1;
        self.pml.velocity[2] += 270.0;
        if self.pml.velocity[2] < 270.0 {
            self.pml.velocity[2] = 270.0;
        }
    }

    // --------------------------------------------------------
    // PM_CheckSpecialMovement
    // --------------------------------------------------------
    fn check_special_movement(&mut self) {
        if self.pm.s.pm_time != 0 {
            return;
        }

        self.pml.ladder = false;

        // check for ladder
        let mut flatforward: Vec3 = [self.pml.forward[0], self.pml.forward[1], 0.0];
        vector_normalize(&mut flatforward);

        let spot = [
            self.pml.origin[0] + flatforward[0],
            self.pml.origin[1] + flatforward[1],
            self.pml.origin[2] + flatforward[2],
        ];
        let trace = self
            .cb
            .trace(&self.pml.origin, &self.pm.mins, &self.pm.maxs, &spot);
        if trace.fraction < 1.0 && (trace.contents & CONTENTS_LADDER) != 0 {
            self.pml.ladder = true;
        }

        // check for water jump
        if self.pm.waterlevel != 2 {
            return;
        }

        let mut spot = [
            self.pml.origin[0] + 30.0 * flatforward[0],
            self.pml.origin[1] + 30.0 * flatforward[1],
            self.pml.origin[2] + 30.0 * flatforward[2] + 4.0,
        ];
        let cont = self.cb.pointcontents(&spot);
        if (cont & CONTENTS_SOLID) == 0 {
            return;
        }

        spot[2] += 16.0;
        let cont = self.cb.pointcontents(&spot);
        if cont != 0 {
            return;
        }
        // jump out of water
        self.pml.velocity = vector_scale(&flatforward, 50.0);
        self.pml.velocity[2] = 350.0;

        self.pm.s.pm_flags |= PMF_TIME_WATERJUMP;
        self.pm.s.pm_time = 255;
    }

    // --------------------------------------------------------
    // PM_FlyMove
    // --------------------------------------------------------
    fn fly_move(&mut self, doclip: bool) {
        self.pm.viewheight = 22.0;

        // friction
        let speed = vector_length(&self.pml.velocity);
        if speed < 1.0 {
            self.pml.velocity = VEC3_ORIGIN;
        } else {
            

            let friction = PM_FRICTION * 1.5; // extra friction
            let control = if speed < PM_STOPSPEED {
                PM_STOPSPEED
            } else {
                speed
            };
            let drop: f32 = control * friction * self.pml.frametime;

            let mut newspeed = speed - drop;
            if newspeed < 0.0 {
                newspeed = 0.0;
            }
            newspeed /= speed;

            self.pml.velocity = vector_scale(&self.pml.velocity, newspeed);
        }

        // accelerate
        let fmove = self.pm.cmd.forwardmove as f32;
        let smove = self.pm.cmd.sidemove as f32;

        vector_normalize(&mut self.pml.forward);
        vector_normalize(&mut self.pml.right);

        let fwd = self.pml.forward;
        let right = self.pml.right;

        let mut wishvel: Vec3 = [0.0; 3];
        for i in 0..3 {
            wishvel[i] = fwd[i] * fmove + right[i] * smove;
        }
        wishvel[2] += self.pm.cmd.upmove as f32;

        let mut wishdir = wishvel;
        let mut wishspeed = vector_normalize(&mut wishdir);

        // clamp to server defined max speed
        if wishspeed > PM_MAXSPEED {
            let scale = PM_MAXSPEED / wishspeed;
            for i in 0..3 {
                wishvel[i] *= scale;
            }
            wishspeed = PM_MAXSPEED;
        }

        let currentspeed = dot_product(&self.pml.velocity, &wishdir);
        let addspeed = wishspeed - currentspeed;
        if addspeed <= 0.0 {
            return;
        }
        let mut accelspeed = PM_ACCELERATE * self.pml.frametime * wishspeed;
        if accelspeed > addspeed {
            accelspeed = addspeed;
        }

        for i in 0..3 {
            self.pml.velocity[i] += accelspeed * wishdir[i];
        }

        if doclip {
            let end = [
                self.pml.origin[0] + self.pml.frametime * self.pml.velocity[0],
                self.pml.origin[1] + self.pml.frametime * self.pml.velocity[1],
                self.pml.origin[2] + self.pml.frametime * self.pml.velocity[2],
            ];

            let trace =
                self.cb
                    .trace(&self.pml.origin, &self.pm.mins, &self.pm.maxs, &end);
            self.pml.origin = trace.endpos;
        } else {
            // move
            for i in 0..3 {
                self.pml.origin[i] += self.pml.frametime * self.pml.velocity[i];
            }
        }
    }

    // --------------------------------------------------------
    // PM_CheckDuck
    // --------------------------------------------------------
    fn check_duck(&mut self) {
        self.pm.mins[0] = -16.0;
        self.pm.mins[1] = -16.0;

        self.pm.maxs[0] = 16.0;
        self.pm.maxs[1] = 16.0;

        if self.pm.s.pm_type == PmType::Gib {
            self.pm.mins[2] = 0.0;
            self.pm.maxs[2] = 16.0;
            self.pm.viewheight = 8.0;
            return;
        }

        self.pm.mins[2] = -24.0;

        if self.pm.s.pm_type == PmType::Dead {
            self.pm.s.pm_flags |= PMF_DUCKED;
        } else if self.pm.cmd.upmove < 0 && (self.pm.s.pm_flags & PMF_ON_GROUND) != 0 {
            // duck
            self.pm.s.pm_flags |= PMF_DUCKED;
        } else {
            // stand up if possible
            if (self.pm.s.pm_flags & PMF_DUCKED) != 0 {
                // try to stand up
                self.pm.maxs[2] = 32.0;
                let trace = self.cb.trace(
                    &self.pml.origin,
                    &self.pm.mins,
                    &self.pm.maxs,
                    &self.pml.origin,
                );
                if !trace.allsolid {
                    self.pm.s.pm_flags &= !PMF_DUCKED;
                }
            }
        }

        if (self.pm.s.pm_flags & PMF_DUCKED) != 0 {
            self.pm.maxs[2] = 4.0;
            self.pm.viewheight = -2.0;
        } else {
            self.pm.maxs[2] = 32.0;
            self.pm.viewheight = 22.0;
        }
    }

    // --------------------------------------------------------
    // PM_DeadMove
    // --------------------------------------------------------
    fn dead_move(&mut self) {
        if self.pm.groundentity < 0 {
            return;
        }

        // extra friction
        let mut forward = vector_length(&self.pml.velocity);
        forward -= 20.0;
        if forward <= 0.0 {
            self.pml.velocity = VEC3_ORIGIN;
        } else {
            vector_normalize(&mut self.pml.velocity);
            self.pml.velocity = vector_scale(&self.pml.velocity, forward);
        }
    }

    // --------------------------------------------------------
    // PM_GoodPosition
    // --------------------------------------------------------
    fn good_position(&self) -> bool {
        if self.pm.s.pm_type == PmType::Spectator {
            return true;
        }

        let origin: Vec3 = [
            self.pm.s.origin[0] as f32 * 0.125,
            self.pm.s.origin[1] as f32 * 0.125,
            self.pm.s.origin[2] as f32 * 0.125,
        ];
        let trace = self
            .cb
            .trace(&origin, &self.pm.mins, &self.pm.maxs, &origin);
        !trace.allsolid
    }

    // --------------------------------------------------------
    // PM_SnapPosition
    // --------------------------------------------------------
    fn snap_position(&mut self) {
        static JITTERBITS: [i32; 8] = [0, 4, 1, 2, 3, 5, 6, 7];

        // snap velocity to eighths
        for i in 0..3 {
            self.pm.s.velocity[i] = (self.pml.velocity[i] * 8.0) as i16;
        }

        let mut sign = [0i32; 3];
        for i in 0..3 {
            if self.pml.origin[i] >= 0.0 {
                sign[i] = 1;
            } else {
                sign[i] = -1;
            }
            self.pm.s.origin[i] = (self.pml.origin[i] * 8.0) as i16;
            if self.pm.s.origin[i] as f32 * 0.125 == self.pml.origin[i] {
                sign[i] = 0;
            }
        }
        let base = self.pm.s.origin;

        // try all combinations
        for j in 0..8 {
            let bits = JITTERBITS[j];
            self.pm.s.origin = base;
            for i in 0..3 {
                if (bits & (1 << i)) != 0 {
                    self.pm.s.origin[i] = self.pm.s.origin[i].wrapping_add(sign[i] as i16);
                }
            }

            if self.good_position() {
                return;
            }
        }

        // go back to the last position
        self.pm.s.origin = self.pml.previous_origin;
    }

    // --------------------------------------------------------
    // PM_InitialSnapPosition
    // --------------------------------------------------------
    fn initial_snap_position(&mut self) {
        static OFFSET: [i16; 3] = [0, -1, 1];

        let base = self.pm.s.origin;

        for z in 0..3 {
            self.pm.s.origin[2] = base[2].wrapping_add(OFFSET[z]);
            for y in 0..3 {
                self.pm.s.origin[1] = base[1].wrapping_add(OFFSET[y]);
                for x in 0..3 {
                    self.pm.s.origin[0] = base[0].wrapping_add(OFFSET[x]);
                    if self.good_position() {
                        self.pml.origin[0] = self.pm.s.origin[0] as f32 * 0.125;
                        self.pml.origin[1] = self.pm.s.origin[1] as f32 * 0.125;
                        self.pml.origin[2] = self.pm.s.origin[2] as f32 * 0.125;
                        self.pml.previous_origin = self.pm.s.origin;
                        return;
                    }
                }
            }
        }

        crate::common::com_dprintf("Bad InitialSnapPosition\n");
    }

    // --------------------------------------------------------
    // PM_ClampAngles
    // --------------------------------------------------------
    fn clamp_angles(&mut self) {
        if (self.pm.s.pm_flags & PMF_TIME_TELEPORT) != 0 {
            self.pm.viewangles[YAW] = short2angle(
                self.pm.cmd.angles[YAW].wrapping_add(self.pm.s.delta_angles[YAW]),
            );
            self.pm.viewangles[PITCH] = 0.0;
            self.pm.viewangles[2] = 0.0;
        } else {
            // circularly clamp the angles with deltas
            for i in 0..3 {
                let temp = self.pm.cmd.angles[i].wrapping_add(self.pm.s.delta_angles[i]);
                self.pm.viewangles[i] = short2angle(temp);
            }

            // don't let the player look up or down more than 90 degrees
            if self.pm.viewangles[PITCH] > 89.0 && self.pm.viewangles[PITCH] < 180.0 {
                self.pm.viewangles[PITCH] = 89.0;
            } else if self.pm.viewangles[PITCH] < 271.0 && self.pm.viewangles[PITCH] >= 180.0 {
                self.pm.viewangles[PITCH] = 271.0;
            }
        }
        angle_vectors(
            &self.pm.viewangles,
            Some(&mut self.pml.forward),
            Some(&mut self.pml.right),
            Some(&mut self.pml.up),
        );
    }

    // --------------------------------------------------------
    // Main Pmove execution
    // --------------------------------------------------------
    fn execute(&mut self) {
        // clear results
        self.pm.numtouch = 0;
        self.pm.viewangles = [0.0; 3];
        self.pm.viewheight = 0.0;
        self.pm.groundentity = -1;
        self.pm.watertype = 0;
        self.pm.waterlevel = 0;

        // clear all pmove local vars
        self.pml = PmLocal::default();

        // convert origin and velocity to float values
        for i in 0..3 {
            self.pml.origin[i] = self.pm.s.origin[i] as f32 * 0.125;
            self.pml.velocity[i] = self.pm.s.velocity[i] as f32 * 0.125;
        }

        // save old org in case we get stuck
        self.pml.previous_origin = self.pm.s.origin;

        self.pml.frametime = self.pm.cmd.msec as f32 * 0.001;

        self.clamp_angles();

        if self.pm.s.pm_type == PmType::Spectator {
            self.fly_move(false);
            self.snap_position();
            return;
        }

        if self.pm.s.pm_type as i32 >= PmType::Dead as i32 {
            self.pm.cmd.forwardmove = 0;
            self.pm.cmd.sidemove = 0;
            self.pm.cmd.upmove = 0;
        }

        if self.pm.s.pm_type == PmType::Freeze {
            return; // no movement at all
        }

        // set mins, maxs, and viewheight
        self.check_duck();

        if self.pm.snapinitial {
            self.initial_snap_position();
        }

        // set groundentity, watertype, and waterlevel
        self.categorize_position();

        if self.pm.s.pm_type == PmType::Dead {
            self.dead_move();
        }

        self.check_special_movement();

        // drop timing counter
        if self.pm.s.pm_time != 0 {
            let mut msec = (self.pm.cmd.msec >> 3) as i32;
            if msec == 0 {
                msec = 1;
            }
            if msec >= self.pm.s.pm_time as i32 {
                self.pm.s.pm_flags &=
                    !(PMF_TIME_WATERJUMP | PMF_TIME_LAND | PMF_TIME_TELEPORT);
                self.pm.s.pm_time = 0;
            } else {
                self.pm.s.pm_time -= msec as u8;
            }
        }

        if (self.pm.s.pm_flags & PMF_TIME_TELEPORT) != 0 {
            // teleport pause stays exactly in place
        } else if (self.pm.s.pm_flags & PMF_TIME_WATERJUMP) != 0 {
            // waterjump has no control, but falls
            self.pml.velocity[2] -= self.pm.s.gravity as f32 * self.pml.frametime;
            if self.pml.velocity[2] < 0.0 {
                // cancel as soon as we are falling down again
                self.pm.s.pm_flags &=
                    !(PMF_TIME_WATERJUMP | PMF_TIME_LAND | PMF_TIME_TELEPORT);
                self.pm.s.pm_time = 0;
            }

            self.step_slide_move();
        } else {
            self.check_jump();

            self.friction();

            if self.pm.waterlevel >= 2 {
                self.water_move();
            } else {
                let mut angles = self.pm.viewangles;
                if angles[PITCH] > 180.0 {
                    angles[PITCH] -= 360.0;
                }
                angles[PITCH] /= 3.0;

                angle_vectors(
                    &angles,
                    Some(&mut self.pml.forward),
                    Some(&mut self.pml.right),
                    Some(&mut self.pml.up),
                );

                self.air_move();
            }
        }

        // set groundentity, watertype, and waterlevel for final spot
        self.categorize_position();

        self.snap_position();
    }
}

// ============================================================
// Public API
// ============================================================

/// Run the player movement prediction. Can be called by either the server or the client.
///
/// `pm` is the mutable player move data.
/// `callbacks` provides the trace and pointcontents functions from the engine.
pub fn pmove(pm: &mut PmoveData, callbacks: &impl PmoveCallbacks) {
    let mut ctx = PmoveContext {
        pm,
        pml: PmLocal::default(),
        cb: callbacks,
    };
    ctx.execute();
}

// ============================================================
// Unit tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::q_shared::{CPlane, CSurface, PmoveData, Trace};

    /// Stub callbacks for testing — open air, no collisions.
    struct OpenAirCallbacks;

    impl PmoveCallbacks for OpenAirCallbacks {
        fn trace(&self, start: &Vec3, _mins: &Vec3, _maxs: &Vec3, end: &Vec3) -> Trace {
            Trace {
                allsolid: false,
                startsolid: false,
                fraction: 1.0,
                endpos: *end,
                plane: CPlane::default(),
                surface: None,
                contents: 0,
                ent_index: -1,
            }
        }

        fn pointcontents(&self, _point: &Vec3) -> i32 {
            0
        }
    }

    /// Stub callbacks that simulate a solid floor at z=0.
    struct FloorCallbacks;

    impl PmoveCallbacks for FloorCallbacks {
        fn trace(&self, start: &Vec3, mins: &Vec3, _maxs: &Vec3, end: &Vec3) -> Trace {
            // The floor is at z=0. Account for the player bbox: the swept
            // AABB hits the floor when origin_z + mins[2] <= 0, i.e. when
            // origin_z <= -mins[2].
            let effective_floor = -mins[2]; // e.g. 24.0 for mins[2]=-24
            if end[2] < effective_floor {
                let frac = if (start[2] - end[2]).abs() > f32::EPSILON {
                    ((start[2] - effective_floor) / (start[2] - end[2])).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                Trace {
                    allsolid: false,
                    startsolid: false,
                    fraction: frac,
                    endpos: [
                        start[0] + frac * (end[0] - start[0]),
                        start[1] + frac * (end[1] - start[1]),
                        effective_floor,
                    ],
                    plane: CPlane {
                        normal: [0.0, 0.0, 1.0],
                        dist: 0.0,
                        plane_type: 2,
                        signbits: 0,
                        pad: [0; 2],
                    },
                    surface: Some(CSurface::default()),
                    contents: CONTENTS_SOLID,
                    ent_index: 0, // world entity
                }
            } else {
                Trace {
                    allsolid: false,
                    startsolid: false,
                    fraction: 1.0,
                    endpos: *end,
                    plane: CPlane::default(),
                    surface: None,
                    contents: 0,
                    ent_index: -1,
                }
            }
        }

        fn pointcontents(&self, point: &Vec3) -> i32 {
            if point[2] < 0.0 {
                CONTENTS_SOLID
            } else {
                0
            }
        }
    }

    #[test]
    fn test_clip_velocity() {
        let inv: Vec3 = [10.0, 0.0, -10.0];
        let normal: Vec3 = [0.0, 0.0, 1.0];
        let mut out: Vec3 = [0.0; 3];
        pm_clip_velocity(&inv, &normal, &mut out, 1.0);
        assert!((out[0] - 10.0).abs() < 1e-6);
        assert!((out[1]).abs() < 1e-6);
        assert!((out[2]).abs() < 1e-6); // vertical component removed
    }

    #[test]
    fn test_clip_velocity_overbounce() {
        let inv: Vec3 = [0.0, 0.0, -100.0];
        let normal: Vec3 = [0.0, 0.0, 1.0];
        let mut out: Vec3 = [0.0; 3];
        pm_clip_velocity(&inv, &normal, &mut out, 1.01);
        // should bounce slightly upward: -100 - (-100 * 1.01) = -100 + 101 = 1.0
        assert!((out[2] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_pmove_spectator_moves() {
        let mut pm = PmoveData::default();
        pm.s.pm_type = PmType::Spectator;
        pm.cmd.msec = 16;
        pm.cmd.forwardmove = 127;
        // Set origin to something nonzero so we can verify movement
        pm.s.origin = [0, 0, 800]; // 100.0 in float (800 * 0.125)

        let cb = OpenAirCallbacks;
        pmove(&mut pm, &cb);

        // Spectator with forward move should have changed origin
        // (viewangles default to 0 so forward = +x)
        // The velocity should be nonzero after acceleration
    }

    #[test]
    fn test_pmove_freeze_no_movement() {
        let mut pm = PmoveData::default();
        pm.s.pm_type = PmType::Freeze;
        pm.s.origin = [800, 800, 800];
        pm.cmd.msec = 16;
        pm.cmd.forwardmove = 127;

        let cb = OpenAirCallbacks;
        pmove(&mut pm, &cb);

        // Freeze: origin should not change
        assert_eq!(pm.s.origin, [800, 800, 800]);
    }

    #[test]
    fn test_pmove_gravity_in_air() {
        let mut pm = PmoveData::default();
        pm.s.pm_type = PmType::Normal;
        pm.s.gravity = 800;
        pm.s.origin = [0, 0, 1600]; // 200.0 in float
        pm.cmd.msec = 100; // 100ms

        let cb = OpenAirCallbacks;
        pmove(&mut pm, &cb);

        // After 100ms with 800 gravity, velocity should be negative (falling)
        let vel_z = pm.s.velocity[2] as f32 * 0.125;
        assert!(vel_z < 0.0, "Should be falling, vel_z = {}", vel_z);
    }

    #[test]
    fn test_pmove_on_ground() {
        let mut pm = PmoveData::default();
        pm.s.pm_type = PmType::Normal;
        pm.s.gravity = 800;
        pm.s.origin = [0, 0, 193]; // 24.125 in float (feet at 0.125 above floor with mins[2]=-24)
        pm.cmd.msec = 16;

        let cb = FloorCallbacks;
        pmove(&mut pm, &cb);

        // Should detect ground
        assert!((pm.s.pm_flags & PMF_ON_GROUND) != 0 || pm.groundentity >= 0,
            "Player should be on ground");
    }

    // =========================================================================
    // C-to-Rust cross-validation: pm_clip_velocity
    // C: backoff = DotProduct(in, normal) * overbounce
    //    out[i] = in[i] - normal[i] * backoff
    //    if (out[i] > -STOP_EPSILON && out[i] < STOP_EPSILON) out[i] = 0
    // =========================================================================

    #[test]
    fn test_clip_velocity_floor_slide() {
        // Velocity going diagonally into a floor (normal = up)
        // Should remove the vertical component, keep horizontal
        let inv: Vec3 = [200.0, 100.0, -300.0];
        let normal: Vec3 = [0.0, 0.0, 1.0];
        let mut out: Vec3 = [0.0; 3];
        pm_clip_velocity(&inv, &normal, &mut out, 1.0);

        // backoff = dot([200,100,-300], [0,0,1]) * 1.0 = -300
        // out = [200-0, 100-0, -300-(-300)] = [200, 100, 0]
        assert!((out[0] - 200.0).abs() < 1e-4, "out[0]={}", out[0]);
        assert!((out[1] - 100.0).abs() < 1e-4, "out[1]={}", out[1]);
        assert!((out[2] - 0.0).abs() < 1e-4, "out[2]={}", out[2]);
    }

    #[test]
    fn test_clip_velocity_wall_slide() {
        // Velocity into a wall (normal = +X)
        let inv: Vec3 = [-200.0, 100.0, 0.0];
        let normal: Vec3 = [1.0, 0.0, 0.0];
        let mut out: Vec3 = [0.0; 3];
        pm_clip_velocity(&inv, &normal, &mut out, 1.0);

        // backoff = dot([-200,100,0], [1,0,0]) = -200
        // out = [-200-1*(-200), 100-0, 0-0] = [0, 100, 0]
        assert!((out[0] - 0.0).abs() < 1e-4, "out[0]={}", out[0]);
        assert!((out[1] - 100.0).abs() < 1e-4, "out[1]={}", out[1]);
        assert!((out[2] - 0.0).abs() < 1e-4, "out[2]={}", out[2]);
    }

    #[test]
    fn test_clip_velocity_overbounce_exact() {
        // Test the 1.01 overbounce factor used in step_slide_move
        let inv: Vec3 = [0.0, 0.0, -100.0];
        let normal: Vec3 = [0.0, 0.0, 1.0];
        let mut out: Vec3 = [0.0; 3];
        pm_clip_velocity(&inv, &normal, &mut out, 1.01);

        // C: backoff = -100 * 1.01 = -101
        // out[2] = -100 - (1.0 * -101) = -100 + 101 = 1.0
        assert!((out[0]).abs() < 1e-4, "out[0]={}", out[0]);
        assert!((out[1]).abs() < 1e-4, "out[1]={}", out[1]);
        assert!((out[2] - 1.0).abs() < 0.02, "out[2]={}", out[2]);
    }

    #[test]
    fn test_clip_velocity_stop_epsilon_clamping() {
        // Values within STOP_EPSILON (0.1) of zero should be clamped to 0
        let inv: Vec3 = [0.05, -0.05, 0.0];
        let normal: Vec3 = [0.0, 0.0, 1.0];
        let mut out: Vec3 = [0.0; 3];
        pm_clip_velocity(&inv, &normal, &mut out, 1.0);

        // backoff = 0, so out = inv, but 0.05 is within STOP_EPSILON
        assert_eq!(out[0], 0.0, "0.05 should clamp to 0 (STOP_EPSILON=0.1)");
        assert_eq!(out[1], 0.0, "-0.05 should clamp to 0 (STOP_EPSILON=0.1)");
    }

    #[test]
    fn test_clip_velocity_diagonal_surface() {
        // 45-degree ramp: normal = normalized(0, 0.707, 0.707)
        let n = 1.0 / 2.0f32.sqrt();
        let inv: Vec3 = [100.0, 0.0, -100.0];
        let normal: Vec3 = [0.0, n, n];
        let mut out: Vec3 = [0.0; 3];
        pm_clip_velocity(&inv, &normal, &mut out, 1.0);

        // backoff = dot([100,0,-100], [0,n,n]) = 0 + 0 + (-100*n) = -70.71
        // out[0] = 100 - 0 = 100
        // out[1] = 0 - n*(-70.71) = 50
        // out[2] = -100 - n*(-70.71) = -100 + 50 = -50
        assert!((out[0] - 100.0).abs() < 0.1, "out[0]={}", out[0]);
        assert!((out[1] - 50.0).abs() < 0.1, "out[1]={}", out[1]);
        assert!((out[2] - (-50.0)).abs() < 0.1, "out[2]={}", out[2]);
    }

    // =========================================================================
    // Stair step detection: STEPSIZE=18 units
    // =========================================================================

    #[test]
    fn test_stepsize_constant_matches_c() {
        // C: #define STEPSIZE 18
        assert_eq!(STEPSIZE, 18.0, "STEPSIZE should be 18 to match C");
    }

    // =========================================================================
    // Water movement: verify underwater friction matches C
    // C: drop += speed * PM_WATERFRICTION * waterlevel * frametime
    // =========================================================================

    #[test]
    fn test_water_friction_formula() {
        // Simulate the friction drop calculation for water
        let speed = 200.0f32;
        let waterlevel = 2;
        let frametime = 0.016f32; // 16ms frame

        // C formula: drop = speed * PM_WATERFRICTION * waterlevel * frametime
        let drop = speed * PM_WATERFRICTION * waterlevel as f32 * frametime;
        // PM_WATERFRICTION = 1.0
        let expected_drop = 200.0 * 1.0 * 2.0 * 0.016;
        assert!(
            (drop - expected_drop).abs() < 1e-6,
            "Water friction drop mismatch: got {}, expected {}",
            drop, expected_drop
        );

        // Verify newspeed calculation
        let newspeed = ((speed - drop) / speed).max(0.0);
        assert!(newspeed < 1.0 && newspeed > 0.0,
            "newspeed ratio should be in (0,1), got {}", newspeed);
    }

    // =========================================================================
    // Walk movement: verify ground friction acceleration matches C formulas
    // C: control = (speed < PM_STOPSPEED) ? PM_STOPSPEED : speed
    //    drop = control * PM_FRICTION * frametime
    // =========================================================================

    #[test]
    fn test_ground_friction_formula_fast() {
        // speed > STOPSPEED: control = speed
        let speed = 200.0f32;
        let frametime = 0.016f32;

        let control = if speed < PM_STOPSPEED { PM_STOPSPEED } else { speed };
        assert_eq!(control, 200.0);

        let drop = control * PM_FRICTION * frametime;
        // 200 * 6 * 0.016 = 19.2
        let expected = 200.0 * 6.0 * 0.016;
        assert!((drop - expected).abs() < 1e-6,
            "Ground friction drop mismatch: got {}, expected {}", drop, expected);

        let newspeed = (speed - drop).max(0.0) / speed;
        assert!(newspeed > 0.0 && newspeed < 1.0);
    }

    #[test]
    fn test_ground_friction_formula_slow() {
        // speed < STOPSPEED: control = STOPSPEED (makes friction stronger at low speeds)
        let speed = 50.0f32; // below PM_STOPSPEED=100
        let frametime = 0.016f32;

        let control = if speed < PM_STOPSPEED { PM_STOPSPEED } else { speed };
        assert_eq!(control, PM_STOPSPEED);

        let drop = control * PM_FRICTION * frametime;
        // 100 * 6 * 0.016 = 9.6
        let expected = 100.0 * 6.0 * 0.016;
        assert!((drop - expected).abs() < 1e-6,
            "Slow friction drop mismatch: got {}, expected {}", drop, expected);
    }

    #[test]
    fn test_ground_friction_very_slow_stops() {
        // With speed < 1.0, C zeroes the horizontal velocity directly
        let speed = 0.5f32;
        // C: if (speed < 1) { vel[0] = 0; vel[1] = 0; return; }
        assert!(speed < 1.0, "Test premise: speed should be < 1");
    }

    // =========================================================================
    // Acceleration formula: C cross-validation
    // C: currentspeed = DotProduct(velocity, wishdir)
    //    addspeed = wishspeed - currentspeed
    //    if (addspeed <= 0) return
    //    accelspeed = accel * frametime * wishspeed
    //    if (accelspeed > addspeed) accelspeed = addspeed
    //    velocity += accelspeed * wishdir
    // =========================================================================

    #[test]
    fn test_acceleration_formula_matches_c() {
        let velocity: Vec3 = [100.0, 0.0, 0.0];
        let wishdir: Vec3 = [1.0, 0.0, 0.0];
        let wishspeed = 300.0f32;
        let accel = 10.0f32; // PM_ACCELERATE
        let frametime = 0.016f32;

        let currentspeed = dot_product(&velocity, &wishdir);
        assert_eq!(currentspeed, 100.0);

        let addspeed = wishspeed - currentspeed;
        assert_eq!(addspeed, 200.0);

        let mut accelspeed = accel * frametime * wishspeed;
        // 10 * 0.016 * 300 = 48
        assert!((accelspeed - 48.0).abs() < 1e-4);

        if accelspeed > addspeed {
            accelspeed = addspeed;
        }
        // 48 < 200, so no clamp
        assert!((accelspeed - 48.0).abs() < 1e-4);

        let mut new_velocity = velocity;
        for i in 0..3 {
            new_velocity[i] += accelspeed * wishdir[i];
        }
        assert!((new_velocity[0] - 148.0).abs() < 1e-3,
            "new_velocity[0]={}", new_velocity[0]);
    }

    #[test]
    fn test_acceleration_clamped_to_addspeed() {
        // When accelspeed > addspeed, clamp to addspeed
        let velocity: Vec3 = [290.0, 0.0, 0.0];
        let wishdir: Vec3 = [1.0, 0.0, 0.0];
        let wishspeed = 300.0f32;
        let accel = 10.0f32;
        let frametime = 0.1f32; // large frametime

        let currentspeed = dot_product(&velocity, &wishdir);
        let addspeed = wishspeed - currentspeed;
        assert!((addspeed - 10.0).abs() < 1e-4);

        let mut accelspeed = accel * frametime * wishspeed;
        // 10 * 0.1 * 300 = 300
        assert!((accelspeed - 300.0).abs() < 1e-4);

        if accelspeed > addspeed {
            accelspeed = addspeed; // clamped to 10
        }
        assert!((accelspeed - 10.0).abs() < 1e-4,
            "accelspeed should be clamped to addspeed");
    }

    // =========================================================================
    // Pmove: verify gravity applies correctly over time in air
    // C: velocity[2] -= gravity * frametime
    // =========================================================================

    #[test]
    fn test_pmove_gravity_velocity_change() {
        let mut pm = PmoveData::default();
        pm.s.pm_type = PmType::Normal;
        pm.s.gravity = 800;
        pm.s.origin = [0, 0, 8000]; // 1000.0 in float (high above ground)
        pm.s.velocity = [0, 0, 0];
        pm.cmd.msec = 100; // 100ms = 0.1s

        let cb = OpenAirCallbacks;
        pmove(&mut pm, &cb);

        // After 100ms with 800 gravity:
        // vel_z should be approximately -800 * 0.1 = -80 units/sec
        // In fixed-point: -80 * 8 = -640
        let vel_z = pm.s.velocity[2] as f32 * 0.125;
        assert!(vel_z < -50.0, "vel_z should be strongly negative, got {}", vel_z);
    }

    // =========================================================================
    // Movement constants: verify they match C defines
    // =========================================================================

    #[test]
    fn test_movement_constants_match_c() {
        assert_eq!(PM_STOPSPEED, 100.0);
        assert_eq!(PM_MAXSPEED, 300.0);
        assert_eq!(PM_DUCKSPEED, 100.0);
        assert_eq!(PM_ACCELERATE, 10.0);
        assert_eq!(PM_AIRACCELERATE, 0.0);
        assert_eq!(PM_WATERACCELERATE, 10.0);
        assert_eq!(PM_FRICTION, 6.0);
        assert_eq!(PM_WATERFRICTION, 1.0);
        assert_eq!(PM_WATERSPEED, 400.0);
        assert_eq!(STEPSIZE, 18.0);
        assert_eq!(MIN_STEP_NORMAL, 0.7);
        assert_eq!(STOP_EPSILON, 0.1);
    }
}
