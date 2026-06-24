// sv_init.rs â€” Server initialization
// Converted from: myq2-original/server/sv_init.c
//
// Copyright (C) 1997-2001 Id Software, Inc.
// Licensed under the GNU General Public License v2.

#![allow(dead_code)]

use crate::server::{ServerContext, ServerState, ClientState, Server};

use myq2_common::q_shared::{
    self, CS_AIRACCEL, CS_MAPCHECKSUM, CS_MODELS, CS_NAME, CS_SOUNDS, CS_IMAGES,
    CVAR_LATCH, CVAR_NOSET, CVAR_SERVERINFO, EntityState, MAX_CLIENTS,
    MAX_IMAGES, MAX_MODELS, MAX_SOUNDS, Multicast, UserCmd,
    vec3_origin,
};
use myq2_common::qcommon::{
    MAX_MSGLEN, NetAdr, PORT_MASTER,
    SizeBuf, SvcOps, UPDATE_BACKUP,
};
use myq2_common::common::{com_printf, com_dprintf, com_error, msg_write_char, msg_write_short, msg_write_string};
use myq2_common::cvar::{cvar_set, cvar_full_set, cvar_variable_value, cvar_get_latched_vars};
use myq2_common::files::fs_gamedir;


use std::path::Path;
use rand::Rng;

// Cross-module client callbacks â€” registered at startup by main.rs.
// These cannot be wired directly because they live in the myq2-client crate.
use std::sync::Mutex;

// ============================================================
// Chunked Map Loading System
// ============================================================

/// Progress tracking for chunked/interruptible map loading.
/// Breaks sv_spawn_server into phases that execute across multiple frames
/// to prevent Windows "Not Responding" white screen during BSP loading.
#[derive(Debug, Clone)]
pub enum MapLoadPhase {
    /// Not currently loading a map
    Idle,
    /// Phase 1: Initialize server state (fast, single frame)
    Init {
        server: String,
        spawnpoint: String,
        serverstate: ServerState,
        attractloop: bool,
        loadgame: bool,
    },
    /// Phase 2: Load BSP collision model (slow, blocking I/O)
    LoadingBSP {
        server: String,
        spawnpoint: String,
        serverstate: ServerState,
        attractloop: bool,
        loadgame: bool,
    },
    /// Phase 3: Setup physics world and inline models (fast, single frame)
    SetupPhysics {
        server: String,
        spawnpoint: String,
        serverstate: ServerState,
        attractloop: bool,
        loadgame: bool,
        checksum: u32,
    },
    /// Phase 4: Spawn entities via game DLL (potentially slow)
    SpawningEntities {
        server: String,
        spawnpoint: String,
        serverstate: ServerState,
        attractloop: bool,
        loadgame: bool,
    },
    /// Phase 5: Run two game frames to settle physics (potentially slow)
    RunningFrames {
        server: String,
        spawnpoint: String,
        serverstate: ServerState,
        frames_run: u32,
    },
    /// Phase 6: Finalization - create baseline, check savegame (fast)
    Finalizing {
        serverstate: ServerState,
    },
    /// Map loading complete
    Complete,
}

/// Client-side callbacks that the server needs to invoke.
/// Registered by main.rs after both client and server are initialized.
pub struct SvClientCallbacks {
    pub cl_drop: fn(),
    pub scr_begin_loading_plaque: fn(),
}

static SV_CLIENT_CALLBACKS: Mutex<Option<SvClientCallbacks>> = Mutex::new(None);

/// Register client callbacks so the server can call into the client module.
pub fn sv_register_client_callbacks(cb: SvClientCallbacks) {
    *SV_CLIENT_CALLBACKS.lock().unwrap_or_else(|e| e.into_inner()) = Some(cb);
}

// ============================================================
// Global ServerContext â€” replaces C global server state
// ============================================================

static GLOBAL_SERVER_CTX: Mutex<Option<ServerContext>> = Mutex::new(None);

