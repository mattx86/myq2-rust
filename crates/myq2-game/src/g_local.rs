// g_local.rs — Local definitions for game module
// Converted from: myq2-original/game/g_local.h

// Re-export all q_shared items so monster files can access them via `use crate::g_local::*`
pub use myq2_common::q_shared::*;
pub use crate::game::{AreaLink, Solid};
// MAX_ENT_CLUSTERS comes from q_shared::* (re-exported above)

pub const GAMEVERSION: &str = "baseq2";

// Protocol bytes — imported from the canonical definitions in myq2_common::qcommon
pub use myq2_common::qcommon::{
    SVC_MUZZLEFLASH, SVC_MUZZLEFLASH2, SVC_TEMP_ENTITY,
    SVC_LAYOUT, SVC_INVENTORY, SVC_STUFFTEXT,
};

// View pitching times
pub const DAMAGE_TIME: f32 = 0.5;
pub const FALL_TIME: f32 = 0.3;

// edict->spawnflags
pub const SPAWNFLAG_NOT_EASY: i32 = 0x00000100;
pub const SPAWNFLAG_NOT_MEDIUM: i32 = 0x00000200;
pub const SPAWNFLAG_NOT_HARD: i32 = 0x00000400;
pub const SPAWNFLAG_NOT_DEATHMATCH: i32 = 0x00000800;
pub const SPAWNFLAG_NOT_COOP: i32 = 0x00001000;

// edict->flags
bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct EntityFlags: i32 {
        const FLY            = 0x00000001;
        const SWIM           = 0x00000002;
        const IMMUNE_LASER   = 0x00000004;
        const INWATER        = 0x00000008;
        const GODMODE        = 0x00000010;
        const NOTARGET       = 0x00000020;
        const IMMUNE_SLIME   = 0x00000040;
        const IMMUNE_LAVA    = 0x00000080;
        const PARTIALGROUND  = 0x00000100;
        const WATERJUMP      = 0x00000200;
        const TEAMSLAVE      = 0x00000400;
        const NO_KNOCKBACK   = 0x00000800;
        const POWER_ARMOR    = 0x00001000;
        const RESPAWN        = -2147483648_i32 as i32; // 0x80000000
    }
}
pub const FL_FLY: EntityFlags = EntityFlags::FLY;
pub const FL_SWIM: EntityFlags = EntityFlags::SWIM;
pub const FL_IMMUNE_LASER: EntityFlags = EntityFlags::IMMUNE_LASER;
pub const FL_INWATER: EntityFlags = EntityFlags::INWATER;
pub const FL_GODMODE: EntityFlags = EntityFlags::GODMODE;
pub const FL_NOTARGET: EntityFlags = EntityFlags::NOTARGET;
pub const FL_IMMUNE_SLIME: EntityFlags = EntityFlags::IMMUNE_SLIME;
pub const FL_IMMUNE_LAVA: EntityFlags = EntityFlags::IMMUNE_LAVA;
pub const FL_PARTIALGROUND: EntityFlags = EntityFlags::PARTIALGROUND;
pub const FL_WATERJUMP: EntityFlags = EntityFlags::WATERJUMP;
pub const FL_TEAMSLAVE: EntityFlags = EntityFlags::TEAMSLAVE;
pub const FL_NO_KNOCKBACK: EntityFlags = EntityFlags::NO_KNOCKBACK;
pub const FL_POWER_ARMOR: EntityFlags = EntityFlags::POWER_ARMOR;
pub const FL_RESPAWN: EntityFlags = EntityFlags::RESPAWN;

pub const FRAMETIME: f32 = 0.1;

// Memory tags
pub const TAG_GAME: i32 = 765;
pub const TAG_LEVEL: i32 = 766;

pub const MELEE_DISTANCE: f32 = 80.0;
pub const BODY_QUEUE_SIZE: usize = 8;

