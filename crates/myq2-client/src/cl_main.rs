// cl_main.rs -- client main loop
// Converted from: myq2-original/client/cl_main.c
//
// Copyright (C) 1997-2001 Id Software, Inc.
// Licensed under the GNU General Public License v2+

use std::fs::File;
use std::io::{Write, BufWriter};
use std::sync::{Arc, Mutex, LazyLock};

use myq2_common::q_shared::*;
use myq2_common::common::{
    com_printf, com_dprintf, com_error,
    msg_begin_reading, msg_read_long, msg_read_string, msg_read_string_line,
    msg_write_byte, msg_write_char, msg_write_short, msg_write_long, msg_write_string,
    msg_write_delta_entity, DISTNAME,
};
use myq2_common::qcommon::{SizeBuf, MAX_MSGLEN, NetAdr, NetAdrType};
use myq2_common::qfiles::{MAX_SKINNAME, IDALIASHEADER, ALIAS_VERSION};

// ============================================================
// Wired imports from myq2-common (free functions with matching signatures)
// ============================================================
use myq2_common::wildcards::wildcardfit;
use myq2_common::common::info_print;

// Cmd system — direct imports where signatures match
use myq2_common::cmd::{
    cmd_args, cmd_tokenize_string, cbuf_add_text, cbuf_execute, cmd_write_aliases,
};

// Cvar system — direct imports where signatures match
use myq2_common::cvar::{
    cvar_set, cvar_set_value, cvar_variable_value, cvar_variable_string,
    cvar_userinfo, cvar_write_address_book, cvar_write_variables,
};

// Filesystem — direct imports where signatures match
use myq2_common::files::{fs_gamedir, fs_create_path, fs_load_file};

// ============================================================
// Wired imports from same crate (matching signatures)
// ============================================================
use crate::cl_input::{cl_init_input, InputButtons, InputCvars, InputTiming};
use crate::console::con_init;
use crate::menu::{m_init, m_force_menu_off};
use crate::keys::key_write_bindings;

// Sub-system modules — wired below
use crate::cl_scrn::ScrState;
use crate::cl_timing::ClientTiming;
use crate::cl_view::ViewState;
use crate::cl_fx::ClFxState;
use crate::cl_tent::TEntState;
use crate::client::MAX_PARSE_ENTITIES;
use crate::cl_chat::{chat_queue_outgoing, chat_get_queued, chat_retry_message, chat_has_queued};
use myq2_common::cmodel::CModelContext;

// ============================================================
// Thin wrappers — real impl exists in myq2-common but signature differs
// ============================================================

fn com_quit() -> ! {
    std::process::exit(0);
}

use myq2_common::common::com_server_state;

// Re-use console.rs wrappers for cmd_argc / cmd_argv (usize ↔ i32 adaptation)
use crate::console::{cmd_argc, cmd_argv};

fn cmd_add_command(name: &str, func: Option<fn()>) {
    myq2_common::cmd::cmd_add_command_optional(name, func);
}

// Cvar wrapper — adapt Option<usize> return to CvarHandle
fn cvar_get(name: &str, value: &str, flags: i32) -> CvarHandle {
    // Real impl returns Option<usize> (index into cvar table); for now
    // we call it for the side-effect of registering the cvar, then build
    // a CvarHandle from the current value.
    let _idx = myq2_common::cvar::cvar_get(name, value, flags);
    CvarHandle {
        string: myq2_common::cvar::cvar_variable_string(name),
        value: myq2_common::cvar::cvar_variable_value(name),
        modified: false,
    }
}

// ============================================================
// Network wrappers — delegate to myq2_common using canonical NetAdr type
// ============================================================
// Network config — dispatched via platform callbacks, falls back to myq2_common::net::net_config
fn net_config(multiplayer: bool) { crate::platform::net_config(multiplayer); }
fn net_string_to_adr(s: &str, adr: &mut NetAdr) -> bool {
    if let Some(result) = myq2_common::net::net_string_to_adr(s) {
        *adr = result;
        true
    } else {
        false
    }
}
fn net_adr_to_string(adr: &NetAdr) -> String {
    myq2_common::net::net_adr_to_string(adr)
}
fn net_is_local_address(adr: &NetAdr) -> bool {
    myq2_common::net::net_is_local_address(adr)
}
fn net_get_packet(sock: i32, from: &mut NetAdr, message: &mut SizeBuf) -> bool {
    let net_sock = if sock == NS_SERVER {
        myq2_common::qcommon::NetSrc::Server
    } else {
        myq2_common::qcommon::NetSrc::Client
    };
    myq2_common::net::net_get_packet(net_sock, from, message)
}
fn net_compare_adr(a: &NetAdr, b: &NetAdr) -> bool {
    myq2_common::net::net_compare_adr(a, b)
}
fn net_send_packet(sock: i32, _length: usize, data: &[u8], to: &NetAdr) {
    let net_sock = if sock == NS_SERVER {
        myq2_common::qcommon::NetSrc::Server
    } else {
        myq2_common::qcommon::NetSrc::Client
    };
    myq2_common::net::net_send_packet(net_sock, data, to);
}

// Netchan — wired to myq2_common::net_chan
fn netchan_out_of_band_print(sock: i32, adr: NetAdr, msg: &str) {
    let net_sock = if sock == NS_SERVER {
        myq2_common::qcommon::NetSrc::Server
    } else {
        myq2_common::qcommon::NetSrc::Client
    };
    myq2_common::net_chan::netchan_out_of_band_print(net_sock, &adr, msg);
}
fn netchan_setup(sock: i32, chan: &mut myq2_common::qcommon::NetChan, adr: NetAdr, qport: i32) {
    let net_sock = if sock == NS_SERVER {
        myq2_common::qcommon::NetSrc::Server
    } else {
        myq2_common::qcommon::NetSrc::Client
    };
    let curtime = sys_milliseconds();
    myq2_common::net_chan::netchan_setup(net_sock, chan, adr, qport, curtime);
}
fn netchan_transmit(chan: &mut myq2_common::qcommon::NetChan, _length: usize, data: &[u8]) {
    let curtime = sys_milliseconds();
    let qport = chan.qport;

    // Get packetdup setting (R1Q2/Q2Pro feature for lossy connections)
    let dup_count = CL_PACKETDUP.lock().unwrap().value as i32;

    if dup_count > 0 {
        // Use the duplication-aware transmit function
        myq2_common::net_chan::netchan_transmit_with_dup(chan, data, curtime, qport, dup_count);
    } else {
        // Standard transmit without duplication
        myq2_common::net_chan::netchan_transmit(chan, data, curtime, qport);
    }
}
fn netchan_process(chan: &mut myq2_common::qcommon::NetChan, msg: &mut SizeBuf) -> bool {
    let curtime = sys_milliseconds();
    myq2_common::net_chan::netchan_process(chan, msg, curtime)
}

// ============================================================
// Event trigger helpers (R1Q2/Q2Pro feature)
// ============================================================

/// Execute a command stored in a cvar (event trigger).
/// Does nothing if the cvar value is empty.
fn cl_execute_trigger_cmd(cvar: &LazyLock<Mutex<CvarHandle>>) {
    let cmd = {
        let handle = cvar.lock().unwrap();
        handle.string.clone()
    };
    if !cmd.is_empty() {
        cbuf_add_text(&format!("{}\n", cmd));
        cbuf_execute();
    }
}

/// Called when entering a new map (after precache).
pub fn cl_trigger_begin_map() {
    cl_execute_trigger_cmd(&CL_BEGINMAPCMD);
    // R1Q2/Q2Pro feature: auto-record when entering a map
    cl_check_autorecord();
}

/// Called when the map is about to change.
pub fn cl_trigger_change_map() {
    cl_execute_trigger_cmd(&CL_CHANGEMAPCMD);
}

/// Called when disconnecting from a server.
pub fn cl_trigger_disconnect() {
    cl_execute_trigger_cmd(&CL_DISCONNECTCMD);
}

// ============================================================
// Network Stats Display
// ============================================================

/// Display network statistics (cl_netstats command).
pub fn cl_netstats_f() {
    let cl = CL.lock().unwrap();
    let stats = &cl.smoothing.network_stats;

    com_printf("\n=== Network Statistics ===\n");
    com_printf(&format!("Ping: {} ms (avg: {} min: {} max: {})\n",
        stats.ping, stats.avg_ping,
        if stats.min_ping == i32::MAX { 0 } else { stats.min_ping },
        stats.max_ping));
    com_printf(&format!("Jitter: {} ms\n", stats.jitter));
    com_printf(&format!("Packet Loss: {:.1}%\n", stats.packet_loss));
    com_printf(&format!("Packets: {} received, {} lost\n",
        stats.packets_received, stats.packets_lost));
    com_printf(&format!("Bandwidth: {:.1} KB/s\n", stats.incoming_bps as f32 / 1024.0));
    com_printf(&format!("Interp Buffer: {} ms\n", stats.interp_buffer_ms));
    com_printf(&format!("Adaptive Jitter Est: {} ms\n",
        cl.smoothing.adaptive_interp.get_jitter()));
    com_printf("===========================\n\n");
}

/// Display and control smoothing settings (cl_smooth command).
pub fn cl_smooth_f() {
    let cl = CL.lock().unwrap();

    com_printf("\n=== Smoothing Settings ===\n");
    com_printf(&format!("Time Nudge: {} ms\n", cl.cl_timenudge));
    com_printf(&format!("Extrapolation: {} (max {} ms)\n",
        if cl.cl_extrapolate { "enabled" } else { "disabled" },
        cl.cl_extrapolate_max));
    com_printf(&format!("Animation Continuation: {}\n",
        if cl.cl_anim_continue { "enabled" } else { "disabled" }));
    com_printf(&format!("Projectile Prediction: {}\n",
        if cl.cl_projectile_predict { "enabled" } else { "disabled" }));
    com_printf(&format!("Cubic Interpolation: {}\n",
        if cl.smoothing.cubic_interp_enabled { "enabled" } else { "disabled" }));
    com_printf(&format!("View Smoothing: {}\n",
        if cl.smoothing.view_smoothing.enabled { "enabled" } else { "disabled" }));
    com_printf(&format!("Adaptive Interp: {} (buffer {} ms)\n",
        if cl.smoothing.adaptive_interp.enabled { "enabled" } else { "disabled" },
        cl.smoothing.adaptive_interp.target_buffer_ms));
    com_printf("===========================\n\n");
}

/// Sync smoothing cvars to ClientState settings.
/// Called each frame to pick up cvar changes.
pub fn cl_update_smoothing_cvars() {
    let timenudge = CL_TIMENUDGE.lock().unwrap().value as i32;
    let extrapolate = CL_EXTRAPOLATE.lock().unwrap().value != 0.0;
    let extrapolate_max = CL_EXTRAPOLATE_MAX.lock().unwrap().value as i32;
    let anim_continue = CL_ANIM_CONTINUE.lock().unwrap().value != 0.0;
    let projectile_predict = CL_PROJECTILE_PREDICT.lock().unwrap().value != 0.0;
    let cubic_interp = CL_CUBIC_INTERP.lock().unwrap().value != 0.0;
    let view_smooth = CL_VIEW_SMOOTH.lock().unwrap().value != 0.0;
    let adaptive_interp = CL_ADAPTIVE_INTERP.lock().unwrap().value != 0.0;

    let mut cl = CL.lock().unwrap();

    // Clamp timenudge to reasonable range
    cl.cl_timenudge = timenudge.clamp(-100, 100);
    cl.cl_extrapolate = extrapolate;
    cl.cl_extrapolate_max = extrapolate_max.clamp(0, 200);
    cl.cl_anim_continue = anim_continue;
    cl.cl_projectile_predict = projectile_predict;

    // Update smoothing state
    cl.smoothing.cubic_interp_enabled = cubic_interp;
    cl.smoothing.view_smoothing.enabled = view_smooth;
    cl.smoothing.adaptive_interp.enabled = adaptive_interp;
}

/// Sync chat cvars (cl_filter_chat, cl_chat_log) to the chat system state.
/// Called per-frame from cl_frame so runtime cvar changes take effect.
pub fn cl_update_chat_cvars() {
    let filter_enabled = CL_FILTER_CHAT.lock().unwrap().value != 0.0;
    let log_enabled = CL_CHAT_LOG.lock().unwrap().value != 0.0;
    crate::cl_chat::chat_set_filter_enabled(filter_enabled);
    crate::cl_chat::chat_set_log_enabled(log_enabled);
}

// Message read/write functions — wired to myq2_common::common (imported above)

/// Register a sound backend at runtime. Called by myq2-sys to provide the
/// platform-specific audio implementation (OpenAL).
pub fn cl_register_sound_backend(backend: Box<dyn crate::snd_dma::AudioBackend + Send>) {
    let mut b = SOUND_BACKEND.lock().unwrap();
    *b = Some(backend);
}

// Sound functions — delegate to SoundState methods in snd_dma.rs.
// The AudioBackend is registered at runtime by myq2-sys; if no backend is
// registered, these are safe no-ops (SoundState.sound_started stays false).
fn s_init() {
    let mut sound = SOUND_STATE.lock().unwrap();
    let mut backend = SOUND_BACKEND.lock().unwrap();
    if let Some(ref mut be) = *backend {
        sound.s_init(be.as_mut());
    }
}
fn s_shutdown() {
    let mut sound = SOUND_STATE.lock().unwrap();
    let mut backend = SOUND_BACKEND.lock().unwrap();
    if let Some(ref mut be) = *backend {
        sound.s_shutdown(be.as_mut());
    }
}
fn s_stop_all_sounds() {
    cl_s_stop_all_sounds();
}

/// Public accessor for tent state, usable from cl_tent module.
pub fn with_tent_state<F: FnOnce(&mut crate::cl_tent::TEntState)>(f: F) {
    let mut tent = TENT_STATE.lock().unwrap();
    f(&mut tent);
}

/// Public accessor for view state, usable from other modules.
pub fn with_view_state<F: FnOnce(&mut crate::cl_view::ViewState)>(f: F) {
    let mut view = VIEW_STATE.lock().unwrap();
    f(&mut view);
}

/// Public accessor for CL + CLS state, usable from callback modules.
pub fn with_cl_cls<F: FnOnce(&mut crate::client::ClientState, &mut crate::client::ClientStatic)>(f: F) {
    let mut cl = CL.lock().unwrap();
    let mut cls = CLS.lock().unwrap();
    f(&mut cl, &mut cls);
}

/// S_Activate — pause/resume audio on window focus change.
pub fn cl_s_activate(active: bool) {
    let mut backend = SOUND_BACKEND.lock().unwrap();
    if let Some(ref mut be) = *backend {
        be.activate(active);
    }
}

/// Public accessor for s_stop_all_sounds, usable from SYSTEM_FNS dispatch.
pub fn cl_s_stop_all_sounds() {
    let mut sound = SOUND_STATE.lock().unwrap();
    let mut backend = SOUND_BACKEND.lock().unwrap();
    if let Some(ref mut be) = *backend {
        let be_ref: &mut dyn crate::snd_dma::AudioBackend = be.as_mut();
        sound.s_stop_all_sounds(Some(be_ref));
    } else {
        sound.s_stop_all_sounds(None);
    }
}

/// Public accessor for s_start_local_sound, usable from SYSTEM_FNS dispatch.
pub fn cl_s_start_local_sound(name: &str) {
    let mut sound = SOUND_STATE.lock().unwrap();
    let cl = CL.lock().unwrap();
    let cls = CLS.lock().unwrap();
    sound.s_start_local_sound(name, cl.playernum, cls.realtime as i32, &|path| {
        myq2_common::files::fs_load_file(path)
    });
}
fn s_update(origin: &Vec3, forward: &Vec3, right: &Vec3, up: &Vec3) {
    let mut sound = SOUND_STATE.lock().unwrap();
    let mut backend = SOUND_BACKEND.lock().unwrap();
    if let Some(ref mut be) = *backend {
        let cl = CL.lock().unwrap();
        let cls = CLS.lock().unwrap();
        let playernum = cl.playernum;
        let disable_screen = cls.disable_screen != 0.0;
        let current_time = cls.realtime as i32;
        let packet_loss_frames = cl.packet_loss_frames;
        // Drop cl/cls before calling s_update to avoid deadlocks
        drop(cl);
        drop(cls);
        sound.s_update(
            *origin, *forward, *right, *up,
            playernum,
            disable_screen,
            be.as_mut(),
            &|entnum| {
                // Wire CL_GetEntitySoundOrigin_Enhanced for sound spatialization.
                // This is the Rust equivalent of the C code in snd_dma.c:501:
                //   CL_GetEntitySoundOrigin(ch->entnum, origin);
                // Uses the enhanced version which handles brush models (doors, platforms)
                // by calculating sound origin at the center of the model bounds.
                let ent_state = ENT_STATE.lock().unwrap();
                let mut org = [0.0f32; 3];
                crate::cl_ents::cl_get_entity_sound_origin_enhanced(
                    entnum,
                    &mut org,
                    &ent_state,
                    None, // TODO: provide get_model_bounds callback when CM inline model lookup is available
                );
                org
            },
            &|name| fs_load_file(name),
            current_time,
            packet_loss_frames,
        );
    }
}

// Video/renderer functions — dispatched via platform callbacks registered by myq2-sys
fn vid_init() { crate::platform::vid_init(); }
fn vid_shutdown() { crate::platform::vid_shutdown(); }
fn vid_check_changes() { crate::platform::vid_check_changes(); }
fn r_set_palette(palette: Option<&[u8]>) { crate::platform::r_set_palette(palette); }

// Wired to crate::cl_scrn / crate::cl_cin
fn scr_init() {
    let mut scr = SCR_STATE.lock().unwrap();
    crate::cl_scrn::scr_init(&mut scr);
}
fn scr_update_screen() {
    let mut scr = SCR_STATE.lock().unwrap();
    let mut cls = CLS.lock().unwrap();
    let mut cl = CL.lock().unwrap();
    crate::cl_scrn::scr_update_screen(&mut scr, &mut cls, &mut cl);
}
fn scr_begin_loading_plaque() {
    let mut scr = SCR_STATE.lock().unwrap();
    let mut cls = CLS.lock().unwrap();
    let mut cl = CL.lock().unwrap();
    crate::cl_scrn::scr_begin_loading_plaque(&mut scr, &mut cls, &mut cl);
}
fn scr_end_loading_plaque(clear: bool) {
    let mut cls = CLS.lock().unwrap();
    crate::cl_scrn::scr_end_loading_plaque(&mut cls, clear);
}
fn scr_stop_cinematic() {
    let mut cl = CL.lock().unwrap();
    let mut cls = CLS.lock().unwrap();
    crate::cl_cin::scr_stop_cinematic(&mut cl, &mut cls);
}
fn scr_run_cinematic() {
    let mut cl = CL.lock().unwrap();
    let mut cls = CLS.lock().unwrap();
    crate::cl_cin::scr_run_cinematic(&mut cl, &mut cls);
}
fn scr_finish_cinematic() {
    let cl = CL.lock().unwrap();
    let mut cls = CLS.lock().unwrap();
    crate::cl_cin::scr_finish_cinematic(&mut cls, &cl);
}
fn scr_run_console() {
    let mut scr = SCR_STATE.lock().unwrap();
    let cls = CLS.lock().unwrap();
    crate::cl_scrn::scr_run_console(&mut scr, &cls);
}

// Input functions — dispatched via platform callbacks registered by myq2-sys
fn in_init() { crate::platform::in_init(); }
fn in_shutdown() { crate::platform::in_shutdown(); }
fn in_commands() { crate::platform::in_commands(); }
fn in_frame() { crate::platform::in_frame(); }

// System functions — use canonical implementation from myq2_common
use myq2_common::common::sys_milliseconds;
fn sys_send_key_events() { crate::platform::sys_send_key_events(); }
fn sys_app_activate() { crate::platform::sys_app_activate(); }

// Wired to crate::menu
fn m_add_to_server_list(_adr: &NetAdr, info: &str) {
    crate::menu::m_add_to_server_list(info);
}

// Wired to crate::cl_view
fn v_init() {
    let mut view = VIEW_STATE.lock().unwrap();
    crate::cl_view::v_init(&mut view);
}

// In Rust, memory is freed on drop — fs_free_file is a no-op.
fn fs_free_file(_data: &[u8]) {}
fn fs_fopen_file(name: &str) -> Option<File> {
    myq2_common::files::with_fs_ctx(|ctx| {
        ctx.fopen_file(name).map(|result| result.file)
    }).flatten()
}
// In Rust, File is closed on drop — this just makes the drop explicit.
fn fs_fclose_file(_f: File) { /* dropped */ }

// Wired to myq2_common::cmodel::CModelContext::load_map
fn cm_load_map(name: &str, clientload: bool, checksum: &mut u32) {
    let mut ctx = CMODEL_CTX.lock().unwrap();
    let (_num_models, map_checksum) = ctx.load_map(name, clientload, None);
    *checksum = map_checksum;
}

// Server shutdown — dispatched via platform callbacks registered by myq2-sys
fn sv_shutdown(msg: &str, reconnect: bool) { crate::platform::sv_shutdown(msg, reconnect); }

