// cl_view.rs -- player rendering positioning
// Converted from: myq2-original/client/cl_view.c

use crate::client::*;
use crate::client::{MAX_DLIGHTS, MAX_ENTITIES, MAX_PARTICLES, MAX_CLIENTWEAPONMODELS, MAX_LIGHTSTYLES};
use crate::cl_scrn::*;
use crate::cl_fx::PT_DEFAULT;
use crate::console::{
    cmd_add_command, cmd_argc, cmd_argv, cvar_get,
    cvar_value, cvar_value_str, cvar_modified, cvar_clear_modified, draw_pic, r_render_frame, r_begin_registration, r_end_registration,
    r_register_model, r_set_sky,
    sys_milliseconds,
    cl_add_entities, cl_add_entities_value, cl_add_lights_value,
    cl_add_particles_value, cl_add_blend_value, cl_timedemo_value, cl_paused_value, cl_register_tent_models,
    cl_load_clientinfo, cl_parse_clientinfo, cm_inline_model, con_clear_notify,
    log_stats_value, log_stats_file_open, log_stats_write, sys_send_key_events,
    v_gun_model_f_cmd, v_gun_next_f_cmd, v_gun_prev_f_cmd, v_viewpos_f_cmd,
    VidDef, draw_find_pic,
};
use myq2_common::q_shared::*;
use myq2_common::common::{com_printf, com_error};

use std::f32::consts::PI;

// ============================================================
// Module-level state
// ============================================================

pub struct ViewState {
    // development tools for weapons
    pub gun_frame: i32,
    pub gun_model: ModelHandle, // opaque handle, 0 = none

    // cvars
    pub crosshair: CvarHandle,
    pub cl_testparticles: CvarHandle,
    pub cl_testentities: CvarHandle,
    pub cl_testlights: CvarHandle,
    pub cl_testblend: CvarHandle,
    pub cl_stats: CvarHandle,

    // scene arrays
    pub r_numdlights: i32,
    pub r_dlights: Vec<DLight>,

    pub r_numentities: i32,
    pub r_entities: Vec<Entity>,

    pub r_numparticles: i32,
    pub r_particles: Vec<Particle>,

    pub r_lightstyles: Vec<LightStyle>,

    pub cl_weaponmodels: Vec<String>,
    pub num_cl_weaponmodels: i32,
}

/// Opaque model handle (index or pointer equivalent).
pub type ModelHandle = i32;

impl Default for ViewState {
    fn default() -> Self {
        Self {
            gun_frame: 0,
            gun_model: 0,
            crosshair: 0,
            cl_testparticles: 0,
            cl_testentities: 0,
            cl_testlights: 0,
            cl_testblend: 0,
            cl_stats: 0,
            r_numdlights: 0,
            r_dlights: vec![DLight::default(); MAX_DLIGHTS],
            r_numentities: 0,
            r_entities: vec![Entity::default(); MAX_ENTITIES],
            r_numparticles: 0,
            r_particles: vec![Particle::default(); MAX_PARTICLES],
            r_lightstyles: vec![LightStyle::default(); MAX_LIGHTSTYLES],
            cl_weaponmodels: vec![String::new(); MAX_CLIENTWEAPONMODELS],
            num_cl_weaponmodels: 0,
        }
    }
}

// PT_DEFAULT comes from crate::cl_part

// ============================================================
// V_ClearScene
// ============================================================

/// Specifies the model that will be used as the world
pub fn v_clear_scene(view: &mut ViewState) {
    view.r_numdlights = 0;
    view.r_numentities = 0;
    view.r_numparticles = 0;
}

// ============================================================
// V_AddEntity
// ============================================================

pub fn v_add_entity(view: &mut ViewState, ent: &Entity) {
    if view.r_numentities >= MAX_ENTITIES as i32 {
        return;
    }
    view.r_entities[view.r_numentities as usize] = ent.clone();
    view.r_numentities += 1;
}

// ============================================================
// V_AddParticle
// ============================================================

pub fn v_add_particle(
    view: &mut ViewState,
    org: &Vec3,
    length: &Vec3,
    color: i32,
    alpha: f32,
    ptype: i32,
) {
    if view.r_numparticles >= MAX_PARTICLES as i32 {
        return;
    }
    let idx = view.r_numparticles as usize;
    view.r_particles[idx].origin = *org;
    view.r_particles[idx].length = *length;
    view.r_particles[idx].color = color;
    view.r_particles[idx].alpha = alpha;
    view.r_particles[idx].particle_type = ptype;
    view.r_numparticles += 1;
}

// ============================================================
// V_AddLight
// ============================================================

