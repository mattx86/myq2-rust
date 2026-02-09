// g_save.rs — Save/load game and level state
// Converted from: myq2-original/game/g_save.c

use std::fs::File;
use std::io::{Read, Write, BufReader, BufWriter};

use rayon::prelude::*;

use crate::g_local::*;
use crate::game::*;
use crate::game_import::*;

// ============================================================
// Field types for save/load system
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    Int,
    Float,
    LString,
    GString,
    Edict,
    Client,
    Item,
    Function,
    MMove,
    Vector,
    Ignore,
    AngleHack,
}

/// Describes which struct a field belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldTarget {
    Edict,
    LevelLocals,
    Client,
    SpawnTemp,
}

/// A field descriptor for the save/load system.
/// Replaces the C `field_t` which used byte offsets into structs.
/// In Rust we use an enum-based field identifier instead.
#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: &'static str,
    pub field_id: FieldId,
    pub field_type: FieldType,
    pub flags: i32,
}

/// Identifies a specific field in a struct for save/load operations.
/// Replaces C's FOFS/LLOFS/CLOFS/STOFS macros.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldId {
    // Edict fields
    Classname,
    Model,
    Spawnflags,
    Speed,
    Accel,
    Decel,
    Target,
    Targetname,
    Pathtarget,
    Deathtarget,
    Killtarget,
    Combattarget,
    Message,
    Team,
    Wait,
    Delay,
    Random,
    MoveOrigin,
    MoveAngles,
    Style,
    Count,
    Health,
    Sounds,
    Light,
    Dmg,
    Mass,
    Volume,
    Attenuation,
    Map,
    SOrigin,
    SAngles,
    AngleHack,

    // Edict entity references
    GoalEntity,
    MoveTarget,
    Enemy,
    OldEnemy,
    Activator,
    GroundEntity,
    TeamChain,
    TeamMaster,
    Owner,
    MyNoise,
    MyNoise2,
    TargetEnt,
    Chain,

    // Edict function pointers
    PreThink,
    Think,
    Blocked,
    Touch,
    Use,
    Pain,
    Die,

    // MonsterInfo function pointers
    MiStand,
    MiIdle,
    MiSearch,
    MiWalk,
    MiRun,
    MiDodge,
    MiAttack,
    MiMelee,
    MiSight,
    MiCheckAttack,
    MiCurrentMove,

    // MoveInfo function pointers
    MiEndFunc,

    // Edict item field
    EdictItem,

    // SpawnTemp fields
    StLip,
    StDistance,
    StHeight,
    StNoise,
    StPauseTime,
    StItem,
    StGravity,
    StSky,
    StSkyRotate,
    StSkyAxis,
    StMinYaw,
    StMaxYaw,
    StMinPitch,
    StMaxPitch,
    StNextMap,

    // LevelLocals fields
    LlChangeMap,
    LlSightClient,
    LlSightEntity,
    LlSoundEntity,
    LlSound2Entity,

    // Client fields
    ClPersWeapon,
    ClPersLastWeapon,
    ClNewWeapon,
}

