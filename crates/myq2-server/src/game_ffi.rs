// game_ffi.rs — FFI wrapper functions for game DLL import table
//
// These extern "C" functions implement the game_import_t interface
// that external C game DLLs will call. They convert between C types
// and Rust types, then delegate to the actual server implementation.

#![allow(non_snake_case)]

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_float, c_int, c_void};
use std::sync::Mutex;

use myq2_common::game_api::{
    self, cvar_t, edict_t, entity_state_t, game_import_t, pmove_t,
    qboolean, trace_t, SOLID_NOT, SOLID_TRIGGER, SOLID_BBOX, SOLID_BSP,
};
use myq2_common::pmove::PmoveCallbacks;
use myq2_common::q_shared::MAX_ENT_CLUSTERS;
use myq2_common::q_shared::{CSurface, Multicast, PmType, PmoveData, PmoveState, Trace, UserCmd, Vec3};

use crate::server::ServerContext;
use crate::sv_game::{
    cm_inline_model, msg_write_byte, msg_write_string, pf_configstring,
    pf_cprintf, pf_dprintf, pf_in_phs, pf_in_pvs, pf_unicast, pf_write_angle,
    pf_write_byte, pf_write_char, pf_write_dir, pf_write_float, pf_write_long,
    pf_write_pos, pf_write_short, pf_write_string,
    sv_model_index, sv_start_sound, Edict, Solid,
};
use crate::sv_send::{sv_client_printf, sv_multicast};

// ============================================================
// Global server context for FFI callbacks
// ============================================================

/// Wrapper to make *mut ServerContext Send-able
struct SendPtr(*mut ServerContext);
unsafe impl Send for SendPtr {}

/// Global server context pointer for FFI callbacks
static FFI_SERVER_CTX: Mutex<Option<SendPtr>> = Mutex::new(None);

/// Set the server context for FFI callbacks
///
/// # Safety
/// Must be called before loading a game DLL and the context must
/// remain valid for the lifetime of the DLL.
pub unsafe fn set_ffi_server_context(ctx: *mut ServerContext) {
    *FFI_SERVER_CTX.lock().unwrap() = Some(SendPtr(ctx));
}

/// Clear the FFI server context
pub fn clear_ffi_server_context() {
    *FFI_SERVER_CTX.lock().unwrap() = None;
}

/// Access the server context from FFI callbacks
fn with_ffi_ctx<F, R>(f: F) -> R
where
    F: FnOnce(&mut ServerContext) -> R,
{
    let guard = FFI_SERVER_CTX.lock().unwrap();
    let ptr = guard.as_ref().expect("FFI ServerContext not set").0;
    let ctx = unsafe { &mut *ptr };
    f(ctx)
}

// ============================================================
// Helper functions for C string conversion
// ============================================================

/// Convert a C string pointer to a Rust &str
/// Returns empty string if pointer is null
unsafe fn c_str_to_str<'a>(ptr: *const c_char) -> &'a str {
    if ptr.is_null() {
        ""
    } else {
        CStr::from_ptr(ptr).to_str().unwrap_or("")
    }
}

// ============================================================
// FFI wrapper functions (43 total)
// ============================================================

// ---- Printing ----

/// gi.bprintf - broadcast print to all clients
/// Note: Variadic format args are not processed - only the format string is used.
/// C game DLLs typically pass pre-formatted strings anyway.
unsafe extern "C" fn gi_bprintf(printlevel: c_int, fmt: *const c_char) {
    let msg = c_str_to_str(fmt);
    with_ffi_ctx(|ctx| {
        use myq2_common::qcommon::SvcOps;
        msg_write_byte(&mut ctx.sv.multicast, SvcOps::Print as i32);
        msg_write_byte(&mut ctx.sv.multicast, printlevel);
        msg_write_string(&mut ctx.sv.multicast, msg);
        sv_multicast(ctx, Some([0.0; 3]), Multicast::AllR);
    });
}

/// gi.dprintf - debug print to console
unsafe extern "C" fn gi_dprintf(fmt: *const c_char) {
    let msg = c_str_to_str(fmt);
    pf_dprintf(msg);
}

/// gi.cprintf - print to specific client
unsafe extern "C" fn gi_cprintf(ent: *mut edict_t, printlevel: c_int, fmt: *const c_char) {
    let msg = c_str_to_str(fmt);
    with_ffi_ctx(|ctx| {
        if ent.is_null() {
            pf_cprintf(ctx, None, printlevel, msg);
        } else {
            let ent_idx = game_api::num_for_edict(ctx_edicts(ctx), ctx_edict_size(ctx), ent);
            if ent_idx >= 1 && ent_idx <= ctx.maxclients_value as c_int {
                let client_idx = (ent_idx - 1) as usize;
                if let Some(client) = ctx.svs.clients.get_mut(client_idx) {
                    sv_client_printf(client, printlevel, msg);
                }
            }
        }
    });
}

/// gi.centerprintf - center screen print to specific client
unsafe extern "C" fn gi_centerprintf(ent: *mut edict_t, fmt: *const c_char) {
    let msg = c_str_to_str(fmt);
    with_ffi_ctx(|ctx| {
        if ent.is_null() {
            return;
        }
        let ent_idx = game_api::num_for_edict(ctx_edicts(ctx), ctx_edict_size(ctx), ent);
        let maxclients_val = ctx.maxclients_value as c_int;
        if ent_idx < 1 || ent_idx > maxclients_val {
            return;
        }

        use myq2_common::qcommon::SvcOps;
        msg_write_byte(&mut ctx.sv.multicast, SvcOps::CenterPrint as i32);
        msg_write_string(&mut ctx.sv.multicast, msg);

        let client_idx = (ent_idx - 1) as usize;
        let mc_data: Vec<u8> = ctx.sv.multicast.data[..ctx.sv.multicast.cursize as usize].to_vec();
        if let Some(client) = ctx.svs.clients.get_mut(client_idx) {
            client.netchan.message.write(&mc_data);
        }
        ctx.sv.multicast.clear();
    });
}

// ---- Sound ----

