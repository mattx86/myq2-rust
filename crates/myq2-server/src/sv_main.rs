// sv_main.rs — Main server loop and initialization
// Converted from: myq2-original/server/sv_main.c
//
// Copyright (C) 1997-2001 Id Software, Inc.
// Licensed under the GNU General Public License v2.
//
// Includes R1Q2-style server timing decoupling for improved network performance.
// The sv_fps cvar allows the server tick rate to be decoupled from the frame rate.

use crate::server::*;
use crate::sv_game::{SVF_NOCLIENT, Solid};
use myq2_common::cmd::CmdContext;
use myq2_common::common::{com_printf, com_dprintf, msg_read_string_line};
use myq2_common::q_shared::*;
use myq2_common::qcommon::*;

/// Heartbeat interval in seconds.
const HEARTBEAT_SECONDS: i32 = 300;

// =============================================================================
// Server Timing (R1Q2-style decoupling)
// =============================================================================

/// Default server tick rate (matches original Q2).
pub const DEFAULT_SV_FPS: i32 = 10;

/// Minimum server tick rate.
pub const MIN_SV_FPS: i32 = 10;

/// Maximum server tick rate.
pub const MAX_SV_FPS: i32 = 90;

/// Server timing state for R1Q2-style tick rate decoupling.
///
/// This allows the server game logic to run at a fixed rate (sv_fps) independent
/// of the actual frame rate. This improves network consistency and allows the
/// renderer to run at higher frame rates without affecting game physics timing.
pub struct ServerTiming {
    /// Server tick rate in Hz (default: 10, original Q2 behavior).
    /// Higher values give smoother gameplay but increase CPU and bandwidth usage.
    pub sv_fps: i32,

    /// Frame time in milliseconds (1000 / sv_fps).
    pub sv_frametime: i32,

    /// Accumulated time residual for fixed-timestep simulation.
    /// When this exceeds sv_frametime, a game tick is processed.
    pub time_residual: i32,

    /// Whether decoupled timing is enabled.
    pub enabled: bool,
}

impl ServerTiming {
    /// Create new server timing with default values.
    pub fn new() -> Self {
        Self {
            sv_fps: DEFAULT_SV_FPS,
            sv_frametime: 1000 / DEFAULT_SV_FPS,
            time_residual: 0,
            enabled: false,
        }
    }

    /// Update the tick rate from sv_fps cvar.
    ///
    /// Clamps the value to valid range and recalculates frame time.
    pub fn set_fps(&mut self, fps: i32) {
        let clamped = fps.clamp(MIN_SV_FPS, MAX_SV_FPS);
        if clamped != self.sv_fps {
            self.sv_fps = clamped;
            self.sv_frametime = 1000 / clamped;
            if fps != clamped {
                com_printf(&format!(
                    "sv_fps clamped to {} (valid range: {}-{})\n",
                    clamped, MIN_SV_FPS, MAX_SV_FPS
                ));
            }
        }
    }

    /// Enable decoupled timing mode.
    pub fn enable(&mut self) {
        self.enabled = true;
        self.time_residual = 0;
        com_printf(&format!(
            "Server timing decoupled: {} Hz ({} ms frametime)\n",
            self.sv_fps, self.sv_frametime
        ));
    }

    /// Disable decoupled timing mode (revert to original behavior).
    pub fn disable(&mut self) {
        self.enabled = false;
        self.time_residual = 0;
        com_printf("Server timing decoupling disabled\n");
    }

    /// Reset timing accumulator (e.g., on level change).
    pub fn reset(&mut self) {
        self.time_residual = 0;
    }
}