pub fn v_add_light(view: &mut ViewState, org: &Vec3, intensity: f32, r: f32, g: f32, b: f32) {
    if view.r_numdlights >= MAX_DLIGHTS as i32 {
        return;
    }
    let idx = view.r_numdlights as usize;
    view.r_dlights[idx].origin = *org;
    view.r_dlights[idx].intensity = intensity;
    view.r_dlights[idx].color[0] = r;
    view.r_dlights[idx].color[1] = g;
    view.r_dlights[idx].color[2] = b;
    view.r_numdlights += 1;
}

// ============================================================
// V_AddLightStyle
// ============================================================

pub fn v_add_light_style(view: &mut ViewState, style: i32, r: f32, g: f32, b: f32) {
    if style < 0 || style as usize > MAX_LIGHTSTYLES {
        com_error(ERR_DROP, &format!("Bad light style {}", style));
    }
    let ls = &mut view.r_lightstyles[style as usize];
    ls.white = r + g + b;
    ls.rgb[0] = r;
    ls.rgb[1] = g;
    ls.rgb[2] = b;
}

// ============================================================
// V_TestParticles
// ============================================================

/// If cl_testparticles is set, create 4096 particles in the view
pub fn v_test_particles(view: &mut ViewState, cl: &ClientState) {
    view.r_numparticles = MAX_PARTICLES as i32;
    for i in 0..view.r_numparticles as usize {
        let d = i as f32 * 0.25;
        let r = 4.0 * ((i & 7) as f32 - 3.5);
        let u = 4.0 * (((i >> 3) & 7) as f32 - 3.5);
        let p = &mut view.r_particles[i];

        for j in 0..3 {
            p.origin[j] = cl.refdef.vieworg[j]
                + cl.v_forward[j] * d
                + cl.v_right[j] * r
                + cl.v_up[j] * u;
        }

        p.color = 8;
        p.particle_type = PT_DEFAULT;
        p.alpha = cvar_value(view.cl_testparticles);
    }
}

// ============================================================
// V_TestEntities
// ============================================================

/// If cl_testentities is set, create 32 player models
pub fn v_test_entities(view: &mut ViewState, cl: &ClientState) {
    view.r_numentities = 32;
    for ent in view.r_entities.iter_mut().take(32) {
        *ent = Entity::default();
    }

    for i in 0..view.r_numentities as usize {
        let ent = &mut view.r_entities[i];

        let r = 64.0 * ((i % 4) as f32 - 1.5);
        let f = 64.0 * (i / 4) as f32 + 128.0;

        for j in 0..3 {
            ent.origin[j] = cl.refdef.vieworg[j]
                + cl.v_forward[j] * f
                + cl.v_right[j] * r;
        }

        ent.model = cl.baseclientinfo.model;
        ent.skin = cl.baseclientinfo.skin;
    }
}

// ============================================================
// V_TestLights
// ============================================================

/// If cl_testlights is set, create 32 lights models
pub fn v_test_lights(view: &mut ViewState, cl: &ClientState) {
    view.r_numdlights = 32;
    for dl in view.r_dlights.iter_mut().take(32) {
        *dl = DLight::default();
    }

    for i in 0..view.r_numdlights as usize {
        let dl = &mut view.r_dlights[i];

        let r = 64.0 * ((i % 4) as f32 - 1.5);
        let f = 64.0 * (i / 4) as f32 + 128.0;

        for j in 0..3 {
            dl.origin[j] = cl.refdef.vieworg[j]
                + cl.v_forward[j] * f
                + cl.v_right[j] * r;
        }
        dl.color[0] = (((i % 6) + 1) & 1) as f32;
        dl.color[1] = ((((i % 6) + 1) & 2) >> 1) as f32;
        dl.color[2] = ((((i % 6) + 1) & 4) >> 2) as f32;
        dl.intensity = 200.0;
    }
}

// ============================================================
// CL_PrepRefresh
// ============================================================