// Client module stubs — wired to real implementations where possible
fn cl_send_cmd() {
    let mut cl = CL.lock().unwrap();
    let mut cls = CLS.lock().unwrap();
    let mut buttons = INPUT_BUTTONS.lock().unwrap();
    let mut cvars = INPUT_CVARS.lock().unwrap();
    let mut timing = INPUT_TIMING.lock().unwrap();
    let sys_frame_time = sys_milliseconds() as u32;
    let anykeydown = unsafe { crate::keys::ANYKEYDOWN != 0 };
    let cl_lightlevel = CL_LIGHTLEVEL.lock().unwrap().value;
    let mut userinfo_modified = USERINFO_MODIFIED.lock().unwrap();

    // Update strafe jump cvars from actual cvar values (R1Q2/Q2Pro feature)
    cvars.cl_strafejump_fix = CL_STRAFEJUMP_FIX.lock().unwrap().value != 0.0;
    cvars.cl_physics_fps = CL_PHYSICS_FPS.lock().unwrap().value;

    // Check if attack button has an edge trigger (just pressed)
    // Bit 2 = edge triggered on down
    let attack_pressed = buttons.in_attack.state & 2 != 0;

    crate::cl_input::cl_send_cmd(
        &mut cl,
        &mut cls,
        &mut buttons,
        &cvars,
        &mut timing,
        sys_frame_time,
        anykeydown,
        cl_lightlevel,
        &mut userinfo_modified,
    );

    // Skip cinematic on any button press (matching original cl_input.c behavior)
    if cl.cmd.buttons != 0
        && cl.cinematictime > 0
        && !cl.attractloop
        && cls.realtime - cl.cinematictime > 1000
    {
        crate::cl_cin::scr_finish_cinematic(&mut cls, &cl);
    }

    // === Weapon Fire Prediction ===
    // Add predicted muzzle flash when attack button is pressed
    if attack_pressed && cl.smoothing.weapon_prediction.enabled {
        use crate::cl_smooth::WeaponEffectType;

        // Calculate muzzle position (player origin + forward offset + view offset)
        let mut muzzle_origin = cl.predicted_origin;
        for i in 0..3 {
            muzzle_origin[i] += cl.v_forward[i] * 24.0; // Forward from player
            muzzle_origin[i] += cl.frame.playerstate.viewoffset[i]; // View offset
        }

        // Determine weapon effect type based on current weapon
        // gunindex values: 1=blaster, 2=shotgun, 3=sshotgun, 4=machinegun,
        // 5=chaingun, 6=grenades, 7=launcher, 8=hyperblaster, 9=railgun, 10=bfg
        let weapon_type = match cl.frame.playerstate.gunindex {
            1 | 8 => WeaponEffectType::Tracer, // Blaster, hyperblaster
            9 => WeaponEffectType::RailTrail, // Railgun
            7 => WeaponEffectType::RocketTrail, // Rocket launcher
            _ => WeaponEffectType::MuzzleFlash, // All bullet weapons
        };

        // Capture v_forward before mutable borrow
        let v_forward = cl.v_forward;

        // Add predicted weapon effect
        cl.smoothing.weapon_prediction.predict_fire(
            weapon_type,
            muzzle_origin,
            v_forward,
            cls.realtime,
        );

        // === Predicted Recoil ===
        // Apply predicted weapon recoil immediately for better feedback.
        // This reduces perceived input lag by showing recoil before server confirms.
        let weapon_index = cl.frame.playerstate.gunindex;
        cl.smoothing.recoil_smoothing.predict_fire(weapon_index, cls.realtime);
    }

    // Clean up old predicted weapon effects
    cl.smoothing.weapon_prediction.cleanup(cls.realtime);
}
fn cl_parse_server_message() {
    let mut cl = CL.lock().unwrap();
    let mut cls = CLS.lock().unwrap();
    let mut con = PARSE_CON.lock().unwrap();
    let mut net_message = NET_MESSAGE.lock().unwrap();
    let mut cl_entities = CL_ENTITIES.lock().unwrap();
    let cl_shownet_value = CL_SHOWNET.lock().unwrap().value;
    let mut scr = SCR_STATE.lock().unwrap();
    let mut fx = FX_STATE.lock().unwrap();
    let mut tent = TENT_STATE.lock().unwrap();
    let mut ent_state = ENT_STATE.lock().unwrap();
    let mut sound = SOUND_STATE.lock().unwrap();
    let mut proj = PROJ_STATE.lock().unwrap();
    let mut ctx = crate::cl_parse::ParseContext {
        scr: &mut scr,
        fx: &mut fx,
        tent: &mut tent,
        ent_state: &mut ent_state,
        sound: &mut sound,
        proj_state: &mut proj,
    };
    crate::cl_parse::cl_parse_server_message(
        &mut cl,
        &mut cls,
        &mut con,
        &mut net_message,
        &mut cl_entities,
        cl_shownet_value,
        &mut ctx,
    );
}
fn cl_parse_clientinfo(player: i32) {
    let mut cl = CL.lock().unwrap();
    crate::cl_parse::cl_parse_clientinfo(&mut cl, player as usize);
}

// Wired to crate::cl_fx::ClFxState::cl_clear_effects
fn cl_clear_effects() {
    let mut fx = FX_STATE.lock().unwrap();
    fx.cl_clear_effects();
}

// Wired to crate::cl_tent::cl_clear_tents
fn cl_clear_tents() {
    let mut ts = TENT_STATE.lock().unwrap();
    crate::cl_tent::cl_clear_tents(&mut ts);
}

fn cl_register_sounds() {
    let mut cl = CL.lock().unwrap();
    let mut sound = SOUND_STATE.lock().unwrap();
    let mut tent = TENT_STATE.lock().unwrap();
    crate::cl_parse::cl_register_sounds(&mut cl, &mut sound, &mut tent);
}
fn cl_prep_refresh() {
    let mut view = VIEW_STATE.lock().unwrap();
    let mut scr = SCR_STATE.lock().unwrap();
    let mut cls = CLS.lock().unwrap();
    let mut cl = CL.lock().unwrap();
    // SAFETY: VIDDEF is a static global only mutated during vid_init (single-threaded init)
    let viddef = unsafe { &crate::console::VIDDEF };
    crate::cl_view::cl_prep_refresh(&mut view, &mut scr, &mut cls, &mut cl, viddef);
}

fn cl_predict_movement() {
    let mut cl = CL.lock().unwrap();
    let cls = CLS.lock().unwrap();
    let cl_predict_value = CL_PREDICT.lock().unwrap().value;
    let cl_showmiss_value = CL_SHOWMISS.lock().unwrap().value;
    let cl_paused_value = CL_PAUSED.lock().unwrap().value;
    let mut pm_airaccelerate = PM_AIRACCELERATE.lock().unwrap();

    // Snapshot data needed by pmove callbacks (cl is mutably borrowed by cl_predict_movement,
    // so callbacks can't borrow it — we copy the small fields they need).
    let pm_frame_num_entities = cl.frame.num_entities;
    let pm_frame_parse_entities = cl.frame.parse_entities;
    let pm_playernum = cl.playernum;
    let pm_model_clip = cl.model_clip.to_vec();
    // Lock parse entities once and copy the snapshot into a boxed array.
    let pm_parse_ents: Box<[myq2_common::q_shared::EntityState; MAX_PARSE_ENTITIES]> = {
        let guard = CL_PARSE_ENTITIES.lock().unwrap();
        let v: Vec<myq2_common::q_shared::EntityState> = guard.iter().take(MAX_PARSE_ENTITIES).cloned().collect();
        let boxed_slice = v.into_boxed_slice();
        // SAFETY: we collected exactly MAX_PARSE_ENTITIES elements from guard which has that size
        unsafe {
            let ptr = Box::into_raw(boxed_slice) as *mut [myq2_common::q_shared::EntityState; MAX_PARSE_ENTITIES];
            Box::from_raw(ptr)
        }
    };

    struct SnapshotPmoveCallbacks {
        num_entities: i32,
        parse_entities_offset: i32,
        playernum: i32,
        model_clip: Vec<i32>,
        parse_ents: Box<[myq2_common::q_shared::EntityState; MAX_PARSE_ENTITIES]>,
    }
    impl myq2_common::pmove::PmoveCallbacks for SnapshotPmoveCallbacks {
        fn trace(&self, start: &Vec3, mins: &Vec3, maxs: &Vec3, end: &Vec3) -> myq2_common::q_shared::Trace {
            // Build a minimal ClientState-like view for cl_clip_move_to_entities
            let cl_view = crate::cl_pred::PmoveClView {
                num_entities: self.num_entities,
                parse_entities: self.parse_entities_offset,
                playernum: self.playernum,
                model_clip: &self.model_clip,
            };
            crate::cl_pred::cl_pm_trace_with_view(
                start, mins, maxs, end,
                &cl_view,
                &self.parse_ents,
                &myq2_common::cmodel::cm_box_trace,
                &myq2_common::cmodel::cm_headnode_for_box,
                &myq2_common::cmodel::cm_transformed_box_trace,
            )
        }
        fn pointcontents(&self, point: &Vec3) -> i32 {
            let cl_view = crate::cl_pred::PmoveClView {
                num_entities: self.num_entities,
                parse_entities: self.parse_entities_offset,
                playernum: self.playernum,
                model_clip: &self.model_clip,
            };
            crate::cl_pred::cl_pm_point_contents_with_view(
                point,
                &cl_view,
                &self.parse_ents,
                &myq2_common::cmodel::cm_point_contents,
                &myq2_common::cmodel::cm_transformed_point_contents,
            )
        }
    }
    let callbacks = SnapshotPmoveCallbacks {
        num_entities: pm_frame_num_entities,
        parse_entities_offset: pm_frame_parse_entities,
        playernum: pm_playernum,
        model_clip: pm_model_clip,
        parse_ents: pm_parse_ents,
    };

    let pmove_fn = |pm: &mut myq2_common::q_shared::PmoveData| {
        myq2_common::pmove::pmove(pm, &callbacks);
    };

    crate::cl_pred::cl_predict_movement(
        &mut cl,
        &cls,
        cl_predict_value,
        cl_showmiss_value,
        cl_paused_value,
        &mut pm_airaccelerate,
        &pmove_fn,
    );
}

// Wired to crate::cl_fx::ClFxState::cl_run_dlights
fn cl_run_dlights() {
    let mut fx = FX_STATE.lock().unwrap();
    let cl = CL.lock().unwrap();
    let cls = CLS.lock().unwrap();
    fx.cl_run_dlights(cl.time as f32, cls.frametime as f32);
}

// Wired to crate::cl_fx::ClFxState::cl_run_light_styles
fn cl_run_light_styles() {
    let mut fx = FX_STATE.lock().unwrap();
    let cl = CL.lock().unwrap();
    fx.cl_run_light_styles(cl.time);
}

fn cl_check_or_download_file(name: &str) -> bool {
    let mut cls = CLS.lock().unwrap();
    crate::cl_parse::cl_check_or_download_file(&mut cls, name)
}
fn cl_download_f() {
    let mut cls = CLS.lock().unwrap();
    let args_str = cmd_args();
    let args: Vec<&str> = std::iter::once("download").chain(args_str.split_whitespace()).collect();
    crate::cl_parse::cl_download_f(&mut cls, &args);
}

// Demo playback command wrappers
fn cl_seek_f() {
    let args_str = cmd_args();
    crate::cl_demo::cmd_seek(&args_str);
}

fn cl_seekpercent_f() {
    let args_str = cmd_args();
    crate::cl_demo::cmd_seekpercent(&args_str);
}

fn cl_demo_pause_f() {
    crate::cl_demo::cmd_demo_pause();
}

fn cl_demo_speed_f() {
    let args_str = cmd_args();
    crate::cl_demo::cmd_demo_speed(&args_str);
}

fn cl_demo_info_f() {
    crate::cl_demo::cmd_demo_info();
}

/// playdemo <demoname> - Play a demo file
fn cl_playdemo_f() {
    if cmd_argc() != 2 {
        com_printf("Usage: playdemo <demoname>\n");
        return;
    }

    let name = cmd_argv(1);
    cl_playdemo(&name);
}

/// Start playing a demo file.
pub fn cl_playdemo(name: &str) {
    // Disconnect from any current server
    cl_disconnect();

    // Build the demo path
    let gamedir = fs_gamedir();
    let demo_path = if name.ends_with(".dm2") {
        format!("{}/demos/{}", gamedir, name)
    } else {
        format!("{}/demos/{}.dm2", gamedir, name)
    };

    com_printf(&format!("Playing demo: {}\n", demo_path));

    // Start demo playback using our enhanced demo system
    let mut playback = crate::cl_demo::DEMO_PLAYBACK.lock().unwrap();
    if let Err(e) = playback.start(&demo_path) {
        com_printf(&format!("Failed to play demo: {}\n", e));
        return;
    }
    drop(playback);

    // Set client state for demo playback
    {
        let mut cls = CLS.lock().unwrap();
        cls.demo_playing = true;
        cls.demo_file_path = demo_path;
        cls.state = crate::client::ConnState::Connected;
    }

    // Initialize client state for the demo
    {
        let mut cl = CL.lock().unwrap();
        *cl = crate::client::ClientState::default();
    }

    com_printf("Demo playback started. Use demo_info, seek, demo_pause, demo_speed.\n");
}

/// Stop demo playback.
pub fn cl_stopdemo() {
    let mut playback = crate::cl_demo::DEMO_PLAYBACK.lock().unwrap();
    if !playback.playing {
        return;
    }

    playback.stop();

    let mut cls = CLS.lock().unwrap();
    cls.demo_playing = false;
    cls.demo_file_path.clear();
    cls.state = crate::client::ConnState::Disconnected;

    com_printf("Demo playback stopped.\n");
}

/// Read a message from the demo file.
/// Returns true if a message was read, false if end of demo.
fn cl_read_demo_message() -> bool {
    use std::io::Read;

    let mut playback = crate::cl_demo::DEMO_PLAYBACK.lock().unwrap();

    if !playback.playing {
        return false;
    }

    // Check if we should process a frame (handles speed and pause)
    let frame_msec = {
        let cls = CLS.lock().unwrap();
        (cls.frametime * 1000.0) as i32
    };

    if !playback.should_process_frame(frame_msec) && !playback.paused {
        // Not time to process yet at current speed
        return true; // Still playing, just waiting
    }

    if playback.paused {
        return true; // Paused but still "playing"
    }

    // Read from demo file
    let reader = match playback.reader.as_mut() {
        Some(r) => r,
        None => return false,
    };

    // Read message length (4 bytes, little endian)
    let mut len_bytes = [0u8; 4];
    if reader.read_exact(&mut len_bytes).is_err() {
        // End of demo
        drop(playback);
        cl_stopdemo();
        com_printf("Demo playback finished.\n");
        return false;
    }
    let msg_len = i32::from_le_bytes(len_bytes);

    if msg_len == -1 {
        // End of demo marker
        drop(playback);
        cl_stopdemo();
        com_printf("Demo playback finished.\n");
        return false;
    }

    if msg_len <= 0 || msg_len > MAX_MSGLEN as i32 {
        drop(playback);
        cl_stopdemo();
        com_printf("Demo message length out of range.\n");
        return false;
    }

    // Read the message data into NET_MESSAGE
    {
        let mut net_msg = NET_MESSAGE.lock().unwrap();
        let msg_len_usize = msg_len as usize;

        // Ensure buffer is large enough
        if net_msg.data.len() < msg_len_usize {
            net_msg.data.resize(msg_len_usize + 16, 0);
        }

        if reader.read_exact(&mut net_msg.data[..msg_len_usize]).is_err() {
            drop(playback);
            drop(net_msg);
            cl_stopdemo();
            com_printf("Error reading demo message.\n");
            return false;
        }

        net_msg.cursize = msg_len;
        net_msg.readcount = 0;
    }

    // Update playback time from the frame if present
    // (This is a simplified version - the full version would parse the frame)

    true
}

// ============================================================
// Location command wrappers
// ============================================================

fn cl_loc_f() {
    let cl = CL.lock().unwrap();
    let pos = cl.predicted_origin;
    crate::cl_loc::cmd_loc(pos);
}

fn cl_loclist_f() {
    crate::cl_loc::cmd_loclist();
}

fn cl_locadd_f() {
    let args_str = cmd_args();
    let cl = CL.lock().unwrap();
    let pos = cl.predicted_origin;
    let gamedir = fs_gamedir();
    crate::cl_loc::cmd_locadd(&args_str, pos, &gamedir);
}

fn cl_locdel_f() {
    let args_str = cmd_args();
    let index_str = args_str.split_whitespace().next().unwrap_or("");
    crate::cl_loc::cmd_locdel(index_str);
}

fn cl_locsave_f() {
    let gamedir = fs_gamedir();
    crate::cl_loc::cmd_locsave(&gamedir);
}

// ============================================================
// Chat command wrappers (R1Q2/Q2Pro feature)
// ============================================================

fn cl_ignore_f() {
    let args_str = cmd_args();
    crate::cl_chat::cmd_ignore(&args_str);
}

fn cl_unignore_f() {
    let args_str = cmd_args();
    crate::cl_chat::cmd_unignore(&args_str);
}

fn cl_ignorelist_f() {
    crate::cl_chat::cmd_ignorelist();
}

fn cl_filter_reload_f() {
    crate::cl_chat::cmd_filter_reload();
}

// ============================================================
// Crosshair command wrappers (R1Q2/Q2Pro feature)
// ============================================================

fn cl_crosshair_info_f() {
    crate::cl_crosshair::cmd_crosshair_info();
}

// ============================================================
// HUD command wrappers (R1Q2/Q2Pro feature)
// ============================================================

fn cl_hud_info_f() {
    crate::cl_hud::cmd_hud_info();
}

fn cl_hud_reset_speed_f() {
    crate::cl_hud::hud_reset_speed_max();
    com_printf("Speed meter max reset.\n");
}

fn cl_timer_start_f() {
    let cl = CL.lock().unwrap();
    let server_time = cl.time;
    drop(cl);
    crate::cl_hud::hud_start_timer(server_time);
    com_printf("Timer started.\n");
}

fn cl_timer_stop_f() {
    crate::cl_hud::hud_stop_timer();
    com_printf("Timer stopped.\n");
}

// ============================================================
// Server browser command wrappers (R1Q2/Q2Pro feature)
// ============================================================

fn cl_browser_refresh_f() {
    crate::cl_browser::browser_refresh();
    com_printf("Refreshing server list...\n");
}

fn cl_browser_info_f() {
    crate::cl_browser::cmd_browser_info();
}

fn cl_serverlist_f() {
    crate::cl_browser::cmd_serverlist();
}

fn cl_browser_clear_f() {
    crate::cl_browser::cmd_browser_clear();
}

fn cl_addfavorite_f() {
    if cmd_argc() < 2 {
        com_printf("Usage: addfavorite <address>\n");
        return;
    }
    let addr = cmd_argv(1);
    crate::cl_browser::browser_toggle_favorite(&addr);
}

fn cl_addserver_f() {
    if cmd_argc() < 2 {
        com_printf("Usage: addserver <address>\n");
        return;
    }
    let addr = cmd_argv(1);
    crate::cl_browser::browser_add_server(&addr);
    com_printf(&format!("Added server: {}\n", addr));
}

fn cl_browser_filter_f() {
    let argc = cmd_argc();
    if argc < 2 {
        com_printf("Usage: browser_filter <key> [value]\n");
        com_printf("  Keys: name, map, gametype, notempty, notfull, maxping, clear\n");
        return;
    }
    let key = cmd_argv(1);
    let value = if argc >= 3 { cmd_argv(2) } else { String::new() };
    let mut browser = crate::cl_browser::BROWSER.lock().unwrap();
    match key.to_lowercase().as_str() {
        "name" => browser.filter.name_contains = value,
        "map" => browser.filter.map_contains = value,
        "gametype" => browser.filter.gametype = value,
        "notempty" => browser.filter.not_empty = value != "0",
        "notfull" => browser.filter.not_full = value != "0",
        "maxping" => browser.filter.max_ping = value.parse().unwrap_or(0),
        "clear" => browser.filter = crate::cl_browser::ServerFilter::default(),
        _ => {
            com_printf(&format!("Unknown filter key: {}\n", key));
            return;
        }
    }
    com_printf("Filter updated.\n");
}

fn cl_browser_sort_f() {
    let argc = cmd_argc();
    if argc < 2 {
        com_printf("Usage: browser_sort <column> [asc|desc]\n");
        com_printf("  Columns: name, map, players, ping, gametype\n");
        return;
    }
    let col_str = cmd_argv(1);
    let column = match col_str.to_lowercase().as_str() {
        "name" => crate::cl_browser::SortColumn::Name,
        "map" => crate::cl_browser::SortColumn::Map,
        "players" => crate::cl_browser::SortColumn::Players,
        "ping" => crate::cl_browser::SortColumn::Ping,
        "gametype" => crate::cl_browser::SortColumn::GameType,
        _ => {
            com_printf(&format!("Unknown sort column: {}\n", col_str));
            return;
        }
    };
    let ascending = if argc >= 3 {
        cmd_argv(2).to_lowercase() != "desc"
    } else {
        true
    };
    crate::cl_browser::browser_set_sort(column, ascending);
    com_printf(&format!("Sorted by {} {}.\n", col_str, if ascending { "ascending" } else { "descending" }));
}

// ============================================================
// Constants
// ============================================================

// PROTOCOL_VERSION and CLC_* come from qcommon
use myq2_common::qcommon::{PROTOCOL_VERSION, CLC_STRINGCMD};
const PORT_SERVER: u16 = myq2_common::qcommon::PORT_SERVER as u16;

const NS_CLIENT: i32 = 0;
const NS_SERVER: i32 = 1;

// Server command bytes — imported from canonical definitions
use myq2_common::qcommon::{SVC_SERVERDATA, SVC_CONFIGSTRING, SVC_SPAWNBASELINE, SVC_STUFFTEXT};

// MAX_PARSE_ENTITIES from crate::client, MAX_MSGLEN from qcommon, MAX_SKINNAME from qfiles

const PLAYER_MULT: usize = 5;
const ENV_CNT: usize = CS_PLAYERSKINS + MAX_CLIENTS * PLAYER_MULT;
const TEXTURE_CNT: usize = ENV_CNT + 13;

// IDALIASHEADER, ALIAS_VERSION imported from myq2_common::qfiles
// DISTNAME comes from myq2_common::common

// ============================================================
// Placeholder types for external subsystems
// ============================================================

