// qmenu.rs — Menu widget system
// Converted from: myq2-original/client/qmenu.c + qmenu.h

#![allow(non_snake_case, non_upper_case_globals, unused)]

use myq2_common::q_shared::*;
use crate::keys::{
    K_TAB, K_ENTER, K_ESCAPE, K_SPACE, K_BACKSPACE,
    K_UPARROW, K_DOWNARROW, K_LEFTARROW, K_RIGHTARROW,
    K_KP_SLASH, K_KP_MINUS, K_KP_PLUS,
    K_KP_HOME, K_KP_UPARROW, K_KP_PGUP, K_KP_LEFTARROW, K_KP_5,
    K_KP_RIGHTARROW, K_KP_END, K_KP_DOWNARROW, K_KP_PGDN,
    K_KP_INS, K_KP_DEL, K_KP_ENTER,
    K_DEL, K_INS, K_CTRL, K_SHIFT,
};

// ============================================================
// Constants
// ============================================================

pub const MAXMENUITEMS: usize = 64;

pub const MTYPE_SLIDER: i32 = 0;
pub const MTYPE_LIST: i32 = 1;
pub const MTYPE_ACTION: i32 = 2;
pub const MTYPE_SPINCONTROL: i32 = 3;
pub const MTYPE_SEPARATOR: i32 = 4;
pub const MTYPE_FIELD: i32 = 5;

pub const QMF_LEFT_JUSTIFY: u32 = 0x00000001;
pub const QMF_GRAYED: u32 = 0x00000002;
pub const QMF_NUMBERSONLY: u32 = 0x00000004;

pub const RCOLUMN_OFFSET: i32 = 16;
pub const LCOLUMN_OFFSET: i32 = -16;

pub const SLIDER_RANGE: i32 = 10;

// ============================================================
// Structures
// ============================================================

pub type MenuCallback = Option<Box<dyn Fn(&mut dyn std::any::Any)>>;

#[derive(Default)]
pub struct MenuFramework {
    pub x: i32,
    pub y: i32,
    pub cursor: i32,
    pub nitems: i32,
    pub nslots: i32,
    pub items: Vec<usize>, // indices into a menu item storage
    pub statusbar: Option<String>,
    pub cursordraw: Option<Box<dyn Fn(&mut MenuFramework)>>,
}


#[derive(Default)]
pub struct MenuCommon {
    pub item_type: i32,
    pub name: Option<String>,
    pub x: i32,
    pub y: i32,
    pub parent_x: i32,
    pub parent_y: i32,
    pub cursor_offset: i32,
    pub localdata: [i32; 4],
    pub flags: u32,
    pub statusbar: Option<String>,
    pub callback: Option<Box<dyn Fn(usize)>>,
    pub statusbarfunc: Option<Box<dyn Fn(usize)>>,
    pub ownerdraw: Option<Box<dyn Fn(usize)>>,
    pub cursordraw: Option<Box<dyn Fn(usize)>>,
}


pub struct MenuField {
    pub generic: MenuCommon,
    pub buffer: String,
    pub cursor: i32,
    pub length: i32,
    pub visible_length: i32,
    pub visible_offset: i32,
}

impl Default for MenuField {
    fn default() -> Self {
        Self {
            generic: MenuCommon { item_type: MTYPE_FIELD, ..Default::default() },
            buffer: String::new(),
            cursor: 0,
            length: 0,
            visible_length: 0,
            visible_offset: 0,
        }
    }
}

pub struct MenuSlider {
    pub generic: MenuCommon,
    pub minvalue: f32,
    pub maxvalue: f32,
    pub curvalue: f32,
    pub range: f32,
}

impl Default for MenuSlider {
    fn default() -> Self {
        Self {
            generic: MenuCommon { item_type: MTYPE_SLIDER, ..Default::default() },
            minvalue: 0.0,
            maxvalue: 0.0,
            curvalue: 0.0,
            range: 0.0,
        }
    }
}

pub struct MenuList {
    pub generic: MenuCommon,
    pub curvalue: i32,
    pub itemnames: Vec<String>,
}

impl Default for MenuList {
    fn default() -> Self {
        Self {
            generic: MenuCommon { item_type: MTYPE_LIST, ..Default::default() },
            curvalue: 0,
            itemnames: Vec::new(),
        }
    }
}

pub struct MenuAction {
    pub generic: MenuCommon,
}

impl Default for MenuAction {
    fn default() -> Self {
        Self {
            generic: MenuCommon { item_type: MTYPE_ACTION, ..Default::default() },
        }
    }
}

pub struct MenuSeparator {
    pub generic: MenuCommon,
}

impl Default for MenuSeparator {
    fn default() -> Self {
        Self {
            generic: MenuCommon { item_type: MTYPE_SEPARATOR, ..Default::default() },
        }
    }
}

/// Enum to hold any menu item type
pub enum MenuItem {
    Action(MenuAction),
    Field(MenuField),
    Slider(MenuSlider),
    List(MenuList),
    Separator(MenuSeparator),
    SpinControl(MenuList),
}

impl MenuItem {
    pub fn generic(&self) -> &MenuCommon {
        match self {
            MenuItem::Action(a) => &a.generic,
            MenuItem::Field(f) => &f.generic,
            MenuItem::Slider(s) => &s.generic,
            MenuItem::List(l) => &l.generic,
            MenuItem::Separator(s) => &s.generic,
            MenuItem::SpinControl(s) => &s.generic,
        }
    }

    pub fn generic_mut(&mut self) -> &mut MenuCommon {
        match self {
            MenuItem::Action(a) => &mut a.generic,
            MenuItem::Field(f) => &mut f.generic,
            MenuItem::Slider(s) => &mut s.generic,
            MenuItem::List(l) => &mut l.generic,
            MenuItem::Separator(s) => &mut s.generic,
            MenuItem::SpinControl(s) => &mut s.generic,
        }
    }

    pub fn item_type(&self) -> i32 {
        self.generic().item_type
    }
}

// ============================================================
// Rendering callback traits (to be provided by the renderer)
// ============================================================

/// Abstraction for drawing operations needed by the menu system.
pub trait MenuRenderer {
    fn draw_char(&mut self, x: i32, y: i32, ch: i32);
    fn draw_fill(&mut self, x: i32, y: i32, w: i32, h: i32, color: i32, alpha: f32);
    fn sys_milliseconds(&self) -> i32;
    fn vid_width(&self) -> i32;
    fn vid_height(&self) -> i32;
    fn sys_get_clipboard_data(&self) -> Option<String>;
    fn keydown(&self, key: i32) -> bool;
}

// ============================================================
// Action functions
// ============================================================

pub fn action_do_enter(action: &MenuAction) {
    if let Some(ref cb) = action.generic.callback {
        // callback with item index would be invoked here
    }
}

pub fn action_draw(renderer: &mut dyn MenuRenderer, action: &MenuAction, parent_x: i32, parent_y: i32) {
    let name = match &action.generic.name {
        Some(n) => n.clone(),
        None => return,
    };

    if action.generic.flags & QMF_LEFT_JUSTIFY != 0 {
        if action.generic.flags & QMF_GRAYED != 0 {
            menu_draw_string_dark(renderer,
                action.generic.x + parent_x + LCOLUMN_OFFSET,
                action.generic.y + parent_y,
                &name);
        } else {
            menu_draw_string(renderer,
                action.generic.x + parent_x + LCOLUMN_OFFSET,
                action.generic.y + parent_y,
                &name);
        }
    } else if action.generic.flags & QMF_GRAYED != 0 {
        menu_draw_string_r2l_dark(renderer,
            action.generic.x + parent_x + LCOLUMN_OFFSET,
            action.generic.y + parent_y,
            &name);
    } else {
        menu_draw_string_r2l(renderer,
            action.generic.x + parent_x + LCOLUMN_OFFSET,
            action.generic.y + parent_y,
            &name);
    }
}

// ============================================================
// Field functions
// ============================================================

pub fn field_do_enter(field: &MenuField) -> bool {
    if field.generic.callback.is_some() {
        // invoke callback
        return true;
    }
    false
}

