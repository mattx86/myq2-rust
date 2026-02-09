// g_ai.rs — Monster AI functions
// Converted from: myq2-original/game/g_ai.c

use myq2_common::q_shared::{
    VEC3_ORIGIN, YAW,
    vector_subtract, vector_length, vector_copy, vector_normalize, dot_product,
    angle_vectors, anglemod,
    MASK_OPAQUE, MASK_PLAYERSOLID,
    CONTENTS_SOLID, CONTENTS_LAVA, CONTENTS_SLIME, CONTENTS_WINDOW, CONTENTS_MONSTER,
};
use myq2_common::common::frand as random;
use crate::game::{SVF_MONSTER};
use crate::g_local::{
    Edict, LevelLocals, GameLocals, GClient,
    MELEE_DISTANCE,
    RANGE_MELEE, RANGE_NEAR, RANGE_MID, RANGE_FAR,
    AI_STAND_GROUND, AI_TEMP_STAND_GROUND, AI_GOOD_GUY, AI_COMBAT_POINT,
    AI_SOUND_TARGET, AI_MEDIC, AI_BRUTAL, AS_SLIDING, AS_MELEE, AS_MISSILE,
    AS_STRAIGHT, AI_LOST_SIGHT, AI_PURSUIT_LAST_SEEN, AI_PURSUE_NEXT,
    AI_PURSUE_TEMP,
    FL_FLY, FL_NOTARGET,
};

/// AI context holding shared game state. Each AI function borrows this
/// instead of accessing C globals directly.
pub struct AiContext {
    pub edicts: Vec<Edict>,
    pub clients: Vec<GClient>,
    pub level: LevelLocals,
    pub game: GameLocals,
    pub coop: f32,
    pub skill: f32,
    /// Cached enemy visibility from last ai_checkattack call.
    pub enemy_vis: bool,
    /// Cached enemy infront check from last ai_checkattack call.
    pub enemy_infront: bool,
    /// Cached enemy range from last ai_checkattack call.
    pub enemy_range: i32,
    /// Cached enemy yaw from last ai_checkattack call.
    pub enemy_yaw: f32,
}

impl AiContext {
    pub fn get_edict(&self, idx: i32) -> Option<&Edict> {
        if idx < 0 || idx as usize >= self.edicts.len() {
            None
        } else {
            Some(&self.edicts[idx as usize])
        }
    }

    pub fn get_edict_mut(&mut self, idx: i32) -> Option<&mut Edict> {
        if idx < 0 || idx as usize >= self.edicts.len() {
            None
        } else {
            Some(&mut self.edicts[idx as usize])
        }
    }

    /// Check if an entity index refers to a valid, in-use entity.
    pub fn edict_valid(&self, idx: i32) -> bool {
        if let Some(e) = self.get_edict(idx) {
            e.inuse
        } else {
            false
        }
    }

    /// Check if the entity at idx is a client (has a client field).
    pub fn is_client(&self, idx: i32) -> bool {
        if let Some(e) = self.get_edict(idx) {
            e.client.is_some()
        } else {
            false
        }
    }
}

// ---- Helper wrappers for cross-module calls ----

/// M_walkmove — delegates to entity_adapters::m_walkmove.
fn m_walkmove(ctx: &mut AiContext, self_idx: i32, yaw: f32, dist: f32) -> bool {
    crate::entity_adapters::m_walkmove(&mut ctx.edicts, &mut ctx.clients, self_idx, yaw, dist)
}

/// M_ChangeYaw — delegates to m_move module.
fn m_change_yaw(ctx: &mut AiContext, self_idx: i32) {
    // Inline implementation matching M_ChangeYaw from m_move.c
    let ent = match ctx.get_edict_mut(self_idx) {
        Some(e) => e,
        None => return,
    };

    let current = anglemod(ent.s.angles[YAW]);
    let ideal = ent.ideal_yaw;

    if current == ideal {
        return;
    }

    let mut mov = ideal - current;
    let speed = ent.yaw_speed;

    if ideal > current {
        if mov >= 180.0 {
            mov -= 360.0;
        }
    } else {
        if mov <= -180.0 {
            mov += 360.0;
        }
    }

    let new_yaw;
    if mov > 0.0 {
        if mov > speed {
            new_yaw = current + speed;
        } else {
            new_yaw = ideal;
        }
    } else {
        if mov < -speed {
            new_yaw = current - speed;
        } else {
            new_yaw = ideal;
        }
    }

    let ent = ctx.get_edict_mut(self_idx).unwrap();
    ent.s.angles[YAW] = anglemod(new_yaw);
}

/// M_MoveToGoal — delegates to m_move module.
fn m_move_to_goal(ctx: &mut AiContext, self_idx: i32, dist: f32) {
    let mut move_ctx = crate::m_move::MoveContext {
        edicts: std::mem::take(&mut ctx.edicts),
        clients: std::mem::take(&mut ctx.clients),
        c_yes: 0,
        c_no: 0,
    };
    crate::m_move::m_move_to_goal(&mut move_ctx, self_idx, dist);
    ctx.edicts = move_ctx.edicts;
    ctx.clients = move_ctx.clients;
}

/// AttackFinished — delegates to g_monster module.
fn attack_finished(ctx: &mut AiContext, self_idx: i32, time: f32) {
    let level_time = ctx.level.time;
    if let Some(ent) = ctx.get_edict_mut(self_idx) {
        ent.monsterinfo.attack_finished = level_time + time;
    }
}



/// PlayerTrail_PickFirst — delegates to p_trail module.
///
/// Bridges AiContext to GameCtx by temporarily moving edicts into the global
/// game context so `p_trail::player_trail_pick_first` can read both the
/// trail state and the current entity data.
fn player_trail_pick_first(ctx: &mut AiContext, self_idx: i32) -> Option<i32> {
    use std::mem;
    crate::g_local::with_global_game_ctx(|game_ctx| {
        // Move current edicts into GameCtx so p_trail sees up-to-date entities.
        let saved_edicts = mem::replace(&mut game_ctx.edicts, mem::take(&mut ctx.edicts));
        let result = crate::p_trail::player_trail_pick_first(game_ctx, self_idx as usize);
        // Restore edicts back to AiContext; put the original GameCtx edicts back.
        ctx.edicts = mem::replace(&mut game_ctx.edicts, saved_edicts);
        result.map(|idx| idx as i32)
    })
    .flatten()
}

/// PlayerTrail_PickNext — delegates to p_trail module.
///
/// Same bridging pattern as `player_trail_pick_first`.
fn player_trail_pick_next(ctx: &mut AiContext, self_idx: i32) -> Option<i32> {
    use std::mem;
    crate::g_local::with_global_game_ctx(|game_ctx| {
        let saved_edicts = mem::replace(&mut game_ctx.edicts, mem::take(&mut ctx.edicts));
        let result = crate::p_trail::player_trail_pick_next(game_ctx, self_idx as usize);
        ctx.edicts = mem::replace(&mut game_ctx.edicts, saved_edicts);
        result.map(|idx| idx as i32)
    })
    .flatten()
}


use myq2_common::q_shared::vectoyaw;

use crate::game_import::{gi_trace, gi_in_phs, gi_areas_connected, gi_dprintf};


// ============================================================================

/// AI_SetSightClient
///
/// Called once each frame to set level.sight_client to the
/// player to be checked for in findtarget.
///
/// If all clients are either dead or in notarget, sight_client
/// will be null (represented as -1).
///
/// In coop games, sight_client will cycle between the clients.
pub fn ai_set_sight_client(ctx: &mut AiContext) {
    // In C, sight_client is a pointer (NULL when unset). In Rust it's an
    // i32 index where -1 and 0 (world entity) both mean "no valid client".
    // Guard against maxclients <= 0 — no clients to search.
    if ctx.game.maxclients <= 0 {
        ctx.level.sight_client = -1;
        return;
    }

    let start = if ctx.level.sight_client <= 0 {
        1
    } else {
        ctx.level.sight_client
    };

    let mut check = start;
    loop {
        check += 1;
        if check > ctx.game.maxclients {
            check = 1;
        }

        if let Some(ent) = ctx.get_edict(check) {
            if ent.inuse
                && ent.health > 0
                && !ent.flags.intersects(FL_NOTARGET)
            {
                ctx.level.sight_client = check;
                return; // got one
            }
        }

        if check == start {
            ctx.level.sight_client = -1;
            return; // nobody to see
        }
    }
}

// ============================================================================

/// ai_move
///
/// Move the specified distance at current facing.
/// This replaces the QC functions: ai_forward, ai_back, ai_pain, and ai_painforward
pub fn ai_move(ctx: &mut AiContext, self_idx: i32, dist: f32) {
    let yaw = match ctx.get_edict(self_idx) {
        Some(e) => e.s.angles[YAW],
        None => return,
    };
    m_walkmove(ctx, self_idx, yaw, dist);
}

/// ai_stand
///
/// Used for standing around and looking for players.
/// Distance is for slight position adjustments needed by the animations.
pub fn ai_stand(ctx: &mut AiContext, self_idx: i32, dist: f32) {
    if dist != 0.0 {
        let yaw = match ctx.get_edict(self_idx) {
            Some(e) => e.s.angles[YAW],
            None => return,
        };
        m_walkmove(ctx, self_idx, yaw, dist);
    }

    let aiflags = match ctx.get_edict(self_idx) {
        Some(e) => e.monsterinfo.aiflags,
        None => return,
    };

    if aiflags.intersects(AI_STAND_GROUND) {
        let enemy_idx = match ctx.get_edict(self_idx) {
            Some(e) => e.enemy,
            None => return,
        };

        if enemy_idx != -1 {
            if let (Some(enemy), Some(self_ent)) = (ctx.get_edict(enemy_idx), ctx.get_edict(self_idx)) {
                let v = vector_subtract(&enemy.s.origin, &self_ent.s.origin);
                let ideal = vectoyaw(&v);
                let cur_yaw = self_ent.s.angles[YAW];
                let ai2 = self_ent.monsterinfo.aiflags;

                if let Some(e) = ctx.get_edict_mut(self_idx) {
                    e.ideal_yaw = ideal;
                }

                if cur_yaw != ideal && ai2.intersects(AI_TEMP_STAND_GROUND) {
                    if let Some(e) = ctx.get_edict_mut(self_idx) {
                        e.monsterinfo.aiflags &= !(AI_STAND_GROUND | AI_TEMP_STAND_GROUND);
                    }
                    crate::dispatch::call_run(self_idx as usize, &mut ctx.edicts, &mut ctx.level);
                }
            }
            m_change_yaw(ctx, self_idx);
            ai_checkattack(ctx, self_idx, 0.0);
        } else {
            find_target(ctx, self_idx);
        }
        return;
    }

    if find_target(ctx, self_idx) {
        return;
    }

    let (pausetime, level_time, idle_time, spawnflags, has_idle_fn) = match ctx.get_edict(self_idx) {
        Some(e) => (
            e.monsterinfo.pausetime,
            ctx.level.time,
            e.monsterinfo.idle_time,
            e.spawnflags,
            e.monsterinfo.idle_fn.is_some(),
        ),
        None => return,
    };

    if level_time > pausetime {
        crate::dispatch::call_walk(self_idx as usize, &mut ctx.edicts, &mut ctx.level);
        return;
    }

    if (spawnflags & 1) == 0 && has_idle_fn && level_time > idle_time {
        if idle_time != 0.0 {
            crate::dispatch::call_idle(self_idx as usize, &mut ctx.edicts, &mut ctx.level);
            if let Some(e) = ctx.get_edict_mut(self_idx) {
                e.monsterinfo.idle_time = level_time + 15.0 + random() * 15.0;
            }
        } else {
            if let Some(e) = ctx.get_edict_mut(self_idx) {
                e.monsterinfo.idle_time = level_time + random() * 15.0;
            }
        }
    }
}