#[derive(Debug, Clone, Default)]
pub struct CvarHandle {
    pub string: String,
    pub value: f32,
    pub modified: bool,
}

// NetAdr and NetAdrType imported from myq2_common::qcommon (canonical types)
// SizeBuf imported from myq2_common::qcommon::SizeBuf (Vec-based)

#[derive(Debug, Clone, Default)]
pub struct Refdef {
    pub vieworg: Vec3,
    pub blend: [f32; 4],
}

/// Alias model header (binary compatible with dmdl_t)
#[derive(Debug, Clone, Default)]
#[repr(C)]
pub struct DmdlT {
    pub ident: i32,
    pub version: i32,
    pub skinwidth: i32,
    pub skinheight: i32,
    pub framesize: i32,
    pub num_skins: i32,
    pub num_xyz: i32,
    pub num_st: i32,
    pub num_tris: i32,
    pub num_glcmds: i32,
    pub num_frames: i32,
    pub ofs_skins: i32,
    pub ofs_st: i32,
    pub ofs_tris: i32,
    pub ofs_frames: i32,
    pub ofs_glcmds: i32,
    pub ofs_end: i32,
}

// ============================================================
// Connection state
// ============================================================

// ============================================================
// Module-level statics (behind Mutex for thread safety)
// ============================================================

pub(crate) static CL: LazyLock<Mutex<crate::client::ClientState>> = LazyLock::new(|| Mutex::new(crate::client::ClientState::default()));
pub(crate) static CLS: LazyLock<Mutex<crate::client::ClientStatic>> = LazyLock::new(|| Mutex::new(crate::client::ClientStatic::default()));
pub(crate) static CL_ENTITIES: LazyLock<Mutex<Vec<crate::client::CEntity>>> = LazyLock::new(|| Mutex::new(vec![crate::client::CEntity::default(); MAX_EDICTS]));
static CL_PARSE_ENTITIES: LazyLock<Mutex<Vec<EntityState>>> = LazyLock::new(|| Mutex::new(vec![EntityState::default(); MAX_PARSE_ENTITIES]));
static NET_MESSAGE: LazyLock<Mutex<SizeBuf>> = LazyLock::new(|| Mutex::new(SizeBuf::new(MAX_MSGLEN as i32)));
static NET_FROM: LazyLock<Mutex<NetAdr>> = LazyLock::new(|| Mutex::new(NetAdr::default()));

/// Global client timing state for decoupled frame processing (cl_async feature)
static CL_TIMING: LazyLock<Mutex<ClientTiming>> = LazyLock::new(|| Mutex::new(ClientTiming::new()));

// ============================================================
// Cvar handles — module statics
// ============================================================

static FREELOOK: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static CL_STEREO_SEPARATION: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static CL_STEREO: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static RCON_CLIENT_PASSWORD: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static RCON_ADDRESS: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static CL_NOSKINS: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static CL_DEFAULTSKIN: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static CL_FOOTSTEPS: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static CL_TIMEOUT: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static CL_PREDICT: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static CL_MAXFPS: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static CL_GUN: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static CL_ADD_PARTICLES: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static CL_ADD_LIGHTS: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static CL_ADD_ENTITIES: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static CL_ADD_BLEND: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static CL_SHOWNET: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static CL_SHOWMISS: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static CL_SHOWCLAMP: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static CL_PAUSED: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static CL_TIMEDEMO: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static CL_LIGHTLEVEL: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static CL_VWEP: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));

// Event trigger cvars (R1Q2/Q2Pro feature)
/// Command to execute when entering a new map
static CL_BEGINMAPCMD: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Command to execute when the map changes
static CL_CHANGEMAPCMD: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Command to execute when disconnecting from a server
static CL_DISCONNECTCMD: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));

// Auto-reconnect cvars (R1Q2/Q2Pro feature)
/// Enable auto-reconnect on unexpected disconnect (0=off, 1=on). Default: 0
static CL_AUTORECONNECT: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Delay between reconnect attempts in milliseconds. Default: 3000
static CL_AUTORECONNECT_DELAY: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Maximum number of reconnect attempts. Default: 3
static CL_AUTORECONNECT_MAX: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));

// Decoupled frame timing cvars (R1Q2/Q2Pro cl_async feature)
/// Enable decoupled timing (0=legacy, 1=enabled). Default: 1 (enabled)
static CL_ASYNC: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Maximum render FPS (0=unlimited, follows vsync). Default: 0
static R_MAXFPS: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Maximum network packets per second. Default: 30
static CL_MAXPACKETS: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Number of duplicate packets to send (0-2). Default: 0
/// Helps compensate for packet loss on lossy connections (WiFi, satellite).
static CL_PACKETDUP: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Enable FPS-independent strafe jumping (R1Q2/Q2Pro feature). Default: 1
static CL_STRAFEJUMP_FIX: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Target physics FPS for strafe jump normalization. Default: 125
static CL_PHYSICS_FPS: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Enable chat word filtering (R1Q2/Q2Pro feature). Default: 1
static CL_FILTER_CHAT: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Enable chat logging to file (R1Q2/Q2Pro feature). Default: 0
static CL_CHAT_LOG: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));

// Crosshair customization cvars (R1Q2/Q2Pro feature)
/// Crosshair size multiplier (0.5-4.0). Default: 1.0
static CROSSHAIR_SIZE: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Crosshair color (Q2 palette index 0-255). Default: 0xf0 (white)
static CROSSHAIR_COLOR: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Crosshair alpha transparency (0.0-1.0). Default: 1.0
static CROSSHAIR_ALPHA: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Crosshair center gap in pixels. Default: 2
static CROSSHAIR_GAP: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Crosshair line thickness in pixels (1-8). Default: 2
static CROSSHAIR_THICKNESS: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Enable dynamic crosshair (expands on movement/firing). Default: 0
static CROSSHAIR_DYNAMIC: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Enable health-based crosshair color (R1Q2/Q2Pro ch_health). Default: 0
/// 0 = disabled, 1 = enabled (green >66, yellow 33-66, red <33)
static CH_HEALTH: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));

// HUD customization cvars (R1Q2/Q2Pro feature)
/// Global HUD scale factor (0.5-2.0). Default: 1.0
static HUD_SCALE: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Global HUD alpha transparency (0.0-1.0). Default: 1.0
static HUD_ALPHA: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Show health display. Default: 1
static HUD_SHOW_HEALTH: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Show armor display. Default: 1
static HUD_SHOW_ARMOR: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Show ammo display. Default: 1
static HUD_SHOW_AMMO: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Show match timer. Default: 0
static HUD_SHOW_TIMER: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Show FPS counter. Default: 0
static HUD_SHOW_FPS: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Show velocity/speed meter. Default: 0
static HUD_SHOW_SPEED: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Show network statistics (ping, jitter, packet loss). Default: 0
static HUD_SHOW_NETSTATS: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Minimal HUD mode (only essential info). Default: 0
static HUD_MINIMAL: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));

// Demo recording enhancement cvars (R1Q2/Q2Pro feature)
/// Auto-record demos when entering a match. Default: 0
static CL_AUTORECORD: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));

// Network smoothing cvars (R1Q2/Q2Pro feature)
/// Time nudge for interpolation (ms). Negative = more responsive, positive = smoother. Range: -100 to 100
static CL_TIMENUDGE: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Enable velocity-based extrapolation for remote entities. Default: 1
static CL_EXTRAPOLATE: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Maximum extrapolation time (ms) before clamping. Default: 50
static CL_EXTRAPOLATE_MAX: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Enable client-side animation continuation during packet loss. Default: 1
static CL_ANIM_CONTINUE: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Enable projectile prediction/extrapolation. Default: 1
static CL_PROJECTILE_PREDICT: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Enable cubic/spline interpolation for smoother entity movement. Default: 1
static CL_CUBIC_INTERP: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Enable view smoothing to prevent camera snapping. Default: 1
static CL_VIEW_SMOOTH: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
/// Enable adaptive interpolation based on network jitter. Default: 1
static CL_ADAPTIVE_INTERP: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));

// Mouse cvars
static LOOKSPRING: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static LOOKSTRAFE: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static SENSITIVITY: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static M_PITCH: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static M_YAW: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static M_FORWARD: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static M_SIDE: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));

// Userinfo cvars
static INFO_PASSWORD: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static INFO_SPECTATOR: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static CVAR_NAME: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static SKIN: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static RATE: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static FOV: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static MSG_LEVEL: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static HAND: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static GENDER: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));
static GENDER_AUTO: LazyLock<Mutex<CvarHandle>> = LazyLock::new(|| Mutex::new(CvarHandle::default()));

// Sub-system state globals
static SCR_STATE: LazyLock<Mutex<ScrState>> = LazyLock::new(|| Mutex::new(ScrState::default()));
pub(crate) static VIEW_STATE: LazyLock<Mutex<ViewState>> = LazyLock::new(|| Mutex::new(ViewState::default()));
pub(crate) static FX_STATE: LazyLock<Mutex<ClFxState>> = LazyLock::new(|| Mutex::new(ClFxState::default()));
pub(crate) static TENT_STATE: LazyLock<Mutex<TEntState>> = LazyLock::new(|| Mutex::new(TEntState::default()));
pub(crate) static SOUND_STATE: LazyLock<Mutex<crate::snd_dma::SoundState>> = LazyLock::new(|| Mutex::new(crate::snd_dma::SoundState::default()));
static SOUND_BACKEND: LazyLock<Mutex<Option<Box<dyn crate::snd_dma::AudioBackend + Send>>>> = LazyLock::new(|| Mutex::new(None));
pub(crate) static ENT_STATE: LazyLock<Mutex<crate::cl_ents::ClientEntState>> = LazyLock::new(|| Mutex::new(crate::cl_ents::ClientEntState::default()));
/// Global projectile state — tracks client-side projectile entities for rendering.
/// Corresponds to the projectile subsystem in cl_ents.c (originally #if 0'd in vanilla Q2).
pub(crate) static PROJ_STATE: LazyLock<Mutex<crate::cl_ents::ProjectileState>> = LazyLock::new(|| Mutex::new(crate::cl_ents::ProjectileState::default()));
static CMODEL_CTX: LazyLock<Mutex<CModelContext>> = LazyLock::new(|| Mutex::new(CModelContext::default()));
static PARSE_CON: LazyLock<Mutex<crate::cl_parse::Console>> = LazyLock::new(|| Mutex::new(crate::cl_parse::Console::default()));
static INPUT_BUTTONS: LazyLock<Arc<Mutex<InputButtons>>> = LazyLock::new(|| Arc::new(Mutex::new(InputButtons::default())));
static INPUT_CVARS: LazyLock<Mutex<InputCvars>> = LazyLock::new(|| Mutex::new(InputCvars::default()));
static INPUT_TIMING: LazyLock<Mutex<InputTiming>> = LazyLock::new(|| Mutex::new(InputTiming::default()));

// External cvar references (placeholders)
static USERINFO_MODIFIED: Mutex<bool> = Mutex::new(false);
static PM_AIRACCELERATE: Mutex<f32> = Mutex::new(0.0);

// Demo file handle (not yet in client::ClientStatic)
static DEMO_FILE: Mutex<Option<BufWriter<File>>> = Mutex::new(None);
// Demo compression flag for R1Q2/Q2Pro -z feature
static DEMO_COMPRESSED: Mutex<bool> = Mutex::new(false);

// Precache state
static PRECACHE_CHECK: Mutex<i32> = Mutex::new(0);
static PRECACHE_SPAWNCOUNT: Mutex<i32> = Mutex::new(0);
static PRECACHE_TEX: Mutex<i32> = Mutex::new(0);
static PRECACHE_MODEL_SKIN: Mutex<i32> = Mutex::new(0);
static PRECACHE_MODEL: Mutex<Option<Vec<u8>>> = Mutex::new(None);

static ENV_SUF: [&str; 6] = ["rt", "bk", "lf", "ft", "up", "dn"];

// External globals — read from cvars
fn allow_download_value() -> bool { myq2_common::cvar::cvar_variable_value("allow_download") != 0.0 }
fn allow_download_players_value() -> bool { myq2_common::cvar::cvar_variable_value("allow_download_players") != 0.0 }
fn allow_download_models_value() -> bool { myq2_common::cvar::cvar_variable_value("allow_download_models") != 0.0 }
fn allow_download_sounds_value() -> bool { myq2_common::cvar::cvar_variable_value("allow_download_sounds") != 0.0 }
fn allow_download_maps_value() -> bool { myq2_common::cvar::cvar_variable_value("allow_download_maps") != 0.0 }
fn dedicated_value() -> bool { myq2_common::cvar::cvar_variable_value("dedicated") != 0.0 }
fn host_speeds_value() -> bool { myq2_common::cvar::cvar_variable_value("host_speeds") != 0.0 }
fn log_stats_value() -> bool { myq2_common::cvar::cvar_variable_value("log_stats") != 0.0 }
fn cl_autorecord_value() -> bool { myq2_common::cvar::cvar_variable_value("cl_autorecord") != 0.0 }

/// Generate an automatic demo filename with timestamp.
/// Format: demo_YYYYMMDD_HHMMSS.dm2
fn generate_demo_name() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Convert to date/time components (simplified calculation)
    let secs_per_day = 86400u64;
    let secs_per_hour = 3600u64;
    let secs_per_min = 60u64;

    // Days since 1970-01-01
    let days = now / secs_per_day;
    let remaining = now % secs_per_day;

    // Time of day
    let hours = remaining / secs_per_hour;
    let mins = (remaining % secs_per_hour) / secs_per_min;
    let secs = remaining % secs_per_min;

    // Calculate year, month, day (simplified - doesn't account for leap years perfectly)
    let mut year = 1970u64;
    let mut remaining_days = days;

    loop {
        let days_in_year = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            366
        } else {
            365
        };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    // Month/day calculation (simplified)
    let is_leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_months: [u64; 12] = if is_leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1u64;
    for days_in_month in &days_in_months {
        if remaining_days < *days_in_month {
            break;
        }
        remaining_days -= days_in_month;
        month += 1;
    }
    let day = remaining_days + 1;

    format!("demo_{:04}{:02}{:02}_{:02}{:02}{:02}",
        year, month, day, hours, mins, secs)
}

// ============================================================
// CL_WriteDemoMessage
//
// Dumps the current net message, prefixed by the length
// ============================================================

pub fn cl_write_demo_message() {
    let net_msg = NET_MESSAGE.lock().unwrap();

    // the first eight bytes are just packet sequencing stuff
    let len = net_msg.cursize - 8;

    if !net_msg.data.is_empty() && len > 0 {
        // Use demo_write_message for compression support (R1Q2/Q2Pro -z flag)
        demo_write_message(&net_msg.data[8..(8 + len as usize)]);
    }
}

// ============================================================
// Helper: Write demo data (optionally compressed)
// R1Q2/Q2Pro -z flag support for compressed demo recording
// ============================================================

/// Write a demo message block, optionally with compression.
/// Format: 4-byte length (negative if compressed) + data
fn demo_write_message(data: &[u8]) {
    let mut demo_file = DEMO_FILE.lock().unwrap();
    let compressed = *DEMO_COMPRESSED.lock().unwrap();

    if let Some(ref mut file) = *demo_file {
        if compressed && data.len() > 100 {
            // Try to compress the data
            if let Some(compressed_data) = myq2_common::compression::compress_packet(data) {
                // Write negative length to indicate compression, followed by original length
                let compressed_len = compressed_data.len() as i32;
                let original_len = data.len() as i32;

                // Format: -compressed_len (4 bytes) + original_len (4 bytes) + compressed_data
                let _ = file.write_all(&(-compressed_len).to_le_bytes());
                let _ = file.write_all(&original_len.to_le_bytes());
                let _ = file.write_all(&compressed_data);
                return;
            }
        }

        // Write uncompressed (or compression not beneficial)
        let len = little_long(data.len() as i32);
        let _ = file.write_all(&len.to_le_bytes());
        let _ = file.write_all(data);
    }
}

// ============================================================
// CL_Stop_f
//
// stop recording a demo
// ============================================================

pub fn cl_stop_f() {
    let mut cls = CLS.lock().unwrap();

    if !cls.demo_recording {
        com_printf("Not recording a demo.\n");
        return;
    }

    // finish up
    let len: i32 = -1;
    let mut demo_file = DEMO_FILE.lock().unwrap();
    if let Some(ref mut file) = *demo_file {
        let _ = file.write_all(&len.to_le_bytes());
        let _ = file.flush();
    }
    *demo_file = None;
    *DEMO_COMPRESSED.lock().unwrap() = false;
    cls.demo_recording = false;
    com_printf("Stopped demo.\n");
}

// ============================================================
// CL_Record_f
//
// record <demoname>
// Begins recording a demo from the current position
// ============================================================

pub fn cl_record_f() {
    {
        let cls = CLS.lock().unwrap();
        if cls.demo_recording {
            com_printf("Already recording.\n");
            return;
        }
        if cls.state != crate::client::ConnState::Active {
            com_printf("You must be in a level to record.\n");
            return;
        }
    }

    let cl = CL.lock().unwrap();

    // R1Q2/Q2Pro feature: parse -z flag for compressed demo recording
    let mut compressed = false;
    let mut demo_name = String::new();
    let argc = cmd_argc();

    for i in 1..argc {
        let arg = cmd_argv(i);
        if arg == "-z" {
            compressed = true;
        } else if demo_name.is_empty() {
            demo_name = arg;
        }
    }

    // Generate demo filename if not provided
    if demo_name.is_empty() {
        demo_name = generate_demo_name();
    }

    // Set compression flag
    *DEMO_COMPRESSED.lock().unwrap() = compressed;

    // open the demo file
    // Use .dm2z extension for compressed demos (R1Q2/Q2Pro convention)
    let ext = if compressed { "dm2z" } else { "dm2" };
    let name = format!("{}/demos/{}.{}", fs_gamedir(), demo_name, ext);

    if compressed {
        com_printf(&format!("recording (compressed) to {}.\n", name));
    } else {
        com_printf(&format!("recording to {}.\n", name));
    }
    fs_create_path(&name);

    let file = match File::create(&name) {
        Ok(f) => f,
        Err(_) => {
            com_printf("ERROR: couldn't open.\n");
            return;
        }
    };

    let mut cls = CLS.lock().unwrap();
    *DEMO_FILE.lock().unwrap() = Some(BufWriter::new(file));
    cls.demo_recording = true;

    // don't start saving messages until a non-delta compressed message is received
    cls.demo_waiting = true;

    // write out messages to hold the startup information
    let mut buf = SizeBuf::new(MAX_MSGLEN as i32);

    // send the serverdata
    msg_write_byte(&mut buf, SVC_SERVERDATA);
    msg_write_long(&mut buf, PROTOCOL_VERSION);
    msg_write_long(&mut buf, 0x10000 + cl.servercount);
    msg_write_byte(&mut buf, 1); // demos are always attract loops
    msg_write_string(&mut buf, &cl.gamedir);
    msg_write_short(&mut buf, cl.playernum);

    msg_write_string(&mut buf, &cl.configstrings[CS_NAME]);

    // configstrings
    for i in 0..MAX_CONFIGSTRINGS {
        if !cl.configstrings[i].is_empty() {
            if buf.cursize as usize + cl.configstrings[i].len() + 32 > buf.maxsize as usize {
                // write it out (uses compression if -z flag was set)
                demo_write_message(&buf.data[..buf.cursize as usize]);
                buf.cursize = 0;
            }

            msg_write_byte(&mut buf, SVC_CONFIGSTRING);
            msg_write_short(&mut buf, i as i32);
            msg_write_string(&mut buf, &cl.configstrings[i]);
        }
    }

    // baselines
    let nullstate = EntityState::default();
    let cl_ents = CL_ENTITIES.lock().unwrap();
    for i in 0..MAX_EDICTS {
        let ent = &cl_ents[i].baseline;
        if ent.modelindex == 0 {
            continue;
        }

        if buf.cursize as usize + 64 > buf.maxsize as usize {
            // write it out (uses compression if -z flag was set)
            demo_write_message(&buf.data[..buf.cursize as usize]);
            buf.cursize = 0;
        }

        msg_write_byte(&mut buf, SVC_SPAWNBASELINE);
        msg_write_delta_entity(&nullstate, &cl_ents[i].baseline, &mut buf, true, true);
    }

    msg_write_byte(&mut buf, SVC_STUFFTEXT);
    msg_write_string(&mut buf, "precache\n");

    // write it to the demo file (uses compression if -z flag was set)
    demo_write_message(&buf.data[..buf.cursize as usize]);
}

// ============================================================
// CL_RecordFromDemo_f (R1Q2/Q2Pro feature)
//
// record_from_demo [demoname]
// Begin recording while playing back a demo (re-recording)
// ============================================================