// ============================================================
// Enums
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
#[derive(Default)]
pub enum Damage {
    #[default]
    No = 0,
    Yes,
    Aim,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
#[derive(Default)]
pub enum WeaponState {
    #[default]
    Ready = 0,
    Activating,
    Dropping,
    Firing,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum AmmoType {
    Bullets = 0,
    Shells,
    Rockets,
    Grenades,
    Cells,
    Slugs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
#[derive(Default)]
pub enum MoveType {
    #[default]
    None = 0,
    Noclip,
    Push,
    Stop,
    Walk,
    Step,
    Fly,
    Toss,
    FlyMissile,
    Bounce,
}


// Damage type constants (integer equivalents for C compatibility)
pub const DAMAGE_NO: i32 = 0;
pub const DAMAGE_YES: i32 = 1;
pub const DAMAGE_AIM: i32 = 2;

// MoveType integer constants (for C compatibility)
pub const MOVETYPE_NONE: i32 = 0;
pub const MOVETYPE_NOCLIP: i32 = 1;
pub const MOVETYPE_PUSH: i32 = 2;
pub const MOVETYPE_STOP: i32 = 3;
pub const MOVETYPE_WALK: i32 = 4;
pub const MOVETYPE_STEP: i32 = 5;
pub const MOVETYPE_FLY: i32 = 6;
pub const MOVETYPE_TOSS: i32 = 7;
pub const MOVETYPE_FLYMISSILE: i32 = 8;
pub const MOVETYPE_BOUNCE: i32 = 9;

// Solid type constants — imported from canonical game_api definitions
pub use myq2_common::game_api::{SOLID_NOT, SOLID_TRIGGER, SOLID_BBOX, SOLID_BSP};

// Dead flags
pub const DEAD_NO: i32 = 0;
pub const DEAD_DYING: i32 = 1;
pub const DEAD_DEAD: i32 = 2;
pub const DEAD_RESPAWNABLE: i32 = 3;

// Range
pub const RANGE_MELEE: i32 = 0;
pub const RANGE_NEAR: i32 = 1;
pub const RANGE_MID: i32 = 2;
pub const RANGE_FAR: i32 = 3;

// Gib types
pub const GIB_ORGANIC: i32 = 0;
pub const GIB_METALLIC: i32 = 1;

// Monster AI flags
bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct AiFlags: i32 {
        const STAND_GROUND      = 0x00000001;
        const TEMP_STAND_GROUND = 0x00000002;
        const SOUND_TARGET      = 0x00000004;
        const LOST_SIGHT        = 0x00000008;
        const PURSUIT_LAST_SEEN = 0x00000010;
        const PURSUE_NEXT       = 0x00000020;
        const PURSUE_TEMP       = 0x00000040;
        const HOLD_FRAME        = 0x00000080;
        const GOOD_GUY          = 0x00000100;
        const BRUTAL            = 0x00000200;
        const NOSTEP            = 0x00000400;
        const DUCKED            = 0x00000800;
        const COMBAT_POINT      = 0x00001000;
        const MEDIC             = 0x00002000;
        const RESURRECTING      = 0x00004000;
    }
}
pub const AI_STAND_GROUND: AiFlags = AiFlags::STAND_GROUND;
pub const AI_TEMP_STAND_GROUND: AiFlags = AiFlags::TEMP_STAND_GROUND;
pub const AI_SOUND_TARGET: AiFlags = AiFlags::SOUND_TARGET;
pub const AI_LOST_SIGHT: AiFlags = AiFlags::LOST_SIGHT;
pub const AI_PURSUIT_LAST_SEEN: AiFlags = AiFlags::PURSUIT_LAST_SEEN;
pub const AI_PURSUE_NEXT: AiFlags = AiFlags::PURSUE_NEXT;
pub const AI_PURSUE_TEMP: AiFlags = AiFlags::PURSUE_TEMP;
pub const AI_HOLD_FRAME: AiFlags = AiFlags::HOLD_FRAME;
pub const AI_GOOD_GUY: AiFlags = AiFlags::GOOD_GUY;
pub const AI_BRUTAL: AiFlags = AiFlags::BRUTAL;
pub const AI_NOSTEP: AiFlags = AiFlags::NOSTEP;
pub const AI_DUCKED: AiFlags = AiFlags::DUCKED;
pub const AI_COMBAT_POINT: AiFlags = AiFlags::COMBAT_POINT;
pub const AI_MEDIC: AiFlags = AiFlags::MEDIC;
pub const AI_RESURRECTING: AiFlags = AiFlags::RESURRECTING;

// Monster attack state
pub const AS_STRAIGHT: i32 = 1;
pub const AS_SLIDING: i32 = 2;
pub const AS_MELEE: i32 = 3;
pub const AS_MISSILE: i32 = 4;

// Armor types
pub const ARMOR_NONE: i32 = 0;
pub const ARMOR_JACKET: i32 = 1;
pub const ARMOR_COMBAT: i32 = 2;
pub const ARMOR_BODY: i32 = 3;
pub const ARMOR_SHARD: i32 = 4;

// Power armor types
pub const POWER_ARMOR_NONE: i32 = 0;
pub const POWER_ARMOR_SCREEN: i32 = 1;
pub const POWER_ARMOR_SHIELD: i32 = 2;

// Handedness
pub const RIGHT_HANDED: i32 = 0;
pub const LEFT_HANDED: i32 = 1;
pub const CENTER_HANDED: i32 = 2;

// Server flags
pub const SFL_CROSS_TRIGGER_1: i32 = 0x00000001;
pub const SFL_CROSS_TRIGGER_2: i32 = 0x00000002;
pub const SFL_CROSS_TRIGGER_3: i32 = 0x00000004;
pub const SFL_CROSS_TRIGGER_4: i32 = 0x00000008;
pub const SFL_CROSS_TRIGGER_5: i32 = 0x00000010;
pub const SFL_CROSS_TRIGGER_6: i32 = 0x00000020;
pub const SFL_CROSS_TRIGGER_7: i32 = 0x00000040;
pub const SFL_CROSS_TRIGGER_8: i32 = 0x00000080;
pub const SFL_CROSS_TRIGGER_MASK: i32 = 0x000000FF;

// Player noise types
pub const PNOISE_SELF: i32 = 0;
pub const PNOISE_WEAPON: i32 = 1;
pub const PNOISE_IMPACT: i32 = 2;

// Means of death
pub const MOD_UNKNOWN: i32 = 0;
pub const MOD_BLASTER: i32 = 1;
pub const MOD_SHOTGUN: i32 = 2;
pub const MOD_SSHOTGUN: i32 = 3;
pub const MOD_MACHINEGUN: i32 = 4;
pub const MOD_CHAINGUN: i32 = 5;
pub const MOD_GRENADE: i32 = 6;
pub const MOD_G_SPLASH: i32 = 7;
pub const MOD_ROCKET: i32 = 8;
pub const MOD_R_SPLASH: i32 = 9;
pub const MOD_HYPERBLASTER: i32 = 10;
pub const MOD_RAILGUN: i32 = 11;
pub const MOD_BFG_LASER: i32 = 12;
pub const MOD_BFG_BLAST: i32 = 13;
pub const MOD_BFG_EFFECT: i32 = 14;
pub const MOD_HANDGRENADE: i32 = 15;
pub const MOD_HG_SPLASH: i32 = 16;
pub const MOD_WATER: i32 = 17;
pub const MOD_SLIME: i32 = 18;
pub const MOD_LAVA: i32 = 19;
pub const MOD_CRUSH: i32 = 20;
pub const MOD_TELEFRAG: i32 = 21;
pub const MOD_FALLING: i32 = 22;
pub const MOD_SUICIDE: i32 = 23;
pub const MOD_HELD_GRENADE: i32 = 24;
pub const MOD_EXPLOSIVE: i32 = 25;
pub const MOD_BARREL: i32 = 26;
pub const MOD_BOMB: i32 = 27;
pub const MOD_EXIT: i32 = 28;
pub const MOD_SPLASH: i32 = 29;
pub const MOD_TARGET_LASER: i32 = 30;
pub const MOD_TRIGGER_HURT: i32 = 31;
pub const MOD_HIT: i32 = 32;
pub const MOD_TARGET_BLASTER: i32 = 33;
pub const MOD_FRIENDLY_FIRE: i32 = 0x08000000;

// Item flags
bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct ItemFlags: i32 {
        const WEAPON    = 1;
        const AMMO      = 2;
        const ARMOR     = 4;
        const STAY_COOP = 8;
        const KEY       = 16;
        const POWERUP   = 32;
    }
}
pub const IT_WEAPON: ItemFlags = ItemFlags::WEAPON;
pub const IT_AMMO: ItemFlags = ItemFlags::AMMO;
pub const IT_ARMOR: ItemFlags = ItemFlags::ARMOR;
pub const IT_STAY_COOP: ItemFlags = ItemFlags::STAY_COOP;
pub const IT_KEY: ItemFlags = ItemFlags::KEY;
pub const IT_POWERUP: ItemFlags = ItemFlags::POWERUP;

// Weapon model indices
pub const WEAP_BLASTER: i32 = 1;
pub const WEAP_SHOTGUN: i32 = 2;
pub const WEAP_SUPERSHOTGUN: i32 = 3;
pub const WEAP_MACHINEGUN: i32 = 4;
pub const WEAP_CHAINGUN: i32 = 5;
pub const WEAP_GRENADES: i32 = 6;
pub const WEAP_GRENADELAUNCHER: i32 = 7;
pub const WEAP_ROCKETLAUNCHER: i32 = 8;
pub const WEAP_HYPERBLASTER: i32 = 9;
pub const WEAP_RAILGUN: i32 = 10;
pub const WEAP_BFG: i32 = 11;

// Damage flags
bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct DamageFlags: i32 {
        const RADIUS        = 0x00000001;
        const NO_ARMOR      = 0x00000002;
        const ENERGY        = 0x00000004;
        const NO_KNOCKBACK  = 0x00000008;
        const BULLET        = 0x00000010;
        const NO_PROTECTION = 0x00000020;
    }
}
pub const DAMAGE_RADIUS: DamageFlags = DamageFlags::RADIUS;
pub const DAMAGE_NO_ARMOR: DamageFlags = DamageFlags::NO_ARMOR;
pub const DAMAGE_ENERGY: DamageFlags = DamageFlags::ENERGY;
pub const DAMAGE_NO_KNOCKBACK: DamageFlags = DamageFlags::NO_KNOCKBACK;
pub const DAMAGE_BULLET: DamageFlags = DamageFlags::BULLET;
pub const DAMAGE_NO_PROTECTION: DamageFlags = DamageFlags::NO_PROTECTION;

// Default spread values
pub const DEFAULT_BULLET_HSPREAD: i32 = 300;
pub const DEFAULT_BULLET_VSPREAD: i32 = 500;
pub const DEFAULT_SHOTGUN_HSPREAD: i32 = 1000;
pub const DEFAULT_SHOTGUN_VSPREAD: i32 = 500;
pub const DEFAULT_DEATHMATCH_SHOTGUN_COUNT: i32 = 12;
pub const DEFAULT_SHOTGUN_COUNT: i32 = 12;
pub const DEFAULT_SSHOTGUN_COUNT: i32 = 20;

// Item spawnflags
pub const ITEM_TRIGGER_SPAWN: i32 = 0x00000001;
pub const ITEM_NO_TOUCH: i32 = 0x00000002;
pub const DROPPED_ITEM: i32 = 0x00010000;
pub const DROPPED_PLAYER_ITEM: i32 = 0x00020000;
pub const ITEM_TARGETS_USED: i32 = 0x00040000;

// Field flags
pub const FFL_SPAWNTEMP: i32 = 1;
pub const FFL_NOSPAWN: i32 = 2;

// Multicast types — imported from canonical game_api definitions
pub use myq2_common::game_api::{
    MULTICAST_ALL, MULTICAST_PHS, MULTICAST_PVS,
    MULTICAST_ALL_R, MULTICAST_PHS_R, MULTICAST_PVS_R,
};