/// Entity fields table — mirrors the C `fields[]` array.
pub fn edict_fields() -> Vec<FieldDef> {
    vec![
        FieldDef { name: "classname",     field_id: FieldId::Classname,     field_type: FieldType::LString,    flags: 0 },
        FieldDef { name: "model",         field_id: FieldId::Model,         field_type: FieldType::LString,    flags: 0 },
        FieldDef { name: "spawnflags",    field_id: FieldId::Spawnflags,    field_type: FieldType::Int,        flags: 0 },
        FieldDef { name: "speed",         field_id: FieldId::Speed,         field_type: FieldType::Float,      flags: 0 },
        FieldDef { name: "accel",         field_id: FieldId::Accel,         field_type: FieldType::Float,      flags: 0 },
        FieldDef { name: "decel",         field_id: FieldId::Decel,         field_type: FieldType::Float,      flags: 0 },
        FieldDef { name: "target",        field_id: FieldId::Target,        field_type: FieldType::LString,    flags: 0 },
        FieldDef { name: "targetname",    field_id: FieldId::Targetname,    field_type: FieldType::LString,    flags: 0 },
        FieldDef { name: "pathtarget",    field_id: FieldId::Pathtarget,    field_type: FieldType::LString,    flags: 0 },
        FieldDef { name: "deathtarget",   field_id: FieldId::Deathtarget,   field_type: FieldType::LString,    flags: 0 },
        FieldDef { name: "killtarget",    field_id: FieldId::Killtarget,    field_type: FieldType::LString,    flags: 0 },
        FieldDef { name: "combattarget",  field_id: FieldId::Combattarget,  field_type: FieldType::LString,    flags: 0 },
        FieldDef { name: "message",       field_id: FieldId::Message,       field_type: FieldType::LString,    flags: 0 },
        FieldDef { name: "team",          field_id: FieldId::Team,          field_type: FieldType::LString,    flags: 0 },
        FieldDef { name: "wait",          field_id: FieldId::Wait,          field_type: FieldType::Float,      flags: 0 },
        FieldDef { name: "delay",         field_id: FieldId::Delay,         field_type: FieldType::Float,      flags: 0 },
        FieldDef { name: "random",        field_id: FieldId::Random,        field_type: FieldType::Float,      flags: 0 },
        FieldDef { name: "move_origin",   field_id: FieldId::MoveOrigin,    field_type: FieldType::Vector,     flags: 0 },
        FieldDef { name: "move_angles",   field_id: FieldId::MoveAngles,    field_type: FieldType::Vector,     flags: 0 },
        FieldDef { name: "style",         field_id: FieldId::Style,         field_type: FieldType::Int,        flags: 0 },
        FieldDef { name: "count",         field_id: FieldId::Count,         field_type: FieldType::Int,        flags: 0 },
        FieldDef { name: "health",        field_id: FieldId::Health,        field_type: FieldType::Int,        flags: 0 },
        FieldDef { name: "sounds",        field_id: FieldId::Sounds,        field_type: FieldType::Int,        flags: 0 },
        FieldDef { name: "light",         field_id: FieldId::Light,         field_type: FieldType::Ignore,     flags: 0 },
        FieldDef { name: "dmg",           field_id: FieldId::Dmg,           field_type: FieldType::Int,        flags: 0 },
        FieldDef { name: "mass",          field_id: FieldId::Mass,          field_type: FieldType::Int,        flags: 0 },
        FieldDef { name: "volume",        field_id: FieldId::Volume,        field_type: FieldType::Float,      flags: 0 },
        FieldDef { name: "attenuation",   field_id: FieldId::Attenuation,   field_type: FieldType::Float,      flags: 0 },
        FieldDef { name: "map",           field_id: FieldId::Map,           field_type: FieldType::LString,    flags: 0 },
        FieldDef { name: "origin",        field_id: FieldId::SOrigin,       field_type: FieldType::Vector,     flags: 0 },
        FieldDef { name: "angles",        field_id: FieldId::SAngles,       field_type: FieldType::Vector,     flags: 0 },
        FieldDef { name: "angle",         field_id: FieldId::AngleHack,     field_type: FieldType::AngleHack,  flags: 0 },

        // Entity references (FFL_NOSPAWN)
        FieldDef { name: "goalentity",    field_id: FieldId::GoalEntity,    field_type: FieldType::Edict,      flags: FFL_NOSPAWN },
        FieldDef { name: "movetarget",    field_id: FieldId::MoveTarget,    field_type: FieldType::Edict,      flags: FFL_NOSPAWN },
        FieldDef { name: "enemy",         field_id: FieldId::Enemy,         field_type: FieldType::Edict,      flags: FFL_NOSPAWN },
        FieldDef { name: "oldenemy",      field_id: FieldId::OldEnemy,      field_type: FieldType::Edict,      flags: FFL_NOSPAWN },
        FieldDef { name: "activator",     field_id: FieldId::Activator,     field_type: FieldType::Edict,      flags: FFL_NOSPAWN },
        FieldDef { name: "groundentity",  field_id: FieldId::GroundEntity,  field_type: FieldType::Edict,      flags: FFL_NOSPAWN },
        FieldDef { name: "teamchain",     field_id: FieldId::TeamChain,     field_type: FieldType::Edict,      flags: FFL_NOSPAWN },
        FieldDef { name: "teammaster",    field_id: FieldId::TeamMaster,    field_type: FieldType::Edict,      flags: FFL_NOSPAWN },
        FieldDef { name: "owner",         field_id: FieldId::Owner,         field_type: FieldType::Edict,      flags: FFL_NOSPAWN },
        FieldDef { name: "mynoise",       field_id: FieldId::MyNoise,       field_type: FieldType::Edict,      flags: FFL_NOSPAWN },
        FieldDef { name: "mynoise2",      field_id: FieldId::MyNoise2,      field_type: FieldType::Edict,      flags: FFL_NOSPAWN },
        FieldDef { name: "target_ent",    field_id: FieldId::TargetEnt,     field_type: FieldType::Edict,      flags: FFL_NOSPAWN },
        FieldDef { name: "chain",         field_id: FieldId::Chain,         field_type: FieldType::Edict,      flags: FFL_NOSPAWN },

        // Function pointers (FFL_NOSPAWN)
        FieldDef { name: "prethink",      field_id: FieldId::PreThink,      field_type: FieldType::Function,   flags: FFL_NOSPAWN },
        FieldDef { name: "think",         field_id: FieldId::Think,         field_type: FieldType::Function,   flags: FFL_NOSPAWN },
        FieldDef { name: "blocked",       field_id: FieldId::Blocked,       field_type: FieldType::Function,   flags: FFL_NOSPAWN },
        FieldDef { name: "touch",         field_id: FieldId::Touch,         field_type: FieldType::Function,   flags: FFL_NOSPAWN },
        FieldDef { name: "use",           field_id: FieldId::Use,           field_type: FieldType::Function,   flags: FFL_NOSPAWN },
        FieldDef { name: "pain",          field_id: FieldId::Pain,          field_type: FieldType::Function,   flags: FFL_NOSPAWN },
        FieldDef { name: "die",           field_id: FieldId::Die,           field_type: FieldType::Function,   flags: FFL_NOSPAWN },

        // MonsterInfo functions (FFL_NOSPAWN)
        FieldDef { name: "stand",         field_id: FieldId::MiStand,       field_type: FieldType::Function,   flags: FFL_NOSPAWN },
        FieldDef { name: "idle",          field_id: FieldId::MiIdle,        field_type: FieldType::Function,   flags: FFL_NOSPAWN },
        FieldDef { name: "search",        field_id: FieldId::MiSearch,      field_type: FieldType::Function,   flags: FFL_NOSPAWN },
        FieldDef { name: "walk",          field_id: FieldId::MiWalk,        field_type: FieldType::Function,   flags: FFL_NOSPAWN },
        FieldDef { name: "run",           field_id: FieldId::MiRun,         field_type: FieldType::Function,   flags: FFL_NOSPAWN },
        FieldDef { name: "dodge",         field_id: FieldId::MiDodge,       field_type: FieldType::Function,   flags: FFL_NOSPAWN },
        FieldDef { name: "attack",        field_id: FieldId::MiAttack,      field_type: FieldType::Function,   flags: FFL_NOSPAWN },
        FieldDef { name: "melee",         field_id: FieldId::MiMelee,       field_type: FieldType::Function,   flags: FFL_NOSPAWN },
        FieldDef { name: "sight",         field_id: FieldId::MiSight,       field_type: FieldType::Function,   flags: FFL_NOSPAWN },
        FieldDef { name: "checkattack",   field_id: FieldId::MiCheckAttack, field_type: FieldType::Function,   flags: FFL_NOSPAWN },
        FieldDef { name: "currentmove",   field_id: FieldId::MiCurrentMove, field_type: FieldType::MMove,      flags: FFL_NOSPAWN },

        // MoveInfo endfunc
        FieldDef { name: "endfunc",       field_id: FieldId::MiEndFunc,     field_type: FieldType::Function,   flags: FFL_NOSPAWN },

        // SpawnTemp fields (FFL_SPAWNTEMP)
        FieldDef { name: "lip",           field_id: FieldId::StLip,         field_type: FieldType::Int,        flags: FFL_SPAWNTEMP },
        FieldDef { name: "distance",      field_id: FieldId::StDistance,    field_type: FieldType::Int,        flags: FFL_SPAWNTEMP },
        FieldDef { name: "height",        field_id: FieldId::StHeight,      field_type: FieldType::Int,        flags: FFL_SPAWNTEMP },
        FieldDef { name: "noise",         field_id: FieldId::StNoise,       field_type: FieldType::LString,    flags: FFL_SPAWNTEMP },
        FieldDef { name: "pausetime",     field_id: FieldId::StPauseTime,   field_type: FieldType::Float,      flags: FFL_SPAWNTEMP },
        FieldDef { name: "item",          field_id: FieldId::StItem,        field_type: FieldType::LString,    flags: FFL_SPAWNTEMP },

        // Edict item field (F_ITEM, not spawntemp)
        FieldDef { name: "item",          field_id: FieldId::EdictItem,     field_type: FieldType::Item,       flags: 0 },

        FieldDef { name: "gravity",       field_id: FieldId::StGravity,     field_type: FieldType::LString,    flags: FFL_SPAWNTEMP },
        FieldDef { name: "sky",           field_id: FieldId::StSky,         field_type: FieldType::LString,    flags: FFL_SPAWNTEMP },
        FieldDef { name: "skyrotate",     field_id: FieldId::StSkyRotate,   field_type: FieldType::Float,      flags: FFL_SPAWNTEMP },
        FieldDef { name: "skyaxis",       field_id: FieldId::StSkyAxis,     field_type: FieldType::Vector,     flags: FFL_SPAWNTEMP },
        FieldDef { name: "minyaw",        field_id: FieldId::StMinYaw,      field_type: FieldType::Float,      flags: FFL_SPAWNTEMP },
        FieldDef { name: "maxyaw",        field_id: FieldId::StMaxYaw,      field_type: FieldType::Float,      flags: FFL_SPAWNTEMP },
        FieldDef { name: "minpitch",      field_id: FieldId::StMinPitch,    field_type: FieldType::Float,      flags: FFL_SPAWNTEMP },
        FieldDef { name: "maxpitch",      field_id: FieldId::StMaxPitch,    field_type: FieldType::Float,      flags: FFL_SPAWNTEMP },
        FieldDef { name: "nextmap",       field_id: FieldId::StNextMap,     field_type: FieldType::LString,    flags: FFL_SPAWNTEMP },
    ]
}

/// Level locals fields table — mirrors the C `levelfields[]` array.
pub fn level_fields() -> Vec<FieldDef> {
    vec![
        FieldDef { name: "changemap",     field_id: FieldId::LlChangeMap,    field_type: FieldType::LString, flags: 0 },
        FieldDef { name: "sight_client",   field_id: FieldId::LlSightClient,  field_type: FieldType::Edict,   flags: 0 },
        FieldDef { name: "sight_entity",   field_id: FieldId::LlSightEntity,  field_type: FieldType::Edict,   flags: 0 },
        FieldDef { name: "sound_entity",   field_id: FieldId::LlSoundEntity,  field_type: FieldType::Edict,   flags: 0 },
        FieldDef { name: "sound2_entity",  field_id: FieldId::LlSound2Entity, field_type: FieldType::Edict,   flags: 0 },
    ]
}

/// Client fields table — mirrors the C `clientfields[]` array.
pub fn client_fields() -> Vec<FieldDef> {
    vec![
        FieldDef { name: "pers.weapon",     field_id: FieldId::ClPersWeapon,     field_type: FieldType::Item, flags: 0 },
        FieldDef { name: "pers.lastweapon", field_id: FieldId::ClPersLastWeapon, field_type: FieldType::Item, flags: 0 },
        FieldDef { name: "newweapon",       field_id: FieldId::ClNewWeapon,      field_type: FieldType::Item, flags: 0 },
    ]
}

// ============================================================
// Save/Load context — replaces C globals
// ============================================================

/// Holds all mutable game state needed by save/load operations.
/// Replaces the C globals: game, level, g_edicts, globals, itemlist, cvars.
pub struct SaveContext<'a> {
    pub game: &'a mut GameLocals,
    pub level: &'a mut LevelLocals,
    pub edicts: &'a mut Vec<Edict>,
    pub clients: &'a mut Vec<GClient>,
    pub num_edicts: &'a mut i32,
    pub items: &'a [GItem],
}

// ============================================================
// Serialization helpers (generic)
// ============================================================

// Generic write helpers that work with any Write impl
fn write_i32_generic<W: Write>(f: &mut W, val: i32) {
    let bytes = val.to_le_bytes();
    f.write_all(&bytes).expect("write_i32 failed");
}

fn write_f32_generic<W: Write>(f: &mut W, val: f32) {
    let bytes = val.to_le_bytes();
    f.write_all(&bytes).expect("write_f32 failed");
}

fn write_string_generic<W: Write>(f: &mut W, s: &str) {
    let len = s.len() as i32;
    write_i32_generic(f, len);
    if len > 0 {
        f.write_all(s.as_bytes()).expect("write_string failed");
    }
}

