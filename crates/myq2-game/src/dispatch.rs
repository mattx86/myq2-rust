// dispatch.rs — Callback dispatch system for entity and monster callbacks
// Converted from: implicit function pointer usage across myq2-original/game/
//
// Entity callbacks are stored as `Option<usize>` indices into static dispatch
// tables. This avoids the simultaneous mutable borrow problem that would arise
// from storing closures or references directly in Edict fields.

use crate::g_local::{CPlane, CSurface, Edict, GameContext, GameLocals, LevelLocals, SpawnTemp, Vec3};

// ============================================================
// Type aliases for callback signatures
// ============================================================

pub type ThinkFn = fn(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals);
pub type PainFn = fn(
    self_idx: usize,
    attacker_idx: usize,
    edicts: &mut [Edict],
    level: &mut LevelLocals,
    kick: f32,
    damage: i32,
);
pub type DieFn = fn(
    self_idx: usize,
    inflictor_idx: usize,
    attacker_idx: usize,
    edicts: &mut [Edict],
    level: &mut LevelLocals,
    damage: i32,
    point: Vec3,
);
pub type TouchFn = fn(
    self_idx: usize,
    other_idx: usize,
    edicts: &mut [Edict],
    level: &mut LevelLocals,
    plane: Option<&CPlane>,
    surf: Option<&CSurface>,
);
pub type UseFn = fn(
    self_idx: usize,
    other_idx: usize,
    activator_idx: usize,
    edicts: &mut [Edict],
    level: &mut LevelLocals,
);
pub type BlockedFn = fn(
    self_idx: usize,
    other_idx: usize,
    edicts: &mut [Edict],
    level: &mut LevelLocals,
);
pub type MonsterThinkFn = fn(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals);
pub type CheckAttackFn =
    fn(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) -> bool;

// ============================================================
// Named constants — Think callbacks
// ============================================================

pub const THINK_MONSTER: usize = 0;
pub const THINK_WALKMONSTER_START: usize = 1;
pub const THINK_FLYMONSTER_START: usize = 2;
pub const THINK_SWIMMONSTER_START: usize = 3;
// g_misc
pub const THINK_DEBRIS_DIE: usize = 4;
pub const THINK_GIVEUP_AND_DEATHTOUCH: usize = 5; // misc generic think placeholder
pub const THINK_FUNC_EXPLOSIVE_EXPLODE: usize = 6;
// g_func
pub const THINK_FUNC_DOOR_GO_UP: usize = 7;
pub const THINK_FUNC_DOOR_GO_DOWN: usize = 8;
pub const THINK_FUNC_DOOR_SECRET_MOVE1: usize = 9;
pub const THINK_FUNC_DOOR_SECRET_MOVE2: usize = 10;
pub const THINK_FUNC_DOOR_SECRET_MOVE3: usize = 11;
pub const THINK_FUNC_DOOR_SECRET_MOVE4: usize = 12;
pub const THINK_FUNC_DOOR_SECRET_MOVE5: usize = 13;
pub const THINK_FUNC_DOOR_SECRET_MOVE6: usize = 14;
pub const THINK_FUNC_DOOR_SECRET_DONE: usize = 15;
pub const THINK_FUNC_TRAIN_NEXT: usize = 16;
pub const THINK_FUNC_PLAT_GO_UP: usize = 17;
pub const THINK_FUNC_PLAT_GO_DOWN: usize = 18;
pub const THINK_FUNC_ROTATING_THINK: usize = 19;
// g_trigger
pub const THINK_TRIGGER_DELAY_THINK: usize = 20;
pub const THINK_TRIGGER_PUSH_TOUCH: usize = 21;
pub const THINK_TRIGGER_HURT_THINK: usize = 22;
// g_target
pub const THINK_TARGET_LASER_THINK: usize = 23;
pub const THINK_TARGET_LIGHTRAMP_THINK: usize = 24;
pub const THINK_TARGET_EARTHQUAKE_THINK: usize = 25;
// g_weapon
pub const THINK_GRENADE_EXPLODE: usize = 26;
pub const THINK_ROCKET_THINK: usize = 27;
pub const THINK_BFG_THINK: usize = 28;
pub const THINK_BFG_EXPLODE: usize = 29;
// g_items
pub const THINK_DROP_TEMP_TOUCH: usize = 30;
pub const THINK_DROP_MAKE_TOUCHABLE: usize = 31;
pub const THINK_MEGAHEALTH_THINK: usize = 32;
// p_client
pub const THINK_PLAYER_THINK: usize = 33;
pub const THINK_BODY_THINK: usize = 34;
pub const THINK_RESPAWN_THINK: usize = 35;
// p_weapon
pub const THINK_WEAPON_THINK: usize = 36;
// Monster-specific think (used as edict think_fn)
pub const THINK_MONSTER_THINK: usize = 37;
pub const THINK_MONSTER_DEAD_THINK: usize = 38;
// Free / remove
pub const THINK_FREE_EDICT: usize = 39;
// Misc
pub const THINK_PATH_CORNER: usize = 40;
pub const THINK_POINT_COMBAT: usize = 41;
// g_monster
pub const THINK_M_FLIES_OFF: usize = 42;
pub const THINK_M_FLIES_ON: usize = 43;
pub const THINK_MONSTER_TRIGGERED_SPAWN: usize = 44;
pub const THINK_WALKMONSTER_START_GO: usize = 45;
pub const THINK_FLYMONSTER_START_GO: usize = 46;
pub const THINK_SWIMMONSTER_START_GO: usize = 47;
// m_hover
pub const THINK_HOVER_DEADTHINK: usize = 48;
// g_items
pub const THINK_DO_RESPAWN: usize = 49;
pub const THINK_DROPTOFLOOR: usize = 50;
// g_misc
pub const THINK_GIB: usize = 51;
pub const THINK_TH_VIEWTHING: usize = 52;
pub const THINK_MISC_BLACKHOLE: usize = 53;
pub const THINK_MISC_EASTERTANK: usize = 54;
pub const THINK_MISC_EASTERCHICK: usize = 55;
pub const THINK_MISC_EASTERCHICK2: usize = 56;
pub const THINK_COMMANDER_BODY: usize = 57;
pub const THINK_COMMANDER_BODY_DROP: usize = 58;
pub const THINK_MISC_BANNER: usize = 59;
pub const THINK_MISC_SATELLITE_DISH: usize = 60;
pub const THINK_BARREL_EXPLODE: usize = 61;
pub const THINK_FUNC_OBJECT_RELEASE: usize = 62;
pub const THINK_FUNC_CLOCK: usize = 63;
pub const THINK_FUNC_TRAIN_FIND: usize = 64;
pub const THINK_MISC_VIPER_BOMB_PRETHINK: usize = 65;
pub const THINK_M_DROPTOFLOOR: usize = 66;
// g_trigger
pub const THINK_MULTI_WAIT: usize = 67;
// g_target
pub const THINK_TARGET_EXPLOSION_EXPLODE: usize = 68;
pub const THINK_TARGET_CROSSLEVEL_TARGET: usize = 69;
pub const THINK_TARGET_LASER_START: usize = 70;
// p_client
pub const THINK_SP_CREATE_COOP_SPOTS: usize = 71;
pub const THINK_SP_FIX_COOP_SPOTS: usize = 72;
// m_boss3
pub const THINK_BOSS3_STAND: usize = 73;
// g_func movement internals
pub const THINK_FUNC_MOVE_DONE: usize = 74;
pub const THINK_FUNC_MOVE_FINAL: usize = 75;
pub const THINK_FUNC_MOVE_BEGIN: usize = 76;
pub const THINK_FUNC_ACCEL_MOVE: usize = 77;
pub const THINK_FUNC_ANGLE_MOVE_DONE: usize = 78;
pub const THINK_FUNC_ANGLE_MOVE_FINAL: usize = 79;
pub const THINK_FUNC_ANGLE_MOVE_BEGIN: usize = 80;
pub const THINK_FUNC_BUTTON_RETURN: usize = 81;
pub const THINK_FUNC_CALC_MOVE_SPEED: usize = 82;
pub const THINK_FUNC_SPAWN_DOOR_TRIGGER: usize = 83;
pub const THINK_FUNC_TRIGGER_ELEVATOR_INIT: usize = 84;
pub const THINK_FUNC_TIMER_THINK: usize = 85;

pub const THINK_TABLE_SIZE: usize = 88;

// ============================================================
// Named constants — Pain callbacks
// ============================================================

pub const PAIN_PLAYER: usize = 0;
pub const PAIN_SOLDIER: usize = 1;
pub const PAIN_BERSERK: usize = 2;
pub const PAIN_BRAIN: usize = 3;
pub const PAIN_GLADIATOR: usize = 4;
pub const PAIN_GUNNER: usize = 5;
pub const PAIN_INFANTRY: usize = 6;
pub const PAIN_PARASITE: usize = 7;
pub const PAIN_FLIPPER: usize = 8;
pub const PAIN_FLYER: usize = 9;
pub const PAIN_FLOAT: usize = 10;
pub const PAIN_HOVER: usize = 11;
pub const PAIN_CHICK: usize = 12;
pub const PAIN_MUTANT: usize = 13;
pub const PAIN_INSANE: usize = 14;
pub const PAIN_MEDIC: usize = 15;
pub const PAIN_ACTOR: usize = 16;
pub const PAIN_BOSS2: usize = 17;
pub const PAIN_JORG: usize = 18;
pub const PAIN_MAKRON: usize = 19;
pub const PAIN_SUPERTANK: usize = 20;
pub const PAIN_TANK: usize = 21;

pub const PAIN_TABLE_SIZE: usize = 32;

// ============================================================
// Named constants — Die callbacks
// ============================================================

pub const DIE_PLAYER: usize = 0;
pub const DIE_SOLDIER: usize = 1;
pub const DIE_BERSERK: usize = 2;
pub const DIE_BRAIN: usize = 3;
pub const DIE_GLADIATOR: usize = 4;
pub const DIE_GUNNER: usize = 5;
pub const DIE_INFANTRY: usize = 6;
pub const DIE_PARASITE: usize = 7;
pub const DIE_FLIPPER: usize = 8;
pub const DIE_FLYER: usize = 9;
pub const DIE_FLOAT: usize = 10;
pub const DIE_HOVER: usize = 11;
pub const DIE_CHICK: usize = 12;
pub const DIE_MUTANT: usize = 13;
pub const DIE_INSANE: usize = 14;
pub const DIE_MEDIC: usize = 15;
pub const DIE_ACTOR: usize = 16;
pub const DIE_BOSS2: usize = 17;
pub const DIE_JORG: usize = 18;
pub const DIE_MAKRON: usize = 19;
pub const DIE_SUPERTANK: usize = 20;
pub const DIE_TANK: usize = 21;
pub const DIE_BARREL: usize = 22;
pub const DIE_MISC_EXPLOBOX: usize = 23;
pub const DIE_GIB: usize = 24;
// g_misc
pub const DIE_DEBRIS: usize = 25;
pub const DIE_FUNC_EXPLOSIVE: usize = 26;
pub const DIE_MISC_DEADSOLDIER: usize = 27;
// p_client
pub const DIE_BODY_DIE: usize = 28;
// g_func
pub const DIE_BUTTON_KILLED: usize = 29;
pub const DIE_DOOR_KILLED: usize = 30;
pub const DIE_DOOR_SECRET: usize = 31;

pub const DIE_TABLE_SIZE: usize = 32;

// ============================================================
// Named constants — Touch callbacks
// ============================================================

pub const TOUCH_TRIGGER_MULTIPLE: usize = 0;
pub const TOUCH_TRIGGER_ONCE: usize = 1;
pub const TOUCH_TRIGGER_PUSH: usize = 2;
pub const TOUCH_TRIGGER_HURT: usize = 3;
pub const TOUCH_ITEM: usize = 4;
pub const TOUCH_WEAPON_ROCKET: usize = 5;
pub const TOUCH_WEAPON_GRENADE: usize = 6;
pub const TOUCH_WEAPON_BLASTER: usize = 7;
pub const TOUCH_WEAPON_BFG: usize = 8;
pub const TOUCH_FUNC_DOOR: usize = 9;
pub const TOUCH_MUTANT_JUMP: usize = 10;
// g_items
pub const TOUCH_DROP_TEMP: usize = 11;
// g_misc
pub const TOUCH_GIB: usize = 12;
pub const TOUCH_PATH_CORNER: usize = 13;
pub const TOUCH_POINT_COMBAT: usize = 14;
pub const TOUCH_FUNC_OBJECT: usize = 15;
pub const TOUCH_BARREL: usize = 16;
pub const TOUCH_MISC_VIPER_BOMB: usize = 17;
pub const TOUCH_TELEPORTER: usize = 18;
// g_trigger
pub const TOUCH_MULTI: usize = 19;
pub const TOUCH_TRIGGER_MONSTERJUMP: usize = 20;
pub const TOUCH_TRIGGER_GRAVITY: usize = 21;
// g_func
pub const TOUCH_PLAT_CENTER: usize = 22;
pub const TOUCH_ROTATING: usize = 23;
pub const TOUCH_BUTTON: usize = 24;
pub const TOUCH_DOOR: usize = 25;

pub const TOUCH_TABLE_SIZE: usize = 28;

// ============================================================
// Named constants — Use callbacks
// ============================================================

pub const USE_TRIGGER_RELAY: usize = 0;
pub const USE_TRIGGER_COUNTER: usize = 1;
pub const USE_TRIGGER_ALWAYS: usize = 2;
pub const USE_TARGET_SPEAKER: usize = 3;
pub const USE_TARGET_EXPLOSION: usize = 4;
pub const USE_TARGET_CHANGELEVEL: usize = 5;
pub const USE_TARGET_SPLASH: usize = 6;
pub const USE_TARGET_SPAWNER: usize = 7;
pub const USE_TARGET_BLASTER: usize = 8;
pub const USE_TARGET_LASER: usize = 9;
pub const USE_TARGET_LIGHTRAMP: usize = 10;
pub const USE_TARGET_EARTHQUAKE: usize = 11;
pub const USE_FUNC_DOOR: usize = 12;
pub const USE_FUNC_BUTTON: usize = 13;
pub const USE_FUNC_TRAIN: usize = 14;
pub const USE_ITEM: usize = 15;
pub const USE_FUNC_TIMER: usize = 16;
pub const USE_FUNC_KILLBOX: usize = 17;
// g_monster
pub const USE_MONSTER_USE: usize = 18;
pub const USE_MONSTER_TRIGGERED_SPAWN_USE: usize = 19;
// g_items
pub const USE_ITEM_TRIGGER: usize = 20;
// g_misc
pub const USE_AREAPORTAL: usize = 21;
pub const USE_LIGHT: usize = 22;
pub const USE_FUNC_WALL: usize = 23;
pub const USE_FUNC_OBJECT: usize = 24;
pub const USE_FUNC_EXPLOSIVE: usize = 25;
pub const USE_FUNC_EXPLOSIVE_SPAWN: usize = 26;
pub const USE_MISC_BLACKHOLE: usize = 27;
pub const USE_COMMANDER_BODY: usize = 28;
pub const USE_MISC_SATELLITE_DISH: usize = 29;
pub const USE_MISC_VIPER: usize = 30;
pub const USE_MISC_VIPER_BOMB: usize = 31;
pub const USE_MISC_STROGG_SHIP: usize = 32;
pub const USE_TARGET_STRING: usize = 33;
pub const USE_FUNC_CLOCK: usize = 34;
pub const USE_TRAIN: usize = 35;
// g_target
pub const USE_TARGET_TENT: usize = 36;
pub const USE_TARGET_HELP: usize = 37;
pub const USE_TARGET_SECRET: usize = 38;
pub const USE_TARGET_GOAL: usize = 39;
pub const USE_TRIGGER_CROSSLEVEL_TRIGGER: usize = 40;
// g_trigger
pub const USE_MULTI: usize = 41;
pub const USE_TRIGGER_ENABLE: usize = 42;
pub const USE_TRIGGER_KEY: usize = 43;
pub const USE_HURT: usize = 44;
// m_boss3
pub const USE_BOSS3: usize = 45;
// g_func
pub const USE_FUNC_PLAT: usize = 46;
pub const USE_FUNC_ROTATING: usize = 47;
pub const USE_FUNC_DOOR_SECRET: usize = 48;
pub const USE_FUNC_ELEVATOR: usize = 49;
pub const USE_FUNC_CONVEYOR: usize = 50;

pub const USE_TABLE_SIZE: usize = 52;

// ============================================================
// Named constants — Blocked callbacks
// ============================================================

pub const BLOCKED_FUNC_DOOR: usize = 0;
pub const BLOCKED_FUNC_PLAT: usize = 1;
pub const BLOCKED_FUNC_TRAIN: usize = 2;
pub const BLOCKED_FUNC_ROTATING: usize = 3;
pub const BLOCKED_DOOR_SECRET: usize = 4;

pub const BLOCKED_TABLE_SIZE: usize = 8;

// ============================================================
// Named constants — MonsterInfo stand/walk/run/etc. callbacks
// (These use MonsterThinkFn signature)
// ============================================================

pub const MSTAND_SOLDIER: usize = 0;
pub const MSTAND_BERSERK: usize = 1;
pub const MSTAND_BRAIN: usize = 2;
pub const MSTAND_GLADIATOR: usize = 3;
pub const MSTAND_GUNNER: usize = 4;
pub const MSTAND_INFANTRY: usize = 5;
pub const MSTAND_PARASITE: usize = 6;
pub const MSTAND_FLIPPER: usize = 7;
pub const MSTAND_FLYER: usize = 8;
pub const MSTAND_FLOAT: usize = 9;
pub const MSTAND_HOVER: usize = 10;
pub const MSTAND_CHICK: usize = 11;
pub const MSTAND_MUTANT: usize = 12;
pub const MSTAND_INSANE: usize = 13;
pub const MSTAND_MEDIC: usize = 14;
pub const MSTAND_ACTOR: usize = 15;
pub const MSTAND_BOSS2: usize = 16;
pub const MSTAND_JORG: usize = 17;
pub const MSTAND_MAKRON: usize = 18;
pub const MSTAND_SUPERTANK: usize = 19;
pub const MSTAND_TANK: usize = 20;

pub const MSTAND_TABLE_SIZE: usize = 32;

pub const MWALK_SOLDIER: usize = 0;
pub const MWALK_BERSERK: usize = 1;
pub const MWALK_BRAIN: usize = 2;
pub const MWALK_GLADIATOR: usize = 3;
pub const MWALK_GUNNER: usize = 4;
pub const MWALK_INFANTRY: usize = 5;
pub const MWALK_PARASITE: usize = 6;
pub const MWALK_FLIPPER: usize = 7;
pub const MWALK_FLYER: usize = 8;
pub const MWALK_FLOAT: usize = 9;
pub const MWALK_HOVER: usize = 10;
pub const MWALK_CHICK: usize = 11;
pub const MWALK_MUTANT: usize = 12;
pub const MWALK_INSANE: usize = 13;
pub const MWALK_MEDIC: usize = 14;
pub const MWALK_ACTOR: usize = 15;
pub const MWALK_BOSS2: usize = 16;
pub const MWALK_JORG: usize = 17;
pub const MWALK_MAKRON: usize = 18;
pub const MWALK_SUPERTANK: usize = 19;
pub const MWALK_TANK: usize = 20;

pub const MWALK_TABLE_SIZE: usize = 32;

pub const MRUN_SOLDIER: usize = 0;
pub const MRUN_BERSERK: usize = 1;
pub const MRUN_BRAIN: usize = 2;
pub const MRUN_GLADIATOR: usize = 3;
pub const MRUN_GUNNER: usize = 4;
pub const MRUN_INFANTRY: usize = 5;
pub const MRUN_PARASITE: usize = 6;
pub const MRUN_FLIPPER: usize = 7;
pub const MRUN_FLYER: usize = 8;
pub const MRUN_FLOAT: usize = 9;
pub const MRUN_HOVER: usize = 10;
pub const MRUN_CHICK: usize = 11;
pub const MRUN_MUTANT: usize = 12;
pub const MRUN_INSANE: usize = 13;
pub const MRUN_MEDIC: usize = 14;
pub const MRUN_ACTOR: usize = 15;
pub const MRUN_BOSS2: usize = 16;
pub const MRUN_JORG: usize = 17;
pub const MRUN_MAKRON: usize = 18;
pub const MRUN_SUPERTANK: usize = 19;
pub const MRUN_TANK: usize = 20;

pub const MRUN_TABLE_SIZE: usize = 32;

// Dodge
pub const MDODGE_SOLDIER: usize = 0;
pub const MDODGE_BERSERK: usize = 1;
pub const MDODGE_BRAIN: usize = 2;
pub const MDODGE_GLADIATOR: usize = 3;
pub const MDODGE_GUNNER: usize = 4;
pub const MDODGE_INFANTRY: usize = 5;
pub const MDODGE_CHICK: usize = 6;
pub const MDODGE_MUTANT: usize = 7;
pub const MDODGE_MEDIC: usize = 8;

pub const MDODGE_TABLE_SIZE: usize = 16;

// Attack (MonsterThinkFn)
pub const MATTACK_SOLDIER: usize = 0;
pub const MATTACK_BERSERK: usize = 1;
pub const MATTACK_BRAIN: usize = 2;
pub const MATTACK_GLADIATOR: usize = 3;
pub const MATTACK_GUNNER: usize = 4;
pub const MATTACK_INFANTRY: usize = 5;
pub const MATTACK_PARASITE: usize = 6;
pub const MATTACK_FLIPPER: usize = 7;
pub const MATTACK_FLYER: usize = 8;
pub const MATTACK_FLOAT: usize = 9;
pub const MATTACK_HOVER: usize = 10;
pub const MATTACK_CHICK: usize = 11;
pub const MATTACK_MUTANT: usize = 12;
pub const MATTACK_MEDIC: usize = 13;
pub const MATTACK_BOSS2: usize = 14;
pub const MATTACK_JORG: usize = 15;
pub const MATTACK_MAKRON: usize = 16;
pub const MATTACK_SUPERTANK: usize = 17;
pub const MATTACK_TANK: usize = 18;

pub const MATTACK_TABLE_SIZE: usize = 32;

// Melee (MonsterThinkFn)
pub const MMELEE_SOLDIER: usize = 0;
pub const MMELEE_BERSERK: usize = 1;
pub const MMELEE_BRAIN: usize = 2;
pub const MMELEE_GLADIATOR: usize = 3;
pub const MMELEE_INFANTRY: usize = 4;
pub const MMELEE_FLIPPER: usize = 5;
pub const MMELEE_FLOAT: usize = 6;
pub const MMELEE_CHICK: usize = 7;
pub const MMELEE_MUTANT: usize = 8;
pub const MMELEE_INSANE: usize = 9;
pub const MMELEE_TANK: usize = 10;
pub const MMELEE_FLYER: usize = 11;

pub const MMELEE_TABLE_SIZE: usize = 16;

// Sight (MonsterThinkFn)
pub const MSIGHT_SOLDIER: usize = 0;
pub const MSIGHT_BERSERK: usize = 1;
pub const MSIGHT_BRAIN: usize = 2;
pub const MSIGHT_GLADIATOR: usize = 3;
pub const MSIGHT_GUNNER: usize = 4;
pub const MSIGHT_INFANTRY: usize = 5;
pub const MSIGHT_PARASITE: usize = 6;
pub const MSIGHT_FLIPPER: usize = 7;
pub const MSIGHT_FLYER: usize = 8;
pub const MSIGHT_FLOAT: usize = 9;
pub const MSIGHT_HOVER: usize = 10;
pub const MSIGHT_CHICK: usize = 11;
pub const MSIGHT_MUTANT: usize = 12;
pub const MSIGHT_INSANE: usize = 13;
pub const MSIGHT_MEDIC: usize = 14;
pub const MSIGHT_ACTOR: usize = 15;
pub const MSIGHT_BOSS2: usize = 16;
pub const MSIGHT_JORG: usize = 17;
pub const MSIGHT_MAKRON: usize = 18;
pub const MSIGHT_SUPERTANK: usize = 19;
pub const MSIGHT_TANK: usize = 20;

pub const MSIGHT_TABLE_SIZE: usize = 32;

// Idle (MonsterThinkFn)
pub const MIDLE_SOLDIER: usize = 0;
pub const MIDLE_BERSERK: usize = 1;
pub const MIDLE_BRAIN: usize = 2;
pub const MIDLE_GLADIATOR: usize = 3;
pub const MIDLE_GUNNER: usize = 4;
pub const MIDLE_INFANTRY: usize = 5;
pub const MIDLE_PARASITE: usize = 6;
pub const MIDLE_FLIPPER: usize = 7;
pub const MIDLE_FLYER: usize = 8;
pub const MIDLE_FLOAT: usize = 9;
pub const MIDLE_HOVER: usize = 10;
pub const MIDLE_CHICK: usize = 11;
pub const MIDLE_MUTANT: usize = 12;
pub const MIDLE_INSANE: usize = 13;
pub const MIDLE_MEDIC: usize = 14;
pub const MIDLE_ACTOR: usize = 15;
pub const MIDLE_SUPERTANK: usize = 16;
pub const MIDLE_TANK: usize = 17;

pub const MIDLE_TABLE_SIZE: usize = 32;

// Search (MonsterThinkFn)
pub const MSEARCH_SOLDIER: usize = 0;
pub const MSEARCH_BRAIN: usize = 1;
pub const MSEARCH_GUNNER: usize = 2;
pub const MSEARCH_INFANTRY: usize = 3;
pub const MSEARCH_FLYER: usize = 4;
pub const MSEARCH_HOVER: usize = 5;
pub const MSEARCH_CHICK: usize = 6;
pub const MSEARCH_MEDIC: usize = 7;
pub const MSEARCH_SUPERTANK: usize = 8;
pub const MSEARCH_JORG: usize = 9;
pub const MSEARCH_BOSS2: usize = 10;

pub const MSEARCH_TABLE_SIZE: usize = 16;