// Temp entity events
pub const TE_GUNSHOT: i32 = 0;
pub const TE_BLOOD: i32 = 1;
pub const TE_BLASTER: i32 = 2;
pub const TE_RAILTRAIL: i32 = 3;
pub const TE_SHOTGUN: i32 = 4;
pub const TE_EXPLOSION1: i32 = 5;
pub const TE_EXPLOSION2: i32 = 6;
pub const TE_ROCKET_EXPLOSION: i32 = 7;
pub const TE_GRENADE_EXPLOSION: i32 = 8;
pub const TE_SPARKS: i32 = 9;
pub const TE_SPLASH: i32 = 10;
pub const TE_BUBBLETRAIL: i32 = 11;
pub const TE_SCREEN_SPARKS: i32 = 12;
pub const TE_SHIELD_SPARKS: i32 = 13;
pub const TE_BULLET_SPARKS: i32 = 14;
pub const TE_LASER_SPARKS: i32 = 15;
pub const TE_PARASITE_ATTACK: i32 = 16;
pub const TE_ROCKET_EXPLOSION_WATER: i32 = 17;
pub const TE_GRENADE_EXPLOSION_WATER: i32 = 18;
pub const TE_MEDIC_CABLE_ATTACK: i32 = 19;
pub const TE_BFG_EXPLOSION: i32 = 20;
pub const TE_BFG_BIGEXPLOSION: i32 = 21;
pub const TE_BOSSTPORT: i32 = 22;
pub const TE_BFG_LASER: i32 = 23;
pub const TE_GRAPPLE_CABLE: i32 = 24;
pub const TE_WELDING_SPARKS: i32 = 25;
pub const TE_GREENBLOOD: i32 = 26;
pub const TE_BLUEHYPERBLASTER: i32 = 27;
pub const TE_PLASMA_EXPLOSION: i32 = 28;
pub const TE_TUNNEL_SPARKS: i32 = 29;

// Sound channels and attenuation are re-exported from q_shared

// Animation priorities
pub const ANIM_BASIC: i32 = 0;
pub const ANIM_WAVE: i32 = 1;
pub const ANIM_JUMP: i32 = 2;
pub const ANIM_PAIN: i32 = 3;
pub const ANIM_ATTACK: i32 = 4;
pub const ANIM_DEATH: i32 = 5;
pub const ANIM_REVERSE: i32 = 6;

// ============================================================
// Monster frame/move types (equivalent to C mframe_t / mmove_t)
// ============================================================

/// A single animation frame for a monster.
#[derive(Clone)]
pub struct MFrame {
    pub ai_fn: fn(&mut Edict, f32),
    pub dist: f32,
    pub think_fn: Option<fn(&mut Edict, &mut GameContext)>,
}

/// A monster move sequence (a set of animation frames).
#[derive(Clone)]
pub struct MMove {
    pub firstframe: i32,
    pub lastframe: i32,
    pub frames: &'static [MFrame],
    pub endfunc: Option<fn(&mut Edict, &mut GameContext)>,
}

// ============================================================
// Structures
// ============================================================

#[derive(Debug, Clone, Default)]
pub struct GItemArmor {
    pub base_count: i32,
    pub max_count: i32,
    pub normal_protection: f32,
    pub energy_protection: f32,
    pub armor: i32,
}

/// Game item definition.
/// In C this used function pointers; in Rust we use indices/enums
/// to reference handlers, which will be resolved by the game logic.
#[derive(Debug, Clone, Default)]
pub struct GItem {
    pub classname: String,
    pub pickup_sound: String,
    pub world_model: String,
    pub world_model_flags: u32,
    pub view_model: String,
    pub icon: String,
    pub pickup_name: String,
    pub count_width: i32,
    pub quantity: i32,
    pub ammo: String,
    pub flags: ItemFlags,
    pub weapmodel: i32,
    pub tag: i32,
    pub precaches: String,
    pub armor_info: Option<GItemArmor>,
    // Function pointers will be handled via dispatch tables
    pub pickup_fn: Option<usize>,
    pub use_fn: Option<usize>,
    pub drop_fn: Option<usize>,
    pub weaponthink_fn: Option<usize>,
}

/// Persistent game state (survives level changes).
#[derive(Debug, Clone, Default)]
pub struct GameLocals {
    pub helpmessage1: String,
    pub helpmessage2: String,
    pub helpchanged: i32,
    pub spawnpoint: String,
    pub maxclients: i32,
    pub maxentities: i32,
    pub serverflags: i32,
    pub num_items: i32,
    pub autosaved: bool,
}

/// Level state (cleared on each map change).
#[derive(Debug, Clone, Default)]
pub struct LevelLocals {
    pub framenum: i32,
    pub time: f32,
    pub level_name: String,
    pub mapname: String,
    pub nextmap: String,
    pub intermissiontime: f32,
    pub changemap: String,
    pub exitintermission: i32,
    pub intermission_origin: Vec3,
    pub intermission_angle: Vec3,
    pub sight_client: i32,      // entity index
    pub sight_entity: i32,      // entity index
    pub sight_entity_framenum: i32,
    pub sound_entity: i32,      // entity index
    pub sound_entity_framenum: i32,
    pub sound2_entity: i32,     // entity index
    pub sound2_entity_framenum: i32,
    pub pic_health: i32,
    pub total_secrets: i32,
    pub found_secrets: i32,
    pub total_goals: i32,
    pub found_goals: i32,
    pub total_monsters: i32,
    pub killed_monsters: i32,
    pub current_entity: i32,    // entity index
    pub body_que: i32,
    pub power_cubes: i32,
}

/// Spawn temporary data (only used during entity parsing).
#[derive(Debug, Clone, Default)]
pub struct SpawnTemp {
    pub sky: String,
    pub skyrotate: f32,
    pub skyaxis: Vec3,
    pub nextmap: String,
    pub lip: i32,
    pub distance: i32,
    pub height: i32,
    pub noise: String,
    pub pausetime: f32,
    pub item: String,
    pub gravity: String,
    pub minyaw: f32,
    pub maxyaw: f32,
    pub minpitch: f32,
    pub maxpitch: f32,
}

/// Movement info for movers (doors, plats, etc.)
#[derive(Debug, Clone, Default)]
pub struct MoveInfo {
    pub start_origin: Vec3,
    pub start_angles: Vec3,
    pub end_origin: Vec3,
    pub end_angles: Vec3,
    pub sound_start: i32,
    pub sound_middle: i32,
    pub sound_end: i32,
    pub accel: f32,
    pub speed: f32,
    pub decel: f32,
    pub distance: f32,
    pub wait: f32,
    pub state: i32,
    pub dir: Vec3,
    pub current_speed: f32,
    pub move_speed: f32,
    pub next_speed: f32,
    pub remaining_distance: f32,
    pub decel_distance: f32,
    pub endfunc: Option<usize>, // callback index
}

/// Monster AI info.
#[derive(Debug, Clone, Default)]
pub struct MonsterInfo {
    pub currentmove: Option<usize>, // index into move table
    pub aiflags: AiFlags,
    pub nextframe: i32,
    pub scale: f32,
    pub pausetime: f32,
    pub attack_finished: f32,
    pub saved_goal: Vec3,
    pub search_time: f32,
    pub trail_time: f32,
    pub last_sighting: Vec3,
    pub attack_state: i32,
    pub lefty: i32,
    pub idle_time: f32,
    pub linkcount: i32,
    pub power_armor_type: i32,
    pub power_armor_power: i32,
    // AI function callbacks — stored as indices into dispatch tables
    pub stand_fn: Option<usize>,
    pub idle_fn: Option<usize>,
    pub search_fn: Option<usize>,
    pub walk_fn: Option<usize>,
    pub run_fn: Option<usize>,
    pub dodge_fn: Option<usize>,
    pub attack_fn: Option<usize>,
    pub melee_fn: Option<usize>,
    pub sight_fn: Option<usize>,
    pub checkattack_fn: Option<usize>,
}

/// Client persistent data (survives respawns in DM, survives level changes).
#[derive(Debug, Clone)]
pub struct ClientPersistant {
    pub userinfo: String,
    pub netname: String,
    pub hand: i32,
    pub connected: bool,
    pub health: i32,
    pub max_health: i32,
    pub saved_flags: i32,
    pub selected_item: i32,
    pub inventory: [i32; MAX_ITEMS],
    pub max_bullets: i32,
    pub max_shells: i32,
    pub max_rockets: i32,
    pub max_grenades: i32,
    pub max_cells: i32,
    pub max_slugs: i32,
    pub weapon: Option<usize>,      // item index
    pub lastweapon: Option<usize>,   // item index
    pub power_cubes: i32,
    pub score: i32,
    pub game_helpchanged: i32,
    pub helpchanged: i32,
    pub spectator: bool,
}