fn write_vec3_generic<W: Write>(f: &mut W, v: &[f32; 3]) {
    write_f32_generic(f, v[0]);
    write_f32_generic(f, v[1]);
    write_f32_generic(f, v[2]);
}

fn write_option_usize_generic<W: Write>(f: &mut W, val: Option<usize>) {
    match val {
        Some(v) => write_i32_generic(f, v as i32),
        None => write_i32_generic(f, -1),
    }
}

// ============================================================
// Serialization helpers (file-specific, use generic versions)
// ============================================================

fn write_i32(f: &mut BufWriter<File>, val: i32) {
    write_i32_generic(f, val);
}

fn read_i32(f: &mut BufReader<File>) -> i32 {
    let mut buf = [0u8; 4];
    f.read_exact(&mut buf).expect("read_i32 failed");
    i32::from_le_bytes(buf)
}

fn write_f32(f: &mut BufWriter<File>, val: f32) {
    write_f32_generic(f, val);
}

fn read_f32(f: &mut BufReader<File>) -> f32 {
    let mut buf = [0u8; 4];
    f.read_exact(&mut buf).expect("read_f32 failed");
    f32::from_le_bytes(buf)
}

fn write_string(f: &mut BufWriter<File>, s: &str) {
    write_string_generic(f, s);
}

fn read_string(f: &mut BufReader<File>) -> String {
    let len = read_i32(f);
    if len <= 0 {
        return String::new();
    }
    let mut buf = vec![0u8; len as usize];
    f.read_exact(&mut buf).expect("read_string failed");
    String::from_utf8_lossy(&buf).into_owned()
}

fn write_vec3(f: &mut BufWriter<File>, v: &[f32; 3]) {
    write_vec3_generic(f, v);
}

fn read_vec3(f: &mut BufReader<File>) -> [f32; 3] {
    [read_f32(f), read_f32(f), read_f32(f)]
}

fn write_option_usize(f: &mut BufWriter<File>, val: Option<usize>) {
    match val {
        Some(v) => write_i32(f, v as i32),
        None => write_i32(f, -1),
    }
}

fn read_option_usize(f: &mut BufReader<File>) -> Option<usize> {
    let idx = read_i32(f);
    if idx < 0 { None } else { Some(idx as usize) }
}

// ============================================================
// Edict field read/write (generic)
// ============================================================

/// Write a single edict field's saveable data (generic version).
fn write_edict_field_generic<W: Write>(f: &mut W, field: &FieldDef, ent: &Edict) {
    if field.flags & FFL_SPAWNTEMP != 0 {
        return;
    }
    match field.field_type {
        FieldType::Int | FieldType::Float | FieldType::AngleHack |
        FieldType::Vector | FieldType::Ignore => {
            // These are written as part of the struct blob; handled in write_edict
        }
        FieldType::LString | FieldType::GString => {
            let s = get_edict_string(ent, field.field_id);
            write_string_generic(f, &s);
        }
        FieldType::Edict => {
            let idx = get_edict_entity_ref(ent, field.field_id);
            write_i32_generic(f, idx);
        }
        FieldType::Client => {
            // Not used in edict fields table
        }
        FieldType::Item => {
            let idx = match get_edict_item_ref(ent, field.field_id) {
                Some(v) => v as i32,
                None => -1,
            };
            write_i32_generic(f, idx);
        }
        FieldType::Function => {
            let idx = match get_edict_function(ent, field.field_id) {
                Some(v) => v as i32,
                None => -1,
            };
            write_i32_generic(f, idx);
        }
        FieldType::MMove => {
            let idx = match get_edict_mmove(ent, field.field_id) {
                Some(v) => v as i32,
                None => -1,
            };
            write_i32_generic(f, idx);
        }
    }
}

/// Write the core edict data to a buffer (generic version for parallel serialization).
fn write_edict_data_generic<W: Write>(f: &mut W, ent: &Edict) {
    write_i32_generic(f, ent.spawnflags);
    write_f32_generic(f, ent.speed);
    write_f32_generic(f, ent.accel);
    write_f32_generic(f, ent.decel);
    write_f32_generic(f, ent.wait);
    write_f32_generic(f, ent.delay);
    write_f32_generic(f, ent.random);
    write_vec3_generic(f, &ent.move_origin);
    write_vec3_generic(f, &ent.move_angles);
    write_i32_generic(f, ent.style);
    write_i32_generic(f, ent.count);
    write_i32_generic(f, ent.health);
    write_i32_generic(f, ent.sounds);
    write_i32_generic(f, ent.dmg);
    write_i32_generic(f, ent.mass);
    write_f32_generic(f, ent.volume);
    write_f32_generic(f, ent.attenuation);
    write_vec3_generic(f, &ent.s.origin);
    write_vec3_generic(f, &ent.s.angles);
    write_i32_generic(f, if ent.inuse { 1 } else { 0 });
    write_i32_generic(f, ent.movetype as i32);
    write_i32_generic(f, ent.flags.bits());
    write_f32_generic(f, ent.freetime);
    write_f32_generic(f, ent.angle);
    write_f32_generic(f, ent.timestamp);
    write_i32_generic(f, ent.svflags);
    write_vec3_generic(f, &ent.mins);
    write_vec3_generic(f, &ent.maxs);
    write_vec3_generic(f, &ent.absmin);
    write_vec3_generic(f, &ent.absmax);
    write_vec3_generic(f, &ent.size);
    write_i32_generic(f, ent.solid as i32);
    write_i32_generic(f, ent.clipmask);
    write_vec3_generic(f, &ent.velocity);
    write_vec3_generic(f, &ent.avelocity);
    write_f32_generic(f, ent.air_finished);
    write_f32_generic(f, ent.gravity);
    write_f32_generic(f, ent.yaw_speed);
    write_f32_generic(f, ent.ideal_yaw);
    write_f32_generic(f, ent.nextthink);
    write_f32_generic(f, ent.touch_debounce_time);
    write_f32_generic(f, ent.pain_debounce_time);
    write_f32_generic(f, ent.damage_debounce_time);
    write_f32_generic(f, ent.fly_sound_debounce_time);
    write_f32_generic(f, ent.last_move_time);
    write_i32_generic(f, ent.max_health);
    write_i32_generic(f, ent.gib_health);
    write_i32_generic(f, ent.deadflag);
    write_f32_generic(f, ent.show_hostile);
    write_f32_generic(f, ent.powerarmor_time);
    write_i32_generic(f, ent.viewheight);
    write_i32_generic(f, ent.takedamage);
    write_i32_generic(f, ent.radius_dmg);
    write_f32_generic(f, ent.dmg_radius);
    write_i32_generic(f, ent.noise_index);
    write_i32_generic(f, ent.noise_index2);
    write_f32_generic(f, ent.teleport_time);
    write_i32_generic(f, ent.watertype);
    write_i32_generic(f, ent.waterlevel);
    write_i32_generic(f, ent.light_level);
    write_i32_generic(f, ent.linkcount);
    write_i32_generic(f, ent.num_clusters);
    write_i32_generic(f, ent.headnode);
    write_i32_generic(f, ent.areanum);
    write_i32_generic(f, ent.areanum2);
    write_i32_generic(f, ent.groundentity_linkcount);
    write_i32_generic(f, match ent.client { Some(v) => v as i32, None => -1 });
    write_vec3_generic(f, &ent.movedir);
    write_vec3_generic(f, &ent.pos1);
    write_vec3_generic(f, &ent.pos2);

    // Write pointer fields via field table
    let fields = edict_fields();
    for field in &fields {
        write_edict_field_generic(f, field, ent);
    }
}

/// Serialize an entity to a byte buffer (for parallel serialization).
/// Returns (entity_index, serialized_data).
fn serialize_edict_to_buffer(index: usize, ent: &Edict) -> (usize, Vec<u8>) {
    let mut buffer = Vec::with_capacity(1024); // Pre-allocate typical size
    // Write entity index
    write_i32_generic(&mut buffer, index as i32);
    // Write entity data
    write_edict_data_generic(&mut buffer, ent);
    (index, buffer)
}

// ============================================================
// Edict field read/write (file-specific)
// ============================================================

/// Write a single edict field's saveable data.
/// Mirrors C `WriteField1` + `WriteField2` combined.
fn write_edict_field(f: &mut BufWriter<File>, field: &FieldDef, ent: &Edict) {
    write_edict_field_generic(f, field, ent);
}

