// myq2-game-dll — Quake 2 game DLL built from Rust
//
// This crate builds our Rust game module as a dynamic library (gamex86.dll)
// that can be loaded by any Quake 2 engine (including the original C engine).
//
// The DLL exports a single function `GetGameApi` which returns a pointer to
// a game_export_t structure containing all game callback functions.

#![allow(non_snake_case, non_camel_case_types)]

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::sync::Mutex;

use myq2_common::game_api::{
    self, edict_t, game_export_t, game_import_t, usercmd_t, GAME_API_VERSION,
};
use myq2_common::q_shared::{Trace, Vec3};

// ============================================================
// Global state
// ============================================================

/// The game_import_t struct received from the engine
static GAME_IMPORT: Mutex<Option<game_import_t>> = Mutex::new(None);

/// The game_export_t struct we return to the engine
static mut GAME_EXPORT: game_export_t = game_export_t {
    apiversion: GAME_API_VERSION,
    Init: Some(ge_init),
    Shutdown: Some(ge_shutdown),
    SpawnEntities: Some(ge_spawn_entities),
    WriteGame: Some(ge_write_game),
    ReadGame: Some(ge_read_game),
    WriteLevel: Some(ge_write_level),
    ReadLevel: Some(ge_read_level),
    ClientConnect: Some(ge_client_connect),
    ClientBegin: Some(ge_client_begin),
    ClientUserinfoChanged: Some(ge_client_userinfo_changed),
    ClientDisconnect: Some(ge_client_disconnect),
    ClientCommand: Some(ge_client_command),
    ClientThink: Some(ge_client_think),
    RunFrame: Some(ge_run_frame),
    ServerCommand: Some(ge_server_command),
    edicts: std::ptr::null_mut(),
    edict_size: 0,
    num_edicts: 0,
    max_edicts: 0,
};

/// The game context - holds all game state
static GAME_CONTEXT: Mutex<Option<myq2_game::g_local::GameContext>> = Mutex::new(None);

/// Edict array storage - allocated once and kept alive
static EDICT_STORAGE: Mutex<Option<Vec<u8>>> = Mutex::new(None);

// ============================================================
// GetGameApi — the DLL entry point
// ============================================================

