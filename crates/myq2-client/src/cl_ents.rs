// cl_ents.rs -- entity parsing and management
// Converted from: myq2-original/client/cl_ents.c
//
// Copyright (C) 1997-2001 Id Software, Inc.
// Licensed under the GNU General Public License v2 or later.

use crate::client::*;
use myq2_common::q_shared::*;
use rayon::prelude::*;
use myq2_common::qcommon::{
    SizeBuf, UPDATE_BACKUP, SVC_PLAYERINFO, SVC_PACKETENTITIES, MAX_PROJECTILES,
    U_ORIGIN1, U_ORIGIN2, U_ORIGIN3, U_ANGLE1, U_ANGLE2, U_ANGLE3,
    U_FRAME8, U_FRAME16, U_EVENT, U_REMOVE, U_MOREBITS1, U_MOREBITS2, U_MOREBITS3,
    U_NUMBER16, U_MODEL, U_MODEL2, U_MODEL3, U_MODEL4,
    U_RENDERFX8, U_RENDERFX16, U_EFFECTS8, U_EFFECTS16,
    U_SKIN8, U_SKIN16, U_SOUND, U_SOLID, U_OLDORIGIN,
    PS_M_TYPE, PS_M_ORIGIN, PS_M_VELOCITY, PS_M_TIME, PS_M_FLAGS,
    PS_M_GRAVITY, PS_M_DELTA_ANGLES, PS_VIEWOFFSET, PS_VIEWANGLES,
    PS_KICKANGLES, PS_BLEND, PS_FOV, PS_WEAPONINDEX, PS_WEAPONFRAME, PS_RDFLAGS,
};
use myq2_common::common::{
    com_printf, com_dprintf,
    msg_read_byte, msg_read_short, msg_read_long,
    msg_read_char, msg_read_coord, msg_read_angle, msg_read_angle16,
    msg_read_pos, msg_read_data,
};

// =========================================================================
// Global state
// =========================================================================

/// Protocol profiling bit counts
pub static mut BITCOUNTS: [i32; 32] = [0; 32];

// The actual cl_entities and cl_parse_entities arrays live in the client
// module's global state. They are passed in by reference to the functions
// below via the ClientEntState parameter.

/// Mutable entity state passed into functions that need the global arrays.
pub struct ClientEntState {
    pub cl_entities: Vec<CEntity>,
    pub cl_parse_entities: Vec<EntityState>,
}