/// Read a single edict field from save file.
/// Mirrors C `ReadField` for edict fields.
fn read_edict_field(f: &mut BufReader<File>, field: &FieldDef, ent: &mut Edict) {
    if field.flags & FFL_SPAWNTEMP != 0 {
        return;
    }
    match field.field_type {
        FieldType::Int | FieldType::Float | FieldType::AngleHack |
        FieldType::Vector | FieldType::Ignore => {
            // These are read as part of the struct blob; handled in read_edict
        }
        FieldType::LString | FieldType::GString => {
            let s = read_string(f);
            set_edict_string(ent, field.field_id, s);
        }
        FieldType::Edict => {
            let idx = read_i32(f);
            set_edict_entity_ref(ent, field.field_id, idx);
        }
        FieldType::Client => {}
        FieldType::Item => {
            let idx = read_i32(f);
            set_edict_item_ref(ent, field.field_id, if idx < 0 { None } else { Some(idx as usize) });
        }
        FieldType::Function => {
            let idx = read_i32(f);
            set_edict_function(ent, field.field_id, if idx < 0 { None } else { Some(idx as usize) });
        }
        FieldType::MMove => {
            let idx = read_i32(f);
            set_edict_mmove(ent, field.field_id, if idx < 0 { None } else { Some(idx as usize) });
        }
    }
}

// ============================================================
// Field accessors for Edict
// ============================================================

fn get_edict_string(ent: &Edict, id: FieldId) -> String {
    match id {
        FieldId::Classname    => ent.classname.clone(),
        FieldId::Model        => ent.model.clone(),
        FieldId::Target       => ent.target.clone(),
        FieldId::Targetname   => ent.targetname.clone(),
        FieldId::Pathtarget   => ent.pathtarget.clone(),
        FieldId::Deathtarget  => ent.deathtarget.clone(),
        FieldId::Killtarget   => ent.killtarget.clone(),
        FieldId::Combattarget => ent.combattarget.clone(),
        FieldId::Message      => ent.message.clone(),
        FieldId::Team         => ent.team.clone(),
        FieldId::Map          => ent.map.clone(),
        _ => String::new(),
    }
}

fn set_edict_string(ent: &mut Edict, id: FieldId, val: String) {
    match id {
        FieldId::Classname    => ent.classname = val,
        FieldId::Model        => ent.model = val,
        FieldId::Target       => ent.target = val,
        FieldId::Targetname   => ent.targetname = val,
        FieldId::Pathtarget   => ent.pathtarget = val,
        FieldId::Deathtarget  => ent.deathtarget = val,
        FieldId::Killtarget   => ent.killtarget = val,
        FieldId::Combattarget => ent.combattarget = val,
        FieldId::Message      => ent.message = val,
        FieldId::Team         => ent.team = val,
        FieldId::Map          => ent.map = val,
        _ => {}
    }
}

fn get_edict_entity_ref(ent: &Edict, id: FieldId) -> i32 {
    match id {
        FieldId::GoalEntity   => ent.goalentity,
        FieldId::MoveTarget   => ent.movetarget,
        FieldId::Enemy        => ent.enemy,
        FieldId::OldEnemy     => ent.oldenemy,
        FieldId::Activator    => ent.activator,
        FieldId::GroundEntity => ent.groundentity,
        FieldId::TeamChain    => ent.teamchain,
        FieldId::TeamMaster   => ent.teammaster,
        FieldId::Owner        => ent.owner,
        FieldId::MyNoise      => ent.mynoise,
        FieldId::MyNoise2     => ent.mynoise2,
        FieldId::TargetEnt    => ent.target_ent,
        FieldId::Chain        => ent.chain,
        _ => -1,
    }
}

fn set_edict_entity_ref(ent: &mut Edict, id: FieldId, val: i32) {
    match id {
        FieldId::GoalEntity   => ent.goalentity = val,
        FieldId::MoveTarget   => ent.movetarget = val,
        FieldId::Enemy        => ent.enemy = val,
        FieldId::OldEnemy     => ent.oldenemy = val,
        FieldId::Activator    => ent.activator = val,
        FieldId::GroundEntity => ent.groundentity = val,
        FieldId::TeamChain    => ent.teamchain = val,
        FieldId::TeamMaster   => ent.teammaster = val,
        FieldId::Owner        => ent.owner = val,
        FieldId::MyNoise      => ent.mynoise = val,
        FieldId::MyNoise2     => ent.mynoise2 = val,
        FieldId::TargetEnt    => ent.target_ent = val,
        FieldId::Chain        => ent.chain = val,
        _ => {}
    }
}

fn get_edict_item_ref(ent: &Edict, id: FieldId) -> Option<usize> {
    match id {
        FieldId::EdictItem => ent.item,
        _ => None,
    }
}

fn set_edict_item_ref(ent: &mut Edict, id: FieldId, val: Option<usize>) {
    if id == FieldId::EdictItem { ent.item = val }
}

fn get_edict_function(ent: &Edict, id: FieldId) -> Option<usize> {
    match id {
        FieldId::PreThink      => ent.prethink_fn,
        FieldId::Think         => ent.think_fn,
        FieldId::Blocked       => ent.blocked_fn,
        FieldId::Touch         => ent.touch_fn,
        FieldId::Use           => ent.use_fn,
        FieldId::Pain          => ent.pain_fn,
        FieldId::Die           => ent.die_fn,
        FieldId::MiStand       => ent.monsterinfo.stand_fn,
        FieldId::MiIdle        => ent.monsterinfo.idle_fn,
        FieldId::MiSearch      => ent.monsterinfo.search_fn,
        FieldId::MiWalk        => ent.monsterinfo.walk_fn,
        FieldId::MiRun         => ent.monsterinfo.run_fn,
        FieldId::MiDodge       => ent.monsterinfo.dodge_fn,
        FieldId::MiAttack      => ent.monsterinfo.attack_fn,
        FieldId::MiMelee       => ent.monsterinfo.melee_fn,
        FieldId::MiSight       => ent.monsterinfo.sight_fn,
        FieldId::MiCheckAttack => ent.monsterinfo.checkattack_fn,
        FieldId::MiEndFunc     => ent.moveinfo.endfunc,
        _ => None,
    }
}

fn set_edict_function(ent: &mut Edict, id: FieldId, val: Option<usize>) {
    match id {
        FieldId::PreThink      => ent.prethink_fn = val,
        FieldId::Think         => ent.think_fn = val,
        FieldId::Blocked       => ent.blocked_fn = val,
        FieldId::Touch         => ent.touch_fn = val,
        FieldId::Use           => ent.use_fn = val,
        FieldId::Pain          => ent.pain_fn = val,
        FieldId::Die           => ent.die_fn = val,
        FieldId::MiStand       => ent.monsterinfo.stand_fn = val,
        FieldId::MiIdle        => ent.monsterinfo.idle_fn = val,
        FieldId::MiSearch      => ent.monsterinfo.search_fn = val,
        FieldId::MiWalk        => ent.monsterinfo.walk_fn = val,
        FieldId::MiRun         => ent.monsterinfo.run_fn = val,
        FieldId::MiDodge       => ent.monsterinfo.dodge_fn = val,
        FieldId::MiAttack      => ent.monsterinfo.attack_fn = val,
        FieldId::MiMelee       => ent.monsterinfo.melee_fn = val,
        FieldId::MiSight       => ent.monsterinfo.sight_fn = val,
        FieldId::MiCheckAttack => ent.monsterinfo.checkattack_fn = val,
        FieldId::MiEndFunc     => ent.moveinfo.endfunc = val,
        _ => {}
    }
}

fn get_edict_mmove(ent: &Edict, id: FieldId) -> Option<usize> {
    match id {
        FieldId::MiCurrentMove => ent.monsterinfo.currentmove,
        _ => None,
    }
}

fn set_edict_mmove(ent: &mut Edict, id: FieldId, val: Option<usize>) {
    if id == FieldId::MiCurrentMove { ent.monsterinfo.currentmove = val }
}

// ============================================================
// Level locals field accessors
// ============================================================

fn write_level_field(f: &mut BufWriter<File>, field: &FieldDef, level: &LevelLocals) {
    match field.field_type {
        FieldType::LString => {
            let s = match field.field_id {
                FieldId::LlChangeMap => &level.changemap,
                _ => return,
            };
            write_string(f, s);
        }
        FieldType::Edict => {
            let idx = match field.field_id {
                FieldId::LlSightClient  => level.sight_client,
                FieldId::LlSightEntity  => level.sight_entity,
                FieldId::LlSoundEntity  => level.sound_entity,
                FieldId::LlSound2Entity => level.sound2_entity,
                _ => -1,
            };
            write_i32(f, idx);
        }
        _ => {}
    }
}

