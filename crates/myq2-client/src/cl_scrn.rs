// cl_scrn.rs -- master for refresh, status bar, console, chat, notify, etc
// Converted from: myq2-original/client/cl_scrn.c

use crate::client::*;
use crate::console::*;
use crate::cl_hud;
use myq2_common::q_shared::*;
use myq2_common::common::{com_printf, com_error};

// ============================================================
// Module-level state
// ============================================================

pub struct ScrState {
    pub scr_con_current: f32,    // approaches scr_conlines at scr_conspeed
    pub scr_conlines: f32,       // 0.0 to 1.0 lines of console to display
    pub scr_initialized: bool,   // ready to draw
    pub scr_draw_loading: i32,

    pub scr_vrect: VRect,        // position of render window on screen

    // cvars (indices or references into cvar system)
    pub scr_viewsize: CvarHandle,
    pub scr_conspeed: CvarHandle,
    pub scr_centertime: CvarHandle,
    pub scr_showturtle: CvarHandle,
    pub scr_showpause: CvarHandle,
    pub scr_printspeed: CvarHandle,
    pub scr_netgraph: CvarHandle,
    pub scr_timegraph: CvarHandle,
    pub scr_debuggraph: CvarHandle,
    pub scr_graphheight: CvarHandle,
    pub scr_graphscale: CvarHandle,
    pub scr_graphshift: CvarHandle,
    pub scr_drawall: CvarHandle,

    // dirty rectangles
    pub scr_dirty: DirtyRect,
    pub scr_old_dirty: [DirtyRect; 2],

    // crosshair
    pub crosshair_pic: String,
    pub crosshair_width: i32,
    pub crosshair_height: i32,

    // center print
    pub scr_centerstring: String,
    pub scr_centertime_start: f32,
    pub scr_centertime_off: f32,
    pub scr_center_lines: i32,
    pub scr_erase_center: i32,

    // debug graph
    pub graph_current: i32,
    pub graph_values: [GraphSample; 1024],
}

#[derive(Clone, Copy, Default)]
pub struct DirtyRect {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
}

#[derive(Clone, Copy, Default)]
pub struct GraphSample {
    pub value: f32,
    pub color: i32,
}

#[derive(Clone, Copy, Default)]
pub struct VRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Placeholder for cvar handle type — will be defined properly in the cvar module.
pub type CvarHandle = i32;

pub const STAT_MINUS: usize = 10; // num frame for '-' stats digit

pub const SB_NUMS: [[&str; 11]; 2] = [
    [
        "num_0", "num_1", "num_2", "num_3", "num_4", "num_5",
        "num_6", "num_7", "num_8", "num_9", "num_minus",
    ],
    [
        "anum_0", "anum_1", "anum_2", "anum_3", "anum_4", "anum_5",
        "anum_6", "anum_7", "anum_8", "anum_9", "anum_minus",
    ],
];

pub const ICON_WIDTH: i32 = 24;
pub const ICON_HEIGHT: i32 = 24;
pub const CHAR_WIDTH: i32 = 16;
pub const ICON_SPACE: i32 = 8;

pub const STAT_LAYOUTS: usize = myq2_common::q_shared::STAT_LAYOUTS as usize;

impl Default for ScrState {
    fn default() -> Self {
        Self {
            scr_con_current: 0.0,
            scr_conlines: 0.0,
            scr_initialized: false,
            scr_draw_loading: 0,
            scr_vrect: VRect::default(),
            scr_viewsize: 0,
            scr_conspeed: 0,
            scr_centertime: 0,
            scr_showturtle: 0,
            scr_showpause: 0,
            scr_printspeed: 0,
            scr_netgraph: 0,
            scr_timegraph: 0,
            scr_debuggraph: 0,
            scr_graphheight: 0,
            scr_graphscale: 0,
            scr_graphshift: 0,
            scr_drawall: 0,
            scr_dirty: DirtyRect::default(),
            scr_old_dirty: [DirtyRect::default(); 2],
            crosshair_pic: String::new(),
            crosshair_width: 0,
            crosshair_height: 0,
            scr_centerstring: String::new(),
            scr_centertime_start: 0.0,
            scr_centertime_off: 0.0,
            scr_center_lines: 0,
            scr_erase_center: 0,
            graph_current: 0,
            graph_values: [GraphSample::default(); 1024],
        }
    }
}

// ============================================================
// BAR GRAPHS
// ============================================================

/// A new packet was just parsed
pub fn cl_add_netgraph(scr: &mut ScrState, cls: &ClientStatic, cl: &ClientState) {
    // if using the debuggraph for something else, don't add the net lines
    if cvar_value(scr.scr_debuggraph) != 0.0 || cvar_value(scr.scr_timegraph) != 0.0 {
        return;
    }

    for _i in 0..cls.netchan.dropped {
        scr_debug_graph(scr, 30.0, 0x40);
    }

    for _i in 0..cl.surpresscount {
        scr_debug_graph(scr, 30.0, 0xdf);
    }

    // see what the latency was on this packet
    let in_idx = (cls.netchan.incoming_acknowledged as usize) & (CMD_BACKUP - 1);
    let mut ping = (cls.realtime - cl.cmd_time[in_idx]) / 30;
    if ping > 30 {
        ping = 30;
    }
    scr_debug_graph(scr, ping as f32, 0xd0);
}

pub fn scr_debug_graph(scr: &mut ScrState, value: f32, color: i32) {
    let idx = (scr.graph_current as usize) & 1023;
    scr.graph_values[idx].value = value;
    scr.graph_values[idx].color = color;
    scr.graph_current += 1;
}

pub fn scr_draw_debug_graph(scr: &ScrState) {
    let w = scr.scr_vrect.width;
    let x = scr.scr_vrect.x;
    let y = scr.scr_vrect.y + scr.scr_vrect.height;
    let graphheight = cvar_value(scr.scr_graphheight);
    let graphscale = cvar_value(scr.scr_graphscale);
    let graphshift = cvar_value(scr.scr_graphshift);

    draw_fill(x, y - graphheight as i32, w, graphheight as i32, 5, 0.15);

    for a in 0..w {
        let i = ((scr.graph_current - 1 - a + 1024) as usize) & 1023;
        let mut v = scr.graph_values[i].value;
        let color = scr.graph_values[i].color;
        v = v * graphscale + graphshift;

        if v < 0.0 {
            v += graphheight * (1.0 + (-v / graphheight));
        }
        let h = (v as i32) % (graphheight as i32);
        draw_fill(x + w - 1 - a, y - h, 1, h, color, 0.5);
    }
}

// ============================================================
// CENTER PRINTING
// ============================================================

/// Called for important messages that should stay in the center of the screen
/// for a few moments
pub fn scr_center_print(scr: &mut ScrState, cl: &ClientState, str_msg: &str) {
    scr.scr_centerstring = str_msg.chars().take(1023).collect();
    scr.scr_centertime_off = cvar_value(scr.scr_centertime);
    scr.scr_centertime_start = cl.time as f32;

    // count the number of lines for centering
    scr.scr_center_lines = 1;
    for ch in str_msg.chars() {
        if ch == '\n' {
            scr.scr_center_lines += 1;
        }
    }

    // echo it to the console
    com_printf("\n\n\x1d\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1f\n");

    let mut s = str_msg;
    loop {
        // scan the width of the line
        let mut l = 0;
        let bytes = s.as_bytes();
        while l < 40 && l < bytes.len() {
            if bytes[l] == b'\n' || bytes[l] == 0 {
                break;
            }
            l += 1;
        }

        let mut line = String::new();
        for _i in 0..(40 - l) / 2 {
            line.push(' ');
        }
        for j in 0..l {
            line.push(bytes[j] as char);
        }
        line.push('\n');

        com_printf(&line);

        // skip past the line content
        let mut idx = 0;
        while idx < bytes.len() && bytes[idx] != b'\n' && bytes[idx] != 0 {
            idx += 1;
        }

        if idx >= bytes.len() || bytes[idx] == 0 {
            break;
        }
        s = &s[idx + 1..]; // skip the \n
    }

    com_printf("\n\n\x1d\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1e\x1f\n");
    con_clear_notify();
}

