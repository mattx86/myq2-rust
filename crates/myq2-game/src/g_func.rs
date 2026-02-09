// g_func.rs — Brush entity functions (doors, platforms, buttons, trains, rotating, etc.)
// Converted from: myq2-original/game/g_func.c

use crate::g_local::*;
use crate::game::*;
use crate::game_import::*;
use myq2_common::common::crand as crandom;
use myq2_common::q_shared::{
    add_point_to_bounds, angle_vectors,
    vector_clear as vec3_clear,
    vector_copy_to as vec3_copy,
    vector_subtract_to as vec3_subtract,
    vector_scale_to as vec3_scale,
    vector_ma_to as vec3_ma,
    vector_normalize as vec3_normalize,
    vector_length as vec3_length,
    vector_compare as vec3_compare,
    vector_negate_to as vec3_negate,
    dot_product as vec3_dot,
};

use crate::g_utils::vtos;

// =========================================================
// Constants
// =========================================================

const PLAT_LOW_TRIGGER: i32 = 1;

const STATE_TOP: i32 = 0;
const STATE_BOTTOM: i32 = 1;
const STATE_UP: i32 = 2;
const STATE_DOWN: i32 = 3;

const DOOR_START_OPEN: i32 = 1;
const DOOR_REVERSE: i32 = 2;
const DOOR_CRUSHER: i32 = 4;
const DOOR_NOMONSTER: i32 = 8;
const DOOR_TOGGLE: i32 = 32;
const DOOR_X_AXIS: i32 = 64;
const DOOR_Y_AXIS: i32 = 128;

const TRAIN_START_ON: i32 = 1;
const TRAIN_TOGGLE: i32 = 2;
const TRAIN_BLOCK_STOPS: i32 = 4;

const SECRET_ALWAYS_SHOOT: i32 = 1;
const SECRET_1ST_LEFT: i32 = 2;
const SECRET_1ST_DOWN: i32 = 4;

/// Acceleration distance helper macro equivalent
fn acceleration_distance(target: f32, rate: f32) -> f32 {
    target * ((target / rate) + 1.0) / 2.0
}


impl GameContext {
    // =========================================================
    // Support routines for movement (changes in origin using velocity)
    // =========================================================

    pub fn move_done(&mut self, ent: usize) {
        vec3_clear(&mut self.edicts[ent].velocity);
        if let Some(endfunc_id) = self.edicts[ent].moveinfo.endfunc {
            self.dispatch_endfunc(endfunc_id, ent);
        }
    }

    pub fn move_final(&mut self, ent: usize) {
        if self.edicts[ent].moveinfo.remaining_distance == 0.0 {
            self.move_done(ent);
            return;
        }

        let remaining = self.edicts[ent].moveinfo.remaining_distance;
        let dir = self.edicts[ent].moveinfo.dir;
        vec3_scale(&dir, remaining / FRAMETIME, &mut self.edicts[ent].velocity);

        self.edicts[ent].think_fn = Some(crate::dispatch::THINK_FUNC_MOVE_DONE);
        self.edicts[ent].nextthink = self.level.time + FRAMETIME;
    }

    pub fn move_begin(&mut self, ent: usize) {
        if (self.edicts[ent].moveinfo.speed * FRAMETIME) >= self.edicts[ent].moveinfo.remaining_distance {
            self.move_final(ent);
            return;
        }

        let dir = self.edicts[ent].moveinfo.dir;
        let speed = self.edicts[ent].moveinfo.speed;
        vec3_scale(&dir, speed, &mut self.edicts[ent].velocity);

        let frames = (self.edicts[ent].moveinfo.remaining_distance / speed / FRAMETIME).floor();
        self.edicts[ent].moveinfo.remaining_distance -= frames * speed * FRAMETIME;
        self.edicts[ent].nextthink = self.level.time + frames * FRAMETIME;
        self.edicts[ent].think_fn = Some(crate::dispatch::THINK_FUNC_MOVE_FINAL);
    }

    pub fn move_calc(&mut self, ent: usize, dest: [f32; 3], endfunc: usize) {
        vec3_clear(&mut self.edicts[ent].velocity);
        let origin = self.edicts[ent].s.origin;
        vec3_subtract(&dest, &origin, &mut self.edicts[ent].moveinfo.dir);
        self.edicts[ent].moveinfo.remaining_distance = vec3_normalize(&mut self.edicts[ent].moveinfo.dir);
        self.edicts[ent].moveinfo.endfunc = Some(endfunc);

        let mi_speed = self.edicts[ent].moveinfo.speed;
        let mi_accel = self.edicts[ent].moveinfo.accel;
        let mi_decel = self.edicts[ent].moveinfo.decel;

        if mi_speed == mi_accel && mi_speed == mi_decel {
            let is_current = self.level.current_entity == self.get_team_entity(ent);
            if is_current {
                self.move_begin(ent);
            } else {
                self.edicts[ent].nextthink = self.level.time + FRAMETIME;
                self.edicts[ent].think_fn = Some(crate::dispatch::THINK_FUNC_MOVE_BEGIN);
            }
        } else {
            // accelerative
            self.edicts[ent].moveinfo.current_speed = 0.0;
            self.edicts[ent].think_fn = Some(crate::dispatch::THINK_FUNC_ACCEL_MOVE);
            self.edicts[ent].nextthink = self.level.time + FRAMETIME;
        }
    }

    // =========================================================
    // Support routines for angular movement
    // =========================================================

    pub fn angle_move_done(&mut self, ent: usize) {
        vec3_clear(&mut self.edicts[ent].avelocity);
        if let Some(endfunc_id) = self.edicts[ent].moveinfo.endfunc {
            self.dispatch_endfunc(endfunc_id, ent);
        }
    }

    pub fn angle_move_final(&mut self, ent: usize) {
        let mut mv = [0.0f32; 3];
        if self.edicts[ent].moveinfo.state == STATE_UP {
            let end = self.edicts[ent].moveinfo.end_angles;
            let cur = self.edicts[ent].s.angles;
            vec3_subtract(&end, &cur, &mut mv);
        } else {
            let start = self.edicts[ent].moveinfo.start_angles;
            let cur = self.edicts[ent].s.angles;
            vec3_subtract(&start, &cur, &mut mv);
        }

        if vec3_compare(&mv, &VEC3_ORIGIN) {
            self.angle_move_done(ent);
            return;
        }

        vec3_scale(&mv, 1.0 / FRAMETIME, &mut self.edicts[ent].avelocity);
        self.edicts[ent].think_fn = Some(crate::dispatch::THINK_FUNC_ANGLE_MOVE_DONE);
        self.edicts[ent].nextthink = self.level.time + FRAMETIME;
    }

    pub fn angle_move_begin(&mut self, ent: usize) {
        let mut destdelta = [0.0f32; 3];
        if self.edicts[ent].moveinfo.state == STATE_UP {
            let end = self.edicts[ent].moveinfo.end_angles;
            let cur = self.edicts[ent].s.angles;
            vec3_subtract(&end, &cur, &mut destdelta);
        } else {
            let start = self.edicts[ent].moveinfo.start_angles;
            let cur = self.edicts[ent].s.angles;
            vec3_subtract(&start, &cur, &mut destdelta);
        }

        let len = vec3_length(&destdelta);
        let traveltime = len / self.edicts[ent].moveinfo.speed;

        if traveltime < FRAMETIME {
            self.angle_move_final(ent);
            return;
        }

        let frames = (traveltime / FRAMETIME).floor();
        vec3_scale(&destdelta, 1.0 / traveltime, &mut self.edicts[ent].avelocity);

        self.edicts[ent].nextthink = self.level.time + frames * FRAMETIME;
        self.edicts[ent].think_fn = Some(crate::dispatch::THINK_FUNC_ANGLE_MOVE_FINAL);
    }

    pub fn angle_move_calc(&mut self, ent: usize, endfunc: usize) {
        vec3_clear(&mut self.edicts[ent].avelocity);
        self.edicts[ent].moveinfo.endfunc = Some(endfunc);
        let is_current = self.level.current_entity == self.get_team_entity(ent);
        if is_current {
            self.angle_move_begin(ent);
        } else {
            self.edicts[ent].nextthink = self.level.time + FRAMETIME;
            self.edicts[ent].think_fn = Some(crate::dispatch::THINK_FUNC_ANGLE_MOVE_BEGIN);
        }
    }

    // =========================================================
    // Accelerated movement
    // =========================================================

    pub fn plat_calc_accelerated_move(&mut self, ent: usize) {
        let mi = &mut self.edicts[ent].moveinfo;
        mi.move_speed = mi.speed;

        if mi.remaining_distance < mi.accel {
            mi.current_speed = mi.remaining_distance;
            return;
        }

        let accel_dist = acceleration_distance(mi.speed, mi.accel);
        let mut decel_dist = acceleration_distance(mi.speed, mi.decel);

        if (mi.remaining_distance - accel_dist - decel_dist) < 0.0 {
            let f = (mi.accel + mi.decel) / (mi.accel * mi.decel);
            mi.move_speed = (-2.0 + (4.0 - 4.0 * f * (-2.0 * mi.remaining_distance)).sqrt()) / (2.0 * f);
            decel_dist = acceleration_distance(mi.move_speed, mi.decel);
        }

        mi.decel_distance = decel_dist;
    }

    pub fn plat_accelerate(&mut self, ent: usize) {
        let mi = &mut self.edicts[ent].moveinfo;

        // are we decelerating?
        if mi.remaining_distance <= mi.decel_distance {
            if mi.remaining_distance < mi.decel_distance {
                if mi.next_speed != 0.0 {
                    mi.current_speed = mi.next_speed;
                    mi.next_speed = 0.0;
                    return;
                }
                if mi.current_speed > mi.decel {
                    mi.current_speed -= mi.decel;
                }
            }
            return;
        }

        // are we at full speed and need to start decelerating during this move?
        if mi.current_speed == mi.move_speed
            && (mi.remaining_distance - mi.current_speed) < mi.decel_distance {
                let p1_distance = mi.remaining_distance - mi.decel_distance;
                let p2_distance = mi.move_speed * (1.0 - (p1_distance / mi.move_speed));
                let distance = p1_distance + p2_distance;
                mi.current_speed = mi.move_speed;
                mi.next_speed = mi.move_speed - mi.decel * (p2_distance / distance);
                return;
            }

        // are we accelerating?
        if mi.current_speed < mi.speed {
            let old_speed = mi.current_speed;
            mi.current_speed += mi.accel;
            if mi.current_speed > mi.speed {
                mi.current_speed = mi.speed;
            }

            if (mi.remaining_distance - mi.current_speed) >= mi.decel_distance {
                return;
            }

            let p1_distance = mi.remaining_distance - mi.decel_distance;
            let p1_speed = (old_speed + mi.move_speed) / 2.0;
            let p2_distance = mi.move_speed * (1.0 - (p1_distance / p1_speed));
            let distance = p1_distance + p2_distance;
            mi.current_speed = (p1_speed * (p1_distance / distance)) + (mi.move_speed * (p2_distance / distance));
            mi.next_speed = mi.move_speed - mi.decel * (p2_distance / distance);
        }

        // we are at constant velocity (move_speed)
    }

    pub fn think_accel_move(&mut self, ent: usize) {
        self.edicts[ent].moveinfo.remaining_distance -= self.edicts[ent].moveinfo.current_speed;

        if self.edicts[ent].moveinfo.current_speed == 0.0 {
            self.plat_calc_accelerated_move(ent);
        }

        self.plat_accelerate(ent);

        // will the entire move complete on next frame?
        if self.edicts[ent].moveinfo.remaining_distance <= self.edicts[ent].moveinfo.current_speed {
            self.move_final(ent);
            return;
        }

        let dir = self.edicts[ent].moveinfo.dir;
        let cs = self.edicts[ent].moveinfo.current_speed;
        vec3_scale(&dir, cs * 10.0, &mut self.edicts[ent].velocity);
        self.edicts[ent].nextthink = self.level.time + FRAMETIME;
        self.edicts[ent].think_fn = Some(crate::dispatch::THINK_FUNC_ACCEL_MOVE);
    }

    // =========================================================
    // Helper
    // =========================================================

    /// Returns the entity index that represents this entity for team comparison.
    /// If FL_TEAMSLAVE, returns teammaster; otherwise returns self.
    fn get_team_entity(&self, ent: usize) -> i32 {
        if self.edicts[ent].flags.intersects(FL_TEAMSLAVE) {
            self.edicts[ent].teammaster
        } else {
            ent as i32
        }
    }

    // =========================================================
    // PLATS
    // =========================================================

    pub fn plat_hit_top(&mut self, ent: usize) {
        if !self.edicts[ent].flags.intersects(FL_TEAMSLAVE) {
            if self.edicts[ent].moveinfo.sound_end != 0 {
                gi_sound(ent as i32, CHAN_NO_PHS_ADD + CHAN_VOICE, self.edicts[ent].moveinfo.sound_end, 1.0, ATTN_STATIC, 0.0);
            }
            self.edicts[ent].s.sound = 0;
        }
        self.edicts[ent].moveinfo.state = STATE_TOP;

        self.edicts[ent].think_fn = Some(crate::dispatch::THINK_FUNC_PLAT_GO_DOWN);
        self.edicts[ent].nextthink = self.level.time + 3.0;
    }

    pub fn plat_hit_bottom(&mut self, ent: usize) {
        if !self.edicts[ent].flags.intersects(FL_TEAMSLAVE) {
            if self.edicts[ent].moveinfo.sound_end != 0 {
                gi_sound(ent as i32, CHAN_NO_PHS_ADD + CHAN_VOICE, self.edicts[ent].moveinfo.sound_end, 1.0, ATTN_STATIC, 0.0);
            }
            self.edicts[ent].s.sound = 0;
        }
        self.edicts[ent].moveinfo.state = STATE_BOTTOM;
    }

    pub fn plat_go_down(&mut self, ent: usize) {
        if !self.edicts[ent].flags.intersects(FL_TEAMSLAVE) {
            if self.edicts[ent].moveinfo.sound_start != 0 {
                gi_sound(ent as i32, CHAN_NO_PHS_ADD + CHAN_VOICE, self.edicts[ent].moveinfo.sound_start, 1.0, ATTN_STATIC, 0.0);
            }
            self.edicts[ent].s.sound = self.edicts[ent].moveinfo.sound_middle;
        }
        self.edicts[ent].moveinfo.state = STATE_DOWN;
        let dest = self.edicts[ent].moveinfo.end_origin;
        self.move_calc(ent, dest, EndFn::PlatHitBottom as usize);
    }

    pub fn plat_go_up(&mut self, ent: usize) {
        if !self.edicts[ent].flags.intersects(FL_TEAMSLAVE) {
            if self.edicts[ent].moveinfo.sound_start != 0 {
                gi_sound(ent as i32, CHAN_NO_PHS_ADD + CHAN_VOICE, self.edicts[ent].moveinfo.sound_start, 1.0, ATTN_STATIC, 0.0);
            }
            self.edicts[ent].s.sound = self.edicts[ent].moveinfo.sound_middle;
        }
        self.edicts[ent].moveinfo.state = STATE_UP;
        let dest = self.edicts[ent].moveinfo.start_origin;
        self.move_calc(ent, dest, EndFn::PlatHitTop as usize);
    }

    pub fn plat_blocked(&mut self, self_ent: usize, other: usize) {
        if (self.edicts[other].svflags & SVF_MONSTER == 0) && self.edicts[other].client.is_none() {
            // give it a chance to go away on its own terms (like gibs)
            let origin = self.edicts[other].s.origin;
            crate::g_combat::ctx_t_damage(self, other, self_ent, self_ent, &VEC3_ORIGIN, &origin, &VEC3_ORIGIN, 100000, 1, DamageFlags::empty(), MOD_CRUSH);
            // if it's still there, nuke it
            if self.edicts[other].inuse {
                crate::g_misc::become_explosion1(self, other);
            }
            return;
        }

        let dmg = self.edicts[self_ent].dmg;
        let origin = self.edicts[other].s.origin;
        crate::g_combat::ctx_t_damage(self, other, self_ent, self_ent, &VEC3_ORIGIN, &origin, &VEC3_ORIGIN, dmg, 1, DamageFlags::empty(), MOD_CRUSH);

        if self.edicts[self_ent].moveinfo.state == STATE_UP {
            self.plat_go_down(self_ent);
        } else if self.edicts[self_ent].moveinfo.state == STATE_DOWN {
            self.plat_go_up(self_ent);
        }
    }