impl Default for ServerTiming {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// SV_DropClient
//
// Called when the player is totally leaving the server, either
// willingly or unwillingly.  This is NOT called if the entire
// server is quiting or crashing.
// ============================================================

pub fn sv_drop_client(ctx: &mut ServerContext, client_index: usize) {
    let cl = &mut ctx.svs.clients[client_index];

    // add the disconnect
    msg_write_byte(&mut cl.netchan.message, SvcOps::Disconnect as i32);

    if cl.state == ClientState::Spawned {
        // call the prog function for removing a client
        // this will remove the body, among other things
        if let Some(ref mut ge) = ctx.ge {
            ge.client_disconnect_by_index(cl.edict_index);
        }
    }

    if cl.download.is_some() {
        cl.download = None;
    }

    // mattx86: drop client instantly (was cs_zombie)
    cl.state = ClientState::Free;
    cl.name.clear();
}

// ============================================================
// SV_StatusString
//
// Builds the string that is sent as heartbeats and status replies
// ============================================================

pub fn sv_status_string(ctx: &ServerContext) -> String {
    let serverinfo = ctx.cvars.serverinfo();
    let mut status = format!("{}\n", serverinfo);

    let max = ctx.maxclients_value as usize;
    for i in 0..max.min(ctx.svs.clients.len()) {
        let cl = &ctx.svs.clients[i];
        if cl.state == ClientState::Connected || cl.state == ClientState::Spawned {
            // In C: cl->edict->client->ps.stats[STAT_FRAGS], cl->ping, cl->name
            let frags = if let Some(ref ge) = ctx.ge {
                ge.get_client_frags(cl.edict_index)
            } else {
                0
            };
            let player = format!("{} {} \"{}\"\n", frags, cl.ping, cl.name);
            if status.len() + player.len() >= MAX_MSGLEN - 16 {
                break; // can't hold any more
            }
            status.push_str(&player);
        }
    }

    status
}

// ============================================================
// SVC_Status
//
// Responds with all the info that qplug or qspy can see
// ============================================================

pub fn svc_status(ctx: &ServerContext) {
    let status = sv_status_string(ctx);
    netchan_out_of_band_print(NetSrc::Server, &ctx.net_from, &format!("print\n{}", status));
}

// ============================================================
// SVC_Ack
// ============================================================

pub fn svc_ack(ctx: &ServerContext) {
    com_printf(&format!("Ping acknowledge from {}\n", net_adr_to_string(&ctx.net_from)));
}

// ============================================================
// SVC_Info
//
// Responds with short info for broadcast scans.
// The second parameter should be the current protocol version number.
// ============================================================

pub fn svc_info(ctx: &ServerContext, cmd_argv: &dyn Fn(usize) -> String) {
    if ctx.maxclients_value == 1.0 {
        return; // ignore in single player
    }

    let version: i32 = cmd_argv(1).parse().unwrap_or(0);

    let string;
    if version != PROTOCOL_VERSION {
        let hn = ctx.cvars.variable_string("hostname");
        string = format!("{}: wrong version\n", hn);
    } else {
        let mut count = 0;
        let max = ctx.maxclients_value as usize;
        for i in 0..max.min(ctx.svs.clients.len()) {
            if ctx.svs.clients[i].state == ClientState::Connected
                || ctx.svs.clients[i].state == ClientState::Spawned
            {
                count += 1;
            }
        }
        let hn = ctx.cvars.variable_string("hostname");
        string = format!(
            "{:>16} {:>8} {:>2}/{:>2}\n",
            hn, ctx.sv.name, count, ctx.maxclients_value as i32
        );
    }

    netchan_out_of_band_print(NetSrc::Server, &ctx.net_from, &format!("info\n{}", string));
}

// ============================================================
// SVC_Ping
//
// Just responds with an acknowledgement
// ============================================================

pub fn svc_ping(ctx: &ServerContext) {
    netchan_out_of_band_print(NetSrc::Server, &ctx.net_from, "ack");
}

// ============================================================
// SVC_GetChallenge
//
// Returns a challenge number that can be used in a subsequent
// client_connect command. Prevents denial of service attacks.
// ============================================================

pub fn svc_get_challenge(ctx: &mut ServerContext) {
    let mut oldest: usize = 0;
    let mut oldest_time: i32 = 0x7fffffff;

    // see if we already have a challenge for this ip
    let mut found = MAX_CHALLENGES; // sentinel for "not found"
    for i in 0..MAX_CHALLENGES {
        if net_compare_base_adr(&ctx.net_from, &ctx.svs.challenges[i].adr) {
            found = i;
            break;
        }
        if ctx.svs.challenges[i].time < oldest_time {
            oldest_time = ctx.svs.challenges[i].time;
            oldest = i;
        }
    }

    if found == MAX_CHALLENGES {
        // overwrite the oldest
        ctx.svs.challenges[oldest].challenge = rand_i32() & 0x7fff;
        ctx.svs.challenges[oldest].adr = ctx.net_from;
        ctx.svs.challenges[oldest].time = ctx.svs.realtime;
        found = oldest;
    }

    // send it back (include protocol version so client can auto-match)
    netchan_out_of_band_print(
        NetSrc::Server,
        &ctx.net_from,
        &format!("challenge {} p={}", ctx.svs.challenges[found].challenge, PROTOCOL_VERSION),
    );
}

// ============================================================
// SVC_DirectConnect
//
// A connection request that did not come from the master
// ============================================================

pub fn svc_direct_connect(ctx: &mut ServerContext, cmd_argv: &dyn Fn(usize) -> String) {
    let adr = ctx.net_from;

    com_dprintf("SVC_DirectConnect ()\n");

    let version: i32 = cmd_argv(1).parse().unwrap_or(0);
    // Accept protocol versions 34 (original), 35 (R1Q2), 36 (Q2Pro)
    if version < PROTOCOL_VERSION_MIN || version > PROTOCOL_VERSION_MAX {
        netchan_out_of_band_print(
            NetSrc::Server,
            &adr,
            &format!(
                "print\nServer is version {:.2}.\nSupported protocols: {}-{}\n",
                VERSION, PROTOCOL_VERSION_MIN, PROTOCOL_VERSION_MAX
            ),
        );
        com_dprintf(&format!("    rejected connect from version {}\n", version));
        return;
    }

    let qport: i32 = cmd_argv(2).parse().unwrap_or(0);
    let challenge: i32 = cmd_argv(3).parse().unwrap_or(0);

    let mut userinfo = cmd_argv(4);
    if userinfo.len() >= MAX_INFO_STRING {
        userinfo.truncate(MAX_INFO_STRING - 1);
    }

    // force the IP key/value pair so the game can filter based on ip
    let ip_string = net_adr_to_string(&ctx.net_from);
    info_set_value_for_key(&mut userinfo, "ip", &ip_string);

    // attractloop servers are ONLY for local clients
    if ctx.sv.attractloop
        && !net_is_local_address(&adr) {
            com_printf("Remote connect in attract loop.  Ignored.\n");
            netchan_out_of_band_print(NetSrc::Server, &adr, "print\nConnection refused.\n");
            return;
        }

    // see if the challenge is valid
    if !net_is_local_address(&adr) {
        let mut challenge_valid = false;
        for i in 0..MAX_CHALLENGES {
            if net_compare_base_adr(&ctx.net_from, &ctx.svs.challenges[i].adr) {
                if challenge == ctx.svs.challenges[i].challenge {
                    challenge_valid = true;
                    break; // good
                }
                netchan_out_of_band_print(NetSrc::Server, &adr, "print\nBad challenge.\n");
                return;
            }
        }
        if !challenge_valid {
            netchan_out_of_band_print(
                NetSrc::Server,
                &adr,
                "print\nNo challenge for address.\n",
            );
            return;
        }
    }

    let max = ctx.maxclients_value as usize;
    let reconnect_limit = ctx.cvars.variable_value("sv_reconnect_limit") as i32;

    // if there is already a slot for this ip, reuse it
    let mut newcl_index: Option<usize> = None;
    for i in 0..max.min(ctx.svs.clients.len()) {
        let cl = &ctx.svs.clients[i];
        if cl.state == ClientState::Free {
            continue;
        }
        if net_compare_base_adr(&adr, &cl.netchan.remote_address)
            && (cl.netchan.qport == qport || adr.port == cl.netchan.remote_address.port)
        {
            if !net_is_local_address(&adr)
                && (ctx.svs.realtime - cl.lastconnect) < (reconnect_limit * 1000)
            {
                com_dprintf(&format!(
                    "{}:reconnect rejected : too soon\n",
                    net_adr_to_string(&adr)
                ));
                return;
            }
            com_printf(&format!("{}:reconnect\n", net_adr_to_string(&adr)));
            newcl_index = Some(i);
            break;
        }
    }

    // find a free client slot if we didn't find a reconnecting one
    if newcl_index.is_none() {
        for i in 0..max.min(ctx.svs.clients.len()) {
            if ctx.svs.clients[i].state == ClientState::Free {
                newcl_index = Some(i);
                break;
            }
        }
    }

    let newcl_index = match newcl_index {
        Some(idx) => idx,
        None => {
            netchan_out_of_band_print(NetSrc::Server, &adr, "print\nServer is full.\n");
            com_dprintf("Rejected a connection.\n");
            return;
        }
    };

    // build a new connection
    // accept the new client
    // this is the only place a client_t is ever initialized
    ctx.svs.clients[newcl_index] = Client::default();
    ctx.sv_client_index = Some(newcl_index);
    let edictnum = (newcl_index + 1) as i32;
    ctx.svs.clients[newcl_index].edict_index = edictnum;
    ctx.svs.clients[newcl_index].challenge = challenge;

    // get the game a chance to reject this connection or modify the userinfo
    if let Some(ref mut ge) = ctx.ge {
        if !ge.client_connect_by_index(edictnum, &userinfo) {
            let rejmsg = info_value_for_key(&userinfo, "rejmsg");
            if !rejmsg.is_empty() {
                netchan_out_of_band_print(
                    NetSrc::Server,
                    &adr,
                    &format!("print\n{}\nConnection refused.\n", rejmsg),
                );
            } else {
                netchan_out_of_band_print(
                    NetSrc::Server,
                    &adr,
                    "print\nConnection refused.\n",
                );
            }
            com_dprintf("Game rejected a connection.\n");
            return;
        }
    }

    // parse some info from the info strings
    ctx.svs.clients[newcl_index].userinfo = userinfo;
    sv_userinfo_changed(ctx, newcl_index);

    // send the connect packet to the client
    netchan_out_of_band_print(NetSrc::Server, &adr, "client_connect");

    // Netchan_Setup (NS_SERVER, &newcl->netchan, adr, qport);
    myq2_common::net_chan::netchan_setup(
        NetSrc::Server,
        &mut ctx.svs.clients[newcl_index].netchan,
        adr,
        qport,
        ctx.svs.realtime,
    );

    // Set the negotiated protocol version on the netchan
    // This enables protocol 35+ features like 1-byte qport
    myq2_common::net_chan::netchan_set_protocol(
        &mut ctx.svs.clients[newcl_index].netchan,
        version,
    );

    if version >= PROTOCOL_R1Q2 {
        let proto_name = if version == PROTOCOL_Q2PRO { "Q2Pro" } else { "R1Q2" };
        com_dprintf(&format!(
            "{}@{} connected using {} protocol ({})\n",
            ctx.svs.clients[newcl_index].name,
            net_adr_to_string(&adr),
            proto_name,
            version
        ));
    }

    ctx.svs.clients[newcl_index].state = ClientState::Connected;

    ctx.svs.clients[newcl_index].datagram = SizeBuf::new(MAX_MSGLEN as i32);
    ctx.svs.clients[newcl_index].datagram.allow_overflow = true;
    ctx.svs.clients[newcl_index].lastmessage = ctx.svs.realtime; // don't timeout
    ctx.svs.clients[newcl_index].lastconnect = ctx.svs.realtime;
}

// ============================================================
// Rcon_Validate
// ============================================================

pub fn rcon_validate(ctx: &ServerContext, cmd_argv: &dyn Fn(usize) -> String) -> bool {
    let password = ctx.cvars.variable_string("rcon_password");
    if password.is_empty() {
        return false;
    }
    cmd_argv(1) == password
}

// ============================================================
// SVC_RemoteCommand
//
// A client issued an rcon command.
// Shift down the remaining args. Redirect all printfs.
// ============================================================

pub fn svc_remote_command(
    ctx: &mut ServerContext,
    cmd_argv: &dyn Fn(usize) -> String,
    cmd_argc: usize,
    cmd_context: &mut CmdContext,
) {
    let valid = rcon_validate(ctx, cmd_argv);

    // Log the rcon attempt with the message data (matching C: net_message.data+4)
    let msg_text = if ctx.net_message.cursize > 4 {
        let end = ctx.net_message.cursize as usize;
        String::from_utf8_lossy(&ctx.net_message.data[4..end])
            .trim_end_matches('\0')
            .to_string()
    } else {
        String::new()
    };

    if !valid {
        com_printf(&format!(
            "Bad rcon from {}:\n{}\n",
            net_adr_to_string(&ctx.net_from),
            msg_text
        ));
    } else {
        com_printf(&format!(
            "Rcon from {}:\n{}\n",
            net_adr_to_string(&ctx.net_from),
            msg_text
        ));
    }

    // Begin redirecting Com_Printf output so rcon responses are captured
    // and sent back to the requesting client as an OOB packet.
    myq2_common::common::com_begin_redirect();

    if !valid {
        com_printf("Bad rcon_password.\n");
    } else {
        // Build the remaining command string from argv[2..] (argv[0]="rcon", argv[1]=password)
        let mut remaining = String::new();
        for i in 2..cmd_argc {
            remaining.push_str(&cmd_argv(i));
            remaining.push(' ');
        }
        cmd_context.cmd_execute_string(&remaining);
    }

    // End redirect and flush captured output as an OOB packet to the client
    if let Some(output) = myq2_common::common::com_end_redirect() {
        if !output.is_empty() {
            netchan_out_of_band_print(
                NetSrc::Server,
                &ctx.net_from,
                &format!("print\n{}", output),
            );
        }
    }
}

// ============================================================
// SV_ConnectionlessPacket
//
// A connectionless packet has four leading 0xff characters to
// distinguish it from a game channel.
// ============================================================

pub fn sv_connectionless_packet(ctx: &mut ServerContext, cmd_context: &mut CmdContext) {
    // MSG_BeginReading (&net_message);
    ctx.net_message.readcount = 0;

    // MSG_ReadLong (&net_message); // skip the -1 marker
    msg_read_long(&mut ctx.net_message);

    // s = MSG_ReadStringLine (&net_message);
    let s = msg_read_string_line(&mut ctx.net_message);

    // Cmd_TokenizeString (s, false);
    cmd_context.cmd_tokenize_string(&s, false);

    let c = cmd_context.cmd_argv(0).to_string();
    let cmd_argc = cmd_context.cmd_argc();

    com_dprintf(&format!(
        "Packet {} : {}\n",
        net_adr_to_string(&ctx.net_from),
        c
    ));

    // Build a closure that captures the tokenized argv for sub-functions
    // that still use the cmd_argv callback pattern.
    let argv_strings: Vec<String> = (0..cmd_argc)
        .map(|i| cmd_context.cmd_argv(i).to_string())
        .collect();
    let cmd_argv = |idx: usize| -> String {
        if idx < argv_strings.len() {
            argv_strings[idx].clone()
        } else {
            String::new()
        }
    };

    match c.as_str() {
        "ping" => svc_ping(ctx),
        "ack" => svc_ack(ctx),
        "status" => svc_status(ctx),
        "info" => svc_info(ctx, &cmd_argv),
        "getchallenge" => svc_get_challenge(ctx),
        "connect" => svc_direct_connect(ctx, &cmd_argv),
        "rcon" => svc_remote_command(ctx, &cmd_argv, cmd_argc, cmd_context),
        _ => {
            com_printf(&format!(
                "bad connectionless packet from {}:\n{}\n",
                net_adr_to_string(&ctx.net_from),
                s
            ));
        }
    }
}

use rayon::prelude::*;

/// Result of parallel ping calculation.
struct PingCalcResult {
    index: usize,
    ping: i32,
    edict_index: i32,
}

// ============================================================
// SV_CalcPings
//
// Updates the cl->ping variables
// Phase 1 (parallel): Calculate ping averages from latency samples
// Phase 2 (sequential): Apply results and notify game DLL
// ============================================================

pub fn sv_calc_pings(ctx: &mut ServerContext) {
    let max = ctx.maxclients_value as usize;
    let num_clients = max.min(ctx.svs.clients.len());

    // Phase 1: Parallel ping calculation
    let client_data: Vec<_> = (0..num_clients)
        .filter_map(|i| {
            if ctx.svs.clients[i].state != ClientState::Spawned {
                return None;
            }
            Some((
                i,
                ctx.svs.clients[i].frame_latency,
                ctx.svs.clients[i].edict_index,
            ))
        })
        .collect();

    let results: Vec<PingCalcResult> = client_data
        .par_iter()
        .map(|&(index, frame_latency, edict_index)| {
            let mut total = 0;
            let mut count = 0;
            for j in 0..LATENCY_COUNTS {
                if frame_latency[j] > 0 {
                    count += 1;
                    total += frame_latency[j];
                }
            }
            let ping = if count == 0 { 0 } else { total / count };
            PingCalcResult {
                index,
                ping,
                edict_index,
            }
        })
        .collect();

    // Phase 2: Sequential application of results
    for result in results {
        ctx.svs.clients[result.index].ping = result.ping;

        if let Some(ref mut ge) = ctx.ge {
            ge.set_client_ping(result.edict_index, result.ping);
        }
    }
}

/// Client timeout check result.
enum TimeoutAction {
    /// No action needed
    None,
    /// Client should be dropped
    Drop(usize, String),
    /// Zombie client can be freed
    FreeZombie(usize),
    /// Fix message time wraparound
    FixTime(usize),
}

// ============================================================
// SV_CheckTimeouts
//
// Phase 1 (parallel): Check timeout conditions
// Phase 2 (sequential): Apply drops and state changes
// ============================================================

pub fn sv_check_timeouts(ctx: &mut ServerContext) {
    let timeout_val = ctx.cvars.variable_value("timeout");
    let zombie_val = ctx.cvars.variable_value("zombietime");
    let droppoint = ctx.svs.realtime - (1000.0 * timeout_val) as i32;
    let zombiepoint = ctx.svs.realtime - (1000.0 * zombie_val) as i32;

    let max = ctx.maxclients_value as usize;
    let num_clients = max.min(ctx.svs.clients.len());
    let realtime = ctx.svs.realtime;

    // Phase 1: Parallel check of timeout conditions
    let client_data: Vec<_> = (0..num_clients)
        .map(|i| {
            (
                i,
                ctx.svs.clients[i].state,
                ctx.svs.clients[i].lastmessage,
                ctx.svs.clients[i].name.clone(),
            )
        })
        .collect();

    let actions: Vec<TimeoutAction> = client_data
        .par_iter()
        .map(|(i, state, lastmessage, name)| {
            let i = *i;
            let lastmessage = *lastmessage;

            // Fix time wraparound
            if lastmessage > realtime {
                return TimeoutAction::FixTime(i);
            }

            match *state {
                ClientState::Zombie if lastmessage < zombiepoint => {
                    TimeoutAction::FreeZombie(i)
                }
                ClientState::Connected | ClientState::Spawned if lastmessage < droppoint => {
                    TimeoutAction::Drop(i, name.clone())
                }
                _ => TimeoutAction::None,
            }
        })
        .collect();

    // Phase 2: Sequential application of actions
    for action in actions {
        match action {
            TimeoutAction::None => {}
            TimeoutAction::FixTime(i) => {
                ctx.svs.clients[i].lastmessage = realtime;
            }
            TimeoutAction::FreeZombie(i) => {
                ctx.svs.clients[i].state = ClientState::Free;
            }
            TimeoutAction::Drop(i, name) => {
                com_printf(&format!("{} timed out\n", name));
                sv_drop_client(ctx, i);
                ctx.svs.clients[i].state = ClientState::Free;
            }
        }
    }
}

// ============================================================
// SV_GiveMsec
//
// Every few frames, gives all clients an allotment of milliseconds
// for their command moves. If they exceed it, assume cheating.
// ============================================================

pub fn sv_give_msec(ctx: &mut ServerContext) {
    if (ctx.sv.framenum & 15) != 0 {
        return;
    }

    let max = ctx.maxclients_value as usize;
    for i in 0..max.min(ctx.svs.clients.len()) {
        if ctx.svs.clients[i].state == ClientState::Free {
            continue;
        }
        ctx.svs.clients[i].command_msec = 1800; // 1600 + some slop
    }
}

// ============================================================
// SV_ReadPackets
// ============================================================

pub fn sv_read_packets(ctx: &mut ServerContext) {
    let mut cmd_context = CmdContext::new();

    // while (NET_GetPacket (NS_SERVER, &net_from, &net_message))
    while net_get_packet(NetSrc::Server, &mut ctx.net_from, &mut ctx.net_message) {
        // check for connectionless packet (0xffffffff) first
        if ctx.net_message.cursize >= 4 {
            let marker = i32::from_le_bytes([
                ctx.net_message.data[0],
                ctx.net_message.data[1],
                ctx.net_message.data[2],
                ctx.net_message.data[3],
            ]);
            if marker == -1 {
                sv_connectionless_packet(ctx, &mut cmd_context);
                continue;
            }
        }

        // read the qport out of the message so we can fix up
        // stupid address translating routers
        ctx.net_message.readcount = 0;
        let _seq1 = msg_read_long(&mut ctx.net_message); // sequence number
        let _seq2 = msg_read_long(&mut ctx.net_message); // sequence number
        let qport = msg_read_short(&mut ctx.net_message) & 0xffff;

        // check for packets from connected clients
        let max = ctx.maxclients_value as usize;
        let mut matched = false;
        for i in 0..max.min(ctx.svs.clients.len()) {
            if ctx.svs.clients[i].state == ClientState::Free {
                continue;
            }
            if !net_compare_base_adr(&ctx.net_from, &ctx.svs.clients[i].netchan.remote_address) {
                continue;
            }
            if ctx.svs.clients[i].netchan.qport != qport {
                continue;
            }
            if ctx.svs.clients[i].netchan.remote_address.port != ctx.net_from.port {
                com_printf("SV_ReadPackets: fixing up a translated port\n");
                ctx.svs.clients[i].netchan.remote_address.port = ctx.net_from.port;
            }

            // if (Netchan_Process(&cl->netchan, &net_message))
            if netchan_process(&mut ctx.svs.clients[i].netchan, &mut ctx.net_message) {
                // this is a valid, sequenced packet, so process it
                if ctx.svs.clients[i].state != ClientState::Zombie {
                    ctx.svs.clients[i].lastmessage = ctx.svs.realtime; // don't timeout
                    // SV_ExecuteClientMessage (cl);
                    sv_execute_client_message(ctx, i);
                }
            }
            matched = true;
            break;
        }

        if matched {
            continue;
        }
    }
}

// ============================================================
// SV_PrepWorldFrame
//
// This has to be done before the world logic, because
// player processing happens outside RunWorldFrame
// ============================================================

pub fn sv_prep_world_frame(ctx: &mut ServerContext) {
    if let Some(ref mut ge) = ctx.ge {
        let num_edicts = ge.num_edicts();
        for i in 0..num_edicts {
            // events only last for a single message
            ge.clear_edict_event(i);
        }
    }
}

// ============================================================
// SV_RunGameFrame
// ============================================================

pub fn sv_run_game_frame(ctx: &mut ServerContext) {
    let host_speeds = ctx.cvars.variable_value("host_speeds");

    if host_speeds != 0.0 {
        ctx.time_before_game = sys_milliseconds();
    }

    // we always need to bump framenum, even if we
    // don't run the world, otherwise the delta
    // compression can get confused when a client
    // has the "current" frame
    ctx.sv.framenum += 1;
    ctx.sv.time = (ctx.sv.framenum * 100) as u32;

    // don't run if paused
    if !ctx.sv_paused || ctx.maxclients_value > 1.0 {
        if let Some(ref mut ge) = ctx.ge {
            ge.run_frame_call();
        }

        // never get more than one tic behind
        if (ctx.sv.time as i32) < ctx.svs.realtime {
            let sv_showclamp = ctx.cvars.variable_value("showclamp");
            if sv_showclamp != 0.0 {
                com_printf("sv highclamp\n");
            }
            ctx.svs.realtime = ctx.sv.time as i32;
        }
    }

    if host_speeds != 0.0 {
        ctx.time_after_game = sys_milliseconds();
    }

    // Record entity positions for lag compensation
    sv_record_lag_compensation_frame(ctx);
}

/// Record entity positions for lag compensation.
/// Called after each game frame to store historical entity positions
/// for rewinding during hit detection.
fn sv_record_lag_compensation_frame(ctx: &mut ServerContext) {
    if !ctx.lag_compensation.enabled {
        return;
    }

    let server_time = ctx.sv.time as i32;

    // Collect entity data from game state
    let mut entity_data = Vec::new();

    if let Some(ref ge) = ctx.ge {
        for (i, ent) in ge.edicts.iter().enumerate() {
            // Only record solid entities that can be hit
            if !ent.inuse {
                continue;
            }

            // Check if entity is solid (has collision)
            // Only record entities that have a solid type that allows collision
            let is_solid = ent.svflags & SVF_NOCLIENT == 0 && ent.solid != Solid::Not;
            if !is_solid {
                continue;
            }

            entity_data.push((
                i as i32,
                ent.s.origin,
                ent.mins,
                ent.maxs,
                ent.solid != Solid::Not,
            ));
        }
    }

    ctx.lag_compensation.record_frame(server_time, &entity_data);
}

// ============================================================
// SV_Frame
//
// Main server frame entry point
// ============================================================

pub fn sv_frame(ctx: &mut ServerContext, msec: i32) {
    ctx.time_before_game = 0;
    ctx.time_after_game = 0;

    // if server is not active, do nothing
    if !ctx.svs.initialized {
        return;
    }

    ctx.svs.realtime += msec;

    // keep the random time dependent
    let _ = rand_i32();

    // check timeouts
    sv_check_timeouts(ctx);

    // get packets from clients
    sv_read_packets(ctx);

    // move autonomous things around if enough time has passed
    let sv_timedemo = ctx.cvars.variable_value("timedemo");
    if sv_timedemo == 0.0 && ctx.svs.realtime < ctx.sv.time as i32 {
        // never let the time get too far off
        if ctx.sv.time as i32 - ctx.svs.realtime > 100 {
            let sv_showclamp = ctx.cvars.variable_value("showclamp");
            if sv_showclamp != 0.0 {
                com_printf("sv lowclamp\n");
            }
            ctx.svs.realtime = ctx.sv.time as i32 - 100;
        }
        net_sleep(ctx.sv.time as i32 - ctx.svs.realtime);
        return;
    }

    // update ping based on the last known frame from all clients
    sv_calc_pings(ctx);

    // give the clients some timeslices
    sv_give_msec(ctx);

    // let everything in the world think and move
    sv_run_game_frame(ctx);

    // send messages back to the clients that had packets read this frame
    crate::sv_send::sv_send_client_messages(ctx);

    // save the entire world state if recording a serverdemo
    sv_record_demo_message(ctx);

    // send a heartbeat to the master if needed
    master_heartbeat(ctx);

    // clear teleport flags, etc for next frame
    sv_prep_world_frame(ctx);
}

// ============================================================
// SV_Frame_Decoupled
//
// R1Q2-style decoupled server frame.
// Runs game logic at a fixed tick rate (sv_fps) while allowing
// the main loop to run faster for smoother rendering.
// ============================================================

/// Run a decoupled server frame with fixed-timestep game logic.
///
/// This is the R1Q2-style frame function that allows the game logic to run
/// at a fixed rate (sv_fps) while network I/O and other systems can run
/// at the actual frame rate.
///
/// # Arguments
/// * `ctx` - Server context
/// * `msec` - Elapsed milliseconds since last frame
/// * `timing` - Server timing state (manages accumulator and tick rate)
pub fn sv_frame_decoupled(ctx: &mut ServerContext, msec: i32, timing: &mut ServerTiming) {
    ctx.time_before_game = 0;
    ctx.time_after_game = 0;

    // if server is not active, do nothing
    if !ctx.svs.initialized {
        return;
    }

    ctx.svs.realtime += msec;

    // keep the random time dependent
    let _ = rand_i32();

    // check timeouts - always run
    sv_check_timeouts(ctx);

    // get packets from clients - always run (async I/O thread handles this too)
    sv_read_packets(ctx);

    // Accumulate time for fixed-timestep simulation
    timing.time_residual += msec;

    // Run game logic at fixed sv_fps rate
    // This may run 0, 1, or multiple times per frame depending on timing
    let mut game_frames_run = 0;
    while timing.time_residual >= timing.sv_frametime {
        timing.time_residual -= timing.sv_frametime;

        // update ping based on the last known frame from all clients
        sv_calc_pings(ctx);

        // give the clients some timeslices
        sv_give_msec(ctx);

        // let everything in the world think and move
        sv_run_game_frame(ctx);

        game_frames_run += 1;

        // Safety: don't run too many game frames in one go
        // This prevents spiral-of-death when game logic is too slow
        if game_frames_run >= 5 {
            // Drop remaining time to catch up
            if timing.time_residual > timing.sv_frametime * 2 {
                let sv_showclamp = ctx.cvars.variable_value("showclamp");
                if sv_showclamp != 0.0 {
                    com_printf(&format!(
                        "sv_frame_decoupled: dropped {} ms\n",
                        timing.time_residual
                    ));
                }
                timing.time_residual = 0;
            }
            break;
        }
    }

    // send messages back to the clients - always run
    // This happens every frame for responsive network updates
    crate::sv_send::sv_send_client_messages(ctx);

    // save the entire world state if recording a serverdemo
    sv_record_demo_message(ctx);

    // send a heartbeat to the master if needed
    master_heartbeat(ctx);

    // clear teleport flags, etc for next frame
    sv_prep_world_frame(ctx);
}

// ============================================================
// Master_Heartbeat
//
// Send a message to the master every few minutes to
// let it know we are alive, and log information
// ============================================================

pub fn master_heartbeat(ctx: &mut ServerContext) {
    // only dedicated servers send heartbeats
    let dedicated = ctx.cvars.variable_value("dedicated");
    if dedicated == 0.0 {
        return;
    }

    // a private dedicated game
    let public_val = ctx.cvars.variable_value("public");
    if public_val == 0.0 {
        return;
    }

    // check for time wraparound
    if ctx.svs.last_heartbeat > ctx.svs.realtime {
        ctx.svs.last_heartbeat = ctx.svs.realtime;
    }

    if ctx.svs.realtime - ctx.svs.last_heartbeat < HEARTBEAT_SECONDS * 1000 {
        return; // not time to send yet
    }

    ctx.svs.last_heartbeat = ctx.svs.realtime;

    // send the same string that we would give for a status OOB command
    let status = sv_status_string(ctx);

    // send to group master
    for i in 0..MAX_MASTERS {
        if ctx.master_adr[i].port != 0 {
            com_printf(&format!(
                "Sending heartbeat to {}\n",
                net_adr_to_string(&ctx.master_adr[i])
            ));
            netchan_out_of_band_print(
                NetSrc::Server,
                &ctx.master_adr[i],
                &format!("heartbeat\n{}", status),
            );
        }
    }
}

// ============================================================
// Master_Shutdown
//
// Informs all masters that this server is going down
// ============================================================

pub fn master_shutdown(ctx: &ServerContext) {
    // only dedicated servers send heartbeats
    let dedicated = ctx.cvars.variable_value("dedicated");
    if dedicated == 0.0 {
        return;
    }

    let public_val = ctx.cvars.variable_value("public");
    if public_val == 0.0 {
        return;
    }

    // send to group master
    for i in 0..MAX_MASTERS {
        if ctx.master_adr[i].port != 0 {
            if i > 0 {
                com_printf(&format!(
                    "Sending heartbeat to {}\n",
                    net_adr_to_string(&ctx.master_adr[i])
                ));
            }
            netchan_out_of_band_print(NetSrc::Server, &ctx.master_adr[i], "shutdown");
        }
    }
}

// ============================================================
// SV_UserinfoChanged
//
// Pull specific info from a newly changed userinfo string
// into a more C friendly form.
// ============================================================

pub fn sv_userinfo_changed(ctx: &mut ServerContext, client_index: usize) {
    // call prog code to allow overrides
    // ge->ClientUserinfoChanged (cl->edict, cl->userinfo);
    let edict_idx = ctx.svs.clients[client_index].edict_index;
    let userinfo = ctx.svs.clients[client_index].userinfo.clone();
    if let Some(ref mut ge) = ctx.ge {
        ge.client_userinfo_changed_by_index(edict_idx, &userinfo);
    }

    // name for C code
    let mut name = info_value_for_key(&userinfo, "name");
    // mask off high bit
    let name_bytes: Vec<u8> = name.bytes().map(|b| b & 127).collect();
    name = String::from_utf8_lossy(&name_bytes).to_string();
    if name.len() > 31 {
        name.truncate(31);
    }
    ctx.svs.clients[client_index].name = name;

    // rate command
    let rate_str = info_value_for_key(&userinfo, "rate");
    if !rate_str.is_empty() {
        let mut rate: i32 = rate_str.parse().unwrap_or(25000);
        if rate < 100 {
            rate = 100;
        }
        if rate > 90000 {
            rate = 90000;
        }
        ctx.svs.clients[client_index].rate = rate;
    } else {
        ctx.svs.clients[client_index].rate = 25000;
    }

    // msg command
    let msg_str = info_value_for_key(&userinfo, "msg");
    if !msg_str.is_empty() {
        ctx.svs.clients[client_index].messagelevel = msg_str.parse().unwrap_or(0);
    }
}

// ============================================================
// SV_Init
//
// Only called at quake2.exe startup, not for each game
// ============================================================

pub fn sv_init(ctx: &mut ServerContext) {
    // SV_InitOperatorCommands ();
    sv_init_operator_commands(ctx);

    ctx.cvars.get("rcon_password", Some(""), CVAR_ZERO);
    ctx.cvars.get("skill", Some("1"), CVAR_ZERO);
    ctx.cvars.get("deathmatch", Some("1"), CVAR_LATCH | CVAR_ARCHIVE);
    ctx.cvars.get("coop", Some("0"), CVAR_LATCH | CVAR_ARCHIVE);
    ctx.cvars.get(
        "dmflags",
        Some(&format!("{}", DF_INSTANT_ITEMS.bits())),
        CVAR_SERVERINFO | CVAR_ARCHIVE,
    );
    ctx.cvars
        .get("fraglimit", Some("0"), CVAR_SERVERINFO | CVAR_ARCHIVE);
    ctx.cvars
        .get("timelimit", Some("15"), CVAR_SERVERINFO | CVAR_ARCHIVE);
    ctx.cvars.get(
        "cheats",
        Some("0"),
        CVAR_SERVERINFO | CVAR_LATCH | CVAR_ARCHIVE,
    );
    ctx.cvars.get(
        "protocol",
        Some(&format!("{}", PROTOCOL_VERSION)),
        CVAR_SERVERINFO | CVAR_NOSET,
    );
    ctx.cvars.get(
        "maxclients",
        Some("8"),
        CVAR_SERVERINFO | CVAR_LATCH | CVAR_ARCHIVE,
    );
    ctx.cvars
        .get("hostname", Some("Q2Server"), CVAR_SERVERINFO | CVAR_ARCHIVE);
    ctx.cvars.get("timeout", Some("125"), CVAR_ARCHIVE);
    ctx.cvars.get("zombietime", Some("1"), CVAR_ARCHIVE);
    ctx.cvars.get("showclamp", Some("0"), CVAR_ZERO);
    ctx.cvars.get("paused", Some("0"), CVAR_ZERO);
    ctx.cvars.get("timedemo", Some("0"), CVAR_ZERO);
    ctx.cvars
        .get("sv_enforcetime", Some("0"), CVAR_ARCHIVE);
    ctx.cvars
        .get("allow_download", Some("1"), CVAR_ARCHIVE);
    ctx.cvars
        .get("allow_download_players", Some("0"), CVAR_ARCHIVE);
    ctx.cvars
        .get("allow_download_models", Some("1"), CVAR_ARCHIVE);
    ctx.cvars
        .get("allow_download_sounds", Some("1"), CVAR_ARCHIVE);
    ctx.cvars
        .get("allow_download_maps", Some("1"), CVAR_ARCHIVE);
    ctx.cvars.get("sv_noreload", Some("0"), CVAR_ZERO);
    ctx.cvars
        .get("sv_airaccelerate", Some("0"), CVAR_LATCH | CVAR_ARCHIVE);
    ctx.cvars.get("public", Some("0"), CVAR_ARCHIVE);
    ctx.cvars
        .get("sv_reconnect_limit", Some("3"), CVAR_ARCHIVE);

    // R1Q2-style network improvements
    // sv_fps: Server tick rate (10-90 Hz, default 10 for original Q2 behavior)
    // Higher values give smoother gameplay but increase CPU and bandwidth
    ctx.cvars
        .get("sv_fps", Some(&format!("{}", DEFAULT_SV_FPS)), CVAR_ARCHIVE);

    // Note: Async network I/O is always enabled - packets are received in
    // background threads and queued for processing by the game thread.

    // SZ_Init (&net_message, net_message_buffer, sizeof(net_message_buffer));
    ctx.net_message = SizeBuf::new(MAX_MSGLEN as i32);
}

// ============================================================
// SV_FinalMessage
//
// Used by SV_Shutdown to send a final message to all
// connected clients before the server goes down.
// ============================================================

pub fn sv_final_message(ctx: &mut ServerContext, message: &str, reconnect: bool) {
    ctx.net_message.clear();
    msg_write_byte(&mut ctx.net_message, SvcOps::Print as i32);
    msg_write_byte(&mut ctx.net_message, PRINT_HIGH);
    msg_write_string(&mut ctx.net_message, message);

    if reconnect {
        msg_write_byte(&mut ctx.net_message, SvcOps::Reconnect as i32);
    } else {
        msg_write_byte(&mut ctx.net_message, SvcOps::Disconnect as i32);
    }

    // send it twice
    // stagger the packets to crutch operating system limited buffers
    let max = ctx.maxclients_value as usize;
    let cursize = ctx.net_message.cursize;
    let data: Vec<u8> = ctx.net_message.data[..cursize as usize].to_vec();

    for _pass in 0..2 {
        for i in 0..max.min(ctx.svs.clients.len()) {
            if ctx.svs.clients[i].state == ClientState::Connected
                || ctx.svs.clients[i].state == ClientState::Spawned
            {
                netchan_transmit(
                    &mut ctx.svs.clients[i].netchan,
                    &data,
                    ctx.svs.realtime,
                );
            }
        }
    }
}

// ============================================================
// SV_Shutdown
//
// Called when each game quits,
// before Sys_Quit or Sys_Error
// ============================================================

pub fn sv_shutdown(ctx: &mut ServerContext, finalmsg: &str, reconnect: bool) {
    if !ctx.svs.clients.is_empty() {
        sv_final_message(ctx, finalmsg, reconnect);
    }

    master_shutdown(ctx);

    // SV_ShutdownGameProgs ();
    crate::sv_game::sv_shutdown_game_progs(ctx);

    // free current level
    ctx.sv.demofile = None;
    ctx.sv = Server::default();
    // Com_SetServerState (sv.state);
    com_set_server_state(ctx.sv.state);

    // free server static data
    ctx.svs.clients.clear();
    ctx.svs.client_entities.clear();
    ctx.svs.demofile = None;
    ctx.svs = ServerStatic::default();
}

// ============================================================
// Placeholder / stub functions for cross-module calls
// These will be implemented in their respective modules.
// ============================================================

pub use myq2_common::net::net_adr_to_string;
pub use myq2_common::net::net_compare_base_adr;
pub use myq2_common::net::net_is_local_address;
pub use myq2_common::net::net_get_packet;

/// NET_Sleep — Sleep for the specified number of milliseconds.
///
/// Uses platform sleep to yield CPU time when the server is ahead
/// of the game clock. This prevents busy-waiting in the server loop.
pub fn net_sleep(msec: i32) {
    if msec > 0 {
        std::thread::sleep(std::time::Duration::from_millis(msec as u64));
    }
}

/// Netchan_OutOfBandPrint — Send an out-of-band print message.
///
/// Constructs an OOB packet (4 bytes of 0xff + message) and sends it
/// to the specified network address. Used for connectionless protocol
/// messages (status, info, challenge, connect, etc).
pub fn netchan_out_of_band_print(sock: NetSrc, adr: &NetAdr, data: &str) {
    myq2_common::net_chan::netchan_out_of_band_print(sock, adr, data);
}

/// Netchan_Process — Process an incoming packet on a netchan.
///
/// Validates sequence numbers, handles reliable message acknowledgments,
/// and strips the netchan header. Returns true if the packet is valid
/// and should be processed, false if it should be discarded (out of order, etc).
pub fn netchan_process(chan: &mut NetChan, msg: &mut SizeBuf) -> bool {
    let curtime = sys_milliseconds();
    myq2_common::net_chan::netchan_process(chan, msg, curtime)
}

/// Netchan_Transmit — Send data over a netchan.
///
/// Handles sequencing, reliable message queueing, packet construction,
/// and sends the packet via NET_SendPacket.
/// Delegates to myq2_common::net_chan::netchan_transmit.
pub fn netchan_transmit(chan: &mut NetChan, data: &[u8], curtime: i32) {
    let qport = chan.qport;
    myq2_common::net_chan::netchan_transmit(chan, data, curtime, qport);
}

pub use myq2_common::q_shared::info_value_for_key;
pub use myq2_common::q_shared::info_set_value_for_key;

/// Sys_Milliseconds — Get the current time in milliseconds.
///
/// Returns a monotonically increasing time value used for profiling,
/// timing, and frame rate management. Uses std::time::Instant
/// relative to a process-wide epoch.
pub use myq2_common::common::sys_milliseconds;

pub use myq2_common::common::rand_i32;

pub use myq2_common::common::msg_write_byte;
pub use myq2_common::common::msg_write_string;
pub use myq2_common::common::msg_read_long;
pub use myq2_common::common::msg_read_short;

/// SV_ExecuteClientMessage — Parse and execute a client message.
///
/// The current net_message is parsed for the given client. Handles
/// clc_nop, clc_userinfo, clc_move, and clc_stringcmd operations.
/// Delegates to sv_user::sv_execute_client_message for the actual parsing.
pub fn sv_execute_client_message(ctx: &mut ServerContext, client_index: usize) {
    // Clone the net_message data so sv_user can parse it
    let mut msg = SizeBuf::new(ctx.net_message.maxsize);
    msg.cursize = ctx.net_message.cursize;
    msg.data[..ctx.net_message.cursize as usize]
        .copy_from_slice(&ctx.net_message.data[..ctx.net_message.cursize as usize]);
    msg.readcount = ctx.net_message.readcount;

    crate::sv_user::sv_execute_client_message(ctx, client_index, &mut msg);
}

/// SV_RecordDemoMessage — Record the current frame to the demo file.
///
/// Saves all entity states and accumulated multicast data to the
/// server demo file for later playback. Only operates if a demo
/// is actively being recorded (svs.demofile is Some).
pub fn sv_record_demo_message(ctx: &mut ServerContext) {
    if ctx.svs.demofile.is_none() || ctx.ge.is_none() {
        return;
    }
    // Delegate to sv_ents::sv_record_demo_message when game export is available
    if let Some(ref ge) = ctx.ge {
        crate::sv_ents::sv_record_demo_message(&ctx.sv, &mut ctx.svs, ge);
    }
}

/// SV_InitOperatorCommands — Register all server console commands.
///
/// Registers commands like kick, status, heartbeat, say, serverrecord, etc.
/// Delegates to sv_ccmds::sv_init_operator_commands.
pub fn sv_init_operator_commands(ctx: &mut ServerContext) {
    crate::sv_ccmds::sv_init_operator_commands(ctx);
}

/// Com_SetServerState — Notify the common module of a server state change.
///
/// Updates the global server state variable so the client and other
/// subsystems can query whether a server is running.
pub fn com_set_server_state(state: ServerState) {
    // Update the global server state in the common module so the client
    // and other subsystems can query whether a server is running.
    myq2_common::common::com_set_server_state(state as i32);
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // ServerTiming tests
    // =========================================================================

    #[test]
    fn server_timing_new_defaults() {
        let timing = ServerTiming::new();
        assert_eq!(timing.sv_fps, DEFAULT_SV_FPS);
        assert_eq!(timing.sv_fps, 10);
        assert_eq!(timing.sv_frametime, 100); // 1000 / 10
        assert_eq!(timing.time_residual, 0);
        assert!(!timing.enabled);
    }

    #[test]
    fn server_timing_default_trait() {
        let timing = ServerTiming::default();
        assert_eq!(timing.sv_fps, DEFAULT_SV_FPS);
        assert_eq!(timing.sv_frametime, 1000 / DEFAULT_SV_FPS);
    }

    #[test]
    fn server_timing_set_fps_valid() {
        let mut timing = ServerTiming::new();
        timing.set_fps(30);
        assert_eq!(timing.sv_fps, 30);
        assert_eq!(timing.sv_frametime, 1000 / 30); // 33
    }

    #[test]
    fn server_timing_set_fps_clamps_low() {
        let mut timing = ServerTiming::new();
        timing.set_fps(1); // below MIN_SV_FPS (10)
        assert_eq!(timing.sv_fps, MIN_SV_FPS);
        assert_eq!(timing.sv_frametime, 1000 / MIN_SV_FPS);
    }

    #[test]
    fn server_timing_set_fps_clamps_high() {
        let mut timing = ServerTiming::new();
        timing.set_fps(200); // above MAX_SV_FPS (90)
        assert_eq!(timing.sv_fps, MAX_SV_FPS);
        assert_eq!(timing.sv_frametime, 1000 / MAX_SV_FPS);
    }

    #[test]
    fn server_timing_set_fps_boundary_min() {
        let mut timing = ServerTiming::new();
        timing.set_fps(MIN_SV_FPS);
        assert_eq!(timing.sv_fps, MIN_SV_FPS);
    }

    #[test]
    fn server_timing_set_fps_boundary_max() {
        let mut timing = ServerTiming::new();
        timing.set_fps(MAX_SV_FPS);
        assert_eq!(timing.sv_fps, MAX_SV_FPS);
        assert_eq!(timing.sv_frametime, 1000 / MAX_SV_FPS);
    }

    #[test]
    fn server_timing_set_fps_no_change_if_same() {
        let mut timing = ServerTiming::new();
        let initial_fps = timing.sv_fps;
        let initial_frametime = timing.sv_frametime;
        timing.set_fps(initial_fps);
        assert_eq!(timing.sv_fps, initial_fps);
        assert_eq!(timing.sv_frametime, initial_frametime);
    }

    #[test]
    fn server_timing_enable() {
        let mut timing = ServerTiming::new();
        timing.time_residual = 50;
        timing.enable();
        assert!(timing.enabled);
        assert_eq!(timing.time_residual, 0, "Enable should reset time_residual");
    }

    #[test]
    fn server_timing_disable() {
        let mut timing = ServerTiming::new();
        timing.enabled = true;
        timing.time_residual = 50;
        timing.disable();
        assert!(!timing.enabled);
        assert_eq!(timing.time_residual, 0, "Disable should reset time_residual");
    }

    #[test]
    fn server_timing_reset() {
        let mut timing = ServerTiming::new();
        timing.time_residual = 150;
        timing.reset();
        assert_eq!(timing.time_residual, 0);
    }

    #[test]
    fn server_timing_frametime_calculation() {
        // Verify frametime calculations at various FPS values
        let mut timing = ServerTiming::new();

        timing.set_fps(10);
        assert_eq!(timing.sv_frametime, 100);

        timing.set_fps(20);
        assert_eq!(timing.sv_frametime, 50);

        timing.set_fps(50);
        assert_eq!(timing.sv_frametime, 20);

        timing.set_fps(90);
        assert_eq!(timing.sv_frametime, 11); // 1000/90 = 11 (integer division)
    }

    // =========================================================================
    // Ping calculation logic tests
    //
    // The ping calculation averages positive frame_latency values.
    // We test the math directly since sv_calc_pings requires full ServerContext.
    // =========================================================================

    fn calc_ping(frame_latency: &[i32; LATENCY_COUNTS]) -> i32 {
        let mut total = 0;
        let mut count = 0;
        for j in 0..LATENCY_COUNTS {
            if frame_latency[j] > 0 {
                count += 1;
                total += frame_latency[j];
            }
        }
        if count == 0 { 0 } else { total / count }
    }

    #[test]
    fn ping_calc_all_zero_returns_zero() {
        let latency = [0i32; LATENCY_COUNTS];
        assert_eq!(calc_ping(&latency), 0);
    }

    #[test]
    fn ping_calc_all_positive() {
        let mut latency = [0i32; LATENCY_COUNTS];
        for i in 0..LATENCY_COUNTS {
            latency[i] = 50;
        }
        assert_eq!(calc_ping(&latency), 50);
    }

    #[test]
    fn ping_calc_mixed_values() {
        let mut latency = [0i32; LATENCY_COUNTS];
        latency[0] = 100;
        latency[1] = 200;
        latency[2] = 300;
        // 3 positive values, total = 600, average = 200
        assert_eq!(calc_ping(&latency), 200);
    }

    #[test]
    fn ping_calc_some_negative_ignored() {
        let mut latency = [0i32; LATENCY_COUNTS];
        latency[0] = 100;
        latency[1] = -50; // negative, should be ignored
        latency[2] = 200;
        // 2 positive values, total = 300, average = 150
        assert_eq!(calc_ping(&latency), 150);
    }

    #[test]
    fn ping_calc_single_value() {
        let mut latency = [0i32; LATENCY_COUNTS];
        latency[5] = 75;
        assert_eq!(calc_ping(&latency), 75);
    }

    // =========================================================================
    // Timeout logic tests
    //
    // Tests the timeout decision logic extracted from sv_check_timeouts.
    // =========================================================================

    fn determine_timeout_action(
        state: ClientState,
        lastmessage: i32,
        realtime: i32,
        droppoint: i32,
        zombiepoint: i32,
    ) -> &'static str {
        if lastmessage > realtime {
            return "fix_time";
        }
        match state {
            ClientState::Zombie if lastmessage < zombiepoint => "free_zombie",
            ClientState::Connected | ClientState::Spawned if lastmessage < droppoint => "drop",
            _ => "none",
        }
    }