/// Initialize the global server context. Called once at startup.
pub fn sv_init_global_context() {
    let mut guard = GLOBAL_SERVER_CTX.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(ServerContext::default());
    }
}

/// Access the global ServerContext with a closure.
pub fn with_server_context<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut ServerContext) -> R,
{
    GLOBAL_SERVER_CTX.lock().unwrap_or_else(|e| e.into_inner()).as_mut().map(f)
}

/// SV_Frame wrapper for the global context â€” suitable as a frame callback.
pub fn sv_frame_global(msec: i32) {
    if let Some(ref mut ctx) = *GLOBAL_SERVER_CTX.lock().unwrap_or_else(|e| e.into_inner()) {
        crate::sv_main::sv_frame(ctx, msec);
    }
}

fn cl_drop() {
    if let Some(ref cb) = *SV_CLIENT_CALLBACKS.lock().unwrap_or_else(|e| e.into_inner()) {
        (cb.cl_drop)();
    }
}
fn scr_begin_loading_plaque() {
    if let Some(ref cb) = *SV_CLIENT_CALLBACKS.lock().unwrap_or_else(|e| e.into_inner()) {
        (cb.scr_begin_loading_plaque)();
    }
}

// Network functions â€” wired to myq2_common::net implementations.
fn net_config(multiplayer: bool) {
    myq2_common::net::net_config(multiplayer);
}
fn net_string_to_adr(s: &str, adr: &mut NetAdr) {
    if let Some(resolved) = myq2_common::net::net_string_to_adr(s) {
        *adr = resolved;
    }
}

/// SV_ClearWorld â€” clears the area node tree and rebuilds it from the world model bounds.
fn sv_clear_world() {
    // In original C: uses sv.models[1]->mins/maxs (the world BSP model).
    // sv.models[1] is the cmodel_t* returned by CM_LoadMap = map_cmodels[0] (worldmodel).
    // Note: inline_model("*N") requires N >= 1, so we access map_cmodels[0] directly.
    let bounds = myq2_common::cmodel::with_cmodel_ctx(|ctx| {
        if ctx.numcmodels > 0 {
            let model = &ctx.map_cmodels[0]; // worldmodel
            (model.mins, model.maxs)
        } else {
            ([-4096.0; 3], [4096.0; 3])
        }
    });
    let (mins, maxs) = bounds.unwrap_or(([-4096.0; 3], [4096.0; 3]));

    crate::sv_world::with_sv_world_ctx(|ctx| {
        ctx.clear_world(&mins, &maxs);
    });
}

fn cm_load_map(name: &str, clientload: bool) -> (i32, u32) {
    let result = myq2_common::cmodel::with_cmodel_ctx(|ctx| {
        let (_num_models, checksum) = ctx.load_map(name, clientload, None);
        let model_index = if ctx.numcmodels > 0 { 1i32 } else { 0i32 };
        (model_index, checksum)
    });
    result.unwrap_or((0, 0))
}
fn cm_entity_string() -> String { myq2_common::cmodel::cm_entity_string() }

/// Placeholder pm_airaccelerate global
pub static PM_AIRACCELERATE: std::sync::Mutex<f32> = std::sync::Mutex::new(0.0);

// Placeholder cvar references
fn maxclients_value() -> f32 { 1.0 }
fn sv_noreload_value() -> f32 { 0.0 }
fn sv_airaccelerate_value() -> f32 { 0.0 }
fn dedicated_value() -> f32 { 0.0 }

// ============================================================
// SV_FindIndex
// ============================================================