/// The main entry point for the game DLL.
///
/// Called by the engine when loading the game DLL.
/// Receives a pointer to the game_import_t struct containing engine functions.
/// Returns a pointer to our game_export_t struct containing game callbacks.
///
/// # Safety
/// The import pointer must be valid and remain valid for the lifetime of the DLL.
#[no_mangle]
pub unsafe extern "C" fn GetGameApi(import: *mut game_import_t) -> *mut game_export_t {
    if import.is_null() {
        return std::ptr::null_mut();
    }

    // Store the import table for later use
    // We need to copy each field since game_import_t doesn't implement Copy
    {
        let mut guard = GAME_IMPORT.lock().unwrap();
        let imp = &*import;
        *guard = Some(game_import_t {
            bprintf: imp.bprintf,
            dprintf: imp.dprintf,
            cprintf: imp.cprintf,
            centerprintf: imp.centerprintf,
            sound: imp.sound,
            positioned_sound: imp.positioned_sound,
            configstring: imp.configstring,
            error: imp.error,
            modelindex: imp.modelindex,
            soundindex: imp.soundindex,
            imageindex: imp.imageindex,
            setmodel: imp.setmodel,
            trace: imp.trace,
            pointcontents: imp.pointcontents,
            inPVS: imp.inPVS,
            inPHS: imp.inPHS,
            SetAreaPortalState: imp.SetAreaPortalState,
            AreasConnected: imp.AreasConnected,
            linkentity: imp.linkentity,
            unlinkentity: imp.unlinkentity,
            BoxEdicts: imp.BoxEdicts,
            Pmove: imp.Pmove,
            multicast: imp.multicast,
            unicast: imp.unicast,
            WriteChar: imp.WriteChar,
            WriteByte: imp.WriteByte,
            WriteShort: imp.WriteShort,
            WriteLong: imp.WriteLong,
            WriteFloat: imp.WriteFloat,
            WriteString: imp.WriteString,
            WritePosition: imp.WritePosition,
            WriteDir: imp.WriteDir,
            WriteAngle: imp.WriteAngle,
            TagMalloc: imp.TagMalloc,
            TagFree: imp.TagFree,
            FreeTags: imp.FreeTags,
            cvar: imp.cvar,
            cvar_set: imp.cvar_set,
            cvar_forceset: imp.cvar_forceset,
            argc: imp.argc,
            argv: imp.argv,
            args: imp.args,
            AddCommandString: imp.AddCommandString,
            DebugGraph: imp.DebugGraph,
        });
    }

    // Set up the game import bridge so game code can call engine functions
    setup_game_import_bridge();

    // Initialize edicts array
    // Use a reasonable default; actual values come from maxclients/maxentities cvars
    let max_edicts = 1024;
    let edict_size = std::mem::size_of::<myq2_game::g_local::Edict>() as c_int;

    // Allocate edict storage
    {
        let mut storage_guard = EDICT_STORAGE.lock().unwrap();
        let total_size = (max_edicts * edict_size) as usize;
        let mut storage = vec![0u8; total_size];

        // Store the pointer in the export struct
        GAME_EXPORT.edicts = storage.as_mut_ptr() as *mut edict_t;
        GAME_EXPORT.edict_size = edict_size;
        GAME_EXPORT.num_edicts = 0;
        GAME_EXPORT.max_edicts = max_edicts;

        *storage_guard = Some(storage);
    }

    // Return pointer to our export struct
    // Use &raw mut to avoid undefined behavior with mutable static references
    &raw mut GAME_EXPORT
}

// ============================================================
// Game import bridge — adapts engine callbacks to Rust game code
// ============================================================