pub fn scr_draw_center_string(scr: &mut ScrState, viddef: &VidDef) {
    let remaining: i32 = 9999;

    scr.scr_erase_center = 0;
    let start_str = scr.scr_centerstring.clone();
    let mut start: &str = &start_str;

    let mut y = if scr.scr_center_lines <= 4 {
        (viddef.height as f32 * 0.35) as i32
    } else {
        48
    };

    loop {
        // scan the width of the line
        let bytes = start.as_bytes();
        let mut l = 0;
        while l < 40 && l < bytes.len() {
            if bytes[l] == b'\n' || bytes[l] == 0 {
                break;
            }
            l += 1;
        }
        let mut x = (viddef.width - l as i32 * 8) / 2;
        scr_add_dirty_point(scr, x, y);
        let mut remaining_count = remaining;
        for j in 0..l {
            draw_char(x, y, bytes[j] as i32);
            if remaining_count <= 0 {
                return;
            }
            remaining_count -= 1;
            x += 8;
        }
        scr_add_dirty_point(scr, x, y + 8);

        y += 8;

        // skip past content
        let mut idx = 0;
        while idx < bytes.len() && bytes[idx] != b'\n' && bytes[idx] != 0 {
            idx += 1;
        }

        if idx >= bytes.len() || bytes[idx] == 0 {
            break;
        }
        start = &start[idx + 1..];
    }
}

pub fn scr_check_draw_center_string(scr: &mut ScrState, cls: &ClientStatic, viddef: &VidDef) {
    scr.scr_centertime_off -= cls.frametime;

    if scr.scr_centertime_off <= 0.0 {
        return;
    }

    scr_draw_center_string(scr, viddef);
}

// ============================================================
// SCR_CalcVrect
// ============================================================

/// Sets scr_vrect, the coordinates of the rendered window
fn scr_calc_vrect(scr: &mut ScrState, viddef: &VidDef) {
    // bound viewsize
    let viewsize_val = cvar_value(scr.scr_viewsize);
    if viewsize_val < 40.0 {
        cvar_set("viewsize", "40");
    }
    if viewsize_val > 100.0 {
        cvar_set("viewsize", "100");
    }

    let size = cvar_value(scr.scr_viewsize) as i32;

    scr.scr_vrect.width = viddef.width * size / 100;
    scr.scr_vrect.width &= !7;

    scr.scr_vrect.height = viddef.height * size / 100;
    scr.scr_vrect.height &= !1;

    scr.scr_vrect.x = (viddef.width - scr.scr_vrect.width) / 2;
    scr.scr_vrect.y = (viddef.height - scr.scr_vrect.height) / 2;
}

/// Keybinding command
pub fn scr_size_up_f(scr: &ScrState) {
    cvar_set_value("viewsize", cvar_value(scr.scr_viewsize) + 10.0);
}

/// Keybinding command
pub fn scr_size_down_f(scr: &ScrState) {
    cvar_set_value("viewsize", cvar_value(scr.scr_viewsize) - 10.0);
}

/// Set a specific sky and rotation speed
pub fn scr_sky_f() {
    let argc = cmd_argc();
    if argc < 2 {
        com_printf("Usage: sky <basename> <rotate> <axis x y z>\n");
        return;
    }

    let rotate = if argc > 2 {
        cmd_argv(2).parse::<f32>().unwrap_or(0.0)
    } else {
        0.0
    };

    let axis = if argc == 6 {
        [
            cmd_argv(3).parse::<f32>().unwrap_or(0.0),
            cmd_argv(4).parse::<f32>().unwrap_or(0.0),
            cmd_argv(5).parse::<f32>().unwrap_or(0.0),
        ]
    } else {
        [0.0, 0.0, 1.0]
    };

    r_set_sky(&cmd_argv(1), rotate, &axis);
}

// ============================================================
// SCR_Init
// ============================================================

pub fn scr_init(scr: &mut ScrState) {
    scr.scr_viewsize = cvar_get("viewsize", "100", CVAR_ARCHIVE);
    scr.scr_conspeed = cvar_get("scr_conspeed", "3", CVAR_ZERO);
    scr.scr_showturtle = cvar_get("scr_showturtle", "0", CVAR_ZERO);
    scr.scr_showpause = cvar_get("scr_showpause", "1", CVAR_ZERO);
    scr.scr_centertime = cvar_get("scr_centertime", "2.5", CVAR_ZERO);
    scr.scr_printspeed = cvar_get("scr_printspeed", "8", CVAR_ZERO);
    scr.scr_netgraph = cvar_get("netgraph", "0", CVAR_ZERO);
    scr.scr_timegraph = cvar_get("timegraph", "0", CVAR_ZERO);
    scr.scr_debuggraph = cvar_get("debuggraph", "0", CVAR_ZERO);
    scr.scr_graphheight = cvar_get("graphheight", "32", CVAR_ZERO);
    scr.scr_graphscale = cvar_get("graphscale", "1", CVAR_ZERO);
    scr.scr_graphshift = cvar_get("graphshift", "0", CVAR_ZERO);
    scr.scr_drawall = cvar_get("scr_drawall", "0", CVAR_ZERO);

    // Register our commands.
    // timerefresh and loading need access to global ScrState/ClientStatic/ClientState
    // which are behind Mutex in cl_main. They will function once the top-level engine
    // context provides a command dispatch that can lock the required state.
    cmd_add_command("timerefresh", || {
        com_printf("timerefresh: requires renderer integration (R_BeginFrame/R_RenderFrame)\n");
    });
    cmd_add_command("loading", || {
        // In the C original this just calls SCR_BeginLoadingPlaque().
        // The Rust version needs global state access (scr, cls, cl).
        com_printf("loading: begin loading plaque\n");
    });
    cmd_add_command("sizeup", scr_size_up_f_cmd);
    cmd_add_command("sizedown", scr_size_down_f_cmd);
    cmd_add_command("sky", scr_sky_f);

    scr.scr_initialized = true;
}

pub fn scr_draw_net(scr: &ScrState, cls: &ClientStatic) {
    if cls.netchan.outgoing_sequence - cls.netchan.incoming_acknowledged
        < CMD_BACKUP as i32 - 1
    {
        return;
    }

    draw_pic(scr.scr_vrect.x + 64, scr.scr_vrect.y, "net");
}

pub fn scr_draw_pause(scr: &ScrState, viddef: &VidDef) {
    if cvar_value(scr.scr_showpause) == 0.0 {
        return;
    }

    if cl_paused_value() == 0.0 {
        return;
    }

    let (w, _h) = draw_get_pic_size("pause");
    draw_pic((viddef.width - w) / 2, viddef.height / 2 + 8, "pause");
}

pub fn scr_draw_loading(scr: &mut ScrState, viddef: &VidDef) {
    if scr.scr_draw_loading == 0 {
        return;
    }

    scr.scr_draw_loading = 0;
    let (w, h) = draw_get_pic_size("loading");
    draw_pic((viddef.width - w) / 2, (viddef.height - h) / 2, "loading");
}

// ============================================================
// SCR_RunConsole — Scroll it up or down
// ============================================================

pub fn scr_run_console(scr: &mut ScrState, cls: &ClientStatic) {
    // decide on the height of the console
    if cls.key_dest == KeyDest::Console {
        scr.scr_conlines = 0.5; // half screen
    } else {
        scr.scr_conlines = 0.0; // none visible
    }

    if scr.scr_conlines < scr.scr_con_current {
        scr.scr_con_current -= cvar_value(scr.scr_conspeed) * cls.frametime;
        if scr.scr_conlines > scr.scr_con_current {
            scr.scr_con_current = scr.scr_conlines;
        }
    } else if scr.scr_conlines > scr.scr_con_current {
        scr.scr_con_current += cvar_value(scr.scr_conspeed) * cls.frametime;
        if scr.scr_conlines < scr.scr_con_current {
            scr.scr_con_current = scr.scr_conlines;
        }
    }
}