    pub fn use_plat(&mut self, ent: usize, _other: usize, _activator: usize) {
        if self.edicts[ent].think_fn.is_some() {
            return; // already down
        }
        self.plat_go_down(ent);
    }

    pub fn touch_plat_center(&mut self, ent: usize, other: usize, _plane: Option<&CPlane>, _surf: Option<&CSurface>) {
        if self.edicts[other].client.is_none() {
            return;
        }
        if self.edicts[other].health <= 0 {
            return;
        }

        let plat = self.edicts[ent].enemy; // now point at the plat, not the trigger
        let plat = plat as usize;
        if self.edicts[plat].moveinfo.state == STATE_BOTTOM {
            self.plat_go_up(plat);
        } else if self.edicts[plat].moveinfo.state == STATE_TOP {
            self.edicts[plat].nextthink = self.level.time + 1.0;
        }
    }

    pub fn plat_spawn_inside_trigger(&mut self, ent: usize) {
        let trigger = crate::g_utils::g_spawn(self);
        self.edicts[trigger].touch_fn = Some(crate::dispatch::TOUCH_PLAT_CENTER);
        self.edicts[trigger].movetype = MoveType::None;
        self.edicts[trigger].solid = Solid::Trigger;
        self.edicts[trigger].enemy = ent as i32;

        let mut tmin = [0.0f32; 3];
        let mut tmax = [0.0f32; 3];

        tmin[0] = self.edicts[ent].mins[0] + 25.0;
        tmin[1] = self.edicts[ent].mins[1] + 25.0;
        tmin[2] = self.edicts[ent].mins[2];

        tmax[0] = self.edicts[ent].maxs[0] - 25.0;
        tmax[1] = self.edicts[ent].maxs[1] - 25.0;
        tmax[2] = self.edicts[ent].maxs[2] + 8.0;

        let lip = self.st.lip as f32;
        tmin[2] = tmax[2] - (self.edicts[ent].pos1[2] - self.edicts[ent].pos2[2] + lip);

        if self.edicts[ent].spawnflags & PLAT_LOW_TRIGGER != 0 {
            tmax[2] = tmin[2] + 8.0;
        }

        if tmax[0] - tmin[0] <= 0.0 {
            tmin[0] = (self.edicts[ent].mins[0] + self.edicts[ent].maxs[0]) * 0.5;
            tmax[0] = tmin[0] + 1.0;
        }
        if tmax[1] - tmin[1] <= 0.0 {
            tmin[1] = (self.edicts[ent].mins[1] + self.edicts[ent].maxs[1]) * 0.5;
            tmax[1] = tmin[1] + 1.0;
        }

        vec3_copy(&tmin, &mut self.edicts[trigger].mins);
        vec3_copy(&tmax, &mut self.edicts[trigger].maxs);

        gi_linkentity(trigger as i32);
    }

    pub fn sp_func_plat(&mut self, ent: usize) {
        vec3_clear(&mut self.edicts[ent].s.angles);
        self.edicts[ent].solid = Solid::Bsp;
        self.edicts[ent].movetype = MoveType::Push;

        let model = self.edicts[ent].model.clone();
        gi_setmodel(ent as i32, &model);

        self.edicts[ent].blocked_fn = Some(crate::dispatch::BLOCKED_FUNC_PLAT);

        if self.edicts[ent].speed == 0.0 {
            self.edicts[ent].speed = 20.0;
        } else {
            self.edicts[ent].speed *= 0.1;
        }

        if self.edicts[ent].accel == 0.0 {
            self.edicts[ent].accel = 5.0;
        } else {
            self.edicts[ent].accel *= 0.1;
        }

        if self.edicts[ent].decel == 0.0 {
            self.edicts[ent].decel = 5.0;
        } else {
            self.edicts[ent].decel *= 0.1;
        }

        if self.edicts[ent].dmg == 0 {
            self.edicts[ent].dmg = 2;
        }

        if self.st.lip == 0 {
            self.st.lip = 8;
        }

        // pos1 is the top position, pos2 is the bottom
        let origin = self.edicts[ent].s.origin;
        vec3_copy(&origin, &mut self.edicts[ent].pos1);
        vec3_copy(&origin, &mut self.edicts[ent].pos2);

        if self.st.height != 0 {
            self.edicts[ent].pos2[2] -= self.st.height as f32;
        } else {
            let lip = self.st.lip as f32;
            self.edicts[ent].pos2[2] -= (self.edicts[ent].maxs[2] - self.edicts[ent].mins[2]) - lip;
        }

        self.edicts[ent].use_fn = Some(crate::dispatch::USE_FUNC_PLAT);

        self.plat_spawn_inside_trigger(ent);

        if !self.edicts[ent].targetname.is_empty() {
            self.edicts[ent].moveinfo.state = STATE_UP;
        } else {
            let pos2 = self.edicts[ent].pos2;
            vec3_copy(&pos2, &mut self.edicts[ent].s.origin);
            gi_linkentity(ent as i32);
            self.edicts[ent].moveinfo.state = STATE_BOTTOM;
        }

        self.edicts[ent].moveinfo.speed = self.edicts[ent].speed;
        self.edicts[ent].moveinfo.accel = self.edicts[ent].accel;
        self.edicts[ent].moveinfo.decel = self.edicts[ent].decel;
        self.edicts[ent].moveinfo.wait = self.edicts[ent].wait;
        let pos1 = self.edicts[ent].pos1;
        let angles = self.edicts[ent].s.angles;
        let pos2 = self.edicts[ent].pos2;
        vec3_copy(&pos1, &mut self.edicts[ent].moveinfo.start_origin);
        vec3_copy(&angles, &mut self.edicts[ent].moveinfo.start_angles);
        vec3_copy(&pos2, &mut self.edicts[ent].moveinfo.end_origin);
        vec3_copy(&angles, &mut self.edicts[ent].moveinfo.end_angles);

        self.edicts[ent].moveinfo.sound_start = gi_soundindex("plats/pt1_strt.wav");
        self.edicts[ent].moveinfo.sound_middle = gi_soundindex("plats/pt1_mid.wav");
        self.edicts[ent].moveinfo.sound_end = gi_soundindex("plats/pt1_end.wav");
    }

    // =========================================================
    // ROTATING
    // =========================================================

    pub fn rotating_blocked(&mut self, self_ent: usize, other: usize) {
        let dmg = self.edicts[self_ent].dmg;
        let origin = self.edicts[other].s.origin;
        crate::g_combat::ctx_t_damage(self, other, self_ent, self_ent, &VEC3_ORIGIN, &origin, &VEC3_ORIGIN, dmg, 1, DamageFlags::empty(), MOD_CRUSH);
    }

    pub fn rotating_touch(&mut self, self_ent: usize, other: usize, _plane: Option<&CPlane>, _surf: Option<&CSurface>) {
        if self.edicts[self_ent].avelocity[0] != 0.0 || self.edicts[self_ent].avelocity[1] != 0.0 || self.edicts[self_ent].avelocity[2] != 0.0 {
            let dmg = self.edicts[self_ent].dmg;
            let origin = self.edicts[other].s.origin;
            crate::g_combat::ctx_t_damage(self, other, self_ent, self_ent, &VEC3_ORIGIN, &origin, &VEC3_ORIGIN, dmg, 1, DamageFlags::empty(), MOD_CRUSH);
        }
    }

    pub fn rotating_use(&mut self, self_ent: usize, _other: usize, _activator: usize) {
        if !vec3_compare(&self.edicts[self_ent].avelocity, &VEC3_ORIGIN) {
            self.edicts[self_ent].s.sound = 0;
            vec3_clear(&mut self.edicts[self_ent].avelocity);
            self.edicts[self_ent].touch_fn = None;
        } else {
            self.edicts[self_ent].s.sound = self.edicts[self_ent].moveinfo.sound_middle;
            let movedir = self.edicts[self_ent].movedir;
            let speed = self.edicts[self_ent].speed;
            vec3_scale(&movedir, speed, &mut self.edicts[self_ent].avelocity);
            if self.edicts[self_ent].spawnflags & 16 != 0 {
                self.edicts[self_ent].touch_fn = Some(crate::dispatch::TOUCH_ROTATING);
            }
        }
    }

    pub fn sp_func_rotating(&mut self, ent: usize) {
        self.edicts[ent].solid = Solid::Bsp;
        if self.edicts[ent].spawnflags & 32 != 0 {
            self.edicts[ent].movetype = MoveType::Stop;
        } else {
            self.edicts[ent].movetype = MoveType::Push;
        }

        // set the axis of rotation
        vec3_clear(&mut self.edicts[ent].movedir);
        if self.edicts[ent].spawnflags & 4 != 0 {
            self.edicts[ent].movedir[2] = 1.0;
        } else if self.edicts[ent].spawnflags & 8 != 0 {
            self.edicts[ent].movedir[0] = 1.0;
        } else {
            // Z_AXIS
            self.edicts[ent].movedir[1] = 1.0;
        }

        // check for reverse rotation
        if self.edicts[ent].spawnflags & 2 != 0 {
            let md = self.edicts[ent].movedir;
            vec3_negate(&md, &mut self.edicts[ent].movedir);
        }

        if self.edicts[ent].speed == 0.0 {
            self.edicts[ent].speed = 100.0;
        }
        if self.edicts[ent].dmg == 0 {
            self.edicts[ent].dmg = 2;
        }

        self.edicts[ent].use_fn = Some(crate::dispatch::USE_FUNC_ROTATING);
        if self.edicts[ent].dmg != 0 {
            self.edicts[ent].blocked_fn = Some(crate::dispatch::BLOCKED_FUNC_ROTATING);
        }

        if self.edicts[ent].spawnflags & 1 != 0 {
            self.rotating_use(ent, 0, 0);
        }

        if self.edicts[ent].spawnflags & 64 != 0 {
            self.edicts[ent].s.effects |= EF_ANIM_ALL;
        }
        if self.edicts[ent].spawnflags & 128 != 0 {
            self.edicts[ent].s.effects |= EF_ANIM_ALLFAST;
        }

        let model = self.edicts[ent].model.clone();
        gi_setmodel(ent as i32, &model);
        gi_linkentity(ent as i32);
    }

    // =========================================================
    // BUTTONS
    // =========================================================

    pub fn button_done(&mut self, self_ent: usize) {
        self.edicts[self_ent].moveinfo.state = STATE_BOTTOM;
        self.edicts[self_ent].s.effects &= !EF_ANIM23;
        self.edicts[self_ent].s.effects |= EF_ANIM01;
    }

    pub fn button_return(&mut self, self_ent: usize) {
        self.edicts[self_ent].moveinfo.state = STATE_DOWN;

        let dest = self.edicts[self_ent].moveinfo.start_origin;
        self.move_calc(self_ent, dest, EndFn::ButtonDone as usize);

        self.edicts[self_ent].s.frame = 0;

        if self.edicts[self_ent].health != 0 {
            self.edicts[self_ent].takedamage = DAMAGE_YES;
        }
    }

    pub fn button_wait(&mut self, self_ent: usize) {
        self.edicts[self_ent].moveinfo.state = STATE_TOP;
        self.edicts[self_ent].s.effects &= !EF_ANIM01;
        self.edicts[self_ent].s.effects |= EF_ANIM23;

        let activator = self.edicts[self_ent].activator;
        crate::g_utils::g_use_targets(self, self_ent, activator as usize);
        self.edicts[self_ent].s.frame = 1;
        if self.edicts[self_ent].moveinfo.wait >= 0.0 {
            self.edicts[self_ent].nextthink = self.level.time + self.edicts[self_ent].moveinfo.wait;
            self.edicts[self_ent].think_fn = Some(crate::dispatch::THINK_FUNC_BUTTON_RETURN);
        }
    }

    pub fn button_fire(&mut self, self_ent: usize) {
        if self.edicts[self_ent].moveinfo.state == STATE_UP || self.edicts[self_ent].moveinfo.state == STATE_TOP {
            return;
        }

        self.edicts[self_ent].moveinfo.state = STATE_UP;
        if self.edicts[self_ent].moveinfo.sound_start != 0 && (!self.edicts[self_ent].flags.intersects(FL_TEAMSLAVE)) {
            gi_sound(self_ent as i32, CHAN_NO_PHS_ADD + CHAN_VOICE, self.edicts[self_ent].moveinfo.sound_start, 1.0, ATTN_STATIC, 0.0);
        }
        let dest = self.edicts[self_ent].moveinfo.end_origin;
        self.move_calc(self_ent, dest, EndFn::ButtonWait as usize);
    }

    pub fn button_use(&mut self, self_ent: usize, _other: usize, activator: usize) {
        self.edicts[self_ent].activator = activator as i32;
        self.button_fire(self_ent);
    }

    pub fn button_touch(&mut self, self_ent: usize, other: usize, _plane: Option<&CPlane>, _surf: Option<&CSurface>) {
        if self.edicts[other].client.is_none() {
            return;
        }
        if self.edicts[other].health <= 0 {
            return;
        }

        self.edicts[self_ent].activator = other as i32;
        self.button_fire(self_ent);
    }

    pub fn button_killed(&mut self, self_ent: usize, _inflictor: usize, attacker: usize, _damage: i32, _point: &[f32; 3]) {
        self.edicts[self_ent].activator = attacker as i32;
        self.edicts[self_ent].health = self.edicts[self_ent].max_health;
        self.edicts[self_ent].takedamage = DAMAGE_NO;
        self.button_fire(self_ent);
    }

    pub fn sp_func_button(&mut self, ent: usize) {
        let angles = self.edicts[ent].s.angles;
        let mut movedir = self.edicts[ent].movedir;
        crate::g_utils::g_set_movedir(&angles, &mut movedir);
        vec3_clear(&mut self.edicts[ent].s.angles);
        self.edicts[ent].movedir = movedir;

        self.edicts[ent].movetype = MoveType::Stop;
        self.edicts[ent].solid = Solid::Bsp;
        let model = self.edicts[ent].model.clone();
        gi_setmodel(ent as i32, &model);

        if self.edicts[ent].sounds != 1 {
            self.edicts[ent].moveinfo.sound_start = gi_soundindex("switches/butn2.wav");
        }

        if self.edicts[ent].speed == 0.0 {
            self.edicts[ent].speed = 40.0;
        }
        if self.edicts[ent].accel == 0.0 {
            self.edicts[ent].accel = self.edicts[ent].speed;
        }
        if self.edicts[ent].decel == 0.0 {
            self.edicts[ent].decel = self.edicts[ent].speed;
        }

        if self.edicts[ent].wait == 0.0 {
            self.edicts[ent].wait = 3.0;
        }
        if self.st.lip == 0 {
            self.st.lip = 4;
        }

        let origin = self.edicts[ent].s.origin;
        vec3_copy(&origin, &mut self.edicts[ent].pos1);

        let movedir = self.edicts[ent].movedir;
        let abs_movedir = [movedir[0].abs(), movedir[1].abs(), movedir[2].abs()];
        let size = self.edicts[ent].size;
        let lip = self.st.lip as f32;
        let dist = abs_movedir[0] * size[0] + abs_movedir[1] * size[1] + abs_movedir[2] * size[2] - lip;
        let pos1 = self.edicts[ent].pos1;
        vec3_ma(&pos1, dist, &movedir, &mut self.edicts[ent].pos2);

        self.edicts[ent].use_fn = Some(crate::dispatch::USE_FUNC_BUTTON);
        self.edicts[ent].s.effects |= EF_ANIM01;

        if self.edicts[ent].health != 0 {
            self.edicts[ent].max_health = self.edicts[ent].health;
            self.edicts[ent].die_fn = Some(crate::dispatch::DIE_BUTTON_KILLED);
            self.edicts[ent].takedamage = DAMAGE_YES;
        } else if self.edicts[ent].targetname.is_empty() {
            self.edicts[ent].touch_fn = Some(crate::dispatch::TOUCH_BUTTON);
        }

        self.edicts[ent].moveinfo.state = STATE_BOTTOM;

        self.edicts[ent].moveinfo.speed = self.edicts[ent].speed;
        self.edicts[ent].moveinfo.accel = self.edicts[ent].accel;
        self.edicts[ent].moveinfo.decel = self.edicts[ent].decel;
        self.edicts[ent].moveinfo.wait = self.edicts[ent].wait;
        let pos1 = self.edicts[ent].pos1;
        let angles = self.edicts[ent].s.angles;
        let pos2 = self.edicts[ent].pos2;
        vec3_copy(&pos1, &mut self.edicts[ent].moveinfo.start_origin);
        vec3_copy(&angles, &mut self.edicts[ent].moveinfo.start_angles);
        vec3_copy(&pos2, &mut self.edicts[ent].moveinfo.end_origin);
        vec3_copy(&angles, &mut self.edicts[ent].moveinfo.end_angles);

        gi_linkentity(ent as i32);
    }