/// Set up the game import interface so game code can call engine functions
fn setup_game_import_bridge() {
    use myq2_game::game_import::{self, GameImport};

    struct DllGameImport;

    impl GameImport for DllGameImport {
        fn bprintf(&self, printlevel: i32, msg: &str) {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.bprintf {
                    let c_msg = CString::new(msg).unwrap_or_default();
                    unsafe { func(printlevel, c_msg.as_ptr()); }
                }
            }
        }

        fn dprintf(&self, msg: &str) {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.dprintf {
                    let c_msg = CString::new(msg).unwrap_or_default();
                    unsafe { func(c_msg.as_ptr()); }
                }
            }
        }

        fn cprintf(&self, ent_index: i32, printlevel: i32, msg: &str) {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.cprintf {
                    let c_msg = CString::new(msg).unwrap_or_default();
                    let ent_ptr = if ent_index > 0 {
                        unsafe { get_edict_ptr(ent_index) }
                    } else {
                        std::ptr::null_mut()
                    };
                    unsafe { func(ent_ptr, printlevel, c_msg.as_ptr()); }
                }
            }
        }

        fn centerprintf(&self, ent_index: i32, msg: &str) {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.centerprintf {
                    let c_msg = CString::new(msg).unwrap_or_default();
                    let ent_ptr = unsafe { get_edict_ptr(ent_index) };
                    unsafe { func(ent_ptr, c_msg.as_ptr()); }
                }
            }
        }

        fn sound(&self, ent_index: i32, channel: i32, soundindex: i32, volume: f32, attenuation: f32, timeofs: f32) {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.sound {
                    let ent_ptr = unsafe { get_edict_ptr(ent_index) };
                    unsafe { func(ent_ptr, channel, soundindex, volume, attenuation, timeofs); }
                }
            }
        }

        fn positioned_sound(&self, origin: &Vec3, ent_index: i32, channel: i32, soundindex: i32, volume: f32, attenuation: f32, timeofs: f32) {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.positioned_sound {
                    let ent_ptr = unsafe { get_edict_ptr(ent_index) };
                    unsafe { func(origin, ent_ptr, channel, soundindex, volume, attenuation, timeofs); }
                }
            }
        }

        fn configstring(&self, num: i32, string: &str) {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.configstring {
                    let c_str = CString::new(string).unwrap_or_default();
                    unsafe { func(num, c_str.as_ptr()); }
                }
            }
        }

        fn error(&self, msg: &str) {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.error {
                    let c_msg = CString::new(msg).unwrap_or_default();
                    unsafe { func(c_msg.as_ptr()); }
                }
            }
            // Note: The engine's error function should not return
        }

        fn modelindex(&self, name: &str) -> i32 {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.modelindex {
                    let c_name = CString::new(name).unwrap_or_default();
                    return unsafe { func(c_name.as_ptr()) };
                }
            }
            0
        }

        fn soundindex(&self, name: &str) -> i32 {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.soundindex {
                    let c_name = CString::new(name).unwrap_or_default();
                    return unsafe { func(c_name.as_ptr()) };
                }
            }
            0
        }

        fn imageindex(&self, name: &str) -> i32 {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.imageindex {
                    let c_name = CString::new(name).unwrap_or_default();
                    return unsafe { func(c_name.as_ptr()) };
                }
            }
            0
        }

        fn setmodel(&self, ent_index: i32, name: &str) {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.setmodel {
                    let ent_ptr = unsafe { get_edict_ptr(ent_index) };
                    let c_name = CString::new(name).unwrap_or_default();
                    unsafe { func(ent_ptr, c_name.as_ptr()); }
                }
            }
        }

        fn trace(&self, start: &Vec3, mins: &Vec3, maxs: &Vec3, end: &Vec3, passent_index: i32, contentmask: i32) -> Trace {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.trace {
                    let passent = if passent_index >= 0 {
                        unsafe { get_edict_ptr(passent_index) }
                    } else {
                        std::ptr::null_mut()
                    };
                    let c_trace = unsafe { func(start, mins, maxs, end, passent, contentmask) };
                    return Trace {
                        allsolid: c_trace.allsolid != 0,
                        startsolid: c_trace.startsolid != 0,
                        fraction: c_trace.fraction,
                        endpos: c_trace.endpos,
                        plane: c_trace.plane,
                        surface: None,
                        contents: c_trace.contents,
                        ent_index: 0,
                    };
                }
            }
            Trace::default()
        }

        fn pointcontents(&self, point: &Vec3) -> i32 {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.pointcontents {
                    return unsafe { func(point) };
                }
            }
            0
        }

        fn in_pvs(&self, p1: &Vec3, p2: &Vec3) -> bool {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.inPVS {
                    return unsafe { func(p1, p2) != 0 };
                }
            }
            false
        }

        fn in_phs(&self, p1: &Vec3, p2: &Vec3) -> bool {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.inPHS {
                    return unsafe { func(p1, p2) != 0 };
                }
            }
            false
        }

        fn set_area_portal_state(&self, portalnum: i32, open: bool) {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.SetAreaPortalState {
                    unsafe { func(portalnum, if open { 1 } else { 0 }); }
                }
            }
        }

        fn areas_connected(&self, area1: i32, area2: i32) -> bool {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.AreasConnected {
                    return unsafe { func(area1, area2) != 0 };
                }
            }
            false
        }

        fn linkentity(&self, ent_index: i32) {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.linkentity {
                    let ent_ptr = unsafe { get_edict_ptr(ent_index) };
                    unsafe { func(ent_ptr); }
                }
            }
        }

        fn unlinkentity(&self, ent_index: i32) {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.unlinkentity {
                    let ent_ptr = unsafe { get_edict_ptr(ent_index) };
                    unsafe { func(ent_ptr); }
                }
            }
        }

        fn box_edicts(&self, mins: &Vec3, maxs: &Vec3, maxcount: i32, areatype: i32) -> Vec<i32> {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.BoxEdicts {
                    // Allocate a list to receive edict pointers
                    let mut list: Vec<*mut edict_t> = vec![std::ptr::null_mut(); maxcount as usize];
                    let count = unsafe { func(mins, maxs, list.as_mut_ptr(), maxcount, areatype) };

                    // Convert pointers to indices
                    let mut result = Vec::with_capacity(count as usize);
                    for i in 0..count as usize {
                        if !list[i].is_null() {
                            unsafe {
                                let idx = game_api::num_for_edict(GAME_EXPORT.edicts, GAME_EXPORT.edict_size, list[i]);
                                result.push(idx);
                            }
                        }
                    }
                    return result;
                }
            }
            Vec::new()
        }

        fn write_char(&self, c: i32) {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.WriteChar {
                    unsafe { func(c); }
                }
            }
        }

        fn write_byte(&self, c: i32) {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.WriteByte {
                    unsafe { func(c); }
                }
            }
        }

        fn write_short(&self, c: i32) {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.WriteShort {
                    unsafe { func(c); }
                }
            }
        }

        fn write_long(&self, c: i32) {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.WriteLong {
                    unsafe { func(c); }
                }
            }
        }

        fn write_float(&self, f: f32) {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.WriteFloat {
                    unsafe { func(f); }
                }
            }
        }

        fn write_string(&self, s: &str) {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.WriteString {
                    let c_str = CString::new(s).unwrap_or_default();
                    unsafe { func(c_str.as_ptr()); }
                }
            }
        }

        fn write_position(&self, pos: &Vec3) {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.WritePosition {
                    unsafe { func(pos); }
                }
            }
        }

        fn write_dir(&self, dir: &Vec3) {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.WriteDir {
                    unsafe { func(dir); }
                }
            }
        }

        fn write_angle(&self, f: f32) {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.WriteAngle {
                    unsafe { func(f); }
                }
            }
        }

        fn multicast(&self, origin: &Vec3, to: i32) {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.multicast {
                    unsafe { func(origin, to); }
                }
            }
        }

        fn unicast(&self, ent_index: i32, reliable: bool) {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.unicast {
                    let ent_ptr = unsafe { get_edict_ptr(ent_index) };
                    unsafe { func(ent_ptr, if reliable { 1 } else { 0 }); }
                }
            }
        }

        fn cvar(&self, var_name: &str, value: &str, flags: i32) -> f32 {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.cvar {
                    let c_name = CString::new(var_name).unwrap_or_default();
                    let c_value = CString::new(value).unwrap_or_default();
                    let cvar_ptr = unsafe { func(c_name.as_ptr(), c_value.as_ptr(), flags) };
                    if !cvar_ptr.is_null() {
                        return unsafe { (*cvar_ptr).value };
                    }
                }
            }
            0.0
        }

        fn cvar_set(&self, var_name: &str, value: &str) {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.cvar_set {
                    let c_name = CString::new(var_name).unwrap_or_default();
                    let c_value = CString::new(value).unwrap_or_default();
                    unsafe { func(c_name.as_ptr(), c_value.as_ptr()); }
                }
            }
        }

        fn cvar_forceset(&self, var_name: &str, value: &str) {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.cvar_forceset {
                    let c_name = CString::new(var_name).unwrap_or_default();
                    let c_value = CString::new(value).unwrap_or_default();
                    unsafe { func(c_name.as_ptr(), c_value.as_ptr()); }
                }
            }
        }

        fn argc(&self) -> i32 {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.argc {
                    return unsafe { func() };
                }
            }
            0
        }

        fn argv(&self, n: i32) -> String {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.argv {
                    let ptr = unsafe { func(n) };
                    if !ptr.is_null() {
                        return unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() };
                    }
                }
            }
            String::new()
        }

        fn args(&self) -> String {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.args {
                    let ptr = unsafe { func() };
                    if !ptr.is_null() {
                        return unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() };
                    }
                }
            }
            String::new()
        }

        fn add_command_string(&self, text: &str) {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.AddCommandString {
                    let c_text = CString::new(text).unwrap_or_default();
                    unsafe { func(c_text.as_ptr()); }
                }
            }
        }

        fn debug_graph(&self, value: f32, color: i32) {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.DebugGraph {
                    unsafe { func(value, color); }
                }
            }
        }

        fn tag_malloc(&self, size: i32, tag: i32) -> Vec<u8> {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.TagMalloc {
                    let ptr = unsafe { func(size, tag) };
                    if !ptr.is_null() {
                        // Note: We allocate through the engine but return a Vec copy
                        // The engine will free its memory, and Rust will manage the Vec
                        let slice = unsafe { std::slice::from_raw_parts(ptr as *const u8, size as usize) };
                        return slice.to_vec();
                    }
                }
            }
            vec![0u8; size as usize]
        }

        fn tag_free(&self, _tag: i32) {
            // In the C API, TagFree takes a pointer, but the Rust trait takes a tag
            // This is a no-op since we're using Rust memory management
        }

        fn free_tags(&self, tag: i32) {
            let guard = GAME_IMPORT.lock().unwrap();
            if let Some(ref gi) = *guard {
                if let Some(func) = gi.FreeTags {
                    unsafe { func(tag); }
                }
            }
        }
    }

    game_import::set_gi(Box::new(DllGameImport));
}

