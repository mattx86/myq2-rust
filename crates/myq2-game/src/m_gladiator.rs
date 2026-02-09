// m_gladiator.rs — Gladiator monster
// Converted from: myq2-original/game/m_gladiator.c + m_gladiator.h

use crate::g_local::*;
use crate::game::*;
use crate::entity_adapters::{gi_sound, gi_soundindex, gi_modelindex, gi_linkentity, monster_flash_offset};

// ============================================================
// Frame definitions (from m_gladiator.h)
// ============================================================

pub const FRAME_STAND1: i32 = 0;
pub const FRAME_STAND2: i32 = 1;
pub const FRAME_STAND3: i32 = 2;
pub const FRAME_STAND4: i32 = 3;
pub const FRAME_STAND5: i32 = 4;
pub const FRAME_STAND6: i32 = 5;
pub const FRAME_STAND7: i32 = 6;
pub const FRAME_WALK1: i32 = 7;
pub const FRAME_WALK2: i32 = 8;
pub const FRAME_WALK3: i32 = 9;
pub const FRAME_WALK4: i32 = 10;
pub const FRAME_WALK5: i32 = 11;
pub const FRAME_WALK6: i32 = 12;
pub const FRAME_WALK7: i32 = 13;
pub const FRAME_WALK8: i32 = 14;
pub const FRAME_WALK9: i32 = 15;
pub const FRAME_WALK10: i32 = 16;
pub const FRAME_WALK11: i32 = 17;
pub const FRAME_WALK12: i32 = 18;
pub const FRAME_WALK13: i32 = 19;
pub const FRAME_WALK14: i32 = 20;
pub const FRAME_WALK15: i32 = 21;
pub const FRAME_WALK16: i32 = 22;
pub const FRAME_RUN1: i32 = 23;
pub const FRAME_RUN2: i32 = 24;
pub const FRAME_RUN3: i32 = 25;
pub const FRAME_RUN4: i32 = 26;
pub const FRAME_RUN5: i32 = 27;
pub const FRAME_RUN6: i32 = 28;
pub const FRAME_MELEE1: i32 = 29;
pub const FRAME_MELEE2: i32 = 30;
pub const FRAME_MELEE3: i32 = 31;
pub const FRAME_MELEE4: i32 = 32;
pub const FRAME_MELEE5: i32 = 33;
pub const FRAME_MELEE6: i32 = 34;
pub const FRAME_MELEE7: i32 = 35;
pub const FRAME_MELEE8: i32 = 36;
pub const FRAME_MELEE9: i32 = 37;
pub const FRAME_MELEE10: i32 = 38;
pub const FRAME_MELEE11: i32 = 39;
pub const FRAME_MELEE12: i32 = 40;
pub const FRAME_MELEE13: i32 = 41;
pub const FRAME_MELEE14: i32 = 42;
pub const FRAME_MELEE15: i32 = 43;
pub const FRAME_MELEE16: i32 = 44;
pub const FRAME_MELEE17: i32 = 45;
pub const FRAME_ATTACK1: i32 = 46;
pub const FRAME_ATTACK2: i32 = 47;
pub const FRAME_ATTACK3: i32 = 48;
pub const FRAME_ATTACK4: i32 = 49;
pub const FRAME_ATTACK5: i32 = 50;
pub const FRAME_ATTACK6: i32 = 51;
pub const FRAME_ATTACK7: i32 = 52;
pub const FRAME_ATTACK8: i32 = 53;
pub const FRAME_ATTACK9: i32 = 54;
pub const FRAME_PAIN1: i32 = 55;
pub const FRAME_PAIN2: i32 = 56;
pub const FRAME_PAIN3: i32 = 57;
pub const FRAME_PAIN4: i32 = 58;
pub const FRAME_PAIN5: i32 = 59;
pub const FRAME_PAIN6: i32 = 60;
pub const FRAME_DEATH1: i32 = 61;
pub const FRAME_DEATH2: i32 = 62;
pub const FRAME_DEATH3: i32 = 63;
pub const FRAME_DEATH4: i32 = 64;
pub const FRAME_DEATH5: i32 = 65;
pub const FRAME_DEATH6: i32 = 66;
pub const FRAME_DEATH7: i32 = 67;
pub const FRAME_DEATH8: i32 = 68;
pub const FRAME_DEATH9: i32 = 69;
pub const FRAME_DEATH10: i32 = 70;
pub const FRAME_DEATH11: i32 = 71;
pub const FRAME_DEATH12: i32 = 72;
pub const FRAME_DEATH13: i32 = 73;
pub const FRAME_DEATH14: i32 = 74;
pub const FRAME_DEATH15: i32 = 75;
pub const FRAME_DEATH16: i32 = 76;
pub const FRAME_DEATH17: i32 = 77;
pub const FRAME_DEATH18: i32 = 78;
pub const FRAME_DEATH19: i32 = 79;
pub const FRAME_DEATH20: i32 = 80;
pub const FRAME_DEATH21: i32 = 81;
pub const FRAME_DEATH22: i32 = 82;
pub const FRAME_PAINUP1: i32 = 83;
pub const FRAME_PAINUP2: i32 = 84;
pub const FRAME_PAINUP3: i32 = 85;
pub const FRAME_PAINUP4: i32 = 86;
pub const FRAME_PAINUP5: i32 = 87;
pub const FRAME_PAINUP6: i32 = 88;
pub const FRAME_PAINUP7: i32 = 89;