/// ai_walk
///
/// The monster is walking its beat.
pub fn ai_walk(ctx: &mut AiContext, self_idx: i32, dist: f32) {
    m_move_to_goal(ctx, self_idx, dist);

    // check for noticing a player
    if find_target(ctx, self_idx) {
        return;
    }

    let (has_search, idle_time, level_time) = match ctx.get_edict(self_idx) {
        Some(e) => (
            e.monsterinfo.search_fn.is_some(),
            e.monsterinfo.idle_time,
            ctx.level.time,
        ),
        None => return,
    };

    if has_search && level_time > idle_time {
        if idle_time != 0.0 {
            crate::dispatch::call_search(self_idx as usize, &mut ctx.edicts, &mut ctx.level);
            if let Some(e) = ctx.get_edict_mut(self_idx) {
                e.monsterinfo.idle_time = level_time + 15.0 + random() * 15.0;
            }
        } else {
            if let Some(e) = ctx.get_edict_mut(self_idx) {
                e.monsterinfo.idle_time = level_time + random() * 15.0;
            }
        }
    }
}

/// ai_charge
///
/// Turns towards target and advances.
/// Use this call with a distance of 0 to replace ai_face.
pub fn ai_charge(ctx: &mut AiContext, self_idx: i32, dist: f32) {
    let enemy_idx = match ctx.get_edict(self_idx) {
        Some(e) => e.enemy,
        None => return,
    };

    if enemy_idx != -1 {
        if let (Some(enemy), Some(self_ent)) = (ctx.get_edict(enemy_idx), ctx.get_edict(self_idx)) {
            let v = vector_subtract(&enemy.s.origin, &self_ent.s.origin);
            let ideal = vectoyaw(&v);
            if let Some(e) = ctx.get_edict_mut(self_idx) {
                e.ideal_yaw = ideal;
            }
        }
    }
    m_change_yaw(ctx, self_idx);

    if dist != 0.0 {
        let yaw = match ctx.get_edict(self_idx) {
            Some(e) => e.s.angles[YAW],
            None => return,
        };
        m_walkmove(ctx, self_idx, yaw, dist);
    }
}

/// ai_turn
///
/// Don't move, but turn towards ideal_yaw.
/// Distance is for slight position adjustments needed by the animations.
pub fn ai_turn(ctx: &mut AiContext, self_idx: i32, dist: f32) {
    if dist != 0.0 {
        let yaw = match ctx.get_edict(self_idx) {
            Some(e) => e.s.angles[YAW],
            None => return,
        };
        m_walkmove(ctx, self_idx, yaw, dist);
    }

    if find_target(ctx, self_idx) {
        return;
    }

    m_change_yaw(ctx, self_idx);
}

// ============================================================================

/// range
///
/// Returns the range categorization of an entity relative to self.
/// 0 melee range, 1 near, 2 mid, 3 far
pub fn range(self_ent: &Edict, other: &Edict) -> i32 {
    let v = vector_subtract(&self_ent.s.origin, &other.s.origin);
    let len = vector_length(&v);

    if len < MELEE_DISTANCE {
        RANGE_MELEE
    } else if len < 500.0 {
        RANGE_NEAR
    } else if len < 1000.0 {
        RANGE_MID
    } else {
        RANGE_FAR
    }
}

/// visible
///
/// Returns true if the entity is visible to self, even if not infront().
pub fn visible(self_ent: &Edict, other: &Edict) -> bool {
    let mut spot1 = vector_copy(&self_ent.s.origin);
    spot1[2] += self_ent.viewheight as f32;

    let mut spot2 = vector_copy(&other.s.origin);
    spot2[2] += other.viewheight as f32;

    let trace = gi_trace(
        &spot1,
        &VEC3_ORIGIN,
        &VEC3_ORIGIN,
        &spot2,
        self_ent.s.number,
        MASK_OPAQUE,
    );

    trace.fraction == 1.0
}

/// visible_idx — same as visible but works with entity indices into AiContext.
fn visible_idx(ctx: &AiContext, self_idx: i32, other_idx: i32) -> bool {
    if let (Some(s), Some(o)) = (ctx.get_edict(self_idx), ctx.get_edict(other_idx)) {
        visible(s, o)
    } else {
        false
    }
}

/// infront
///
/// Returns true if the entity is in front (in sight) of self.
pub fn infront(self_ent: &Edict, other: &Edict) -> bool {
    let mut forward = [0.0; 3];

    angle_vectors(&self_ent.s.angles, Some(&mut forward), None, None);
    let mut vec = vector_subtract(&other.s.origin, &self_ent.s.origin);
    vector_normalize(&mut vec);
    let dot = dot_product(&vec, &forward);

    dot > 0.3
}

/// infront_idx — same as infront but works with entity indices.
fn infront_idx(ctx: &AiContext, self_idx: i32, other_idx: i32) -> bool {
    if let (Some(s), Some(o)) = (ctx.get_edict(self_idx), ctx.get_edict(other_idx)) {
        infront(s, o)
    } else {
        false
    }
}

/// range_idx — same as range but works with entity indices.
fn range_idx(ctx: &AiContext, self_idx: i32, other_idx: i32) -> i32 {
    if let (Some(s), Some(o)) = (ctx.get_edict(self_idx), ctx.get_edict(other_idx)) {
        range(s, o)
    } else {
        RANGE_FAR
    }
}

// ============================================================================

/// HuntTarget
pub fn hunt_target(ctx: &mut AiContext, self_idx: i32) {
    {
        let enemy_idx = match ctx.get_edict(self_idx) {
            Some(e) => e.enemy,
            None => return,
        };

        if let Some(e) = ctx.get_edict_mut(self_idx) {
            e.goalentity = e.enemy;
        }

        let aiflags = match ctx.get_edict(self_idx) {
            Some(e) => e.monsterinfo.aiflags,
            None => return,
        };

        if aiflags.intersects(AI_STAND_GROUND) {
            crate::dispatch::call_stand(self_idx as usize, &mut ctx.edicts, &mut ctx.level);
        } else {
            crate::dispatch::call_run(self_idx as usize, &mut ctx.edicts, &mut ctx.level);
        }

        // Compute ideal_yaw from enemy position
        if let (Some(enemy), Some(self_ent)) = (ctx.get_edict(enemy_idx), ctx.get_edict(self_idx)) {
            let vec = vector_subtract(&enemy.s.origin, &self_ent.s.origin);
            let yaw = vectoyaw(&vec);
            if let Some(e) = ctx.get_edict_mut(self_idx) {
                e.ideal_yaw = yaw;
            }
        }

        // wait a while before first attack
        if !aiflags.intersects(AI_STAND_GROUND) {
            attack_finished(ctx, self_idx, 1.0);
        }
    }
}

/// FoundTarget
pub fn found_target(ctx: &mut AiContext, self_idx: i32) {
    let enemy_idx = match ctx.get_edict(self_idx) {
        Some(e) => e.enemy,
        None => return,
    };

    // let other monsters see this monster for a while
    if ctx.is_client(enemy_idx) {
        ctx.level.sight_entity = self_idx;
        ctx.level.sight_entity_framenum = ctx.level.framenum;
        if let Some(e) = ctx.get_edict_mut(self_idx) {
            e.light_level = 128;
        }
    }

    let level_time = ctx.level.time;
    if let Some(e) = ctx.get_edict_mut(self_idx) {
        e.show_hostile = level_time + 1.0; // wake up other monsters
    }

    // Copy enemy origin to last_sighting
    if let Some(enemy) = ctx.get_edict(enemy_idx) {
        let enemy_origin = vector_copy(&enemy.s.origin);
        if let Some(e) = ctx.get_edict_mut(self_idx) {
            e.monsterinfo.last_sighting = enemy_origin;
            e.monsterinfo.trail_time = level_time;
        }
    }

    let combattarget = match ctx.get_edict(self_idx) {
        Some(e) => e.combattarget.clone(),
        None => return,
    };

    if combattarget.is_empty() {
        hunt_target(ctx, self_idx);
        return;
    }

    // Try to pick combat target (inlined from G_PickTarget)
    let target_idx: Option<i32> = {
        let mut choices = Vec::new();
        for i in 0..ctx.edicts.len() {
            if !ctx.edicts[i].inuse { continue; }
            if ctx.edicts[i].targetname == combattarget {
                choices.push(i as i32);
            }
        }
        if choices.is_empty() {
            None
        } else {
            let idx = (rand::random::<u32>() as usize) % choices.len();
            Some(choices[idx])
        }
    };
    match target_idx {
        Some(t) => {
            if let Some(e) = ctx.get_edict_mut(self_idx) {
                e.goalentity = t;
                e.movetarget = t;
                // clear out combattarget (one shot deal)
                e.combattarget = String::new();
                e.monsterinfo.aiflags |= AI_COMBAT_POINT;
                e.monsterinfo.pausetime = 0.0;
            }
            // clear the targetname on the combat point
            if let Some(target) = ctx.get_edict_mut(t) {
                target.targetname = String::new();
            }
            crate::dispatch::call_run(self_idx as usize, &mut ctx.edicts, &mut ctx.level);
        }
        None => {
            // Combat target not found
            let (classname, origin) = match ctx.get_edict(self_idx) {
                Some(e) => (e.classname.clone(), e.s.origin),
                None => return,
            };
            gi_dprintf(&format!(
                "{} at ({} {} {}), combattarget {} not found\n",
                classname, origin[0], origin[1], origin[2], combattarget
            ));
            if let Some(e) = ctx.get_edict_mut(self_idx) {
                e.goalentity = e.enemy;
                e.movetarget = e.enemy;
            }
            hunt_target(ctx, self_idx);
        }
    }
}