impl Default for ClientPersistant {
    fn default() -> Self {
        Self {
            userinfo: String::new(),
            netname: String::new(),
            hand: 0,
            connected: false,
            health: 0,
            max_health: 0,
            saved_flags: 0,
            selected_item: 0,
            inventory: [0; MAX_ITEMS],
            max_bullets: 0,
            max_shells: 0,
            max_rockets: 0,
            max_grenades: 0,
            max_cells: 0,
            max_slugs: 0,
            weapon: None,
            lastweapon: None,
            power_cubes: 0,
            score: 0,
            game_helpchanged: 0,
            helpchanged: 0,
            spectator: false,
        }
    }
}

/// Client data that stays across deathmatch respawns.
#[derive(Debug, Clone, Default)]
pub struct ClientRespawn {
    pub coop_respawn: ClientPersistant,
    pub enterframe: i32,
    pub score: i32,
    pub cmd_angles: Vec3,
    pub spectator: bool,
}

/// Full game client structure.
#[derive(Debug, Clone, Default)]
pub struct GClient {
    // Known to server
    pub ps: PlayerState,
    pub ping: i32,

    // Private to game
    pub pers: ClientPersistant,
    pub resp: ClientRespawn,
    pub old_pmove: PmoveState,

    pub showscores: bool,
    pub showinventory: bool,
    pub showhelp: bool,
    pub showhelpicon: bool,

    pub ammo_index: i32,
    pub buttons: i32,
    pub oldbuttons: i32,
    pub latched_buttons: i32,
    pub weapon_thunk: bool,
    pub newweapon: Option<usize>, // item index

    pub damage_armor: i32,
    pub damage_parmor: i32,
    pub damage_blood: i32,
    pub damage_knockback: i32,
    pub damage_from: Vec3,

    pub killer_yaw: f32,
    pub weaponstate: WeaponState,
    pub kick_angles: Vec3,
    pub kick_origin: Vec3,
    pub v_dmg_roll: f32,
    pub v_dmg_pitch: f32,
    pub v_dmg_time: f32,
    pub fall_time: f32,
    pub fall_value: f32,
    pub damage_alpha: f32,
    pub bonus_alpha: f32,
    pub damage_blend: Vec3,
    pub v_angle: Vec3,
    pub bobtime: f32,
    pub oldviewangles: Vec3,
    pub oldvelocity: Vec3,

    pub next_drown_time: f32,
    pub old_waterlevel: i32,
    pub breather_sound: i32,
    pub machinegun_shots: i32,

    pub anim_end: i32,
    pub anim_priority: i32,
    pub anim_duck: bool,
    pub anim_run: bool,

    pub quad_framenum: f32,
    pub invincible_framenum: f32,
    pub breather_framenum: f32,
    pub enviro_framenum: f32,

    pub grenade_blew_up: bool,
    pub grenade_time: f32,
    pub silencer_shots: i32,
    pub weapon_sound: i32,

    pub pickup_msg_time: f32,
    pub flood_locktill: f32,
    pub flood_when: [f32; 10],
    pub flood_whenhead: i32,
    pub respawn_time: f32,

    pub chase_target: i32, // entity index, -1 = none
    pub update_chase: bool,
}

/// Full edict structure.
#[derive(Debug, Clone, Default)]
pub struct Edict {
    // Server-visible fields (DO NOT reorder)
    pub s: EntityState,
    pub client: Option<usize>,  // index into clients array, None if not a player
    pub inuse: bool,
    pub linkcount: i32,
    pub area: AreaLink,
    pub num_clusters: i32,
    pub clusternums: [i32; MAX_ENT_CLUSTERS],
    pub headnode: i32,
    pub areanum: i32,
    pub areanum2: i32,
    pub svflags: i32,
    pub mins: Vec3,
    pub maxs: Vec3,
    pub absmin: Vec3,
    pub absmax: Vec3,
    pub size: Vec3,
    pub solid: Solid,
    pub clipmask: i32,
    pub owner: i32, // entity index, -1 = none

    // Game-private fields
    pub movetype: MoveType,
    pub flags: EntityFlags,
    pub model: String,
    pub freetime: f32,
    pub message: String,
    pub classname: String,
    pub spawnflags: i32,
    pub timestamp: f32,
    pub angle: f32,
    pub target: String,
    pub targetname: String,
    pub killtarget: String,
    pub team: String,
    pub pathtarget: String,
    pub deathtarget: String,
    pub combattarget: String,
    pub target_ent: i32, // entity index

    pub speed: f32,
    pub accel: f32,
    pub decel: f32,
    pub movedir: Vec3,
    pub pos1: Vec3,
    pub pos2: Vec3,

    pub velocity: Vec3,
    pub avelocity: Vec3,
    pub mass: i32,
    pub air_finished: f32,
    pub gravity: f32,

    pub goalentity: i32,     // entity index
    pub movetarget: i32,     // entity index
    pub yaw_speed: f32,
    pub ideal_yaw: f32,

    pub nextthink: f32,
    // Function callbacks — stored as indices into dispatch tables
    pub prethink_fn: Option<usize>,
    pub think_fn: Option<usize>,
    pub blocked_fn: Option<usize>,
    pub touch_fn: Option<usize>,
    pub use_fn: Option<usize>,
    pub pain_fn: Option<usize>,
    pub die_fn: Option<usize>,

    pub touch_debounce_time: f32,
    pub pain_debounce_time: f32,
    pub damage_debounce_time: f32,
    pub fly_sound_debounce_time: f32,
    pub last_move_time: f32,

    pub health: i32,
    pub max_health: i32,
    pub gib_health: i32,
    pub deadflag: i32,
    pub show_hostile: f32, // C declares as qboolean but uses it to store time values

    pub powerarmor_time: f32,
    pub map: String,

    pub viewheight: i32,
    pub takedamage: i32,
    pub dmg: i32,
    pub radius_dmg: i32,
    pub dmg_radius: f32,
    pub sounds: i32,
    pub count: i32,

    pub chain: i32,              // entity index
    pub enemy: i32,              // entity index
    pub oldenemy: i32,           // entity index
    pub activator: i32,          // entity index
    pub groundentity: i32,       // entity index
    pub groundentity_linkcount: i32,
    pub teamchain: i32,          // entity index
    pub teammaster: i32,         // entity index

    pub mynoise: i32,            // entity index
    pub mynoise2: i32,           // entity index

    pub noise_index: i32,
    pub noise_index2: i32,
    pub volume: f32,
    pub attenuation: f32,

    pub wait: f32,
    pub delay: f32,
    pub random: f32,

    pub teleport_time: f32,

    pub watertype: i32,
    pub waterlevel: i32,

    pub move_origin: Vec3,
    pub move_angles: Vec3,

    pub light_level: i32,
    pub style: i32,

    pub item: Option<usize>, // item index

    pub moveinfo: MoveInfo,
    pub monsterinfo: MonsterInfo,
}

// ============================================================
// Player Trail State
// ============================================================

pub(crate) const TRAIL_LENGTH: usize = 8;

/// Holds the player trail state for AI pursuit.
#[derive(Debug, Clone)]
pub struct PlayerTrailState {
    /// Entity indices for the trail marker edicts.
    pub trail: [i32; TRAIL_LENGTH],
    /// Current head position in the circular buffer.
    pub trail_head: usize,
    /// Whether the trail system is active.
    pub trail_active: bool,
}

impl Default for PlayerTrailState {
    fn default() -> Self {
        PlayerTrailState {
            trail: [-1; TRAIL_LENGTH],
            trail_head: 0,
            trail_active: false,
        }
    }
}

// ============================================================
// Unified Game Context
// ============================================================

/// Unified game context — replaces all per-module GameContext variants.
/// Holds all game state needed by any game module.
pub struct GameCtx {
    // Core state
    pub edicts: Vec<Edict>,
    pub clients: Vec<GClient>,
    pub game: GameLocals,
    pub level: LevelLocals,
    pub st: SpawnTemp,
    pub items: Vec<GItem>,

    // Counts
    pub num_edicts: i32,
    pub max_edicts: i32,

    // Precache indices
    pub sm_meat_index: i32,
    pub snd_fry: i32,
    pub means_of_death: i32,

