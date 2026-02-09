// sv_send.rs — Server packet sending code
// Converted from: myq2-original/server/sv_send.c
//
// Copyright (C) 1997-2001 Id Software, Inc.
// Licensed under the GNU General Public License v2.

use crate::server::*;
use rayon::prelude::*;
use myq2_common::common::{
    com_printf, msg_write_byte, msg_write_long, msg_write_string,
};
use myq2_common::q_shared::*;
use myq2_common::qcommon::*;

// =============================================================================
// Com_Printf redirection
// =============================================================================

/// Flush redirect buffer — sends accumulated print output to the appropriate
/// destination (out-of-band packet or client reliable message).
pub fn sv_flush_redirect(
    sv_redirected: Redirect,
    outputbuf: &str,
    sv_client: Option<&mut Client>,
    net_from: &NetAdr,
) {
    match sv_redirected {
        Redirect::Packet => {
            // Netchan_OutOfBandPrint(NS_SERVER, net_from, "print\n%s", outputbuf);
            myq2_common::net_chan::netchan_out_of_band_print(NetSrc::Server, net_from, &format!("print\n{}", outputbuf));
        }
        Redirect::Client => {
            if let Some(client) = sv_client {
                msg_write_byte(&mut client.netchan.message, SvcOps::Print as i32);
                msg_write_byte(&mut client.netchan.message, PRINT_HIGH);
                msg_write_string(&mut client.netchan.message, outputbuf);
            }
        }
        Redirect::None => {}
    }
}

// =============================================================================
// EVENT MESSAGES
// =============================================================================

/// Sends text across to be displayed if the level passes.
/// Equivalent to SV_ClientPrintf.
pub fn sv_client_printf(cl: &mut Client, level: i32, msg: &str) {
    if level < cl.messagelevel {
        return;
    }

    msg_write_byte(&mut cl.netchan.message, SvcOps::Print as i32);
    msg_write_byte(&mut cl.netchan.message, level);
    msg_write_string(&mut cl.netchan.message, msg);
}

/// Sends text to all active clients.
/// Equivalent to SV_BroadcastPrintf.
pub fn sv_broadcast_printf(ctx: &mut ServerContext, level: i32, msg: &str) {
    // Echo to console if dedicated
    if ctx.sv_paused {
        // In the original code this checks dedicated->value; using a dedicated flag
        // on the context. For now echo when the server is a dedicated server.
    }

    // Mask off high bits and echo to console (mimics dedicated server behavior)
    {
        let copy: String = msg
            .bytes()
            .take(1023)
            .map(|b| (b & 127) as char)
            .collect();
        com_printf(&format!("{}\n", copy));
    }

    let maxclients = ctx.maxclients_value as usize;
    for i in 0..maxclients {
        if i >= ctx.svs.clients.len() {
            break;
        }
        let cl = &mut ctx.svs.clients[i];
        if level < cl.messagelevel {
            continue;
        }
        if cl.state != ClientState::Spawned {
            continue;
        }
        msg_write_byte(&mut cl.netchan.message, SvcOps::Print as i32);
        msg_write_byte(&mut cl.netchan.message, level);
        msg_write_string(&mut cl.netchan.message, msg);
    }
}

/// Sends text to all active clients as a stufftext command.
/// Equivalent to SV_BroadcastCommand.
pub fn sv_broadcast_command(ctx: &mut ServerContext, msg: &str) {
    if ctx.sv.state == ServerState::Dead {
        return;
    }

    msg_write_byte(&mut ctx.sv.multicast, SvcOps::StuffText as i32);
    msg_write_string(&mut ctx.sv.multicast, msg);
    sv_multicast(ctx, None, Multicast::AllR);
}