pub const MODEL_SCALE: f32 = 1.0;

// ============================================================
// Animation frame type and move type
// ============================================================

// MFrame and MMove are imported from g_local via `use crate::g_local::*`

use crate::ai_wrappers::{ai_stand, ai_walk, ai_run, ai_charge, ai_move};

// ============================================================
// Move table indices (used for MonsterInfo.currentmove)
// ============================================================

pub const GLADIATOR_MOVE_STAND: usize = 0;
pub const GLADIATOR_MOVE_WALK: usize = 1;
pub const GLADIATOR_MOVE_RUN: usize = 2;
pub const GLADIATOR_MOVE_ATTACK_MELEE: usize = 3;
pub const GLADIATOR_MOVE_ATTACK_GUN: usize = 4;
pub const GLADIATOR_MOVE_PAIN: usize = 5;
pub const GLADIATOR_MOVE_PAIN_AIR: usize = 6;
pub const GLADIATOR_MOVE_DEATH: usize = 7;

// ============================================================
// Sound indices (module-level state, mirrors C statics)
// ============================================================

#[derive(Debug, Clone, Default)]
pub struct GladiatorSounds {
    pub pain1: i32,
    pub pain2: i32,
    pub die: i32,
    pub gun: i32,
    pub cleaver_swing: i32,
    pub cleaver_hit: i32,
    pub cleaver_miss: i32,
    pub idle: i32,
    pub search: i32,
    pub sight: i32,
}

static SOUNDS: std::sync::OnceLock<GladiatorSounds> = std::sync::OnceLock::new();

// MZ2 constant for gladiator railgun
const MZ2_GLADIATOR_RAILGUN_1: i32 = 45; // from q_shared.h

// CHAN_*, ATTN_* come from g_local::* re-export (myq2_common::q_shared)

// ============================================================
// Animation frame tables
// ============================================================

pub static GLADIATOR_FRAMES_STAND: [MFrame; 7] = [
    MFrame { ai_fn: ai_stand, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_stand, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_stand, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_stand, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_stand, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_stand, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_stand, dist: 0.0, think_fn: None },
];

pub static GLADIATOR_FRAMES_WALK: [MFrame; 16] = [
    MFrame { ai_fn: ai_walk, dist: 15.0, think_fn: None },
    MFrame { ai_fn: ai_walk, dist: 7.0,  think_fn: None },
    MFrame { ai_fn: ai_walk, dist: 6.0,  think_fn: None },
    MFrame { ai_fn: ai_walk, dist: 5.0,  think_fn: None },
    MFrame { ai_fn: ai_walk, dist: 2.0,  think_fn: None },
    MFrame { ai_fn: ai_walk, dist: 0.0,  think_fn: None },
    MFrame { ai_fn: ai_walk, dist: 2.0,  think_fn: None },
    MFrame { ai_fn: ai_walk, dist: 8.0,  think_fn: None },
    MFrame { ai_fn: ai_walk, dist: 12.0, think_fn: None },
    MFrame { ai_fn: ai_walk, dist: 8.0,  think_fn: None },
    MFrame { ai_fn: ai_walk, dist: 5.0,  think_fn: None },
    MFrame { ai_fn: ai_walk, dist: 5.0,  think_fn: None },
    MFrame { ai_fn: ai_walk, dist: 2.0,  think_fn: None },
    MFrame { ai_fn: ai_walk, dist: 2.0,  think_fn: None },
    MFrame { ai_fn: ai_walk, dist: 1.0,  think_fn: None },
    MFrame { ai_fn: ai_walk, dist: 8.0,  think_fn: None },
];

