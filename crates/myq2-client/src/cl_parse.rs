// cl_parse.rs -- parse a message received from the server
// Converted from: myq2-original/client/cl_parse.c

use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use myq2_common::q_shared::*;
use myq2_common::qcommon::{
    SizeBuf,
    BASEDIRNAME, PROTOCOL_VERSION, PROTOCOL_R1Q2, PROTOCOL_Q2PRO,
    PROTOCOL_VERSION_MIN, PROTOCOL_VERSION_MAX, SvcOps, ClcOps,
    SND_VOLUME, SND_ATTENUATION, SND_POS, SND_ENT, SND_OFFSET,
    DEFAULT_SOUND_PACKET_VOLUME, DEFAULT_SOUND_PACKET_ATTENUATION,
    SVC_ZPACKET, SVC_ZDOWNLOAD, MAX_MSGLEN_R1Q2,
};
use myq2_common::common::{com_printf, com_dprintf, com_error};
use myq2_common::compression;

use crate::cl_http::{self, cl_http_init};

use crate::client::*;
use crate::client::MAX_LIGHTSTYLES;
use crate::cl_ents::ClientEntState;
use crate::cl_fx::ClFxState;
use crate::cl_scrn::ScrState;
use crate::cl_tent::TEntState;
use crate::snd_dma::SoundState;

/// Bundles all mutable state contexts needed during server message parsing.
/// This avoids passing many individual parameters through the call chain.
pub struct ParseContext<'a> {
    pub scr: &'a mut ScrState,
    pub fx: &'a mut ClFxState,
    pub tent: &'a mut TEntState,
    pub ent_state: &'a mut ClientEntState,
    pub sound: &'a mut SoundState,
    /// Projectile subsystem state. Used for parsing and rendering projectile entities
    /// via the compact bit-packed protocol (originally #if 0'd in vanilla Q2).
    pub proj_state: &'a mut crate::cl_ents::ProjectileState,
}

use crate::cl_ents::ClientCallbacks;

/// Callbacks struct for cl_parse_frame and cl_add_entities. Holds mutable
/// references to the effect subsystems so that trail/particle/tent callbacks
/// can delegate to the real implementations on `ClFxState` and `TEntState`.
/// `ent_state` is borrowed separately by the callers, so it is NOT included here.
pub struct FrameCallbacks<'a> {
    pub fx: &'a mut ClFxState,
    pub tent: &'a mut TEntState,
    pub sound: &'a mut SoundState,
    pub cl_time: f32,
}

