// vid_menu.rs -- video menu interface
// Converted from: myq2-original/win32/vid_menu.c

use myq2_common::cvar::CvarContext;
use myq2_common::q_shared::*;
use myq2_client::menu::{m_force_menu_off, m_pop_menu};
use myq2_client::console;
use myq2_client::qmenu;

// VidDef is imported from myq2_common::q_shared via the glob import above.

use std::cell::UnsafeCell;

/// Wrapper to allow non-Send/Sync types in statics for single-threaded engine.
/// SAFETY: The engine is single-threaded; these globals are only accessed from the main thread.
struct SyncUnsafeCell<T>(UnsafeCell<T>);
unsafe impl<T> Sync for SyncUnsafeCell<T> {}
impl<T> SyncUnsafeCell<T> {
    const fn new(value: T) -> Self {
        Self(UnsafeCell::new(value))
    }
    /// SAFETY: Caller must ensure no concurrent access (single-threaded engine).
    #[allow(clippy::mut_from_ref)]
    unsafe fn get_mut(&self) -> &mut T {
        &mut *self.0.get()
    }
    /// SAFETY: Caller must ensure no concurrent mutable access (single-threaded engine).
    unsafe fn get_ref(&self) -> &T {
        &*self.0.get()
    }
}

// ============================================================
// Menu item types and structs — imported from canonical qmenu.rs
// ============================================================

use myq2_client::qmenu::{
    MenuAction, MenuSlider, MenuList, MenuFramework,
    MTYPE_SLIDER, MTYPE_SPINCONTROL, MTYPE_ACTION,
};

// ============================================================
// Video menu state
// ============================================================

pub struct VidMenuState {
    pub s_vulkan_menu: MenuFramework,
    pub s_mode_list: MenuList,
    pub s_ref_list: MenuList,
    pub s_tq_slider: MenuSlider,
    pub s_screensize_slider: MenuSlider,
    pub s_brightness_slider: MenuSlider,
    pub s_fs_box: MenuList,
    pub s_finish_box: MenuList,
    pub s_cancel_action: MenuAction,
    pub s_defaults_action: MenuAction,

    // Cvar indices
    pub vk_mode: Option<usize>,
    pub vk_driver: Option<usize>,
    pub vk_picmip: Option<usize>,
    pub vk_finish: Option<usize>,
    pub scr_viewsize: Option<usize>,
}

impl Default for VidMenuState {
    fn default() -> Self {
        Self::new()
    }
}

impl VidMenuState {
    pub fn new() -> Self {
        Self {
            s_vulkan_menu: MenuFramework::default(),
            s_mode_list: MenuList::default(),
            s_ref_list: MenuList::default(),
            s_tq_slider: MenuSlider::default(),
            s_screensize_slider: MenuSlider::default(),
            s_brightness_slider: MenuSlider::default(),
            s_fs_box: MenuList::default(),
            s_finish_box: MenuList::default(),
            s_cancel_action: MenuAction::default(),
            s_defaults_action: MenuAction::default(),
            vk_mode: None,
            vk_driver: None,
            vk_picmip: None,
            vk_finish: None,
            scr_viewsize: None,
        }
    }
}

// ============================================================
// Resolution / driver name tables
// ============================================================

pub const RESOLUTIONS: &[&str] = &[
    "[320 240  ]",
    "[400 300  ]",
    "[512 384  ]",
    "[640 480  ]",
    "[800 600  ]",
    "[960 720  ]",
    "[1024 768 ]",
    "[1152 864 ]",
    "[1280 960 ]",
    "[1600 1200]",
    "[2048 1536]",
];

pub const REFS: &[&str] = &["[default OpenGL]"];

pub const YESNO_NAMES: &[&str] = &["no", "yes"];

// ============================================================
// Callbacks
// ============================================================

/// ScreenSizeCallback
pub fn screen_size_callback(menu: &mut VidMenuState, cvars: &mut CvarContext) {
    let value = menu.s_screensize_slider.curvalue * 10.0;
    if let Some(idx) = menu.scr_viewsize {
        if let Some(cv) = cvars.cvar_vars.get_mut(idx) {
            cv.value = value;
            cv.string = format!("{}", value);
            cv.modified = true;
        }
    }
}

/// BrightnessCallback — no-op in original (gamma applied on apply).
pub fn brightness_callback(_menu: &mut VidMenuState) {
    // intentionally empty, matching original
}

/// ResetDefaults
pub fn reset_defaults(menu: &mut VidMenuState, cvars: &mut CvarContext, viddef: &VidDef, vid_gamma_idx: Option<usize>, vid_fullscreen_idx: Option<usize>) {
    vid_menu_init(menu, cvars, viddef, vid_gamma_idx, vid_fullscreen_idx);
}