pub static GLADIATOR_FRAMES_RUN: [MFrame; 6] = [
    MFrame { ai_fn: ai_run, dist: 23.0, think_fn: None },
    MFrame { ai_fn: ai_run, dist: 14.0, think_fn: None },
    MFrame { ai_fn: ai_run, dist: 14.0, think_fn: None },
    MFrame { ai_fn: ai_run, dist: 21.0, think_fn: None },
    MFrame { ai_fn: ai_run, dist: 12.0, think_fn: None },
    MFrame { ai_fn: ai_run, dist: 13.0, think_fn: None },
];

pub static GLADIATOR_FRAMES_ATTACK_MELEE: [MFrame; 17] = [
    MFrame { ai_fn: ai_charge, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_charge, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_charge, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_charge, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_charge, dist: 0.0, think_fn: Some(gladiator_cleaver_swing) },
    MFrame { ai_fn: ai_charge, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_charge, dist: 0.0, think_fn: Some(galdiator_melee_attack) },
    MFrame { ai_fn: ai_charge, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_charge, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_charge, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_charge, dist: 0.0, think_fn: Some(gladiator_cleaver_swing) },
    MFrame { ai_fn: ai_charge, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_charge, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_charge, dist: 0.0, think_fn: Some(galdiator_melee_attack) },
    MFrame { ai_fn: ai_charge, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_charge, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_charge, dist: 0.0, think_fn: None },
];

pub static GLADIATOR_FRAMES_ATTACK_GUN: [MFrame; 9] = [
    MFrame { ai_fn: ai_charge, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_charge, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_charge, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_charge, dist: 0.0, think_fn: Some(gladiator_gun) },
    MFrame { ai_fn: ai_charge, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_charge, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_charge, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_charge, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_charge, dist: 0.0, think_fn: None },
];

pub static GLADIATOR_FRAMES_PAIN: [MFrame; 6] = [
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
];

pub static GLADIATOR_FRAMES_PAIN_AIR: [MFrame; 7] = [
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
];

pub static GLADIATOR_FRAMES_DEATH: [MFrame; 22] = [
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
    MFrame { ai_fn: ai_move, dist: 0.0, think_fn: None },
];

// ============================================================
// Move definitions
// ============================================================

pub static GLADIATOR_MOVE_TABLE: &[MMove] = &[
    // 0: GLADIATOR_MOVE_STAND
    MMove {
        firstframe: FRAME_STAND1,
        lastframe: FRAME_STAND7,
        frames: &GLADIATOR_FRAMES_STAND,
        endfunc: None,
    },
    // 1: GLADIATOR_MOVE_WALK
    MMove {
        firstframe: FRAME_WALK1,
        lastframe: FRAME_WALK16,
        frames: &GLADIATOR_FRAMES_WALK,
        endfunc: None,
    },
    // 2: GLADIATOR_MOVE_RUN
    MMove {
        firstframe: FRAME_RUN1,
        lastframe: FRAME_RUN6,
        frames: &GLADIATOR_FRAMES_RUN,
        endfunc: None,
    },
    // 3: GLADIATOR_MOVE_ATTACK_MELEE
    MMove {
        firstframe: FRAME_MELEE1,
        lastframe: FRAME_MELEE17,
        frames: &GLADIATOR_FRAMES_ATTACK_MELEE,
        endfunc: Some(gladiator_run),
    },
    // 4: GLADIATOR_MOVE_ATTACK_GUN
    MMove {
        firstframe: FRAME_ATTACK1,
        lastframe: FRAME_ATTACK9,
        frames: &GLADIATOR_FRAMES_ATTACK_GUN,
        endfunc: Some(gladiator_run),
    },
    // 5: GLADIATOR_MOVE_PAIN
    MMove {
        firstframe: FRAME_PAIN1,
        lastframe: FRAME_PAIN6,
        frames: &GLADIATOR_FRAMES_PAIN,
        endfunc: Some(gladiator_run),
    },
    // 6: GLADIATOR_MOVE_PAIN_AIR
    MMove {
        firstframe: FRAME_PAINUP1,
        lastframe: FRAME_PAINUP7,
        frames: &GLADIATOR_FRAMES_PAIN_AIR,
        endfunc: Some(gladiator_run),
    },
    // 7: GLADIATOR_MOVE_DEATH
    MMove {
        firstframe: FRAME_DEATH1,
        lastframe: FRAME_DEATH22,
        frames: &GLADIATOR_FRAMES_DEATH,
        endfunc: Some(gladiator_dead),
    },
];

