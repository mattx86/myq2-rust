// Sound types — converted from myq2-original/client/sound.h

use myq2_common::q_shared::Vec3;

/// Opaque sound effect handle (forward declaration equivalent of `struct sfx_s`).
/// The actual fields live in snd_loc and will be defined when that module is converted.
#[derive(Clone, Default)]
pub struct Sfx {
    // Converted from snd_loc.h: struct sfx_s
    pub name: String,                    // MAX_QPATH name
    pub registration_sequence: i32,      // for registration tracking
    pub cache: Option<Box<SfxCache>>,    // cached sound data
    pub truename: Option<String>,        // aliased name (if any)
}

/// Cached sound data (converted from sfxcache_t in snd_loc.h).
#[derive(Clone)]
pub struct SfxCache {
    pub length: i32,
    pub loopstart: i32,
    pub speed: i32,
    pub width: i32,
    pub stereo: i32,
    pub data: Vec<u8>,    // variable-sized in C (byte data[1])
}

impl Default for SfxCache {
    fn default() -> Self {
        Self {
            length: 0,
            loopstart: -1,
            speed: 0,
            width: 0,
            stereo: 0,
            data: Vec::new(),
        }
    }
}

// ---- Function signatures as trait ----
// These mirror the public API declared in sound.h.
// Implementing them as a trait allows swapping backends (real audio vs null driver).

pub trait SoundSystem {
    fn init(&mut self);
    fn shutdown(&mut self);

    /// If `origin` is `None`, the sound will be dynamically sourced from the entity.
    fn start_sound(
        &mut self,
        origin: Option<&Vec3>,
        entnum: i32,
        entchannel: i32,
        sfx: &Sfx,
        fvol: f32,
        attenuation: f32,
        timeofs: f32,
    );

    fn start_local_sound(&mut self, name: &str);

    fn raw_samples(
        &mut self,
        samples: i32,
        rate: i32,
        width: i32,
        channels: i32,
        data: &[u8],
    );

    fn stop_all_sounds(&mut self);
    fn update(&mut self, origin: &Vec3, v_forward: &Vec3, v_right: &Vec3, v_up: &Vec3);

    fn activate(&mut self, active: bool);

    fn begin_registration(&mut self);
    fn register_sound(&mut self, sample: &str) -> Option<&Sfx>;
    fn end_registration(&mut self);

    fn find_name(&mut self, name: &str, create: bool) -> Option<&Sfx>;
}