    // =========================================================
    // DOORS
    // =========================================================

    pub fn door_use_areaportals(&mut self, self_ent: usize, open: bool) {
        let target = self.edicts[self_ent].target.clone();
        if target.is_empty() {
            return;
        }

        let mut search_from: usize = 0;
        loop {
            match crate::g_utils::g_find(self, search_from, "targetname", &target) {
                Some(t) => {
                    if self.edicts[t].classname == "func_areaportal" {
                        let style = self.edicts[t].style;
                        gi_set_area_portal_state(style, open);
                    }
                    search_from = t + 1;
                }
                None => break,
            }
        }
    }

    pub fn door_hit_top(&mut self, self_ent: usize) {
        if !self.edicts[self_ent].flags.intersects(FL_TEAMSLAVE) {
            if self.edicts[self_ent].moveinfo.sound_end != 0 {
                gi_sound(self_ent as i32, CHAN_NO_PHS_ADD + CHAN_VOICE, self.edicts[self_ent].moveinfo.sound_end, 1.0, ATTN_STATIC, 0.0);
            }
            self.edicts[self_ent].s.sound = 0;
        }
        self.edicts[self_ent].moveinfo.state = STATE_TOP;
        if self.edicts[self_ent].spawnflags & DOOR_TOGGLE != 0 {
            return;
        }
        if self.edicts[self_ent].moveinfo.wait >= 0.0 {
            self.edicts[self_ent].think_fn = Some(crate::dispatch::THINK_FUNC_DOOR_GO_DOWN);
            self.edicts[self_ent].nextthink = self.level.time + self.edicts[self_ent].moveinfo.wait;
        }
    }

    pub fn door_hit_bottom(&mut self, self_ent: usize) {
        if !self.edicts[self_ent].flags.intersects(FL_TEAMSLAVE) {
            if self.edicts[self_ent].moveinfo.sound_end != 0 {
                gi_sound(self_ent as i32, CHAN_NO_PHS_ADD + CHAN_VOICE, self.edicts[self_ent].moveinfo.sound_end, 1.0, ATTN_STATIC, 0.0);
            }
            self.edicts[self_ent].s.sound = 0;
        }
        self.edicts[self_ent].moveinfo.state = STATE_BOTTOM;
        self.door_use_areaportals(self_ent, false);
    }

    pub fn door_go_down(&mut self, self_ent: usize) {
        if !self.edicts[self_ent].flags.intersects(FL_TEAMSLAVE) {
            if self.edicts[self_ent].moveinfo.sound_start != 0 {
                gi_sound(self_ent as i32, CHAN_NO_PHS_ADD + CHAN_VOICE, self.edicts[self_ent].moveinfo.sound_start, 1.0, ATTN_STATIC, 0.0);
            }
            self.edicts[self_ent].s.sound = self.edicts[self_ent].moveinfo.sound_middle;
        }
        if self.edicts[self_ent].max_health != 0 {
            self.edicts[self_ent].takedamage = DAMAGE_YES;
            self.edicts[self_ent].health = self.edicts[self_ent].max_health;
        }

        self.edicts[self_ent].moveinfo.state = STATE_DOWN;
        let classname = self.edicts[self_ent].classname.clone();
        if classname == "func_door" {
            let dest = self.edicts[self_ent].moveinfo.start_origin;
            self.move_calc(self_ent, dest, EndFn::DoorHitBottom as usize);
        } else if classname == "func_door_rotating" {
            self.angle_move_calc(self_ent, EndFn::DoorHitBottom as usize);
        }
    }

    pub fn door_go_up(&mut self, self_ent: usize, activator: usize) {
        if self.edicts[self_ent].moveinfo.state == STATE_UP {
            return; // already going up
        }

        if self.edicts[self_ent].moveinfo.state == STATE_TOP {
            if self.edicts[self_ent].moveinfo.wait >= 0.0 {
                self.edicts[self_ent].nextthink = self.level.time + self.edicts[self_ent].moveinfo.wait;
            }
            return;
        }

        if !self.edicts[self_ent].flags.intersects(FL_TEAMSLAVE) {
            if self.edicts[self_ent].moveinfo.sound_start != 0 {
                gi_sound(self_ent as i32, CHAN_NO_PHS_ADD + CHAN_VOICE, self.edicts[self_ent].moveinfo.sound_start, 1.0, ATTN_STATIC, 0.0);
            }
            self.edicts[self_ent].s.sound = self.edicts[self_ent].moveinfo.sound_middle;
        }
        self.edicts[self_ent].moveinfo.state = STATE_UP;
        let classname = self.edicts[self_ent].classname.clone();
        if classname == "func_door" {
            let dest = self.edicts[self_ent].moveinfo.end_origin;
            self.move_calc(self_ent, dest, EndFn::DoorHitTop as usize);
        } else if classname == "func_door_rotating" {
            self.angle_move_calc(self_ent, EndFn::DoorHitTop as usize);
        }

        crate::g_utils::g_use_targets(self, self_ent, activator);
        self.door_use_areaportals(self_ent, true);
    }

    pub fn door_use(&mut self, self_ent: usize, _other: usize, activator: usize) {
        if self.edicts[self_ent].flags.intersects(FL_TEAMSLAVE) {
            return;
        }

        if self.edicts[self_ent].spawnflags & DOOR_TOGGLE != 0
            && (self.edicts[self_ent].moveinfo.state == STATE_UP || self.edicts[self_ent].moveinfo.state == STATE_TOP) {
                // trigger all paired doors
                let mut ent_idx = self_ent as i32;
                while ent_idx >= 0 {
                    let e = ent_idx as usize;
                    self.edicts[e].message = String::new();
                    self.edicts[e].touch_fn = None;
                    self.door_go_down(e);
                    ent_idx = self.edicts[e].teamchain;
                }
                return;
            }

        // trigger all paired doors
        let mut ent_idx = self_ent as i32;
        while ent_idx >= 0 {
            let e = ent_idx as usize;
            self.edicts[e].message = String::new();
            self.edicts[e].touch_fn = None;
            self.door_go_up(e, activator);
            ent_idx = self.edicts[e].teamchain;
        }
    }

    pub fn touch_door_trigger(&mut self, self_ent: usize, other: usize, _plane: Option<&CPlane>, _surf: Option<&CSurface>) {
        if self.edicts[other].health <= 0 {
            return;
        }
        if (self.edicts[other].svflags & SVF_MONSTER == 0) && self.edicts[other].client.is_none() {
            return;
        }

        let owner = self.edicts[self_ent].owner as usize;
        if (self.edicts[owner].spawnflags & DOOR_NOMONSTER != 0) && (self.edicts[other].svflags & SVF_MONSTER != 0) {
            return;
        }

        if self.level.time < self.edicts[self_ent].touch_debounce_time {
            return;
        }
        self.edicts[self_ent].touch_debounce_time = self.level.time + 1.0;

        self.door_use(owner, other, other);
    }

    pub fn think_calc_move_speed(&mut self, self_ent: usize) {
        if self.edicts[self_ent].flags.intersects(FL_TEAMSLAVE) {
            return;
        }

        // find the smallest distance any member of the team will be moving
        let mut min = self.edicts[self_ent].moveinfo.distance.abs();
        let mut ent_idx = self.edicts[self_ent].teamchain;
        while ent_idx >= 0 {
            let e = ent_idx as usize;
            let dist = self.edicts[e].moveinfo.distance.abs();
            if dist < min {
                min = dist;
            }
            ent_idx = self.edicts[e].teamchain;
        }

        let time = min / self.edicts[self_ent].moveinfo.speed;

        // adjust speeds so they will all complete at the same time
        let mut ent_idx = self_ent as i32;
        while ent_idx >= 0 {
            let e = ent_idx as usize;
            let newspeed = self.edicts[e].moveinfo.distance.abs() / time;
            let ratio = newspeed / self.edicts[e].moveinfo.speed;
            if self.edicts[e].moveinfo.accel == self.edicts[e].moveinfo.speed {
                self.edicts[e].moveinfo.accel = newspeed;
            } else {
                self.edicts[e].moveinfo.accel *= ratio;
            }
            if self.edicts[e].moveinfo.decel == self.edicts[e].moveinfo.speed {
                self.edicts[e].moveinfo.decel = newspeed;
            } else {
                self.edicts[e].moveinfo.decel *= ratio;
            }
            self.edicts[e].moveinfo.speed = newspeed;
            ent_idx = self.edicts[e].teamchain;
        }
    }

    pub fn think_spawn_door_trigger(&mut self, ent: usize) {
        if self.edicts[ent].flags.intersects(FL_TEAMSLAVE) {
            return;
        }

        let mut mins = self.edicts[ent].absmin;
        let mut maxs = self.edicts[ent].absmax;

        let mut other_idx = self.edicts[ent].teamchain;
        while other_idx >= 0 {
            let o = other_idx as usize;
            add_point_to_bounds(&self.edicts[o].absmin.clone(), &mut mins, &mut maxs);
            add_point_to_bounds(&self.edicts[o].absmax.clone(), &mut mins, &mut maxs);
            other_idx = self.edicts[o].teamchain;
        }

        // expand
        mins[0] -= 60.0;
        mins[1] -= 60.0;
        maxs[0] += 60.0;
        maxs[1] += 60.0;

        let other = crate::g_utils::g_spawn(self);
        vec3_copy(&mins, &mut self.edicts[other].mins);
        vec3_copy(&maxs, &mut self.edicts[other].maxs);
        self.edicts[other].owner = ent as i32;
        self.edicts[other].solid = Solid::Trigger;
        self.edicts[other].movetype = MoveType::None;
        self.edicts[other].touch_fn = Some(crate::dispatch::TOUCH_FUNC_DOOR);
        gi_linkentity(other as i32);

        if self.edicts[ent].spawnflags & DOOR_START_OPEN != 0 {
            self.door_use_areaportals(ent, true);
        }

        self.think_calc_move_speed(ent);
    }

    pub fn door_blocked(&mut self, self_ent: usize, other: usize) {
        if (self.edicts[other].svflags & SVF_MONSTER == 0) && self.edicts[other].client.is_none() {
            let origin = self.edicts[other].s.origin;
            crate::g_combat::ctx_t_damage(self, other, self_ent, self_ent, &VEC3_ORIGIN, &origin, &VEC3_ORIGIN, 100000, 1, DamageFlags::empty(), MOD_CRUSH);
            if self.edicts[other].inuse {
                crate::g_misc::become_explosion1(self, other);
            }
            return;
        }

        let dmg = self.edicts[self_ent].dmg;
        let origin = self.edicts[other].s.origin;
        crate::g_combat::ctx_t_damage(self, other, self_ent, self_ent, &VEC3_ORIGIN, &origin, &VEC3_ORIGIN, dmg, 1, DamageFlags::empty(), MOD_CRUSH);

        if self.edicts[self_ent].spawnflags & DOOR_CRUSHER != 0 {
            return;
        }

        if self.edicts[self_ent].moveinfo.wait >= 0.0 {
            let teammaster = self.edicts[self_ent].teammaster;
            if self.edicts[self_ent].moveinfo.state == STATE_DOWN {
                let mut ent_idx = teammaster;
                while ent_idx >= 0 {
                    let e = ent_idx as usize;
                    let act = self.edicts[e].activator as usize;
                    self.door_go_up(e, act);
                    ent_idx = self.edicts[e].teamchain;
                }
            } else {
                let mut ent_idx = teammaster;
                while ent_idx >= 0 {
                    let e = ent_idx as usize;
                    self.door_go_down(e);
                    ent_idx = self.edicts[e].teamchain;
                }
            }
        }
    }

    pub fn door_killed(&mut self, self_ent: usize, _inflictor: usize, attacker: usize, _damage: i32, _point: &[f32; 3]) {
        let teammaster = self.edicts[self_ent].teammaster;
        let mut ent_idx = teammaster;
        while ent_idx >= 0 {
            let e = ent_idx as usize;
            self.edicts[e].health = self.edicts[e].max_health;
            self.edicts[e].takedamage = DAMAGE_NO;
            ent_idx = self.edicts[e].teamchain;
        }
        self.door_use(teammaster as usize, attacker, attacker);
    }

    pub fn door_touch(&mut self, self_ent: usize, other: usize, _plane: Option<&CPlane>, _surf: Option<&CSurface>) {
        if self.edicts[other].client.is_none() {
            return;
        }

        if self.level.time < self.edicts[self_ent].touch_debounce_time {
            return;
        }
        self.edicts[self_ent].touch_debounce_time = self.level.time + 5.0;

        let msg = self.edicts[self_ent].message.clone();
        gi_centerprintf(other as i32, &msg);
        let si = gi_soundindex("misc/talk1.wav");
        gi_sound(other as i32, CHAN_AUTO, si, 1.0, ATTN_NORM, 0.0);
    }