fn read_level_field(f: &mut BufReader<File>, field: &FieldDef, level: &mut LevelLocals) {
    match field.field_type {
        FieldType::LString => {
            let s = read_string(f);
            if field.field_id == FieldId::LlChangeMap { level.changemap = s }
        }
        FieldType::Edict => {
            let idx = read_i32(f);
            match field.field_id {
                FieldId::LlSightClient  => level.sight_client = idx,
                FieldId::LlSightEntity  => level.sight_entity = idx,
                FieldId::LlSoundEntity  => level.sound_entity = idx,
                FieldId::LlSound2Entity => level.sound2_entity = idx,
                _ => {}
            }
        }
        _ => {}
    }
}

// ============================================================
// Client field accessors
// ============================================================

fn write_client_field(f: &mut BufWriter<File>, field: &FieldDef, client: &GClient) {
    if field.field_type == FieldType::Item {
        let idx = match field.field_id {
            FieldId::ClPersWeapon     => client.pers.weapon,
            FieldId::ClPersLastWeapon => client.pers.lastweapon,
            FieldId::ClNewWeapon      => client.newweapon,
            _ => None,
        };
        write_option_usize(f, idx);
    }
}

fn read_client_field(f: &mut BufReader<File>, field: &FieldDef, client: &mut GClient) {
    if field.field_type == FieldType::Item {
        let idx = read_option_usize(f);
        match field.field_id {
            FieldId::ClPersWeapon     => client.pers.weapon = idx,
            FieldId::ClPersLastWeapon => client.pers.lastweapon = idx,
            FieldId::ClNewWeapon      => client.newweapon = idx,
            _ => {}
        }
    }
}

// ============================================================
// Edict serialization
// ============================================================

/// Write the core (non-pointer) edict data, then pointer fields.
/// Mirrors C `WriteEdict`.
fn write_edict_data(f: &mut BufWriter<File>, ent: &Edict) {
    // Write scalar/vector fields inline
    write_i32(f, ent.spawnflags);
    write_f32(f, ent.speed);
    write_f32(f, ent.accel);
    write_f32(f, ent.decel);
    write_f32(f, ent.wait);
    write_f32(f, ent.delay);
    write_f32(f, ent.random);
    write_vec3(f, &ent.move_origin);
    write_vec3(f, &ent.move_angles);
    write_i32(f, ent.style);
    write_i32(f, ent.count);
    write_i32(f, ent.health);
    write_i32(f, ent.sounds);
    write_i32(f, ent.dmg);
    write_i32(f, ent.mass);
    write_f32(f, ent.volume);
    write_f32(f, ent.attenuation);
    write_vec3(f, &ent.s.origin);
    write_vec3(f, &ent.s.angles);
    write_i32(f, if ent.inuse { 1 } else { 0 });
    write_i32(f, ent.movetype as i32);
    write_i32(f, ent.flags.bits());
    write_f32(f, ent.freetime);
    write_f32(f, ent.angle);
    write_f32(f, ent.timestamp);
    write_i32(f, ent.svflags);
    write_vec3(f, &ent.mins);
    write_vec3(f, &ent.maxs);
    write_vec3(f, &ent.absmin);
    write_vec3(f, &ent.absmax);
    write_vec3(f, &ent.size);
    write_i32(f, ent.solid as i32);
    write_i32(f, ent.clipmask);
    write_vec3(f, &ent.velocity);
    write_vec3(f, &ent.avelocity);
    write_f32(f, ent.air_finished);
    write_f32(f, ent.gravity);
    write_f32(f, ent.yaw_speed);
    write_f32(f, ent.ideal_yaw);
    write_f32(f, ent.nextthink);
    write_f32(f, ent.touch_debounce_time);
    write_f32(f, ent.pain_debounce_time);
    write_f32(f, ent.damage_debounce_time);
    write_f32(f, ent.fly_sound_debounce_time);
    write_f32(f, ent.last_move_time);
    write_i32(f, ent.max_health);
    write_i32(f, ent.gib_health);
    write_i32(f, ent.deadflag);
    write_f32(f, ent.show_hostile);
    write_f32(f, ent.powerarmor_time);
    write_i32(f, ent.viewheight);
    write_i32(f, ent.takedamage);
    write_i32(f, ent.radius_dmg);
    write_f32(f, ent.dmg_radius);
    write_i32(f, ent.noise_index);
    write_i32(f, ent.noise_index2);
    write_f32(f, ent.teleport_time);
    write_i32(f, ent.watertype);
    write_i32(f, ent.waterlevel);
    write_i32(f, ent.light_level);
    write_i32(f, ent.linkcount);
    write_i32(f, ent.num_clusters);
    write_i32(f, ent.headnode);
    write_i32(f, ent.areanum);
    write_i32(f, ent.areanum2);
    write_i32(f, ent.groundentity_linkcount);
    write_i32(f, match ent.client { Some(v) => v as i32, None => -1 });
    write_vec3(f, &ent.movedir);
    write_vec3(f, &ent.pos1);
    write_vec3(f, &ent.pos2);

    // Write pointer fields via field table
    let fields = edict_fields();
    for field in &fields {
        write_edict_field(f, field, ent);
    }
}

/// Read the core edict data and pointer fields.
/// Mirrors C `ReadEdict`.
fn read_edict_data(f: &mut BufReader<File>, ent: &mut Edict) {
    ent.spawnflags = read_i32(f);
    ent.speed = read_f32(f);
    ent.accel = read_f32(f);
    ent.decel = read_f32(f);
    ent.wait = read_f32(f);
    ent.delay = read_f32(f);
    ent.random = read_f32(f);
    ent.move_origin = read_vec3(f);
    ent.move_angles = read_vec3(f);
    ent.style = read_i32(f);
    ent.count = read_i32(f);
    ent.health = read_i32(f);
    ent.sounds = read_i32(f);
    ent.dmg = read_i32(f);
    ent.mass = read_i32(f);
    ent.volume = read_f32(f);
    ent.attenuation = read_f32(f);
    ent.s.origin = read_vec3(f);
    ent.s.angles = read_vec3(f);
    ent.inuse = read_i32(f) != 0;
    ent.movetype = match read_i32(f) {
        0 => MoveType::None,
        1 => MoveType::Noclip,
        2 => MoveType::Push,
        3 => MoveType::Stop,
        4 => MoveType::Walk,
        5 => MoveType::Step,
        6 => MoveType::Fly,
        7 => MoveType::Toss,
        8 => MoveType::FlyMissile,
        9 => MoveType::Bounce,
        _ => MoveType::None,
    };
    ent.flags = crate::g_local::EntityFlags::from_bits_truncate(read_i32(f));
    ent.freetime = read_f32(f);
    ent.angle = read_f32(f);
    ent.timestamp = read_f32(f);
    ent.svflags = read_i32(f);
    ent.mins = read_vec3(f);
    ent.maxs = read_vec3(f);
    ent.absmin = read_vec3(f);
    ent.absmax = read_vec3(f);
    ent.size = read_vec3(f);
    ent.solid = match read_i32(f) {
        0 => Solid::Not,
        1 => Solid::Trigger,
        2 => Solid::Bbox,
        3 => Solid::Bsp,
        _ => Solid::Not,
    };
    ent.clipmask = read_i32(f);
    ent.velocity = read_vec3(f);
    ent.avelocity = read_vec3(f);
    ent.air_finished = read_f32(f);
    ent.gravity = read_f32(f);
    ent.yaw_speed = read_f32(f);
    ent.ideal_yaw = read_f32(f);
    ent.nextthink = read_f32(f);
    ent.touch_debounce_time = read_f32(f);
    ent.pain_debounce_time = read_f32(f);
    ent.damage_debounce_time = read_f32(f);
    ent.fly_sound_debounce_time = read_f32(f);
    ent.last_move_time = read_f32(f);
    ent.max_health = read_i32(f);
    ent.gib_health = read_i32(f);
    ent.deadflag = read_i32(f);
    ent.show_hostile = read_f32(f);
    ent.powerarmor_time = read_f32(f);
    ent.viewheight = read_i32(f);
    ent.takedamage = read_i32(f);
    ent.radius_dmg = read_i32(f);
    ent.dmg_radius = read_f32(f);
    ent.noise_index = read_i32(f);
    ent.noise_index2 = read_i32(f);
    ent.teleport_time = read_f32(f);
    ent.watertype = read_i32(f);
    ent.waterlevel = read_i32(f);
    ent.light_level = read_i32(f);
    ent.linkcount = read_i32(f);
    ent.num_clusters = read_i32(f);
    ent.headnode = read_i32(f);
    ent.areanum = read_i32(f);
    ent.areanum2 = read_i32(f);
    ent.groundentity_linkcount = read_i32(f);
    let client_idx = read_i32(f);
    ent.client = if client_idx < 0 { None } else { Some(client_idx as usize) };
    ent.movedir = read_vec3(f);
    ent.pos1 = read_vec3(f);
    ent.pos2 = read_vec3(f);

    // Read pointer fields via field table
    let fields = edict_fields();
    for field in &fields {
        read_edict_field(f, field, ent);
    }
}

