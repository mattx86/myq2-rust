// cl_fx.rs -- entity effects parsing and management
// Converted from: myq2-original/client/cl_fx.c

use std::f32::consts::PI;

use myq2_common::q_shared::*;
use myq2_common::qcommon::NUMVERTEXNORMALS;
use myq2_common::common::{com_printf, BYTEDIRS};

use crate::client::{
    MAX_DLIGHTS, MAX_PARTICLES, PARTICLE_GRAVITY, BLASTER_PARTICLE_COLOR,
    INSTANT_PARTICLE, MAX_SUSTAINS, BEAMLENGTH, MAX_LIGHTSTYLES,
};
// Particle type constants — canonical definitions in myq2_common::q_shared
pub use myq2_common::q_shared::{PT_DEFAULT, PT_FIRE, PT_SMOKE, PT_BUBBLE, PT_BLOOD, PT_MAX};

// Entity event constants — from myq2_common::q_shared (already imported via wildcard)
// EV_ITEM_RESPAWN=1, EV_FOOTSTEP=2, EV_FALLSHORT=3, EV_FALL=4, EV_FALLFAR=5, EV_PLAYER_TELEPORT=6

pub use myq2_common::q_shared::StainType;

pub const STAIN_MODULATE: StainType = StainType::Modulate;

// ============================================================
// Structures
// ============================================================

#[derive(Debug, Clone)]
#[repr(C)]
pub struct CDlight {
    pub key: i32,
    pub color: Vec3,
    pub origin: Vec3,
    pub radius: f32,
    pub die: f32,
    pub decay: f32,
    pub minlight: f32,
    /// Original die time before packet loss extension
    pub original_die: f32,
    /// Whether this light has been extended during packet loss
    pub extended: bool,
}