    pub fn sp_func_door(&mut self, ent: usize) {
        if self.edicts[ent].sounds != 1 {
            self.edicts[ent].moveinfo.sound_start = gi_soundindex("doors/dr1_strt.wav");
            self.edicts[ent].moveinfo.sound_middle = gi_soundindex("doors/dr1_mid.wav");
            self.edicts[ent].moveinfo.sound_end = gi_soundindex("doors/dr1_end.wav");
        }

        let angles = self.edicts[ent].s.angles;
        let mut movedir = self.edicts[ent].movedir;
        crate::g_utils::g_set_movedir(&angles, &mut movedir);
        vec3_clear(&mut self.edicts[ent].s.angles);
        self.edicts[ent].movedir = movedir;

        self.edicts[ent].movetype = MoveType::Push;
        self.edicts[ent].solid = Solid::Bsp;
        let model = self.edicts[ent].model.clone();
        gi_setmodel(ent as i32, &model);

        self.edicts[ent].blocked_fn = Some(crate::dispatch::BLOCKED_FUNC_DOOR);
        self.edicts[ent].use_fn = Some(crate::dispatch::USE_FUNC_DOOR);

        if self.edicts[ent].speed == 0.0 {
            self.edicts[ent].speed = 100.0;
        }
        if self.deathmatch != 0.0 {
            self.edicts[ent].speed *= 2.0;
        }

        if self.edicts[ent].accel == 0.0 {
            self.edicts[ent].accel = self.edicts[ent].speed;
        }
        if self.edicts[ent].decel == 0.0 {
            self.edicts[ent].decel = self.edicts[ent].speed;
        }

        if self.edicts[ent].wait == 0.0 {
            self.edicts[ent].wait = 3.0;
        }
        if self.st.lip == 0 {
            self.st.lip = 8;
        }
        if self.edicts[ent].dmg == 0 {
            self.edicts[ent].dmg = 2;
        }

        // calculate second position
        let origin = self.edicts[ent].s.origin;
        vec3_copy(&origin, &mut self.edicts[ent].pos1);
        let movedir = self.edicts[ent].movedir;
        let abs_movedir = [movedir[0].abs(), movedir[1].abs(), movedir[2].abs()];
        let size = self.edicts[ent].size;
        let lip = self.st.lip as f32;
        self.edicts[ent].moveinfo.distance = abs_movedir[0] * size[0] + abs_movedir[1] * size[1] + abs_movedir[2] * size[2] - lip;
        let pos1 = self.edicts[ent].pos1;
        let dist = self.edicts[ent].moveinfo.distance;
        vec3_ma(&pos1, dist, &movedir, &mut self.edicts[ent].pos2);

        // if it starts open, switch the positions
        if self.edicts[ent].spawnflags & DOOR_START_OPEN != 0 {
            let pos2 = self.edicts[ent].pos2;
            let pos1 = self.edicts[ent].pos1;
            vec3_copy(&pos2, &mut self.edicts[ent].s.origin);
            vec3_copy(&pos1, &mut self.edicts[ent].pos2);
            let origin = self.edicts[ent].s.origin;
            vec3_copy(&origin, &mut self.edicts[ent].pos1);
        }

        self.edicts[ent].moveinfo.state = STATE_BOTTOM;

        if self.edicts[ent].health != 0 {
            self.edicts[ent].takedamage = DAMAGE_YES;
            self.edicts[ent].die_fn = Some(crate::dispatch::DIE_DOOR_KILLED);
            self.edicts[ent].max_health = self.edicts[ent].health;
        } else if !self.edicts[ent].targetname.is_empty() && !self.edicts[ent].message.is_empty() {
            gi_soundindex("misc/talk.wav");
            self.edicts[ent].touch_fn = Some(crate::dispatch::TOUCH_DOOR);
        }

        self.edicts[ent].moveinfo.speed = self.edicts[ent].speed;
        self.edicts[ent].moveinfo.accel = self.edicts[ent].accel;
        self.edicts[ent].moveinfo.decel = self.edicts[ent].decel;
        self.edicts[ent].moveinfo.wait = self.edicts[ent].wait;
        let pos1 = self.edicts[ent].pos1;
        let angles = self.edicts[ent].s.angles;
        let pos2 = self.edicts[ent].pos2;
        vec3_copy(&pos1, &mut self.edicts[ent].moveinfo.start_origin);
        vec3_copy(&angles, &mut self.edicts[ent].moveinfo.start_angles);
        vec3_copy(&pos2, &mut self.edicts[ent].moveinfo.end_origin);
        vec3_copy(&angles, &mut self.edicts[ent].moveinfo.end_angles);

        if self.edicts[ent].spawnflags & 16 != 0 {
            self.edicts[ent].s.effects |= EF_ANIM_ALL;
        }
        if self.edicts[ent].spawnflags & 64 != 0 {
            self.edicts[ent].s.effects |= EF_ANIM_ALLFAST;
        }

        // to simplify logic elsewhere, make non-teamed doors into a team of one
        if self.edicts[ent].team.is_empty() {
            self.edicts[ent].teammaster = ent as i32;
        }

        gi_linkentity(ent as i32);

        self.edicts[ent].nextthink = self.level.time + FRAMETIME;
        if self.edicts[ent].health != 0 || !self.edicts[ent].targetname.is_empty() {
            self.edicts[ent].think_fn = Some(crate::dispatch::THINK_FUNC_CALC_MOVE_SPEED);
        } else {
            self.edicts[ent].think_fn = Some(crate::dispatch::THINK_FUNC_SPAWN_DOOR_TRIGGER);
        }
    }

    pub fn sp_func_door_rotating(&mut self, ent: usize) {
        vec3_clear(&mut self.edicts[ent].s.angles);

        // set the axis of rotation
        vec3_clear(&mut self.edicts[ent].movedir);
        if self.edicts[ent].spawnflags & DOOR_X_AXIS != 0 {
            self.edicts[ent].movedir[2] = 1.0;
        } else if self.edicts[ent].spawnflags & DOOR_Y_AXIS != 0 {
            self.edicts[ent].movedir[0] = 1.0;
        } else {
            self.edicts[ent].movedir[1] = 1.0;
        }

        // check for reverse rotation
        if self.edicts[ent].spawnflags & DOOR_REVERSE != 0 {
            let md = self.edicts[ent].movedir;
            vec3_negate(&md, &mut self.edicts[ent].movedir);
        }

        if self.st.distance == 0 {
            let origin = self.edicts[ent].s.origin;
            let classname = self.edicts[ent].classname.clone();
            gi_dprintf(&format!("{} at {} with no distance set", classname, vtos(&origin)));
            self.st.distance = 90;
        }

        let angles = self.edicts[ent].s.angles;
        vec3_copy(&angles, &mut self.edicts[ent].pos1);
        let distance = self.st.distance as f32;
        let movedir = self.edicts[ent].movedir;
        vec3_ma(&angles, distance, &movedir, &mut self.edicts[ent].pos2);
        self.edicts[ent].moveinfo.distance = distance;

        self.edicts[ent].movetype = MoveType::Push;
        self.edicts[ent].solid = Solid::Bsp;
        let model = self.edicts[ent].model.clone();
        gi_setmodel(ent as i32, &model);

        self.edicts[ent].blocked_fn = Some(crate::dispatch::BLOCKED_FUNC_DOOR);
        self.edicts[ent].use_fn = Some(crate::dispatch::USE_FUNC_DOOR);

        if self.edicts[ent].speed == 0.0 {
            self.edicts[ent].speed = 100.0;
        }
        if self.edicts[ent].accel == 0.0 {
            self.edicts[ent].accel = self.edicts[ent].speed;
        }
        if self.edicts[ent].decel == 0.0 {
            self.edicts[ent].decel = self.edicts[ent].speed;
        }

        if self.edicts[ent].wait == 0.0 {
            self.edicts[ent].wait = 3.0;
        }
        if self.edicts[ent].dmg == 0 {
            self.edicts[ent].dmg = 2;
        }

        if self.edicts[ent].sounds != 1 {
            self.edicts[ent].moveinfo.sound_start = gi_soundindex("doors/dr1_strt.wav");
            self.edicts[ent].moveinfo.sound_middle = gi_soundindex("doors/dr1_mid.wav");
            self.edicts[ent].moveinfo.sound_end = gi_soundindex("doors/dr1_end.wav");
        }

        // if it starts open, switch the positions
        if self.edicts[ent].spawnflags & DOOR_START_OPEN != 0 {
            let pos2 = self.edicts[ent].pos2;
            let pos1 = self.edicts[ent].pos1;
            vec3_copy(&pos2, &mut self.edicts[ent].s.angles);
            vec3_copy(&pos1, &mut self.edicts[ent].pos2);
            let angles = self.edicts[ent].s.angles;
            vec3_copy(&angles, &mut self.edicts[ent].pos1);
            let md = self.edicts[ent].movedir;
            vec3_negate(&md, &mut self.edicts[ent].movedir);
        }

        if self.edicts[ent].health != 0 {
            self.edicts[ent].takedamage = DAMAGE_YES;
            self.edicts[ent].die_fn = Some(crate::dispatch::DIE_DOOR_KILLED);
            self.edicts[ent].max_health = self.edicts[ent].health;
        }

        if !self.edicts[ent].targetname.is_empty() && !self.edicts[ent].message.is_empty() {
            gi_soundindex("misc/talk.wav");
            self.edicts[ent].touch_fn = Some(crate::dispatch::TOUCH_DOOR);
        }

        self.edicts[ent].moveinfo.state = STATE_BOTTOM;
        self.edicts[ent].moveinfo.speed = self.edicts[ent].speed;
        self.edicts[ent].moveinfo.accel = self.edicts[ent].accel;
        self.edicts[ent].moveinfo.decel = self.edicts[ent].decel;
        self.edicts[ent].moveinfo.wait = self.edicts[ent].wait;
        let origin = self.edicts[ent].s.origin;
        let pos1 = self.edicts[ent].pos1;
        let pos2 = self.edicts[ent].pos2;
        vec3_copy(&origin, &mut self.edicts[ent].moveinfo.start_origin);
        vec3_copy(&pos1, &mut self.edicts[ent].moveinfo.start_angles);
        vec3_copy(&origin, &mut self.edicts[ent].moveinfo.end_origin);
        vec3_copy(&pos2, &mut self.edicts[ent].moveinfo.end_angles);

        if self.edicts[ent].spawnflags & 16 != 0 {
            self.edicts[ent].s.effects |= EF_ANIM_ALL;
        }

        // to simplify logic elsewhere, make non-teamed doors into a team of one
        if self.edicts[ent].team.is_empty() {
            self.edicts[ent].teammaster = ent as i32;
        }

        gi_linkentity(ent as i32);

        self.edicts[ent].nextthink = self.level.time + FRAMETIME;
        if self.edicts[ent].health != 0 || !self.edicts[ent].targetname.is_empty() {
            self.edicts[ent].think_fn = Some(crate::dispatch::THINK_FUNC_CALC_MOVE_SPEED);
        } else {
            self.edicts[ent].think_fn = Some(crate::dispatch::THINK_FUNC_SPAWN_DOOR_TRIGGER);
        }
    }

    // =========================================================
    // WATER
    // =========================================================

    pub fn sp_func_water(&mut self, self_ent: usize) {
        let angles = self.edicts[self_ent].s.angles;
        let mut movedir = self.edicts[self_ent].movedir;
        crate::g_utils::g_set_movedir(&angles, &mut movedir);
        vec3_clear(&mut self.edicts[self_ent].s.angles);
        self.edicts[self_ent].movedir = movedir;

        self.edicts[self_ent].movetype = MoveType::Push;
        self.edicts[self_ent].solid = Solid::Bsp;
        let model = self.edicts[self_ent].model.clone();
        gi_setmodel(self_ent as i32, &model);

        match self.edicts[self_ent].sounds {
            1 => {
                self.edicts[self_ent].moveinfo.sound_start = gi_soundindex("world/mov_watr.wav");
                self.edicts[self_ent].moveinfo.sound_end = gi_soundindex("world/stp_watr.wav");
            }
            2 => {
                self.edicts[self_ent].moveinfo.sound_start = gi_soundindex("world/mov_watr.wav");
                self.edicts[self_ent].moveinfo.sound_end = gi_soundindex("world/stp_watr.wav");
            }
            _ => {}
        }

        // calculate second position
        let origin = self.edicts[self_ent].s.origin;
        vec3_copy(&origin, &mut self.edicts[self_ent].pos1);
        let movedir = self.edicts[self_ent].movedir;
        let abs_movedir = [movedir[0].abs(), movedir[1].abs(), movedir[2].abs()];
        let size = self.edicts[self_ent].size;
        let lip = self.st.lip as f32;
        self.edicts[self_ent].moveinfo.distance = abs_movedir[0] * size[0] + abs_movedir[1] * size[1] + abs_movedir[2] * size[2] - lip;
        let pos1 = self.edicts[self_ent].pos1;
        let dist = self.edicts[self_ent].moveinfo.distance;
        vec3_ma(&pos1, dist, &movedir, &mut self.edicts[self_ent].pos2);

        // if it starts open, switch the positions
        if self.edicts[self_ent].spawnflags & DOOR_START_OPEN != 0 {
            let pos2 = self.edicts[self_ent].pos2;
            let pos1 = self.edicts[self_ent].pos1;
            vec3_copy(&pos2, &mut self.edicts[self_ent].s.origin);
            vec3_copy(&pos1, &mut self.edicts[self_ent].pos2);
            let origin = self.edicts[self_ent].s.origin;
            vec3_copy(&origin, &mut self.edicts[self_ent].pos1);
        }

        let pos1 = self.edicts[self_ent].pos1;
        let angles = self.edicts[self_ent].s.angles;
        let pos2 = self.edicts[self_ent].pos2;
        vec3_copy(&pos1, &mut self.edicts[self_ent].moveinfo.start_origin);
        vec3_copy(&angles, &mut self.edicts[self_ent].moveinfo.start_angles);
        vec3_copy(&pos2, &mut self.edicts[self_ent].moveinfo.end_origin);
        vec3_copy(&angles, &mut self.edicts[self_ent].moveinfo.end_angles);

        self.edicts[self_ent].moveinfo.state = STATE_BOTTOM;

        if self.edicts[self_ent].speed == 0.0 {
            self.edicts[self_ent].speed = 25.0;
        }
        let speed = self.edicts[self_ent].speed;
        self.edicts[self_ent].moveinfo.accel = speed;
        self.edicts[self_ent].moveinfo.decel = speed;
        self.edicts[self_ent].moveinfo.speed = speed;

        if self.edicts[self_ent].wait == 0.0 {
            self.edicts[self_ent].wait = -1.0;
        }
        self.edicts[self_ent].moveinfo.wait = self.edicts[self_ent].wait;

        self.edicts[self_ent].use_fn = Some(crate::dispatch::USE_FUNC_DOOR);

        if self.edicts[self_ent].wait == -1.0 {
            self.edicts[self_ent].spawnflags |= DOOR_TOGGLE;
        }

        self.edicts[self_ent].classname = "func_door".to_string();

        gi_linkentity(self_ent as i32);
    }

    // =========================================================
    // TRAINS
    // =========================================================

    pub fn train_blocked(&mut self, self_ent: usize, other: usize) {
        if (self.edicts[other].svflags & SVF_MONSTER == 0) && self.edicts[other].client.is_none() {
            let origin = self.edicts[other].s.origin;
            crate::g_combat::ctx_t_damage(self, other, self_ent, self_ent, &VEC3_ORIGIN, &origin, &VEC3_ORIGIN, 100000, 1, DamageFlags::empty(), MOD_CRUSH);
            if self.edicts[other].inuse {
                crate::g_misc::become_explosion1(self, other);
            }
            return;
        }

        if self.level.time < self.edicts[self_ent].touch_debounce_time {
            return;
        }

        if self.edicts[self_ent].dmg == 0 {
            return;
        }
        self.edicts[self_ent].touch_debounce_time = self.level.time + 0.5;
        let dmg = self.edicts[self_ent].dmg;
        let origin = self.edicts[other].s.origin;
        crate::g_combat::ctx_t_damage(self, other, self_ent, self_ent, &VEC3_ORIGIN, &origin, &VEC3_ORIGIN, dmg, 1, DamageFlags::empty(), MOD_CRUSH);
    }

    pub fn train_wait(&mut self, self_ent: usize) {
        let target_ent = self.edicts[self_ent].target_ent as usize;
        if !self.edicts[target_ent].pathtarget.is_empty() {
            let savetarget = self.edicts[target_ent].target.clone();
            self.edicts[target_ent].target = self.edicts[target_ent].pathtarget.clone();
            let activator = self.edicts[self_ent].activator as usize;
            crate::g_utils::g_use_targets(self, target_ent, activator);
            self.edicts[target_ent].target = savetarget;

            // make sure we didn't get killed by a killtarget
            if !self.edicts[self_ent].inuse {
                return;
            }
        }

        if self.edicts[self_ent].moveinfo.wait != 0.0 {
            if self.edicts[self_ent].moveinfo.wait > 0.0 {
                self.edicts[self_ent].nextthink = self.level.time + self.edicts[self_ent].moveinfo.wait;
                self.edicts[self_ent].think_fn = Some(crate::dispatch::THINK_FUNC_TRAIN_NEXT);
            } else if self.edicts[self_ent].spawnflags & TRAIN_TOGGLE != 0 {
                // wait < 0
                self.train_next(self_ent);
                self.edicts[self_ent].spawnflags &= !TRAIN_START_ON;
                vec3_clear(&mut self.edicts[self_ent].velocity);
                self.edicts[self_ent].nextthink = 0.0;
            }

            if !self.edicts[self_ent].flags.intersects(FL_TEAMSLAVE) {
                if self.edicts[self_ent].moveinfo.sound_end != 0 {
                    gi_sound(self_ent as i32, CHAN_NO_PHS_ADD + CHAN_VOICE, self.edicts[self_ent].moveinfo.sound_end, 1.0, ATTN_STATIC, 0.0);
                }
                self.edicts[self_ent].s.sound = 0;
            }
        } else {
            self.train_next(self_ent);
        }
    }