/// gi.sound - start a sound on an entity
unsafe extern "C" fn gi_sound(
    ent: *mut edict_t,
    channel: c_int,
    soundindex: c_int,
    volume: c_float,
    attenuation: c_float,
    timeofs: c_float,
) {
    with_ffi_ctx(|ctx| {
        let ent_idx = if ent.is_null() {
            0
        } else {
            game_api::num_for_edict(ctx_edicts(ctx), ctx_edict_size(ctx), ent)
        };

        // Create a local Edict copy for sv_start_sound
        let local_ent = if !ent.is_null() {
            Edict {
                s: entity_state_from_c(&(*ent).s),
                svflags: (*ent).svflags,
                solid: solid_from_c((*ent).solid),
                mins: (*ent).mins,
                maxs: (*ent).maxs,
                ..Edict::default()
            }
        } else {
            let mut e = Edict::default();
            e.s.number = ent_idx;
            e
        };

        sv_start_sound(ctx, None, &local_ent, channel, soundindex, volume, attenuation, timeofs);
    });
}

/// gi.positioned_sound - start a sound at a position
unsafe extern "C" fn gi_positioned_sound(
    origin: *const Vec3,
    ent: *mut edict_t,
    channel: c_int,
    soundindex: c_int,
    volume: c_float,
    attenuation: c_float,
    timeofs: c_float,
) {
    with_ffi_ctx(|ctx| {
        let ent_idx = if ent.is_null() {
            0
        } else {
            game_api::num_for_edict(ctx_edicts(ctx), ctx_edict_size(ctx), ent)
        };

        let origin_ref = if origin.is_null() {
            None
        } else {
            Some(&*origin)
        };

        let local_ent = if !ent.is_null() {
            Edict {
                s: entity_state_from_c(&(*ent).s),
                svflags: (*ent).svflags,
                solid: solid_from_c((*ent).solid),
                mins: (*ent).mins,
                maxs: (*ent).maxs,
                ..Edict::default()
            }
        } else {
            let mut e = Edict::default();
            e.s.number = ent_idx;
            e
        };

        sv_start_sound(ctx, origin_ref, &local_ent, channel, soundindex, volume, attenuation, timeofs);
    });
}

// ---- Config ----

/// gi.configstring - set a config string
unsafe extern "C" fn gi_configstring(num: c_int, string: *const c_char) {
    let s = c_str_to_str(string);
    with_ffi_ctx(|ctx| {
        pf_configstring(ctx, num, s);
    });
}

/// gi.error - fatal error (does not return)
unsafe extern "C" fn gi_error(fmt: *const c_char) -> ! {
    let msg = c_str_to_str(fmt);
    myq2_common::common::com_error(myq2_common::qcommon::ERR_DROP, msg);
    // If com_error somehow returns, panic to ensure we never return
    panic!("com_error returned unexpectedly");
}

// ---- Resource indexing ----

/// gi.modelindex - get or create model index
unsafe extern "C" fn gi_modelindex(name: *const c_char) -> c_int {
    let name_str = c_str_to_str(name);
    with_ffi_ctx(|ctx| sv_model_index(ctx, name_str))
}

/// gi.soundindex - get or create sound index
unsafe extern "C" fn gi_soundindex(name: *const c_char) -> c_int {
    let name_str = c_str_to_str(name);
    with_ffi_ctx(|ctx| {
        crate::sv_init::sv_find_index(
            ctx,
            name_str,
            myq2_common::q_shared::CS_SOUNDS,
            myq2_common::q_shared::MAX_SOUNDS,
            true,
        )
    })
}

/// gi.imageindex - get or create image index
unsafe extern "C" fn gi_imageindex(name: *const c_char) -> c_int {
    let name_str = c_str_to_str(name);
    with_ffi_ctx(|ctx| {
        crate::sv_init::sv_find_index(
            ctx,
            name_str,
            myq2_common::q_shared::CS_IMAGES,
            myq2_common::q_shared::MAX_IMAGES,
            true,
        )
    })
}

/// gi.setmodel - set entity model
unsafe extern "C" fn gi_setmodel(ent: *mut edict_t, name: *const c_char) {
    if ent.is_null() {
        return;
    }
    let name_str = c_str_to_str(name);
    with_ffi_ctx(|ctx| {
        let i = sv_model_index(ctx, name_str);
        (*ent).s.modelindex = i;

        // If it's an inline model (*N), also set bounds
        if name_str.starts_with('*') {
            let model = cm_inline_model(ctx, name_str);
            (*ent).mins = model.mins;
            (*ent).maxs = model.maxs;
            for j in 0..3 {
                (*ent).size[j] = (*ent).maxs[j] - (*ent).mins[j];
                (*ent).absmin[j] = (*ent).s.origin[j] + (*ent).mins[j] - 1.0;
                (*ent).absmax[j] = (*ent).s.origin[j] + (*ent).maxs[j] + 1.0;
            }
            if (*ent).linkcount == 0 {
                (*ent).s.old_origin = (*ent).s.origin;
            }
            (*ent).linkcount += 1;
        }
    });
}

// ---- Collision detection ----

/// gi.trace - trace a line through the world
unsafe extern "C" fn gi_trace(
    start: *const Vec3,
    mins: *const Vec3,
    maxs: *const Vec3,
    end: *const Vec3,
    passent: *mut edict_t,
    contentmask: c_int,
) -> trace_t {
    let start_ref = if start.is_null() { &[0.0; 3] } else { &*start };
    let end_ref = if end.is_null() { &[0.0; 3] } else { &*end };
    let mins_ref = if mins.is_null() { &[0.0; 3] } else { &*mins };
    let maxs_ref = if maxs.is_null() { &[0.0; 3] } else { &*maxs };

    // Perform trace through collision model
    let rust_trace = myq2_common::cmodel::with_cmodel_ctx(|cctx| {
        let headnode = if cctx.numcmodels > 0 {
            cctx.map_cmodels[0].headnode
        } else {
            0
        };
        cctx.box_trace(start_ref, end_ref, mins_ref, maxs_ref, headnode, contentmask)
    })
    .unwrap_or_default();

    // Store surface in static storage and get a stable pointer
    let surface_ptr = if let Some(surf) = rust_trace.surface {
        let boxed = Box::new(surf);
        let ptr = &*boxed as *const CSurface as *mut CSurface;
        SURFACE_STORAGE.lock().unwrap().0.push(boxed);
        ptr
    } else {
        std::ptr::null_mut()
    };

    // Convert to C trace_t
    trace_t {
        allsolid: if rust_trace.allsolid { 1 } else { 0 },
        startsolid: if rust_trace.startsolid { 1 } else { 0 },
        fraction: rust_trace.fraction,
        endpos: rust_trace.endpos,
        plane: rust_trace.plane,
        surface: surface_ptr,
        contents: rust_trace.contents,
        ent: passent, // Return passent as the hit entity for now
    }
}