/// Search for `name` in configstrings starting at `start`, up to `max` entries.
/// If `create` is true and the name is not found, add it.
/// Returns the index (relative to `start`), or 0 if not found / empty name.
pub fn sv_find_index(
    ctx: &mut ServerContext,
    name: &str,
    start: usize,
    max: usize,
    create: bool,
) -> i32 {
    if name.is_empty() {
        return 0;
    }

    let mut i = 1;
    while i < max && !ctx.sv.configstrings[start + i].is_empty() {
        if ctx.sv.configstrings[start + i] == name {
            return i as i32;
        }
        i += 1;
    }

    if !create {
        return 0;
    }

    if i == max {
        com_error(q_shared::ERR_DROP, "*Index: overflow");
    }

    ctx.sv.configstrings[start + i] = name.to_string();

    if ctx.sv.state != ServerState::Loading {
        // send the update to everyone
        ctx.sv.multicast.clear();
        msg_write_char(&mut ctx.sv.multicast, SvcOps::ConfigString as i32);
        msg_write_short(&mut ctx.sv.multicast, (start + i) as i32);
        msg_write_string(&mut ctx.sv.multicast, name);
        crate::sv_send::sv_multicast(ctx, Some(vec3_origin), Multicast::AllR);
    }

    i as i32
}

// ============================================================
// SV_ModelIndex
// ============================================================

pub fn sv_model_index(ctx: &mut ServerContext, name: &str) -> i32 {
    sv_find_index(ctx, name, CS_MODELS, MAX_MODELS, true)
}

// ============================================================
// SV_SoundIndex
// ============================================================

pub fn sv_sound_index(ctx: &mut ServerContext, name: &str) -> i32 {
    sv_find_index(ctx, name, CS_SOUNDS, MAX_SOUNDS, true)
}

// ============================================================
// SV_ImageIndex
// ============================================================

pub fn sv_image_index(ctx: &mut ServerContext, name: &str) -> i32 {
    sv_find_index(ctx, name, CS_IMAGES, MAX_IMAGES, true)
}

// ============================================================
// SV_CreateBaseline
// ============================================================

pub fn sv_create_baseline(ctx: &mut ServerContext) {
    let num_edicts = ctx.ge.as_ref().map_or(0, |ge| ge.num_edicts);
    let mut baseline_count = 0;
    for entnum in 1..num_edicts as usize {
        if let Some(ref mut ge) = ctx.ge {
            if let Some(ent) = ge.edicts.get_mut(entnum) {
                if !ent.inuse {
                    continue;
                }
                if ent.s.modelindex == 0 && ent.s.sound == 0 && ent.s.effects == 0 {
                    continue;
                }
                ent.s.number = entnum as i32;
                ent.s.old_origin = ent.s.origin;

                if entnum < ctx.sv.baselines.len() {
                    ctx.sv.baselines[entnum] = ent.s.clone();
                    baseline_count += 1;
                }
            }
        }
    }
}

// ============================================================
// SV_CheckForSavegame
// ============================================================

pub fn sv_check_for_savegame(ctx: &mut ServerContext) {
    if sv_noreload_value() != 0.0 {
        return;
    }

    if cvar_variable_value("deathmatch") != 0.0 {
        return;
    }

    let name = format!("{}/save/current/{}.sav", fs_gamedir(), ctx.sv.name);
    if !Path::new(&name).exists() {
        return; // no savegame
    }

    sv_clear_world();

    // get configstrings and areaportals
    crate::sv_ccmds::sv_read_level_file(ctx);

    if !ctx.sv.loadgame {
        // coming back to a level after being in a different
        // level, so run it for ten seconds

        // rlava2 was sending too many lightstyles, and overflowing the
        // reliable data. temporarily changing the server state to loading
        // prevents these from being passed down.
        let previous_state = ctx.sv.state;          // PGM
        ctx.sv.state = ServerState::Loading;        // PGM
        for _i in 0..100 {
            if let Some(ref ge) = ctx.ge {
                if let Some(run_fn) = ge.run_frame {
                    run_fn();
                }
            }
        }
        ctx.sv.state = previous_state;              // PGM
    }
}

// ============================================================
// SV_SpawnServer
// ============================================================