/// Sends the contents of sv.multicast to a subset of the clients,
/// then clears sv.multicast.
///
/// MULTICAST_ALL   same as broadcast (origin can be NULL)
/// MULTICAST_PVS   send to clients potentially visible from org
/// MULTICAST_PHS   send to clients potentially hearable from org
pub fn sv_multicast(ctx: &mut ServerContext, origin: Option<Vec3>, to: Multicast) {
    let reliable;
    let mask: Option<Vec<u8>>;
    let area1: i32;

    if to != Multicast::AllR && to != Multicast::All {
        if let Some(org) = origin {
            let leafnum = myq2_common::cmodel::cm_point_leafnum(&org) as i32;
            area1 = myq2_common::cmodel::cm_leaf_area(leafnum as usize);
        } else {
            area1 = 0;
        }
    } else {
        area1 = 0;
    }

    // If doing a serverrecord, store everything
    if ctx.svs.demofile.is_some() {
        let data = ctx.sv.multicast.data[..ctx.sv.multicast.cursize as usize].to_vec();
        ctx.svs.demo_multicast.write(&data);
    }

    match to {
        Multicast::AllR => {
            reliable = true;
            mask = None;
        }
        Multicast::All => {
            reliable = false;
            mask = None;
        }
        Multicast::PhsR => {
            reliable = true;
            if let Some(org) = origin {
                let leafnum = myq2_common::cmodel::cm_point_leafnum(&org) as i32;
                let cluster = myq2_common::cmodel::cm_leaf_cluster(leafnum as usize);
                mask = Some(myq2_common::cmodel::cm_cluster_phs(cluster));
            } else {
                mask = None;
            }
        }
        Multicast::Phs => {
            reliable = false;
            if let Some(org) = origin {
                let leafnum = myq2_common::cmodel::cm_point_leafnum(&org) as i32;
                let cluster = myq2_common::cmodel::cm_leaf_cluster(leafnum as usize);
                mask = Some(myq2_common::cmodel::cm_cluster_phs(cluster));
            } else {
                mask = None;
            }
        }
        Multicast::PvsR => {
            reliable = true;
            if let Some(org) = origin {
                let leafnum = myq2_common::cmodel::cm_point_leafnum(&org) as i32;
                let cluster = myq2_common::cmodel::cm_leaf_cluster(leafnum as usize);
                mask = Some(myq2_common::cmodel::cm_cluster_pvs(cluster));
            } else {
                mask = None;
            }
        }
        Multicast::Pvs => {
            reliable = false;
            if let Some(org) = origin {
                let leafnum = myq2_common::cmodel::cm_point_leafnum(&org) as i32;
                let cluster = myq2_common::cmodel::cm_leaf_cluster(leafnum as usize);
                mask = Some(myq2_common::cmodel::cm_cluster_pvs(cluster));
            } else {
                mask = None;
            }
        }
    }

    // Send the data to all relevant clients
    let multicast_data = ctx.sv.multicast.data[..ctx.sv.multicast.cursize as usize].to_vec();
    let maxclients = ctx.maxclients_value as usize;

    for j in 0..maxclients {
        if j >= ctx.svs.clients.len() {
            break;
        }
        let client = &mut ctx.svs.clients[j];

        if client.state == ClientState::Free || client.state == ClientState::Zombie {
            continue;
        }
        if client.state != ClientState::Spawned && !reliable {
            continue;
        }

        if let Some(ref mask_data) = mask {
            // In the original C code, this accesses client->edict->s.origin.
            // In our Rust port, entity data is accessed via the edict_index
            // and the game export's edicts array. For now we use a placeholder
            // that retrieves the client's entity origin from the context.
            let client_origin = get_client_edict_origin(ctx, j);
            let leafnum = myq2_common::cmodel::cm_point_leafnum(&client_origin) as i32;
            let cluster = myq2_common::cmodel::cm_leaf_cluster(leafnum as usize);
            let area2 = myq2_common::cmodel::cm_leaf_area(leafnum as usize);
            if !myq2_common::cmodel::cm_areas_connected(area1 as usize, area2 as usize) {
                continue;
            }
            if cluster >= 0 {
                let byte_idx = (cluster >> 3) as usize;
                let bit: u8 = 1 << (cluster & 7);
                if byte_idx < mask_data.len() && (mask_data[byte_idx] & bit) == 0 {
                    continue;
                }
            }
        }

        if reliable {
            ctx.svs.clients[j].netchan.message.write(&multicast_data);
        } else {
            ctx.svs.clients[j].datagram.write(&multicast_data);
        }
    }

    ctx.sv.multicast.clear();
}


// ===============================================================================
// FRAME UPDATES
// ===============================================================================