/// gi.pointcontents - get contents at a point
unsafe extern "C" fn gi_pointcontents(point: *const Vec3) -> c_int {
    if point.is_null() {
        return 0;
    }
    myq2_common::cmodel::cm_point_contents(&*point, 0)
}

/// gi.inPVS - check if two points are in the same PVS
unsafe extern "C" fn gi_inPVS(p1: *const Vec3, p2: *const Vec3) -> qboolean {
    if p1.is_null() || p2.is_null() {
        return 0;
    }
    with_ffi_ctx(|ctx| if pf_in_pvs(ctx, &*p1, &*p2) { 1 } else { 0 })
}

/// gi.inPHS - check if two points are in the same PHS
unsafe extern "C" fn gi_inPHS(p1: *const Vec3, p2: *const Vec3) -> qboolean {
    if p1.is_null() || p2.is_null() {
        return 0;
    }
    with_ffi_ctx(|ctx| if pf_in_phs(ctx, &*p1, &*p2) { 1 } else { 0 })
}

/// gi.SetAreaPortalState - open/close an area portal
unsafe extern "C" fn gi_SetAreaPortalState(portalnum: c_int, open: qboolean) {
    myq2_common::cmodel::with_cmodel_ctx(|ctx| {
        ctx.set_area_portal_state(portalnum as usize, open != 0);
    });
}

/// gi.AreasConnected - check if two areas are connected
unsafe extern "C" fn gi_AreasConnected(area1: c_int, area2: c_int) -> qboolean {
    let connected = myq2_common::cmodel::with_cmodel_ctx(|ctx| {
        ctx.areas_connected(area1 as usize, area2 as usize)
    })
    .unwrap_or(false);
    if connected { 1 } else { 0 }
}

// ---- Entity linking ----

/// gi.linkentity - link an entity into the world
unsafe extern "C" fn gi_linkentity(ent: *mut edict_t) {
    if ent.is_null() {
        return;
    }

    // Compute size
    for i in 0..3 {
        (*ent).size[i] = (*ent).maxs[i] - (*ent).mins[i];
    }

    // Set abs box
    for i in 0..3 {
        (*ent).absmin[i] = (*ent).s.origin[i] + (*ent).mins[i] - 1.0;
        (*ent).absmax[i] = (*ent).s.origin[i] + (*ent).maxs[i] + 1.0;
    }

    // Copy origin to old_origin on first link
    if (*ent).linkcount == 0 {
        (*ent).s.old_origin = (*ent).s.origin;
    }
    (*ent).linkcount += 1;

    // Compute PVS cluster membership
    if (*ent).solid != SOLID_NOT || (*ent).s.modelindex != 0 {
        let leafs = myq2_common::cmodel::cm_box_leafnums(&(*ent).absmin, &(*ent).absmax, 0);

        if leafs.len() > MAX_ENT_CLUSTERS {
            (*ent).num_clusters = -1;
            (*ent).headnode = myq2_common::cmodel::cm_headnode_for_box(&(*ent).absmin, &(*ent).absmax);
        } else {
            (*ent).num_clusters = 0;
            for &leaf in &leafs {
                let cluster = myq2_common::cmodel::cm_leaf_cluster(leaf as usize);
                let area = myq2_common::cmodel::cm_leaf_area(leaf as usize);

                if area != 0 {
                    if (*ent).areanum != 0 && (*ent).areanum != area {
                        (*ent).areanum2 = area;
                    } else {
                        (*ent).areanum = area;
                    }
                }

                if cluster != -1 && ((*ent).num_clusters as usize) < MAX_ENT_CLUSTERS {
                    (*ent).clusternums[(*ent).num_clusters as usize] = cluster;
                    (*ent).num_clusters += 1;
                }
            }
        }
    }
}

/// gi.unlinkentity - unlink an entity from the world
unsafe extern "C" fn gi_unlinkentity(ent: *mut edict_t) {
    if ent.is_null() {
        return;
    }
    (*ent).area.prev = std::ptr::null_mut();
    (*ent).area.next = std::ptr::null_mut();
    (*ent).num_clusters = 0;
    (*ent).areanum = 0;
    (*ent).areanum2 = 0;
}

/// gi.BoxEdicts - find all edicts in a box
unsafe extern "C" fn gi_BoxEdicts(
    mins: *const Vec3,
    maxs: *const Vec3,
    list: *mut *mut edict_t,
    maxcount: c_int,
    _areatype: c_int,
) -> c_int {
    if mins.is_null() || maxs.is_null() || list.is_null() {
        return 0;
    }

    with_ffi_ctx(|ctx| {
        let edicts = ctx_edicts(ctx);
        let edict_size = ctx_edict_size(ctx);
        let num_edicts = ctx_num_edicts(ctx);

        let mins_ref = &*mins;
        let maxs_ref = &*maxs;
        let mut count = 0;

        for i in 0..num_edicts {
            if count >= maxcount {
                break;
            }
            let ent = game_api::edict_num(edicts, edict_size, i);
            if (*ent).inuse == 0 {
                continue;
            }

            // AABB overlap test
            if (*ent).absmin[0] > maxs_ref[0]
                || (*ent).absmin[1] > maxs_ref[1]
                || (*ent).absmin[2] > maxs_ref[2]
            {
                continue;
            }
            if (*ent).absmax[0] < mins_ref[0]
                || (*ent).absmax[1] < mins_ref[1]
                || (*ent).absmax[2] < mins_ref[2]
            {
                continue;
            }

            *list.add(count as usize) = ent;
            count += 1;
        }

        count
    })
}

/// FFI callback wrapper that implements PmoveCallbacks for C function pointers
struct FfiPmoveCallbacks {
    trace_fn: Option<
        unsafe extern "C" fn(
            start: *const Vec3,
            mins: *const Vec3,
            maxs: *const Vec3,
            end: *const Vec3,
        ) -> trace_t,
    >,
    pointcontents_fn: Option<unsafe extern "C" fn(point: *const Vec3) -> c_int>,
}