pub fn sv_spawn_server(
    ctx: &mut ServerContext,
    server: &str,
    _spawnpoint: &str,
    serverstate: ServerState,
    attractloop: bool,
    loadgame: bool,
) {
    if attractloop {
        cvar_set("paused", "0");
    }

    com_printf("------- Server Initialization -------\n");
    com_dprintf(&format!("SpawnServer: {}\n", server));

    if ctx.sv.demofile.is_some() {
        ctx.sv.demofile = None; // drop/close the file
    }

    ctx.svs.spawncount += 1; // any partially connected client will be restarted
    ctx.sv.state = ServerState::Dead;
    crate::sv_main::com_set_server_state(ctx.sv.state);

    // wipe the entire per-level structure
    ctx.sv = Server::default();
    ctx.svs.realtime = 0;
    ctx.sv.loadgame = loadgame;
    ctx.sv.attractloop = attractloop;

    // save name for levels that don't set message
    ctx.sv.configstrings[CS_NAME] = server.to_string();
    if cvar_variable_value("deathmatch") != 0.0 {
        ctx.sv.configstrings[CS_AIRACCEL] = format!("{}", sv_airaccelerate_value());
        *PM_AIRACCELERATE.lock().unwrap_or_else(|e| e.into_inner()) = sv_airaccelerate_value();
    } else {
        ctx.sv.configstrings[CS_AIRACCEL] = "0".to_string();
        *PM_AIRACCELERATE.lock().unwrap_or_else(|e| e.into_inner()) = 0.0;
    }

    ctx.sv.multicast = SizeBuf::new(MAX_MSGLEN as i32);

    ctx.sv.name = server.to_string();

    // leave slots at start for clients only
    let max_cl = maxclients_value() as usize;
    for i in 0..max_cl {
        if i < ctx.svs.clients.len() {
            // needs to reconnect
            if ctx.svs.clients[i].state as i32 > ClientState::Connected as i32 {
                ctx.svs.clients[i].state = ClientState::Connected;
            }
            ctx.svs.clients[i].lastframe = -1;
        }
    }

    ctx.sv.time = 1000;

    ctx.sv.name = server.to_string();
    ctx.sv.configstrings[CS_NAME] = server.to_string();

    let checksum: u32;
    if serverstate != ServerState::Game {
        let (model, chk) = cm_load_map("", false); // no real map
        ctx.sv.models[1] = model;
        checksum = chk;
    } else {
        ctx.sv.configstrings[CS_MODELS + 1] = format!("maps/{}.bsp", server);
        let map_name = ctx.sv.configstrings[CS_MODELS + 1].clone();
        let (model, chk) = cm_load_map(&map_name, false);
        ctx.sv.models[1] = model;
        checksum = chk;
    }
    ctx.sv.configstrings[CS_MAPCHECKSUM] = format!("{}", checksum);

    //
    // clear physics interaction links
    //
    sv_clear_world();

    let num_inline = myq2_common::cmodel::cm_num_inline_models() as i32;
    for i in 1..num_inline as usize {
        ctx.sv.configstrings[CS_MODELS + 1 + i] = format!("*{}", i);
        let model_name = ctx.sv.configstrings[CS_MODELS + 1 + i].clone();
        ctx.sv.models[i + 1] = myq2_common::cmodel::cm_inline_model(&model_name).headnode;
    }

    //
    // spawn the rest of the entities on the map
    //

    // precache and static commands can be issued during
    // map initialization
    ctx.sv.state = ServerState::Loading;
    crate::sv_main::com_set_server_state(ctx.sv.state);

    // load and spawn all other entities
    let entity_string = cm_entity_string();
    let sv_name = ctx.sv.name.clone();
    let sp = _spawnpoint.to_string();
    if let Some(ref ge) = ctx.ge {
        if let Some(spawn_fn) = ge.spawn_entities {
            spawn_fn(&sv_name, &entity_string, &sp);
        }
    }
    // Sync edicts from game context to server after spawning.
    // Engine-set fields (modelindex, num_clusters, etc.) are written back to
    // GAME_CONTEXT immediately by setmodel/linkentity via GAME_CTX_PTR.
    if let Some(ref mut ge) = ctx.ge {
        crate::sv_game::sync_edicts_to_server(ge);
    }

    // run two frames to allow everything to settle
    if let Some(ref ge) = ctx.ge {
        if let Some(run_fn) = ge.run_frame {
            run_fn();
            run_fn();
        }
    }
    // Sync edicts after run frames
    if let Some(ref mut ge) = ctx.ge {
        crate::sv_game::sync_edicts_to_server(ge);
    }

    // all precaches are complete
    ctx.sv.state = serverstate;
    crate::sv_main::com_set_server_state(ctx.sv.state);

    // create a baseline for more efficient communications
    sv_create_baseline(ctx);

    // check for a savegame
    sv_check_for_savegame(ctx);

    // set serverinfo variable
    cvar_full_set("mapname", &ctx.sv.name, CVAR_SERVERINFO | CVAR_NOSET);

    com_printf("-------------------------------------\n");
}