    pub fn train_next(&mut self, self_ent: usize) {
        let mut first = true;

        loop {
            if self.edicts[self_ent].target.is_empty() {
                return;
            }

            let target = self.edicts[self_ent].target.clone();
            let ent_opt = crate::g_utils::g_pick_target(self, &target);
            if ent_opt.is_none() {
                gi_dprintf(&format!("train_next: bad target {}", target));
                return;
            }
            let path_ent = ent_opt.unwrap();

            self.edicts[self_ent].target = self.edicts[path_ent].target.clone();

            // check for a teleport path_corner
            if self.edicts[path_ent].spawnflags & 1 != 0 {
                if !first {
                    let classname = self.edicts[path_ent].classname.clone();
                    let origin = self.edicts[path_ent].s.origin;
                    gi_dprintf(&format!("connected teleport path_corners, see {} at {}", classname, vtos(&origin)));
                    return;
                }
                first = false;
                let path_origin = self.edicts[path_ent].s.origin;
                let self_mins = self.edicts[self_ent].mins;
                vec3_subtract(&path_origin, &self_mins, &mut self.edicts[self_ent].s.origin);
                let origin = self.edicts[self_ent].s.origin;
                vec3_copy(&origin, &mut self.edicts[self_ent].s.old_origin);
                self.edicts[self_ent].s.event = EV_OTHER_TELEPORT;
                gi_linkentity(self_ent as i32);
                continue; // goto again
            }

            self.edicts[self_ent].moveinfo.wait = self.edicts[path_ent].wait;
            self.edicts[self_ent].target_ent = path_ent as i32;

            if !self.edicts[self_ent].flags.intersects(FL_TEAMSLAVE) {
                if self.edicts[self_ent].moveinfo.sound_start != 0 {
                    gi_sound(self_ent as i32, CHAN_NO_PHS_ADD + CHAN_VOICE, self.edicts[self_ent].moveinfo.sound_start, 1.0, ATTN_STATIC, 0.0);
                }
                self.edicts[self_ent].s.sound = self.edicts[self_ent].moveinfo.sound_middle;
            }

            let path_origin = self.edicts[path_ent].s.origin;
            let self_mins = self.edicts[self_ent].mins;
            let mut dest = [0.0f32; 3];
            vec3_subtract(&path_origin, &self_mins, &mut dest);
            self.edicts[self_ent].moveinfo.state = STATE_TOP;
            let origin = self.edicts[self_ent].s.origin;
            vec3_copy(&origin, &mut self.edicts[self_ent].moveinfo.start_origin);
            vec3_copy(&dest, &mut self.edicts[self_ent].moveinfo.end_origin);
            self.move_calc(self_ent, dest, EndFn::TrainWait as usize);
            self.edicts[self_ent].spawnflags |= TRAIN_START_ON;
            break;
        }
    }

    pub fn train_resume(&mut self, self_ent: usize) {
        let target_ent = self.edicts[self_ent].target_ent as usize;
        let path_origin = self.edicts[target_ent].s.origin;
        let self_mins = self.edicts[self_ent].mins;
        let mut dest = [0.0f32; 3];
        vec3_subtract(&path_origin, &self_mins, &mut dest);
        self.edicts[self_ent].moveinfo.state = STATE_TOP;
        let origin = self.edicts[self_ent].s.origin;
        vec3_copy(&origin, &mut self.edicts[self_ent].moveinfo.start_origin);
        vec3_copy(&dest, &mut self.edicts[self_ent].moveinfo.end_origin);
        self.move_calc(self_ent, dest, EndFn::TrainWait as usize);
        self.edicts[self_ent].spawnflags |= TRAIN_START_ON;
    }

    pub fn func_train_find(&mut self, self_ent: usize) {
        if self.edicts[self_ent].target.is_empty() {
            gi_dprintf("train_find: no target");
            return;
        }
        let target = self.edicts[self_ent].target.clone();
        let ent_opt = crate::g_utils::g_pick_target(self, &target);
        if ent_opt.is_none() {
            gi_dprintf(&format!("train_find: target {} not found", target));
            return;
        }
        let path_ent = ent_opt.unwrap();
        self.edicts[self_ent].target = self.edicts[path_ent].target.clone();

        let path_origin = self.edicts[path_ent].s.origin;
        let self_mins = self.edicts[self_ent].mins;
        vec3_subtract(&path_origin, &self_mins, &mut self.edicts[self_ent].s.origin);
        gi_linkentity(self_ent as i32);

        // if not triggered, start immediately
        if self.edicts[self_ent].targetname.is_empty() {
            self.edicts[self_ent].spawnflags |= TRAIN_START_ON;
        }

        if self.edicts[self_ent].spawnflags & TRAIN_START_ON != 0 {
            self.edicts[self_ent].nextthink = self.level.time + FRAMETIME;
            self.edicts[self_ent].think_fn = Some(crate::dispatch::THINK_FUNC_TRAIN_NEXT);
            self.edicts[self_ent].activator = self_ent as i32;
        }
    }

    pub fn train_use(&mut self, self_ent: usize, _other: usize, activator: usize) {
        self.edicts[self_ent].activator = activator as i32;

        if self.edicts[self_ent].spawnflags & TRAIN_START_ON != 0 {
            if self.edicts[self_ent].spawnflags & TRAIN_TOGGLE == 0 {
                return;
            }
            self.edicts[self_ent].spawnflags &= !TRAIN_START_ON;
            vec3_clear(&mut self.edicts[self_ent].velocity);
            self.edicts[self_ent].nextthink = 0.0;
        } else if self.edicts[self_ent].target_ent >= 0 {
            self.train_resume(self_ent);
        } else {
            self.train_next(self_ent);
        }
    }

    pub fn sp_func_train(&mut self, self_ent: usize) {
        self.edicts[self_ent].movetype = MoveType::Push;

        vec3_clear(&mut self.edicts[self_ent].s.angles);
        self.edicts[self_ent].blocked_fn = Some(crate::dispatch::BLOCKED_FUNC_TRAIN);
        if self.edicts[self_ent].spawnflags & TRAIN_BLOCK_STOPS != 0 {
            self.edicts[self_ent].dmg = 0;
        } else if self.edicts[self_ent].dmg == 0 {
            self.edicts[self_ent].dmg = 100;
        }
        self.edicts[self_ent].solid = Solid::Bsp;
        let model = self.edicts[self_ent].model.clone();
        gi_setmodel(self_ent as i32, &model);

        if !self.st.noise.is_empty() {
            let noise = self.st.noise.clone();
            self.edicts[self_ent].moveinfo.sound_middle = gi_soundindex(&noise);
        }

        if self.edicts[self_ent].speed == 0.0 {
            self.edicts[self_ent].speed = 100.0;
        }

        let speed = self.edicts[self_ent].speed;
        self.edicts[self_ent].moveinfo.speed = speed;
        self.edicts[self_ent].moveinfo.accel = speed;
        self.edicts[self_ent].moveinfo.decel = speed;

        self.edicts[self_ent].use_fn = Some(crate::dispatch::USE_FUNC_TRAIN);

        gi_linkentity(self_ent as i32);

        if !self.edicts[self_ent].target.is_empty() {
            self.edicts[self_ent].nextthink = self.level.time + FRAMETIME;
            self.edicts[self_ent].think_fn = Some(crate::dispatch::THINK_FUNC_TRAIN_FIND);
        } else {
            let absmin = self.edicts[self_ent].absmin;
            gi_dprintf(&format!("func_train without a target at {}", vtos(&absmin)));
        }
    }

    // =========================================================
    // TRIGGER ELEVATOR
    // =========================================================

    pub fn trigger_elevator_use(&mut self, self_ent: usize, other: usize, _activator: usize) {
        let movetarget = self.edicts[self_ent].movetarget as usize;
        if self.edicts[movetarget].nextthink != 0.0 {
            return;
        }

        if self.edicts[other].pathtarget.is_empty() {
            gi_dprintf("elevator used with no pathtarget");
            return;
        }

        let pathtarget = self.edicts[other].pathtarget.clone();
        let target_opt = crate::g_utils::g_pick_target(self, &pathtarget);
        if target_opt.is_none() {
            gi_dprintf(&format!("elevator used with bad pathtarget: {}", pathtarget));
            return;
        }
        let target = target_opt.unwrap();

        self.edicts[movetarget].target_ent = target as i32;
        self.train_resume(movetarget);
    }

    pub fn trigger_elevator_init(&mut self, self_ent: usize) {
        if self.edicts[self_ent].target.is_empty() {
            gi_dprintf("trigger_elevator has no target");
            return;
        }
        let target = self.edicts[self_ent].target.clone();
        let mt_opt = crate::g_utils::g_pick_target(self, &target);
        if mt_opt.is_none() {
            gi_dprintf(&format!("trigger_elevator unable to find target {}", target));
            return;
        }
        let mt = mt_opt.unwrap();
        if self.edicts[mt].classname != "func_train" {
            gi_dprintf(&format!("trigger_elevator target {} is not a train", target));
            return;
        }

        self.edicts[self_ent].movetarget = mt as i32;
        self.edicts[self_ent].use_fn = Some(crate::dispatch::USE_FUNC_ELEVATOR);
        self.edicts[self_ent].svflags = SVF_NOCLIENT;
    }

    pub fn sp_trigger_elevator(&mut self, self_ent: usize) {
        self.edicts[self_ent].think_fn = Some(crate::dispatch::THINK_FUNC_TRIGGER_ELEVATOR_INIT);
        self.edicts[self_ent].nextthink = self.level.time + FRAMETIME;
    }

    // =========================================================
    // TIMER
    // =========================================================

    pub fn func_timer_think(&mut self, self_ent: usize) {
        let activator = self.edicts[self_ent].activator as usize;
        crate::g_utils::g_use_targets(self, self_ent, activator);
        let wait = self.edicts[self_ent].wait;
        let random = self.edicts[self_ent].random;
        self.edicts[self_ent].nextthink = self.level.time + wait + crandom() * random;
    }

    pub fn func_timer_use(&mut self, self_ent: usize, _other: usize, activator: usize) {
        self.edicts[self_ent].activator = activator as i32;

        // if on, turn it off
        if self.edicts[self_ent].nextthink != 0.0 {
            self.edicts[self_ent].nextthink = 0.0;
            return;
        }

        // turn it on
        if self.edicts[self_ent].delay != 0.0 {
            self.edicts[self_ent].nextthink = self.level.time + self.edicts[self_ent].delay;
        } else {
            self.func_timer_think(self_ent);
        }
    }

    pub fn sp_func_timer(&mut self, self_ent: usize) {
        if self.edicts[self_ent].wait == 0.0 {
            self.edicts[self_ent].wait = 1.0;
        }

        self.edicts[self_ent].use_fn = Some(crate::dispatch::USE_FUNC_TIMER);
        self.edicts[self_ent].think_fn = Some(crate::dispatch::THINK_FUNC_TIMER_THINK);

        if self.edicts[self_ent].random >= self.edicts[self_ent].wait {
            self.edicts[self_ent].random = self.edicts[self_ent].wait - FRAMETIME;
            let origin = self.edicts[self_ent].s.origin;
            gi_dprintf(&format!("func_timer at {} has random >= wait", vtos(&origin)));
        }

        if self.edicts[self_ent].spawnflags & 1 != 0 {
            let wait = self.edicts[self_ent].wait;
            let delay = self.edicts[self_ent].delay;
            let random = self.edicts[self_ent].random;
            let pausetime = self.st.pausetime;
            self.edicts[self_ent].nextthink = self.level.time + 1.0 + pausetime + delay + wait + crandom() * random;
            self.edicts[self_ent].activator = self_ent as i32;
        }

        self.edicts[self_ent].svflags = SVF_NOCLIENT;
    }

    // =========================================================
    // CONVEYOR
    // =========================================================

    pub fn func_conveyor_use(&mut self, self_ent: usize, _other: usize, _activator: usize) {
        if self.edicts[self_ent].spawnflags & 1 != 0 {
            self.edicts[self_ent].speed = 0.0;
            self.edicts[self_ent].spawnflags &= !1;
        } else {
            self.edicts[self_ent].speed = self.edicts[self_ent].count as f32;
            self.edicts[self_ent].spawnflags |= 1;
        }

        if self.edicts[self_ent].spawnflags & 2 == 0 {
            self.edicts[self_ent].count = 0;
        }
    }

    pub fn sp_func_conveyor(&mut self, self_ent: usize) {
        if self.edicts[self_ent].speed == 0.0 {
            self.edicts[self_ent].speed = 100.0;
        }

        if self.edicts[self_ent].spawnflags & 1 == 0 {
            self.edicts[self_ent].count = self.edicts[self_ent].speed as i32;
            self.edicts[self_ent].speed = 0.0;
        }

        self.edicts[self_ent].use_fn = Some(crate::dispatch::USE_FUNC_CONVEYOR);

        let model = self.edicts[self_ent].model.clone();
        gi_setmodel(self_ent as i32, &model);
        self.edicts[self_ent].solid = Solid::Bsp;
        gi_linkentity(self_ent as i32);
    }

    // =========================================================
    // SECRET DOOR
    // =========================================================

    pub fn door_secret_use(&mut self, self_ent: usize, _other: usize, _activator: usize) {
        // make sure we're not already moving
        if !vec3_compare(&self.edicts[self_ent].s.origin, &VEC3_ORIGIN) {
            return;
        }

        let pos1 = self.edicts[self_ent].pos1;
        self.move_calc(self_ent, pos1, EndFn::DoorSecretMove1 as usize);
        self.door_use_areaportals(self_ent, true);
    }

    pub fn door_secret_move1(&mut self, self_ent: usize) {
        self.edicts[self_ent].nextthink = self.level.time + 1.0;
        self.edicts[self_ent].think_fn = Some(crate::dispatch::THINK_FUNC_DOOR_SECRET_MOVE2);
    }

    pub fn door_secret_move2(&mut self, self_ent: usize) {
        let pos2 = self.edicts[self_ent].pos2;
        self.move_calc(self_ent, pos2, EndFn::DoorSecretMove3 as usize);
    }

    pub fn door_secret_move3(&mut self, self_ent: usize) {
        if self.edicts[self_ent].wait == -1.0 {
            return;
        }
        self.edicts[self_ent].nextthink = self.level.time + self.edicts[self_ent].wait;
        self.edicts[self_ent].think_fn = Some(crate::dispatch::THINK_FUNC_DOOR_SECRET_MOVE4);
    }

    pub fn door_secret_move4(&mut self, self_ent: usize) {
        let pos1 = self.edicts[self_ent].pos1;
        self.move_calc(self_ent, pos1, EndFn::DoorSecretMove5 as usize);
    }

    pub fn door_secret_move5(&mut self, self_ent: usize) {
        self.edicts[self_ent].nextthink = self.level.time + 1.0;
        self.edicts[self_ent].think_fn = Some(crate::dispatch::THINK_FUNC_DOOR_SECRET_MOVE6);
    }

    pub fn door_secret_move6(&mut self, self_ent: usize) {
        self.move_calc(self_ent, VEC3_ORIGIN, EndFn::DoorSecretDone as usize);
    }