/// FindTarget
///
/// Self is currently not attacking anything, so try to find a target.
///
/// Returns TRUE if an enemy was sighted.
///
/// When a player fires a missile, the point of impact becomes a fakeplayer so
/// that monsters that see the impact will respond as if they had seen the
/// player.
///
/// To avoid spending too much time, only a single client (or fakeclient) is
/// checked each frame. This means multi player games will have slightly
/// slower noticing monsters.
pub fn find_target(ctx: &mut AiContext, self_idx: i32) -> bool {
    let aiflags = match ctx.get_edict(self_idx) {
        Some(e) => e.monsterinfo.aiflags,
        None => return false,
    };

    if aiflags.intersects(AI_GOOD_GUY) {
        // Check if we have a goalentity that is a target_actor
        let goal = match ctx.get_edict(self_idx) {
            Some(e) => e.goalentity,
            None => return false,
        };
        if goal != -1 {
            if let Some(g) = ctx.get_edict(goal) {
                if g.inuse && g.classname == "target_actor" {
                    return false;
                }
            }
        }
        return false;
    }

    // if we're going to a combat point, just proceed
    if aiflags.intersects(AI_COMBAT_POINT) {
        return false;
    }

    let self_spawnflags = match ctx.get_edict(self_idx) {
        Some(e) => e.spawnflags,
        None => return false,
    };
    let self_enemy = match ctx.get_edict(self_idx) {
        Some(e) => e.enemy,
        None => return false,
    };

    let mut heardit = false;
    let client_idx: i32;

    if ctx.level.sight_entity_framenum >= (ctx.level.framenum - 1)
        && (self_spawnflags & 1) == 0
    {
        client_idx = ctx.level.sight_entity;
        // if client->enemy == self->enemy, return false
        if let Some(c) = ctx.get_edict(client_idx) {
            if c.enemy == self_enemy {
                return false;
            }
        }
    } else if ctx.level.sound_entity_framenum >= (ctx.level.framenum - 1) {
        client_idx = ctx.level.sound_entity;
        heardit = true;
    } else if self_enemy == -1
        && ctx.level.sound2_entity_framenum >= (ctx.level.framenum - 1)
        && (self_spawnflags & 1) == 0
    {
        client_idx = ctx.level.sound2_entity;
        heardit = true;
    } else {
        client_idx = ctx.level.sight_client;
        if client_idx == -1 {
            return false; // no clients to get mad at
        }
    }

    // if the entity went away, forget it
    if !ctx.edict_valid(client_idx) {
        return false;
    }

    // if client is our current enemy, return true (JDC)
    if client_idx == self_enemy {
        return true;
    }

    let client_is_player = ctx.is_client(client_idx);
    let client_svflags = match ctx.get_edict(client_idx) {
        Some(e) => e.svflags,
        None => return false,
    };
    let client_flags = match ctx.get_edict(client_idx) {
        Some(e) => e.flags,
        None => return false,
    };

    if client_is_player {
        if client_flags.intersects(FL_NOTARGET) {
            return false;
        }
    } else if (client_svflags & SVF_MONSTER) != 0 {
        let client_enemy = match ctx.get_edict(client_idx) {
            Some(e) => e.enemy,
            None => return false,
        };
        if client_enemy == -1 {
            return false;
        }
        if let Some(ce) = ctx.get_edict(client_enemy) {
            if ce.flags.intersects(FL_NOTARGET) {
                return false;
            }
        }
    } else if heardit {
        let client_owner = match ctx.get_edict(client_idx) {
            Some(e) => e.owner,
            None => return false,
        };
        if let Some(owner) = ctx.get_edict(client_owner) {
            if owner.flags.intersects(FL_NOTARGET) {
                return false;
            }
        } else {
            return false;
        }
    } else {
        return false;
    }

    if !heardit {
        let r = range_idx(ctx, self_idx, client_idx);

        if r == RANGE_FAR {
            return false;
        }

        // is client too dark to be seen?
        let client_light = match ctx.get_edict(client_idx) {
            Some(e) => e.light_level,
            None => return false,
        };
        if client_light <= 5 {
            return false;
        }

        if !visible_idx(ctx, self_idx, client_idx) {
            return false;
        }

        if r == RANGE_NEAR {
            let client_show_hostile = match ctx.get_edict(client_idx) {
                Some(e) => e.show_hostile,
                None => return false,
            };
            if client_show_hostile < ctx.level.time && !infront_idx(ctx, self_idx, client_idx) {
                return false;
            }
        } else if r == RANGE_MID {
            if !infront_idx(ctx, self_idx, client_idx) {
                return false;
            }
        }

        if let Some(e) = ctx.get_edict_mut(self_idx) {
            e.enemy = client_idx;
        }

        // Check if enemy is a player_noise — if so, trace back to real player
        let enemy_classname = match ctx.get_edict(client_idx) {
            Some(e) => e.classname.clone(),
            None => String::new(),
        };
        if enemy_classname != "player_noise" {
            if let Some(e) = ctx.get_edict_mut(self_idx) {
                e.monsterinfo.aiflags &= !AI_SOUND_TARGET;
            }

            if !ctx.is_client(client_idx) {
                // enemy is not a client; follow chain to the client
                let client_enemy = match ctx.get_edict(client_idx) {
                    Some(e) => e.enemy,
                    None => -1,
                };
                if let Some(e) = ctx.get_edict_mut(self_idx) {
                    e.enemy = client_enemy;
                }
                let new_enemy = match ctx.get_edict(self_idx) {
                    Some(e) => e.enemy,
                    None => return false,
                };
                if !ctx.is_client(new_enemy) {
                    if let Some(e) = ctx.get_edict_mut(self_idx) {
                        e.enemy = -1;
                    }
                    return false;
                }
            }
        }
    } else {
        // heardit
        if (self_spawnflags & 1) != 0 {
            if !visible_idx(ctx, self_idx, client_idx) {
                return false;
            }
        } else {
            let self_origin = match ctx.get_edict(self_idx) {
                Some(e) => e.s.origin,
                None => return false,
            };
            let client_origin = match ctx.get_edict(client_idx) {
                Some(e) => e.s.origin,
                None => return false,
            };
            if !gi_in_phs(&self_origin, &client_origin) {
                return false;
            }
        }

        let (self_origin, client_origin) = match (ctx.get_edict(self_idx), ctx.get_edict(client_idx)) {
            (Some(s), Some(c)) => (s.s.origin, c.s.origin),
            _ => return false,
        };
        let temp = vector_subtract(&client_origin, &self_origin);

        if vector_length(&temp) > 1000.0 {
            // too far to hear
            return false;
        }

        // check area portals
        let (self_areanum, client_areanum) = match (ctx.get_edict(self_idx), ctx.get_edict(client_idx)) {
            (Some(s), Some(c)) => (s.areanum, c.areanum),
            _ => return false,
        };
        if client_areanum != self_areanum {
            if !gi_areas_connected(self_areanum, client_areanum) {
                return false;
            }
        }

        let ideal = vectoyaw(&temp);
        if let Some(e) = ctx.get_edict_mut(self_idx) {
            e.ideal_yaw = ideal;
        }
        m_change_yaw(ctx, self_idx);

        // hunt the sound for a bit; hopefully find the real player
        if let Some(e) = ctx.get_edict_mut(self_idx) {
            e.monsterinfo.aiflags |= AI_SOUND_TARGET;
            e.enemy = client_idx;
        }
    }

    // got one
    found_target(ctx, self_idx);

    let aiflags_after = match ctx.get_edict(self_idx) {
        Some(e) => e.monsterinfo.aiflags,
        None => return true,
    };
    if !aiflags_after.intersects(AI_SOUND_TARGET) {
        let has_sight = match ctx.get_edict(self_idx) {
            Some(e) => e.monsterinfo.sight_fn.is_some(),
            None => false,
        };
        if has_sight {
            crate::dispatch::call_sight(self_idx as usize, &mut ctx.edicts, &mut ctx.level);
        }
    }

    true
}

// =============================================================================

/// FacingIdeal
pub fn facing_ideal(self_ent: &Edict) -> bool {
    let delta = anglemod(self_ent.s.angles[YAW] - self_ent.ideal_yaw);
    !(delta > 45.0 && delta < 315.0)
}

// =============================================================================

/// M_CheckAttack
///
/// Default check attack routine. Determines whether to melee or missile attack.
pub fn m_check_attack(ctx: &mut AiContext, self_idx: i32) -> bool {
    let enemy_idx = match ctx.get_edict(self_idx) {
        Some(e) => e.enemy,
        None => return false,
    };

    // Check if enemy is alive — if so, verify line of sight
    let enemy_health = match ctx.get_edict(enemy_idx) {
        Some(e) => e.health,
        None => return false,
    };

    if enemy_health > 0 {
        // see if any entities are in the way of the shot
        let spot1 = match ctx.get_edict(self_idx) {
            Some(e) => {
                let mut s = vector_copy(&e.s.origin);
                s[2] += e.viewheight as f32;
                s
            }
            None => return false,
        };
        let spot2 = match ctx.get_edict(enemy_idx) {
            Some(e) => {
                let mut s = vector_copy(&e.s.origin);
                s[2] += e.viewheight as f32;
                s
            }
            None => return false,
        };
        let tr = gi_trace(
            &spot1,
            &VEC3_ORIGIN,
            &VEC3_ORIGIN,
            &spot2,
            self_idx,
            CONTENTS_SOLID | CONTENTS_MONSTER | CONTENTS_SLIME | CONTENTS_LAVA | CONTENTS_WINDOW,
        );

        // do we have a clear shot?
        if tr.ent_index != enemy_idx {
            return false;
        }
    }

    // melee attack
    if ctx.enemy_range == RANGE_MELEE {
        // don't always melee in easy mode
        if ctx.skill == 0.0 && (rand::random::<i32>() & 3) != 0 {
            return false;
        }
        let has_melee = match ctx.get_edict(self_idx) {
            Some(e) => e.monsterinfo.melee_fn.is_some(),
            None => return false,
        };
        if has_melee {
            if let Some(e) = ctx.get_edict_mut(self_idx) {
                e.monsterinfo.attack_state = AS_MELEE;
            }
        } else {
            if let Some(e) = ctx.get_edict_mut(self_idx) {
                e.monsterinfo.attack_state = AS_MISSILE;
            }
        }
        return true;
    }

    // missile attack
    let has_attack = match ctx.get_edict(self_idx) {
        Some(e) => e.monsterinfo.attack_fn.is_some(),
        None => return false,
    };
    if !has_attack {
        return false;
    }

    let attack_finished_time = match ctx.get_edict(self_idx) {
        Some(e) => e.monsterinfo.attack_finished,
        None => return false,
    };
    if ctx.level.time < attack_finished_time {
        return false;
    }

    if ctx.enemy_range == RANGE_FAR {
        return false;
    }

    let aiflags = match ctx.get_edict(self_idx) {
        Some(e) => e.monsterinfo.aiflags,
        None => return false,
    };

    let mut chance: f32;
    if aiflags.intersects(AI_STAND_GROUND) {
        chance = 0.4;
    } else if ctx.enemy_range == RANGE_MELEE {
        chance = 0.2;
    } else if ctx.enemy_range == RANGE_NEAR {
        chance = 0.1;
    } else if ctx.enemy_range == RANGE_MID {
        chance = 0.02;
    } else {
        return false;
    }

    if ctx.skill == 0.0 {
        chance *= 0.5;
    } else if ctx.skill >= 2.0 {
        chance *= 2.0;
    }

    if random() < chance {
        let finished_time = ctx.level.time + 2.0 * random();
        if let Some(e) = ctx.get_edict_mut(self_idx) {
            e.monsterinfo.attack_state = AS_MISSILE;
            e.monsterinfo.attack_finished = finished_time;
        }
        return true;
    }

    let flags = match ctx.get_edict(self_idx) {
        Some(e) => e.flags,
        None => return false,
    };
    if flags.intersects(FL_FLY) {
        if random() < 0.3 {
            if let Some(e) = ctx.get_edict_mut(self_idx) {
                e.monsterinfo.attack_state = AS_SLIDING;
            }
        } else {
            if let Some(e) = ctx.get_edict_mut(self_idx) {
                e.monsterinfo.attack_state = AS_STRAIGHT;
            }
        }
    }

    false
}