impl<'a> ClientCallbacks for FrameCallbacks<'a> {
    fn cl_entity_event(&mut self, ent: &EntityState) {
        use myq2_common::q_shared::{EV_ITEM_RESPAWN, EV_PLAYER_TELEPORT, EV_FOOTSTEP, EV_FALLSHORT, EV_FALL, EV_FALLFAR};
        match ent.event {
            x if x == EV_ITEM_RESPAWN => {
                let sfx = crate::cl_main::cl_s_register_sound("items/respawn1.wav");
                crate::cl_main::cl_s_start_sound(Some(&ent.origin), ent.number, CHAN_WEAPON, sfx, 1.0, ATTN_IDLE, 0.0);
                self.fx.cl_item_respawn_particles(&ent.origin, self.cl_time);
            }
            x if x == EV_PLAYER_TELEPORT => {
                let sfx = crate::cl_main::cl_s_register_sound("misc/tele1.wav");
                crate::cl_main::cl_s_start_sound(Some(&ent.origin), ent.number, CHAN_WEAPON, sfx, 1.0, ATTN_IDLE, 0.0);
                self.fx.cl_teleport_particles(&ent.origin, self.cl_time);
            }
            x if x == EV_FOOTSTEP => {
                let cl_footsteps = myq2_common::cvar::cvar_variable_value("cl_footsteps");
                if cl_footsteps != 0.0 {
                    let idx = (rand::random::<u32>() & 3) as usize;
                    let sfx = self.tent.cl_sfx_footsteps[idx];
                    crate::cl_main::cl_s_start_sound(None, ent.number, CHAN_BODY, sfx, 1.0, ATTN_NORM, 0.0);
                }
            }
            x if x == EV_FALLSHORT => {
                let sfx = crate::cl_main::cl_s_register_sound("player/land1.wav");
                crate::cl_main::cl_s_start_sound(None, ent.number, CHAN_AUTO, sfx, 1.0, ATTN_NORM, 0.0);
            }
            x if x == EV_FALL => {
                let sfx = crate::cl_main::cl_s_register_sound("*fall2.wav");
                crate::cl_main::cl_s_start_sound(None, ent.number, CHAN_AUTO, sfx, 1.0, ATTN_NORM, 0.0);
            }
            x if x == EV_FALLFAR => {
                let sfx = crate::cl_main::cl_s_register_sound("*fall1.wav");
                crate::cl_main::cl_s_start_sound(None, ent.number, CHAN_AUTO, sfx, 1.0, ATTN_NORM, 0.0);
            }
            _ => {}
        }
    }
    fn cl_teleporter_particles(&mut self, ent: &EntityState) {
        self.fx.cl_teleporter_particles(ent, self.cl_time);
    }
    fn add_stain(&mut self, org: &Vec3, intensity: f32, r: f32, g: f32, b: f32, a: f32, stain_type: StainType) {
        // SAFETY: single-threaded engine, RENDERER_FNS set at startup
        unsafe {
            (crate::console::RENDERER_FNS.r_add_stain)(org, intensity, r, g, b, a, stain_type as i32);
        }
    }
    fn shownet(&self, _s: &str) {
        // Debug output handled at call site
    }
    fn scr_end_loading_plaque(&mut self, _clear: bool) {
        // Handled via cl_main wrapper
    }
    fn cl_check_prediction_error(&mut self) {
        crate::cl_main::with_cl_cls(|cl, cls| {
            let cl_predict = myq2_common::cvar::cvar_variable_value("cl_predict");
            let cl_showmiss = myq2_common::cvar::cvar_variable_value("cl_showmiss");
            crate::cl_pred::cl_check_prediction_error(
                cl,
                cls.netchan.incoming_acknowledged,
                cl_predict,
                cl_showmiss,
            );
        });
    }
    fn v_add_entity(&mut self, ent: &Entity) {
        crate::cl_main::with_view_state(|view| {
            crate::cl_view::v_add_entity(view, ent);
        });
    }
    fn v_add_light(&mut self, org: &Vec3, intensity: f32, r: f32, g: f32, b: f32) {
        crate::cl_main::with_view_state(|view| {
            crate::cl_view::v_add_light(view, org, intensity, r, g, b);
        });
    }
    fn r_register_model(&self, name: &str) -> i32 { crate::console::r_register_model(name) }
    fn r_register_skin(&self, name: &str) -> i32 { crate::console::r_register_skin(name) }
    fn get_skin_name(&self, _skin: i32) -> Option<String> { None } // skin names stored in renderer image table
    fn developer_searchpath(&self, who: i32) -> i32 {
        myq2_common::files::with_fs_ctx(|ctx| ctx.developer_searchpath(who)).unwrap_or(0)
    }
    fn cl_rocket_trail(&mut self, start: &Vec3, end: &Vec3, old: &mut CEntity) {
        self.fx.cl_rocket_trail(start, end, old, self.cl_time);
    }
    fn cl_blaster_trail(&mut self, start: &Vec3, end: &Vec3) {
        self.fx.cl_blaster_trail(start, end, self.cl_time);
    }
    fn cl_blaster_trail2(&mut self, start: &Vec3, end: &Vec3) {
        self.fx.cl_blaster_trail2(start, end, self.cl_time);
    }
    fn cl_diminishing_trail(&mut self, start: &Vec3, end: &Vec3, old: &mut CEntity, flags: u32) {
        self.fx.cl_diminishing_trail(start, end, old, flags, self.cl_time);
    }
    fn cl_fly_effect(&mut self, ent: &mut CEntity, origin: &Vec3) {
        self.fx.cl_fly_effect(ent, origin, self.cl_time as i32);
    }
    fn cl_bfg_particles(&mut self, ent: &Entity) {
        let render_ent = crate::cl_fx::RenderEntity { origin: ent.origin };
        self.fx.cl_bfg_particles(&render_ent, self.cl_time);
    }
    fn cl_trap_particles(&mut self, ent: &Entity) {
        let mut render_ent = crate::cl_fx::RenderEntity { origin: ent.origin };
        self.fx.cl_trap_particles(&mut render_ent, self.cl_time);
    }
    fn cl_flag_trail(&mut self, start: &Vec3, end: &Vec3, color: f32) {
        self.fx.cl_flag_trail(start, end, color, self.cl_time);
    }
    fn cl_tag_trail(&mut self, start: &Vec3, end: &Vec3, color: f32) {
        self.fx.cl_tag_trail(start, end, color, self.cl_time);
    }
    fn cl_tracker_trail(&mut self, start: &Vec3, end: &Vec3, particle_color: i32) {
        self.fx.cl_tracker_trail(start, end, particle_color, self.cl_time);
    }
    fn cl_tracker_shell(&mut self, origin: &Vec3) {
        self.fx.cl_tracker_shell(origin, self.cl_time);
    }
    fn cl_ionripper_trail(&mut self, start: &Vec3, end: &Vec3) {
        self.fx.cl_ionripper_trail(start, end, self.cl_time);
    }
    fn cl_add_tents(&mut self) {
        let cl = crate::cl_main::CL.lock().unwrap();
        let mut view = crate::cl_main::VIEW_STATE.lock().unwrap();
        let hand = myq2_common::cvar::cvar_variable_value("hand");

        // === Extend sustain effect lifetimes during packet loss ===
        // This prevents effects (widow splash, nuke, steam) from disappearing during packet gaps
        if cl.packet_loss_frames > 0 {
            // Extend sustains by 300ms per packet loss detection
            self.tent.cl_extend_sustains_for_packet_loss(cl.time, 300);
        } else {
            // Reset extended sustains when packets resume
            self.tent.cl_reset_extended_sustains();
        }

        // Entity lookup closure for beam endpoint interpolation
        // This allows beams to track moving entities smoothly
        let cl_ents = crate::cl_main::CL_ENTITIES.lock().unwrap();
        let entity_lookup = |entnum: i32| -> Option<Vec3> {
            if entnum > 0 && (entnum as usize) < cl_ents.len() {
                let cent = &cl_ents[entnum as usize];
                // Only return position if entity has recent data
                if cent.serverframe > 0 {
                    return Some(cent.lerp_origin);
                }
            }
            None
        };

        crate::cl_tent::cl_add_tents(self.tent, self.fx, &cl, &mut view, Some(hand), &entity_lookup);
    }
    fn cl_add_particles(&mut self) {
        let mut view = crate::cl_main::VIEW_STATE.lock().unwrap();
        // Use smart particle update: parallel when 256+ particles, sequential otherwise
        self.fx.cl_add_particles_smart(self.cl_time, |org, length, color, alpha, ptype| {
            crate::cl_view::v_add_particle(&mut view, org, length, color, alpha, ptype);
        });

        // Transfer recent effects from fx state to the main continuation system
        let recent_effects = self.fx.take_recent_effects();

        // Add continuing effects during packet loss (effect continuation system)
        {
            let mut cl = crate::cl_main::CL.lock().unwrap();
            let current_time = self.cl_time as i32;

            // Register recently created effects for continuation
            for effect in recent_effects {
                cl.smoothing.effect_continuation.register(
                    effect.effect_type,
                    effect.origin,
                    effect.velocity,
                    effect.duration_ms,
                    -1, // entity_num not tracked here
                    effect.start_time,
                );
            }

            // Only render continuing effects during packet loss
            // This fills in gaps when we don't receive particle updates from the server
            if cl.packet_loss_frames > 0 {
                let continuing = cl.smoothing.effect_continuation.get_continuing_effects(current_time);
                let zero_length = [0.0f32; 3];
                for (pos, effect_type) in continuing {
                    // Map effect types to particle colors and types
                    // Effect types: 0=smoke, 1=fire, 2=sparks, 3=blood, 4=debris
                    let (color, ptype) = match effect_type {
                        0 => (7, crate::cl_fx::PT_SMOKE),   // Gray smoke
                        1 => (0xe0, crate::cl_fx::PT_FIRE), // Orange fire
                        2 => (0xdc, crate::cl_fx::PT_DEFAULT), // Yellow sparks
                        3 => (0xe8, crate::cl_fx::PT_BLOOD),   // Red blood
                        _ => (0x0f, crate::cl_fx::PT_DEFAULT), // White default
                    };
                    // Fade alpha based on packet loss duration (more faded as loss continues)
                    let base_alpha = 0.6f32;
                    let fade_factor = 1.0 - (cl.packet_loss_frames as f32 * 0.1).min(0.5);
                    let alpha = base_alpha * fade_factor;
                    crate::cl_view::v_add_particle(&mut view, &pos, &zero_length, color, alpha, ptype);
                }
            }

            // Clean up expired effects
            cl.smoothing.effect_continuation.cleanup(current_time);
        }
    }
    fn cl_add_dlights(&mut self) {
        let mut view = crate::cl_main::VIEW_STATE.lock().unwrap();

        // === Extend dlight lifetimes during packet loss ===
        // This prevents lights from abruptly disappearing when packets are dropped
        let (packet_loss_frames, lerpfrac, current_time) = {
            let cl = crate::cl_main::CL.lock().unwrap();
            if cl.packet_loss_frames > 0 {
                // Extend dlights by 200ms per packet loss detection
                // (extension is capped internally to prevent infinite lights)
                self.fx.cl_extend_dlights_for_packet_loss(self.cl_time, 200.0);
            } else {
                // Reset extended dlights when packets resume
                self.fx.cl_reset_extended_dlights();
            }
            (cl.packet_loss_frames, cl.lerpfrac, cl.time)
        };

        // Add standard dlights from fx system with per-entity smoothing
        // This provides smoother light position and radius transitions
        {
            let mut cl = crate::cl_main::CL.lock().unwrap();
            self.fx.cl_add_dlights_smoothed(
                &mut cl.smoothing.dynamic_lights,
                current_time,
                lerpfrac,
                |org, intensity, r, g, b| {
                    crate::cl_view::v_add_light(&mut view, org, intensity, r, g, b);
                },
            );
        }

        // Add predicted weapon effects as dlights
        let cl = crate::cl_main::CL.lock().unwrap();
        let cls = crate::cl_main::CLS.lock().unwrap();
        self.fx.cl_add_predicted_weapon_effects(
            &cl.smoothing.weapon_prediction,
            cls.realtime,
            |org, intensity, r, g, b| {
                crate::cl_view::v_add_light(&mut view, org, intensity, r, g, b);
            },
        );
    }
    fn cl_add_light_styles(&mut self) {
        let mut view = crate::cl_main::VIEW_STATE.lock().unwrap();
        let r_timebasedfx = myq2_common::cvar::cvar_variable_value("r_timebasedfx");
        self.fx.cl_add_light_styles(r_timebasedfx, |i, r, g, b| {
            if i < view.r_lightstyles.len() {
                view.r_lightstyles[i].rgb[0] = r;
                view.r_lightstyles[i].rgb[1] = g;
                view.r_lightstyles[i].rgb[2] = b;
                view.r_lightstyles[i].white = r + g + b;
            }
        });
    }
    fn cl_play_footstep(&mut self, origin: &Vec3, _entity_num: i32) {
        // Play a predicted footstep sound at the given position
        // Use random footstep sound (0-3)
        let idx = (rand::random::<usize>()) % 4;
        let sfx = self.tent.cl_sfx_footsteps[idx];
        if sfx != 0 {
            crate::cl_main::cl_s_start_sound(Some(origin), 0, 0, sfx, 1.0, 1.0, 0.0);
        }
    }
}

// ============================================================
// Server command name strings (for debug display)
// ============================================================