    pub fn door_secret_done(&mut self, self_ent: usize) {
        if self.edicts[self_ent].targetname.is_empty() || (self.edicts[self_ent].spawnflags & SECRET_ALWAYS_SHOOT != 0) {
            self.edicts[self_ent].health = 0;
            self.edicts[self_ent].takedamage = DAMAGE_YES;
        }
        self.door_use_areaportals(self_ent, false);
    }

    pub fn door_secret_blocked(&mut self, self_ent: usize, other: usize) {
        if (self.edicts[other].svflags & SVF_MONSTER == 0) && self.edicts[other].client.is_none() {
            let origin = self.edicts[other].s.origin;
            crate::g_combat::ctx_t_damage(self, other, self_ent, self_ent, &VEC3_ORIGIN, &origin, &VEC3_ORIGIN, 100000, 1, DamageFlags::empty(), MOD_CRUSH);
            if self.edicts[other].inuse {
                crate::g_misc::become_explosion1(self, other);
            }
            return;
        }

        if self.level.time < self.edicts[self_ent].touch_debounce_time {
            return;
        }
        self.edicts[self_ent].touch_debounce_time = self.level.time + 0.5;

        let dmg = self.edicts[self_ent].dmg;
        let origin = self.edicts[other].s.origin;
        crate::g_combat::ctx_t_damage(self, other, self_ent, self_ent, &VEC3_ORIGIN, &origin, &VEC3_ORIGIN, dmg, 1, DamageFlags::empty(), MOD_CRUSH);
    }

    pub fn door_secret_die(&mut self, self_ent: usize, _inflictor: usize, attacker: usize, _damage: i32, _point: &[f32; 3]) {
        self.edicts[self_ent].takedamage = DAMAGE_NO;
        self.door_secret_use(self_ent, attacker, attacker);
    }

    pub fn sp_func_door_secret(&mut self, ent: usize) {
        self.edicts[ent].moveinfo.sound_start = gi_soundindex("doors/dr1_strt.wav");
        self.edicts[ent].moveinfo.sound_middle = gi_soundindex("doors/dr1_mid.wav");
        self.edicts[ent].moveinfo.sound_end = gi_soundindex("doors/dr1_end.wav");

        self.edicts[ent].movetype = MoveType::Push;
        self.edicts[ent].solid = Solid::Bsp;
        let model = self.edicts[ent].model.clone();
        gi_setmodel(ent as i32, &model);

        self.edicts[ent].blocked_fn = Some(crate::dispatch::BLOCKED_DOOR_SECRET);
        self.edicts[ent].use_fn = Some(crate::dispatch::USE_FUNC_DOOR_SECRET);

        if self.edicts[ent].targetname.is_empty() || (self.edicts[ent].spawnflags & SECRET_ALWAYS_SHOOT != 0) {
            self.edicts[ent].health = 0;
            self.edicts[ent].takedamage = DAMAGE_YES;
            self.edicts[ent].die_fn = Some(crate::dispatch::DIE_DOOR_SECRET);
        }

        if self.edicts[ent].dmg == 0 {
            self.edicts[ent].dmg = 2;
        }

        if self.edicts[ent].wait == 0.0 {
            self.edicts[ent].wait = 5.0;
        }

        self.edicts[ent].moveinfo.accel = 50.0;
        self.edicts[ent].moveinfo.decel = 50.0;
        self.edicts[ent].moveinfo.speed = 50.0;

        // calculate positions
        let angles = self.edicts[ent].s.angles;
        let mut forward = [0.0f32; 3];
        let mut right = [0.0f32; 3];
        let mut up = [0.0f32; 3];
        angle_vectors(&angles, Some(&mut forward), Some(&mut right), Some(&mut up));
        vec3_clear(&mut self.edicts[ent].s.angles);

        let side = 1.0 - (self.edicts[ent].spawnflags & SECRET_1ST_LEFT) as f32;
        let size = self.edicts[ent].size;

        let width;
        if self.edicts[ent].spawnflags & SECRET_1ST_DOWN != 0 {
            width = vec3_dot(&up, &size).abs();
        } else {
            width = vec3_dot(&right, &size).abs();
        }
        let length = vec3_dot(&forward, &size).abs();

        let origin = self.edicts[ent].s.origin;
        if self.edicts[ent].spawnflags & SECRET_1ST_DOWN != 0 {
            vec3_ma(&origin, -width, &up, &mut self.edicts[ent].pos1);
        } else {
            vec3_ma(&origin, side * width, &right, &mut self.edicts[ent].pos1);
        }
        let pos1 = self.edicts[ent].pos1;
        vec3_ma(&pos1, length, &forward, &mut self.edicts[ent].pos2);

        if self.edicts[ent].health != 0 {
            self.edicts[ent].takedamage = DAMAGE_YES;
            self.edicts[ent].die_fn = Some(crate::dispatch::DIE_DOOR_KILLED);
            self.edicts[ent].max_health = self.edicts[ent].health;
        } else if !self.edicts[ent].targetname.is_empty() && !self.edicts[ent].message.is_empty() {
            gi_soundindex("misc/talk.wav");
            self.edicts[ent].touch_fn = Some(crate::dispatch::TOUCH_DOOR);
        }

        self.edicts[ent].classname = "func_door".to_string();

        gi_linkentity(ent as i32);
    }

    // =========================================================
    // KILLBOX
    // =========================================================

    pub fn use_killbox(&mut self, self_ent: usize, _other: usize, _activator: usize) {
        crate::g_utils::killbox(self, self_ent);
    }

    pub fn sp_func_killbox(&mut self, ent: usize) {
        let model = self.edicts[ent].model.clone();
        gi_setmodel(ent as i32, &model);
        self.edicts[ent].use_fn = Some(crate::dispatch::USE_FUNC_KILLBOX);
        self.edicts[ent].svflags = SVF_NOCLIENT;
    }

    // =========================================================
    // Dispatch tables for function callbacks
    // =========================================================

    /// Dispatch an endfunc callback by its ID.
    fn dispatch_endfunc(&mut self, id: usize, ent: usize) {
        match EndFn::from_usize(id) {
            Some(EndFn::PlatHitTop) => self.plat_hit_top(ent),
            Some(EndFn::PlatHitBottom) => self.plat_hit_bottom(ent),
            Some(EndFn::ButtonDone) => self.button_done(ent),
            Some(EndFn::ButtonWait) => self.button_wait(ent),
            Some(EndFn::DoorHitTop) => self.door_hit_top(ent),
            Some(EndFn::DoorHitBottom) => self.door_hit_bottom(ent),
            Some(EndFn::TrainWait) => self.train_wait(ent),
            Some(EndFn::DoorSecretMove1) => self.door_secret_move1(ent),
            Some(EndFn::DoorSecretMove3) => self.door_secret_move3(ent),
            Some(EndFn::DoorSecretMove5) => self.door_secret_move5(ent),
            Some(EndFn::DoorSecretDone) => self.door_secret_done(ent),
            None => panic!("Unknown endfunc id: {}", id),
        }
    }
}

// =========================================================
// Callback enums — used as dispatch table indices
// =========================================================

/// End function IDs (moveinfo.endfunc callbacks).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum EndFn {
    PlatHitTop = 100,
    PlatHitBottom,
    ButtonDone,
    ButtonWait,
    DoorHitTop,
    DoorHitBottom,
    TrainWait,
    DoorSecretMove1,
    DoorSecretMove3,
    DoorSecretMove5,
    DoorSecretDone,
}