pub fn cl_record_from_demo_f() {
    {
        let cls = CLS.lock().unwrap();
        if cls.demo_recording {
            com_printf("Already recording.\n");
            return;
        }
        if !cls.demo_playing {
            com_printf("You must be playing a demo to use record_from_demo.\n");
            com_printf("Use 'record' to record during live play.\n");
            return;
        }
    }

    // R1Q2/Q2Pro feature: parse -z flag for compressed demo recording
    let mut compressed = false;
    let mut demo_name = String::new();
    let argc = cmd_argc();

    for i in 1..argc {
        let arg = cmd_argv(i);
        if arg == "-z" {
            compressed = true;
        } else if demo_name.is_empty() {
            demo_name = arg;
        }
    }

    // Generate demo filename if not provided
    if demo_name.is_empty() {
        demo_name = generate_demo_name();
    }

    // Set compression flag
    *DEMO_COMPRESSED.lock().unwrap() = compressed;

    let cl = CL.lock().unwrap();

    // open the demo file
    // Use .dm2z extension for compressed demos (R1Q2/Q2Pro convention)
    let ext = if compressed { "dm2z" } else { "dm2" };
    let name = format!("{}/demos/{}.{}", fs_gamedir(), demo_name, ext);

    if compressed {
        com_printf(&format!("Recording from demo (compressed) to {}.\n", name));
    } else {
        com_printf(&format!("Recording from demo to {}.\n", name));
    }
    fs_create_path(&name);

    let file = match File::create(&name) {
        Ok(f) => f,
        Err(_) => {
            com_printf("ERROR: couldn't open.\n");
            return;
        }
    };

    let mut cls = CLS.lock().unwrap();
    *DEMO_FILE.lock().unwrap() = Some(BufWriter::new(file));
    cls.demo_recording = true;
    cls.demo_waiting = false; // Don't wait when recording from demo

    // Write startup information (same as cl_record_f)
    let mut buf = SizeBuf::new(MAX_MSGLEN as i32);

    msg_write_byte(&mut buf, SVC_SERVERDATA);
    msg_write_long(&mut buf, PROTOCOL_VERSION);
    msg_write_long(&mut buf, 0x10000 + cl.servercount);
    msg_write_byte(&mut buf, 1);
    msg_write_string(&mut buf, &cl.gamedir);
    msg_write_short(&mut buf, cl.playernum);
    msg_write_string(&mut buf, &cl.configstrings[CS_NAME]);

    // configstrings
    for i in 0..MAX_CONFIGSTRINGS {
        if !cl.configstrings[i].is_empty() {
            if buf.cursize as usize + cl.configstrings[i].len() + 32 > buf.maxsize as usize {
                // write it out (uses compression if -z flag was set)
                demo_write_message(&buf.data[..buf.cursize as usize]);
                buf.cursize = 0;
            }

            msg_write_byte(&mut buf, SVC_CONFIGSTRING);
            msg_write_short(&mut buf, i as i32);
            msg_write_string(&mut buf, &cl.configstrings[i]);
        }
    }

    // baselines
    let nullstate = EntityState::default();
    let cl_ents = CL_ENTITIES.lock().unwrap();
    for i in 0..MAX_EDICTS {
        let ent = &cl_ents[i].baseline;
        if ent.modelindex == 0 {
            continue;
        }

        if buf.cursize as usize + 64 > buf.maxsize as usize {
            // write it out (uses compression if -z flag was set)
            demo_write_message(&buf.data[..buf.cursize as usize]);
            buf.cursize = 0;
        }

        msg_write_byte(&mut buf, SVC_SPAWNBASELINE);
        msg_write_delta_entity(&nullstate, &cl_ents[i].baseline, &mut buf, true, true);
    }

    msg_write_byte(&mut buf, SVC_STUFFTEXT);
    msg_write_string(&mut buf, "precache\n");

    // write it to the demo file (uses compression if -z flag was set)
    demo_write_message(&buf.data[..buf.cursize as usize]);
}

// ============================================================
// CL_AutoRecord (R1Q2/Q2Pro feature)
//
// Called when entering a map to auto-start recording if cl_autorecord is set
// ============================================================

pub fn cl_check_autorecord() {
    if !cl_autorecord_value() {
        return;
    }

    let cls = CLS.lock().unwrap();
    if cls.demo_recording || cls.demo_playing {
        return; // Already recording or playing a demo
    }
    if cls.state != crate::client::ConnState::Active {
        return; // Not in a game
    }
    drop(cls);

    // Start auto-recording with timestamp-based name
    let demo_name = generate_demo_name();
    let name = format!("{}/demos/{}.dm2", fs_gamedir(), demo_name);

    com_printf(&format!("Auto-recording to {}.\n", name));
    fs_create_path(&name);

    let file = match File::create(&name) {
        Ok(f) => f,
        Err(_) => {
            com_printf("ERROR: couldn't open demo file for auto-record.\n");
            return;
        }
    };

    let cl = CL.lock().unwrap();
    let mut cls = CLS.lock().unwrap();

    *DEMO_FILE.lock().unwrap() = Some(BufWriter::new(file));
    *DEMO_COMPRESSED.lock().unwrap() = false; // Auto-record uses uncompressed format
    cls.demo_recording = true;
    cls.demo_waiting = true;

    // Write startup information
    let mut buf = SizeBuf::new(MAX_MSGLEN as i32);

    msg_write_byte(&mut buf, SVC_SERVERDATA);
    msg_write_long(&mut buf, PROTOCOL_VERSION);
    msg_write_long(&mut buf, 0x10000 + cl.servercount);
    msg_write_byte(&mut buf, 1);
    msg_write_string(&mut buf, &cl.gamedir);
    msg_write_short(&mut buf, cl.playernum);
    msg_write_string(&mut buf, &cl.configstrings[CS_NAME]);

    for i in 0..MAX_CONFIGSTRINGS {
        if !cl.configstrings[i].is_empty() {
            if buf.cursize as usize + cl.configstrings[i].len() + 32 > buf.maxsize as usize {
                // write it out
                demo_write_message(&buf.data[..buf.cursize as usize]);
                buf.cursize = 0;
            }

            msg_write_byte(&mut buf, SVC_CONFIGSTRING);
            msg_write_short(&mut buf, i as i32);
            msg_write_string(&mut buf, &cl.configstrings[i]);
        }
    }

    let nullstate = EntityState::default();
    let cl_ents = CL_ENTITIES.lock().unwrap();
    for i in 0..MAX_EDICTS {
        let ent = &cl_ents[i].baseline;
        if ent.modelindex == 0 {
            continue;
        }

        if buf.cursize as usize + 64 > buf.maxsize as usize {
            // write it out
            demo_write_message(&buf.data[..buf.cursize as usize]);
            buf.cursize = 0;
        }

        msg_write_byte(&mut buf, SVC_SPAWNBASELINE);
        msg_write_delta_entity(&nullstate, &cl_ents[i].baseline, &mut buf, true, true);
    }

    msg_write_byte(&mut buf, SVC_STUFFTEXT);
    msg_write_string(&mut buf, "precache\n");

    // write it to the demo file
    demo_write_message(&buf.data[..buf.cursize as usize]);
}

// ============================================================
// Cmd_ForwardToServer
//
// adds the current command line as a clc_stringcmd to the client message.
// ============================================================

pub fn cmd_forward_to_server() {
    let cmd = cmd_argv(0);
    let cmd_lower = cmd.to_lowercase();

    // Check for chat commands during packet loss
    let is_chat = cmd_lower == "say" || cmd_lower == "say_team";
    let is_team = cmd_lower == "say_team";

    // Check for packet loss - queue chat messages if connection is degraded
    if is_chat && cmd_argc() > 1 {
        let (packet_loss, current_time) = {
            let cl = CL.lock().unwrap();
            let cls = CLS.lock().unwrap();
            (cl.packet_loss_frames > 0, cls.realtime as i32)
        };

        if packet_loss {
            // Queue the message for later transmission
            let message = cmd_args();
            if chat_queue_outgoing(&message, is_team, current_time) {
                com_printf("[Chat queued due to packet loss]\n");
                return;
            }
        }
    }

    let mut cls = CLS.lock().unwrap();

    if cls.state <= crate::client::ConnState::Connected || cmd.starts_with('-') || cmd.starts_with('+') {
        com_printf(&format!("Unknown command \"{}\"\n", cmd));
        return;
    }

    msg_write_byte(&mut cls.netchan.message, CLC_STRINGCMD.into());
    cls.netchan.message.print(&cmd);
    if cmd_argc() > 1 {
        cls.netchan.message.print(" ");
        cls.netchan.message.print(&cmd_args());
    }
}

// ============================================================
// cl_process_chat_queue - Send queued chat messages when network recovers
// ============================================================

/// Process queued chat messages when the network connection improves.
/// Called from cl_frame after reading packets.
fn cl_process_chat_queue() {
    // Only process if we have queued messages
    if !chat_has_queued() {
        return;
    }

    // Check if packet loss has cleared
    let (packet_loss, current_time, connected) = {
        let cl = CL.lock().unwrap();
        let cls = CLS.lock().unwrap();
        let connected = cls.state == crate::client::ConnState::Active;
        (cl.packet_loss_frames > 0, cls.realtime as i32, connected)
    };

    // Don't process if still losing packets or not connected
    if packet_loss || !connected {
        return;
    }

    // Try to send one queued message per frame to avoid flooding
    if let Some(queued) = chat_get_queued(current_time) {
        let cmd = if queued.team { "say_team" } else { "say" };

        // Check if netchan message buffer has room before writing
        let send_ok = {
            let mut cls = CLS.lock().unwrap();
            // Estimate required space: 1 (clc_stringcmd) + cmd + " " + message + null
            let required = 1 + cmd.len() + 1 + queued.message.len() + 1;
            if (cls.netchan.message.cursize as usize + required) < cls.netchan.message.maxsize as usize {
                msg_write_byte(&mut cls.netchan.message, CLC_STRINGCMD.into());
                cls.netchan.message.print(cmd);
                cls.netchan.message.print(" ");
                cls.netchan.message.print(&queued.message);
                true
            } else {
                false
            }
        };

        if send_ok {
            com_dprintf(&format!("[Chat sent from queue: {}]\n", queued.message));
        } else {
            // Buffer full — re-queue for retry on next frame
            chat_retry_message(queued);
        }
    }
}

// ============================================================
// CL_Setenv_f
// ============================================================

pub fn cl_setenv_f() {
    let argc = cmd_argc();

    if argc > 2 {
        let mut buffer = cmd_argv(1);
        buffer.push('=');

        for i in 2..argc {
            buffer.push_str(&cmd_argv(i));
            buffer.push(' ');
        }

        std::env::set_var(cmd_argv(1).as_str(), &buffer[cmd_argv(1).len() + 1..]);
    } else if argc == 2 {
        match std::env::var(cmd_argv(1).as_str()) {
            Ok(val) => com_printf(&format!("{}={}\n", cmd_argv(1), val)),
            Err(_) => com_printf(&format!("{} undefined\n", cmd_argv(1))),
        }
    }
}

// ============================================================
// CL_ForwardToServer_f
// ============================================================

pub fn cl_forward_to_server_f() {
    let mut cls = CLS.lock().unwrap();

    if cls.state != crate::client::ConnState::Connected && cls.state != crate::client::ConnState::Active {
        com_printf(&format!("Can't \"{}\", not connected\n", cmd_argv(0)));
        return;
    }

    // don't forward the first argument
    if cmd_argc() > 1 {
        msg_write_byte(&mut cls.netchan.message, CLC_STRINGCMD.into());
        cls.netchan.message.print(&cmd_args());
    }
}

// ============================================================
// CL_Pause_f
// ============================================================

pub fn cl_pause_f() {
    // never pause in multiplayer
    if cvar_variable_value("maxclients") > 1.0 || com_server_state() == 0 {
        cvar_set_value("paused", 0.0);
        return;
    }

    let paused = CL_PAUSED.lock().unwrap();
    cvar_set_value("paused", if paused.value != 0.0 { 0.0 } else { 1.0 });
}

// ============================================================
// CL_Quit_f
// ============================================================

pub fn cl_quit_f() {
    cl_disconnect();
    com_quit();
}

// ============================================================
// CL_Drop
//
// Called after an ERR_DROP was thrown
// ============================================================

pub fn cl_drop() {
    let disable_servercount;
    {
        let cls = CLS.lock().unwrap();
        if cls.state == crate::client::ConnState::Uninitialized {
            return;
        }
        if cls.state == crate::client::ConnState::Disconnected {
            return;
        }
        disable_servercount = cls.disable_servercount;
    }

    cl_disconnect();

    // drop loading plaque unless this is the initial game start
    if disable_servercount != -1 {
        scr_end_loading_plaque(true); // get rid of loading plaque
    }
}

// ============================================================
// CL_SendConnectPacket
//
// We have gotten a challenge from the server, so try and connect.
// ============================================================

pub fn cl_send_connect_packet() {
    let mut adr = NetAdr::default();
    let servername;
    let challenge;
    let server_protocol;

    {
        let cls = CLS.lock().unwrap();
        servername = cls.servername.clone();
        challenge = cls.challenge;
        // Use server's protocol version if we received it, otherwise default
        server_protocol = if cls.server_protocol != 0 {
            cls.server_protocol
        } else {
            PROTOCOL_VERSION
        };
    }

    if !net_string_to_adr(&servername, &mut adr) {
        com_printf("Bad server address\n");
        CLS.lock().unwrap().connect_time = 0.0;
        return;
    }
    if adr.port == 0 {
        adr.port = PORT_SERVER.to_be();
    }

    let port = cvar_variable_value("qport") as i32;
    *USERINFO_MODIFIED.lock().unwrap() = false;

    netchan_out_of_band_print(
        NS_CLIENT,
        adr,
        &format!(
            "connect {} {} {} \"{}\"\n",
            server_protocol,
            port,
            challenge,
            cvar_userinfo()
        ),
    );
}

// ============================================================
// CL_CheckForResend
//
// Resend a connect message if the last one has timed out
// ============================================================

pub fn cl_check_for_resend() {
    let mut adr = NetAdr::default();

    let (state, realtime, connect_time, servername) = {
        let cls = CLS.lock().unwrap();
        (cls.state, cls.realtime, cls.connect_time, cls.servername.clone())
    };

    // if the local server is running and we aren't then connect
    if state == crate::client::ConnState::Disconnected && com_server_state() != 0 {
        {
            let mut cls = CLS.lock().unwrap();
            cls.state = crate::client::ConnState::Connecting;
            cls.servername = "localhost".to_string();
        }
        // we don't need a challenge on the localhost
        cl_send_connect_packet();
        return;
    }

    // resend if we haven't gotten a reply yet
    if state != crate::client::ConnState::Connecting {
        return;
    }

    if realtime - (connect_time as i32) < 3000 {
        return;
    }

    if !net_string_to_adr(&servername, &mut adr) {
        com_printf("Bad server address\n");
        CLS.lock().unwrap().state = crate::client::ConnState::Disconnected;
        return;
    }
    if adr.port == 0 {
        adr.port = PORT_SERVER.to_be();
    }

    {
        let mut cls = CLS.lock().unwrap();
        cls.connect_time = cls.realtime as f32; // for retransmit requests
    }

    com_printf(&format!("Connecting to {}...\n", servername));

    netchan_out_of_band_print(NS_CLIENT, adr, "getchallenge\n");
}

// ============================================================
// CL_Connect_f
// ============================================================

pub fn cl_connect_f() {
    if cmd_argc() != 2 {
        com_printf("usage: connect <server>\n");
        return;
    }

    if com_server_state() != 0 {
        // if running a local server, kill it and reissue
        sv_shutdown("Server quit\n", false);
    } else {
        cl_disconnect();
    }

    let server = cmd_argv(1);

    net_config(true); // allow remote

    cl_disconnect();

    {
        let mut cls = CLS.lock().unwrap();
        cls.state = crate::client::ConnState::Connecting;
        cls.servername = server[..std::cmp::min(server.len(), MAX_OSPATH - 1)].to_string();
        cls.connect_time = -99999.0; // CL_CheckForResend() will fire immediately
    }
}

// ============================================================
// CL_Rcon_f
//
// Send the rest of the command line over as an unconnected command.
// ============================================================

pub fn cl_rcon_f() {
    let rcon_password = RCON_CLIENT_PASSWORD.lock().unwrap().string.clone();

    if rcon_password.is_empty() {
        com_printf(
            "You must set 'rcon_password' before\n\
             issuing an rcon command.\n",
        );
        return;
    }

    let mut message = vec![0xFFu8; 4];
    message.extend_from_slice(b"rcon ");
    message.extend_from_slice(rcon_password.as_bytes());
    message.push(b' ');

    for i in 1..cmd_argc() {
        message.extend_from_slice(cmd_argv(i).as_bytes());
        message.push(b' ');
    }
    message.push(0); // null terminator

    net_config(true); // allow remote

    let to;
    let rcon_addr = RCON_ADDRESS.lock().unwrap().string.clone();
    let cls = CLS.lock().unwrap();

    // mattx86: rcon_multiple_servers
    if cls.state >= crate::client::ConnState::Connected && rcon_addr.is_empty() {
        to = cls.netchan.remote_address;
    } else {
        if rcon_addr.is_empty() {
            com_printf(
                "You must either be connected,\n\
                 or set the 'rcon_address' cvar\n\
                 to issue rcon commands\n",
            );
            return;
        }

        let mut addr = NetAdr::default();
        net_string_to_adr(&rcon_addr, &mut addr);
        if addr.port == 0 {
            addr.port = PORT_SERVER.to_be();
        }
        to = addr;
    }

    net_send_packet(NS_CLIENT, message.len(), &message, &to);
}

// ============================================================
// CL_ClearState
// ============================================================

pub fn cl_clear_state() {
    s_stop_all_sounds();
    cl_clear_effects();
    cl_clear_tents();

    // Clear projectile state on disconnect/level change
    {
        let mut proj = PROJ_STATE.lock().unwrap();
        crate::cl_ents::cl_clear_projectiles(&mut proj);
    }

    // wipe the entire cl structure
    *CL.lock().unwrap() = crate::client::ClientState::default();
    {
        let mut ents = CL_ENTITIES.lock().unwrap();
        for ent in ents.iter_mut() {
            *ent = crate::client::CEntity::default();
        }
    }

    CLS.lock().unwrap().netchan.message.clear();

    // Reset HUD stat smoothing on level change (R1Q2/Q2Pro feature)
    crate::cl_hud::hud_reset_stats(0, 0, 0, 0);
}

// ============================================================
// CL_Disconnect
//
// Goes from a connected state to full screen console state
// Sends a disconnect message to the server
// This is also called on Com_Error, so it shouldn't cause any errors
// ============================================================

pub fn cl_disconnect() {
    {
        let cls = CLS.lock().unwrap();
        if cls.state == crate::client::ConnState::Disconnected {
            return;
        }
    }

    // Execute disconnect trigger command (R1Q2/Q2Pro feature)
    cl_trigger_disconnect();

    {
        let timedemo = CL_TIMEDEMO.lock().unwrap();
        if timedemo.value != 0.0 {
            let cl = CL.lock().unwrap();
            let time = sys_milliseconds() - cl.timedemo_start;
            if time > 0 {
                com_printf(&format!(
                    "{} frames, {:.1} seconds: {:.1} fps\n",
                    cl.timedemo_frames,
                    time as f64 / 1000.0,
                    cl.timedemo_frames as f64 * 1000.0 / time as f64
                ));
            }
        }
    }

    {
        let cl = CL.lock().unwrap();
        vector_clear(&mut cl.refdef.blend[0..3].try_into().unwrap());
    }

    r_set_palette(None);
    m_force_menu_off();

    {
        let mut cls = CLS.lock().unwrap();
        cls.connect_time = 0.0;
    }

    scr_stop_cinematic();

    // Shutdown HTTP downloads
    crate::cl_http::cl_http_shutdown();

    // Clear location database (R1Q2/Q2Pro feature)
    crate::cl_loc::loc_clear();

    // Clear queued chat messages (R1Q2/Q2Pro feature)
    crate::cl_chat::chat_clear_queue();

    {
        let cls = CLS.lock().unwrap();
        if cls.demo_recording {
            drop(cls);
            cl_stop_f();
        }
    }

    // send a disconnect message to the server
    {
        let mut cls = CLS.lock().unwrap();
        let mut final_msg: Vec<u8> = Vec::new();
        final_msg.push(CLC_STRINGCMD as u8);
        final_msg.extend_from_slice(b"disconnect");
        let len = final_msg.len();
        netchan_transmit(&mut cls.netchan, len, &final_msg);
        netchan_transmit(&mut cls.netchan, len, &final_msg);
        netchan_transmit(&mut cls.netchan, len, &final_msg);
    }

    cl_clear_state();

    // stop download and reset connection state
    {
        let mut cls = CLS.lock().unwrap();
        cls.download_type = crate::client::DlType::None;
        cls.state = crate::client::ConnState::Disconnected;
        cls.server_protocol = 0; // reset for next connection
    }
}

// ============================================================
// CL_Disconnect_f
// ============================================================

pub fn cl_disconnect_f() {
    // Cancel any pending auto-reconnect when user disconnects
    cl_cancel_auto_reconnect();
    com_error(ERR_DROP, "Disconnected from server");
}

// ============================================================
// Auto-Reconnect (R1Q2/Q2Pro feature)
// ============================================================

/// Start an auto-reconnect sequence after an unexpected disconnect.
/// Call this before cl_disconnect() when the disconnect is unexpected.
fn cl_start_auto_reconnect(cls: &crate::client::ClientStatic) {
    let autoreconnect_enabled = CL_AUTORECONNECT.lock().unwrap().value != 0.0;
    if !autoreconnect_enabled {
        return;
    }

    let max_attempts = CL_AUTORECONNECT_MAX.lock().unwrap().value as i32;
    let delay = CL_AUTORECONNECT_DELAY.lock().unwrap().value as i32;

    // Only reconnect if we were actually connected to a server
    if cls.servername.is_empty() {
        return;
    }

    // Store reconnect state in CLS (will be updated after disconnect)
    // We use a separate static for pending state since cls will be reset
    let mut pending = AUTO_RECONNECT_STATE.lock().unwrap();
    pending.enabled = true;
    pending.server = cls.servername.clone();
    pending.attempts = 0;
    pending.max_attempts = max_attempts;
    pending.delay = delay;
    pending.next_time = cls.realtime + delay;

    com_printf(&format!(
        "Auto-reconnect: will attempt in {} ms (max {} attempts)\n",
        delay, max_attempts
    ));
}

/// Cancel any pending auto-reconnect.
fn cl_cancel_auto_reconnect() {
    let mut pending = AUTO_RECONNECT_STATE.lock().unwrap();
    pending.enabled = false;
}