/// Call before entering a new level, or after changing dlls
pub fn cl_prep_refresh(
    view: &mut ViewState,
    scr: &mut ScrState,
    cls: &mut ClientStatic,
    cl: &mut ClientState,
    viddef: &VidDef,
) {
    if cl.configstrings[CS_MODELS + 1].is_empty() {
        return; // no map loaded
    }

    scr_add_dirty_point(scr, 0, 0);
    scr_add_dirty_point(scr, viddef.width - 1, viddef.height - 1);

    // let the render dll load the map
    let cs = &cl.configstrings[CS_MODELS + 1];
    let mapname = if cs.len() > 5 {
        let without_prefix = &cs[5..]; // skip "maps/"
        if without_prefix.len() > 4 {
            &without_prefix[..without_prefix.len() - 4] // cut off ".bsp"
        } else {
            without_prefix
        }
    } else {
        cs.as_str()
    };
    let mapname = mapname.to_string();

    // Load locations for this map (R1Q2/Q2Pro feature)
    crate::cl_loc::loc_load_map(&mapname, &cl.gamedir);

    // register models, pics, and skins
    com_printf(&format!("Map: {}\r", mapname));
    scr_update_screen(scr, cls, cl);
    r_begin_registration(&mapname);
    com_printf("                                     \r");

    // precache status bar pics
    com_printf("pics\r");
    scr_update_screen(scr, cls, cl);
    scr_touch_pics(scr);
    com_printf("                                     \r");

    cl_register_tent_models();

    view.num_cl_weaponmodels = 1;
    view.cl_weaponmodels[0] = "weapon.md2".to_string();

    let mut i = 1;
    while i < MAX_MODELS && !cl.configstrings[CS_MODELS + i].is_empty() {
        let mut name = cl.configstrings[CS_MODELS + i].clone();
        name.truncate(37); // never go beyond one line
        if !name.starts_with('*') {
            com_printf(&format!("{}\r", name));
        }
        scr_update_screen(scr, cls, cl);
        sys_send_key_events(); // pump message loop
        if name.starts_with('#') {
            // special player weapon model
            if (view.num_cl_weaponmodels as usize) < MAX_CLIENTWEAPONMODELS {
                view.cl_weaponmodels[view.num_cl_weaponmodels as usize] =
                    cl.configstrings[CS_MODELS + i][1..].to_string();
                view.num_cl_weaponmodels += 1;
            }
        } else {
            cl.model_draw[i] = r_register_model(&cl.configstrings[CS_MODELS + i]);
            if name.starts_with('*') {
                cl.model_clip[i] = cm_inline_model(&cl.configstrings[CS_MODELS + i]);
            } else {
                cl.model_clip[i] = 0;
            }
        }
        if !name.starts_with('*') {
            com_printf("                                     \r");
        }
        i += 1;
    }

    com_printf("images\r");
    scr_update_screen(scr, cls, cl);
    i = 1;
    while i < MAX_IMAGES && !cl.configstrings[CS_IMAGES + i].is_empty() {
        cl.image_precache[i] = draw_find_pic(&cl.configstrings[CS_IMAGES + i]);
        sys_send_key_events(); // pump message loop
        i += 1;
    }

    com_printf("                                     \r");
    for i in 0..MAX_CLIENTS {
        if cl.configstrings[CS_PLAYERSKINS + i].is_empty() {
            continue;
        }
        com_printf(&format!("client {}\r", i));
        scr_update_screen(scr, cls, cl);
        sys_send_key_events(); // pump message loop
        cl_parse_clientinfo(cl, i);
        com_printf("                                     \r");
    }

    cl_load_clientinfo(&mut cl.baseclientinfo, "Player\\male/grunt");

    // set sky textures and speed
    com_printf("sky\r");
    scr_update_screen(scr, cls, cl);
    let rotate = cl.configstrings[CS_SKYROTATE].parse::<f32>().unwrap_or(0.0);
    let axis_str = &cl.configstrings[CS_SKYAXIS];
    let parts: Vec<f32> = axis_str
        .split_whitespace()
        .filter_map(|s| s.parse::<f32>().ok())
        .collect();
    let axis = [
        parts.first().copied().unwrap_or(0.0),
        parts.get(1).copied().unwrap_or(0.0),
        parts.get(2).copied().unwrap_or(0.0),
    ];
    r_set_sky(&cl.configstrings[CS_SKY], rotate, &axis);
    com_printf("                                     \r");

    // the renderer can now free unneeded stuff
    r_end_registration();

    // clear any lines of console text
    con_clear_notify();

    scr_update_screen(scr, cls, cl);
    cl.refresh_prepped = true;
    cl.force_refdef = true; // make sure we have a valid refdef

}

// ============================================================
// CalcFov
// ============================================================

pub fn calc_fov(fov_x: f32, width: f32, height: f32) -> f32 {
    if !(1.0..=179.0).contains(&fov_x) {
        com_error(ERR_DROP, &format!("Bad fov: {}", fov_x));
    }

    let x = width / (fov_x / 360.0 * PI).tan();
    let a = (height / x).atan();
    a * 360.0 / PI
}

// ============================================================
// Gun frame debugging functions
// ============================================================

pub fn v_gun_next_f(view: &mut ViewState) {
    view.gun_frame += 1;
    com_printf(&format!("frame {}\n", view.gun_frame));
}

pub fn v_gun_prev_f(view: &mut ViewState) {
    view.gun_frame -= 1;
    if view.gun_frame < 0 {
        view.gun_frame = 0;
    }
    com_printf(&format!("frame {}\n", view.gun_frame));
}