// ============================================================
// Client serialization
// ============================================================

/// Write a client to the save file.
/// Mirrors C `WriteClient`.
fn write_client_data(f: &mut BufWriter<File>, client: &GClient) {
    // In C, this writes the entire gclient_t struct as a raw binary blob.
    // In Rust, we serialize via the field table. Full binary compat deferred.
    gi_dprintf("WriteClient: writing client data (scalar serialization placeholder)\n");

    let fields = client_fields();
    for field in &fields {
        write_client_field(f, field, client);
    }
}

/// Read a client from the save file.
/// Mirrors C `ReadClient`.
fn read_client_data(f: &mut BufReader<File>, client: &mut GClient) {
    // In C, this reads the entire gclient_t struct as a raw binary blob.
    // In Rust, we deserialize via the field table. Full binary compat deferred.
    gi_dprintf("ReadClient: reading client data (scalar serialization placeholder)\n");

    let fields = client_fields();
    for field in &fields {
        read_client_field(f, field, client);
    }
}

// ============================================================
// Level locals serialization
// ============================================================

/// Write level locals to save file.
/// Mirrors C `WriteLevelLocals`.
fn write_level_locals(f: &mut BufWriter<File>, level: &LevelLocals) {
    // Write scalar fields
    write_i32(f, level.framenum);
    write_f32(f, level.time);
    write_string(f, &level.level_name);
    write_string(f, &level.mapname);
    write_string(f, &level.nextmap);
    write_f32(f, level.intermissiontime);
    write_i32(f, level.exitintermission);
    write_vec3(f, &level.intermission_origin);
    write_vec3(f, &level.intermission_angle);
    write_i32(f, level.sight_entity_framenum);
    write_i32(f, level.sound_entity_framenum);
    write_i32(f, level.sound2_entity_framenum);
    write_i32(f, level.pic_health);
    write_i32(f, level.total_secrets);
    write_i32(f, level.found_secrets);
    write_i32(f, level.total_goals);
    write_i32(f, level.found_goals);
    write_i32(f, level.total_monsters);
    write_i32(f, level.killed_monsters);
    write_i32(f, level.current_entity);
    write_i32(f, level.body_que);
    write_i32(f, level.power_cubes);

    // Write pointer fields
    let fields = level_fields();
    for field in &fields {
        write_level_field(f, field, level);
    }
}

/// Read level locals from save file.
/// Mirrors C `ReadLevelLocals`.
fn read_level_locals(f: &mut BufReader<File>, level: &mut LevelLocals) {
    level.framenum = read_i32(f);
    level.time = read_f32(f);
    level.level_name = read_string(f);
    level.mapname = read_string(f);
    level.nextmap = read_string(f);
    level.intermissiontime = read_f32(f);
    level.exitintermission = read_i32(f);
    level.intermission_origin = read_vec3(f);
    level.intermission_angle = read_vec3(f);
    level.sight_entity_framenum = read_i32(f);
    level.sound_entity_framenum = read_i32(f);
    level.sound2_entity_framenum = read_i32(f);
    level.pic_health = read_i32(f);
    level.total_secrets = read_i32(f);
    level.found_secrets = read_i32(f);
    level.total_goals = read_i32(f);
    level.found_goals = read_i32(f);
    level.total_monsters = read_i32(f);
    level.killed_monsters = read_i32(f);
    level.current_entity = read_i32(f);
    level.body_que = read_i32(f);
    level.power_cubes = read_i32(f);

    // Read pointer fields
    let fields = level_fields();
    for field in &fields {
        read_level_field(f, field, level);
    }
}

// ============================================================
// Game-level serialization
// ============================================================

/// Write the game locals to save file.
fn write_game_locals(f: &mut BufWriter<File>, game: &GameLocals) {
    write_string(f, &game.helpmessage1);
    write_string(f, &game.helpmessage2);
    write_i32(f, game.helpchanged);
    write_string(f, &game.spawnpoint);
    write_i32(f, game.maxclients);
    write_i32(f, game.maxentities);
    write_i32(f, game.serverflags);
    write_i32(f, game.num_items);
    write_i32(f, if game.autosaved { 1 } else { 0 });
}

/// Read the game locals from save file.
fn read_game_locals(f: &mut BufReader<File>, game: &mut GameLocals) {
    game.helpmessage1 = read_string(f);
    game.helpmessage2 = read_string(f);
    game.helpchanged = read_i32(f);
    game.spawnpoint = read_string(f);
    game.maxclients = read_i32(f);
    game.maxentities = read_i32(f);
    game.serverflags = read_i32(f);
    game.num_items = read_i32(f);
    game.autosaved = read_i32(f) != 0;
}

// ============================================================
// Public API — mirrors the C functions
// ============================================================

/// Save version string for compatibility checking.
const SAVE_VERSION: &str = "myq2-rust-save-v1";

/// Initialize the game. Called when the DLL is first loaded.
/// Mirrors C `InitGame`.
pub fn init_game(ctx: &mut SaveContext) {
    gi_dprintf("==== InitGame ====\n");

    // gi.cvar calls — placeholder: in final version these register cvars via gi interface
    gi_dprintf("InitGame: registering cvars (placeholder)\n");

    // InitItems deferred: g_items::GameContext differs from g_save::SaveContext

    ctx.game.helpmessage1 = String::new();
    ctx.game.helpmessage2 = String::new();

    // Initialize all entities for this game
    // In C: game.maxentities = maxentities->value
    // Here we assume maxentities is already set in game struct
    ctx.edicts.clear();
    ctx.edicts.resize(ctx.game.maxentities as usize, Edict::default());

    // Initialize all clients for this game
    ctx.clients.clear();
    ctx.clients.resize(ctx.game.maxclients as usize, GClient::default());

    *ctx.num_edicts = ctx.game.maxclients + 1;
}

/// Write the game state to a file.
/// Called on level transitions and explicit saves.
/// Mirrors C `WriteGame`.
pub fn write_game(ctx: &mut SaveContext, filename: &str, autosave: bool) {
    if !autosave {
        // SaveClientData deferred: p_client::GameContext differs from g_save::SaveContext
    }

    let file = File::create(filename)
        .unwrap_or_else(|_| panic!("Couldn't open {} for writing", filename));
    let mut f = BufWriter::new(file);

    // Write version string
    write_string(&mut f, SAVE_VERSION);

    // Write game locals
    ctx.game.autosaved = autosave;
    write_game_locals(&mut f, ctx.game);
    ctx.game.autosaved = false;

    // Write all clients
    for i in 0..ctx.game.maxclients as usize {
        write_client_data(&mut f, &ctx.clients[i]);
    }
}

/// Read the game state from a file.
/// Mirrors C `ReadGame`.
pub fn read_game(ctx: &mut SaveContext, filename: &str) {
    gi_free_tags(TAG_GAME);

    let file = File::open(filename)
        .unwrap_or_else(|_| panic!("Couldn't open {} for reading", filename));
    let mut f = BufReader::new(file);

    // Read and check version string
    let version = read_string(&mut f);
    if version != SAVE_VERSION {
        panic!("Savegame from an older version.");
    }

    // Read game locals
    read_game_locals(&mut f, ctx.game);

    // Reinitialize edicts and clients
    ctx.edicts.clear();
    ctx.edicts.resize(ctx.game.maxentities as usize, Edict::default());

    ctx.clients.clear();
    ctx.clients.resize(ctx.game.maxclients as usize, GClient::default());

    // Read all clients
    for i in 0..ctx.game.maxclients as usize {
        read_client_data(&mut f, &mut ctx.clients[i]);
    }
}

/// Threshold for parallel entity serialization.
/// Below this count, sequential serialization has less overhead.
const PARALLEL_SAVE_THRESHOLD: usize = 32;