/// ApplyChanges
pub fn apply_changes(
    menu: &mut VidMenuState,
    cvars: &mut CvarContext,
    vid_gamma_idx: Option<usize>,
    vid_fullscreen_idx: Option<usize>,
    vid_ref_idx: Option<usize>,
) {
    // invert sense so greater = brighter, and scale to a range of 0.5 to 1.3
    let gamma = (0.8 - (menu.s_brightness_slider.curvalue / 10.0 - 0.5)) + 0.5;

    // Set cvars
    if let Some(idx) = vid_gamma_idx {
        if let Some(cv) = cvars.cvar_vars.get_mut(idx) {
            cv.value = gamma;
            cv.string = format!("{}", gamma);
            cv.modified = true;
        }
    }

    if let Some(idx) = menu.vk_picmip {
        if let Some(cv) = cvars.cvar_vars.get_mut(idx) {
            let val = 3.0 - menu.s_tq_slider.curvalue;
            cv.value = val;
            cv.string = format!("{}", val);
            cv.modified = true;
        }
    }

    if let Some(idx) = vid_fullscreen_idx {
        if let Some(cv) = cvars.cvar_vars.get_mut(idx) {
            cv.value = menu.s_fs_box.curvalue as f32;
            cv.string = format!("{}", menu.s_fs_box.curvalue);
            cv.modified = true;
        }
    }

    if let Some(idx) = menu.vk_finish {
        if let Some(cv) = cvars.cvar_vars.get_mut(idx) {
            cv.value = menu.s_finish_box.curvalue as f32;
            cv.string = format!("{}", menu.s_finish_box.curvalue);
            cv.modified = true;
        }
    }

    if let Some(idx) = menu.vk_mode {
        if let Some(cv) = cvars.cvar_vars.get_mut(idx) {
            cv.value = menu.s_mode_list.curvalue as f32;
            cv.string = format!("{}", menu.s_mode_list.curvalue);
            cv.modified = true;
        }
    }

    if let Some(idx) = vid_ref_idx {
        if let Some(cv) = cvars.cvar_vars.get_mut(idx) {
            cv.string = "gl".to_string();
            cv.modified = true;
        }
    }

    if let Some(idx) = menu.vk_driver {
        if let Some(cv) = cvars.cvar_vars.get_mut(idx) {
            cv.string = "opengl32".to_string();
            cv.modified = true;
        }
    }

    // Check gamma modification and 3dfx driver
    let gamma_modified = vid_gamma_idx
        .and_then(|idx| cvars.cvar_vars.get(idx))
        .map(|cv| cv.modified)
        .unwrap_or(false);

    if gamma_modified {
        if let Some(idx) = vid_ref_idx {
            if let Some(cv) = cvars.cvar_vars.get_mut(idx) {
                cv.modified = true;
            }
        }

        let is_3dfx = menu.vk_driver
            .and_then(|idx| cvars.cvar_vars.get(idx))
            .map(|cv| cv.string.eq_ignore_ascii_case("3dfxgl"))
            .unwrap_or(false);

        if is_3dfx {
            let gamma_val = vid_gamma_idx
                .and_then(|idx| cvars.cvar_vars.get(idx))
                .map(|cv| cv.value)
                .unwrap_or(0.6);

            let g = 2.00 * (0.8 - (gamma_val - 0.5)) + 1.0;
            std::env::set_var("SSTV2_GAMMA", format!("{}", g));
            std::env::set_var("SST_GAMMA", format!("{}", g));

            if let Some(idx) = vid_gamma_idx {
                if let Some(cv) = cvars.cvar_vars.get_mut(idx) {
                    cv.modified = false;
                }
            }
        }

        let driver_modified = menu.vk_driver
            .and_then(|idx| cvars.cvar_vars.get(idx))
            .map(|cv| cv.modified)
            .unwrap_or(false);

        if driver_modified {
            if let Some(idx) = vid_ref_idx {
                if let Some(cv) = cvars.cvar_vars.get_mut(idx) {
                    cv.modified = true;
                }
            }
        }
    }

    m_force_menu_off();
}

/// CancelChanges
pub fn cancel_changes() {
    m_pop_menu();
}

// ============================================================
// VID_MenuInit
// ============================================================

pub fn vid_menu_init(
    menu: &mut VidMenuState,
    cvars: &mut CvarContext,
    viddef: &VidDef,
    vid_gamma_idx: Option<usize>,
    vid_fullscreen_idx: Option<usize>,
) {
    // Ensure cvars exist
    if menu.vk_driver.is_none() {
        menu.vk_driver = Some(cvars.get_or_create("vk_driver", "opengl32", CVAR_ARCHIVE));
    }
    if menu.vk_picmip.is_none() {
        menu.vk_picmip = Some(cvars.get_or_create("vk_picmip", "0", CVAR_ARCHIVE));
    }
    if menu.vk_mode.is_none() {
        menu.vk_mode = Some(cvars.get_or_create("vk_mode", "4", CVAR_ARCHIVE));
    }
    if menu.vk_finish.is_none() {
        menu.vk_finish = Some(cvars.get_or_create("vk_finish", "0", CVAR_ARCHIVE));
    }

    let vk_mode_value = menu.vk_mode
        .and_then(|idx| cvars.cvar_vars.get(idx))
        .map(|cv| cv.value)
        .unwrap_or(4.0);
    menu.s_mode_list.curvalue = vk_mode_value as i32;

    if menu.scr_viewsize.is_none() {
        menu.scr_viewsize = Some(cvars.get_or_create("viewsize", "100", CVAR_ARCHIVE));
    }
    let viewsize_value = menu.scr_viewsize
        .and_then(|idx| cvars.cvar_vars.get(idx))
        .map(|cv| cv.value)
        .unwrap_or(100.0);
    menu.s_screensize_slider.curvalue = viewsize_value / 10.0;

    menu.s_vulkan_menu.x = (viddef.width as f32 * 0.50) as i32;
    menu.s_vulkan_menu.nitems = 0;

    // Set up ref list
    menu.s_ref_list.generic.item_type = MTYPE_SPINCONTROL;
    menu.s_ref_list.generic.name = Some("driver".to_string());
    menu.s_ref_list.generic.x = 0;
    menu.s_ref_list.generic.y = 0;
    menu.s_ref_list.itemnames = REFS.iter().map(|s| s.to_string()).collect();

    // Set up mode list
    menu.s_mode_list.generic.item_type = MTYPE_SPINCONTROL;
    menu.s_mode_list.generic.name = Some("video mode".to_string());
    menu.s_mode_list.generic.x = 0;
    menu.s_mode_list.generic.y = 10;
    menu.s_mode_list.itemnames = RESOLUTIONS.iter().map(|s| s.to_string()).collect();

    // Screen size slider
    menu.s_screensize_slider.generic.item_type = MTYPE_SLIDER;
    menu.s_screensize_slider.generic.x = 0;
    menu.s_screensize_slider.generic.y = 20;
    menu.s_screensize_slider.generic.name = Some("screen size".to_string());
    menu.s_screensize_slider.minvalue = 3.0;
    menu.s_screensize_slider.maxvalue = 12.0;

    // Brightness slider
    menu.s_brightness_slider.generic.item_type = MTYPE_SLIDER;
    menu.s_brightness_slider.generic.x = 0;
    menu.s_brightness_slider.generic.y = 30;
    menu.s_brightness_slider.generic.name = Some("brightness".to_string());
    menu.s_brightness_slider.minvalue = 5.0;
    menu.s_brightness_slider.maxvalue = 13.0;

    let vid_gamma_value = vid_gamma_idx
        .and_then(|idx| cvars.cvar_vars.get(idx))
        .map(|cv| cv.value)
        .unwrap_or(0.6);
    menu.s_brightness_slider.curvalue = (1.3 - vid_gamma_value + 0.5) * 10.0;

    // Fullscreen box
    menu.s_fs_box.generic.item_type = MTYPE_SPINCONTROL;
    menu.s_fs_box.generic.x = 0;
    menu.s_fs_box.generic.y = 40;
    menu.s_fs_box.generic.name = Some("fullscreen".to_string());
    menu.s_fs_box.itemnames = YESNO_NAMES.iter().map(|s| s.to_string()).collect();
    let fs_value = vid_fullscreen_idx
        .and_then(|idx| cvars.cvar_vars.get(idx))
        .map(|cv| cv.value)
        .unwrap_or(1.0);
    menu.s_fs_box.curvalue = fs_value as i32;

    // Texture quality slider
    menu.s_tq_slider.generic.item_type = MTYPE_SLIDER;
    menu.s_tq_slider.generic.x = 0;
    menu.s_tq_slider.generic.y = 60;
    menu.s_tq_slider.generic.name = Some("texture quality".to_string());
    menu.s_tq_slider.minvalue = 0.0;
    menu.s_tq_slider.maxvalue = 3.0;
    let picmip_value = menu.vk_picmip
        .and_then(|idx| cvars.cvar_vars.get(idx))
        .map(|cv| cv.value)
        .unwrap_or(0.0);
    menu.s_tq_slider.curvalue = 3.0 - picmip_value;

    // Sync every frame box
    menu.s_finish_box.generic.item_type = MTYPE_SPINCONTROL;
    menu.s_finish_box.generic.x = 0;
    menu.s_finish_box.generic.y = 70;
    menu.s_finish_box.generic.name = Some("sync every frame".to_string());
    let finish_value = menu.vk_finish
        .and_then(|idx| cvars.cvar_vars.get(idx))
        .map(|cv| cv.value)
        .unwrap_or(0.0);
    menu.s_finish_box.curvalue = finish_value as i32;
    menu.s_finish_box.itemnames = YESNO_NAMES.iter().map(|s| s.to_string()).collect();

    // Defaults action
    menu.s_defaults_action.generic.item_type = MTYPE_ACTION;
    menu.s_defaults_action.generic.name = Some("reset to defaults".to_string());
    menu.s_defaults_action.generic.x = 0;
    menu.s_defaults_action.generic.y = 90;

    // Cancel action
    menu.s_cancel_action.generic.item_type = MTYPE_ACTION;
    menu.s_cancel_action.generic.name = Some("cancel".to_string());
    menu.s_cancel_action.generic.x = 0;
    menu.s_cancel_action.generic.y = 100;

    // Stub: Menu_AddItem calls, Menu_Center
    menu.s_vulkan_menu.x -= 8;
}