impl EndFn {
    fn from_usize(v: usize) -> Option<EndFn> {
        match v {
            100 => Some(EndFn::PlatHitTop),
            101 => Some(EndFn::PlatHitBottom),
            102 => Some(EndFn::ButtonDone),
            103 => Some(EndFn::ButtonWait),
            104 => Some(EndFn::DoorHitTop),
            105 => Some(EndFn::DoorHitBottom),
            106 => Some(EndFn::TrainWait),
            107 => Some(EndFn::DoorSecretMove1),
            108 => Some(EndFn::DoorSecretMove3),
            109 => Some(EndFn::DoorSecretMove5),
            110 => Some(EndFn::DoorSecretDone),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::g_local::*;
    use crate::game::Solid;

    fn init_test_gi() {
        crate::game_import::set_gi(Box::new(crate::game_import::StubGameImport));
    }

    /// Helper: create a GameContext with N edicts (index 0 = world).
    fn make_ctx(num_edicts: usize) -> GameContext {
        init_test_gi();
        let mut ctx = GameContext::default();
        for _ in 0..num_edicts {
            ctx.edicts.push(Edict::default());
        }
        ctx.num_edicts = num_edicts as i32;
        ctx.max_edicts = (num_edicts + 10) as i32;
        ctx.game.maxclients = 1;
        ctx.maxclients = 1.0;
        ctx
    }

    // ============================================================
    // acceleration_distance tests
    // ============================================================

    #[test]
    fn test_acceleration_distance_basic() {
        // acceleration_distance(100, 10) = 100 * ((100/10) + 1) / 2 = 100 * 11 / 2 = 550
        let result = acceleration_distance(100.0, 10.0);
        assert!((result - 550.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_acceleration_distance_equal() {
        // acceleration_distance(50, 50) = 50 * ((50/50) + 1) / 2 = 50 * 2 / 2 = 50
        let result = acceleration_distance(50.0, 50.0);
        assert!((result - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_acceleration_distance_high_rate() {
        // acceleration_distance(10, 100) = 10 * ((10/100) + 1) / 2 = 10 * 1.1 / 2 = 5.5
        let result = acceleration_distance(10.0, 100.0);
        assert!((result - 5.5).abs() < f32::EPSILON);
    }

    // ============================================================
    // EndFn enum tests
    // ============================================================

    #[test]
    fn test_endfn_from_usize_all_values() {
        assert_eq!(EndFn::from_usize(100), Some(EndFn::PlatHitTop));
        assert_eq!(EndFn::from_usize(101), Some(EndFn::PlatHitBottom));
        assert_eq!(EndFn::from_usize(102), Some(EndFn::ButtonDone));
        assert_eq!(EndFn::from_usize(103), Some(EndFn::ButtonWait));
        assert_eq!(EndFn::from_usize(104), Some(EndFn::DoorHitTop));
        assert_eq!(EndFn::from_usize(105), Some(EndFn::DoorHitBottom));
        assert_eq!(EndFn::from_usize(106), Some(EndFn::TrainWait));
        assert_eq!(EndFn::from_usize(107), Some(EndFn::DoorSecretMove1));
        assert_eq!(EndFn::from_usize(108), Some(EndFn::DoorSecretMove3));
        assert_eq!(EndFn::from_usize(109), Some(EndFn::DoorSecretMove5));
        assert_eq!(EndFn::from_usize(110), Some(EndFn::DoorSecretDone));
    }

    #[test]
    fn test_endfn_from_usize_invalid() {
        assert_eq!(EndFn::from_usize(0), None);
        assert_eq!(EndFn::from_usize(99), None);
        assert_eq!(EndFn::from_usize(111), None);
        assert_eq!(EndFn::from_usize(usize::MAX), None);
    }

    #[test]
    fn test_endfn_repr_values() {
        assert_eq!(EndFn::PlatHitTop as usize, 100);
        assert_eq!(EndFn::PlatHitBottom as usize, 101);
        assert_eq!(EndFn::ButtonDone as usize, 102);
        assert_eq!(EndFn::TrainWait as usize, 106);
        assert_eq!(EndFn::DoorSecretDone as usize, 110);
    }

    // ============================================================
    // State constants tests
    // ============================================================

    #[test]
    fn test_state_constants() {
        assert_eq!(STATE_TOP, 0);
        assert_eq!(STATE_BOTTOM, 1);
        assert_eq!(STATE_UP, 2);
        assert_eq!(STATE_DOWN, 3);
    }

    // ============================================================
    // move_done / move_final / move_begin / move_calc tests
    // ============================================================

    #[test]
    fn test_move_done_clears_velocity() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].velocity = [100.0, 200.0, 300.0];
        ctx.edicts[1].moveinfo.endfunc = None;

        ctx.move_done(1);

        assert_eq!(ctx.edicts[1].velocity, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_move_final_zero_remaining() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].moveinfo.remaining_distance = 0.0;
        ctx.edicts[1].moveinfo.endfunc = None;
        ctx.edicts[1].velocity = [10.0, 20.0, 30.0];

        ctx.move_final(1);

        // Should call move_done which clears velocity
        assert_eq!(ctx.edicts[1].velocity, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_move_final_with_remaining() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].moveinfo.remaining_distance = 5.0;
        ctx.edicts[1].moveinfo.dir = [1.0, 0.0, 0.0];
        ctx.level.time = 10.0;

        ctx.move_final(1);

        // velocity = dir * (remaining / FRAMETIME) = [1,0,0] * (5/0.1) = [50,0,0]
        assert!((ctx.edicts[1].velocity[0] - 50.0).abs() < 0.01);
        assert_eq!(ctx.edicts[1].velocity[1], 0.0);
        assert_eq!(ctx.edicts[1].velocity[2], 0.0);
        // nextthink should be set
        assert!((ctx.edicts[1].nextthink - (10.0 + FRAMETIME)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_move_begin_completes_in_one_frame() {
        let mut ctx = make_ctx(3);
        // If speed * FRAMETIME >= remaining_distance, should call move_final
        ctx.edicts[1].moveinfo.speed = 100.0;
        ctx.edicts[1].moveinfo.remaining_distance = 5.0; // 100*0.1=10 >= 5
        ctx.edicts[1].moveinfo.dir = [1.0, 0.0, 0.0];
        ctx.edicts[1].moveinfo.endfunc = None;
        ctx.level.time = 10.0;

        ctx.move_begin(1);

        // Should have called move_final which sets velocity
        // remaining=5, dir=[1,0,0], vel = 5/0.1 = 50
        assert!((ctx.edicts[1].velocity[0] - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_move_begin_multi_frame() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].moveinfo.speed = 100.0;
        ctx.edicts[1].moveinfo.remaining_distance = 50.0; // 100*0.1=10 < 50
        ctx.edicts[1].moveinfo.dir = [0.0, 1.0, 0.0];
        ctx.level.time = 5.0;

        ctx.move_begin(1);

        // velocity = dir * speed = [0, 100, 0]
        assert!((ctx.edicts[1].velocity[1] - 100.0).abs() < 0.01);
        // frames = floor(50 / 100 / 0.1) = floor(5) = 5
        // remaining -= 5 * 100 * 0.1 = 50 => remaining = 0
        // nextthink = 5.0 + 5 * 0.1 = 5.5
        assert!((ctx.edicts[1].nextthink - 5.5).abs() < 0.01);
    }

    #[test]
    fn test_move_calc_sets_direction_and_distance() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].s.origin = [0.0, 0.0, 0.0];
        ctx.edicts[1].moveinfo.speed = 100.0;
        ctx.edicts[1].moveinfo.accel = 100.0;
        ctx.edicts[1].moveinfo.decel = 100.0;
        ctx.level.time = 1.0;
        ctx.level.current_entity = -1; // not the current entity

        let dest = [100.0, 0.0, 0.0];
        ctx.move_calc(1, dest, EndFn::PlatHitTop as usize);

        // dir should be normalized direction to dest
        assert!((ctx.edicts[1].moveinfo.dir[0] - 1.0).abs() < 0.01);
        assert!((ctx.edicts[1].moveinfo.dir[1]).abs() < 0.01);
        // remaining_distance should be 100
        assert!((ctx.edicts[1].moveinfo.remaining_distance - 100.0).abs() < 0.01);
        // endfunc should be set
        assert_eq!(ctx.edicts[1].moveinfo.endfunc, Some(EndFn::PlatHitTop as usize));
    }

    // ============================================================
    // angle_move tests
    // ============================================================

    #[test]
    fn test_angle_move_done_clears_avelocity() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].avelocity = [10.0, 20.0, 30.0];
        ctx.edicts[1].moveinfo.endfunc = None;

        ctx.angle_move_done(1);

        assert_eq!(ctx.edicts[1].avelocity, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_angle_move_final_no_movement_needed() {
        let mut ctx = make_ctx(3);
        // STATE_UP, and angles already match end_angles
        ctx.edicts[1].moveinfo.state = STATE_UP;
        ctx.edicts[1].moveinfo.end_angles = [90.0, 0.0, 0.0];
        ctx.edicts[1].s.angles = [90.0, 0.0, 0.0];
        ctx.edicts[1].moveinfo.endfunc = None;

        ctx.angle_move_final(1);

        // Since mv == origin, should call angle_move_done (clears avelocity)
        assert_eq!(ctx.edicts[1].avelocity, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_angle_move_final_with_remaining() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].moveinfo.state = STATE_UP;
        ctx.edicts[1].moveinfo.end_angles = [90.0, 0.0, 0.0];
        ctx.edicts[1].s.angles = [80.0, 0.0, 0.0];
        ctx.level.time = 5.0;

        ctx.angle_move_final(1);

        // mv = [10, 0, 0], avelocity = mv / FRAMETIME = [100, 0, 0]
        assert!((ctx.edicts[1].avelocity[0] - 100.0).abs() < 0.01);
        assert!((ctx.edicts[1].nextthink - 5.1).abs() < 0.01);
    }

    // ============================================================
    // plat_calc_accelerated_move tests
    // ============================================================

    #[test]
    fn test_plat_calc_accelerated_move_short_distance() {
        let mut ctx = make_ctx(3);
        let mi = &mut ctx.edicts[1].moveinfo;
        mi.speed = 100.0;
        mi.accel = 200.0; // remaining < accel
        mi.decel = 50.0;
        mi.remaining_distance = 150.0; // < accel(200)

        ctx.plat_calc_accelerated_move(1);

        // When remaining < accel, current_speed = remaining_distance
        assert!((ctx.edicts[1].moveinfo.current_speed - 150.0).abs() < f32::EPSILON);
        assert!((ctx.edicts[1].moveinfo.move_speed - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_plat_calc_accelerated_move_normal() {
        let mut ctx = make_ctx(3);
        let mi = &mut ctx.edicts[1].moveinfo;
        mi.speed = 50.0;
        mi.accel = 50.0;
        mi.decel = 50.0;
        mi.remaining_distance = 500.0;

        ctx.plat_calc_accelerated_move(1);

        // move_speed should be set to speed initially
        assert!((ctx.edicts[1].moveinfo.move_speed - 50.0).abs() < f32::EPSILON);
        // decel_distance should be set
        assert!(ctx.edicts[1].moveinfo.decel_distance > 0.0);
    }

    // ============================================================
    // plat_accelerate tests
    // ============================================================

    #[test]
    fn test_plat_accelerate_decelerating() {
        let mut ctx = make_ctx(3);
        let mi = &mut ctx.edicts[1].moveinfo;
        mi.remaining_distance = 10.0;
        mi.decel_distance = 20.0; // remaining <= decel_distance
        mi.current_speed = 30.0;
        mi.decel = 5.0;
        mi.next_speed = 0.0;

        ctx.plat_accelerate(1);

        // remaining(10) < decel_distance(20), next_speed==0, current_speed(30) > decel(5)
        // current_speed -= decel => 25
        assert!((ctx.edicts[1].moveinfo.current_speed - 25.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_plat_accelerate_with_next_speed() {
        let mut ctx = make_ctx(3);
        let mi = &mut ctx.edicts[1].moveinfo;
        mi.remaining_distance = 5.0;
        mi.decel_distance = 20.0;
        mi.current_speed = 10.0;
        mi.next_speed = 8.0;
        mi.decel = 5.0;

        ctx.plat_accelerate(1);

        // remaining < decel_distance and next_speed != 0
        // current_speed = next_speed, next_speed = 0
        assert!((ctx.edicts[1].moveinfo.current_speed - 8.0).abs() < f32::EPSILON);
        assert!((ctx.edicts[1].moveinfo.next_speed).abs() < f32::EPSILON);
    }

    #[test]
    fn test_plat_accelerate_accelerating() {
        let mut ctx = make_ctx(3);
        let mi = &mut ctx.edicts[1].moveinfo;
        mi.remaining_distance = 100.0;
        mi.decel_distance = 10.0;
        mi.current_speed = 5.0; // < speed
        mi.speed = 50.0;
        mi.accel = 10.0;
        mi.move_speed = 50.0;

        ctx.plat_accelerate(1);

        // current_speed += accel => 5 + 10 = 15
        // remaining - current_speed (100-15=85) >= decel_distance (10) => just return
        assert!((ctx.edicts[1].moveinfo.current_speed - 15.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_plat_accelerate_caps_at_speed() {
        let mut ctx = make_ctx(3);
        let mi = &mut ctx.edicts[1].moveinfo;
        mi.remaining_distance = 100.0;
        mi.decel_distance = 10.0;
        mi.current_speed = 48.0; // < speed(50), but +accel(10) = 58 > 50
        mi.speed = 50.0;
        mi.accel = 10.0;
        mi.move_speed = 50.0;

        ctx.plat_accelerate(1);

        // current_speed = min(48+10, 50) = 50
        assert!((ctx.edicts[1].moveinfo.current_speed - 50.0).abs() < f32::EPSILON);
    }

    // ============================================================
    // Platform state machine tests
    // ============================================================

    #[test]
    fn test_plat_hit_top_sets_state() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].moveinfo.state = STATE_UP;
        ctx.edicts[1].moveinfo.sound_end = 0;
        ctx.level.time = 10.0;

        ctx.plat_hit_top(1);

        assert_eq!(ctx.edicts[1].moveinfo.state, STATE_TOP);
        // nextthink should be set for go_down after 3 seconds
        assert!((ctx.edicts[1].nextthink - 13.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_plat_hit_bottom_sets_state() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].moveinfo.state = STATE_DOWN;
        ctx.edicts[1].moveinfo.sound_end = 0;

        ctx.plat_hit_bottom(1);

        assert_eq!(ctx.edicts[1].moveinfo.state, STATE_BOTTOM);
    }

    #[test]
    fn test_plat_go_down_sets_state() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].moveinfo.state = STATE_TOP;
        ctx.edicts[1].moveinfo.sound_start = 0;
        ctx.edicts[1].moveinfo.end_origin = [0.0, 0.0, -100.0];
        ctx.edicts[1].s.origin = [0.0, 0.0, 0.0];
        ctx.edicts[1].moveinfo.speed = 50.0;
        ctx.edicts[1].moveinfo.accel = 50.0;
        ctx.edicts[1].moveinfo.decel = 50.0;
        ctx.level.time = 1.0;
        ctx.level.current_entity = -1;

        ctx.plat_go_down(1);

        assert_eq!(ctx.edicts[1].moveinfo.state, STATE_DOWN);
    }

    #[test]
    fn test_plat_go_up_sets_state() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].moveinfo.state = STATE_BOTTOM;
        ctx.edicts[1].moveinfo.sound_start = 0;
        ctx.edicts[1].moveinfo.start_origin = [0.0, 0.0, 100.0];
        ctx.edicts[1].s.origin = [0.0, 0.0, 0.0];
        ctx.edicts[1].moveinfo.speed = 50.0;
        ctx.edicts[1].moveinfo.accel = 50.0;
        ctx.edicts[1].moveinfo.decel = 50.0;
        ctx.level.time = 1.0;
        ctx.level.current_entity = -1;

        ctx.plat_go_up(1);

        assert_eq!(ctx.edicts[1].moveinfo.state, STATE_UP);
    }

    // ============================================================
    // Button state machine tests
    // ============================================================

    #[test]
    fn test_button_done_sets_bottom() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].moveinfo.state = STATE_DOWN;
        ctx.edicts[1].s.effects = EF_ANIM23;

        ctx.button_done(1);

        assert_eq!(ctx.edicts[1].moveinfo.state, STATE_BOTTOM);
        assert_eq!(ctx.edicts[1].s.effects & EF_ANIM23, 0);
        assert_ne!(ctx.edicts[1].s.effects & EF_ANIM01, 0);
    }

    #[test]
    fn test_button_wait_sets_top() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].moveinfo.state = STATE_UP;
        ctx.edicts[1].moveinfo.wait = 5.0;
        ctx.edicts[1].s.effects = EF_ANIM01;
        ctx.edicts[1].activator = 0;
        ctx.level.time = 10.0;
        ctx.num_edicts = ctx.edicts.len() as i32;
        ctx.max_edicts = (ctx.edicts.len() + 10) as i32;

        ctx.button_wait(1);

        assert_eq!(ctx.edicts[1].moveinfo.state, STATE_TOP);
        assert_eq!(ctx.edicts[1].s.effects & EF_ANIM01, 0);
        assert_ne!(ctx.edicts[1].s.effects & EF_ANIM23, 0);
        assert_eq!(ctx.edicts[1].s.frame, 1);
        // nextthink = time + wait = 10 + 5 = 15
        assert!((ctx.edicts[1].nextthink - 15.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_button_fire_already_up() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].moveinfo.state = STATE_UP;
        let old_state = ctx.edicts[1].moveinfo.state;

        ctx.button_fire(1);

        // Should return early without changes
        assert_eq!(ctx.edicts[1].moveinfo.state, old_state);
    }

    #[test]
    fn test_button_fire_already_top() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].moveinfo.state = STATE_TOP;

        ctx.button_fire(1);

        // Should return early
        assert_eq!(ctx.edicts[1].moveinfo.state, STATE_TOP);
    }

    #[test]
    fn test_button_fire_from_bottom() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].moveinfo.state = STATE_BOTTOM;
        ctx.edicts[1].moveinfo.sound_start = 0;
        ctx.edicts[1].moveinfo.end_origin = [0.0, 50.0, 0.0];
        ctx.edicts[1].s.origin = [0.0, 0.0, 0.0];
        ctx.edicts[1].moveinfo.speed = 40.0;
        ctx.edicts[1].moveinfo.accel = 40.0;
        ctx.edicts[1].moveinfo.decel = 40.0;
        ctx.level.time = 0.0;
        ctx.level.current_entity = -1;

        ctx.button_fire(1);

        assert_eq!(ctx.edicts[1].moveinfo.state, STATE_UP);
    }

    #[test]
    fn test_button_use_sets_activator() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].moveinfo.state = STATE_UP; // already up, fire will bail

        ctx.button_use(1, 0, 2);

        assert_eq!(ctx.edicts[1].activator, 2);
    }

    #[test]
    fn test_button_touch_requires_client() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].moveinfo.state = STATE_BOTTOM;
        ctx.edicts[2].client = None; // no client
        ctx.edicts[2].health = 100;
        let original_state = ctx.edicts[1].moveinfo.state;

        ctx.button_touch(1, 2, None, None);

        // Should return early since other has no client
        assert_eq!(ctx.edicts[1].moveinfo.state, original_state);
    }

    #[test]
    fn test_button_touch_requires_positive_health() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].moveinfo.state = STATE_BOTTOM;
        ctx.edicts[2].client = Some(0);
        ctx.edicts[2].health = 0; // dead

        ctx.button_touch(1, 2, None, None);

