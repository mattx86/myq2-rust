// server_game_import.rs — Real GameImport implementation backed by server state.
//
// This struct implements myq2_game::game_import::GameImport, bridging the game
// module to the server without introducing a circular dependency. The server
// crate depends on myq2-game (for the trait), and this file provides the
// concrete implementation that wires every trait method to the actual pf_*
// functions and server subsystems.

use std::sync::Mutex;

use myq2_common::common::com_dprintf;
use myq2_common::q_shared::{
    Vec3, Trace, Multicast,
    CS_SOUNDS, CS_IMAGES, MAX_SOUNDS, MAX_IMAGES,
    vector_length, vector_subtract,
};
use myq2_common::qcommon::SvcOps;

use myq2_game::game_import::GameImport;

use crate::server::ServerContext;
use crate::sv_game::*;
use crate::sv_send::{sv_client_printf, sv_multicast};

// ============================================================
// Global server context pointer
//
// Before calling into game code, the server sets this pointer
// to its ServerContext. Game callbacks (via this GameImport impl)
// then use it to access server state.
// ============================================================

/// Wrapper to make *mut ServerContext Send-able. The server guarantees
/// the pointer is valid whenever game code is running.
struct SendPtr(*mut ServerContext);

// SAFETY: The server sets this pointer before game callbacks and clears it
// after. Only one thread accesses server state at a time.
unsafe impl Send for SendPtr {}

static SERVER_CTX: Mutex<Option<SendPtr>> = Mutex::new(None);

/// Set the global server context pointer. Must be called before any game code
/// runs. The caller must ensure the pointer remains valid for the duration.
///
/// # Safety
/// The caller must guarantee that `ctx` lives at least as long as any game
/// code that might call back through the GameImport trait.
pub unsafe fn set_server_context(ctx: *mut ServerContext) {
    *SERVER_CTX.lock().unwrap() = Some(SendPtr(ctx));
}

/// Clear the global server context pointer.
pub fn clear_server_context() {
    *SERVER_CTX.lock().unwrap() = None;
}

/// Access the server context. Panics if not set.
fn with_ctx<F, R>(f: F) -> R
where
    F: FnOnce(&mut ServerContext) -> R,
{
    let guard = SERVER_CTX.lock().unwrap();
    let ptr = guard.as_ref().expect("ServerContext not set").0;
    // SAFETY: The server sets this pointer before calling game code and
    // clears it after. The pointer is valid for the duration of the game call.
    let ctx = unsafe { &mut *ptr };
    f(ctx)
}

// ============================================================
// ServerGameImport
// ============================================================

/// Real implementation of GameImport backed by server state.
/// All methods delegate to the pf_* functions in sv_game.rs and
/// related server modules.
pub struct ServerGameImport;

impl GameImport for ServerGameImport {
    // ---- Printing ----

    fn bprintf(&self, printlevel: i32, msg: &str) {
        with_ctx(|ctx| {
            msg_write_byte(&mut ctx.sv.multicast, SvcOps::Print as i32);
            msg_write_byte(&mut ctx.sv.multicast, printlevel);
            msg_write_string(&mut ctx.sv.multicast, msg);
            sv_multicast(ctx, Some([0.0; 3]), Multicast::AllR);
        });
    }

    fn dprintf(&self, msg: &str) {
        pf_dprintf(msg);
    }

    fn cprintf(&self, ent_idx: i32, printlevel: i32, msg: &str) {
        with_ctx(|ctx| {
            if ent_idx == 0 {
                pf_cprintf(ctx, None, printlevel, msg);
            } else {
                let maxclients_val = ctx.maxclients_value as i32;
                if ent_idx >= 1 && ent_idx <= maxclients_val {
                    let client_idx = (ent_idx - 1) as usize;
                    if let Some(client) = ctx.svs.clients.get_mut(client_idx) {
                        sv_client_printf(client, printlevel, msg);
                    }
                }
            }
        });
    }

    fn centerprintf(&self, ent_idx: i32, msg: &str) {
        with_ctx(|ctx| {
            let maxclients_val = ctx.maxclients_value as i32;
            if ent_idx < 1 || ent_idx > maxclients_val {
                return;
            }
            msg_write_byte(&mut ctx.sv.multicast, SvcOps::CenterPrint as i32);
            msg_write_string(&mut ctx.sv.multicast, msg);

            // Unicast to the specific client
            let client_idx = (ent_idx - 1) as usize;
            let mc_data: Vec<u8> = ctx.sv.multicast.data[..ctx.sv.multicast.cursize as usize].to_vec();
            if let Some(client) = ctx.svs.clients.get_mut(client_idx) {
                client.netchan.message.write(&mc_data);
            }
            ctx.sv.multicast.clear();
        });
    }