pub static SVC_STRINGS: [&str; 21] = [
    "svc_bad",
    "svc_muzzleflash",
    "svc_muzzlflash2",
    "svc_temp_entity",
    "svc_layout",
    "svc_inventory",
    "svc_nop",
    "svc_disconnect",
    "svc_reconnect",
    "svc_sound",
    "svc_print",
    "svc_stufftext",
    "svc_serverdata",
    "svc_configstring",
    "svc_spawnbaseline",
    "svc_centerprint",
    "svc_download",
    "svc_playerinfo",
    "svc_packetentities",
    "svc_deltapacketentities",
    "svc_frame",
];

// ============================================================
// Message reading/writing helpers — re-exported from myq2_common::common
// ============================================================

pub use myq2_common::common::{
    msg_read_byte, msg_read_short, msg_read_long, msg_read_float,
    msg_read_string, msg_read_data, msg_write_byte, msg_write_string,
};

/// MSG_ReadPos — reads a position and writes into an existing `&mut Vec3`.
/// Wraps the common version which returns a `Vec3` by value.
pub fn msg_read_pos(msg: &mut SizeBuf, pos: &mut Vec3) {
    let v = myq2_common::common::msg_read_pos(msg);
    *pos = v;
}

/// MSG_ReadDir — reads a direction and writes into an existing `&mut Vec3`.
/// Wraps the common version which returns a `Vec3` by value.
pub fn msg_read_dir(msg: &mut SizeBuf, dir: &mut Vec3) {
    let v = myq2_common::common::msg_read_dir(msg);
    *dir = v;
}

/// SZ_Print — append a null-terminated string, merging trailing nulls.

// ============================================================
// External function imports (formerly wrappers)
// ============================================================

use myq2_common::common::com_server_state;
use myq2_common::files::fs_gamedir;
use myq2_common::cmd::cbuf_add_text;
use myq2_common::cmd::cbuf_execute;
use myq2_common::cvar::cvar_set;
use crate::console::{r_register_model, r_register_skin, draw_find_pic, cm_inline_model};
use crate::cl_main::{cl_request_next_download, cl_write_demo_message};

fn cl_clear_state(cl: &mut ClientState) { *cl = ClientState::default(); }

/// CL_ParseMuzzleFlash — reads entity index + weapon byte from net_message.
/// Full dlight/sound effects require sound system wiring; for now we consume
/// the message bytes to keep the parse stream in sync.
///
/// Also confirms predicted weapon fires when receiving our own muzzle flash.
fn cl_parse_muzzle_flash(cl: &mut ClientState, net_message: &mut SizeBuf) {
    let i = msg_read_short(net_message);
    if i < 1 || i >= MAX_EDICTS as i32 {
        com_error(ERR_DROP, "CL_ParseMuzzleFlash: bad entity");
        return;
    }
    let _weapon = msg_read_byte(net_message);

    // Confirm predicted weapon fire if this is our own muzzle flash
    // Player entity number is playernum + 1
    if i == cl.playernum + 1 {
        // Find the most recent unconfirmed prediction and confirm it
        // This provides the visual feedback loop - prediction was correct
        if let Some(effect) = cl.smoothing.weapon_prediction.effects.iter_mut()
            .rev()
            .find(|e| !e.confirmed)
        {
            effect.confirmed = true;
        }
    }

    // Dlight allocation, sound playback, and particle effects will be wired
    // when the sound system (AudioBackend) and dlight/particle subsystems
    // are integrated into the parse context.
    com_dprintf(&format!("CL_ParseMuzzleFlash: ent={} weapon={}\n", i, _weapon));
}

/// CL_ParseMuzzleFlash2 — reads entity index + flash_number from net_message.
/// Full dlight/sound effects require sound system wiring; for now we consume
/// the message bytes to keep the parse stream in sync.
fn cl_parse_muzzle_flash2(_cl: &ClientState, net_message: &mut SizeBuf) {
    let ent = msg_read_short(net_message);
    if ent < 1 || ent >= MAX_EDICTS as i32 {
        com_error(ERR_DROP, "CL_ParseMuzzleFlash2: bad entity");
        return;
    }
    let _flash_number = msg_read_byte(net_message);
    // Monster muzzle flash effects (dlight + sound) will be wired when
    // monster_flash_offset table and sound system are available.
    com_dprintf(&format!("CL_ParseMuzzleFlash2: ent={} flash={}\n", ent, _flash_number));
}

/// Forward to cl_tent module for tent sound registration.
pub fn cl_register_tent_sounds_on(tent: &mut TEntState) {
    crate::cl_tent::cl_register_tent_sounds_on(tent);
}

/// Forward to cl_tent module for parsing temp entities.
pub fn cl_parse_tent_dispatch(
    tent: &mut TEntState,
    fx: &mut ClFxState,
    cl: &ClientState,
    net_message: &mut SizeBuf,
) {
    crate::cl_tent::cl_parse_tent(tent, fx, cl, net_message);
}

// ============================================================
// Download support
// ============================================================

/// Build the full download filename path.
pub fn cl_download_filename(fn_name: &str) -> String {
    if fn_name.starts_with("players") {
        format!("{}/{}", BASEDIRNAME, fn_name)
    } else {
        format!("{}/{}", fs_gamedir(), fn_name)
    }
}

/// Check stufftext for sv_downloadurl and initialize HTTP downloads.
/// R1Q2-style servers send: set sv_downloadurl "http://example.com/q2/"
pub fn cl_check_download_url(stufftext: &str) {
    // Look for sv_downloadurl in various formats:
    // set sv_downloadurl "url"
    // sv_downloadurl "url"
    let text = stufftext.to_lowercase();

    if !text.contains("sv_downloadurl") {
        return;
    }

    // Parse the URL from the stufftext
    // Format variations:
    // set sv_downloadurl "http://example.com/q2/"
    // set sv_downloadurl http://example.com/q2/
    let url = extract_download_url(stufftext);

    if let Some(url) = url {
        let url = url.trim().trim_matches('"').trim();
        if !url.is_empty() {
            com_printf(&format!("Server download URL: {}\n", url));
            cl_http_init(url);
        } else {
            // Empty URL disables HTTP downloads
            cl_http::cl_http_shutdown();
        }
    }
}

/// Extract the URL value from a sv_downloadurl command.
fn extract_download_url(stufftext: &str) -> Option<&str> {
    // Find sv_downloadurl (case insensitive)
    let lower = stufftext.to_lowercase();
    let pos = lower.find("sv_downloadurl")?;

    // Skip past "sv_downloadurl"
    let rest = &stufftext[pos + "sv_downloadurl".len()..];
    let rest = rest.trim_start();

    // Handle quoted or unquoted value
    if rest.starts_with('"') {
        // Find closing quote
        let start = 1;
        let end = rest[1..].find('"').map(|p| p + 1)?;
        Some(&rest[start..end])
    } else {
        // Unquoted - take until whitespace or newline
        let end = rest.find(|c: char| c.is_whitespace() || c == '\n' || c == '\r')
            .unwrap_or(rest.len());
        if end > 0 {
            Some(&rest[..end])
        } else {
            None
        }
    }
}