pub fn v_gun_model_f(view: &mut ViewState) {
    if cmd_argc() != 2 {
        view.gun_model = 0;
        return;
    }
    let name = format!("models/{}/tris.md2", cmd_argv(1));
    view.gun_model = r_register_model(&name);
}

// ============================================================
// SCR_DrawCrosshair
// ============================================================

pub fn scr_draw_crosshair(view: &ViewState, scr: &ScrState) {
    scr_draw_crosshair_with_health(view, scr, None);
}

/// Draw crosshair with optional health-based coloring (R1Q2/Q2Pro ch_health feature)
pub fn scr_draw_crosshair_with_health(view: &ViewState, scr: &ScrState, health: Option<i32>) {
    if cvar_value(view.crosshair) == 0.0 {
        return;
    }

    if cvar_modified(view.crosshair) {
        cvar_clear_modified(view.crosshair);
        // Update procedural crosshair config from cvars
        crate::cl_crosshair::crosshair_update_config();
        // SCR_TouchPics is called separately for image-based crosshairs
    }

    // Calculate screen center position
    let center_x = scr.scr_vrect.x + (scr.scr_vrect.width >> 1);
    let center_y = scr.scr_vrect.y + (scr.scr_vrect.height >> 1);

    // Check if using procedural crosshair (styles 1-5)
    if crate::cl_crosshair::crosshair_is_procedural() {
        // Use health-based coloring if enabled and health is provided
        if let Some(h) = health {
            if crate::cl_crosshair::crosshair_health_enabled() {
                crate::cl_crosshair::crosshair_draw_with_health(center_x, center_y, h);
                return;
            }
        }
        crate::cl_crosshair::crosshair_draw(center_x, center_y);
        return;
    }

    // Fall back to image-based crosshair (style 6+)
    if scr.crosshair_pic.is_empty() {
        return;
    }

    draw_pic(
        scr.scr_vrect.x + ((scr.scr_vrect.width - scr.crosshair_width) >> 1),
        scr.scr_vrect.y + ((scr.scr_vrect.height - scr.crosshair_height) >> 1),
        &scr.crosshair_pic,
    );
}

// ============================================================
// V_RenderView
// ============================================================