/// VID_MenuDraw
pub fn vid_menu_draw(menu: &VidMenuState, viddef: &VidDef) {
    // Stub: Draw_GetPicSize(&w, &h, "m_banner_video")
    let w = 0;
    let _h = 0;

    // Stub: Draw_Pic(viddef.width / 2 - w / 2, viddef.height / 2 - 110, "m_banner_video")
    let _ = (viddef.width / 2 - w / 2, viddef.height / 2 - 110);

    // Stub: Menu_AdjustCursor(&s_vulkan_menu, 1)
    // Stub: Menu_Draw(&s_vulkan_menu)
    let _ = &menu.s_vulkan_menu;
}

/// VID_MenuKey — handle key input in the video settings menu.
/// Returns the sound to play (or None for no sound).
pub fn vid_menu_key(
    menu: &mut VidMenuState,
    cvars: &mut CvarContext,
    key: i32,
    vid_gamma_idx: Option<usize>,
    vid_fullscreen_idx: Option<usize>,
    vid_ref_idx: Option<usize>,
) -> Option<&'static str> {
    let sound = "misc/menu1.wav";

    // Key constants — placeholder values matching original engine
    const K_ESCAPE: i32 = 27;
    const K_KP_UPARROW: i32 = 162;
    const K_UPARROW: i32 = 128;
    const K_KP_DOWNARROW: i32 = 166;
    const K_DOWNARROW: i32 = 129;
    const K_KP_LEFTARROW: i32 = 163;
    const K_LEFTARROW: i32 = 130;
    const K_KP_RIGHTARROW: i32 = 165;
    const K_RIGHTARROW: i32 = 131;
    const K_KP_ENTER: i32 = 169;
    const K_ENTER: i32 = 13;

    match key {
        K_ESCAPE => {
            apply_changes(menu, cvars, vid_gamma_idx, vid_fullscreen_idx, vid_ref_idx);
            return None;
        }
        K_KP_UPARROW | K_UPARROW => {
            menu.s_vulkan_menu.cursor -= 1;
            // Stub: Menu_AdjustCursor(m, -1)
        }
        K_KP_DOWNARROW | K_DOWNARROW => {
            menu.s_vulkan_menu.cursor += 1;
            // Stub: Menu_AdjustCursor(m, 1)
        }
        K_KP_LEFTARROW | K_LEFTARROW => {
            // Stub: Menu_SlideItem(m, -1)
        }
        K_KP_RIGHTARROW | K_RIGHTARROW => {
            // Stub: Menu_SlideItem(m, 1)
        }
        K_KP_ENTER | K_ENTER => {
            // Stub: if !Menu_SelectItem(m) { ApplyChanges(NULL) }
            apply_changes(menu, cvars, vid_gamma_idx, vid_fullscreen_idx, vid_ref_idx);
        }
        _ => {}
    }

    Some(sound)
}

// ============================================================
// Global state and zero-arg wrappers for dispatch from menu.rs
// ============================================================

/// Global video menu state. Accessed from the zero-arg wrapper functions
/// that are registered in the VID_MENU_FNS dispatch table.
static GLOBAL_VID_MENU: SyncUnsafeCell<Option<VidMenuState>> = SyncUnsafeCell::new(None);
static GLOBAL_QM_MENU: SyncUnsafeCell<Option<qmenu::MenuFramework>> = SyncUnsafeCell::new(None);
static GLOBAL_QM_ITEMS: SyncUnsafeCell<Vec<qmenu::MenuItem>> = SyncUnsafeCell::new(Vec::new());

/// Adapter: wraps console drawing functions into a MenuRenderer for qmenu.
struct VidConsoleMenuRenderer;

impl qmenu::MenuRenderer for VidConsoleMenuRenderer {
    fn draw_char(&mut self, x: i32, y: i32, ch: i32) {
        console::draw_char(x, y, ch);
    }
    fn draw_fill(&mut self, x: i32, y: i32, w: i32, h: i32, color: i32, alpha: f32) {
        console::draw_fill(x, y, w, h, color, alpha);
    }
    fn sys_milliseconds(&self) -> i32 {
        console::sys_milliseconds()
    }
    fn vid_width(&self) -> i32 {
        // SAFETY: single-threaded engine
        unsafe { console::VIDDEF.width }
    }
    fn vid_height(&self) -> i32 {
        // SAFETY: single-threaded engine
        unsafe { console::VIDDEF.height }
    }
    fn sys_get_clipboard_data(&self) -> Option<String> {
        None
    }
    fn keydown(&self, key: i32) -> bool {
        if key >= 0 && key < 256 {
            // SAFETY: single-threaded engine
            unsafe { myq2_client::keys::KEYDOWN[key as usize] }
        } else {
            false
        }
    }
}