pub fn field_draw(renderer: &mut dyn MenuRenderer, field: &MenuField, parent_x: i32, parent_y: i32, is_at_cursor: bool) {
    let x = field.generic.x + parent_x;
    let y = field.generic.y + parent_y;

    if let Some(ref name) = field.generic.name {
        menu_draw_string_r2l_dark(renderer, x + LCOLUMN_OFFSET, y, name);
    }

    let visible: String = field.buffer.chars()
        .skip(field.visible_offset as usize)
        .take(field.visible_length as usize)
        .collect();

    renderer.draw_char(x + 16, y - 4, 18);
    renderer.draw_char(x + 16, y + 4, 24);

    renderer.draw_char(x + 24 + field.visible_length * 8, y - 4, 20);
    renderer.draw_char(x + 24 + field.visible_length * 8, y + 4, 26);

    for i in 0..field.visible_length {
        renderer.draw_char(x + 24 + i * 8, y - 4, 19);
        renderer.draw_char(x + 24 + i * 8, y + 4, 25);
    }

    menu_draw_string(renderer, x + 24, y, &visible);

    if is_at_cursor {
        let offset = if field.visible_offset != 0 {
            field.visible_length
        } else {
            field.cursor
        };

        if (renderer.sys_milliseconds() / 250) & 1 != 0 {
            renderer.draw_char(x + (offset + 2) * 8 + 8, y, 11);
        } else {
            renderer.draw_char(x + (offset + 2) * 8 + 8, y, ' ' as i32);
        }
    }
}

pub fn field_key(field: &mut MenuField, key: i32, renderer: &dyn MenuRenderer) -> bool {
    let mut key = key;

    // Remap keypad keys
    match key {
        K_KP_SLASH => key = '/' as i32,
        K_KP_MINUS => key = '-' as i32,
        K_KP_PLUS => key = '+' as i32,
        K_KP_HOME => key = '7' as i32,
        K_KP_UPARROW => key = '8' as i32,
        K_KP_PGUP => key = '9' as i32,
        K_KP_LEFTARROW => key = '4' as i32,
        K_KP_5 => key = '5' as i32,
        K_KP_RIGHTARROW => key = '6' as i32,
        K_KP_END => key = '1' as i32,
        K_KP_DOWNARROW => key = '2' as i32,
        K_KP_PGDN => key = '3' as i32,
        K_KP_INS => key = '0' as i32,
        K_KP_DEL => key = '.' as i32,
        _ => {}
    }

    if key > 127 {
        return false;
    }

    // Support pasting from the clipboard
    let upper = (key as u8 as char).to_ascii_uppercase();
    if (upper == 'V' && renderer.keydown(K_CTRL))
        || ((key == K_INS || key == K_KP_INS) && renderer.keydown(K_SHIFT))
    {
        if let Some(cbd) = renderer.sys_get_clipboard_data() {
            let cbd: String = cbd.chars().take_while(|c| *c != '\n' && *c != '\r').collect();
            let len = (field.length - 1) as usize;
            field.buffer = cbd.chars().take(len).collect();
            field.cursor = field.buffer.len() as i32;
            field.visible_offset = field.cursor - field.visible_length;
            if field.visible_offset < 0 {
                field.visible_offset = 0;
            }
        }
        return true;
    }

    match key {
        K_KP_LEFTARROW | K_LEFTARROW | K_BACKSPACE => {
            if field.cursor > 0 {
                field.cursor -= 1;
                let cursor = field.cursor as usize;
                field.buffer.remove(cursor);
                if field.visible_offset > 0 {
                    field.visible_offset -= 1;
                }
            }
        }
        K_KP_DEL | K_DEL => {
            let cursor = field.cursor as usize;
            if cursor < field.buffer.len() {
                field.buffer.remove(cursor);
            }
        }
        K_KP_ENTER | K_ENTER | K_ESCAPE | K_TAB => {
            return false;
        }
        _ => {
            // K_SPACE / default
            let ch = key as u8 as char;
            if !ch.is_ascii_digit() && (field.generic.flags & QMF_NUMBERSONLY != 0) {
                return false;
            }

            if field.cursor < field.length {
                let cursor = field.cursor as usize;
                field.buffer.insert(cursor, ch);
                field.cursor += 1;

                if field.cursor > field.visible_length {
                    field.visible_offset += 1;
                }
            }
        }
    }

    true
}

// ============================================================
// Menu framework functions
// ============================================================

pub fn menu_add_item(menu: &mut MenuFramework, items: &mut Vec<MenuItem>, item: MenuItem) {
    if menu.nitems == 0 {
        menu.nslots = 0;
    }

    if (menu.nitems as usize) < MAXMENUITEMS {
        let idx = items.len();
        items.push(item);
        menu.items.push(idx);
        menu.nitems += 1;
    }

    menu.nslots = menu_tally_slots(menu, items);
}

/// Adjust cursor to next valid (non-separator) slot.
pub fn menu_adjust_cursor(menu: &mut MenuFramework, items: &[MenuItem], dir: i32) {
    // See if it's in a valid spot
    if menu.cursor >= 0 && menu.cursor < menu.nitems {
        if let Some(idx) = menu.items.get(menu.cursor as usize) {
            if let Some(item) = items.get(*idx) {
                if item.item_type() != MTYPE_SEPARATOR {
                    return;
                }
            }
        }
    }

    // Crawl in the given direction
    if dir == 1 {
        loop {
            if let Some(idx) = menu.items.get(menu.cursor as usize) {
                if let Some(item) = items.get(*idx) {
                    if item.item_type() != MTYPE_SEPARATOR {
                        break;
                    }
                }
            }
            menu.cursor += dir;
            if menu.cursor >= menu.nitems {
                menu.cursor = 0;
            }
        }
    } else {
        loop {
            if let Some(idx) = menu.items.get(menu.cursor as usize) {
                if let Some(item) = items.get(*idx) {
                    if item.item_type() != MTYPE_SEPARATOR {
                        break;
                    }
                }
            }
            menu.cursor += dir;
            if menu.cursor < 0 {
                menu.cursor = menu.nitems - 1;
            }
        }
    }
}

pub fn menu_center(menu: &mut MenuFramework, items: &[MenuItem], vid_height: i32) {
    if menu.nitems <= 0 {
        return;
    }
    let last_idx = menu.items[(menu.nitems - 1) as usize];
    let height = items[last_idx].generic().y + 10;
    menu.y = (vid_height - height) / 2;
}

pub fn menu_draw(renderer: &mut dyn MenuRenderer, menu: &MenuFramework, items: &mut [MenuItem]) {
    let parent_x = menu.x;
    let parent_y = menu.y;

    // Draw contents
    for i in 0..menu.nitems as usize {
        let idx = menu.items[i];
        match &items[idx] {
            MenuItem::Field(f) => {
                let is_at_cursor = menu.cursor as usize == i;
                field_draw(renderer, f, parent_x, parent_y, is_at_cursor);
            }
            MenuItem::Slider(s) => {
                slider_draw(renderer, s, parent_x, parent_y);
            }
            MenuItem::List(l) => {
                menulist_draw(renderer, l, parent_x, parent_y);
            }
            MenuItem::SpinControl(s) => {
                spincontrol_draw(renderer, s, parent_x, parent_y);
            }
            MenuItem::Action(a) => {
                action_draw(renderer, a, parent_x, parent_y);
            }
            MenuItem::Separator(s) => {
                separator_draw(renderer, s, parent_x, parent_y);
            }
        }
    }

    // Draw cursor
    let cursor_idx = if menu.cursor >= 0 && (menu.cursor as usize) < menu.items.len() {
        Some(menu.items[menu.cursor as usize])
    } else {
        None
    };

    if let Some(idx) = cursor_idx {
        let item = &items[idx];
        let g = item.generic();

        if g.cursordraw.is_some() {
            // item cursordraw callback
        } else if menu.cursordraw.is_some() {
            // menu cursordraw callback
        } else if g.item_type != MTYPE_FIELD {
            let blink = 12 + ((renderer.sys_milliseconds() / 250) & 1);
            if g.flags & QMF_LEFT_JUSTIFY != 0 {
                renderer.draw_char(parent_x + g.x - 24 + g.cursor_offset, parent_y + g.y, blink);
            } else {
                renderer.draw_char(parent_x + g.cursor_offset, parent_y + g.y, blink);
            }
        }

        // Status bar
        if g.statusbarfunc.is_some() {
            // statusbar callback
        } else if let Some(ref sb) = g.statusbar {
            menu_draw_status_bar(renderer, Some(sb));
        } else {
            menu_draw_status_bar(renderer, menu.statusbar.as_deref());
        }
    } else {
        menu_draw_status_bar(renderer, menu.statusbar.as_deref());
    }
}

pub fn menu_draw_status_bar(renderer: &mut dyn MenuRenderer, string: Option<&str>) {
    let vid_width = renderer.vid_width();
    let vid_height = renderer.vid_height();

    if let Some(s) = string {
        let l = s.len() as i32;
        let maxcol = vid_width / 8;
        let col = maxcol / 2 - l / 2;

        renderer.draw_fill(0, vid_height - 8, vid_width, 8, 4, 1.0);
        menu_draw_string(renderer, col * 8, vid_height - 8, s);
    } else {
        renderer.draw_fill(0, vid_height - 8, vid_width, 8, 0, 1.0);
    }
}