/// Write the current level state to a file.
/// Mirrors C `WriteLevel`.
///
/// This version uses parallel entity serialization when there are many entities,
/// which improves save performance in levels with hundreds of entities.
pub fn write_level(ctx: &SaveContext, filename: &str) {
    let file = File::create(filename)
        .unwrap_or_else(|_| panic!("Couldn't open {} for writing", filename));
    let mut f = BufWriter::new(file);

    // Write out a sentinel/version marker (replaces edict size check and function pointer check)
    write_i32(&mut f, 0x4D515253); // "MQRS" magic number

    // Write level locals
    write_level_locals(&mut f, ctx.level);

    // Collect in-use entity indices
    let num_edicts = *ctx.num_edicts as usize;
    let in_use_entities: Vec<usize> = (0..num_edicts)
        .filter(|&i| ctx.edicts[i].inuse)
        .collect();

    // Use parallel serialization for many entities
    if in_use_entities.len() > PARALLEL_SAVE_THRESHOLD {
        // Serialize entities in parallel to byte buffers
        let mut serialized: Vec<(usize, Vec<u8>)> = in_use_entities
            .par_iter()
            .map(|&i| serialize_edict_to_buffer(i, &ctx.edicts[i]))
            .collect();

        // Sort by entity index to maintain deterministic order
        serialized.sort_by_key(|(idx, _)| *idx);

        // Write serialized data sequentially to file
        for (_, buffer) in serialized {
            f.write_all(&buffer).expect("Failed to write entity data");
        }
    } else {
        // Sequential serialization for few entities
        for &i in &in_use_entities {
            write_i32(&mut f, i as i32);
            write_edict_data(&mut f, &ctx.edicts[i]);
        }
    }

    // Terminator
    write_i32(&mut f, -1);
}