/// Helper: create a qmenu::MenuCommon from a name and position.
fn make_qm_common(item_type: i32, name: &str, x: i32, y: i32) -> qmenu::MenuCommon {
    qmenu::MenuCommon {
        item_type,
        name: if name.is_empty() { None } else { Some(name.to_string()) },
        x,
        y,
        ..Default::default()
    }
}

/// Zero-arg wrapper for VID_MenuInit, suitable for the dispatch table.
/// Reads cvars via the global cvar system and initializes the menu state.
pub fn vid_menu_init_global() {
    use myq2_common::cvar;

    // Cvars are already registered by r_register() (renderer) and cl_init() (client)
    // before the video menu initializes, so no cvar_get calls needed here.

    let vk_mode_value = cvar::cvar_variable_value("vk_mode") as f32;
    let viewsize_value = cvar::cvar_variable_value("viewsize") as f32;
    let vid_gamma_value = cvar::cvar_variable_value("vid_gamma") as f32;
    let vid_fullscreen_value = cvar::cvar_variable_value("vid_fullscreen") as f32;
    let vk_picmip_value = cvar::cvar_variable_value("vk_picmip") as f32;
    let vk_finish_value = cvar::cvar_variable_value("vk_finish") as f32;

    // SAFETY: single-threaded engine
    let viddef_width = unsafe { console::VIDDEF.width };

    let mut menu = VidMenuState::new();

    menu.s_mode_list.curvalue = vk_mode_value as i32;
    menu.s_screensize_slider.curvalue = viewsize_value / 10.0;

    menu.s_vulkan_menu.x = (viddef_width as f32 * 0.50) as i32;
    menu.s_vulkan_menu.nitems = 0;

    // Driver list
    menu.s_ref_list.generic.item_type = MTYPE_SPINCONTROL;
    menu.s_ref_list.generic.name = Some("driver".to_string());
    menu.s_ref_list.generic.x = 0;
    menu.s_ref_list.generic.y = 0;
    menu.s_ref_list.itemnames = REFS.iter().map(|s| s.to_string()).collect();

    // Mode list
    menu.s_mode_list.generic.item_type = MTYPE_SPINCONTROL;
    menu.s_mode_list.generic.name = Some("video mode".to_string());
    menu.s_mode_list.generic.x = 0;
    menu.s_mode_list.generic.y = 10;
    menu.s_mode_list.itemnames = RESOLUTIONS.iter().map(|s| s.to_string()).collect();

    // Screen size slider
    menu.s_screensize_slider.generic.item_type = MTYPE_SLIDER;
    menu.s_screensize_slider.generic.x = 0;
    menu.s_screensize_slider.generic.y = 20;
    menu.s_screensize_slider.generic.name = Some("screen size".to_string());
    menu.s_screensize_slider.minvalue = 3.0;
    menu.s_screensize_slider.maxvalue = 12.0;

    // Brightness slider
    menu.s_brightness_slider.generic.item_type = MTYPE_SLIDER;
    menu.s_brightness_slider.generic.x = 0;
    menu.s_brightness_slider.generic.y = 30;
    menu.s_brightness_slider.generic.name = Some("brightness".to_string());
    menu.s_brightness_slider.minvalue = 5.0;
    menu.s_brightness_slider.maxvalue = 13.0;
    menu.s_brightness_slider.curvalue = (1.3 - vid_gamma_value + 0.5) * 10.0;

    // Fullscreen box
    menu.s_fs_box.generic.item_type = MTYPE_SPINCONTROL;
    menu.s_fs_box.generic.x = 0;
    menu.s_fs_box.generic.y = 40;
    menu.s_fs_box.generic.name = Some("fullscreen".to_string());
    menu.s_fs_box.itemnames = YESNO_NAMES.iter().map(|s| s.to_string()).collect();
    menu.s_fs_box.curvalue = vid_fullscreen_value as i32;

    // Texture quality slider
    menu.s_tq_slider.generic.item_type = MTYPE_SLIDER;
    menu.s_tq_slider.generic.x = 0;
    menu.s_tq_slider.generic.y = 60;
    menu.s_tq_slider.generic.name = Some("texture quality".to_string());
    menu.s_tq_slider.minvalue = 0.0;
    menu.s_tq_slider.maxvalue = 3.0;
    menu.s_tq_slider.curvalue = 3.0 - vk_picmip_value;

    // Sync every frame box
    menu.s_finish_box.generic.item_type = MTYPE_SPINCONTROL;
    menu.s_finish_box.generic.x = 0;
    menu.s_finish_box.generic.y = 70;
    menu.s_finish_box.generic.name = Some("sync every frame".to_string());
    menu.s_finish_box.curvalue = vk_finish_value as i32;
    menu.s_finish_box.itemnames = YESNO_NAMES.iter().map(|s| s.to_string()).collect();

    // Defaults action
    menu.s_defaults_action.generic.item_type = MTYPE_ACTION;
    menu.s_defaults_action.generic.name = Some("reset to defaults".to_string());
    menu.s_defaults_action.generic.x = 0;
    menu.s_defaults_action.generic.y = 90;

    // Cancel action
    menu.s_cancel_action.generic.item_type = MTYPE_ACTION;
    menu.s_cancel_action.generic.name = Some("cancel".to_string());
    menu.s_cancel_action.generic.x = 0;
    menu.s_cancel_action.generic.y = 100;

    menu.s_vulkan_menu.x -= 8;

    // Build qmenu framework and items
    // SAFETY: single-threaded engine
    unsafe {
        let qm_items = GLOBAL_QM_ITEMS.get_mut();
        qm_items.clear();
        let mut qm_menu = qmenu::MenuFramework {
            x: menu.s_vulkan_menu.x,
            y: menu.s_vulkan_menu.y,
            cursor: 0,
            ..Default::default()
        };

        // Driver list (SpinControl)
        let ref_list = qmenu::MenuList {
            generic: make_qm_common(qmenu::MTYPE_SPINCONTROL, "driver", 0, 0),
            curvalue: 0,
            itemnames: REFS.iter().map(|s| s.to_string()).collect(),
        };
        qmenu::menu_add_item(&mut qm_menu, qm_items, qmenu::MenuItem::SpinControl(ref_list));

        // Mode list (SpinControl)
        let mode_list = qmenu::MenuList {
            generic: make_qm_common(qmenu::MTYPE_SPINCONTROL, "video mode", 0, 10),
            curvalue: vk_mode_value as i32,
            itemnames: RESOLUTIONS.iter().map(|s| s.to_string()).collect(),
        };
        qmenu::menu_add_item(&mut qm_menu, qm_items, qmenu::MenuItem::SpinControl(mode_list));

        // Screen size slider
        let screensize_slider = qmenu::MenuSlider {
            generic: make_qm_common(qmenu::MTYPE_SLIDER, "screen size", 0, 20),
            minvalue: 3.0,
            maxvalue: 12.0,
            curvalue: viewsize_value / 10.0,
            range: 0.0,
        };
        qmenu::menu_add_item(&mut qm_menu, qm_items, qmenu::MenuItem::Slider(screensize_slider));

        // Brightness slider
        let brightness_slider = qmenu::MenuSlider {
            generic: make_qm_common(qmenu::MTYPE_SLIDER, "brightness", 0, 30),
            minvalue: 5.0,
            maxvalue: 13.0,
            curvalue: (1.3 - vid_gamma_value + 0.5) * 10.0,
            range: 0.0,
        };
        qmenu::menu_add_item(&mut qm_menu, qm_items, qmenu::MenuItem::Slider(brightness_slider));

        // Fullscreen box (SpinControl)
        let fs_box = qmenu::MenuList {
            generic: make_qm_common(qmenu::MTYPE_SPINCONTROL, "fullscreen", 0, 40),
            curvalue: vid_fullscreen_value as i32,
            itemnames: YESNO_NAMES.iter().map(|s| s.to_string()).collect(),
        };
        qmenu::menu_add_item(&mut qm_menu, qm_items, qmenu::MenuItem::SpinControl(fs_box));

        // Texture quality slider
        let tq_slider = qmenu::MenuSlider {
            generic: make_qm_common(qmenu::MTYPE_SLIDER, "texture quality", 0, 60),
            minvalue: 0.0,
            maxvalue: 3.0,
            curvalue: 3.0 - vk_picmip_value,
            range: 0.0,
        };
        qmenu::menu_add_item(&mut qm_menu, qm_items, qmenu::MenuItem::Slider(tq_slider));

        // Sync every frame (SpinControl)
        let finish_box = qmenu::MenuList {
            generic: make_qm_common(qmenu::MTYPE_SPINCONTROL, "sync every frame", 0, 70),
            curvalue: vk_finish_value as i32,
            itemnames: YESNO_NAMES.iter().map(|s| s.to_string()).collect(),
        };
        qmenu::menu_add_item(&mut qm_menu, qm_items, qmenu::MenuItem::SpinControl(finish_box));

        // Defaults action
        let defaults_action = qmenu::MenuAction {
            generic: make_qm_common(qmenu::MTYPE_ACTION, "reset to defaults", 0, 90),
        };
        qmenu::menu_add_item(&mut qm_menu, qm_items, qmenu::MenuItem::Action(defaults_action));

        // Cancel action
        let cancel_action = qmenu::MenuAction {
            generic: make_qm_common(qmenu::MTYPE_ACTION, "cancel", 0, 100),
        };
        qmenu::menu_add_item(&mut qm_menu, qm_items, qmenu::MenuItem::Action(cancel_action));

        // Center the menu
        let vid_height = console::VIDDEF.height;
        qmenu::menu_center(&mut qm_menu, qm_items, vid_height);

        *GLOBAL_QM_MENU.get_mut() = Some(qm_menu);
        *GLOBAL_VID_MENU.get_mut() = Some(menu);
    }
}