        // Should return early
        assert_eq!(ctx.edicts[1].moveinfo.state, STATE_BOTTOM);
    }

    // ============================================================
    // Door state machine tests
    // ============================================================

    #[test]
    fn test_door_hit_top_state() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].moveinfo.state = STATE_UP;
        ctx.edicts[1].moveinfo.sound_end = 0;
        ctx.edicts[1].moveinfo.wait = 5.0;
        ctx.edicts[1].spawnflags = 0; // not DOOR_TOGGLE
        ctx.level.time = 10.0;

        ctx.door_hit_top(1);

        assert_eq!(ctx.edicts[1].moveinfo.state, STATE_TOP);
        // With wait >= 0, nextthink should be set
        assert!((ctx.edicts[1].nextthink - 15.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_door_hit_top_toggle() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].moveinfo.state = STATE_UP;
        ctx.edicts[1].moveinfo.sound_end = 0;
        ctx.edicts[1].spawnflags = DOOR_TOGGLE;
        ctx.level.time = 10.0;

        ctx.door_hit_top(1);

        assert_eq!(ctx.edicts[1].moveinfo.state, STATE_TOP);
        // Toggle doors don't auto-close
        assert_eq!(ctx.edicts[1].nextthink, 0.0);
    }

    #[test]
    fn test_door_hit_bottom_state() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].moveinfo.state = STATE_DOWN;
        ctx.edicts[1].moveinfo.sound_end = 0;

        ctx.door_hit_bottom(1);

        assert_eq!(ctx.edicts[1].moveinfo.state, STATE_BOTTOM);
    }

    #[test]
    fn test_door_go_up_already_up() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].moveinfo.state = STATE_UP;
        ctx.edicts[1].classname = "func_door".to_string();

        ctx.door_go_up(1, 0);

        // Should return without changing state
        assert_eq!(ctx.edicts[1].moveinfo.state, STATE_UP);
    }

    #[test]
    fn test_door_go_up_from_top_resets_timer() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].moveinfo.state = STATE_TOP;
        ctx.edicts[1].moveinfo.wait = 3.0;
        ctx.edicts[1].classname = "func_door".to_string();
        ctx.level.time = 10.0;

        ctx.door_go_up(1, 0);

        // Should reset the nextthink timer
        assert!((ctx.edicts[1].nextthink - 13.0).abs() < f32::EPSILON);
        // State should remain TOP
        assert_eq!(ctx.edicts[1].moveinfo.state, STATE_TOP);
    }

    #[test]
    fn test_door_go_down_sets_state() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].moveinfo.state = STATE_TOP;
        ctx.edicts[1].moveinfo.sound_start = 0;
        ctx.edicts[1].classname = "func_door".to_string();
        ctx.edicts[1].moveinfo.start_origin = [0.0, 0.0, 0.0];
        ctx.edicts[1].s.origin = [0.0, 0.0, 100.0];
        ctx.edicts[1].moveinfo.speed = 100.0;
        ctx.edicts[1].moveinfo.accel = 100.0;
        ctx.edicts[1].moveinfo.decel = 100.0;
        ctx.level.time = 5.0;
        ctx.level.current_entity = -1;

        ctx.door_go_down(1);

        assert_eq!(ctx.edicts[1].moveinfo.state, STATE_DOWN);
    }

    #[test]
    fn test_door_use_toggle_goes_down() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].spawnflags = DOOR_TOGGLE;
        ctx.edicts[1].moveinfo.state = STATE_TOP;
        ctx.edicts[1].moveinfo.sound_start = 0;
        ctx.edicts[1].classname = "func_door".to_string();
        ctx.edicts[1].moveinfo.start_origin = [0.0, 0.0, 0.0];
        ctx.edicts[1].s.origin = [0.0, 0.0, 100.0];
        ctx.edicts[1].moveinfo.speed = 100.0;
        ctx.edicts[1].moveinfo.accel = 100.0;
        ctx.edicts[1].moveinfo.decel = 100.0;
        ctx.edicts[1].teamchain = -1; // no team chain
        ctx.level.time = 5.0;
        ctx.level.current_entity = -1;

        ctx.door_use(1, 0, 0);

        assert_eq!(ctx.edicts[1].moveinfo.state, STATE_DOWN);
    }

    #[test]
    fn test_door_use_teamslave_returns_early() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].flags = FL_TEAMSLAVE;

        ctx.door_use(1, 0, 0);

        // Should return without doing anything
        assert_eq!(ctx.edicts[1].moveinfo.state, 0);
    }

    // ============================================================
    // think_calc_move_speed tests
    // ============================================================

    #[test]
    fn test_calc_move_speed_single_entity() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].teamchain = -1; // no team
        ctx.edicts[1].moveinfo.distance = 100.0;
        ctx.edicts[1].moveinfo.speed = 50.0;
        ctx.edicts[1].moveinfo.accel = 50.0;
        ctx.edicts[1].moveinfo.decel = 50.0;

        ctx.think_calc_move_speed(1);

        // With single entity, min distance = 100, time = 100/50 = 2
        // newspeed = 100/2 = 50 (unchanged)
        assert!((ctx.edicts[1].moveinfo.speed - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_calc_move_speed_team() {
        let mut ctx = make_ctx(4);
        // Entity 1 leads team, entity 2 is chain member
        ctx.edicts[1].teamchain = 2;
        ctx.edicts[1].moveinfo.distance = 100.0;
        ctx.edicts[1].moveinfo.speed = 100.0;
        ctx.edicts[1].moveinfo.accel = 100.0;
        ctx.edicts[1].moveinfo.decel = 100.0;

        ctx.edicts[2].teamchain = -1;
        ctx.edicts[2].moveinfo.distance = 50.0; // shorter distance
        ctx.edicts[2].moveinfo.speed = 100.0;
        ctx.edicts[2].moveinfo.accel = 100.0;
        ctx.edicts[2].moveinfo.decel = 100.0;

        ctx.think_calc_move_speed(1);

        // min distance = 50, time = 50/100 = 0.5
        // entity 1: newspeed = 100/0.5 = 200
        // entity 2: newspeed = 50/0.5 = 100 (same)
        assert!((ctx.edicts[1].moveinfo.speed - 200.0).abs() < 0.01);
        assert!((ctx.edicts[2].moveinfo.speed - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_calc_move_speed_teamslave_returns_early() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].flags = FL_TEAMSLAVE;
        ctx.edicts[1].moveinfo.speed = 100.0;

        ctx.think_calc_move_speed(1);

        // Should return early without changing anything
        assert!((ctx.edicts[1].moveinfo.speed - 100.0).abs() < f32::EPSILON);
    }

    // ============================================================
    // Rotating entity tests
    // ============================================================

    #[test]
    fn test_rotating_use_toggle_on() {
        let mut ctx = make_ctx(3);
        // Currently off (avelocity = 0)
        ctx.edicts[1].avelocity = [0.0, 0.0, 0.0];
        ctx.edicts[1].movedir = [0.0, 1.0, 0.0];
        ctx.edicts[1].speed = 100.0;
        ctx.edicts[1].moveinfo.sound_middle = 5;

        ctx.rotating_use(1, 0, 0);

        // Should turn on: avelocity = movedir * speed
        assert!((ctx.edicts[1].avelocity[1] - 100.0).abs() < 0.01);
        assert_eq!(ctx.edicts[1].s.sound, 5);
    }

    #[test]
    fn test_rotating_use_toggle_off() {
        let mut ctx = make_ctx(3);
        // Currently on
        ctx.edicts[1].avelocity = [0.0, 100.0, 0.0];
        ctx.edicts[1].movedir = [0.0, 1.0, 0.0];
        ctx.edicts[1].speed = 100.0;

        ctx.rotating_use(1, 0, 0);

        // Should turn off
        assert_eq!(ctx.edicts[1].avelocity, [0.0, 0.0, 0.0]);
        assert_eq!(ctx.edicts[1].s.sound, 0);
        assert!(ctx.edicts[1].touch_fn.is_none());
    }

    // ============================================================
    // Conveyor tests
    // ============================================================

    #[test]
    fn test_conveyor_toggle_off() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].spawnflags = 1; // currently on
        ctx.edicts[1].speed = 100.0;
        ctx.edicts[1].count = 100;

        ctx.func_conveyor_use(1, 0, 0);

        assert_eq!(ctx.edicts[1].speed, 0.0);
        assert_eq!(ctx.edicts[1].spawnflags & 1, 0);
    }

    #[test]
    fn test_conveyor_toggle_on() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].spawnflags = 0; // currently off
        ctx.edicts[1].speed = 0.0;
        ctx.edicts[1].count = 100;

        ctx.func_conveyor_use(1, 0, 0);

        assert_eq!(ctx.edicts[1].speed, 100.0);
        assert_ne!(ctx.edicts[1].spawnflags & 1, 0);
    }

    // ============================================================
    // Timer tests
    // ============================================================

    #[test]
    fn test_timer_use_turn_off() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].nextthink = 10.0; // currently on

        ctx.func_timer_use(1, 0, 0);

        assert_eq!(ctx.edicts[1].nextthink, 0.0); // turned off
    }

    #[test]
    fn test_timer_use_turn_on_with_delay() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].nextthink = 0.0; // currently off
        ctx.edicts[1].delay = 2.0;
        ctx.level.time = 5.0;

        ctx.func_timer_use(1, 0, 0);

        // nextthink = time + delay = 5.0 + 2.0 = 7.0
        assert!((ctx.edicts[1].nextthink - 7.0).abs() < f32::EPSILON);
    }

    // ============================================================
    // Platform spawn defaults tests
    // ============================================================

    #[test]
    fn test_sp_func_plat_default_speed() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].speed = 0.0;
        ctx.edicts[1].accel = 0.0;
        ctx.edicts[1].decel = 0.0;
        ctx.edicts[1].dmg = 0;
        ctx.edicts[1].s.origin = [0.0, 0.0, 100.0];
        ctx.edicts[1].mins = [-32.0, -32.0, 0.0];
        ctx.edicts[1].maxs = [32.0, 32.0, 64.0];
        ctx.edicts[1].size = [64.0, 64.0, 64.0];
        ctx.st.lip = 0;
        ctx.st.height = 0;
        ctx.level.time = 0.0;

        ctx.sp_func_plat(1);

        // Default speed = 20, accel = 5, decel = 5
        assert!((ctx.edicts[1].speed - 20.0).abs() < f32::EPSILON);
        assert!((ctx.edicts[1].accel - 5.0).abs() < f32::EPSILON);
        assert!((ctx.edicts[1].decel - 5.0).abs() < f32::EPSILON);
        assert_eq!(ctx.edicts[1].dmg, 2);
    }

    #[test]
    fn test_sp_func_plat_custom_speed() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].speed = 100.0;
        ctx.edicts[1].accel = 50.0;
        ctx.edicts[1].decel = 30.0;
        ctx.edicts[1].s.origin = [0.0, 0.0, 100.0];
        ctx.edicts[1].mins = [-32.0, -32.0, 0.0];
        ctx.edicts[1].maxs = [32.0, 32.0, 64.0];
        ctx.edicts[1].size = [64.0, 64.0, 64.0];
        ctx.st.lip = 0;
        ctx.st.height = 0;
        ctx.level.time = 0.0;

        ctx.sp_func_plat(1);

        // Custom speed *= 0.1
        assert!((ctx.edicts[1].speed - 10.0).abs() < f32::EPSILON);
        assert!((ctx.edicts[1].accel - 5.0).abs() < f32::EPSILON);
        assert!((ctx.edicts[1].decel - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_sp_func_plat_pos2_with_height() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].speed = 0.0;
        ctx.edicts[1].s.origin = [0.0, 0.0, 200.0];
        ctx.edicts[1].mins = [-32.0, -32.0, 0.0];
        ctx.edicts[1].maxs = [32.0, 32.0, 64.0];
        ctx.edicts[1].size = [64.0, 64.0, 64.0];
        ctx.st.lip = 0;
        ctx.st.height = 80;
        ctx.level.time = 0.0;

        ctx.sp_func_plat(1);

        // pos1 = origin = [0, 0, 200]
        assert_eq!(ctx.edicts[1].pos1, [0.0, 0.0, 200.0]);
        // pos2 = origin with z -= height = [0, 0, 120]
        assert!((ctx.edicts[1].pos2[2] - 120.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_sp_func_plat_pos2_without_height() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].speed = 0.0;
        ctx.edicts[1].s.origin = [0.0, 0.0, 200.0];
        ctx.edicts[1].mins = [-32.0, -32.0, 0.0];
        ctx.edicts[1].maxs = [32.0, 32.0, 64.0];
        ctx.edicts[1].size = [64.0, 64.0, 64.0];
        ctx.st.lip = 8;
        ctx.st.height = 0;
        ctx.level.time = 0.0;

        ctx.sp_func_plat(1);

        // pos2[2] = origin[2] - (maxs[2] - mins[2]) + lip = 200 - 64 + 8 = 144
        assert!((ctx.edicts[1].pos2[2] - 144.0).abs() < f32::EPSILON);
    }

    // ============================================================
    // Door spawn defaults tests
    // ============================================================

    #[test]
    fn test_sp_func_door_defaults() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].speed = 0.0;
        ctx.edicts[1].wait = 0.0;
        ctx.edicts[1].dmg = 0;
        ctx.edicts[1].s.angles = [0.0, 90.0, 0.0]; // faces east
        ctx.edicts[1].s.origin = [100.0, 0.0, 0.0];
        ctx.edicts[1].size = [64.0, 32.0, 128.0];
        ctx.st.lip = 0;
        ctx.level.time = 0.0;

        ctx.sp_func_door(1);

        // Default values
        assert!((ctx.edicts[1].speed - 100.0).abs() < f32::EPSILON);
        assert!((ctx.edicts[1].wait - 3.0).abs() < f32::EPSILON);
        assert_eq!(ctx.edicts[1].dmg, 2);
    }

    #[test]
    fn test_sp_func_door_deathmatch_speed_boost() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].speed = 0.0;
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0];
        ctx.edicts[1].s.origin = [0.0, 0.0, 0.0];
        ctx.edicts[1].size = [64.0, 64.0, 128.0];
        ctx.st.lip = 0;
        ctx.level.time = 0.0;
        ctx.deathmatch = 1.0;

        ctx.sp_func_door(1);

        // Default speed 100 * 2 = 200
        assert!((ctx.edicts[1].speed - 200.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_sp_func_door_movement_distance() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].speed = 100.0;
        // Door facing up: VEC_UP is [0, -1, 0] => movedir = [0, 0, 1]
        ctx.edicts[1].s.angles = [0.0, -1.0, 0.0];
        ctx.edicts[1].s.origin = [0.0, 0.0, 0.0];
        ctx.edicts[1].size = [64.0, 64.0, 128.0];
        ctx.st.lip = 8;
        ctx.level.time = 0.0;

        ctx.sp_func_door(1);

        // For upward-facing door: movedir = [0, 0, 1]
        // distance = abs(0)*64 + abs(0)*64 + abs(1)*128 - 8 = 120
        assert!((ctx.edicts[1].moveinfo.distance - 120.0).abs() < 0.01);
    }

    // ============================================================
    // Button position calculation tests
    // ============================================================

    #[test]
    fn test_sp_func_button_pos2_calculation() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].speed = 0.0;
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0]; // default (up direction)
        ctx.edicts[1].s.origin = [0.0, 0.0, 50.0];
        ctx.edicts[1].size = [32.0, 32.0, 16.0];
        ctx.edicts[1].sounds = 1; // no sound
        ctx.st.lip = 4;
        ctx.level.time = 0.0;

        ctx.sp_func_button(1);

        // Default speed = 40, wait = 3
        assert!((ctx.edicts[1].speed - 40.0).abs() < f32::EPSILON);
        assert!((ctx.edicts[1].wait - 3.0).abs() < f32::EPSILON);

        // pos1 = origin = [0, 0, 50]
        assert_eq!(ctx.edicts[1].pos1, [0.0, 0.0, 50.0]);
    }

    // ============================================================
    // Door rotating tests
    // ============================================================

    #[test]
    fn test_sp_func_door_rotating_axis_default() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].spawnflags = 0;
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0];
        ctx.edicts[1].s.origin = [0.0, 0.0, 0.0];
        ctx.st.distance = 90;
        ctx.level.time = 0.0;

        ctx.sp_func_door_rotating(1);

        // Default Y axis rotation
        assert_eq!(ctx.edicts[1].movedir, [0.0, 1.0, 0.0]);
        assert!((ctx.edicts[1].moveinfo.distance - 90.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_sp_func_door_rotating_x_axis() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].spawnflags = DOOR_X_AXIS;
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0];
        ctx.edicts[1].s.origin = [0.0, 0.0, 0.0];
        ctx.st.distance = 90;
        ctx.level.time = 0.0;

        ctx.sp_func_door_rotating(1);

        // X_AXIS flag sets movedir[2] = 1
        assert_eq!(ctx.edicts[1].movedir, [0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_sp_func_door_rotating_reverse() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].spawnflags = DOOR_REVERSE;
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0];
        ctx.edicts[1].s.origin = [0.0, 0.0, 0.0];
        ctx.st.distance = 90;
        ctx.level.time = 0.0;

        ctx.sp_func_door_rotating(1);

        // REVERSE negates movedir
        assert_eq!(ctx.edicts[1].movedir, [0.0, -1.0, 0.0]);
    }

    // ============================================================
    // Train defaults tests
    // ============================================================

    #[test]
    fn test_sp_func_train_defaults() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].speed = 0.0;
        ctx.edicts[1].dmg = 0;
        ctx.edicts[1].spawnflags = 0;
        ctx.st.noise = String::new();
        ctx.level.time = 0.0;

        ctx.sp_func_train(1);

        // Default speed = 100
        assert!((ctx.edicts[1].speed - 100.0).abs() < f32::EPSILON);
        assert!((ctx.edicts[1].moveinfo.speed - 100.0).abs() < f32::EPSILON);
        // Default dmg = 100 (when not BLOCK_STOPS)
        assert_eq!(ctx.edicts[1].dmg, 100);
    }

    #[test]
    fn test_sp_func_train_block_stops() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].speed = 200.0;
        ctx.edicts[1].dmg = 50;
        ctx.edicts[1].spawnflags = TRAIN_BLOCK_STOPS;
        ctx.st.noise = String::new();
        ctx.level.time = 0.0;

        ctx.sp_func_train(1);

        // TRAIN_BLOCK_STOPS sets dmg = 0
        assert_eq!(ctx.edicts[1].dmg, 0);
    }

    // ============================================================
    // Water entity tests
    // ============================================================

    #[test]
    fn test_sp_func_water_defaults() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].speed = 0.0;
        ctx.edicts[1].wait = 0.0;
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0];
        ctx.edicts[1].s.origin = [0.0, 0.0, 0.0];
        ctx.edicts[1].size = [64.0, 64.0, 32.0];
        ctx.st.lip = 0;
        ctx.level.time = 0.0;

        ctx.sp_func_water(1);

        // Default speed = 25
        assert!((ctx.edicts[1].speed - 25.0).abs() < f32::EPSILON);
        // Default wait = -1
        assert!((ctx.edicts[1].wait - (-1.0)).abs() < f32::EPSILON);
        // Classname should be changed to func_door
        assert_eq!(ctx.edicts[1].classname, "func_door");
    }

    // ============================================================
    // Secret door position calculation tests
    // ============================================================

    #[test]
    fn test_sp_func_door_secret_defaults() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].dmg = 0;
        ctx.edicts[1].wait = 0.0;
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0]; // facing forward
        ctx.edicts[1].s.origin = [0.0, 0.0, 0.0];
        ctx.edicts[1].size = [64.0, 32.0, 128.0];
        ctx.edicts[1].spawnflags = 0;
        ctx.level.time = 0.0;

        ctx.sp_func_door_secret(1);

        assert_eq!(ctx.edicts[1].dmg, 2);
        assert!((ctx.edicts[1].wait - 5.0).abs() < f32::EPSILON);
        assert!((ctx.edicts[1].moveinfo.speed - 50.0).abs() < f32::EPSILON);
        assert!((ctx.edicts[1].moveinfo.accel - 50.0).abs() < f32::EPSILON);
        assert!((ctx.edicts[1].moveinfo.decel - 50.0).abs() < f32::EPSILON);
    }

    // ============================================================
    // get_team_entity tests
    // ============================================================

    #[test]
    fn test_get_team_entity_not_slave() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].flags = EntityFlags::empty();
        assert_eq!(ctx.get_team_entity(1), 1);
    }

    #[test]
    fn test_get_team_entity_is_slave() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].flags = FL_TEAMSLAVE;
        ctx.edicts[1].teammaster = 2;
        assert_eq!(ctx.get_team_entity(1), 2);
    }
}