// ============================================================
// sv_spawn_server_tick â€” Process one chunk of map loading
// ============================================================

/// Process one chunk of the map loading state machine.
/// Returns true if loading is complete, false if more chunks remain.
///
/// This function should be called each frame from the main loop until
/// it returns true. It breaks the monolithic sv_spawn_server() into
/// interruptible chunks to prevent Windows "Not Responding" freeze.
pub fn sv_spawn_server_tick(ctx: &mut ServerContext) -> bool {
    let current_phase = ctx.map_load_progress.clone();

    match current_phase {
        MapLoadPhase::Idle | MapLoadPhase::Complete => {
            return true; // Nothing to do
        }

        MapLoadPhase::Init { server, spawnpoint, serverstate, attractloop, loadgame } => {
            com_printf("------- Server Initialization -------\n");
            com_dprintf(&format!("SpawnServer: {}\n", server));

            if attractloop {
                cvar_set("paused", "0");
            }

            if ctx.sv.demofile.is_some() {
                ctx.sv.demofile = None; // drop/close the file
            }

            ctx.svs.spawncount += 1; // any partially connected client will be restarted
            ctx.sv.state = ServerState::Dead;
            crate::sv_main::com_set_server_state(ctx.sv.state);

            // wipe the entire per-level structure
            ctx.sv = Server::default();
            ctx.svs.realtime = 0;
            ctx.sv.loadgame = loadgame;
            ctx.sv.attractloop = attractloop;

            // save name for levels that don't set message
            ctx.sv.configstrings[CS_NAME] = server.clone();
            if cvar_variable_value("deathmatch") != 0.0 {
                ctx.sv.configstrings[CS_AIRACCEL] = format!("{}", sv_airaccelerate_value());
                *PM_AIRACCELERATE.lock().unwrap_or_else(|e| e.into_inner()) = sv_airaccelerate_value();
            } else {
                ctx.sv.configstrings[CS_AIRACCEL] = "0".to_string();
                *PM_AIRACCELERATE.lock().unwrap_or_else(|e| e.into_inner()) = 0.0;
            }

            ctx.sv.multicast = SizeBuf::new(MAX_MSGLEN as i32);
            ctx.sv.name = server.clone();

            // leave slots at start for clients only
            let max_cl = maxclients_value() as usize;
            for i in 0..max_cl {
                if i < ctx.svs.clients.len() {
                    // needs to reconnect
                    if ctx.svs.clients[i].state as i32 > ClientState::Connected as i32 {
                        ctx.svs.clients[i].state = ClientState::Connected;
                    }
                    ctx.svs.clients[i].lastframe = -1;
                }
            }

            ctx.sv.time = 1000;
            ctx.sv.name = server.clone();
            ctx.sv.configstrings[CS_NAME] = server.clone();

            // Advance to next phase
            ctx.map_load_progress = MapLoadPhase::LoadingBSP {
                server,
                spawnpoint,
                serverstate,
                attractloop,
                loadgame,
            };
            com_printf("[Chunk 1/6] Initialization complete\n");
            return false; // More work to do
        }

        MapLoadPhase::LoadingBSP { server, spawnpoint, serverstate, attractloop, loadgame } => {
            com_printf("[Chunk 2/6] Loading BSP collision model...\n");

            let checksum: u32;
            if serverstate != ServerState::Game {
                let (model, chk) = cm_load_map("", false); // no real map
                ctx.sv.models[1] = model;
                checksum = chk;
            } else {
                ctx.sv.configstrings[CS_MODELS + 1] = format!("maps/{}.bsp", server);
                let map_name = ctx.sv.configstrings[CS_MODELS + 1].clone();
                let (model, chk) = cm_load_map(&map_name, false);
                if model == 0 && chk == 0 {
                    // BSP load failed - abort map loading
                    com_printf(&format!("ERROR: Failed to load maps/{}.bsp\n", server));
                    ctx.map_load_progress = MapLoadPhase::Idle;
                    return true; // Loading aborted
                }
                ctx.sv.models[1] = model;
                checksum = chk;
            }
            ctx.sv.configstrings[CS_MAPCHECKSUM] = format!("{}", checksum);

            // Advance to next phase
            ctx.map_load_progress = MapLoadPhase::SetupPhysics {
                server,
                spawnpoint,
                serverstate,
                attractloop,
                loadgame,
                checksum,
            };
            com_printf("[Chunk 2/6] BSP loaded\n");
            return false; // More work to do
        }

        MapLoadPhase::SetupPhysics { server, spawnpoint, serverstate, attractloop, loadgame, checksum: _ } => {
            com_printf("[Chunk 3/6] Setting up physics world...\n");

            // clear physics interaction links
            sv_clear_world();

            let num_inline = myq2_common::cmodel::cm_num_inline_models() as i32;
            for i in 1..num_inline as usize {
                ctx.sv.configstrings[CS_MODELS + 1 + i] = format!("*{}", i);
                let model_name = ctx.sv.configstrings[CS_MODELS + 1 + i].clone();
                ctx.sv.models[i + 1] = myq2_common::cmodel::cm_inline_model(&model_name).headnode;
            }

            // Advance to next phase
            ctx.map_load_progress = MapLoadPhase::SpawningEntities {
                server,
                spawnpoint,
                serverstate,
                attractloop,
                loadgame,
            };
            com_printf("[Chunk 3/6] Physics setup complete\n");
            return false; // More work to do
        }

        MapLoadPhase::SpawningEntities { server, spawnpoint, serverstate, attractloop: _, loadgame: _ } => {
            com_printf("[Chunk 4/6] Spawning entities...\n");

            // precache and static commands can be issued during map initialization
            ctx.sv.state = ServerState::Loading;
            crate::sv_main::com_set_server_state(ctx.sv.state);

            // load and spawn all other entities
            let entity_string = cm_entity_string();
            let sv_name = ctx.sv.name.clone();
            if let Some(ref ge) = ctx.ge {
                if let Some(spawn_fn) = ge.spawn_entities {
                    spawn_fn(&sv_name, &entity_string, &spawnpoint);
                }
            }
            // Sync edicts from game context to server after spawning
            if let Some(ref mut ge) = ctx.ge {
                crate::sv_game::sync_edicts_to_server(ge);
            }

            // Advance to next phase
            ctx.map_load_progress = MapLoadPhase::RunningFrames {
                server,
                spawnpoint,
                serverstate,
                frames_run: 0,
            };
            com_printf("[Chunk 4/6] Entities spawned\n");
            return false; // More work to do
        }

        MapLoadPhase::RunningFrames { server: _, spawnpoint: _, serverstate, frames_run } => {
            com_printf(&format!("[Chunk 5/6] Running settle frame {}/2...\n", frames_run + 1));

            // run one frame to allow things to settle
            if let Some(ref ge) = ctx.ge {
                if let Some(run_fn) = ge.run_frame {
                    run_fn();
                }
            }
            // Sync edicts after run frame
            if let Some(ref mut ge) = ctx.ge {
                crate::sv_game::sync_edicts_to_server(ge);
            }

            if frames_run + 1 >= 2 {
                // Both frames complete, advance to finalization
                ctx.map_load_progress = MapLoadPhase::Finalizing { serverstate };
                com_printf("[Chunk 5/6] Settle frames complete\n");
            } else {
                // Need to run another frame
                ctx.map_load_progress = MapLoadPhase::RunningFrames {
                    server: String::new(),
                    spawnpoint: String::new(),
                    serverstate,
                    frames_run: frames_run + 1,
                };
            }
            return false; // More work to do
        }

        MapLoadPhase::Finalizing { serverstate } => {
            com_printf("[Chunk 6/6] Finalizing...\n");

            // all precaches are complete
            ctx.sv.state = serverstate;
            crate::sv_main::com_set_server_state(ctx.sv.state);

            // create a baseline for more efficient communications
            sv_create_baseline(ctx);

            // check for a savegame
            sv_check_for_savegame(ctx);

            // set serverinfo variable
            cvar_full_set("mapname", &ctx.sv.name, CVAR_SERVERINFO | CVAR_NOSET);

            com_printf("-------------------------------------\n");

            // Mark as complete
            ctx.map_load_progress = MapLoadPhase::Complete;
            return true; // Loading complete!
        }
    }
}