/// Returns true if the file exists, otherwise it attempts
/// to start a download from the server.
///
/// If cl_http_downloads is enabled and the server provides a download URL,
/// HTTP download is attempted first. Falls back to in-game protocol on failure.
pub fn cl_check_or_download_file(
    cls: &mut ClientStatic,
    filename: &str,
) -> bool {
    if filename.contains("..") {
        com_printf("Refusing to download a path with ..\n");
        return true;
    }

    if myq2_common::files::fs_load_file(filename).is_some() {
        return true;
    }

    // Try async HTTP download if enabled
    let cl_http_downloads = myq2_common::cvar::cvar_variable_value("cl_http_downloads");
    if cl_http_downloads != 0.0 && cl_http::cl_http_available() {
        let dest_path = PathBuf::from(cl_download_filename(filename));

        // Create parent directories if needed
        if let Some(parent) = dest_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        com_printf(&format!("HTTP: Queuing async download {}\n", filename));

        if let Some(_download_id) = cl_http::cl_http_download(filename, &dest_path) {
            // Download queued successfully — game continues while downloading.
            // Progress and completion are polled via cl_http::cl_http_poll()
            // in the main frame loop.
            return true;
        }
        // If queue failed, fall through to in-game download
    }

    // Fall back to in-game download protocol
    cls.download_name = filename.to_string();

    // download to a temp name, and only rename
    // to the real name when done, so if interrupted
    // a runt file wont be left
    cls.download_tempname = com_strip_extension(&cls.download_name);
    cls.download_tempname.push_str(".tmp");

    // check to see if we already have a tmp for this file, if so, try to resume
    let name = cl_download_filename(&cls.download_tempname);

    match fs::OpenOptions::new().read(true).write(true).open(&name) {
        Ok(mut fp) => {
            let len = fp.seek(SeekFrom::End(0)).unwrap_or(0) as i32;
            // Note: original stores FILE* in cls.download; Rust version doesn't have that field.
            // Download file handle management will be handled when full download system is wired up.

            com_printf(&format!("Resuming {}\n", cls.download_name));
            msg_write_byte(&mut cls.netchan.message, ClcOps::StringCmd as i32);
            msg_write_string(
                &mut cls.netchan.message,
                &format!("download {} {}", cls.download_name, len),
            );
        }
        Err(_) => {
            com_printf(&format!("Downloading {}\n", cls.download_name));
            msg_write_byte(&mut cls.netchan.message, ClcOps::StringCmd as i32);
            msg_write_string(
                &mut cls.netchan.message,
                &format!("download {}", cls.download_name),
            );
        }
    }

    cls.download_number += 1;
    false
}

/// Request a download from the server (console command handler).
pub fn cl_download_f(cls: &mut ClientStatic, args: &[&str]) {
    if args.len() != 2 {
        com_printf("Usage: download <filename>\n");
        return;
    }

    let filename = args[1];

    if filename.contains("..") {
        com_printf("Refusing to download a path with ..\n");
        return;
    }

    if myq2_common::files::fs_load_file(filename).is_some() {
        com_printf("File already exists.\n");
        return;
    }

    cls.download_name = filename.to_string();
    com_printf(&format!("Downloading {}\n", cls.download_name));

    cls.download_tempname = com_strip_extension(&cls.download_name);
    cls.download_tempname.push_str(".tmp");

    msg_write_byte(&mut cls.netchan.message, ClcOps::StringCmd as i32);
    msg_write_string(
        &mut cls.netchan.message,
        &format!("download {}", cls.download_name),
    );

    cls.download_number += 1;
}

// ============================================================
// CL_RegisterSounds
// ============================================================

pub fn cl_register_sounds(cl: &mut ClientState, sound: &mut SoundState, tent: &mut TEntState) {
    sound.s_begin_registration();
    cl_register_tent_sounds_on(tent);
    for i in 1..MAX_SOUNDS {
        if cl.configstrings[CS_SOUNDS + i].is_empty() {
            break;
        }
        cl.sound_precache[i] = sound.s_register_sound(&cl.configstrings[CS_SOUNDS + i], &crate::snd_dma::snd_load_file).unwrap_or(0) as i32;
        crate::console::sys_send_key_events();
    }
    sound.s_end_registration(&crate::snd_dma::snd_load_file);
}

// ============================================================
// CL_ParseDownload
// ============================================================

/// A download message has been received from the server.
pub fn cl_parse_download(cls: &mut ClientStatic, net_message: &mut SizeBuf) {
    let size = msg_read_short(net_message);
    let percent = msg_read_byte(net_message);

    if size == -1 {
        com_printf("Server does not have this file.\n");
        cl_request_next_download();
        return;
    }

    // File I/O for download is simplified here; original uses cls.download FILE* field.
    // The full download pipeline will be connected when the file transfer system is wired up.

    net_message.readcount += size;

    if percent != 100 {
        cls.download_percent = percent;
        msg_write_byte(&mut cls.netchan.message, ClcOps::StringCmd as i32);
        cls.netchan.message.print("nextdl");
    } else {
        let oldn = cl_download_filename(&cls.download_tempname);
        let newn = cl_download_filename(&cls.download_name);
        if fs::rename(&oldn, &newn).is_err() {
            com_printf("failed to rename.\n");
        }

        cls.download_percent = 0;
        cl_request_next_download();
    }
}

/// A compressed download message has been received from the server (protocol 35+).
/// Format: short compressed_size, short uncompressed_size, byte percent, compressed data
pub fn cl_parse_zdownload(cls: &mut ClientStatic, net_message: &mut SizeBuf) {
    let compressed_size = msg_read_short(net_message);
    let uncompressed_size = msg_read_short(net_message);
    let percent = msg_read_byte(net_message);

    if compressed_size == -1 {
        com_printf("Server does not have this file.\n");
        cl_request_next_download();
        return;
    }

    // Read the compressed data
    let compressed_data = msg_read_data(net_message, compressed_size as usize);

    // Decompress the data
    match compression::decompress_with_size(&compressed_data, uncompressed_size as usize) {
        Ok(decompressed) => {
            com_dprintf(&format!(
                "svc_zdownload: {} -> {} bytes ({}%)\n",
                compressed_size, uncompressed_size, percent
            ));

            // File I/O for download is simplified here; original uses cls.download FILE* field.
            // The full download pipeline will be connected when the file transfer system is wired up.
            // In a full implementation, we'd write `decompressed` to the download file.
            let _ = decompressed; // Placeholder for actual file write

            if percent != 100 {
                cls.download_percent = percent;
                msg_write_byte(&mut cls.netchan.message, ClcOps::StringCmd as i32);
                cls.netchan.message.print("nextdl");
            } else {
                let oldn = cl_download_filename(&cls.download_tempname);
                let newn = cl_download_filename(&cls.download_name);
                if fs::rename(&oldn, &newn).is_err() {
                    com_printf("failed to rename.\n");
                }

                cls.download_percent = 0;
                cl_request_next_download();
            }
        }
        Err(e) => {
            com_printf(&format!("Error decompressing download: {}\n", e));
            // Skip the rest of this download chunk
        }
    }
}

// ============================================================
// SERVER CONNECTING MESSAGES
// ============================================================

/// Parse server data message.
pub fn cl_parse_server_data(
    cl: &mut ClientState,
    cls: &mut ClientStatic,
    net_message: &mut SizeBuf,
    // cinematic playback not yet wired through here; see scr_play_cinematic call below
) {
    com_dprintf("Serverdata packet received.\n");

    // Execute change map trigger command (R1Q2/Q2Pro feature)
    // Called before clearing state, so commands can access current map info
    crate::cl_main::cl_trigger_change_map();

    cl_clear_state(cl);
    cls.state = ConnState::Connected;

    // parse protocol version number
    let i = msg_read_long(net_message);
    cls.server_protocol = i;

    // Validate protocol version - support 34 (original), 35 (R1Q2), 36 (Q2Pro)
    if com_server_state() != 0 && PROTOCOL_VERSION == 34 {
        // BIG HACK to let demos from release work with the 3.0x patch!!!
        // Local server, allow any version
    } else if i < PROTOCOL_VERSION_MIN || i > PROTOCOL_VERSION_MAX {
        com_error(
            ERR_DROP,
            &format!(
                "Server returned version {}, expected {}-{}",
                i, PROTOCOL_VERSION_MIN, PROTOCOL_VERSION_MAX
            ),
        );
    }

    // Set the protocol on the netchan for features like 1-byte qport
    myq2_common::net_chan::netchan_set_protocol(&mut cls.netchan, i);

    // Log the protocol version for debugging
    if i >= PROTOCOL_R1Q2 {
        let proto_name = if i == PROTOCOL_Q2PRO {
            "Q2Pro"
        } else {
            "R1Q2"
        };
        com_dprintf(&format!("Using {} protocol ({})\n", proto_name, i));
    }

    cl.servercount = msg_read_long(net_message);
    cl.attractloop = msg_read_byte(net_message) != 0;

    // game directory
    let str_val = msg_read_string(net_message);
    cl.gamedir = str_val.clone();

    if !str_val.is_empty() {
        cvar_set("game", &str_val);
    }

    // parse player entity number
    cl.playernum = msg_read_short(net_message);

    // get the full level name
    let str_val = msg_read_string(net_message);

    if cl.playernum == -1 {
        // Playing a cinematic — cl_cin uses the same crate::client types.
        crate::cl_cin::scr_play_cinematic(&str_val, cl, cls);
    } else {
        com_printf("\n\n\x1d\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1f\n");
        com_printf(&format!("\x02{}\n", str_val));
        cl.refresh_prepped = false;
    }
}