/// ai_run_melee
///
/// Turn and close until within an angle to launch a melee attack.
pub fn ai_run_melee(ctx: &mut AiContext, self_idx: i32) {
    let yaw = ctx.enemy_yaw;
    if let Some(e) = ctx.get_edict_mut(self_idx) {
        e.ideal_yaw = yaw;
    }
    m_change_yaw(ctx, self_idx);

    let is_facing = match ctx.get_edict(self_idx) {
        Some(e) => facing_ideal(e),
        None => return,
    };
    if is_facing {
        crate::dispatch::call_melee(self_idx as usize, &mut ctx.edicts, &mut ctx.level);
        if let Some(e) = ctx.get_edict_mut(self_idx) {
            e.monsterinfo.attack_state = AS_STRAIGHT;
        }
    }
}

/// ai_run_missile
///
/// Turn in place until within an angle to launch a missile attack.
pub fn ai_run_missile(ctx: &mut AiContext, self_idx: i32) {
    let yaw = ctx.enemy_yaw;
    if let Some(e) = ctx.get_edict_mut(self_idx) {
        e.ideal_yaw = yaw;
    }
    m_change_yaw(ctx, self_idx);

    let is_facing = match ctx.get_edict(self_idx) {
        Some(e) => facing_ideal(e),
        None => return,
    };
    if is_facing {
        crate::dispatch::call_attack(self_idx as usize, &mut ctx.edicts, &mut ctx.level);
        if let Some(e) = ctx.get_edict_mut(self_idx) {
            e.monsterinfo.attack_state = AS_STRAIGHT;
        }
    }
}

/// ai_run_slide
///
/// Strafe sideways, but stay at approximately the same range.
pub fn ai_run_slide(ctx: &mut AiContext, self_idx: i32, distance: f32) {
    let yaw = ctx.enemy_yaw;
    if let Some(e) = ctx.get_edict_mut(self_idx) {
        e.ideal_yaw = yaw;
    }
    m_change_yaw(ctx, self_idx);

    let (lefty, ideal_yaw) = match ctx.get_edict(self_idx) {
        Some(e) => (e.monsterinfo.lefty, e.ideal_yaw),
        None => return,
    };

    let ofs: f32 = if lefty != 0 { 90.0 } else { -90.0 };

    if m_walkmove(ctx, self_idx, ideal_yaw + ofs, distance) {
        return;
    }

    if let Some(e) = ctx.get_edict_mut(self_idx) {
        e.monsterinfo.lefty = 1 - e.monsterinfo.lefty;
    }
    m_walkmove(ctx, self_idx, ideal_yaw - ofs, distance);
}

/// ai_checkattack
///
/// Decides if we're going to attack or do something else.
/// Used by ai_run and ai_stand.
pub fn ai_checkattack(ctx: &mut AiContext, self_idx: i32, _dist: f32) -> bool {
    // this causes monsters to run blindly to the combat point w/o firing
    let goalentity = match ctx.get_edict(self_idx) {
        Some(e) => e.goalentity,
        None => return false,
    };

    if goalentity != -1 {
        let aiflags = match ctx.get_edict(self_idx) {
            Some(e) => e.monsterinfo.aiflags,
            None => return false,
        };

        if aiflags.intersects(AI_COMBAT_POINT) {
            return false;
        }

        if aiflags.intersects(AI_SOUND_TARGET) {
            let enemy_idx = match ctx.get_edict(self_idx) {
                Some(e) => e.enemy,
                None => return false,
            };
            let teleport_time = match ctx.get_edict(enemy_idx) {
                Some(e) => e.teleport_time,
                None => 0.0,
            };
            if (ctx.level.time - teleport_time) > 5.0 {
                let (goal_eq_enemy, movetarget) = match ctx.get_edict(self_idx) {
                    Some(e) => (e.goalentity == e.enemy, e.movetarget),
                    None => return false,
                };
                if goal_eq_enemy {
                    if movetarget != -1 {
                        if let Some(e) = ctx.get_edict_mut(self_idx) {
                            e.goalentity = e.movetarget;
                        }
                    } else {
                        if let Some(e) = ctx.get_edict_mut(self_idx) {
                            e.goalentity = -1;
                        }
                    }
                }
                if let Some(e) = ctx.get_edict_mut(self_idx) {
                    e.monsterinfo.aiflags &= !AI_SOUND_TARGET;
                    if e.monsterinfo.aiflags.intersects(AI_TEMP_STAND_GROUND) {
                        e.monsterinfo.aiflags &= !(AI_STAND_GROUND | AI_TEMP_STAND_GROUND);
                    }
                }
            } else {
                let level_time = ctx.level.time;
                if let Some(e) = ctx.get_edict_mut(self_idx) {
                    e.show_hostile = level_time + 1.0;
                }
                return false;
            }
        }
    }

    ctx.enemy_vis = false;

    // see if the enemy is dead
    let mut hes_dead_jim = false;
    let enemy_idx = match ctx.get_edict(self_idx) {
        Some(e) => e.enemy,
        None => return false,
    };

    if enemy_idx == -1 || !ctx.edict_valid(enemy_idx) {
        hes_dead_jim = true;
    } else {
        let aiflags = match ctx.get_edict(self_idx) {
            Some(e) => e.monsterinfo.aiflags,
            None => return false,
        };
        let enemy_health = match ctx.get_edict(enemy_idx) {
            Some(e) => e.health,
            None => { hes_dead_jim = true; 0 },
        };

        if aiflags.intersects(AI_MEDIC) {
            if enemy_health > 0 {
                hes_dead_jim = true;
                if let Some(e) = ctx.get_edict_mut(self_idx) {
                    e.monsterinfo.aiflags &= !AI_MEDIC;
                }
            }
        } else if aiflags.intersects(AI_BRUTAL) {
            if enemy_health <= -80 {
                hes_dead_jim = true;
            }
        } else {
            if enemy_health <= 0 {
                hes_dead_jim = true;
            }
        }
    }

    if hes_dead_jim {
        if let Some(e) = ctx.get_edict_mut(self_idx) {
            e.enemy = -1;
        }

        // look for oldenemy
        let oldenemy = match ctx.get_edict(self_idx) {
            Some(e) => e.oldenemy,
            None => return true,
        };
        if oldenemy != -1 {
            let old_health = match ctx.get_edict(oldenemy) {
                Some(e) => e.health,
                None => 0,
            };
            if old_health > 0 {
                if let Some(e) = ctx.get_edict_mut(self_idx) {
                    e.enemy = oldenemy;
                    e.oldenemy = -1;
                }
                hunt_target(ctx, self_idx);
            } else {
                if let Some(e) = ctx.get_edict_mut(self_idx) {
                    e.oldenemy = -1;
                }
                // fall through to movetarget check below
                let movetarget = match ctx.get_edict(self_idx) {
                    Some(e) => e.movetarget,
                    None => return true,
                };
                if movetarget != -1 {
                    if let Some(e) = ctx.get_edict_mut(self_idx) {
                        e.goalentity = e.movetarget;
                    }
                    crate::dispatch::call_walk(self_idx as usize, &mut ctx.edicts, &mut ctx.level);
                } else {
                    let level_time = ctx.level.time;
                    if let Some(e) = ctx.get_edict_mut(self_idx) {
                        e.monsterinfo.pausetime = level_time + 100000000.0;
                    }
                    crate::dispatch::call_stand(self_idx as usize, &mut ctx.edicts, &mut ctx.level);
                }
                return true;
            }
        } else {
            let movetarget = match ctx.get_edict(self_idx) {
                Some(e) => e.movetarget,
                None => return true,
            };
            if movetarget != -1 {
                if let Some(e) = ctx.get_edict_mut(self_idx) {
                    e.goalentity = e.movetarget;
                }
                crate::dispatch::call_walk(self_idx as usize, &mut ctx.edicts, &mut ctx.level);
            } else {
                let level_time = ctx.level.time;
                if let Some(e) = ctx.get_edict_mut(self_idx) {
                    e.monsterinfo.pausetime = level_time + 100000000.0;
                }
                crate::dispatch::call_stand(self_idx as usize, &mut ctx.edicts, &mut ctx.level);
            }
            return true;
        }
    }

    let level_time = ctx.level.time;
    if let Some(e) = ctx.get_edict_mut(self_idx) {
        e.show_hostile = level_time + 1.0; // wake up other monsters
    }

    // check knowledge of enemy
    ctx.enemy_vis = visible_idx(ctx, self_idx, enemy_idx);
    if ctx.enemy_vis {
        if let Some(enemy) = ctx.get_edict(enemy_idx) {
            let enemy_origin = vector_copy(&enemy.s.origin);
            if let Some(e) = ctx.get_edict_mut(self_idx) {
                e.monsterinfo.search_time = level_time + 5.0;
                e.monsterinfo.last_sighting = enemy_origin;
            }
        }
    }

    ctx.enemy_infront = infront_idx(ctx, self_idx, enemy_idx);
    ctx.enemy_range = range_idx(ctx, self_idx, enemy_idx);

    if let (Some(enemy), Some(self_ent)) = (ctx.get_edict(enemy_idx), ctx.get_edict(self_idx)) {
        let temp = vector_subtract(&enemy.s.origin, &self_ent.s.origin);
        ctx.enemy_yaw = vectoyaw(&temp);
    }

    let attack_state = match ctx.get_edict(self_idx) {
        Some(e) => e.monsterinfo.attack_state,
        None => return false,
    };

    if attack_state == AS_MISSILE {
        ai_run_missile(ctx, self_idx);
        return true;
    }
    if attack_state == AS_MELEE {
        ai_run_melee(ctx, self_idx);
        return true;
    }

    // if enemy is not currently visible, we will never attack
    if !ctx.enemy_vis {
        return false;
    }

    // Call entity's checkattack function (default: m_check_attack)
    crate::dispatch::call_checkattack(self_idx as usize, &mut ctx.edicts, &mut ctx.level)
}