/// Start chunked map loading. Call sv_spawn_server_tick() each frame until complete.
pub fn sv_spawn_server_chunked(
    ctx: &mut ServerContext,
    server: &str,
    spawnpoint: &str,
    serverstate: ServerState,
    attractloop: bool,
    loadgame: bool,
) {
    ctx.map_load_progress = MapLoadPhase::Init {
        server: server.to_string(),
        spawnpoint: spawnpoint.to_string(),
        serverstate,
        attractloop,
        loadgame,
    };
}

// ============================================================
// SV_InitGame
// ============================================================

pub fn sv_init_game(ctx: &mut ServerContext) {
    if ctx.svs.initialized {
        // cause any connected clients to reconnect
        crate::sv_main::sv_shutdown(ctx, "Server restarted\n", true);
    } else {
        // make sure the client is down
        cl_drop();
        scr_begin_loading_plaque();
    }

    // get any latched variable changes (maxclients, etc)
    cvar_get_latched_vars();

    ctx.svs.initialized = true;

    if cvar_variable_value("coop") != 0.0 && cvar_variable_value("deathmatch") != 0.0 {
        com_printf("Deathmatch and Coop both set, disabling Coop\n");
        cvar_full_set("coop", "0", CVAR_SERVERINFO | CVAR_LATCH);
    }

    // dedicated servers can't be single player and are usually DM
    // so unless they explicitly set coop, force it to deathmatch
    if dedicated_value() != 0.0
        && cvar_variable_value("coop") == 0.0 {
            cvar_full_set("deathmatch", "1", CVAR_SERVERINFO | CVAR_LATCH);
        }

    // init clients
    if cvar_variable_value("deathmatch") != 0.0 {
        let mc = maxclients_value();
        if mc <= 1.0 {
            cvar_full_set("maxclients", "8", CVAR_SERVERINFO | CVAR_LATCH);
        } else if mc > MAX_CLIENTS as f32 {
            cvar_full_set(
                "maxclients",
                &format!("{}", MAX_CLIENTS),
                CVAR_SERVERINFO | CVAR_LATCH,
            );
        }
    } else if cvar_variable_value("coop") != 0.0 {
        let mc = maxclients_value();
        if mc <= 1.0 || mc > 4.0 {
            cvar_full_set("maxclients", "4", CVAR_SERVERINFO | CVAR_LATCH);
        }
    } else {
        // non-deathmatch, non-coop is one player
        cvar_full_set("maxclients", "1", CVAR_SERVERINFO | CVAR_LATCH);
    }

    ctx.svs.spawncount = rand::thread_rng().gen::<i32>().abs();

    let max_cl = maxclients_value() as usize;
    ctx.svs.clients.clear();
    for _ in 0..max_cl {
        ctx.svs.clients.push(crate::server::Client::default());
    }

    ctx.svs.num_client_entities = (max_cl as i32) * UPDATE_BACKUP * crate::server::MAX_PACKET_ENTITIES as i32;
    ctx.svs.client_entities.clear();
    ctx.svs.client_entities
        .resize_with(ctx.svs.num_client_entities as usize, EntityState::default);

    // init network stuff
    net_config(max_cl > 1);

    // heartbeats will always be sent to the id master
    ctx.svs.last_heartbeat = -99999; // send immediately
    let idmaster = format!("192.246.40.37:{}", PORT_MASTER);
    net_string_to_adr(&idmaster, &mut ctx.master_adr[0]);

    // init game
    crate::sv_game::sv_init_game_progs(ctx);

    for i in 0..max_cl {
        // In the C code:
        //   ent = EDICT_NUM(i+1);
        //   ent->s.number = i+1;
        //   svs.clients[i].edict = ent;
        //   memset(&svs.clients[i].lastcmd, 0, sizeof(svs.clients[i].lastcmd));
        ctx.svs.clients[i].edict_index = (i + 1) as i32;
        ctx.svs.clients[i].lastcmd = UserCmd::default();
    }
}