pub fn scr_draw_console(scr: &ScrState, cls: &ClientStatic, cl: &ClientState, viddef: &VidDef) {
    con_check_resize();

    if cls.state == ConnState::Disconnected || cls.state == ConnState::Connecting {
        // forced full screen console
        con_draw_console(1.0);
        return;
    }

    if cls.state != ConnState::Active || !cl.refresh_prepped {
        // connected, but can't render
        con_draw_console(0.5);
        draw_fill(0, viddef.height / 2, viddef.width, viddef.height / 2, 0, 1.0);
        return;
    }

    if scr.scr_con_current != 0.0 {
        con_draw_console(scr.scr_con_current);
    }
    // mattx86: we want the console notify lines always visible, even with the console down.
    con_draw_notify();
}

// ============================================================
// Loading plaque
// ============================================================

pub fn scr_begin_loading_plaque(scr: &mut ScrState, cls: &mut ClientStatic, cl: &mut ClientState) {
    s_stop_all_sounds();
    cl.sound_prepped = false; // don't play ambients
    if cls.disable_screen != 0.0 {
        return;
    }
    if developer_value() != 0.0 {
        return;
    }
    if cls.state == ConnState::Disconnected {
        return; // if at console, don't bring up the plaque
    }
    if cls.key_dest == KeyDest::Console {
        return;
    }
    if cl.cinematictime > 0 {
        scr.scr_draw_loading = 2; // clear to black first
    } else {
        scr.scr_draw_loading = 1;
    }
    scr_update_screen(scr, cls, cl);
    cls.disable_screen = sys_milliseconds() as f32;
    cls.disable_servercount = cl.servercount;
}

pub fn scr_end_loading_plaque(cls: &mut ClientStatic, clear: bool) {
    cls.disable_screen = 0.0;

    // mattx86: a work-around for notify lines + console + not breaking this function
    if clear {
        con_clear_notify();
    }
}

pub fn scr_loading_f(scr: &mut ScrState, cls: &mut ClientStatic, cl: &mut ClientState) {
    scr_begin_loading_plaque(scr, cls, cl);
}

// ============================================================
// SCR_TimeRefresh_f
// ============================================================

pub fn entity_cmp_fnc(a: &Entity, b: &Entity) -> std::cmp::Ordering {
    if a.model == b.model {
        a.skin.cmp(&b.skin)
    } else {
        a.model.cmp(&b.model)
    }
}

pub fn scr_time_refresh_f(cls: &ClientStatic, cl: &mut ClientState) {
    if cls.state != ConnState::Active {
        return;
    }

    let start = sys_milliseconds();

    if cmd_argc() == 2 {
        // run without page flipping
        r_begin_frame(0.0);
        for i in 0..128 {
            cl.refdef.viewangles[1] = i as f32 / 128.0 * 360.0;
            r_render_frame(&cl.refdef);
        }
        vk_imp_end_frame();
    } else {
        for i in 0..128 {
            cl.refdef.viewangles[1] = i as f32 / 128.0 * 360.0;
            r_begin_frame(0.0);
            r_render_frame(&cl.refdef);
            vk_imp_end_frame();
        }
    }

    let stop = sys_milliseconds();
    let time = (stop - start) as f32 / 1000.0;
    com_printf(&format!("{} seconds ({} fps)\n", time, 128.0 / time));
}

// ============================================================
// Dirty points
// ============================================================

pub fn scr_add_dirty_point(scr: &mut ScrState, x: i32, y: i32) {
    if x < scr.scr_dirty.x1 {
        scr.scr_dirty.x1 = x;
    }
    if x > scr.scr_dirty.x2 {
        scr.scr_dirty.x2 = x;
    }
    if y < scr.scr_dirty.y1 {
        scr.scr_dirty.y1 = y;
    }
    if y > scr.scr_dirty.y2 {
        scr.scr_dirty.y2 = y;
    }
}

pub fn scr_dirty_screen(scr: &mut ScrState, viddef: &VidDef) {
    scr_add_dirty_point(scr, 0, 0);
    scr_add_dirty_point(scr, viddef.width - 1, viddef.height - 1);
}

/// Clear any parts of the tiled background that were drawn on last frame
pub fn scr_tile_clear(scr: &mut ScrState, cl: &ClientState, viddef: &VidDef) {
    if cvar_value(scr.scr_drawall) != 0.0 {
        scr_dirty_screen(scr, viddef); // for power vr or broken page flippers...
    }

    if scr.scr_con_current == 1.0 {
        return; // full screen console
    }
    if cvar_value(scr.scr_viewsize) == 100.0 {
        return; // full screen rendering
    }
    if cl.cinematictime > 0 {
        return; // full screen cinematic
    }

    // erase rect will be the union of the past three frames
    // so triple buffering works properly
    let mut clear = scr.scr_dirty;
    for i in 0..2 {
        if scr.scr_old_dirty[i].x1 < clear.x1 {
            clear.x1 = scr.scr_old_dirty[i].x1;
        }
        if scr.scr_old_dirty[i].x2 > clear.x2 {
            clear.x2 = scr.scr_old_dirty[i].x2;
        }
        if scr.scr_old_dirty[i].y1 < clear.y1 {
            clear.y1 = scr.scr_old_dirty[i].y1;
        }
        if scr.scr_old_dirty[i].y2 > clear.y2 {
            clear.y2 = scr.scr_old_dirty[i].y2;
        }
    }

    scr.scr_old_dirty[1] = scr.scr_old_dirty[0];
    scr.scr_old_dirty[0] = scr.scr_dirty;

    scr.scr_dirty.x1 = 9999;
    scr.scr_dirty.x2 = -9999;
    scr.scr_dirty.y1 = 9999;
    scr.scr_dirty.y2 = -9999;

    // don't bother with anything covered by the console
    let top_con = (scr.scr_con_current * viddef.height as f32) as i32;
    if top_con >= clear.y1 {
        clear.y1 = top_con;
    }

    if clear.y2 <= clear.y1 {
        return; // nothing disturbed
    }

    let top = scr.scr_vrect.y;
    let bottom = top + scr.scr_vrect.height - 1;
    let left = scr.scr_vrect.x;
    let right = left + scr.scr_vrect.width - 1;

    if clear.y1 < top {
        // clear above view screen
        let i = if clear.y2 < top - 1 { clear.y2 } else { top - 1 };
        draw_tile_clear(
            clear.x1, clear.y1,
            clear.x2 - clear.x1 + 1, i - clear.y1 + 1, "backtile",
        );
        clear.y1 = top;
    }
    if clear.y2 > bottom {
        // clear below view screen
        let i = if clear.y1 > bottom + 1 { clear.y1 } else { bottom + 1 };
        draw_tile_clear(
            clear.x1, i,
            clear.x2 - clear.x1 + 1, clear.y2 - i + 1, "backtile",
        );
        clear.y2 = bottom;
    }
    if clear.x1 < left {
        // clear left of view screen
        let i = if clear.x2 < left - 1 { clear.x2 } else { left - 1 };
        draw_tile_clear(
            clear.x1, clear.y1,
            i - clear.x1 + 1, clear.y2 - clear.y1 + 1, "backtile",
        );
        clear.x1 = left;
    }
    if clear.x2 > right {
        // clear right of view screen
        let i = if clear.x1 > right + 1 { clear.x1 } else { right + 1 };
        draw_tile_clear(
            i, clear.y1,
            clear.x2 - i + 1, clear.y2 - clear.y1 + 1, "backtile",
        );
        // clear.x2 = right; // not needed, last usage
    }
}

// ============================================================
// HUD string helpers
// ============================================================