impl PmoveCallbacks for FfiPmoveCallbacks {
    fn trace(&self, start: &Vec3, mins: &Vec3, maxs: &Vec3, end: &Vec3) -> Trace {
        if let Some(trace_fn) = self.trace_fn {
            let c_trace = unsafe { trace_fn(start, mins, maxs, end) };
            // Convert C trace_t to Rust Trace
            Trace {
                allsolid: c_trace.allsolid != 0,
                startsolid: c_trace.startsolid != 0,
                fraction: c_trace.fraction,
                endpos: c_trace.endpos,
                plane: c_trace.plane,
                surface: if c_trace.surface.is_null() {
                    None
                } else {
                    Some(unsafe { (*c_trace.surface).clone() })
                },
                contents: c_trace.contents,
                ent_index: -1, // Entity index not used in pmove trace results
            }
        } else {
            Trace::default()
        }
    }

    fn pointcontents(&self, point: &Vec3) -> i32 {
        if let Some(pc_fn) = self.pointcontents_fn {
            unsafe { pc_fn(point) }
        } else {
            0
        }
    }
}

/// gi.Pmove - run player movement
unsafe extern "C" fn gi_Pmove(pmove: *mut pmove_t) {
    if pmove.is_null() {
        return;
    }

    let pm = &mut *pmove;

    // Convert C pmove_state_t.pm_type to Rust PmType
    let pm_type = match pm.s.pm_type {
        0 => PmType::Normal,
        1 => PmType::Spectator,
        2 => PmType::Dead,
        3 => PmType::Gib,
        4 => PmType::Freeze,
        _ => PmType::Normal,
    };

    // Convert C pmove_t to Rust PmoveData
    let mut pm_data = PmoveData {
        s: PmoveState {
            pm_type,
            origin: pm.s.origin,
            velocity: pm.s.velocity,
            pm_flags: pm.s.pm_flags,
            pm_time: pm.s.pm_time,
            gravity: pm.s.gravity,
            delta_angles: pm.s.delta_angles,
        },
        cmd: UserCmd {
            msec: pm.cmd.msec,
            buttons: pm.cmd.buttons,
            angles: pm.cmd.angles,
            forwardmove: pm.cmd.forwardmove,
            sidemove: pm.cmd.sidemove,
            upmove: pm.cmd.upmove,
            impulse: pm.cmd.impulse,
            lightlevel: pm.cmd.lightlevel,
        },
        snapinitial: pm.snapinitial != 0,
        numtouch: 0,
        touchents: [-1; myq2_common::q_shared::MAXTOUCH],
        viewangles: pm.viewangles,
        viewheight: pm.viewheight,
        mins: pm.mins,
        maxs: pm.maxs,
        groundentity: -1,
        watertype: pm.watertype,
        waterlevel: pm.waterlevel,
    };

    // Create FFI callback wrapper
    let callbacks = FfiPmoveCallbacks {
        trace_fn: pm.trace,
        pointcontents_fn: pm.pointcontents,
    };

    // Execute pmove
    myq2_common::pmove::pmove(&mut pm_data, &callbacks);

    // Copy results back to C struct
    pm.s.pm_type = pm_data.s.pm_type as c_int;
    pm.s.origin = pm_data.s.origin;
    pm.s.velocity = pm_data.s.velocity;
    pm.s.pm_flags = pm_data.s.pm_flags;
    pm.s.pm_time = pm_data.s.pm_time;
    pm.s.gravity = pm_data.s.gravity;
    pm.s.delta_angles = pm_data.s.delta_angles;

    pm.numtouch = pm_data.numtouch;
    // Note: touchents conversion from indices to pointers would require access to
    // the game's edict array. For now, we leave touchents empty since the game
    // DLL typically doesn't use them directly from pmove results.
    for i in 0..pm_data.numtouch as usize {
        if i >= game_api::MAXTOUCH {
            break;
        }
        // touchents are entity indices in Rust but edict_t* in C
        // Without access to the edict array, we can't convert. Set to null.
        pm.touchents[i] = std::ptr::null_mut();
    }

    pm.viewangles = pm_data.viewangles;
    pm.viewheight = pm_data.viewheight;
    pm.mins = pm_data.mins;
    pm.maxs = pm_data.maxs;

    // groundentity: Convert index to pointer (would need edict array access)
    pm.groundentity = if pm_data.groundentity < 0 {
        std::ptr::null_mut()
    } else {
        // Without edict array access, we can't convert. Set to null.
        // The game DLL typically recalculates groundentity anyway.
        std::ptr::null_mut()
    };

    pm.watertype = pm_data.watertype;
    pm.waterlevel = pm_data.waterlevel;
}

// ---- Network messaging ----

/// gi.multicast - send multicast message
unsafe extern "C" fn gi_multicast(origin: *const Vec3, to: c_int) {
    let origin_ref = if origin.is_null() {
        &[0.0; 3]
    } else {
        unsafe { &*origin }
    };

    let mc = match to {
        0 => Multicast::All,
        1 => Multicast::Phs,
        2 => Multicast::Pvs,
        3 => Multicast::AllR,
        4 => Multicast::PhsR,
        5 => Multicast::PvsR,
        _ => Multicast::All,
    };

    with_ffi_ctx(|ctx| {
        sv_multicast(ctx, Some(*origin_ref), mc);
    });
}

/// gi.unicast - send unicast message to a client
unsafe extern "C" fn gi_unicast(ent: *mut edict_t, reliable: qboolean) {
    with_ffi_ctx(|ctx| {
        let ent_idx = if ent.is_null() {
            0
        } else {
            game_api::num_for_edict(ctx_edicts(ctx), ctx_edict_size(ctx), ent)
        };
        let mut local_ent = Edict::default();
        local_ent.s.number = ent_idx;
        pf_unicast(ctx, &local_ent, reliable != 0);
    });
}

/// gi.WriteChar - write a char to message buffer
unsafe extern "C" fn gi_WriteChar(c: c_int) {
    with_ffi_ctx(|ctx| pf_write_char(ctx, c));
}