    // Cvar values (cached as f32 for fast access, matching C globals)
    pub deathmatch: f32,
    pub coop: f32,
    pub skill: f32,
    pub dmflags: f32,
    pub maxclients: f32,
    pub maxspectators: f32,
    pub maxentities: f32,
    pub sv_gravity: f32,
    pub sv_maxvelocity: f32,
    pub sv_rollspeed: f32,
    pub sv_rollangle: f32,
    pub gun_x: f32,
    pub gun_y: f32,
    pub gun_z: f32,
    pub run_pitch: f32,
    pub run_roll: f32,
    pub bob_up: f32,
    pub bob_pitch: f32,
    pub bob_roll: f32,
    pub sv_cheats: f32,
    pub g_select_empty: f32,
    pub filterban: f32,
    pub flood_msgs: f32,
    pub flood_persecond: f32,
    pub flood_waitdelay: f32,
    pub dedicated: f32,
    pub fraglimit: f32,
    pub timelimit: f32,
    pub needpass: f32,

    // Specialized state
    pub password: String,
    pub spectator_password: String,
    pub sv_maplist: String,

    // Player trail
    pub player_trail: PlayerTrailState,

    // Trigger state
    pub windsound: i32,

    // Weapon state
    pub is_quad: bool,
    pub is_silenced: u8,

    // p_client state
    pub death_anim_index: i32,
    pub pm_passent: usize,

    // Item state and lookup indices (from g_items)
    pub items_state: crate::g_items::ItemsState,
    pub item_by_classname: std::collections::HashMap<String, usize>,
    pub item_by_pickup_name: std::collections::HashMap<String, usize>,

    // Entity lookup indices (O(1) search by targetname/classname)
    pub entity_by_targetname: std::collections::HashMap<String, Vec<i32>>,
    pub entity_by_classname: std::collections::HashMap<String, Vec<i32>>,
}

/// Convenience alias so every game module can refer to the context as `GameContext`.
pub type GameContext = GameCtx;

impl Default for GameCtx {
    fn default() -> Self {
        Self {
            edicts: Vec::new(),
            clients: Vec::new(),
            game: GameLocals::default(),
            level: LevelLocals::default(),
            st: SpawnTemp::default(),
            items: Vec::new(),
            num_edicts: 0,
            max_edicts: 0,
            sm_meat_index: 0,
            snd_fry: 0,
            means_of_death: 0,
            deathmatch: 0.0,
            coop: 0.0,
            skill: 0.0,
            dmflags: 0.0,
            maxclients: 0.0,
            maxspectators: 0.0,
            maxentities: 0.0,
            sv_gravity: 800.0,
            sv_maxvelocity: 2000.0,
            sv_rollspeed: 200.0,
            sv_rollangle: 2.0,
            gun_x: 0.0,
            gun_y: 0.0,
            gun_z: 0.0,
            run_pitch: 0.002,
            run_roll: 0.005,
            bob_up: 0.005,
            bob_pitch: 0.002,
            bob_roll: 0.002,
            sv_cheats: 0.0,
            g_select_empty: 0.0,
            filterban: 1.0,
            flood_msgs: 4.0,
            flood_persecond: 4.0,
            flood_waitdelay: 10.0,
            dedicated: 0.0,
            fraglimit: 0.0,
            timelimit: 0.0,
            needpass: 0.0,
            password: String::new(),
            spectator_password: String::new(),
            sv_maplist: String::new(),
            player_trail: PlayerTrailState::default(),
            windsound: 0,
            is_quad: false,
            is_silenced: 0,
            death_anim_index: 0,
            pm_passent: 0,
            items_state: crate::g_items::ItemsState::default(),
            item_by_classname: std::collections::HashMap::new(),
            item_by_pickup_name: std::collections::HashMap::new(),
            entity_by_targetname: std::collections::HashMap::new(),
            entity_by_classname: std::collections::HashMap::new(),
        }
    }
}

impl GameCtx {
    /// Create a new game context with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new game context with specified capacity for edicts and clients.
    pub fn with_capacity(max_edicts: usize, max_clients: usize) -> Self {
        Self {
            edicts: Vec::with_capacity(max_edicts),
            clients: Vec::with_capacity(max_clients),
            max_edicts: max_edicts as i32,
            ..Self::default()
        }
    }

    /// Get an immutable reference to an edict by index.
    pub fn get_edict(&self, idx: usize) -> Option<&Edict> {
        self.edicts.get(idx)
    }

    /// Get a mutable reference to an edict by index.
    pub fn get_edict_mut(&mut self, idx: usize) -> Option<&mut Edict> {
        self.edicts.get_mut(idx)
    }

    /// Get an immutable reference to a client by index.
    pub fn get_client(&self, idx: usize) -> Option<&GClient> {
        self.clients.get(idx)
    }

    /// Get a mutable reference to a client by index.
    pub fn get_client_mut(&mut self, idx: usize) -> Option<&mut GClient> {
        self.clients.get_mut(idx)
    }

    /// Get an immutable reference to an item by index.
    pub fn get_item(&self, idx: usize) -> Option<&GItem> {
        self.items.get(idx)
    }

    /// Get a mutable reference to an item by index.
    pub fn get_item_mut(&mut self, idx: usize) -> Option<&mut GItem> {
        self.items.get_mut(idx)
    }

    /// Get the GClient for an edict (panics if the edict has no client).
    pub fn client_of(&self, ent_idx: usize) -> &GClient {
        let client_idx = self.edicts[ent_idx].client.expect("edict has no client");
        &self.clients[client_idx]
    }

    /// Get the GClient mutably for an edict (panics if the edict has no client).
    pub fn client_of_mut(&mut self, ent_idx: usize) -> &mut GClient {
        let client_idx = self.edicts[ent_idx].client.expect("edict has no client");
        &mut self.clients[client_idx]
    }

    /// Build entity lookup indices for O(1) search by targetname/classname.
    /// Call after entities are spawned or when entity names change.
    pub fn build_entity_indices(&mut self) {
        self.entity_by_targetname.clear();
        self.entity_by_classname.clear();

        for i in 0..self.num_edicts as usize {
            let ent = &self.edicts[i];
            if !ent.inuse {
                continue;
            }

            if !ent.targetname.is_empty() {
                self.entity_by_targetname
                    .entry(ent.targetname.to_lowercase())
                    .or_default()
                    .push(i as i32);
            }

            if !ent.classname.is_empty() {
                self.entity_by_classname
                    .entry(ent.classname.to_lowercase())
                    .or_default()
                    .push(i as i32);
            }
        }
    }

    /// Register a single entity in the indices (call when spawning new entity).
    pub fn register_entity_in_index(&mut self, ent_idx: usize) {
        let ent = &self.edicts[ent_idx];
        if !ent.inuse {
            return;
        }

        if !ent.targetname.is_empty() {
            self.entity_by_targetname
                .entry(ent.targetname.to_lowercase())
                .or_default()
                .push(ent_idx as i32);
        }

        if !ent.classname.is_empty() {
            self.entity_by_classname
                .entry(ent.classname.to_lowercase())
                .or_default()
                .push(ent_idx as i32);
        }
    }

    /// O(1) lookup of entities by targetname.
    pub fn find_entities_by_targetname(&self, targetname: &str) -> &[i32] {
        static EMPTY: Vec<i32> = Vec::new();
        self.entity_by_targetname
            .get(&targetname.to_lowercase())
            .map(|v| v.as_slice())
            .unwrap_or(&EMPTY)
    }

    /// O(1) lookup of entities by classname.
    pub fn find_entities_by_classname(&self, classname: &str) -> &[i32] {
        static EMPTY: Vec<i32> = Vec::new();
        self.entity_by_classname
            .get(&classname.to_lowercase())
            .map(|v| v.as_slice())
            .unwrap_or(&EMPTY)
    }
}

// ============================================================
// Global Game Context
// ============================================================

use std::sync::Mutex;

static GLOBAL_GAME_CTX: Mutex<Option<GameCtx>> = Mutex::new(None);

/// Initialize the global game context. Called once at game init time.
pub fn init_global_game_ctx(ctx: GameCtx) {
    *GLOBAL_GAME_CTX.lock().unwrap() = Some(ctx);
}

/// Access the global game context via a closure.
/// Returns None if the context hasn't been initialized yet.
pub fn with_global_game_ctx<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut GameCtx) -> R,
{
    let mut guard = GLOBAL_GAME_CTX.lock().unwrap();
    guard.as_mut().map(f)
}