impl Default for ClientEntState {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientEntState {
    pub fn new() -> Self {
        // Parallel initialization of entity arrays - beneficial for MAX_EDICTS (1024) entities
        let cl_entities: Vec<CEntity> = (0..MAX_EDICTS)
            .into_par_iter()
            .map(|_| CEntity::default())
            .collect();
        let cl_parse_entities: Vec<EntityState> = (0..MAX_PARSE_ENTITIES)
            .into_par_iter()
            .map(|_| EntityState::default())
            .collect();
        Self {
            cl_entities,
            cl_parse_entities,
        }
    }
}

// =========================================================================
// Callback trait — functions defined in other modules that this module calls
// =========================================================================

pub trait ClientCallbacks {
    fn cl_entity_event(&mut self, ent: &EntityState);
    fn cl_teleporter_particles(&mut self, ent: &EntityState);
    fn add_stain(&mut self, org: &Vec3, intensity: f32, r: f32, g: f32, b: f32, a: f32, stain_type: StainType);
    fn shownet(&self, s: &str);
    fn scr_end_loading_plaque(&mut self, clear: bool);
    fn cl_check_prediction_error(&mut self);
    fn v_add_entity(&mut self, ent: &Entity);
    fn v_add_light(&mut self, org: &Vec3, intensity: f32, r: f32, g: f32, b: f32);
    fn r_register_model(&self, name: &str) -> i32;
    fn r_register_skin(&self, name: &str) -> i32;
    fn get_skin_name(&self, skin: i32) -> Option<String>;
    fn developer_searchpath(&self, who: i32) -> i32;
    fn cl_rocket_trail(&mut self, start: &Vec3, end: &Vec3, old: &mut CEntity);
    fn cl_blaster_trail(&mut self, start: &Vec3, end: &Vec3);
    fn cl_blaster_trail2(&mut self, start: &Vec3, end: &Vec3);
    fn cl_diminishing_trail(&mut self, start: &Vec3, end: &Vec3, old: &mut CEntity, flags: u32);
    fn cl_fly_effect(&mut self, ent: &mut CEntity, origin: &Vec3);
    fn cl_bfg_particles(&mut self, ent: &Entity);
    fn cl_trap_particles(&mut self, ent: &Entity);
    fn cl_flag_trail(&mut self, start: &Vec3, end: &Vec3, color: f32);
    fn cl_tag_trail(&mut self, start: &Vec3, end: &Vec3, color: f32);
    fn cl_tracker_trail(&mut self, start: &Vec3, end: &Vec3, particle_color: i32);
    fn cl_tracker_shell(&mut self, origin: &Vec3);
    fn cl_ionripper_trail(&mut self, start: &Vec3, end: &Vec3);
    fn cl_add_tents(&mut self);
    fn cl_add_particles(&mut self);
    fn cl_add_dlights(&mut self);
    fn cl_add_light_styles(&mut self);
    /// Play a predicted footstep sound at the given position
    fn cl_play_footstep(&mut self, origin: &Vec3, entity_num: i32);
}

// =========================================================================
// FRAME PARSING
// =========================================================================

/// CL_ParseEntityBits — Returns the entity number and the header bits.
pub fn cl_parse_entity_bits(net_message: &mut SizeBuf, bits: &mut i32) -> i32 {
    let mut total = msg_read_byte(net_message);
    if total & U_MOREBITS1 != 0 {
        let b = msg_read_byte(net_message);
        total |= b << 8;
    }
    if total & U_MOREBITS2 != 0 {
        let b = msg_read_byte(net_message);
        total |= b << 16;
    }
    if total & U_MOREBITS3 != 0 {
        let b = msg_read_byte(net_message);
        total |= b << 24;
    }

    // count the bits for net profiling
    // SAFETY: single-threaded access to static profiling counters
    unsafe {
        for i in 0..32 {
            if total & (1 << i) != 0 {
                BITCOUNTS[i] += 1;
            }
        }
    }

    let number: i32;
    if total & U_NUMBER16 != 0 {
        number = msg_read_short(net_message);
    } else {
        number = msg_read_byte(net_message);
    }

    *bits = total;
    number
}

/// CL_ParseDelta — Can go from either a baseline or a previous packet_entity.
pub fn cl_parse_delta(
    from: &EntityState,
    to: &mut EntityState,
    number: i32,
    bits: i32,
    net_message: &mut SizeBuf,
) {
    // set everything to the state we are delta'ing from
    *to = from.clone();

    to.old_origin = vector_copy(&from.origin);
    to.number = number;

    if bits & U_MODEL != 0 {
        to.modelindex = msg_read_byte(net_message);
    }
    if bits & U_MODEL2 != 0 {
        to.modelindex2 = msg_read_byte(net_message);
    }
    if bits & U_MODEL3 != 0 {
        to.modelindex3 = msg_read_byte(net_message);
    }
    if bits & U_MODEL4 != 0 {
        to.modelindex4 = msg_read_byte(net_message);
    }

    if bits & U_FRAME8 != 0 {
        to.frame = msg_read_byte(net_message);
    }
    if bits & U_FRAME16 != 0 {
        to.frame = msg_read_short(net_message);
    }

    if (bits & U_SKIN8 != 0) && (bits & U_SKIN16 != 0) {
        // used for laser colors
        to.skinnum = msg_read_long(net_message);
    } else if bits & U_SKIN8 != 0 {
        to.skinnum = msg_read_byte(net_message);
    } else if bits & U_SKIN16 != 0 {
        to.skinnum = msg_read_short(net_message);
    }

    if (bits & (U_EFFECTS8 | U_EFFECTS16)) == (U_EFFECTS8 | U_EFFECTS16) {
        to.effects = msg_read_long(net_message) as u32;
    } else if bits & U_EFFECTS8 != 0 {
        to.effects = msg_read_byte(net_message) as u32;
    } else if bits & U_EFFECTS16 != 0 {
        to.effects = msg_read_short(net_message) as u32;
    }

    if (bits & (U_RENDERFX8 | U_RENDERFX16)) == (U_RENDERFX8 | U_RENDERFX16) {
        to.renderfx = msg_read_long(net_message);
    } else if bits & U_RENDERFX8 != 0 {
        to.renderfx = msg_read_byte(net_message);
    } else if bits & U_RENDERFX16 != 0 {
        to.renderfx = msg_read_short(net_message);
    }

    if bits & U_ORIGIN1 != 0 {
        to.origin[0] = msg_read_coord(net_message);
    }
    if bits & U_ORIGIN2 != 0 {
        to.origin[1] = msg_read_coord(net_message);
    }
    if bits & U_ORIGIN3 != 0 {
        to.origin[2] = msg_read_coord(net_message);
    }

    if bits & U_ANGLE1 != 0 {
        to.angles[0] = msg_read_angle(net_message);
    }
    if bits & U_ANGLE2 != 0 {
        to.angles[1] = msg_read_angle(net_message);
    }
    if bits & U_ANGLE3 != 0 {
        to.angles[2] = msg_read_angle(net_message);
    }

    if bits & U_OLDORIGIN != 0 {
        to.old_origin = msg_read_pos(net_message);
    }

    if bits & U_SOUND != 0 {
        to.sound = msg_read_byte(net_message);
    }

    if bits & U_EVENT != 0 {
        to.event = msg_read_byte(net_message);
    } else {
        to.event = 0;
    }

    if bits & U_SOLID != 0 {
        to.solid = msg_read_short(net_message);
    }
}

/// CL_DeltaEntity — Parses deltas from the given base and adds the resulting
/// entity to the current frame.
pub fn cl_delta_entity(
    frame: &mut Frame,
    newnum: i32,
    old: &EntityState,
    bits: i32,
    cl: &mut ClientState,
    ent_state: &mut ClientEntState,
    net_message: &mut SizeBuf,
) {
    let state_idx = (cl.parse_entities & (MAX_PARSE_ENTITIES as i32 - 1)) as usize;
    cl.parse_entities += 1;
    frame.num_entities += 1;

    cl_parse_delta(old, &mut ent_state.cl_parse_entities[state_idx], newnum, bits, net_message);

    let state = ent_state.cl_parse_entities[state_idx].clone();
    let ent = &mut ent_state.cl_entities[newnum as usize];

    // some data changes will force no lerping
    if state.modelindex != ent.current.modelindex
        || state.modelindex2 != ent.current.modelindex2
        || state.modelindex3 != ent.current.modelindex3
        || state.modelindex4 != ent.current.modelindex4
        || (state.origin[0] - ent.current.origin[0]).abs() > 512.0
        || (state.origin[1] - ent.current.origin[1]).abs() > 512.0
        || (state.origin[2] - ent.current.origin[2]).abs() > 512.0
        || state.event == EV_PLAYER_TELEPORT
        || state.event == EV_OTHER_TELEPORT
    {
        ent.serverframe = -99;
    }

    if ent.serverframe != cl.frame.serverframe - 1 {
        // wasn't in last update, so initialize some things
        ent.trailcount = 1024; // for diminishing rocket / grenade trails
        // duplicate the current state so lerping doesn't hurt anything
        ent.prev = state.clone();
        if state.event == EV_OTHER_TELEPORT || state.event == EV_PLAYER_TELEPORT {
            ent.prev.origin = vector_copy(&state.origin);
            ent.lerp_origin = vector_copy(&state.origin);
            // Teleporting entities appear instantly (no fade-in)
            ent.spawn_time = 0;
        } else {
            ent.prev.origin = vector_copy(&state.old_origin);
            ent.lerp_origin = vector_copy(&state.old_origin);
            // Set spawn time for fade-in effect on newly appearing entities
            ent.spawn_time = cl.frame.servertime;
        }
        // Reset velocity tracking on gap
        ent.velocity.valid = false;
        ent.velocity.angular_valid = false;
        ent.missed_frames = 0;
    } else {
        // shuffle the last state to previous
        ent.prev = ent.current.clone();

        // === Velocity tracking for extrapolation ===
        // Calculate velocity from position delta between frames.
        // Use actual time delta when available, otherwise fall back to
        // SERVER_FRAMETIME_SEC (100ms = 0.1s, the standard Q2 server tick rate).
        let actual_delta_ms = cl.frame.servertime - ent.velocity.prev_time;
        let frame_time = if actual_delta_ms > 0 && actual_delta_ms <= SERVER_FRAMETIME_MS * 3 {
            // Use actual measured delta (convert ms to seconds)
            actual_delta_ms as f32 / 1000.0
        } else {
            // Fall back to standard server frame time
            SERVER_FRAMETIME_SEC
        };
        for i in 0..3 {
            ent.velocity.velocity[i] = (state.origin[i] - ent.prev.origin[i]) / frame_time;
        }
        ent.velocity.prev_origin = ent.prev.origin;
        ent.velocity.prev_time = cl.frame.servertime - SERVER_FRAMETIME_MS;
        ent.velocity.last_update_time = cl.frame.servertime;
        ent.velocity.valid = true;

        // === Calculate angular velocity for rotation extrapolation ===
        // This allows smooth rotation continuation during packet loss
        for i in 0..3 {
            // Calculate angle delta with proper wrapping (-180 to 180)
            let mut angle_delta = state.angles[i] - ent.prev.angles[i];
            // Normalize to -180..180 range
            while angle_delta > 180.0 { angle_delta -= 360.0; }
            while angle_delta < -180.0 { angle_delta += 360.0; }
            ent.velocity.angular_velocity[i] = angle_delta / frame_time;
        }
        ent.velocity.prev_angles = ent.prev.angles;
        ent.velocity.angular_valid = true;

        // Reset missed frames counter on successful update
        ent.missed_frames = 0;

        // === Update smoothing state for enhanced interpolation ===
        // Add to spline history for Catmull-Rom interpolation
        if cl.smoothing.cubic_interp_enabled && (newnum as usize) < cl.smoothing.spline_histories.len() {
            cl.smoothing.spline_histories[newnum as usize].add(cl.frame.servertime, state.origin);
        }

        // Update dead reckoning state for player entities (modelindex 255 = player model)
        if state.modelindex == 255 && (newnum as usize) < cl.smoothing.dead_reckoning.len() {
            cl.smoothing.dead_reckoning[newnum as usize].update(state.origin, cl.frame.servertime);
        }

        // === Update mover/platform velocity tracking ===
        // Track brush entity velocities for platform prediction
        cl.smoothing.mover_prediction.update_entity(
            newnum as usize,
            &state.origin,
            cl.frame.servertime,
            state.solid,
        );
    }

    // Update animation state tracking
    if state.frame != ent.current.frame {
        ent.anim_state.oldframe = ent.current.frame;
        ent.anim_state.frame = state.frame;
        // Calculate actual frame duration from time since last animation change.
        // This adapts to different animation rates (10fps, 20fps, etc.).
        let time_since_last_change = cl.frame.servertime - ent.last_update_time;
        ent.anim_state.frame_duration = if time_since_last_change > 0 && time_since_last_change <= SERVER_FRAMETIME_MS * 5 {
            // Use actual measured duration
            time_since_last_change as f32
        } else {
            // Fall back to standard server frame time (100ms)
            SERVER_FRAMETIME_MS as f32
        };
        ent.anim_state.frame_time = 0.0;
        ent.anim_state.animating = true;

        // Add frame sample to history for spline interpolation
        ent.anim_state.add_frame_sample(state.frame, cl.frame.servertime);
    }
    ent.anim_state.last_server_frame = cl.frame.serverframe;
    ent.last_update_time = cl.frame.servertime;

    ent.serverframe = cl.frame.serverframe;
    ent.current = state.clone();

    // Store effects for persistence during packet loss
    // Only store non-zero effects (when player has a powerup/shell)
    if state.effects != 0 {
        ent.last_effects = state.effects as i32;
    }
    if state.renderfx != 0 {
        ent.last_renderfx = state.renderfx as i32;
    }
}

/// CL_ParsePacketEntities — An svc_packetentities has just been parsed,
/// deal with the rest of the data stream.
pub fn cl_parse_packet_entities(
    oldframe: Option<&Frame>,
    newframe: &mut Frame,
    cl: &mut ClientState,
    ent_state: &mut ClientEntState,
    net_message: &mut SizeBuf,
    cl_shownet_value: f32,
) {
    newframe.parse_entities = cl.parse_entities;
    newframe.num_entities = 0;

    // delta from the entities present in oldframe
    let mut oldindex: i32 = 0;
    let mut oldstate: EntityState;
    let mut oldnum: i32;

    if oldframe.is_none() {
        oldnum = 99999;
        oldstate = EntityState::default();
    } else {
        let of = oldframe.unwrap();
        if oldindex >= of.num_entities {
            oldnum = 99999;
            oldstate = EntityState::default();
        } else {
            let idx = ((of.parse_entities + oldindex) & (MAX_PARSE_ENTITIES as i32 - 1)) as usize;
            oldstate = ent_state.cl_parse_entities[idx].clone();
            oldnum = oldstate.number;
        }
    }

    loop {
        let mut bits: i32 = 0;
        let newnum = cl_parse_entity_bits(net_message, &mut bits);
        if newnum >= MAX_EDICTS as i32 {
            panic!("CL_ParsePacketEntities: bad number:{}", newnum);
        }

        if net_message.readcount > net_message.cursize {
            panic!("CL_ParsePacketEntities: end of message");
        }

        if newnum == 0 {
            break;
        }

        while oldnum < newnum {
            // one or more entities from the old packet are unchanged
            if cl_shownet_value == 3.0 {
                com_dprintf(&format!("   unchanged: {}\n", oldnum));
            }
            let os = oldstate.clone();
            cl_delta_entity(newframe, oldnum, &os, 0, cl, ent_state, net_message);

            oldindex += 1;

            if let Some(of) = oldframe {
                if oldindex >= of.num_entities {
                    oldnum = 99999;
                } else {
                    let idx = ((of.parse_entities + oldindex) & (MAX_PARSE_ENTITIES as i32 - 1)) as usize;
                    oldstate = ent_state.cl_parse_entities[idx].clone();
                    oldnum = oldstate.number;
                }
            } else {
                oldnum = 99999;
            }
        }

        if bits & U_REMOVE != 0 {
            // the entity present in oldframe is not in the current frame
            if cl_shownet_value == 3.0 {
                com_dprintf(&format!("   remove: {}\n", newnum));
            }
            if oldnum != newnum {
                com_dprintf("U_REMOVE: oldnum != newnum\n");
            }

            oldindex += 1;

            if let Some(of) = oldframe {
                if oldindex >= of.num_entities {
                    oldnum = 99999;
                } else {
                    let idx = ((of.parse_entities + oldindex) & (MAX_PARSE_ENTITIES as i32 - 1)) as usize;
                    oldstate = ent_state.cl_parse_entities[idx].clone();
                    oldnum = oldstate.number;
                }
            } else {
                oldnum = 99999;
            }
            continue;
        }

        if oldnum == newnum {
            // delta from previous state
            if cl_shownet_value == 3.0 {
                com_dprintf(&format!("   delta: {}\n", newnum));
            }
            let os = oldstate.clone();
            cl_delta_entity(newframe, newnum, &os, bits, cl, ent_state, net_message);

            oldindex += 1;

            if let Some(of) = oldframe {
                if oldindex >= of.num_entities {
                    oldnum = 99999;
                } else {
                    let idx = ((of.parse_entities + oldindex) & (MAX_PARSE_ENTITIES as i32 - 1)) as usize;
                    oldstate = ent_state.cl_parse_entities[idx].clone();
                    oldnum = oldstate.number;
                }
            } else {
                oldnum = 99999;
            }
            continue;
        }

        if oldnum > newnum {
            // delta from baseline
            if cl_shownet_value == 3.0 {
                com_dprintf(&format!("   baseline: {}\n", newnum));
            }
            let baseline = ent_state.cl_entities[newnum as usize].baseline.clone();
            cl_delta_entity(newframe, newnum, &baseline, bits, cl, ent_state, net_message);
            continue;
        }
    }

    // any remaining entities in the old frame are copied over
    while oldnum != 99999 {
        if cl_shownet_value == 3.0 {
            com_dprintf(&format!("   unchanged: {}\n", oldnum));
        }
        let os = oldstate.clone();
        cl_delta_entity(newframe, oldnum, &os, 0, cl, ent_state, net_message);

        oldindex += 1;

        if let Some(of) = oldframe {
            if oldindex >= of.num_entities {
                oldnum = 99999;
            } else {
                let idx = ((of.parse_entities + oldindex) & (MAX_PARSE_ENTITIES as i32 - 1)) as usize;
                oldstate = ent_state.cl_parse_entities[idx].clone();
                oldnum = oldstate.number;
            }
        } else {
            oldnum = 99999;
        }
    }
}

/// CL_ParsePlayerstate
pub fn cl_parse_playerstate(
    oldframe: Option<&Frame>,
    newframe: &mut Frame,
    net_message: &mut SizeBuf,
    attractloop: bool,
) {
    let state = &mut newframe.playerstate;

    // clear to old value before delta parsing
    if let Some(old) = oldframe {
        *state = old.playerstate.clone();
    } else {
        *state = PlayerState::default();
    }

    let flags = msg_read_short(net_message);

    // parse the pmove_state_t
    if flags & PS_M_TYPE != 0 {
        state.pmove.pm_type = match msg_read_byte(net_message) {
            0 => PmType::Normal,
            1 => PmType::Spectator,
            2 => PmType::Dead,
            3 => PmType::Gib,
            4 => PmType::Freeze,
            _ => PmType::Normal,
        };
    }

    if flags & PS_M_ORIGIN != 0 {
        state.pmove.origin[0] = msg_read_short(net_message) as i16;
        state.pmove.origin[1] = msg_read_short(net_message) as i16;
        state.pmove.origin[2] = msg_read_short(net_message) as i16;
    }

    if flags & PS_M_VELOCITY != 0 {
        state.pmove.velocity[0] = msg_read_short(net_message) as i16;
        state.pmove.velocity[1] = msg_read_short(net_message) as i16;
        state.pmove.velocity[2] = msg_read_short(net_message) as i16;
    }

    if flags & PS_M_TIME != 0 {
        state.pmove.pm_time = msg_read_byte(net_message) as u8;
    }

    if flags & PS_M_FLAGS != 0 {
        state.pmove.pm_flags = msg_read_byte(net_message) as u8;
    }

    if flags & PS_M_GRAVITY != 0 {
        state.pmove.gravity = msg_read_short(net_message) as i16;
    }

    if flags & PS_M_DELTA_ANGLES != 0 {
        state.pmove.delta_angles[0] = msg_read_short(net_message) as i16;
        state.pmove.delta_angles[1] = msg_read_short(net_message) as i16;
        state.pmove.delta_angles[2] = msg_read_short(net_message) as i16;
    }

    if attractloop {
        state.pmove.pm_type = PmType::Freeze; // demo playback
    }

    // parse the rest of the player_state_t
    if flags & PS_VIEWOFFSET != 0 {
        state.viewoffset[0] = msg_read_char(net_message) as f32 * 0.25;
        state.viewoffset[1] = msg_read_char(net_message) as f32 * 0.25;
        state.viewoffset[2] = msg_read_char(net_message) as f32 * 0.25;
    }

    if flags & PS_VIEWANGLES != 0 {
        state.viewangles[0] = msg_read_angle16(net_message);
        state.viewangles[1] = msg_read_angle16(net_message);
        state.viewangles[2] = msg_read_angle16(net_message);
    }

    if flags & PS_KICKANGLES != 0 {
        state.kick_angles[0] = msg_read_char(net_message) as f32 * 0.25;
        state.kick_angles[1] = msg_read_char(net_message) as f32 * 0.25;
        state.kick_angles[2] = msg_read_char(net_message) as f32 * 0.25;
    }

    if flags & PS_WEAPONINDEX != 0 {
        state.gunindex = msg_read_byte(net_message);
    }

    if flags & PS_WEAPONFRAME != 0 {
        state.gunframe = msg_read_byte(net_message);
        state.gunoffset[0] = msg_read_char(net_message) as f32 * 0.25;
        state.gunoffset[1] = msg_read_char(net_message) as f32 * 0.25;
        state.gunoffset[2] = msg_read_char(net_message) as f32 * 0.25;
        state.gunangles[0] = msg_read_char(net_message) as f32 * 0.25;
        state.gunangles[1] = msg_read_char(net_message) as f32 * 0.25;
        state.gunangles[2] = msg_read_char(net_message) as f32 * 0.25;
    }

    if flags & PS_BLEND != 0 {
        state.blend[0] = msg_read_byte(net_message) as f32 / 255.0;
        state.blend[1] = msg_read_byte(net_message) as f32 / 255.0;
        state.blend[2] = msg_read_byte(net_message) as f32 / 255.0;
        state.blend[3] = msg_read_byte(net_message) as f32 / 255.0;
    }

    if flags & PS_FOV != 0 {
        state.fov = msg_read_byte(net_message) as f32;
    }

    if flags & PS_RDFLAGS != 0 {
        state.rdflags = msg_read_byte(net_message);
    }

    // parse stats
    let statbits = msg_read_long(net_message);
    for i in 0..MAX_STATS {
        if statbits & (1 << i) != 0 {
            state.stats[i] = msg_read_short(net_message) as i16;
        }
    }
}

/// CL_FireEntityEvents
pub fn cl_fire_entity_events(
    frame: &Frame,
    ent_state: &ClientEntState,
    callbacks: &mut dyn ClientCallbacks,
) {
    for pnum in 0..frame.num_entities {
        let num = ((frame.parse_entities + pnum) & (MAX_PARSE_ENTITIES as i32 - 1)) as usize;
        let s1 = &ent_state.cl_parse_entities[num];

        if s1.event != 0 {
            callbacks.cl_entity_event(s1);
        }

        // add stains if moving...
        if s1.origin[0] != s1.old_origin[0]
            || s1.origin[1] != s1.old_origin[1]
            || s1.origin[2] != s1.old_origin[2]
        {
            let num_val = 20 + (rand::random::<i32>().unsigned_abs() as i32 % 75);
            if s1.effects & EF_GIB != 0 {
                callbacks.add_stain(
                    &s1.origin,
                    (20 + (num_val / 10)) as f32,
                    (num_val / 2) as f32,
                    0.0,
                    0.0,
                    (num_val * 2) as f32,
                    StainType::Modulate,
                );
            }
            if s1.effects & EF_GREENGIB != 0 {
                callbacks.add_stain(
                    &s1.origin,
                    (20 + (num_val / 10)) as f32,
                    0.0,
                    (num_val / 2) as f32,
                    0.0,
                    (num_val * 2) as f32,
                    StainType::Modulate,
                );
            }
        }

        // EF_TELEPORTER acts like an event, but is not cleared each frame
        if s1.effects & EF_TELEPORTER != 0 {
            callbacks.cl_teleporter_particles(s1);
        }
    }
}

/// CL_ParseFrame
pub fn cl_parse_frame(
    cl: &mut ClientState,
    cls: &mut ClientStatic,
    ent_state: &mut ClientEntState,
    net_message: &mut SizeBuf,
    cl_shownet_value: f32,
    svc_strings: &[&str; 256],
    callbacks: &mut dyn ClientCallbacks,
) {
    cl.frame = Frame::default();

    cl.frame.serverframe = msg_read_long(net_message);
    cl.frame.deltaframe = msg_read_long(net_message);
    cl.frame.servertime = cl.frame.serverframe * 100;

    // BIG HACK to let old demos continue to work
    if cls.server_protocol != 26 {
        cl.surpresscount = msg_read_byte(net_message);
    }

    if cl_shownet_value == 3.0 {
        com_dprintf(&format!(
            "   frame:{}  delta:{}\n",
            cl.frame.serverframe, cl.frame.deltaframe
        ));
    }

    // If the frame is delta compressed from data that we
    // no longer have available, we must suck up the rest of
    // the frame, but not use it, then ask for a non-compressed
    // message
    let old: Option<Frame>;
    if cl.frame.deltaframe <= 0 {
        cl.frame.valid = true; // uncompressed frame
        old = None;
        cls.demo_waiting = false; // we can start recording now
    } else {
        let old_frame = cl.frames[(cl.frame.deltaframe as usize) & (UPDATE_BACKUP as usize - 1)].clone();
        if !old_frame.valid {
            // should never happen
            com_printf("Delta from invalid frame (not supposed to happen!).\n");
        }
        if old_frame.serverframe != cl.frame.deltaframe {
            com_printf("Delta frame too old.\n");
        } else if cl.parse_entities - old_frame.parse_entities > (MAX_PARSE_ENTITIES as i32 - 128) {
            com_printf("Delta parse_entities too old.\n");
        } else {
            cl.frame.valid = true; // valid delta parse
        }
        old = Some(old_frame);
    }

    // Clamp client time to be within one server frame of the last received frame.
    // This prevents interpolation from going too far ahead (future) or too far
    // behind (past) the server's reported time. The window is SERVER_FRAMETIME_MS
    // because that's the expected time between server updates at 10Hz.
    if cl.time > cl.frame.servertime {
        cl.time = cl.frame.servertime;
    } else if cl.time < cl.frame.servertime - SERVER_FRAMETIME_MS {
        cl.time = cl.frame.servertime - SERVER_FRAMETIME_MS;
    }

    // read areabits
    let len = msg_read_byte(net_message) as usize;
    let areabits_data = msg_read_data(net_message, len);
    for i in 0..len.min(cl.frame.areabits.len()) {
        cl.frame.areabits[i] = areabits_data[i];
    }

    // read playerinfo
    let cmd = msg_read_byte(net_message);
    callbacks.shownet(svc_strings[cmd as usize]);
    if cmd != SVC_PLAYERINFO {
        panic!("CL_ParseFrame: not playerinfo");
    }
    cl_parse_playerstate(old.as_ref(), &mut cl.frame, net_message, cl.attractloop);

    // read packet entities
    let cmd = msg_read_byte(net_message);
    callbacks.shownet(svc_strings[cmd as usize]);
    if cmd != SVC_PACKETENTITIES {
        panic!("CL_ParseFrame: not packetentities");
    }
    // SAFETY: We need both &mut cl.frame and &mut cl simultaneously.
    // This is safe because cl_parse_packet_entities only modifies cl.frame
    // and cl.parse_entities / cl_parse_entities array, which don't overlap.
    let cl_ptr = cl as *mut ClientState;
    cl_parse_packet_entities(old.as_ref(), &mut cl.frame, unsafe { &mut *cl_ptr }, ent_state, net_message, cl_shownet_value);

    // save the frame off in the backup array for later delta comparisons
    let idx = (cl.frame.serverframe as usize) & (UPDATE_BACKUP as usize - 1);
    cl.frames[idx] = cl.frame.clone();

    if cl.frame.valid {
        // getting a valid frame message ends the connection process
        if cls.state != ConnState::Active {
            cls.state = ConnState::Active;
            cl.force_refdef = true;
            // Notify auto-reconnect system of successful connection
            crate::cl_main::cl_auto_reconnect_success();
            cl.predicted_origin[0] = cl.frame.playerstate.pmove.origin[0] as f32 * 0.125;
            cl.predicted_origin[1] = cl.frame.playerstate.pmove.origin[1] as f32 * 0.125;
            cl.predicted_origin[2] = cl.frame.playerstate.pmove.origin[2] as f32 * 0.125;
            cl.predicted_angles = vector_copy(&cl.frame.playerstate.viewangles);
            if cls.disable_servercount != cl.servercount && cl.refresh_prepped {
                callbacks.scr_end_loading_plaque(true);
            }
        }
        cl.sound_prepped = true; // can start mixing ambient sounds

        // === Calculate and record ping from server frame acknowledgment ===
        // The server frame number corresponds to the command it processed
        // Find when that command was sent and calculate round-trip time
        let cmd_idx = (cl.frame.serverframe as usize) & (crate::client::CMD_BACKUP - 1);
        let cmd_time = cl.cmd_time[cmd_idx];
        if cmd_time > 0 {
            let ping = cls.realtime - cmd_time;
            if ping > 0 && ping < 1000 {
                cl.smoothing.network_stats.record_ping(ping, cls.realtime);
            }
        }

        // Update valid frame tracking for packet loss detection
        cl.last_valid_frame_time = cls.realtime;
        cl.packet_loss_frames = 0;

        // === Add snapshot to buffer for improved interpolation ===
        cl.smoothing.snapshot_buffer.add_snapshot(
            cl.frame.servertime,
            cl.frame.serverframe,
            cls.realtime,
        );

        // === Track packet loss by sequence gaps ===
        // If deltaframe is valid, check for gaps
        if cl.frame.deltaframe > 0 {
            let expected_frame = cl.frame.deltaframe + 1;
            if cl.frame.serverframe > expected_frame {
                // We missed some frames
                let missed = cl.frame.serverframe - expected_frame;
                cl.smoothing.network_stats.record_loss(expected_frame, cl.frame.serverframe);

                // Track this for stats display
                if missed > 0 {
                    cl.smoothing.network_stats.packets_lost += missed as u64;
                }
            }
        }

        // fire entity events
        cl_fire_entity_events(&cl.frame, ent_state, callbacks);
        callbacks.cl_check_prediction_error();
    } else {
        // Invalid frame - increment packet loss counter
        cl.packet_loss_frames += 1;

        // Also track in network stats
        cl.smoothing.network_stats.packets_lost += 1;
    }
}

// =========================================================================
// INTERPOLATE BETWEEN FRAMES TO GET RENDERING PARMS
// =========================================================================

/// S_RegisterSexedModel — determine the correct player model path for
/// gendered model/skin combinations.
pub fn s_register_sexed_model(
    ent: &EntityState,
    base: &str,
    configstrings: &[String],
    callbacks: &dyn ClientCallbacks,
) -> i32 {
    // determine what model the client is using
    let mut model = String::new();
    let n = CS_PLAYERSKINS + (ent.number as usize) - 1;
    if n < configstrings.len() && !configstrings[n].is_empty() {
        if let Some(pos) = configstrings[n].find('\\') {
            let after = &configstrings[n][pos + 1..];
            if let Some(slash) = after.find('/') {
                model = after[..slash].to_string();
            } else {
                model = after.to_string();
            }
        }
    }

    // if we can't figure it out, they're male
    if model.is_empty() {
        model = "male".to_string();
    }

    let buffer = format!("players/{}/{}", model, &base[1..]);
    let mut mdl = callbacks.r_register_model(&buffer);

    if mdl == 0 {
        // not found, try default weapon model
        let buffer2 = format!("players/{}/weapon.md2", model);
        mdl = callbacks.r_register_model(&buffer2);
        if mdl == 0 {
            // no, revert to the male model
            let buffer3 = format!("players/{}/{}", "male", &base[1..]);
            mdl = callbacks.r_register_model(&buffer3);
            if mdl == 0 {
                // last try, default male weapon.md2
                mdl = callbacks.r_register_model("players/male/weapon.md2");
            }
        }
    }

    mdl
}

/// CL_AddPacketEntities
pub fn cl_add_packet_entities(
    frame: &Frame,
    cl: &mut ClientState,
    ent_state: &mut ClientEntState,
    callbacks: &mut dyn ClientCallbacks,
) {
    // bonus items rotate at a fixed rate
    let autorotate = anglemod(cl.time as f32 / 10.0);

    // brush models can auto animate their frames
    let autoanim = 2 * cl.time / 1000;

    for pnum in 0..frame.num_entities {
        let s1 = &ent_state.cl_parse_entities
            [((frame.parse_entities + pnum) & (MAX_PARSE_ENTITIES as i32 - 1)) as usize];

        let cent = &mut ent_state.cl_entities[s1.number as usize];

        let mut effects = s1.effects;
        let mut renderfx = s1.renderfx;

        // === Shell effect persistence during packet loss ===
        // During packet loss, powerup effects may be missing from the entity state.
        // Use the last known effects to maintain visual continuity (e.g., quad/pent shells).
        // Only apply persistence for player entities (modelindex 255) and recently active effects.
        let frames_since_update = cl.frame.serverframe - cent.serverframe;
        let is_player = s1.modelindex == 255;

        if is_player && frames_since_update > 0 && frames_since_update <= 5 {
            // Check if current effects are missing shell effects that we recently had
            let shell_effects: u32 = EF_PENT | EF_QUAD | EF_DOUBLE | EF_HALF_DAMAGE | EF_COLOR_SHELL;
            let shell_renderfx: i32 = RF_SHELL_RED | RF_SHELL_BLUE | RF_SHELL_GREEN
                | RF_SHELL_DOUBLE | RF_SHELL_HALF_DAM;

            // If current state has no shell effects but we had them recently, restore them
            if (effects & shell_effects) == 0 && (cent.last_effects as u32 & shell_effects) != 0 {
                effects |= cent.last_effects as u32 & shell_effects;
            }
            if (renderfx & shell_renderfx) == 0 && (cent.last_renderfx & shell_renderfx) != 0 {
                renderfx |= cent.last_renderfx & shell_renderfx;
            }
        }

        let mut ent = Entity::default();

        // set frame - with client-side animation continuation during packet loss
        if effects & EF_ANIM01 != 0 {
            ent.frame = autoanim & 1;
        } else if effects & EF_ANIM23 != 0 {
            ent.frame = 2 + (autoanim & 1);
        } else if effects & EF_ANIM_ALL != 0 {
            ent.frame = autoanim;
        } else if effects & EF_ANIM_ALLFAST != 0 {
            ent.frame = cl.time / 100;
        } else if cl.cl_anim_continue && cent.anim_state.animating {
            // Check if we should use client-predicted animation frame
            let frames_since_update = cl.frame.serverframe - cent.anim_state.last_server_frame;

            // Continue animation during packet loss or short delays
            if cl.packet_loss_frames > 0 || (frames_since_update > 0 && frames_since_update <= 3) {
                // Use cl_predict_animation_frame for client-side animation continuation
                let time_since_update = (cl.time - cent.last_update_time) as f32;
                let (predicted_frame, _predicted_oldframe, _predicted_backlerp) =
                    cl_predict_animation_frame(cent, time_since_update, cl.cl_anim_continue);

                // During packet loss, allow more frames to advance (up to 10 instead of 3)
                let max_advance = if cl.packet_loss_frames > 0 { 10 } else { 3 };
                let frame_delta = predicted_frame - cent.anim_state.frame;
                ent.frame = cent.anim_state.frame + frame_delta.min(max_advance);
            } else {
                ent.frame = s1.frame;
            }
        } else {
            ent.frame = s1.frame;
        }

        // quad and pent can do different things on client
        if effects & EF_PENT != 0 {
            effects &= !EF_PENT;
            effects |= EF_COLOR_SHELL;
            renderfx |= RF_SHELL_RED;
        }

        if effects & EF_QUAD != 0 {
            effects &= !EF_QUAD;
            effects |= EF_COLOR_SHELL;
            renderfx |= RF_SHELL_BLUE;
        }

        // PMM
        if effects & EF_DOUBLE != 0 {
            effects &= !EF_DOUBLE;
            effects |= EF_COLOR_SHELL;
            renderfx |= RF_SHELL_DOUBLE;
        }

        if effects & EF_HALF_DAMAGE != 0 {
            effects &= !EF_HALF_DAMAGE;
            effects |= EF_COLOR_SHELL;
            renderfx |= RF_SHELL_HALF_DAM;
        }

        // === Animation frame interpolation ===
        // Try spline interpolation first for smoother multi-frame animation
        if let Some((spline_frame, spline_oldframe, spline_backlerp)) = cent.anim_state.get_spline_frame(cl.time) {
            ent.frame = spline_frame;
            ent.oldframe = spline_oldframe;
            ent.backlerp = spline_backlerp;
        } else {
            // Fall back to standard animation-specific backlerp for smoother frame transitions
            ent.oldframe = cent.prev.frame;

            // Calculate animation-specific backlerp based on frame transition timing
            if cent.current.frame != cent.prev.frame && cent.anim_state.animating {
                // Frame changed - interpolate between old and new frame
                // Use time since frame change for smoother animation
                let time_in_frame = (cl.time - cent.last_update_time) as f32;
                let frame_lerp = (time_in_frame / cent.anim_state.frame_duration.max(50.0)).clamp(0.0, 1.0);
                ent.backlerp = 1.0 - frame_lerp;
            } else {
                // Same frame or not animating - use position-based lerp
                ent.backlerp = 1.0 - cl.lerpfrac;
            }
        }

        // === Calculate entity priority for interpolation quality ===
        // Use frustum-aware priority for better off-screen entity handling
        let is_player = s1.modelindex == 255;
        let is_projectile = effects & (EF_ROCKET | EF_BLASTER | EF_HYPERBLASTER | EF_GRENADE | EF_GIB | EF_GREENGIB) != 0;

        // Get view forward vector for frustum check
        // If v_forward is not yet computed, compute it from predicted angles
        let view_forward = if cl.v_forward[0] != 0.0 || cl.v_forward[1] != 0.0 || cl.v_forward[2] != 0.0 {
            cl.v_forward
        } else {
            // Compute forward from predicted angles as fallback
            let mut fwd = [0.0f32; 3];
            angle_vectors(&cl.predicted_angles, Some(&mut fwd), None, None);
            fwd
        };

        let entity_priority = cl.smoothing.priority_system.calculate_priority_with_frustum(
            &cl.refdef.vieworg,
            &view_forward,
            &cent.current.origin,
            is_player,
            is_projectile,
            false, // not tracking attacker here
        );

        if renderfx & (RF_FRAMELERP | RF_BEAM) != 0 {
            // step origin discretely, because the frames
            // do the animation properly
            ent.origin = vector_copy(&cent.current.origin);
            ent.oldorigin = vector_copy(&cent.current.old_origin);
        } else {
            // === Enhanced interpolation with smoothness features ===
            // Use priority to determine interpolation quality
            let use_advanced = matches!(entity_priority,
                crate::cl_smooth::EntityPriority::Critical |
                crate::cl_smooth::EntityPriority::High);

            // Use advanced smoothing with spline/dead reckoning for player entities
            if is_player && use_advanced {
                // Player entity - use advanced smoothing with dead reckoning
                ent.origin = cl_smooth_entity_origin_advanced(
                    s1.number as usize,
                    cent,
                    &cl.smoothing,
                    cl.lerpfrac,
                    cl.cl_timenudge,
                    cl.cl_extrapolate,
                    cl.cl_extrapolate_max,
                    cl.frame.servertime,
                    cl.time,
                    800.0, // standard Quake 2 gravity
                );
            } else if is_projectile && use_advanced {
                // Projectile entity - use aggressive velocity extrapolation
                // Rockets (EF_ROCKET) are not gravity affected, grenades and gibs are
                let is_gravity_affected = effects & (EF_GRENADE | EF_GIB | EF_GREENGIB) != 0;
                ent.origin = cl_smooth_projectile_origin(
                    cent,
                    cl.lerpfrac,
                    cl.time,
                    800.0, // standard Quake 2 gravity
                    is_gravity_affected,
                );
            } else if use_advanced {
                // High priority non-player/non-projectile entity - use standard smoothing with extrapolation
                ent.origin = cl_smooth_entity_origin(
                    cent,
                    &cent.prev.origin,
                    &cent.current.origin,
                    cl.lerpfrac,
                    cl.cl_timenudge,
                    cl.cl_extrapolate,
                    cl.cl_extrapolate_max,
                    cl.frame.servertime,
                    cl.time,
                );
            } else {
                // Low priority entity - use simple interpolation (cheaper)
                for i in 0..3 {
                    ent.origin[i] = cent.prev.origin[i]
                        + cl.lerpfrac * (cent.current.origin[i] - cent.prev.origin[i]);
                }

                // During packet loss, apply velocity extrapolation via cl_extrapolate_entity_position
                // to prevent entity freezing. Uses cl_extrapolate cvar to enable/disable.
                if cl.packet_loss_frames > 0 {
                    ent.origin = cl_extrapolate_entity_position(
                        cent,
                        &ent.origin,
                        cl.time,
                        cl.cl_extrapolate_max,
                        cl.cl_extrapolate,
                    );
                }
            }

            // Apply packet loss concealment for entities missing from recent frames.
            // cl_conceal_packet_loss extrapolates entity position based on velocity
            // when the entity hasn't been updated by the server for a few frames.
            // This prevents entities from "freezing" during short packet loss bursts.
            if cl.packet_loss_frames > 0 {
                let mut concealed_origin = ent.origin;
                if cl_conceal_packet_loss(cent, cl.frame.serverframe, &mut concealed_origin, cl.cl_extrapolate) {
                    ent.origin = concealed_origin;
                }
            }

            ent.oldorigin = ent.origin;
        }

        // create a new entity

        // tweak the color of beams
        if renderfx & RF_BEAM != 0 {
            // the four beam colors are encoded in 32 bits of skinnum (hack)
            ent.alpha = 0.30;
            ent.skinnum = (s1.skinnum >> ((rand::random::<i32>().unsigned_abs() as i32 % 4) * 8)) & 0xff;
            ent.model = 0;
        } else {
            // set skin
            if s1.modelindex == 255 {
                // use custom player skin
                ent.skinnum = 0;
                let ci = &cl.clientinfo[(s1.skinnum & 0xff) as usize];
                ent.skin = ci.skin;
                ent.model = ci.model;
                if ent.skin == 0 || ent.model == 0 {
                    ent.skin = cl.baseclientinfo.skin;
                    ent.model = cl.baseclientinfo.model;
                }

                // PGM
                if renderfx & RF_USE_DISGUISE != 0 {
                    if let Some(skin_name) = callbacks.get_skin_name(ent.skin) {
                        if skin_name.starts_with("players/male") {
                            ent.skin = callbacks.r_register_skin("players/male/disguise.pcx");
                            ent.model = callbacks.r_register_model("players/male/tris.md2");
                        } else if skin_name.starts_with("players/female") {
                            ent.skin = callbacks.r_register_skin("players/female/disguise.pcx");
                            ent.model = callbacks.r_register_model("players/female/tris.md2");
                        } else if skin_name.starts_with("players/cyborg") {
                            ent.skin = callbacks.r_register_skin("players/cyborg/disguise.pcx");
                            ent.model = callbacks.r_register_model("players/cyborg/tris.md2");
                        }
                    }
                }
                // PGM

                // === Client-side footstep prediction for other players ===
                // During packet loss, predict footstep sounds based on movement velocity
                // to maintain audio continuity for other players' movement
                if s1.number != cl.playernum + 1 { // Don't predict for self (server handles that)
                    let entity_num = s1.number as usize;
                    // Check if on ground (using pmove flags would be ideal, but we use velocity heuristic)
                    let is_on_ground = cent.velocity.velocity[2].abs() < 50.0;

                    // Update footstep prediction with current position
                    let should_play = if cl.packet_loss_frames > 0 && cent.velocity.valid {
                        // During packet loss, use velocity-based prediction
                        let time_since_update = cl.time - cent.velocity.last_update_time;
                        cl.smoothing.footstep_prediction.predict_during_loss(
                            entity_num,
                            &cent.velocity.velocity,
                            cl.time,
                            time_since_update,
                        )
                    } else {
                        // Normal update - check if moved enough for a footstep
                        cl.smoothing.footstep_prediction.update_entity(
                            entity_num,
                            &ent.origin,
                            cl.time,
                            is_on_ground,
                        )
                    };

                    // Play predicted footstep sound
                    if should_play && is_on_ground {
                        callbacks.cl_play_footstep(&ent.origin, s1.number);
                    }

                    // === Player animation continuation during packet loss ===
                    // Update animation state from server, or continue during loss
                    if cl.packet_loss_frames > 0 && cent.velocity.valid {
                        // Packet loss - continue animation based on velocity
                        if let Some(predicted_frame) = cl.smoothing.player_anim.continue_animation(
                            entity_num,
                            cl.time,
                        ) {
                            ent.frame = predicted_frame;
                        }
                    } else {
                        // Normal update - record animation state for future prediction
                        cl.smoothing.player_anim.update_from_server(
                            entity_num,
                            s1.frame,
                            &cent.velocity.velocity,
                            cl.time,
                        );
                    }
                }
            } else {
                ent.skinnum = s1.skinnum;
                ent.skin = 0;
                ent.model = cl.model_draw[s1.modelindex as usize];
            }
        }

        // Translucent render effect (used for black hole model)
        if renderfx == RF_TRANSLUCENT {
            ent.alpha = 0.70;
        }

        // render effects (fullbright, translucent, etc)
        if effects & EF_COLOR_SHELL != 0 {
            ent.flags = 0; // renderfx go on color shell entity
        } else {
            ent.flags = renderfx;
        }

        // calculate angles
        if effects & EF_ROTATE != 0 {
            // some bonus items auto-rotate
            ent.angles[0] = 0.0;
            ent.angles[2] = 0.0;

            // mattx86: bobbing_items (ENABLE_BOBBING_ITEMS)
            let bob_scale = (0.005 + s1.number as f32 * 0.00001) * 1.10;
            let bob = 5.0 + ((cl.time as f32 + 1000.0) * bob_scale).cos() * 5.0;

            // Update item rotation tracking for smooth continuation during packet loss
            let entity_num = s1.number as usize;
            cl.smoothing.item_rotation.update(entity_num, autorotate, bob, cl.time);

            // During packet loss, extrapolate rotation and bob for smooth movement
            if cl.packet_loss_frames > 0 {
                // Try to get extrapolated rotation angle
                if let Some(extrapolated_angle) = cl.smoothing.item_rotation.get_extrapolated_angle(entity_num, cl.time) {
                    ent.angles[1] = extrapolated_angle;
                } else {
                    ent.angles[1] = autorotate;
                }

                // Try to get extrapolated bob
                if let Some(extrapolated_bob) = cl.smoothing.item_rotation.get_extrapolated_bob(entity_num, cl.time, bob_scale) {
                    ent.oldorigin[2] += extrapolated_bob;
                    ent.origin[2] += extrapolated_bob;
                } else {
                    ent.oldorigin[2] += bob;
                    ent.origin[2] += bob;
                }
            } else {
                ent.angles[1] = autorotate;
                ent.oldorigin[2] += bob;
                ent.origin[2] += bob;
            }
        } else if effects & EF_SPINNINGLIGHTS != 0 {
            // RAFAEL
            ent.angles[0] = 0.0;
            ent.angles[1] = anglemod(cl.time as f32 / 2.0) + s1.angles[1];
            ent.angles[2] = 180.0;
            {
                let mut forward = [0.0f32; 3];
                angle_vectors(&ent.angles, Some(&mut forward), None, None);
                let start = vector_ma(&ent.origin, 64.0, &forward);
                callbacks.v_add_light(&start, 100.0, 1.0, 0.0, 0.0);
            }
        } else {
            // interpolate angles with optional extrapolation during packet loss
            for i in 0..3 {
                let a1 = cent.current.angles[i];
                let a2 = cent.prev.angles[i];
                ent.angles[i] = lerp_angle(a2, a1, cl.lerpfrac);
            }

            // Apply angular velocity extrapolation during packet loss
            // This continues smooth rotation when packets are dropped
            if cl.packet_loss_frames > 0 && cent.velocity.angular_valid {
                let time_since_update = (cl.time - cent.velocity.last_update_time) as f32 / 1000.0;
                // Limit extrapolation to 500ms to prevent runaway rotation
                if time_since_update > 0.0 && time_since_update < 0.5 {
                    // Only extrapolate if there's meaningful rotation (>5 deg/sec)
                    for i in 0..3 {
                        if cent.velocity.angular_velocity[i].abs() > 5.0 {
                            let extrap = cent.velocity.angular_velocity[i] * time_since_update;
                            // Limit extrapolation to prevent huge jumps
                            let clamped_extrap = extrap.clamp(-45.0, 45.0);
                            ent.angles[i] = anglemod(ent.angles[i] + clamped_extrap);
                        }
                    }
                }
            }
        }

        if s1.number == cl.playernum + 1 {
            ent.flags |= RF_VIEWERMODEL; // only draw from mirrors
            // Add lights for player effects (entity is RF_VIEWERMODEL so not drawn normally)

            if effects & EF_FLAG1 != 0 {
                callbacks.v_add_light(&ent.origin, 225.0, 1.0, 0.1, 0.1);
            } else if effects & EF_FLAG2 != 0 {
                callbacks.v_add_light(&ent.origin, 225.0, 0.1, 0.1, 1.0);
            } else if effects & EF_TAGTRAIL != 0 {
                callbacks.v_add_light(&ent.origin, 225.0, 1.0, 1.0, 0.0);
            } else if effects & EF_TRACKERTRAIL != 0 {
                callbacks.v_add_light(&ent.origin, 225.0, -1.0, -1.0, -1.0);
            }

            continue;
        }

        // if set to invisible, skip
        if s1.modelindex == 0 {
            continue;
        }

        if effects & EF_BFG != 0 {
            ent.flags |= RF_TRANSLUCENT;
            ent.alpha = 0.30;
        }

        // RAFAEL
        if effects & EF_PLASMA != 0 {
            ent.flags |= RF_TRANSLUCENT;
            ent.alpha = 0.6;
        }

        if effects & EF_SPHERETRANS != 0 {
            ent.flags |= RF_TRANSLUCENT;
            // PMM - *sigh* yet more EF overloading
            if effects & EF_TRACKERTRAIL != 0 {
                ent.alpha = 0.6;
            } else {
                ent.alpha = 0.3;
            }
        }

        // === Apply spawn fade-in effect ===
        // New entities fade in over time to prevent visual pop-in
        if cent.spawn_time > 0 && cl.smoothing.entity_fadein.enabled {
            let spawn_alpha = cl.smoothing.entity_fadein.calculate_alpha(cl.time, cent.spawn_time);
            if spawn_alpha < 1.0 {
                // Multiply existing alpha by spawn alpha
                ent.alpha = if ent.alpha > 0.0 { ent.alpha * spawn_alpha } else { spawn_alpha };
                ent.flags |= RF_TRANSLUCENT;
            }
        }

        // add to refresh list
        callbacks.v_add_entity(&ent);

        // color shells generate a separate entity for the main model
        if effects & EF_COLOR_SHELL != 0 {
            if renderfx & RF_SHELL_HALF_DAM != 0
                && callbacks.developer_searchpath(2) == 2
                    && renderfx & (RF_SHELL_RED | RF_SHELL_BLUE | RF_SHELL_DOUBLE) != 0 {
                        renderfx &= !RF_SHELL_HALF_DAM;
                    }

            if renderfx & RF_SHELL_DOUBLE != 0
                && callbacks.developer_searchpath(2) == 2 {
                    if renderfx & (RF_SHELL_RED | RF_SHELL_BLUE | RF_SHELL_GREEN) != 0 {
                        renderfx &= !RF_SHELL_DOUBLE;
                    }
                    if renderfx & RF_SHELL_RED != 0 {
                        renderfx |= RF_SHELL_BLUE;
                    } else if renderfx & RF_SHELL_BLUE != 0 {
                        if renderfx & RF_SHELL_GREEN != 0 {
                            renderfx &= !RF_SHELL_BLUE;
                        } else {
                            renderfx |= RF_SHELL_GREEN;
                        }
                    }
                }

            ent.flags = renderfx | RF_TRANSLUCENT;
            ent.alpha = 0.30;
            callbacks.v_add_entity(&ent);
        }

        ent.skin = 0; // never use a custom skin on others
        ent.skinnum = 0;
        ent.flags = 0;
        ent.alpha = 0.0;

        // duplicate for linked models
        if s1.modelindex2 != 0 {
            if s1.modelindex2 == 255 {
                // custom weapon
                let ci = &cl.clientinfo[(s1.skinnum & 0xff) as usize];
                let mut wi = (s1.skinnum >> 8) as usize; // 0 is default weapon model
                if !cl_vwep_enabled(cl) || wi > MAX_CLIENTWEAPONMODELS - 1 {
                    wi = 0;
                }
                ent.model = ci.weaponmodel[wi];
                if ent.model == 0 {
                    if wi != 0 {
                        ent.model = ci.weaponmodel[0];
                    }
                    if ent.model == 0 {
                        ent.model = cl.baseclientinfo.weaponmodel[0];
                    }
                }
            } else {
                ent.model = cl.model_draw[s1.modelindex2 as usize];
            }

            // PMM - check for the defender sphere shell .. make it translucent
            let cs_idx = CS_MODELS + s1.modelindex2 as usize;
            if cs_idx < cl.configstrings.len()
                && cl.configstrings[cs_idx].eq_ignore_ascii_case("models/items/shell/tris.md2")
            {
                ent.alpha = 0.32;
                ent.flags = RF_TRANSLUCENT;
            }

            callbacks.v_add_entity(&ent);

            // PGM - make sure these get reset.
            ent.flags = 0;
            ent.alpha = 0.0;
        }
        if s1.modelindex3 != 0 {
            ent.model = cl.model_draw[s1.modelindex3 as usize];
            callbacks.v_add_entity(&ent);
        }
        if s1.modelindex4 != 0 {
            ent.model = cl.model_draw[s1.modelindex4 as usize];
            callbacks.v_add_entity(&ent);
        }

        if effects & EF_POWERSCREEN != 0 {
            ent.model = cl.model_draw[0]; // cl_mod_powerscreen placeholder
            ent.oldframe = 0;
            ent.frame = 0;
            ent.flags |= RF_TRANSLUCENT | RF_SHELL_GREEN;
            ent.alpha = 0.30;
            callbacks.v_add_entity(&ent);
        }

        // add automatic particle trails
        let lerp_origin_copy = cent.lerp_origin;
        if (effects & !EF_ROTATE) != 0 {
            if effects & EF_ROCKET != 0 {
                callbacks.cl_rocket_trail(&lerp_origin_copy, &ent.origin, cent);
                callbacks.v_add_light(&ent.origin, 200.0, 1.0, 1.0, 0.0);
            }
            // PGM - Do not reorder EF_BLASTER and EF_HYPERBLASTER.
            else if effects & EF_BLASTER != 0 {
                if effects & EF_TRACKER != 0 {
                    callbacks.cl_blaster_trail2(&lerp_origin_copy, &ent.origin);
                    callbacks.v_add_light(&ent.origin, 200.0, 0.0, 1.0, 0.0);
                } else {
                    callbacks.cl_blaster_trail(&lerp_origin_copy, &ent.origin);
                    callbacks.v_add_light(&ent.origin, 200.0, 1.0, 1.0, 0.0);
                }
            } else if effects & EF_HYPERBLASTER != 0 {
                if effects & EF_TRACKER != 0 {
                    callbacks.v_add_light(&ent.origin, 200.0, 0.0, 1.0, 0.0);
                } else {
                    callbacks.v_add_light(&ent.origin, 200.0, 1.0, 1.0, 0.0);
                }
            } else if effects & EF_GIB != 0 {
                callbacks.cl_diminishing_trail(&lerp_origin_copy, &ent.origin, cent, effects);
            } else if effects & EF_GRENADE != 0 {
                callbacks.cl_diminishing_trail(&lerp_origin_copy, &ent.origin, cent, effects);
            } else if effects & EF_FLIES != 0 {
                callbacks.cl_fly_effect(cent, &ent.origin);
            } else if effects & EF_BFG != 0 {
                static BFG_LIGHTRAMP: [i32; 6] = [300, 400, 600, 300, 150, 75];

                let light_i: i32;
                if effects & EF_ANIM_ALLFAST != 0 {
                    callbacks.cl_bfg_particles(&ent);
                    light_i = 200;
                } else {
                    light_i = BFG_LIGHTRAMP[s1.frame as usize];
                }
                callbacks.v_add_light(&ent.origin, light_i as f32, 0.0, 1.0, 0.0);
            }
            // RAFAEL
            else if effects & EF_TRAP != 0 {
                let mut trap_ent = ent.clone();
                trap_ent.origin[2] += 32.0;
                callbacks.cl_trap_particles(&trap_ent);
                let ri = (rand::random::<i32>().unsigned_abs() as i32 % 100) + 100;
                callbacks.v_add_light(&trap_ent.origin, ri as f32, 1.0, 0.8, 0.1);
            } else if effects & EF_FLAG1 != 0 {
                callbacks.cl_flag_trail(&lerp_origin_copy, &ent.origin, 242.0);
                callbacks.v_add_light(&ent.origin, 225.0, 1.0, 0.1, 0.1);
            } else if effects & EF_FLAG2 != 0 {
                callbacks.cl_flag_trail(&lerp_origin_copy, &ent.origin, 115.0);
                callbacks.v_add_light(&ent.origin, 225.0, 0.1, 0.1, 1.0);
            }
            // ROGUE
            else if effects & EF_TAGTRAIL != 0 {
                callbacks.cl_tag_trail(&lerp_origin_copy, &ent.origin, 220.0);
                callbacks.v_add_light(&ent.origin, 225.0, 1.0, 1.0, 0.0);
            } else if effects & EF_TRACKERTRAIL != 0 {
                if effects & EF_TRACKER != 0 {
                    let intensity =
                        50.0 + (500.0 * ((cl.time as f32 / 500.0).sin() + 1.0));
                    callbacks.v_add_light(&ent.origin, intensity, -1.0, -1.0, -1.0);
                } else {
                    callbacks.cl_tracker_shell(&lerp_origin_copy);
                    callbacks.v_add_light(&ent.origin, 155.0, -1.0, -1.0, -1.0);
                }
            } else if effects & EF_TRACKER != 0 {
                callbacks.cl_tracker_trail(&lerp_origin_copy, &ent.origin, 0);
                callbacks.v_add_light(&ent.origin, 200.0, -1.0, -1.0, -1.0);
            }
            // RAFAEL
            else if effects & EF_GREENGIB != 0 {
                callbacks.cl_diminishing_trail(&lerp_origin_copy, &ent.origin, cent, effects);
            }
            // RAFAEL
            else if effects & EF_IONRIPPER != 0 {
                callbacks.cl_ionripper_trail(&lerp_origin_copy, &ent.origin);
                callbacks.v_add_light(&ent.origin, 100.0, 1.0, 0.5, 0.5);
            }
            // RAFAEL
            else if effects & EF_BLUEHYPERBLASTER != 0 {
                callbacks.v_add_light(&ent.origin, 200.0, 0.0, 0.0, 1.0);
            }
            // RAFAEL
            else if effects & EF_PLASMA != 0 {
                if effects & EF_ANIM_ALLFAST != 0 {
                    callbacks.cl_blaster_trail(&lerp_origin_copy, &ent.origin);
                }
                callbacks.v_add_light(&ent.origin, 130.0, 1.0, 0.5, 0.5);
            }
        }

        // Update animation state for client-side continuation.
        // cl_update_entity_animation advances the animation frame locally when
        // server updates are missed, keeping entities animated during packet loss.
        {
            let frame_delta_ms = if cl.time > ent_state.cl_entities[s1.number as usize].last_update_time {
                (cl.time - ent_state.cl_entities[s1.number as usize].last_update_time) as f32
            } else {
                0.0
            };
            cl_update_entity_animation(
                &mut ent_state.cl_entities[s1.number as usize],
                frame_delta_ms,
                cl.frame.serverframe,
            );
        }

        ent_state.cl_entities[s1.number as usize].lerp_origin = vector_copy(&ent.origin);
    }

    // === Entity fadeout rendering ===
    // Render recently-disappeared entities with alpha fadeout to prevent pop-out
    cl_add_fadeout_entities(frame, cl, ent_state, callbacks);
}

/// Render entities that have recently disappeared from server updates with fadeout alpha
fn cl_add_fadeout_entities(
    frame: &Frame,
    cl: &ClientState,
    ent_state: &mut ClientEntState,
    callbacks: &mut dyn ClientCallbacks,
) {
    // Skip if fadeout is disabled
    if !cl.smoothing.entity_fadeout.enabled {
        return;
    }

    // Build set of entity numbers that are in the current frame
    // (so we don't re-render entities that are already visible)
    let mut current_entities = vec![false; MAX_EDICTS];
    for pnum in 0..frame.num_entities {
        let idx = ((frame.parse_entities + pnum) & (MAX_PARSE_ENTITIES as i32 - 1)) as usize;
        let entnum = ent_state.cl_parse_entities[idx].number as usize;
        if entnum < MAX_EDICTS {
            current_entities[entnum] = true;
        }
    }

    // Check all client entities for fadeout candidates
    for entnum in 1..MAX_EDICTS {
        // Skip entities that are in the current frame
        if current_entities[entnum] {
            continue;
        }

        let cent = &ent_state.cl_entities[entnum];

        // Skip entities that were never seen or have no model
        if cent.serverframe <= 0 || cent.current.modelindex == 0 {
            continue;
        }

        // Calculate fadeout alpha
        let alpha = match cl.smoothing.entity_fadeout.calculate_alpha(
            cl.time,
            cent.last_update_time,
            frame.serverframe,
            cent.serverframe,
        ) {
            Some(a) => a,
            None => continue, // Not eligible for fadeout or fully faded
        };

        // Create entity for rendering with fadeout
        let mut ent = Entity::default();
        ent.model = cl.model_draw.get(cent.current.modelindex as usize).copied().unwrap_or(0);
        if ent.model == 0 {
            continue;
        }

        // Use velocity extrapolation for fading entities to prevent "frozen" appearance
        // This makes gibs, projectiles, and dying entities continue moving smoothly during fadeout
        let mut origin = cent.lerp_origin;
        if cent.velocity.valid {
            let time_since_update = (cl.time - cent.velocity.last_update_time) as f32 / 1000.0;
            // Limit extrapolation to 500ms (fadeout duration) to prevent runaway movement
            if time_since_update > 0.0 && time_since_update < 0.5 {
                for i in 0..3 {
                    origin[i] += cent.velocity.velocity[i] * time_since_update;
                }
                // Apply gravity for gibs and grenades (EF_GIB, EF_GRENADE, EF_GREENGIB)
                let effects = cent.current.effects;
                if effects & (EF_GIB | EF_GRENADE | EF_GREENGIB) != 0 {
                    origin[2] -= 0.5 * 800.0 * time_since_update * time_since_update;
                }
            }
        }
        ent.origin = origin;
        ent.oldorigin = origin;

        // Set frame to last known frame
        ent.frame = cent.current.frame;
        ent.oldframe = cent.prev.frame;
        ent.backlerp = 0.0; // No interpolation needed since we're extrapolating position

        // Apply angles
        ent.angles = cent.current.angles;

        // Set skin/skinnum
        ent.skinnum = cent.current.skinnum;
        ent.skin = 0; // Will be resolved by renderer

        // Make translucent with fadeout alpha
        ent.flags = cent.current.renderfx | RF_TRANSLUCENT;
        ent.alpha = alpha;

        // Add entity to render list
        callbacks.v_add_entity(&ent);
    }
}

/// Helper: check if cl_vwep is enabled. In the real engine this reads
/// the cvar; here we check a field on ClientState or default to true.
fn cl_vwep_enabled(_cl: &ClientState) -> bool {
    myq2_common::cvar::cvar_variable_value("cl_vwep") != 0.0
}

/// CL_AddViewWeapon
pub fn cl_add_view_weapon(
    ps: &PlayerState,
    ops: &PlayerState,
    cl: &mut ClientState,
    frametime: f32,
    gun_model_override: i32,
    gun_frame_override: i32,
    hand_value: i32,
    callbacks: &mut dyn ClientCallbacks,
) {
    // allow the gun to be completely removed
    // (cl_gun cvar check — assumed enabled; caller should gate this)

    // don't draw gun if in wide angle view
    // mattx86: gun_wideangle — VISIBLE_GUN_WIDEANGLE is enabled, so we skip this check

    let mut gun = Entity::default();

    if gun_model_override != 0 {
        gun.model = gun_model_override; // development tool
    } else {
        gun.model = cl.model_draw[ps.gunindex as usize];
    }
    if gun.model == 0 {
        return;
    }

    // set up gun position (raw calculated position)
    let mut raw_origin = [0.0f32; 3];
    let mut raw_angles = [0.0f32; 3];
    for i in 0..3 {
        raw_origin[i] = cl.refdef.vieworg[i] + ops.gunoffset[i]
            + cl.lerpfrac * (ps.gunoffset[i] - ops.gunoffset[i]);
        raw_angles[i] = cl.refdef.viewangles[i]
            + lerp_angle(ops.gunangles[i], ps.gunangles[i], cl.lerpfrac);
    }

    // RIOT - Centered gun (CENTERED_GUN enabled)
    if hand_value == 2 {
        let mut anglemove = [0.0f32; 3];
        let mut anglemove2 = [0.0f32; 3];
        angle_vectors(&raw_angles, None, Some(&mut anglemove), Some(&mut anglemove2));
        let am_scaled = vector_scale(&anglemove, -8.0);
        let am2_scaled = vector_scale(&anglemove2, -5.0);
        raw_origin = vector_add(&raw_origin, &am_scaled);
        raw_origin = vector_add(&raw_origin, &am2_scaled);
    }

    // Apply weapon view smoothing for smoother gun movement
    let (smoothed_origin, smoothed_angles) = if cl.smoothing.weapon_smoothing.enabled {
        cl.smoothing.weapon_smoothing.update(&raw_origin, &raw_angles, frametime)
    } else {
        (raw_origin, raw_angles)
    };

    // Apply weapon sway (inertial gun movement based on player movement)
    let current_time = cl.time;
    if cl.packet_loss_frames > 0 {
        // During packet loss, continue sway momentum (gradually settle)
        cl.smoothing.weapon_sway.continue_during_packet_loss(current_time);
    } else {
        // Normal update - sway based on player velocity and view angles
        let velocity = [
            ps.pmove.velocity[0] as f32 * 0.125,
            ps.pmove.velocity[1] as f32 * 0.125,
            ps.pmove.velocity[2] as f32 * 0.125,
        ];
        cl.smoothing.weapon_sway.update(&velocity, &raw_angles, current_time);
    }

    // Get sway offset and apply to gun position
    let sway_offset = cl.smoothing.weapon_sway.get_offset();
    gun.origin = [
        smoothed_origin[0] + sway_offset[0],
        smoothed_origin[1] + sway_offset[1],
        smoothed_origin[2] + sway_offset[2],
    ];
    gun.angles = smoothed_angles;

    if gun_frame_override != 0 {
        gun.frame = gun_frame_override; // development tool
        gun.oldframe = gun_frame_override;
        gun.backlerp = 0.0;
    } else {
        gun.frame = ps.gunframe;
        if gun.frame == 0 {
            gun.oldframe = 0; // just changed weapons, don't lerp from old
            gun.backlerp = 0.0;
            // Clear weapon animation state on weapon switch
            cl.smoothing.weapon_anim.clear();
        } else {
            // Update weapon animation smoothing system
            let current_time = cl.time;
            if cl.packet_loss_frames > 0 {
                // During packet loss, continue animation extrapolation
                cl.smoothing.weapon_anim.continue_animation(current_time);
            } else {
                // Normal update with new frame from server
                cl.smoothing.weapon_anim.update(ps.gunframe, current_time);
            }

            // Use smoothed frames if enabled
            if cl.smoothing.weapon_anim.enabled {
                let (frame, oldframe, backlerp) = cl.smoothing.weapon_anim.get_smooth_frames();
                gun.frame = frame;
                gun.oldframe = oldframe;
                gun.backlerp = backlerp;
            } else {
                // Fallback to standard interpolation
                gun.oldframe = ops.gunframe;
                if gun.frame != gun.oldframe {
                    gun.backlerp = 1.0 - cl.lerpfrac;
                } else {
                    gun.backlerp = 0.0;
                }
            }
        }
    }

    gun.flags = RF_MINLIGHT | RF_DEPTHHACK | RF_WEAPONMODEL;
    gun.oldorigin = vector_copy(&gun.origin); // don't lerp at all
    callbacks.v_add_entity(&gun);
}

/// CL_CalcViewValues — Sets cl.refdef view values
pub fn cl_calc_view_values(
    cl: &mut ClientState,
    cls: &ClientStatic,
    ent_state: &ClientEntState,
    gun_model_override: i32,
    gun_frame_override: i32,
    hand_value: i32,
    cl_predict_enabled: bool,
    cl_gun_enabled: bool,
    callbacks: &mut dyn ClientCallbacks,
) {
    // find the previous frame to interpolate from
    let ps = cl.frame.playerstate.clone();
    let i = ((cl.frame.serverframe - 1) as usize) & (UPDATE_BACKUP as usize - 1);
    let oldframe = &cl.frames[i];
    let mut ops: PlayerState;
    if oldframe.serverframe != cl.frame.serverframe - 1 || !oldframe.valid {
        ops = ps.clone(); // previous frame was dropped or invalid
    } else {
        ops = oldframe.playerstate.clone();
    }

    // see if the player entity was teleported this frame
    if (ops.pmove.origin[0] as i32 - ps.pmove.origin[0] as i32).unsigned_abs() > 256 * 8
        || (ops.pmove.origin[1] as i32 - ps.pmove.origin[1] as i32).unsigned_abs() > 256 * 8
        || (ops.pmove.origin[2] as i32 - ps.pmove.origin[2] as i32).unsigned_abs() > 256 * 8
    {
        ops = ps.clone(); // don't interpolate

        // Reset view and weapon smoothing on teleport to prevent trying to smooth a large jump
        let teleport_origin = [
            ps.pmove.origin[0] as f32 * 0.125 + ps.viewoffset[0],
            ps.pmove.origin[1] as f32 * 0.125 + ps.viewoffset[1],
            ps.pmove.origin[2] as f32 * 0.125 + ps.viewoffset[2],
        ];
        cl.smoothing.view_smoothing.snap_to(&teleport_origin, &ps.viewangles);
        cl.smoothing.weapon_smoothing.snap_to(&teleport_origin, &ps.viewangles);
    }

    let _ent = &ent_state.cl_entities[(cl.playernum + 1) as usize];
    let lerp = cl.lerpfrac;

    // calculate the origin
    if cl_predict_enabled && (cl.frame.playerstate.pmove.pm_flags & PMF_NO_PREDICTION) == 0 {
        // use predicted values
        let backlerp = 1.0 - lerp;

        // Use smoothed prediction error for smoother corrections
        // This interpolates the error over 150ms instead of snapping instantly
        let smoothed_error = crate::cl_pred::cl_get_smoothed_prediction_error(cl, cls.realtime);

        for i in 0..3 {
            cl.refdef.vieworg[i] = cl.predicted_origin[i] + ops.viewoffset[i]
                + cl.lerpfrac * (ps.viewoffset[i] - ops.viewoffset[i])
                - backlerp * smoothed_error[i];
        }

        // smooth out stair climbing
        let delta = (cls.realtime as u32).wrapping_sub(cl.predicted_step_time);
        if delta < 100 {
            cl.refdef.vieworg[2] -= cl.predicted_step * (100 - delta) as f32 * 0.01;
        }
    } else {
        // just use interpolated values
        for i in 0..3 {
            cl.refdef.vieworg[i] = ops.pmove.origin[i] as f32 * 0.125
                + ops.viewoffset[i]
                + lerp
                    * (ps.pmove.origin[i] as f32 * 0.125 + ps.viewoffset[i]
                        - (ops.pmove.origin[i] as f32 * 0.125 + ops.viewoffset[i]));
        }
    }

    // if not running a demo or on a locked frame, add the local angle movement
    if (cl.frame.playerstate.pmove.pm_type as i32) < (PmType::Dead as i32) {
        // use predicted values
        for i in 0..3 {
            cl.refdef.viewangles[i] = cl.predicted_angles[i];
        }
    } else {
        // just use interpolated values
        for i in 0..3 {
            cl.refdef.viewangles[i] = lerp_angle(ops.viewangles[i], ps.viewangles[i], lerp);
        }
    }

    // === View bob continuation during packet loss ===
    // This continues the walking/running bob motion when packets are dropped
    // to prevent the view from suddenly becoming static
    if cl.smoothing.view_bob.enabled {
        let current_time = cls.realtime as i32;

        // Get player velocity from playerstate (convert from fixed-point)
        let velocity = [
            ps.pmove.velocity[0] as f32 * 0.125,
            ps.pmove.velocity[1] as f32 * 0.125,
            ps.pmove.velocity[2] as f32 * 0.125,
        ];

        if cl.packet_loss_frames > 0 {
            // During packet loss, continue the bob motion
            cl.smoothing.view_bob.continue_bob(current_time);
        } else {
            // Normal frame - update bob with actual velocity
            cl.smoothing.view_bob.update(&velocity, current_time);
        }

        // Apply bob offset to view
        let (bob_y, bob_roll) = cl.smoothing.view_bob.get_bob();
        cl.refdef.vieworg[2] += bob_y;
        cl.refdef.viewangles[2] += bob_roll;
    }

    // === Weapon kick/recoil smoothing ===
    // Apply momentum-based smoothing for natural-feeling weapon recoil
    // This prevents abrupt kick changes and adds momentum decay
    let delta_time = cls.frametime; // frametime is already in seconds

    let smoothed_kick = if cl.packet_loss_frames > 0 {
        // During packet loss, continue natural decay toward zero
        // This prevents the view from freezing with kick applied
        cl.smoothing.recoil_smoothing.continue_decay(delta_time)
    } else {
        // Normal update with interpolated kick from playerstate
        let mut target_kick = [0.0f32; 3];
        for i in 0..3 {
            target_kick[i] = lerp_angle(ops.kick_angles[i], ps.kick_angles[i], lerp);
        }
        cl.smoothing.recoil_smoothing.update(&target_kick, delta_time)
    };

    for i in 0..3 {
        cl.refdef.viewangles[i] += smoothed_kick[i];
    }

    // === Apply view smoothing to prevent camera snapping ===
    // This smooths out abrupt changes in view position/angles
    if cl.smoothing.view_smoothing.enabled {
        // Calculate delta time in seconds (frame_msec is in milliseconds)
        let delta_time = 0.01; // Assume ~100Hz for now, will be passed in future

        let (smoothed_origin, smoothed_angles) = cl.smoothing.view_smoothing.update(
            &cl.refdef.vieworg,
            &cl.refdef.viewangles,
            delta_time,
        );

        cl.refdef.vieworg = smoothed_origin;
        cl.refdef.viewangles = smoothed_angles;
    }

    angle_vectors(
        &cl.refdef.viewangles,
        Some(&mut cl.v_forward),
        Some(&mut cl.v_right),
        Some(&mut cl.v_up),
    );

    // interpolate field of view
    cl.refdef.fov_x = ops.fov + lerp * (ps.fov - ops.fov);

    // Smooth blend color transitions (damage flash, powerups)
    // Update screen blend smoothing with new values from server
    cl.smoothing.screen_blend.update(&ps.blend, cl.time);

    // During packet loss, continue smoothing toward current target
    if cl.packet_loss_frames > 0 {
        cl.smoothing.screen_blend.continue_smoothing(cl.time);
    }

    // Get the smoothed blend values
    cl.refdef.blend = cl.smoothing.screen_blend.get_blend();

    // add the weapon
    if cl_gun_enabled {
        cl_add_view_weapon(&ps, &ops, cl, cls.frametime, gun_model_override, gun_frame_override, hand_value, callbacks);
    }
}

/// CL_AddEntities — Emits all entities, particles, and lights to the refresh.
pub fn cl_add_entities(
    cl: &mut ClientState,
    cls: &ClientStatic,
    ent_state: &mut ClientEntState,
    proj_state: &mut ProjectileState,
    cl_showclamp: bool,
    cl_timedemo: bool,
    cl_predict_enabled: bool,
    cl_gun_enabled: bool,
    gun_model_override: i32,
    gun_frame_override: i32,
    hand_value: i32,
    callbacks: &mut dyn ClientCallbacks,
) {
    if cls.state != ConnState::Active {
        return;
    }

    // === Adaptive Interpolation Buffer ===
    // Get the adaptive interpolation delay based on network jitter.
    // This provides a larger buffer when jitter is high, smaller when stable.
    let adaptive_buffer_ms = if cl.smoothing.adaptive_interp.enabled {
        cl.smoothing.adaptive_interp.get_lerp_delay()
    } else {
        100 // Default 100ms interpolation buffer
    };

    // === Snapshot-based interpolation ===
    // Use the snapshot buffer for smoother interpolation when enabled.
    // This helps reduce jitter from variable packet arrival times by
    // using a buffer of recorded snapshots to calculate render time.
    let snapshot_lerp = if cl.smoothing.snapshot_buffer.enabled {
        cl.smoothing.snapshot_buffer.get_interpolation_snapshots(cls.realtime)
            .map(|(before, after, lerp)| {
                // Adjust cl.time based on snapshot interpolation
                // This smooths out the rendering by accounting for packet jitter
                let interp_time = before.server_time + ((after.server_time - before.server_time) as f32 * lerp) as i32;
                Some((interp_time, lerp))
            })
            .flatten()
    } else {
        None
    };

    // Use snapshot-based time if available, otherwise fall back to standard calculation
    if let Some((interp_time, snapshot_lerpfrac)) = snapshot_lerp {
        // Blend between snapshot-based lerp and standard lerp for smoothness
        // Use adaptive buffer for the standard lerp calculation
        let standard_lerp = if cl.time > cl.frame.servertime {
            1.0
        } else if cl.time < cl.frame.servertime - adaptive_buffer_ms {
            0.0
        } else {
            1.0 - (cl.frame.servertime - cl.time) as f32 / adaptive_buffer_ms as f32
        };

        // Use 70% snapshot-based, 30% standard for smooth blending
        cl.lerpfrac = snapshot_lerpfrac * 0.7 + standard_lerp * 0.3;

        // Adjust time to match interpolation
        if (interp_time - cl.time).abs() < 50 {
            // Only adjust if close to avoid jarring jumps
            cl.time = (cl.time * 7 + interp_time * 3) / 10; // Smooth transition
        }
    } else {
        // Standard lerpfrac calculation with adaptive buffer
        if cl.time > cl.frame.servertime {
            if cl_showclamp {
                com_dprintf(&format!("high clamp {}\n", cl.time - cl.frame.servertime));
            }
            cl.time = cl.frame.servertime;
            cl.lerpfrac = 1.0;
        } else if cl.time < cl.frame.servertime - adaptive_buffer_ms {
            if cl_showclamp {
                com_dprintf(&format!(
                    "low clamp {}\n",
                    cl.frame.servertime - adaptive_buffer_ms - cl.time
                ));
            }
            cl.time = cl.frame.servertime - adaptive_buffer_ms;
            cl.lerpfrac = 0.0;
        } else {
            // Use adaptive buffer size for lerp calculation
            cl.lerpfrac = 1.0 - (cl.frame.servertime - cl.time) as f32 / adaptive_buffer_ms as f32;
        }
    }

    if cl_timedemo {
        cl.lerpfrac = 1.0;
    }

    // Apply time nudge to global lerpfrac.
    // cl_timenudge allows players to trade responsiveness vs smoothness.
    // Positive values show entities ahead of their interpolation (more responsive),
    // negative values add more interpolation delay (smoother).
    if cl.cl_timenudge != 0 && !cl_timedemo {
        cl.lerpfrac = cl_apply_timenudge(
            cl.lerpfrac,
            cl.cl_timenudge,
            cl.frame.servertime,
            cl.time,
        );
    }

    cl_calc_view_values(
        cl, cls, ent_state,
        gun_model_override, gun_frame_override, hand_value,
        cl_predict_enabled, cl_gun_enabled,
        callbacks,
    );
    // PMM - moved this here so the heat beam has the right values for the
    // vieworg, and can lock the beam to the gun
    let frame = cl.frame.clone();
    cl_add_packet_entities(&frame, cl, ent_state, callbacks);

    // Add projectiles to the scene.
    // In the original C code this was #if 0'd (disabled), but the implementation
    // is complete. Wire it here gated by the projectile predict cvar which
    // controls whether the subsystem is active.
    cl_add_projectiles(proj_state, cl, callbacks);

    callbacks.cl_add_tents();
    callbacks.cl_add_particles();
    callbacks.cl_add_dlights();
    callbacks.cl_add_light_styles();
}

/// CL_GetEntitySoundOrigin — Called to get the sound spatialization origin.
///
/// For brush models (doors, platforms, etc.), we need special handling since
/// their origin is often at world origin and the actual entity is offset by
/// their bounding box. This function handles that case.
pub fn cl_get_entity_sound_origin(ent: i32, org: &mut Vec3, ent_state: &ClientEntState) {
    if ent < 0 || ent >= MAX_EDICTS as i32 {
        panic!("CL_GetEntitySoundOrigin: bad ent");
    }
    let cent = &ent_state.cl_entities[ent as usize];

    // For brush models (solid == 31), the origin may not represent the
    // actual center of the entity. However, without access to the model's
    // bounding box data here, we use lerp_origin as best estimate.
    *org = vector_copy(&cent.lerp_origin);
}

/// CL_GetEntitySoundOrigin with model bounding box support.
///
/// This enhanced version properly calculates the center of brush models
/// using their bounding box data for accurate sound spatialization.
/// For brush models like doors and elevators, this places the sound
/// at the center of the brush rather than at the world origin.
///
/// # Arguments
/// * `ent` - Entity number
/// * `org` - Output: sound origin
/// * `ent_state` - Entity state
/// * `get_model_bounds` - Callback to get inline model bounding box: (modelindex) -> (mins, maxs)
pub fn cl_get_entity_sound_origin_enhanced(
    ent: i32,
    org: &mut Vec3,
    ent_state: &ClientEntState,
    get_model_bounds: Option<&dyn Fn(i32) -> Option<(Vec3, Vec3)>>,
) {
    if ent < 0 || ent >= MAX_EDICTS as i32 {
        panic!("CL_GetEntitySoundOrigin: bad ent");
    }
    let cent = &ent_state.cl_entities[ent as usize];

    // Check if this is a brush model (solid value 31 indicates inline bmodel)
    if cent.current.solid == 31 {
        // Try to get model bounds for proper center calculation
        if let Some(get_bounds) = get_model_bounds {
            if let Some((mins, maxs)) = get_bounds(cent.current.modelindex as i32) {
                // Calculate center of bounding box in world space
                // sound_origin = entity_origin + (mins + maxs) / 2
                for i in 0..3 {
                    org[i] = cent.lerp_origin[i] + (mins[i] + maxs[i]) * 0.5;
                }
                return;
            }
        }
    }

    // Fallback to the basic cl_get_entity_sound_origin for point entities
    cl_get_entity_sound_origin(ent, org, ent_state);
}

// =========================================================================
// NETWORK SMOOTHNESS HELPERS
//
// These functions provide velocity-based extrapolation, animation
// continuation, and packet loss concealment for smoother online gameplay.
// =========================================================================

/// Calculate extrapolated position based on entity velocity.
/// Returns the extrapolated origin if velocity is valid and extrapolation is enabled.
///
/// # Arguments
/// * `cent` - The client entity with velocity data
/// * `lerp_origin` - The base interpolated origin
/// * `current_time` - Current client time in ms
/// * `extrapolate_max` - Maximum extrapolation time in ms
/// * `enabled` - Whether extrapolation is enabled
pub fn cl_extrapolate_entity_position(
    cent: &CEntity,
    lerp_origin: &Vec3,
    current_time: i32,
    extrapolate_max: i32,
    enabled: bool,
) -> Vec3 {
    if !enabled || !cent.velocity.valid {
        return *lerp_origin;
    }

    // Calculate time since last server update
    let time_since_update = current_time - cent.velocity.last_update_time;

    // Clamp extrapolation to max (avoid runaway extrapolation on packet loss)
    let extrap_time = time_since_update.min(extrapolate_max).max(0) as f32 / 1000.0;

    // Only extrapolate if we have a small time delta (normal frame gaps)
    if time_since_update < 0 || time_since_update > extrapolate_max * 2 {
        return *lerp_origin;
    }

    let mut result = *lerp_origin;
    for i in 0..3 {
        result[i] += cent.velocity.velocity[i] * extrap_time;
    }
    result
}

/// Apply time nudge adjustment to lerpfrac.
/// Time nudge allows players to trade responsiveness vs smoothness.
///
/// # Arguments
/// * `lerpfrac` - Base interpolation fraction (0.0 to 1.0)
/// * `timenudge` - Time nudge in ms (-100 to +100)
/// * `servertime` - Current server frame time
/// * `client_time` - Current client time
pub fn cl_apply_timenudge(
    lerpfrac: f32,
    timenudge: i32,
    servertime: i32,
    client_time: i32,
) -> f32 {
    if timenudge == 0 {
        return lerpfrac;
    }

    // Adjust client time by timenudge
    let adjusted_time = client_time + timenudge;

    // Recalculate lerpfrac with nudged time
    let new_lerpfrac = 1.0 - (servertime - adjusted_time) as f32 * 0.01;

    // Clamp to valid range
    new_lerpfrac.clamp(0.0, 1.0)
}

/// Predict the next animation frame for client-side continuation.
/// Used to keep animations running smoothly during packet loss.
///
/// # Arguments
/// * `cent` - The client entity with animation state
/// * `frame_delta_ms` - Time since last frame in milliseconds
/// * `enabled` - Whether animation continuation is enabled
pub fn cl_predict_animation_frame(
    cent: &CEntity,
    frame_delta_ms: f32,
    enabled: bool,
) -> (i32, i32, f32) {
    if !enabled || !cent.anim_state.animating {
        return (cent.current.frame, cent.anim_state.oldframe, 0.0);
    }

    let anim = &cent.anim_state;

    // Accumulate frame time
    let new_frame_time = anim.frame_time + frame_delta_ms;

    // Check if we should advance to next frame
    if new_frame_time >= anim.frame_duration {
        // Simple frame advancement - wrap around or continue
        let new_frame = anim.frame + 1;
        let backlerp = 0.0; // Start of new frame
        (new_frame, anim.frame, backlerp)
    } else {
        // Still in current frame - calculate backlerp for smooth interpolation
        let backlerp = 1.0 - (new_frame_time / anim.frame_duration);
        (anim.frame, anim.oldframe, backlerp)
    }
}

/// Conceal packet loss by extrapolating entity state.
/// Returns true if packet loss concealment is active.
///
/// # Arguments
/// * `cent` - The client entity
/// * `current_frame` - Current server frame number
/// * `origin_out` - Output: concealed origin position
/// * `enabled` - Whether PLC is enabled
pub fn cl_conceal_packet_loss(
    cent: &CEntity,
    current_frame: i32,
    origin_out: &mut Vec3,
    enabled: bool,
) -> bool {
    if !enabled {
        return false;
    }

    // Check if this entity is missing from recent frames
    let frames_missed = current_frame - cent.serverframe;
    if frames_missed <= 0 || frames_missed > 5 {
        return false;
    }

    // Use velocity to predict where entity should be
    if cent.velocity.valid {
        // Each missed frame represents SERVER_FRAMETIME_SEC (100ms at 10Hz)
        let extrap_time = (frames_missed as f32) * SERVER_FRAMETIME_SEC;
        for i in 0..3 {
            origin_out[i] = cent.current.origin[i] + cent.velocity.velocity[i] * extrap_time;
        }
        true
    } else {
        // No velocity - use last known position
        *origin_out = cent.current.origin;
        true
    }
}

/// Calculate interpolated origin with all smoothness features applied.
/// This is the main function for smooth entity rendering.
///
/// # Arguments
/// * `cent` - The client entity
/// * `cl` - Client state with timing and settings
/// * `current_time` - Current client time in ms
pub fn cl_smooth_entity_origin(
    cent: &CEntity,
    prev_origin: &Vec3,
    current_origin: &Vec3,
    lerpfrac: f32,
    cl_timenudge: i32,
    cl_extrapolate: bool,
    cl_extrapolate_max: i32,
    servertime: i32,
    current_time: i32,
) -> Vec3 {
    // Step 1: Apply time nudge to lerp fraction
    let adjusted_lerpfrac = cl_apply_timenudge(lerpfrac, cl_timenudge, servertime, current_time);

    // Step 2: Basic linear interpolation
    let mut result = [0.0f32; 3];
    for i in 0..3 {
        result[i] = prev_origin[i] + adjusted_lerpfrac * (current_origin[i] - prev_origin[i]);
    }

    // Step 3: Apply velocity extrapolation for smoother movement
    if cl_extrapolate && cent.velocity.valid && adjusted_lerpfrac >= 0.9 {
        // Only extrapolate near the end of interpolation window
        let extrap_amount = (adjusted_lerpfrac - 0.9) * 10.0; // 0.0 to 1.0
        let extrap_time = extrap_amount * (cl_extrapolate_max as f32 / 1000.0);

        for i in 0..3 {
            result[i] += cent.velocity.velocity[i] * extrap_time;
        }
    }

    result
}

/// Enhanced entity origin calculation with spline and dead reckoning support.
/// Use this for player entities that need the smoothest possible interpolation.
///
/// # Arguments
/// * `entity_num` - Entity number for looking up smoothing state
/// * `cent` - The client entity
/// * `smoothing` - The smoothing state from ClientState
/// * `lerpfrac` - Base interpolation fraction
/// * `cl_timenudge` - Time nudge setting
/// * `cl_extrapolate` - Whether extrapolation is enabled
/// * `cl_extrapolate_max` - Maximum extrapolation time
/// * `servertime` - Current server time
/// * `current_time` - Current client time
/// * `gravity` - Gravity value for dead reckoning
pub fn cl_smooth_entity_origin_advanced(
    entity_num: usize,
    cent: &CEntity,
    smoothing: &crate::cl_smooth::SmoothingState,
    lerpfrac: f32,
    cl_timenudge: i32,
    cl_extrapolate: bool,
    cl_extrapolate_max: i32,
    servertime: i32,
    current_time: i32,
    gravity: f32,
) -> Vec3 {
    // Step 1: Try spline interpolation if enabled and we have enough history
    if smoothing.cubic_interp_enabled && entity_num < smoothing.spline_histories.len() {
        let target_time = current_time + cl_timenudge;
        if let Some(spline_pos) = smoothing.spline_histories[entity_num].interpolate(target_time) {
            // Spline gave us a position - blend with extrapolation if needed
            if cl_extrapolate && cent.velocity.valid && lerpfrac >= 0.9 {
                let extrap_amount = (lerpfrac - 0.9) * 10.0;
                let extrap_time = extrap_amount * (cl_extrapolate_max as f32 / 1000.0);
                let mut result = spline_pos;
                for i in 0..3 {
                    result[i] += cent.velocity.velocity[i] * extrap_time;
                }
                return result;
            }
            return spline_pos;
        }
    }

    // Step 2: Try dead reckoning for player entities with confidence blending
    if cent.current.modelindex == 255 && entity_num < smoothing.dead_reckoning.len() {
        let dr = &smoothing.dead_reckoning[entity_num];
        if dr.confidence > 0.2 {
            // Get base prediction
            let predicted = dr.predict(current_time + cl_timenudge, gravity);

            // Calculate current confidence based on time since last update
            // Confidence decays over time (2x per second decay rate)
            let dt = (current_time - dr.last_update_time) as f32 / 1000.0;
            let current_confidence = (dr.confidence * (1.0 - dt * 2.0)).max(0.0);

            if current_confidence > 0.5 {
                // High confidence - use pure prediction
                return predicted;
            } else if current_confidence > 0.0 {
                // Medium confidence - blend prediction with last known position
                let mut result = [0.0f32; 3];
                for i in 0..3 {
                    result[i] = predicted[i] * current_confidence + dr.position[i] * (1.0 - current_confidence);
                }
                return result;
            }
            // Low confidence - fall through to standard smoothing
        }
    }

    // Step 3: Fall back to standard smoothing
    cl_smooth_entity_origin(
        cent,
        &cent.prev.origin,
        &cent.current.origin,
        lerpfrac,
        cl_timenudge,
        cl_extrapolate,
        cl_extrapolate_max,
        servertime,
        current_time,
    )
}

/// Enhanced projectile origin calculation with aggressive velocity extrapolation.
/// Projectiles have predictable physics (straight line or gravity arc) so we can
/// extrapolate more aggressively than player entities.
///
/// # Arguments
/// * `cent` - The client entity (projectile)
/// * `lerpfrac` - Base interpolation fraction
/// * `current_time` - Current client time in ms
/// * `gravity` - Gravity value (800.0 standard Q2) - only used for gravity-affected projectiles
/// * `is_grenade` - Whether this projectile is affected by gravity (grenades)
pub fn cl_smooth_projectile_origin(
    cent: &CEntity,
    lerpfrac: f32,
    current_time: i32,
    gravity: f32,
    is_grenade: bool,
) -> Vec3 {
    // Step 1: Basic linear interpolation as fallback
    let mut result = [0.0f32; 3];
    for i in 0..3 {
        result[i] = cent.prev.origin[i] + lerpfrac * (cent.current.origin[i] - cent.prev.origin[i]);
    }

    // Step 2: Apply velocity extrapolation if valid
    if !cent.velocity.valid {
        return result;
    }

    // Calculate time since last server update
    let time_since_update = current_time - cent.velocity.last_update_time;
    if time_since_update <= 0 {
        return result;
    }

    // Projectiles can extrapolate more aggressively (up to 200ms)
    let max_extrap_ms = 200;
    let extrap_time = time_since_update.min(max_extrap_ms) as f32 / 1000.0;

    // Only extrapolate if we're near the end of the interpolation window
    // or if we've missed an update
    if lerpfrac < 0.8 && time_since_update < 100 {
        return result;
    }

    // Calculate extrapolation blend factor
    let extrap_blend = if lerpfrac >= 0.9 {
        (lerpfrac - 0.9) * 10.0 // 0.0 to 1.0 in final 10% of lerp
    } else {
        // Beyond normal lerp - fully extrapolate
        1.0
    };

    // Apply velocity extrapolation
    let mut extrap_pos = cent.current.origin;
    for i in 0..3 {
        extrap_pos[i] += cent.velocity.velocity[i] * extrap_time;
    }

    // Apply gravity for grenades
    if is_grenade {
        // v = v0 - g*t, position = p0 + v0*t - 0.5*g*t^2
        extrap_pos[2] -= 0.5 * gravity * extrap_time * extrap_time;
    }

    // Blend between interpolated and extrapolated positions
    for i in 0..3 {
        result[i] = result[i] * (1.0 - extrap_blend) + extrap_pos[i] * extrap_blend;
    }

    result
}

/// Update entity animation state for client-side continuation.
/// Call this each frame to advance animations locally.
pub fn cl_update_entity_animation(
    cent: &mut CEntity,
    frame_delta_ms: f32,
    current_server_frame: i32,
) {
    // Check if we missed server updates
    let frames_missed = current_server_frame - cent.anim_state.last_server_frame;

    if frames_missed > 0 && frames_missed <= 3 {
        // Server update missed - advance animation locally
        cent.anim_state.frame_time += frame_delta_ms;

        if cent.anim_state.frame_time >= cent.anim_state.frame_duration {
            cent.anim_state.oldframe = cent.anim_state.frame;
            cent.anim_state.frame += 1;
            cent.anim_state.frame_time = 0.0;
        }
    }
}

// =========================================================================
// PROJECTILE SUBSYSTEM
//
// Because there can be a lot of projectiles, there is a special
// network protocol for them. Flechettes are passed as efficient
// temporary entities.
// =========================================================================

// MAX_PROJECTILES imported from myq2_common::qcommon

/// Client-side projectile representation.
#[derive(Clone)]
pub struct Projectile {
    pub modelindex: i32,
    pub num: i32,        // entity number
    pub effects: i32,
    pub origin: Vec3,
    pub oldorigin: Vec3,
    pub angles: Vec3,
    pub present: bool,