/// Parse baseline entity.
pub fn cl_parse_baseline(
    cl_entities: &mut [CEntity],
    net_message: &mut SizeBuf,
) {
    let mut bits: i32 = 0;
    let nullstate = EntityState::default();

    let newnum = crate::cl_ents::cl_parse_entity_bits(net_message, &mut bits);
    if (newnum as usize) < cl_entities.len() {
        let es = &mut cl_entities[newnum as usize].baseline;
        crate::cl_ents::cl_parse_delta(&nullstate, es, newnum, bits, net_message);
    }
}

// ============================================================
// CL_LoadClientinfo
// ============================================================

/// Load the skin, icon, and model for a client.
pub fn cl_load_clientinfo(ci: &mut ClientInfo, s: &str) {
    ci.cinfo = s.to_string();

    // isolate the player's name
    if let Some(pos) = s.find('\\') {
        ci.name = s[..pos].to_string();
        let remainder = &s[pos + 1..];

        if remainder.is_empty() {
            cl_load_default_skin(ci);
            return;
        }

        // isolate the model name
        let (model_name, skin_name) = if let Some(sep) = remainder.find('/') {
            (remainder[..sep].to_string(), remainder[sep + 1..].to_string())
        } else if let Some(sep) = remainder.find('\\') {
            (remainder[..sep].to_string(), remainder[sep + 1..].to_string())
        } else {
            (remainder.to_string(), String::new())
        };

        // model file
        let model_filename = format!("players/{}/tris.md2", model_name);
        ci.model = r_register_model(&model_filename);

        let mut model_name = model_name;
        if ci.model == 0 {
            model_name = "male".to_string();
            ci.model = r_register_model("players/male/tris.md2");
        }

        // skin file
        let skin_filename = format!("players/{}/{}.pcx", model_name, skin_name);
        ci.skin = r_register_skin(&skin_filename);

        // if we don't have the skin and the model wasn't male,
        // see if the male has it (this is for CTF's skins)
        if ci.skin == 0 && !model_name.eq_ignore_ascii_case("male") {
            model_name = "male".to_string();
            ci.model = r_register_model("players/male/tris.md2");

            let skin_filename = format!("players/male/{}.pcx", skin_name);
            ci.skin = r_register_skin(&skin_filename);
        }

        // if we still don't have a skin, default to grunt
        if ci.skin == 0 {
            ci.skin = r_register_skin("players/male/grunt.pcx");
        }

        // weapon file
        let weapon_filename = format!("players/{}/weapon.md2", model_name);
        ci.weaponmodel[0] = r_register_model(&weapon_filename);
        if ci.weaponmodel[0] == 0 && model_name == "cyborg" {
            ci.weaponmodel[0] = r_register_model("players/male/weapon.md2");
        }

        // icon file
        ci.iconname = format!("/players/{}/{}_i.pcx", model_name, skin_name);
        ci.icon = draw_find_pic(&ci.iconname);
    } else {
        ci.name = s.to_string();
        cl_load_default_skin(ci);
        return;
    }

    // must have loaded all data types to be valid
    if ci.skin == 0 || ci.icon == 0 || ci.model == 0 || ci.weaponmodel[0] == 0 {
        ci.skin = 0;
        ci.icon = 0;
        ci.model = 0;
        ci.weaponmodel[0] = 0;
    }
}

fn cl_load_default_skin(ci: &mut ClientInfo) {
    ci.model = r_register_model("players/male/tris.md2");
    ci.weaponmodel = [0; MAX_CLIENTWEAPONMODELS];
    ci.weaponmodel[0] = r_register_model("players/male/weapon.md2");
    ci.skin = r_register_skin("players/male/grunt.pcx");
    ci.iconname = "/players/male/grunt_i.pcx".to_string();
    ci.icon = draw_find_pic(&ci.iconname);
}

/// Load the skin, icon, and model for a client.
pub fn cl_parse_clientinfo(cl: &mut ClientState, player: usize) {
    let s = cl.configstrings[player + CS_PLAYERSKINS].clone();
    cl_load_clientinfo(&mut cl.clientinfo[player], &s);
}

// ============================================================
// CL_ParseConfigString
// ============================================================

pub fn cl_parse_config_string(cl: &mut ClientState, net_message: &mut SizeBuf, fx: &mut ClFxState, sound: &mut SoundState) {
    let i = msg_read_short(net_message) as usize;
    if i >= MAX_CONFIGSTRINGS {
        com_error(ERR_DROP, "configstring > MAX_CONFIGSTRINGS");
        return;
    }

    let s = msg_read_string(net_message);
    let olds = cl.configstrings[i].clone();
    cl.configstrings[i] = s.clone();

    if (CS_LIGHTS..CS_LIGHTS + MAX_LIGHTSTYLES).contains(&i) {
        let lightstyle_str = cl.configstrings[i].clone();
        fx.cl_set_lightstyle(i - CS_LIGHTS, &lightstyle_str);
    } else if i == CS_CDTRACK {
        // CD audio removed
    } else if (CS_MODELS..CS_MODELS + MAX_MODELS).contains(&i) {
        if cl.refresh_prepped {
            cl.model_draw[i - CS_MODELS] = r_register_model(&cl.configstrings[i]);
            if cl.configstrings[i].starts_with('*') {
                cl.model_clip[i - CS_MODELS] = cm_inline_model(&cl.configstrings[i]);
            } else {
                cl.model_clip[i - CS_MODELS] = 0;
            }
        }
    } else if (CS_SOUNDS..CS_SOUNDS + MAX_MODELS).contains(&i) {
        if cl.refresh_prepped {
            cl.sound_precache[i - CS_SOUNDS] = sound.s_register_sound(&cl.configstrings[i], &crate::snd_dma::snd_load_file).unwrap_or(0) as i32;
        }
    } else if (CS_IMAGES..CS_IMAGES + MAX_MODELS).contains(&i) {
        if cl.refresh_prepped {
            cl.image_precache[i - CS_IMAGES] = draw_find_pic(&cl.configstrings[i]);
        }
    } else if (CS_PLAYERSKINS..CS_PLAYERSKINS + MAX_CLIENTS).contains(&i)
        && cl.refresh_prepped && olds != s {
            cl_parse_clientinfo(cl, i - CS_PLAYERSKINS);
        }
}

// ============================================================
// ACTION MESSAGES
// ============================================================

/// Parse a start sound packet from the server.
pub fn cl_parse_start_sound_packet(cl: &ClientState, net_message: &mut SizeBuf, sound: &mut SoundState) {
    let flags = msg_read_byte(net_message);
    let sound_num = msg_read_byte(net_message) as usize;

    let volume = if flags & SND_VOLUME != 0 {
        msg_read_byte(net_message) as f32 / 255.0
    } else {
        DEFAULT_SOUND_PACKET_VOLUME
    };

    let attenuation = if flags & SND_ATTENUATION != 0 {
        msg_read_byte(net_message) as f32 / 64.0
    } else {
        DEFAULT_SOUND_PACKET_ATTENUATION
    };

    let ofs = if flags & SND_OFFSET != 0 {
        msg_read_byte(net_message) as f32 / 1000.0
    } else {
        0.0
    };

    let (ent, channel) = if flags & SND_ENT != 0 {
        let ch = msg_read_short(net_message);
        let e = ch >> 3;
        if e as usize > MAX_EDICTS {
            com_error(
                ERR_DROP,
                &format!("CL_ParseStartSoundPacket: ent = {}", e),
            );
        }
        (e, ch & 7)
    } else {
        (0, 0)
    };

    let mut pos_v: Vec3 = [0.0; 3];
    let pos = if flags & SND_POS != 0 {
        msg_read_pos(net_message, &mut pos_v);
        Some(pos_v)
    } else {
        None
    };

    if sound_num < MAX_SOUNDS && cl.sound_precache[sound_num] != 0 {
        sound.s_start_sound(
            pos,
            ent,
            channel,
            cl.sound_precache[sound_num] as usize,
            volume,
            attenuation,
            ofs,
            cl.frame.servertime,
        );
    }
}