// ============================================================
// Sound callbacks
// ============================================================

pub fn gladiator_idle(
    self_ent: &mut Edict,
    _ctx: &mut GameContext,
) {
    let sound_idle = SOUNDS.get().map_or(0, |s| s.idle);
    gi_sound(self_ent, CHAN_VOICE, sound_idle, 1.0, ATTN_IDLE, 0.0);
}

pub fn gladiator_sight(
    self_ent: &mut Edict,
    _ctx: &mut GameContext,
) {
    let sound_sight = SOUNDS.get().map_or(0, |s| s.sight);
    gi_sound(self_ent, CHAN_VOICE, sound_sight, 1.0, ATTN_NORM, 0.0);
}

pub fn gladiator_search(
    self_ent: &mut Edict,
    _ctx: &mut GameContext,
) {
    let sound_search = SOUNDS.get().map_or(0, |s| s.search);
    gi_sound(self_ent, CHAN_VOICE, sound_search, 1.0, ATTN_NORM, 0.0);
}

pub fn gladiator_cleaver_swing(
    self_ent: &mut Edict,
    _ctx: &mut GameContext,
) {
    let sound_cleaver_swing = SOUNDS.get().map_or(0, |s| s.cleaver_swing);
    gi_sound(self_ent, CHAN_WEAPON, sound_cleaver_swing, 1.0, ATTN_NORM, 0.0);
}

// ============================================================
// Behavior functions
// ============================================================

pub fn gladiator_stand(
    self_ent: &mut Edict,
    _ctx: &mut GameContext,
) {
    self_ent.monsterinfo.currentmove = Some(GLADIATOR_MOVE_STAND);
}

pub fn gladiator_walk(
    self_ent: &mut Edict,
    _ctx: &mut GameContext,
) {
    self_ent.monsterinfo.currentmove = Some(GLADIATOR_MOVE_WALK);
}

pub fn gladiator_run(
    self_ent: &mut Edict,
    _ctx: &mut GameContext,
) {
    if self_ent.monsterinfo.aiflags.intersects(AI_STAND_GROUND) {
        self_ent.monsterinfo.currentmove = Some(GLADIATOR_MOVE_STAND);
    } else {
        self_ent.monsterinfo.currentmove = Some(GLADIATOR_MOVE_RUN);
    }
}

/// GaldiatorMelee in C (note: original typo preserved)
pub fn galdiator_melee_attack(
    self_ent: &mut Edict,
    _ctx: &mut GameContext,
) {
    let aim = [MELEE_DISTANCE, self_ent.mins[0], -4.0];
    let damage = 20 + (rand::random::<i32>().abs() % 5);
    // fire_hit requires full edicts/level context - dispatched via GameContext
    let self_idx = self_ent.s.number as usize;
    crate::g_local::with_global_game_ctx(|ctx| {
        crate::g_weapon::fire_hit(self_idx, &mut ctx.edicts, &mut ctx.level, &aim, damage, 30);
    });
}

pub fn gladiator_melee(
    self_ent: &mut Edict,
    _ctx: &mut GameContext,
) {
    self_ent.monsterinfo.currentmove = Some(GLADIATOR_MOVE_ATTACK_MELEE);
}