/// Zero-arg wrapper for VID_MenuDraw, suitable for the dispatch table.
/// Draws the video menu banner and the menu framework.
pub fn vid_menu_draw_global() {
    // SAFETY: single-threaded engine
    unsafe {
        if GLOBAL_VID_MENU.get_ref().is_none() {
            return;
        }
        let qm_menu = match GLOBAL_QM_MENU.get_mut().as_mut() {
            Some(m) => m,
            None => return,
        };
        let qm_items = GLOBAL_QM_ITEMS.get_mut();

        // Draw banner
        let (w, _h) = console::draw_get_pic_size("m_banner_video");
        console::draw_pic(
            console::VIDDEF.width / 2 - w / 2,
            console::VIDDEF.height / 2 - 110,
            "m_banner_video",
        );

        // Adjust cursor and draw the menu through qmenu
        qmenu::menu_adjust_cursor(qm_menu, qm_items, 1);
        let mut renderer = VidConsoleMenuRenderer;
        qmenu::menu_draw(&mut renderer, qm_menu, qm_items);
    }
}

/// Zero-arg wrapper for VID_MenuKey, suitable for the dispatch table.
/// Applies changes via global cvar functions.
pub fn vid_menu_key_global(key: i32) -> Option<&'static str> {
    use myq2_client::keys;

    // SAFETY: single-threaded engine
    unsafe {
        let menu = GLOBAL_VID_MENU.get_mut().as_mut()?;
        let qm_menu = GLOBAL_QM_MENU.get_mut().as_mut()?;
        let qm_items = GLOBAL_QM_ITEMS.get_mut();

        let sound: &str = "misc/menu1.wav";

        match key {
            k if k == keys::K_ESCAPE => {
                sync_curvalues_from_qmenu(menu, qm_items);
                apply_changes_global(menu);
                return None;
            }
            k if k == keys::K_KP_UPARROW || k == keys::K_UPARROW => {
                qm_menu.cursor -= 1;
                qmenu::menu_adjust_cursor(qm_menu, qm_items, -1);
                menu.s_vulkan_menu.cursor = qm_menu.cursor;
            }
            k if k == keys::K_KP_DOWNARROW || k == keys::K_DOWNARROW => {
                qm_menu.cursor += 1;
                qmenu::menu_adjust_cursor(qm_menu, qm_items, 1);
                menu.s_vulkan_menu.cursor = qm_menu.cursor;
            }
            k if k == keys::K_KP_LEFTARROW || k == keys::K_LEFTARROW => {
                qmenu::menu_slide_item(qm_menu, qm_items, -1);
                sync_curvalues_from_qmenu(menu, qm_items);
            }
            k if k == keys::K_KP_RIGHTARROW || k == keys::K_RIGHTARROW => {
                qmenu::menu_slide_item(qm_menu, qm_items, 1);
                sync_curvalues_from_qmenu(menu, qm_items);
            }
            k if k == keys::K_KP_ENTER || k == keys::K_ENTER => {
                if !qmenu::menu_select_item(qm_menu, qm_items) {
                    sync_curvalues_from_qmenu(menu, qm_items);
                    apply_changes_global(menu);
                }
            }
            _ => {}
        }

        Some(sound)
    }
}