pub fn menu_draw_string(renderer: &mut dyn MenuRenderer, x: i32, y: i32, string: &str) {
    for (i, ch) in string.bytes().enumerate() {
        renderer.draw_char(x + i as i32 * 8, y, ch as i32);
    }
}

pub fn menu_draw_string_dark(renderer: &mut dyn MenuRenderer, x: i32, y: i32, string: &str) {
    for (i, ch) in string.bytes().enumerate() {
        renderer.draw_char(x + i as i32 * 8, y, ch as i32 + 128);
    }
}

pub fn menu_draw_string_r2l(renderer: &mut dyn MenuRenderer, x: i32, y: i32, string: &str) {
    let bytes: Vec<u8> = string.bytes().collect();
    let len = bytes.len();
    for i in 0..len {
        renderer.draw_char(x - i as i32 * 8, y, bytes[len - i - 1] as i32);
    }
}

pub fn menu_draw_string_r2l_dark(renderer: &mut dyn MenuRenderer, x: i32, y: i32, string: &str) {
    let bytes: Vec<u8> = string.bytes().collect();
    let len = bytes.len();
    for i in 0..len {
        renderer.draw_char(x - i as i32 * 8, y, bytes[len - i - 1] as i32 + 128);
    }
}

pub fn menu_item_at_cursor<'a>(menu: &MenuFramework, items: &'a [MenuItem]) -> Option<&'a MenuItem> {
    if menu.cursor < 0 || menu.cursor >= menu.nitems {
        return None;
    }
    let idx = menu.items[menu.cursor as usize];
    items.get(idx)
}

pub fn menu_select_item(menu: &MenuFramework, items: &[MenuItem]) -> bool {
    if let Some(item) = menu_item_at_cursor(menu, items) {
        match item {
            MenuItem::Field(f) => return field_do_enter(f),
            MenuItem::Action(a) => {
                action_do_enter(a);
                return true;
            }
            MenuItem::List(_) => return false,
            MenuItem::SpinControl(_) => return false,
            _ => {}
        }
    }
    false
}

pub fn menu_set_status_bar(menu: &mut MenuFramework, string: Option<String>) {
    menu.statusbar = string;
}

pub fn menu_slide_item(menu: &MenuFramework, items: &mut [MenuItem], dir: i32) {
    if menu.cursor < 0 || menu.cursor >= menu.nitems {
        return;
    }
    let idx = menu.items[menu.cursor as usize];
    match &mut items[idx] {
        MenuItem::Slider(s) => slider_do_slide(s, dir),
        MenuItem::SpinControl(s) => spincontrol_do_slide(s, dir),
        _ => {}
    }
}

pub fn menu_tally_slots(menu: &MenuFramework, items: &[MenuItem]) -> i32 {
    let mut total = 0i32;

    for i in 0..menu.nitems as usize {
        let idx = menu.items[i];
        match &items[idx] {
            MenuItem::List(l) => {
                total += l.itemnames.len() as i32;
            }
            _ => {
                total += 1;
            }
        }
    }

    total
}

// ============================================================
// Menulist functions
// ============================================================

pub fn menulist_do_enter(list: &mut MenuList, parent_cursor: i32) {
    let start = list.generic.y / 10 + 1;
    list.curvalue = parent_cursor - start;
    // callback would be invoked here
}

pub fn menulist_draw(renderer: &mut dyn MenuRenderer, list: &MenuList, parent_x: i32, parent_y: i32) {
    if let Some(ref name) = list.generic.name {
        menu_draw_string_r2l_dark(renderer,
            list.generic.x + parent_x + LCOLUMN_OFFSET,
            list.generic.y + parent_y,
            name);
    }

    renderer.draw_fill(
        list.generic.x - 112 + parent_x,
        parent_y + list.generic.y + list.curvalue * 10 + 10,
        128, 10, 16, 1.0,
    );

    let mut y = 0;
    for itemname in &list.itemnames {
        menu_draw_string_r2l_dark(renderer,
            list.generic.x + parent_x + LCOLUMN_OFFSET,
            list.generic.y + parent_y + y + 10,
            itemname);
        y += 10;
    }
}

// ============================================================
// Separator functions
// ============================================================

pub fn separator_draw(renderer: &mut dyn MenuRenderer, sep: &MenuSeparator, parent_x: i32, parent_y: i32) {
    if let Some(ref name) = sep.generic.name {
        menu_draw_string_r2l_dark(renderer,
            sep.generic.x + parent_x,
            sep.generic.y + parent_y,
            name);
    }
}

// ============================================================
// Slider functions
// ============================================================

pub fn slider_do_slide(slider: &mut MenuSlider, dir: i32) {
    slider.curvalue += dir as f32;

    if slider.curvalue > slider.maxvalue {
        slider.curvalue = slider.maxvalue;
    } else if slider.curvalue < slider.minvalue {
        slider.curvalue = slider.minvalue;
    }

    // callback would be invoked here
}

pub fn slider_draw(renderer: &mut dyn MenuRenderer, slider: &MenuSlider, parent_x: i32, parent_y: i32) {
    if let Some(ref name) = slider.generic.name {
        menu_draw_string_r2l_dark(renderer,
            slider.generic.x + parent_x + LCOLUMN_OFFSET,
            slider.generic.y + parent_y,
            name);
    }

    let range = if slider.maxvalue != slider.minvalue {
        let r = (slider.curvalue - slider.minvalue) / (slider.maxvalue - slider.minvalue);
        r.clamp(0.0, 1.0)
    } else {
        0.0
    };

    let x = slider.generic.x + parent_x;
    let y = slider.generic.y + parent_y;

    renderer.draw_char(x + RCOLUMN_OFFSET, y, 128);
    for i in 0..SLIDER_RANGE {
        renderer.draw_char(RCOLUMN_OFFSET + x + i * 8 + 8, y, 129);
    }
    renderer.draw_char(RCOLUMN_OFFSET + x + SLIDER_RANGE * 8 + 8, y, 130);
    renderer.draw_char(
        (8.0 + RCOLUMN_OFFSET as f32 + parent_x as f32 + slider.generic.x as f32
            + (SLIDER_RANGE - 1) as f32 * 8.0 * range) as i32,
        y, 131,
    );
}

// ============================================================
// SpinControl functions
// ============================================================

pub fn spincontrol_do_enter(spin: &mut MenuList) {
    spin.curvalue += 1;
    if spin.curvalue >= spin.itemnames.len() as i32 {
        spin.curvalue = 0;
    }
    // callback would be invoked here
}

pub fn spincontrol_do_slide(spin: &mut MenuList, dir: i32) {
    spin.curvalue += dir;

    if spin.curvalue < 0 {
        spin.curvalue = 0;
    } else if spin.curvalue >= spin.itemnames.len() as i32 {
        spin.curvalue -= 1;
    }

    // callback would be invoked here
}