/// gi.WriteByte - write a byte to message buffer
unsafe extern "C" fn gi_WriteByte(c: c_int) {
    with_ffi_ctx(|ctx| pf_write_byte(ctx, c));
}

/// gi.WriteShort - write a short to message buffer
unsafe extern "C" fn gi_WriteShort(c: c_int) {
    with_ffi_ctx(|ctx| pf_write_short(ctx, c));
}

/// gi.WriteLong - write a long to message buffer
unsafe extern "C" fn gi_WriteLong(c: c_int) {
    with_ffi_ctx(|ctx| pf_write_long(ctx, c));
}

/// gi.WriteFloat - write a float to message buffer
unsafe extern "C" fn gi_WriteFloat(f: c_float) {
    with_ffi_ctx(|ctx| pf_write_float(ctx, f));
}

/// gi.WriteString - write a string to message buffer
unsafe extern "C" fn gi_WriteString(s: *const c_char) {
    let msg = c_str_to_str(s);
    with_ffi_ctx(|ctx| pf_write_string(ctx, msg));
}

/// gi.WritePosition - write a position to message buffer
unsafe extern "C" fn gi_WritePosition(pos: *const Vec3) {
    if pos.is_null() {
        return;
    }
    with_ffi_ctx(|ctx| pf_write_pos(ctx, unsafe { &*pos }));
}

/// gi.WriteDir - write a direction to message buffer
unsafe extern "C" fn gi_WriteDir(dir: *const Vec3) {
    if dir.is_null() {
        return;
    }
    with_ffi_ctx(|ctx| pf_write_dir(ctx, unsafe { &*dir }));
}

/// gi.WriteAngle - write an angle to message buffer
unsafe extern "C" fn gi_WriteAngle(f: c_float) {
    with_ffi_ctx(|ctx| pf_write_angle(ctx, f));
}

// ---- Memory management (stubs) ----

/// gi.TagMalloc - allocate tagged memory
unsafe extern "C" fn gi_TagMalloc(size: c_int, _tag: c_int) -> *mut c_void {
    // Allocate memory using Rust's allocator
    let layout = std::alloc::Layout::from_size_align(size as usize, 8).unwrap();
    std::alloc::alloc_zeroed(layout) as *mut c_void
}

/// gi.TagFree - free tagged memory (intentional no-op)
///
/// Q2's game DLLs primarily use FreeTags() for batch deallocation at level changes.
/// TagFree() is rarely called individually. The original Q2 hunk allocator also
/// did not support individual frees. Memory is reclaimed when the process exits
/// or via FreeTags().
unsafe extern "C" fn gi_TagFree(block: *mut c_void) {
    if block.is_null() {
        return;
    }
    // Intentional no-op - matches original Q2 hunk allocator behavior.
    // Implementing would require tracking allocation sizes in a HashMap.
}

/// gi.FreeTags - free all memory with a tag
unsafe extern "C" fn gi_FreeTags(_tag: c_int) {
    // No-op - would need allocation tracking
}

// ---- Cvar interaction ----

// Static storage for surface pointers returned from gi_trace
// This is needed because the C API expects stable pointers to CSurface
struct SurfaceStorageWrapper(Vec<Box<CSurface>>);
// SAFETY: CSurface is plain data, accessed only from the main thread during game callbacks
unsafe impl Send for SurfaceStorageWrapper {}
static SURFACE_STORAGE: Mutex<SurfaceStorageWrapper> = Mutex::new(SurfaceStorageWrapper(Vec::new()));

/// Clear the surface storage (call on map change or periodically)
pub fn clear_surface_storage() {
    SURFACE_STORAGE.lock().unwrap().0.clear();
}

// Static storage for cvar return values
// This is needed because the C API returns pointers to cvar_t
// We use a wrapper to implement Send for the raw pointers
struct CvarStorageWrapper(Vec<Box<CvarStorage>>);
// SAFETY: CvarStorage contains raw pointers, but they're only accessed
// from the main thread during game callbacks
unsafe impl Send for CvarStorageWrapper {}
static CVAR_STORAGE: Mutex<CvarStorageWrapper> = Mutex::new(CvarStorageWrapper(Vec::new()));

struct CvarStorage {
    name: CString,
    string: CString,
    cvar: cvar_t,
}

/// gi.cvar - get or create a cvar
unsafe extern "C" fn gi_cvar(
    var_name: *const c_char,
    value: *const c_char,
    flags: c_int,
) -> *mut cvar_t {
    let name_str = c_str_to_str(var_name);
    let value_str = c_str_to_str(value);

    myq2_common::cvar::cvar_get(name_str, value_str, flags);
    let actual_value = myq2_common::cvar::cvar_variable_value(name_str);

    // Create persistent storage for the cvar
    let name_c = CString::new(name_str).unwrap_or_default();
    let string_c = CString::new(format!("{}", actual_value)).unwrap_or_default();

    let mut storage = Box::new(CvarStorage {
        name: name_c,
        string: string_c,
        cvar: cvar_t {
            name: std::ptr::null_mut(),
            string: std::ptr::null_mut(),
            latched_string: std::ptr::null_mut(),
            flags,
            modified: 0,
            value: actual_value,
            next: std::ptr::null_mut(),
        },
    });

    storage.cvar.name = storage.name.as_ptr() as *mut c_char;
    storage.cvar.string = storage.string.as_ptr() as *mut c_char;

    let ptr = &mut storage.cvar as *mut cvar_t;

    let mut guard = CVAR_STORAGE.lock().unwrap();
    guard.0.push(storage);

    ptr
}

/// gi.cvar_set - set a cvar value
unsafe extern "C" fn gi_cvar_set(var_name: *const c_char, value: *const c_char) -> *mut cvar_t {
    let name_str = c_str_to_str(var_name);
    let value_str = c_str_to_str(value);
    myq2_common::cvar::cvar_set(name_str, value_str);
    gi_cvar(var_name, value, 0)
}

/// gi.cvar_forceset - force set a cvar value
unsafe extern "C" fn gi_cvar_forceset(var_name: *const c_char, value: *const c_char) -> *mut cvar_t {
    let name_str = c_str_to_str(var_name);
    let value_str = c_str_to_str(value);
    myq2_common::cvar::cvar_force_set(name_str, value_str);
    gi_cvar(var_name, value, 0)
}