pub fn shownet(net_message: &SizeBuf, cl_shownet_value: f32, s: &str) {
    if cl_shownet_value >= 2.0 {
        com_printf(&format!("{:3}:{}\n", net_message.readcount - 1, s));
    }
}

/// Parse projectile data from a network message with time tracking.
/// This wraps cl_parse_projectiles_with_time for use during server message parsing.
/// In vanilla Q2 the projectile protocol was #if 0'd, but this function enables
/// parsing when a protocol extension (e.g., R1Q2/Q2Pro) includes compact projectile data.
///
/// Call this from the SVC dispatch when a projectile message type is encountered.
pub fn cl_parse_projectiles_dispatch(
    ctx: &mut ParseContext,
    net_message: &mut SizeBuf,
    client_time: i32,
) {
    crate::cl_ents::cl_parse_projectiles_with_time(
        ctx.proj_state,
        net_message,
        client_time,
    );
}

// ============================================================
// CL_ParseServerMessage
// ============================================================

// Use the canonical Console struct from console_types.
pub use crate::console_types::Console;

/// Helper to parse a single command from a decompressed zpacket.
/// This handles the same commands as the main cl_parse_server_message loop
/// but operates on a separate message buffer.
fn cl_parse_decompressed_cmd(
    cmd: i32,
    cl: &mut ClientState,
    cls: &mut ClientStatic,
    con: &mut Console,
    net_message: &mut SizeBuf,
    cl_entities: &mut [CEntity],
    cl_shownet_value: f32,
    ctx: &mut ParseContext,
) {
    if cl_shownet_value >= 2.0 {
        let cmd_usize = cmd as usize;
        if cmd_usize >= SVC_STRINGS.len() {
            com_printf(&format!("{:3}:BAD CMD {} (zpacket)\n", net_message.readcount - 1, cmd));
        } else {
            com_printf(&format!("{:3}:{} (zpacket)\n", net_message.readcount - 1, SVC_STRINGS[cmd_usize]));
        }
    }

    match cmd {
        x if x == SvcOps::Nop as i32 => {}

        x if x == SvcOps::Disconnect as i32 => {
            com_error(ERR_DROP, "Server disconnected\n");
        }

        x if x == SvcOps::Reconnect as i32 => {
            com_printf("Server disconnected, reconnecting\n");
            cls.state = ConnState::Connecting;
            cls.connect_time = -99999.0;
        }

        x if x == SvcOps::Print as i32 => {
            let level = msg_read_byte(net_message);
            if level == PRINT_CHAT {
                ctx.sound.s_start_local_sound("misc/talk.wav", cl.playernum, cl.frame.servertime, &crate::snd_dma::snd_load_file);
                con.ormask = 128;
            }
            let s = msg_read_string(net_message);
            // Chat filtering and logging (R1Q2/Q2Pro feature)
            if level == PRINT_CHAT {
                if let Some(sender) = crate::cl_chat::chat_extract_sender(&s) {
                    let sender_owned = sender.to_string();
                    if let Some(filtered) = crate::cl_chat::chat_process_message(&sender_owned, &s) {
                        com_printf(&filtered);
                    }
                    // If chat_process_message returns None, sender is ignored — skip printing
                } else {
                    // No sender extracted (server message, etc.) — print as-is
                    com_printf(&s);
                }
            } else {
                com_printf(&s);
            }
            con.ormask = 0;
        }

        x if x == SvcOps::CenterPrint as i32 => {
            let s = msg_read_string(net_message);
            crate::cl_scrn::scr_center_print(ctx.scr, cl, &s);
        }

        x if x == SvcOps::StuffText as i32 => {
            let s = msg_read_string(net_message);
            com_dprintf(&format!("stufftext: {}\n", s));
            cl_check_download_url(&s);
            cbuf_add_text(&s);
        }

        x if x == SvcOps::ServerData as i32 => {
            cbuf_execute();
            cl_parse_server_data(cl, cls, net_message);
        }

        x if x == SvcOps::ConfigString as i32 => {
            cl_parse_config_string(cl, net_message, ctx.fx, ctx.sound);
        }

        x if x == SvcOps::Sound as i32 => {
            cl_parse_start_sound_packet(cl, net_message, ctx.sound);
        }

        x if x == SvcOps::SpawnBaseline as i32 => {
            cl_parse_baseline(cl_entities, net_message);
        }

        x if x == SvcOps::TempEntity as i32 => {
            cl_parse_tent_dispatch(ctx.tent, ctx.fx, cl, net_message);
        }

        x if x == SvcOps::MuzzleFlash as i32 => {
            cl_parse_muzzle_flash(cl, net_message);
        }

        x if x == SvcOps::MuzzleFlash2 as i32 => {
            cl_parse_muzzle_flash2(cl, net_message);
        }

        x if x == SvcOps::Download as i32 => {
            cl_parse_download(cls, net_message);
        }

        x if x == SvcOps::Frame as i32 => {
            let svc_strs: [&str; 256] = {
                let mut arr = [""; 256];
                for (i, s) in SVC_STRINGS.iter().enumerate() {
                    arr[i] = s;
                }
                arr
            };
            let mut frame_cb = FrameCallbacks {
                fx: ctx.fx,
                tent: ctx.tent,
                sound: ctx.sound,
                cl_time: cl.time as f32,
            };
            crate::cl_ents::cl_parse_frame(
                cl,
                cls,
                ctx.ent_state,
                net_message,
                cl_shownet_value,
                &svc_strs,
                &mut frame_cb,
            );

            // Clear projectiles for new frame (see main dispatch for full comment)
            crate::cl_ents::cl_clear_projectiles(ctx.proj_state);

            // === Update network smoothing state on frame arrival ===
            // Record packet arrival for adaptive interpolation
            cl.smoothing.adaptive_interp.record_packet(cls.realtime);

            // Record network stats
            cl.smoothing.network_stats.record_packet(
                net_message.cursize as i32,
                cls.realtime,
            );
            cl.smoothing.network_stats.interp_buffer_ms =
                cl.smoothing.adaptive_interp.get_lerp_delay();
        }

        x if x == SvcOps::Inventory as i32 => {
            crate::cl_inv::cl_parse_inventory(cl);
        }

        x if x == SvcOps::Layout as i32 => {
            let s = msg_read_string(net_message);
            cl.layout = s;
        }

        x if x == SvcOps::PlayerInfo as i32
            || x == SvcOps::PacketEntities as i32
            || x == SvcOps::DeltaPacketEntities as i32 =>
        {
            com_error(ERR_DROP, "Out of place frame data (in zpacket)");
        }

        // Nested zpacket not allowed
        x if x == SVC_ZPACKET => {
            com_error(ERR_DROP, "Nested svc_zpacket not allowed");
        }

        _ => {
            com_error(ERR_DROP, &format!("CL_ParseServerMessage: Illegible server message {} in zpacket\n", cmd));
        }
    }
}