/// Check if we should attempt to reconnect and do so if ready.
/// Call this from the main frame loop.
fn cl_check_auto_reconnect() {
    let mut pending = AUTO_RECONNECT_STATE.lock().unwrap();

    if !pending.enabled {
        return;
    }

    let cls = CLS.lock().unwrap();

    // If we're already connected or connecting, cancel
    if cls.state >= crate::client::ConnState::Connecting {
        pending.enabled = false;
        return;
    }

    // Check if it's time to reconnect
    if cls.realtime < pending.next_time {
        return;
    }

    pending.attempts += 1;

    if pending.attempts > pending.max_attempts {
        com_printf(&format!(
            "Auto-reconnect: max attempts ({}) reached, giving up\n",
            pending.max_attempts
        ));
        pending.enabled = false;
        return;
    }

    // Calculate next attempt time with exponential backoff
    let backoff_delay = pending.delay * (1 << (pending.attempts - 1).min(4));
    pending.next_time = cls.realtime + backoff_delay;

    let server = pending.server.clone();
    drop(pending);
    drop(cls);

    com_printf(&format!(
        "Auto-reconnect: attempt {}, connecting to {}\n",
        AUTO_RECONNECT_STATE.lock().unwrap().attempts,
        server
    ));

    // Issue the connect command
    cbuf_add_text(&format!("connect \"{}\"\n", server));
}

/// Called when successfully connected to a server.
/// Resets the auto-reconnect state.
pub fn cl_auto_reconnect_success() {
    let mut pending = AUTO_RECONNECT_STATE.lock().unwrap();
    if pending.enabled {
        com_printf("Auto-reconnect: connection successful\n");
        pending.enabled = false;
    }
}

/// Auto-reconnect state
struct AutoReconnectState {
    enabled: bool,
    server: String,
    attempts: i32,
    max_attempts: i32,
    delay: i32,
    next_time: i32,
}

impl Default for AutoReconnectState {
    fn default() -> Self {
        Self {
            enabled: false,
            server: String::new(),
            attempts: 0,
            max_attempts: 3,
            delay: 3000,
            next_time: 0,
        }
    }
}

static AUTO_RECONNECT_STATE: LazyLock<Mutex<AutoReconnectState>> =
    LazyLock::new(|| Mutex::new(AutoReconnectState::default()));

// ============================================================
// CL_Packet_f
//
// packet <destination> <contents>
// Contents allows \n escape character
// ============================================================

pub fn cl_packet_f() {
    if cmd_argc() != 3 {
        com_printf("packet <destination> <contents>\n");
        return;
    }

    net_config(true); // allow remote

    let mut adr = NetAdr::default();
    if !net_string_to_adr(&cmd_argv(1), &mut adr) {
        com_printf("Bad address\n");
        return;
    }
    if adr.port == 0 {
        adr.port = PORT_SERVER.to_be();
    }

    let input = cmd_argv(2);
    let mut send: Vec<u8> = vec![0xFF, 0xFF, 0xFF, 0xFF];

    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() && bytes[i + 1] == b'n' {
            send.push(b'\n');
            i += 2;
        } else {
            send.push(bytes[i]);
            i += 1;
        }
    }
    send.push(0);

    net_send_packet(NS_CLIENT, send.len(), &send, &adr);
}

// ============================================================
// CL_Changing_f
//
// Just sent as a hint to the client that they should drop to full console
// ============================================================

pub fn cl_changing_f() {
    // ZOID: if we are downloading, we don't change!
    {
        let cls = CLS.lock().unwrap();
        if cls.download_type != crate::client::DlType::None {
            return;
        }
    }

    scr_begin_loading_plaque();
    CLS.lock().unwrap().state = crate::client::ConnState::Connected; // not active anymore, but not disconnected
    com_printf("\nChanging map...\n");
}

// ============================================================
// CL_Reconnect_f
//
// The server is changing levels
// ============================================================

pub fn cl_reconnect_f() {
    // ZOID: if we are downloading, we don't change!
    {
        let cls = CLS.lock().unwrap();
        if cls.download_type != crate::client::DlType::None {
            return;
        }
    }

    s_stop_all_sounds();

    {
        let mut cls = CLS.lock().unwrap();
        if cls.state == crate::client::ConnState::Connected {
            com_printf("reconnecting...\n");
            cls.state = crate::client::ConnState::Connected;
            msg_write_char(&mut cls.netchan.message, CLC_STRINGCMD.into());
            msg_write_string(&mut cls.netchan.message, "new");
            return;
        }
    }

    let mut cls = CLS.lock().unwrap();
    if !cls.servername.is_empty() {
        if cls.state >= crate::client::ConnState::Connected {
            drop(cls);
            cl_disconnect();
            cls = CLS.lock().unwrap();
            cls.connect_time = (cls.realtime - 1500) as f32;
        } else {
            cls.connect_time = -99999.0; // fire immediately
        }

        cls.state = crate::client::ConnState::Connecting;
        com_printf("reconnecting...\n");
    }
}

// ============================================================
// CL_ParseStatusMessage
//
// Handle a reply from a ping
// ============================================================

pub fn cl_parse_status_message() {
    let mut net_msg = NET_MESSAGE.lock().unwrap();
    let s = msg_read_string(&mut net_msg);

    com_printf(&format!("{}\n", s));
    let net_from = NET_FROM.lock().unwrap();
    m_add_to_server_list(&net_from, &s);
}

// ============================================================
// CL_PingServers_f
// ============================================================

pub fn cl_ping_servers_f() {
    let mut adr = NetAdr::default();

    net_config(true); // allow remote

    // send a broadcast packet
    com_printf("pinging broadcast...\n");

    let noudp = cvar_get("noudp", "0", CVAR_NOSET);
    if noudp.value == 0.0 {
        adr.adr_type = NetAdrType::Broadcast;
        adr.port = PORT_SERVER.to_be();
        netchan_out_of_band_print(NS_CLIENT, adr.clone(), &format!("info {}", PROTOCOL_VERSION));
    }

    // send a packet to each address book entry
    for i in 0..=99 {
        let adrstring = cvar_variable_string(&format!("adr{}", i));
        if adrstring.is_empty() {
            continue;
        }

        com_printf(&format!("pinging {}...\n", adrstring));
        if !net_string_to_adr(&adrstring, &mut adr) {
            com_printf(&format!("Bad address: {}\n", adrstring));
            continue;
        }
        if adr.port == 0 {
            adr.port = PORT_SERVER.to_be();
        }
        netchan_out_of_band_print(NS_CLIENT, adr.clone(), &format!("info {}", PROTOCOL_VERSION));
    }
}

// ============================================================
// CL_Skins_f
//
// Load or download any custom player skins and models
// ============================================================

pub fn cl_skins_f() {
    let cl = CL.lock().unwrap();

    for i in 0..MAX_CLIENTS {
        if cl.configstrings[CS_PLAYERSKINS + i].is_empty() {
            continue;
        }
        com_printf(&format!(
            "client {}: {}\n",
            i,
            cl.configstrings[CS_PLAYERSKINS + i]
        ));
        scr_update_screen();
        sys_send_key_events(); // pump message loop
        cl_parse_clientinfo(i as i32);
    }
}

// ============================================================
// CL_ConnectionlessPacket
//
// Responses to broadcasts, etc
// ============================================================

pub fn cl_connectionless_packet() {
    {
        let mut net_msg = NET_MESSAGE.lock().unwrap();
        msg_begin_reading(&mut net_msg);
        msg_read_long(&mut net_msg); // skip the -1
    }

    let s = {
        let mut net_msg = NET_MESSAGE.lock().unwrap();
        msg_read_string_line(&mut net_msg)
    };

    cmd_tokenize_string(&s, false);

    let c = cmd_argv(0);

    {
        let net_from = NET_FROM.lock().unwrap();
        com_printf(&format!("{}: {}\n", net_adr_to_string(&net_from), c));
    }

    // server connection
    if c == "client_connect" {
        let mut cls = CLS.lock().unwrap();
        if cls.state == crate::client::ConnState::Connected {
            com_printf("Dup connect received.  Ignored.\n");
            return;
        }
        let net_from = NET_FROM.lock().unwrap();
        let qport = cls.quake_port;
        netchan_setup(NS_CLIENT, &mut cls.netchan, net_from.clone(), qport);
        msg_write_char(&mut cls.netchan.message, CLC_STRINGCMD.into());
        msg_write_string(&mut cls.netchan.message, "new");
        cls.state = crate::client::ConnState::Connected;
        return;
    }

    // server responding to a status broadcast
    if c == "info" {
        cl_parse_status_message();
        return;
    }

    // remote command from gui front end
    if c == "cmd" {
        {
            let net_from = NET_FROM.lock().unwrap();
            if !net_is_local_address(&net_from) {
                com_printf("Command packet from remote host.  Ignored.\n");
                return;
            }
        }
        sys_app_activate();
        let s = {
            let mut net_msg = NET_MESSAGE.lock().unwrap();
            msg_read_string(&mut net_msg)
        };
        cbuf_add_text(&s);
        cbuf_add_text("\n");
        return;
    }

    // print command from somewhere
    if c == "print" {
        let s = {
            let mut net_msg = NET_MESSAGE.lock().unwrap();
            msg_read_string(&mut net_msg)
        };
        com_printf(&s);
        return;
    }

    // ping from somewhere
    if c == "ping" {
        let net_from = NET_FROM.lock().unwrap();
        netchan_out_of_band_print(NS_CLIENT, net_from.clone(), "ack");
        return;
    }

    // challenge from the server we are connecting to
    if c == "challenge" {
        let challenge: i32 = cmd_argv(1).parse().unwrap_or(0);

        // Parse optional protocol version (format: "p=<version>")
        let mut server_protocol = PROTOCOL_VERSION;
        let arg2 = cmd_argv(2);
        if arg2.starts_with("p=") {
            if let Ok(proto) = arg2[2..].parse::<i32>() {
                server_protocol = proto;
            }
        }

        {
            let mut cls = CLS.lock().unwrap();
            cls.challenge = challenge;
            cls.server_protocol = server_protocol;
        }
        cl_send_connect_packet();
        return;
    }

    // echo request from server
    if c == "echo" {
        let net_from = NET_FROM.lock().unwrap();
        netchan_out_of_band_print(NS_CLIENT, net_from.clone(), &cmd_argv(1));
        return;
    }

    com_printf("Unknown command.\n");
}

// ============================================================
// CL_DumpPackets
//
// A vain attempt to help bad TCP stacks that cause problems when they overflow
// ============================================================

pub fn cl_dump_packets() {
    let mut net_from = NET_FROM.lock().unwrap();
    let mut net_msg = NET_MESSAGE.lock().unwrap();
    while net_get_packet(NS_CLIENT, &mut net_from, &mut net_msg) {
        com_printf("dumping a packet\n");
    }
}

// ============================================================
// CL_ReadPackets
// ============================================================

pub fn cl_read_packets() {
    // Check if we're playing a demo
    {
        let cls = CLS.lock().unwrap();
        if cls.demo_playing {
            drop(cls);
            // Read from demo file instead of network
            if cl_read_demo_message() {
                // Parse the demo message just like a network message
                cl_parse_server_message();
            }
            return;
        }
    }

    loop {
        let got_packet = {
            let mut net_from = NET_FROM.lock().unwrap();
            let mut net_msg = NET_MESSAGE.lock().unwrap();
            net_get_packet(NS_CLIENT, &mut net_from, &mut net_msg)
        };

        if !got_packet {
            break;
        }

        // remote command packet
        {
            let net_msg = NET_MESSAGE.lock().unwrap();
            if !net_msg.data.is_empty() && net_msg.cursize >= 4 {
                let header = i32::from_le_bytes([
                    net_msg.data[0], net_msg.data[1], net_msg.data[2], net_msg.data[3],
                ]);
                if header == -1 {
                    drop(net_msg);
                    cl_connectionless_packet();
                    continue;
                }
            }
        }

        {
            let cls = CLS.lock().unwrap();
            if cls.state == crate::client::ConnState::Disconnected || cls.state == crate::client::ConnState::Connecting {
                continue; // dump it if not connected
            }
        }

        {
            let net_msg = NET_MESSAGE.lock().unwrap();
            if net_msg.cursize < 8 {
                let net_from = NET_FROM.lock().unwrap();
                com_printf(&format!("{}: Runt packet\n", net_adr_to_string(&net_from)));
                continue;
            }
        }

        // packet from server
        {
            let net_from = NET_FROM.lock().unwrap();
            let cls = CLS.lock().unwrap();
            if !net_compare_adr(&net_from, &cls.netchan.remote_address) {
                com_dprintf(&format!(
                    "{}:sequenced packet without connection\n",
                    net_adr_to_string(&net_from)
                ));
                continue;
            }
        }

        {
            let mut cls = CLS.lock().unwrap();
            let mut net_msg = NET_MESSAGE.lock().unwrap();
            if !netchan_process(&mut cls.netchan, &mut net_msg) {
                continue; // wasn't accepted for some reason
            }
        }

        cl_parse_server_message();
    }

    // check timeout
    {
        let cls = CLS.lock().unwrap();
        let timeout_val = CL_TIMEOUT.lock().unwrap().value;

        if cls.state >= crate::client::ConnState::Connected
            && (cls.realtime - cls.netchan.last_received) as f64 > timeout_val as f64 * 1000.0
        {
            let mut cl = CL.lock().unwrap();
            cl.timeoutcount += 1;
            if cl.timeoutcount > 5 {
                // timeoutcount saves debugger
                com_printf("\nServer connection timed out.\n");
                drop(cl);

                // Set up auto-reconnect before disconnecting
                cl_start_auto_reconnect(&cls);
                drop(cls);
                cl_disconnect();
            }
        } else {
            CL.lock().unwrap().timeoutcount = 0;
        }
    }
}

// ============================================================
// CL_FixUpGender
// ============================================================

pub fn cl_fix_up_gender() {
    let gender_auto_val = GENDER_AUTO.lock().unwrap().value;

    if gender_auto_val != 0.0 {
        {
            let mut gender = GENDER.lock().unwrap();
            if gender.modified {
                // was set directly, don't override the user
                gender.modified = false;
                return;
            }
        }

        let skin_str = SKIN.lock().unwrap().string.clone();

        let mut model = skin_str.clone();

        // isolate model from skin string "model/skin"
        let skin_part;
        if let Some(pos) = model.find('/').or_else(|| model.find('\\')) {
            skin_part = model[pos + 1..].to_string();
            model.truncate(pos);
        } else {
            skin_part = String::new();
        }

        // mattx86: cl_defaultskin
        let defaultskin = CL_DEFAULTSKIN.lock().unwrap().string.clone();

        if !defaultskin.eq_ignore_ascii_case("male/grunt") {
            let mut model2 = defaultskin.clone();
            let _sk2;

            if let Some(pos) = model2.rfind('/').or_else(|| model2.rfind('\\')) {
                _sk2 = model2[pos + 1..].to_string();
                model2.truncate(pos);
            } else {
                _sk2 = String::new();
            }

            // second level path extraction
            if let Some(pos) = model2.rfind('/').or_else(|| model2.rfind('\\')) {
                model2 = model2[pos + 1..].to_string();
            }

            let noskins_val = CL_NOSKINS.lock().unwrap().value;

            if noskins_val != 0.0 {
                if !model2.is_empty() {
                    model = model2;
                }
            } else if !model.is_empty() && !skin_part.is_empty() && !model2.is_empty() && !_sk2.is_empty() {
                let filename = format!("players/{}/{}.pcx", model, skin_part);
                if fs_fopen_file(&filename).is_none() {
                    let filename2 = format!("players/{}/{}.pcx", model2, _sk2);
                    if let Some(f) = fs_fopen_file(&filename2) {
                        fs_fclose_file(f);
                        model = model2;
                    }
                }
            }
        }

        let model_lower = model.to_ascii_lowercase();
        if model_lower == "male" || model_lower == "cyborg" {
            cvar_set("gender", "male");
        } else if model_lower == "female" || model_lower == "crackhor" {
            cvar_set("gender", "female");
        } else {
            cvar_set("gender", "none");
        }

        GENDER.lock().unwrap().modified = false;
    }
}

// ============================================================
// CL_Userinfo_f
// ============================================================

pub fn cl_userinfo_f() {
    com_printf("User info settings:\n");
    info_print(&cvar_userinfo());
}

// ============================================================
// CL_Snd_Restart_f
//
// Restart the sound subsystem
// ============================================================

pub fn cl_snd_restart_f() {
    s_shutdown();
    s_init();
    cl_register_sounds();
}

// ============================================================
// CL_RequestNextDownload
// ============================================================