pub fn v_render_view(
    scr: &mut ScrState,
    cls: &ClientStatic,
    cl: &mut ClientState,
    _viddef: &VidDef,
    stereo_separation: f32,
) {
    if cls.state != ConnState::Active {
        return;
    }

    if !cl.refresh_prepped {
        return; // still loading
    }

    if cl_timedemo_value() != 0.0 {
        if cl.timedemo_start == 0 {
            cl.timedemo_start = sys_milliseconds();
        }
        cl.timedemo_frames += 1;
    }

    // an invalid frame will just use the exact previous refdef
    // we can't use the old frame if the video mode has changed, though...
    if cl.frame.valid && (cl.force_refdef || cl_paused_value() == 0.0) {
        cl.force_refdef = false;

        let mut view = ViewState::default();
        v_clear_scene(&mut view);

        // build a refresh entity list and calc cl.sim*
        // this also calls CL_CalcViewValues which loads v_forward, etc.
        cl_add_entities(cl);

        if cvar_value_str("cl_testparticles") != 0.0 {
            v_test_particles(&mut view, cl);
        }
        if cvar_value_str("cl_testentities") != 0.0 {
            v_test_entities(&mut view, cl);
        }
        if cvar_value_str("cl_testlights") != 0.0 {
            v_test_lights(&mut view, cl);
        }
        if cvar_value_str("cl_testblend") != 0.0 {
            cl.refdef.blend[0] = 1.0;
            cl.refdef.blend[1] = 0.5;
            cl.refdef.blend[2] = 0.25;
            cl.refdef.blend[3] = 0.5;
        }

        // offset vieworg appropriately if we're doing stereo separation
        if stereo_separation != 0.0 {
            let tmp = vector_scale(&cl.v_right, stereo_separation);
            cl.refdef.vieworg = vector_add(&cl.refdef.vieworg, &tmp);
        }

        // never let it sit exactly on a node line, because a water plane can
        // disappear when viewed with the eye exactly on it.
        // the server protocol only specifies to 1/8 pixel, so add 1/16 in each axis
        cl.refdef.vieworg[0] += 1.0 / 16.0;
        cl.refdef.vieworg[1] += 1.0 / 16.0;
        cl.refdef.vieworg[2] += 1.0 / 16.0;

        cl.refdef.x = scr.scr_vrect.x;
        cl.refdef.y = scr.scr_vrect.y;
        cl.refdef.width = scr.scr_vrect.width;
        cl.refdef.height = scr.scr_vrect.height;
        cl.refdef.fov_y = calc_fov(
            cl.refdef.fov_x,
            cl.refdef.width as f32,
            cl.refdef.height as f32,
        );
        cl.refdef.time = cl.time as f32 * 0.001;

        cl.refdef.areabits = cl.frame.areabits.to_vec();

        if cl_add_entities_value() == 0.0 {
            view.r_numentities = 0;
        }
        if cl_add_particles_value() == 0.0 {
            view.r_numparticles = 0;
        }
        if cl_add_lights_value() == 0.0 {
            view.r_numdlights = 0;
        }
        if cl_add_blend_value() == 0.0 {
            cl.refdef.blend = [0.0; 4];
        }

        cl.refdef.num_entities = view.r_numentities;
        cl.refdef.num_particles = view.r_numparticles;
        cl.refdef.num_dlights = view.r_numdlights;

        cl.refdef.rdflags = cl.frame.playerstate.rdflags;

        // sort entities for better cache locality
        let num_ents = cl.refdef.num_entities as usize;
        view.r_entities[..num_ents].sort_by(entity_cmp_fnc);
    }

    r_render_frame(&cl.refdef);
    if cvar_value_str("cl_stats") != 0.0 {
        com_printf(&format!(
            "ent:{}  lt:{}  part:{}\n",
            cl.refdef.num_entities, cl.refdef.num_dlights, cl.refdef.num_particles
        ));
    }
    if log_stats_value() != 0.0 && log_stats_file_open() {
        log_stats_write(&format!(
            "{},{},{},",
            cl.refdef.num_entities, cl.refdef.num_dlights, cl.refdef.num_particles
        ));
    }

    scr_add_dirty_point(scr, scr.scr_vrect.x, scr.scr_vrect.y);
    scr_add_dirty_point(
        scr,
        scr.scr_vrect.x + scr.scr_vrect.width - 1,
        scr.scr_vrect.y + scr.scr_vrect.height - 1,
    );

    // Update dynamic crosshair expansion based on player movement and attack state
    let moving = cl.cmd.forwardmove != 0 || cl.cmd.sidemove != 0;
    let attacking = cl.cmd.buttons & BUTTON_ATTACK != 0;
    crate::cl_crosshair::crosshair_update_dynamic(moving, attacking, cls.frametime);

    // Get player health for ch_health crosshair coloring (R1Q2/Q2Pro feature)
    let health = cl.frame.playerstate.stats[STAT_HEALTH as usize] as i32;
    scr_draw_crosshair_with_health(&ViewState::default(), scr, Some(health));
}

// ============================================================
// V_Viewpos_f
// ============================================================

pub fn v_viewpos_f(cl: &ClientState) {
    com_printf(&format!(
        "({} {} {}) : {}\n",
        cl.refdef.vieworg[0] as i32,
        cl.refdef.vieworg[1] as i32,
        cl.refdef.vieworg[2] as i32,
        cl.refdef.viewangles[YAW] as i32
    ));
}

// ============================================================
// V_Init
// ============================================================