/// Parse the entire server message.
pub fn cl_parse_server_message(
    cl: &mut ClientState,
    cls: &mut ClientStatic,
    con: &mut Console,
    net_message: &mut SizeBuf,
    cl_entities: &mut [CEntity],
    cl_shownet_value: f32,
    ctx: &mut ParseContext,
) {
    if cl_shownet_value == 1.0 {
        com_printf(&format!("{} ", net_message.cursize));
    } else if cl_shownet_value >= 2.0 {
        com_printf("------------------\n");
    }

    loop {
        if net_message.readcount > net_message.cursize {
            com_error(ERR_DROP, "CL_ParseServerMessage: Bad server message");
            break;
        }

        let cmd = msg_read_byte(net_message);

        if cmd == -1 {
            shownet(net_message, cl_shownet_value, "END OF MESSAGE");
            break;
        }

        if cl_shownet_value >= 2.0 {
            let cmd_usize = cmd as usize;
            if cmd_usize >= SVC_STRINGS.len() {
                com_printf(&format!("{:3}:BAD CMD {}\n", net_message.readcount - 1, cmd));
            } else {
                shownet(net_message, cl_shownet_value, SVC_STRINGS[cmd_usize]);
            }
        }

        match cmd {
            x if x == SvcOps::Nop as i32 => {}

            x if x == SvcOps::Disconnect as i32 => {
                com_error(ERR_DROP, "Server disconnected\n");
            }

            x if x == SvcOps::Reconnect as i32 => {
                com_printf("Server disconnected, reconnecting\n");
                cls.state = ConnState::Connecting;
                cls.connect_time = -99999.0;
            }

            x if x == SvcOps::Print as i32 => {
                let level = msg_read_byte(net_message);
                if level == PRINT_CHAT {
                    ctx.sound.s_start_local_sound("misc/talk.wav", cl.playernum, cl.frame.servertime, &crate::snd_dma::snd_load_file);
                    con.ormask = 128;
                }
                let s = msg_read_string(net_message);
                // Chat filtering and logging (R1Q2/Q2Pro feature)
                if level == PRINT_CHAT {
                    if let Some(sender) = crate::cl_chat::chat_extract_sender(&s) {
                        let sender_owned = sender.to_string();
                        if let Some(filtered) = crate::cl_chat::chat_process_message(&sender_owned, &s) {
                            com_printf(&filtered);
                        }
                        // If chat_process_message returns None, sender is ignored — skip printing
                    } else {
                        // No sender extracted (server message, etc.) — print as-is
                        com_printf(&s);
                    }
                } else {
                    com_printf(&s);
                }
                con.ormask = 0;
            }

            x if x == SvcOps::CenterPrint as i32 => {
                let s = msg_read_string(net_message);
                crate::cl_scrn::scr_center_print(ctx.scr, cl, &s);
            }

            x if x == SvcOps::StuffText as i32 => {
                let s = msg_read_string(net_message);
                com_dprintf(&format!("stufftext: {}\n", s));

                // Check for sv_downloadurl (R1Q2-style HTTP downloads)
                // Format: set sv_downloadurl "http://example.com/q2/"
                cl_check_download_url(&s);

                cbuf_add_text(&s);
            }

            x if x == SvcOps::ServerData as i32 => {
                cbuf_execute();
                cl_parse_server_data(cl, cls, net_message);
            }

            x if x == SvcOps::ConfigString as i32 => {
                cl_parse_config_string(cl, net_message, ctx.fx, ctx.sound);
            }

            x if x == SvcOps::Sound as i32 => {
                cl_parse_start_sound_packet(cl, net_message, ctx.sound);
            }

            x if x == SvcOps::SpawnBaseline as i32 => {
                cl_parse_baseline(cl_entities, net_message);
            }

            x if x == SvcOps::TempEntity as i32 => {
                cl_parse_tent_dispatch(ctx.tent, ctx.fx, cl, net_message);
            }

            x if x == SvcOps::MuzzleFlash as i32 => {
                cl_parse_muzzle_flash(cl, net_message);
            }

            x if x == SvcOps::MuzzleFlash2 as i32 => {
                cl_parse_muzzle_flash2(cl, net_message);
            }

            x if x == SvcOps::Download as i32 => {
                cl_parse_download(cls, net_message);
            }

            x if x == SVC_ZDOWNLOAD => {
                // R1Q2/Q2Pro compressed download (protocol 35+)
                cl_parse_zdownload(cls, net_message);
            }

            x if x == SvcOps::Frame as i32 => {
                let svc_strs: [&str; 256] = {
                    let mut arr = [""; 256];
                    for (i, s) in SVC_STRINGS.iter().enumerate() {
                        arr[i] = s;
                    }
                    arr
                };
                let mut frame_cb = FrameCallbacks {
                    fx: ctx.fx,
                    tent: ctx.tent,
                    sound: ctx.sound,
                    cl_time: cl.time as f32,
                };
                crate::cl_ents::cl_parse_frame(
                    cl,
                    cls,
                    ctx.ent_state,
                    net_message,
                    cl_shownet_value,
                    &svc_strs,
                    &mut frame_cb,
                );

                // Clear projectiles for new frame and parse if protocol supports it.
                // In vanilla Q2 this was #if 0'd (the compact projectile protocol was
                // never shipped). The functions are wired here for protocol extensions.
                // cl_clear_projectiles resets the 'present' flag each frame so stale
                // projectiles are detected and faded out in cl_add_projectiles.
                crate::cl_ents::cl_clear_projectiles(ctx.proj_state);

                // === Update network smoothing state on frame arrival ===
                cl.smoothing.adaptive_interp.record_packet(cls.realtime);
                cl.smoothing.network_stats.record_packet(
                    net_message.cursize as i32,
                    cls.realtime,
                );
                cl.smoothing.network_stats.interp_buffer_ms =
                    cl.smoothing.adaptive_interp.get_lerp_delay();
            }

            x if x == SvcOps::Inventory as i32 => {
                crate::cl_inv::cl_parse_inventory(cl);
            }

            x if x == SvcOps::Layout as i32 => {
                let s = msg_read_string(net_message);
                cl.layout = s;
            }

            x if x == SVC_ZPACKET => {
                // R1Q2/Q2Pro compressed packet (protocol 35+)
                // Format: short compressed_length, then compressed_length bytes of zlib data
                let compressed_len = msg_read_short(net_message) as usize;

                if compressed_len == 0 {
                    com_error(ERR_DROP, "CL_ParseServerMessage: Zero-length compressed packet");
                    break;
                }

                if compressed_len > MAX_MSGLEN_R1Q2 {
                    com_error(ERR_DROP, &format!(
                        "CL_ParseServerMessage: Compressed packet too large ({})",
                        compressed_len
                    ));
                    break;
                }

                // Read the compressed data
                let compressed_data = msg_read_data(net_message, compressed_len);

                // Decompress the packet
                let decompressed = match compression::decompress_packet(&compressed_data, MAX_MSGLEN_R1Q2) {
                    Some(data) => data,
                    None => {
                        com_error(ERR_DROP, "CL_ParseServerMessage: Failed to decompress packet");
                        break;
                    }
                };

                com_dprintf(&format!(
                    "svc_zpacket: {} -> {} bytes\n",
                    compressed_len,
                    decompressed.len()
                ));

                // Create a temporary SizeBuf for the decompressed data and process it
                // We recursively parse the decompressed messages
                let mut decompressed_msg = SizeBuf::new(decompressed.len() as i32);
                decompressed_msg.data = decompressed;
                decompressed_msg.cursize = decompressed_msg.data.len() as i32;

                // Process the decompressed messages (inner loop)
                while decompressed_msg.readcount < decompressed_msg.cursize {
                    let inner_cmd = msg_read_byte(&mut decompressed_msg);
                    if inner_cmd == -1 {
                        break;
                    }

                    // Process inner command - delegate to individual handlers
                    // Note: We handle the most common message types here
                    cl_parse_decompressed_cmd(
                        inner_cmd,
                        cl,
                        cls,
                        con,
                        &mut decompressed_msg,
                        cl_entities,
                        cl_shownet_value,
                        ctx,
                    );
                }
            }

            x if x == SvcOps::PlayerInfo as i32
                || x == SvcOps::PacketEntities as i32
                || x == SvcOps::DeltaPacketEntities as i32 =>
            {
                com_error(ERR_DROP, "Out of place frame data");
            }

            _ => {
                com_error(ERR_DROP, "CL_ParseServerMessage: Illegible server message\n");
            }
        }
    }

    crate::cl_scrn::cl_add_netgraph(ctx.scr, cls, cl);

    if cls.demo_recording && !cls.demo_waiting {
        cl_write_demo_message();
    }
}