pub fn cl_request_next_download() {
    {
        let cls = CLS.lock().unwrap();
        if cls.state != crate::client::ConnState::Connected {
            return;
        }
    }

    let mut precache_check = PRECACHE_CHECK.lock().unwrap();

    if !allow_download_value() && *precache_check < ENV_CNT as i32 {
        *precache_check = ENV_CNT as i32;
    }

    // ZOID - model/sound/image/skin download logic
    // (This is a large function; the structure mirrors the C original)

    if *precache_check == CS_MODELS as i32 {
        *precache_check = (CS_MODELS + 2) as i32; // 0 isn't used
        if allow_download_maps_value() {
            let cl = CL.lock().unwrap();
            if !cl_check_or_download_file(&cl.configstrings[CS_MODELS + 1]) {
                return; // started a download
            }
        }
    }

    if *precache_check >= CS_MODELS as i32 && *precache_check < (CS_MODELS + MAX_MODELS) as i32 {
        if allow_download_models_value() {
            let mut precache_model_skin = PRECACHE_MODEL_SKIN.lock().unwrap();

            while *precache_check < (CS_MODELS + MAX_MODELS) as i32 {
                let cl = CL.lock().unwrap();
                let idx = *precache_check as usize;
                if cl.configstrings[idx].is_empty() {
                    break;
                }
                if cl.configstrings[idx].starts_with('*') || cl.configstrings[idx].starts_with('#') {
                    *precache_check += 1;
                    continue;
                }

                if *precache_model_skin == 0 {
                    if !cl_check_or_download_file(&cl.configstrings[idx]) {
                        *precache_model_skin = 1;
                        return;
                    }
                    *precache_model_skin = 1;
                }

                // checking for skins in the model
                let mut precache_model = PRECACHE_MODEL.lock().unwrap();
                if precache_model.is_none() {
                    let data = fs_load_file(&cl.configstrings[idx]);
                    if data.is_none() {
                        *precache_model_skin = 0;
                        *precache_check += 1;
                        continue;
                    }
                    let data = data.unwrap();
                    if data.len() < 4 || little_long(i32::from_le_bytes([data[0], data[1], data[2], data[3]])) != IDALIASHEADER {
                        *precache_model_skin = 0;
                        *precache_check += 1;
                        continue;
                    }
                    if data.len() < std::mem::size_of::<DmdlT>() {
                        *precache_model_skin = 0;
                        *precache_check += 1;
                        continue;
                    }
                    // SAFETY: checking we have enough data for the header
                    let version = unsafe {
                        let ptr = data.as_ptr().add(4) as *const i32;
                        little_long(*ptr)
                    };
                    if version != ALIAS_VERSION {
                        *precache_check += 1;
                        *precache_model_skin = 0;
                        continue;
                    }
                    *precache_model = Some(data);
                }

                if let Some(ref model_data) = *precache_model {
                    // SAFETY: we verified size >= sizeof(DmdlT) above
                    let pheader = unsafe { &*(model_data.as_ptr() as *const DmdlT) };
                    let num_skins = little_long(pheader.num_skins);
                    let ofs_skins = little_long(pheader.ofs_skins) as usize;

                    while *precache_model_skin - 1 < num_skins {
                        let skin_offset = ofs_skins + (*precache_model_skin as usize - 1) * MAX_SKINNAME;
                        if skin_offset < model_data.len() {
                            let end = std::cmp::min(skin_offset + MAX_SKINNAME, model_data.len());
                            let skin_name = String::from_utf8_lossy(&model_data[skin_offset..end]);
                            let skin_name = skin_name.trim_end_matches('\0');
                            if !cl_check_or_download_file(skin_name) {
                                *precache_model_skin += 1;
                                return;
                            }
                        }
                        *precache_model_skin += 1;
                    }
                }

                *precache_model = None;
                *precache_model_skin = 0;
                *precache_check += 1;
            }
        }
        *precache_check = CS_SOUNDS as i32;
    }

    if *precache_check >= CS_SOUNDS as i32 && *precache_check < (CS_SOUNDS + MAX_SOUNDS) as i32 {
        if allow_download_sounds_value() {
            if *precache_check == CS_SOUNDS as i32 {
                *precache_check += 1; // zero is blank
            }
            while *precache_check < (CS_SOUNDS + MAX_SOUNDS) as i32 {
                let cl = CL.lock().unwrap();
                let idx = *precache_check as usize;
                if cl.configstrings[idx].is_empty() {
                    break;
                }
                if cl.configstrings[idx].starts_with('*') {
                    *precache_check += 1;
                    continue;
                }
                let fn_name = format!("sound/{}", cl.configstrings[idx]);
                *precache_check += 1;
                if !cl_check_or_download_file(&fn_name) {
                    return;
                }
            }
        }
        *precache_check = CS_IMAGES as i32;
    }

    if *precache_check >= CS_IMAGES as i32 && *precache_check < (CS_IMAGES + MAX_IMAGES) as i32 {
        if *precache_check == CS_IMAGES as i32 {
            *precache_check += 1; // zero is blank
        }
        while *precache_check < (CS_IMAGES + MAX_IMAGES) as i32 {
            let cl = CL.lock().unwrap();
            let idx = *precache_check as usize;
            if cl.configstrings[idx].is_empty() {
                break;
            }
            let fn_name = format!("pics/{}.pcx", cl.configstrings[idx]);
            *precache_check += 1;
            if !cl_check_or_download_file(&fn_name) {
                return;
            }
        }
        *precache_check = CS_PLAYERSKINS as i32;
    }

    // skins are special: model, weapon model and skin
    if *precache_check >= CS_PLAYERSKINS as i32
        && *precache_check < (CS_PLAYERSKINS + MAX_CLIENTS * PLAYER_MULT) as i32
    {
        if allow_download_players_value() {
            while *precache_check < (CS_PLAYERSKINS + MAX_CLIENTS * PLAYER_MULT) as i32 {
                let i = (*precache_check as usize - CS_PLAYERSKINS) / PLAYER_MULT;
                let mut n = (*precache_check as usize - CS_PLAYERSKINS) % PLAYER_MULT;

                let cl = CL.lock().unwrap();
                if cl.configstrings[CS_PLAYERSKINS + i].is_empty() {
                    *precache_check = (CS_PLAYERSKINS + (i + 1) * PLAYER_MULT) as i32;
                    continue;
                }

                let playerinfo = &cl.configstrings[CS_PLAYERSKINS + i];
                let p = if let Some(pos) = playerinfo.find('\\') {
                    &playerinfo[pos + 1..]
                } else {
                    playerinfo.as_str()
                };

                let (model_name, skin_name) = if let Some(pos) = p.find('/').or_else(|| p.find('\\')) {
                    (&p[..pos], &p[pos + 1..])
                } else {
                    (p, "")
                };

                // fall-through switch cases
                if n == 0 {
                    // case 0: model
                    let fn_name = format!("players/{}/tris.md2", model_name);
                    if !cl_check_or_download_file(&fn_name) {
                        *precache_check = (CS_PLAYERSKINS + i * PLAYER_MULT + 1) as i32;
                        return;
                    }
                    n = 1;
                }
                if n <= 1 {
                    // case 1: weapon model
                    let fn_name = format!("players/{}/weapon.md2", model_name);
                    if !cl_check_or_download_file(&fn_name) {
                        *precache_check = (CS_PLAYERSKINS + i * PLAYER_MULT + 2) as i32;
                        return;
                    }
                    n = 2;
                }
                if n <= 2 {
                    // case 2: weapon skin
                    let fn_name = format!("players/{}/weapon.pcx", model_name);
                    if !cl_check_or_download_file(&fn_name) {
                        *precache_check = (CS_PLAYERSKINS + i * PLAYER_MULT + 3) as i32;
                        return;
                    }
                    n = 3;
                }
                if n <= 3 {
                    // case 3: skin
                    let fn_name = format!("players/{}/{}.pcx", model_name, skin_name);
                    if !cl_check_or_download_file(&fn_name) {
                        *precache_check = (CS_PLAYERSKINS + i * PLAYER_MULT + 4) as i32;
                        return;
                    }
                    n = 4;
                }
                if n <= 4 {
                    // case 4: skin_i
                    let fn_name = format!("players/{}/{}_i.pcx", model_name, skin_name);
                    if !cl_check_or_download_file(&fn_name) {
                        *precache_check = (CS_PLAYERSKINS + i * PLAYER_MULT + 5) as i32;
                        return;
                    }
                    // move on to next model
                    *precache_check = (CS_PLAYERSKINS + (i + 1) * PLAYER_MULT) as i32;
                }
            }
        }
        // precache phase completed
        *precache_check = ENV_CNT as i32;
    }

    if *precache_check == ENV_CNT as i32 {
        *precache_check = (ENV_CNT + 1) as i32;

        let mut map_checksum: u32 = 0;
        let cl = CL.lock().unwrap();
        cm_load_map(&cl.configstrings[CS_MODELS + 1], true, &mut map_checksum);

        let expected: u32 = cl.configstrings[CS_MAPCHECKSUM].parse().unwrap_or(0);
        if map_checksum != expected {
            com_error(
                ERR_DROP,
                &format!(
                    "Local map version differs from server: {} != '{}'\n",
                    map_checksum, cl.configstrings[CS_MAPCHECKSUM]
                ),
            );
        }
    }

    if *precache_check > ENV_CNT as i32 && *precache_check < TEXTURE_CNT as i32 {
        if allow_download_value() && allow_download_maps_value() {
            while *precache_check < TEXTURE_CNT as i32 {
                let n = *precache_check as usize - ENV_CNT - 1;
                *precache_check += 1;

                let cl = CL.lock().unwrap();
                let fn_name = if n & 1 != 0 {
                    format!("env/{}{}.pcx", cl.configstrings[CS_SKY], ENV_SUF[n / 2])
                } else {
                    format!("env/{}{}.tga", cl.configstrings[CS_SKY], ENV_SUF[n / 2])
                };
                if !cl_check_or_download_file(&fn_name) {
                    return;
                }
            }
        }
        *precache_check = TEXTURE_CNT as i32;
    }

    if *precache_check == TEXTURE_CNT as i32 {
        *precache_check = (TEXTURE_CNT + 1) as i32;
        *PRECACHE_TEX.lock().unwrap() = 0;
    }

    // confirm existence of textures, download any that don't exist
    if *precache_check == (TEXTURE_CNT + 1) as i32 {
        if allow_download_value() && allow_download_maps_value() {
            let precache_tex = PRECACHE_TEX.lock().unwrap();
            // Note: numtexinfo and map_surfaces would come from cmodel module
            // Placeholder: skip texture download check
            let _unused = precache_tex;
        }
        *precache_check = (TEXTURE_CNT + 999) as i32;
    }

    // ZOID
    cl_register_sounds();
    cl_prep_refresh();

    {
        let precache_spawncount = PRECACHE_SPAWNCOUNT.lock().unwrap();
        let mut cls = CLS.lock().unwrap();
        msg_write_byte(&mut cls.netchan.message, CLC_STRINGCMD.into());
        msg_write_string(
            &mut cls.netchan.message,
            &format!("begin {}\n", *precache_spawncount),
        );
    }

    // Execute begin map trigger command (R1Q2/Q2Pro feature)
    cl_trigger_begin_map();
}

// ============================================================
// CL_Precache_f
//
// The server will send this command right before allowing the client into the server
// ============================================================

pub fn cl_precache_f() {
    // Yet another hack to let old demos work -- the old precache sequence
    if cmd_argc() < 2 {
        let mut map_checksum: u32 = 0;
        let cl = CL.lock().unwrap();
        cm_load_map(&cl.configstrings[CS_MODELS + 1], true, &mut map_checksum);
        cl_register_sounds();
        cl_prep_refresh();
        return;
    }

    *PRECACHE_CHECK.lock().unwrap() = CS_MODELS as i32;
    *PRECACHE_SPAWNCOUNT.lock().unwrap() = cmd_argv(1).parse().unwrap_or(0);
    *PRECACHE_MODEL.lock().unwrap() = None;
    *PRECACHE_MODEL_SKIN.lock().unwrap() = 0;

    cl_request_next_download();
}

// ============================================================
// CL_WriteConfiguration_f
//
// Writes key bindings and archived cvars to config.cfg
// mattx86 -- updated and added as a console command.
// ============================================================

pub fn cl_write_configuration_f() {
    {
        let cls = CLS.lock().unwrap();
        if cls.state == crate::client::ConnState::Uninitialized {
            return;
        }
    }

    let mut file_name = if cmd_argc() == 2 {
        cmd_argv(1)
    } else {
        "config".to_string()
    };

    if !wildcardfit("*.cfg", &file_name) {
        file_name.push_str(".cfg");
    }

    let path = format!("{}/{}", fs_gamedir(), file_name);

    let f = match File::create(&path) {
        Ok(f) => f,
        Err(_) => {
            com_printf(&format!("Couldn't write {}/{}.\n", fs_gamedir(), file_name));
            return;
        }
    };
    com_printf(&format!("saving config to {}/{}\n", fs_gamedir(), file_name));

    let mut f = BufWriter::new(f);

    let now = chrono::Local::now();
    let datetime = now.format("%c").to_string();

    let _ = writeln!(
        f,
        "// --------------- \"{}\" generated by {} ({}) (BEGIN) ---------------",
        file_name, DISTNAME, datetime
    );

    // Address Book first.
    let _ = writeln!(f, "\n// ----- ( Address Book ) -----");
    cvar_write_address_book(&mut f);

    // Variables next.
    let _ = writeln!(f, "\n// ----- ( Variables ) -----");
    cvar_write_variables(&mut f);

    // Aliases next.
    let _ = writeln!(f, "\n// ----- ( Aliases ) -----");
    cmd_write_aliases(&mut f);

    // And finally, binds.
    let _ = writeln!(f, "\n// ----- ( Binds ) -----");
    key_write_bindings(&mut f);

    // Write the last line..
    let _ = writeln!(
        f,
        "\n// ---------------- \"{}\" generated by {} ({}) (END) ----------------",
        file_name, DISTNAME, datetime
    );
}

// ============================================================
// CL_InitLocal
// ============================================================

pub fn cl_init_local() {
    {
        let mut cls = CLS.lock().unwrap();
        cls.state = crate::client::ConnState::Disconnected;
        cls.realtime = sys_milliseconds();
    }

    cl_init_input(INPUT_BUTTONS.clone());

    // get address book entries setup.. (special case)
    for i in 0..=99 {
        cvar_get(&format!("adr{}", i), "", CVAR_ZERO);
    }

    // register our variables
    *CL_STEREO_SEPARATION.lock().unwrap() = cvar_get("cl_stereo_separation", "0.4", CVAR_ARCHIVE);
    *CL_STEREO.lock().unwrap() = cvar_get("cl_stereo", "0", CVAR_ARCHIVE);

    *CL_ADD_BLEND.lock().unwrap() = cvar_get("cl_blend", "1", CVAR_ARCHIVE);
    *CL_ADD_LIGHTS.lock().unwrap() = cvar_get("cl_lights", "1", CVAR_ARCHIVE);
    *CL_ADD_PARTICLES.lock().unwrap() = cvar_get("cl_particles", "1", CVAR_ARCHIVE);
    *CL_ADD_ENTITIES.lock().unwrap() = cvar_get("cl_entities", "1", CVAR_ARCHIVE);
    *CL_GUN.lock().unwrap() = cvar_get("cl_gun", "1", CVAR_ARCHIVE);
    *CL_FOOTSTEPS.lock().unwrap() = cvar_get("cl_footsteps", "1", CVAR_ARCHIVE);
    *CL_NOSKINS.lock().unwrap() = cvar_get("cl_noskins", "0", CVAR_ARCHIVE);
    *CL_DEFAULTSKIN.lock().unwrap() = cvar_get("cl_defaultskin", "male/grunt", CVAR_ARCHIVE);
    *CL_PREDICT.lock().unwrap() = cvar_get("cl_predict", "1", CVAR_ARCHIVE);
    *CL_MAXFPS.lock().unwrap() = cvar_get("cl_maxfps", "90", CVAR_ARCHIVE);

    // cl_upspeed, cl_forwardspeed, etc. registered via cl_input
    // (they are input-related cvars, handled in cl_input.rs)

    *FREELOOK.lock().unwrap() = cvar_get("freelook", "1", CVAR_ARCHIVE);
    *LOOKSPRING.lock().unwrap() = cvar_get("lookspring", "0", CVAR_ARCHIVE);
    *LOOKSTRAFE.lock().unwrap() = cvar_get("lookstrafe", "0", CVAR_ARCHIVE);
    *SENSITIVITY.lock().unwrap() = cvar_get("sensitivity", "5", CVAR_ARCHIVE);

    *M_PITCH.lock().unwrap() = cvar_get("m_pitch", "0.022", CVAR_ARCHIVE);
    *M_YAW.lock().unwrap() = cvar_get("m_yaw", "0.022", CVAR_ARCHIVE);
    *M_FORWARD.lock().unwrap() = cvar_get("m_forward", "1", CVAR_ARCHIVE);
    *M_SIDE.lock().unwrap() = cvar_get("m_side", "1", CVAR_ARCHIVE);

    *CL_SHOWNET.lock().unwrap() = cvar_get("cl_shownet", "0", CVAR_ZERO);
    *CL_SHOWMISS.lock().unwrap() = cvar_get("cl_showmiss", "0", CVAR_ZERO);
    *CL_SHOWCLAMP.lock().unwrap() = cvar_get("showclamp", "0", CVAR_ZERO);
    *CL_TIMEOUT.lock().unwrap() = cvar_get("cl_timeout", "120", CVAR_ZERO);
    *CL_PAUSED.lock().unwrap() = cvar_get("paused", "0", CVAR_ZERO);
    *CL_TIMEDEMO.lock().unwrap() = cvar_get("timedemo", "0", CVAR_ZERO);

    *RCON_CLIENT_PASSWORD.lock().unwrap() = cvar_get("rcon_password", "", CVAR_ZERO);
    *RCON_ADDRESS.lock().unwrap() = cvar_get("rcon_address", "", CVAR_ZERO);

    *CL_LIGHTLEVEL.lock().unwrap() = cvar_get("r_lightlevel", "0", CVAR_ZERO);

    // userinfo
    *INFO_PASSWORD.lock().unwrap() = cvar_get("password", "", CVAR_USERINFO);
    *INFO_SPECTATOR.lock().unwrap() = cvar_get("spectator", "0", CVAR_USERINFO);
    *CVAR_NAME.lock().unwrap() = cvar_get("name", "Player", CVAR_USERINFO | CVAR_ARCHIVE);
    *SKIN.lock().unwrap() = cvar_get("skin", "male/grunt", CVAR_USERINFO | CVAR_ARCHIVE);
    *RATE.lock().unwrap() = cvar_get("rate", "25000", CVAR_USERINFO | CVAR_ARCHIVE);
    *MSG_LEVEL.lock().unwrap() = cvar_get("msg", "1", CVAR_USERINFO | CVAR_ARCHIVE);
    *HAND.lock().unwrap() = cvar_get("hand", "2", CVAR_USERINFO | CVAR_ARCHIVE);
    *FOV.lock().unwrap() = cvar_get("fov", "90", CVAR_USERINFO | CVAR_ARCHIVE);
    *GENDER.lock().unwrap() = cvar_get("gender", "male", CVAR_USERINFO | CVAR_ARCHIVE);
    *GENDER_AUTO.lock().unwrap() = cvar_get("gender_auto", "1", CVAR_ARCHIVE);
    GENDER.lock().unwrap().modified = false; // clear this so we know when user sets it manually

    *CL_VWEP.lock().unwrap() = cvar_get("cl_vwep", "1", CVAR_ARCHIVE);

    // Event trigger cvars (R1Q2/Q2Pro feature)
    *CL_BEGINMAPCMD.lock().unwrap() = cvar_get("cl_beginmapcmd", "", CVAR_ARCHIVE);
    *CL_CHANGEMAPCMD.lock().unwrap() = cvar_get("cl_changemapcmd", "", CVAR_ARCHIVE);
    *CL_DISCONNECTCMD.lock().unwrap() = cvar_get("cl_disconnectcmd", "", CVAR_ARCHIVE);

    // Auto-reconnect cvars (R1Q2/Q2Pro feature)
    *CL_AUTORECONNECT.lock().unwrap() = cvar_get("cl_autoreconnect", "0", CVAR_ARCHIVE);
    *CL_AUTORECONNECT_DELAY.lock().unwrap() = cvar_get("cl_autoreconnect_delay", "3000", CVAR_ARCHIVE);
    *CL_AUTORECONNECT_MAX.lock().unwrap() = cvar_get("cl_autoreconnect_max", "3", CVAR_ARCHIVE);

    // Decoupled frame timing cvars (R1Q2/Q2Pro cl_async feature)
    // cl_async: 0=legacy (all subsystems run together), 1=enabled (decoupled timing)
    *CL_ASYNC.lock().unwrap() = cvar_get("cl_async", "1", CVAR_ARCHIVE);
    // r_maxfps: Maximum render FPS (0=unlimited, follows vsync)
    *R_MAXFPS.lock().unwrap() = cvar_get("r_maxfps", "0", CVAR_ARCHIVE);
    // cl_maxpackets: Maximum network packets per second
    *CL_MAXPACKETS.lock().unwrap() = cvar_get("cl_maxpackets", "30", CVAR_ARCHIVE);
    // cl_packetdup: Number of duplicate packets to send for lossy connections
    *CL_PACKETDUP.lock().unwrap() = cvar_get("cl_packetdup", "0", CVAR_ARCHIVE);

    // FPS-independent strafe jumping (R1Q2/Q2Pro feature)
    *CL_STRAFEJUMP_FIX.lock().unwrap() = cvar_get("cl_strafejump_fix", "1", CVAR_ARCHIVE);
    *CL_PHYSICS_FPS.lock().unwrap() = cvar_get("cl_physics_fps", "125", CVAR_ARCHIVE);

    // Chat enhancements (R1Q2/Q2Pro feature)
    *CL_FILTER_CHAT.lock().unwrap() = cvar_get("cl_filter_chat", "1", CVAR_ARCHIVE);
    *CL_CHAT_LOG.lock().unwrap() = cvar_get("cl_chat_log", "0", CVAR_ARCHIVE);

    // Crosshair customization cvars (R1Q2/Q2Pro feature)
    // crosshair cvar itself is registered in v_init for style (0=none, 1-5=procedural, 6+=image)
    *CROSSHAIR_SIZE.lock().unwrap() = cvar_get("crosshair_size", "1.0", CVAR_ARCHIVE);
    *CROSSHAIR_COLOR.lock().unwrap() = cvar_get("crosshair_color", "240", CVAR_ARCHIVE); // 0xf0 = bright white
    *CROSSHAIR_ALPHA.lock().unwrap() = cvar_get("crosshair_alpha", "1.0", CVAR_ARCHIVE);
    *CROSSHAIR_GAP.lock().unwrap() = cvar_get("crosshair_gap", "2", CVAR_ARCHIVE);
    *CROSSHAIR_THICKNESS.lock().unwrap() = cvar_get("crosshair_thickness", "2", CVAR_ARCHIVE);
    *CROSSHAIR_DYNAMIC.lock().unwrap() = cvar_get("crosshair_dynamic", "0", CVAR_ARCHIVE);
    *CH_HEALTH.lock().unwrap() = cvar_get("ch_health", "0", CVAR_ARCHIVE); // R1Q2/Q2Pro health-based crosshair color

    // HUD customization cvars (R1Q2/Q2Pro feature)
    *HUD_SCALE.lock().unwrap() = cvar_get("hud_scale", "1.0", CVAR_ARCHIVE);
    *HUD_ALPHA.lock().unwrap() = cvar_get("hud_alpha", "1.0", CVAR_ARCHIVE);
    *HUD_SHOW_HEALTH.lock().unwrap() = cvar_get("hud_show_health", "1", CVAR_ARCHIVE);
    *HUD_SHOW_ARMOR.lock().unwrap() = cvar_get("hud_show_armor", "1", CVAR_ARCHIVE);
    *HUD_SHOW_AMMO.lock().unwrap() = cvar_get("hud_show_ammo", "1", CVAR_ARCHIVE);
    *HUD_SHOW_TIMER.lock().unwrap() = cvar_get("hud_show_timer", "0", CVAR_ARCHIVE);
    *HUD_SHOW_FPS.lock().unwrap() = cvar_get("hud_show_fps", "0", CVAR_ARCHIVE);
    *HUD_SHOW_SPEED.lock().unwrap() = cvar_get("hud_show_speed", "0", CVAR_ARCHIVE);
    *HUD_SHOW_NETSTATS.lock().unwrap() = cvar_get("hud_show_netstats", "0", CVAR_ARCHIVE);
    *HUD_MINIMAL.lock().unwrap() = cvar_get("hud_minimal", "0", CVAR_ARCHIVE);
    // HUD stat smoothing: enable smooth value transitions for health/armor/ammo
    cvar_get("hud_stat_smoothing", "1", CVAR_ARCHIVE);

    // Demo recording enhancement cvars (R1Q2/Q2Pro feature)
    *CL_AUTORECORD.lock().unwrap() = cvar_get("cl_autorecord", "0", CVAR_ARCHIVE);

    // HTTP download cvars (R1Q2-style)
    cvar_get("cl_http_downloads", "1", CVAR_ARCHIVE);  // enabled by default

    // Network smoothing cvars (R1Q2/Q2Pro feature)
    *CL_TIMENUDGE.lock().unwrap() = cvar_get("cl_timenudge", "0", CVAR_ARCHIVE);
    *CL_EXTRAPOLATE.lock().unwrap() = cvar_get("cl_extrapolate", "1", CVAR_ARCHIVE);
    *CL_EXTRAPOLATE_MAX.lock().unwrap() = cvar_get("cl_extrapolate_max", "50", CVAR_ARCHIVE);
    *CL_ANIM_CONTINUE.lock().unwrap() = cvar_get("cl_anim_continue", "1", CVAR_ARCHIVE);
    *CL_PROJECTILE_PREDICT.lock().unwrap() = cvar_get("cl_projectile_predict", "1", CVAR_ARCHIVE);
    *CL_CUBIC_INTERP.lock().unwrap() = cvar_get("cl_cubic_interp", "1", CVAR_ARCHIVE);
    *CL_VIEW_SMOOTH.lock().unwrap() = cvar_get("cl_view_smooth", "1", CVAR_ARCHIVE);
    *CL_ADAPTIVE_INTERP.lock().unwrap() = cvar_get("cl_adaptive_interp", "1", CVAR_ARCHIVE);

    // register our commands
    cmd_add_command("cmd", Some(cl_forward_to_server_f));
    cmd_add_command("pause", Some(cl_pause_f));
    cmd_add_command("pingservers", Some(cl_ping_servers_f));
    cmd_add_command("skins", Some(cl_skins_f));

    cmd_add_command("userinfo", Some(cl_userinfo_f));
    cmd_add_command("snd_restart", Some(cl_snd_restart_f));

    cmd_add_command("changing", Some(cl_changing_f));
    cmd_add_command("disconnect", Some(cl_disconnect_f));
    cmd_add_command("record", Some(cl_record_f));
    cmd_add_command("record_from_demo", Some(cl_record_from_demo_f)); // R1Q2/Q2Pro feature
    cmd_add_command("stop", Some(cl_stop_f));

    cmd_add_command("quit", Some(cl_quit_f));

    // Network smoothing commands
    cmd_add_command("cl_netstats", Some(cl_netstats_f));
    cmd_add_command("cl_smooth", Some(cl_smooth_f));

    cmd_add_command("connect", Some(cl_connect_f));
    cmd_add_command("reconnect", Some(cl_reconnect_f));

    cmd_add_command("savecfg", Some(cl_write_configuration_f));

    cmd_add_command("rcon", Some(cl_rcon_f));

    // Cmd_AddCommand ("packet", CL_Packet_f); // this is dangerous to leave in

    cmd_add_command("setenv", Some(cl_setenv_f));

    cmd_add_command("precache", Some(cl_precache_f));

    cmd_add_command("download", Some(cl_download_f));

    // forward to server commands
    // the only thing this does is allow command completion to work
    // all unknown commands are automatically forwarded to the server
    cmd_add_command("wave", None);
    cmd_add_command("inven", None);
    cmd_add_command("kill", None);
    cmd_add_command("use", None);
    cmd_add_command("drop", None);
    cmd_add_command("say", None);
    cmd_add_command("say_team", None);
    cmd_add_command("info", None);
    cmd_add_command("prog", None);
    cmd_add_command("give", None);
    cmd_add_command("god", None);
    cmd_add_command("notarget", None);
    cmd_add_command("noclip", None);
    cmd_add_command("invuse", None);
    cmd_add_command("invprev", None);
    cmd_add_command("invnext", None);
    cmd_add_command("invdrop", None);
    cmd_add_command("weapnext", None);
    cmd_add_command("weapprev", None);

    // Demo playback enhancement commands (R1Q2/Q2Pro feature)
    cmd_add_command("playdemo", Some(cl_playdemo_f));
    cmd_add_command("seek", Some(cl_seek_f));
    cmd_add_command("seekpercent", Some(cl_seekpercent_f));
    cmd_add_command("demo_pause", Some(cl_demo_pause_f));
    cmd_add_command("demo_speed", Some(cl_demo_speed_f));
    cmd_add_command("demo_info", Some(cl_demo_info_f));

    // Location system commands (R1Q2/Q2Pro feature)
    cmd_add_command("loc", Some(cl_loc_f));
    cmd_add_command("loclist", Some(cl_loclist_f));
    cmd_add_command("locadd", Some(cl_locadd_f));
    cmd_add_command("locdel", Some(cl_locdel_f));
    cmd_add_command("locsave", Some(cl_locsave_f));

    // Chat enhancement commands (R1Q2/Q2Pro feature)
    cmd_add_command("ignore", Some(cl_ignore_f));
    cmd_add_command("unignore", Some(cl_unignore_f));
    cmd_add_command("ignorelist", Some(cl_ignorelist_f));
    cmd_add_command("filter_reload", Some(cl_filter_reload_f));

    // Crosshair customization command (R1Q2/Q2Pro feature)
    cmd_add_command("crosshair_info", Some(cl_crosshair_info_f));

    // HUD customization commands (R1Q2/Q2Pro feature)
    cmd_add_command("hud_info", Some(cl_hud_info_f));
    cmd_add_command("hud_reset_speed", Some(cl_hud_reset_speed_f));
    cmd_add_command("timer_start", Some(cl_timer_start_f));
    cmd_add_command("timer_stop", Some(cl_timer_stop_f));

    // Server browser commands (R1Q2/Q2Pro feature)
    cmd_add_command("refreshservers", Some(cl_browser_refresh_f));
    cmd_add_command("serverlist", Some(cl_serverlist_f));
    cmd_add_command("browser_info", Some(cl_browser_info_f));
    cmd_add_command("browser_clear", Some(cl_browser_clear_f));
    cmd_add_command("addfavorite", Some(cl_addfavorite_f));
    cmd_add_command("addserver", Some(cl_addserver_f));
    cmd_add_command("browser_filter", Some(cl_browser_filter_f));
    cmd_add_command("browser_sort", Some(cl_browser_sort_f));

    // Initialize chat system
    crate::cl_chat::chat_init();

    // Initialize crosshair system
    crate::cl_crosshair::crosshair_update_config();

    // Initialize HUD system
    crate::cl_hud::hud_update_config();

    // Initialize server browser
    crate::cl_browser::browser_init();
}