/// Take the global game context out (for transferring ownership).
pub fn take_global_game_ctx() -> Option<GameCtx> {
    GLOBAL_GAME_CTX.lock().unwrap().take()
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Edict default initialization ----

    #[test]
    fn test_edict_default() {
        let e = Edict::default();
        assert!(!e.inuse, "Edict should not be in use by default");
        assert_eq!(e.linkcount, 0);
        assert!(e.client.is_none(), "Edict should have no client by default");
        assert_eq!(e.s.number, 0);
        assert_eq!(e.s.origin, [0.0; 3]);
        assert_eq!(e.s.angles, [0.0; 3]);
        assert_eq!(e.s.modelindex, 0);
        assert_eq!(e.svflags, 0);
        assert_eq!(e.mins, [0.0; 3]);
        assert_eq!(e.maxs, [0.0; 3]);
        assert_eq!(e.absmin, [0.0; 3]);
        assert_eq!(e.absmax, [0.0; 3]);
        assert_eq!(e.size, [0.0; 3]);
        assert_eq!(e.solid, Solid::Not);
        assert_eq!(e.clipmask, 0);
        assert_eq!(e.owner, 0);
        assert_eq!(e.num_clusters, 0);
        assert_eq!(e.headnode, 0);
        assert_eq!(e.areanum, 0);
        assert_eq!(e.areanum2, 0);
        assert_eq!(e.clusternums, [0; MAX_ENT_CLUSTERS]);
    }

    #[test]
    fn test_edict_default_game_fields() {
        let e = Edict::default();
        assert_eq!(e.movetype, MoveType::None);
        assert_eq!(e.flags, EntityFlags::empty());
        assert!(e.model.is_empty());
        assert_eq!(e.freetime, 0.0);
        assert!(e.message.is_empty());
        assert!(e.classname.is_empty());
        assert_eq!(e.spawnflags, 0);
        assert_eq!(e.health, 0);
        assert_eq!(e.max_health, 0);
        assert_eq!(e.deadflag, 0);
        assert_eq!(e.takedamage, 0);
        assert_eq!(e.dmg, 0);
        assert_eq!(e.velocity, [0.0; 3]);
        assert_eq!(e.avelocity, [0.0; 3]);
        assert_eq!(e.mass, 0);
        assert_eq!(e.gravity, 0.0);
        assert_eq!(e.nextthink, 0.0);
        assert!(e.think_fn.is_none());
        assert!(e.touch_fn.is_none());
        assert!(e.use_fn.is_none());
        assert!(e.pain_fn.is_none());
        assert!(e.die_fn.is_none());
        assert!(e.blocked_fn.is_none());
        assert!(e.prethink_fn.is_none());
        assert!(e.item.is_none());
        assert_eq!(e.enemy, 0);
        assert_eq!(e.groundentity, 0);
        assert_eq!(e.chain, 0);
        assert_eq!(e.watertype, 0);
        assert_eq!(e.waterlevel, 0);
    }

    // ---- GClient default initialization ----

    #[test]
    fn test_gclient_default() {
        let gc = GClient::default();
        assert_eq!(gc.ping, 0);
        assert_eq!(gc.ps.pmove.origin, [0; 3]);
        assert_eq!(gc.ps.viewangles, [0.0; 3]);
        assert_eq!(gc.ps.fov, 90.0, "Default FOV should be 90");
        assert_eq!(gc.ps.stats, [0i16; MAX_STATS]);
        assert!(!gc.pers.connected);
        assert_eq!(gc.pers.health, 0);
        assert_eq!(gc.pers.max_health, 0);
        assert_eq!(gc.pers.inventory, [0; MAX_ITEMS]);
        assert!(gc.pers.weapon.is_none());
        assert!(gc.pers.lastweapon.is_none());
        assert_eq!(gc.pers.score, 0);
        assert!(!gc.pers.spectator);
        assert_eq!(gc.weaponstate, WeaponState::Ready);
        assert_eq!(gc.buttons, 0);
        assert_eq!(gc.oldbuttons, 0);
        assert_eq!(gc.latched_buttons, 0);
        assert!(!gc.weapon_thunk);
        assert!(gc.newweapon.is_none());
        assert_eq!(gc.damage_armor, 0);
        assert_eq!(gc.damage_blood, 0);
        assert_eq!(gc.kick_angles, [0.0; 3]);
        assert_eq!(gc.kick_origin, [0.0; 3]);
        assert_eq!(gc.v_angle, [0.0; 3]);
        assert!(!gc.grenade_blew_up);
        assert_eq!(gc.flood_when, [0.0; 10]);
    }

    // ---- GameLocals default values ----

    #[test]
    fn test_game_locals_default() {
        let gl = GameLocals::default();
        assert!(gl.helpmessage1.is_empty());
        assert!(gl.helpmessage2.is_empty());
        assert_eq!(gl.helpchanged, 0);
        assert!(gl.spawnpoint.is_empty());
        assert_eq!(gl.maxclients, 0);
        assert_eq!(gl.maxentities, 0);
        assert_eq!(gl.serverflags, 0);
        assert_eq!(gl.num_items, 0);
        assert!(!gl.autosaved);
    }

    // ---- LevelLocals default values ----

    #[test]
    fn test_level_locals_default() {
        let ll = LevelLocals::default();
        assert_eq!(ll.framenum, 0);
        assert_eq!(ll.time, 0.0);
        assert!(ll.level_name.is_empty());
        assert!(ll.mapname.is_empty());
        assert!(ll.nextmap.is_empty());
        assert_eq!(ll.intermissiontime, 0.0);
        assert_eq!(ll.exitintermission, 0);
        assert_eq!(ll.intermission_origin, [0.0; 3]);
        assert_eq!(ll.intermission_angle, [0.0; 3]);
        assert_eq!(ll.sight_client, 0);
        assert_eq!(ll.total_secrets, 0);
        assert_eq!(ll.found_secrets, 0);
        assert_eq!(ll.total_goals, 0);
        assert_eq!(ll.found_goals, 0);
        assert_eq!(ll.total_monsters, 0);
        assert_eq!(ll.killed_monsters, 0);
    }

    // ---- SpawnTemp default values ----

    #[test]
    fn test_spawn_temp_default() {
        let st = SpawnTemp::default();
        assert!(st.sky.is_empty());
        assert_eq!(st.skyrotate, 0.0);
        assert_eq!(st.skyaxis, [0.0; 3]);
        assert!(st.nextmap.is_empty());
        assert_eq!(st.lip, 0);
        assert_eq!(st.distance, 0);
        assert_eq!(st.height, 0);
        assert!(st.noise.is_empty());
        assert_eq!(st.pausetime, 0.0);
        assert!(st.item.is_empty());
        assert!(st.gravity.is_empty());
        assert_eq!(st.minyaw, 0.0);
        assert_eq!(st.maxyaw, 0.0);
        assert_eq!(st.minpitch, 0.0);
        assert_eq!(st.maxpitch, 0.0);
    }

    // ---- MoveType enum conversion (i32 <-> enum) ----

    #[test]
    fn test_movetype_enum_values() {
        assert_eq!(MoveType::None as i32, MOVETYPE_NONE);
        assert_eq!(MoveType::Noclip as i32, MOVETYPE_NOCLIP);
        assert_eq!(MoveType::Push as i32, MOVETYPE_PUSH);
        assert_eq!(MoveType::Stop as i32, MOVETYPE_STOP);
        assert_eq!(MoveType::Walk as i32, MOVETYPE_WALK);
        assert_eq!(MoveType::Step as i32, MOVETYPE_STEP);
        assert_eq!(MoveType::Fly as i32, MOVETYPE_FLY);
        assert_eq!(MoveType::Toss as i32, MOVETYPE_TOSS);
        assert_eq!(MoveType::FlyMissile as i32, MOVETYPE_FLYMISSILE);
        assert_eq!(MoveType::Bounce as i32, MOVETYPE_BOUNCE);
    }

    #[test]
    fn test_movetype_constants_sequential() {
        assert_eq!(MOVETYPE_NONE, 0);
        assert_eq!(MOVETYPE_NOCLIP, 1);
        assert_eq!(MOVETYPE_PUSH, 2);
        assert_eq!(MOVETYPE_STOP, 3);
        assert_eq!(MOVETYPE_WALK, 4);
        assert_eq!(MOVETYPE_STEP, 5);
        assert_eq!(MOVETYPE_FLY, 6);
        assert_eq!(MOVETYPE_TOSS, 7);
        assert_eq!(MOVETYPE_FLYMISSILE, 8);
        assert_eq!(MOVETYPE_BOUNCE, 9);
    }

    #[test]
    fn test_movetype_default() {
        let mt = MoveType::default();
        assert_eq!(mt, MoveType::None);
        assert_eq!(mt as i32, 0);
    }

    // ---- Solid enum conversion (i32 <-> enum) ----

    #[test]
    fn test_solid_enum_values() {
        assert_eq!(Solid::Not as i32, SOLID_NOT);
        assert_eq!(Solid::Trigger as i32, SOLID_TRIGGER);
        assert_eq!(Solid::Bbox as i32, SOLID_BBOX);
        assert_eq!(Solid::Bsp as i32, SOLID_BSP);
    }

    #[test]
    fn test_solid_default() {
        let s = Solid::default();
        assert_eq!(s, Solid::Not);
        assert_eq!(s as i32, 0);
    }

    // ---- DamageFlags bitfield operations ----

    #[test]
    fn test_damage_flags_individual() {
        assert_eq!(DAMAGE_RADIUS.bits(), 0x00000001);
        assert_eq!(DAMAGE_NO_ARMOR.bits(), 0x00000002);
        assert_eq!(DAMAGE_ENERGY.bits(), 0x00000004);
        assert_eq!(DAMAGE_NO_KNOCKBACK.bits(), 0x00000008);
        assert_eq!(DAMAGE_BULLET.bits(), 0x00000010);
        assert_eq!(DAMAGE_NO_PROTECTION.bits(), 0x00000020);
    }

    #[test]
    fn test_damage_flags_combination() {
        let flags = DAMAGE_RADIUS | DAMAGE_ENERGY;
        assert!(flags.contains(DAMAGE_RADIUS));
        assert!(flags.contains(DAMAGE_ENERGY));
        assert!(!flags.contains(DAMAGE_NO_ARMOR));
        assert!(!flags.contains(DAMAGE_BULLET));
    }

    #[test]
    fn test_damage_flags_default_is_empty() {
        let flags = DamageFlags::default();
        assert!(flags.is_empty());
        assert!(!flags.contains(DAMAGE_RADIUS));
    }

    #[test]
    fn test_damage_flags_all_distinct() {
        // Each flag should be a unique bit
        let all_flags = [
            DAMAGE_RADIUS, DAMAGE_NO_ARMOR, DAMAGE_ENERGY,
            DAMAGE_NO_KNOCKBACK, DAMAGE_BULLET, DAMAGE_NO_PROTECTION,
        ];
        for i in 0..all_flags.len() {
            for j in (i + 1)..all_flags.len() {
                assert_eq!(all_flags[i] & all_flags[j], DamageFlags::empty(),
                    "Flags {} and {} should not overlap", i, j);
            }
        }
    }

    #[test]
    fn test_damage_flags_bitwise_operations() {
        let mut flags = DamageFlags::empty();
        flags |= DAMAGE_RADIUS;
        flags |= DAMAGE_BULLET;
        assert!(flags.contains(DAMAGE_RADIUS));
        assert!(flags.contains(DAMAGE_BULLET));
        assert!(!flags.contains(DAMAGE_ENERGY));

        flags &= !DAMAGE_RADIUS;
        assert!(!flags.contains(DAMAGE_RADIUS));
        assert!(flags.contains(DAMAGE_BULLET));
    }

    // ---- EntityFlags bitfield operations ----

    #[test]
    fn test_entity_flags_individual_values() {
        assert_eq!(FL_FLY.bits(), 0x00000001);
        assert_eq!(FL_SWIM.bits(), 0x00000002);
        assert_eq!(FL_IMMUNE_LASER.bits(), 0x00000004);
        assert_eq!(FL_INWATER.bits(), 0x00000008);
        assert_eq!(FL_GODMODE.bits(), 0x00000010);
        assert_eq!(FL_NOTARGET.bits(), 0x00000020);
        assert_eq!(FL_IMMUNE_SLIME.bits(), 0x00000040);
        assert_eq!(FL_IMMUNE_LAVA.bits(), 0x00000080);
        assert_eq!(FL_PARTIALGROUND.bits(), 0x00000100);
        assert_eq!(FL_WATERJUMP.bits(), 0x00000200);
        assert_eq!(FL_TEAMSLAVE.bits(), 0x00000400);
        assert_eq!(FL_NO_KNOCKBACK.bits(), 0x00000800);
        assert_eq!(FL_POWER_ARMOR.bits(), 0x00001000);
    }

    #[test]
    fn test_entity_flags_respawn_is_sign_bit() {
        // FL_RESPAWN uses the sign bit (0x80000000)
        assert_eq!(FL_RESPAWN.bits() as u32, 0x80000000);
    }

    #[test]
    fn test_entity_flags_combination() {
        let flags = FL_FLY | FL_GODMODE | FL_NOTARGET;
        assert!(flags.contains(FL_FLY));
        assert!(flags.contains(FL_GODMODE));
        assert!(flags.contains(FL_NOTARGET));
        assert!(!flags.contains(FL_SWIM));
        assert!(!flags.contains(FL_INWATER));
    }

    #[test]
    fn test_entity_flags_default_is_empty() {
        let flags = EntityFlags::default();
        assert!(flags.is_empty());
    }

    // ---- AiFlags bitfield operations ----

    #[test]
    fn test_ai_flags_values() {
        assert_eq!(AI_STAND_GROUND.bits(), 0x00000001);
        assert_eq!(AI_TEMP_STAND_GROUND.bits(), 0x00000002);
        assert_eq!(AI_SOUND_TARGET.bits(), 0x00000004);
        assert_eq!(AI_LOST_SIGHT.bits(), 0x00000008);
        assert_eq!(AI_PURSUIT_LAST_SEEN.bits(), 0x00000010);
        assert_eq!(AI_PURSUE_NEXT.bits(), 0x00000020);
        assert_eq!(AI_PURSUE_TEMP.bits(), 0x00000040);
        assert_eq!(AI_HOLD_FRAME.bits(), 0x00000080);
        assert_eq!(AI_GOOD_GUY.bits(), 0x00000100);
        assert_eq!(AI_BRUTAL.bits(), 0x00000200);
        assert_eq!(AI_NOSTEP.bits(), 0x00000400);
        assert_eq!(AI_DUCKED.bits(), 0x00000800);
        assert_eq!(AI_COMBAT_POINT.bits(), 0x00001000);
        assert_eq!(AI_MEDIC.bits(), 0x00002000);
        assert_eq!(AI_RESURRECTING.bits(), 0x00004000);
    }

    #[test]
    fn test_ai_flags_default_is_empty() {
        let flags = AiFlags::default();
        assert!(flags.is_empty());
    }

    // ---- ItemFlags bitfield operations ----

    #[test]
    fn test_item_flags_values() {
        assert_eq!(IT_WEAPON.bits(), 1);
        assert_eq!(IT_AMMO.bits(), 2);
        assert_eq!(IT_ARMOR.bits(), 4);
        assert_eq!(IT_STAY_COOP.bits(), 8);
        assert_eq!(IT_KEY.bits(), 16);
        assert_eq!(IT_POWERUP.bits(), 32);
    }

    #[test]
    fn test_item_flags_combination() {
        let flags = IT_WEAPON | IT_AMMO;
        assert!(flags.contains(IT_WEAPON));
        assert!(flags.contains(IT_AMMO));
        assert!(!flags.contains(IT_ARMOR));
    }

    // ---- Damage enum conversion ----

    #[test]
    fn test_damage_enum_values() {
        assert_eq!(Damage::No as i32, DAMAGE_NO);
        assert_eq!(Damage::Yes as i32, DAMAGE_YES);
        assert_eq!(Damage::Aim as i32, DAMAGE_AIM);
    }

    #[test]
    fn test_damage_default() {
        assert_eq!(Damage::default(), Damage::No);
    }

    // ---- WeaponState enum ----

    #[test]
    fn test_weapon_state_values() {
        assert_eq!(WeaponState::Ready as i32, 0);
        assert_eq!(WeaponState::Activating as i32, 1);
        assert_eq!(WeaponState::Dropping as i32, 2);
        assert_eq!(WeaponState::Firing as i32, 3);
    }

    #[test]
    fn test_weapon_state_default() {
        assert_eq!(WeaponState::default(), WeaponState::Ready);
    }

    // ---- GameContext default and accessors ----

    #[test]
    fn test_game_context_default() {
        let ctx = GameCtx::default();
        assert!(ctx.edicts.is_empty());
        assert!(ctx.clients.is_empty());
        assert_eq!(ctx.num_edicts, 0);
        assert_eq!(ctx.max_edicts, 0);
        assert_eq!(ctx.deathmatch, 0.0);
        assert_eq!(ctx.coop, 0.0);
        assert_eq!(ctx.skill, 0.0);
        assert_eq!(ctx.sv_gravity, 800.0, "Default gravity should be 800");
        assert_eq!(ctx.sv_maxvelocity, 2000.0);
        assert_eq!(ctx.filterban, 1.0);
        assert!(!ctx.is_quad);
        assert_eq!(ctx.is_silenced, 0);
    }

    #[test]
    fn test_game_context_with_capacity() {
        let ctx = GameCtx::with_capacity(1024, 8);
        assert!(ctx.edicts.capacity() >= 1024);
        assert!(ctx.clients.capacity() >= 8);
        assert_eq!(ctx.max_edicts, 1024);
    }

    #[test]
    fn test_game_context_edict_access() {
        let mut ctx = GameCtx::default();
        ctx.edicts.resize_with(4, Default::default);
        ctx.edicts[2].classname = "monster_soldier".to_string();
        ctx.edicts[2].inuse = true;

        assert!(ctx.get_edict(2).is_some());
        assert_eq!(ctx.get_edict(2).unwrap().classname, "monster_soldier");
        assert!(ctx.get_edict(2).unwrap().inuse);
        assert!(ctx.get_edict(10).is_none());
    }

    #[test]
    fn test_game_context_client_access() {
        let mut ctx = GameCtx::default();
        ctx.clients.resize_with(4, Default::default);
        ctx.clients[0].pers.netname = "Player1".to_string();
        ctx.clients[0].pers.connected = true;

        assert!(ctx.get_client(0).is_some());
        assert_eq!(ctx.get_client(0).unwrap().pers.netname, "Player1");
        assert!(ctx.get_client(0).unwrap().pers.connected);
        assert!(ctx.get_client(10).is_none());
    }

    // ---- MoveInfo default ----

    #[test]
    fn test_moveinfo_default() {
        let mi = MoveInfo::default();
        assert_eq!(mi.start_origin, [0.0; 3]);
        assert_eq!(mi.start_angles, [0.0; 3]);
        assert_eq!(mi.end_origin, [0.0; 3]);
        assert_eq!(mi.end_angles, [0.0; 3]);
        assert_eq!(mi.sound_start, 0);
        assert_eq!(mi.sound_middle, 0);
        assert_eq!(mi.sound_end, 0);
        assert_eq!(mi.accel, 0.0);
        assert_eq!(mi.speed, 0.0);
        assert_eq!(mi.decel, 0.0);
        assert_eq!(mi.distance, 0.0);
        assert_eq!(mi.wait, 0.0);
        assert_eq!(mi.state, 0);
        assert!(mi.endfunc.is_none());
    }

    // ---- MonsterInfo default ----

    #[test]
    fn test_monsterinfo_default() {
        let mi = MonsterInfo::default();
        assert!(mi.currentmove.is_none());
        assert!(mi.aiflags.is_empty());
        assert_eq!(mi.nextframe, 0);
        assert_eq!(mi.scale, 0.0);
        assert_eq!(mi.pausetime, 0.0);
        assert_eq!(mi.attack_finished, 0.0);
        assert_eq!(mi.saved_goal, [0.0; 3]);
        assert_eq!(mi.attack_state, 0);
        assert!(mi.stand_fn.is_none());
        assert!(mi.idle_fn.is_none());
        assert!(mi.walk_fn.is_none());
        assert!(mi.run_fn.is_none());
        assert!(mi.attack_fn.is_none());
        assert!(mi.melee_fn.is_none());
        assert!(mi.sight_fn.is_none());
    }

    // ---- ClientPersistant default ----

    #[test]
    fn test_client_persistant_default() {
        let cp = ClientPersistant::default();
        assert!(cp.userinfo.is_empty());
        assert!(cp.netname.is_empty());
        assert_eq!(cp.hand, 0);
        assert!(!cp.connected);
        assert_eq!(cp.health, 0);
        assert_eq!(cp.max_health, 0);
        assert_eq!(cp.saved_flags, 0);
        assert_eq!(cp.selected_item, 0);
        assert_eq!(cp.inventory, [0; MAX_ITEMS]);
        assert!(cp.weapon.is_none());
        assert!(cp.lastweapon.is_none());
        assert_eq!(cp.score, 0);
        assert!(!cp.spectator);
    }

    // ---- PlayerTrailState default ----

    #[test]
    fn test_player_trail_state_default() {
        let pts = PlayerTrailState::default();
        assert_eq!(pts.trail, [-1; TRAIL_LENGTH]);
        assert_eq!(pts.trail_head, 0);
        assert!(!pts.trail_active);
    }

    // ---- Constants validation ----

    #[test]
    fn test_game_constants() {
        assert_eq!(FRAMETIME, 0.1);
        assert_eq!(TAG_GAME, 765);
        assert_eq!(TAG_LEVEL, 766);
        assert_eq!(MELEE_DISTANCE, 80.0);
        assert_eq!(BODY_QUEUE_SIZE, 8);
        assert_eq!(DAMAGE_TIME, 0.5);
        assert_eq!(FALL_TIME, 0.3);
    }

    #[test]
    fn test_dead_constants() {
        assert_eq!(DEAD_NO, 0);
        assert_eq!(DEAD_DYING, 1);
        assert_eq!(DEAD_DEAD, 2);
        assert_eq!(DEAD_RESPAWNABLE, 3);
    }

    #[test]
    fn test_range_constants() {
        assert_eq!(RANGE_MELEE, 0);
        assert_eq!(RANGE_NEAR, 1);
        assert_eq!(RANGE_MID, 2);
        assert_eq!(RANGE_FAR, 3);
    }

    // ---- build_entity_indices ----

    #[test]
    fn test_build_entity_indices() {
        let mut ctx = GameCtx::default();
        ctx.edicts.resize_with(4, Default::default);
        ctx.num_edicts = 4;

        ctx.edicts[1].inuse = true;
        ctx.edicts[1].classname = "info_player_start".to_string();
        ctx.edicts[1].targetname = "spawn1".to_string();

        ctx.edicts[2].inuse = true;
        ctx.edicts[2].classname = "trigger_once".to_string();
        ctx.edicts[2].targetname = "trig1".to_string();

        ctx.edicts[3].inuse = false;
        ctx.edicts[3].classname = "should_not_appear".to_string();

        ctx.build_entity_indices();

        assert_eq!(ctx.find_entities_by_classname("info_player_start"), &[1]);
        assert_eq!(ctx.find_entities_by_classname("trigger_once"), &[2]);
        assert_eq!(ctx.find_entities_by_targetname("spawn1"), &[1]);
        assert_eq!(ctx.find_entities_by_targetname("trig1"), &[2]);
        // Inactive entity should not appear
        assert!(ctx.find_entities_by_classname("should_not_appear").is_empty());
    }

    #[test]
    fn test_build_entity_indices_case_insensitive() {
        let mut ctx = GameCtx::default();
        ctx.edicts.resize_with(2, Default::default);
        ctx.num_edicts = 2;

        ctx.edicts[1].inuse = true;
        ctx.edicts[1].classname = "Info_Player_Start".to_string();

        ctx.build_entity_indices();

        // Lookup should be case-insensitive
        assert_eq!(ctx.find_entities_by_classname("info_player_start"), &[1]);
        assert_eq!(ctx.find_entities_by_classname("INFO_PLAYER_START"), &[1]);
    }
}