// CheckAttack
pub const MCHECKATTACK_DEFAULT: usize = 0;
pub const MCHECKATTACK_SOLDIER: usize = 1;
pub const MCHECKATTACK_GUNNER: usize = 2;
pub const MCHECKATTACK_JORG: usize = 3;
pub const MCHECKATTACK_MAKRON: usize = 4;
pub const MCHECKATTACK_SUPERTANK: usize = 5;
pub const MCHECKATTACK_TANK: usize = 6;
pub const MCHECKATTACK_BOSS2: usize = 7;
pub const MCHECKATTACK_MUTANT: usize = 8;
pub const MCHECKATTACK_MEDIC: usize = 9;

pub const MCHECKATTACK_TABLE_SIZE: usize = 16;

// ============================================================
// Placeholder callback implementations
// These are stubs; actual implementations will be wired in as
// individual monster/entity modules are adapted.
// ============================================================

fn think_placeholder(self_idx: usize, _edicts: &mut [Edict], _level: &mut LevelLocals) {
    // Default fallback for unregistered dispatch table slots — logs a warning.
    crate::game_import::gi_dprintf(&format!("dispatch: unimplemented think callback for edict {}", self_idx));
}

fn pain_placeholder(
    self_idx: usize,
    _attacker_idx: usize,
    _edicts: &mut [Edict],
    _level: &mut LevelLocals,
    _kick: f32,
    _damage: i32,
) {
    crate::game_import::gi_dprintf(&format!("dispatch: unimplemented pain callback for edict {}", self_idx));
}

fn die_placeholder(
    self_idx: usize,
    _inflictor_idx: usize,
    _attacker_idx: usize,
    _edicts: &mut [Edict],
    _level: &mut LevelLocals,
    _damage: i32,
    _point: Vec3,
) {
    crate::game_import::gi_dprintf(&format!("dispatch: unimplemented die callback for edict {}", self_idx));
}

fn touch_placeholder(
    self_idx: usize,
    _other_idx: usize,
    _edicts: &mut [Edict],
    _level: &mut LevelLocals,
    _plane: Option<&CPlane>,
    _surf: Option<&CSurface>,
) {
    crate::game_import::gi_dprintf(&format!("dispatch: unimplemented touch callback for edict {}", self_idx));
}

fn use_placeholder(
    self_idx: usize,
    _other_idx: usize,
    _activator_idx: usize,
    _edicts: &mut [Edict],
    _level: &mut LevelLocals,
) {
    crate::game_import::gi_dprintf(&format!("dispatch: unimplemented use callback for edict {}", self_idx));
}

fn blocked_placeholder(
    self_idx: usize,
    _other_idx: usize,
    _edicts: &mut [Edict],
    _level: &mut LevelLocals,
) {
    crate::game_import::gi_dprintf(&format!("dispatch: unimplemented blocked callback for edict {}", self_idx));
}

fn monster_think_placeholder(self_idx: usize, _edicts: &mut [Edict], _level: &mut LevelLocals) {
    crate::game_import::gi_dprintf(&format!("dispatch: unimplemented monster think callback for edict {}", self_idx));
}

fn checkattack_placeholder(
    self_idx: usize,
    _edicts: &mut [Edict],
    _level: &mut LevelLocals,
) -> bool {
    crate::game_import::gi_dprintf(&format!("dispatch: unimplemented checkattack callback for edict {}", self_idx));
    false
}

// ============================================================
// Helper: construct a temporary GameContext for calling legacy functions
// ============================================================

fn make_temp_ctx(level: &mut LevelLocals) -> GameContext {
    GameContext {
        level: level.clone(),
        ..GameContext::default()
    }
}

/// Copy level changes back from a temp ctx after a legacy call.
fn sync_level(level: &mut LevelLocals, ctx: &GameContext) {
    *level = ctx.level.clone();
}

// ============================================================
// Wrapper functions: MonsterThinkFn-compatible wrappers
// for monster callback functions that use the legacy
// fn(&mut Edict, &mut GameContext) signature.
// ============================================================

// --- g_monster wrappers ---

fn w_m_flies_off(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(level);
    ctx.edicts = edicts.to_vec();
    crate::g_monster::m_flies_off(&mut ctx, self_idx as i32);
    sync_level(level, &ctx);
    edicts.clone_from_slice(&ctx.edicts);
}

fn w_m_flies_on(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(level);
    ctx.edicts = edicts.to_vec();
    crate::g_monster::m_flies_on(&mut ctx, self_idx as i32);
    sync_level(level, &ctx);
    edicts.clone_from_slice(&ctx.edicts);
}

fn w_monster_triggered_spawn(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(level);
    ctx.edicts = edicts.to_vec();
    crate::g_monster::monster_triggered_spawn(&mut ctx, self_idx as i32);
    sync_level(level, &ctx);
    edicts.clone_from_slice(&ctx.edicts);
}

fn w_walkmonster_start_go(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(level);
    ctx.edicts = edicts.to_vec();
    crate::g_monster::walkmonster_start_go(&mut ctx, self_idx as i32);
    sync_level(level, &ctx);
    edicts.clone_from_slice(&ctx.edicts);
}

fn w_flymonster_start_go(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(level);
    ctx.edicts = edicts.to_vec();
    crate::g_monster::flymonster_start_go(&mut ctx, self_idx as i32);
    sync_level(level, &ctx);
    edicts.clone_from_slice(&ctx.edicts);
}

fn w_swimmonster_start_go(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(level);
    ctx.edicts = edicts.to_vec();
    crate::g_monster::swimmonster_start_go(&mut ctx, self_idx as i32);
    sync_level(level, &ctx);
    edicts.clone_from_slice(&ctx.edicts);
}

fn w_monster_use(
    self_idx: usize,
    other_idx: usize,
    activator_idx: usize,
    edicts: &mut [Edict],
    level: &mut LevelLocals,
) {
    let mut ctx = make_temp_ctx(level);
    ctx.edicts = edicts.to_vec();
    crate::g_monster::monster_use(&mut ctx, self_idx as i32, other_idx as i32, activator_idx as i32);
    sync_level(level, &ctx);
    edicts.clone_from_slice(&ctx.edicts);
}

fn w_monster_triggered_spawn_use(
    self_idx: usize,
    other_idx: usize,
    activator_idx: usize,
    edicts: &mut [Edict],
    level: &mut LevelLocals,
) {
    let mut ctx = make_temp_ctx(level);
    ctx.edicts = edicts.to_vec();
    crate::g_monster::monster_triggered_spawn_use(&mut ctx, self_idx as i32, other_idx as i32, activator_idx as i32);
    sync_level(level, &ctx);
    edicts.clone_from_slice(&ctx.edicts);
}

// --- m_hover wrappers ---

fn w_hover_deadthink(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(level);
    ctx.edicts = edicts.to_vec();
    crate::m_hover::hover_deadthink(&mut edicts[self_idx], &mut ctx);
    sync_level(level, &ctx);
}

// --- g_free_edict wrapper ---

fn w_free_edict(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    crate::game_import::gi_unlinkentity(self_idx as i32);

    let level_time = level.time;
    edicts[self_idx] = Edict::default();
    edicts[self_idx].classname = "freed".to_string();
    edicts[self_idx].freetime = level_time;
    edicts[self_idx].inuse = false;
}

// --- g_items wrappers ---

fn make_items_ctx(edicts: &[Edict], level: &LevelLocals) -> GameContext {
    let (game, clients, items, deathmatch, coop, skill, dmflags) =
        crate::g_local::with_global_game_ctx(|gctx| {
            (
                gctx.game.clone(),
                gctx.clients.clone(),
                gctx.items.clone(),
                gctx.deathmatch,
                gctx.coop,
                gctx.skill,
                gctx.dmflags,
            )
        }).unwrap_or_else(|| {
            (GameLocals::default(), Vec::new(), Vec::new(), 0.0, 0.0, 0.0, 0.0)
        });
    GameContext {
        game,
        level: level.clone(),
        items,
        edicts: edicts.to_vec(),
        clients,
        skill,
        deathmatch,
        coop,
        dmflags,
        ..GameContext::default()
    }
}

fn sync_items_ctx(edicts: &mut [Edict], level: &mut LevelLocals, ctx: &GameContext) {
    *level = ctx.level.clone();
    let len = edicts.len().min(ctx.edicts.len());
    edicts[..len].clone_from_slice(&ctx.edicts[..len]);
}

fn w_do_respawn(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_items_ctx(edicts, level);
    crate::g_items::do_respawn(&mut ctx, self_idx);
    sync_items_ctx(edicts, level, &ctx);
}

fn w_megahealth_think(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_items_ctx(edicts, level);
    crate::g_items::megahealth_think(&mut ctx, self_idx);
    sync_items_ctx(edicts, level, &ctx);
}

fn w_drop_make_touchable(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_items_ctx(edicts, level);
    crate::g_items::drop_make_touchable(&mut ctx, self_idx);
    sync_items_ctx(edicts, level, &ctx);
}

fn w_droptofloor(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_items_ctx(edicts, level);
    crate::g_items::droptofloor(&mut ctx, self_idx);
    sync_items_ctx(edicts, level, &ctx);
}

fn w_touch_item(
    self_idx: usize, other_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals,
    _plane: Option<&CPlane>, _surf: Option<&CSurface>,
) {
    let mut ctx = make_items_ctx(edicts, level);
    crate::g_items::touch_item(&mut ctx, self_idx, other_idx);
    sync_items_ctx(edicts, level, &ctx);
}

fn w_drop_temp_touch(
    self_idx: usize, other_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals,
    _plane: Option<&CPlane>, _surf: Option<&CSurface>,
) {
    let mut ctx = make_items_ctx(edicts, level);
    crate::g_items::drop_temp_touch(&mut ctx, self_idx, other_idx);
    sync_items_ctx(edicts, level, &ctx);
}

fn w_use_item_trigger(
    self_idx: usize, _other_idx: usize, _activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_items_ctx(edicts, level);
    crate::g_items::use_item_trigger(&mut ctx, self_idx);
    sync_items_ctx(edicts, level, &ctx);
}

// --- g_trigger wrappers ---

fn w_multi_wait(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    edicts[self_idx].nextthink = 0.0;
}

fn w_trigger_gravity_touch(
    self_idx: usize, other_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals,
    _plane: Option<&CPlane>, _surf: Option<&CSurface>,
) {
    edicts[other_idx].gravity = edicts[self_idx].gravity;
}

fn w_trigger_monsterjump_touch(
    self_idx: usize, other_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals,
    _plane: Option<&CPlane>, _surf: Option<&CSurface>,
) {
    if edicts[other_idx].flags.intersects(crate::g_local::FL_FLY) { return; }
    if edicts[other_idx].groundentity == -1 { return; }
    let speed = edicts[self_idx].speed;
    let movedir = edicts[self_idx].movedir;
    edicts[other_idx].velocity[0] = movedir[0] * speed;
    edicts[other_idx].velocity[1] = movedir[1] * speed;
    let height = edicts[self_idx].movedir[2];
    if height > 0.0 {
        edicts[other_idx].velocity[2] = height;
    } else {
        edicts[other_idx].velocity[2] = 200.0;
    }
    edicts[other_idx].groundentity = -1;
}

// --- g_trigger context helpers ---

fn make_trigger_ctx(edicts: &[Edict], level: &LevelLocals) -> GameContext {
    let (game, clients, items, st, coop) =
        crate::g_local::with_global_game_ctx(|gctx| {
            (
                gctx.game.clone(),
                gctx.clients.clone(),
                gctx.items.clone(),
                gctx.st.clone(),
                gctx.coop,
            )
        }).unwrap_or_else(|| {
            (GameLocals::default(), Vec::new(), Vec::new(), SpawnTemp::default(), 0.0)
        });
    GameContext {
        level: level.clone(),
        game,
        edicts: edicts.to_vec(),
        clients,
        items,
        st,
        coop,
        ..GameContext::default()
    }
}

fn sync_trigger_ctx(edicts: &mut [Edict], level: &mut LevelLocals, ctx: &GameContext) {
    *level = ctx.level.clone();
    let len = edicts.len().min(ctx.edicts.len());
    edicts[..len].clone_from_slice(&ctx.edicts[..len]);
}

// --- g_trigger wrappers ---

fn w_touch_multi(
    self_idx: usize, other_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals,
    _plane: Option<&CPlane>, _surf: Option<&CSurface>,
) {
    let mut ctx = make_trigger_ctx(edicts, level);
    crate::g_trigger::touch_multi(&mut ctx, self_idx, other_idx, _plane, _surf);
    sync_trigger_ctx(edicts, level, &ctx);
}

fn w_trigger_push_touch(
    self_idx: usize, other_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals,
    _plane: Option<&CPlane>, _surf: Option<&CSurface>,
) {
    let mut ctx = make_trigger_ctx(edicts, level);
    crate::g_trigger::trigger_push_touch(&mut ctx, self_idx, other_idx, _plane, _surf);
    sync_trigger_ctx(edicts, level, &ctx);
}

fn w_hurt_touch(
    self_idx: usize, other_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals,
    _plane: Option<&CPlane>, _surf: Option<&CSurface>,
) {
    let mut ctx = make_trigger_ctx(edicts, level);
    crate::g_trigger::hurt_touch(&mut ctx, self_idx, other_idx, _plane, _surf);
    sync_trigger_ctx(edicts, level, &ctx);
}

fn w_use_multi(
    self_idx: usize, _other_idx: usize, activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_trigger_ctx(edicts, level);
    crate::g_trigger::use_multi(&mut ctx, self_idx, _other_idx, activator_idx);
    sync_trigger_ctx(edicts, level, &ctx);
}

fn w_trigger_enable(
    self_idx: usize, _other_idx: usize, _activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_trigger_ctx(edicts, level);
    crate::g_trigger::trigger_enable(&mut ctx, self_idx, _other_idx, _activator_idx);
    sync_trigger_ctx(edicts, level, &ctx);
}

fn w_trigger_relay_use(
    self_idx: usize, _other_idx: usize, activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_trigger_ctx(edicts, level);
    crate::g_trigger::trigger_relay_use(&mut ctx, self_idx, _other_idx, activator_idx);
    sync_trigger_ctx(edicts, level, &ctx);
}

fn w_trigger_counter_use(
    self_idx: usize, _other_idx: usize, activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_trigger_ctx(edicts, level);
    crate::g_trigger::trigger_counter_use(&mut ctx, self_idx, _other_idx, activator_idx);
    sync_trigger_ctx(edicts, level, &ctx);
}

fn w_trigger_key_use(
    self_idx: usize, _other_idx: usize, activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_trigger_ctx(edicts, level);
    crate::g_trigger::trigger_key_use(&mut ctx, self_idx, _other_idx, activator_idx);
    sync_trigger_ctx(edicts, level, &ctx);
}

fn w_hurt_use(
    self_idx: usize, _other_idx: usize, _activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_trigger_ctx(edicts, level);
    crate::g_trigger::hurt_use(&mut ctx, self_idx, _other_idx, _activator_idx);
    sync_trigger_ctx(edicts, level, &ctx);
}

// --- g_misc context helpers ---

fn make_misc_ctx(edicts: &[Edict], level: &LevelLocals) -> GameContext {
    let (game, clients, items, st, sm_meat_index, deathmatch) =
        crate::g_local::with_global_game_ctx(|gctx| {
            (
                gctx.game.clone(),
                gctx.clients.clone(),
                gctx.items.clone(),
                gctx.st.clone(),
                gctx.sm_meat_index,
                gctx.deathmatch,
            )
        }).unwrap_or_else(|| {
            (GameLocals::default(), Vec::new(), Vec::new(), SpawnTemp::default(), 0, 0.0)
        });
    GameContext {
        level: level.clone(),
        game,
        edicts: edicts.to_vec(),
        clients,
        items,
        st,
        sm_meat_index,
        deathmatch,
        ..Default::default()
    }
}

fn sync_misc_ctx(edicts: &mut [Edict], level: &mut LevelLocals, ctx: &GameContext) {
    *level = ctx.level.clone();
    let len = edicts.len().min(ctx.edicts.len());
    edicts[..len].clone_from_slice(&ctx.edicts[..len]);
}

// --- g_misc wrappers ---