    // ---- Sound ----

    fn sound(&self, ent_idx: i32, channel: i32, soundindex: i32, volume: f32, attenuation: f32, timeofs: f32) {
        with_ctx(|ctx| {
            // Copy entity data from game export to avoid borrow conflicts
            let local_ent = if let Some(ref ge) = ctx.ge {
                if let Some(ent) = ge.edicts.get(ent_idx as usize) {
                    Edict {
                        s: ent.s.clone(),
                        svflags: ent.svflags,
                        solid: ent.solid,
                        mins: ent.mins,
                        maxs: ent.maxs,
                        ..Edict::default()
                    }
                } else {
                    let mut e = Edict::default();
                    e.s.number = ent_idx;
                    e
                }
            } else {
                let mut e = Edict::default();
                e.s.number = ent_idx;
                e
            };
            sv_start_sound(ctx, None, &local_ent, channel, soundindex, volume, attenuation, timeofs);
        });
    }

    fn positioned_sound(&self, origin: &Vec3, ent_idx: i32, channel: i32, soundindex: i32, volume: f32, attenuation: f32, timeofs: f32) {
        with_ctx(|ctx| {
            let mut local_ent = Edict::default();
            local_ent.s.number = ent_idx;

            if let Some(ref ge) = ctx.ge {
                if let Some(ent) = ge.edicts.get(ent_idx as usize) {
                    local_ent.s = ent.s.clone();
                    local_ent.svflags = ent.svflags;
                    local_ent.solid = ent.solid;
                    local_ent.mins = ent.mins;
                    local_ent.maxs = ent.maxs;
                }
            }

            sv_start_sound(ctx, Some(origin), &local_ent, channel, soundindex, volume, attenuation, timeofs);
        });
    }

    // ---- Config ----

    fn configstring(&self, num: i32, string: &str) {
        with_ctx(|ctx| {
            pf_configstring(ctx, num, string);
        });
    }

    fn error(&self, msg: &str) {
        myq2_common::common::com_error(myq2_common::qcommon::ERR_DROP, msg);
    }

    // ---- Indexing ----

    fn modelindex(&self, name: &str) -> i32 {
        with_ctx(|ctx| {
            let result = sv_model_index(ctx, name);
            com_dprintf(&format!("gi_modelindex: name={}, result={}\n", name, result));
            result
        })
    }

    fn soundindex(&self, name: &str) -> i32 {
        with_ctx(|ctx| {
            crate::sv_init::sv_find_index(ctx, name, CS_SOUNDS, MAX_SOUNDS, true)
        })
    }

    fn imageindex(&self, name: &str) -> i32 {
        with_ctx(|ctx| {
            crate::sv_init::sv_find_index(ctx, name, CS_IMAGES, MAX_IMAGES, true)
        })
    }

    fn setmodel(&self, ent_idx: i32, name: &str) {
        let is_inline = name.starts_with('*');

        with_ctx(|ctx| {
            // Get the model index
            let i = sv_model_index(ctx, name);

            // For inline models, get mins/maxs from BSP
            let inline_model = if is_inline {
                Some(cm_inline_model(ctx, name))
            } else {
                None
            };

            // Update the server edict
            if let Some(ref mut ge) = ctx.ge {
                if let Some(ent) = ge.edicts.get_mut(ent_idx as usize) {
                    ent.s.modelindex = i;

                    if let Some(ref model) = inline_model {
                        ent.mins = model.mins;
                        ent.maxs = model.maxs;
                    }

                }
            }

            // Note: We do NOT write modelindex back to GAME_CONTEXT here.
            // The dispatch wrapper clone pattern would clobber it anyway.
            // sync_edicts_to_server preserves ge.edicts modelindex via conditional copy.
        });

        // For inline models, call linkentity to compute size, absbox, PVS clusters.
        // This matches the original C PF_setmodel which calls SV_LinkEdict.
        if is_inline {
            self.linkentity(ent_idx);
        }
    }

    // ---- Collision ----