/// Get a pointer to edict N using the stored edict array
unsafe fn get_edict_ptr(n: c_int) -> *mut edict_t {
    game_api::edict_num(GAME_EXPORT.edicts, GAME_EXPORT.edict_size, n)
}

// ============================================================
// Game export callbacks
// ============================================================

/// ge->Init()
unsafe extern "C" fn ge_init() {
    // Get maxclients from engine cvar
    let maxclients = {
        let guard = GAME_IMPORT.lock().unwrap();
        if let Some(ref gi) = *guard {
            if let Some(func) = gi.cvar {
                let name = CString::new("maxclients").unwrap();
                let value = CString::new("4").unwrap();
                let cvar = func(name.as_ptr(), value.as_ptr(), 0);
                if !cvar.is_null() {
                    (*cvar).value as i32
                } else {
                    4
                }
            } else {
                4
            }
        } else {
            4
        }
    };

    let maxentities = std::cmp::max(maxclients + 1, 1024);

    // Create game context
    let mut game_ctx = myq2_game::g_local::GameContext::default();
    game_ctx.edicts.resize_with(maxentities as usize, Default::default);
    game_ctx.game.maxclients = maxclients;
    game_ctx.game.maxentities = maxentities;
    game_ctx.max_edicts = maxentities;
    game_ctx.clients = vec![Default::default(); maxclients as usize];
    game_ctx.maxclients = maxclients as f32;

    // Store globally
    *GAME_CONTEXT.lock().unwrap() = Some(game_ctx);

    // Update export values
    GAME_EXPORT.num_edicts = maxclients + 1;
    GAME_EXPORT.max_edicts = maxentities;
}