/// ai_run
///
/// The monster has an enemy it is trying to kill.
pub fn ai_run(ctx: &mut AiContext, self_idx: i32, dist: f32) {
    let aiflags = match ctx.get_edict(self_idx) {
        Some(e) => e.monsterinfo.aiflags,
        None => return,
    };

    // if we're going to a combat point, just proceed
    if aiflags.intersects(AI_COMBAT_POINT) {
        m_move_to_goal(ctx, self_idx, dist);
        return;
    }

    if aiflags.intersects(AI_SOUND_TARGET) {
        let enemy_idx = match ctx.get_edict(self_idx) {
            Some(e) => e.enemy,
            None => return,
        };
        if let (Some(self_ent), Some(enemy)) = (ctx.get_edict(self_idx), ctx.get_edict(enemy_idx)) {
            let v = vector_subtract(&self_ent.s.origin, &enemy.s.origin);
            if vector_length(&v) < 64.0 {
                if let Some(e) = ctx.get_edict_mut(self_idx) {
                    e.monsterinfo.aiflags |= AI_STAND_GROUND | AI_TEMP_STAND_GROUND;
                }
                crate::dispatch::call_stand(self_idx as usize, &mut ctx.edicts, &mut ctx.level);
                return;
            }
        }

        m_move_to_goal(ctx, self_idx, dist);

        if !find_target(ctx, self_idx) {
            return;
        }
    }

    if ai_checkattack(ctx, self_idx, dist) {
        return;
    }

    let attack_state = match ctx.get_edict(self_idx) {
        Some(e) => e.monsterinfo.attack_state,
        None => return,
    };
    if attack_state == AS_SLIDING {
        ai_run_slide(ctx, self_idx, dist);
        return;
    }

    if ctx.enemy_vis {
        m_move_to_goal(ctx, self_idx, dist);
        if let Some(e) = ctx.get_edict_mut(self_idx) {
            e.monsterinfo.aiflags &= !AI_LOST_SIGHT;
        }
        let enemy_idx = match ctx.get_edict(self_idx) {
            Some(e) => e.enemy,
            None => return,
        };
        if let Some(enemy) = ctx.get_edict(enemy_idx) {
            let enemy_origin = vector_copy(&enemy.s.origin);
            let level_time = ctx.level.time;
            if let Some(e) = ctx.get_edict_mut(self_idx) {
                e.monsterinfo.last_sighting = enemy_origin;
                e.monsterinfo.trail_time = level_time;
            }
        }
        return;
    }

    // coop will change to another enemy if visible
    if ctx.coop != 0.0 {
        if find_target(ctx, self_idx) {
            return;
        }
    }

    let (search_time, level_time) = (
        match ctx.get_edict(self_idx) {
            Some(e) => e.monsterinfo.search_time,
            None => return,
        },
        ctx.level.time,
    );
    if search_time != 0.0 && level_time > (search_time + 20.0) {
        m_move_to_goal(ctx, self_idx, dist);
        if let Some(e) = ctx.get_edict_mut(self_idx) {
            e.monsterinfo.search_time = 0.0;
        }
        return;
    }

    // Save goalentity, create temp goal
    let save_goal = match ctx.get_edict(self_idx) {
        Some(e) => e.goalentity,
        None => return,
    };
    let tempgoal = {
        let mut num = ctx.edicts.len();
        crate::g_utils::spawn_edict_raw(
            &mut ctx.edicts,
            ctx.game.maxclients as usize,
            &mut num,
            1024, // MAX_EDICTS
            ctx.level.time,
        ) as i32
    };
    if tempgoal < 0 {
        // Could not allocate temp entity; just move toward last sighting
        m_move_to_goal(ctx, self_idx, dist);
        return;
    }
    if let Some(e) = ctx.get_edict_mut(self_idx) {
        e.goalentity = tempgoal;
    }

    let mut new_path = false;

    let aiflags = match ctx.get_edict(self_idx) {
        Some(e) => e.monsterinfo.aiflags,
        None => {
            crate::g_utils::free_edict_raw(&mut ctx.edicts, tempgoal as usize, ctx.game.maxclients as usize, ctx.level.time);
            return;
        },
    };

    if !aiflags.intersects(AI_LOST_SIGHT) {
        // just lost sight of the player, decide where to go first
        if let Some(e) = ctx.get_edict_mut(self_idx) {
            e.monsterinfo.aiflags |= AI_LOST_SIGHT | AI_PURSUIT_LAST_SEEN;
            e.monsterinfo.aiflags &= !(AI_PURSUE_NEXT | AI_PURSUE_TEMP);
        }
        new_path = true;
    }

    let aiflags = match ctx.get_edict(self_idx) {
        Some(e) => e.monsterinfo.aiflags,
        None => {
            crate::g_utils::free_edict_raw(&mut ctx.edicts, tempgoal as usize, ctx.game.maxclients as usize, ctx.level.time);
            return;
        },
    };

    if aiflags.intersects(AI_PURSUE_NEXT) {
        let search_time_new = ctx.level.time + 5.0;
        if let Some(e) = ctx.get_edict_mut(self_idx) {
            e.monsterinfo.aiflags &= !AI_PURSUE_NEXT;
            e.monsterinfo.search_time = search_time_new;
        }

        let ai2 = match ctx.get_edict(self_idx) {
            Some(e) => e.monsterinfo.aiflags,
            None => {
                crate::g_utils::free_edict_raw(&mut ctx.edicts, tempgoal as usize, ctx.game.maxclients as usize, ctx.level.time);
                return;
            },
        };

        if ai2.intersects(AI_PURSUE_TEMP) {
            if let Some(e) = ctx.get_edict_mut(self_idx) {
                e.monsterinfo.aiflags &= !AI_PURSUE_TEMP;
                e.monsterinfo.last_sighting = e.monsterinfo.saved_goal;
            }
            new_path = true;
        } else if ai2.intersects(AI_PURSUIT_LAST_SEEN) {
            if let Some(e) = ctx.get_edict_mut(self_idx) {
                e.monsterinfo.aiflags &= !AI_PURSUIT_LAST_SEEN;
            }
            let marker = player_trail_pick_first(ctx, self_idx);
            if let Some(marker_idx) = marker {
                if let Some(m) = ctx.get_edict(marker_idx) {
                    let m_origin = m.s.origin;
                    let m_timestamp = m.timestamp;
                    let m_yaw = m.s.angles[YAW];
                    if let Some(e) = ctx.get_edict_mut(self_idx) {
                        e.monsterinfo.last_sighting = m_origin;
                        e.monsterinfo.trail_time = m_timestamp;
                        e.s.angles[YAW] = m_yaw;
                        e.ideal_yaw = m_yaw;
                    }
                }
                new_path = true;
            }
        } else {
            let marker = player_trail_pick_next(ctx, self_idx);
            if let Some(marker_idx) = marker {
                if let Some(m) = ctx.get_edict(marker_idx) {
                    let m_origin = m.s.origin;
                    let m_timestamp = m.timestamp;
                    let m_yaw = m.s.angles[YAW];
                    if let Some(e) = ctx.get_edict_mut(self_idx) {
                        e.monsterinfo.last_sighting = m_origin;
                        e.monsterinfo.trail_time = m_timestamp;
                        e.s.angles[YAW] = m_yaw;
                        e.ideal_yaw = m_yaw;
                    }
                }
                new_path = true;
            }
        }
    }

    // Check distance to last_sighting
    let (self_origin, last_sighting) = match ctx.get_edict(self_idx) {
        Some(e) => (e.s.origin, e.monsterinfo.last_sighting),
        None => {
            crate::g_utils::free_edict_raw(&mut ctx.edicts, tempgoal as usize, ctx.game.maxclients as usize, ctx.level.time);
            return;
        },
    };
    let v = vector_subtract(&self_origin, &last_sighting);
    let d1 = vector_length(&v);
    let mut actual_dist = dist;
    if d1 <= dist {
        if let Some(e) = ctx.get_edict_mut(self_idx) {
            e.monsterinfo.aiflags |= AI_PURSUE_NEXT;
        }
        actual_dist = d1;
    }

    // Set tempgoal origin to last_sighting
    let last_sighting = match ctx.get_edict(self_idx) {
        Some(e) => e.monsterinfo.last_sighting,
        None => {
            crate::g_utils::free_edict_raw(&mut ctx.edicts, tempgoal as usize, ctx.game.maxclients as usize, ctx.level.time);
            return;
        },
    };
    if let Some(tg) = ctx.get_edict_mut(tempgoal) {
        tg.s.origin = last_sighting;
    }

    if new_path {
        // Course correction: trace to last_sighting and see if we need to go around
        let (self_origin, self_mins, self_maxs) = match ctx.get_edict(self_idx) {
            Some(e) => (e.s.origin, e.mins, e.maxs),
            None => {
                crate::g_utils::free_edict_raw(&mut ctx.edicts, tempgoal as usize, ctx.game.maxclients as usize, ctx.level.time);
                return;
            },
        };

        let tr = gi_trace(&self_origin, &self_mins, &self_maxs, &last_sighting, self_idx, MASK_PLAYERSOLID);
        if tr.fraction < 1.0 {
            let goal_origin = match ctx.get_edict(tempgoal) {
                Some(e) => e.s.origin,
                None => {
                    crate::g_utils::free_edict_raw(&mut ctx.edicts, tempgoal as usize, ctx.game.maxclients as usize, ctx.level.time);
                    return;
                },
            };
            let v = vector_subtract(&goal_origin, &self_origin);
            let d1_local = vector_length(&v);
            let center = tr.fraction;
            let d2 = d1_local * ((center + 1.0) / 2.0);

            let ideal = vectoyaw(&v);
            if let Some(e) = ctx.get_edict_mut(self_idx) {
                e.s.angles[YAW] = ideal;
                e.ideal_yaw = ideal;
            }

            let angles = match ctx.get_edict(self_idx) {
                Some(e) => e.s.angles,
                None => {
                    crate::g_utils::free_edict_raw(&mut ctx.edicts, tempgoal as usize, ctx.game.maxclients as usize, ctx.level.time);
                    return;
                },
            };
            let mut v_forward = [0.0_f32; 3];
            let mut v_right = [0.0_f32; 3];
            angle_vectors(&angles, Some(&mut v_forward), Some(&mut v_right), None);

            // Check left
            let mut v_set = [d2, -16.0, 0.0];
            let mut left_target = [0.0_f32; 3];
            crate::entity_adapters::g_project_source(&self_origin, &v_set, &v_forward, &v_right, &mut left_target);
            let tr_left = gi_trace(&self_origin, &self_mins, &self_maxs, &left_target, self_idx, MASK_PLAYERSOLID);
            let left = tr_left.fraction;

            // Check right
            v_set = [d2, 16.0, 0.0];
            let mut right_target = [0.0_f32; 3];
            crate::entity_adapters::g_project_source(&self_origin, &v_set, &v_forward, &v_right, &mut right_target);
            let tr_right = gi_trace(&self_origin, &self_mins, &self_maxs, &right_target, self_idx, MASK_PLAYERSOLID);
            let right = tr_right.fraction;

            let center_adj = (d1_local * center) / d2;

            if left >= center_adj && left > right {
                if left < 1.0 {
                    v_set = [d2 * left * 0.5, -16.0, 0.0];
                    crate::entity_adapters::g_project_source(&self_origin, &v_set, &v_forward, &v_right, &mut left_target);
                }
                let last_sighting2 = match ctx.get_edict(self_idx) {
                    Some(e) => e.monsterinfo.last_sighting,
                    None => {
                        crate::g_utils::free_edict_raw(&mut ctx.edicts, tempgoal as usize, ctx.game.maxclients as usize, ctx.level.time);
                        return;
                    },
                };
                if let Some(e) = ctx.get_edict_mut(self_idx) {
                    e.monsterinfo.saved_goal = last_sighting2;
                    e.monsterinfo.aiflags |= AI_PURSUE_TEMP;
                    e.monsterinfo.last_sighting = left_target;
                }
                if let Some(tg) = ctx.get_edict_mut(tempgoal) {
                    tg.s.origin = left_target;
                }
                let goal_origin = match ctx.get_edict(tempgoal) {
                    Some(e) => e.s.origin,
                    None => {
                        crate::g_utils::free_edict_raw(&mut ctx.edicts, tempgoal as usize, ctx.game.maxclients as usize, ctx.level.time);
                        return;
                    },
                };
                let v2 = vector_subtract(&goal_origin, &self_origin);
                let new_yaw = vectoyaw(&v2);
                if let Some(e) = ctx.get_edict_mut(self_idx) {
                    e.s.angles[YAW] = new_yaw;
                    e.ideal_yaw = new_yaw;
                }
            } else if right >= center_adj && right > left {
                if right < 1.0 {
                    v_set = [d2 * right * 0.5, 16.0, 0.0];
                    crate::entity_adapters::g_project_source(&self_origin, &v_set, &v_forward, &v_right, &mut right_target);
                }
                let last_sighting2 = match ctx.get_edict(self_idx) {
                    Some(e) => e.monsterinfo.last_sighting,
                    None => {
                        crate::g_utils::free_edict_raw(&mut ctx.edicts, tempgoal as usize, ctx.game.maxclients as usize, ctx.level.time);
                        return;
                    },
                };
                if let Some(e) = ctx.get_edict_mut(self_idx) {
                    e.monsterinfo.saved_goal = last_sighting2;
                    e.monsterinfo.aiflags |= AI_PURSUE_TEMP;
                    e.monsterinfo.last_sighting = right_target;
                }
                if let Some(tg) = ctx.get_edict_mut(tempgoal) {
                    tg.s.origin = right_target;
                }
                let goal_origin = match ctx.get_edict(tempgoal) {
                    Some(e) => e.s.origin,
                    None => {
                        crate::g_utils::free_edict_raw(&mut ctx.edicts, tempgoal as usize, ctx.game.maxclients as usize, ctx.level.time);
                        return;
                    },
                };
                let v2 = vector_subtract(&goal_origin, &self_origin);
                let new_yaw = vectoyaw(&v2);
                if let Some(e) = ctx.get_edict_mut(self_idx) {
                    e.s.angles[YAW] = new_yaw;
                    e.ideal_yaw = new_yaw;
                }
            }
        }
    }

    m_move_to_goal(ctx, self_idx, actual_dist);

    crate::g_utils::free_edict_raw(&mut ctx.edicts, tempgoal as usize, ctx.game.maxclients as usize, ctx.level.time);

    if let Some(e) = ctx.get_edict_mut(self_idx) {
        e.goalentity = save_goal;
    }
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::g_local::{
        Edict, LevelLocals, GameLocals, EntityFlags,
        MELEE_DISTANCE, RANGE_MELEE, RANGE_NEAR, RANGE_MID, RANGE_FAR,
        AS_STRAIGHT, AS_MELEE, AS_MISSILE, AS_SLIDING,
    };
    use myq2_common::q_shared::{Vec3, YAW};

    // ---- Helpers ----

    fn make_edict() -> Edict {
        let mut e = Edict::default();
        e.inuse = true;
        e.health = 100;
        e.gravity = 1.0;
        e
    }

    fn make_edict_at(origin: Vec3) -> Edict {
        let mut e = make_edict();
        e.s.origin = origin;
        e
    }

    fn make_ai_context_with_edicts(edicts: Vec<Edict>) -> AiContext {
        AiContext {
            edicts,
            clients: vec![],
            level: LevelLocals::default(),
            game: GameLocals::default(),
            coop: 0.0,
            skill: 1.0,
            enemy_vis: false,
            enemy_infront: false,
            enemy_range: RANGE_FAR,
            enemy_yaw: 0.0,
        }
    }

    // ================================================================
    // range() tests
    // ================================================================

    #[test]
    fn range_melee_distance() {
        // Entities within MELEE_DISTANCE (80.0) should be RANGE_MELEE
        let self_ent = make_edict_at([0.0, 0.0, 0.0]);
        let other = make_edict_at([50.0, 0.0, 0.0]);

        assert_eq!(range(&self_ent, &other), RANGE_MELEE);
    }

    #[test]
    fn range_melee_boundary() {
        // Exactly at MELEE_DISTANCE boundary
        let self_ent = make_edict_at([0.0, 0.0, 0.0]);
        // At distance = MELEE_DISTANCE (80.0), should be RANGE_NEAR (>= MELEE_DISTANCE)
        let other = make_edict_at([MELEE_DISTANCE, 0.0, 0.0]);

        assert_eq!(range(&self_ent, &other), RANGE_NEAR);
    }

    #[test]
    fn range_melee_just_below() {
        // Just below MELEE_DISTANCE
        let self_ent = make_edict_at([0.0, 0.0, 0.0]);
        let other = make_edict_at([79.9, 0.0, 0.0]);

        assert_eq!(range(&self_ent, &other), RANGE_MELEE);
    }

    #[test]
    fn range_near() {
        // Between MELEE_DISTANCE (80) and 500 should be RANGE_NEAR
        let self_ent = make_edict_at([0.0, 0.0, 0.0]);
        let other = make_edict_at([300.0, 0.0, 0.0]);

        assert_eq!(range(&self_ent, &other), RANGE_NEAR);
    }

    #[test]
    fn range_near_boundary_at_500() {
        // At exactly 500 units, should be RANGE_MID (>= 500)
        let self_ent = make_edict_at([0.0, 0.0, 0.0]);
        let other = make_edict_at([500.0, 0.0, 0.0]);

        assert_eq!(range(&self_ent, &other), RANGE_MID);
    }

    #[test]
    fn range_mid() {
        // Between 500 and 1000 should be RANGE_MID
        let self_ent = make_edict_at([0.0, 0.0, 0.0]);
        let other = make_edict_at([750.0, 0.0, 0.0]);

        assert_eq!(range(&self_ent, &other), RANGE_MID);
    }

    #[test]
    fn range_mid_boundary_at_1000() {
        // At exactly 1000 units, should be RANGE_FAR (>= 1000)
        let self_ent = make_edict_at([0.0, 0.0, 0.0]);
        let other = make_edict_at([1000.0, 0.0, 0.0]);

        assert_eq!(range(&self_ent, &other), RANGE_FAR);
    }

    #[test]
    fn range_far() {
        // Beyond 1000 should be RANGE_FAR
        let self_ent = make_edict_at([0.0, 0.0, 0.0]);
        let other = make_edict_at([2000.0, 0.0, 0.0]);

        assert_eq!(range(&self_ent, &other), RANGE_FAR);
    }

    #[test]
    fn range_same_position() {
        // Same position should be RANGE_MELEE (distance = 0)
        let self_ent = make_edict_at([100.0, 200.0, 300.0]);
        let other = make_edict_at([100.0, 200.0, 300.0]);

        assert_eq!(range(&self_ent, &other), RANGE_MELEE);
    }

    #[test]
    fn range_diagonal_distance() {
        // Test with a diagonal distance
        // sqrt(300^2 + 400^2) = sqrt(90000 + 160000) = sqrt(250000) = 500
        let self_ent = make_edict_at([0.0, 0.0, 0.0]);
        let other = make_edict_at([300.0, 400.0, 0.0]);

        assert_eq!(range(&self_ent, &other), RANGE_MID, "Distance should be 500 (MID boundary)");
    }

    #[test]
    fn range_3d_distance() {
        // sqrt(100^2 + 100^2 + 100^2) = sqrt(30000) ~ 173.2 => RANGE_NEAR
        let self_ent = make_edict_at([0.0, 0.0, 0.0]);
        let other = make_edict_at([100.0, 100.0, 100.0]);

        assert_eq!(range(&self_ent, &other), RANGE_NEAR);
    }

    #[test]
    fn range_negative_coordinates() {
        // Distance should be the same regardless of signs
        let self_ent = make_edict_at([-500.0, -500.0, 0.0]);
        let other = make_edict_at([500.0, 500.0, 0.0]);
        // Distance = sqrt(1000^2 + 1000^2) = sqrt(2000000) ~ 1414.2 => RANGE_FAR
        assert_eq!(range(&self_ent, &other), RANGE_FAR);
    }

    #[test]
    fn range_constants_are_ordered() {
        assert!(RANGE_MELEE < RANGE_NEAR);
        assert!(RANGE_NEAR < RANGE_MID);
        assert!(RANGE_MID < RANGE_FAR);
    }

    // ================================================================
    // infront() tests
    // ================================================================

    #[test]
    fn infront_directly_ahead() {
        // Entity facing along +X, target is directly in front along +X
        let mut self_ent = make_edict_at([0.0, 0.0, 0.0]);
        // In Quake 2, angles[YAW] = 0 means facing +X
        self_ent.s.angles = [0.0, 0.0, 0.0];

        let other = make_edict_at([100.0, 0.0, 0.0]);

        assert!(infront(&self_ent, &other), "Target directly ahead should be infront");
    }

    #[test]
    fn infront_directly_behind() {
        // Entity facing along +X, target is directly behind along -X
        let mut self_ent = make_edict_at([0.0, 0.0, 0.0]);
        self_ent.s.angles = [0.0, 0.0, 0.0]; // facing +X

        let other = make_edict_at([-100.0, 0.0, 0.0]);

        assert!(!infront(&self_ent, &other), "Target directly behind should not be infront");
    }

    #[test]
    fn infront_to_the_side() {
        // Entity facing along +X, target is directly to the side (+Y)
        let mut self_ent = make_edict_at([0.0, 0.0, 0.0]);
        self_ent.s.angles = [0.0, 0.0, 0.0]; // facing +X

        let other = make_edict_at([0.0, 100.0, 0.0]);

        // dot product of forward [1,0,0] and normalized [0,1,0] = 0
        // 0 is not > 0.3, so not infront
        assert!(!infront(&self_ent, &other), "Target at 90 degrees should not be infront");
    }

    #[test]
    fn infront_slightly_off_center() {
        // Entity facing +X, target is slightly ahead and to the side
        let mut self_ent = make_edict_at([0.0, 0.0, 0.0]);
        self_ent.s.angles = [0.0, 0.0, 0.0]; // facing +X

        // target at 45 degrees: dot product = cos(45) = 0.707 > 0.3
        let other = make_edict_at([100.0, 100.0, 0.0]);

        assert!(infront(&self_ent, &other), "Target at 45 degrees should be infront");
    }

    #[test]
    fn infront_at_threshold() {
        // The threshold is dot > 0.3, which corresponds to about 72.5 degrees
        // cos(72.5deg) ~ 0.3007
        let mut self_ent = make_edict_at([0.0, 0.0, 0.0]);
        self_ent.s.angles = [0.0, 0.0, 0.0]; // facing +X

        // Create a target at approximately 72 degrees (~0.309 dot product)
        // Using forward [1,0,0], target direction needs dot ~ 0.309
        // [cos(72deg), sin(72deg), 0] = [0.309, 0.951, 0]
        let other = make_edict_at([30.9, 95.1, 0.0]);
        assert!(infront(&self_ent, &other), "Just inside 72-degree cone should be infront");

        // At ~73 degrees, dot ~ 0.292 < 0.3 => not infront
        let other2 = make_edict_at([29.2, 95.6, 0.0]);
        assert!(!infront(&self_ent, &other2), "Just outside ~73 degrees should not be infront");
    }

    #[test]
    fn infront_facing_different_yaw() {
        // Entity facing +Y (yaw = 90)
        let mut self_ent = make_edict_at([0.0, 0.0, 0.0]);
        self_ent.s.angles = [0.0, 90.0, 0.0]; // facing +Y

        // Target directly in front (along +Y)
        let in_front = make_edict_at([0.0, 100.0, 0.0]);
        assert!(infront(&self_ent, &in_front), "Target along +Y should be infront when facing +Y");

        // Target behind (along -Y)
        let behind = make_edict_at([0.0, -100.0, 0.0]);
        assert!(!infront(&self_ent, &behind), "Target along -Y should not be infront when facing +Y");
    }

    #[test]
    fn infront_facing_negative_x() {
        // Entity facing -X (yaw = 180)
        let mut self_ent = make_edict_at([0.0, 0.0, 0.0]);
        self_ent.s.angles = [0.0, 180.0, 0.0];

        let in_front = make_edict_at([-100.0, 0.0, 0.0]);
        assert!(infront(&self_ent, &in_front), "Target along -X should be infront when facing 180");

        let behind = make_edict_at([100.0, 0.0, 0.0]);
        assert!(!infront(&self_ent, &behind), "Target along +X should not be infront when facing 180");
    }

    // ================================================================
    // facing_ideal() tests
    // ================================================================

    #[test]
    fn facing_ideal_exactly_on_target() {
        let mut ent = make_edict();
        ent.s.angles[YAW] = 90.0;
        ent.ideal_yaw = 90.0;

        assert!(facing_ideal(&ent), "Should be facing ideal when yaw matches exactly");
    }

    #[test]
    fn facing_ideal_within_45_degrees() {
        let mut ent = make_edict();
        ent.s.angles[YAW] = 90.0;
        ent.ideal_yaw = 120.0; // 30 degrees off

        // delta = anglemod(90 - 120) = anglemod(-30) = 330
        // 330 >= 315, so NOT in the (45, 315) range => facing ideal
        assert!(facing_ideal(&ent), "30 degrees off should be facing ideal");
    }

    #[test]
    fn facing_ideal_barely_outside() {
        let mut ent = make_edict();
        ent.s.angles[YAW] = 0.0;
        ent.ideal_yaw = 50.0;

        // delta = anglemod(0 - 50) = anglemod(-50) = 310
        // 310 is in (45, 315) range => NOT facing ideal
        // Wait: 310 < 315 => it IS in the range (45, 315), so NOT facing ideal
        assert!(!facing_ideal(&ent), "50 degrees off should not be facing ideal");
    }

    #[test]
    fn facing_ideal_exactly_45_boundary() {
        let mut ent = make_edict();
        ent.s.angles[YAW] = 0.0;
        ent.ideal_yaw = 45.0;

        // delta = anglemod(-45) = 315
        // !(delta > 45.0 && delta < 315.0)
        // 315 is NOT < 315 => condition is false => !(false) = true => facing ideal
        assert!(facing_ideal(&ent), "Exactly 45 degrees off should be facing ideal (at boundary)");
    }

    #[test]
    fn facing_ideal_exactly_315_boundary() {
        let mut ent = make_edict();
        ent.s.angles[YAW] = 0.0;
        // ideal_yaw that gives delta = 45 after anglemod
        // anglemod(yaw - ideal) = 45 => 0 - ideal maps to 45
        // anglemod(-315) = anglemod(-315) ... let's compute differently
        // We want delta = 45: anglemod(0 - x) = 45 means x = -45 or x = 315
        ent.ideal_yaw = 315.0;

        // delta = anglemod(0 - 315) = anglemod(-315)
        // The anglemod formula: (360/65536) * (((-315)*(65536/360)) as i32 & 65535)
        // -315 * 182.044... = -57344 => as i32 = -57344 => & 65535 = 8192 => *0.00549... = 45
        let delta = anglemod(0.0 - 315.0);
        // delta should be ~45
        // !(45 > 45 && 45 < 315) = !(false && true) = !(false) = true
        assert!(facing_ideal(&ent), "315 degrees ideal should give delta=45, which is facing ideal");
    }

    #[test]
    fn facing_ideal_opposite_direction() {
        let mut ent = make_edict();
        ent.s.angles[YAW] = 0.0;
        ent.ideal_yaw = 180.0;

        // delta = anglemod(-180) = 180
        // 180 > 45 && 180 < 315 => true => !(true) = false
        assert!(!facing_ideal(&ent), "Facing opposite direction should not be facing ideal");
    }

    // ================================================================
    // AiContext helper tests
    // ================================================================

    #[test]
    fn ai_context_get_edict_valid_index() {
        let edicts = vec![make_edict(), make_edict()];
        let ctx = make_ai_context_with_edicts(edicts);

        assert!(ctx.get_edict(0).is_some());
        assert!(ctx.get_edict(1).is_some());
        assert!(ctx.get_edict(2).is_none());
        assert!(ctx.get_edict(-1).is_none());
    }

    #[test]
    fn ai_context_edict_valid() {
        let mut edicts = vec![make_edict(), make_edict()];
        edicts[1].inuse = false;
        let ctx = make_ai_context_with_edicts(edicts);

        assert!(ctx.edict_valid(0));
        assert!(!ctx.edict_valid(1), "Entity not in use should be invalid");
        assert!(!ctx.edict_valid(2), "Out of bounds should be invalid");
        assert!(!ctx.edict_valid(-1), "Negative index should be invalid");
    }

    #[test]
    fn ai_context_is_client() {
        let mut edicts = vec![make_edict(), make_edict()];
        edicts[0].client = Some(0); // has client
        edicts[1].client = None;    // no client
        let ctx = make_ai_context_with_edicts(edicts);

        assert!(ctx.is_client(0));
        assert!(!ctx.is_client(1));
        assert!(!ctx.is_client(99));
    }

    // ================================================================
    // range_idx tests
    // ================================================================

    #[test]
    fn range_idx_returns_far_for_invalid_indices() {
        let edicts = vec![make_edict_at([0.0, 0.0, 0.0])];
        let ctx = make_ai_context_with_edicts(edicts);

        // Invalid indices should return RANGE_FAR
        assert_eq!(range_idx(&ctx, 0, 99), RANGE_FAR);
        assert_eq!(range_idx(&ctx, -1, 0), RANGE_FAR);
    }

    #[test]
    fn range_idx_valid_indices() {
        let edicts = vec![
            make_edict_at([0.0, 0.0, 0.0]),
            make_edict_at([50.0, 0.0, 0.0]), // within melee
        ];
        let ctx = make_ai_context_with_edicts(edicts);

        assert_eq!(range_idx(&ctx, 0, 1), RANGE_MELEE);
    }

    // ================================================================
    // infront_idx tests
    // ================================================================

    #[test]
    fn infront_idx_valid() {
        let mut e0 = make_edict_at([0.0, 0.0, 0.0]);
        e0.s.angles = [0.0, 0.0, 0.0]; // facing +X
        let e1 = make_edict_at([100.0, 0.0, 0.0]); // ahead
        let e2 = make_edict_at([-100.0, 0.0, 0.0]); // behind

        let edicts = vec![e0, e1, e2];
        let ctx = make_ai_context_with_edicts(edicts);

        assert!(infront_idx(&ctx, 0, 1), "Target ahead should be infront");
        assert!(!infront_idx(&ctx, 0, 2), "Target behind should not be infront");
    }

    #[test]
    fn infront_idx_invalid_returns_false() {
        let edicts = vec![make_edict()];
        let ctx = make_ai_context_with_edicts(edicts);

        assert!(!infront_idx(&ctx, 0, 99));
        assert!(!infront_idx(&ctx, -1, 0));
    }

    // ================================================================
    // visible_idx tests (these call gi_trace, which is a stub in test,
    // but we can test the invalid-index early-return logic)
    // ================================================================

    #[test]
    fn visible_idx_invalid_indices_returns_false() {
        let edicts = vec![make_edict()];
        let ctx = make_ai_context_with_edicts(edicts);

        assert!(!visible_idx(&ctx, 0, 99));
        assert!(!visible_idx(&ctx, -1, 0));
        assert!(!visible_idx(&ctx, -1, -1));
    }

    // ================================================================
    // m_change_yaw logic tests (tested via the AiContext wrapper)
    // ================================================================

    #[test]
    fn m_change_yaw_already_at_ideal() {
        let mut ent = make_edict();
        ent.s.angles[YAW] = 90.0;
        ent.ideal_yaw = 90.0;
        ent.yaw_speed = 20.0;

        let mut ctx = make_ai_context_with_edicts(vec![ent]);
        m_change_yaw(&mut ctx, 0);

        assert_eq!(ctx.edicts[0].s.angles[YAW], 90.0, "Should not change when already at ideal");
    }

    #[test]
    fn m_change_yaw_small_adjustment() {
        let mut ent = make_edict();
        ent.s.angles[YAW] = 90.0;
        ent.ideal_yaw = 100.0; // 10 degrees off, less than yaw_speed
        ent.yaw_speed = 20.0;

        let mut ctx = make_ai_context_with_edicts(vec![ent]);
        m_change_yaw(&mut ctx, 0);

        // Since the difference (10) < speed (20), should snap to ideal
        let result = ctx.edicts[0].s.angles[YAW];
        // The result goes through anglemod, so compare approximately
        let expected = anglemod(100.0);
        assert!((result - expected).abs() < 1.0,
            "Should snap to ideal yaw: got {} expected {}", result, expected);
    }

    #[test]
    fn m_change_yaw_large_adjustment_positive() {
        let mut ent = make_edict();
        ent.s.angles[YAW] = 0.0;
        ent.ideal_yaw = 90.0; // 90 degrees off, more than yaw_speed
        ent.yaw_speed = 20.0;

        let mut ctx = make_ai_context_with_edicts(vec![ent]);
        m_change_yaw(&mut ctx, 0);

        // Should only turn by yaw_speed (20 degrees)
        let result = ctx.edicts[0].s.angles[YAW];
        let expected = anglemod(20.0);
        assert!((result - expected).abs() < 1.0,
            "Should turn by yaw_speed: got {} expected {}", result, expected);
    }

    #[test]
    fn m_change_yaw_large_adjustment_negative() {
        let mut ent = make_edict();
        ent.s.angles[YAW] = 90.0;
        ent.ideal_yaw = 0.0; // need to turn left 90 degrees
        ent.yaw_speed = 20.0;

        let mut ctx = make_ai_context_with_edicts(vec![ent]);
        m_change_yaw(&mut ctx, 0);

        // Should turn by -yaw_speed
        let result = ctx.edicts[0].s.angles[YAW];
        let expected = anglemod(90.0 - 20.0);
        assert!((result - expected).abs() < 1.0,
            "Should turn by -yaw_speed: got {} expected {}", result, expected);
    }

    #[test]
    fn m_change_yaw_wraps_around_360() {
        let mut ent = make_edict();
        ent.s.angles[YAW] = 350.0;
        ent.ideal_yaw = 10.0; // shortest path is +20 degrees (through 0)
        ent.yaw_speed = 30.0;

        let mut ctx = make_ai_context_with_edicts(vec![ent]);
        m_change_yaw(&mut ctx, 0);

        // mov = ideal - current = 10 - 350 = -340
        // ideal < current, so check: mov <= -180 => -340 <= -180 => true => mov += 360 => 20
        // mov > 0 and 20 < speed(30) => snap to ideal
        let result = ctx.edicts[0].s.angles[YAW];
        let expected = anglemod(10.0);
        assert!((result - expected).abs() < 1.0,
            "Should wrap around correctly: got {} expected {}", result, expected);
    }

    // ================================================================
    // ai_set_sight_client tests
    // ================================================================

    #[test]
    fn ai_set_sight_client_no_clients() {
        let mut ctx = make_ai_context_with_edicts(vec![make_edict()]);
        ctx.game.maxclients = 0;
        ctx.level.sight_client = -1;

        ai_set_sight_client(&mut ctx);

        assert_eq!(ctx.level.sight_client, -1, "With no clients, sight_client should be -1");
    }

    #[test]
    fn ai_set_sight_client_finds_alive_player() {
        // Create world entity + 2 player entities
        let mut world = make_edict();
        world.inuse = true;

        let mut player1 = make_edict();
        player1.inuse = true;
        player1.health = 100;
        player1.flags = EntityFlags::empty(); // no NOTARGET

        let mut player2 = make_edict();
        player2.inuse = true;
        player2.health = 50;
        player2.flags = EntityFlags::empty();

        let mut ctx = make_ai_context_with_edicts(vec![world, player1, player2]);
        ctx.game.maxclients = 2;
        ctx.level.sight_client = -1;

        ai_set_sight_client(&mut ctx);

        // Should find player 1 (first alive, visible client)
        assert!(ctx.level.sight_client == 1 || ctx.level.sight_client == 2,
            "Should find a valid player, got {}", ctx.level.sight_client);
    }

    #[test]
    fn ai_set_sight_client_skips_dead_players() {
        let world = make_edict();

        let mut player1 = make_edict();
        player1.inuse = true;
        player1.health = 0; // dead

        let mut player2 = make_edict();
        player2.inuse = true;
        player2.health = 100; // alive

        let mut ctx = make_ai_context_with_edicts(vec![world, player1, player2]);
        ctx.game.maxclients = 2;
        ctx.level.sight_client = -1;

        ai_set_sight_client(&mut ctx);

        assert_eq!(ctx.level.sight_client, 2, "Should skip dead player and find player 2");
    }

    #[test]
    fn ai_set_sight_client_skips_notarget() {
        let world = make_edict();

        let mut player1 = make_edict();
        player1.inuse = true;
        player1.health = 100;
        player1.flags = FL_NOTARGET; // has notarget

        let mut player2 = make_edict();
        player2.inuse = true;
        player2.health = 100;
        player2.flags = EntityFlags::empty();

        let mut ctx = make_ai_context_with_edicts(vec![world, player1, player2]);
        ctx.game.maxclients = 2;
        ctx.level.sight_client = -1;

        ai_set_sight_client(&mut ctx);

        assert_eq!(ctx.level.sight_client, 2, "Should skip NOTARGET player");
    }

    #[test]
    fn ai_set_sight_client_all_dead() {
        let world = make_edict();

        let mut player1 = make_edict();
        player1.inuse = true;
        player1.health = 0;

        let mut player2 = make_edict();
        player2.inuse = true;
        player2.health = -10;

        let mut ctx = make_ai_context_with_edicts(vec![world, player1, player2]);
        ctx.game.maxclients = 2;
        ctx.level.sight_client = -1;

        ai_set_sight_client(&mut ctx);

        assert_eq!(ctx.level.sight_client, -1, "All dead should give -1");
    }

    #[test]
    fn ai_set_sight_client_cycles() {
        let world = make_edict();

        let mut player1 = make_edict();
        player1.inuse = true;
        player1.health = 100;
        player1.flags = EntityFlags::empty();

        let mut player2 = make_edict();
        player2.inuse = true;
        player2.health = 100;
        player2.flags = EntityFlags::empty();

        let mut ctx = make_ai_context_with_edicts(vec![world, player1, player2]);
        ctx.game.maxclients = 2;

        // Start from -1: start=1, check increments to 2 first
        ctx.level.sight_client = -1;
        ai_set_sight_client(&mut ctx);
        assert_eq!(ctx.level.sight_client, 2);

        // Next call: start=2, check increments to 3 > maxclients, wraps to 1
        ai_set_sight_client(&mut ctx);
        assert_eq!(ctx.level.sight_client, 1);

        // Next call: start=1, check increments to 2
        ai_set_sight_client(&mut ctx);
        assert_eq!(ctx.level.sight_client, 2);
    }

    // ================================================================
    // range constant values match Quake 2 original
    // ================================================================

    #[test]
    fn range_constants_match_original() {
        assert_eq!(RANGE_MELEE, 0);
        assert_eq!(RANGE_NEAR, 1);
        assert_eq!(RANGE_MID, 2);
        assert_eq!(RANGE_FAR, 3);
        assert_eq!(MELEE_DISTANCE, 80.0);
    }

    // ================================================================
    // attack_finished helper tests
    // ================================================================

    #[test]
    fn attack_finished_sets_time() {
        let mut ent = make_edict();
        ent.monsterinfo.attack_finished = 0.0;

        let mut ctx = make_ai_context_with_edicts(vec![ent]);
        ctx.level.time = 10.0;

        attack_finished(&mut ctx, 0, 2.0);

        assert_eq!(ctx.edicts[0].monsterinfo.attack_finished, 12.0,
            "Should set attack_finished to level.time + time");
    }

    #[test]
    fn attack_finished_invalid_index() {
        let mut ctx = make_ai_context_with_edicts(vec![make_edict()]);
        ctx.level.time = 5.0;

        // Should not panic on invalid index
        attack_finished(&mut ctx, 99, 1.0);
        attack_finished(&mut ctx, -1, 1.0);
    }

    // ================================================================
    // attack state constants
    // ================================================================

    #[test]
    fn attack_state_constants() {
        assert_eq!(AS_STRAIGHT, 1);
        assert_eq!(AS_SLIDING, 2);
        assert_eq!(AS_MELEE, 3);
        assert_eq!(AS_MISSILE, 4);
    }

    // ================================================================
    // range with vertical distance
    // ================================================================

    #[test]
    fn range_vertical_distance() {
        // Pure vertical separation
        let self_ent = make_edict_at([0.0, 0.0, 0.0]);
        let other = make_edict_at([0.0, 0.0, 600.0]);

        assert_eq!(range(&self_ent, &other), RANGE_MID, "600 units vertical should be MID");
    }

    #[test]
    fn range_symmetry() {
        // range(a, b) should equal range(b, a) since it uses absolute distance
        let a = make_edict_at([100.0, 200.0, 50.0]);
        let b = make_edict_at([500.0, 600.0, 400.0]);

        assert_eq!(range(&a, &b), range(&b, &a), "Range should be symmetric");
    }
}