    fn trace(&self, start: &Vec3, mins: &Vec3, maxs: &Vec3, end: &Vec3, _passent: i32, contentmask: i32) -> Trace {
        // Delegate to cmodel collision
        // Full implementation would use SV_Trace from sv_world.rs
        let mut tr = myq2_common::cmodel::with_cmodel_ctx(|cctx| {
            let headnode = if cctx.numcmodels > 0 {
                cctx.map_cmodels[0].headnode
            } else {
                0
            };
            cctx.box_trace(start, end, mins, maxs, headnode, contentmask)
        }).unwrap_or_default();
        // In C Q2, SV_Trace always ensures trace.ent points to g_edicts[0] (world entity)
        // when no specific entity was hit. Match that behavior.
        if tr.ent_index < 0 {
            tr.ent_index = 0; // world entity
        }
        tr
    }

    fn pointcontents(&self, point: &Vec3) -> i32 {
        myq2_common::cmodel::cm_point_contents(point, 0)
    }

    fn lag_compensated_trace(
        &self,
        start: &Vec3,
        mins: &Vec3,
        maxs: &Vec3,
        end: &Vec3,
        passent: i32,
        contentmask: i32,
        attacker_idx: i32,
    ) -> Trace {
        with_ctx(|ctx| {
            // Check if lag compensation is enabled and we have valid attacker
            if !ctx.lag_compensation.enabled || attacker_idx < 0 {
                // Fall back to regular trace
                return self.trace(start, mins, maxs, end, passent, contentmask);
            }

            // Get attacker's ping from client
            let attacker_ping = if let Some(client_idx) = ctx.svs.clients.iter()
                .position(|c| c.edict_index == attacker_idx)
            {
                ctx.svs.clients[client_idx].ping
            } else {
                0
            };

            if attacker_ping == 0 {
                // No compensation needed for low-ping players
                return self.trace(start, mins, maxs, end, passent, contentmask);
            }

            // Calculate rewind time
            let server_time = ctx.sv.time as i32;
            let rewind_time = ctx.lag_compensation.calculate_rewind_time(server_time, attacker_ping);

            // For now, do the regular BSP trace first
            let mut result = myq2_common::cmodel::with_cmodel_ctx(|cctx| {
                let headnode = if cctx.numcmodels > 0 {
                    cctx.map_cmodels[0].headnode
                } else {
                    0
                };
                cctx.box_trace(start, end, mins, maxs, headnode, contentmask)
            }).unwrap_or_default();

            // Check for lag-compensated hits against entities
            // Test the trace line against all recorded entity positions at rewind_time
            if let Some(ref ge) = ctx.ge {
                for (entity_num, ent) in ge.edicts.iter().enumerate() {
                    if !ent.inuse || entity_num as i32 == passent {
                        continue;
                    }

                    // Skip non-solid entities
                    if ent.solid == crate::sv_game::Solid::Not {
                        continue;
                    }

                    // Test against historical position
                    let (hit, hit_point) = ctx.lag_compensation.test_hit(
                        entity_num as i32,
                        server_time,
                        attacker_ping,
                        start,
                        end,
                    );

                    if hit {
                        // Calculate fraction to hit point
                        let dist_to_hit = vector_length(&vector_subtract(&hit_point, start));
                        let total_dist = vector_length(&vector_subtract(end, start));
                        let fraction = if total_dist > 0.0 {
                            dist_to_hit / total_dist
                        } else {
                            0.0
                        };

                        // If this hit is closer than current result, use it
                        if fraction < result.fraction {
                            result.fraction = fraction;
                            result.endpos = hit_point;
                            result.ent_index = entity_num as i32;
                            result.allsolid = false;
                            result.startsolid = false;
                        }
                    }
                }
            }

            result
        })
    }

    fn in_pvs(&self, p1: &Vec3, p2: &Vec3) -> bool {
        with_ctx(|ctx| {
            pf_in_pvs(ctx, p1, p2)
        })
    }

    fn in_phs(&self, p1: &Vec3, p2: &Vec3) -> bool {
        with_ctx(|ctx| {
            pf_in_phs(ctx, p1, p2)
        })
    }

    fn set_area_portal_state(&self, portalnum: i32, open: bool) {
        myq2_common::cmodel::with_cmodel_ctx(|ctx| {
            ctx.set_area_portal_state(portalnum as usize, open);
        });
    }

    fn areas_connected(&self, area1: i32, area2: i32) -> bool {
        myq2_common::cmodel::with_cmodel_ctx(|ctx| {
            ctx.areas_connected(area1 as usize, area2 as usize)
        }).unwrap_or(false)
    }

    // ---- Entity linking ----