    // === Smoothness improvements ===
    /// Velocity for client-side prediction (units/sec)
    pub velocity: Vec3,
    /// Whether this projectile is affected by gravity
    pub gravity_affected: bool,
    /// Last update time in client ms
    pub last_update_time: i32,
    /// Whether velocity is valid for prediction
    pub velocity_valid: bool,
    /// Number of frames this projectile has been missing from updates (for trail continuation)
    pub missed_frames: i32,
    /// Last known origin before packet loss (for trail continuation)
    pub trail_origin: Vec3,
}

impl Default for Projectile {
    fn default() -> Self {
        Self {
            modelindex: 0,
            num: 0,
            effects: 0,
            origin: [0.0; 3],
            oldorigin: [0.0; 3],
            angles: [0.0; 3],
            present: false,
            velocity: [0.0; 3],
            gravity_affected: false,
            last_update_time: 0,
            velocity_valid: false,
            missed_frames: 0,
            trail_origin: [0.0; 3],
        }
    }
}

/// Predict projectile position based on velocity and gravity.
/// Used to keep projectiles moving smoothly during packet gaps.
pub fn cl_predict_projectile(
    proj: &Projectile,
    current_time: i32,
    gravity: f32,
) -> Vec3 {
    if !proj.velocity_valid {
        return proj.origin;
    }

    let dt = (current_time - proj.last_update_time) as f32 / 1000.0;
    if dt <= 0.0 || dt > 0.5 {
        return proj.origin;
    }

    let mut predicted = proj.origin;
    for i in 0..3 {
        predicted[i] += proj.velocity[i] * dt;
    }

    // Apply gravity if this is a grenade-type projectile
    if proj.gravity_affected {
        predicted[2] -= 0.5 * gravity * dt * dt;
    }

    predicted
}

/// Client-side projectile list state.
pub struct ProjectileState {
    pub projectiles: [Projectile; MAX_PROJECTILES],
}

impl Default for ProjectileState {
    fn default() -> Self {
        Self {
            projectiles: std::array::from_fn(|_| Projectile::default()),
        }
    }
}

/// Clears all projectiles.
///
/// Corresponds to `CL_ClearProjectiles` in the original C code.
pub fn cl_clear_projectiles(state: &mut ProjectileState) {
    for i in 0..MAX_PROJECTILES {
        state.projectiles[i].present = false;
    }
}

/// Parses projectile data from a network message.
///
/// Flechettes are passed as efficient temporary entities using a compact
/// bit-packed encoding.
///
/// Corresponds to `CL_ParseProjectiles` in the original C code.
pub fn cl_parse_projectiles(
    state: &mut ProjectileState,
    net_message: &mut SizeBuf,
) {
    let c = msg_read_byte(net_message);

    for _i in 0..c {
        let mut bits = [0u8; 8];
        bits[0] = msg_read_byte(net_message) as u8;
        bits[1] = msg_read_byte(net_message) as u8;
        bits[2] = msg_read_byte(net_message) as u8;
        bits[3] = msg_read_byte(net_message) as u8;
        bits[4] = msg_read_byte(net_message) as u8;

        let mut pr = Projectile::default();
        pr.origin[0] = (((bits[0] as i32) + ((bits[1] as i32 & 15) << 8)) << 1) as f32 - 4096.0;
        pr.origin[1] = ((((bits[1] as i32) >> 4) + ((bits[2] as i32) << 4)) << 1) as f32 - 4096.0;
        pr.origin[2] = (((bits[3] as i32) + ((bits[4] as i32 & 15) << 8)) << 1) as f32 - 4096.0;
        pr.oldorigin = vector_copy(&pr.origin);

        if bits[4] & 64 != 0 {
            pr.effects = EF_BLASTER as i32;
        } else {
            pr.effects = 0;
        }

        let mut old = false;
        if bits[4] & 128 != 0 {
            old = true;
            bits[0] = msg_read_byte(net_message) as u8;
            bits[1] = msg_read_byte(net_message) as u8;
            bits[2] = msg_read_byte(net_message) as u8;
            bits[3] = msg_read_byte(net_message) as u8;
            bits[4] = msg_read_byte(net_message) as u8;
            pr.oldorigin[0] = (((bits[0] as i32) + ((bits[1] as i32 & 15) << 8)) << 1) as f32 - 4096.0;
            pr.oldorigin[1] = ((((bits[1] as i32) >> 4) + ((bits[2] as i32) << 4)) << 1) as f32 - 4096.0;
            pr.oldorigin[2] = (((bits[3] as i32) + ((bits[4] as i32 & 15) << 8)) << 1) as f32 - 4096.0;
        }

        bits[0] = msg_read_byte(net_message) as u8;
        bits[1] = msg_read_byte(net_message) as u8;
        bits[2] = msg_read_byte(net_message) as u8;

        pr.angles[0] = 360.0 * bits[0] as f32 / 256.0;
        pr.angles[1] = 360.0 * bits[1] as f32 / 256.0;
        pr.modelindex = bits[2] as i32;

        let b = msg_read_byte(net_message) as u8;
        pr.num = (b & 0x7f) as i32;
        if b & 128 != 0 {
            // extra entity number byte
            pr.num |= (msg_read_byte(net_message) as i32) << 7;
        }

        pr.present = true;

        let mut lastempty: i32 = -1;

        // find if this projectile already exists from previous frame
        let mut found = false;
        for j in 0..MAX_PROJECTILES {
            if state.projectiles[j].modelindex != 0 {
                if state.projectiles[j].num == pr.num {
                    // already present, set up oldorigin for interpolation
                    if !old {
                        pr.oldorigin = vector_copy(&state.projectiles[j].origin);
                    }

                    // === Calculate velocity for prediction ===
                    // Projectiles update at server tick rate (10Hz = SERVER_FRAMETIME_SEC)
                    for k in 0..3 {
                        pr.velocity[k] = (pr.origin[k] - state.projectiles[j].origin[k]) / SERVER_FRAMETIME_SEC;
                    }
                    pr.velocity_valid = true;

                    // Detect gravity-affected projectiles (grenades have downward velocity increase)
                    let z_accel = pr.velocity[2] - state.projectiles[j].velocity[2];
                    pr.gravity_affected = z_accel < -50.0; // Significant downward acceleration

                    state.projectiles[j] = pr.clone();
                    found = true;
                    break;
                }
            } else {
                lastempty = j as i32;
            }
        }

        // not present previous frame, add it
        if !found {
            if lastempty != -1 {
                // New projectile - estimate velocity from oldorigin if available
                if old {
                    for k in 0..3 {
                        pr.velocity[k] = (pr.origin[k] - pr.oldorigin[k]) / SERVER_FRAMETIME_SEC;
                    }
                    pr.velocity_valid = true;
                }
                state.projectiles[lastempty as usize] = pr;
            }
        }
    }
}

/// Parses projectile data with client time tracking for prediction.
pub fn cl_parse_projectiles_with_time(
    state: &mut ProjectileState,
    net_message: &mut SizeBuf,
    client_time: i32,
) {
    cl_parse_projectiles(state, net_message);

    // Update last update time for all present projectiles
    for proj in state.projectiles.iter_mut() {
        if proj.present {
            proj.last_update_time = client_time;
        }
    }
}

/// Adds projectiles to the render entity list.
///
/// Corresponds to `CL_AddProjectiles` (aka `CL_LinkProjectiles`) in the
/// original C code.
///
/// Uses the cl_projectile_predict setting to determine if velocity-based
/// prediction should be used for smoother projectile movement during
/// packet gaps.
pub fn cl_add_projectiles(
    state: &mut ProjectileState,
    cl: &ClientState,
    callbacks: &mut dyn ClientCallbacks,
) {
    // Use the cl_projectile_predict setting to enable/disable prediction
    cl_add_projectiles_with_prediction(state, cl, callbacks, cl.cl_projectile_predict, 800.0);
}

/// Adds projectiles to the render entity list with optional prediction.
///
/// # Arguments
/// * `state` - Projectile state
/// * `cl` - Client state
/// * `callbacks` - Rendering callbacks
/// * `predict` - Whether to use velocity-based prediction
/// * `gravity` - Gravity value for grenade prediction (default 800)
pub fn cl_add_projectiles_with_prediction(
    state: &mut ProjectileState,
    cl: &ClientState,
    callbacks: &mut dyn ClientCallbacks,
    predict: bool,
    gravity: f32,
) {
    let mut ent = Entity::default();

    for i in 0..MAX_PROJECTILES {
        let pr = &mut state.projectiles[i];

        // grab an entity to fill in
        if pr.modelindex < 1 {
            continue;
        }

        // === Trail continuation during packet loss ===
        // Instead of immediately clearing missing projectiles, continue their trails
        // for a few frames using extrapolation to maintain visual continuity.
        if !pr.present {
            pr.missed_frames += 1;

            // Only continue for up to 5 frames (500ms at 10fps server)
            if pr.missed_frames > 5 || !pr.velocity_valid {
                pr.modelindex = 0;
                pr.missed_frames = 0;
                continue;
            }

            // Use cl_predict_projectile for velocity-based position extrapolation
            // during packet loss. This handles both straight-line and gravity-affected
            // projectiles (e.g., grenades).
            let extrapolated = cl_predict_projectile(pr, cl.time, gravity);
            let dt = (cl.time - pr.last_update_time) as f32 / 1000.0;
            if dt > 0.0 && dt < 1.0 {
                // Draw trail from last known trail origin to extrapolated position
                if pr.effects & (EF_BLASTER as i32) != 0 {
                    callbacks.cl_blaster_trail(&pr.trail_origin, &extrapolated);
                }
                // Add light at extrapolated position
                callbacks.v_add_light(&extrapolated, 200.0 * (1.0 - pr.missed_frames as f32 * 0.15), 1.0, 1.0, 0.0);

                // Update trail origin for next frame
                pr.trail_origin = extrapolated;

                // Add the entity at extrapolated position
                ent.model = cl.model_draw[pr.modelindex as usize];
                ent.origin = extrapolated;
                ent.oldorigin = extrapolated;
                ent.angles = vector_copy(&pr.angles);
                callbacks.v_add_entity(&ent);
            }
            continue;
        }

        // Reset missed frames on successful update
        pr.missed_frames = 0;
        ent.model = cl.model_draw[pr.modelindex as usize];

        // Calculate origin with optional prediction
        if predict && pr.velocity_valid && cl.lerpfrac >= 0.9 {
            // Use prediction near end of interpolation window
            let base_origin = [
                pr.oldorigin[0] + cl.lerpfrac * (pr.origin[0] - pr.oldorigin[0]),
                pr.oldorigin[1] + cl.lerpfrac * (pr.origin[1] - pr.oldorigin[1]),
                pr.oldorigin[2] + cl.lerpfrac * (pr.origin[2] - pr.oldorigin[2]),
            ];

            // Add prediction for smooth continuation when near end of interpolation.
            // Convert the last 10% of lerp (0.9-1.0) to extrapolation time.
            let extrap_time = (cl.lerpfrac - 0.9) * SERVER_FRAMETIME_SEC;
            for j in 0..3 {
                ent.origin[j] = base_origin[j] + pr.velocity[j] * extrap_time;
            }

            // Apply gravity for grenades
            if pr.gravity_affected {
                ent.origin[2] -= 0.5 * gravity * extrap_time * extrap_time;
            }
        } else {
            // Standard interpolation
            for j in 0..3 {
                ent.origin[j] = pr.oldorigin[j]
                    + cl.lerpfrac * (pr.origin[j] - pr.oldorigin[j]);
            }
        }
        ent.oldorigin = ent.origin;

        if pr.effects & (EF_BLASTER as i32) != 0 {
            callbacks.cl_blaster_trail(&pr.oldorigin, &ent.origin);
        }
        callbacks.v_add_light(&pr.origin, 200.0, 1.0, 1.0, 0.0);

        // Store current origin for trail continuation during packet loss
        pr.trail_origin = ent.origin;

        ent.angles = vector_copy(&pr.angles);
        callbacks.v_add_entity(&ent);
    }
}

// ============================================================
// Unit tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::CEntity;