pub fn v_init(view: &mut ViewState) {
    cmd_add_command("gun_next", v_gun_next_f_cmd);
    cmd_add_command("gun_prev", v_gun_prev_f_cmd);
    cmd_add_command("gun_model", v_gun_model_f_cmd);

    cmd_add_command("viewpos", v_viewpos_f_cmd);

    view.crosshair = cvar_get("crosshair", "1", CVAR_ARCHIVE);

    view.cl_testblend = cvar_get("cl_testblend", "0", CVAR_ZERO);
    view.cl_testparticles = cvar_get("cl_testparticles", "0", CVAR_ZERO);
    view.cl_testentities = cvar_get("cl_testentities", "0", CVAR_ZERO);
    view.cl_testlights = cvar_get("cl_testlights", "0", CVAR_ZERO);

    view.cl_stats = cvar_get("cl_stats", "0", CVAR_ZERO);
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{MAX_DLIGHTS, MAX_ENTITIES, MAX_PARTICLES};

    // -------------------------------------------------------
    // ViewState construction
    // -------------------------------------------------------

    fn make_view() -> ViewState {
        ViewState::default()
    }

    fn make_client_state() -> ClientState {
        ClientState::default()
    }

    // -------------------------------------------------------
    // v_clear_scene
    // -------------------------------------------------------

    #[test]
    fn test_v_clear_scene_zeroes_counts() {
        let mut view = make_view();
        view.r_numdlights = 5;
        view.r_numentities = 10;
        view.r_numparticles = 20;

        v_clear_scene(&mut view);

        assert_eq!(view.r_numdlights, 0);
        assert_eq!(view.r_numentities, 0);
        assert_eq!(view.r_numparticles, 0);
    }

    // -------------------------------------------------------
    // v_add_entity
    // -------------------------------------------------------

    #[test]
    fn test_v_add_entity_basic() {
        let mut view = make_view();
        let mut ent = Entity::default();
        ent.model = 42;
        ent.origin = [1.0, 2.0, 3.0];

        v_add_entity(&mut view, &ent);

        assert_eq!(view.r_numentities, 1);
        assert_eq!(view.r_entities[0].model, 42);
        assert_eq!(view.r_entities[0].origin, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_v_add_entity_multiple() {
        let mut view = make_view();
        for i in 0..5 {
            let mut ent = Entity::default();
            ent.model = i;
            v_add_entity(&mut view, &ent);
        }
        assert_eq!(view.r_numentities, 5);
        for i in 0..5 {
            assert_eq!(view.r_entities[i].model, i as i32);
        }
    }

    #[test]
    fn test_v_add_entity_overflow_clamped() {
        let mut view = make_view();
        view.r_numentities = MAX_ENTITIES as i32;

        let ent = Entity::default();
        v_add_entity(&mut view, &ent);

        // Should not increase past MAX_ENTITIES
        assert_eq!(view.r_numentities, MAX_ENTITIES as i32);
    }

    // -------------------------------------------------------
    // v_add_particle
    // -------------------------------------------------------

    #[test]
    fn test_v_add_particle_basic() {
        let mut view = make_view();
        let org = [10.0, 20.0, 30.0];
        let length = [0.0, 0.0, 0.0];

        v_add_particle(&mut view, &org, &length, 8, 0.75, 0);

        assert_eq!(view.r_numparticles, 1);
        assert_eq!(view.r_particles[0].origin, org);
        assert_eq!(view.r_particles[0].color, 8);
        assert!((view.r_particles[0].alpha - 0.75).abs() < 1e-6);
        assert_eq!(view.r_particles[0].particle_type, 0);
    }

    #[test]
    fn test_v_add_particle_overflow_clamped() {
        let mut view = make_view();
        view.r_numparticles = MAX_PARTICLES as i32;

        let org = [0.0; 3];
        let length = [0.0; 3];
        v_add_particle(&mut view, &org, &length, 0, 1.0, 0);

        assert_eq!(view.r_numparticles, MAX_PARTICLES as i32);
    }

    // -------------------------------------------------------
    // v_add_light
    // -------------------------------------------------------

    #[test]
    fn test_v_add_light_basic() {
        let mut view = make_view();
        let org = [100.0, 200.0, 300.0];

        v_add_light(&mut view, &org, 500.0, 1.0, 0.5, 0.25);

        assert_eq!(view.r_numdlights, 1);
        assert_eq!(view.r_dlights[0].origin, org);
        assert!((view.r_dlights[0].intensity - 500.0).abs() < 1e-6);
        assert!((view.r_dlights[0].color[0] - 1.0).abs() < 1e-6);
        assert!((view.r_dlights[0].color[1] - 0.5).abs() < 1e-6);
        assert!((view.r_dlights[0].color[2] - 0.25).abs() < 1e-6);
    }

    #[test]
    fn test_v_add_light_overflow_clamped() {
        let mut view = make_view();
        view.r_numdlights = MAX_DLIGHTS as i32;

        let org = [0.0; 3];
        v_add_light(&mut view, &org, 100.0, 1.0, 1.0, 1.0);

        assert_eq!(view.r_numdlights, MAX_DLIGHTS as i32);
    }

    #[test]
    fn test_v_add_light_multiple() {
        let mut view = make_view();
        for i in 0..5 {
            let org = [i as f32 * 10.0, 0.0, 0.0];
            v_add_light(&mut view, &org, 100.0, 1.0, 0.0, 0.0);
        }
        assert_eq!(view.r_numdlights, 5);
        for i in 0..5 {
            assert!((view.r_dlights[i].origin[0] - i as f32 * 10.0).abs() < 1e-6);
        }
    }

    // -------------------------------------------------------
    // v_add_light_style
    // -------------------------------------------------------

    #[test]
    fn test_v_add_light_style_basic() {
        let mut view = make_view();

        v_add_light_style(&mut view, 0, 0.8, 0.6, 0.4);

        let ls = &view.r_lightstyles[0];
        assert!((ls.rgb[0] - 0.8).abs() < 1e-6);
        assert!((ls.rgb[1] - 0.6).abs() < 1e-6);
        assert!((ls.rgb[2] - 0.4).abs() < 1e-6);
        assert!((ls.white - 1.8).abs() < 1e-6); // 0.8 + 0.6 + 0.4
    }

    #[test]
    fn test_v_add_light_style_various_indices() {
        let mut view = make_view();

        v_add_light_style(&mut view, 5, 1.0, 1.0, 1.0);

        let ls = &view.r_lightstyles[5];
        assert!((ls.white - 3.0).abs() < 1e-6);
    }

    // -------------------------------------------------------
    // calc_fov
    // -------------------------------------------------------

    #[test]
    fn test_calc_fov_90_degree_4_3() {
        // Standard 90 degree horizontal FOV on a 4:3 display
        let fov_y = calc_fov(90.0, 640.0, 480.0);
        // fov_y for 90 horizontal on 4:3 should be about 73.74 degrees
        assert!(fov_y > 70.0 && fov_y < 80.0,
            "Expected FOV_Y ~73.7 for 90deg 4:3, got {}", fov_y);
    }

    #[test]
    fn test_calc_fov_90_degree_16_9() {
        // Standard 90 degree horizontal FOV on a 16:9 display
        let fov_y = calc_fov(90.0, 1920.0, 1080.0);
        // fov_y should be narrower due to wider aspect ratio
        assert!(fov_y > 50.0 && fov_y < 65.0,
            "Expected FOV_Y ~59 for 90deg 16:9, got {}", fov_y);
    }

    #[test]
    fn test_calc_fov_square_aspect() {
        // On a square display, fov_y should equal fov_x
        let fov_y = calc_fov(90.0, 100.0, 100.0);
        assert!((fov_y - 90.0).abs() < 0.1,
            "Expected FOV_Y ~90 for square aspect, got {}", fov_y);
    }

    #[test]
    fn test_calc_fov_wide_fov() {
        let fov_y = calc_fov(120.0, 640.0, 480.0);
        // Wide FOV should give a larger fov_y than standard
        assert!(fov_y > 90.0 && fov_y < 120.0,
            "Expected FOV_Y >90 for 120deg 4:3, got {}", fov_y);
    }

    #[test]
    fn test_calc_fov_narrow_fov() {
        let fov_y = calc_fov(60.0, 640.0, 480.0);
        assert!(fov_y > 40.0 && fov_y < 55.0,
            "Expected FOV_Y ~46 for 60deg 4:3, got {}", fov_y);
    }

    #[test]
    fn test_calc_fov_boundary_values() {
        // Test with minimum valid FOV
        let fov_y = calc_fov(1.0, 640.0, 480.0);
        assert!(fov_y > 0.0 && fov_y < 5.0);

        // Test with maximum valid FOV
        let fov_y = calc_fov(179.0, 640.0, 480.0);
        assert!(fov_y > 100.0);
    }

    // -------------------------------------------------------
    // v_gun_next_f / v_gun_prev_f
    // -------------------------------------------------------

    #[test]
    fn test_v_gun_next_f_increments() {
        let mut view = make_view();
        assert_eq!(view.gun_frame, 0);

        v_gun_next_f(&mut view);
        assert_eq!(view.gun_frame, 1);

        v_gun_next_f(&mut view);
        assert_eq!(view.gun_frame, 2);
    }

    #[test]
    fn test_v_gun_prev_f_decrements() {
        let mut view = make_view();
        view.gun_frame = 5;

        v_gun_prev_f(&mut view);
        assert_eq!(view.gun_frame, 4);

        v_gun_prev_f(&mut view);
        assert_eq!(view.gun_frame, 3);
    }

    #[test]
    fn test_v_gun_prev_f_clamps_to_zero() {
        let mut view = make_view();
        view.gun_frame = 0;

        v_gun_prev_f(&mut view);
        assert_eq!(view.gun_frame, 0);
    }

    #[test]
    fn test_v_gun_prev_f_from_one() {
        let mut view = make_view();
        view.gun_frame = 1;

        v_gun_prev_f(&mut view);
        assert_eq!(view.gun_frame, 0);

        // Going below zero clamps
        v_gun_prev_f(&mut view);
        assert_eq!(view.gun_frame, 0);
    }

    // -------------------------------------------------------
    // v_test_particles
    // -------------------------------------------------------

    #[test]
    fn test_v_test_particles_fills_all() {
        let mut view = make_view();
        let cl = make_client_state();

        v_test_particles(&mut view, &cl);

        assert_eq!(view.r_numparticles, MAX_PARTICLES as i32);
    }

    #[test]
    fn test_v_test_particles_color_is_8() {
        let mut view = make_view();
        let cl = make_client_state();

        v_test_particles(&mut view, &cl);

        for i in 0..MAX_PARTICLES {
            assert_eq!(view.r_particles[i].color, 8);
        }
    }

    // -------------------------------------------------------
    // v_test_entities
    // -------------------------------------------------------

    #[test]
    fn test_v_test_entities_creates_32() {
        let mut view = make_view();
        let cl = make_client_state();

        v_test_entities(&mut view, &cl);

        assert_eq!(view.r_numentities, 32);
    }

    // -------------------------------------------------------
    // v_test_lights
    // -------------------------------------------------------

    #[test]
    fn test_v_test_lights_creates_32() {
        let mut view = make_view();
        let cl = make_client_state();

        v_test_lights(&mut view, &cl);

        assert_eq!(view.r_numdlights, 32);
    }

    #[test]
    fn test_v_test_lights_intensity() {
        let mut view = make_view();
        let cl = make_client_state();

        v_test_lights(&mut view, &cl);

        for i in 0..32 {
            assert!((view.r_dlights[i].intensity - 200.0).abs() < 1e-6);
        }
    }

    #[test]
    fn test_v_test_lights_color_cycling() {
        let mut view = make_view();
        let cl = make_client_state();

        v_test_lights(&mut view, &cl);

        // Color cycles through (i%6)+1: 1,2,3,4,5,6,1,2,3,4,...
        // Color bits: R = bit0, G = bit1, B = bit2
        // i=0: (0%6)+1=1 => R=1,G=0,B=0
        assert!((view.r_dlights[0].color[0] - 1.0).abs() < 1e-6);
        assert!((view.r_dlights[0].color[1] - 0.0).abs() < 1e-6);
        assert!((view.r_dlights[0].color[2] - 0.0).abs() < 1e-6);

        // i=1: (1%6)+1=2 => R=0,G=1,B=0
        assert!((view.r_dlights[1].color[0] - 0.0).abs() < 1e-6);
        assert!((view.r_dlights[1].color[1] - 1.0).abs() < 1e-6);
        assert!((view.r_dlights[1].color[2] - 0.0).abs() < 1e-6);

        // i=2: (2%6)+1=3 => R=1,G=1,B=0
        assert!((view.r_dlights[2].color[0] - 1.0).abs() < 1e-6);
        assert!((view.r_dlights[2].color[1] - 1.0).abs() < 1e-6);
        assert!((view.r_dlights[2].color[2] - 0.0).abs() < 1e-6);
    }

    // -------------------------------------------------------
    // ViewState default
    // -------------------------------------------------------

    #[test]
    fn test_view_state_default_has_correct_sizes() {
        let view = ViewState::default();
        assert_eq!(view.r_dlights.len(), MAX_DLIGHTS);
        assert_eq!(view.r_entities.len(), MAX_ENTITIES);
        assert_eq!(view.r_particles.len(), MAX_PARTICLES);
        assert_eq!(view.r_lightstyles.len(), MAX_LIGHTSTYLES);
        assert_eq!(view.cl_weaponmodels.len(), MAX_CLIENTWEAPONMODELS);
    }

    #[test]
    fn test_view_state_default_counts_zero() {
        let view = ViewState::default();
        assert_eq!(view.r_numdlights, 0);
        assert_eq!(view.r_numentities, 0);
        assert_eq!(view.r_numparticles, 0);
        assert_eq!(view.num_cl_weaponmodels, 0);
        assert_eq!(view.gun_frame, 0);
        assert_eq!(view.gun_model, 0);
    }

    // -------------------------------------------------------
    // Integration: clear_scene then add
    // -------------------------------------------------------

    #[test]
    fn test_clear_then_add_entities() {
        let mut view = make_view();

        // Add some entities
        for i in 0..5 {
            let mut ent = Entity::default();
            ent.model = i;
            v_add_entity(&mut view, &ent);
        }
        assert_eq!(view.r_numentities, 5);

        // Clear
        v_clear_scene(&mut view);
        assert_eq!(view.r_numentities, 0);

        // Add again
        let ent = Entity::default();
        v_add_entity(&mut view, &ent);
        assert_eq!(view.r_numentities, 1);
    }

    #[test]
    fn test_clear_scene_and_add_mixed() {
        let mut view = make_view();

        v_add_entity(&mut view, &Entity::default());
        v_add_particle(&mut view, &[0.0; 3], &[0.0; 3], 0, 1.0, 0);
        v_add_light(&mut view, &[0.0; 3], 100.0, 1.0, 1.0, 1.0);

        assert_eq!(view.r_numentities, 1);
        assert_eq!(view.r_numparticles, 1);
        assert_eq!(view.r_numdlights, 1);

        v_clear_scene(&mut view);

        assert_eq!(view.r_numentities, 0);
        assert_eq!(view.r_numparticles, 0);
        assert_eq!(view.r_numdlights, 0);
    }
}