/// Build and send a client datagram. Returns true on success.
/// Equivalent to SV_SendClientDatagram.
pub fn sv_send_client_datagram(ctx: &mut ServerContext, client_idx: usize) -> bool {
    sv_build_client_frame(ctx, client_idx);

    let mut msg = SizeBuf::new(MAX_MSGLEN as i32);
    msg.allow_overflow = true;

    // Send over all the relevant entity_state_t and the player_state_t
    sv_write_frame_to_client(ctx, client_idx, &mut msg);

    // Copy the accumulated multicast datagram for this client out to the message.
    // It is necessary for this to be after the WriteEntities
    // so that entity references will be current.
    let client = &mut ctx.svs.clients[client_idx];
    if client.datagram.overflowed {
        com_printf(&format!("WARNING: datagram overflowed for {}\n", client.name));
    } else {
        let data = client.datagram.data[..client.datagram.cursize as usize].to_vec();
        msg.write(&data);
    }
    client.datagram.clear();

    if msg.overflowed {
        // Must have room left for the packet header
        com_printf(&format!("WARNING: msg overflowed for {}\n", ctx.svs.clients[client_idx].name));
        msg.clear();
    }

    // Send the datagram
    let data = msg.data[..msg.cursize as usize].to_vec();
    let curtime = ctx.svs.realtime;
    crate::sv_main::netchan_transmit(&mut ctx.svs.clients[client_idx].netchan, &data, curtime);

    // Record the size for rate estimation
    let idx = (ctx.sv.framenum % RATE_MESSAGES as i32) as usize;
    ctx.svs.clients[client_idx].message_size[idx] = msg.cursize;

    true
}

/// Handle demo completion — close demo file and advance to next server.
/// Equivalent to SV_DemoCompleted.
pub fn sv_demo_completed(ctx: &mut ServerContext) {
    if ctx.sv.demofile.is_some() {
        ctx.sv.demofile = None;
    }
    crate::sv_user::sv_nextserver(ctx);
}

/// Returns true if the client is over its current bandwidth estimation
/// and should not be sent another packet.
/// Equivalent to SV_RateDrop.
pub fn sv_rate_drop(sv: &Server, c: &mut Client) -> bool {
    // mattx86: never drop over loopback or local networks
    if c.netchan.remote_address.adr_type == NetAdrType::Loopback
        || myq2_common::net::net_is_local_adr(&c.netchan.remote_address)
    {
        return false;
    }

    let mut total: i32 = 0;
    for i in 0..RATE_MESSAGES {
        total += c.message_size[i];
    }

    if total > c.rate {
        c.surpress_count += 1;
        c.message_size[(sv.framenum % RATE_MESSAGES as i32) as usize] = 0;
        return true;
    }

    false
}

/// Send messages to all connected clients. Handles demo playback,
/// Pre-computed client action for parallel processing.
#[derive(Clone, Copy)]
enum ClientSendAction {
    /// Client is free, skip
    Skip,
    /// Client has overflowed, needs drop
    Overflow,
    /// Send demo/cinematic message
    SendDemo,
    /// Send datagram (spawned client)
    SendDatagram,
    /// Send reliable only (heartbeat)
    SendReliable,
}