    // -------------------------------------------------------
    // Helper: create a CEntity with velocity data
    // -------------------------------------------------------
    fn make_entity_with_velocity(
        origin: Vec3,
        velocity: Vec3,
        last_update_time: i32,
    ) -> CEntity {
        let mut cent = CEntity::default();
        cent.current.origin = origin;
        cent.prev.origin = [
            origin[0] - velocity[0] * SERVER_FRAMETIME_SEC,
            origin[1] - velocity[1] * SERVER_FRAMETIME_SEC,
            origin[2] - velocity[2] * SERVER_FRAMETIME_SEC,
        ];
        cent.velocity.velocity = velocity;
        cent.velocity.last_update_time = last_update_time;
        cent.velocity.valid = true;
        cent
    }

    // -------------------------------------------------------
    // cl_extrapolate_entity_position tests
    // -------------------------------------------------------

    #[test]
    fn test_extrapolate_disabled_returns_lerp_origin() {
        let cent = make_entity_with_velocity(
            [100.0, 200.0, 300.0],
            [500.0, 0.0, 0.0],
            1000,
        );
        let lerp = [100.0, 200.0, 300.0];
        let result = cl_extrapolate_entity_position(&cent, &lerp, 1050, 200, false);
        assert_eq!(result, lerp);
    }