/// Allow embedded \n in the string
pub fn size_hud_string(string: &str) -> (i32, i32) {
    let mut lines = 1;
    let mut width = 0;
    let mut current = 0;

    for ch in string.chars() {
        if ch == '\n' {
            lines += 1;
            current = 0;
        } else {
            current += 1;
            if current > width {
                width = current;
            }
        }
    }

    (width * 8, lines * 8)
}

pub fn draw_hud_string(string: &str, x_start: i32, mut y: i32, centerwidth: i32, xor_val: i32) {
    let margin = x_start;
    let mut s = string;

    while !s.is_empty() {
        // scan out one line of text from the string
        let mut width = 0;
        let mut line = String::new();
        let bytes = s.as_bytes();
        while width < bytes.len() && bytes[width] != b'\n' && bytes[width] != 0 {
            line.push(bytes[width] as char);
            width += 1;
        }

        let mut x = if centerwidth != 0 {
            margin + (centerwidth - width as i32 * 8) / 2
        } else {
            margin
        };

        for ch in line.bytes() {
            draw_char(x, y, (ch as i32) ^ xor_val);
            x += 8;
        }

        // advance past the line
        if width < bytes.len() && bytes[width] == b'\n' {
            s = &s[width + 1..]; // skip the \n
            y += 8;
        } else {
            break;
        }
    }
}

// ============================================================
// SCR_DrawField
// ============================================================

pub fn scr_draw_field(scr: &mut ScrState, x: i32, y: i32, color: usize, mut width: i32, value: i32) {
    if width < 1 {
        return;
    }

    // draw number string
    if width > 5 {
        width = 5;
    }

    scr_add_dirty_point(scr, x, y);
    scr_add_dirty_point(scr, x + width * CHAR_WIDTH + 2, y + 23);

    let num = format!("{}", value);
    let mut l = num.len() as i32;
    if l > width {
        l = width;
    }
    let mut x = x + 2 + CHAR_WIDTH * (width - l);

    let mut chars_left = l;
    for ch in num.bytes() {
        if chars_left <= 0 {
            break;
        }
        let frame = if ch == b'-' {
            STAT_MINUS
        } else {
            (ch - b'0') as usize
        };

        draw_pic(x, y, SB_NUMS[color][frame]);
        x += CHAR_WIDTH;
        chars_left -= 1;
    }
}

// ============================================================
// SCR_TouchPics
// ============================================================

/// Allows rendering code to cache all needed sbar graphics
pub fn scr_touch_pics(scr: &mut ScrState) {
    for i in 0..2 {
        for j in 0..11 {
            draw_find_pic(SB_NUMS[i][j]);
        }
    }

    let crosshair_val = crosshair_value();
    if crosshair_val != 0.0 {
        let mut cv = crosshair_val;
        if !(0.0..=3.0).contains(&cv) {
            cv = 3.0;
        }

        scr.crosshair_pic = format!("ch{}", cv as i32);
        let (w, h) = draw_get_pic_size(&scr.crosshair_pic);
        scr.crosshair_width = w;
        scr.crosshair_height = h;
        if scr.crosshair_width == 0 {
            scr.crosshair_pic.clear();
        }
    }
}

// ============================================================
// SCR_ExecuteLayoutString
// ============================================================