// ============================================================
// Unit tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------
    // cl_download_filename tests
    // -------------------------------------------------------

    #[test]
    fn test_download_filename_players_prefix() {
        // Player skins always download to BASEDIRNAME, regardless of gamedir
        let result = cl_download_filename("players/male/grunt.pcx");
        assert_eq!(result, format!("{}/players/male/grunt.pcx", BASEDIRNAME));
    }

    #[test]
    fn test_download_filename_non_players() {
        // Non-player files download to the current gamedir
        let gamedir = fs_gamedir();
        let result = cl_download_filename("models/weapons/v_blast/tris.md2");
        assert_eq!(result, format!("{}/models/weapons/v_blast/tris.md2", gamedir));
    }

    #[test]
    fn test_download_filename_empty_string() {
        let gamedir = fs_gamedir();
        let result = cl_download_filename("");
        assert_eq!(result, format!("{}/", gamedir));
    }

    #[test]
    fn test_download_filename_players_at_start_only() {
        // "players" must be a prefix, not just appear anywhere in the path
        let gamedir = fs_gamedir();
        let result = cl_download_filename("maps/players_arena.bsp");
        assert_eq!(result, format!("{}/maps/players_arena.bsp", gamedir));
    }

    // -------------------------------------------------------
    // extract_download_url tests
    // -------------------------------------------------------

    #[test]
    fn test_extract_download_url_quoted() {
        let input = r#"set sv_downloadurl "http://example.com/q2/""#;
        let result = extract_download_url(input);
        assert_eq!(result, Some("http://example.com/q2/"));
    }

    #[test]
    fn test_extract_download_url_unquoted() {
        let input = "set sv_downloadurl http://example.com/q2/";
        let result = extract_download_url(input);
        assert_eq!(result, Some("http://example.com/q2/"));
    }

    #[test]
    fn test_extract_download_url_case_insensitive() {
        let input = r#"set SV_DOWNLOADURL "http://example.com/q2/""#;
        let result = extract_download_url(input);
        assert_eq!(result, Some("http://example.com/q2/"));
    }

    #[test]
    fn test_extract_download_url_mixed_case() {
        let input = r#"set Sv_DownloadUrl "http://example.com/q2/""#;
        let result = extract_download_url(input);
        assert_eq!(result, Some("http://example.com/q2/"));
    }

    #[test]
    fn test_extract_download_url_no_url() {
        let input = "set some_other_cvar value";
        let result = extract_download_url(input);
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_download_url_empty_quoted() {
        let input = r#"set sv_downloadurl """#;
        let result = extract_download_url(input);
        assert_eq!(result, Some(""));
    }

    #[test]
    fn test_extract_download_url_without_set_prefix() {
        // Some servers send the cvar directly without "set"
        let input = r#"sv_downloadurl "http://myserver.com/files/""#;
        let result = extract_download_url(input);
        assert_eq!(result, Some("http://myserver.com/files/"));
    }

    #[test]
    fn test_extract_download_url_trailing_whitespace() {
        let input = "set sv_downloadurl http://example.com/q2/  \n";
        let result = extract_download_url(input);
        assert_eq!(result, Some("http://example.com/q2/"));
    }

    // -------------------------------------------------------
    // SVC_STRINGS tests
    // -------------------------------------------------------

    #[test]
    fn test_svc_strings_length() {
        assert_eq!(SVC_STRINGS.len(), 21);
    }

    #[test]
    fn test_svc_strings_known_entries() {
        assert_eq!(SVC_STRINGS[0], "svc_bad");
        assert_eq!(SVC_STRINGS[1], "svc_muzzleflash");
        assert_eq!(SVC_STRINGS[6], "svc_nop");
        assert_eq!(SVC_STRINGS[7], "svc_disconnect");
        assert_eq!(SVC_STRINGS[12], "svc_serverdata");
        assert_eq!(SVC_STRINGS[20], "svc_frame");
    }

    // -------------------------------------------------------
    // shownet tests
    // -------------------------------------------------------

    #[test]
    fn test_shownet_no_output_below_threshold() {
        // shownet with value < 2.0 should not panic or crash
        let msg = SizeBuf::new(16);
        shownet(&msg, 0.0, "test");
        shownet(&msg, 1.0, "test");
        shownet(&msg, 1.99, "test");
    }

    #[test]
    fn test_shownet_at_threshold() {
        // shownet with value >= 2.0 prints; just verify it doesn't panic
        let msg = SizeBuf::new(16);
        shownet(&msg, 2.0, "svc_nop");
        shownet(&msg, 3.0, "svc_frame");
    }

    // -------------------------------------------------------
    // msg_read_pos / msg_read_dir wrapper tests
    // -------------------------------------------------------

    #[test]
    fn test_msg_read_pos_writes_into_vec3() {
        // Create a SizeBuf with 3 encoded coordinates (each as short * 0.125)
        let mut msg = SizeBuf::new(64);
        // Write 3 i16 values at the start of the buffer: 800, -400, 1200
        let vals: [i16; 3] = [800, -400, 1200];
        let mut offset = 0;
        for v in &vals {
            let bytes = v.to_le_bytes();
            msg.data[offset] = bytes[0];
            msg.data[offset + 1] = bytes[1];
            offset += 2;
        }
        msg.cursize = 6;
        msg.readcount = 0;

        let mut pos: Vec3 = [0.0; 3];
        msg_read_pos(&mut msg, &mut pos);

        // msg_read_pos reads 3 shorts and multiplies by 1/8 = 0.125
        let expected = [800.0 * 0.125, -400.0 * 0.125, 1200.0 * 0.125];
        for i in 0..3 {
            assert!((pos[i] - expected[i]).abs() < 0.01,
                "pos[{}] = {}, expected {}", i, pos[i], expected[i]);
        }
    }

    #[test]
    fn test_msg_read_dir_writes_into_vec3() {
        // msg_read_dir reads a single byte index into the direction table.
        // Just verify it writes *something* into the output and doesn't panic.
        let mut msg = SizeBuf::new(16);
        msg.data[0] = 0; // direction index 0
        msg.cursize = 1;

        let mut dir: Vec3 = [999.0; 3];
        msg_read_dir(&mut msg, &mut dir);

        // After reading, dir should have been overwritten
        // (direction 0 may be all zeros or a specific normal)
        let was_written = dir[0] != 999.0 || dir[1] != 999.0 || dir[2] != 999.0;
        assert!(was_written, "msg_read_dir should overwrite the output vec3");
    }

    // -------------------------------------------------------
    // cl_check_download_url tests (integration-ish, checks routing)
    // -------------------------------------------------------

    #[test]
    fn test_cl_check_download_url_ignores_non_download_text() {
        // Should not panic when given unrelated stufftext
        cl_check_download_url("set cl_maxfps 125");
        cl_check_download_url("");
        cl_check_download_url("echo hello world");
    }

    #[test]
    fn test_cl_check_download_url_handles_empty_url() {
        // Empty URL between quotes should disable HTTP downloads (not panic)
        cl_check_download_url(r#"set sv_downloadurl """#);
    }

    // -------------------------------------------------------
    // com_strip_extension (used by download temp name logic)
    // -------------------------------------------------------

    #[test]
    fn test_strip_extension_for_download_temp() {
        let name = "maps/q2dm1.bsp";
        let stripped = com_strip_extension(name);
        assert_eq!(stripped, "maps/q2dm1");

        let temp = format!("{}.tmp", stripped);
        assert_eq!(temp, "maps/q2dm1.tmp");
    }

    #[test]
    fn test_strip_extension_no_extension() {
        let name = "models/weapon";
        let stripped = com_strip_extension(name);
        assert_eq!(stripped, "models/weapon");
    }

    #[test]
    fn test_strip_extension_multiple_dots() {
        let name = "textures/e1u1.wall.pcx";
        let stripped = com_strip_extension(name);
        assert_eq!(stripped, "textures/e1u1.wall");
    }
}