    fn linkentity(&self, ent_idx: i32) {
        with_ctx(|ctx| {
            if let Some(ref mut ge) = ctx.ge {
                if let Some(ent) = ge.edicts.get_mut(ent_idx as usize) {
                    // Copy current entity state from GAME_CONTEXT to ge so we use
                    // up-to-date origin/mins/maxs/solid for PVS computation.
                    // In original C Q2, these are shared memory; here we must sync explicitly.
                    //
                    // NOTE: We do NOT copy modelindex here — it's already set on ge.edicts
                    // by setmodel. Copying from GAME_CONTEXT would overwrite it with stale
                    // data (0) when called from dispatch wrapper contexts.
                    //
                    // SAFETY: GAME_CTX_PTR is valid during game code execution (single-threaded).
                    with_game_ctx_ptr(|ptr| {
                        let game_ctx = unsafe { &*ptr };
                        if let Some(game_ent) = game_ctx.edicts.get(ent_idx as usize) {
                            ent.s.origin = game_ent.s.origin;
                            ent.s.angles = game_ent.s.angles;
                            ent.mins = game_ent.mins;
                            ent.maxs = game_ent.maxs;
                            ent.solid = match game_ent.solid {
                                myq2_game::game::Solid::Not => crate::sv_game::Solid::Not,
                                myq2_game::game::Solid::Trigger => crate::sv_game::Solid::Trigger,
                                myq2_game::game::Solid::Bbox => crate::sv_game::Solid::Bbox,
                                myq2_game::game::Solid::Bsp => crate::sv_game::Solid::Bsp,
                            };
                            ent.svflags = game_ent.svflags;
                        }
                    });

                    // Compute size
                    for i in 0..3 {
                        ent.size[i] = ent.maxs[i] - ent.mins[i];
                    }
                    // Set abs box
                    for i in 0..3 {
                        ent.absmin[i] = ent.s.origin[i] + ent.mins[i] - 1.0;
                        ent.absmax[i] = ent.s.origin[i] + ent.maxs[i] + 1.0;
                    }
                    // Copy origin to old_origin on first link
                    if ent.linkcount == 0 {
                        ent.s.old_origin = ent.s.origin;
                    }
                    ent.linkcount += 1;

                    // Compute PVS cluster membership via the collision model
                    if ent.solid != crate::sv_game::Solid::Not || ent.s.modelindex != 0 {
                        let leafs = myq2_common::cmodel::cm_box_leafnums(&ent.absmin, &ent.absmax, 0);

                        if leafs.len() > crate::sv_game::MAX_ENT_CLUSTERS {
                            ent.num_clusters = -1;
                            ent.headnode = myq2_common::cmodel::cm_headnode_for_box(&ent.absmin, &ent.absmax);
                        } else {
                            ent.num_clusters = 0;
                            for &leaf in &leafs {
                                let cluster = myq2_common::cmodel::cm_leaf_cluster(leaf as usize);
                                let area = myq2_common::cmodel::cm_leaf_area(leaf as usize);

                                if area != 0 {
                                    if ent.areanum != 0 && ent.areanum != area {
                                        ent.areanum2 = area;
                                    } else {
                                        ent.areanum = area;
                                    }
                                }

                                if cluster != -1 && (ent.num_clusters as usize) < crate::sv_game::MAX_ENT_CLUSTERS {
                                    ent.clusternums[ent.num_clusters as usize] = cluster;
                                    ent.num_clusters += 1;
                                }
                            }
                        }
                    }

                    // Write computed fields back to GAME_CONTEXT so that game code
                    // (g_touch_triggers, g_phys, etc.) can read them. In original C Q2,
                    // the game DLL shares memory with the server, so these fields are
                    // automatically visible. In our Rust port, we must sync explicitly.
                    //
                    // Dispatch wrappers may overwrite these with stale clone data, but
                    // the next linkentity call will recompute them. The critical case is
                    // the player edict, which goes through client_think → gi_linkentity
                    // directly (not through dispatch), so the values stay current.
                    with_game_ctx_ptr(|ptr| {
                        let game_ctx = unsafe { &mut *ptr };
                        if let Some(game_ent) = game_ctx.edicts.get_mut(ent_idx as usize) {
                            game_ent.absmin = ent.absmin;
                            game_ent.absmax = ent.absmax;
                            game_ent.size = ent.size;
                            game_ent.linkcount = ent.linkcount;
                        }
                    });
                }
            }
        });
    }

    fn unlinkentity(&self, ent_idx: i32) {
        with_ctx(|ctx| {
            if let Some(ref mut ge) = ctx.ge {
                if let Some(ent) = ge.edicts.get_mut(ent_idx as usize) {
                    // Clear area link state (removes from area node chain)
                    ent.area_node = -1;
                    ent.area_linked = false;
                    ent.num_clusters = 0;
                    ent.areanum = 0;
                    ent.areanum2 = 0;
                }
            }
        });

        // Note: We do NOT write cleared fields back to GAME_CONTEXT.
        // sync_edicts_to_server preserves ge.edicts engine fields.
    }