    #[test]
    fn test_extrapolate_invalid_velocity_returns_lerp_origin() {
        let mut cent = CEntity::default();
        cent.velocity.valid = false;
        let lerp = [100.0, 200.0, 300.0];
        let result = cl_extrapolate_entity_position(&cent, &lerp, 1050, 200, true);
        assert_eq!(result, lerp);
    }

    #[test]
    fn test_extrapolate_basic_forward() {
        let cent = make_entity_with_velocity(
            [100.0, 0.0, 0.0],
            [1000.0, 0.0, 0.0], // 1000 units/sec in X
            1000,
        );
        let lerp = [100.0, 0.0, 0.0];
        // At time 1100 (100ms after last update), extrapolate_max = 200ms
        let result = cl_extrapolate_entity_position(&cent, &lerp, 1100, 200, true);
        // Expected: 100 + 1000 * 0.1 = 200
        assert!((result[0] - 200.0).abs() < 0.01, "result[0] = {}", result[0]);
        assert!((result[1] - 0.0).abs() < 0.01);
        assert!((result[2] - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_extrapolate_clamped_to_max() {
        let cent = make_entity_with_velocity(
            [0.0, 0.0, 0.0],
            [1000.0, 0.0, 0.0],
            1000,
        );
        let lerp = [0.0, 0.0, 0.0];
        // Time 1500 (500ms after update), but max is 200ms
        let result = cl_extrapolate_entity_position(&cent, &lerp, 1500, 200, true);
        // Clamped to 200ms: 0 + 1000 * 0.2 = 200
        // But 500ms > max*2 (400ms), so it returns lerp_origin unmodified
        assert_eq!(result, lerp);
    }

    #[test]
    fn test_extrapolate_within_double_max() {
        let cent = make_entity_with_velocity(
            [0.0, 0.0, 0.0],
            [1000.0, 0.0, 0.0],
            1000,
        );
        let lerp = [0.0, 0.0, 0.0];
        // Time 1300 (300ms), max 200ms. 300 < 2*200 so it's accepted, clamped to 200ms
        let result = cl_extrapolate_entity_position(&cent, &lerp, 1300, 200, true);
        // extrap_time = min(300, 200) = 200ms = 0.2s
        assert!((result[0] - 200.0).abs() < 0.01, "result[0] = {}", result[0]);
    }

    #[test]
    fn test_extrapolate_negative_time_returns_lerp() {
        let cent = make_entity_with_velocity(
            [100.0, 0.0, 0.0],
            [1000.0, 0.0, 0.0],
            1000,
        );
        let lerp = [100.0, 0.0, 0.0];
        // Time before last update
        let result = cl_extrapolate_entity_position(&cent, &lerp, 900, 200, true);
        assert_eq!(result, lerp);
    }

    // -------------------------------------------------------
    // cl_apply_timenudge tests
    // -------------------------------------------------------

    #[test]
    fn test_timenudge_zero_returns_original() {
        let result = cl_apply_timenudge(0.5, 0, 1000, 950);
        assert!((result - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_timenudge_positive_increases_lerpfrac() {
        // Positive timenudge pushes time ahead, increasing lerpfrac
        let base = cl_apply_timenudge(0.5, 0, 1000, 950);
        let nudged = cl_apply_timenudge(0.5, 20, 1000, 950);
        assert!(nudged > base,
            "positive timenudge should increase lerpfrac: nudged={} base={}", nudged, base);
    }

    #[test]
    fn test_timenudge_negative_decreases_lerpfrac() {
        // Negative timenudge pushes time behind, decreasing lerpfrac
        let base = cl_apply_timenudge(0.5, 0, 1000, 950);
        let nudged = cl_apply_timenudge(0.5, -20, 1000, 950);
        assert!(nudged < base,
            "negative timenudge should decrease lerpfrac: nudged={} base={}", nudged, base);
    }

    #[test]
    fn test_timenudge_clamps_to_zero_one() {
        // Very large positive nudge should clamp to 1.0
        let result = cl_apply_timenudge(0.5, 1000, 1000, 950);
        assert!((result - 1.0).abs() < 0.001, "result={}", result);

        // Very large negative nudge should clamp to 0.0
        let result = cl_apply_timenudge(0.5, -1000, 1000, 950);
        assert!((result - 0.0).abs() < 0.001, "result={}", result);
    }

    #[test]
    fn test_timenudge_at_servertime() {
        // When client_time == servertime, lerpfrac should be 1.0
        let result = cl_apply_timenudge(1.0, 0, 1000, 1000);
        assert!((result - 1.0).abs() < 0.001);
    }

    // -------------------------------------------------------
    // cl_predict_animation_frame tests
    // -------------------------------------------------------

    #[test]
    fn test_predict_anim_disabled_returns_current() {
        let mut cent = CEntity::default();
        cent.current.frame = 5;
        cent.anim_state.oldframe = 4;

        let (frame, oldframe, backlerp) = cl_predict_animation_frame(&cent, 50.0, false);
        assert_eq!(frame, 5);
        assert_eq!(oldframe, 4);
        assert!((backlerp - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_predict_anim_not_animating_returns_current() {
        let mut cent = CEntity::default();
        cent.current.frame = 10;
        cent.anim_state.animating = false;
        cent.anim_state.oldframe = 9;

        let (frame, oldframe, _) = cl_predict_animation_frame(&cent, 50.0, true);
        assert_eq!(frame, 10);
        assert_eq!(oldframe, 9);
    }

    #[test]
    fn test_predict_anim_advances_frame() {
        let mut cent = CEntity::default();
        cent.anim_state.animating = true;
        cent.anim_state.frame = 5;
        cent.anim_state.oldframe = 4;
        cent.anim_state.frame_time = 80.0; // 80ms accumulated
        cent.anim_state.frame_duration = 100.0; // 100ms per frame
        cent.current.frame = 5;

        // delta of 25ms pushes total to 105ms, crossing threshold
        let (frame, oldframe, backlerp) = cl_predict_animation_frame(&cent, 25.0, true);
        assert_eq!(frame, 6, "should advance to frame 6");
        assert_eq!(oldframe, 5, "oldframe should be previous frame");
        assert!((backlerp - 0.0).abs() < 0.001, "backlerp should be 0 at start of new frame");
    }

    #[test]
    fn test_predict_anim_within_frame_calculates_backlerp() {
        let mut cent = CEntity::default();
        cent.anim_state.animating = true;
        cent.anim_state.frame = 5;
        cent.anim_state.oldframe = 4;
        cent.anim_state.frame_time = 0.0;
        cent.anim_state.frame_duration = 100.0;
        cent.current.frame = 5;

        // 50ms into 100ms frame = 50% progress
        let (frame, oldframe, backlerp) = cl_predict_animation_frame(&cent, 50.0, true);
        assert_eq!(frame, 5);
        assert_eq!(oldframe, 4);
        // backlerp = 1.0 - (50/100) = 0.5
        assert!((backlerp - 0.5).abs() < 0.01, "backlerp={}", backlerp);
    }

    // -------------------------------------------------------
    // cl_conceal_packet_loss tests
    // -------------------------------------------------------

    #[test]
    fn test_conceal_disabled_returns_false() {
        let cent = make_entity_with_velocity([100.0, 0.0, 0.0], [500.0, 0.0, 0.0], 1000);
        let mut origin = [0.0; 3];
        let result = cl_conceal_packet_loss(&cent, 102, &mut origin, false);
        assert!(!result);
    }

    #[test]
    fn test_conceal_no_missed_frames() {
        let mut cent = make_entity_with_velocity([100.0, 0.0, 0.0], [500.0, 0.0, 0.0], 1000);
        cent.serverframe = 100; // same as current_frame
        let mut origin = [0.0; 3];
        let result = cl_conceal_packet_loss(&cent, 100, &mut origin, true);
        assert!(!result);
    }

    #[test]
    fn test_conceal_too_many_missed_frames() {
        let mut cent = make_entity_with_velocity([100.0, 0.0, 0.0], [500.0, 0.0, 0.0], 1000);
        cent.serverframe = 90; // 10 frames behind
        let mut origin = [0.0; 3];
        let result = cl_conceal_packet_loss(&cent, 100, &mut origin, true);
        assert!(!result, "Should not conceal when >5 frames missed");
    }

    #[test]
    fn test_conceal_extrapolates_with_velocity() {
        let mut cent = make_entity_with_velocity(
            [100.0, 200.0, 300.0],
            [1000.0, -500.0, 0.0],
            1000,
        );
        cent.serverframe = 98; // 2 frames behind frame 100

        let mut origin = [0.0; 3];
        let result = cl_conceal_packet_loss(&cent, 100, &mut origin, true);
        assert!(result);

        // 2 missed frames * 0.1s = 0.2s
        let expected_x = 100.0 + 1000.0 * 0.2;
        let expected_y = 200.0 + (-500.0) * 0.2;
        let expected_z = 300.0;

        assert!((origin[0] - expected_x).abs() < 0.01, "origin[0]={} expected {}", origin[0], expected_x);
        assert!((origin[1] - expected_y).abs() < 0.01, "origin[1]={} expected {}", origin[1], expected_y);
        assert!((origin[2] - expected_z).abs() < 0.01);
    }

    #[test]
    fn test_conceal_without_velocity_uses_current_origin() {
        let mut cent = CEntity::default();
        cent.current.origin = [100.0, 200.0, 300.0];
        cent.serverframe = 98;
        cent.velocity.valid = false;

        let mut origin = [0.0; 3];
        let result = cl_conceal_packet_loss(&cent, 100, &mut origin, true);
        assert!(result);
        assert_eq!(origin, [100.0, 200.0, 300.0]);
    }

    // -------------------------------------------------------
    // cl_smooth_entity_origin tests
    // -------------------------------------------------------

    #[test]
    fn test_smooth_origin_basic_lerp() {
        let cent = CEntity::default(); // velocity not valid
        let prev = [0.0, 0.0, 0.0];
        let curr = [100.0, 200.0, 300.0];

        let result = cl_smooth_entity_origin(
            &cent, &prev, &curr,
            0.5,    // lerpfrac
            0,      // no timenudge
            false,  // no extrapolation
            100,    // extrapolate max
            1000,   // servertime
            950,    // current_time
        );

        assert!((result[0] - 50.0).abs() < 0.01);
        assert!((result[1] - 100.0).abs() < 0.01);
        assert!((result[2] - 150.0).abs() < 0.01);
    }

    #[test]
    fn test_smooth_origin_lerp_zero() {
        let cent = CEntity::default();
        let prev = [10.0, 20.0, 30.0];
        let curr = [100.0, 200.0, 300.0];

        let result = cl_smooth_entity_origin(
            &cent, &prev, &curr, 0.0, 0, false, 100, 1000, 900,
        );

        assert!((result[0] - 10.0).abs() < 0.01);
        assert!((result[1] - 20.0).abs() < 0.01);
        assert!((result[2] - 30.0).abs() < 0.01);
    }

    #[test]
    fn test_smooth_origin_lerp_one() {
        let cent = CEntity::default();
        let prev = [10.0, 20.0, 30.0];
        let curr = [100.0, 200.0, 300.0];

        let result = cl_smooth_entity_origin(
            &cent, &prev, &curr, 1.0, 0, false, 100, 1000, 1000,
        );

        assert!((result[0] - 100.0).abs() < 0.01);
        assert!((result[1] - 200.0).abs() < 0.01);
        assert!((result[2] - 300.0).abs() < 0.01);
    }

    #[test]
    fn test_smooth_origin_with_extrapolation() {
        let cent = make_entity_with_velocity(
            [100.0, 0.0, 0.0],
            [1000.0, 0.0, 0.0],
            1000,
        );

        let result = cl_smooth_entity_origin(
            &cent,
            &cent.prev.origin,
            &cent.current.origin,
            0.95,   // near end of lerp
            0,      // no timenudge
            true,   // extrapolation enabled
            100,    // max 100ms
            1000,
            995,
        );

        // At lerpfrac 0.95, base lerp = prev + 0.95 * (curr - prev)
        // Extrapolation adds velocity * extrap_time when lerpfrac >= 0.9
        // extrap_amount = (0.95 - 0.9) * 10.0 = 0.5
        // extrap_time = 0.5 * (100 / 1000) = 0.05s
        // extra_x = 1000 * 0.05 = 50
        let base_x = cent.prev.origin[0] + 0.95 * (cent.current.origin[0] - cent.prev.origin[0]);
        assert!(result[0] > base_x, "extrapolation should push x forward: result={} base={}", result[0], base_x);
    }

    #[test]
    fn test_smooth_origin_no_extrapolation_below_threshold() {
        let cent = make_entity_with_velocity(
            [100.0, 0.0, 0.0],
            [1000.0, 0.0, 0.0],
            1000,
        );

        // lerpfrac 0.5 is below the 0.9 threshold for extrapolation
        let result = cl_smooth_entity_origin(
            &cent,
            &cent.prev.origin,
            &cent.current.origin,
            0.5,
            0,
            true, // extrapolation enabled but should not trigger
            100,
            1000,
            950,
        );

        let expected = cent.prev.origin[0] + 0.5 * (cent.current.origin[0] - cent.prev.origin[0]);
        assert!((result[0] - expected).abs() < 0.01,
            "no extrapolation expected below 0.9: result={} expected={}", result[0], expected);
    }

    // -------------------------------------------------------
    // cl_smooth_projectile_origin tests
    // -------------------------------------------------------

    #[test]
    fn test_smooth_projectile_basic_lerp() {
        let cent = CEntity::default(); // no valid velocity
        let result = cl_smooth_projectile_origin(&cent, 0.5, 1000, 800.0, false);
        // Both prev and current are [0,0,0] by default, so result should be [0,0,0]
        assert_eq!(result, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_smooth_projectile_with_velocity() {
        let cent = make_entity_with_velocity(
            [100.0, 0.0, 0.0],
            [2000.0, 0.0, 0.0], // fast projectile
            1000,
        );

        // lerpfrac 0.95, current_time 1050 (50ms after last update)
        let result = cl_smooth_projectile_origin(&cent, 0.95, 1050, 800.0, false);

        // Base lerp: prev + 0.95 * (curr - prev)
        let base_x = cent.prev.origin[0] + 0.95 * (cent.current.origin[0] - cent.prev.origin[0]);
        // Extrapolation should add to this
        assert!(result[0] >= base_x, "projectile should extrapolate forward: result={} base={}", result[0], base_x);
    }

    #[test]
    fn test_smooth_projectile_grenade_gravity() {
        let mut cent = make_entity_with_velocity(
            [0.0, 0.0, 500.0],
            [200.0, 0.0, 200.0],
            1000,
        );

        // lerpfrac 0.95, current_time 1150 (150ms after update)
        let result_no_grav = cl_smooth_projectile_origin(&cent, 0.95, 1150, 800.0, false);
        let result_with_grav = cl_smooth_projectile_origin(&cent, 0.95, 1150, 800.0, true);

        // With gravity, Z should be lower
        assert!(result_with_grav[2] < result_no_grav[2],
            "gravity should reduce Z: grav={} nograv={}", result_with_grav[2], result_no_grav[2]);
    }

    // -------------------------------------------------------
    // cl_predict_projectile tests
    // -------------------------------------------------------

    #[test]
    fn test_predict_projectile_no_velocity() {
        let proj = Projectile {
            origin: [100.0, 200.0, 300.0],
            velocity_valid: false,
            ..Projectile::default()
        };
        let result = cl_predict_projectile(&proj, 1100, 800.0);
        assert_eq!(result, [100.0, 200.0, 300.0]);
    }

    #[test]
    fn test_predict_projectile_straight_line() {
        let proj = Projectile {
            origin: [100.0, 0.0, 0.0],
            velocity: [2000.0, 0.0, 0.0],
            velocity_valid: true,
            last_update_time: 1000,
            gravity_affected: false,
            ..Projectile::default()
        };

        // 100ms later
        let result = cl_predict_projectile(&proj, 1100, 800.0);
        let expected_x = 100.0 + 2000.0 * 0.1;
        assert!((result[0] - expected_x).abs() < 0.01, "result[0]={} expected={}", result[0], expected_x);
        assert!((result[1] - 0.0).abs() < 0.01);
        assert!((result[2] - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_predict_projectile_with_gravity() {
        let proj = Projectile {
            origin: [0.0, 0.0, 500.0],
            velocity: [100.0, 0.0, 200.0],
            velocity_valid: true,
            last_update_time: 1000,
            gravity_affected: true,
            ..Projectile::default()
        };

        // 200ms later
        let result = cl_predict_projectile(&proj, 1200, 800.0);
        let dt = 0.2;
        let expected_x = 0.0 + 100.0 * dt;
        let expected_z = 500.0 + 200.0 * dt - 0.5 * 800.0 * dt * dt;

        assert!((result[0] - expected_x).abs() < 0.01, "x: {} vs {}", result[0], expected_x);
        assert!((result[2] - expected_z).abs() < 0.01, "z: {} vs {}", result[2], expected_z);
    }

    #[test]
    fn test_predict_projectile_too_old() {
        let proj = Projectile {
            origin: [100.0, 0.0, 0.0],
            velocity: [2000.0, 0.0, 0.0],
            velocity_valid: true,
            last_update_time: 1000,
            ..Projectile::default()
        };

        // 600ms later (> 500ms limit)
        let result = cl_predict_projectile(&proj, 1600, 800.0);
        assert_eq!(result, [100.0, 0.0, 0.0], "should return origin unmodified for old data");
    }

    // -------------------------------------------------------
    // cl_update_entity_animation tests
    // -------------------------------------------------------

    #[test]
    fn test_update_animation_no_missed_frames() {
        let mut cent = CEntity::default();
        cent.anim_state.frame = 5;
        cent.anim_state.oldframe = 4;
        cent.anim_state.frame_time = 50.0;
        cent.anim_state.frame_duration = 100.0;
        cent.anim_state.last_server_frame = 100;

        // Server frame 100 (no frames missed)
        cl_update_entity_animation(&mut cent, 20.0, 100);
        // No change expected when frames_missed == 0
        assert_eq!(cent.anim_state.frame, 5);
        assert!((cent.anim_state.frame_time - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_update_animation_advances_on_missed_frame() {
        let mut cent = CEntity::default();
        cent.anim_state.frame = 5;
        cent.anim_state.oldframe = 4;
        cent.anim_state.frame_time = 90.0;
        cent.anim_state.frame_duration = 100.0;
        cent.anim_state.last_server_frame = 100;

        // Server frame 101 (1 frame missed)
        cl_update_entity_animation(&mut cent, 15.0, 101);
        // 90 + 15 = 105 >= 100, so frame should advance
        assert_eq!(cent.anim_state.frame, 6, "frame should advance to 6");
        assert_eq!(cent.anim_state.oldframe, 5);
        assert!((cent.anim_state.frame_time - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_update_animation_accumulates_time() {
        let mut cent = CEntity::default();
        cent.anim_state.frame = 5;
        cent.anim_state.oldframe = 4;
        cent.anim_state.frame_time = 40.0;
        cent.anim_state.frame_duration = 100.0;
        cent.anim_state.last_server_frame = 100;

        // 1 missed frame, but not enough time to advance
        cl_update_entity_animation(&mut cent, 20.0, 101);
        assert_eq!(cent.anim_state.frame, 5, "not enough time to advance");
        assert!((cent.anim_state.frame_time - 60.0).abs() < 0.01);
    }

    #[test]
    fn test_update_animation_too_many_missed_frames() {
        let mut cent = CEntity::default();
        cent.anim_state.frame = 5;
        cent.anim_state.frame_time = 90.0;
        cent.anim_state.frame_duration = 100.0;
        cent.anim_state.last_server_frame = 100;

        // 5 frames missed (> 3 limit)
        cl_update_entity_animation(&mut cent, 15.0, 105);
        // Should NOT advance (too many missed frames)
        assert_eq!(cent.anim_state.frame, 5);
    }

    // -------------------------------------------------------
    // cl_clear_projectiles test
    // -------------------------------------------------------

    #[test]
    fn test_clear_projectiles() {
        let mut state = ProjectileState::default();
        state.projectiles[0].present = true;
        state.projectiles[0].modelindex = 1;
        state.projectiles[5].present = true;
        state.projectiles[5].modelindex = 2;

        cl_clear_projectiles(&mut state);

        for i in 0..MAX_PROJECTILES {
            assert!(!state.projectiles[i].present, "projectile {} should not be present", i);
        }
        // modelindex is preserved (clear only sets present=false)
        assert_eq!(state.projectiles[0].modelindex, 1);
    }

    // -------------------------------------------------------
    // Projectile default test
    // -------------------------------------------------------

    #[test]
    fn test_projectile_default() {
        let p = Projectile::default();
        assert_eq!(p.modelindex, 0);
        assert_eq!(p.num, 0);
        assert_eq!(p.effects, 0);
        assert_eq!(p.origin, [0.0; 3]);
        assert!(!p.present);
        assert!(!p.velocity_valid);
        assert!(!p.gravity_affected);
    }

    // -------------------------------------------------------
    // cl_smooth_entity_origin with timenudge
    // -------------------------------------------------------

    #[test]
    fn test_smooth_origin_timenudge_affects_lerp() {
        let cent = CEntity::default();
        let prev = [0.0, 0.0, 0.0];
        let curr = [100.0, 0.0, 0.0];

        let result_no_nudge = cl_smooth_entity_origin(
            &cent, &prev, &curr, 0.5, 0, false, 100, 1000, 950,
        );
        let result_positive_nudge = cl_smooth_entity_origin(
            &cent, &prev, &curr, 0.5, 20, false, 100, 1000, 950,
        );

        // Positive timenudge should result in a higher X (more lerped toward current)
        assert!(result_positive_nudge[0] > result_no_nudge[0],
            "timenudge should shift lerp: nudged={} base={}", result_positive_nudge[0], result_no_nudge[0]);
    }
}