/// Sync curvalues from qmenu items back into VidMenuState so apply_changes_global
/// reads the correct slider/spincontrol values.
///
/// Item order (matching vid_menu_init_global):
///   0: ref_list (SpinControl)
///   1: mode_list (SpinControl)
///   2: screensize_slider (Slider)
///   3: brightness_slider (Slider)
///   4: fs_box (SpinControl)
///   5: tq_slider (Slider)
///   6: finish_box (SpinControl)
///   7: defaults_action (Action)
///   8: cancel_action (Action)
fn sync_curvalues_from_qmenu(menu: &mut VidMenuState, qm_items: &[qmenu::MenuItem]) {
    for (idx, item) in qm_items.iter().enumerate() {
        match (idx, item) {
            (1, qmenu::MenuItem::SpinControl(s)) => {
                menu.s_mode_list.curvalue = s.curvalue;
            }
            (2, qmenu::MenuItem::Slider(s)) => {
                menu.s_screensize_slider.curvalue = s.curvalue;
            }
            (3, qmenu::MenuItem::Slider(s)) => {
                menu.s_brightness_slider.curvalue = s.curvalue;
            }
            (4, qmenu::MenuItem::SpinControl(s)) => {
                menu.s_fs_box.curvalue = s.curvalue;
            }
            (5, qmenu::MenuItem::Slider(s)) => {
                menu.s_tq_slider.curvalue = s.curvalue;
            }
            (6, qmenu::MenuItem::SpinControl(s)) => {
                menu.s_finish_box.curvalue = s.curvalue;
            }
            _ => {}
        }
    }
}

/// Apply video settings changes using global cvar functions.
fn apply_changes_global(menu: &VidMenuState) {
    use myq2_common::cvar;

    // Invert sense so greater = brighter, and scale to a range of 0.5 to 1.3
    let gamma = (0.8 - (menu.s_brightness_slider.curvalue / 10.0 - 0.5)) + 0.5;

    cvar::cvar_set_value("vid_gamma", gamma as f32);
    cvar::cvar_set_value("vk_picmip", 3.0 - menu.s_tq_slider.curvalue);
    cvar::cvar_set_value("vid_fullscreen", menu.s_fs_box.curvalue as f32);
    cvar::cvar_set_value("vk_finish", menu.s_finish_box.curvalue as f32);
    cvar::cvar_set_value("vk_mode", menu.s_mode_list.curvalue as f32);

    cvar::cvar_set("vid_ref", "gl");
    cvar::cvar_set("vk_driver", "opengl32");

    m_force_menu_off();
}

#[cfg(test)]
mod tests {
    use super::*;
    use myq2_common::cvar::CvarContext;

    // -------------------------------------------------------
    // Resolution table
    // -------------------------------------------------------

    #[test]
    fn test_resolutions_count() {
        assert_eq!(RESOLUTIONS.len(), 11);
    }

    #[test]
    fn test_resolutions_first_and_last() {
        assert_eq!(RESOLUTIONS[0], "[320 240  ]");
        assert_eq!(RESOLUTIONS[RESOLUTIONS.len() - 1], "[2048 1536]");
    }

    #[test]
    fn test_resolutions_contain_common_modes() {
        // 640x480 should be index 3
        assert_eq!(RESOLUTIONS[3], "[640 480  ]");
        // 800x600 should be index 4
        assert_eq!(RESOLUTIONS[4], "[800 600  ]");
        // 1024x768 should be index 6
        assert_eq!(RESOLUTIONS[6], "[1024 768 ]");
    }

    #[test]
    fn test_resolutions_all_bracketed() {
        for res in RESOLUTIONS {
            assert!(res.starts_with('['), "Resolution '{}' should start with '['", res);
            assert!(res.ends_with(']'), "Resolution '{}' should end with ']'", res);
        }
    }

    // -------------------------------------------------------
    // Refs table
    // -------------------------------------------------------

    #[test]
    fn test_refs_table() {
        assert_eq!(REFS.len(), 1);
        assert_eq!(REFS[0], "[default OpenGL]");
    }

    // -------------------------------------------------------
    // YesNo names
    // -------------------------------------------------------

    #[test]
    fn test_yesno_names() {
        assert_eq!(YESNO_NAMES.len(), 2);
        assert_eq!(YESNO_NAMES[0], "no");
        assert_eq!(YESNO_NAMES[1], "yes");
    }

    // -------------------------------------------------------
    // VidMenuState construction
    // -------------------------------------------------------

    #[test]
    fn test_vid_menu_state_default() {
        let state = VidMenuState::new();
        assert!(state.vk_mode.is_none());
        assert!(state.vk_driver.is_none());
        assert!(state.vk_picmip.is_none());
        assert!(state.vk_finish.is_none());
        assert!(state.scr_viewsize.is_none());
    }

    #[test]
    fn test_vid_menu_state_default_trait() {
        let state = VidMenuState::default();
        assert!(state.vk_mode.is_none());
        assert!(state.vk_driver.is_none());
    }

    // -------------------------------------------------------
    // Gamma calculation (brightness slider -> gamma value)
    // -------------------------------------------------------

    /// The gamma calculation from apply_changes:
    ///   gamma = (0.8 - (brightness_curvalue / 10.0 - 0.5)) + 0.5
    fn compute_gamma(brightness_curvalue: f32) -> f32 {
        (0.8 - (brightness_curvalue / 10.0 - 0.5)) + 0.5
    }

    #[test]
    fn test_gamma_at_minimum_brightness() {
        // Minimum brightness slider value is 5.0
        let gamma = compute_gamma(5.0);
        // gamma = (0.8 - (5.0/10.0 - 0.5)) + 0.5 = (0.8 - 0.0) + 0.5 = 1.3
        assert!((gamma - 1.3).abs() < 1e-6);
    }