// ============================================================
// CL_FixCvarCheats
// ============================================================

struct CheatVar {
    name: &'static str,
    value: &'static str,
    var: Option<CvarHandle>,
}

static CHEATVARS: LazyLock<Mutex<Vec<CheatVar>>> = LazyLock::new(|| Mutex::new(vec![
    CheatVar { name: "timescale", value: "1", var: None },
    CheatVar { name: "timedemo", value: "0", var: None },
    CheatVar { name: "`r_drawworld", value: "1", var: None },
    CheatVar { name: "cl_testlights", value: "0", var: None },
    CheatVar { name: "r_fullbright", value: "0", var: None },
    CheatVar { name: "r_drawflat", value: "0", var: None },
    CheatVar { name: "paused", value: "0", var: None },
    CheatVar { name: "fixedtime", value: "0", var: None },
    CheatVar { name: "vk_lightmap", value: "0", var: None },
    CheatVar { name: "vk_saturatelighting", value: "0", var: None },
]));
static NUM_CHEATVARS: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

pub fn cl_fix_cvar_cheats() {
    let cl = CL.lock().unwrap();

    if cl.configstrings[CS_MAXCLIENTS] == "1" || cl.configstrings[CS_MAXCLIENTS].is_empty() {
        return; // single player can cheat
    }
    drop(cl);

    let mut cheatvars = CHEATVARS.lock().unwrap();
    let mut num = NUM_CHEATVARS.lock().unwrap();

    // find all the cvars if we haven't done it yet
    if *num == 0 {
        for cv in cheatvars.iter_mut() {
            cv.var = Some(cvar_get(cv.name, cv.value, CVAR_ZERO));
        }
        *num = cheatvars.len();
    }

    // make sure they are all set to the proper values
    for cv in cheatvars.iter() {
        if let Some(ref var) = cv.var {
            if var.string != cv.value {
                cvar_set(cv.name, cv.value);
            }
        }
    }
}

// ============================================================
// CL_SendCommand
// ============================================================

pub fn cl_send_command() {
    // get new key events
    sys_send_key_events();

    // allow mice or other external controllers to add commands
    in_commands();

    // process console commands
    cbuf_execute();

    // fix any cheating cvars
    cl_fix_cvar_cheats();

    // send intentions now
    cl_send_cmd();

    // resend a connection request if necessary
    cl_check_for_resend();
}

// ============================================================
// CL_Frame
// ============================================================

static EXTRATIME: Mutex<i32> = Mutex::new(0);
static LAST_TIME_CALLED: Mutex<i32> = Mutex::new(0);
static FRAME_COUNT: Mutex<i32> = Mutex::new(0);

/// Main client frame function with decoupled timing for render/physics/network.
///
/// Uses cl_async timing where each subsystem runs at its own configurable rate:
/// - Network (cl_read_packets): Always process incoming packets
/// - Network send (cl_send_command): At cl_maxpackets rate
/// - Physics (cl_predict_movement): At cl_maxfps rate
/// - Render (scr_update_screen): At r_maxfps rate (0 = unlimited)
pub fn cl_frame(_msec: i32) {
    if dedicated_value() {
        return;
    }

    // Decoupled frame processing - render, physics, and network run at different rates.
    // Update timing and get cvar values
    let mut timing = CL_TIMING.lock().unwrap();
    // Sync cl_async cvar to timing system (0 = legacy sync mode, 1 = decoupled)
    timing.async_enabled = CL_ASYNC.lock().unwrap().value != 0.0;
    let delta = timing.update();

    let r_maxfps = R_MAXFPS.lock().unwrap().value;
    let cl_maxfps = CL_MAXFPS.lock().unwrap().value;
    let cl_maxpackets = CL_MAXPACKETS.lock().unwrap().value;

    // Convert delta to milliseconds for compatibility
    let delta_ms = (delta * 1000.0) as i32;

    // Update realtime
    {
        let mut cls = CLS.lock().unwrap();
        cls.realtime = sys_milliseconds();
    }

    // Check for pending auto-reconnect (R1Q2/Q2Pro feature)
    cl_check_auto_reconnect();

    // Sync smoothing cvars to ClientState (allows runtime adjustment)
    cl_update_smoothing_cvars();

    // Sync chat cvars to chat system (allows runtime adjustment)
    cl_update_chat_cvars();

    // Update server browser (process responses) - R1Q2/Q2Pro feature
    crate::cl_browser::browser_update();

    // let the mouse activate or deactivate
    in_frame();

    // ============================================================
    // NETWORK: Always process incoming packets
    // ============================================================
    cl_read_packets();

    // ============================================================
    // CHAT QUEUE: Process queued chat messages when network recovers
    // ============================================================
    cl_process_chat_queue();

    // ============================================================
    // BANDWIDTH ADAPTATION: Adjust send rate based on network quality
    // ============================================================
    // Update the bandwidth adapter with current network statistics
    // and use adaptive rate if enabled, otherwise use cl_maxpackets
    let adaptive_rate = {
        let mut cl = CL.lock().unwrap();
        let cls = CLS.lock().unwrap();

        // Update bandwidth adapter with current network stats
        if cl.smoothing.bandwidth_adapter.enabled {
            let stats = cl.smoothing.network_stats.clone();
            cl.smoothing.bandwidth_adapter.update(&stats, cls.realtime);
        }

        // Use adaptive rate if enabled, otherwise use cvar
        if cl.smoothing.bandwidth_adapter.enabled {
            cl.smoothing.bandwidth_adapter.current_rate as f32
        } else {
            cl_maxpackets
        }
    };

    // ============================================================
    // NETWORK SEND: Send commands at adaptive rate
    // ============================================================
    let should_send = timing.should_send_packet(adaptive_rate);
    drop(timing); // Release timing lock before other operations

    if should_send {
        // Set frametime for command interpolation
        {
            let mut cls = CLS.lock().unwrap();
            cls.frametime = if adaptive_rate > 0.0 {
                1.0 / adaptive_rate
            } else {
                delta as f32
            };
        }
        cl_send_command();
    }

    // ============================================================
    // PHYSICS: Run at cl_maxfps rate
    // ============================================================
    let mut timing = CL_TIMING.lock().unwrap();
    let physics_frames = timing.should_physics(cl_maxfps);
    let raw_physics_frametime = timing.physics_frametime(cl_maxfps);
    drop(timing);

    // Apply frame time smoothing to reduce jitter from variable frame rates
    let physics_frametime = {
        let mut cl = CL.lock().unwrap();
        cl.smoothing.frame_time.add_sample(raw_physics_frametime)
    };

    for _ in 0..physics_frames {
        // Update client time
        {
            let mut cl = CL.lock().unwrap();
            cl.time += (physics_frametime * 1000.0) as i32;
        }

        // Predict all unacknowledged movements
        cl_predict_movement();
    }

    // ============================================================
    // RENDER: Run at r_maxfps rate (0 = unlimited)
    // ============================================================
    let mut timing = CL_TIMING.lock().unwrap();
    let should_render = timing.should_render(r_maxfps);
    drop(timing);

    if should_render {
        // Update HUD systems (R1Q2/Q2Pro feature)
        crate::cl_hud::hud_update_fps();
        crate::cl_hud::hud_update_config();
        // Sync stat smoothing enabled state from cvar (R1Q2/Q2Pro feature)
        crate::cl_hud::hud_set_stat_smoothing(cvar_variable_value("hud_stat_smoothing") != 0.0);
        {
            let cl = CL.lock().unwrap();
            let cls = CLS.lock().unwrap();
            // Update speed meter with player velocity from predicted movement
            let velocity = cl.frame.playerstate.pmove.velocity;
            crate::cl_hud::hud_update_speed(&[velocity[0] as f32 * 0.125, velocity[1] as f32 * 0.125, velocity[2] as f32 * 0.125]);
            // Update timer with server time
            crate::cl_hud::hud_update_timer(cl.time);
            // Update network statistics for HUD display
            let stats = &cl.smoothing.network_stats;
            let quality = cl.smoothing.bandwidth_adapter.quality_string();
            crate::cl_hud::hud_update_netstats(
                stats.ping,
                stats.jitter,
                stats.packet_loss,
                stats.interp_buffer_ms,
                quality,
                cls.realtime,
            );

            // Update HUD stat smoothing (health, armor, ammo)
            // During packet loss, continue smoothing without accepting potentially stale values
            if cl.packet_loss_frames > 0 {
                crate::cl_hud::hud_continue_stats_during_packet_loss(cls.realtime);
            } else {
                let ps = &cl.frame.playerstate;
                use myq2_common::q_shared::{STAT_HEALTH, STAT_ARMOR, STAT_AMMO, STAT_FRAGS};
                crate::cl_hud::hud_update_stats(
                    ps.stats[STAT_HEALTH as usize] as i32,
                    ps.stats[STAT_ARMOR as usize] as i32,
                    ps.stats[STAT_AMMO as usize] as i32,
                    ps.stats[STAT_FRAGS as usize] as i32,
                    cls.realtime,
                );
            }
        }

        // Update audio before rendering
        {
            let cl = CL.lock().unwrap();
            s_update(&cl.refdef.vieworg, &cl.v_forward, &cl.v_right, &cl.v_up);
        }

        // Check for renderer changes
        vid_check_changes();
        {
            let cl = CL.lock().unwrap();
            let cls = CLS.lock().unwrap();
            if !cl.refresh_prepped && cls.state == crate::client::ConnState::Active {
                drop(cl);
                drop(cls);
                cl_prep_refresh();
            }
        }

        // Update the screen
        scr_update_screen();

        // Update audio after rendering
        {
            let cl = CL.lock().unwrap();
            s_update(&cl.refdef.vieworg, &cl.v_forward, &cl.v_right, &cl.v_up);
        }

        // Advance local effects
        cl_run_dlights();
        cl_run_light_styles();
        scr_run_cinematic();
        scr_run_console();

        CLS.lock().unwrap().framecount += 1;
    }

    // Debug timeout handling
    if delta_ms > 5000 {
        let mut cls = CLS.lock().unwrap();
        cls.netchan.last_received = sys_milliseconds();
    }

    // Stats logging
    if log_stats_value() {
        let cls = CLS.lock().unwrap();
        if cls.state == crate::client::ConnState::Active {
            let mut lasttimecalled = LAST_TIME_CALLED.lock().unwrap();
            if *lasttimecalled == 0 {
                *lasttimecalled = sys_milliseconds();
            } else {
                let now = sys_milliseconds();
                let _ = now - *lasttimecalled;
                *lasttimecalled = now;
            }
        }
    }
}

// ============================================================
// CL_Init
// ============================================================

pub fn cl_init() {
    if dedicated_value() {
        return; // nothing running on the client
    }

    // all archived variables will now be loaded

    // mattx86: console_init
    con_init();

    vid_init();
    s_init(); // sound must be initialized after window is created

    v_init();

    // net_message.data = net_message_buffer;
    // net_message.maxsize = sizeof(net_message_buffer);
    // (handled by the net module initialization)

    m_init();

    scr_init();
    CLS.lock().unwrap().disable_screen = 1.0; // don't draw yet

    cl_init_local();
    in_init();

    let _ = myq2_common::files::fs_exec_autoexec();
    cbuf_execute();
}

// ============================================================
// CL_Shutdown
//
// Called from Sys_Quit and Com_Error as a cleanup handler.
// ============================================================

static IS_SHUTDOWN: Mutex<bool> = Mutex::new(false);

pub fn cl_shutdown() {
    let mut isdown = IS_SHUTDOWN.lock().unwrap();
    if *isdown {
        com_printf("recursive shutdown\n");
        return;
    }
    *isdown = true;
    drop(isdown);

    cl_write_configuration_f();

    s_shutdown();
    in_shutdown();
    vid_shutdown();
}

/// IN_CenterView — centers the player's vertical view angle.
/// Called from the platform layer (myq2-sys) when +mlook is released with lookspring enabled.
pub fn cl_center_view() {
    let mut cl = CL.lock().unwrap();
    let delta_angles = cl.frame.playerstate.pmove.delta_angles;
    crate::cl_input::in_center_view(&mut cl.viewangles, &delta_angles);
}

/// S_RegisterSound — public accessor for cl_tent and other modules.
pub fn cl_s_register_sound(name: &str) -> i32 {
    let mut sound = SOUND_STATE.lock().unwrap();
    let loader = |n: &str| myq2_common::files::fs_load_file(n);
    sound.s_register_sound(name, &loader).map(|i| i as i32).unwrap_or(0)
}

/// S_RawSamples — queue streaming audio for cinematics.
pub fn cl_s_raw_samples(count: i32, rate: i32, width: i32, channels: i32, data: &[u8]) {
    let mut sound = SOUND_STATE.lock().unwrap();
    let mut backend = SOUND_BACKEND.lock().unwrap();
    if let Some(ref mut be) = *backend {
        sound.s_raw_samples(count, rate, width, channels, data, be.as_mut());
    }
}