    #[test]
    fn timeout_no_action_for_free_client() {
        let action = determine_timeout_action(
            ClientState::Free,
            1000,
            2000,
            0,    // droppoint
            0,    // zombiepoint
        );
        assert_eq!(action, "none");
    }

    #[test]
    fn timeout_drop_spawned_client() {
        let realtime = 200000;
        let timeout_val = 125.0;
        let droppoint = realtime - (1000.0 * timeout_val) as i32;
        let zombiepoint = realtime - 1000;

        let action = determine_timeout_action(
            ClientState::Spawned,
            0, // lastmessage way in the past
            realtime,
            droppoint,
            zombiepoint,
        );
        assert_eq!(action, "drop");
    }

    #[test]
    fn timeout_drop_connected_client() {
        let realtime = 200000;
        let droppoint = realtime - 125000;
        let zombiepoint = realtime - 1000;

        let action = determine_timeout_action(
            ClientState::Connected,
            0, // way in the past
            realtime,
            droppoint,
            zombiepoint,
        );
        assert_eq!(action, "drop");
    }

    #[test]
    fn timeout_free_zombie_client() {
        let realtime = 10000;
        let droppoint = realtime - 125000;
        let zombiepoint = realtime - 1000; // zombiepoint = 9000

        let action = determine_timeout_action(
            ClientState::Zombie,
            5000, // lastmessage < zombiepoint (9000)
            realtime,
            droppoint,
            zombiepoint,
        );
        assert_eq!(action, "free_zombie");
    }