/// ge->Shutdown()
unsafe extern "C" fn ge_shutdown() {
    *GAME_CONTEXT.lock().unwrap() = None;
}

/// ge->SpawnEntities(mapname, entstring, spawnpoint)
unsafe extern "C" fn ge_spawn_entities(
    mapname: *const c_char,
    entstring: *const c_char,
    spawnpoint: *const c_char,
) {
    let mapname_str = if mapname.is_null() { "" } else { CStr::from_ptr(mapname).to_str().unwrap_or("") };
    let entstring_str = if entstring.is_null() { "" } else { CStr::from_ptr(entstring).to_str().unwrap_or("") };
    let spawnpoint_str = if spawnpoint.is_null() { "" } else { CStr::from_ptr(spawnpoint).to_str().unwrap_or("") };

    let mut guard = GAME_CONTEXT.lock().unwrap();
    if let Some(ref mut ctx) = *guard {
        myq2_game::g_spawn::spawn_entities(ctx, mapname_str, entstring_str, spawnpoint_str);
        GAME_EXPORT.num_edicts = ctx.num_edicts;
    }
}

/// ge->WriteGame(filename, autosave)
unsafe extern "C" fn ge_write_game(filename: *const c_char, autosave: c_int) {
    let filename_str = if filename.is_null() { "" } else { CStr::from_ptr(filename).to_str().unwrap_or("") };

    let mut guard = GAME_CONTEXT.lock().unwrap();
    if let Some(ref mut ctx) = *guard {
        use myq2_game::g_save::SaveContext;
        let mut save_ctx = SaveContext {
            game: &mut ctx.game,
            level: &mut ctx.level,
            edicts: &mut ctx.edicts,
            clients: &mut ctx.clients,
            num_edicts: &mut ctx.num_edicts,
            items: &ctx.items,
        };
        myq2_game::g_save::write_game(&mut save_ctx, filename_str, autosave != 0);
    }
}