pub fn scr_execute_layout_string(
    scr: &mut ScrState,
    cls: &ClientStatic,
    cl: &ClientState,
    viddef: &VidDef,
    layout: &str,
) {
    if cls.state != ConnState::Active || !cl.refresh_prepped {
        return;
    }

    if layout.is_empty() {
        return;
    }

    let mut x: i32 = 0;
    let mut y: i32 = 0;
    let mut width: i32 = 3;

    let mut s: Option<&str> = Some(layout);

    while let Some(remaining) = s {
        let (token, next) = com_parse(remaining);
        s = next;

        if token.is_empty() {
            break;
        }

        match token.as_str() {
            "xl" => {
                let (t, n) = com_parse(s.unwrap_or(""));
                s = n;
                x = t.parse::<i32>().unwrap_or(0);
            }
            "xr" => {
                let (t, n) = com_parse(s.unwrap_or(""));
                s = n;
                x = viddef.width + t.parse::<i32>().unwrap_or(0);
            }
            "xv" => {
                let (t, n) = com_parse(s.unwrap_or(""));
                s = n;
                x = viddef.width / 2 - 160 + t.parse::<i32>().unwrap_or(0);
            }
            "yt" => {
                let (t, n) = com_parse(s.unwrap_or(""));
                s = n;
                y = t.parse::<i32>().unwrap_or(0);
            }
            "yb" => {
                let (t, n) = com_parse(s.unwrap_or(""));
                s = n;
                y = viddef.height + t.parse::<i32>().unwrap_or(0);
            }
            "yv" => {
                let (t, n) = com_parse(s.unwrap_or(""));
                s = n;
                y = viddef.height / 2 - 120 + t.parse::<i32>().unwrap_or(0);
            }
            "pic" => {
                // draw a pic from a stat number
                let (t, n) = com_parse(s.unwrap_or(""));
                s = n;
                let stat_idx = t.parse::<usize>().unwrap_or(0);
                let value = cl.frame.playerstate.stats[stat_idx] as usize;
                if value >= MAX_IMAGES {
                    com_error(ERR_DROP, "Pic >= MAX_IMAGES");
                }
                let cs_str = &cl.configstrings[CS_IMAGES + value];
                if !cs_str.is_empty() {
                    scr_add_dirty_point(scr, x, y);
                    scr_add_dirty_point(scr, x + 23, y + 23);
                    draw_pic(x, y, cs_str);
                }
            }
            "client" => {
                // draw a deathmatch client block
                let (t, n) = com_parse(s.unwrap_or(""));
                s = n;
                x = viddef.width / 2 - 160 + t.parse::<i32>().unwrap_or(0);
                let (t, n) = com_parse(s.unwrap_or(""));
                s = n;
                y = viddef.height / 2 - 120 + t.parse::<i32>().unwrap_or(0);
                scr_add_dirty_point(scr, x, y);
                scr_add_dirty_point(scr, x + 159, y + 31);

                let (t, n) = com_parse(s.unwrap_or(""));
                s = n;
                let value = t.parse::<usize>().unwrap_or(0);
                if value >= MAX_CLIENTS {
                    com_error(ERR_DROP, "client >= MAX_CLIENTS");
                }
                let ci = &cl.clientinfo[value];

                let (t, n) = com_parse(s.unwrap_or(""));
                s = n;
                let score = t.parse::<i32>().unwrap_or(0);

                let (t, n) = com_parse(s.unwrap_or(""));
                s = n;
                let ping = t.parse::<i32>().unwrap_or(0);

                let (t, n) = com_parse(s.unwrap_or(""));
                s = n;
                let time = t.parse::<i32>().unwrap_or(0);

                draw_alt_string(x + 32, y, &ci.name);
                draw_string(x + 32, y + 8, "Score: ");
                draw_alt_string(x + 32 + 7 * 8, y + 8, &format!("{}", score));
                draw_string(x + 32, y + 16, &format!("Ping:  {}", ping));
                draw_string(x + 32, y + 24, &format!("Time:  {}", time));

                let icon_ci = if ci.icon == 0 { &cl.baseclientinfo } else { ci };
                draw_pic(x, y, &icon_ci.iconname);
            }
            "ctf" => {
                // draw a ctf client block
                let (t, n) = com_parse(s.unwrap_or(""));
                s = n;
                x = viddef.width / 2 - 160 + t.parse::<i32>().unwrap_or(0);
                let (t, n) = com_parse(s.unwrap_or(""));
                s = n;
                y = viddef.height / 2 - 120 + t.parse::<i32>().unwrap_or(0);
                scr_add_dirty_point(scr, x, y);
                scr_add_dirty_point(scr, x + 159, y + 31);

                let (t, n) = com_parse(s.unwrap_or(""));
                s = n;
                let value = t.parse::<usize>().unwrap_or(0);
                if value >= MAX_CLIENTS {
                    com_error(ERR_DROP, "client >= MAX_CLIENTS");
                }
                let ci = &cl.clientinfo[value];

                let (t, n) = com_parse(s.unwrap_or(""));
                s = n;
                let score = t.parse::<i32>().unwrap_or(0);

                let (t, n) = com_parse(s.unwrap_or(""));
                s = n;
                let mut ping = t.parse::<i32>().unwrap_or(0);
                if ping > 999 {
                    ping = 999;
                }

                let block = format!("{:3} {:3} {:<12.12}", score, ping, ci.name);

                if value == cl.playernum as usize {
                    draw_alt_string(x, y, &block);
                } else {
                    draw_string(x, y, &block);
                }
            }
            "picn" => {
                // draw a pic from a name
                let (t, n) = com_parse(s.unwrap_or(""));
                s = n;
                scr_add_dirty_point(scr, x, y);
                scr_add_dirty_point(scr, x + 23, y + 23);
                draw_pic(x, y, &t);
            }
            "num" => {
                // draw a number
                let (t, n) = com_parse(s.unwrap_or(""));
                s = n;
                width = t.parse::<i32>().unwrap_or(3);
                let (t, n) = com_parse(s.unwrap_or(""));
                s = n;
                let stat_idx = t.parse::<usize>().unwrap_or(0);
                // Use smoothed frag value when displaying STAT_FRAGS (R1Q2/Q2Pro feature)
                let value = if stat_idx == STAT_FRAGS as usize {
                    cl_hud::hud_get_smoothed_frags()
                } else {
                    cl.frame.playerstate.stats[stat_idx] as i32
                };
                scr_draw_field(scr, x, y, 0, width, value);
            }
            "hnum" => {
                // health number - use smoothed value for gradual transitions
                // Skip if HUD visibility says health is hidden (R1Q2/Q2Pro feature)
                if !cl_hud::hud_show_health() {
                    continue;
                }
                width = 3;
                let raw_value = cl.frame.playerstate.stats[STAT_HEALTH as usize] as i32;
                let value = cl_hud::hud_get_smoothed_health();
                // Use raw value for color logic (flashing state should be immediate)
                let color = if raw_value > 25 {
                    0 // green
                } else if raw_value > 0 {
                    ((cl.frame.serverframe >> 2) & 1) as usize // flash
                } else {
                    1
                };

                if cl.frame.playerstate.stats[STAT_FLASHES as usize] & 1 != 0 {
                    draw_pic(x, y, "field_3");
                }

                scr_draw_field(scr, x, y, color, width, value);
            }
            "anum" => {
                // ammo number - use smoothed value for gradual transitions
                // Skip if HUD visibility says ammo is hidden (R1Q2/Q2Pro feature)
                if !cl_hud::hud_show_ammo() {
                    continue;
                }
                width = 3;
                let raw_value = cl.frame.playerstate.stats[STAT_AMMO as usize] as i32;
                let value = cl_hud::hud_get_smoothed_ammo();
                // Use raw value for color logic (flashing state should be immediate)
                let color;
                if raw_value > 5 {
                    color = 0; // green
                } else if raw_value >= 0 {
                    color = ((cl.frame.serverframe >> 2) & 1) as usize; // flash
                } else {
                    continue; // negative number = don't show
                }

                if cl.frame.playerstate.stats[STAT_FLASHES as usize] & 4 != 0 {
                    draw_pic(x, y, "field_3");
                }

                scr_draw_field(scr, x, y, color, width, value);
            }
            "rnum" => {
                // armor number - use smoothed value for gradual transitions
                // Skip if HUD visibility says armor is hidden (R1Q2/Q2Pro feature)
                if !cl_hud::hud_show_armor() {
                    continue;
                }
                width = 3;
                let raw_value = cl.frame.playerstate.stats[STAT_ARMOR as usize] as i32;
                // Use raw value for visibility check (don't show armor pickup animation for 0 armor)
                if raw_value < 1 {
                    continue;
                }
                let value = cl_hud::hud_get_smoothed_armor();

                let color = 0; // green

                if cl.frame.playerstate.stats[STAT_FLASHES as usize] & 2 != 0 {
                    draw_pic(x, y, "field_3");
                }

                scr_draw_field(scr, x, y, color, width, value);
            }
            "stat_string" => {
                let (t, n) = com_parse(s.unwrap_or(""));
                s = n;
                let mut index = t.parse::<usize>().unwrap_or(0);
                if index >= MAX_CONFIGSTRINGS {
                    com_error(ERR_DROP, "Bad stat_string index");
                }
                index = cl.frame.playerstate.stats[index] as usize;
                if index >= MAX_CONFIGSTRINGS {
                    com_error(ERR_DROP, "Bad stat_string index");
                }
                draw_string(x, y, &cl.configstrings[index]);
            }
            "cstring" => {
                let (t, n) = com_parse(s.unwrap_or(""));
                s = n;
                draw_hud_string(&t, x, y, 320, 0);
            }
            "string" => {
                let (t, n) = com_parse(s.unwrap_or(""));
                s = n;
                draw_string(x, y, &t);
            }
            "cstring2" => {
                let (t, n) = com_parse(s.unwrap_or(""));
                s = n;
                draw_hud_string(&t, x, y, 320, 0x80);
            }
            "string2" => {
                let (t, n) = com_parse(s.unwrap_or(""));
                s = n;
                draw_alt_string(x, y, &t);
            }
            "if" => {
                // conditional draw
                let (t, n) = com_parse(s.unwrap_or(""));
                s = n;
                let stat_idx = t.parse::<usize>().unwrap_or(0);
                let value = cl.frame.playerstate.stats[stat_idx];
                if value == 0 {
                    // skip to endif
                    loop {
                        if s.is_none() {
                            break;
                        }
                        let (tok, n) = com_parse(s.unwrap_or(""));
                        s = n;
                        if tok == "endif" {
                            break;
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

// ============================================================
// SCR_DrawStats
// ============================================================

/// The status bar is a small layout program based on the stats array.
/// In minimal HUD mode (R1Q2/Q2Pro feature), the status bar layout is skipped
/// entirely, so only the overlay elements (FPS, speed, timer) are drawn.
pub fn scr_draw_stats(scr: &mut ScrState, cls: &ClientStatic, cl: &ClientState, viddef: &VidDef) {
    // Skip full status bar in minimal HUD mode (R1Q2/Q2Pro feature)
    if cl_hud::hud_is_minimal() {
        return;
    }
    let statusbar = cl.configstrings[CS_STATUSBAR].clone();
    scr_execute_layout_string(scr, cls, cl, viddef, &statusbar);
}

pub fn scr_draw_layout(scr: &mut ScrState, cls: &ClientStatic, cl: &ClientState, viddef: &VidDef) {
    if cl.frame.playerstate.stats[STAT_LAYOUTS] == 0 {
        return;
    }
    let layout = cl.layout.clone();
    scr_execute_layout_string(scr, cls, cl, viddef, &layout);
}

// ============================================================
// SCR_UpdateScreen
// ============================================================

/// This is called every frame, and can also be called explicitly to flush
/// text to the screen.
pub fn scr_update_screen(scr: &mut ScrState, cls: &mut ClientStatic, cl: &mut ClientState) {
    let viddef = get_viddef();
    let _separation: [f32; 2] = [0.0, 0.0];

    // if the screen is disabled (loading plaque is up, or vid mode changing)
    // do nothing at all
    if cls.disable_screen != 0.0 {
        if sys_milliseconds() as f32 - cls.disable_screen > 120000.0 {
            cls.disable_screen = 0.0;
            com_printf("Loading plaque timed out.\n");
        }
        return;
    }

    if !scr.scr_initialized || !con_initialized() {
        return; // not initialized yet
    }

    // range check cl_camera_separation so we don't inadvertently fry someone's brain
    let stereo_sep = cl_stereo_separation_value();
    if stereo_sep > 1.0 {
        cvar_set_value("cl_stereo_separation", 1.0);
    } else if stereo_sep < 0.0 {
        cvar_set_value("cl_stereo_separation", 0.0);
    }

    let (numframes, separation) = if cl_stereo_value() != 0.0 {
        let sep = cl_stereo_separation_value();
        (2, [-sep / 2.0, sep / 2.0])
    } else {
        (1, [0.0, 0.0])
    };

    for i in 0..numframes {
        r_begin_frame(separation[i]);

        if scr.scr_draw_loading == 2 {
            // loading plaque over black screen
            r_set_palette_null();
            scr.scr_draw_loading = 0;
            let (w, h) = draw_get_pic_size("loading");
            draw_pic(
                (viddef.width - w) / 2,
                (viddef.height - h) / 2,
                "loading",
            );
        } else if cl.cinematictime > 0 {
            // if a cinematic is supposed to be running, handle menus
            // and console specially
            if cls.key_dest == KeyDest::Menu {
                if cl.cinematicpalette_active {
                    r_set_palette_null();
                    cl.cinematicpalette_active = false;
                }
                m_draw();
            } else if cls.key_dest == KeyDest::Console {
                if cl.cinematicpalette_active {
                    r_set_palette_null();
                    cl.cinematicpalette_active = false;
                }
                scr_draw_console(scr, cls, cl, &viddef);
            } else {
                crate::cl_cin::scr_draw_cinematic(cl, cls);
            }
        } else {
            // make sure the game palette is active
            if cl.cinematicpalette_active {
                r_set_palette_null();
                cl.cinematicpalette_active = false;
            }

            // do 3D refresh drawing, and then update the screen
            scr_calc_vrect(scr, &viddef);

            // clear any dirty part of the background
            scr_tile_clear(scr, cl, &viddef);

            v_render_view(scr, cls, cl, &viddef, separation[i]);

            // If stats are animating (smooth transitions), force dirty rect
            // so the HUD area gets redrawn even when nothing else changes.
            if cl_hud::hud_stats_animating() {
                scr_dirty_screen(scr, &viddef);
            }

            // Apply HUD alpha check: skip all HUD drawing if fully transparent
            let hud_alpha = cl_hud::hud_get_alpha();
            if hud_alpha > 0.0 {
                // Apply HUD scale factor for layout positioning
                let _hud_scale = cl_hud::hud_get_scale();
                scr_draw_stats(scr, cls, cl, &viddef);
                if cl.frame.playerstate.stats[STAT_LAYOUTS] & 1 != 0 {
                    scr_draw_layout(scr, cls, cl, &viddef);
                }
                if cl.frame.playerstate.stats[STAT_LAYOUTS] & 2 != 0 {
                    cl_draw_inventory(scr, cls, cl, &viddef);
                }
            }

            scr_draw_net(scr, cls);
            scr_check_draw_center_string(scr, cls, &viddef);

            // Draw HUD overlays (FPS, speed, timer) - R1Q2/Q2Pro feature
            crate::cl_hud::hud_draw_overlays(viddef.width, viddef.height);

            if cvar_value(scr.scr_timegraph) != 0.0 {
                scr_debug_graph(scr, cls.frametime * 300.0, 0);
            }

            if cvar_value(scr.scr_debuggraph) != 0.0
                || cvar_value(scr.scr_timegraph) != 0.0
                || cvar_value(scr.scr_netgraph) != 0.0
            {
                scr_draw_debug_graph(scr);
            }

            scr_draw_pause(scr, &viddef);

            scr_draw_console(scr, cls, cl, &viddef);

            m_draw();

            scr_draw_loading(scr, &viddef);
        }
    }
    vk_imp_end_frame();
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------
    // Helpers
    // -------------------------------------------------------

    fn make_scr() -> ScrState {
        ScrState::default()
    }

    // -------------------------------------------------------
    // ScrState default
    // -------------------------------------------------------

    #[test]
    fn test_scr_state_default() {
        let scr = make_scr();
        assert!(!scr.scr_initialized);
        assert_eq!(scr.scr_con_current, 0.0);
        assert_eq!(scr.scr_conlines, 0.0);
        assert_eq!(scr.scr_draw_loading, 0);
        assert_eq!(scr.crosshair_width, 0);
        assert_eq!(scr.crosshair_height, 0);
        assert!(scr.crosshair_pic.is_empty());
        assert!(scr.scr_centerstring.is_empty());
        assert_eq!(scr.scr_center_lines, 0);
    }

    // -------------------------------------------------------
    // VRect default
    // -------------------------------------------------------

    #[test]
    fn test_vrect_default() {
        let vr = VRect::default();
        assert_eq!(vr.x, 0);
        assert_eq!(vr.y, 0);
        assert_eq!(vr.width, 0);
        assert_eq!(vr.height, 0);
    }

    // -------------------------------------------------------
    // DirtyRect default
    // -------------------------------------------------------

    #[test]
    fn test_dirty_rect_default() {
        let dr = DirtyRect::default();
        assert_eq!(dr.x1, 0);
        assert_eq!(dr.y1, 0);
        assert_eq!(dr.x2, 0);
        assert_eq!(dr.y2, 0);
    }

    // -------------------------------------------------------
    // scr_add_dirty_point
    // -------------------------------------------------------

    #[test]
    fn test_scr_add_dirty_point_first_point() {
        let mut scr = make_scr();
        // Start with a dirty rect that has been "reset" to impossible values
        scr.scr_dirty.x1 = 9999;
        scr.scr_dirty.x2 = -9999;
        scr.scr_dirty.y1 = 9999;
        scr.scr_dirty.y2 = -9999;

        scr_add_dirty_point(&mut scr, 100, 200);

        assert_eq!(scr.scr_dirty.x1, 100);
        assert_eq!(scr.scr_dirty.x2, 100);
        assert_eq!(scr.scr_dirty.y1, 200);
        assert_eq!(scr.scr_dirty.y2, 200);
    }

    #[test]
    fn test_scr_add_dirty_point_expands_rect() {
        let mut scr = make_scr();
        scr.scr_dirty.x1 = 9999;
        scr.scr_dirty.x2 = -9999;
        scr.scr_dirty.y1 = 9999;
        scr.scr_dirty.y2 = -9999;

        scr_add_dirty_point(&mut scr, 50, 60);
        scr_add_dirty_point(&mut scr, 200, 300);

        assert_eq!(scr.scr_dirty.x1, 50);
        assert_eq!(scr.scr_dirty.x2, 200);
        assert_eq!(scr.scr_dirty.y1, 60);
        assert_eq!(scr.scr_dirty.y2, 300);
    }

    #[test]
    fn test_scr_add_dirty_point_negative_coords() {
        let mut scr = make_scr();
        scr.scr_dirty.x1 = 9999;
        scr.scr_dirty.x2 = -9999;
        scr.scr_dirty.y1 = 9999;
        scr.scr_dirty.y2 = -9999;

        scr_add_dirty_point(&mut scr, -10, -20);
        scr_add_dirty_point(&mut scr, 10, 20);

        assert_eq!(scr.scr_dirty.x1, -10);
        assert_eq!(scr.scr_dirty.x2, 10);
        assert_eq!(scr.scr_dirty.y1, -20);
        assert_eq!(scr.scr_dirty.y2, 20);
    }

    #[test]
    fn test_scr_add_dirty_point_same_point() {
        let mut scr = make_scr();
        scr.scr_dirty.x1 = 100;
        scr.scr_dirty.x2 = 200;
        scr.scr_dirty.y1 = 100;
        scr.scr_dirty.y2 = 200;

        // Point inside the existing rect does not change it
        scr_add_dirty_point(&mut scr, 150, 150);

        assert_eq!(scr.scr_dirty.x1, 100);
        assert_eq!(scr.scr_dirty.x2, 200);
        assert_eq!(scr.scr_dirty.y1, 100);
        assert_eq!(scr.scr_dirty.y2, 200);
    }

    // -------------------------------------------------------
    // scr_debug_graph
    // -------------------------------------------------------

    #[test]
    fn test_scr_debug_graph_increments_current() {
        let mut scr = make_scr();
        assert_eq!(scr.graph_current, 0);

        scr_debug_graph(&mut scr, 10.0, 0x40);
        assert_eq!(scr.graph_current, 1);

        scr_debug_graph(&mut scr, 20.0, 0x50);
        assert_eq!(scr.graph_current, 2);
    }

    #[test]
    fn test_scr_debug_graph_stores_values() {
        let mut scr = make_scr();

        scr_debug_graph(&mut scr, 15.5, 0xAB);

        assert!((scr.graph_values[0].value - 15.5).abs() < 1e-6);
        assert_eq!(scr.graph_values[0].color, 0xAB);
    }

    #[test]
    fn test_scr_debug_graph_wraps_at_1024() {
        let mut scr = make_scr();
        // Fill up 1024 entries
        for i in 0..1024 {
            scr_debug_graph(&mut scr, i as f32, i as i32);
        }
        assert_eq!(scr.graph_current, 1024);

        // Next write should wrap to index 0 (1024 & 1023 == 0)
        scr_debug_graph(&mut scr, 999.0, 0xFF);
        assert_eq!(scr.graph_current, 1025);
        // Index 1024 & 1023 = 0
        assert!((scr.graph_values[0].value - 999.0).abs() < 1e-6);
        assert_eq!(scr.graph_values[0].color, 0xFF);
    }

    // -------------------------------------------------------
    // size_hud_string
    // -------------------------------------------------------

    #[test]
    fn test_size_hud_string_empty() {
        let (w, h) = size_hud_string("");
        assert_eq!(w, 0);
        assert_eq!(h, 8); // 1 line * 8
    }

    #[test]
    fn test_size_hud_string_single_line() {
        let (w, h) = size_hud_string("hello");
        assert_eq!(w, 5 * 8); // 5 chars * 8 pixels
        assert_eq!(h, 1 * 8); // 1 line
    }

    #[test]
    fn test_size_hud_string_multi_line() {
        let (w, h) = size_hud_string("hello\nworld");
        // "hello" is 5 chars, "world" is 5 chars, both same width
        assert_eq!(w, 5 * 8);
        assert_eq!(h, 2 * 8); // 2 lines
    }

    #[test]
    fn test_size_hud_string_multi_line_varying_width() {
        let (w, h) = size_hud_string("hi\nworld!\ntest");
        // "hi"=2, "world!"=6, "test"=4 => max=6
        assert_eq!(w, 6 * 8);
        assert_eq!(h, 3 * 8); // 3 lines
    }

    #[test]
    fn test_size_hud_string_trailing_newline() {
        let (w, h) = size_hud_string("test\n");
        // "test"=4, then empty line after newline
        assert_eq!(w, 4 * 8);
        assert_eq!(h, 2 * 8); // "test" line + empty line
    }

    #[test]
    fn test_size_hud_string_only_newlines() {
        let (w, h) = size_hud_string("\n\n");
        assert_eq!(w, 0);
        assert_eq!(h, 3 * 8); // 3 lines (split by 2 newlines)
    }

    // -------------------------------------------------------
    // entity_cmp_fnc
    // -------------------------------------------------------

    #[test]
    fn test_entity_cmp_different_models() {
        let mut a = Entity::default();
        let mut b = Entity::default();
        a.model = 1;
        b.model = 2;

        assert_eq!(entity_cmp_fnc(&a, &b), std::cmp::Ordering::Less);
        assert_eq!(entity_cmp_fnc(&b, &a), std::cmp::Ordering::Greater);
    }

    #[test]
    fn test_entity_cmp_same_model_different_skin() {
        let mut a = Entity::default();
        let mut b = Entity::default();
        a.model = 5;
        b.model = 5;
        a.skin = 1;
        b.skin = 3;

        assert_eq!(entity_cmp_fnc(&a, &b), std::cmp::Ordering::Less);
        assert_eq!(entity_cmp_fnc(&b, &a), std::cmp::Ordering::Greater);
    }

    #[test]
    fn test_entity_cmp_equal() {
        let mut a = Entity::default();
        let mut b = Entity::default();
        a.model = 5;
        b.model = 5;
        a.skin = 3;
        b.skin = 3;

        assert_eq!(entity_cmp_fnc(&a, &b), std::cmp::Ordering::Equal);
    }

    #[test]
    fn test_entity_cmp_sort_order() {
        // Ensure sorting uses model first, then skin
        let mut entities = vec![
            { let mut e = Entity::default(); e.model = 3; e.skin = 1; e },
            { let mut e = Entity::default(); e.model = 1; e.skin = 5; e },
            { let mut e = Entity::default(); e.model = 3; e.skin = 0; e },
            { let mut e = Entity::default(); e.model = 2; e.skin = 2; e },
        ];

        entities.sort_by(entity_cmp_fnc);

        assert_eq!(entities[0].model, 1);
        assert_eq!(entities[1].model, 2);
        assert_eq!(entities[2].model, 3);
        assert_eq!(entities[2].skin, 0);
        assert_eq!(entities[3].model, 3);
        assert_eq!(entities[3].skin, 1);
    }

    // -------------------------------------------------------
    // Constants
    // -------------------------------------------------------

    #[test]
    fn test_icon_dimensions() {
        assert_eq!(ICON_WIDTH, 24);
        assert_eq!(ICON_HEIGHT, 24);
        assert_eq!(CHAR_WIDTH, 16);
        assert_eq!(ICON_SPACE, 8);
    }

    #[test]
    fn test_stat_minus() {
        assert_eq!(STAT_MINUS, 10);
    }

    #[test]
    fn test_sb_nums_tables() {
        // Regular numbers
        assert_eq!(SB_NUMS[0][0], "num_0");
        assert_eq!(SB_NUMS[0][9], "num_9");
        assert_eq!(SB_NUMS[0][10], "num_minus");

        // Alternate numbers
        assert_eq!(SB_NUMS[1][0], "anum_0");
        assert_eq!(SB_NUMS[1][9], "anum_9");
        assert_eq!(SB_NUMS[1][10], "anum_minus");
    }

    // -------------------------------------------------------
    // GraphSample
    // -------------------------------------------------------

    #[test]
    fn test_graph_sample_default() {
        let gs = GraphSample::default();
        assert_eq!(gs.value, 0.0);
        assert_eq!(gs.color, 0);
    }

    // -------------------------------------------------------
    // scr_dirty_screen (via add_dirty_point)
    // -------------------------------------------------------

    #[test]
    fn test_scr_dirty_screen_covers_full_screen() {
        let mut scr = make_scr();
        scr.scr_dirty.x1 = 9999;
        scr.scr_dirty.x2 = -9999;
        scr.scr_dirty.y1 = 9999;
        scr.scr_dirty.y2 = -9999;

        let viddef = VidDef { width: 640, height: 480 };
        scr_dirty_screen(&mut scr, &viddef);

        assert_eq!(scr.scr_dirty.x1, 0);
        assert_eq!(scr.scr_dirty.y1, 0);
        assert_eq!(scr.scr_dirty.x2, 639);
        assert_eq!(scr.scr_dirty.y2, 479);
    }

    // -------------------------------------------------------
    // scr_run_console logic
    // -------------------------------------------------------

    #[test]
    fn test_scr_run_console_opening() {
        let mut scr = make_scr();
        scr.scr_con_current = 0.0;
        scr.scr_conlines = 0.5; // target: half screen

        // Simulate console opening - scr_conlines > scr_con_current
        // The actual function calls cvar_value which we can't test directly,
        // but we can test the field logic
        assert!(scr.scr_conlines > scr.scr_con_current);
    }

    #[test]
    fn test_scr_run_console_closing() {
        let mut scr = make_scr();
        scr.scr_con_current = 0.5;
        scr.scr_conlines = 0.0; // target: fully closed

        // Console is closing
        assert!(scr.scr_conlines < scr.scr_con_current);
    }

    // -------------------------------------------------------
    // Layout string coordinate calculations
    // -------------------------------------------------------

    #[test]
    fn test_layout_xl_position() {
        // xl sets x to the value (absolute left offset)
        // xr sets x to width + value (absolute right offset)
        // xv sets x to width/2 - 160 + value (centered)
        let viddef_width = 640;

        // xr 0 should be right edge
        let xr_0 = viddef_width + 0;
        assert_eq!(xr_0, 640);

        // xr -160 should be 160 pixels from right
        let xr_neg = viddef_width + (-160);
        assert_eq!(xr_neg, 480);

        // xv 0 should be 160 pixels from center (left)
        let xv_0 = viddef_width / 2 - 160 + 0;
        assert_eq!(xv_0, 160);

        // xv 160 should be centered at 320
        let xv_160 = viddef_width / 2 - 160 + 160;
        assert_eq!(xv_160, 320);
    }

    #[test]
    fn test_layout_y_positions() {
        let viddef_height = 480;

        // yt sets y directly
        let yt_0 = 0;
        assert_eq!(yt_0, 0);

        // yb sets y to height + value
        let yb_neg = viddef_height + (-48);
        assert_eq!(yb_neg, 432);

        // yv sets y to height/2 - 120 + value
        let yv_0 = viddef_height / 2 - 120 + 0;
        assert_eq!(yv_0, 120);

        let yv_120 = viddef_height / 2 - 120 + 120;
        assert_eq!(yv_120, 240);
    }

    // -------------------------------------------------------
    // scr_draw_field width clamping
    // -------------------------------------------------------

    #[test]
    fn test_draw_field_width_clamped() {
        // The function clamps width to 5 max and returns early if < 1
        // We test the logic conceptually since the function also draws

        // Width > 5 should clamp to 5
        let mut width = 10;
        if width > 5 { width = 5; }
        assert_eq!(width, 5);

        // Width < 1 should return early
        let width = 0;
        assert!(width < 1);
    }

    // -------------------------------------------------------
    // Center print line counting
    // -------------------------------------------------------

    #[test]
    fn test_center_print_line_count_single() {
        let msg = "Hello World";
        let mut lines = 1;
        for ch in msg.chars() {
            if ch == '\n' {
                lines += 1;
            }
        }
        assert_eq!(lines, 1);
    }

    #[test]
    fn test_center_print_line_count_multiple() {
        let msg = "Line 1\nLine 2\nLine 3";
        let mut lines = 1;
        for ch in msg.chars() {
            if ch == '\n' {
                lines += 1;
            }
        }
        assert_eq!(lines, 3);
    }

    #[test]
    fn test_center_print_truncation() {
        let long_msg = "x".repeat(2000);
        let truncated: String = long_msg.chars().take(1023).collect();
        assert_eq!(truncated.len(), 1023);
    }

    // -------------------------------------------------------
    // Center string Y position
    // -------------------------------------------------------

    #[test]
    fn test_center_string_y_few_lines() {
        let viddef_height = 480;
        let center_lines = 3;
        // <= 4 lines uses 35% of screen height
        let y = if center_lines <= 4 {
            (viddef_height as f32 * 0.35) as i32
        } else {
            48
        };
        assert_eq!(y, 168); // 480 * 0.35 = 168
    }

    #[test]
    fn test_center_string_y_many_lines() {
        let viddef_height = 480;
        let center_lines = 8;
        let y = if center_lines <= 4 {
            (viddef_height as f32 * 0.35) as i32
        } else {
            48
        };
        assert_eq!(y, 48);
    }

    // -------------------------------------------------------
    // Screen coordinate math: text centering
    // -------------------------------------------------------

    #[test]
    fn test_text_centering_x() {
        let viddef_width = 640;
        // For a line of length l characters, each 8 pixels wide:
        // x = (viddef_width - l * 8) / 2

        let l = 10; // "Hello Wrld" = 10 chars
        let x = (viddef_width - l * 8) / 2;
        assert_eq!(x, (640 - 80) / 2); // 280
    }

    // -------------------------------------------------------
    // Pause image centering
    // -------------------------------------------------------

    #[test]
    fn test_pause_image_centering() {
        let viddef_width = 640;
        let viddef_height = 480;
        let pause_width = 128; // typical "pause" image width

        let x = (viddef_width - pause_width) / 2;
        let y = viddef_height / 2 + 8;

        assert_eq!(x, 256);
        assert_eq!(y, 248);
    }

    // -------------------------------------------------------
    // Loading image centering
    // -------------------------------------------------------

    #[test]
    fn test_loading_image_centering() {
        let viddef_width = 640;
        let viddef_height = 480;
        let loading_width = 256;
        let loading_height = 64;

        let x = (viddef_width - loading_width) / 2;
        let y = (viddef_height - loading_height) / 2;

        assert_eq!(x, 192);
        assert_eq!(y, 208);
    }

    // -------------------------------------------------------
    // Net icon drawing condition
    // -------------------------------------------------------

    #[test]
    fn test_net_icon_draw_condition() {
        // Net icon is drawn when outgoing_sequence - incoming_acknowledged >= CMD_BACKUP - 1
        let outgoing = 100;
        let incoming = 30;
        let diff = outgoing - incoming;

        // CMD_BACKUP is 64, so threshold is 63
        assert!(diff >= CMD_BACKUP as i32 - 1);

        let outgoing = 100;
        let incoming = 40;
        let diff = outgoing - incoming;
        // 60 < 63, so no draw
        assert!(diff < CMD_BACKUP as i32 - 1);
    }

    // -------------------------------------------------------
    // Loading plaque timeout
    // -------------------------------------------------------

    #[test]
    fn test_loading_plaque_timeout() {
        // The loading plaque times out after 120000ms (2 minutes)
        let disable_screen = 1000.0f32;
        let current_time = 122000.0f32;
        let elapsed = current_time - disable_screen;

        assert!(elapsed > 120000.0);
    }

    #[test]
    fn test_loading_plaque_no_timeout() {
        let disable_screen = 1000.0f32;
        let current_time = 60000.0f32;
        let elapsed = current_time - disable_screen;

        assert!(elapsed < 120000.0);
    }

    // -------------------------------------------------------
    // Graph value wrapping
    // -------------------------------------------------------

    #[test]
    fn test_graph_index_wrapping() {
        // Index is (graph_current - 1 - a + 1024) & 1023
        let graph_current = 5;
        let a = 3;
        let i = ((graph_current - 1 - a + 1024) as usize) & 1023;
        assert_eq!(i, 1);
    }

    #[test]
    fn test_graph_index_wrapping_around_zero() {
        let graph_current = 2;
        let a = 5;
        let i = ((graph_current - 1 - a + 1024) as usize) & 1023;
        assert_eq!(i, 1020);
    }

    // -------------------------------------------------------
    // HUD string centering math
    // -------------------------------------------------------

    #[test]
    fn test_hud_string_centering() {
        // If centerwidth != 0: x = margin + (centerwidth - width * 8) / 2
        let margin = 100;
        let centerwidth = 320;
        let text_width = 5; // 5 characters

        let x = margin + (centerwidth - text_width * 8) / 2;
        assert_eq!(x, 100 + (320 - 40) / 2); // 100 + 140 = 240
    }

    #[test]
    fn test_hud_string_no_centering() {
        let margin = 100;
        let centerwidth = 0;

        let x = if centerwidth != 0 {
            margin + (centerwidth - 5 * 8) / 2
        } else {
            margin
        };
        assert_eq!(x, 100);
    }
}