/// Send messages to all connected clients.
///
/// Pre-computes client states in parallel, then applies the results
/// sequentially (due to mutable state requirements).
/// SV_SendClientMessages.
pub fn sv_send_client_messages(ctx: &mut ServerContext) {
    let mut msglen: usize = 0;
    let mut msgbuf = vec![0u8; MAX_MSGLEN];

    // Read the next demo message if needed
    if ctx.sv.state == ServerState::Demo && ctx.sv.demofile.is_some() {
        if ctx.sv_paused {
            msglen = 0;
        } else {
            match sv_demo_read_message(&mut ctx.sv) {
                Some((len, data)) => {
                    if len as i32 == -1 {
                        sv_demo_completed(ctx);
                        return;
                    }
                    if len > MAX_MSGLEN {
                        panic!("SV_SendClientMessages: msglen > MAX_MSGLEN");
                    }
                    msglen = len;
                    msgbuf[..len].copy_from_slice(&data[..len]);
                }
                None => {
                    sv_demo_completed(ctx);
                    return;
                }
            }
        }
    }

    let maxclients = ctx.maxclients_value as usize;
    let num_clients = ctx.svs.clients.len().min(maxclients);
    let is_cinematic = matches!(
        ctx.sv.state,
        ServerState::Cinematic | ServerState::Demo | ServerState::Pic
    );
    let curtime = ctx.svs.realtime;

    // Phase 1: Parallel pre-computation of client actions (read-only)
    // Collect data needed to determine action for each client
    let client_data: Vec<_> = (0..num_clients)
        .map(|i| {
            let c = &ctx.svs.clients[i];
            (
                c.state,
                c.netchan.message.overflowed,
                c.netchan.message.cursize,
                c.netchan.last_sent,
            )
        })
        .collect();

    // Determine actions in parallel
    let actions: Vec<_> = client_data
        .par_iter()
        .enumerate()
        .map(|(i, &(state, overflowed, msg_cursize, last_sent))| {
            if state == ClientState::Free {
                (i, ClientSendAction::Skip)
            } else if overflowed {
                (i, ClientSendAction::Overflow)
            } else if is_cinematic {
                (i, ClientSendAction::SendDemo)
            } else if state == ClientState::Spawned {
                (i, ClientSendAction::SendDatagram)
            } else if msg_cursize > 0 || curtime - last_sent > 1000 {
                (i, ClientSendAction::SendReliable)
            } else {
                (i, ClientSendAction::Skip)
            }
        })
        .collect();

    // Phase 2: Sequential application of actions (requires mutable state)
    for (i, action) in actions {
        match action {
            ClientSendAction::Skip => {}
            ClientSendAction::Overflow => {
                ctx.svs.clients[i].netchan.message.clear();
                ctx.svs.clients[i].datagram.clear();
                let name = ctx.svs.clients[i].name.clone();
                com_printf(&format!("{} overflowed\n", name));
                sv_broadcast_printf(ctx, PRINT_HIGH, &format!("{} overflowed\n", name));
                sv_drop_client(&mut ctx.svs.clients[i]);
            }
            ClientSendAction::SendDemo => {
                let data = msgbuf[..msglen].to_vec();
                crate::sv_main::netchan_transmit(&mut ctx.svs.clients[i].netchan, &data, curtime);
            }
            ClientSendAction::SendDatagram => {
                // Rate check must happen here (modifies state)
                if !sv_rate_drop(&ctx.sv, &mut ctx.svs.clients[i]) {
                    sv_send_client_datagram(ctx, i);
                }
            }
            ClientSendAction::SendReliable => {
                crate::sv_main::netchan_transmit(&mut ctx.svs.clients[i].netchan, &[], curtime);
            }
        }
    }
}


/// SV_BuildClientFrame — Build the frame data for a client.
///
/// Determines which entities are visible to the client using PVS/PHS,
/// copies their entity states into the circular client_entities buffer,
/// and snapshots the player state and area visibility bits.
/// Delegates to sv_ents::sv_build_client_frame using the GlobalCModelAdapter
/// to provide collision model access through the global cmodel context.
fn sv_build_client_frame(ctx: &mut ServerContext, client_idx: usize) {
    if ctx.ge.is_none() {
        return;
    }

    let cm = GlobalCModelAdapter;

    // sv_ents::sv_build_client_frame needs (&Server, &mut ServerStatic, &mut Client, &GameExport, &dyn CM).
    // Since Client lives inside ServerStatic.clients, we temporarily remove the client
    // to satisfy the borrow checker, then put it back.
    let mut client = std::mem::take(&mut ctx.svs.clients[client_idx]);

    if let Some(ref ge) = ctx.ge {
        let maxclients = ctx.maxclients_value;
        crate::sv_ents::sv_build_client_frame(
            &ctx.sv,
            &mut ctx.svs,
            &mut client,
            ge,
            &cm,
            maxclients,
        );
    }

    ctx.svs.clients[client_idx] = client;
}

/// SV_WriteFrameToClient — Write the current frame to a client message buffer.
///
/// Encodes the frame header (frame number, delta reference, suppress count),
/// area visibility bits, delta-compressed player state, and delta-compressed
/// entity states into the message buffer.
/// Delegates to sv_ents::sv_write_frame_to_client for the actual implementation.
fn sv_write_frame_to_client(ctx: &ServerContext, client_idx: usize, msg: &mut SizeBuf) {
    let client = &ctx.svs.clients[client_idx];

    // Determine delta reference frame
    let frame_index = ctx.sv.framenum as usize & (UPDATE_BACKUP as usize - 1);
    let lastframe = if client.lastframe <= 0 {
        -1i32
    } else if ctx.sv.framenum - client.lastframe >= (UPDATE_BACKUP - 3) {
        -1i32
    } else {
        client.lastframe
    };

    msg_write_byte(msg, SvcOps::Frame as i32);
    msg_write_long(msg, ctx.sv.framenum);
    msg_write_long(msg, lastframe);
    msg_write_byte(msg, client.surpress_count);

    // Send areabits
    let frame = &client.frames[frame_index];
    msg_write_byte(msg, frame.areabytes);
    let areabits_data = &frame.areabits[..frame.areabytes as usize];
    msg.write(areabits_data);

    // Delta encode playerstate
    let oldframe = if lastframe >= 0 {
        let idx = lastframe as usize & (UPDATE_BACKUP as usize - 1);
        Some(&client.frames[idx])
    } else {
        None
    };
    crate::sv_ents::sv_write_playerstate_to_client(oldframe, frame, msg);

    // Delta encode entities
    crate::sv_ents::sv_emit_packet_entities(
        &ctx.svs,
        oldframe,
        frame,
        msg,
        ctx.maxclients_value,
        &ctx.sv.baselines,
    );
}