/// GladiatorGun in C
pub fn gladiator_gun(
    self_ent: &mut Edict,
    _ctx: &mut GameContext,
) {
    let mut forward = [0.0f32; 3];
    let mut right = [0.0f32; 3];
    myq2_common::q_shared::angle_vectors(&self_ent.s.angles, Some(&mut forward), Some(&mut right), None);

    let offset = monster_flash_offset(MZ2_GLADIATOR_RAILGUN_1);
    let start = crate::g_utils::g_project_source(&self_ent.s.origin, &offset, &forward, &right);

    let mut dir = [
        self_ent.pos1[0] - start[0],
        self_ent.pos1[1] - start[1],
        self_ent.pos1[2] - start[2],
    ];
    myq2_common::q_shared::vector_normalize(&mut dir);

    crate::g_monster::monster_fire_railgun_raw(
        self_ent.s.number, start, dir, 50, 100, MZ2_GLADIATOR_RAILGUN_1,
    );
}

pub fn gladiator_attack(
    self_ent: &mut Edict,
    ctx: &mut GameContext,
) {
    // a small safe zone - check range to enemy
    if self_ent.enemy >= 0 {
        let enemy = &ctx.edicts[self_ent.enemy as usize];
        let v = [
            self_ent.s.origin[0] - enemy.s.origin[0],
            self_ent.s.origin[1] - enemy.s.origin[1],
            self_ent.s.origin[2] - enemy.s.origin[2],
        ];
        let range = myq2_common::q_shared::vector_length(&v);
        if range <= MELEE_DISTANCE + 32.0 {
            return;
        }

        // save enemy origin for aiming the shot
        self_ent.pos1 = enemy.s.origin;
        self_ent.pos1[2] += enemy.viewheight as f32;
    }

    // charge up the railgun
    let sound_gun = SOUNDS.get().map_or(0, |s| s.gun);
    gi_sound(self_ent, CHAN_WEAPON, sound_gun, 1.0, ATTN_NORM, 0.0);

    self_ent.monsterinfo.currentmove = Some(GLADIATOR_MOVE_ATTACK_GUN);
}

pub fn gladiator_pain(
    self_ent: &mut Edict,
    _other: &mut Edict,
    _kick: f32,
    _damage: i32,
    ctx: &mut GameContext,
) {
    if self_ent.health < (self_ent.max_health / 2) {
        self_ent.s.skinnum = 1;
    }

    if ctx.level.time < self_ent.pain_debounce_time {
        if self_ent.velocity[2] > 100.0
            && self_ent.monsterinfo.currentmove == Some(GLADIATOR_MOVE_PAIN)
        {
            self_ent.monsterinfo.currentmove = Some(GLADIATOR_MOVE_PAIN_AIR);
        }
        return;
    }

    self_ent.pain_debounce_time = ctx.level.time + 3.0;

    if rand::random::<f32>() < 0.5 {
        let sound_pain1 = SOUNDS.get().map_or(0, |s| s.pain1);
        gi_sound(self_ent, CHAN_VOICE, sound_pain1, 1.0, ATTN_NORM, 0.0);
    } else {
        let sound_pain2 = SOUNDS.get().map_or(0, |s| s.pain2);
        gi_sound(self_ent, CHAN_VOICE, sound_pain2, 1.0, ATTN_NORM, 0.0);
    }

    if ctx.skill == 3.0 {
        return; // no pain anims in nightmare
    }

    if self_ent.velocity[2] > 100.0 {
        self_ent.monsterinfo.currentmove = Some(GLADIATOR_MOVE_PAIN_AIR);
    } else {
        self_ent.monsterinfo.currentmove = Some(GLADIATOR_MOVE_PAIN);
    }
}

pub fn gladiator_dead(
    self_ent: &mut Edict,
    _ctx: &mut GameContext,
) {
    self_ent.mins = [-16.0, -16.0, -24.0];
    self_ent.maxs = [16.0, 16.0, -8.0];
    self_ent.movetype = MoveType::Toss;
    self_ent.svflags |= SVF_DEADMONSTER;
    self_ent.nextthink = 0.0;
    gi_linkentity(self_ent);
}