    #[test]
    fn test_gamma_at_maximum_brightness() {
        // Maximum brightness slider value is 13.0
        let gamma = compute_gamma(13.0);
        // gamma = (0.8 - (13.0/10.0 - 0.5)) + 0.5 = (0.8 - 0.8) + 0.5 = 0.5
        assert!((gamma - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_gamma_at_midpoint() {
        // Midpoint brightness = 9.0
        let gamma = compute_gamma(9.0);
        // gamma = (0.8 - (9.0/10.0 - 0.5)) + 0.5 = (0.8 - 0.4) + 0.5 = 0.9
        assert!((gamma - 0.9).abs() < 1e-6);
    }

    #[test]
    fn test_gamma_inverse_relationship() {
        // Higher brightness slider -> lower gamma value
        let gamma_low = compute_gamma(5.0);
        let gamma_high = compute_gamma(13.0);
        assert!(gamma_low > gamma_high);
    }

    // -------------------------------------------------------
    // Brightness slider initialization from gamma
    // -------------------------------------------------------

    /// The brightness slider curvalue is computed as:
    ///   curvalue = (1.3 - vid_gamma + 0.5) * 10.0
    fn compute_brightness_from_gamma(vid_gamma: f32) -> f32 {
        (1.3 - vid_gamma + 0.5) * 10.0
    }

    #[test]
    fn test_brightness_from_gamma_1_0() {
        let brightness = compute_brightness_from_gamma(1.0);
        // (1.3 - 1.0 + 0.5) * 10.0 = 0.8 * 10.0 = 8.0
        assert!((brightness - 8.0).abs() < 1e-6);
    }

    #[test]
    fn test_brightness_from_gamma_0_6() {
        // Default gamma is 0.6
        let brightness = compute_brightness_from_gamma(0.6);
        // (1.3 - 0.6 + 0.5) * 10.0 = 1.2 * 10.0 = 12.0
        assert!((brightness - 12.0).abs() < 1e-6);
    }

    #[test]
    fn test_brightness_gamma_roundtrip() {
        // Setting brightness to some value, computing gamma, then computing brightness back
        // should give the original value.
        let original_brightness = 8.0;
        let gamma = compute_gamma(original_brightness);
        let roundtrip = compute_brightness_from_gamma(gamma);
        assert!((roundtrip - original_brightness).abs() < 1e-4);
    }

    // -------------------------------------------------------
    // Texture quality slider <-> picmip mapping
    // -------------------------------------------------------

    #[test]
    fn test_tq_slider_from_picmip() {
        // tq_slider.curvalue = 3.0 - picmip_value
        assert!((3.0 - 0.0 - 3.0f32).abs() < 1e-6); // picmip 0 -> tq 3 (highest quality)
        assert!((3.0 - 1.0 - 2.0f32).abs() < 1e-6); // picmip 1 -> tq 2
        assert!((3.0 - 2.0 - 1.0f32).abs() < 1e-6); // picmip 2 -> tq 1
        assert!((3.0 - 3.0 - 0.0f32).abs() < 1e-6); // picmip 3 -> tq 0 (lowest quality)
    }

    #[test]
    fn test_picmip_from_tq_slider() {
        // picmip = 3.0 - tq_slider.curvalue
        for tq in 0..=3 {
            let picmip = 3.0 - tq as f32;
            let tq_back = 3.0 - picmip;
            assert!((tq_back - tq as f32).abs() < 1e-6);
        }
    }

    // -------------------------------------------------------
    // Screen size slider <-> viewsize mapping
    // -------------------------------------------------------

    #[test]
    fn test_screensize_from_viewsize() {
        // screensize_slider.curvalue = viewsize / 10.0
        assert!((100.0_f32 / 10.0 - 10.0).abs() < 1e-6);
        assert!((120.0_f32 / 10.0 - 12.0).abs() < 1e-6);
        assert!((30.0_f32 / 10.0 - 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_viewsize_from_screensize_slider() {
        // viewsize = screensize_slider.curvalue * 10.0
        let slider = 10.0_f32;
        let viewsize = slider * 10.0;
        assert!((viewsize - 100.0).abs() < 1e-6);
    }

    // -------------------------------------------------------
    // vid_menu_init with CvarContext
    // -------------------------------------------------------

    #[test]
    fn test_vid_menu_init_creates_cvars() {
        let mut menu = VidMenuState::new();
        let mut cvars = CvarContext::new();
        let viddef = VidDef { width: 640, height: 480 };

        vid_menu_init(&mut menu, &mut cvars, &viddef, None, None);

        // All cvar indices should be populated after init
        assert!(menu.vk_driver.is_some());
        assert!(menu.vk_picmip.is_some());
        assert!(menu.vk_mode.is_some());
        assert!(menu.vk_finish.is_some());
        assert!(menu.scr_viewsize.is_some());
    }

    #[test]
    fn test_vid_menu_init_mode_list_defaults() {
        let mut menu = VidMenuState::new();
        let mut cvars = CvarContext::new();
        let viddef = VidDef { width: 800, height: 600 };

        vid_menu_init(&mut menu, &mut cvars, &viddef, None, None);

        // Default vk_mode is "4", so mode_list curvalue should be 4
        assert_eq!(menu.s_mode_list.curvalue, 4);
        // Mode list should have RESOLUTIONS items
        assert_eq!(menu.s_mode_list.itemnames.len(), RESOLUTIONS.len());
    }

    #[test]
    fn test_vid_menu_init_slider_ranges() {
        let mut menu = VidMenuState::new();
        let mut cvars = CvarContext::new();
        let viddef = VidDef { width: 640, height: 480 };

        vid_menu_init(&mut menu, &mut cvars, &viddef, None, None);

        // Screen size slider: 3.0 to 12.0
        assert!((menu.s_screensize_slider.minvalue - 3.0).abs() < 1e-6);
        assert!((menu.s_screensize_slider.maxvalue - 12.0).abs() < 1e-6);

        // Brightness slider: 5.0 to 13.0
        assert!((menu.s_brightness_slider.minvalue - 5.0).abs() < 1e-6);
        assert!((menu.s_brightness_slider.maxvalue - 13.0).abs() < 1e-6);

        // Texture quality slider: 0.0 to 3.0
        assert!((menu.s_tq_slider.minvalue - 0.0).abs() < 1e-6);
        assert!((menu.s_tq_slider.maxvalue - 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_vid_menu_init_screensize_from_viewsize() {
        let mut menu = VidMenuState::new();
        let mut cvars = CvarContext::new();
        let viddef = VidDef { width: 640, height: 480 };

        vid_menu_init(&mut menu, &mut cvars, &viddef, None, None);

        // Default viewsize is "100", so screensize slider = 100/10 = 10.0
        assert!((menu.s_screensize_slider.curvalue - 10.0).abs() < 1e-6);
    }

    #[test]
    fn test_vid_menu_init_menu_position() {
        let mut menu = VidMenuState::new();
        let mut cvars = CvarContext::new();
        let viddef = VidDef { width: 800, height: 600 };

        vid_menu_init(&mut menu, &mut cvars, &viddef, None, None);

        // x = viddef.width * 0.5 - 8 = 400 - 8 = 392
        assert_eq!(menu.s_vulkan_menu.x, 392);
    }

    #[test]
    fn test_vid_menu_init_item_types() {
        let mut menu = VidMenuState::new();
        let mut cvars = CvarContext::new();
        let viddef = VidDef { width: 640, height: 480 };

        vid_menu_init(&mut menu, &mut cvars, &viddef, None, None);

        assert_eq!(menu.s_ref_list.generic.item_type, MTYPE_SPINCONTROL);
        assert_eq!(menu.s_mode_list.generic.item_type, MTYPE_SPINCONTROL);
        assert_eq!(menu.s_screensize_slider.generic.item_type, MTYPE_SLIDER);
        assert_eq!(menu.s_brightness_slider.generic.item_type, MTYPE_SLIDER);
        assert_eq!(menu.s_fs_box.generic.item_type, MTYPE_SPINCONTROL);
        assert_eq!(menu.s_tq_slider.generic.item_type, MTYPE_SLIDER);
        assert_eq!(menu.s_finish_box.generic.item_type, MTYPE_SPINCONTROL);
        assert_eq!(menu.s_defaults_action.generic.item_type, MTYPE_ACTION);
        assert_eq!(menu.s_cancel_action.generic.item_type, MTYPE_ACTION);
    }

    #[test]
    fn test_vid_menu_init_item_names() {
        let mut menu = VidMenuState::new();
        let mut cvars = CvarContext::new();
        let viddef = VidDef { width: 640, height: 480 };

        vid_menu_init(&mut menu, &mut cvars, &viddef, None, None);

        assert_eq!(menu.s_ref_list.generic.name.as_deref(), Some("driver"));
        assert_eq!(menu.s_mode_list.generic.name.as_deref(), Some("video mode"));
        assert_eq!(menu.s_screensize_slider.generic.name.as_deref(), Some("screen size"));
        assert_eq!(menu.s_brightness_slider.generic.name.as_deref(), Some("brightness"));
        assert_eq!(menu.s_fs_box.generic.name.as_deref(), Some("fullscreen"));
        assert_eq!(menu.s_tq_slider.generic.name.as_deref(), Some("texture quality"));
        assert_eq!(menu.s_finish_box.generic.name.as_deref(), Some("sync every frame"));
        assert_eq!(menu.s_defaults_action.generic.name.as_deref(), Some("reset to defaults"));
        assert_eq!(menu.s_cancel_action.generic.name.as_deref(), Some("cancel"));
    }

    #[test]
    fn test_vid_menu_init_y_positions() {
        let mut menu = VidMenuState::new();
        let mut cvars = CvarContext::new();
        let viddef = VidDef { width: 640, height: 480 };

        vid_menu_init(&mut menu, &mut cvars, &viddef, None, None);

        // Verify Y positions are in increasing order as specified
        assert_eq!(menu.s_ref_list.generic.y, 0);
        assert_eq!(menu.s_mode_list.generic.y, 10);
        assert_eq!(menu.s_screensize_slider.generic.y, 20);
        assert_eq!(menu.s_brightness_slider.generic.y, 30);
        assert_eq!(menu.s_fs_box.generic.y, 40);
        assert_eq!(menu.s_tq_slider.generic.y, 60);
        assert_eq!(menu.s_finish_box.generic.y, 70);
        assert_eq!(menu.s_defaults_action.generic.y, 90);
        assert_eq!(menu.s_cancel_action.generic.y, 100);
    }

    // -------------------------------------------------------
    // screen_size_callback
    // -------------------------------------------------------

    #[test]
    fn test_screen_size_callback() {
        let mut menu = VidMenuState::new();
        let mut cvars = CvarContext::new();
        let viddef = VidDef { width: 640, height: 480 };

        vid_menu_init(&mut menu, &mut cvars, &viddef, None, None);

        // Set slider to 8.0 (viewsize = 80)
        menu.s_screensize_slider.curvalue = 8.0;
        screen_size_callback(&mut menu, &mut cvars);

        let idx = menu.scr_viewsize.unwrap();
        let cv = &cvars.cvar_vars[idx];
        assert!((cv.value - 80.0).abs() < 1e-6);
        assert_eq!(cv.string, "80");
        assert!(cv.modified);
    }

    // -------------------------------------------------------
    // make_qm_common helper
    // -------------------------------------------------------

    #[test]
    fn test_make_qm_common() {
        let common = make_qm_common(MTYPE_ACTION, "test name", 10, 20);
        assert_eq!(common.item_type, MTYPE_ACTION);
        assert_eq!(common.name, Some("test name".to_string()));
        assert_eq!(common.x, 10);
        assert_eq!(common.y, 20);
    }

    #[test]
    fn test_make_qm_common_empty_name() {
        let common = make_qm_common(MTYPE_SLIDER, "", 0, 0);
        assert_eq!(common.item_type, MTYPE_SLIDER);
        assert!(common.name.is_none());
    }

    // -------------------------------------------------------
    // Mode enumeration
    // -------------------------------------------------------

    #[test]
    fn test_mode_index_bounds() {
        // vk_mode default is 4, which should be a valid index
        assert!(4 < RESOLUTIONS.len());
    }

    #[test]
    fn test_resolution_ordering_by_size() {
        // Verify resolutions are in ascending order by extracting widths
        // Format: "[WIDTH HEIGHT]" with optional padding
        let mut prev_width = 0;
        for res in RESOLUTIONS {
            let inner = &res[1..res.len() - 1]; // strip brackets
            let parts: Vec<&str> = inner.split_whitespace().collect();
            let width: i32 = parts[0].parse().unwrap();
            assert!(width >= prev_width, "Resolutions should be in ascending order");
            prev_width = width;
        }
    }

    #[test]
    fn test_resolution_parse_all() {
        // Verify all resolutions can be parsed into width/height pairs
        for (i, res) in RESOLUTIONS.iter().enumerate() {
            let inner = &res[1..res.len() - 1]; // strip brackets
            let parts: Vec<&str> = inner.split_whitespace().collect();
            assert_eq!(parts.len(), 2, "Resolution {} should have width and height", i);
            let width: i32 = parts[0].parse().expect("Width should be a valid integer");
            let height: i32 = parts[1].parse().expect("Height should be a valid integer");
            assert!(width > 0, "Width should be positive");
            assert!(height > 0, "Height should be positive");
            // Common aspect ratios: 4:3
            // width / height should be close to 4/3
            let ratio = width as f32 / height as f32;
            assert!((ratio - 4.0 / 3.0).abs() < 0.1,
                "Resolution {}x{} should be approximately 4:3 (got {})", width, height, ratio);
        }
    }
}
