// cl_inv.rs -- client inventory screen
// Converted from: myq2-original/client/cl_inv.c

use crate::client::*;
use crate::cl_scrn::*;
use crate::console::{
    draw_char, draw_pic, draw_string, keybindings, msg_read_short, VidDef,
};
use crate::keys::key_keynum_to_string;
use myq2_common::q_shared::*;

pub const DISPLAY_ITEMS: i32 = 17;

// ============================================================
// CL_ParseInventory
// ============================================================

pub fn cl_parse_inventory(cl: &mut ClientState) {
    for i in 0..MAX_ITEMS {
        cl.inventory[i] = msg_read_short();
    }
}

// ============================================================
// SetStringHighBit
// ============================================================

pub fn set_string_high_bit(s: &mut String) {
    // SAFETY: We need to set the high bit on each byte, matching the C behavior
    // of toggling bit 7 for alternate-color rendering in Quake 2.
    let bytes = unsafe { s.as_bytes_mut() };
    for b in bytes.iter_mut() {
        *b |= 128;
    }
}

// ============================================================
// CL_DrawInventory
// ============================================================

pub fn cl_draw_inventory(
    scr: &mut ScrState,
    cls: &ClientStatic,
    cl: &ClientState,
    viddef: &VidDef,
) {
    let selected = cl.frame.playerstate.stats[STAT_SELECTED_ITEM as usize] as i32;

    let mut num: i32 = 0;
    let mut selected_num: i32 = 0;
    let mut index = [0i32; MAX_ITEMS];

    for i in 0..MAX_ITEMS as i32 {
        if i == selected {
            selected_num = num;
        }
        if cl.inventory[i as usize] != 0 {
            index[num as usize] = i;
            num += 1;
        }
    }

    // determine scroll point
    let mut top = selected_num - DISPLAY_ITEMS / 2;
    if num - top < DISPLAY_ITEMS {
        top = num - DISPLAY_ITEMS;
    }
    if top < 0 {
        top = 0;
    }

    let mut x = (viddef.width - 256) / 2;
    let mut y = (viddef.height - 240) / 2;

    // repaint everything next frame
    scr_dirty_screen(scr, viddef);

    draw_pic(x, y + 8, "inventory");

    y += 24;
    x += 24;
    draw_string(x, y, "hotkey ### item");
    draw_string(x, y + 8, "------ --- ----");
    y += 16;

    let mut i = top;
    while i < num && i < top + DISPLAY_ITEMS {
        let item = index[i as usize];
        // search for a binding
        let binding = format!("use {}", cl.configstrings[CS_ITEMS + item as usize]);
        let mut bind = String::new();
        for j in 0..256 {
            if let Some(ref kb) = keybindings(j) {
                if kb.eq_ignore_ascii_case(&binding) {
                    bind = key_keynum_to_string(j);
                    break;
                }
            }
        }

        let mut string = format!(
            "{:>6} {:3} {}",
            bind,
            cl.inventory[item as usize],
            cl.configstrings[CS_ITEMS + item as usize]
        );

        if item != selected {
            set_string_high_bit(&mut string);
        } else {
            // draw a blinky cursor by the selected item
            if ((cls.realtime as f32 * 10.0) as i32) & 1 != 0 {
                draw_char(x - 8, y, 15);
            }
        }
        draw_string(x, y, &string);
        y += 8;
        i += 1;
    }
}