pub fn spincontrol_draw(renderer: &mut dyn MenuRenderer, spin: &MenuList, parent_x: i32, parent_y: i32) {
    if let Some(ref name) = spin.generic.name {
        menu_draw_string_r2l_dark(renderer,
            spin.generic.x + parent_x + LCOLUMN_OFFSET,
            spin.generic.y + parent_y,
            name);
    }

    if spin.curvalue < 0 || spin.curvalue >= spin.itemnames.len() as i32 {
        return;
    }

    let current = &spin.itemnames[spin.curvalue as usize];

    if !current.contains('\n') {
        menu_draw_string(renderer,
            RCOLUMN_OFFSET + spin.generic.x + parent_x,
            spin.generic.y + parent_y,
            current);
    } else {
        let parts: Vec<&str> = current.splitn(2, '\n').collect();
        menu_draw_string(renderer,
            RCOLUMN_OFFSET + spin.generic.x + parent_x,
            spin.generic.y + parent_y,
            parts[0]);
        if parts.len() > 1 {
            menu_draw_string(renderer,
                RCOLUMN_OFFSET + spin.generic.x + parent_x,
                spin.generic.y + parent_y + 10,
                parts[1]);
        }
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------
    // Mock renderer for testing drawing functions
    // ----------------------------------------------------------
    struct MockRenderer {
        chars: Vec<(i32, i32, i32)>,
        fills: Vec<(i32, i32, i32, i32, i32)>,
        milliseconds: i32,
        width: i32,
        height: i32,
        clipboard: Option<String>,
        keys_down: [bool; 256],
    }

    impl MockRenderer {
        fn new() -> Self {
            Self {
                chars: Vec::new(),
                fills: Vec::new(),
                milliseconds: 0,
                width: 640,
                height: 480,
                clipboard: None,
                keys_down: [false; 256],
            }
        }
    }

    impl MenuRenderer for MockRenderer {
        fn draw_char(&mut self, x: i32, y: i32, ch: i32) {
            self.chars.push((x, y, ch));
        }
        fn draw_fill(&mut self, x: i32, y: i32, w: i32, h: i32, color: i32, _alpha: f32) {
            self.fills.push((x, y, w, h, color));
        }
        fn sys_milliseconds(&self) -> i32 { self.milliseconds }
        fn vid_width(&self) -> i32 { self.width }
        fn vid_height(&self) -> i32 { self.height }
        fn sys_get_clipboard_data(&self) -> Option<String> { self.clipboard.clone() }
        fn keydown(&self, key: i32) -> bool {
            if key >= 0 && key < 256 { self.keys_down[key as usize] } else { false }
        }
    }

    // ----------------------------------------------------------
    // Widget default creation tests
    // ----------------------------------------------------------

    #[test]
    fn test_menu_field_default_type() {
        let f = MenuField::default();
        assert_eq!(f.generic.item_type, MTYPE_FIELD);
        assert_eq!(f.cursor, 0);
        assert!(f.buffer.is_empty());
        assert_eq!(f.length, 0);
        assert_eq!(f.visible_length, 0);
        assert_eq!(f.visible_offset, 0);
    }

    #[test]
    fn test_menu_slider_default_type() {
        let s = MenuSlider::default();
        assert_eq!(s.generic.item_type, MTYPE_SLIDER);
        assert_eq!(s.minvalue, 0.0);
        assert_eq!(s.maxvalue, 0.0);
        assert_eq!(s.curvalue, 0.0);
        assert_eq!(s.range, 0.0);
    }

    #[test]
    fn test_menu_list_default_type() {
        let l = MenuList::default();
        assert_eq!(l.generic.item_type, MTYPE_LIST);
        assert_eq!(l.curvalue, 0);
        assert!(l.itemnames.is_empty());
    }

    #[test]
    fn test_menu_action_default_type() {
        let a = MenuAction::default();
        assert_eq!(a.generic.item_type, MTYPE_ACTION);
    }

    #[test]
    fn test_menu_separator_default_type() {
        let s = MenuSeparator::default();
        assert_eq!(s.generic.item_type, MTYPE_SEPARATOR);
    }

    #[test]
    fn test_menu_common_default() {
        let c = MenuCommon::default();
        assert_eq!(c.item_type, 0);
        assert!(c.name.is_none());
        assert_eq!(c.x, 0);
        assert_eq!(c.y, 0);
        assert_eq!(c.flags, 0);
        assert_eq!(c.localdata, [0, 0, 0, 0]);
        assert!(c.callback.is_none());
        assert!(c.statusbar.is_none());
    }

    // ----------------------------------------------------------
    // MenuItem enum tests
    // ----------------------------------------------------------

    #[test]
    fn test_menuitem_generic_access() {
        let item = MenuItem::Action(MenuAction {
            generic: MenuCommon {
                item_type: MTYPE_ACTION,
                name: Some("test".to_string()),
                x: 10,
                y: 20,
                ..Default::default()
            },
        });
        assert_eq!(item.generic().item_type, MTYPE_ACTION);
        assert_eq!(item.generic().name.as_deref(), Some("test"));
        assert_eq!(item.generic().x, 10);
        assert_eq!(item.generic().y, 20);
    }

    #[test]
    fn test_menuitem_generic_mut_access() {
        let mut item = MenuItem::Slider(MenuSlider::default());
        item.generic_mut().x = 42;
        item.generic_mut().name = Some("volume".to_string());
        assert_eq!(item.generic().x, 42);
        assert_eq!(item.generic().name.as_deref(), Some("volume"));
    }

    #[test]
    fn test_menuitem_item_type() {
        assert_eq!(MenuItem::Action(MenuAction::default()).item_type(), MTYPE_ACTION);
        assert_eq!(MenuItem::Slider(MenuSlider::default()).item_type(), MTYPE_SLIDER);
        assert_eq!(MenuItem::Field(MenuField::default()).item_type(), MTYPE_FIELD);
        assert_eq!(MenuItem::List(MenuList::default()).item_type(), MTYPE_LIST);
        assert_eq!(MenuItem::Separator(MenuSeparator::default()).item_type(), MTYPE_SEPARATOR);
        assert_eq!(MenuItem::SpinControl(MenuList {
            generic: MenuCommon { item_type: MTYPE_SPINCONTROL, ..Default::default() },
            ..Default::default()
        }).item_type(), MTYPE_SPINCONTROL);
    }

    // ----------------------------------------------------------
    // Slider tests
    // ----------------------------------------------------------

    #[test]
    fn test_slider_do_slide_increase() {
        let mut slider = MenuSlider {
            minvalue: 0.0,
            maxvalue: 10.0,
            curvalue: 5.0,
            ..Default::default()
        };
        slider_do_slide(&mut slider, 1);
        assert_eq!(slider.curvalue, 6.0);
    }

    #[test]
    fn test_slider_do_slide_decrease() {
        let mut slider = MenuSlider {
            minvalue: 0.0,
            maxvalue: 10.0,
            curvalue: 5.0,
            ..Default::default()
        };
        slider_do_slide(&mut slider, -1);
        assert_eq!(slider.curvalue, 4.0);
    }

    #[test]
    fn test_slider_do_slide_clamps_max() {
        let mut slider = MenuSlider {
            minvalue: 0.0,
            maxvalue: 10.0,
            curvalue: 10.0,
            ..Default::default()
        };
        slider_do_slide(&mut slider, 1);
        assert_eq!(slider.curvalue, 10.0);
    }

    #[test]
    fn test_slider_do_slide_clamps_min() {
        let mut slider = MenuSlider {
            minvalue: 0.0,
            maxvalue: 10.0,
            curvalue: 0.0,
            ..Default::default()
        };
        slider_do_slide(&mut slider, -1);
        assert_eq!(slider.curvalue, 0.0);
    }

    #[test]
    fn test_slider_do_slide_multiple_steps() {
        let mut slider = MenuSlider {
            minvalue: 0.0,
            maxvalue: 10.0,
            curvalue: 0.0,
            ..Default::default()
        };
        for _ in 0..15 {
            slider_do_slide(&mut slider, 1);
        }
        assert_eq!(slider.curvalue, 10.0);
    }

    #[test]
    fn test_slider_do_slide_negative_range() {
        let mut slider = MenuSlider {
            minvalue: -5.0,
            maxvalue: 5.0,
            curvalue: 0.0,
            ..Default::default()
        };
        slider_do_slide(&mut slider, -1);
        assert_eq!(slider.curvalue, -1.0);
        for _ in 0..10 {
            slider_do_slide(&mut slider, -1);
        }
        assert_eq!(slider.curvalue, -5.0);
    }

    // ----------------------------------------------------------
    // SpinControl tests
    // ----------------------------------------------------------

    #[test]
    fn test_spincontrol_do_enter_cycles() {
        let mut spin = MenuList {
            generic: MenuCommon { item_type: MTYPE_SPINCONTROL, ..Default::default() },
            curvalue: 0,
            itemnames: vec!["a".into(), "b".into(), "c".into()],
        };
        spincontrol_do_enter(&mut spin);
        assert_eq!(spin.curvalue, 1);
        spincontrol_do_enter(&mut spin);
        assert_eq!(spin.curvalue, 2);
        spincontrol_do_enter(&mut spin);
        assert_eq!(spin.curvalue, 0); // wraps around
    }

    #[test]
    fn test_spincontrol_do_enter_single_item() {
        let mut spin = MenuList {
            generic: MenuCommon { item_type: MTYPE_SPINCONTROL, ..Default::default() },
            curvalue: 0,
            itemnames: vec!["only".into()],
        };
        spincontrol_do_enter(&mut spin);
        assert_eq!(spin.curvalue, 0); // wraps back immediately
    }

    #[test]
    fn test_spincontrol_do_slide_forward() {
        let mut spin = MenuList {
            generic: MenuCommon { item_type: MTYPE_SPINCONTROL, ..Default::default() },
            curvalue: 0,
            itemnames: vec!["a".into(), "b".into(), "c".into()],
        };
        spincontrol_do_slide(&mut spin, 1);
        assert_eq!(spin.curvalue, 1);
        spincontrol_do_slide(&mut spin, 1);
        assert_eq!(spin.curvalue, 2);
        // At end, should not advance further
        spincontrol_do_slide(&mut spin, 1);
        assert_eq!(spin.curvalue, 2);
    }

    #[test]
    fn test_spincontrol_do_slide_backward() {
        let mut spin = MenuList {
            generic: MenuCommon { item_type: MTYPE_SPINCONTROL, ..Default::default() },
            curvalue: 2,
            itemnames: vec!["a".into(), "b".into(), "c".into()],
        };
        spincontrol_do_slide(&mut spin, -1);
        assert_eq!(spin.curvalue, 1);
        spincontrol_do_slide(&mut spin, -1);
        assert_eq!(spin.curvalue, 0);
        // At beginning, should clamp to 0
        spincontrol_do_slide(&mut spin, -1);
        assert_eq!(spin.curvalue, 0);
    }

    #[test]
    fn test_spincontrol_do_slide_clamps_at_boundaries() {
        let mut spin = MenuList {
            generic: MenuCommon { item_type: MTYPE_SPINCONTROL, ..Default::default() },
            curvalue: 0,
            itemnames: vec!["x".into(), "y".into()],
        };
        spincontrol_do_slide(&mut spin, -1);
        assert_eq!(spin.curvalue, 0);
        spin.curvalue = 1;
        spincontrol_do_slide(&mut spin, 1);
        assert_eq!(spin.curvalue, 1);
    }

    // ----------------------------------------------------------
    // MenuList (do_enter) tests
    // ----------------------------------------------------------

    #[test]
    fn test_menulist_do_enter_sets_curvalue() {
        let mut list = MenuList {
            generic: MenuCommon {
                item_type: MTYPE_LIST,
                y: 50,
                ..Default::default()
            },
            curvalue: 0,
            itemnames: vec!["item0".into(), "item1".into(), "item2".into()],
        };
        // parent_cursor represents the cursor position in the menu framework;
        // curvalue = parent_cursor - (list.generic.y / 10 + 1) = parent_cursor - 6
        menulist_do_enter(&mut list, 7);
        assert_eq!(list.curvalue, 1);

        menulist_do_enter(&mut list, 6);
        assert_eq!(list.curvalue, 0);
    }

    // ----------------------------------------------------------
    // Field key input tests
    // ----------------------------------------------------------

    fn make_field(buffer: &str, cursor: i32, length: i32, visible_length: i32) -> MenuField {
        MenuField {
            generic: MenuCommon { item_type: MTYPE_FIELD, ..Default::default() },
            buffer: buffer.to_string(),
            cursor,
            length,
            visible_length,
            visible_offset: 0,
        }
    }

    fn make_mock_renderer() -> MockRenderer {
        MockRenderer::new()
    }

    #[test]
    fn test_field_key_insert_character() {
        let mut field = make_field("", 0, 20, 10);
        let renderer = make_mock_renderer();
        let result = field_key(&mut field, 'a' as i32, &renderer);
        assert!(result);
        assert_eq!(field.buffer, "a");
        assert_eq!(field.cursor, 1);
    }

    #[test]
    fn test_field_key_insert_multiple_characters() {
        let mut field = make_field("", 0, 20, 10);
        let renderer = make_mock_renderer();
        field_key(&mut field, 'h' as i32, &renderer);
        field_key(&mut field, 'i' as i32, &renderer);
        assert_eq!(field.buffer, "hi");
        assert_eq!(field.cursor, 2);
    }

    #[test]
    fn test_field_key_backspace_deletes() {
        let mut field = make_field("abc", 3, 20, 10);
        let renderer = make_mock_renderer();
        let result = field_key(&mut field, K_BACKSPACE, &renderer);
        assert!(result);
        assert_eq!(field.buffer, "ab");
        assert_eq!(field.cursor, 2);
    }

    #[test]
    fn test_field_key_backspace_at_start() {
        let mut field = make_field("abc", 0, 20, 10);
        let renderer = make_mock_renderer();
        let result = field_key(&mut field, K_BACKSPACE, &renderer);
        assert!(result);
        assert_eq!(field.buffer, "abc"); // no change
        assert_eq!(field.cursor, 0);
    }

    #[test]
    fn test_field_key_delete_at_cursor() {
        // K_DEL (148) is > 127 and not remapped by keypad remap,
        // so field_key returns false. This matches the original C behavior
        // where only K_KP_DEL (remapped to '.') and K_BACKSPACE work in fields.
        let mut field = make_field("abc", 1, 20, 10);
        let renderer = make_mock_renderer();
        let result = field_key(&mut field, K_DEL, &renderer);
        assert!(!result);
        assert_eq!(field.buffer, "abc"); // unchanged
    }

    #[test]
    fn test_field_key_delete_at_end() {
        // K_DEL (148) > 127, returns false without reaching the match arm
        let mut field = make_field("abc", 3, 20, 10);
        let renderer = make_mock_renderer();
        let result = field_key(&mut field, K_DEL, &renderer);
        assert!(!result);
        assert_eq!(field.buffer, "abc"); // unchanged
    }

    #[test]
    fn test_field_key_enter_returns_false() {
        let mut field = make_field("abc", 0, 20, 10);
        let renderer = make_mock_renderer();
        let result = field_key(&mut field, K_ENTER, &renderer);
        assert!(!result);
    }

    #[test]
    fn test_field_key_escape_returns_false() {
        let mut field = make_field("abc", 0, 20, 10);
        let renderer = make_mock_renderer();
        let result = field_key(&mut field, K_ESCAPE, &renderer);
        assert!(!result);
    }

    #[test]
    fn test_field_key_tab_returns_false() {
        let mut field = make_field("abc", 0, 20, 10);
        let renderer = make_mock_renderer();
        let result = field_key(&mut field, K_TAB, &renderer);
        assert!(!result);
    }

    #[test]
    fn test_field_key_kp_enter_returns_false() {
        let mut field = make_field("abc", 0, 20, 10);
        let renderer = make_mock_renderer();
        let result = field_key(&mut field, K_KP_ENTER, &renderer);
        assert!(!result);
    }

    #[test]
    fn test_field_key_length_limit() {
        let mut field = make_field("", 0, 3, 3);
        let renderer = make_mock_renderer();
        field_key(&mut field, 'a' as i32, &renderer);
        field_key(&mut field, 'b' as i32, &renderer);
        field_key(&mut field, 'c' as i32, &renderer);
        field_key(&mut field, 'd' as i32, &renderer); // should not be added
        assert_eq!(field.buffer, "abc");
        assert_eq!(field.cursor, 3);
    }

    #[test]
    fn test_field_key_numbers_only() {
        let mut field = MenuField {
            generic: MenuCommon {
                item_type: MTYPE_FIELD,
                flags: QMF_NUMBERSONLY,
                ..Default::default()
            },
            buffer: String::new(),
            cursor: 0,
            length: 20,
            visible_length: 10,
            visible_offset: 0,
        };
        let renderer = make_mock_renderer();
        // digit should work
        let result = field_key(&mut field, '5' as i32, &renderer);
        assert!(result);
        assert_eq!(field.buffer, "5");

        // letter should be rejected
        let result = field_key(&mut field, 'a' as i32, &renderer);
        assert!(!result);
        assert_eq!(field.buffer, "5");
    }

    #[test]
    fn test_field_key_keypad_remapping() {
        let mut field = make_field("", 0, 20, 10);
        let renderer = make_mock_renderer();
        // KP_HOME should remap to '7'
        field_key(&mut field, K_KP_HOME, &renderer);
        assert_eq!(field.buffer, "7");

        // KP_5 should remap to '5'
        field_key(&mut field, K_KP_5, &renderer);
        assert_eq!(field.buffer, "75");

        // KP_DEL should remap to '.'
        field_key(&mut field, K_KP_DEL, &renderer);
        assert_eq!(field.buffer, "75.");
    }

    #[test]
    fn test_field_key_keypad_slash_minus_plus() {
        let mut field = make_field("", 0, 20, 10);
        let renderer = make_mock_renderer();
        field_key(&mut field, K_KP_SLASH, &renderer);
        assert_eq!(field.buffer, "/");
        field_key(&mut field, K_KP_MINUS, &renderer);
        assert_eq!(field.buffer, "/-");
        field_key(&mut field, K_KP_PLUS, &renderer);
        assert_eq!(field.buffer, "/-+");
    }

    #[test]
    fn test_field_key_high_key_returns_false() {
        let mut field = make_field("abc", 0, 20, 10);
        let renderer = make_mock_renderer();
        // Keys > 127 (except remapped ones) should return false
        let result = field_key(&mut field, 200, &renderer);
        assert!(!result);
        assert_eq!(field.buffer, "abc");
    }

    #[test]
    fn test_field_key_visible_offset_advances() {
        let mut field = make_field("", 0, 20, 3);
        let renderer = make_mock_renderer();
        // Type 4 characters with visible_length of 3
        field_key(&mut field, 'a' as i32, &renderer);
        field_key(&mut field, 'b' as i32, &renderer);
        field_key(&mut field, 'c' as i32, &renderer);
        assert_eq!(field.visible_offset, 0);
        // 4th character should push visible_offset
        field_key(&mut field, 'd' as i32, &renderer);
        assert_eq!(field.visible_offset, 1);
    }

    #[test]
    fn test_field_key_paste_from_clipboard() {
        let mut field = make_field("", 0, 20, 10);
        let mut renderer = make_mock_renderer();
        renderer.clipboard = Some("hello world".to_string());
        renderer.keys_down[K_CTRL as usize] = true;
        // Send 'V' (uppercase) to trigger paste
        let result = field_key(&mut field, 'V' as i32, &renderer);
        assert!(result);
        assert_eq!(field.buffer, "hello world");
        assert_eq!(field.cursor, 11);
    }

    #[test]
    fn test_field_key_paste_truncates_at_newline() {
        let mut field = make_field("", 0, 20, 10);
        let mut renderer = make_mock_renderer();
        renderer.clipboard = Some("line1\nline2".to_string());
        renderer.keys_down[K_CTRL as usize] = true;
        field_key(&mut field, 'v' as i32, &renderer);
        assert_eq!(field.buffer, "line1");
    }

    #[test]
    fn test_field_key_paste_truncates_at_length() {
        let mut field = make_field("", 0, 5, 5);
        let mut renderer = make_mock_renderer();
        renderer.clipboard = Some("abcdefghij".to_string());
        renderer.keys_down[K_CTRL as usize] = true;
        field_key(&mut field, 'v' as i32, &renderer);
        // length is 5, so field.length - 1 = 4 chars max
        assert_eq!(field.buffer, "abcd");
    }

    #[test]
    fn test_field_do_enter_no_callback() {
        let field = MenuField::default();
        assert!(!field_do_enter(&field));
    }

    #[test]
    fn test_field_do_enter_with_callback() {
        let mut field = MenuField::default();
        field.generic.callback = Some(Box::new(|_idx| {}));
        assert!(field_do_enter(&field));
    }

    // ----------------------------------------------------------
    // Menu framework: add item tests
    // ----------------------------------------------------------

    #[test]
    fn test_menu_add_item_single() {
        let mut menu = MenuFramework::default();
        let mut items: Vec<MenuItem> = Vec::new();
        let action = MenuItem::Action(MenuAction::default());
        menu_add_item(&mut menu, &mut items, action);
        assert_eq!(menu.nitems, 1);
        assert_eq!(items.len(), 1);
        assert_eq!(menu.items.len(), 1);
    }

    #[test]
    fn test_menu_add_item_multiple() {
        let mut menu = MenuFramework::default();
        let mut items: Vec<MenuItem> = Vec::new();
        for _ in 0..5 {
            menu_add_item(&mut menu, &mut items, MenuItem::Action(MenuAction::default()));
        }
        assert_eq!(menu.nitems, 5);
        assert_eq!(items.len(), 5);
    }

    #[test]
    fn test_menu_add_item_max() {
        let mut menu = MenuFramework::default();
        let mut items: Vec<MenuItem> = Vec::new();
        for _ in 0..(MAXMENUITEMS + 5) {
            menu_add_item(&mut menu, &mut items, MenuItem::Action(MenuAction::default()));
        }
        assert_eq!(menu.nitems as usize, MAXMENUITEMS);
        assert_eq!(items.len(), MAXMENUITEMS);
    }

    #[test]
    fn test_menu_tally_slots_actions_only() {
        let mut menu = MenuFramework::default();
        let mut items: Vec<MenuItem> = Vec::new();
        for _ in 0..3 {
            menu_add_item(&mut menu, &mut items, MenuItem::Action(MenuAction::default()));
        }
        assert_eq!(menu_tally_slots(&menu, &items), 3);
    }

    #[test]
    fn test_menu_tally_slots_with_list() {
        let mut menu = MenuFramework::default();
        let mut items: Vec<MenuItem> = Vec::new();
        // Add a list with 4 item names
        let list = MenuItem::List(MenuList {
            generic: MenuCommon { item_type: MTYPE_LIST, ..Default::default() },
            curvalue: 0,
            itemnames: vec!["a".into(), "b".into(), "c".into(), "d".into()],
        });
        menu_add_item(&mut menu, &mut items, list);
        // Add an action
        menu_add_item(&mut menu, &mut items, MenuItem::Action(MenuAction::default()));
        // slots = 4 (list items) + 1 (action) = 5
        assert_eq!(menu_tally_slots(&menu, &items), 5);
    }

    // ----------------------------------------------------------
    // Menu framework: cursor and navigation tests
    // ----------------------------------------------------------

    #[test]
    fn test_menu_adjust_cursor_skips_separator_forward() {
        let mut menu = MenuFramework::default();
        let mut items: Vec<MenuItem> = Vec::new();
        menu_add_item(&mut menu, &mut items, MenuItem::Action(MenuAction::default()));
        menu_add_item(&mut menu, &mut items, MenuItem::Separator(MenuSeparator::default()));
        menu_add_item(&mut menu, &mut items, MenuItem::Action(MenuAction::default()));

        menu.cursor = 1; // on separator
        menu_adjust_cursor(&mut menu, &items, 1);
        assert_eq!(menu.cursor, 2); // should skip to next action
    }

    #[test]
    fn test_menu_adjust_cursor_skips_separator_backward() {
        let mut menu = MenuFramework::default();
        let mut items: Vec<MenuItem> = Vec::new();
        menu_add_item(&mut menu, &mut items, MenuItem::Action(MenuAction::default()));
        menu_add_item(&mut menu, &mut items, MenuItem::Separator(MenuSeparator::default()));
        menu_add_item(&mut menu, &mut items, MenuItem::Action(MenuAction::default()));

        menu.cursor = 1; // on separator
        menu_adjust_cursor(&mut menu, &items, -1);
        assert_eq!(menu.cursor, 0); // should go back to first action
    }

    #[test]
    fn test_menu_adjust_cursor_wraps_forward() {
        let mut menu = MenuFramework::default();
        let mut items: Vec<MenuItem> = Vec::new();
        menu_add_item(&mut menu, &mut items, MenuItem::Action(MenuAction::default()));
        menu_add_item(&mut menu, &mut items, MenuItem::Separator(MenuSeparator::default()));

        menu.cursor = 1; // on separator (last item)
        menu_adjust_cursor(&mut menu, &items, 1);
        assert_eq!(menu.cursor, 0); // should wrap to first action
    }

    #[test]
    fn test_menu_adjust_cursor_wraps_backward() {
        let mut menu = MenuFramework::default();
        let mut items: Vec<MenuItem> = Vec::new();
        menu_add_item(&mut menu, &mut items, MenuItem::Separator(MenuSeparator::default()));
        menu_add_item(&mut menu, &mut items, MenuItem::Action(MenuAction::default()));

        menu.cursor = 0; // on separator (first item)
        menu_adjust_cursor(&mut menu, &items, -1);
        assert_eq!(menu.cursor, 1); // should wrap to last action
    }

    #[test]
    fn test_menu_adjust_cursor_stays_on_valid_item() {
        let mut menu = MenuFramework::default();
        let mut items: Vec<MenuItem> = Vec::new();
        menu_add_item(&mut menu, &mut items, MenuItem::Action(MenuAction::default()));
        menu_add_item(&mut menu, &mut items, MenuItem::Action(MenuAction::default()));

        menu.cursor = 0; // already on action
        menu_adjust_cursor(&mut menu, &items, 1);
        assert_eq!(menu.cursor, 0); // should stay
    }

    // ----------------------------------------------------------
    // menu_item_at_cursor tests
    // ----------------------------------------------------------

    #[test]
    fn test_menu_item_at_cursor_valid() {
        let mut menu = MenuFramework::default();
        let mut items: Vec<MenuItem> = Vec::new();
        menu_add_item(&mut menu, &mut items, MenuItem::Action(MenuAction {
            generic: MenuCommon {
                item_type: MTYPE_ACTION,
                name: Some("first".to_string()),
                ..Default::default()
            },
        }));
        menu_add_item(&mut menu, &mut items, MenuItem::Action(MenuAction {
            generic: MenuCommon {
                item_type: MTYPE_ACTION,
                name: Some("second".to_string()),
                ..Default::default()
            },
        }));

        menu.cursor = 1;
        let item = menu_item_at_cursor(&menu, &items);
        assert!(item.is_some());
        assert_eq!(item.unwrap().generic().name.as_deref(), Some("second"));
    }

    #[test]
    fn test_menu_item_at_cursor_negative() {
        let mut menu = MenuFramework::default();
        let mut items: Vec<MenuItem> = Vec::new();
        menu_add_item(&mut menu, &mut items, MenuItem::Action(MenuAction::default()));
        menu.cursor = -1;
        assert!(menu_item_at_cursor(&menu, &items).is_none());
    }

    #[test]
    fn test_menu_item_at_cursor_beyond_nitems() {
        let mut menu = MenuFramework::default();
        let mut items: Vec<MenuItem> = Vec::new();
        menu_add_item(&mut menu, &mut items, MenuItem::Action(MenuAction::default()));
        menu.cursor = 5;
        assert!(menu_item_at_cursor(&menu, &items).is_none());
    }

    // ----------------------------------------------------------
    // menu_center tests
    // ----------------------------------------------------------

    #[test]
    fn test_menu_center_calculates_y() {
        let mut menu = MenuFramework::default();
        let mut items: Vec<MenuItem> = Vec::new();
        let mut action = MenuAction::default();
        action.generic.y = 80;
        menu_add_item(&mut menu, &mut items, MenuItem::Action(action));

        menu_center(&mut menu, &items, 480);
        // height = last_item.y + 10 = 90
        // menu.y = (480 - 90) / 2 = 195
        assert_eq!(menu.y, 195);
    }

    #[test]
    fn test_menu_center_empty_menu() {
        let mut menu = MenuFramework::default();
        let items: Vec<MenuItem> = Vec::new();
        menu.y = 42;
        menu_center(&mut menu, &items, 480);
        // should not change y for empty menu
        assert_eq!(menu.y, 42);
    }

    // ----------------------------------------------------------
    // menu_set_status_bar tests
    // ----------------------------------------------------------

    #[test]
    fn test_menu_set_status_bar() {
        let mut menu = MenuFramework::default();
        assert!(menu.statusbar.is_none());
        menu_set_status_bar(&mut menu, Some("hello".to_string()));
        assert_eq!(menu.statusbar.as_deref(), Some("hello"));
        menu_set_status_bar(&mut menu, None);
        assert!(menu.statusbar.is_none());
    }

    // ----------------------------------------------------------
    // menu_select_item tests
    // ----------------------------------------------------------

    #[test]
    fn test_menu_select_item_action_returns_true() {
        let mut menu = MenuFramework::default();
        let mut items: Vec<MenuItem> = Vec::new();
        let mut action = MenuAction::default();
        action.generic.callback = Some(Box::new(|_| {}));
        menu_add_item(&mut menu, &mut items, MenuItem::Action(action));
        menu.cursor = 0;
        assert!(menu_select_item(&menu, &items));
    }

    #[test]
    fn test_menu_select_item_list_returns_false() {
        let mut menu = MenuFramework::default();
        let mut items: Vec<MenuItem> = Vec::new();
        menu_add_item(&mut menu, &mut items, MenuItem::List(MenuList::default()));
        menu.cursor = 0;
        assert!(!menu_select_item(&menu, &items));
    }

    #[test]
    fn test_menu_select_item_spincontrol_returns_false() {
        let mut menu = MenuFramework::default();
        let mut items: Vec<MenuItem> = Vec::new();
        menu_add_item(&mut menu, &mut items, MenuItem::SpinControl(MenuList {
            generic: MenuCommon { item_type: MTYPE_SPINCONTROL, ..Default::default() },
            ..Default::default()
        }));
        menu.cursor = 0;
        assert!(!menu_select_item(&menu, &items));
    }

    #[test]
    fn test_menu_select_item_invalid_cursor() {
        let menu = MenuFramework::default();
        let items: Vec<MenuItem> = Vec::new();
        assert!(!menu_select_item(&menu, &items));
    }

    // ----------------------------------------------------------
    // menu_slide_item tests
    // ----------------------------------------------------------

    #[test]
    fn test_menu_slide_item_slider() {
        let mut menu = MenuFramework::default();
        let mut items: Vec<MenuItem> = Vec::new();
        let slider = MenuItem::Slider(MenuSlider {
            minvalue: 0.0,
            maxvalue: 10.0,
            curvalue: 5.0,
            ..Default::default()
        });
        menu_add_item(&mut menu, &mut items, slider);
        menu.cursor = 0;
        menu_slide_item(&menu, &mut items, 1);
        if let MenuItem::Slider(ref s) = items[0] {
            assert_eq!(s.curvalue, 6.0);
        } else {
            panic!("Expected slider");
        }
    }

    #[test]
    fn test_menu_slide_item_spin() {
        let mut menu = MenuFramework::default();
        let mut items: Vec<MenuItem> = Vec::new();
        let spin = MenuItem::SpinControl(MenuList {
            generic: MenuCommon { item_type: MTYPE_SPINCONTROL, ..Default::default() },
            curvalue: 0,
            itemnames: vec!["a".into(), "b".into(), "c".into()],
        });
        menu_add_item(&mut menu, &mut items, spin);
        menu.cursor = 0;
        menu_slide_item(&menu, &mut items, 1);
        if let MenuItem::SpinControl(ref s) = items[0] {
            assert_eq!(s.curvalue, 1);
        } else {
            panic!("Expected spin control");
        }
    }

    #[test]
    fn test_menu_slide_item_invalid_cursor() {
        let mut menu = MenuFramework::default();
        let mut items: Vec<MenuItem> = Vec::new();
        menu_add_item(&mut menu, &mut items, MenuItem::Action(MenuAction::default()));
        menu.cursor = -1;
        // should not panic
        menu_slide_item(&menu, &mut items, 1);
    }

    // ----------------------------------------------------------
    // Drawing function coordinate tests
    // ----------------------------------------------------------

    #[test]
    fn test_menu_draw_string_coordinates() {
        let mut renderer = MockRenderer::new();
        menu_draw_string(&mut renderer, 100, 50, "AB");
        assert_eq!(renderer.chars.len(), 2);
        assert_eq!(renderer.chars[0], (100, 50, 'A' as i32));
        assert_eq!(renderer.chars[1], (108, 50, 'B' as i32));
    }

    #[test]
    fn test_menu_draw_string_dark_adds_128() {
        let mut renderer = MockRenderer::new();
        menu_draw_string_dark(&mut renderer, 0, 0, "A");
        assert_eq!(renderer.chars[0].2, 'A' as i32 + 128);
    }

    #[test]
    fn test_menu_draw_string_r2l_coordinates() {
        let mut renderer = MockRenderer::new();
        menu_draw_string_r2l(&mut renderer, 100, 50, "AB");
        // Right-to-left: draws last char first at x, then x-8
        assert_eq!(renderer.chars.len(), 2);
        // i=0: x - 0*8, char = B (reversed)
        assert_eq!(renderer.chars[0], (100, 50, 'B' as i32));
        // i=1: x - 1*8, char = A (reversed)
        assert_eq!(renderer.chars[1], (92, 50, 'A' as i32));
    }

    #[test]
    fn test_menu_draw_string_r2l_dark_adds_128() {
        let mut renderer = MockRenderer::new();
        menu_draw_string_r2l_dark(&mut renderer, 0, 0, "A");
        assert_eq!(renderer.chars[0].2, 'A' as i32 + 128);
    }

    #[test]
    fn test_menu_draw_string_empty() {
        let mut renderer = MockRenderer::new();
        menu_draw_string(&mut renderer, 0, 0, "");
        assert!(renderer.chars.is_empty());
    }

    #[test]
    fn test_menu_draw_status_bar_with_text() {
        let mut renderer = MockRenderer::new();
        renderer.width = 640;
        renderer.height = 480;
        menu_draw_status_bar(&mut renderer, Some("test"));
        // Should draw a fill for the bar background
        assert_eq!(renderer.fills.len(), 1);
        assert_eq!(renderer.fills[0], (0, 472, 640, 8, 4));
        // Should draw the text characters
        assert_eq!(renderer.chars.len(), 4);
    }

    #[test]
    fn test_menu_draw_status_bar_no_text() {
        let mut renderer = MockRenderer::new();
        renderer.width = 640;
        renderer.height = 480;
        menu_draw_status_bar(&mut renderer, None);
        // Should still draw a fill (black bar)
        assert_eq!(renderer.fills.len(), 1);
        assert_eq!(renderer.fills[0], (0, 472, 640, 8, 0));
        assert!(renderer.chars.is_empty());
    }

    // ----------------------------------------------------------
    // Slider draw range calculation test
    // ----------------------------------------------------------

    #[test]
    fn test_slider_draw_range_calculation() {
        let mut renderer = MockRenderer::new();
        let slider = MenuSlider {
            generic: MenuCommon {
                item_type: MTYPE_SLIDER,
                name: Some("test".to_string()),
                x: 0,
                y: 0,
                ..Default::default()
            },
            minvalue: 0.0,
            maxvalue: 10.0,
            curvalue: 5.0,
            range: 0.0,
        };
        slider_draw(&mut renderer, &slider, 100, 50);
        // The slider should draw: name (r2l dark), left cap (128), 10 middle pieces (129),
        // right cap (130), and the handle (131)
        let cap_chars: Vec<_> = renderer.chars.iter().filter(|c| c.2 == 128 || c.2 == 129 || c.2 == 130 || c.2 == 131).collect();
        // 1 left cap + 10 middle + 1 right cap + 1 handle = 13
        assert_eq!(cap_chars.len(), 13);
    }

    #[test]
    fn test_slider_draw_at_min() {
        let mut renderer = MockRenderer::new();
        let slider = MenuSlider {
            generic: MenuCommon { item_type: MTYPE_SLIDER, x: 0, y: 0, ..Default::default() },
            minvalue: 0.0,
            maxvalue: 10.0,
            curvalue: 0.0,
            range: 0.0,
        };
        slider_draw(&mut renderer, &slider, 0, 0);
        // Handle (131) should be at the leftmost position
        let handle = renderer.chars.iter().find(|c| c.2 == 131).unwrap();
        // range = 0.0, so handle_x = 8 + RCOLUMN_OFFSET + 0 + 0 + 0 = 24
        assert_eq!(handle.0, 24);
    }

    #[test]
    fn test_slider_draw_at_max() {
        let mut renderer = MockRenderer::new();
        let slider = MenuSlider {
            generic: MenuCommon { item_type: MTYPE_SLIDER, x: 0, y: 0, ..Default::default() },
            minvalue: 0.0,
            maxvalue: 10.0,
            curvalue: 10.0,
            range: 0.0,
        };
        slider_draw(&mut renderer, &slider, 0, 0);
        let handle = renderer.chars.iter().find(|c| c.2 == 131).unwrap();
        // range = 1.0, so handle_x = (8 + 16 + 0 + 0 + (10-1)*8*1.0) as i32 = (8+16+72) = 96
        assert_eq!(handle.0, 96);
    }

    #[test]
    fn test_slider_draw_equal_min_max() {
        let mut renderer = MockRenderer::new();
        let slider = MenuSlider {
            generic: MenuCommon { item_type: MTYPE_SLIDER, x: 0, y: 0, ..Default::default() },
            minvalue: 5.0,
            maxvalue: 5.0,
            curvalue: 5.0,
            range: 0.0,
        };
        slider_draw(&mut renderer, &slider, 0, 0);
        let handle = renderer.chars.iter().find(|c| c.2 == 131).unwrap();
        // range = 0.0 (minvalue == maxvalue), so handle at leftmost
        assert_eq!(handle.0, 24);
    }

    // ----------------------------------------------------------
    // Action draw tests
    // ----------------------------------------------------------

    #[test]
    fn test_action_draw_no_name_draws_nothing() {
        let mut renderer = MockRenderer::new();
        let action = MenuAction {
            generic: MenuCommon {
                item_type: MTYPE_ACTION,
                name: None,
                ..Default::default()
            },
        };
        action_draw(&mut renderer, &action, 0, 0);
        assert!(renderer.chars.is_empty());
    }

    #[test]
    fn test_action_draw_left_justify() {
        let mut renderer = MockRenderer::new();
        let action = MenuAction {
            generic: MenuCommon {
                item_type: MTYPE_ACTION,
                name: Some("A".to_string()),
                flags: QMF_LEFT_JUSTIFY,
                x: 0,
                y: 0,
                ..Default::default()
            },
        };
        action_draw(&mut renderer, &action, 100, 50);
        // left justify: x + parent_x + LCOLUMN_OFFSET = 0 + 100 + (-16) = 84
        assert_eq!(renderer.chars[0].0, 84);
        assert_eq!(renderer.chars[0].1, 50);
        // Normal text (not dark)
        assert_eq!(renderer.chars[0].2, 'A' as i32);
    }

    #[test]
    fn test_action_draw_left_justify_grayed() {
        let mut renderer = MockRenderer::new();
        let action = MenuAction {
            generic: MenuCommon {
                item_type: MTYPE_ACTION,
                name: Some("A".to_string()),
                flags: QMF_LEFT_JUSTIFY | QMF_GRAYED,
                x: 0,
                y: 0,
                ..Default::default()
            },
        };
        action_draw(&mut renderer, &action, 100, 50);
        // dark text: char + 128
        assert_eq!(renderer.chars[0].2, 'A' as i32 + 128);
    }

    #[test]
    fn test_action_draw_right_justify_grayed() {
        let mut renderer = MockRenderer::new();
        let action = MenuAction {
            generic: MenuCommon {
                item_type: MTYPE_ACTION,
                name: Some("A".to_string()),
                flags: QMF_GRAYED,
                x: 0,
                y: 0,
                ..Default::default()
            },
        };
        action_draw(&mut renderer, &action, 100, 50);
        // R2L dark: char + 128
        assert_eq!(renderer.chars[0].2, 'A' as i32 + 128);
    }

    // ----------------------------------------------------------
    // Constants tests
    // ----------------------------------------------------------

    #[test]
    fn test_mtype_constants_are_unique() {
        let types = [MTYPE_SLIDER, MTYPE_LIST, MTYPE_ACTION, MTYPE_SPINCONTROL, MTYPE_SEPARATOR, MTYPE_FIELD];
        for i in 0..types.len() {
            for j in (i + 1)..types.len() {
                assert_ne!(types[i], types[j], "MTYPE constants must be unique");
            }
        }
    }

    #[test]
    fn test_qmf_flags_are_distinct_bits() {
        assert_eq!(QMF_LEFT_JUSTIFY & QMF_GRAYED, 0);
        assert_eq!(QMF_LEFT_JUSTIFY & QMF_NUMBERSONLY, 0);
        assert_eq!(QMF_GRAYED & QMF_NUMBERSONLY, 0);
    }

    #[test]
    fn test_column_offsets() {
        assert_eq!(RCOLUMN_OFFSET, 16);
        assert_eq!(LCOLUMN_OFFSET, -16);
    }

    #[test]
    fn test_slider_range_constant() {
        assert_eq!(SLIDER_RANGE, 10);
    }

    #[test]
    fn test_maxmenuitems_constant() {
        assert_eq!(MAXMENUITEMS, 64);
    }

    // ----------------------------------------------------------
    // MenuFramework default tests
    // ----------------------------------------------------------

    #[test]
    fn test_menu_framework_default() {
        let menu = MenuFramework::default();
        assert_eq!(menu.x, 0);
        assert_eq!(menu.y, 0);
        assert_eq!(menu.cursor, 0);
        assert_eq!(menu.nitems, 0);
        assert_eq!(menu.nslots, 0);
        assert!(menu.items.is_empty());
        assert!(menu.statusbar.is_none());
        assert!(menu.cursordraw.is_none());
    }

    // ----------------------------------------------------------
    // Complex field editing sequences
    // ----------------------------------------------------------

    #[test]
    fn test_field_key_insert_in_middle() {
        let mut field = make_field("ac", 1, 20, 10);
        let renderer = make_mock_renderer();
        field_key(&mut field, 'b' as i32, &renderer);
        assert_eq!(field.buffer, "abc");
        assert_eq!(field.cursor, 2);
    }

    #[test]
    fn test_field_key_backspace_in_middle() {
        let mut field = make_field("abc", 2, 20, 10);
        let renderer = make_mock_renderer();
        field_key(&mut field, K_BACKSPACE, &renderer);
        assert_eq!(field.buffer, "ac");
        assert_eq!(field.cursor, 1);
    }

    #[test]
    fn test_field_key_delete_in_middle_via_backspace() {
        // Use backspace at cursor=1 to delete character before cursor
        let mut field = make_field("abc", 1, 20, 10);
        let renderer = make_mock_renderer();
        field_key(&mut field, K_BACKSPACE, &renderer);
        assert_eq!(field.buffer, "bc");
        assert_eq!(field.cursor, 0);
    }

    #[test]
    fn test_field_key_full_edit_sequence() {
        let mut field = make_field("", 0, 20, 10);
        let renderer = make_mock_renderer();
        // Type "hello"
        for ch in "hello".chars() {
            field_key(&mut field, ch as i32, &renderer);
        }
        assert_eq!(field.buffer, "hello");
        assert_eq!(field.cursor, 5);
        // Backspace twice
        field_key(&mut field, K_BACKSPACE, &renderer);
        field_key(&mut field, K_BACKSPACE, &renderer);
        assert_eq!(field.buffer, "hel");
        assert_eq!(field.cursor, 3);
        // Type "p"
        field_key(&mut field, 'p' as i32, &renderer);
        assert_eq!(field.buffer, "help");
        assert_eq!(field.cursor, 4);
    }

    // ----------------------------------------------------------
    // Backspace with visible_offset
    // ----------------------------------------------------------

    #[test]
    fn test_field_key_backspace_decrements_visible_offset() {
        let mut field = MenuField {
            generic: MenuCommon { item_type: MTYPE_FIELD, ..Default::default() },
            buffer: "abcd".to_string(),
            cursor: 4,
            length: 20,
            visible_length: 3,
            visible_offset: 1,
        };
        let renderer = make_mock_renderer();
        field_key(&mut field, K_BACKSPACE, &renderer);
        assert_eq!(field.buffer, "abc");
        assert_eq!(field.cursor, 3);
        assert_eq!(field.visible_offset, 0);
    }
}