    fn box_edicts(&self, mins: &Vec3, maxs: &Vec3, maxcount: i32, _areatype: i32) -> Vec<i32> {
        // Brute-force AABB overlap check against all in-use edicts.
        // The C version uses area-node spatial partitioning; this is functionally
        // equivalent but slower. Acceptable until the dual Edict types are unified.
        with_ctx(|ctx| {
            let mut result = Vec::new();
            if let Some(ref ge) = ctx.ge {
                for (i, ent) in ge.edicts.iter().enumerate() {
                    if !ent.inuse { continue; }
                    if result.len() >= maxcount as usize { break; }
                    // AABB overlap test
                    if ent.absmin[0] > maxs[0] || ent.absmin[1] > maxs[1] || ent.absmin[2] > maxs[2] { continue; }
                    if ent.absmax[0] < mins[0] || ent.absmax[1] < mins[1] || ent.absmax[2] < mins[2] { continue; }
                    result.push(i as i32);
                }
            }
            result
        })
    }

    // ---- Network messaging: write to sv.multicast ----

    fn multicast(&self, origin: &Vec3, to: i32) {
        with_ctx(|ctx| {
            let mc = match to {
                0 => Multicast::All,
                1 => Multicast::Phs,
                2 => Multicast::Pvs,
                3 => Multicast::AllR,
                4 => Multicast::PhsR,
                5 => Multicast::PvsR,
                _ => Multicast::All,
            };
            sv_multicast(ctx, Some(*origin), mc);
        });
    }

    fn unicast(&self, ent_idx: i32, reliable: bool) {
        with_ctx(|ctx| {
            let mut local_ent = Edict::default();
            local_ent.s.number = ent_idx;
            pf_unicast(ctx, &local_ent, reliable);
        });
    }

    fn write_char(&self, c: i32) {
        with_ctx(|ctx| { pf_write_char(ctx, c); });
    }

    fn write_byte(&self, c: i32) {
        with_ctx(|ctx| { pf_write_byte(ctx, c); });
    }

    fn write_short(&self, c: i32) {
        with_ctx(|ctx| { pf_write_short(ctx, c); });
    }

    fn write_long(&self, c: i32) {
        with_ctx(|ctx| { pf_write_long(ctx, c); });
    }

    fn write_float(&self, f: f32) {
        with_ctx(|ctx| { pf_write_float(ctx, f); });
    }

    fn write_string(&self, s: &str) {
        with_ctx(|ctx| { pf_write_string(ctx, s); });
    }

    fn write_position(&self, pos: &Vec3) {
        with_ctx(|ctx| { pf_write_pos(ctx, pos); });
    }

    fn write_dir(&self, dir: &Vec3) {
        with_ctx(|ctx| { pf_write_dir(ctx, dir); });
    }

    fn write_angle(&self, f: f32) {
        with_ctx(|ctx| { pf_write_angle(ctx, f); });
    }

    // ---- Memory: Rust handles this ----

    fn tag_malloc(&self, size: i32, _tag: i32) -> Vec<u8> {
        vec![0u8; size as usize]
    }

    fn tag_free(&self, _tag: i32) {}

    fn free_tags(&self, _tag: i32) {}

    // ---- Cvars ----

    fn cvar(&self, var_name: &str, value: &str, flags: i32) -> f32 {
        myq2_common::cvar::cvar_get(var_name, value, flags);
        myq2_common::cvar::cvar_variable_value(var_name)
    }

    fn cvar_set(&self, var_name: &str, value: &str) {
        myq2_common::cvar::cvar_set(var_name, value);
    }

    fn cvar_forceset(&self, var_name: &str, value: &str) {
        myq2_common::cvar::cvar_force_set(var_name, value);
    }

    // ---- Command args ----

    fn argc(&self) -> i32 {
        myq2_common::cmd::cmd_argc() as i32
    }

    fn argv(&self, n: i32) -> String {
        myq2_common::cmd::cmd_argv(n as usize)
    }

    fn args(&self) -> String {
        myq2_common::cmd::cmd_args()
    }

    // ---- Misc ----

    fn add_command_string(&self, text: &str) {
        myq2_common::cmd::cbuf_add_text(text);
    }

    fn debug_graph(&self, _value: f32, _color: i32) {
        // no-op
    }
}