impl Default for CDlight {
    fn default() -> Self {
        Self {
            key: 0,
            color: [0.0; 3],
            origin: [0.0; 3],
            radius: 0.0,
            die: 0.0,
            decay: 0.0,
            minlight: 0.0,
            original_die: 0.0,
            extended: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CParticle {
    pub next: Option<usize>, // index into particles array (linked list)
    pub time: f32,
    pub org: Vec3,
    pub vel: Vec3,
    pub accel: Vec3,
    pub length: Vec3,
    pub particle_type: i32,
    pub color: f32,
    pub colorvel: f32,
    pub alpha: f32,
    pub alphavel: f32,
}

impl Default for CParticle {
    fn default() -> Self {
        Self {
            next: None,
            time: 0.0,
            org: [0.0; 3],
            vel: [0.0; 3],
            accel: [0.0; 3],
            length: [0.0; 3],
            particle_type: PT_DEFAULT,
            color: 0.0,
            colorvel: 0.0,
            alpha: 0.0,
            alphavel: 0.0,
        }
    }
}

// CEntity and ClSustain are defined in client.rs — re-use them to avoid type mismatches.
pub use crate::client::CEntity;
pub use crate::client::ClSustain;

/// Render entity_t (simplified for effects code)
#[derive(Debug, Clone)]
pub struct RenderEntity {
    pub origin: Vec3,
}

impl Default for RenderEntity {
    fn default() -> Self {
        Self {
            origin: [0.0; 3],
        }
    }
}

// ============================================================
// Light style
// ============================================================

#[derive(Clone)]
pub struct CLightStyle {
    length: i32,
    value: [f32; 3],
    map: [f32; MAX_QPATH],
}

impl Default for CLightStyle {
    fn default() -> Self {
        Self {
            length: 0,
            value: [0.0; 3],
            map: [0.0; MAX_QPATH],
        }
    }
}

// ============================================================
// Helper functions (rand/frand/crand replacements)
// ============================================================

use std::sync::atomic::{AtomicU32, Ordering};
use rayon::prelude::*;

static RAND_SEED: AtomicU32 = AtomicU32::new(0);

/// Simple LCG random, matching C rand() behavior
pub(crate) fn qrand() -> i32 {
    let mut seed = RAND_SEED.load(Ordering::Relaxed);
    seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
    RAND_SEED.store(seed, Ordering::Relaxed);
    ((seed >> 16) & 0x7fff) as i32
}

/// Random float in [0, 1)
pub(crate) fn frand() -> f32 {
    (qrand() as f32) / 32768.0
}

/// Random float in [-1, 1)
pub(crate) fn crand() -> f32 {
    (qrand() as f32) / 16384.0 - 1.0
}

// ============================================================
// Global state — in the full engine these are true globals.
// Here they are module-level statics behind a struct for context.
// ============================================================

/// Effect registration info for continuation during packet loss
#[derive(Debug, Clone, Copy, Default)]
pub struct RecentEffect {
    pub origin: Vec3,
    pub velocity: Vec3,
    pub effect_type: i32,  // 0=smoke, 1=fire, 2=sparks, 3=blood, 4=debris
    pub start_time: i32,
    pub duration_ms: i32,
    pub active: bool,
}

pub struct ClFxState {
    pub cl_lightstyle: Vec<CLightStyle>,
    pub lastofs: i32,
    pub cl_dlights: Vec<CDlight>,
    pub particles: Vec<CParticle>,
    pub active_particles: Option<usize>,
    pub free_particles: Option<usize>,
    pub cl_numparticles: usize,
    avelocities: [[f32; 3]; NUMVERTEXNORMALS],
    /// Contiguous indices of active particles for parallel processing.
    /// Kept in sync with the linked list for efficient rayon iteration.
    pub active_indices: Vec<usize>,
    /// Recent significant effects for continuation tracking
    pub recent_effects: Vec<RecentEffect>,
}

impl Default for ClFxState {
    fn default() -> Self {
        Self::new()
    }
}

impl ClFxState {
    pub fn new() -> Self {
        let mut particles = Vec::with_capacity(MAX_PARTICLES);
        for _ in 0..MAX_PARTICLES {
            particles.push(CParticle::default());
        }
        let mut cl_lightstyle = Vec::with_capacity(MAX_LIGHTSTYLES);
        for _ in 0..MAX_LIGHTSTYLES {
            cl_lightstyle.push(CLightStyle::default());
        }
        let mut cl_dlights = Vec::with_capacity(MAX_DLIGHTS);
        for _ in 0..MAX_DLIGHTS {
            cl_dlights.push(CDlight::default());
        }
        Self {
            cl_lightstyle,
            lastofs: -1,
            cl_dlights,
            particles,
            active_particles: None,
            free_particles: None,
            cl_numparticles: MAX_PARTICLES,
            avelocities: [[0.0; 3]; NUMVERTEXNORMALS],
            active_indices: Vec::with_capacity(MAX_PARTICLES),
            recent_effects: Vec::with_capacity(32),
        }
    }

    /// Register a significant effect for potential continuation during packet loss.
    /// Effect types: 0=smoke, 1=fire, 2=sparks, 3=blood, 4=debris
    pub fn register_effect(&mut self, origin: &Vec3, velocity: &Vec3, effect_type: i32, duration_ms: i32, current_time: i32) {
        // Limit recent effects to 32
        if self.recent_effects.len() >= 32 {
            // Remove oldest effect
            self.recent_effects.remove(0);
        }
        self.recent_effects.push(RecentEffect {
            origin: *origin,
            velocity: *velocity,
            effect_type,
            start_time: current_time,
            duration_ms,
            active: true,
        });
    }

    /// Get and clear recent effects for transfer to the main continuation system.
    pub fn take_recent_effects(&mut self) -> Vec<RecentEffect> {
        std::mem::take(&mut self.recent_effects)
    }

    /// Cleanup expired recent effects.
    pub fn cleanup_recent_effects(&mut self, current_time: i32) {
        self.recent_effects.retain(|e| {
            e.active && (current_time - e.start_time) < e.duration_ms + 500
        });
    }

    // ============================================================
    // LIGHT STYLE MANAGEMENT
    // ============================================================

    pub fn cl_clear_light_styles(&mut self) {
        for ls in self.cl_lightstyle.iter_mut() {
            *ls = CLightStyle::default();
        }
        self.lastofs = -1;
    }

    pub fn cl_run_light_styles(&mut self, cl_time: i32) {
        let ofs = cl_time / 100;
        if ofs == self.lastofs {
            return;
        }
        self.lastofs = ofs;

        for ls in self.cl_lightstyle.iter_mut() {
            if ls.length == 0 {
                ls.value[0] = 1.0;
                ls.value[1] = 1.0;
                ls.value[2] = 1.0;
                continue;
            }
            if ls.length == 1 {
                let v = ls.map[0];
                ls.value[0] = v;
                ls.value[1] = v;
                ls.value[2] = v;
            } else {
                let v = ls.map[(ofs % ls.length) as usize];
                ls.value[0] = v;
                ls.value[1] = v;
                ls.value[2] = v;
            }
        }
    }

    pub fn cl_set_lightstyle(&mut self, i: usize, s: &str) {
        let j = s.len();
        if j >= MAX_QPATH {
            com_printf(&format!("svc_lightstyle length={}\n", j));
            return;
        }

        self.cl_lightstyle[i].length = j as i32;

        for (k, ch) in s.bytes().enumerate() {
            self.cl_lightstyle[i].map[k] =
                (ch as f32 - b'a' as f32) / (b'm' as f32 - b'a' as f32);
        }
    }

    /// CL_AddLightStyles — calls V_AddLightStyle callback for each style.
    /// `r_timebasedfx`: value of the r_timebasedfx cvar.
    /// `add_light_style_fn`: callback to V_AddLightStyle(style, r, g, b).
    pub fn cl_add_light_styles<F>(&self, r_timebasedfx: f32, mut add_light_style_fn: F)
    where
        F: FnMut(usize, f32, f32, f32),
    {
        for (i, ls) in self.cl_lightstyle.iter().enumerate() {
            if i == 0 && r_timebasedfx != 0.0 {
                // Time-based brightness (MyQ2 feature)
                let now = std::time::SystemTime::now();
                let since_epoch = now
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default();
                // Get current hour in local time approximation
                let total_secs = since_epoch.as_secs();
                let hour_i = ((total_secs % 86400) / 3600) as i32; // UTC hour

                // Convert to 12-hour clock
                let (am, hour_12) = if hour_i <= 11 {
                    let h = if hour_i == 0 { 12 } else { hour_i };
                    (true, h)
                } else {
                    let h = if hour_i > 12 { hour_i - 12 } else { hour_i };
                    (false, h)
                };

                let brightness = if am {
                    if hour_12 == 12 {
                        // midnight
                        0.15
                    } else {
                        (hour_12 as f32 / 12.0) * 0.75 + 0.25
                    }
                } else if hour_12 == 12 {
                    // noon
                    1.0
                } else {
                    // PM uses reverse hour of AM
                    let pm_hour = 12 - hour_12;
                    (pm_hour as f32 / 12.0) * 0.75 + 0.25
                };

                add_light_style_fn(
                    i,
                    ls.value[0] * brightness,
                    ls.value[1] * brightness,
                    ls.value[2] * brightness,
                );
            } else {
                add_light_style_fn(i, ls.value[0], ls.value[1], ls.value[2]);
            }
        }
    }

    // ============================================================
    // DLIGHT MANAGEMENT
    // ============================================================

    pub fn cl_clear_dlights(&mut self) {
        for dl in self.cl_dlights.iter_mut() {
            *dl = CDlight::default();
        }
    }

    pub fn cl_alloc_dlight(&mut self, key: i32, cl_time: f32) -> usize {
        // first look for an exact key match
        if key != 0 {
            for i in 0..MAX_DLIGHTS {
                if self.cl_dlights[i].key == key {
                    self.cl_dlights[i] = CDlight::default();
                    self.cl_dlights[i].key = key;
                    return i;
                }
            }
        }

        // then look for anything else
        for i in 0..MAX_DLIGHTS {
            if self.cl_dlights[i].die < cl_time {
                self.cl_dlights[i] = CDlight::default();
                self.cl_dlights[i].key = key;
                return i;
            }
        }

        self.cl_dlights[0] = CDlight::default();
        self.cl_dlights[0].key = key;
        0
    }

    pub fn cl_new_dlight(
        &mut self,
        key: i32,
        x: f32,
        y: f32,
        z: f32,
        radius: f32,
        time: f32,
        cl_time: f32,
    ) {
        let idx = self.cl_alloc_dlight(key, cl_time);
        let dl = &mut self.cl_dlights[idx];
        dl.origin[0] = x;
        dl.origin[1] = y;
        dl.origin[2] = z;
        dl.radius = radius;
        dl.die = cl_time + time;
        dl.original_die = dl.die;
        dl.extended = false;
    }

    pub fn cl_run_dlights(&mut self, cl_time: f32, frametime: f32) {
        for dl in self.cl_dlights.iter_mut() {
            if dl.radius == 0.0 {
                continue;
            }

            if dl.die < cl_time {
                dl.radius = 0.0;
                return;
            }
            dl.radius -= frametime * dl.decay;
            if dl.radius < 0.0 {
                dl.radius = 0.0;
            }
        }
    }

    /// Extend dynamic light lifetimes during packet loss to prevent abrupt expiration.
    /// Call this when packet_loss_frames > 0 to keep lights visible longer.
    pub fn cl_extend_dlights_for_packet_loss(&mut self, cl_time: f32, extension_ms: f32) {
        let extension_sec = extension_ms / 1000.0;

        for dl in self.cl_dlights.iter_mut() {
            if dl.radius == 0.0 {
                continue;
            }

            // Only extend if the light is about to expire
            if dl.die < cl_time + 0.2 && !dl.extended {
                // Store original die time if not already extended
                if dl.original_die == 0.0 {
                    dl.original_die = dl.die;
                }

                // Extend the die time
                dl.die = cl_time + extension_sec;
                dl.extended = true;

                // Slow down decay during packet loss to prevent lights from shrinking too fast
                dl.decay *= 0.5;
            }
        }
    }

    /// Reset extended dlights when packets are received again.
    /// This restores normal decay behavior.
    pub fn cl_reset_extended_dlights(&mut self) {
        for dl in self.cl_dlights.iter_mut() {
            if dl.extended {
                dl.extended = false;
                // Restore normal decay rate (double it back since we halved it)
                dl.decay *= 2.0;
            }
        }
    }

    /// CL_AddDLights — calls V_AddLight for each active dlight.
    pub fn cl_add_dlights<F>(&self, mut add_light_fn: F)
    where
        F: FnMut(&Vec3, f32, f32, f32, f32),
    {
        for dl in self.cl_dlights.iter() {
            if dl.radius == 0.0 {
                continue;
            }
            add_light_fn(
                &dl.origin,
                dl.radius,
                dl.color[0],
                dl.color[1],
                dl.color[2],
            );
        }
    }

    /// CL_AddDLights with per-entity smoothing for smoother light transitions.
    /// Updates the smoothing system with current light state and uses interpolated
    /// values for rendering, providing smoother light position and radius changes.
    pub fn cl_add_dlights_smoothed<F>(
        &self,
        smoothing: &mut crate::cl_smooth::DynamicLightSmoothing,
        current_time: i32,
        lerp: f32,
        mut add_light_fn: F,
    )
    where
        F: FnMut(&Vec3, f32, f32, f32, f32),
    {
        for dl in self.cl_dlights.iter() {
            if dl.radius == 0.0 {
                continue;
            }

            // Use entity key to track per-entity light state
            let entity_num = dl.key.unsigned_abs() as usize;

            // Update smoothing state with current light position and properties
            smoothing.update(entity_num, &dl.origin, dl.radius, &dl.color, current_time);

            // Get interpolated light values for smoother rendering
            if let Some((smooth_origin, smooth_radius, smooth_color)) = smoothing.get_interpolated(entity_num, lerp) {
                add_light_fn(
                    &smooth_origin,
                    smooth_radius,
                    smooth_color[0],
                    smooth_color[1],
                    smooth_color[2],
                );
            } else {
                // Fallback to raw values if smoothing not available
                add_light_fn(
                    &dl.origin,
                    dl.radius,
                    dl.color[0],
                    dl.color[1],
                    dl.color[2],
                );
            }
        }
    }

    /// Add predicted weapon effect lights.
    ///
    /// Renders predicted muzzle flashes and tracers from the weapon prediction system
    /// for immediate visual feedback before server confirmation.
    pub fn cl_add_predicted_weapon_effects<F>(
        &self,
        weapon_prediction: &crate::cl_smooth::WeaponPrediction,
        current_time: i32,
        mut add_light_fn: F,
    ) where
        F: FnMut(&Vec3, f32, f32, f32, f32),
    {
        use crate::cl_smooth::WeaponEffectType;

        for effect in weapon_prediction.get_active_effects(current_time) {
            // Determine light color and intensity based on effect type
            let (r, g, b, radius) = match effect.effect_type {
                WeaponEffectType::MuzzleFlash => {
                    // Bright yellow-orange flash
                    (1.0, 0.8, 0.2, 200.0)
                }
                WeaponEffectType::Tracer => {
                    // Dim yellow trail
                    (0.8, 0.6, 0.2, 50.0)
                }
                WeaponEffectType::BulletImpact => {
                    // Spark flash
                    (1.0, 0.6, 0.0, 75.0)
                }
                WeaponEffectType::RocketTrail => {
                    // Orange rocket glow
                    (1.0, 0.5, 0.0, 100.0)
                }
                WeaponEffectType::RailTrail => {
                    // Blue rail
                    (0.3, 0.3, 1.0, 150.0)
                }
            };

            // Fade based on age
            let age = (current_time - effect.create_time) as f32;
            let fade = 1.0 - (age / effect.duration_ms as f32).clamp(0.0, 1.0);

            if fade > 0.0 {
                add_light_fn(&effect.origin, radius * fade, r, g, b);
            }
        }
    }

    // ============================================================
    // PARTICLE MANAGEMENT
    // ============================================================

    pub fn cl_clear_particles(&mut self) {
        self.free_particles = Some(0);
        self.active_particles = None;
        self.active_indices.clear();

        for i in 0..self.cl_numparticles - 1 {
            self.particles[i].next = Some(i + 1);
        }
        self.particles[self.cl_numparticles - 1].next = None;
    }

    /// Allocate a particle from the free list. Returns index or None.
    pub fn alloc_particle(&mut self) -> Option<usize> {
        let idx = self.free_particles?;
        self.free_particles = self.particles[idx].next;
        self.particles[idx].next = self.active_particles;
        self.active_particles = Some(idx);
        // Reset defaults
        self.particles[idx].colorvel = 0.0;
        self.particles[idx].length = [0.0; 3];
        Some(idx)
    }

    // ============================================================
    // CL_BloodEffect
    // ============================================================

    pub fn cl_blood_effect(&mut self, org: &Vec3, dir: &Vec3, color: i32, count: i32, cl_time: f32) {
        for _ in 0..count {
            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];

            p.time = cl_time;
            p.color = (color + (qrand() & 7)) as f32;
            p.particle_type = PT_BLOOD;

            let d = (qrand() & 31) as f32;
            for j in 0..3 {
                p.org[j] = org[j] + ((qrand() & 7) - 4) as f32 + d * dir[j];
                p.vel[j] = crand() * 20.0;
            }

            p.accel[0] = 0.0;
            p.accel[1] = 0.0;
            p.accel[2] = -PARTICLE_GRAVITY;
            p.alpha = 1.0;

            p.alphavel = -1.0 / (0.5 + frand() * 0.3);
        }

        // Register significant blood effects (count >= 10) for continuation
        // Effect type 3 = blood
        if count >= 10 {
            self.register_effect(org, &[dir[0] * 20.0, dir[1] * 20.0, dir[2] * 20.0], 3, 300, cl_time as i32);
        }
    }

    // ============================================================
    // CL_ParticleEffect — Wall impact puffs
    // ============================================================

    pub fn cl_particle_effect(&mut self, org: &Vec3, dir: &Vec3, color: i32, count: i32, cl_time: f32) {
        for _ in 0..count {
            let d = (qrand() & 31) as f32;
            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            p.time = cl_time;
            p.color = (color + (qrand() & 7)) as f32;
            p.particle_type = PT_DEFAULT;

            for j in 0..3 {
                p.org[j] = org[j] + ((qrand() & 7) - 4) as f32 + d * dir[j];
                p.vel[j] = crand() * 20.0;
            }
            p.accel[0] = 0.0;
            p.accel[1] = 0.0;
            p.accel[2] = -PARTICLE_GRAVITY;
            p.alpha = 1.0;
            p.alphavel = -1.0 / (0.5 + frand() * 0.3);
        }
    }

    // ============================================================
    // CL_ParticleEffect2
    // ============================================================

    pub fn cl_particle_effect2(&mut self, org: &Vec3, dir: &Vec3, color: i32, count: i32, cl_time: f32) {
        for _ in 0..count {
            let d = (qrand() & 7) as f32;
            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            p.time = cl_time;
            p.color = color as f32;
            p.particle_type = PT_DEFAULT;

            for j in 0..3 {
                p.org[j] = org[j] + ((qrand() & 7) - 4) as f32 + d * dir[j];
                p.vel[j] = crand() * 20.0;
            }
            p.accel[0] = 0.0;
            p.accel[1] = 0.0;
            p.accel[2] = -PARTICLE_GRAVITY;
            p.alpha = 1.0;
            p.alphavel = -1.0 / (0.5 + frand() * 0.3);
        }
    }

    // ============================================================
    // CL_ParticleEffect3 (RAFAEL)
    // ============================================================

    pub fn cl_particle_effect3(&mut self, org: &Vec3, dir: &Vec3, color: i32, count: i32, cl_time: f32) {
        for _ in 0..count {
            let d = (qrand() & 7) as f32;
            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            p.time = cl_time;
            p.color = color as f32;
            p.particle_type = PT_DEFAULT;

            for j in 0..3 {
                p.org[j] = org[j] + ((qrand() & 7) - 4) as f32 + d * dir[j];
                p.vel[j] = crand() * 20.0;
            }
            p.accel[0] = 0.0;
            p.accel[1] = 0.0;
            p.accel[2] = PARTICLE_GRAVITY; // note: positive gravity
            p.alpha = 1.0;
            p.alphavel = -1.0 / (0.5 + frand() * 0.3);
        }
    }

    // ============================================================
    // CL_TeleporterParticles
    // ============================================================

    pub fn cl_teleporter_particles(&mut self, ent: &EntityState, cl_time: f32) {
        for _ in 0..8 {
            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            p.time = cl_time;
            p.color = 0xdb as f32;
            p.particle_type = PT_DEFAULT;

            p.org[0] = ent.origin[0] - 16.0 + (qrand() & 31) as f32;
            p.org[1] = ent.origin[1] - 16.0 + (qrand() & 31) as f32;
            p.org[2] = ent.origin[2] - 8.0 + (qrand() & 7) as f32;
            p.vel[0] = crand() * 14.0;
            p.vel[1] = crand() * 14.0;
            p.vel[2] = 80.0 + (qrand() & 7) as f32;
            p.accel[0] = 0.0;
            p.accel[1] = 0.0;
            p.accel[2] = -PARTICLE_GRAVITY;
            p.alpha = 1.0;
            p.alphavel = -0.5;
        }
    }

    // ============================================================
    // CL_LogoutEffect
    // ============================================================

    pub fn cl_logout_effect(&mut self, org: &Vec3, effect_type: i32, cl_time: f32) {
        for _ in 0..500 {
            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            p.time = cl_time;

            let base_color = if effect_type == MZ_LOGIN {
                0xd0
            } else if effect_type == MZ_LOGOUT {
                0x40
            } else {
                0xe0
            };
            p.color = (base_color + (qrand() & 7)) as f32;
            p.particle_type = PT_DEFAULT;

            p.org[0] = org[0] - 16.0 + frand() * 32.0;
            p.org[1] = org[1] - 16.0 + frand() * 32.0;
            p.org[2] = org[2] - 24.0 + frand() * 56.0;
            p.vel[0] = crand() * 20.0;
            p.vel[1] = crand() * 20.0;
            p.vel[2] = crand() * 20.0;
            p.accel[0] = 0.0;
            p.accel[1] = 0.0;
            p.accel[2] = -PARTICLE_GRAVITY;
            p.alpha = 1.0;
            p.alphavel = -1.0 / (1.0 + frand() * 0.3);
        }
    }

    // ============================================================
    // CL_ItemRespawnParticles
    // ============================================================

    pub fn cl_item_respawn_particles(&mut self, org: &Vec3, cl_time: f32) {
        for _ in 0..64 {
            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            p.time = cl_time;
            p.color = (0xd4 + (qrand() & 3)) as f32; // green
            p.particle_type = PT_DEFAULT;

            p.org[0] = org[0] + crand() * 8.0;
            p.org[1] = org[1] + crand() * 8.0;
            p.org[2] = org[2] + crand() * 8.0;

            for j in 0..3 {
                p.vel[j] = crand() * 8.0;
            }

            p.accel[0] = 0.0;
            p.accel[1] = 0.0;
            p.accel[2] = -PARTICLE_GRAVITY * 0.2;
            p.alpha = 1.0;
            p.alphavel = -1.0 / (1.0 + frand() * 0.3);
        }
    }

    // ============================================================
    // CL_ExplosionParticles
    // ============================================================

    pub fn cl_explosion_particles(&mut self, org: &Vec3, cl_time: f32) {
        for _ in 0..256 {
            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            p.time = cl_time;
            p.color = (0xe0 + (qrand() & 7)) as f32;
            p.particle_type = PT_DEFAULT;

            for j in 0..3 {
                p.org[j] = org[j] + ((qrand() % 32) - 16) as f32;
                p.vel[j] = ((qrand() % 384) - 192) as f32;
            }

            p.accel[0] = 0.0;
            p.accel[1] = 0.0;
            p.accel[2] = -PARTICLE_GRAVITY;
            p.alpha = 1.0;
            p.alphavel = -0.8 / (0.5 + frand() * 0.3);
        }

        // Register explosion for continuation during packet loss
        // Effect type 1 = fire (orange explosion particles)
        self.register_effect(org, &[0.0, 0.0, 0.0], 1, 500, cl_time as i32);

        // AddStain call — deferred to renderer integration
        // let i = (qrand() % 30) + (qrand() % 30);
        // add_stain(org, 45.0, i as f32, i as f32, i as f32, (175 + (qrand() % 100)) as f32, STAIN_MODULATE);
    }

    // ============================================================
    // CL_BigTeleportParticles
    // ============================================================

    pub fn cl_big_teleport_particles(&mut self, org: &Vec3, cl_time: f32) {
        let colortable: [i32; 4] = [2 * 8, 13 * 8, 21 * 8, 18 * 8];

        for _ in 0..4096 {
            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            p.time = cl_time;
            p.color = colortable[(qrand() & 3) as usize] as f32;
            p.particle_type = PT_DEFAULT;

            let angle = PI * 2.0 * (qrand() & 1023) as f32 / 1023.0;
            let dist = (qrand() & 31) as f32;
            p.org[0] = org[0] + angle.cos() * dist;
            p.vel[0] = angle.cos() * (70.0 + (qrand() & 63) as f32);
            p.accel[0] = -angle.cos() * 100.0;

            p.org[1] = org[1] + angle.sin() * dist;
            p.vel[1] = angle.sin() * (70.0 + (qrand() & 63) as f32);
            p.accel[1] = -angle.sin() * 100.0;

            p.org[2] = org[2] + 8.0 + (qrand() % 90) as f32;
            p.vel[2] = -100.0 + (qrand() & 31) as f32;
            p.accel[2] = PARTICLE_GRAVITY * 4.0;
            p.alpha = 1.0;
            p.alphavel = -0.3 / (0.5 + frand() * 0.3);
        }
    }

    // ============================================================
    // CL_BlasterParticles — Wall impact puffs
    // ============================================================

    pub fn cl_blaster_particles(&mut self, org: &Vec3, dir: &Vec3, cl_time: f32) {
        let count = 40;
        for _ in 0..count {
            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            p.time = cl_time;
            p.color = (0xe0 + (qrand() & 7)) as f32;
            p.particle_type = PT_DEFAULT;

            let d = (qrand() & 15) as f32;
            for j in 0..3 {
                p.org[j] = org[j] + ((qrand() & 7) - 4) as f32 + d * dir[j];
                p.vel[j] = dir[j] * 30.0 + crand() * 40.0;
            }

            p.accel[0] = 0.0;
            p.accel[1] = 0.0;
            p.accel[2] = -PARTICLE_GRAVITY;
            p.alpha = 1.0;
            p.alphavel = -1.0 / (0.5 + frand() * 0.3);
        }
    }

    // ============================================================
    // CL_BlasterTrail
    // ============================================================

    pub fn cl_blaster_trail(&mut self, start: &Vec3, end: &Vec3, cl_time: f32) {
        let mut mov = *start;
        let mut vec = vector_subtract(end, start);
        let mut len = vector_normalize(&mut vec);

        let dec = 5;
        vec = vector_scale(&vec, dec as f32);

        while len > 0.0 {
            len -= dec as f32;

            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            vector_clear(&mut p.accel);

            p.time = cl_time;
            p.alpha = 1.0;
            p.alphavel = -1.0 / (0.3 + frand() * 0.2);
            p.color = 0xe0 as f32;
            p.particle_type = PT_DEFAULT;
            for j in 0..3 {
                p.org[j] = mov[j] + crand();
                p.vel[j] = crand() * 5.0;
                p.accel[j] = 0.0;
            }

            mov = vector_add(&mov, &vec);
        }
    }

    // ============================================================
    // CL_QuadTrail
    // ============================================================

    pub fn cl_quad_trail(&mut self, start: &Vec3, end: &Vec3, cl_time: f32) {
        let mut mov = *start;
        let mut vec = vector_subtract(end, start);
        let mut len = vector_normalize(&mut vec);

        let dec = 5;
        vec = vector_scale(&vec, 5.0);

        while len > 0.0 {
            len -= dec as f32;

            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            vector_clear(&mut p.accel);

            p.time = cl_time;
            p.alpha = 1.0;
            p.alphavel = -1.0 / (0.8 + frand() * 0.2);
            p.color = 115.0;
            p.particle_type = PT_DEFAULT;
            for j in 0..3 {
                p.org[j] = mov[j] + crand() * 16.0;
                p.vel[j] = crand() * 5.0;
                p.accel[j] = 0.0;
            }

            mov = vector_add(&mov, &vec);
        }
    }

    // ============================================================
    // CL_FlagTrail
    // ============================================================

    pub fn cl_flag_trail(&mut self, start: &Vec3, end: &Vec3, color: f32, cl_time: f32) {
        let mut mov = *start;
        let mut vec = vector_subtract(end, start);
        let mut len = vector_normalize(&mut vec);

        let dec = 5;
        vec = vector_scale(&vec, 5.0);

        while len > 0.0 {
            len -= dec as f32;

            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            vector_clear(&mut p.accel);

            p.time = cl_time;
            p.alpha = 1.0;
            p.alphavel = -1.0 / (0.8 + frand() * 0.2);
            p.color = color;
            p.particle_type = PT_DEFAULT;
            for j in 0..3 {
                p.org[j] = mov[j] + crand() * 16.0;
                p.vel[j] = crand() * 5.0;
                p.accel[j] = 0.0;
            }

            mov = vector_add(&mov, &vec);
        }
    }

    // ============================================================
    // CL_DiminishingTrail
    // ============================================================

    pub fn cl_diminishing_trail(
        &mut self,
        start: &Vec3,
        end: &Vec3,
        old: &mut CEntity,
        flags: u32,
        cl_time: f32,
    ) {
        let mut mov = *start;
        let mut vec = vector_subtract(end, start);
        let mut len = vector_normalize(&mut vec);

        let dec: f32 = 0.5;
        vec = vector_scale(&vec, dec);

        let (orgscale, velscale) = if old.trailcount > 900 {
            (4.0f32, 15.0f32)
        } else if old.trailcount > 800 {
            (2.0, 10.0)
        } else {
            (1.0, 5.0)
        };

        while len > 0.0 {
            len -= dec;

            if self.free_particles.is_none() {
                return;
            }

            // drop less particles as it flies
            if (qrand() & 1023) < old.trailcount {
                let idx = match self.alloc_particle() {
                    Some(i) => i,
                    None => return,
                };
                let p = &mut self.particles[idx];
                vector_clear(&mut p.accel);

                p.time = cl_time;

                if flags & EF_GIB != 0 {
                    p.alpha = 1.0;
                    p.alphavel = -1.0 / (1.0 + frand() * 0.4);
                    p.color = (0xe8 + (qrand() & 7)) as f32;
                    p.particle_type = PT_SMOKE;
                    for j in 0..3 {
                        p.org[j] = mov[j] + crand() * orgscale;
                        p.vel[j] = crand() * velscale;
                        p.accel[j] = 0.0;
                    }
                    p.vel[2] -= PARTICLE_GRAVITY;
                } else if flags & EF_GREENGIB != 0 {
                    p.alpha = 1.0;
                    p.alphavel = -1.0 / (1.0 + frand() * 0.4);
                    p.color = (0xdb + (qrand() & 7)) as f32;
                    p.particle_type = PT_SMOKE;
                    for j in 0..3 {
                        p.org[j] = mov[j] + crand() * orgscale;
                        p.vel[j] = crand() * velscale;
                        p.accel[j] = 0.0;
                    }
                    p.vel[2] -= PARTICLE_GRAVITY;
                } else {
                    p.alpha = 1.0;
                    p.alphavel = -1.0 / (1.0 + frand() * 0.2);
                    p.color = (4 + (qrand() & 7)) as f32;
                    p.particle_type = PT_SMOKE;
                    for j in 0..3 {
                        p.org[j] = mov[j] + crand() * orgscale;
                        p.vel[j] = crand() * velscale;
                    }
                    p.accel[2] = 20.0;
                }
            }

            old.trailcount -= 5;
            if old.trailcount < 100 {
                old.trailcount = 100;
            }
            mov = vector_add(&mov, &vec);
        }
    }

    // ============================================================
    // CL_RocketTrail
    // ============================================================

    pub fn cl_rocket_trail(&mut self, start: &Vec3, end: &Vec3, old: &mut CEntity, cl_time: f32) {
        // smoke
        self.cl_diminishing_trail(start, end, old, EF_ROCKET, cl_time);

        // fire
        let mut mov = *start;
        let mut vec = vector_subtract(end, start);
        let mut len = vector_normalize(&mut vec);

        let dec: f32 = 1.0;
        vec = vector_scale(&vec, dec);

        while len > 0.0 {
            len -= dec;

            if self.free_particles.is_none() {
                return;
            }

            if (qrand() & 7) == 0 {
                let idx = match self.alloc_particle() {
                    Some(i) => i,
                    None => return,
                };
                let p = &mut self.particles[idx];
                vector_clear(&mut p.accel);
                p.time = cl_time;

                p.alpha = 1.0;
                p.alphavel = -1.0 / (1.0 + frand() * 0.2);
                p.color = (0xdc + (qrand() & 3)) as f32;
                p.particle_type = PT_FIRE;
                for j in 0..3 {
                    p.org[j] = mov[j] + crand() * 5.0;
                    p.vel[j] = crand() * 20.0;
                }
                p.accel[2] = -PARTICLE_GRAVITY;
            }
            mov = vector_add(&mov, &vec);
        }

        // Register rocket trail for continuation during packet loss
        // Calculate trail velocity from start to end direction
        let trail_vec = vector_subtract(end, start);
        let trail_vel = vector_scale(&trail_vec, 10.0); // Trail moves along path
        // Effect type 1 = fire (orange rocket trail)
        self.register_effect(start, &trail_vel, 1, 400, cl_time as i32);
    }

    // ============================================================
    // CL_RailTrail
    // ============================================================

    pub fn cl_rail_trail(&mut self, start: &Vec3, end: &Vec3, cl_time: f32, new_particles: bool) {
        let clr: u8 = 0x74;

        if new_particles {
            // new rail trail
            let mut mov = *start;
            let mut vec = vector_subtract(end, start);
            let len = vector_normalize(&mut vec);
            let (mut right, mut up) = ([0.0f32; 3], [0.0f32; 3]);
            make_normal_vectors(&vec, &mut right, &mut up);

            // beam
            let dec: f32 = 28.0;
            let vec2 = vector_scale(&vec, dec);
            let mut mov_beam = *start;
            let mut i: f32 = 0.0;
            while i < len {
                mov_beam = vector_add(&mov_beam, &vec2);

                let idx = match self.alloc_particle() {
                    Some(i) => i,
                    None => return,
                };
                let p = &mut self.particles[idx];
                p.time = cl_time;
                vector_clear(&mut p.accel);

                p.alpha = 1.0;
                p.alphavel = -1.0;
                p.color = 0x0f as f32;
                p.particle_type = PT_DEFAULT;

                for j in 0..3 {
                    p.org[j] = mov_beam[j];
                    p.vel[j] = 0.0;
                    p.accel[j] = crand() * 3.0;
                }
                i += dec;
            }

            // spiral
            let _size: f32 = 1.0;
            let dec2: f32 = 1.0;
            let vec2b = vector_scale(&vec, dec2);
            mov = *start;
            for i in 0..(len as i32) {
                let idx = match self.alloc_particle() {
                    Some(i) => i,
                    None => return,
                };
                let p = &mut self.particles[idx];
                p.time = cl_time;
                vector_clear(&mut p.accel);

                let d = i as f32 * 0.1;
                let c = d.cos();
                let s = d.sin();

                let dir = vector_ma(&vector_scale(&right, c), s, &up);

                p.alpha = 1.0;
                p.alphavel = -1.0;
                p.color = clr as f32;
                p.particle_type = PT_DEFAULT;
                for j in 0..3 {
                    p.org[j] = mov[j] + dir[j] * 3.0;
                    p.vel[j] = dir[j] * 4.0;
                }
                mov = vector_add(&mov, &vec2b);
            }
        } else {
            // old rail trail
            let mut mov = *start;
            let mut vec = vector_subtract(end, start);
            let mut len = vector_normalize(&mut vec);

            let (mut right, mut up) = ([0.0f32; 3], [0.0f32; 3]);
            make_normal_vectors(&vec, &mut right, &mut up);

            // spiral
            for i in 0..(len as i32) {
                let idx = match self.alloc_particle() {
                    Some(idx) => idx,
                    None => return,
                };
                let p = &mut self.particles[idx];
                p.time = cl_time;
                vector_clear(&mut p.accel);

                let d = i as f32 * 0.1;
                let c = d.cos();
                let s = d.sin();

                let dir = vector_ma(&vector_scale(&right, c), s, &up);

                p.alpha = 1.0;
                p.alphavel = -1.0 / (1.0 + frand() * 0.2);
                p.color = (clr + (qrand() & 7) as u8) as f32;
                p.particle_type = PT_DEFAULT;
                for j in 0..3 {
                    p.org[j] = mov[j] + dir[j] * 3.0;
                    p.vel[j] = dir[j] * 6.0;
                }
                mov = vector_add(&mov, &vec);
            }

            // beam
            let dec: f32 = 0.75;
            vec = vector_scale(&vec, dec);
            mov = *start;
            while len > 0.0 {
                len -= dec;

                let idx = match self.alloc_particle() {
                    Some(i) => i,
                    None => return,
                };
                let p = &mut self.particles[idx];
                p.time = cl_time;
                vector_clear(&mut p.accel);

                p.alpha = 1.0;
                p.alphavel = -1.0 / (0.6 + frand() * 0.2);
                p.color = (qrand() & 15) as f32;
                p.particle_type = PT_DEFAULT;

                for j in 0..3 {
                    p.org[j] = mov[j] + crand() * 3.0;
                    p.vel[j] = crand() * 3.0;
                    p.accel[j] = 0.0;
                }
                mov = vector_add(&mov, &vec);
            }
        }

        // Register rail trail for continuation during packet loss
        // Rail trails are instant/short-lived, use sparks effect type
        // Effect type 2 = sparks (for the beam effect)
        self.register_effect(start, &[0.0, 0.0, 0.0], 2, 300, cl_time as i32);
    }

    // ============================================================
    // CL_IonripperTrail (RAFAEL)
    // ============================================================

    pub fn cl_ionripper_trail(&mut self, start: &Vec3, end: &Vec3, cl_time: f32) {
        let mut mov = *start;
        let mut vec = vector_subtract(end, start);
        let mut len = vector_normalize(&mut vec);

        let dec = 5;
        vec = vector_scale(&vec, 5.0);

        let mut left = false;

        while len > 0.0 {
            len -= dec as f32;

            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            vector_clear(&mut p.accel);

            p.time = cl_time;
            p.alpha = 0.5;
            p.alphavel = -1.0 / (0.3 + frand() * 0.2);
            p.color = (0xe4 + (qrand() & 3)) as f32;
            p.particle_type = PT_DEFAULT;

            for j in 0..3 {
                p.org[j] = mov[j];
                p.accel[j] = 0.0;
            }
            if left {
                left = false;
                p.vel[0] = 10.0;
            } else {
                left = true;
                p.vel[0] = -10.0;
            }
            p.vel[1] = 0.0;
            p.vel[2] = 0.0;

            mov = vector_add(&mov, &vec);
        }
    }

    // ============================================================
    // CL_BubbleTrail
    // ============================================================

    pub fn cl_bubble_trail(&mut self, start: &Vec3, end: &Vec3, cl_time: f32) {
        let mut mov = *start;
        let mut vec = vector_subtract(end, start);
        let len = vector_normalize(&mut vec);

        let dec: f32 = 32.0;
        vec = vector_scale(&vec, dec);

        let mut i: f32 = 0.0;
        while i < len {
            i += dec;

            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            vector_clear(&mut p.accel);
            p.time = cl_time;

            p.alpha = 1.0;
            p.alphavel = -1.0 / (1.0 + frand() * 0.2);
            p.color = (4 + (qrand() & 7)) as f32;
            p.particle_type = PT_BUBBLE;
            for j in 0..3 {
                p.org[j] = mov[j] + crand() * 2.0;
                p.vel[j] = crand() * 5.0;
            }
            p.vel[2] += 6.0;

            mov = vector_add(&mov, &vec);
        }
    }

    // ============================================================
    // CL_FlyParticles
    // ============================================================

    pub fn cl_fly_particles(&mut self, origin: &Vec3, count: i32, cl_time: f32) {
        let count = count.min(NUMVERTEXNORMALS as i32) as usize;

        if self.avelocities[0][0] == 0.0 {
            for i in 0..NUMVERTEXNORMALS * 3 {
                // flatten: avelocities[i/3][i%3]
                self.avelocities[i / 3][i % 3] = (qrand() & 255) as f32 * 0.01;
            }
        }

        let ltime = cl_time / 1000.0;
        let mut i = 0;
        while i < count {
            let angle = ltime * self.avelocities[i][0];
            let sy = angle.sin();
            let cy = angle.cos();
            let angle = ltime * self.avelocities[i][1];
            let sp = angle.sin();
            let cp = angle.cos();
            let _angle = ltime * self.avelocities[i][2];
            // sr, cr unused in original

            let mut forward = [0.0f32; 3];
            forward[0] = cp * cy;
            forward[1] = cp * sy;
            forward[2] = -sp;

            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            p.time = cl_time;

            let dist = (ltime + i as f32).sin() * 64.0;
            p.org[0] = origin[0] + BYTEDIRS[i][0] * dist + forward[0] * BEAMLENGTH;
            p.org[1] = origin[1] + BYTEDIRS[i][1] * dist + forward[1] * BEAMLENGTH;
            p.org[2] = origin[2] + BYTEDIRS[i][2] * dist + forward[2] * BEAMLENGTH;

            vector_clear(&mut p.vel);
            vector_clear(&mut p.accel);

            p.color = 0.0;
            p.particle_type = PT_DEFAULT;
            p.colorvel = 0.0;

            p.alpha = 1.0;
            p.alphavel = -100.0;

            i += 2;
        }
    }

    pub fn cl_fly_effect(&mut self, ent: &mut CEntity, origin: &Vec3, cl_time: i32) {
        let starttime;

        if ent.fly_stoptime < cl_time {
            starttime = cl_time;
            ent.fly_stoptime = cl_time + 60000;
        } else {
            starttime = ent.fly_stoptime - 60000;
        }

        let n = cl_time - starttime;
        let count = if n < 20000 {
            (n as f32 * 162.0 / 20000.0) as i32
        } else {
            let n = ent.fly_stoptime - cl_time;
            if n < 20000 {
                (n as f32 * 162.0 / 20000.0) as i32
            } else {
                162
            }
        };

        self.cl_fly_particles(origin, count, cl_time as f32);
    }

    // ============================================================
    // CL_BfgParticles
    // ============================================================

    pub fn cl_bfg_particles(&mut self, ent: &RenderEntity, cl_time: f32) {
        if self.avelocities[0][0] == 0.0 {
            for i in 0..NUMVERTEXNORMALS * 3 {
                self.avelocities[i / 3][i % 3] = (qrand() & 255) as f32 * 0.01;
            }
        }

        let ltime = cl_time / 1000.0;
        for i in 0..NUMVERTEXNORMALS {
            let angle = ltime * self.avelocities[i][0];
            let sy = angle.sin();
            let cy = angle.cos();
            let angle = ltime * self.avelocities[i][1];
            let sp = angle.sin();
            let cp = angle.cos();

            let mut forward = [0.0f32; 3];
            forward[0] = cp * cy;
            forward[1] = cp * sy;
            forward[2] = -sp;

            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            p.time = cl_time;

            let dist = (ltime + i as f32).sin() * 64.0;
            p.org[0] = ent.origin[0] + BYTEDIRS[i][0] * dist + forward[0] * BEAMLENGTH;
            p.org[1] = ent.origin[1] + BYTEDIRS[i][1] * dist + forward[1] * BEAMLENGTH;
            p.org[2] = ent.origin[2] + BYTEDIRS[i][2] * dist + forward[2] * BEAMLENGTH;

            vector_clear(&mut p.vel);
            vector_clear(&mut p.accel);

            let v = vector_subtract(&p.org, &ent.origin);
            let dist2 = vector_length(&v) / 90.0;
            p.color = (0xd0 as f32 + dist2 * 7.0).floor();
            p.particle_type = PT_DEFAULT;
            p.colorvel = 0.0;

            p.alpha = 1.0 - dist2;
            p.alphavel = -100.0;
        }
    }

    // ============================================================
    // CL_TrapParticles (RAFAEL)
    // ============================================================

    pub fn cl_trap_particles(&mut self, ent: &mut RenderEntity, cl_time: f32) {
        ent.origin[2] -= 14.0;
        let start = ent.origin;
        let mut end = ent.origin;
        end[2] += 64.0;

        let mut mov = start;
        let mut vec = vector_subtract(&end, &start);
        let mut len = vector_normalize(&mut vec);

        let dec = 5;
        vec = vector_scale(&vec, 5.0);

        while len > 0.0 {
            len -= dec as f32;

            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            vector_clear(&mut p.accel);

            p.time = cl_time;
            p.alpha = 1.0;
            p.alphavel = -1.0 / (0.3 + frand() * 0.2);
            p.color = 0xe0 as f32;
            p.particle_type = PT_DEFAULT;
            for j in 0..3 {
                p.org[j] = mov[j] + crand();
                p.vel[j] = crand() * 15.0;
                p.accel[j] = 0.0;
            }
            p.accel[2] = PARTICLE_GRAVITY;

            mov = vector_add(&mov, &vec);
        }

        {
            ent.origin[2] += 14.0;
            let org = ent.origin;

            let mut i: i32 = -2;
            while i <= 2 {
                let mut j: i32 = -2;
                while j <= 2 {
                    let mut k: i32 = -2;
                    while k <= 4 {
                        let idx = match self.alloc_particle() {
                            Some(i) => i,
                            None => return,
                        };
                        let p = &mut self.particles[idx];

                        p.time = cl_time;
                        p.color = (0xe0 + (qrand() & 3)) as f32;
                        p.particle_type = PT_DEFAULT;

                        p.alpha = 1.0;
                        p.alphavel = -1.0 / (0.3 + (qrand() & 7) as f32 * 0.02);

                        p.org[0] = org[0] + i as f32 + ((qrand() & 23) as f32 * crand());
                        p.org[1] = org[1] + j as f32 + ((qrand() & 23) as f32 * crand());
                        p.org[2] = org[2] + k as f32 + ((qrand() & 23) as f32 * crand());

                        let mut dir = [0.0f32; 3];
                        dir[0] = j as f32 * 8.0;
                        dir[1] = i as f32 * 8.0;
                        dir[2] = k as f32 * 8.0;

                        vector_normalize(&mut dir);
                        let vel = ((50 + qrand()) & 63) as f32;
                        vector_scale_to(&dir, vel, &mut p.vel);

                        p.accel[0] = 0.0;
                        p.accel[1] = 0.0;
                        p.accel[2] = -PARTICLE_GRAVITY;

                        k += 4;
                    }
                    j += 4;
                }
                i += 4;
            }
        }
    }

    // ============================================================
    // CL_BFGExplosionParticles
    // ============================================================

    pub fn cl_bfg_explosion_particles(&mut self, org: &Vec3, cl_time: f32) {
        for _ in 0..256 {
            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            p.time = cl_time;
            p.color = (0xd0 + (qrand() & 7)) as f32;
            p.particle_type = PT_DEFAULT;

            for j in 0..3 {
                p.org[j] = org[j] + ((qrand() % 32) - 16) as f32;
                p.vel[j] = ((qrand() % 384) - 192) as f32;
            }

            p.accel[0] = 0.0;
            p.accel[1] = 0.0;
            p.accel[2] = -PARTICLE_GRAVITY;
            p.alpha = 1.0;
            p.alphavel = -0.8 / (0.5 + frand() * 0.3);
        }

        // Register BFG explosion for continuation during packet loss
        // Effect type 2 = sparks (green BFG particles)
        self.register_effect(org, &[0.0, 0.0, 0.0], 2, 600, cl_time as i32);
    }

    // ============================================================
    // CL_TeleportParticles
    // ============================================================

    pub fn cl_teleport_particles(&mut self, org: &Vec3, cl_time: f32) {
        let mut i: i32 = -16;
        while i <= 16 {
            let mut j: i32 = -16;
            while j <= 16 {
                let mut k: i32 = -16;
                while k <= 32 {
                    let idx = match self.alloc_particle() {
                        Some(idx) => idx,
                        None => return,
                    };
                    let p = &mut self.particles[idx];

                    p.time = cl_time;
                    p.color = (7 + (qrand() & 7)) as f32;
                    p.particle_type = PT_DEFAULT;

                    p.alpha = 1.0;
                    p.alphavel = -1.0 / (0.3 + (qrand() & 7) as f32 * 0.02);

                    p.org[0] = org[0] + i as f32 + (qrand() & 3) as f32;
                    p.org[1] = org[1] + j as f32 + (qrand() & 3) as f32;
                    p.org[2] = org[2] + k as f32 + (qrand() & 3) as f32;

                    let mut dir = [0.0f32; 3];
                    dir[0] = j as f32 * 8.0;
                    dir[1] = i as f32 * 8.0;
                    dir[2] = k as f32 * 8.0;

                    vector_normalize(&mut dir);
                    let vel = 50.0 + (qrand() & 63) as f32;
                    vector_scale_to(&dir, vel, &mut p.vel);

                    p.accel[0] = 0.0;
                    p.accel[1] = 0.0;
                    p.accel[2] = -PARTICLE_GRAVITY;

                    k += 4;
                }
                j += 4;
            }
            i += 4;
        }
    }

    // ============================================================
    // CL_AddParticles
    // ============================================================

    fn cl_add_particles<F>(&mut self, cl_time: f32, mut add_particle_fn: F)
    where
        F: FnMut(&Vec3, &Vec3, i32, f32, i32),
    {
        let mut active: Option<usize> = None;
        let mut tail: Option<usize> = None;

        let mut p_idx = self.active_particles;
        while let Some(idx) = p_idx {
            let next = self.particles[idx].next;

            let alpha;
            let time;

            // PMM - added INSTANT_PARTICLE handling for heat beam
            if self.particles[idx].alphavel != INSTANT_PARTICLE {
                time = (cl_time - self.particles[idx].time) * 0.001;
                alpha = self.particles[idx].alpha + time * self.particles[idx].alphavel;
                if alpha <= 0.0 {
                    // faded out
                    self.particles[idx].next = self.free_particles;
                    self.free_particles = Some(idx);
                    p_idx = next;
                    continue;
                }
            } else {
                time = 0.0;
                alpha = self.particles[idx].alpha;
            }

            self.particles[idx].next = None;
            if let Some(t) = tail {
                self.particles[t].next = Some(idx);
                tail = Some(idx);
            } else {
                active = Some(idx);
                tail = Some(idx);
            }

            let alpha = alpha.min(1.0);
            let color = self.particles[idx].color as i32;

            let time2 = time * time;

            let mut org = [0.0f32; 3];
            org[0] = self.particles[idx].org[0]
                + self.particles[idx].vel[0] * time
                + self.particles[idx].accel[0] * time2;
            org[1] = self.particles[idx].org[1]
                + self.particles[idx].vel[1] * time
                + self.particles[idx].accel[1] * time2;
            org[2] = self.particles[idx].org[2]
                + self.particles[idx].vel[2] * time
                + self.particles[idx].accel[2] * time2;

            add_particle_fn(
                &org,
                &self.particles[idx].length,
                color,
                alpha,
                self.particles[idx].particle_type,
            );

            // PMM
            if self.particles[idx].alphavel == INSTANT_PARTICLE {
                self.particles[idx].alphavel = 0.0;
                self.particles[idx].alpha = 0.0;
            }

            p_idx = next;
        }

        self.active_particles = active;
    }

    /// Parallel particle update - uses rayon for physics computation.
    ///
    /// This version maintains an `active_indices` array for contiguous parallel
    /// iteration, avoiding the linked-list traversal bottleneck.
    ///
    /// Phase 1: Parallel physics computation (position, alpha)
    /// Phase 2: Sequential render callback + compaction
    fn cl_add_particles_parallel<F>(&mut self, cl_time: f32, mut add_particle_fn: F)
    where
        F: FnMut(&Vec3, &Vec3, i32, f32, i32),
    {
        // Build active_indices from linked list if empty (first call or after clear)
        if self.active_indices.is_empty() && self.active_particles.is_some() {
            self.rebuild_active_indices();
        }

        if self.active_indices.is_empty() {
            self.active_particles = None;
            return;
        }

        // Phase 1: Parallel physics computation
        // Compute position and alpha for each particle in parallel
        let particles = &self.particles;
        let results: Vec<_> = self.active_indices
            .par_iter()
            .map(|&idx| {
                let p = &particles[idx];

                let (time, alpha) = if p.alphavel != INSTANT_PARTICLE {
                    let t = (cl_time - p.time) * 0.001;
                    let a = p.alpha + t * p.alphavel;
                    (t, a)
                } else {
                    (0.0, p.alpha)
                };

                if alpha <= 0.0 {
                    // Particle is dead
                    return (idx, None);
                }

                let time2 = time * time;
                let org = [
                    p.org[0] + p.vel[0] * time + p.accel[0] * time2,
                    p.org[1] + p.vel[1] * time + p.accel[1] * time2,
                    p.org[2] + p.vel[2] * time + p.accel[2] * time2,
                ];

                (idx, Some((
                    org,
                    p.length,
                    p.color as i32,
                    alpha.min(1.0),
                    p.particle_type,
                    p.alphavel == INSTANT_PARTICLE,
                )))
            })
            .collect();

        // Phase 2: Sequential render callback + compaction
        let mut new_active_indices = Vec::with_capacity(self.active_indices.len());

        for (idx, result) in results {
            if let Some((org, length, color, alpha, ptype, is_instant)) = result {
                // Render the particle
                add_particle_fn(&org, &length, color, alpha, ptype);

                // Handle instant particles
                if is_instant {
                    self.particles[idx].alphavel = 0.0;
                    self.particles[idx].alpha = 0.0;
                }

                // Keep this particle active
                new_active_indices.push(idx);
            } else {
                // Return dead particle to free list
                self.particles[idx].next = self.free_particles;
                self.free_particles = Some(idx);
            }
        }

        self.active_indices = new_active_indices;

        // Rebuild linked list from active_indices for compatibility
        self.rebuild_linked_list_from_indices();
    }

    /// Rebuild active_indices from the linked list.
    fn rebuild_active_indices(&mut self) {
        self.active_indices.clear();
        let mut p_idx = self.active_particles;
        while let Some(idx) = p_idx {
            self.active_indices.push(idx);
            p_idx = self.particles[idx].next;
        }
    }

    /// Rebuild the linked list from active_indices for API compatibility.
    fn rebuild_linked_list_from_indices(&mut self) {
        if self.active_indices.is_empty() {
            self.active_particles = None;
            return;
        }

        self.active_particles = Some(self.active_indices[0]);

        for i in 0..self.active_indices.len() {
            let idx = self.active_indices[i];
            self.particles[idx].next = if i + 1 < self.active_indices.len() {
                Some(self.active_indices[i + 1])
            } else {
                None
            };
        }
    }

    /// Track a newly allocated particle in active_indices.
    /// Call this after allocating a particle to keep indices in sync.
    pub fn track_active_particle(&mut self, idx: usize) {
        self.active_indices.push(idx);
    }

    /// Threshold for parallel particle processing.
    /// Below this, sequential is faster due to rayon overhead.
    const PARALLEL_PARTICLE_THRESHOLD: usize = 256;

    /// Smart particle update that chooses parallel or sequential based on count.
    /// Uses parallel processing when particle count exceeds threshold.
    pub fn cl_add_particles_smart<F>(&mut self, cl_time: f32, add_particle_fn: F)
    where
        F: FnMut(&Vec3, &Vec3, i32, f32, i32),
    {
        // Estimate particle count from active_indices or linked list
        let particle_count = if !self.active_indices.is_empty() {
            self.active_indices.len()
        } else {
            // Count from linked list
            let mut count = 0;
            let mut p_idx = self.active_particles;
            while let Some(idx) = p_idx {
                count += 1;
                p_idx = self.particles[idx].next;
            }
            count
        };

        if particle_count >= Self::PARALLEL_PARTICLE_THRESHOLD {
            self.cl_add_particles_parallel(cl_time, add_particle_fn);
        } else {
            self.cl_add_particles(cl_time, add_particle_fn);
        }
    }

    // ============================================================
    // CL_ClearEffects
    // ============================================================

    pub fn cl_clear_effects(&mut self) {
        self.cl_clear_particles();
        self.cl_clear_dlights();
        self.cl_clear_light_styles();
    }
}

// ============================================================
// MakeNormalVectors (standalone utility)
// ============================================================

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use myq2_common::q_shared::*;
    use crate::client::PARTICLE_GRAVITY;

    // ============================================================
    // Random number generator tests
    // ============================================================

    #[test]
    fn test_qrand_produces_values_in_range() {
        for _ in 0..1000 {
            let val = qrand();
            assert!(val >= 0, "qrand should return non-negative values, got {}", val);
            assert!(val <= 0x7fff, "qrand should return values <= 0x7fff, got {}", val);
        }
    }

    #[test]
    fn test_frand_produces_values_in_range() {
        for _ in 0..1000 {
            let val = frand();
            assert!(val >= 0.0, "frand should return >= 0.0, got {}", val);
            assert!(val < 1.0, "frand should return < 1.0, got {}", val);
        }
    }

    #[test]
    fn test_crand_produces_values_in_range() {
        for _ in 0..1000 {
            let val = crand();
            assert!(val >= -1.0, "crand should return >= -1.0, got {}", val);
            assert!(val < 1.0, "crand should return < 1.0, got {}", val);
        }
    }

    #[test]
    fn test_qrand_not_all_same() {
        // With 100 calls, we should get at least a few different values
        let mut values = std::collections::HashSet::new();
        for _ in 0..100 {
            values.insert(qrand());
        }
        assert!(values.len() > 1, "qrand should produce varied values");
    }

    // ============================================================
    // ClFxState construction and initialization tests
    // ============================================================

    #[test]
    fn test_cl_fx_state_new() {
        let state = ClFxState::new();
        assert_eq!(state.cl_lightstyle.len(), crate::client::MAX_LIGHTSTYLES);
        assert_eq!(state.cl_dlights.len(), crate::client::MAX_DLIGHTS);
        assert_eq!(state.particles.len(), crate::client::MAX_PARTICLES);
        assert_eq!(state.cl_numparticles, crate::client::MAX_PARTICLES);
        assert!(state.active_particles.is_none());
        assert!(state.free_particles.is_none());
        assert_eq!(state.lastofs, -1);
    }

    #[test]
    fn test_cl_clear_particles() {
        let mut state = ClFxState::new();
        state.cl_clear_particles();

        // Free list should start at 0
        assert_eq!(state.free_particles, Some(0));
        // Active list should be empty
        assert!(state.active_particles.is_none());
        // active_indices should be empty
        assert!(state.active_indices.is_empty());

        // Free list should form a chain
        for i in 0..(state.cl_numparticles - 1) {
            assert_eq!(state.particles[i].next, Some(i + 1),
                       "particle {} should point to {}", i, i + 1);
        }
        // Last particle should have no next
        assert!(state.particles[state.cl_numparticles - 1].next.is_none());
    }

    #[test]
    fn test_alloc_particle() {
        let mut state = ClFxState::new();
        state.cl_clear_particles();

        // Should allocate from index 0
        let idx = state.alloc_particle();
        assert_eq!(idx, Some(0));

        // Second allocation should be from index 1
        let idx = state.alloc_particle();
        assert_eq!(idx, Some(1));
    }

    #[test]
    fn test_alloc_particle_returns_none_when_exhausted() {
        let mut state = ClFxState::new();
        state.cl_clear_particles();

        // Exhaust all particles
        for _ in 0..state.cl_numparticles {
            assert!(state.alloc_particle().is_some());
        }

        // Next allocation should fail
        assert!(state.alloc_particle().is_none());
    }

    // ============================================================
    // Dlight management tests
    // ============================================================

    #[test]
    fn test_cl_clear_dlights() {
        let mut state = ClFxState::new();
        state.cl_dlights[0].radius = 100.0;
        state.cl_dlights[0].key = 42;
        state.cl_clear_dlights();

        for dl in &state.cl_dlights {
            assert_eq!(dl.radius, 0.0);
            assert_eq!(dl.key, 0);
        }
    }

    #[test]
    fn test_cl_alloc_dlight_by_key() {
        let mut state = ClFxState::new();
        state.cl_clear_dlights();

        // Allocate a dlight with key=5
        let idx = state.cl_alloc_dlight(5, 100.0);
        assert_eq!(state.cl_dlights[idx].key, 5);

        // Allocating with same key should reuse the same slot
        state.cl_dlights[idx].radius = 200.0;
        let idx2 = state.cl_alloc_dlight(5, 100.0);
        assert_eq!(idx, idx2);
        // It should be reset
        assert_eq!(state.cl_dlights[idx2].radius, 0.0);
    }

    #[test]
    fn test_cl_alloc_dlight_expired_slot() {
        let mut state = ClFxState::new();
        state.cl_clear_dlights();

        // Set up a dlight that has already expired
        state.cl_dlights[3].die = 50.0;
        state.cl_dlights[3].key = 99;

        // Allocate with key=0 at time=100; should find the expired slot
        let idx = state.cl_alloc_dlight(0, 100.0);
        assert!(idx <= crate::client::MAX_DLIGHTS);
    }

    #[test]
    fn test_cl_new_dlight() {
        let mut state = ClFxState::new();
        state.cl_clear_dlights();

        state.cl_new_dlight(10, 1.0, 2.0, 3.0, 100.0, 500.0, 1000.0);

        // Find the dlight with key 10
        let dl = state.cl_dlights.iter().find(|d| d.key == 10).unwrap();
        assert_eq!(dl.origin, [1.0, 2.0, 3.0]);
        assert_eq!(dl.radius, 100.0);
        assert_eq!(dl.die, 1500.0); // cl_time + time
        assert!(!dl.extended);
    }

    #[test]
    fn test_cl_run_dlights_decay() {
        let mut state = ClFxState::new();
        state.cl_clear_dlights();

        state.cl_dlights[0].radius = 100.0;
        state.cl_dlights[0].die = 2000.0;
        state.cl_dlights[0].decay = 50.0; // 50 units per second

        state.cl_run_dlights(1000.0, 0.5); // 0.5 second frame
        assert_eq!(state.cl_dlights[0].radius, 75.0); // 100 - 50*0.5
    }

    #[test]
    fn test_cl_run_dlights_expired() {
        let mut state = ClFxState::new();
        state.cl_clear_dlights();

        state.cl_dlights[0].radius = 100.0;
        state.cl_dlights[0].die = 500.0;

        state.cl_run_dlights(1000.0, 0.1); // die < cl_time -> radius = 0
        assert_eq!(state.cl_dlights[0].radius, 0.0);
    }

    #[test]
    fn test_cl_run_dlights_clamp_to_zero() {
        let mut state = ClFxState::new();
        state.cl_clear_dlights();

        state.cl_dlights[0].radius = 10.0;
        state.cl_dlights[0].die = 2000.0;
        state.cl_dlights[0].decay = 500.0;

        state.cl_run_dlights(1000.0, 1.0); // 10 - 500*1.0 = -490 -> clamped to 0
        assert_eq!(state.cl_dlights[0].radius, 0.0);
    }

    // ============================================================
    // Dlight packet loss extension tests
    // ============================================================

    #[test]
    fn test_cl_extend_dlights_for_packet_loss() {
        let mut state = ClFxState::new();
        state.cl_clear_dlights();

        state.cl_dlights[0].radius = 100.0;
        // die must be < cl_time + 0.2 to be considered "about to expire"
        // cl_time = 1000.0, threshold = 1000.2, so die = 1000.1 qualifies
        state.cl_dlights[0].die = 1000.1;
        state.cl_dlights[0].decay = 20.0;

        state.cl_extend_dlights_for_packet_loss(1000.0, 500.0);

        assert!(state.cl_dlights[0].extended);
        assert_eq!(state.cl_dlights[0].die, 1000.5); // cl_time + 500/1000
        assert_eq!(state.cl_dlights[0].decay, 10.0); // halved
    }

    #[test]
    fn test_cl_reset_extended_dlights() {
        let mut state = ClFxState::new();
        state.cl_clear_dlights();

        state.cl_dlights[0].extended = true;
        state.cl_dlights[0].decay = 10.0;

        state.cl_reset_extended_dlights();

        assert!(!state.cl_dlights[0].extended);
        assert_eq!(state.cl_dlights[0].decay, 20.0); // doubled back
    }

    // ============================================================
    // Light style tests
    // ============================================================

    #[test]
    fn test_cl_set_lightstyle() {
        let mut state = ClFxState::new();

        // 'a' is 0.0, 'm' is 1.0, 'z' is ~2.0
        state.cl_set_lightstyle(0, "am");
        assert_eq!(state.cl_lightstyle[0].length, 2);
        assert_eq!(state.cl_lightstyle[0].map[0], 0.0);     // 'a' - 'a' / ('m' - 'a') = 0.0
        assert_eq!(state.cl_lightstyle[0].map[1], 1.0);     // 'm' - 'a' / ('m' - 'a') = 1.0
    }

    #[test]
    fn test_cl_run_light_styles_default() {
        let mut state = ClFxState::new();
        // With length=0, values should be [1,1,1]
        state.cl_run_light_styles(100);
        assert_eq!(state.cl_lightstyle[0].value, [1.0, 1.0, 1.0]);
    }

    #[test]
    fn test_cl_run_light_styles_single_char() {
        let mut state = ClFxState::new();
        state.cl_set_lightstyle(1, "m"); // length=1, map[0]=1.0

        state.cl_run_light_styles(100);
        assert_eq!(state.cl_lightstyle[1].value, [1.0, 1.0, 1.0]);
    }

    #[test]
    fn test_cl_run_light_styles_animation() {
        let mut state = ClFxState::new();
        state.cl_set_lightstyle(2, "az"); // two frames

        // ofs = time/100; at time=0 -> ofs=0 -> map[0] = 0.0
        state.cl_run_light_styles(0);
        assert_eq!(state.cl_lightstyle[2].value[0], 0.0);

        // at time=100 -> ofs=1 -> map[1] = ('z'-'a')/('m'-'a') = 25/12
        state.lastofs = -1; // force recalc
        state.cl_run_light_styles(100);
        let expected = (b'z' as f32 - b'a' as f32) / (b'm' as f32 - b'a' as f32);
        assert!((state.cl_lightstyle[2].value[0] - expected).abs() < 0.001);
    }

    #[test]
    fn test_cl_clear_light_styles() {
        let mut state = ClFxState::new();
        state.cl_set_lightstyle(0, "mmmm");
        state.cl_clear_light_styles();
        assert_eq!(state.cl_lightstyle[0].length, 0);
        assert_eq!(state.lastofs, -1);
    }

    // ============================================================
    // Effect registration tests
    // ============================================================

    #[test]
    fn test_register_effect() {
        let mut state = ClFxState::new();
        let origin = [10.0, 20.0, 30.0];
        let velocity = [1.0, 2.0, 3.0];

        state.register_effect(&origin, &velocity, 1, 500, 1000);

        assert_eq!(state.recent_effects.len(), 1);
        assert_eq!(state.recent_effects[0].origin, origin);
        assert_eq!(state.recent_effects[0].velocity, velocity);
        assert_eq!(state.recent_effects[0].effect_type, 1);
        assert_eq!(state.recent_effects[0].start_time, 1000);
        assert_eq!(state.recent_effects[0].duration_ms, 500);
        assert!(state.recent_effects[0].active);
    }

    #[test]
    fn test_register_effect_limit() {
        let mut state = ClFxState::new();
        let origin = [0.0; 3];
        let velocity = [0.0; 3];

        // Fill to capacity (32)
        for i in 0..32 {
            state.register_effect(&origin, &velocity, 0, 100, i);
        }
        assert_eq!(state.recent_effects.len(), 32);

        // Adding one more should remove oldest
        state.register_effect(&origin, &velocity, 0, 100, 999);
        assert_eq!(state.recent_effects.len(), 32);
        // The oldest (start_time=0) should be gone, newest should be start_time=999
        assert_eq!(state.recent_effects.last().unwrap().start_time, 999);
        assert_eq!(state.recent_effects[0].start_time, 1); // second oldest is now first
    }

    #[test]
    fn test_take_recent_effects() {
        let mut state = ClFxState::new();
        let origin = [0.0; 3];
        let velocity = [0.0; 3];
        state.register_effect(&origin, &velocity, 0, 100, 0);
        state.register_effect(&origin, &velocity, 1, 200, 100);

        let taken = state.take_recent_effects();
        assert_eq!(taken.len(), 2);
        assert!(state.recent_effects.is_empty()); // cleared after take
    }

    #[test]
    fn test_cleanup_recent_effects() {
        let mut state = ClFxState::new();
        let origin = [0.0; 3];
        let velocity = [0.0; 3];

        // Active, not expired
        state.register_effect(&origin, &velocity, 0, 500, 1000);
        // Active, expired
        state.register_effect(&origin, &velocity, 1, 100, 100);

        state.cleanup_recent_effects(1200);

        // Only the first should remain (duration_ms=500, start=1000, 1200-1000=200 < 500+500)
        assert_eq!(state.recent_effects.len(), 1);
        assert_eq!(state.recent_effects[0].effect_type, 0);
    }

    // ============================================================
    // cl_clear_effects test
    // ============================================================

    #[test]
    fn test_cl_clear_effects() {
        let mut state = ClFxState::new();
        // Dirty up the state
        state.cl_dlights[0].radius = 100.0;
        state.cl_set_lightstyle(0, "zzzzz");
        state.cl_clear_particles();
        let _ = state.alloc_particle(); // have an active particle

        state.cl_clear_effects();

        // Dlights cleared
        assert_eq!(state.cl_dlights[0].radius, 0.0);
        // Light styles cleared
        assert_eq!(state.cl_lightstyle[0].length, 0);
        // Particles cleared (free list starts at 0)
        assert_eq!(state.free_particles, Some(0));
        assert!(state.active_particles.is_none());
    }

    // ============================================================
    // Particle effect tests (structural/count verification)
    // ============================================================

    #[test]
    fn test_cl_explosion_particles_count() {
        let mut state = ClFxState::new();
        state.cl_clear_particles();

        let org = [100.0, 200.0, 300.0];
        state.cl_explosion_particles(&org, 0.0);

        // Should have allocated 256 particles
        let mut count = 0;
        let mut idx = state.active_particles;
        while let Some(i) = idx {
            count += 1;
            idx = state.particles[i].next;
        }
        assert_eq!(count, 256);
    }

    #[test]
    fn test_cl_particle_effect_count() {
        let mut state = ClFxState::new();
        state.cl_clear_particles();

        let org = [0.0; 3];
        let dir = [0.0, 0.0, 1.0];
        state.cl_particle_effect(&org, &dir, 0xe0, 20, 100.0);

        let mut count = 0;
        let mut idx = state.active_particles;
        while let Some(i) = idx {
            count += 1;
            idx = state.particles[i].next;
        }
        assert_eq!(count, 20);
    }

    #[test]
    fn test_cl_blood_effect_particle_type() {
        let mut state = ClFxState::new();
        state.cl_clear_particles();

        let org = [0.0; 3];
        let dir = [0.0, 0.0, 1.0];
        state.cl_blood_effect(&org, &dir, 0xe8, 5, 100.0);

        // All blood particles should have PT_BLOOD type
        let mut idx = state.active_particles;
        while let Some(i) = idx {
            assert_eq!(state.particles[i].particle_type, PT_BLOOD);
            assert_eq!(state.particles[i].accel[2], -PARTICLE_GRAVITY);
            assert_eq!(state.particles[i].alpha, 1.0);
            idx = state.particles[i].next;
        }
    }

    #[test]
    fn test_cl_blaster_particles_properties() {
        let mut state = ClFxState::new();
        state.cl_clear_particles();

        let org = [50.0, 50.0, 50.0];
        let dir = [1.0, 0.0, 0.0];
        state.cl_blaster_particles(&org, &dir, 0.0);

        // Should allocate 40 particles
        let mut count = 0;
        let mut idx = state.active_particles;
        while let Some(i) = idx {
            count += 1;
            assert_eq!(state.particles[i].particle_type, PT_DEFAULT);
            assert_eq!(state.particles[i].alpha, 1.0);
            idx = state.particles[i].next;
        }
        assert_eq!(count, 40);
    }

    // ============================================================
    // make_normal_vectors tests
    // ============================================================

    #[test]
    fn test_make_normal_vectors_orthogonal() {
        let forward = [1.0, 0.0, 0.0];
        let mut right = [0.0; 3];
        let mut up = [0.0; 3];

        make_normal_vectors(&forward, &mut right, &mut up);

        // right and up should be perpendicular to forward
        let dot_fr = dot_product(&forward, &right);
        let dot_fu = dot_product(&forward, &up);
        let dot_ru = dot_product(&right, &up);

        assert!(dot_fr.abs() < 1e-5, "forward.right = {} (should be ~0)", dot_fr);
        assert!(dot_fu.abs() < 1e-5, "forward.up = {} (should be ~0)", dot_fu);
        assert!(dot_ru.abs() < 1e-5, "right.up = {} (should be ~0)", dot_ru);
    }

    #[test]
    fn test_make_normal_vectors_unit_length() {
        let forward = [0.0, 0.0, 1.0];
        let mut right = [0.0; 3];
        let mut up = [0.0; 3];

        make_normal_vectors(&forward, &mut right, &mut up);

        let right_len = vector_length(&right);
        let up_len = vector_length(&up);

        assert!((right_len - 1.0).abs() < 1e-5, "right length = {} (should be ~1)", right_len);
        assert!((up_len - 1.0).abs() < 1e-5, "up length = {} (should be ~1)", up_len);
    }

    #[test]
    fn test_make_normal_vectors_diagonal() {
        let mut forward = [1.0, 1.0, 0.0];
        vector_normalize(&mut forward);
        let mut right = [0.0; 3];
        let mut up = [0.0; 3];

        make_normal_vectors(&forward, &mut right, &mut up);

        let dot_fr = dot_product(&forward, &right);
        let dot_fu = dot_product(&forward, &up);

        assert!(dot_fr.abs() < 1e-5, "forward.right = {} (should be ~0)", dot_fr);
        assert!(dot_fu.abs() < 1e-5, "forward.up = {} (should be ~0)", dot_fu);
    }

    // ============================================================
    // CDlight default test
    // ============================================================

    #[test]
    fn test_cdlight_default() {
        let dl = CDlight::default();
        assert_eq!(dl.key, 0);
        assert_eq!(dl.radius, 0.0);
        assert_eq!(dl.die, 0.0);
        assert_eq!(dl.decay, 0.0);
        assert_eq!(dl.minlight, 0.0);
        assert_eq!(dl.color, [0.0; 3]);
        assert_eq!(dl.origin, [0.0; 3]);
        assert!(!dl.extended);
        assert_eq!(dl.original_die, 0.0);
    }

    // ============================================================
    // CParticle default test
    // ============================================================

    #[test]
    fn test_cparticle_default() {
        let p = CParticle::default();
        assert!(p.next.is_none());
        assert_eq!(p.time, 0.0);
        assert_eq!(p.org, [0.0; 3]);
        assert_eq!(p.vel, [0.0; 3]);
        assert_eq!(p.accel, [0.0; 3]);
        assert_eq!(p.particle_type, PT_DEFAULT);
        assert_eq!(p.alpha, 0.0);
    }

    // ============================================================
    // Dlight add callback test
    // ============================================================

    #[test]
    fn test_cl_add_dlights_callback() {
        let mut state = ClFxState::new();
        state.cl_clear_dlights();

        state.cl_dlights[0].radius = 150.0;
        state.cl_dlights[0].origin = [10.0, 20.0, 30.0];
        state.cl_dlights[0].color = [1.0, 0.5, 0.0];

        let mut calls = Vec::new();
        state.cl_add_dlights(|origin, radius, r, g, b| {
            calls.push((*origin, radius, r, g, b));
        });

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, [10.0, 20.0, 30.0]);
        assert_eq!(calls[0].1, 150.0);
        assert_eq!(calls[0].2, 1.0);
        assert_eq!(calls[0].3, 0.5);
        assert_eq!(calls[0].4, 0.0);
    }

    #[test]
    fn test_cl_add_dlights_skips_zero_radius() {
        let mut state = ClFxState::new();
        state.cl_clear_dlights();

        // All dlights have radius=0 by default
        let mut count = 0;
        state.cl_add_dlights(|_, _, _, _, _| { count += 1; });
        assert_eq!(count, 0);
    }
}

pub fn make_normal_vectors(forward: &Vec3, right: &mut Vec3, up: &mut Vec3) {
    // this rotate and negate guarantees a vector
    // not colinear with the original
    right[1] = -forward[0];
    right[2] = forward[1];
    right[0] = forward[2];

    let d = dot_product(right, forward);
    let tmp = vector_ma(right, -d, forward);
    *right = tmp;
    vector_normalize(right);
    *up = cross_product(right, forward);
}