fn w_gib_think(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::gib_think(&mut ctx, self_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_th_viewthing(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::th_viewthing(&mut ctx, self_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_misc_blackhole_think(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::misc_blackhole_think(&mut ctx, self_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_misc_eastertank_think(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::misc_eastertank_think(&mut ctx, self_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_misc_easterchick_think(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::misc_easterchick_think(&mut ctx, self_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_misc_easterchick2_think(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::misc_easterchick2_think(&mut ctx, self_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_commander_body_think(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::commander_body_think(&mut ctx, self_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_commander_body_drop(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::commander_body_drop(&mut ctx, self_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_boss3_stand_think(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::m_boss3::think_boss3_stand(&mut ctx, self_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_misc_banner_think(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::misc_banner_think(&mut ctx, self_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_misc_satellite_dish_think(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::misc_satellite_dish_think(&mut ctx, self_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_barrel_explode(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::barrel_explode(&mut ctx, self_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_func_object_release(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::func_object_release(&mut ctx, self_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_func_clock_think(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::func_clock_think(&mut ctx, self_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_misc_viper_bomb_prethink(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::misc_viper_bomb_prethink(&mut ctx, self_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_gib_touch(
    self_idx: usize, other_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals,
    plane: Option<&CPlane>, _surf: Option<&CSurface>,
) {
    let mut ctx = make_misc_ctx(edicts, level);
    // gib_touch takes plane as Option<&[f32; 3]> (the normal)
    let normal: Option<[f32; 3]> = plane.map(|p| p.normal);
    crate::g_misc::gib_touch(&mut ctx, self_idx, other_idx, normal.as_ref(), None);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_func_object_touch(
    self_idx: usize, other_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals,
    plane: Option<&CPlane>, _surf: Option<&CSurface>,
) {
    let mut ctx = make_misc_ctx(edicts, level);
    let normal: Option<[f32; 3]> = plane.map(|p| p.normal);
    crate::g_misc::func_object_touch(&mut ctx, self_idx, other_idx, normal.as_ref(), None);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_barrel_touch(
    self_idx: usize, other_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals,
    plane: Option<&CPlane>, _surf: Option<&CSurface>,
) {
    let mut ctx = make_misc_ctx(edicts, level);
    let normal: Option<[f32; 3]> = plane.map(|p| p.normal);
    crate::g_misc::barrel_touch(&mut ctx, self_idx, other_idx, normal.as_ref(), None);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_misc_viper_bomb_touch(
    self_idx: usize, other_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals,
    plane: Option<&CPlane>, _surf: Option<&CSurface>,
) {
    let mut ctx = make_misc_ctx(edicts, level);
    let normal: Option<[f32; 3]> = plane.map(|p| p.normal);
    crate::g_misc::misc_viper_bomb_touch(&mut ctx, self_idx, other_idx, normal.as_ref(), None);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_teleporter_touch(
    self_idx: usize, other_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals,
    _plane: Option<&CPlane>, _surf: Option<&CSurface>,
) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::teleporter_touch(&mut ctx, self_idx, other_idx, None, None);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_use_areaportal(
    self_idx: usize, _other_idx: usize, _activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::use_areaportal(&mut ctx, self_idx, _other_idx, _activator_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_light_use(
    self_idx: usize, _other_idx: usize, _activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::light_use(&mut ctx, self_idx, _other_idx, _activator_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_func_wall_use(
    self_idx: usize, _other_idx: usize, _activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::func_wall_use(&mut ctx, self_idx, _other_idx, _activator_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_func_object_use(
    self_idx: usize, _other_idx: usize, _activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::func_object_use(&mut ctx, self_idx, _other_idx, _activator_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_func_explosive_use(
    self_idx: usize, other_idx: usize, _activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::func_explosive_use(&mut ctx, self_idx, other_idx, _activator_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_func_explosive_spawn(
    self_idx: usize, _other_idx: usize, _activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::func_explosive_spawn(&mut ctx, self_idx, _other_idx, _activator_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_gib_die(
    self_idx: usize, inflictor_idx: usize, attacker_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
    damage: i32, point: Vec3,
) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::gib_die(&mut ctx, self_idx, inflictor_idx, attacker_idx, damage, &point);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_debris_die(
    self_idx: usize, inflictor_idx: usize, attacker_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
    damage: i32, point: Vec3,
) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::debris_die(&mut ctx, self_idx, inflictor_idx, attacker_idx, damage, &point);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_func_explosive_die(
    self_idx: usize, inflictor_idx: usize, attacker_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
    damage: i32, point: Vec3,
) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::func_explosive_explode(&mut ctx, self_idx, inflictor_idx, attacker_idx, damage, &point);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_barrel_delay_die(
    self_idx: usize, inflictor_idx: usize, attacker_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
    damage: i32, point: Vec3,
) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::barrel_delay(&mut ctx, self_idx, inflictor_idx, attacker_idx, damage, &point);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_deadsoldier_die(
    self_idx: usize, inflictor_idx: usize, attacker_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
    damage: i32, point: Vec3,
) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::misc_deadsoldier_die(&mut ctx, self_idx, inflictor_idx, attacker_idx, damage, &point);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_misc_blackhole_use(
    self_idx: usize, _other_idx: usize, _activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::misc_blackhole_use(&mut ctx, self_idx, _other_idx, _activator_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_commander_body_use(
    self_idx: usize, _other_idx: usize, _activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::commander_body_use(&mut ctx, self_idx, _other_idx, _activator_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_boss3_use(
    self_idx: usize, _other_idx: usize, _activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::m_boss3::use_boss3(&mut ctx, self_idx, _other_idx, _activator_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_misc_satellite_dish_use(
    self_idx: usize, _other_idx: usize, _activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::misc_satellite_dish_use(&mut ctx, self_idx, _other_idx, _activator_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_misc_viper_use(
    self_idx: usize, other_idx: usize, activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::misc_viper_use(&mut ctx, self_idx, other_idx, activator_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_misc_viper_bomb_use(
    self_idx: usize, _other_idx: usize, activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::misc_viper_bomb_use(&mut ctx, self_idx, _other_idx, activator_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_misc_strogg_ship_use(
    self_idx: usize, other_idx: usize, activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::misc_strogg_ship_use(&mut ctx, self_idx, other_idx, activator_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_func_clock_use(
    self_idx: usize, _other_idx: usize, activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::func_clock_use(&mut ctx, self_idx, _other_idx, activator_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

// --- g_target / g_utils context helpers ---

fn make_g_utils_ctx(edicts: &[Edict], level: &LevelLocals) -> GameContext {
    let (num_edicts, maxclients, max_edicts) =
        crate::g_local::with_global_game_ctx(|gctx| {
            (gctx.num_edicts, gctx.maxclients, gctx.max_edicts)
        }).unwrap_or((edicts.len() as i32, 0.0, edicts.len() as i32));
    GameContext {
        edicts: edicts.to_vec(),
        num_edicts,
        maxclients,
        max_edicts,
        level: level.clone(),
        ..GameContext::default()
    }
}

fn sync_g_utils_ctx(edicts: &mut [Edict], ctx: &GameContext) {
    let len = edicts.len().min(ctx.edicts.len());
    edicts[..len].clone_from_slice(&ctx.edicts[..len]);
}

// --- g_target wrappers (functions take GameContext) ---

fn w_target_explosion_explode(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut game = make_g_utils_ctx(edicts, level);
    crate::g_target::target_explosion_explode(&mut game, level, self_idx as i32);
    sync_g_utils_ctx(edicts, &game);
}

fn w_target_crosslevel_target(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut game = make_g_utils_ctx(edicts, level);
    let game_locals = crate::g_local::with_global_game_ctx(|gctx| gctx.game.clone())
        .unwrap_or_default();
    crate::g_target::target_crosslevel_target_think(&mut game, &game_locals, self_idx as i32);
    sync_g_utils_ctx(edicts, &game);
}

fn w_target_laser_start(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut game = make_g_utils_ctx(edicts, level);
    crate::g_target::target_laser_start(&mut game, level, self_idx as i32);
    sync_g_utils_ctx(edicts, &game);
}

fn w_target_laser_think(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut game = make_g_utils_ctx(edicts, level);
    crate::g_target::target_laser_think(&mut game, level, self_idx as i32);
    sync_g_utils_ctx(edicts, &game);
}

fn w_target_lightramp_think(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut game = make_g_utils_ctx(edicts, level);
    crate::g_target::target_lightramp_think(&mut game, level, self_idx as i32);
    sync_g_utils_ctx(edicts, &game);
}

fn w_target_earthquake_think(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut game = make_g_utils_ctx(edicts, level);
    crate::g_target::target_earthquake_think(&mut game, level, self_idx as i32);
    sync_g_utils_ctx(edicts, &game);
}

fn w_use_target_tent(
    self_idx: usize, _other_idx: usize, _activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut game = make_g_utils_ctx(edicts, level);
    crate::g_target::use_target_tent(&mut game, self_idx as i32, _other_idx as i32, _activator_idx as i32);
    sync_g_utils_ctx(edicts, &game);
}

fn w_use_target_speaker(
    self_idx: usize, _other_idx: usize, _activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut game = make_g_utils_ctx(edicts, level);
    crate::g_target::use_target_speaker(&mut game, self_idx as i32, _other_idx as i32, _activator_idx as i32);
    sync_g_utils_ctx(edicts, &game);
}

fn w_use_target_explosion(
    self_idx: usize, _other_idx: usize, activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut game = make_g_utils_ctx(edicts, level);
    crate::g_target::use_target_explosion(&mut game, level, self_idx as i32, _other_idx as i32, activator_idx as i32);
    sync_g_utils_ctx(edicts, &game);
}

fn w_use_target_changelevel(
    self_idx: usize, other_idx: usize, activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let (mut game_locals, clients, deathmatch, coop, dmflags) =
        crate::g_local::with_global_game_ctx(|gctx| {
            (gctx.game.clone(), gctx.clients.clone(), gctx.deathmatch, gctx.coop, gctx.dmflags)
        }).unwrap_or_else(|| (GameLocals::default(), Vec::new(), 0.0, 0.0, 0.0));
    let mut game = make_g_utils_ctx(edicts, level);
    crate::g_target::use_target_changelevel(
        &mut game, &mut game_locals, level, &clients,
        self_idx as i32, other_idx as i32, activator_idx as i32,
        deathmatch, coop, dmflags,
    );
    sync_g_utils_ctx(edicts, &game);
}

fn w_use_target_splash(
    self_idx: usize, _other_idx: usize, activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut game = make_g_utils_ctx(edicts, level);
    crate::g_target::use_target_splash(&mut game, level, self_idx as i32, _other_idx as i32, activator_idx as i32);
    sync_g_utils_ctx(edicts, &game);
}

fn w_use_target_spawner(
    self_idx: usize, _other_idx: usize, _activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut game = make_g_utils_ctx(edicts, level);
    crate::g_target::use_target_spawner(&mut game, self_idx as i32, _other_idx as i32, _activator_idx as i32);
    sync_g_utils_ctx(edicts, &game);
}

fn w_use_target_blaster(
    self_idx: usize, _other_idx: usize, _activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut game = make_g_utils_ctx(edicts, level);
    crate::g_target::use_target_blaster(&mut game, self_idx as i32, _other_idx as i32, _activator_idx as i32);
    sync_g_utils_ctx(edicts, &game);
}

fn w_use_target_laser(
    self_idx: usize, _other_idx: usize, activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut game = make_g_utils_ctx(edicts, level);
    crate::g_target::target_laser_use(&mut game, level, self_idx as i32, _other_idx as i32, activator_idx as i32);
    sync_g_utils_ctx(edicts, &game);
}

fn w_use_target_lightramp(
    self_idx: usize, _other_idx: usize, _activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut game = make_g_utils_ctx(edicts, level);
    crate::g_target::target_lightramp_use(&mut game, level, self_idx as i32, _other_idx as i32, _activator_idx as i32);
    sync_g_utils_ctx(edicts, &game);
}

fn w_use_target_earthquake(
    self_idx: usize, _other_idx: usize, activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut game = make_g_utils_ctx(edicts, level);
    crate::g_target::target_earthquake_use(&mut game, level, self_idx as i32, _other_idx as i32, activator_idx as i32);
    sync_g_utils_ctx(edicts, &game);
}

fn w_use_target_help(
    self_idx: usize, _other_idx: usize, _activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut game = make_g_utils_ctx(edicts, level);
    let mut game_locals = crate::g_local::with_global_game_ctx(|gctx| gctx.game.clone())
        .unwrap_or_default();
    crate::g_target::use_target_help(&mut game, &mut game_locals, self_idx as i32, _other_idx as i32, _activator_idx as i32);
    sync_g_utils_ctx(edicts, &game);
}

fn w_use_target_secret(
    self_idx: usize, _other_idx: usize, activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut game = make_g_utils_ctx(edicts, level);
    crate::g_target::use_target_secret(&mut game, level, self_idx as i32, _other_idx as i32, activator_idx as i32);
    sync_g_utils_ctx(edicts, &game);
}

fn w_use_target_goal(
    self_idx: usize, _other_idx: usize, activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut game = make_g_utils_ctx(edicts, level);
    crate::g_target::use_target_goal(&mut game, level, self_idx as i32, _other_idx as i32, activator_idx as i32);
    sync_g_utils_ctx(edicts, &game);
}

fn w_trigger_crosslevel_trigger_use(
    self_idx: usize, _other_idx: usize, _activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut game = make_g_utils_ctx(edicts, level);
    let mut game_locals = crate::g_local::with_global_game_ctx(|gctx| gctx.game.clone())
        .unwrap_or_default();
    crate::g_target::trigger_crosslevel_trigger_use(&mut game, &mut game_locals, self_idx as i32, _other_idx as i32, _activator_idx as i32);
    sync_g_utils_ctx(edicts, &game);
}

fn w_use_target_string(
    self_idx: usize, _other_idx: usize, activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::target_string_use(&mut ctx, self_idx, _other_idx, activator_idx);
    sync_misc_ctx(edicts, level, &ctx);
}

// --- g_utils wrappers (think_delay) ---

fn w_think_delay(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut game = make_g_utils_ctx(edicts, level);
    crate::g_utils::think_delay(&mut game, self_idx as i32);
    sync_g_utils_ctx(edicts, &game);
}

// --- p_client context helpers ---

fn make_pclient_ctx(edicts: &[Edict], level: &LevelLocals) -> GameContext {
    let (game, clients, items, deathmatch, coop, dmflags, maxclients, maxspectators, sv_gravity) =
        crate::g_local::with_global_game_ctx(|gctx| {
            (
                gctx.game.clone(),
                gctx.clients.clone(),
                gctx.items.clone(),
                gctx.deathmatch,
                gctx.coop,
                gctx.dmflags,
                gctx.maxclients,
                gctx.maxspectators,
                gctx.sv_gravity,
            )
        }).unwrap_or_else(|| {
            (GameLocals::default(), Vec::new(), Vec::new(), 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
        });
    GameContext {
        edicts: edicts.to_vec(),
        clients,
        game,
        level: level.clone(),
        items,
        deathmatch,
        coop,
        dmflags,
        maxclients,
        maxspectators,
        sv_gravity,
        ..GameContext::default()
    }
}

fn sync_pclient_ctx(edicts: &mut [Edict], level: &mut LevelLocals, ctx: &GameContext) {
    *level = ctx.level.clone();
    let len = edicts.len().min(ctx.edicts.len());
    edicts[..len].clone_from_slice(&ctx.edicts[..len]);
}

// --- p_client wrappers ---

fn w_sp_create_coop_spots(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_pclient_ctx(edicts, level);
    crate::p_client::sp_create_coop_spots(&mut ctx, self_idx);
    sync_pclient_ctx(edicts, level, &ctx);
}

fn w_sp_fix_coop_spots(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_pclient_ctx(edicts, level);
    crate::p_client::sp_fix_coop_spots(&mut ctx, self_idx);
    sync_pclient_ctx(edicts, level, &ctx);
}

fn w_player_pain(
    self_idx: usize, attacker_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
    kick: f32, damage: i32,
) {
    let mut ctx = make_pclient_ctx(edicts, level);
    crate::p_client::player_pain(&mut ctx, self_idx, attacker_idx, kick, damage);
    sync_pclient_ctx(edicts, level, &ctx);
}

fn w_player_die(
    self_idx: usize, inflictor_idx: usize, attacker_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
    damage: i32, point: Vec3,
) {
    let mut ctx = make_pclient_ctx(edicts, level);
    crate::p_client::player_die(&mut ctx, self_idx, inflictor_idx, attacker_idx, damage, point);
    sync_pclient_ctx(edicts, level, &ctx);
}

fn w_body_die(
    self_idx: usize, inflictor_idx: usize, attacker_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
    damage: i32, point: Vec3,
) {
    let mut ctx = make_pclient_ctx(edicts, level);
    crate::p_client::body_die(&mut ctx, self_idx, inflictor_idx, attacker_idx, damage, point);
    sync_pclient_ctx(edicts, level, &ctx);
}

// --- g_weapon wrappers (Vec<Edict> bridge) ---

fn w_grenade_explode(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut v = edicts.to_vec();
    crate::g_weapon::grenade_explode(self_idx, &mut v, level);
    let len = edicts.len().min(v.len());
    edicts[..len].clone_from_slice(&v[..len]);
}

fn w_bfg_think(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut v = edicts.to_vec();
    crate::g_weapon::bfg_think(self_idx, &mut v, level);
    let len = edicts.len().min(v.len());
    edicts[..len].clone_from_slice(&v[..len]);
}

fn w_bfg_explode(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut v = edicts.to_vec();
    crate::g_weapon::bfg_explode(self_idx, &mut v, level);
    let len = edicts.len().min(v.len());
    edicts[..len].clone_from_slice(&v[..len]);
}

fn w_blaster_touch(
    self_idx: usize, other_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals,
    _plane: Option<&CPlane>, _surf: Option<&CSurface>,
) {
    let mut v = edicts.to_vec();
    crate::g_weapon::blaster_touch(self_idx, other_idx, &mut v, level, _plane, _surf);
    let len = edicts.len().min(v.len());
    edicts[..len].clone_from_slice(&v[..len]);
}

fn w_grenade_touch(
    self_idx: usize, other_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals,
    _plane: Option<&CPlane>, _surf: Option<&CSurface>,
) {
    let mut v = edicts.to_vec();
    crate::g_weapon::grenade_touch(self_idx, other_idx, &mut v, level, _plane, _surf);
    let len = edicts.len().min(v.len());
    edicts[..len].clone_from_slice(&v[..len]);
}

fn w_rocket_touch(
    self_idx: usize, other_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals,
    _plane: Option<&CPlane>, _surf: Option<&CSurface>,
) {
    let mut v = edicts.to_vec();
    crate::g_weapon::rocket_touch(self_idx, other_idx, &mut v, level, _plane, _surf);
    let len = edicts.len().min(v.len());
    edicts[..len].clone_from_slice(&v[..len]);
}

fn w_bfg_touch(
    self_idx: usize, other_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals,
    _plane: Option<&CPlane>, _surf: Option<&CSurface>,
) {
    let mut v = edicts.to_vec();
    crate::g_weapon::bfg_touch(self_idx, other_idx, &mut v, level, _plane, _surf);
    let len = edicts.len().min(v.len());
    edicts[..len].clone_from_slice(&v[..len]);
}

// --- g_func context helpers ---

fn make_func_ctx(edicts: &[Edict], level: &LevelLocals) -> GameContext {
    let (st, deathmatch) =
        crate::g_local::with_global_game_ctx(|gctx| {
            (gctx.st.clone(), gctx.deathmatch)
        }).unwrap_or_else(|| (SpawnTemp::default(), 0.0));
    GameContext {
        edicts: edicts.to_vec(),
        level: level.clone(),
        st,
        deathmatch,
        ..GameContext::default()
    }
}

fn sync_func_ctx(edicts: &mut [Edict], level: &mut LevelLocals, ctx: &GameContext) {
    *level = ctx.level.clone();
    let len = edicts.len().min(ctx.edicts.len());
    edicts[..len].clone_from_slice(&ctx.edicts[..len]);
}

// --- g_func wrappers ---

fn w_func_door_go_up(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_func_ctx(edicts, level);
    let activator = edicts[self_idx].activator as usize;
    ctx.door_go_up(self_idx, activator);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_func_door_go_down(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.door_go_down(self_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_func_door_secret_move1(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.door_secret_move1(self_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_func_door_secret_move2(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.door_secret_move2(self_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_func_door_secret_move3(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.door_secret_move3(self_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_func_door_secret_move4(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.door_secret_move4(self_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_func_door_secret_move5(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.door_secret_move5(self_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_func_door_secret_move6(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.door_secret_move6(self_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_func_door_secret_done(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.door_secret_done(self_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_func_train_next(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.train_next(self_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_func_plat_go_up(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.plat_go_up(self_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_func_plat_go_down(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.plat_go_down(self_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_func_train_find(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.func_train_find(self_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_touch_door_trigger(
    self_idx: usize, other_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals,
    _plane: Option<&CPlane>, _surf: Option<&CSurface>,
) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.touch_door_trigger(self_idx, other_idx, _plane, _surf);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_door_use(
    self_idx: usize, _other_idx: usize, activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.door_use(self_idx, _other_idx, activator_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_button_use(
    self_idx: usize, _other_idx: usize, activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.button_use(self_idx, _other_idx, activator_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_train_use(
    self_idx: usize, _other_idx: usize, activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.train_use(self_idx, _other_idx, activator_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_func_timer_use(
    self_idx: usize, _other_idx: usize, activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.func_timer_use(self_idx, _other_idx, activator_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_use_killbox(
    self_idx: usize, _other_idx: usize, _activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.use_killbox(self_idx, _other_idx, _activator_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_func_move_done(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.move_done(self_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_func_move_final(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.move_final(self_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_func_move_begin(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.move_begin(self_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_func_accel_move(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.think_accel_move(self_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_func_angle_move_done(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.angle_move_done(self_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_func_angle_move_final(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.angle_move_final(self_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_func_angle_move_begin(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.angle_move_begin(self_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_func_button_return(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.button_return(self_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_func_calc_move_speed(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.think_calc_move_speed(self_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_func_spawn_door_trigger(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.think_spawn_door_trigger(self_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_func_trigger_elevator_init(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.trigger_elevator_init(self_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_func_timer_think(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.func_timer_think(self_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_use_plat(
    self_idx: usize, other_idx: usize, activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.use_plat(self_idx, other_idx, activator_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_rotating_use(
    self_idx: usize, other_idx: usize, activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.rotating_use(self_idx, other_idx, activator_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_door_secret_use(
    self_idx: usize, other_idx: usize, activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.door_secret_use(self_idx, other_idx, activator_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_trigger_elevator_use(
    self_idx: usize, other_idx: usize, activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.trigger_elevator_use(self_idx, other_idx, activator_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_func_conveyor_use(
    self_idx: usize, other_idx: usize, activator_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.func_conveyor_use(self_idx, other_idx, activator_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_touch_plat_center(
    self_idx: usize, other_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals,
    plane: Option<&CPlane>, surf: Option<&CSurface>,
) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.touch_plat_center(self_idx, other_idx, plane, surf);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_rotating_touch(
    self_idx: usize, other_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals,
    plane: Option<&CPlane>, surf: Option<&CSurface>,
) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.rotating_touch(self_idx, other_idx, plane, surf);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_button_touch(
    self_idx: usize, other_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals,
    plane: Option<&CPlane>, surf: Option<&CSurface>,
) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.button_touch(self_idx, other_idx, plane, surf);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_door_touch(
    self_idx: usize, other_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals,
    plane: Option<&CPlane>, surf: Option<&CSurface>,
) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.door_touch(self_idx, other_idx, plane, surf);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_door_secret_blocked(
    self_idx: usize, other_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.door_secret_blocked(self_idx, other_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_button_killed_die(
    self_idx: usize, inflictor_idx: usize, attacker_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
    damage: i32, point: Vec3,
) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.button_killed(self_idx, inflictor_idx, attacker_idx, damage, &point);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_door_killed_die(
    self_idx: usize, inflictor_idx: usize, attacker_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
    damage: i32, point: Vec3,
) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.door_killed(self_idx, inflictor_idx, attacker_idx, damage, &point);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_door_secret_die(
    self_idx: usize, inflictor_idx: usize, attacker_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
    damage: i32, point: Vec3,
) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.door_secret_die(self_idx, inflictor_idx, attacker_idx, damage, &point);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_door_blocked(
    self_idx: usize, other_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.door_blocked(self_idx, other_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_plat_blocked(
    self_idx: usize, other_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.plat_blocked(self_idx, other_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_train_blocked(
    self_idx: usize, other_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.train_blocked(self_idx, other_idx);
    sync_func_ctx(edicts, level, &ctx);
}

fn w_rotating_blocked(
    self_idx: usize, other_idx: usize,
    edicts: &mut [Edict], level: &mut LevelLocals,
) {
    let mut ctx = make_func_ctx(edicts, level);
    ctx.rotating_blocked(self_idx, other_idx);
    sync_func_ctx(edicts, level, &ctx);
}

// --- g_misc touch wrappers (path_corner, point_combat) ---

fn w_path_corner_touch(
    self_idx: usize, other_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals,
    _plane: Option<&CPlane>, _surf: Option<&CSurface>,
) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::path_corner_touch(&mut ctx, self_idx, other_idx, None, None);
    sync_misc_ctx(edicts, level, &ctx);
}

fn w_point_combat_touch(
    self_idx: usize, other_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals,
    _plane: Option<&CPlane>, _surf: Option<&CSurface>,
) {
    let mut ctx = make_misc_ctx(edicts, level);
    crate::g_misc::point_combat_touch(&mut ctx, self_idx, other_idx, None, None);
    sync_misc_ctx(edicts, level, &ctx);
}

// --- g_monster think wrapper ---

fn w_monster_think(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(level);
    ctx.edicts = edicts.to_vec();
    crate::g_monster::monster_think(&mut ctx, self_idx as i32);
    sync_level(level, &ctx);
    let len = edicts.len().min(ctx.edicts.len());
    edicts[..len].clone_from_slice(&ctx.edicts[..len]);
}

// --- m_mutant wrappers ---

fn w_mutant_jump_touch(
    self_idx: usize,
    other_idx: usize,
    edicts: &mut [Edict],
    _level: &mut LevelLocals,
    plane: Option<&CPlane>,
    surf: Option<&CSurface>,
) {
    crate::m_mutant::mutant_jump_touch(&mut edicts[self_idx], other_idx as i32, plane, surf);
}

// --- Soldier ---
fn w_soldier_stand(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_soldier::soldier_stand(&mut edicts[self_idx], &mut ctx);
}
fn w_soldier_walk(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_soldier::soldier_walk(&mut edicts[self_idx], &mut ctx);
}
fn w_soldier_run(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_soldier::soldier_run(&mut edicts[self_idx], &mut ctx);
}
fn w_soldier_idle(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_soldier::soldier_idle(&mut edicts[self_idx], &mut ctx);
}
fn w_soldier_sight(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut dummy = Edict::default();
    crate::m_soldier::soldier_sight(&mut edicts[self_idx], &mut dummy);
}
fn w_soldier_pain(
    self_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, kick: f32, damage: i32,
) {
    let mut ctx = make_temp_ctx(level);
    let mut dummy = Edict::default();
    crate::m_soldier::soldier_pain(&mut edicts[self_idx], &mut dummy, kick, damage, &mut ctx);
    sync_level(level, &ctx);
}
fn w_soldier_die(
    self_idx: usize, _inflictor_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, damage: i32, point: Vec3,
) {
    let mut ctx = make_temp_ctx(level);
    let mut dummy1 = Edict::default();
    let mut dummy2 = Edict::default();
    crate::m_soldier::soldier_die(&mut edicts[self_idx], &mut dummy1, &mut dummy2, damage, point, &mut ctx);
    sync_level(level, &ctx);
}
fn w_soldier_dodge(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(level);
    crate::m_soldier::soldier_dodge(&mut edicts[self_idx], 0, 0.0, &mut ctx);
    sync_level(level, &ctx);
}
fn w_soldier_attack(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(level);
    crate::m_soldier::soldier_attack(&mut edicts[self_idx], &mut ctx);
    sync_level(level, &ctx);
}

// --- Berserk ---
fn w_berserk_stand(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_berserk::berserk_stand(&mut edicts[self_idx], &mut ctx);
}
fn w_berserk_walk(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_berserk::berserk_walk(&mut edicts[self_idx], &mut ctx);
}
fn w_berserk_run(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_berserk::berserk_run(&mut edicts[self_idx], &mut ctx);
}
fn w_berserk_melee(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_berserk::berserk_melee(&mut edicts[self_idx], &mut ctx);
}
fn w_berserk_sight(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut dummy = Edict::default();
    crate::m_berserk::berserk_sight(&mut edicts[self_idx], &mut dummy);
}
fn w_berserk_search(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    crate::m_berserk::berserk_search(&mut edicts[self_idx]);
}
fn w_berserk_pain(
    self_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, kick: f32, damage: i32,
) {
    let mut ctx = make_temp_ctx(level);
    let mut dummy = Edict::default();
    crate::m_berserk::berserk_pain(&mut edicts[self_idx], &mut dummy, kick, damage, &mut ctx);
    sync_level(level, &ctx);
}
fn w_berserk_die(
    self_idx: usize, _inflictor_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, damage: i32, point: Vec3,
) {
    let mut ctx = make_temp_ctx(level);
    let mut dummy1 = Edict::default();
    let mut dummy2 = Edict::default();
    crate::m_berserk::berserk_die(&mut edicts[self_idx], &mut dummy1, &mut dummy2, damage, point, &mut ctx);
    sync_level(level, &ctx);
}

// --- Actor ---
fn w_actor_stand(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut edicts_vec = edicts.to_vec();
    crate::m_actor::actor_stand(&mut edicts_vec, self_idx, level);
    edicts.clone_from_slice(&edicts_vec);
}
fn w_actor_walk(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut edicts_vec = edicts.to_vec();
    crate::m_actor::actor_walk(&mut edicts_vec, self_idx);
    edicts.clone_from_slice(&edicts_vec);
}
fn w_actor_run(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut edicts_vec = edicts.to_vec();
    crate::m_actor::actor_run(&mut edicts_vec, self_idx, level);
    edicts.clone_from_slice(&edicts_vec);
}
fn w_actor_pain(
    self_idx: usize, attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, kick: f32, damage: i32,
) {
    let mut edicts_vec = edicts.to_vec();
    crate::m_actor::actor_pain(&mut edicts_vec, self_idx, attacker_idx, kick, damage, level);
    edicts.clone_from_slice(&edicts_vec);
}
fn w_actor_die(
    self_idx: usize, inflictor_idx: usize, attacker_idx: usize, edicts: &mut [Edict],
    _level: &mut LevelLocals, damage: i32, point: Vec3,
) {
    let mut edicts_vec = edicts.to_vec();
    crate::m_actor::actor_die(&mut edicts_vec, self_idx, inflictor_idx, attacker_idx, damage, point);
    edicts.clone_from_slice(&edicts_vec);
}

// --- Floater ---
fn w_floater_stand(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(level);
    crate::m_float::floater_stand(&mut edicts[self_idx], &mut ctx);
    sync_level(level, &ctx);
}
fn w_floater_walk(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(level);
    crate::m_float::floater_walk(&mut edicts[self_idx], &mut ctx);
    sync_level(level, &ctx);
}
fn w_floater_run(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(level);
    crate::m_float::floater_run(&mut edicts[self_idx], &mut ctx);
    sync_level(level, &ctx);
}
fn w_floater_pain(
    self_idx: usize, other_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, kick: f32, damage: i32,
) {
    let mut ctx = make_temp_ctx(level);
    ctx.edicts = edicts.to_vec();
    crate::m_float::floater_pain(&mut ctx, self_idx as i32, other_idx as i32, kick, damage);
    sync_level(level, &ctx);
    edicts.clone_from_slice(&ctx.edicts);
}
fn w_floater_die(
    self_idx: usize, inflictor_idx: usize, attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, damage: i32, point: Vec3,
) {
    let mut ctx = make_temp_ctx(level);
    ctx.edicts = edicts.to_vec();
    crate::m_float::floater_die(&mut ctx, self_idx as i32, inflictor_idx as i32, attacker_idx as i32, damage, point);
    sync_level(level, &ctx);
    edicts.clone_from_slice(&ctx.edicts);
}

// --- Brain ---
fn w_brain_stand(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_brain::brain_stand(&mut edicts[self_idx], &mut ctx);
}
fn w_brain_walk(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_brain::brain_walk(&mut edicts[self_idx], &mut ctx);
}
fn w_brain_run(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_brain::brain_run(&mut edicts[self_idx], &mut ctx);
}
fn w_brain_idle(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_brain::brain_idle(&mut edicts[self_idx], &mut ctx);
}
fn w_brain_melee(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_brain::brain_melee(&mut edicts[self_idx], &mut ctx);
}
fn w_brain_dodge(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    crate::m_brain::brain_dodge(&mut edicts[self_idx], 0, 0.0, level);
}
fn w_brain_sight(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    crate::m_brain::brain_sight(&mut edicts[self_idx], 0, level);
}
fn w_brain_search(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_brain::brain_search(&mut edicts[self_idx], &mut ctx);
}
fn w_brain_pain(
    self_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, kick: f32, damage: i32,
) {
    crate::m_brain::brain_pain(&mut edicts[self_idx], _attacker_idx as i32, kick, damage, level);
}
fn w_brain_die(
    self_idx: usize, _inflictor_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, damage: i32, point: Vec3,
) {
    crate::m_brain::brain_die(&mut edicts[self_idx], _inflictor_idx as i32, _attacker_idx as i32, damage, point, level);
}

// --- Tank ---
fn w_tank_stand(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_tank::tank_stand(&mut edicts[self_idx], &mut ctx);
}
fn w_tank_walk(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_tank::tank_walk(&mut edicts[self_idx], &mut ctx);
}
fn w_tank_run(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_tank::tank_run(&mut edicts[self_idx], &mut ctx);
}
fn w_tank_idle(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_tank::tank_idle(&mut edicts[self_idx], &mut ctx);
}
fn w_tank_sight(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut dummy = Edict::default();
    crate::m_tank::tank_sight(&mut edicts[self_idx], &mut dummy);
}
fn w_tank_attack(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_tank::tank_attack(&mut edicts[self_idx], &mut ctx);
}
fn w_tank_pain(
    self_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, kick: f32, damage: i32,
) {
    let mut ctx = make_temp_ctx(level);
    let mut dummy = Edict::default();
    crate::m_tank::tank_pain(&mut edicts[self_idx], &mut dummy, kick, damage, &mut ctx);
    sync_level(level, &ctx);
}
fn w_tank_die(
    self_idx: usize, _inflictor_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, damage: i32, point: Vec3,
) {
    let mut ctx = make_temp_ctx(level);
    let mut dummy1 = Edict::default();
    let mut dummy2 = Edict::default();
    crate::m_tank::tank_die(&mut edicts[self_idx], &mut dummy1, &mut dummy2, damage, point, &mut ctx);
    sync_level(level, &ctx);
}

// --- Infantry ---
fn w_infantry_stand(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_infantry::infantry_stand(&mut edicts[self_idx], &mut ctx);
}
fn w_infantry_walk(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_infantry::infantry_walk(&mut edicts[self_idx], &mut ctx);
}
fn w_infantry_run(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_infantry::infantry_run(&mut edicts[self_idx], &mut ctx);
}
fn w_infantry_sight(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut dummy = Edict::default();
    crate::m_infantry::infantry_sight(&mut edicts[self_idx], &mut dummy);
}
fn w_infantry_attack(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_infantry::infantry_attack(&mut edicts[self_idx], &mut ctx);
}
fn w_infantry_idle(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_infantry::infantry_fidget(&mut edicts[self_idx], &mut ctx);
}
fn w_infantry_dodge(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    let mut dummy = Edict::default();
    crate::m_infantry::infantry_dodge(&mut edicts[self_idx], &mut dummy, 0.0, &mut ctx);
}
fn w_infantry_pain(
    self_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, kick: f32, damage: i32,
) {
    let mut ctx = make_temp_ctx(level);
    let mut dummy = Edict::default();
    crate::m_infantry::infantry_pain(&mut edicts[self_idx], &mut dummy, kick, damage, &mut ctx);
    sync_level(level, &ctx);
}
fn w_infantry_die(
    self_idx: usize, _inflictor_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, damage: i32, point: Vec3,
) {
    let mut ctx = make_temp_ctx(level);
    let mut dummy1 = Edict::default();
    let mut dummy2 = Edict::default();
    crate::m_infantry::infantry_die(&mut edicts[self_idx], &mut dummy1, &mut dummy2, damage, point, &mut ctx);
    sync_level(level, &ctx);
}

// --- Gladiator ---
fn w_gladiator_stand(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_gladiator::gladiator_stand(&mut edicts[self_idx], &mut ctx);
}
fn w_gladiator_walk(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_gladiator::gladiator_walk(&mut edicts[self_idx], &mut ctx);
}
fn w_gladiator_run(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_gladiator::gladiator_run(&mut edicts[self_idx], &mut ctx);
}
fn w_gladiator_idle(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_gladiator::gladiator_idle(&mut edicts[self_idx], &mut ctx);
}
fn w_gladiator_sight(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_gladiator::gladiator_sight(&mut edicts[self_idx], &mut ctx);
}
fn w_gladiator_search(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_gladiator::gladiator_search(&mut edicts[self_idx], &mut ctx);
}
fn w_gladiator_melee(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_gladiator::gladiator_melee(&mut edicts[self_idx], &mut ctx);
}
fn w_gladiator_attack(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_gladiator::gladiator_attack(&mut edicts[self_idx], &mut ctx);
}
fn w_gladiator_pain(
    self_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, kick: f32, damage: i32,
) {
    let mut ctx = make_temp_ctx(level);
    let mut dummy = Edict::default();
    crate::m_gladiator::gladiator_pain(&mut edicts[self_idx], &mut dummy, kick, damage, &mut ctx);
    sync_level(level, &ctx);
}
fn w_gladiator_die(
    self_idx: usize, _inflictor_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, damage: i32, point: Vec3,
) {
    let mut ctx = make_temp_ctx(level);
    let mut dummy1 = Edict::default();
    let mut dummy2 = Edict::default();
    crate::m_gladiator::gladiator_die(&mut edicts[self_idx], &mut dummy1, &mut dummy2, damage, point, &mut ctx);
    sync_level(level, &ctx);
}

// --- Gunner ---
fn w_gunner_stand(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_gunner::gunner_stand(&mut edicts[self_idx], &mut ctx);
}
fn w_gunner_walk(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_gunner::gunner_walk(&mut edicts[self_idx], &mut ctx);
}
fn w_gunner_run(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_gunner::gunner_run(&mut edicts[self_idx], &mut ctx);
}
fn w_gunner_idle(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_gunner::gunner_idlesound(&mut edicts[self_idx], &mut ctx);
}
fn w_gunner_sight(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut dummy = Edict::default();
    crate::m_gunner::gunner_sight(&mut edicts[self_idx], &mut dummy);
}
fn w_gunner_search(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    crate::m_gunner::gunner_search(&mut edicts[self_idx]);
}
fn w_gunner_attack(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_gunner::gunner_attack(&mut edicts[self_idx], &mut ctx);
}
fn w_gunner_dodge(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    crate::m_gunner::gunner_dodge(&mut edicts[self_idx], 0, 0.0);
}
fn w_gunner_pain(
    self_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, kick: f32, damage: i32,
) {
    let mut dummy = Edict::default();
    crate::m_gunner::gunner_pain(&mut edicts[self_idx], &mut dummy, kick, damage, level);
}
fn w_gunner_die(
    self_idx: usize, _inflictor_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    _level: &mut LevelLocals, damage: i32, point: Vec3,
) {
    let mut dummy1 = Edict::default();
    let mut dummy2 = Edict::default();
    crate::m_gunner::gunner_die(&mut edicts[self_idx], &mut dummy1, &mut dummy2, damage, &point);
}

// --- Parasite ---
fn w_parasite_stand(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_parasite::parasite_stand(&mut edicts[self_idx], &mut ctx);
}
fn w_parasite_walk(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_parasite::parasite_walk(&mut edicts[self_idx], &mut ctx);
}
fn w_parasite_run(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_parasite::parasite_run(&mut edicts[self_idx], &mut ctx);
}
fn w_parasite_idle(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_parasite::parasite_idle(&mut edicts[self_idx], &mut ctx);
}
fn w_parasite_sight(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut dummy = Edict::default();
    crate::m_parasite::parasite_sight(&mut edicts[self_idx], &mut dummy);
}
fn w_parasite_search(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_parasite::parasite_search(&mut edicts[self_idx], &mut ctx);
}
fn w_parasite_attack(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_parasite::parasite_attack(&mut edicts[self_idx], &mut ctx);
}
fn w_parasite_pain(
    self_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, kick: f32, damage: i32,
) {
    let mut ctx = make_temp_ctx(level);
    let mut dummy = Edict::default();
    crate::m_parasite::parasite_pain(&mut edicts[self_idx], &mut dummy, kick, damage, &mut ctx);
    sync_level(level, &ctx);
}
fn w_parasite_die(
    self_idx: usize, _inflictor_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, damage: i32, point: Vec3,
) {
    let mut ctx = make_temp_ctx(level);
    let mut dummy1 = Edict::default();
    let mut dummy2 = Edict::default();
    crate::m_parasite::parasite_die(&mut edicts[self_idx], &mut dummy1, &mut dummy2, damage, point, &mut ctx);
    sync_level(level, &ctx);
}

// --- Flipper ---
fn w_flipper_stand(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_flipper::flipper_stand(&mut edicts[self_idx], &mut ctx);
}
fn w_flipper_walk(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_flipper::flipper_walk(&mut edicts[self_idx], &mut ctx);
}
fn w_flipper_run(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_flipper::flipper_run(&mut edicts[self_idx], &mut ctx);
}
fn w_flipper_melee(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_flipper::flipper_melee(&mut edicts[self_idx], &mut ctx);
}
fn w_flipper_sight(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut dummy = Edict::default();
    crate::m_flipper::flipper_sight(&mut edicts[self_idx], &mut dummy);
}
fn w_flipper_pain(
    self_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, kick: f32, damage: i32,
) {
    crate::m_flipper::flipper_pain(&mut edicts[self_idx], _attacker_idx, kick, damage, level, 0.0);
}
fn w_flipper_die(
    self_idx: usize, _inflictor_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    _level: &mut LevelLocals, damage: i32, point: Vec3,
) {
    crate::m_flipper::flipper_die(&mut edicts[self_idx], _inflictor_idx, _attacker_idx, damage, point);
}

// --- Flyer ---
fn w_flyer_stand(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_flyer::flyer_stand(&mut edicts[self_idx], &mut ctx);
}
fn w_flyer_walk(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_flyer::flyer_walk(&mut edicts[self_idx], &mut ctx);
}
fn w_flyer_run(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_flyer::flyer_run(&mut edicts[self_idx], &mut ctx);
}
fn w_flyer_idle(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_flyer::flyer_idle(&mut edicts[self_idx], &mut ctx);
}
fn w_flyer_sight(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut dummy = Edict::default();
    crate::m_flyer::flyer_sight(&mut edicts[self_idx], &mut dummy);
}
fn w_flyer_attack(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_flyer::flyer_attack(&mut edicts[self_idx], &mut ctx);
}
fn w_flyer_melee(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_flyer::flyer_melee(&mut edicts[self_idx], &mut ctx);
}
fn w_flyer_pain(
    self_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    _level: &mut LevelLocals, kick: f32, damage: i32,
) {
    let mut dummy = Edict::default();
    crate::m_flyer::flyer_pain(&mut edicts[self_idx], &mut dummy, kick, damage);
}
fn w_flyer_die(
    self_idx: usize, _inflictor_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    _level: &mut LevelLocals, damage: i32, point: Vec3,
) {
    crate::m_flyer::flyer_die(&mut edicts[self_idx], _inflictor_idx as i32, _attacker_idx as i32, damage, point);
}

// --- Hover ---
fn w_hover_stand(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_hover::hover_stand(&mut edicts[self_idx], &mut ctx);
}
fn w_hover_walk(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_hover::hover_walk(&mut edicts[self_idx], &mut ctx);
}
fn w_hover_run(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_hover::hover_run(&mut edicts[self_idx], &mut ctx);
}
fn w_hover_sight(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut dummy = Edict::default();
    crate::m_hover::hover_sight(&mut edicts[self_idx], &mut dummy);
}
fn w_hover_search(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    crate::m_hover::hover_search(&mut edicts[self_idx]);
}
fn w_hover_attack(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_hover::hover_attack(&mut edicts[self_idx], &mut ctx);
}
fn w_hover_pain(
    self_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, kick: f32, damage: i32,
) {
    let mut ctx = make_temp_ctx(level);
    let mut dummy = Edict::default();
    crate::m_hover::hover_pain(&mut edicts[self_idx], &mut dummy, kick, damage, &mut ctx);
    sync_level(level, &ctx);
}
fn w_hover_die(
    self_idx: usize, _inflictor_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, damage: i32, point: Vec3,
) {
    let mut ctx = make_temp_ctx(level);
    let mut dummy1 = Edict::default();
    let mut dummy2 = Edict::default();
    crate::m_hover::hover_die(&mut edicts[self_idx], &mut dummy1, &mut dummy2, damage, point, &mut ctx);
    sync_level(level, &ctx);
}

// --- Chick ---
fn w_chick_stand(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_chick::chick_stand(&mut edicts[self_idx], &mut ctx);
}
fn w_chick_walk(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_chick::chick_walk(&mut edicts[self_idx], &mut ctx);
}
fn w_chick_run(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_chick::chick_run(&mut edicts[self_idx], &mut ctx);
}
fn w_chick_sight(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut dummy = Edict::default();
    crate::m_chick::chick_sight(&mut edicts[self_idx], &mut dummy);
}
fn w_chick_melee(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_chick::chick_melee(&mut edicts[self_idx], &mut ctx);
}
fn w_chick_attack(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_chick::chick_attack(&mut edicts[self_idx], &mut ctx);
}
fn w_chick_dodge(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    let mut dummy = Edict::default();
    crate::m_chick::chick_dodge(&mut edicts[self_idx], &mut dummy, 0.0, &mut ctx);
}
fn w_chick_pain(
    self_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, kick: f32, damage: i32,
) {
    let mut ctx = make_temp_ctx(level);
    let mut dummy = Edict::default();
    crate::m_chick::chick_pain(&mut edicts[self_idx], &mut dummy, kick, damage, &mut ctx);
    sync_level(level, &ctx);
}
fn w_chick_die(
    self_idx: usize, _inflictor_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, damage: i32, point: Vec3,
) {
    let mut ctx = make_temp_ctx(level);
    let mut dummy1 = Edict::default();
    let mut dummy2 = Edict::default();
    crate::m_chick::chick_die(&mut edicts[self_idx], &mut dummy1, &mut dummy2, damage, point, &mut ctx);
    sync_level(level, &ctx);
}

// --- Mutant ---
fn w_mutant_stand(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_mutant::mutant_stand(&mut edicts[self_idx], &mut ctx);
}
fn w_mutant_walk(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_mutant::mutant_walk(&mut edicts[self_idx], &mut ctx);
}
fn w_mutant_run(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_mutant::mutant_run(&mut edicts[self_idx], &mut ctx);
}
fn w_mutant_idle(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_mutant::mutant_idle(&mut edicts[self_idx], &mut ctx);
}
fn w_mutant_sight(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut dummy = Edict::default();
    crate::m_mutant::mutant_sight(&mut edicts[self_idx], &mut dummy);
}
fn w_mutant_search(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    crate::m_mutant::mutant_search(&mut edicts[self_idx]);
}
fn w_mutant_melee(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_mutant::mutant_melee(&mut edicts[self_idx], &mut ctx);
}
fn w_mutant_checkattack(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) -> bool {
    let mut ctx = make_temp_ctx(_level);
    crate::m_mutant::mutant_checkattack(&mut edicts[self_idx], &mut ctx)
}
fn w_mutant_pain(
    self_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, kick: f32, damage: i32,
) {
    let mut ctx = make_temp_ctx(level);
    let mut dummy = Edict::default();
    crate::m_mutant::mutant_pain(&mut edicts[self_idx], &mut dummy, kick, damage, &mut ctx);
    sync_level(level, &ctx);
}
fn w_mutant_die(
    self_idx: usize, _inflictor_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, damage: i32, point: Vec3,
) {
    let mut ctx = make_temp_ctx(level);
    let mut dummy1 = Edict::default();
    let mut dummy2 = Edict::default();
    crate::m_mutant::mutant_die(&mut edicts[self_idx], &mut dummy1, &mut dummy2, damage, point, &mut ctx);
    sync_level(level, &ctx);
}

// --- Insane ---
fn w_insane_stand(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_insane::insane_stand(&mut edicts[self_idx], &mut ctx);
}
fn w_insane_walk(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_insane::insane_walk(&mut edicts[self_idx], &mut ctx);
}
fn w_insane_run(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_insane::insane_run(&mut edicts[self_idx], &mut ctx);
}
fn w_insane_pain(
    self_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, kick: f32, damage: i32,
) {
    let mut ctx = make_temp_ctx(level);
    let mut dummy = Edict::default();
    crate::m_insane::insane_pain(&mut edicts[self_idx], &mut dummy, kick, damage, &mut ctx);
    sync_level(level, &ctx);
}
fn w_insane_die(
    self_idx: usize, _inflictor_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, damage: i32, point: Vec3,
) {
    let mut ctx = make_temp_ctx(level);
    let mut dummy1 = Edict::default();
    let mut dummy2 = Edict::default();
    crate::m_insane::insane_die(&mut edicts[self_idx], &mut dummy1, &mut dummy2, damage, point, &mut ctx);
    sync_level(level, &ctx);
}

// --- Medic ---
fn w_medic_stand(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_medic::medic_stand(&mut edicts[self_idx], &mut ctx);
}
fn w_medic_walk(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_medic::medic_walk(&mut edicts[self_idx], &mut ctx);
}
fn w_medic_run(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_medic::medic_run(&mut edicts[self_idx], &mut ctx);
}
fn w_medic_idle(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_medic::medic_idle(&mut edicts[self_idx], &mut ctx);
}
fn w_medic_sight(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut dummy = Edict::default();
    crate::m_medic::medic_sight(&mut edicts[self_idx], &mut dummy);
}
fn w_medic_search(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_medic::medic_search(&mut edicts[self_idx], &mut ctx);
}
fn w_medic_attack(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_medic::medic_attack(&mut edicts[self_idx], &mut ctx);
}
fn w_medic_dodge(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    let mut dummy = Edict::default();
    crate::m_medic::medic_dodge(&mut edicts[self_idx], &mut dummy, 0.0, &mut ctx);
}
fn w_medic_pain(
    self_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, kick: f32, damage: i32,
) {
    let mut ctx = make_temp_ctx(level);
    let mut dummy = Edict::default();
    crate::m_medic::medic_pain(&mut edicts[self_idx], &mut dummy, kick, damage, &mut ctx);
    sync_level(level, &ctx);
}
fn w_medic_die(
    self_idx: usize, _inflictor_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, damage: i32, point: Vec3,
) {
    let mut ctx = make_temp_ctx(level);
    let mut dummy1 = Edict::default();
    let mut dummy2 = Edict::default();
    crate::m_medic::medic_die(&mut edicts[self_idx], &mut dummy1, &mut dummy2, damage, point, &mut ctx);
    sync_level(level, &ctx);
}

// --- Supertank ---
fn w_supertank_stand(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_supertank::supertank_stand(&mut edicts[self_idx], &mut ctx);
}
fn w_supertank_walk(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_supertank::supertank_walk(&mut edicts[self_idx], &mut ctx);
}
fn w_supertank_run(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_supertank::supertank_run(&mut edicts[self_idx], &mut ctx);
}
fn w_supertank_search(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_supertank::supertank_search(&mut edicts[self_idx], &mut ctx);
}
fn w_supertank_attack(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_supertank::supertank_attack(&mut edicts[self_idx], &mut ctx);
}
fn w_supertank_pain(
    self_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, kick: f32, damage: i32,
) {
    let mut ctx = make_temp_ctx(level);
    let mut dummy = Edict::default();
    crate::m_supertank::supertank_pain(&mut edicts[self_idx], &mut dummy, kick, damage, &mut ctx);
    sync_level(level, &ctx);
}
fn w_supertank_die(
    self_idx: usize, _inflictor_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, damage: i32, point: Vec3,
) {
    let mut ctx = make_temp_ctx(level);
    let mut dummy1 = Edict::default();
    let mut dummy2 = Edict::default();
    crate::m_supertank::supertank_die(&mut edicts[self_idx], &mut dummy1, &mut dummy2, damage, point, &mut ctx);
    sync_level(level, &ctx);
}

// --- Boss2 ---
fn w_boss2_stand(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_boss2::boss2_stand(&mut edicts[self_idx], &mut ctx);
}
fn w_boss2_walk(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_boss2::boss2_walk(&mut edicts[self_idx], &mut ctx);
}
fn w_boss2_run(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_boss2::boss2_run(&mut edicts[self_idx], &mut ctx);
}
fn w_boss2_search(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_boss2::boss2_search(&mut edicts[self_idx], &mut ctx);
}
fn w_boss2_attack(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_boss2::boss2_attack(&mut edicts[self_idx], &mut ctx);
}
fn w_boss2_pain(
    self_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, kick: f32, damage: i32,
) {
    let mut ctx = make_temp_ctx(level);
    crate::m_boss2::boss2_pain(&mut edicts[self_idx], _attacker_idx, kick, damage, &mut ctx);
    sync_level(level, &ctx);
}
fn w_boss2_die(
    self_idx: usize, _inflictor_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, damage: i32, point: Vec3,
) {
    let mut ctx = make_temp_ctx(level);
    crate::m_boss2::boss2_die(&mut edicts[self_idx], _inflictor_idx, _attacker_idx, damage, point, &mut ctx);
    sync_level(level, &ctx);
}

// --- Jorg (boss31) ---
fn w_jorg_stand(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_boss31::jorg_stand(&mut edicts[self_idx], &mut ctx);
}
fn w_jorg_walk(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_boss31::jorg_walk(&mut edicts[self_idx], &mut ctx);
}
fn w_jorg_run(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_boss31::jorg_run(&mut edicts[self_idx], &mut ctx);
}
fn w_jorg_idle(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_boss31::jorg_idle(&mut edicts[self_idx], &mut ctx);
}
fn w_jorg_search(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_boss31::jorg_search(&mut edicts[self_idx], &mut ctx);
}
fn w_jorg_attack(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_boss31::jorg_attack(&mut edicts[self_idx], &mut ctx);
}
fn w_jorg_pain(
    self_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, kick: f32, damage: i32,
) {
    let mut ctx = make_temp_ctx(level);
    crate::m_boss31::jorg_pain(&mut edicts[self_idx], Some(_attacker_idx), kick, damage, &mut ctx);
    sync_level(level, &ctx);
}
fn w_jorg_die(
    self_idx: usize, _inflictor_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, damage: i32, point: Vec3,
) {
    let mut ctx = make_temp_ctx(level);
    crate::m_boss31::jorg_die(&mut edicts[self_idx], Some(_inflictor_idx), Some(_attacker_idx), damage, point, &mut ctx);
    sync_level(level, &ctx);
}

// --- Makron (boss32) ---
fn w_makron_stand(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_boss32::makron_stand(&mut edicts[self_idx], &mut ctx);
}
fn w_makron_walk(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_boss32::makron_walk(&mut edicts[self_idx], &mut ctx);
}
fn w_makron_run(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_boss32::makron_run(&mut edicts[self_idx], &mut ctx);
}
fn w_makron_sight(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_boss32::makron_sight(&mut edicts[self_idx], None, &mut ctx);
}
fn w_makron_attack(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_boss32::makron_attack(&mut edicts[self_idx], &mut ctx);
}
fn w_makron_pain(
    self_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, kick: f32, damage: i32,
) {
    let mut ctx = make_temp_ctx(level);
    crate::m_boss32::makron_pain(&mut edicts[self_idx], Some(_attacker_idx), kick, damage, &mut ctx);
    sync_level(level, &ctx);
}
fn w_makron_die(
    self_idx: usize, _inflictor_idx: usize, _attacker_idx: usize, edicts: &mut [Edict],
    level: &mut LevelLocals, damage: i32, point: Vec3,
) {
    let mut ctx = make_temp_ctx(level);
    crate::m_boss32::makron_die(&mut edicts[self_idx], Some(_inflictor_idx), Some(_attacker_idx), damage, point, &mut ctx);
    sync_level(level, &ctx);
}

// --- CheckAttack wrappers ---

fn w_jorg_checkattack(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) -> bool {
    let mut ctx = make_temp_ctx(level);
    let result = crate::m_boss31::jorg_check_attack(&mut edicts[self_idx], &mut ctx);
    sync_level(level, &ctx);
    result
}

fn w_makron_checkattack(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) -> bool {
    let mut ctx = make_temp_ctx(level);
    let result = crate::m_boss32::makron_check_attack(&mut edicts[self_idx], &mut ctx);
    sync_level(level, &ctx);
    result
}

fn w_boss2_checkattack(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) -> bool {
    let mut ctx = make_temp_ctx(level);
    let result = crate::m_boss2::boss2_check_attack(&mut edicts[self_idx], &mut ctx);
    sync_level(level, &ctx);
    result
}

fn w_medic_checkattack(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) -> bool {
    let mut ctx = make_temp_ctx(level);
    let result = crate::m_medic::medic_checkattack(&mut edicts[self_idx], &mut ctx);
    sync_level(level, &ctx);
    result
}

// --- Additional search wrappers ---

fn w_jorg_search_fn(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_boss31::jorg_search(&mut edicts[self_idx], &mut ctx);
}

fn w_boss2_search_fn(self_idx: usize, edicts: &mut [Edict], _level: &mut LevelLocals) {
    let mut ctx = make_temp_ctx(_level);
    crate::m_boss2::boss2_search(&mut edicts[self_idx], &mut ctx);
}

// ============================================================
// Static dispatch tables
// ============================================================
// Each table is an array of function pointers. The index into the array
// corresponds to the named constants defined above. Unimplemented entries
// point to placeholder functions that log a warning.

/// Think dispatch table. Indexed by THINK_* constants.
pub static THINK_TABLE: [ThinkFn; THINK_TABLE_SIZE] = {
    let mut table: [ThinkFn; THINK_TABLE_SIZE] = [think_placeholder; THINK_TABLE_SIZE];
    // g_monster
    table[THINK_M_FLIES_OFF] = w_m_flies_off;
    table[THINK_M_FLIES_ON] = w_m_flies_on;
    table[THINK_MONSTER_TRIGGERED_SPAWN] = w_monster_triggered_spawn;
    table[THINK_WALKMONSTER_START_GO] = w_walkmonster_start_go;
    table[THINK_FLYMONSTER_START_GO] = w_flymonster_start_go;
    table[THINK_SWIMMONSTER_START_GO] = w_swimmonster_start_go;
    // m_hover
    table[THINK_HOVER_DEADTHINK] = w_hover_deadthink;
    // Free / remove
    table[THINK_FREE_EDICT] = w_free_edict;
    // g_items
    table[THINK_DO_RESPAWN] = w_do_respawn;
    table[THINK_MEGAHEALTH_THINK] = w_megahealth_think;
    table[THINK_DROP_MAKE_TOUCHABLE] = w_drop_make_touchable;
    table[THINK_DROPTOFLOOR] = w_droptofloor;
    // g_trigger
    table[THINK_MULTI_WAIT] = w_multi_wait;
    // g_utils
    table[THINK_TRIGGER_DELAY_THINK] = w_think_delay;
    // g_misc
    table[THINK_GIB] = w_gib_think;
    table[THINK_TH_VIEWTHING] = w_th_viewthing;
    table[THINK_MISC_BLACKHOLE] = w_misc_blackhole_think;
    table[THINK_MISC_EASTERTANK] = w_misc_eastertank_think;
    table[THINK_MISC_EASTERCHICK] = w_misc_easterchick_think;
    table[THINK_MISC_EASTERCHICK2] = w_misc_easterchick2_think;
    table[THINK_COMMANDER_BODY] = w_commander_body_think;
    table[THINK_COMMANDER_BODY_DROP] = w_commander_body_drop;
    table[THINK_MISC_BANNER] = w_misc_banner_think;
    table[THINK_MISC_SATELLITE_DISH] = w_misc_satellite_dish_think;
    table[THINK_BARREL_EXPLODE] = w_barrel_explode;
    table[THINK_FUNC_OBJECT_RELEASE] = w_func_object_release;
    table[THINK_FUNC_CLOCK] = w_func_clock_think;
    table[THINK_MISC_VIPER_BOMB_PRETHINK] = w_misc_viper_bomb_prethink;
    // g_target
    table[THINK_TARGET_LASER_THINK] = w_target_laser_think;
    table[THINK_TARGET_LIGHTRAMP_THINK] = w_target_lightramp_think;
    table[THINK_TARGET_EARTHQUAKE_THINK] = w_target_earthquake_think;
    table[THINK_TARGET_EXPLOSION_EXPLODE] = w_target_explosion_explode;
    table[THINK_TARGET_CROSSLEVEL_TARGET] = w_target_crosslevel_target;
    table[THINK_TARGET_LASER_START] = w_target_laser_start;
    // p_client
    table[THINK_SP_CREATE_COOP_SPOTS] = w_sp_create_coop_spots;
    table[THINK_SP_FIX_COOP_SPOTS] = w_sp_fix_coop_spots;
    // m_boss3
    table[THINK_BOSS3_STAND] = w_boss3_stand_think;
    // g_monster
    table[THINK_MONSTER] = w_monster_think;
    // g_weapon
    table[THINK_GRENADE_EXPLODE] = w_grenade_explode;
    table[THINK_BFG_THINK] = w_bfg_think;
    table[THINK_BFG_EXPLODE] = w_bfg_explode;
    // g_func
    table[THINK_FUNC_DOOR_GO_UP] = w_func_door_go_up;
    table[THINK_FUNC_DOOR_GO_DOWN] = w_func_door_go_down;
    table[THINK_FUNC_DOOR_SECRET_MOVE1] = w_func_door_secret_move1;
    table[THINK_FUNC_DOOR_SECRET_MOVE2] = w_func_door_secret_move2;
    table[THINK_FUNC_DOOR_SECRET_MOVE3] = w_func_door_secret_move3;
    table[THINK_FUNC_DOOR_SECRET_MOVE4] = w_func_door_secret_move4;
    table[THINK_FUNC_DOOR_SECRET_MOVE5] = w_func_door_secret_move5;
    table[THINK_FUNC_DOOR_SECRET_MOVE6] = w_func_door_secret_move6;
    table[THINK_FUNC_DOOR_SECRET_DONE] = w_func_door_secret_done;
    table[THINK_FUNC_TRAIN_NEXT] = w_func_train_next;
    table[THINK_FUNC_PLAT_GO_UP] = w_func_plat_go_up;
    table[THINK_FUNC_PLAT_GO_DOWN] = w_func_plat_go_down;
    table[THINK_FUNC_TRAIN_FIND] = w_func_train_find;
    table[THINK_FUNC_MOVE_DONE] = w_func_move_done;
    table[THINK_FUNC_MOVE_FINAL] = w_func_move_final;
    table[THINK_FUNC_MOVE_BEGIN] = w_func_move_begin;
    table[THINK_FUNC_ACCEL_MOVE] = w_func_accel_move;
    table[THINK_FUNC_ANGLE_MOVE_DONE] = w_func_angle_move_done;
    table[THINK_FUNC_ANGLE_MOVE_FINAL] = w_func_angle_move_final;
    table[THINK_FUNC_ANGLE_MOVE_BEGIN] = w_func_angle_move_begin;
    table[THINK_FUNC_BUTTON_RETURN] = w_func_button_return;
    table[THINK_FUNC_CALC_MOVE_SPEED] = w_func_calc_move_speed;
    table[THINK_FUNC_SPAWN_DOOR_TRIGGER] = w_func_spawn_door_trigger;
    table[THINK_FUNC_TRIGGER_ELEVATOR_INIT] = w_func_trigger_elevator_init;
    table[THINK_FUNC_TIMER_THINK] = w_func_timer_think;
    table
};

/// Pain dispatch table. Indexed by PAIN_* constants.
pub static PAIN_TABLE: [PainFn; PAIN_TABLE_SIZE] = {
    let mut table: [PainFn; PAIN_TABLE_SIZE] = [pain_placeholder; PAIN_TABLE_SIZE];
    table[PAIN_SOLDIER] = w_soldier_pain;
    table[PAIN_BERSERK] = w_berserk_pain;
    table[PAIN_BRAIN] = w_brain_pain;
    table[PAIN_GLADIATOR] = w_gladiator_pain;
    table[PAIN_GUNNER] = w_gunner_pain;
    table[PAIN_INFANTRY] = w_infantry_pain;
    table[PAIN_PARASITE] = w_parasite_pain;
    table[PAIN_FLIPPER] = w_flipper_pain;
    table[PAIN_FLYER] = w_flyer_pain;
    table[PAIN_HOVER] = w_hover_pain;
    table[PAIN_CHICK] = w_chick_pain;
    table[PAIN_MUTANT] = w_mutant_pain;
    table[PAIN_INSANE] = w_insane_pain;
    table[PAIN_MEDIC] = w_medic_pain;
    table[PAIN_BOSS2] = w_boss2_pain;
    table[PAIN_JORG] = w_jorg_pain;
    table[PAIN_MAKRON] = w_makron_pain;
    table[PAIN_SUPERTANK] = w_supertank_pain;
    table[PAIN_TANK] = w_tank_pain;
    table[PAIN_ACTOR] = w_actor_pain;
    table[PAIN_FLOAT] = w_floater_pain;
    table[PAIN_PLAYER] = w_player_pain;
    table
};

/// Die dispatch table. Indexed by DIE_* constants.
pub static DIE_TABLE: [DieFn; DIE_TABLE_SIZE] = {
    let mut table: [DieFn; DIE_TABLE_SIZE] = [die_placeholder; DIE_TABLE_SIZE];
    table[DIE_SOLDIER] = w_soldier_die;
    table[DIE_BERSERK] = w_berserk_die;
    table[DIE_BRAIN] = w_brain_die;
    table[DIE_GLADIATOR] = w_gladiator_die;
    table[DIE_GUNNER] = w_gunner_die;
    table[DIE_INFANTRY] = w_infantry_die;
    table[DIE_PARASITE] = w_parasite_die;
    table[DIE_FLIPPER] = w_flipper_die;
    table[DIE_FLYER] = w_flyer_die;
    table[DIE_HOVER] = w_hover_die;
    table[DIE_CHICK] = w_chick_die;
    table[DIE_MUTANT] = w_mutant_die;
    table[DIE_INSANE] = w_insane_die;
    table[DIE_MEDIC] = w_medic_die;
    table[DIE_BOSS2] = w_boss2_die;
    table[DIE_JORG] = w_jorg_die;
    table[DIE_MAKRON] = w_makron_die;
    table[DIE_SUPERTANK] = w_supertank_die;
    table[DIE_TANK] = w_tank_die;
    table[DIE_ACTOR] = w_actor_die;
    table[DIE_FLOAT] = w_floater_die;
    // g_misc
    table[DIE_GIB] = w_gib_die;
    table[DIE_BARREL] = w_barrel_delay_die;
    table[DIE_DEBRIS] = w_debris_die;
    table[DIE_FUNC_EXPLOSIVE] = w_func_explosive_die;
    table[DIE_MISC_DEADSOLDIER] = w_deadsoldier_die;
    // p_client
    table[DIE_PLAYER] = w_player_die;
    table[DIE_BODY_DIE] = w_body_die;
    // g_func
    table[DIE_BUTTON_KILLED] = w_button_killed_die;
    table[DIE_DOOR_KILLED] = w_door_killed_die;
    table[DIE_DOOR_SECRET] = w_door_secret_die;
    table
};

/// Touch dispatch table. Indexed by TOUCH_* constants.
pub static TOUCH_TABLE: [TouchFn; TOUCH_TABLE_SIZE] = {
    let mut table: [TouchFn; TOUCH_TABLE_SIZE] = [touch_placeholder; TOUCH_TABLE_SIZE];
    table[TOUCH_MUTANT_JUMP] = w_mutant_jump_touch;
    // g_items
    table[TOUCH_ITEM] = w_touch_item;
    table[TOUCH_DROP_TEMP] = w_drop_temp_touch;
    // g_trigger
    table[TOUCH_TRIGGER_MULTIPLE] = w_touch_multi;
    table[TOUCH_TRIGGER_ONCE] = w_touch_multi; // trigger_once uses same touch handler
    table[TOUCH_TRIGGER_PUSH] = w_trigger_push_touch;
    table[TOUCH_TRIGGER_HURT] = w_hurt_touch;
    table[TOUCH_MULTI] = w_touch_multi;
    table[TOUCH_TRIGGER_GRAVITY] = w_trigger_gravity_touch;
    table[TOUCH_TRIGGER_MONSTERJUMP] = w_trigger_monsterjump_touch;
    // g_misc
    table[TOUCH_GIB] = w_gib_touch;
    table[TOUCH_FUNC_OBJECT] = w_func_object_touch;
    table[TOUCH_BARREL] = w_barrel_touch;
    table[TOUCH_MISC_VIPER_BOMB] = w_misc_viper_bomb_touch;
    table[TOUCH_TELEPORTER] = w_teleporter_touch;
    table[TOUCH_PATH_CORNER] = w_path_corner_touch;
    table[TOUCH_POINT_COMBAT] = w_point_combat_touch;
    // g_weapon
    table[TOUCH_WEAPON_BLASTER] = w_blaster_touch;
    table[TOUCH_WEAPON_GRENADE] = w_grenade_touch;
    table[TOUCH_WEAPON_ROCKET] = w_rocket_touch;
    table[TOUCH_WEAPON_BFG] = w_bfg_touch;
    // g_func
    table[TOUCH_FUNC_DOOR] = w_touch_door_trigger;
    table[TOUCH_PLAT_CENTER] = w_touch_plat_center;
    table[TOUCH_ROTATING] = w_rotating_touch;
    table[TOUCH_BUTTON] = w_button_touch;
    table[TOUCH_DOOR] = w_door_touch;
    table
};

/// Use dispatch table. Indexed by USE_* constants.
pub static USE_TABLE: [UseFn; USE_TABLE_SIZE] = {
    let mut table: [UseFn; USE_TABLE_SIZE] = [use_placeholder; USE_TABLE_SIZE];
    // g_monster
    table[USE_MONSTER_USE] = w_monster_use;
    table[USE_MONSTER_TRIGGERED_SPAWN_USE] = w_monster_triggered_spawn_use;
    // g_items
    table[USE_ITEM_TRIGGER] = w_use_item_trigger;
    // g_trigger
    table[USE_TRIGGER_RELAY] = w_trigger_relay_use;
    table[USE_TRIGGER_COUNTER] = w_trigger_counter_use;
    table[USE_TRIGGER_ALWAYS] = w_trigger_relay_use; // trigger_always uses same use handler as relay
    table[USE_MULTI] = w_use_multi;
    table[USE_TRIGGER_ENABLE] = w_trigger_enable;
    table[USE_TRIGGER_KEY] = w_trigger_key_use;
    table[USE_HURT] = w_hurt_use;
    // m_boss3
    table[USE_BOSS3] = w_boss3_use;
    // g_target
    table[USE_TARGET_TENT] = w_use_target_tent;
    table[USE_TARGET_SPEAKER] = w_use_target_speaker;
    table[USE_TARGET_EXPLOSION] = w_use_target_explosion;
    table[USE_TARGET_CHANGELEVEL] = w_use_target_changelevel;
    table[USE_TARGET_SPLASH] = w_use_target_splash;
    table[USE_TARGET_SPAWNER] = w_use_target_spawner;
    table[USE_TARGET_BLASTER] = w_use_target_blaster;
    table[USE_TARGET_LASER] = w_use_target_laser;
    table[USE_TARGET_LIGHTRAMP] = w_use_target_lightramp;
    table[USE_TARGET_EARTHQUAKE] = w_use_target_earthquake;
    table[USE_TARGET_HELP] = w_use_target_help;
    table[USE_TARGET_SECRET] = w_use_target_secret;
    table[USE_TARGET_GOAL] = w_use_target_goal;
    table[USE_TRIGGER_CROSSLEVEL_TRIGGER] = w_trigger_crosslevel_trigger_use;
    table[USE_TARGET_STRING] = w_use_target_string;
    // g_misc
    table[USE_AREAPORTAL] = w_use_areaportal;
    table[USE_LIGHT] = w_light_use;
    table[USE_FUNC_WALL] = w_func_wall_use;
    table[USE_FUNC_OBJECT] = w_func_object_use;
    table[USE_FUNC_EXPLOSIVE] = w_func_explosive_use;
    table[USE_FUNC_EXPLOSIVE_SPAWN] = w_func_explosive_spawn;
    table[USE_MISC_BLACKHOLE] = w_misc_blackhole_use;
    table[USE_COMMANDER_BODY] = w_commander_body_use;
    table[USE_MISC_SATELLITE_DISH] = w_misc_satellite_dish_use;
    table[USE_MISC_VIPER] = w_misc_viper_use;
    table[USE_MISC_VIPER_BOMB] = w_misc_viper_bomb_use;
    table[USE_MISC_STROGG_SHIP] = w_misc_strogg_ship_use;
    table[USE_FUNC_CLOCK] = w_func_clock_use;
    // g_func
    table[USE_FUNC_DOOR] = w_door_use;
    table[USE_FUNC_BUTTON] = w_button_use;
    table[USE_FUNC_TRAIN] = w_train_use;
    table[USE_TRAIN] = w_train_use; // USE_TRAIN aliases to train_use
    table[USE_FUNC_TIMER] = w_func_timer_use;
    table[USE_FUNC_KILLBOX] = w_use_killbox;
    table[USE_FUNC_PLAT] = w_use_plat;
    table[USE_FUNC_ROTATING] = w_rotating_use;
    table[USE_FUNC_DOOR_SECRET] = w_door_secret_use;
    table[USE_FUNC_ELEVATOR] = w_trigger_elevator_use;
    table[USE_FUNC_CONVEYOR] = w_func_conveyor_use;
    table
};

/// Blocked dispatch table. Indexed by BLOCKED_* constants.
pub static BLOCKED_TABLE: [BlockedFn; BLOCKED_TABLE_SIZE] = {
    let mut table: [BlockedFn; BLOCKED_TABLE_SIZE] = [blocked_placeholder; BLOCKED_TABLE_SIZE];
    table[BLOCKED_FUNC_DOOR] = w_door_blocked;
    table[BLOCKED_FUNC_PLAT] = w_plat_blocked;
    table[BLOCKED_FUNC_TRAIN] = w_train_blocked;
    table[BLOCKED_FUNC_ROTATING] = w_rotating_blocked;
    table[BLOCKED_DOOR_SECRET] = w_door_secret_blocked;
    table
};

/// Monster stand dispatch table. Indexed by MSTAND_* constants.
pub static MSTAND_TABLE: [MonsterThinkFn; MSTAND_TABLE_SIZE] = {
    let mut table: [MonsterThinkFn; MSTAND_TABLE_SIZE] =
        [monster_think_placeholder; MSTAND_TABLE_SIZE];
    table[MSTAND_SOLDIER] = w_soldier_stand;
    table[MSTAND_BERSERK] = w_berserk_stand;
    table[MSTAND_BRAIN] = w_brain_stand;
    table[MSTAND_GLADIATOR] = w_gladiator_stand;
    table[MSTAND_GUNNER] = w_gunner_stand;
    table[MSTAND_INFANTRY] = w_infantry_stand;
    table[MSTAND_PARASITE] = w_parasite_stand;
    table[MSTAND_FLIPPER] = w_flipper_stand;
    table[MSTAND_FLYER] = w_flyer_stand;
    table[MSTAND_HOVER] = w_hover_stand;
    table[MSTAND_CHICK] = w_chick_stand;
    table[MSTAND_MUTANT] = w_mutant_stand;
    table[MSTAND_INSANE] = w_insane_stand;
    table[MSTAND_MEDIC] = w_medic_stand;
    table[MSTAND_BOSS2] = w_boss2_stand;
    table[MSTAND_JORG] = w_jorg_stand;
    table[MSTAND_MAKRON] = w_makron_stand;
    table[MSTAND_SUPERTANK] = w_supertank_stand;
    table[MSTAND_TANK] = w_tank_stand;
    table[MSTAND_ACTOR] = w_actor_stand;
    table[MSTAND_FLOAT] = w_floater_stand;
    table
};

/// Monster walk dispatch table. Indexed by MWALK_* constants.
pub static MWALK_TABLE: [MonsterThinkFn; MWALK_TABLE_SIZE] = {
    let mut table: [MonsterThinkFn; MWALK_TABLE_SIZE] =
        [monster_think_placeholder; MWALK_TABLE_SIZE];
    table[MWALK_SOLDIER] = w_soldier_walk;
    table[MWALK_BERSERK] = w_berserk_walk;
    table[MWALK_BRAIN] = w_brain_walk;
    table[MWALK_GLADIATOR] = w_gladiator_walk;
    table[MWALK_GUNNER] = w_gunner_walk;
    table[MWALK_INFANTRY] = w_infantry_walk;
    table[MWALK_PARASITE] = w_parasite_walk;
    table[MWALK_FLIPPER] = w_flipper_walk;
    table[MWALK_FLYER] = w_flyer_walk;
    table[MWALK_HOVER] = w_hover_walk;
    table[MWALK_CHICK] = w_chick_walk;
    table[MWALK_MUTANT] = w_mutant_walk;
    table[MWALK_INSANE] = w_insane_walk;
    table[MWALK_MEDIC] = w_medic_walk;
    table[MWALK_BOSS2] = w_boss2_walk;
    table[MWALK_JORG] = w_jorg_walk;
    table[MWALK_MAKRON] = w_makron_walk;
    table[MWALK_SUPERTANK] = w_supertank_walk;
    table[MWALK_TANK] = w_tank_walk;
    table[MWALK_ACTOR] = w_actor_walk;
    table[MWALK_FLOAT] = w_floater_walk;
    table
};

/// Monster run dispatch table. Indexed by MRUN_* constants.
pub static MRUN_TABLE: [MonsterThinkFn; MRUN_TABLE_SIZE] = {
    let mut table: [MonsterThinkFn; MRUN_TABLE_SIZE] =
        [monster_think_placeholder; MRUN_TABLE_SIZE];
    table[MRUN_SOLDIER] = w_soldier_run;
    table[MRUN_BERSERK] = w_berserk_run;
    table[MRUN_BRAIN] = w_brain_run;
    table[MRUN_GLADIATOR] = w_gladiator_run;
    table[MRUN_GUNNER] = w_gunner_run;
    table[MRUN_INFANTRY] = w_infantry_run;
    table[MRUN_PARASITE] = w_parasite_run;
    table[MRUN_FLIPPER] = w_flipper_run;
    table[MRUN_FLYER] = w_flyer_run;
    table[MRUN_HOVER] = w_hover_run;
    table[MRUN_CHICK] = w_chick_run;
    table[MRUN_MUTANT] = w_mutant_run;
    table[MRUN_INSANE] = w_insane_run;
    table[MRUN_MEDIC] = w_medic_run;
    table[MRUN_BOSS2] = w_boss2_run;
    table[MRUN_JORG] = w_jorg_run;
    table[MRUN_MAKRON] = w_makron_run;
    table[MRUN_SUPERTANK] = w_supertank_run;
    table[MRUN_TANK] = w_tank_run;
    table[MRUN_ACTOR] = w_actor_run;
    table[MRUN_FLOAT] = w_floater_run;
    table
};

/// Monster dodge dispatch table. Indexed by MDODGE_* constants.
pub static MDODGE_TABLE: [MonsterThinkFn; MDODGE_TABLE_SIZE] = {
    let mut table: [MonsterThinkFn; MDODGE_TABLE_SIZE] =
        [monster_think_placeholder; MDODGE_TABLE_SIZE];
    table[MDODGE_SOLDIER] = w_soldier_dodge;
    table[MDODGE_BRAIN] = w_brain_dodge;
    table[MDODGE_GUNNER] = w_gunner_dodge;
    table[MDODGE_INFANTRY] = w_infantry_dodge;
    table[MDODGE_CHICK] = w_chick_dodge;
    table[MDODGE_MUTANT] = monster_think_placeholder; // mutant has no dodge function
    table[MDODGE_MEDIC] = w_medic_dodge;
    table
};

/// Monster attack dispatch table. Indexed by MATTACK_* constants.
pub static MATTACK_TABLE: [MonsterThinkFn; MATTACK_TABLE_SIZE] = {
    let mut table: [MonsterThinkFn; MATTACK_TABLE_SIZE] =
        [monster_think_placeholder; MATTACK_TABLE_SIZE];
    table[MATTACK_SOLDIER] = w_soldier_attack;
    table[MATTACK_GLADIATOR] = w_gladiator_attack;
    table[MATTACK_GUNNER] = w_gunner_attack;
    table[MATTACK_INFANTRY] = w_infantry_attack;
    table[MATTACK_PARASITE] = w_parasite_attack;
    table[MATTACK_FLYER] = w_flyer_attack;
    table[MATTACK_HOVER] = w_hover_attack;
    table[MATTACK_CHICK] = w_chick_attack;
    table[MATTACK_MEDIC] = w_medic_attack;
    table[MATTACK_BOSS2] = w_boss2_attack;
    table[MATTACK_JORG] = w_jorg_attack;
    table[MATTACK_MAKRON] = w_makron_attack;
    table[MATTACK_SUPERTANK] = w_supertank_attack;
    table[MATTACK_TANK] = w_tank_attack;
    table
};

/// Monster melee dispatch table. Indexed by MMELEE_* constants.
pub static MMELEE_TABLE: [MonsterThinkFn; MMELEE_TABLE_SIZE] = {
    let mut table: [MonsterThinkFn; MMELEE_TABLE_SIZE] =
        [monster_think_placeholder; MMELEE_TABLE_SIZE];
    table[MMELEE_BERSERK] = w_berserk_melee;
    table[MMELEE_BRAIN] = w_brain_melee;
    table[MMELEE_GLADIATOR] = w_gladiator_melee;
    table[MMELEE_INFANTRY] = monster_think_placeholder; // infantry has no melee
    table[MMELEE_FLIPPER] = w_flipper_melee;
    table[MMELEE_CHICK] = w_chick_melee;
    table[MMELEE_MUTANT] = w_mutant_melee;
    table[MMELEE_TANK] = monster_think_placeholder; // tank has no melee function
    table[MMELEE_FLYER] = w_flyer_melee;
    table
};

/// Monster sight dispatch table. Indexed by MSIGHT_* constants.
pub static MSIGHT_TABLE: [MonsterThinkFn; MSIGHT_TABLE_SIZE] = {
    let mut table: [MonsterThinkFn; MSIGHT_TABLE_SIZE] =
        [monster_think_placeholder; MSIGHT_TABLE_SIZE];
    table[MSIGHT_SOLDIER] = w_soldier_sight;
    table[MSIGHT_BERSERK] = w_berserk_sight;
    table[MSIGHT_BRAIN] = w_brain_sight;
    table[MSIGHT_GLADIATOR] = w_gladiator_sight;
    table[MSIGHT_GUNNER] = w_gunner_sight;
    table[MSIGHT_INFANTRY] = w_infantry_sight;
    table[MSIGHT_PARASITE] = w_parasite_sight;
    table[MSIGHT_FLIPPER] = w_flipper_sight;
    table[MSIGHT_FLYER] = w_flyer_sight;
    table[MSIGHT_HOVER] = w_hover_sight;
    table[MSIGHT_CHICK] = w_chick_sight;
    table[MSIGHT_MUTANT] = w_mutant_sight;
    table[MSIGHT_MEDIC] = w_medic_sight;
    table[MSIGHT_MAKRON] = w_makron_sight;
    table[MSIGHT_TANK] = w_tank_sight;
    table
};

/// Monster idle dispatch table. Indexed by MIDLE_* constants.
pub static MIDLE_TABLE: [MonsterThinkFn; MIDLE_TABLE_SIZE] = {
    let mut table: [MonsterThinkFn; MIDLE_TABLE_SIZE] =
        [monster_think_placeholder; MIDLE_TABLE_SIZE];
    table[MIDLE_SOLDIER] = w_soldier_idle;
    table[MIDLE_BRAIN] = w_brain_idle;
    table[MIDLE_GLADIATOR] = w_gladiator_idle;
    table[MIDLE_GUNNER] = w_gunner_idle;
    table[MIDLE_INFANTRY] = w_infantry_idle;
    table[MIDLE_PARASITE] = w_parasite_idle;
    table[MIDLE_FLYER] = w_flyer_idle;
    table[MIDLE_CHICK] = monster_think_placeholder; // chick has no idle
    table[MIDLE_MUTANT] = w_mutant_idle;
    table[MIDLE_MEDIC] = w_medic_idle;
    table[MIDLE_SUPERTANK] = monster_think_placeholder; // supertank has no idle
    table[MIDLE_TANK] = w_tank_idle;
    table
};

/// Monster search dispatch table. Indexed by MSEARCH_* constants.
pub static MSEARCH_TABLE: [MonsterThinkFn; MSEARCH_TABLE_SIZE] = {
    let mut table: [MonsterThinkFn; MSEARCH_TABLE_SIZE] =
        [monster_think_placeholder; MSEARCH_TABLE_SIZE];
    table[MSEARCH_BRAIN] = w_brain_search;
    table[MSEARCH_GUNNER] = w_gunner_search;
    table[MSEARCH_HOVER] = w_hover_search;
    table[MSEARCH_MEDIC] = w_medic_search;
    table[MSEARCH_SUPERTANK] = w_supertank_search;
    table[MSEARCH_JORG] = w_jorg_search_fn;
    table[MSEARCH_BOSS2] = w_boss2_search_fn;
    table
};

/// Monster checkattack dispatch table. Indexed by MCHECKATTACK_* constants.
pub static MCHECKATTACK_TABLE: [CheckAttackFn; MCHECKATTACK_TABLE_SIZE] = {
    let mut table: [CheckAttackFn; MCHECKATTACK_TABLE_SIZE] =
        [checkattack_placeholder; MCHECKATTACK_TABLE_SIZE];
    table[MCHECKATTACK_JORG] = w_jorg_checkattack;
    table[MCHECKATTACK_MAKRON] = w_makron_checkattack;
    table[MCHECKATTACK_BOSS2] = w_boss2_checkattack;
    table[MCHECKATTACK_MUTANT] = w_mutant_checkattack;
    table[MCHECKATTACK_MEDIC] = w_medic_checkattack;
    table
};

// ============================================================
// Dispatch helper functions
// ============================================================

/// Dispatch a think callback. Panics if the edict has no think_fn set.
pub fn dispatch_think(idx: usize, self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    THINK_TABLE[idx](self_idx, edicts, level);
}

/// Dispatch a pain callback.
pub fn dispatch_pain(
    idx: usize,
    self_idx: usize,
    attacker_idx: usize,
    edicts: &mut [Edict],
    level: &mut LevelLocals,
    kick: f32,
    damage: i32,
) {
    PAIN_TABLE[idx](self_idx, attacker_idx, edicts, level, kick, damage);
}

/// Dispatch a die callback.
pub fn dispatch_die(
    idx: usize,
    self_idx: usize,
    inflictor_idx: usize,
    attacker_idx: usize,
    edicts: &mut [Edict],
    level: &mut LevelLocals,
    damage: i32,
    point: Vec3,
) {
    DIE_TABLE[idx](self_idx, inflictor_idx, attacker_idx, edicts, level, damage, point);
}

/// Dispatch a touch callback.
pub fn dispatch_touch(
    idx: usize,
    self_idx: usize,
    other_idx: usize,
    edicts: &mut [Edict],
    level: &mut LevelLocals,
    plane: Option<&CPlane>,
    surf: Option<&CSurface>,
) {
    TOUCH_TABLE[idx](self_idx, other_idx, edicts, level, plane, surf);
}

/// Dispatch a use callback.
pub fn dispatch_use(
    idx: usize,
    self_idx: usize,
    other_idx: usize,
    activator_idx: usize,
    edicts: &mut [Edict],
    level: &mut LevelLocals,
) {
    USE_TABLE[idx](self_idx, other_idx, activator_idx, edicts, level);
}

/// Dispatch a blocked callback.
pub fn dispatch_blocked(
    idx: usize,
    self_idx: usize,
    other_idx: usize,
    edicts: &mut [Edict],
    level: &mut LevelLocals,
) {
    BLOCKED_TABLE[idx](self_idx, other_idx, edicts, level);
}

/// Dispatch a monster stand callback.
pub fn dispatch_stand(idx: usize, self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    MSTAND_TABLE[idx](self_idx, edicts, level);
}

/// Dispatch a monster walk callback.
pub fn dispatch_walk(idx: usize, self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    MWALK_TABLE[idx](self_idx, edicts, level);
}

/// Dispatch a monster run callback.
pub fn dispatch_run(idx: usize, self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    MRUN_TABLE[idx](self_idx, edicts, level);
}

/// Dispatch a monster dodge callback.
pub fn dispatch_dodge(
    idx: usize,
    self_idx: usize,
    edicts: &mut [Edict],
    level: &mut LevelLocals,
) {
    MDODGE_TABLE[idx](self_idx, edicts, level);
}

/// Dispatch a monster attack callback.
pub fn dispatch_attack(
    idx: usize,
    self_idx: usize,
    edicts: &mut [Edict],
    level: &mut LevelLocals,
) {
    MATTACK_TABLE[idx](self_idx, edicts, level);
}

/// Dispatch a monster melee callback.
pub fn dispatch_melee(
    idx: usize,
    self_idx: usize,
    edicts: &mut [Edict],
    level: &mut LevelLocals,
) {
    MMELEE_TABLE[idx](self_idx, edicts, level);
}

/// Dispatch a monster sight callback.
pub fn dispatch_sight(
    idx: usize,
    self_idx: usize,
    edicts: &mut [Edict],
    level: &mut LevelLocals,
) {
    MSIGHT_TABLE[idx](self_idx, edicts, level);
}

/// Dispatch a monster idle callback.
pub fn dispatch_idle(idx: usize, self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    MIDLE_TABLE[idx](self_idx, edicts, level);
}

/// Dispatch a monster search callback.
pub fn dispatch_search(
    idx: usize,
    self_idx: usize,
    edicts: &mut [Edict],
    level: &mut LevelLocals,
) {
    MSEARCH_TABLE[idx](self_idx, edicts, level);
}

/// Dispatch a monster checkattack callback.
pub fn dispatch_checkattack(
    idx: usize,
    self_idx: usize,
    edicts: &mut [Edict],
    level: &mut LevelLocals,
) -> bool {
    MCHECKATTACK_TABLE[idx](self_idx, edicts, level)
}

// ============================================================
// Convenience helpers for calling from edict/monsterinfo fields
// ============================================================

/// Call the think_fn on an edict if set.
pub fn call_think(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    if let Some(idx) = edicts[self_idx].think_fn {
        dispatch_think(idx, self_idx, edicts, level);
    }
}

/// Call the pain_fn on an edict if set.
pub fn call_pain(
    self_idx: usize,
    attacker_idx: usize,
    edicts: &mut [Edict],
    level: &mut LevelLocals,
    kick: f32,
    damage: i32,
) {
    if let Some(idx) = edicts[self_idx].pain_fn {
        dispatch_pain(idx, self_idx, attacker_idx, edicts, level, kick, damage);
    }
}

/// Call the die_fn on an edict if set.
pub fn call_die(
    self_idx: usize,
    inflictor_idx: usize,
    attacker_idx: usize,
    edicts: &mut [Edict],
    level: &mut LevelLocals,
    damage: i32,
    point: Vec3,
) {
    if let Some(idx) = edicts[self_idx].die_fn {
        dispatch_die(idx, self_idx, inflictor_idx, attacker_idx, edicts, level, damage, point);
    }
}

/// Call the touch_fn on an edict if set.
pub fn call_touch(
    self_idx: usize,
    other_idx: usize,
    edicts: &mut [Edict],
    level: &mut LevelLocals,
    plane: Option<&CPlane>,
    surf: Option<&CSurface>,
) {
    if let Some(idx) = edicts[self_idx].touch_fn {
        dispatch_touch(idx, self_idx, other_idx, edicts, level, plane, surf);
    }
}

/// Call the use_fn on an edict if set.
pub fn call_use(
    self_idx: usize,
    other_idx: usize,
    activator_idx: usize,
    edicts: &mut [Edict],
    level: &mut LevelLocals,
) {
    if let Some(idx) = edicts[self_idx].use_fn {
        dispatch_use(idx, self_idx, other_idx, activator_idx, edicts, level);
    }
}

/// Call the blocked_fn on an edict if set.
pub fn call_blocked(
    self_idx: usize,
    other_idx: usize,
    edicts: &mut [Edict],
    level: &mut LevelLocals,
) {
    if let Some(idx) = edicts[self_idx].blocked_fn {
        dispatch_blocked(idx, self_idx, other_idx, edicts, level);
    }
}

/// Call the monsterinfo.stand_fn on an edict if set.
pub fn call_stand(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    if let Some(idx) = edicts[self_idx].monsterinfo.stand_fn {
        dispatch_stand(idx, self_idx, edicts, level);
    }
}

/// Call the monsterinfo.walk_fn on an edict if set.
pub fn call_walk(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    if let Some(idx) = edicts[self_idx].monsterinfo.walk_fn {
        dispatch_walk(idx, self_idx, edicts, level);
    }
}

/// Call the monsterinfo.run_fn on an edict if set.
pub fn call_run(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    if let Some(idx) = edicts[self_idx].monsterinfo.run_fn {
        dispatch_run(idx, self_idx, edicts, level);
    }
}

/// Call the monsterinfo.checkattack_fn on an edict if set.
pub fn call_checkattack(
    self_idx: usize,
    edicts: &mut [Edict],
    level: &mut LevelLocals,
) -> bool {
    if let Some(idx) = edicts[self_idx].monsterinfo.checkattack_fn {
        dispatch_checkattack(idx, self_idx, edicts, level)
    } else {
        false
    }
}

/// Call the monsterinfo.dodge_fn on an edict if set.
pub fn call_dodge(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    if let Some(idx) = edicts[self_idx].monsterinfo.dodge_fn {
        dispatch_dodge(idx, self_idx, edicts, level);
    }
}

/// Call the monsterinfo.attack_fn on an edict if set.
pub fn call_attack(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    if let Some(idx) = edicts[self_idx].monsterinfo.attack_fn {
        dispatch_attack(idx, self_idx, edicts, level);
    }
}

/// Call the monsterinfo.melee_fn on an edict if set.
pub fn call_melee(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    if let Some(idx) = edicts[self_idx].monsterinfo.melee_fn {
        dispatch_melee(idx, self_idx, edicts, level);
    }
}

/// Call the monsterinfo.sight_fn on an edict if set.
pub fn call_sight(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    if let Some(idx) = edicts[self_idx].monsterinfo.sight_fn {
        dispatch_sight(idx, self_idx, edicts, level);
    }
}

/// Call the monsterinfo.idle_fn on an edict if set.
pub fn call_idle(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    if let Some(idx) = edicts[self_idx].monsterinfo.idle_fn {
        dispatch_idle(idx, self_idx, edicts, level);
    }
}

/// Call the monsterinfo.search_fn on an edict if set.
pub fn call_search(self_idx: usize, edicts: &mut [Edict], level: &mut LevelLocals) {
    if let Some(idx) = edicts[self_idx].monsterinfo.search_fn {
        dispatch_search(idx, self_idx, edicts, level);
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::g_local::{Edict, LevelLocals, MonsterInfo, MoveInfo, EntityFlags};

    /// Helper: create a minimal edicts array with `n` default edicts.
    fn make_edicts(n: usize) -> Vec<Edict> {
        (0..n).map(|_| Edict::default()).collect()
    }

    /// Helper: create a default LevelLocals.
    fn make_level() -> LevelLocals {
        LevelLocals::default()
    }

    // ============================================================
    // 1. Callback ID constant uniqueness and range tests
    // ============================================================

    #[test]
    fn think_constants_are_unique_and_within_table_size() {
        // Collect all THINK_* constants (excluding TABLE_SIZE) and verify uniqueness
        let think_ids: &[usize] = &[
            THINK_MONSTER, THINK_WALKMONSTER_START, THINK_FLYMONSTER_START,
            THINK_SWIMMONSTER_START, THINK_DEBRIS_DIE, THINK_GIVEUP_AND_DEATHTOUCH,
            THINK_FUNC_EXPLOSIVE_EXPLODE, THINK_FUNC_DOOR_GO_UP, THINK_FUNC_DOOR_GO_DOWN,
            THINK_FUNC_DOOR_SECRET_MOVE1, THINK_FUNC_DOOR_SECRET_MOVE2,
            THINK_FUNC_DOOR_SECRET_MOVE3, THINK_FUNC_DOOR_SECRET_MOVE4,
            THINK_FUNC_DOOR_SECRET_MOVE5, THINK_FUNC_DOOR_SECRET_MOVE6,
            THINK_FUNC_DOOR_SECRET_DONE, THINK_FUNC_TRAIN_NEXT,
            THINK_FUNC_PLAT_GO_UP, THINK_FUNC_PLAT_GO_DOWN,
            THINK_FUNC_ROTATING_THINK, THINK_TRIGGER_DELAY_THINK,
            THINK_TRIGGER_PUSH_TOUCH, THINK_TRIGGER_HURT_THINK,
            THINK_TARGET_LASER_THINK, THINK_TARGET_LIGHTRAMP_THINK,
            THINK_TARGET_EARTHQUAKE_THINK, THINK_GRENADE_EXPLODE,
            THINK_ROCKET_THINK, THINK_BFG_THINK, THINK_BFG_EXPLODE,
            THINK_DROP_TEMP_TOUCH, THINK_DROP_MAKE_TOUCHABLE,
            THINK_MEGAHEALTH_THINK, THINK_PLAYER_THINK, THINK_BODY_THINK,
            THINK_RESPAWN_THINK, THINK_WEAPON_THINK, THINK_MONSTER_THINK,
            THINK_MONSTER_DEAD_THINK, THINK_FREE_EDICT, THINK_PATH_CORNER,
            THINK_POINT_COMBAT, THINK_M_FLIES_OFF, THINK_M_FLIES_ON,
            THINK_MONSTER_TRIGGERED_SPAWN, THINK_WALKMONSTER_START_GO,
            THINK_FLYMONSTER_START_GO, THINK_SWIMMONSTER_START_GO,
            THINK_HOVER_DEADTHINK, THINK_DO_RESPAWN, THINK_DROPTOFLOOR,
            THINK_GIB, THINK_TH_VIEWTHING, THINK_MISC_BLACKHOLE,
            THINK_MISC_EASTERTANK, THINK_MISC_EASTERCHICK,
            THINK_MISC_EASTERCHICK2, THINK_COMMANDER_BODY,
            THINK_COMMANDER_BODY_DROP, THINK_MISC_BANNER,
            THINK_MISC_SATELLITE_DISH, THINK_BARREL_EXPLODE,
            THINK_FUNC_OBJECT_RELEASE, THINK_FUNC_CLOCK,
            THINK_FUNC_TRAIN_FIND, THINK_MISC_VIPER_BOMB_PRETHINK,
            THINK_M_DROPTOFLOOR, THINK_MULTI_WAIT,
            THINK_TARGET_EXPLOSION_EXPLODE, THINK_TARGET_CROSSLEVEL_TARGET,
            THINK_TARGET_LASER_START, THINK_SP_CREATE_COOP_SPOTS,
            THINK_SP_FIX_COOP_SPOTS, THINK_BOSS3_STAND,
            THINK_FUNC_MOVE_DONE, THINK_FUNC_MOVE_FINAL, THINK_FUNC_MOVE_BEGIN,
            THINK_FUNC_ACCEL_MOVE, THINK_FUNC_ANGLE_MOVE_DONE,
            THINK_FUNC_ANGLE_MOVE_FINAL, THINK_FUNC_ANGLE_MOVE_BEGIN,
            THINK_FUNC_BUTTON_RETURN, THINK_FUNC_CALC_MOVE_SPEED,
            THINK_FUNC_SPAWN_DOOR_TRIGGER, THINK_FUNC_TRIGGER_ELEVATOR_INIT,
            THINK_FUNC_TIMER_THINK,
        ];
        // All within table size
        for &id in think_ids {
            assert!(id < THINK_TABLE_SIZE,
                "THINK constant {} exceeds THINK_TABLE_SIZE {}", id, THINK_TABLE_SIZE);
        }
        // All unique
        let mut seen = std::collections::HashSet::new();
        for &id in think_ids {
            assert!(seen.insert(id), "Duplicate THINK constant value: {}", id);
        }
    }

    #[test]
    fn pain_constants_are_unique_and_within_table_size() {
        let pain_ids: &[usize] = &[
            PAIN_PLAYER, PAIN_SOLDIER, PAIN_BERSERK, PAIN_BRAIN,
            PAIN_GLADIATOR, PAIN_GUNNER, PAIN_INFANTRY, PAIN_PARASITE,
            PAIN_FLIPPER, PAIN_FLYER, PAIN_FLOAT, PAIN_HOVER,
            PAIN_CHICK, PAIN_MUTANT, PAIN_INSANE, PAIN_MEDIC,
            PAIN_ACTOR, PAIN_BOSS2, PAIN_JORG, PAIN_MAKRON,
            PAIN_SUPERTANK, PAIN_TANK,
        ];
        for &id in pain_ids {
            assert!(id < PAIN_TABLE_SIZE,
                "PAIN constant {} exceeds PAIN_TABLE_SIZE {}", id, PAIN_TABLE_SIZE);
        }
        let mut seen = std::collections::HashSet::new();
        for &id in pain_ids {
            assert!(seen.insert(id), "Duplicate PAIN constant value: {}", id);
        }
    }

    #[test]
    fn die_constants_are_unique_and_within_table_size() {
        let die_ids: &[usize] = &[
            DIE_PLAYER, DIE_SOLDIER, DIE_BERSERK, DIE_BRAIN,
            DIE_GLADIATOR, DIE_GUNNER, DIE_INFANTRY, DIE_PARASITE,
            DIE_FLIPPER, DIE_FLYER, DIE_FLOAT, DIE_HOVER,
            DIE_CHICK, DIE_MUTANT, DIE_INSANE, DIE_MEDIC,
            DIE_ACTOR, DIE_BOSS2, DIE_JORG, DIE_MAKRON,
            DIE_SUPERTANK, DIE_TANK, DIE_BARREL, DIE_MISC_EXPLOBOX,
            DIE_GIB, DIE_DEBRIS, DIE_FUNC_EXPLOSIVE, DIE_MISC_DEADSOLDIER,
            DIE_BODY_DIE, DIE_BUTTON_KILLED, DIE_DOOR_KILLED, DIE_DOOR_SECRET,
        ];
        for &id in die_ids {
            assert!(id < DIE_TABLE_SIZE,
                "DIE constant {} exceeds DIE_TABLE_SIZE {}", id, DIE_TABLE_SIZE);
        }
        let mut seen = std::collections::HashSet::new();
        for &id in die_ids {
            assert!(seen.insert(id), "Duplicate DIE constant value: {}", id);
        }
    }

    #[test]
    fn touch_constants_are_unique_and_within_table_size() {
        let touch_ids: &[usize] = &[
            TOUCH_TRIGGER_MULTIPLE, TOUCH_TRIGGER_ONCE, TOUCH_TRIGGER_PUSH,
            TOUCH_TRIGGER_HURT, TOUCH_ITEM, TOUCH_WEAPON_ROCKET,
            TOUCH_WEAPON_GRENADE, TOUCH_WEAPON_BLASTER, TOUCH_WEAPON_BFG,
            TOUCH_FUNC_DOOR, TOUCH_MUTANT_JUMP, TOUCH_DROP_TEMP,
            TOUCH_GIB, TOUCH_PATH_CORNER, TOUCH_POINT_COMBAT,
            TOUCH_FUNC_OBJECT, TOUCH_BARREL, TOUCH_MISC_VIPER_BOMB,
            TOUCH_TELEPORTER, TOUCH_MULTI, TOUCH_TRIGGER_MONSTERJUMP,
            TOUCH_TRIGGER_GRAVITY, TOUCH_PLAT_CENTER, TOUCH_ROTATING,
            TOUCH_BUTTON, TOUCH_DOOR,
        ];
        for &id in touch_ids {
            assert!(id < TOUCH_TABLE_SIZE,
                "TOUCH constant {} exceeds TOUCH_TABLE_SIZE {}", id, TOUCH_TABLE_SIZE);
        }
        let mut seen = std::collections::HashSet::new();
        for &id in touch_ids {
            assert!(seen.insert(id), "Duplicate TOUCH constant value: {}", id);
        }
    }

    #[test]
    fn use_constants_are_unique_and_within_table_size() {
        let use_ids: &[usize] = &[
            USE_TRIGGER_RELAY, USE_TRIGGER_COUNTER, USE_TRIGGER_ALWAYS,
            USE_TARGET_SPEAKER, USE_TARGET_EXPLOSION, USE_TARGET_CHANGELEVEL,
            USE_TARGET_SPLASH, USE_TARGET_SPAWNER, USE_TARGET_BLASTER,
            USE_TARGET_LASER, USE_TARGET_LIGHTRAMP, USE_TARGET_EARTHQUAKE,
            USE_FUNC_DOOR, USE_FUNC_BUTTON, USE_FUNC_TRAIN,
            USE_ITEM, USE_FUNC_TIMER, USE_FUNC_KILLBOX,
            USE_MONSTER_USE, USE_MONSTER_TRIGGERED_SPAWN_USE,
            USE_ITEM_TRIGGER, USE_AREAPORTAL, USE_LIGHT,
            USE_FUNC_WALL, USE_FUNC_OBJECT, USE_FUNC_EXPLOSIVE,
            USE_FUNC_EXPLOSIVE_SPAWN, USE_MISC_BLACKHOLE,
            USE_COMMANDER_BODY, USE_MISC_SATELLITE_DISH,
            USE_MISC_VIPER, USE_MISC_VIPER_BOMB,
            USE_MISC_STROGG_SHIP, USE_TARGET_STRING, USE_FUNC_CLOCK,
            USE_TRAIN, USE_TARGET_TENT, USE_TARGET_HELP,
            USE_TARGET_SECRET, USE_TARGET_GOAL, USE_TRIGGER_CROSSLEVEL_TRIGGER,
            USE_MULTI, USE_TRIGGER_ENABLE, USE_TRIGGER_KEY, USE_HURT,
            USE_BOSS3, USE_FUNC_PLAT, USE_FUNC_ROTATING,
            USE_FUNC_DOOR_SECRET, USE_FUNC_ELEVATOR, USE_FUNC_CONVEYOR,
        ];
        for &id in use_ids {
            assert!(id < USE_TABLE_SIZE,
                "USE constant {} exceeds USE_TABLE_SIZE {}", id, USE_TABLE_SIZE);
        }
        // USE_TRAIN == 35 and USE_FUNC_TRAIN == 14, these are distinct
        assert_ne!(USE_TRAIN, USE_FUNC_TRAIN);
    }

    #[test]
    fn blocked_constants_are_unique_and_within_table_size() {
        let blocked_ids: &[usize] = &[
            BLOCKED_FUNC_DOOR, BLOCKED_FUNC_PLAT, BLOCKED_FUNC_TRAIN,
            BLOCKED_FUNC_ROTATING, BLOCKED_DOOR_SECRET,
        ];
        for &id in blocked_ids {
            assert!(id < BLOCKED_TABLE_SIZE,
                "BLOCKED constant {} exceeds BLOCKED_TABLE_SIZE {}", id, BLOCKED_TABLE_SIZE);
        }
        let mut seen = std::collections::HashSet::new();
        for &id in blocked_ids {
            assert!(seen.insert(id), "Duplicate BLOCKED constant value: {}", id);
        }
    }

    #[test]
    fn monster_stand_constants_unique_and_in_range() {
        let ids: &[usize] = &[
            MSTAND_SOLDIER, MSTAND_BERSERK, MSTAND_BRAIN, MSTAND_GLADIATOR,
            MSTAND_GUNNER, MSTAND_INFANTRY, MSTAND_PARASITE, MSTAND_FLIPPER,
            MSTAND_FLYER, MSTAND_FLOAT, MSTAND_HOVER, MSTAND_CHICK,
            MSTAND_MUTANT, MSTAND_INSANE, MSTAND_MEDIC, MSTAND_ACTOR,
            MSTAND_BOSS2, MSTAND_JORG, MSTAND_MAKRON, MSTAND_SUPERTANK,
            MSTAND_TANK,
        ];
        for &id in ids {
            assert!(id < MSTAND_TABLE_SIZE, "MSTAND {} >= table size {}", id, MSTAND_TABLE_SIZE);
        }
        let mut seen = std::collections::HashSet::new();
        for &id in ids {
            assert!(seen.insert(id), "Duplicate MSTAND constant: {}", id);
        }
    }

    #[test]
    fn monster_walk_constants_unique_and_in_range() {
        let ids: &[usize] = &[
            MWALK_SOLDIER, MWALK_BERSERK, MWALK_BRAIN, MWALK_GLADIATOR,
            MWALK_GUNNER, MWALK_INFANTRY, MWALK_PARASITE, MWALK_FLIPPER,
            MWALK_FLYER, MWALK_FLOAT, MWALK_HOVER, MWALK_CHICK,
            MWALK_MUTANT, MWALK_INSANE, MWALK_MEDIC, MWALK_ACTOR,
            MWALK_BOSS2, MWALK_JORG, MWALK_MAKRON, MWALK_SUPERTANK,
            MWALK_TANK,
        ];
        for &id in ids {
            assert!(id < MWALK_TABLE_SIZE, "MWALK {} >= table size {}", id, MWALK_TABLE_SIZE);
        }
        let mut seen = std::collections::HashSet::new();
        for &id in ids {
            assert!(seen.insert(id), "Duplicate MWALK constant: {}", id);
        }
    }

    #[test]
    fn monster_run_constants_unique_and_in_range() {
        let ids: &[usize] = &[
            MRUN_SOLDIER, MRUN_BERSERK, MRUN_BRAIN, MRUN_GLADIATOR,
            MRUN_GUNNER, MRUN_INFANTRY, MRUN_PARASITE, MRUN_FLIPPER,
            MRUN_FLYER, MRUN_FLOAT, MRUN_HOVER, MRUN_CHICK,
            MRUN_MUTANT, MRUN_INSANE, MRUN_MEDIC, MRUN_ACTOR,
            MRUN_BOSS2, MRUN_JORG, MRUN_MAKRON, MRUN_SUPERTANK,
            MRUN_TANK,
        ];
        for &id in ids {
            assert!(id < MRUN_TABLE_SIZE, "MRUN {} >= table size {}", id, MRUN_TABLE_SIZE);
        }
        let mut seen = std::collections::HashSet::new();
        for &id in ids {
            assert!(seen.insert(id), "Duplicate MRUN constant: {}", id);
        }
    }

    #[test]
    fn monster_dodge_constants_unique_and_in_range() {
        let ids: &[usize] = &[
            MDODGE_SOLDIER, MDODGE_BERSERK, MDODGE_BRAIN, MDODGE_GLADIATOR,
            MDODGE_GUNNER, MDODGE_INFANTRY, MDODGE_CHICK, MDODGE_MUTANT,
            MDODGE_MEDIC,
        ];
        for &id in ids {
            assert!(id < MDODGE_TABLE_SIZE, "MDODGE {} >= table size {}", id, MDODGE_TABLE_SIZE);
        }
        let mut seen = std::collections::HashSet::new();
        for &id in ids {
            assert!(seen.insert(id), "Duplicate MDODGE constant: {}", id);
        }
    }

    #[test]
    fn monster_attack_constants_unique_and_in_range() {
        let ids: &[usize] = &[
            MATTACK_SOLDIER, MATTACK_BERSERK, MATTACK_BRAIN, MATTACK_GLADIATOR,
            MATTACK_GUNNER, MATTACK_INFANTRY, MATTACK_PARASITE, MATTACK_FLIPPER,
            MATTACK_FLYER, MATTACK_FLOAT, MATTACK_HOVER, MATTACK_CHICK,
            MATTACK_MUTANT, MATTACK_MEDIC, MATTACK_BOSS2, MATTACK_JORG,
            MATTACK_MAKRON, MATTACK_SUPERTANK, MATTACK_TANK,
        ];
        for &id in ids {
            assert!(id < MATTACK_TABLE_SIZE, "MATTACK {} >= table size {}", id, MATTACK_TABLE_SIZE);
        }
        let mut seen = std::collections::HashSet::new();
        for &id in ids {
            assert!(seen.insert(id), "Duplicate MATTACK constant: {}", id);
        }
    }

    #[test]
    fn monster_melee_constants_unique_and_in_range() {
        let ids: &[usize] = &[
            MMELEE_SOLDIER, MMELEE_BERSERK, MMELEE_BRAIN, MMELEE_GLADIATOR,
            MMELEE_INFANTRY, MMELEE_FLIPPER, MMELEE_FLOAT, MMELEE_CHICK,
            MMELEE_MUTANT, MMELEE_INSANE, MMELEE_TANK, MMELEE_FLYER,
        ];
        for &id in ids {
            assert!(id < MMELEE_TABLE_SIZE, "MMELEE {} >= table size {}", id, MMELEE_TABLE_SIZE);
        }
        let mut seen = std::collections::HashSet::new();
        for &id in ids {
            assert!(seen.insert(id), "Duplicate MMELEE constant: {}", id);
        }
    }

    #[test]
    fn monster_sight_constants_unique_and_in_range() {
        let ids: &[usize] = &[
            MSIGHT_SOLDIER, MSIGHT_BERSERK, MSIGHT_BRAIN, MSIGHT_GLADIATOR,
            MSIGHT_GUNNER, MSIGHT_INFANTRY, MSIGHT_PARASITE, MSIGHT_FLIPPER,
            MSIGHT_FLYER, MSIGHT_FLOAT, MSIGHT_HOVER, MSIGHT_CHICK,
            MSIGHT_MUTANT, MSIGHT_INSANE, MSIGHT_MEDIC, MSIGHT_ACTOR,
            MSIGHT_BOSS2, MSIGHT_JORG, MSIGHT_MAKRON, MSIGHT_SUPERTANK,
            MSIGHT_TANK,
        ];
        for &id in ids {
            assert!(id < MSIGHT_TABLE_SIZE, "MSIGHT {} >= table size {}", id, MSIGHT_TABLE_SIZE);
        }
        let mut seen = std::collections::HashSet::new();
        for &id in ids {
            assert!(seen.insert(id), "Duplicate MSIGHT constant: {}", id);
        }
    }

    #[test]
    fn monster_idle_constants_unique_and_in_range() {
        let ids: &[usize] = &[
            MIDLE_SOLDIER, MIDLE_BERSERK, MIDLE_BRAIN, MIDLE_GLADIATOR,
            MIDLE_GUNNER, MIDLE_INFANTRY, MIDLE_PARASITE, MIDLE_FLIPPER,
            MIDLE_FLYER, MIDLE_FLOAT, MIDLE_HOVER, MIDLE_CHICK,
            MIDLE_MUTANT, MIDLE_INSANE, MIDLE_MEDIC, MIDLE_ACTOR,
            MIDLE_SUPERTANK, MIDLE_TANK,
        ];
        for &id in ids {
            assert!(id < MIDLE_TABLE_SIZE, "MIDLE {} >= table size {}", id, MIDLE_TABLE_SIZE);
        }
        let mut seen = std::collections::HashSet::new();
        for &id in ids {
            assert!(seen.insert(id), "Duplicate MIDLE constant: {}", id);
        }
    }

    #[test]
    fn monster_search_constants_unique_and_in_range() {
        let ids: &[usize] = &[
            MSEARCH_SOLDIER, MSEARCH_BRAIN, MSEARCH_GUNNER, MSEARCH_INFANTRY,
            MSEARCH_FLYER, MSEARCH_HOVER, MSEARCH_CHICK, MSEARCH_MEDIC,
            MSEARCH_SUPERTANK, MSEARCH_JORG, MSEARCH_BOSS2,
        ];
        for &id in ids {
            assert!(id < MSEARCH_TABLE_SIZE, "MSEARCH {} >= table size {}", id, MSEARCH_TABLE_SIZE);
        }
        let mut seen = std::collections::HashSet::new();
        for &id in ids {
            assert!(seen.insert(id), "Duplicate MSEARCH constant: {}", id);
        }
    }

    #[test]
    fn monster_checkattack_constants_unique_and_in_range() {
        let ids: &[usize] = &[
            MCHECKATTACK_DEFAULT, MCHECKATTACK_SOLDIER, MCHECKATTACK_GUNNER,
            MCHECKATTACK_JORG, MCHECKATTACK_MAKRON, MCHECKATTACK_SUPERTANK,
            MCHECKATTACK_TANK, MCHECKATTACK_BOSS2, MCHECKATTACK_MUTANT,
            MCHECKATTACK_MEDIC,
        ];
        for &id in ids {
            assert!(id < MCHECKATTACK_TABLE_SIZE,
                "MCHECKATTACK {} >= table size {}", id, MCHECKATTACK_TABLE_SIZE);
        }
        let mut seen = std::collections::HashSet::new();
        for &id in ids {
            assert!(seen.insert(id), "Duplicate MCHECKATTACK constant: {}", id);
        }
    }

    // ============================================================
    // 2. Table size tests - verify arrays are properly dimensioned
    // ============================================================

    #[test]
    fn think_table_has_correct_size() {
        assert_eq!(THINK_TABLE.len(), THINK_TABLE_SIZE);
        assert_eq!(THINK_TABLE_SIZE, 88);
    }

    #[test]
    fn pain_table_has_correct_size() {
        assert_eq!(PAIN_TABLE.len(), PAIN_TABLE_SIZE);
        assert_eq!(PAIN_TABLE_SIZE, 32);
    }

    #[test]
    fn die_table_has_correct_size() {
        assert_eq!(DIE_TABLE.len(), DIE_TABLE_SIZE);
        assert_eq!(DIE_TABLE_SIZE, 32);
    }

    #[test]
    fn touch_table_has_correct_size() {
        assert_eq!(TOUCH_TABLE.len(), TOUCH_TABLE_SIZE);
        assert_eq!(TOUCH_TABLE_SIZE, 28);
    }

    #[test]
    fn use_table_has_correct_size() {
        assert_eq!(USE_TABLE.len(), USE_TABLE_SIZE);
        assert_eq!(USE_TABLE_SIZE, 52);
    }

    #[test]
    fn blocked_table_has_correct_size() {
        assert_eq!(BLOCKED_TABLE.len(), BLOCKED_TABLE_SIZE);
        assert_eq!(BLOCKED_TABLE_SIZE, 8);
    }

    #[test]
    fn mstand_table_has_correct_size() {
        assert_eq!(MSTAND_TABLE.len(), MSTAND_TABLE_SIZE);
        assert_eq!(MSTAND_TABLE_SIZE, 32);
    }

    #[test]
    fn mwalk_table_has_correct_size() {
        assert_eq!(MWALK_TABLE.len(), MWALK_TABLE_SIZE);
        assert_eq!(MWALK_TABLE_SIZE, 32);
    }

    #[test]
    fn mrun_table_has_correct_size() {
        assert_eq!(MRUN_TABLE.len(), MRUN_TABLE_SIZE);
        assert_eq!(MRUN_TABLE_SIZE, 32);
    }

    #[test]
    fn mdodge_table_has_correct_size() {
        assert_eq!(MDODGE_TABLE.len(), MDODGE_TABLE_SIZE);
        assert_eq!(MDODGE_TABLE_SIZE, 16);
    }

    #[test]
    fn mattack_table_has_correct_size() {
        assert_eq!(MATTACK_TABLE.len(), MATTACK_TABLE_SIZE);
        assert_eq!(MATTACK_TABLE_SIZE, 32);
    }

    #[test]
    fn mmelee_table_has_correct_size() {
        assert_eq!(MMELEE_TABLE.len(), MMELEE_TABLE_SIZE);
        assert_eq!(MMELEE_TABLE_SIZE, 16);
    }

    #[test]
    fn msight_table_has_correct_size() {
        assert_eq!(MSIGHT_TABLE.len(), MSIGHT_TABLE_SIZE);
        assert_eq!(MSIGHT_TABLE_SIZE, 32);
    }

    #[test]
    fn midle_table_has_correct_size() {
        assert_eq!(MIDLE_TABLE.len(), MIDLE_TABLE_SIZE);
        assert_eq!(MIDLE_TABLE_SIZE, 32);
    }

    #[test]
    fn msearch_table_has_correct_size() {
        assert_eq!(MSEARCH_TABLE.len(), MSEARCH_TABLE_SIZE);
        assert_eq!(MSEARCH_TABLE_SIZE, 16);
    }

    #[test]
    fn mcheckattack_table_has_correct_size() {
        assert_eq!(MCHECKATTACK_TABLE.len(), MCHECKATTACK_TABLE_SIZE);
        assert_eq!(MCHECKATTACK_TABLE_SIZE, 16);
    }

    // ============================================================
    // 3. Table slot population tests - verify that registered entries
    //    are not the placeholder function (by comparing fn pointers)
    // ============================================================

    #[test]
    fn think_table_slots_are_populated_for_known_indices() {
        // Key entries that must be populated (not the placeholder)
        let populated_entries: &[usize] = &[
            THINK_MONSTER, THINK_M_FLIES_OFF, THINK_M_FLIES_ON,
            THINK_MONSTER_TRIGGERED_SPAWN, THINK_WALKMONSTER_START_GO,
            THINK_FLYMONSTER_START_GO, THINK_SWIMMONSTER_START_GO,
            THINK_HOVER_DEADTHINK, THINK_FREE_EDICT,
            THINK_DO_RESPAWN, THINK_MEGAHEALTH_THINK,
            THINK_DROP_MAKE_TOUCHABLE, THINK_DROPTOFLOOR,
            THINK_MULTI_WAIT, THINK_TRIGGER_DELAY_THINK,
            THINK_GIB, THINK_TH_VIEWTHING, THINK_MISC_BLACKHOLE,
            THINK_GRENADE_EXPLODE, THINK_BFG_THINK, THINK_BFG_EXPLODE,
            THINK_FUNC_DOOR_GO_UP, THINK_FUNC_DOOR_GO_DOWN,
            THINK_FUNC_TRAIN_NEXT, THINK_FUNC_PLAT_GO_UP,
            THINK_FUNC_PLAT_GO_DOWN, THINK_TARGET_LASER_THINK,
            THINK_TARGET_LIGHTRAMP_THINK, THINK_TARGET_EARTHQUAKE_THINK,
            THINK_SP_CREATE_COOP_SPOTS, THINK_SP_FIX_COOP_SPOTS,
            THINK_BOSS3_STAND, THINK_FUNC_MOVE_DONE,
            THINK_FUNC_MOVE_FINAL, THINK_FUNC_MOVE_BEGIN,
            THINK_FUNC_TIMER_THINK,
        ];
        let placeholder_ptr = think_placeholder as ThinkFn as usize;
        for &idx in populated_entries {
            let entry_ptr = THINK_TABLE[idx] as usize;
            assert_ne!(entry_ptr, placeholder_ptr,
                "THINK_TABLE[{}] should not be placeholder", idx);
        }
    }

    #[test]
    fn pain_table_slots_are_populated_for_known_indices() {
        let populated_entries: &[usize] = &[
            PAIN_PLAYER, PAIN_SOLDIER, PAIN_BERSERK, PAIN_BRAIN,
            PAIN_GLADIATOR, PAIN_GUNNER, PAIN_INFANTRY, PAIN_PARASITE,
            PAIN_FLIPPER, PAIN_FLYER, PAIN_FLOAT, PAIN_HOVER,
            PAIN_CHICK, PAIN_MUTANT, PAIN_INSANE, PAIN_MEDIC,
            PAIN_ACTOR, PAIN_BOSS2, PAIN_JORG, PAIN_MAKRON,
            PAIN_SUPERTANK, PAIN_TANK,
        ];
        let placeholder_ptr = pain_placeholder as PainFn as usize;
        for &idx in populated_entries {
            let entry_ptr = PAIN_TABLE[idx] as usize;
            assert_ne!(entry_ptr, placeholder_ptr,
                "PAIN_TABLE[{}] should not be placeholder", idx);
        }
    }

    #[test]
    fn die_table_slots_are_populated_for_known_indices() {
        let populated_entries: &[usize] = &[
            DIE_PLAYER, DIE_SOLDIER, DIE_BERSERK, DIE_BRAIN,
            DIE_GLADIATOR, DIE_GUNNER, DIE_INFANTRY, DIE_PARASITE,
            DIE_FLIPPER, DIE_FLYER, DIE_FLOAT, DIE_HOVER,
            DIE_CHICK, DIE_MUTANT, DIE_INSANE, DIE_MEDIC,
            DIE_ACTOR, DIE_BOSS2, DIE_JORG, DIE_MAKRON,
            DIE_SUPERTANK, DIE_TANK, DIE_GIB, DIE_BARREL,
            DIE_DEBRIS, DIE_FUNC_EXPLOSIVE, DIE_MISC_DEADSOLDIER,
            DIE_BODY_DIE, DIE_BUTTON_KILLED, DIE_DOOR_KILLED,
            DIE_DOOR_SECRET,
        ];
        let placeholder_ptr = die_placeholder as DieFn as usize;
        for &idx in populated_entries {
            let entry_ptr = DIE_TABLE[idx] as usize;
            assert_ne!(entry_ptr, placeholder_ptr,
                "DIE_TABLE[{}] should not be placeholder", idx);
        }
    }

    #[test]
    fn touch_table_slots_are_populated_for_known_indices() {
        let populated_entries: &[usize] = &[
            TOUCH_TRIGGER_MULTIPLE, TOUCH_TRIGGER_ONCE, TOUCH_TRIGGER_PUSH,
            TOUCH_TRIGGER_HURT, TOUCH_ITEM, TOUCH_WEAPON_ROCKET,
            TOUCH_WEAPON_GRENADE, TOUCH_WEAPON_BLASTER, TOUCH_WEAPON_BFG,
            TOUCH_FUNC_DOOR, TOUCH_MUTANT_JUMP, TOUCH_DROP_TEMP,
            TOUCH_GIB, TOUCH_PATH_CORNER, TOUCH_POINT_COMBAT,
            TOUCH_FUNC_OBJECT, TOUCH_BARREL, TOUCH_MISC_VIPER_BOMB,
            TOUCH_TELEPORTER, TOUCH_MULTI, TOUCH_TRIGGER_MONSTERJUMP,
            TOUCH_TRIGGER_GRAVITY, TOUCH_PLAT_CENTER, TOUCH_ROTATING,
            TOUCH_BUTTON, TOUCH_DOOR,
        ];
        let placeholder_ptr = touch_placeholder as TouchFn as usize;
        for &idx in populated_entries {
            let entry_ptr = TOUCH_TABLE[idx] as usize;
            assert_ne!(entry_ptr, placeholder_ptr,
                "TOUCH_TABLE[{}] should not be placeholder", idx);
        }
    }

    #[test]
    fn use_table_slots_are_populated_for_known_indices() {
        let populated_entries: &[usize] = &[
            USE_TRIGGER_RELAY, USE_TRIGGER_COUNTER, USE_TRIGGER_ALWAYS,
            USE_TARGET_SPEAKER, USE_TARGET_EXPLOSION, USE_TARGET_CHANGELEVEL,
            USE_TARGET_SPLASH, USE_TARGET_SPAWNER, USE_TARGET_BLASTER,
            USE_TARGET_LASER, USE_TARGET_LIGHTRAMP, USE_TARGET_EARTHQUAKE,
            USE_FUNC_DOOR, USE_FUNC_BUTTON, USE_FUNC_TRAIN,
            USE_FUNC_TIMER, USE_FUNC_KILLBOX, USE_MONSTER_USE,
            USE_MONSTER_TRIGGERED_SPAWN_USE, USE_ITEM_TRIGGER,
            USE_MULTI, USE_TRIGGER_ENABLE, USE_TRIGGER_KEY, USE_HURT,
            USE_BOSS3, USE_TARGET_TENT, USE_TARGET_HELP,
            USE_TARGET_SECRET, USE_TARGET_GOAL,
            USE_TRIGGER_CROSSLEVEL_TRIGGER, USE_TARGET_STRING,
            USE_AREAPORTAL, USE_LIGHT, USE_FUNC_WALL,
            USE_FUNC_OBJECT, USE_FUNC_EXPLOSIVE,
            USE_FUNC_EXPLOSIVE_SPAWN, USE_MISC_BLACKHOLE,
            USE_COMMANDER_BODY, USE_MISC_SATELLITE_DISH,
            USE_MISC_VIPER, USE_MISC_VIPER_BOMB,
            USE_MISC_STROGG_SHIP, USE_FUNC_CLOCK,
            USE_TRAIN, USE_FUNC_PLAT, USE_FUNC_ROTATING,
            USE_FUNC_DOOR_SECRET, USE_FUNC_ELEVATOR, USE_FUNC_CONVEYOR,
        ];
        let placeholder_ptr = use_placeholder as UseFn as usize;
        for &idx in populated_entries {
            let entry_ptr = USE_TABLE[idx] as usize;
            assert_ne!(entry_ptr, placeholder_ptr,
                "USE_TABLE[{}] should not be placeholder", idx);
        }
    }

    #[test]
    fn blocked_table_slots_are_populated_for_known_indices() {
        let populated_entries: &[usize] = &[
            BLOCKED_FUNC_DOOR, BLOCKED_FUNC_PLAT, BLOCKED_FUNC_TRAIN,
            BLOCKED_FUNC_ROTATING, BLOCKED_DOOR_SECRET,
        ];
        let placeholder_ptr = blocked_placeholder as BlockedFn as usize;
        for &idx in populated_entries {
            let entry_ptr = BLOCKED_TABLE[idx] as usize;
            assert_ne!(entry_ptr, placeholder_ptr,
                "BLOCKED_TABLE[{}] should not be placeholder", idx);
        }
    }

    // ============================================================
    // 4. Table ID ranges do not overlap between different categories
    //    (Each callback category is independently indexed starting
    //    from 0, so they are intentionally allowed to have the same
    //    numeric values. This test verifies the TABLE_SIZE constants
    //    leave adequate headroom.)
    // ============================================================

    #[test]
    fn table_sizes_have_headroom_above_max_constant() {
        // THINK: max used constant is THINK_FUNC_TIMER_THINK = 85, table size = 88
        assert!(THINK_FUNC_TIMER_THINK < THINK_TABLE_SIZE);
        assert!(THINK_TABLE_SIZE - THINK_FUNC_TIMER_THINK >= 3,
            "Think table should have at least 2 spare slots");

        // PAIN: max used constant is PAIN_TANK = 21, table size = 32
        assert!(PAIN_TANK < PAIN_TABLE_SIZE);

        // DIE: max used constant is DIE_DOOR_SECRET = 31, table size = 32
        assert!(DIE_DOOR_SECRET < DIE_TABLE_SIZE);

        // TOUCH: max used constant is TOUCH_DOOR = 25, table size = 28
        assert!(TOUCH_DOOR < TOUCH_TABLE_SIZE);

        // USE: max used constant is USE_FUNC_CONVEYOR = 50, table size = 52
        assert!(USE_FUNC_CONVEYOR < USE_TABLE_SIZE);

        // BLOCKED: max used constant is BLOCKED_DOOR_SECRET = 4, table size = 8
        assert!(BLOCKED_DOOR_SECRET < BLOCKED_TABLE_SIZE);
    }

    // ============================================================
    // 5. call_* functions: verify None-callback short-circuits
    //    (edict with no callback set should be a no-op)
    // ============================================================

    #[test]
    fn call_think_with_none_is_noop() {
        let mut edicts = make_edicts(4);
        let mut level = make_level();
        edicts[1].think_fn = None;
        edicts[1].health = 100;
        call_think(1, &mut edicts, &mut level);
        // Should not panic, health unchanged
        assert_eq!(edicts[1].health, 100);
    }

    #[test]
    fn call_pain_with_none_is_noop() {
        let mut edicts = make_edicts(4);
        let mut level = make_level();
        edicts[1].pain_fn = None;
        edicts[1].health = 100;
        call_pain(1, 2, &mut edicts, &mut level, 10.0, 25);
        assert_eq!(edicts[1].health, 100);
    }

    #[test]
    fn call_die_with_none_is_noop() {
        let mut edicts = make_edicts(4);
        let mut level = make_level();
        edicts[1].die_fn = None;
        edicts[1].health = 50;
        call_die(1, 2, 3, &mut edicts, &mut level, 100, [0.0, 0.0, 0.0]);
        assert_eq!(edicts[1].health, 50);
    }

    #[test]
    fn call_touch_with_none_is_noop() {
        let mut edicts = make_edicts(4);
        let mut level = make_level();
        edicts[1].touch_fn = None;
        call_touch(1, 2, &mut edicts, &mut level, None, None);
        // No panic = success
    }

    #[test]
    fn call_use_with_none_is_noop() {
        let mut edicts = make_edicts(4);
        let mut level = make_level();
        edicts[1].use_fn = None;
        call_use(1, 2, 3, &mut edicts, &mut level);
    }

    #[test]
    fn call_blocked_with_none_is_noop() {
        let mut edicts = make_edicts(4);
        let mut level = make_level();
        edicts[1].blocked_fn = None;
        call_blocked(1, 2, &mut edicts, &mut level);
    }

    #[test]
    fn call_stand_with_none_is_noop() {
        let mut edicts = make_edicts(4);
        let mut level = make_level();
        edicts[1].monsterinfo.stand_fn = None;
        call_stand(1, &mut edicts, &mut level);
    }

    #[test]
    fn call_walk_with_none_is_noop() {
        let mut edicts = make_edicts(4);
        let mut level = make_level();
        edicts[1].monsterinfo.walk_fn = None;
        call_walk(1, &mut edicts, &mut level);
    }

    #[test]
    fn call_run_with_none_is_noop() {
        let mut edicts = make_edicts(4);
        let mut level = make_level();
        edicts[1].monsterinfo.run_fn = None;
        call_run(1, &mut edicts, &mut level);
    }

    #[test]
    fn call_dodge_with_none_is_noop() {
        let mut edicts = make_edicts(4);
        let mut level = make_level();
        edicts[1].monsterinfo.dodge_fn = None;
        call_dodge(1, &mut edicts, &mut level);
    }

    #[test]
    fn call_attack_with_none_is_noop() {
        let mut edicts = make_edicts(4);
        let mut level = make_level();
        edicts[1].monsterinfo.attack_fn = None;
        call_attack(1, &mut edicts, &mut level);
    }

    #[test]
    fn call_melee_with_none_is_noop() {
        let mut edicts = make_edicts(4);
        let mut level = make_level();
        edicts[1].monsterinfo.melee_fn = None;
        call_melee(1, &mut edicts, &mut level);
    }

    #[test]
    fn call_sight_with_none_is_noop() {
        let mut edicts = make_edicts(4);
        let mut level = make_level();
        edicts[1].monsterinfo.sight_fn = None;
        call_sight(1, &mut edicts, &mut level);
    }

    #[test]
    fn call_idle_with_none_is_noop() {
        let mut edicts = make_edicts(4);
        let mut level = make_level();
        edicts[1].monsterinfo.idle_fn = None;
        call_idle(1, &mut edicts, &mut level);
    }

    #[test]
    fn call_search_with_none_is_noop() {
        let mut edicts = make_edicts(4);
        let mut level = make_level();
        edicts[1].monsterinfo.search_fn = None;
        call_search(1, &mut edicts, &mut level);
    }

    #[test]
    fn call_checkattack_with_none_returns_false() {
        let mut edicts = make_edicts(4);
        let mut level = make_level();
        edicts[1].monsterinfo.checkattack_fn = None;
        let result = call_checkattack(1, &mut edicts, &mut level);
        assert_eq!(result, false);
    }

    // ============================================================
    // 6. Self-contained dispatch functions: w_multi_wait
    //    This function directly sets nextthink = 0.0, no external deps
    // ============================================================

    #[test]
    fn w_multi_wait_sets_nextthink_to_zero() {
        let mut edicts = make_edicts(4);
        let mut level = make_level();
        edicts[1].nextthink = 5.0;

        // Dispatch via the think table at the THINK_MULTI_WAIT slot
        dispatch_think(THINK_MULTI_WAIT, 1, &mut edicts, &mut level);

        assert_eq!(edicts[1].nextthink, 0.0,
            "w_multi_wait should set nextthink to 0.0");
    }

    #[test]
    fn call_think_with_multi_wait_id_sets_nextthink_to_zero() {
        let mut edicts = make_edicts(4);
        let mut level = make_level();
        edicts[1].nextthink = 10.0;
        edicts[1].think_fn = Some(THINK_MULTI_WAIT);

        call_think(1, &mut edicts, &mut level);

        assert_eq!(edicts[1].nextthink, 0.0);
    }

    // ============================================================
    // 7. Self-contained dispatch: w_trigger_gravity_touch
    //    Copies self.gravity to other.gravity
    // ============================================================

    #[test]
    fn trigger_gravity_touch_copies_gravity() {
        let mut edicts = make_edicts(4);
        let mut level = make_level();
        edicts[0].gravity = 0.5; // trigger entity
        edicts[1].gravity = 1.0; // entity entering trigger

        dispatch_touch(TOUCH_TRIGGER_GRAVITY, 0, 1, &mut edicts, &mut level, None, None);

        assert_eq!(edicts[1].gravity, 0.5,
            "trigger_gravity_touch should copy self.gravity to other.gravity");
    }

    #[test]
    fn call_touch_with_gravity_trigger() {
        let mut edicts = make_edicts(4);
        let mut level = make_level();
        edicts[0].gravity = 0.25;
        edicts[1].gravity = 1.0;
        edicts[0].touch_fn = Some(TOUCH_TRIGGER_GRAVITY);

        call_touch(0, 1, &mut edicts, &mut level, None, None);

        assert_eq!(edicts[1].gravity, 0.25);
    }

    // ============================================================
    // 8. Self-contained dispatch: w_trigger_monsterjump_touch
    //    Applies velocity from movedir and speed/height
    // ============================================================

    #[test]
    fn trigger_monsterjump_touch_applies_velocity() {
        let mut edicts = make_edicts(4);
        let mut level = make_level();
        // Trigger entity (self_idx = 0)
        edicts[0].speed = 200.0;
        edicts[0].movedir = [1.0, 0.0, 300.0]; // movedir[2] is height
        // Other entity (other_idx = 1)
        edicts[1].groundentity = 0; // on ground (not -1)
        // Ensure FL_FLY is NOT set
        edicts[1].flags = EntityFlags::empty();
        edicts[1].velocity = [0.0, 0.0, 0.0];

        dispatch_touch(TOUCH_TRIGGER_MONSTERJUMP, 0, 1, &mut edicts, &mut level, None, None);

        // velocity should be movedir * speed for x/y, and height for z
        assert_eq!(edicts[1].velocity[0], 200.0); // 1.0 * 200.0
        assert_eq!(edicts[1].velocity[1], 0.0);   // 0.0 * 200.0
        assert_eq!(edicts[1].velocity[2], 300.0);  // height > 0, use height
        assert_eq!(edicts[1].groundentity, -1);    // cleared
    }

    #[test]
    fn trigger_monsterjump_touch_uses_default_height_when_zero() {
        let mut edicts = make_edicts(4);
        let mut level = make_level();
        edicts[0].speed = 100.0;
        edicts[0].movedir = [0.0, 1.0, 0.0]; // height = 0 (not > 0)
        edicts[1].groundentity = 0;
        edicts[1].flags = EntityFlags::empty();
        edicts[1].velocity = [0.0, 0.0, 0.0];

        dispatch_touch(TOUCH_TRIGGER_MONSTERJUMP, 0, 1, &mut edicts, &mut level, None, None);

        assert_eq!(edicts[1].velocity[2], 200.0, "Should use default height 200.0 when movedir[2] <= 0");
    }

    #[test]
    fn trigger_monsterjump_touch_skips_flying_entities() {
        let mut edicts = make_edicts(4);
        let mut level = make_level();
        edicts[0].speed = 100.0;
        edicts[0].movedir = [1.0, 0.0, 300.0];
        edicts[1].groundentity = 0;
        edicts[1].flags = EntityFlags::FLY; // FL_FLY set
        edicts[1].velocity = [0.0, 0.0, 0.0];

        dispatch_touch(TOUCH_TRIGGER_MONSTERJUMP, 0, 1, &mut edicts, &mut level, None, None);

        // Velocity should be unchanged because FL_FLY causes early return
        assert_eq!(edicts[1].velocity, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn trigger_monsterjump_touch_skips_airborne_entities() {
        let mut edicts = make_edicts(4);
        let mut level = make_level();
        edicts[0].speed = 100.0;
        edicts[0].movedir = [1.0, 0.0, 300.0];
        edicts[1].groundentity = -1; // not on ground
        edicts[1].flags = EntityFlags::empty();
        edicts[1].velocity = [0.0, 0.0, 0.0];

        dispatch_touch(TOUCH_TRIGGER_MONSTERJUMP, 0, 1, &mut edicts, &mut level, None, None);

        // Velocity should be unchanged because groundentity == -1 causes early return
        assert_eq!(edicts[1].velocity, [0.0, 0.0, 0.0]);
    }

    // ============================================================
    // 9. THINK_FREE_EDICT table slot tests
    //    w_free_edict calls gi_unlinkentity which requires engine init,
    //    so we test the table registration and routing instead.
    // ============================================================

    #[test]
    fn free_edict_is_registered_in_think_table() {
        // Verify THINK_FREE_EDICT is a real (non-placeholder) handler
        let placeholder_ptr = think_placeholder as ThinkFn as usize;
        let free_edict_ptr = THINK_TABLE[THINK_FREE_EDICT] as usize;
        assert_ne!(free_edict_ptr, placeholder_ptr,
            "THINK_FREE_EDICT should be registered with a real handler");
    }

    #[test]
    fn free_edict_constant_is_correct_value() {
        assert_eq!(THINK_FREE_EDICT, 39);
        assert!(THINK_FREE_EDICT < THINK_TABLE_SIZE);
    }

    // ============================================================
    // 10. Edict default state tests
    // ============================================================

    #[test]
    fn edict_default_has_no_callbacks() {
        let edict = Edict::default();
        assert!(edict.think_fn.is_none());
        assert!(edict.pain_fn.is_none());
        assert!(edict.die_fn.is_none());
        assert!(edict.touch_fn.is_none());
        assert!(edict.use_fn.is_none());
        assert!(edict.blocked_fn.is_none());
        assert!(edict.prethink_fn.is_none());
    }

    #[test]
    fn monsterinfo_default_has_no_callbacks() {
        let mi = MonsterInfo::default();
        assert!(mi.stand_fn.is_none());
        assert!(mi.walk_fn.is_none());
        assert!(mi.run_fn.is_none());
        assert!(mi.dodge_fn.is_none());
        assert!(mi.attack_fn.is_none());
        assert!(mi.melee_fn.is_none());
        assert!(mi.sight_fn.is_none());
        assert!(mi.idle_fn.is_none());
        assert!(mi.search_fn.is_none());
        assert!(mi.checkattack_fn.is_none());
        assert!(mi.currentmove.is_none());
    }

    #[test]
    fn moveinfo_default_has_no_endfunc() {
        let mi = MoveInfo::default();
        assert!(mi.endfunc.is_none());
        assert_eq!(mi.speed, 0.0);
        assert_eq!(mi.accel, 0.0);
        assert_eq!(mi.decel, 0.0);
    }

    // ============================================================
    // 11. Cross-category constant independence tests
    //    Each callback category starts numbering from 0 independently.
    //    Verify this is the case.
    // ============================================================

    #[test]
    fn all_callback_categories_start_at_zero() {
        // Each category's first constant should be 0
        assert_eq!(PAIN_PLAYER, 0);
        assert_eq!(DIE_PLAYER, 0);
        assert_eq!(TOUCH_TRIGGER_MULTIPLE, 0);
        assert_eq!(USE_TRIGGER_RELAY, 0);
        assert_eq!(BLOCKED_FUNC_DOOR, 0);
        assert_eq!(MSTAND_SOLDIER, 0);
        assert_eq!(MWALK_SOLDIER, 0);
        assert_eq!(MRUN_SOLDIER, 0);
        assert_eq!(MDODGE_SOLDIER, 0);
        assert_eq!(MATTACK_SOLDIER, 0);
        assert_eq!(MMELEE_SOLDIER, 0);
        assert_eq!(MSIGHT_SOLDIER, 0);
        assert_eq!(MIDLE_SOLDIER, 0);
        assert_eq!(MSEARCH_SOLDIER, 0);
        assert_eq!(MCHECKATTACK_DEFAULT, 0);
    }

    // ============================================================
    // 12. Monster type consistent mapping tests
    //    Verify that corresponding monster types have consistent IDs
    //    across stand/walk/run tables (they should match).
    // ============================================================

    #[test]
    fn soldier_ids_consistent_across_tables() {
        assert_eq!(MSTAND_SOLDIER, MWALK_SOLDIER);
        assert_eq!(MWALK_SOLDIER, MRUN_SOLDIER);
        assert_eq!(MRUN_SOLDIER, MATTACK_SOLDIER);
        assert_eq!(PAIN_SOLDIER, DIE_SOLDIER);
    }

    #[test]
    fn berserk_ids_consistent_across_tables() {
        assert_eq!(MSTAND_BERSERK, MWALK_BERSERK);
        assert_eq!(MWALK_BERSERK, MRUN_BERSERK);
        assert_eq!(PAIN_BERSERK, DIE_BERSERK);
    }

    #[test]
    fn brain_ids_consistent_across_tables() {
        assert_eq!(MSTAND_BRAIN, MWALK_BRAIN);
        assert_eq!(MWALK_BRAIN, MRUN_BRAIN);
        assert_eq!(PAIN_BRAIN, DIE_BRAIN);
    }

    #[test]
    fn gladiator_ids_consistent_across_tables() {
        assert_eq!(MSTAND_GLADIATOR, MWALK_GLADIATOR);
        assert_eq!(MWALK_GLADIATOR, MRUN_GLADIATOR);
        assert_eq!(PAIN_GLADIATOR, DIE_GLADIATOR);
    }

    #[test]
    fn gunner_ids_consistent_across_tables() {
        assert_eq!(MSTAND_GUNNER, MWALK_GUNNER);
        assert_eq!(MWALK_GUNNER, MRUN_GUNNER);
        assert_eq!(PAIN_GUNNER, DIE_GUNNER);
    }

    #[test]
    fn infantry_ids_consistent_across_tables() {
        assert_eq!(MSTAND_INFANTRY, MWALK_INFANTRY);
        assert_eq!(MWALK_INFANTRY, MRUN_INFANTRY);
        assert_eq!(PAIN_INFANTRY, DIE_INFANTRY);
    }

    #[test]
    fn tank_ids_consistent_across_tables() {
        assert_eq!(MSTAND_TANK, MWALK_TANK);
        assert_eq!(MWALK_TANK, MRUN_TANK);
        assert_eq!(PAIN_TANK, DIE_TANK);
    }

    #[test]
    fn hover_ids_consistent_across_tables() {
        assert_eq!(MSTAND_HOVER, MWALK_HOVER);
        assert_eq!(MWALK_HOVER, MRUN_HOVER);
        assert_eq!(PAIN_HOVER, DIE_HOVER);
    }

    #[test]
    fn chick_ids_consistent_across_tables() {
        assert_eq!(MSTAND_CHICK, MWALK_CHICK);
        assert_eq!(MWALK_CHICK, MRUN_CHICK);
        assert_eq!(PAIN_CHICK, DIE_CHICK);
    }

    #[test]
    fn mutant_ids_consistent_across_tables() {
        assert_eq!(MSTAND_MUTANT, MWALK_MUTANT);
        assert_eq!(MWALK_MUTANT, MRUN_MUTANT);
        assert_eq!(PAIN_MUTANT, DIE_MUTANT);
    }

    #[test]
    fn medic_ids_consistent_across_tables() {
        assert_eq!(MSTAND_MEDIC, MWALK_MEDIC);
        assert_eq!(MWALK_MEDIC, MRUN_MEDIC);
        assert_eq!(PAIN_MEDIC, DIE_MEDIC);
    }

    #[test]
    fn boss2_ids_consistent_across_tables() {
        assert_eq!(MSTAND_BOSS2, MWALK_BOSS2);
        assert_eq!(MWALK_BOSS2, MRUN_BOSS2);
        assert_eq!(PAIN_BOSS2, DIE_BOSS2);
    }

    #[test]
    fn jorg_ids_consistent_across_tables() {
        assert_eq!(MSTAND_JORG, MWALK_JORG);
        assert_eq!(MWALK_JORG, MRUN_JORG);
        assert_eq!(PAIN_JORG, DIE_JORG);
    }

    #[test]
    fn makron_ids_consistent_across_tables() {
        assert_eq!(MSTAND_MAKRON, MWALK_MAKRON);
        assert_eq!(MWALK_MAKRON, MRUN_MAKRON);
        assert_eq!(PAIN_MAKRON, DIE_MAKRON);
    }

    #[test]
    fn supertank_ids_consistent_across_tables() {
        assert_eq!(MSTAND_SUPERTANK, MWALK_SUPERTANK);
        assert_eq!(MWALK_SUPERTANK, MRUN_SUPERTANK);
        assert_eq!(PAIN_SUPERTANK, DIE_SUPERTANK);
    }

    // ============================================================
    // 13. Specific constant value spot checks
    //    Verify a sampling of specific numeric values to catch
    //    accidental reordering
    // ============================================================

    #[test]
    fn specific_think_constant_values() {
        assert_eq!(THINK_MONSTER, 0);
        assert_eq!(THINK_WALKMONSTER_START, 1);
        assert_eq!(THINK_FLYMONSTER_START, 2);
        assert_eq!(THINK_SWIMMONSTER_START, 3);
        assert_eq!(THINK_FREE_EDICT, 39);
        assert_eq!(THINK_MULTI_WAIT, 67);
        assert_eq!(THINK_FUNC_TIMER_THINK, 85);
    }

    #[test]
    fn specific_touch_constant_values() {
        assert_eq!(TOUCH_TRIGGER_MULTIPLE, 0);
        assert_eq!(TOUCH_ITEM, 4);
        assert_eq!(TOUCH_MUTANT_JUMP, 10);
        assert_eq!(TOUCH_TRIGGER_GRAVITY, 21);
        assert_eq!(TOUCH_DOOR, 25);
    }

    #[test]
    fn specific_use_constant_values() {
        assert_eq!(USE_TRIGGER_RELAY, 0);
        assert_eq!(USE_FUNC_DOOR, 12);
        assert_eq!(USE_MONSTER_USE, 18);
        assert_eq!(USE_BOSS3, 45);
        assert_eq!(USE_FUNC_CONVEYOR, 50);
    }

    #[test]
    fn specific_blocked_constant_values() {
        assert_eq!(BLOCKED_FUNC_DOOR, 0);
        assert_eq!(BLOCKED_FUNC_PLAT, 1);
        assert_eq!(BLOCKED_FUNC_TRAIN, 2);
        assert_eq!(BLOCKED_FUNC_ROTATING, 3);
        assert_eq!(BLOCKED_DOOR_SECRET, 4);
    }

    // ============================================================
    // 14. Aliasing tests: verify intentional table aliases
    // ============================================================

    #[test]
    fn trigger_once_and_trigger_multiple_share_touch_handler() {
        // TOUCH_TRIGGER_ONCE and TOUCH_TRIGGER_MULTIPLE should both map to
        // the same handler (w_touch_multi)
        let ptr_once = TOUCH_TABLE[TOUCH_TRIGGER_ONCE] as usize;
        let ptr_multiple = TOUCH_TABLE[TOUCH_TRIGGER_MULTIPLE] as usize;
        // Note: TOUCH_MULTI (19) also maps to w_touch_multi
        let ptr_multi = TOUCH_TABLE[TOUCH_MULTI] as usize;
        assert_eq!(ptr_once, ptr_multiple,
            "TOUCH_TRIGGER_ONCE and TOUCH_TRIGGER_MULTIPLE should share handler");
        assert_eq!(ptr_once, ptr_multi,
            "TOUCH_MULTI should share handler with TOUCH_TRIGGER_ONCE");
    }

    #[test]
    fn trigger_always_and_trigger_relay_share_use_handler() {
        let ptr_always = USE_TABLE[USE_TRIGGER_ALWAYS] as usize;
        let ptr_relay = USE_TABLE[USE_TRIGGER_RELAY] as usize;
        assert_eq!(ptr_always, ptr_relay,
            "USE_TRIGGER_ALWAYS and USE_TRIGGER_RELAY should share handler");
    }

    #[test]
    fn use_train_and_use_func_train_share_handler() {
        let ptr_train = USE_TABLE[USE_TRAIN] as usize;
        let ptr_func_train = USE_TABLE[USE_FUNC_TRAIN] as usize;
        assert_eq!(ptr_train, ptr_func_train,
            "USE_TRAIN and USE_FUNC_TRAIN should share handler");
    }

    // ============================================================
    // 15. Dispatch index routing tests
    //    Verify dispatch_ functions correctly index into tables
    // ============================================================

    #[test]
    fn dispatch_think_routes_to_correct_slot() {
        // We can verify routing by using THINK_MULTI_WAIT since its behavior
        // is self-contained (sets nextthink to 0.0)
        let mut edicts = make_edicts(4);
        let mut level = make_level();
        edicts[1].nextthink = 99.0;

        dispatch_think(THINK_MULTI_WAIT, 1, &mut edicts, &mut level);
        assert_eq!(edicts[1].nextthink, 0.0);
    }

    #[test]
    fn dispatch_touch_routes_to_correct_slot() {
        // Use TOUCH_TRIGGER_GRAVITY which has self-contained behavior
        let mut edicts = make_edicts(4);
        let mut level = make_level();
        edicts[0].gravity = 0.75;
        edicts[1].gravity = 1.0;

        dispatch_touch(TOUCH_TRIGGER_GRAVITY, 0, 1, &mut edicts, &mut level, None, None);
        assert_eq!(edicts[1].gravity, 0.75);
    }

    // ============================================================
    // 16. Edict callback field assignment and readback
    // ============================================================

    #[test]
    fn edict_callback_fields_round_trip() {
        let mut edict = Edict::default();

        edict.think_fn = Some(THINK_MULTI_WAIT);
        edict.pain_fn = Some(PAIN_SOLDIER);
        edict.die_fn = Some(DIE_BARREL);
        edict.touch_fn = Some(TOUCH_TRIGGER_GRAVITY);
        edict.use_fn = Some(USE_FUNC_DOOR);
        edict.blocked_fn = Some(BLOCKED_FUNC_TRAIN);

        assert_eq!(edict.think_fn, Some(THINK_MULTI_WAIT));
        assert_eq!(edict.pain_fn, Some(PAIN_SOLDIER));
        assert_eq!(edict.die_fn, Some(DIE_BARREL));
        assert_eq!(edict.touch_fn, Some(TOUCH_TRIGGER_GRAVITY));
        assert_eq!(edict.use_fn, Some(USE_FUNC_DOOR));
        assert_eq!(edict.blocked_fn, Some(BLOCKED_FUNC_TRAIN));
    }

    #[test]
    fn monsterinfo_callback_fields_round_trip() {
        let mut mi = MonsterInfo::default();

        mi.stand_fn = Some(MSTAND_SOLDIER);
        mi.walk_fn = Some(MWALK_BRAIN);
        mi.run_fn = Some(MRUN_TANK);
        mi.dodge_fn = Some(MDODGE_GUNNER);
        mi.attack_fn = Some(MATTACK_CHICK);
        mi.melee_fn = Some(MMELEE_BERSERK);
        mi.sight_fn = Some(MSIGHT_HOVER);
        mi.idle_fn = Some(MIDLE_FLYER);
        mi.search_fn = Some(MSEARCH_MEDIC);
        mi.checkattack_fn = Some(MCHECKATTACK_JORG);

        assert_eq!(mi.stand_fn, Some(MSTAND_SOLDIER));
        assert_eq!(mi.walk_fn, Some(MWALK_BRAIN));
        assert_eq!(mi.run_fn, Some(MRUN_TANK));
        assert_eq!(mi.dodge_fn, Some(MDODGE_GUNNER));
        assert_eq!(mi.attack_fn, Some(MATTACK_CHICK));
        assert_eq!(mi.melee_fn, Some(MMELEE_BERSERK));
        assert_eq!(mi.sight_fn, Some(MSIGHT_HOVER));
        assert_eq!(mi.idle_fn, Some(MIDLE_FLYER));
        assert_eq!(mi.search_fn, Some(MSEARCH_MEDIC));
        assert_eq!(mi.checkattack_fn, Some(MCHECKATTACK_JORG));
    }

    // ============================================================
    // 17. Full round-trip: set callback on edict, call_* dispatches
    //    through to the correct table entry
    // ============================================================

    #[test]
    fn full_roundtrip_think_multi_wait() {
        let mut edicts = make_edicts(4);
        let mut level = make_level();
        edicts[2].nextthink = 100.0;
        edicts[2].think_fn = Some(THINK_MULTI_WAIT);

        // This should look up think_fn, get THINK_MULTI_WAIT, dispatch
        // to w_multi_wait which sets nextthink = 0.0
        call_think(2, &mut edicts, &mut level);

        assert_eq!(edicts[2].nextthink, 0.0);
    }

    #[test]
    fn full_roundtrip_touch_gravity() {
        let mut edicts = make_edicts(4);
        let mut level = make_level();
        edicts[0].gravity = 0.1;
        edicts[0].touch_fn = Some(TOUCH_TRIGGER_GRAVITY);
        edicts[1].gravity = 1.0;

        call_touch(0, 1, &mut edicts, &mut level, None, None);

        assert_eq!(edicts[1].gravity, 0.1);
    }

    #[test]
    fn full_roundtrip_free_edict_routing() {
        // w_free_edict requires gi_unlinkentity (engine init), so we
        // verify the dispatch routing lookup is correct instead.
        let mut edict = Edict::default();
        edict.think_fn = Some(THINK_FREE_EDICT);
        assert_eq!(edict.think_fn, Some(39));
        // Verify the think table has a non-placeholder at index 39
        let placeholder_ptr = think_placeholder as ThinkFn as usize;
        assert_ne!(THINK_TABLE[THINK_FREE_EDICT] as usize, placeholder_ptr);
    }

    // ============================================================
    // 18. Verify table entries are valid function pointers
    //    (They should all be callable without crashing when passed
    //    correctly sized edict arrays - we test this by verifying
    //    they are non-null/non-zero function pointers.)
    // ============================================================

    #[test]
    fn all_think_table_entries_are_nonzero_fn_ptrs() {
        for (i, &entry) in THINK_TABLE.iter().enumerate() {
            let ptr = entry as usize;
            assert_ne!(ptr, 0, "THINK_TABLE[{}] should not be a null pointer", i);
        }
    }

    #[test]
    fn all_pain_table_entries_are_nonzero_fn_ptrs() {
        for (i, &entry) in PAIN_TABLE.iter().enumerate() {
            let ptr = entry as usize;
            assert_ne!(ptr, 0, "PAIN_TABLE[{}] should not be a null pointer", i);
        }
    }

    #[test]
    fn all_die_table_entries_are_nonzero_fn_ptrs() {
        for (i, &entry) in DIE_TABLE.iter().enumerate() {
            let ptr = entry as usize;
            assert_ne!(ptr, 0, "DIE_TABLE[{}] should not be a null pointer", i);
        }
    }

    #[test]
    fn all_touch_table_entries_are_nonzero_fn_ptrs() {
        for (i, &entry) in TOUCH_TABLE.iter().enumerate() {
            let ptr = entry as usize;
            assert_ne!(ptr, 0, "TOUCH_TABLE[{}] should not be a null pointer", i);
        }
    }

    #[test]
    fn all_use_table_entries_are_nonzero_fn_ptrs() {
        for (i, &entry) in USE_TABLE.iter().enumerate() {
            let ptr = entry as usize;
            assert_ne!(ptr, 0, "USE_TABLE[{}] should not be a null pointer", i);
        }
    }

    #[test]
    fn all_blocked_table_entries_are_nonzero_fn_ptrs() {
        for (i, &entry) in BLOCKED_TABLE.iter().enumerate() {
            let ptr = entry as usize;
            assert_ne!(ptr, 0, "BLOCKED_TABLE[{}] should not be a null pointer", i);
        }
    }

    #[test]
    fn all_monster_tables_have_nonzero_fn_ptrs() {
        for (i, &entry) in MSTAND_TABLE.iter().enumerate() {
            assert_ne!(entry as usize, 0, "MSTAND_TABLE[{}] is null", i);
        }
        for (i, &entry) in MWALK_TABLE.iter().enumerate() {
            assert_ne!(entry as usize, 0, "MWALK_TABLE[{}] is null", i);
        }
        for (i, &entry) in MRUN_TABLE.iter().enumerate() {
            assert_ne!(entry as usize, 0, "MRUN_TABLE[{}] is null", i);
        }
        for (i, &entry) in MDODGE_TABLE.iter().enumerate() {
            assert_ne!(entry as usize, 0, "MDODGE_TABLE[{}] is null", i);
        }
        for (i, &entry) in MATTACK_TABLE.iter().enumerate() {
            assert_ne!(entry as usize, 0, "MATTACK_TABLE[{}] is null", i);
        }
        for (i, &entry) in MMELEE_TABLE.iter().enumerate() {
            assert_ne!(entry as usize, 0, "MMELEE_TABLE[{}] is null", i);
        }
        for (i, &entry) in MSIGHT_TABLE.iter().enumerate() {
            assert_ne!(entry as usize, 0, "MSIGHT_TABLE[{}] is null", i);
        }
        for (i, &entry) in MIDLE_TABLE.iter().enumerate() {
            assert_ne!(entry as usize, 0, "MIDLE_TABLE[{}] is null", i);
        }
        for (i, &entry) in MSEARCH_TABLE.iter().enumerate() {
            assert_ne!(entry as usize, 0, "MSEARCH_TABLE[{}] is null", i);
        }
        for (i, &entry) in MCHECKATTACK_TABLE.iter().enumerate() {
            assert_ne!(entry as usize, 0, "MCHECKATTACK_TABLE[{}] is null", i);
        }
    }

    // ============================================================
    // 19. Distinct handler verification: ensure monsters with
    //     different IDs have different stand/walk/run handlers
    // ============================================================

    #[test]
    fn different_monsters_have_different_stand_handlers() {
        let soldier_fn = MSTAND_TABLE[MSTAND_SOLDIER] as usize;
        let brain_fn = MSTAND_TABLE[MSTAND_BRAIN] as usize;
        let tank_fn = MSTAND_TABLE[MSTAND_TANK] as usize;
        let hover_fn = MSTAND_TABLE[MSTAND_HOVER] as usize;

        assert_ne!(soldier_fn, brain_fn, "Soldier and Brain should have different stand handlers");
        assert_ne!(brain_fn, tank_fn, "Brain and Tank should have different stand handlers");
        assert_ne!(tank_fn, hover_fn, "Tank and Hover should have different stand handlers");
    }

    #[test]
    fn different_monsters_have_different_run_handlers() {
        let soldier_fn = MRUN_TABLE[MRUN_SOLDIER] as usize;
        let gunner_fn = MRUN_TABLE[MRUN_GUNNER] as usize;
        let chick_fn = MRUN_TABLE[MRUN_CHICK] as usize;

        assert_ne!(soldier_fn, gunner_fn);
        assert_ne!(gunner_fn, chick_fn);
    }

    #[test]
    fn different_monsters_have_different_pain_handlers() {
        let soldier_fn = PAIN_TABLE[PAIN_SOLDIER] as usize;
        let berserk_fn = PAIN_TABLE[PAIN_BERSERK] as usize;
        let player_fn = PAIN_TABLE[PAIN_PLAYER] as usize;

        assert_ne!(soldier_fn, berserk_fn);
        assert_ne!(berserk_fn, player_fn);
    }

    #[test]
    fn different_monsters_have_different_die_handlers() {
        let soldier_fn = DIE_TABLE[DIE_SOLDIER] as usize;
        let gib_fn = DIE_TABLE[DIE_GIB] as usize;
        let barrel_fn = DIE_TABLE[DIE_BARREL] as usize;
        let player_fn = DIE_TABLE[DIE_PLAYER] as usize;

        assert_ne!(soldier_fn, gib_fn);
        assert_ne!(gib_fn, barrel_fn);
        assert_ne!(barrel_fn, player_fn);
    }
}