// ---- Command argument access ----

// Static storage for command argument strings
static ARGV_STORAGE: Mutex<Vec<CString>> = Mutex::new(Vec::new());

/// gi.argc - get command argument count
unsafe extern "C" fn gi_argc() -> c_int {
    myq2_common::cmd::cmd_argc() as c_int
}

/// gi.argv - get command argument N
unsafe extern "C" fn gi_argv(n: c_int) -> *mut c_char {
    let arg = myq2_common::cmd::cmd_argv(n as usize);
    let c_arg = CString::new(arg).unwrap_or_default();
    let ptr = c_arg.as_ptr() as *mut c_char;

    // Store to keep alive
    let mut guard = ARGV_STORAGE.lock().unwrap();
    guard.push(c_arg);

    ptr
}

/// gi.args - get all command arguments
unsafe extern "C" fn gi_args() -> *mut c_char {
    let args = myq2_common::cmd::cmd_args();
    let c_args = CString::new(args).unwrap_or_default();
    let ptr = c_args.as_ptr() as *mut c_char;

    let mut guard = ARGV_STORAGE.lock().unwrap();
    guard.push(c_args);

    ptr
}

// ---- Misc ----

/// gi.AddCommandString - add command to command buffer
unsafe extern "C" fn gi_AddCommandString(text: *const c_char) {
    let cmd = c_str_to_str(text);
    myq2_common::cmd::cbuf_add_text(cmd);
}

/// gi.DebugGraph - draw debug graph (no-op)
unsafe extern "C" fn gi_DebugGraph(_value: c_float, _color: c_int) {
    // No-op
}

// ============================================================
// Build the game_import_t struct with all function pointers
// ============================================================

/// Build a game_import_t struct populated with our FFI wrapper functions
///
/// This is passed to GetGameApi when loading an external game DLL.
pub fn build_game_import() -> game_import_t {
    game_import_t {
        bprintf: Some(gi_bprintf),
        dprintf: Some(gi_dprintf),
        cprintf: Some(gi_cprintf),
        centerprintf: Some(gi_centerprintf),
        sound: Some(gi_sound),
        positioned_sound: Some(gi_positioned_sound),
        configstring: Some(gi_configstring),
        error: Some(gi_error),
        modelindex: Some(gi_modelindex),
        soundindex: Some(gi_soundindex),
        imageindex: Some(gi_imageindex),
        setmodel: Some(gi_setmodel),
        trace: Some(gi_trace),
        pointcontents: Some(gi_pointcontents),
        inPVS: Some(gi_inPVS),
        inPHS: Some(gi_inPHS),
        SetAreaPortalState: Some(gi_SetAreaPortalState),
        AreasConnected: Some(gi_AreasConnected),
        linkentity: Some(gi_linkentity),
        unlinkentity: Some(gi_unlinkentity),
        BoxEdicts: Some(gi_BoxEdicts),
        Pmove: Some(gi_Pmove),
        multicast: Some(gi_multicast),
        unicast: Some(gi_unicast),
        WriteChar: Some(gi_WriteChar),
        WriteByte: Some(gi_WriteByte),
        WriteShort: Some(gi_WriteShort),
        WriteLong: Some(gi_WriteLong),
        WriteFloat: Some(gi_WriteFloat),
        WriteString: Some(gi_WriteString),
        WritePosition: Some(gi_WritePosition),
        WriteDir: Some(gi_WriteDir),
        WriteAngle: Some(gi_WriteAngle),
        TagMalloc: Some(gi_TagMalloc),
        TagFree: Some(gi_TagFree),
        FreeTags: Some(gi_FreeTags),
        cvar: Some(gi_cvar),
        cvar_set: Some(gi_cvar_set),
        cvar_forceset: Some(gi_cvar_forceset),
        argc: Some(gi_argc),
        argv: Some(gi_argv),
        args: Some(gi_args),
        AddCommandString: Some(gi_AddCommandString),
        DebugGraph: Some(gi_DebugGraph),
    }
}

// ============================================================
// Helper functions for context access
// ============================================================

/// Get edicts pointer from context (for dynamic DLL mode)
fn ctx_edicts(ctx: &ServerContext) -> *mut edict_t {
    if let Some(ref game_module) = ctx.game_module {
        unsafe { game_module.edicts_ptr() }
    } else {
        std::ptr::null_mut()
    }
}

/// Get edict size from context
fn ctx_edict_size(ctx: &ServerContext) -> c_int {
    if let Some(ref game_module) = ctx.game_module {
        game_module.edict_size()
    } else {
        std::mem::size_of::<edict_t>() as c_int
    }
}

/// Get num_edicts from context
fn ctx_num_edicts(ctx: &ServerContext) -> c_int {
    if let Some(ref game_module) = ctx.game_module {
        game_module.num_edicts()
    } else {
        0
    }
}

/// Convert C entity_state_t to Rust EntityState
fn entity_state_from_c(c: &entity_state_t) -> myq2_common::q_shared::EntityState {
    myq2_common::q_shared::EntityState {
        number: c.number,
        origin: c.origin,
        angles: c.angles,
        old_origin: c.old_origin,
        modelindex: c.modelindex,
        modelindex2: c.modelindex2,
        modelindex3: c.modelindex3,
        modelindex4: c.modelindex4,
        frame: c.frame,
        skinnum: c.skinnum,
        effects: c.effects,
        renderfx: c.renderfx,
        solid: c.solid,
        sound: c.sound,
        event: c.event,
    }
}