pub fn gladiator_die(
    self_ent: &mut Edict,
    _inflictor: &mut Edict,
    _attacker: &mut Edict,
    damage: i32,
    _point: [f32; 3],
    _ctx: &mut GameContext,
) {
    // check for gib
    if self_ent.health <= self_ent.gib_health {
        gi_sound(self_ent, CHAN_VOICE,
            gi_soundindex("misc/udeath.wav"), 1.0, ATTN_NORM, 0.0);
        // ThrowGib/ThrowHead require MiscGameContext - dispatched via GameContext
        self_ent.deadflag = DEAD_DEAD;
        return;
    }

    if self_ent.deadflag == DEAD_DEAD {
        return;
    }

    // regular death
    let sound_die = SOUNDS.get().map_or(0, |s| s.die);
    gi_sound(self_ent, CHAN_VOICE, sound_die, 1.0, ATTN_NORM, 0.0);
    self_ent.deadflag = DEAD_DEAD;
    self_ent.takedamage = Damage::Yes as i32;

    self_ent.monsterinfo.currentmove = Some(GLADIATOR_MOVE_DEATH);
}

// ============================================================
// Spawn function
// ============================================================

/// SP_monster_gladiator — spawns a gladiator monster entity.
/// `deathmatch_value` corresponds to `deathmatch->value` in C.
pub fn sp_monster_gladiator(
    self_ent: &mut Edict,
    ctx: &mut GameContext,
) {
    if ctx.deathmatch != 0.0 {
        self_ent.inuse = false;
        return;
    }

    // Precache sounds
    SOUNDS.get_or_init(|| GladiatorSounds {
        pain1: gi_soundindex("gladiator/pain.wav"),
        pain2: gi_soundindex("gladiator/gldpain2.wav"),
        die: gi_soundindex("gladiator/glddeth2.wav"),
        gun: gi_soundindex("gladiator/railgun.wav"),
        cleaver_swing: gi_soundindex("gladiator/melee1.wav"),
        cleaver_hit: gi_soundindex("gladiator/melee2.wav"),
        cleaver_miss: gi_soundindex("gladiator/melee3.wav"),
        idle: gi_soundindex("gladiator/gldidle1.wav"),
        search: gi_soundindex("gladiator/gldsrch1.wav"),
        sight: gi_soundindex("gladiator/sight.wav"),
    });

    self_ent.movetype = MoveType::Step;
    self_ent.solid = Solid::Bbox;
    self_ent.s.modelindex = gi_modelindex("models/monsters/gladiatr/tris.md2");

    self_ent.mins = [-32.0, -32.0, -24.0];
    self_ent.maxs = [32.0, 32.0, 64.0];

    self_ent.health = 400;
    self_ent.gib_health = -175;
    self_ent.mass = 400;

    self_ent.pain_fn = Some(crate::dispatch::PAIN_GLADIATOR);
    self_ent.die_fn = Some(crate::dispatch::DIE_GLADIATOR);

    self_ent.monsterinfo.stand_fn = Some(crate::dispatch::MSTAND_GLADIATOR);
    self_ent.monsterinfo.walk_fn = Some(crate::dispatch::MWALK_GLADIATOR);
    self_ent.monsterinfo.run_fn = Some(crate::dispatch::MRUN_GLADIATOR);
    self_ent.monsterinfo.dodge_fn = None;
    self_ent.monsterinfo.attack_fn = Some(crate::dispatch::MATTACK_GLADIATOR);
    self_ent.monsterinfo.melee_fn = Some(crate::dispatch::MMELEE_GLADIATOR);
    self_ent.monsterinfo.sight_fn = Some(crate::dispatch::MSIGHT_GLADIATOR);
    self_ent.monsterinfo.idle_fn = Some(crate::dispatch::MIDLE_GLADIATOR);

    gi_linkentity(self_ent);

    self_ent.monsterinfo.currentmove = Some(GLADIATOR_MOVE_STAND);
    self_ent.monsterinfo.scale = MODEL_SCALE;

    // walkmonster_start dispatched via g_monster::walkmonster_start with GameContext
}