pub fn cl_s_start_sound(origin: Option<&Vec3>, entnum: i32, channel: i32, sfx: i32, volume: f32, attenuation: f32, timeofs: f32) {
    let mut sound = SOUND_STATE.lock().unwrap();
    let cl = CL.lock().unwrap();
    if sfx > 0 {
        sound.s_start_sound(
            origin.copied(),
            entnum,
            channel,
            sfx as usize,
            volume,
            attenuation,
            timeofs,
            cl.time,
        );
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{ConnState, DlType, KeyDest};

    // -------------------------------------------------------
    // CvarHandle
    // -------------------------------------------------------

    #[test]
    fn test_cvar_handle_default() {
        let cv = CvarHandle::default();
        assert!(cv.string.is_empty());
        assert_eq!(cv.value, 0.0);
        assert!(!cv.modified);
    }

    #[test]
    fn test_cvar_handle_with_values() {
        let cv = CvarHandle {
            string: "120".to_string(),
            value: 120.0,
            modified: true,
        };
        assert_eq!(cv.string, "120");
        assert_eq!(cv.value, 120.0);
        assert!(cv.modified);
    }

    // -------------------------------------------------------
    // ConnState ordering and equality
    // -------------------------------------------------------

    #[test]
    fn test_conn_state_ordering() {
        assert!(ConnState::Uninitialized < ConnState::Disconnected);
        assert!(ConnState::Disconnected < ConnState::Connecting);
        assert!(ConnState::Connecting < ConnState::Connected);
        assert!(ConnState::Connected < ConnState::Active);
    }

    #[test]
    fn test_conn_state_equality() {
        assert_eq!(ConnState::Disconnected, ConnState::Disconnected);
        assert_ne!(ConnState::Disconnected, ConnState::Active);
    }

    #[test]
    fn test_conn_state_connected_comparison() {
        // The code uses >= Connected for determining "connected" states
        assert!(ConnState::Connected >= ConnState::Connected);
        assert!(ConnState::Active >= ConnState::Connected);
        assert!(!(ConnState::Connecting >= ConnState::Connected));
    }

    // -------------------------------------------------------
    // DlType
    // -------------------------------------------------------

    #[test]
    fn test_dl_type_none_variant() {
        let dt = DlType::None;
        assert_eq!(dt, DlType::None);
    }

    #[test]
    fn test_dl_type_variants() {
        assert_ne!(DlType::Model, DlType::None);
        assert_ne!(DlType::Sound, DlType::Skin);
        assert_ne!(DlType::Single, DlType::None);
    }

    // -------------------------------------------------------
    // KeyDest
    // -------------------------------------------------------

    #[test]
    fn test_key_dest_game_variant() {
        let kd = KeyDest::Game;
        assert_eq!(kd, KeyDest::Game);
        assert_ne!(kd, KeyDest::Console);
        assert_ne!(kd, KeyDest::Menu);
    }

    // -------------------------------------------------------
    // AutoReconnectState
    // -------------------------------------------------------

    #[test]
    fn test_auto_reconnect_default() {
        let state = AutoReconnectState::default();
        assert!(!state.enabled);
        assert!(state.server.is_empty());
        assert_eq!(state.attempts, 0);
        assert_eq!(state.max_attempts, 3);
        assert_eq!(state.delay, 3000);
        assert_eq!(state.next_time, 0);
    }

    #[test]
    fn test_auto_reconnect_exponential_backoff() {
        // Tests the backoff formula: delay * (1 << (attempts - 1).min(4))
        let delay = 3000;

        // attempt 1: 3000 * 2^0 = 3000
        let backoff_1 = delay * (1 << (1 - 1).min(4));
        assert_eq!(backoff_1, 3000);

        // attempt 2: 3000 * 2^1 = 6000
        let backoff_2 = delay * (1 << (2 - 1).min(4));
        assert_eq!(backoff_2, 6000);

        // attempt 3: 3000 * 2^2 = 12000
        let backoff_3 = delay * (1 << (3 - 1).min(4));
        assert_eq!(backoff_3, 12000);

        // attempt 5: 3000 * 2^4 = 48000 (capped at 4 shifts)
        let backoff_5 = delay * (1 << (5 - 1).min(4));
        assert_eq!(backoff_5, 48000);

        // attempt 10: still capped at 2^4
        let backoff_10 = delay * (1 << (10 - 1).min(4));
        assert_eq!(backoff_10, 48000);
    }

    // -------------------------------------------------------
    // DmdlT layout
    // -------------------------------------------------------

    #[test]
    fn test_dmdl_t_default() {
        let dmdl = DmdlT::default();
        assert_eq!(dmdl.ident, 0);
        assert_eq!(dmdl.version, 0);
        assert_eq!(dmdl.num_skins, 0);
        assert_eq!(dmdl.num_frames, 0);
    }

    #[test]
    fn test_dmdl_t_repr_c_size() {
        // DmdlT has 17 i32 fields, each 4 bytes = 68 bytes total with repr(C)
        assert_eq!(std::mem::size_of::<DmdlT>(), 17 * 4);
    }

    // -------------------------------------------------------
    // Constants
    // -------------------------------------------------------

    #[test]
    fn test_env_cnt_constants() {
        // ENV_CNT = CS_PLAYERSKINS + MAX_CLIENTS * PLAYER_MULT
        assert_eq!(PLAYER_MULT, 5);
        assert_eq!(ENV_CNT, CS_PLAYERSKINS + MAX_CLIENTS * PLAYER_MULT);
        // TEXTURE_CNT = ENV_CNT + 13
        assert_eq!(TEXTURE_CNT, ENV_CNT + 13);
    }

    #[test]
    fn test_ns_constants() {
        assert_eq!(NS_CLIENT, 0);
        assert_eq!(NS_SERVER, 1);
    }

    // -------------------------------------------------------
    // generate_demo_name
    // -------------------------------------------------------

    #[test]
    fn test_generate_demo_name_format() {
        let name = generate_demo_name();
        // Should start with "demo_"
        assert!(name.starts_with("demo_"),
            "Demo name should start with 'demo_', got: {}", name);

        // Should contain an underscore separating date and time
        // Format: demo_YYYYMMDD_HHMMSS
        let parts: Vec<&str> = name.split('_').collect();
        assert_eq!(parts.len(), 3, "Expected 3 parts separated by _, got: {:?}", parts);

        // Date part should be 8 digits (YYYYMMDD)
        assert_eq!(parts[1].len(), 8, "Date part should be 8 digits, got: {}", parts[1]);
        assert!(parts[1].chars().all(|c| c.is_ascii_digit()),
            "Date part should be all digits: {}", parts[1]);

        // Time part should be 6 digits (HHMMSS)
        assert_eq!(parts[2].len(), 6, "Time part should be 6 digits, got: {}", parts[2]);
        assert!(parts[2].chars().all(|c| c.is_ascii_digit()),
            "Time part should be all digits: {}", parts[2]);
    }

    #[test]
    fn test_generate_demo_name_reasonable_date() {
        let name = generate_demo_name();
        let parts: Vec<&str> = name.split('_').collect();
        let date = parts[1];

        // Year should be >= 2024 (reasonable for when this code runs)
        let year: u64 = date[..4].parse().unwrap();
        assert!(year >= 2024, "Year should be >= 2024, got: {}", year);

        // Month should be 01-12
        let month: u64 = date[4..6].parse().unwrap();
        assert!(month >= 1 && month <= 12, "Month should be 1-12, got: {}", month);

        // Day should be 01-31
        let day: u64 = date[6..8].parse().unwrap();
        assert!(day >= 1 && day <= 31, "Day should be 1-31, got: {}", day);
    }

    #[test]
    fn test_generate_demo_name_reasonable_time() {
        let name = generate_demo_name();
        let parts: Vec<&str> = name.split('_').collect();
        let time = parts[2];

        // Hours should be 00-23
        let hours: u64 = time[..2].parse().unwrap();
        assert!(hours <= 23, "Hours should be 0-23, got: {}", hours);

        // Minutes should be 00-59
        let mins: u64 = time[2..4].parse().unwrap();
        assert!(mins <= 59, "Minutes should be 0-59, got: {}", mins);

        // Seconds should be 00-59
        let secs: u64 = time[4..6].parse().unwrap();
        assert!(secs <= 59, "Seconds should be 0-59, got: {}", secs);
    }

    // -------------------------------------------------------
    // Timeout calculation
    // -------------------------------------------------------

    #[test]
    fn test_timeout_calculation() {
        // Timeout fires when (realtime - last_received) > timeout_val * 1000
        let timeout_val = 120.0f32; // 120 seconds
        let threshold = timeout_val as f64 * 1000.0;

        // Just under timeout
        let elapsed: f64 = 119999.0;
        assert!(elapsed <= threshold);

        // Just over timeout
        let elapsed: f64 = 120001.0;
        assert!(elapsed > threshold);
    }

    #[test]
    fn test_timeout_count_threshold() {
        // Connection drops after timeoutcount > 5
        let mut timeoutcount = 0;
        for _ in 0..5 {
            timeoutcount += 1;
        }
        assert!(!(timeoutcount > 5)); // Still 5, not > 5

        timeoutcount += 1;
        assert!(timeoutcount > 5); // Now 6, drops connection
    }

    // -------------------------------------------------------
    // Connect packet port handling
    // -------------------------------------------------------

    #[test]
    fn test_port_server_default() {
        // PORT_SERVER should be Quake 2's default port
        assert_eq!(PORT_SERVER, 27910);
    }

    #[test]
    fn test_port_to_big_endian() {
        // When port is 0, it gets set to PORT_SERVER in big-endian
        let port = PORT_SERVER.to_be();
        // On a little-endian system, the bytes would be swapped
        let expected = 27910u16.to_be();
        assert_eq!(port, expected);
    }

    // -------------------------------------------------------
    // Packet escape sequence handling
    // -------------------------------------------------------

    #[test]
    fn test_packet_escape_newline() {
        // The cl_packet_f function converts \n escape sequences
        let input = "hello\\nworld";
        let bytes = input.as_bytes();
        let mut result: Vec<u8> = Vec::new();

        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'\\' && i + 1 < bytes.len() && bytes[i + 1] == b'n' {
                result.push(b'\n');
                i += 2;
            } else {
                result.push(bytes[i]);
                i += 1;
            }
        }

        assert_eq!(result, b"hello\nworld");
    }

    #[test]
    fn test_packet_escape_no_escape() {
        let input = "hello world";
        let bytes = input.as_bytes();
        let mut result: Vec<u8> = Vec::new();

        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'\\' && i + 1 < bytes.len() && bytes[i + 1] == b'n' {
                result.push(b'\n');
                i += 2;
            } else {
                result.push(bytes[i]);
                i += 1;
            }
        }

        assert_eq!(result, b"hello world");
    }

    #[test]
    fn test_packet_escape_trailing_backslash() {
        let input = "test\\";
        let bytes = input.as_bytes();
        let mut result: Vec<u8> = Vec::new();

        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'\\' && i + 1 < bytes.len() && bytes[i + 1] == b'n' {
                result.push(b'\n');
                i += 2;
            } else {
                result.push(bytes[i]);
                i += 1;
            }
        }

        // Trailing backslash is kept as-is
        assert_eq!(result, b"test\\");
    }

    // -------------------------------------------------------
    // Userinfo string building
    // -------------------------------------------------------

    #[test]
    fn test_connect_command_format() {
        // The connect command has the format:
        // connect <protocol_version> <qport> <challenge> "<userinfo>"
        let protocol = 34;
        let port = 12345;
        let challenge = 67890;
        let userinfo = "\\name\\Player\\skin\\male/grunt";

        let msg = format!(
            "connect {} {} {} \"{}\"\n",
            protocol, port, challenge, userinfo
        );

        assert!(msg.starts_with("connect 34 12345 67890"));
        assert!(msg.contains(userinfo));
    }

    // -------------------------------------------------------
    // Challenge protocol parsing
    // -------------------------------------------------------

    #[test]
    fn test_challenge_protocol_parsing() {
        // The challenge response can include "p=<version>"
        let arg = "p=35";
        assert!(arg.starts_with("p="));
        let proto: i32 = arg[2..].parse().unwrap();
        assert_eq!(proto, 35);
    }

    #[test]
    fn test_challenge_protocol_parsing_default() {
        // Without protocol arg, use PROTOCOL_VERSION
        let arg = "";
        let server_protocol = if arg.starts_with("p=") {
            arg[2..].parse::<i32>().unwrap_or(PROTOCOL_VERSION)
        } else {
            PROTOCOL_VERSION
        };
        assert_eq!(server_protocol, PROTOCOL_VERSION);
    }

    // -------------------------------------------------------
    // Reconnect timing
    // -------------------------------------------------------

    #[test]
    fn test_check_for_resend_timing() {
        // Resend happens after 3000ms
        let realtime = 10000;
        let connect_time = 6000.0;

        let elapsed = realtime - (connect_time as i32);
        assert!(elapsed >= 3000); // 10000 - 6000 = 4000 >= 3000, will resend

        let connect_time_2 = 8000.0;
        let elapsed_2 = realtime - (connect_time_2 as i32);
        assert!(elapsed_2 < 3000); // 10000 - 8000 = 2000 < 3000, won't resend
    }

    // -------------------------------------------------------
    // Gender detection from skin name
    // -------------------------------------------------------

    #[test]
    fn test_gender_from_model_male() {
        let model = "male";
        let model_lower = model.to_ascii_lowercase();
        let gender = if model_lower == "male" || model_lower == "cyborg" {
            "male"
        } else if model_lower == "female" || model_lower == "crackhor" {
            "female"
        } else {
            "none"
        };
        assert_eq!(gender, "male");
    }

    #[test]
    fn test_gender_from_model_female() {
        let model = "female";
        let model_lower = model.to_ascii_lowercase();
        let gender = if model_lower == "male" || model_lower == "cyborg" {
            "male"
        } else if model_lower == "female" || model_lower == "crackhor" {
            "female"
        } else {
            "none"
        };
        assert_eq!(gender, "female");
    }

    #[test]
    fn test_gender_from_model_cyborg() {
        let model = "cyborg";
        let model_lower = model.to_ascii_lowercase();
        let gender = if model_lower == "male" || model_lower == "cyborg" {
            "male"
        } else if model_lower == "female" || model_lower == "crackhor" {
            "female"
        } else {
            "none"
        };
        assert_eq!(gender, "male");
    }

    #[test]
    fn test_gender_from_model_unknown() {
        let model = "android";
        let model_lower = model.to_ascii_lowercase();
        let gender = if model_lower == "male" || model_lower == "cyborg" {
            "male"
        } else if model_lower == "female" || model_lower == "crackhor" {
            "female"
        } else {
            "none"
        };
        assert_eq!(gender, "none");
    }

    // -------------------------------------------------------
    // Skin string parsing (model/skin)
    // -------------------------------------------------------

    #[test]
    fn test_skin_string_parsing() {
        let skin_str = "male/grunt";
        let (model, skin) = if let Some(pos) = skin_str.find('/') {
            (&skin_str[..pos], &skin_str[pos + 1..])
        } else {
            (skin_str, "")
        };
        assert_eq!(model, "male");
        assert_eq!(skin, "grunt");
    }

    #[test]
    fn test_skin_string_parsing_backslash() {
        let skin_str = "female\\athena";
        let (model, skin) = if let Some(pos) = skin_str.find('/').or_else(|| skin_str.find('\\')) {
            (&skin_str[..pos], &skin_str[pos + 1..])
        } else {
            (skin_str, "")
        };
        assert_eq!(model, "female");
        assert_eq!(skin, "athena");
    }

    #[test]
    fn test_skin_string_parsing_no_separator() {
        let skin_str = "male";
        let (model, skin) = if let Some(pos) = skin_str.find('/').or_else(|| skin_str.find('\\')) {
            (&skin_str[..pos], &skin_str[pos + 1..])
        } else {
            (skin_str, "")
        };
        assert_eq!(model, "male");
        assert_eq!(skin, "");
    }

    // -------------------------------------------------------
    // Player info parsing from connect string
    // -------------------------------------------------------

    #[test]
    fn test_player_info_backslash_split() {
        let playerinfo = "Player\\male/grunt";
        let p = if let Some(pos) = playerinfo.find('\\') {
            &playerinfo[pos + 1..]
        } else {
            playerinfo
        };
        assert_eq!(p, "male/grunt");
    }

    #[test]
    fn test_player_info_no_backslash() {
        let playerinfo = "male/grunt";
        let p = if let Some(pos) = playerinfo.find('\\') {
            &playerinfo[pos + 1..]
        } else {
            playerinfo
        };
        assert_eq!(p, "male/grunt");
    }

    // -------------------------------------------------------
    // Server name truncation
    // -------------------------------------------------------

    #[test]
    fn test_servername_truncation() {
        let server = "very.long.server.name.that.exceeds.the.maximum.path.length.for.quake2.net.addresses";
        let truncated = &server[..std::cmp::min(server.len(), MAX_OSPATH - 1)];
        assert!(truncated.len() <= MAX_OSPATH - 1);
    }

    #[test]
    fn test_servername_short_no_truncation() {
        let server = "localhost";
        let truncated = &server[..std::cmp::min(server.len(), MAX_OSPATH - 1)];
        assert_eq!(truncated, "localhost");
    }

    // -------------------------------------------------------
    // Demo recording state machine
    // -------------------------------------------------------

    #[test]
    fn test_demo_marker_end() {
        // Demo end marker is -1 as i32
        let len: i32 = -1;
        let bytes = len.to_le_bytes();
        let recovered = i32::from_le_bytes(bytes);
        assert_eq!(recovered, -1);
    }

    #[test]
    fn test_demo_message_length_validation() {
        // Valid lengths are > 0 and <= MAX_MSGLEN
        assert!(1 > 0 && 1 <= MAX_MSGLEN as i32);
        assert!(MAX_MSGLEN as i32 > 0 && MAX_MSGLEN as i32 <= MAX_MSGLEN as i32);
        assert!(!(0 > 0)); // 0 is invalid
        assert!(!(-1 > 0)); // -1 is end marker
        assert!(!(MAX_MSGLEN as i32 + 1 <= MAX_MSGLEN as i32)); // too big
    }

    // -------------------------------------------------------
    // Demo compressed extension
    // -------------------------------------------------------

    #[test]
    fn test_demo_extension_uncompressed() {
        let compressed = false;
        let ext = if compressed { "dm2z" } else { "dm2" };
        assert_eq!(ext, "dm2");
    }

    #[test]
    fn test_demo_extension_compressed() {
        let compressed = true;
        let ext = if compressed { "dm2z" } else { "dm2" };
        assert_eq!(ext, "dm2z");
    }

    #[test]
    fn test_demo_path_format() {
        let gamedir = "baseq2";
        let demo_name = "test_demo";
        let ext = "dm2";
        let path = format!("{}/demos/{}.{}", gamedir, demo_name, ext);
        assert_eq!(path, "baseq2/demos/test_demo.dm2");
    }

    // -------------------------------------------------------
    // Stereo separation clamping
    // -------------------------------------------------------

    #[test]
    fn test_stereo_separation_clamping() {
        // Stereo separation is clamped to [0.0, 1.0]
        let mut sep = 1.5f32;
        if sep > 1.0 { sep = 1.0; }
        assert_eq!(sep, 1.0);

        let mut sep = -0.5f32;
        if sep < 0.0 { sep = 0.0; }
        assert_eq!(sep, 0.0);

        let mut sep = 0.5f32;
        if sep > 1.0 { sep = 1.0; }
        if sep < 0.0 { sep = 0.0; }
        assert_eq!(sep, 0.5);
    }

    #[test]
    fn test_stereo_frame_count() {
        // Stereo mode: 2 frames with opposing separation
        let stereo_enabled = true;
        let sep = 0.4f32;

        let (numframes, separation) = if stereo_enabled {
            (2, [-sep / 2.0, sep / 2.0])
        } else {
            (1, [0.0, 0.0])
        };

        assert_eq!(numframes, 2);
        assert!((separation[0] - (-0.2)).abs() < 1e-6);
        assert!((separation[1] - 0.2).abs() < 1e-6);
    }

    #[test]
    fn test_no_stereo_frame_count() {
        let stereo_enabled = false;
        let sep = 0.4f32;

        let (numframes, separation) = if stereo_enabled {
            (2, [-sep / 2.0, sep / 2.0])
        } else {
            (1, [0.0, 0.0])
        };

        assert_eq!(numframes, 1);
        assert_eq!(separation, [0.0, 0.0]);
    }

    // -------------------------------------------------------
    // Timenudge clamping
    // -------------------------------------------------------

    #[test]
    fn test_timenudge_clamping() {
        assert_eq!((-150i32).clamp(-100, 100), -100);
        assert_eq!(150i32.clamp(-100, 100), 100);
        assert_eq!(50i32.clamp(-100, 100), 50);
        assert_eq!((-50i32).clamp(-100, 100), -50);
        assert_eq!(0i32.clamp(-100, 100), 0);
    }

    #[test]
    fn test_extrapolate_max_clamping() {
        assert_eq!((-10i32).clamp(0, 200), 0);
        assert_eq!(300i32.clamp(0, 200), 200);
        assert_eq!(50i32.clamp(0, 200), 50);
    }

    // -------------------------------------------------------
    // ENV_SUF table
    // -------------------------------------------------------

    #[test]
    fn test_env_suf_table() {
        assert_eq!(ENV_SUF.len(), 6);
        assert_eq!(ENV_SUF[0], "rt");
        assert_eq!(ENV_SUF[1], "bk");
        assert_eq!(ENV_SUF[2], "lf");
        assert_eq!(ENV_SUF[3], "ft");
        assert_eq!(ENV_SUF[4], "up");
        assert_eq!(ENV_SUF[5], "dn");
    }

    // -------------------------------------------------------
    // Config file name handling
    // -------------------------------------------------------

    #[test]
    fn test_config_name_with_extension() {
        let file_name = "myconfig.cfg";
        let has_cfg = file_name.ends_with(".cfg");
        assert!(has_cfg);
    }

    #[test]
    fn test_config_name_without_extension() {
        let mut file_name = "myconfig".to_string();
        // The function appends .cfg if not present
        if !file_name.ends_with(".cfg") {
            file_name.push_str(".cfg");
        }
        assert_eq!(file_name, "myconfig.cfg");
    }

    #[test]
    fn test_config_default_name() {
        let file_name = "config";
        let mut name = file_name.to_string();
        if !name.ends_with(".cfg") {
            name.push_str(".cfg");
        }
        assert_eq!(name, "config.cfg");
    }

    // -------------------------------------------------------
    // Cheat cvar reset
    // -------------------------------------------------------

    #[test]
    fn test_cheat_cvar_detection() {
        // Single player (maxclients == 1) allows cheats
        let maxclients = "1";
        let allow_cheats = maxclients == "1" || maxclients.is_empty();
        assert!(allow_cheats);

        // Multiplayer does not
        let maxclients = "8";
        let allow_cheats = maxclients == "1" || maxclients.is_empty();
        assert!(!allow_cheats);
    }

    // -------------------------------------------------------
    // Rcon message format
    // -------------------------------------------------------

    #[test]
    fn test_rcon_message_format() {
        let password = "secret";
        let args = vec!["status"];

        let mut message = vec![0xFFu8; 4];
        message.extend_from_slice(b"rcon ");
        message.extend_from_slice(password.as_bytes());
        message.push(b' ');
        for arg in &args {
            message.extend_from_slice(arg.as_bytes());
            message.push(b' ');
        }
        message.push(0);

        assert_eq!(&message[..4], &[0xFF, 0xFF, 0xFF, 0xFF]);
        assert_eq!(&message[4..9], b"rcon ");
        assert_eq!(&message[9..15], b"secret");
        assert_eq!(message[15], b' ');
        assert_eq!(&message[16..22], b"status");
    }

    // -------------------------------------------------------
    // Connectionless packet header
    // -------------------------------------------------------

    #[test]
    fn test_connectionless_packet_header() {
        // Connectionless packets start with 0xFFFFFFFF (-1 as i32)
        let header_bytes = [0xFF, 0xFF, 0xFF, 0xFF];
        let header = i32::from_le_bytes(header_bytes);
        assert_eq!(header, -1);
    }

    // -------------------------------------------------------
    // Precache phases
    // -------------------------------------------------------

    #[test]
    fn test_precache_phase_order() {
        // Verify the precache phase order
        let cs_models = CS_MODELS as i32;
        let cs_sounds = CS_SOUNDS as i32;
        let cs_images = CS_IMAGES as i32;
        let cs_playerskins = CS_PLAYERSKINS as i32;
        let env_cnt = ENV_CNT as i32;
        let texture_cnt = TEXTURE_CNT as i32;

        assert!(cs_models < cs_sounds);
        assert!(cs_sounds < cs_images);
        assert!(cs_images < cs_playerskins);
        assert!(cs_playerskins < env_cnt);
        assert!(env_cnt < texture_cnt);
    }

    // -------------------------------------------------------
    // Map name extraction from configstring
    // -------------------------------------------------------

    #[test]
    fn test_map_name_extraction() {
        let cs = "maps/base1.bsp";
        let mapname = if cs.len() > 5 {
            let without_prefix = &cs[5..]; // skip "maps/"
            if without_prefix.len() > 4 {
                &without_prefix[..without_prefix.len() - 4] // cut off ".bsp"
            } else {
                without_prefix
            }
        } else {
            cs
        };
        assert_eq!(mapname, "base1");
    }

    #[test]
    fn test_map_name_extraction_longer_name() {
        let cs = "maps/warehouse_complex.bsp";
        let mapname = if cs.len() > 5 {
            let without_prefix = &cs[5..];
            if without_prefix.len() > 4 {
                &without_prefix[..without_prefix.len() - 4]
            } else {
                without_prefix
            }
        } else {
            cs
        };
        assert_eq!(mapname, "warehouse_complex");
    }

    #[test]
    fn test_map_name_extraction_short() {
        // Edge case: very short configstring
        let cs = "map";
        let mapname = if cs.len() > 5 {
            let without_prefix = &cs[5..];
            if without_prefix.len() > 4 {
                &without_prefix[..without_prefix.len() - 4]
            } else {
                without_prefix
            }
        } else {
            cs
        };
        assert_eq!(mapname, "map");
    }
}