// ============================================================
// SV_Map
// ============================================================

pub fn sv_map(ctx: &mut ServerContext, attractloop: bool, levelstring: &str, loadgame: bool) {
    ctx.sv.loadgame = loadgame;
    ctx.sv.attractloop = attractloop;

    if ctx.sv.state == ServerState::Dead && !ctx.sv.loadgame {
        sv_init_game(ctx); // the game is just starting
    }

    let mut level = levelstring.to_string();

    // if there is a + in the map, set nextserver to the remainder
    if let Some(pos) = level.find('+') {
        let remainder = level[pos + 1..].to_string();
        level.truncate(pos);
        cvar_set("nextserver", &format!("gamemap \"{}\"", remainder));
    } else {
        cvar_set("nextserver", "");
    }

    // ZOID special hack for end game screen in coop mode
    if cvar_variable_value("coop") != 0.0
        && level.eq_ignore_ascii_case("victory.pcx")
    {
        cvar_set("nextserver", "gamemap \"*base1\"");
    }

    // if there is a $, use the remainder as a spawnpoint
    let spawnpoint;
    if let Some(pos) = level.find('$') {
        spawnpoint = level[pos + 1..].to_string();
        level.truncate(pos);
    } else {
        spawnpoint = String::new();
    }

    // skip the end-of-unit flag if necessary
    if level.starts_with('*') {
        level = level[1..].to_string();
    }

    let l = level.len();
    if l > 4 && level.ends_with(".cin") {
        scr_begin_loading_plaque();
        crate::sv_send::sv_broadcast_command(ctx, "changing\n");
        sv_spawn_server_chunked(ctx, &level, &spawnpoint, ServerState::Cinematic, attractloop, loadgame);
    } else if l > 4 && level.ends_with(".dm2") {
        scr_begin_loading_plaque();
        crate::sv_send::sv_broadcast_command(ctx, "changing\n");
        sv_spawn_server_chunked(ctx, &level, &spawnpoint, ServerState::Demo, attractloop, loadgame);
    } else if l > 4 && level.ends_with(".pcx") {
        scr_begin_loading_plaque();
        crate::sv_send::sv_broadcast_command(ctx, "changing\n");
        sv_spawn_server_chunked(ctx, &level, &spawnpoint, ServerState::Pic, attractloop, loadgame);
    } else {
        scr_begin_loading_plaque();
        crate::sv_send::sv_broadcast_command(ctx, "changing\n");
        crate::sv_send::sv_send_client_messages(ctx);
        sv_spawn_server_chunked(ctx, &level, &spawnpoint, ServerState::Game, attractloop, loadgame);
        // NOTE: cbuf_copy_to_defer() is called by the CALLER (cl_map_f) instead of here,
        // because we're already inside CMD_CTX lock (via cbuf_execute â†’ cmd_execute_string).
        // Calling the global cbuf_copy_to_defer() would deadlock on the non-reentrant mutex.
    }

    crate::sv_send::sv_broadcast_command(ctx, "reconnect\n");
}