/// ge->ReadGame(filename)
unsafe extern "C" fn ge_read_game(filename: *const c_char) {
    let filename_str = if filename.is_null() { "" } else { CStr::from_ptr(filename).to_str().unwrap_or("") };

    let mut guard = GAME_CONTEXT.lock().unwrap();
    if let Some(ref mut ctx) = *guard {
        use myq2_game::g_save::SaveContext;
        let mut save_ctx = SaveContext {
            game: &mut ctx.game,
            level: &mut ctx.level,
            edicts: &mut ctx.edicts,
            clients: &mut ctx.clients,
            num_edicts: &mut ctx.num_edicts,
            items: &ctx.items,
        };
        myq2_game::g_save::read_game(&mut save_ctx, filename_str);
    }
}

/// ge->WriteLevel(filename)
unsafe extern "C" fn ge_write_level(filename: *const c_char) {
    let filename_str = if filename.is_null() { "" } else { CStr::from_ptr(filename).to_str().unwrap_or("") };

    let guard = GAME_CONTEXT.lock().unwrap();
    if let Some(ref ctx) = *guard {
        use myq2_game::g_save::SaveContext;
        let save_ctx = SaveContext {
            game: &mut ctx.game.clone(),
            level: &mut ctx.level.clone(),
            edicts: &mut ctx.edicts.clone(),
            clients: &mut ctx.clients.clone(),
            num_edicts: &mut ctx.num_edicts.clone(),
            items: &ctx.items,
        };
        myq2_game::g_save::write_level(&save_ctx, filename_str);
    }
}

/// ge->ReadLevel(filename)
unsafe extern "C" fn ge_read_level(filename: *const c_char) {
    let filename_str = if filename.is_null() { "" } else { CStr::from_ptr(filename).to_str().unwrap_or("") };

    let mut guard = GAME_CONTEXT.lock().unwrap();
    if let Some(ref mut ctx) = *guard {
        use myq2_game::g_save::SaveContext;
        let mut save_ctx = SaveContext {
            game: &mut ctx.game,
            level: &mut ctx.level,
            edicts: &mut ctx.edicts,
            clients: &mut ctx.clients,
            num_edicts: &mut ctx.num_edicts,
            items: &ctx.items,
        };
        myq2_game::g_save::read_level(&mut save_ctx, filename_str);
    }
}

/// ge->ClientConnect(ent, userinfo)
unsafe extern "C" fn ge_client_connect(_ent: *mut edict_t, _userinfo: *mut c_char) -> c_int {
    // For now, just accept all connections
    // Full implementation would delegate to game context
    1
}

/// ge->ClientBegin(ent)
unsafe extern "C" fn ge_client_begin(_ent: *mut edict_t) {
    // Placeholder - delegate to game context
}

/// ge->ClientUserinfoChanged(ent, userinfo)
unsafe extern "C" fn ge_client_userinfo_changed(_ent: *mut edict_t, _userinfo: *mut c_char) {
    // Placeholder - delegate to game context
}