/// Convert C solid value to Rust Solid enum
fn solid_from_c(c: c_int) -> Solid {
    match c {
        SOLID_TRIGGER => Solid::Trigger,
        SOLID_BBOX => Solid::Bbox,
        SOLID_BSP => Solid::Bsp,
        _ => Solid::Not,
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;
    use myq2_common::q_shared::{CPlane, CSurface, Trace};

    // ---- Struct size validation ----

    #[test]
    fn test_edict_t_size_is_nonzero() {
        // edict_t is the C-compatible FFI struct; it must have a nonzero size
        let sz = size_of::<edict_t>();
        assert!(sz > 0, "edict_t size should be nonzero");
        // It contains entity_state_t, several Vec3 fields, pointer fields, and
        // an array of MAX_ENT_CLUSTERS ints. On 64-bit it should be several hundred bytes.
        assert!(sz >= 200, "edict_t size should be at least 200 bytes, got {}", sz);
    }

    #[test]
    fn test_trace_t_size_and_layout() {
        // trace_t is repr(C) and must have a stable size for FFI
        let sz = size_of::<trace_t>();
        assert!(sz > 0, "trace_t size should be nonzero");
        // Contains: 2 ints, 1 float, Vec3(12), CPlane, 1 ptr, 1 int, 1 ptr
        // On 64-bit: at least 2*4 + 4 + 12 + sizeof(CPlane) + 8 + 4 + 8 = ~60+ bytes
        assert!(sz >= 48, "trace_t should be at least 48 bytes, got {}", sz);
    }

    #[test]
    fn test_game_import_t_size() {
        // game_import_t has 44 Option<fn> fields. On 64-bit each is 8 bytes.
        let sz = size_of::<game_import_t>();
        let expected = 44 * size_of::<Option<unsafe extern "C" fn()>>();
        assert_eq!(sz, expected, "game_import_t should be 44 function pointers ({} bytes), got {}", expected, sz);
    }

    // ---- Edict pointer arithmetic ----

    #[test]
    fn test_edict_num_returns_correct_offset() {
        // Allocate a buffer large enough for 4 edicts
        let edict_size = size_of::<edict_t>() as c_int;
        let num_edicts = 4;
        let buf = vec![0u8; (edict_size * num_edicts) as usize];
        let base = buf.as_ptr() as *mut edict_t;

        for i in 0..num_edicts {
            let ptr = unsafe { game_api::edict_num(base, edict_size, i) };
            let expected_offset = (edict_size * i) as usize;
            let actual_offset = ptr as usize - base as usize;
            assert_eq!(actual_offset, expected_offset,
                "edict_num({}) offset mismatch: expected {}, got {}", i, expected_offset, actual_offset);
        }
    }

    #[test]
    fn test_edict_num_zero_is_base() {
        let edict_size = size_of::<edict_t>() as c_int;
        let buf = vec![0u8; edict_size as usize * 2];
        let base = buf.as_ptr() as *mut edict_t;

        let ptr = unsafe { game_api::edict_num(base, edict_size, 0) };
        assert_eq!(ptr, base, "edict_num(0) should return base pointer");
    }

    // ---- Entity number extraction ----

    #[test]
    fn test_num_for_edict_roundtrip() {
        // Verify that num_for_edict(edict_num(n)) == n
        let edict_size = size_of::<edict_t>() as c_int;
        let num_edicts = 8;
        let buf = vec![0u8; (edict_size * num_edicts) as usize];
        let base = buf.as_ptr() as *mut edict_t;

        for i in 0..num_edicts {
            let ptr = unsafe { game_api::edict_num(base, edict_size, i) };
            let idx = unsafe { game_api::num_for_edict(base, edict_size, ptr) };
            assert_eq!(idx, i, "Round-trip failed for edict {}: got {}", i, idx);
        }
    }

    #[test]
    fn test_num_for_edict_base_is_zero() {
        let edict_size = size_of::<edict_t>() as c_int;
        let buf = vec![0u8; edict_size as usize * 2];
        let base = buf.as_ptr() as *mut edict_t;

        let idx = unsafe { game_api::num_for_edict(base, edict_size, base) };
        assert_eq!(idx, 0, "num_for_edict on base pointer should return 0");
    }

    // ---- Edict pointer arithmetic with custom edict_size ----

    #[test]
    fn test_edict_num_with_larger_edict_size() {
        // Simulate a game DLL that has a larger edict struct (game extends edict_t)
        let base_size = size_of::<edict_t>();
        let extended_size = base_size + 256; // game DLL adds 256 bytes of private data
        let num_edicts = 4;
        let buf = vec![0u8; extended_size * num_edicts];
        let base = buf.as_ptr() as *mut edict_t;

        for i in 0..num_edicts as c_int {
            let ptr = unsafe { game_api::edict_num(base, extended_size as c_int, i) };
            let offset = ptr as usize - base as usize;
            assert_eq!(offset, (extended_size as c_int * i) as usize,
                "Extended edict_num({}) offset mismatch", i);

            let idx = unsafe { game_api::num_for_edict(base, extended_size as c_int, ptr) };
            assert_eq!(idx, i, "Extended round-trip failed for edict {}", i);
        }
    }

    // ---- solid_from_c conversion ----

    #[test]
    fn test_solid_from_c_all_values() {
        assert_eq!(solid_from_c(SOLID_NOT), Solid::Not);
        assert_eq!(solid_from_c(SOLID_TRIGGER), Solid::Trigger);
        assert_eq!(solid_from_c(SOLID_BBOX), Solid::Bbox);
        assert_eq!(solid_from_c(SOLID_BSP), Solid::Bsp);
        // Unknown values should default to Not
        assert_eq!(solid_from_c(99), Solid::Not);
        assert_eq!(solid_from_c(-1), Solid::Not);
    }

    // ---- entity_state_from_c conversion ----

    #[test]
    fn test_entity_state_from_c_preserves_fields() {
        let c_state = entity_state_t {
            number: 42,
            origin: [1.0, 2.0, 3.0],
            angles: [10.0, 20.0, 30.0],
            old_origin: [4.0, 5.0, 6.0],
            modelindex: 7,
            modelindex2: 8,
            modelindex3: 9,
            modelindex4: 10,
            frame: 11,
            skinnum: 12,
            effects: 0x0000_CAFE,
            renderfx: 14,
            solid: 15,
            sound: 16,
            event: 17,
        };

        let rust_state = entity_state_from_c(&c_state);

        assert_eq!(rust_state.number, 42);
        assert_eq!(rust_state.origin, [1.0, 2.0, 3.0]);
        assert_eq!(rust_state.angles, [10.0, 20.0, 30.0]);
        assert_eq!(rust_state.old_origin, [4.0, 5.0, 6.0]);
        assert_eq!(rust_state.modelindex, 7);
        assert_eq!(rust_state.modelindex2, 8);
        assert_eq!(rust_state.modelindex3, 9);
        assert_eq!(rust_state.modelindex4, 10);
        assert_eq!(rust_state.frame, 11);
        assert_eq!(rust_state.skinnum, 12);
        assert_eq!(rust_state.effects, 0x0000_CAFE);
        assert_eq!(rust_state.renderfx, 14);
        assert_eq!(rust_state.solid, 15);
        assert_eq!(rust_state.sound, 16);
        assert_eq!(rust_state.event, 17);
    }

    // ---- build_game_import validation ----

    #[test]
    fn test_build_game_import_all_functions_set() {
        let gi = build_game_import();

        // Every function pointer in the import table should be Some
        assert!(gi.bprintf.is_some(), "bprintf should be set");
        assert!(gi.dprintf.is_some(), "dprintf should be set");
        assert!(gi.cprintf.is_some(), "cprintf should be set");
        assert!(gi.centerprintf.is_some(), "centerprintf should be set");
        assert!(gi.sound.is_some(), "sound should be set");
        assert!(gi.positioned_sound.is_some(), "positioned_sound should be set");
        assert!(gi.configstring.is_some(), "configstring should be set");
        assert!(gi.error.is_some(), "error should be set");
        assert!(gi.modelindex.is_some(), "modelindex should be set");
        assert!(gi.soundindex.is_some(), "soundindex should be set");
        assert!(gi.imageindex.is_some(), "imageindex should be set");
        assert!(gi.setmodel.is_some(), "setmodel should be set");
        assert!(gi.trace.is_some(), "trace should be set");
        assert!(gi.pointcontents.is_some(), "pointcontents should be set");
        assert!(gi.inPVS.is_some(), "inPVS should be set");
        assert!(gi.inPHS.is_some(), "inPHS should be set");
        assert!(gi.SetAreaPortalState.is_some(), "SetAreaPortalState should be set");
        assert!(gi.AreasConnected.is_some(), "AreasConnected should be set");
        assert!(gi.linkentity.is_some(), "linkentity should be set");
        assert!(gi.unlinkentity.is_some(), "unlinkentity should be set");
        assert!(gi.BoxEdicts.is_some(), "BoxEdicts should be set");
        assert!(gi.Pmove.is_some(), "Pmove should be set");
        assert!(gi.multicast.is_some(), "multicast should be set");
        assert!(gi.unicast.is_some(), "unicast should be set");
        assert!(gi.WriteChar.is_some(), "WriteChar should be set");
        assert!(gi.WriteByte.is_some(), "WriteByte should be set");
        assert!(gi.WriteShort.is_some(), "WriteShort should be set");
        assert!(gi.WriteLong.is_some(), "WriteLong should be set");
        assert!(gi.WriteFloat.is_some(), "WriteFloat should be set");
        assert!(gi.WriteString.is_some(), "WriteString should be set");
        assert!(gi.WritePosition.is_some(), "WritePosition should be set");
        assert!(gi.WriteDir.is_some(), "WriteDir should be set");
        assert!(gi.WriteAngle.is_some(), "WriteAngle should be set");
        assert!(gi.TagMalloc.is_some(), "TagMalloc should be set");
        assert!(gi.TagFree.is_some(), "TagFree should be set");
        assert!(gi.FreeTags.is_some(), "FreeTags should be set");
        assert!(gi.cvar.is_some(), "cvar should be set");
        assert!(gi.cvar_set.is_some(), "cvar_set should be set");
        assert!(gi.cvar_forceset.is_some(), "cvar_forceset should be set");
        assert!(gi.argc.is_some(), "argc should be set");
        assert!(gi.argv.is_some(), "argv should be set");
        assert!(gi.args.is_some(), "args should be set");
        assert!(gi.AddCommandString.is_some(), "AddCommandString should be set");
        assert!(gi.DebugGraph.is_some(), "DebugGraph should be set");
    }

    // ---- trace_t default/conversion ----

    #[test]
    fn test_trace_t_default_values() {
        let trace = trace_t::default();
        assert_eq!(trace.allsolid, 0);
        assert_eq!(trace.startsolid, 0);
        assert_eq!(trace.fraction, 1.0);
        assert_eq!(trace.endpos, [0.0; 3]);
        assert!(trace.surface.is_null());
        assert_eq!(trace.contents, 0);
        assert!(trace.ent.is_null());
    }

    #[test]
    fn test_trace_t_to_rust_trace_conversion_logic() {
        // Verify the conversion pattern used in gi_trace and FfiPmoveCallbacks
        let c_trace = trace_t {
            allsolid: 1,
            startsolid: 0,
            fraction: 0.5,
            endpos: [10.0, 20.0, 30.0],
            plane: CPlane {
                normal: [0.0, 1.0, 0.0],
                dist: 42.0,
                plane_type: 1,
                signbits: 2,
                pad: [0; 2],
            },
            surface: std::ptr::null_mut(),
            contents: 0x0001,
            ent: std::ptr::null_mut(),
        };

        // Simulate the conversion done in FfiPmoveCallbacks::trace
        let rust_trace = Trace {
            allsolid: c_trace.allsolid != 0,
            startsolid: c_trace.startsolid != 0,
            fraction: c_trace.fraction,
            endpos: c_trace.endpos,
            plane: c_trace.plane,
            surface: None,
            contents: c_trace.contents,
            ent_index: -1,
        };

        assert!(rust_trace.allsolid);
        assert!(!rust_trace.startsolid);
        assert_eq!(rust_trace.fraction, 0.5);
        assert_eq!(rust_trace.endpos, [10.0, 20.0, 30.0]);
        assert_eq!(rust_trace.plane.normal, [0.0, 1.0, 0.0]);
        assert_eq!(rust_trace.plane.dist, 42.0);
        assert!(rust_trace.surface.is_none());
        assert_eq!(rust_trace.contents, 0x0001);
    }

    // ---- CPlane struct layout ----

    #[test]
    fn test_cplane_struct_size() {
        // CPlane: Vec3(12) + f32(4) + u8 + u8 + [u8;2] = 20 bytes
        assert_eq!(size_of::<CPlane>(), 20, "CPlane should be 20 bytes");
    }

    // ---- CSurface struct layout ----

    #[test]
    fn test_csurface_struct_size() {
        // CSurface: [u8;16] + i32(4) + i32(4) = 24 bytes
        assert_eq!(size_of::<CSurface>(), 24, "CSurface should be 24 bytes");
    }
}