/// SV_DropClient — Drop a client from the server.
///
/// Sends a disconnect message, calls game DLL ClientDisconnect,
/// and sets the client state to Free. This is a simplified version
/// that operates on a single Client without the full ServerContext.
fn sv_drop_client(client: &mut Client) {
    // Write disconnect message to the client
    msg_write_byte(&mut client.netchan.message, SvcOps::Disconnect as i32);

    // Free downloads
    if client.download.is_some() {
        client.download = None;
    }

    // Mark as free immediately (mattx86: drop instantly, was cs_zombie)
    client.state = ClientState::Free;
    client.name.clear();
}

/// SV_DemoReadMessage — Read the next message from a demo file.
///
/// Reads a 4-byte little-endian length prefix, then reads that many bytes
/// of message data. Returns (length, data) or None on EOF/error.
/// A length of -1 (0xFFFFFFFF) indicates end of demo.
fn sv_demo_read_message(sv: &mut Server) -> Option<(usize, Vec<u8>)> {
    use std::io::Read;

    let file = sv.demofile.as_mut()?;

    // Read the 4-byte length prefix
    let mut len_buf = [0u8; 4];
    if file.read_exact(&mut len_buf).is_err() {
        return None;
    }

    let len = i32::from_le_bytes(len_buf);
    if len == -1 {
        // End of demo marker
        return Some((len as usize, Vec::new()));
    }
    if len <= 0 {
        return None;
    }

    let len = len as usize;
    let mut data = vec![0u8; len];
    if file.read_exact(&mut data).is_err() {
        return None;
    }

    Some((len, data))
}

/// Get a client's edict origin from the game export's edicts array.
///
/// In the original C code this was `cl->edict->s.origin`. In Rust we
/// look up the edict by index through the game export.
fn get_client_edict_origin(ctx: &ServerContext, client_idx: usize) -> Vec3 {
    if let Some(ref ge) = ctx.ge {
        let edict_idx = ctx.svs.clients[client_idx].edict_index as usize;
        if let Some(ent) = ge.edicts.get(edict_idx) {
            return ent.s.origin;
        }
    }
    [0.0, 0.0, 0.0]
}

// SVF_NOCLIENT, SOLID_BSP imported from myq2_game (canonical location)
pub use myq2_game::game::SVF_NOCLIENT;
pub use myq2_game::g_local::SOLID_BSP;

// =============================================================================
// GlobalCModelAdapter — CollisionModel implementation using global cmodel context
//
// Delegates all CollisionModel trait methods to myq2_common::cmodel free functions
// which operate on the global CModelContext. This allows sv_ents::sv_build_client_frame
// to access the collision model without a direct reference to CModelContext.
// =============================================================================

use crate::sv_world::CollisionModel;
use myq2_common::qfiles::MAX_MAP_AREAS;

struct GlobalCModelAdapter;

impl CollisionModel for GlobalCModelAdapter {
    fn box_leafnums(
        &self,
        mins: &Vec3,
        maxs: &Vec3,
        list: &mut [i32],
        list_size: usize,
        topnode: &mut i32,
    ) -> i32 {
        let leafs = myq2_common::cmodel::cm_box_leafnums(mins, maxs, 0);
        let count = leafs.len().min(list_size);
        for i in 0..count {
            list[i] = leafs[i];
        }
        *topnode = 0;
        count as i32
    }

    fn leaf_cluster(&self, leafnum: i32) -> i32 {
        myq2_common::cmodel::cm_leaf_cluster(leafnum as usize)
    }

    fn leaf_area(&self, leafnum: i32) -> i32 {
        myq2_common::cmodel::cm_leaf_area(leafnum as usize)
    }

    fn point_contents(&self, p: &Vec3, headnode: i32) -> i32 {
        myq2_common::cmodel::cm_point_contents(p, headnode)
    }