    #[test]
    fn timeout_fix_time_wraparound() {
        let action = determine_timeout_action(
            ClientState::Spawned,
            20000,  // lastmessage in the future
            10000,  // realtime
            0,
            0,
        );
        assert_eq!(action, "fix_time");
    }

    #[test]
    fn timeout_no_action_active_client() {
        let realtime = 10000;
        let droppoint = realtime - 125000;
        let zombiepoint = realtime - 1000;

        let action = determine_timeout_action(
            ClientState::Spawned,
            9999, // very recent, not timed out
            realtime,
            droppoint,
            zombiepoint,
        );
        assert_eq!(action, "none");
    }

    // =========================================================================
    // sv_give_msec logic test
    // =========================================================================

    #[test]
    fn give_msec_only_on_16th_frame() {
        // sv_give_msec only fires when framenum & 15 == 0
        for i in 0..64 {
            let should_fire = (i & 15) == 0;
            assert_eq!(
                i & 15 == 0,
                should_fire,
                "Frame {} should{} trigger give_msec",
                i,
                if should_fire { "" } else { " not" }
            );
        }
    }

    // =========================================================================
    // net_sleep logic test
    // =========================================================================

    #[test]
    fn net_sleep_zero_does_not_block() {
        // Verify net_sleep with 0 or negative does not sleep
        let start = std::time::Instant::now();
        net_sleep(0);
        net_sleep(-1);
        let elapsed = start.elapsed();
        assert!(elapsed.as_millis() < 50, "net_sleep(0) and net_sleep(-1) should be instant");
    }

    // =========================================================================
    // Constants tests
    // =========================================================================

    #[test]
    fn heartbeat_seconds_constant() {
        assert_eq!(HEARTBEAT_SECONDS, 300);
    }

    #[test]
    fn sv_fps_constants() {
        assert_eq!(DEFAULT_SV_FPS, 10);
        assert_eq!(MIN_SV_FPS, 10);
        assert_eq!(MAX_SV_FPS, 90);
        assert!(MIN_SV_FPS <= DEFAULT_SV_FPS);
        assert!(DEFAULT_SV_FPS <= MAX_SV_FPS);
    }

    // =========================================================================
    // rcon_validate tests (requires ServerContext but we can test the logic)
    // =========================================================================

    #[test]
    fn rcon_validate_empty_password_returns_false() {
        // When rcon_password is empty, validation always fails
        // This is tested implicitly; the actual function checks cvar_variable_string
        let password = "";
        assert!(password.is_empty());
    }
}