/// Read the level state from a file.
/// SpawnEntities will already have been called on the level the same way
/// it was when the level was saved, to get baselines set up identically.
/// Mirrors C `ReadLevel`.
pub fn read_level(ctx: &mut SaveContext, filename: &str) {
    let file = File::open(filename)
        .unwrap_or_else(|_| panic!("Couldn't open {} for reading", filename));
    let mut f = BufReader::new(file);

    gi_free_tags(TAG_LEVEL);

    // Wipe all entities
    let max_ents = ctx.game.maxentities as usize;
    ctx.edicts.clear();
    ctx.edicts.resize(max_ents, Edict::default());
    *ctx.num_edicts = ctx.game.maxclients + 1;

    // Check magic number
    let magic = read_i32(&mut f);
    if magic != 0x4D515253 {
        panic!("ReadLevel: mismatched save format");
    }

    // Load level locals
    read_level_locals(&mut f, ctx.level);

    // Load all entities
    loop {
        let entnum = read_i32(&mut f);
        if entnum == -1 {
            break;
        }
        if entnum >= *ctx.num_edicts {
            *ctx.num_edicts = entnum + 1;
        }

        read_edict_data(&mut f, &mut ctx.edicts[entnum as usize]);

        // Clear area link for server to rebuild
        ctx.edicts[entnum as usize].area = AreaLink::default();

        gi_linkentity(entnum);
    }

    // Mark all clients as unconnected
    for i in 0..ctx.game.maxclients as usize {
        ctx.edicts[i + 1].client = Some(i);
        ctx.clients[i].pers.connected = false;
    }

    // Fire any cross-level triggers
    let num_edicts = *ctx.num_edicts as usize;
    for i in 0..num_edicts {
        if !ctx.edicts[i].inuse {
            continue;
        }
        if ctx.edicts[i].classname == "target_crosslevel_target" {
            ctx.edicts[i].nextthink = ctx.level.time + ctx.edicts[i].delay;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    // ============================================================
    // Helper: read values back from a byte buffer written by generic writers
    // ============================================================

    fn read_i32_from_cursor(c: &mut Cursor<Vec<u8>>) -> i32 {
        let mut buf = [0u8; 4];
        std::io::Read::read_exact(c, &mut buf).expect("read_i32 failed");
        i32::from_le_bytes(buf)
    }

    fn read_f32_from_cursor(c: &mut Cursor<Vec<u8>>) -> f32 {
        let mut buf = [0u8; 4];
        std::io::Read::read_exact(c, &mut buf).expect("read_f32 failed");
        f32::from_le_bytes(buf)
    }

    fn read_string_from_cursor(c: &mut Cursor<Vec<u8>>) -> String {
        let len = read_i32_from_cursor(c);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u8; len as usize];
        std::io::Read::read_exact(c, &mut buf).expect("read_string failed");
        String::from_utf8_lossy(&buf).into_owned()
    }

    fn read_vec3_from_cursor(c: &mut Cursor<Vec<u8>>) -> [f32; 3] {
        [
            read_f32_from_cursor(c),
            read_f32_from_cursor(c),
            read_f32_from_cursor(c),
        ]
    }

    fn read_option_usize_from_cursor(c: &mut Cursor<Vec<u8>>) -> Option<usize> {
        let idx = read_i32_from_cursor(c);
        if idx < 0 { None } else { Some(idx as usize) }
    }

    // ============================================================
    // write_i32_generic round-trip tests
    // ============================================================

    #[test]
    fn test_write_i32_zero() {
        let mut buf = Vec::new();
        write_i32_generic(&mut buf, 0);
        let mut cursor = Cursor::new(buf);
        assert_eq!(read_i32_from_cursor(&mut cursor), 0);
    }

    #[test]
    fn test_write_i32_negative_one() {
        let mut buf = Vec::new();
        write_i32_generic(&mut buf, -1);
        let mut cursor = Cursor::new(buf);
        assert_eq!(read_i32_from_cursor(&mut cursor), -1);
    }

    #[test]
    fn test_write_i32_max() {
        let mut buf = Vec::new();
        write_i32_generic(&mut buf, i32::MAX);
        let mut cursor = Cursor::new(buf);
        assert_eq!(read_i32_from_cursor(&mut cursor), i32::MAX);
    }

    #[test]
    fn test_write_i32_min() {
        let mut buf = Vec::new();
        write_i32_generic(&mut buf, i32::MIN);
        let mut cursor = Cursor::new(buf);
        assert_eq!(read_i32_from_cursor(&mut cursor), i32::MIN);
    }

    #[test]
    fn test_write_i32_positive() {
        let mut buf = Vec::new();
        write_i32_generic(&mut buf, 42);
        let mut cursor = Cursor::new(buf);
        assert_eq!(read_i32_from_cursor(&mut cursor), 42);
    }

    // ============================================================
    // write_f32_generic round-trip tests
    // ============================================================

    #[test]
    fn test_write_f32_zero() {
        let mut buf = Vec::new();
        write_f32_generic(&mut buf, 0.0);
        let mut cursor = Cursor::new(buf);
        assert_eq!(read_f32_from_cursor(&mut cursor), 0.0);
    }

    #[test]
    fn test_write_f32_negative() {
        let mut buf = Vec::new();
        write_f32_generic(&mut buf, -1.5);
        let mut cursor = Cursor::new(buf);
        assert_eq!(read_f32_from_cursor(&mut cursor), -1.5);
    }

    #[test]
    fn test_write_f32_max() {
        let mut buf = Vec::new();
        write_f32_generic(&mut buf, f32::MAX);
        let mut cursor = Cursor::new(buf);
        assert_eq!(read_f32_from_cursor(&mut cursor), f32::MAX);
    }

    #[test]
    fn test_write_f32_min_positive() {
        let mut buf = Vec::new();
        write_f32_generic(&mut buf, f32::MIN_POSITIVE);
        let mut cursor = Cursor::new(buf);
        assert_eq!(read_f32_from_cursor(&mut cursor), f32::MIN_POSITIVE);
    }

    #[test]
    fn test_write_f32_pi() {
        let mut buf = Vec::new();
        write_f32_generic(&mut buf, std::f32::consts::PI);
        let mut cursor = Cursor::new(buf);
        assert_eq!(read_f32_from_cursor(&mut cursor), std::f32::consts::PI);
    }

    // ============================================================
    // write_string_generic round-trip tests
    // ============================================================

    #[test]
    fn test_write_string_empty() {
        let mut buf = Vec::new();
        write_string_generic(&mut buf, "");
        let mut cursor = Cursor::new(buf);
        assert_eq!(read_string_from_cursor(&mut cursor), "");
    }

    #[test]
    fn test_write_string_hello() {
        let mut buf = Vec::new();
        write_string_generic(&mut buf, "hello");
        let mut cursor = Cursor::new(buf);
        assert_eq!(read_string_from_cursor(&mut cursor), "hello");
    }

    #[test]
    fn test_write_string_long() {
        let long_str = "a".repeat(1000);
        let mut buf = Vec::new();
        write_string_generic(&mut buf, &long_str);
        let mut cursor = Cursor::new(buf);
        assert_eq!(read_string_from_cursor(&mut cursor), long_str);
    }

    #[test]
    fn test_write_string_with_spaces() {
        let mut buf = Vec::new();
        write_string_generic(&mut buf, "hello world foo bar");
        let mut cursor = Cursor::new(buf);
        assert_eq!(read_string_from_cursor(&mut cursor), "hello world foo bar");
    }

    #[test]
    fn test_write_string_special_chars() {
        let mut buf = Vec::new();
        write_string_generic(&mut buf, "path/to/file.bsp");
        let mut cursor = Cursor::new(buf);
        assert_eq!(read_string_from_cursor(&mut cursor), "path/to/file.bsp");
    }

    // ============================================================
    // write_vec3_generic round-trip tests
    // ============================================================

    #[test]
    fn test_write_vec3_zeros() {
        let mut buf = Vec::new();
        write_vec3_generic(&mut buf, &[0.0, 0.0, 0.0]);
        let mut cursor = Cursor::new(buf);
        assert_eq!(read_vec3_from_cursor(&mut cursor), [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_write_vec3_mixed() {
        let mut buf = Vec::new();
        write_vec3_generic(&mut buf, &[1.0, -2.0, 3.5]);
        let mut cursor = Cursor::new(buf);
        assert_eq!(read_vec3_from_cursor(&mut cursor), [1.0, -2.0, 3.5]);
    }

    #[test]
    fn test_write_vec3_large_values() {
        let mut buf = Vec::new();
        write_vec3_generic(&mut buf, &[99999.0, -88888.0, 77777.0]);
        let mut cursor = Cursor::new(buf);
        assert_eq!(read_vec3_from_cursor(&mut cursor), [99999.0, -88888.0, 77777.0]);
    }

    // ============================================================
    // write_option_usize_generic round-trip tests
    // ============================================================

    #[test]
    fn test_write_option_usize_none() {
        let mut buf = Vec::new();
        write_option_usize_generic(&mut buf, None);
        let mut cursor = Cursor::new(buf);
        assert_eq!(read_option_usize_from_cursor(&mut cursor), None);
    }

    #[test]
    fn test_write_option_usize_some_zero() {
        let mut buf = Vec::new();
        write_option_usize_generic(&mut buf, Some(0));
        let mut cursor = Cursor::new(buf);
        assert_eq!(read_option_usize_from_cursor(&mut cursor), Some(0));
    }

    #[test]
    fn test_write_option_usize_some_large() {
        let mut buf = Vec::new();
        write_option_usize_generic(&mut buf, Some(999));
        let mut cursor = Cursor::new(buf);
        assert_eq!(read_option_usize_from_cursor(&mut cursor), Some(999));
    }

    #[test]
    fn test_write_option_usize_some_one() {
        let mut buf = Vec::new();
        write_option_usize_generic(&mut buf, Some(1));
        let mut cursor = Cursor::new(buf);
        assert_eq!(read_option_usize_from_cursor(&mut cursor), Some(1));
    }

    // ============================================================
    // Multiple values in sequence round-trip
    // ============================================================

    #[test]
    fn test_write_multiple_values_roundtrip() {
        let mut buf = Vec::new();
        write_i32_generic(&mut buf, 42);
        write_f32_generic(&mut buf, 3.14);
        write_string_generic(&mut buf, "test");
        write_vec3_generic(&mut buf, &[1.0, 2.0, 3.0]);
        write_option_usize_generic(&mut buf, Some(7));
        write_option_usize_generic(&mut buf, None);

        let mut cursor = Cursor::new(buf);
        assert_eq!(read_i32_from_cursor(&mut cursor), 42);
        let f = read_f32_from_cursor(&mut cursor);
        assert!((f - 3.14).abs() < 0.001);
        assert_eq!(read_string_from_cursor(&mut cursor), "test");
        assert_eq!(read_vec3_from_cursor(&mut cursor), [1.0, 2.0, 3.0]);
        assert_eq!(read_option_usize_from_cursor(&mut cursor), Some(7));
        assert_eq!(read_option_usize_from_cursor(&mut cursor), None);
    }

    // ============================================================
    // get_edict_string / set_edict_string round-trips
    // ============================================================

    #[test]
    fn test_edict_string_classname_roundtrip() {
        let mut ent = Edict::default();
        set_edict_string(&mut ent, FieldId::Classname, "monster_tank".to_string());
        assert_eq!(get_edict_string(&ent, FieldId::Classname), "monster_tank");
    }

    #[test]
    fn test_edict_string_target_roundtrip() {
        let mut ent = Edict::default();
        set_edict_string(&mut ent, FieldId::Target, "my_target".to_string());
        assert_eq!(get_edict_string(&ent, FieldId::Target), "my_target");
    }

    #[test]
    fn test_edict_string_model_roundtrip() {
        let mut ent = Edict::default();
        set_edict_string(&mut ent, FieldId::Model, "*42".to_string());
        assert_eq!(get_edict_string(&ent, FieldId::Model), "*42");
    }

    #[test]
    fn test_edict_string_message_roundtrip() {
        let mut ent = Edict::default();
        set_edict_string(&mut ent, FieldId::Message, "Hello World".to_string());
        assert_eq!(get_edict_string(&ent, FieldId::Message), "Hello World");
    }

    #[test]
    fn test_edict_string_empty() {
        let mut ent = Edict::default();
        set_edict_string(&mut ent, FieldId::Classname, String::new());
        assert_eq!(get_edict_string(&ent, FieldId::Classname), "");
    }

    #[test]
    fn test_edict_string_all_fields() {
        let mut ent = Edict::default();

        let fields_and_values = [
            (FieldId::Classname, "cls"),
            (FieldId::Model, "mdl"),
            (FieldId::Target, "tgt"),
            (FieldId::Targetname, "tgtn"),
            (FieldId::Pathtarget, "ptgt"),
            (FieldId::Deathtarget, "dtgt"),
            (FieldId::Killtarget, "ktgt"),
            (FieldId::Combattarget, "ctgt"),
            (FieldId::Message, "msg"),
            (FieldId::Team, "team_a"),
            (FieldId::Map, "base1"),
        ];

        for (id, val) in &fields_and_values {
            set_edict_string(&mut ent, *id, val.to_string());
        }

        for (id, val) in &fields_and_values {
            assert_eq!(get_edict_string(&ent, *id), *val);
        }
    }

    #[test]
    fn test_edict_string_unknown_field() {
        let ent = Edict::default();
        // An unknown field ID should return empty string
        assert_eq!(get_edict_string(&ent, FieldId::GoalEntity), "");
    }

    // ============================================================
    // get_edict_entity_ref / set_edict_entity_ref round-trips
    // ============================================================

    #[test]
    fn test_edict_entity_ref_enemy() {
        let mut ent = Edict::default();
        set_edict_entity_ref(&mut ent, FieldId::Enemy, 42);
        assert_eq!(get_edict_entity_ref(&ent, FieldId::Enemy), 42);
    }

    #[test]
    fn test_edict_entity_ref_goalentity() {
        let mut ent = Edict::default();
        set_edict_entity_ref(&mut ent, FieldId::GoalEntity, 7);
        assert_eq!(get_edict_entity_ref(&ent, FieldId::GoalEntity), 7);
    }

    #[test]
    fn test_edict_entity_ref_owner() {
        let mut ent = Edict::default();
        set_edict_entity_ref(&mut ent, FieldId::Owner, 100);
        assert_eq!(get_edict_entity_ref(&ent, FieldId::Owner), 100);
    }

    #[test]
    fn test_edict_entity_ref_negative() {
        let mut ent = Edict::default();
        set_edict_entity_ref(&mut ent, FieldId::Enemy, -1);
        assert_eq!(get_edict_entity_ref(&ent, FieldId::Enemy), -1);
    }

    #[test]
    fn test_edict_entity_ref_all_fields() {
        let mut ent = Edict::default();

        let fields = [
            (FieldId::GoalEntity, 1),
            (FieldId::MoveTarget, 2),
            (FieldId::Enemy, 3),
            (FieldId::OldEnemy, 4),
            (FieldId::Activator, 5),
            (FieldId::GroundEntity, 6),
            (FieldId::TeamChain, 7),
            (FieldId::TeamMaster, 8),
            (FieldId::Owner, 9),
            (FieldId::MyNoise, 10),
            (FieldId::MyNoise2, 11),
            (FieldId::TargetEnt, 12),
            (FieldId::Chain, 13),
        ];

        for (id, val) in &fields {
            set_edict_entity_ref(&mut ent, *id, *val);
        }

        for (id, val) in &fields {
            assert_eq!(get_edict_entity_ref(&ent, *id), *val);
        }
    }

    #[test]
    fn test_edict_entity_ref_unknown_field() {
        let ent = Edict::default();
        // An unknown field should return -1
        assert_eq!(get_edict_entity_ref(&ent, FieldId::Classname), -1);
    }
}