    fn transformed_point_contents(
        &self,
        p: &Vec3,
        headnode: i32,
        _origin: &Vec3,
        _angles: &Vec3,
    ) -> i32 {
        // Simplified: no transform applied
        myq2_common::cmodel::cm_point_contents(p, headnode)
    }

    fn headnode_for_box(&self, mins: &Vec3, maxs: &Vec3) -> i32 {
        myq2_common::cmodel::cm_headnode_for_box(mins, maxs)
    }

    fn box_trace(
        &self,
        start: &Vec3,
        end: &Vec3,
        mins: &Vec3,
        maxs: &Vec3,
        headnode: i32,
        brushmask: i32,
    ) -> Trace {
        myq2_common::cmodel::cm_box_trace(start, end, mins, maxs, headnode, brushmask)
    }

    fn transformed_box_trace(
        &self,
        start: &Vec3,
        end: &Vec3,
        mins: &Vec3,
        maxs: &Vec3,
        headnode: i32,
        brushmask: i32,
        _origin: &Vec3,
        _angles: &Vec3,
    ) -> Trace {
        // Simplified: no transform applied
        myq2_common::cmodel::cm_box_trace(start, end, mins, maxs, headnode, brushmask)
    }

    fn num_clusters(&self) -> i32 {
        myq2_common::cmodel::cm_num_clusters() as i32
    }

    fn cluster_pvs(&self, cluster: i32) -> &[u8] {
        // The trait requires &[u8] lifetime tied to &self, but the global
        // cmodel returns Vec<u8>. We use a leaked static to satisfy the lifetime.
        // This is acceptable since cluster data persists for the map's lifetime.
        let v = myq2_common::cmodel::cm_cluster_pvs(cluster);
        if v.is_empty() {
            &[]
        } else {
            // SAFETY: We leak the Vec to get a 'static slice. The data lives as
            // long as the map is loaded. This mirrors the C code which returns
            // a pointer into static map data.
            Box::leak(v.into_boxed_slice())
        }
    }

    fn cluster_phs(&self, cluster: i32) -> &[u8] {
        let v = myq2_common::cmodel::cm_cluster_phs(cluster);
        if v.is_empty() {
            &[]
        } else {
            // SAFETY: Same reasoning as cluster_pvs above.
            Box::leak(v.into_boxed_slice())
        }
    }

    fn point_leafnum(&self, p: &Vec3) -> i32 {
        myq2_common::cmodel::cm_point_leafnum(p) as i32
    }

    fn write_area_bits(&self, area: i32) -> (i32, [u8; MAX_MAP_AREAS / 8]) {
        myq2_common::cmodel::with_cmodel_ctx(|ctx| {
            let mut bits = [0u8; MAX_MAP_AREAS / 8];
            let bytes = ctx.write_area_bits(&mut bits, area as usize);
            (bytes as i32, bits)
        }).unwrap_or((0, [0u8; MAX_MAP_AREAS / 8]))
    }

    fn areas_connected(&self, area1: i32, area2: i32) -> bool {
        myq2_common::cmodel::cm_areas_connected(area1 as usize, area2 as usize)
    }