/// ge->ClientDisconnect(ent)
unsafe extern "C" fn ge_client_disconnect(_ent: *mut edict_t) {
    // Placeholder - delegate to game context
}

/// ge->ClientCommand(ent)
unsafe extern "C" fn ge_client_command(_ent: *mut edict_t) {
    // Placeholder - delegate to game context
}

/// ge->ClientThink(ent, cmd)
unsafe extern "C" fn ge_client_think(_ent: *mut edict_t, _cmd: *mut usercmd_t) {
    // Placeholder - delegate to game context
}

/// ge->RunFrame()
unsafe extern "C" fn ge_run_frame() {
    let mut guard = GAME_CONTEXT.lock().unwrap();
    if let Some(ref mut ctx) = *guard {
        // Update num_edicts in export
        GAME_EXPORT.num_edicts = ctx.num_edicts;
    }
}

/// ge->ServerCommand()
unsafe extern "C" fn ge_server_command() {
    // Placeholder - delegate to game context
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    // ---- Game API version constant ----

    #[test]
    fn test_game_api_version() {
        // Q2 game API version must be 3
        assert_eq!(GAME_API_VERSION, 3, "GAME_API_VERSION must be 3 for Q2 compatibility");
    }

    // ---- GAME_EXPORT static initialization ----

    #[test]
    fn test_game_export_initial_apiversion() {
        // The static GAME_EXPORT should be initialized with the correct API version
        let version = unsafe { (&raw const GAME_EXPORT).as_ref().unwrap().apiversion };
        assert_eq!(version, GAME_API_VERSION,
            "GAME_EXPORT.apiversion should be {} at init, got {}", GAME_API_VERSION, version);
    }

    #[test]
    fn test_game_export_initial_function_pointers() {
        // All callback function pointers should be Some (not None) in the static initializer
        unsafe {
            let ge = (&raw const GAME_EXPORT).as_ref().unwrap();
            assert!(ge.Init.is_some(), "Init should be set");
            assert!(ge.Shutdown.is_some(), "Shutdown should be set");
            assert!(ge.SpawnEntities.is_some(), "SpawnEntities should be set");
            assert!(ge.WriteGame.is_some(), "WriteGame should be set");
            assert!(ge.ReadGame.is_some(), "ReadGame should be set");
            assert!(ge.WriteLevel.is_some(), "WriteLevel should be set");
            assert!(ge.ReadLevel.is_some(), "ReadLevel should be set");
            assert!(ge.ClientConnect.is_some(), "ClientConnect should be set");
            assert!(ge.ClientBegin.is_some(), "ClientBegin should be set");
            assert!(ge.ClientUserinfoChanged.is_some(), "ClientUserinfoChanged should be set");
            assert!(ge.ClientDisconnect.is_some(), "ClientDisconnect should be set");
            assert!(ge.ClientCommand.is_some(), "ClientCommand should be set");
            assert!(ge.ClientThink.is_some(), "ClientThink should be set");
            assert!(ge.RunFrame.is_some(), "RunFrame should be set");
            assert!(ge.ServerCommand.is_some(), "ServerCommand should be set");
        }
    }

    #[test]
    fn test_game_export_initial_edict_fields() {
        // Before GetGameApi is called, edict fields should be in their initial state
        unsafe {
            let ge = (&raw const GAME_EXPORT).as_ref().unwrap();
            // edicts pointer starts as null before GetGameApi sets it up
            // edict_size and num_edicts start at 0
            // Note: these may change if GetGameApi has been called in another test
            // so we just verify they have sensible values
            assert!(ge.edict_size >= 0, "edict_size should be non-negative");
            assert!(ge.max_edicts >= 0, "max_edicts should be non-negative");
        }
    }

    // ---- game_export_t struct layout ----

    #[test]
    fn test_game_export_t_size() {
        // game_export_t: 1 int + 15 function pointers + 1 ptr + 3 ints
        // On 64-bit: 4 + 15*8 + 8 + 3*4 = 4 + 120 + 8 + 12 = 144
        // But alignment padding may add bytes. The int fields may be padded.
        let sz = size_of::<game_export_t>();
        // Minimum: at least apiversion(4) + 15 ptrs (120 on 64-bit) + ptr(8) + 3*int(12)
        assert!(sz >= 100, "game_export_t should be at least 100 bytes, got {}", sz);
        // On 64-bit, with padding for the leading int to align the first pointer:
        // 8 (padded int) + 15*8 + 8 + 8 (padded int) + 4 + 4 = 152? Just verify nonzero.
        println!("game_export_t size: {} bytes", sz);
    }

    #[test]
    fn test_game_import_t_size() {
        // game_import_t: 44 function pointer fields (all Option<fn>)
        let sz = size_of::<game_import_t>();
        let fn_ptr_size = size_of::<Option<unsafe extern "C" fn()>>();
        assert_eq!(sz, 44 * fn_ptr_size,
            "game_import_t should be exactly 44 function pointers");
    }

    // ---- get_edict_ptr arithmetic ----

    #[test]
    fn test_edict_num_pointer_arithmetic() {
        // Verify the game_api::edict_num function used by get_edict_ptr
        let edict_size = size_of::<edict_t>() as c_int;
        let num = 8;
        let buf = vec![0u8; edict_size as usize * num];
        let base = buf.as_ptr() as *mut edict_t;

        for i in 0..num as c_int {
            let ptr = unsafe { game_api::edict_num(base, edict_size, i) };
            let offset = ptr as usize - base as usize;
            let expected = (edict_size * i) as usize;
            assert_eq!(offset, expected, "edict_num({}) offset mismatch", i);
        }
    }

    // ---- edict_t / entity_state_t FFI struct sizes ----

    #[test]
    fn test_edict_t_ffi_struct_size() {
        let sz = size_of::<edict_t>();
        // edict_t is a large repr(C) struct with entity_state_t, pointers,
        // arrays, and many integer fields
        assert!(sz > 100, "edict_t should be > 100 bytes, got {}", sz);
        println!("edict_t size: {} bytes", sz);
    }

    #[test]
    fn test_entity_state_t_ffi_struct_size() {
        use myq2_common::game_api::entity_state_t;
        let sz = size_of::<entity_state_t>();
        // entity_state_t: 3 Vec3s (36 bytes) + 12 int/u32 fields (48 bytes) = 84 bytes
        assert_eq!(sz, 84, "entity_state_t should be 84 bytes, got {}", sz);
    }

    #[test]
    fn test_usercmd_t_ffi_struct_size() {
        use myq2_common::game_api::usercmd_t;
        let sz = size_of::<usercmd_t>();
        // usercmd_t: u8 + u8 + [i16;3](6) + i16 + i16 + i16 + u8 + u8 = 16 bytes
        assert_eq!(sz, 16, "usercmd_t should be 16 bytes, got {}", sz);
    }

    #[test]
    fn test_pmove_state_t_ffi_struct_size() {
        use myq2_common::game_api::pmove_state_t;
        let sz = size_of::<pmove_state_t>();
        // pmove_state_t: int(4) + [i16;3](6) + [i16;3](6) + u8 + u8 + i16(2) + [i16;3](6)
        // = 4 + 6 + 6 + 1 + 1 + 2 + 6 = 26 -> padded to 28
        println!("pmove_state_t size: {} bytes", sz);
        // Must be at least 26 bytes (sum of fields without padding)
        assert!(sz >= 26, "pmove_state_t should be at least 26 bytes, got {}", sz);
    }

    // ---- GetGameApi null safety ----

    #[test]
    fn test_get_game_api_null_import_returns_null() {
        let result = unsafe { GetGameApi(std::ptr::null_mut()) };
        assert!(result.is_null(), "GetGameApi with null import should return null");
    }
}