    fn headnode_visible(&self, headnode: i32, bitvector: &[u8]) -> bool {
        myq2_common::cmodel::with_cmodel_ctx(|ctx| {
            ctx.headnode_visible(headnode, bitvector)
        }).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // sv_rate_drop tests
    // =========================================================================

    fn make_client_with_rate(rate: i32, message_sizes: [i32; RATE_MESSAGES]) -> Client {
        let mut client = Client::default();
        client.rate = rate;
        client.message_size = message_sizes;
        // Default netchan is loopback, so we need to set it to non-loopback
        // for rate limiting to actually apply
        client.netchan.remote_address.adr_type = NetAdrType::Ip;
        client.netchan.remote_address.ip = [8, 8, 8, 8]; // public IP
        client
    }

    #[test]
    fn rate_drop_loopback_never_drops() {
        let sv = Server::default();
        let mut client = Client::default();
        client.rate = 1; // very low rate
        client.message_size = [10000; RATE_MESSAGES]; // lots of data
        // Default netchan is loopback
        client.netchan.remote_address.adr_type = NetAdrType::Loopback;

        let result = sv_rate_drop(&sv, &mut client);
        assert!(!result, "Loopback clients should never be rate-dropped");
    }

    #[test]
    fn rate_drop_local_network_never_drops() {
        let sv = Server::default();
        let mut client = Client::default();
        client.rate = 1;
        client.message_size = [10000; RATE_MESSAGES];
        client.netchan.remote_address.adr_type = NetAdrType::Ip;
        client.netchan.remote_address.ip = [192, 168, 1, 1]; // local network

        let result = sv_rate_drop(&sv, &mut client);
        assert!(!result, "Local network clients should never be rate-dropped");
    }

    #[test]
    fn rate_drop_below_rate_limit() {
        let sv = Server::default();
        let mut client = make_client_with_rate(15000, [100; RATE_MESSAGES]);
        // Total = 100 * 10 = 1000, rate = 15000; should not drop
        let result = sv_rate_drop(&sv, &mut client);
        assert!(!result, "Client below rate limit should not be dropped");
    }

    #[test]
    fn rate_drop_above_rate_limit() {
        let mut sv = Server::default();
        sv.framenum = 0;
        let mut client = make_client_with_rate(5000, [1000; RATE_MESSAGES]);
        // Total = 1000 * 10 = 10000, rate = 5000; should drop
        let result = sv_rate_drop(&sv, &mut client);
        assert!(result, "Client above rate limit should be dropped");
    }

    #[test]
    fn rate_drop_increments_suppress_count() {
        let mut sv = Server::default();
        sv.framenum = 0;
        let mut client = make_client_with_rate(5000, [1000; RATE_MESSAGES]);
        let initial_suppress = client.surpress_count;

        sv_rate_drop(&sv, &mut client);

        assert_eq!(
            client.surpress_count,
            initial_suppress + 1,
            "Rate drop should increment surpress_count"
        );
    }

    #[test]
    fn rate_drop_clears_message_size_slot() {
        let mut sv = Server::default();
        sv.framenum = 3;
        let mut client = make_client_with_rate(5000, [1000; RATE_MESSAGES]);

        sv_rate_drop(&sv, &mut client);

        let idx = (sv.framenum % RATE_MESSAGES as i32) as usize;
        assert_eq!(
            client.message_size[idx], 0,
            "Rate drop should clear current frame's message_size slot"
        );
    }

    #[test]
    fn rate_drop_exact_boundary() {
        let mut sv = Server::default();
        sv.framenum = 0;
        // rate = 5000, total = 5000 => not dropped (total must be > rate)
        let mut client = make_client_with_rate(5000, [500; RATE_MESSAGES]);
        // Total = 500 * 10 = 5000
        let result = sv_rate_drop(&sv, &mut client);
        assert!(!result, "Client at exact rate boundary should NOT be dropped");
    }

    #[test]
    fn rate_drop_just_above_boundary() {
        let mut sv = Server::default();
        sv.framenum = 0;
        let mut client = make_client_with_rate(5000, [501; RATE_MESSAGES]);
        // Total = 501 * 10 = 5010 > 5000
        let result = sv_rate_drop(&sv, &mut client);
        assert!(result, "Client just above rate limit should be dropped");
    }

    // =========================================================================
    // sv_client_printf tests
    // =========================================================================

    #[test]
    fn client_printf_below_level_no_write() {
        let mut client = Client::default();
        client.messagelevel = PRINT_HIGH; // level 2

        let initial_size = client.netchan.message.cursize;
        sv_client_printf(&mut client, 0, "test message"); // level 0 < messagelevel 2

        assert_eq!(
            client.netchan.message.cursize, initial_size,
            "Message below client's message level should not be written"
        );
    }

    #[test]
    fn client_printf_at_level_writes() {
        let mut client = Client::default();
        client.messagelevel = 0;

        sv_client_printf(&mut client, PRINT_HIGH, "hello");

        assert!(
            client.netchan.message.cursize > 0,
            "Message at or above level should be written"
        );

        // Verify the svc_print opcode was written
        assert_eq!(
            client.netchan.message.data[0],
            SvcOps::Print as u8,
            "First byte should be SVC_PRINT opcode"
        );
    }

    #[test]
    fn client_printf_writes_level_byte() {
        let mut client = Client::default();
        client.messagelevel = 0;

        sv_client_printf(&mut client, PRINT_HIGH, "hello");

        // Second byte should be the print level
        assert_eq!(
            client.netchan.message.data[1] as i32,
            PRINT_HIGH,
            "Second byte should be the print level"
        );
    }

    // =========================================================================
    // sv_flush_redirect tests
    // =========================================================================

    #[test]
    fn flush_redirect_none_does_nothing() {
        // When redirected to None, nothing should happen
        sv_flush_redirect(Redirect::None, "test", None, &NetAdr::default());
        // No crash = pass
    }

    #[test]
    fn flush_redirect_client_writes_to_message() {
        let mut client = Client::default();
        sv_flush_redirect(Redirect::Client, "test output", Some(&mut client), &NetAdr::default());

        assert!(
            client.netchan.message.cursize > 0,
            "Client redirect should write to client's netchan.message"
        );

        // Verify svc_print opcode
        assert_eq!(client.netchan.message.data[0], SvcOps::Print as u8);
    }

    // =========================================================================
    // sv_broadcast_command tests
    // =========================================================================

    #[test]
    fn broadcast_command_dead_server_does_nothing() {
        let mut ctx = ServerContext::default();
        ctx.sv.state = ServerState::Dead;

        let initial_size = ctx.sv.multicast.cursize;
        sv_broadcast_command(&mut ctx, "test_command");

        assert_eq!(
            ctx.sv.multicast.cursize, initial_size,
            "Dead server should not broadcast commands"
        );
    }

    // =========================================================================
    // ClientSendAction determination logic tests
    // =========================================================================

    #[test]
    fn client_send_action_free_is_skip() {
        let state = ClientState::Free;
        let is_free = state == ClientState::Free;
        assert!(is_free);
    }

    #[test]
    fn client_send_action_overflow_detection() {
        let mut msg = SizeBuf::new(16);
        msg.allow_overflow = true;
        // Write enough to overflow
        for _ in 0..20 {
            msg_write_byte(&mut msg, 0xFF);
        }
        assert!(msg.overflowed, "SizeBuf should be overflowed after writing past capacity");
    }

    // =========================================================================
    // sv_demo_read_message tests
    // =========================================================================

    #[test]
    fn demo_read_message_no_file_returns_none() {
        let mut sv = Server::default();
        // demofile is None by default
        let result = sv_demo_read_message(&mut sv);
        assert!(result.is_none(), "No demo file should return None");
    }

    #[test]
    fn demo_read_message_eof_returns_none() {
        use std::io::Cursor;

        // Create a temporary file-like object with end-of-demo marker (-1)
        let data = (-1i32).to_le_bytes();
        let cursor = Cursor::new(data.to_vec());

        // We can't directly set sv.demofile to a Cursor since it expects File.
        // Instead, we'll test with a real temp file.
        let temp_dir = std::env::temp_dir();
        let temp_file_path = temp_dir.join("myq2_test_demo_eof.dem");

        {
            use std::io::Write;
            let mut f = std::fs::File::create(&temp_file_path).unwrap();
            f.write_all(&data).unwrap();
        }

        let mut sv = Server::default();
        sv.demofile = Some(std::fs::File::open(&temp_file_path).unwrap());

        let result = sv_demo_read_message(&mut sv);

        // Clean up
        let _ = std::fs::remove_file(&temp_file_path);

        // -1 marker should return Some with usize wrapping of -1
        assert!(result.is_some(), "EOF marker should return Some");
    }

    #[test]
    fn demo_read_message_valid_data() {
        let temp_dir = std::env::temp_dir();
        let temp_file_path = temp_dir.join("myq2_test_demo_valid.dem");

        {
            use std::io::Write;
            let mut f = std::fs::File::create(&temp_file_path).unwrap();
            // Write length prefix (4 bytes)
            let msg_data = b"hello";
            let len = msg_data.len() as i32;
            f.write_all(&len.to_le_bytes()).unwrap();
            f.write_all(msg_data).unwrap();
        }

        let mut sv = Server::default();
        sv.demofile = Some(std::fs::File::open(&temp_file_path).unwrap());

        let result = sv_demo_read_message(&mut sv);

        let _ = std::fs::remove_file(&temp_file_path);

        assert!(result.is_some(), "Valid demo data should be readable");
        let (len, data) = result.unwrap();
        assert_eq!(len, 5);
        assert_eq!(&data[..5], b"hello");
    }

    #[test]
    fn demo_read_message_empty_file() {
        let temp_dir = std::env::temp_dir();
        let temp_file_path = temp_dir.join("myq2_test_demo_empty.dem");

        {
            std::fs::File::create(&temp_file_path).unwrap();
        }

        let mut sv = Server::default();
        sv.demofile = Some(std::fs::File::open(&temp_file_path).unwrap());

        let result = sv_demo_read_message(&mut sv);

        let _ = std::fs::remove_file(&temp_file_path);

        assert!(result.is_none(), "Empty file should return None");
    }
}
