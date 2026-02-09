// snd_dma.rs — Sound system dispatch and channel management
// Converted from: myq2-original/client/snd_dma.c
// Rewritten to dispatch via AudioBackend (OpenAL) instead of DMA ring buffer + software mixer.

#![allow(non_snake_case, non_upper_case_globals, unused)]

use myq2_common::q_shared::*;
use myq2_common::common::{com_printf, com_dprintf};
use rayon::prelude::*;

// ============================================================
// Constants
// ============================================================

/// Only begin attenuating sound volumes when outside this range
pub const SOUND_FULLVOLUME: f32 = 80.0;
pub const SOUND_LOOPATTENUATE: f32 = 0.003;

pub const MAX_SFX: usize = MAX_SOUNDS * 2;
pub const MAX_PLAYSOUNDS: usize = 128;
pub const MAX_CHANNELS: usize = 32;

// ============================================================
// Types
// ============================================================

/// Action to take on a sound channel during parallel update.
#[derive(Debug, Clone)]
enum ChannelAction {
    /// Stop the channel and clear it (for autosound)
    StopAndClear,
    /// Just clear the channel (no longer playing)
    Clear,
    /// Update channel position to new origin
    UpdatePosition(Vec3),
    /// Extrapolate position using velocity (packet loss)
    ExtrapolatePosition,
    /// Channel is fine, no action needed
    KeepPlaying,
}

/// Audio format descriptor for buffer uploads to the backend.
pub struct AudioFormat {
    pub sample_rate: u32,
    pub bits_per_sample: u16,
    pub channels: u16,
}

/// Room analysis data for automatic reverb environment detection.
///
/// Computed by tracing rays from the listener position to detect room characteristics.
#[derive(Debug, Clone, Default)]
pub struct RoomAnalysis {
    /// Whether the listener is underwater (waterlevel >= 3).
    pub underwater: bool,
    /// Average distance to walls in each direction (units).
    /// Index: 0=+X, 1=-X, 2=+Y, 3=-Y, 4=+Z (ceiling), 5=-Z (floor)
    pub wall_distances: [f32; 6],
    /// Estimated room volume in cubic units.
    pub room_volume: f32,
    /// Dominant surface material (detected from textures).
    pub surface_material: SurfaceMaterial,
    /// Whether the room is outdoors (sky visible above).
    pub outdoors: bool,
    /// Estimated room height (ceiling - floor distance).
    pub room_height: f32,
    /// Number of walls detected (for corridor detection).
    pub walls_detected: u8,
}

/// Surface material types for reverb calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SurfaceMaterial {
    #[default]
    Stone,
    Metal,
    Wood,
    Carpet,
    Concrete,
    Water,
    Snow,
    Glass,
    Dirt,
}

impl RoomAnalysis {
    /// Create a new room analysis from ray trace results.
    pub fn from_traces(
        underwater: bool,
        distances: [f32; 6],
        material: SurfaceMaterial,
        sky_above: bool,
    ) -> Self {
        let room_width = distances[0] + distances[1];
        let room_depth = distances[2] + distances[3];
        let room_height = distances[4] + distances[5];
        let room_volume = room_width * room_depth * room_height;

        // Count walls closer than 512 units
        let walls_detected = distances.iter()
            .filter(|&&d| d < 512.0)
            .count() as u8;

        Self {
            underwater,
            wall_distances: distances,
            room_volume,
            surface_material: material,
            outdoors: sky_above,
            room_height,
            walls_detected,
        }
    }
}

// SfxCache is defined in sound_types.rs (canonical location)
pub use crate::sound_types::SfxCache;

// Sfx is defined in sound_types.rs (canonical location)
pub use crate::sound_types::Sfx;

#[derive(Clone, Default)]
pub struct Playsound {
    pub prev: usize,
    pub next: usize,
    pub sfx_index: Option<usize>,
    pub volume: f32,
    pub attenuation: f32,
    pub entnum: i32,
    pub entchannel: i32,
    pub fixed_origin: bool,
    pub origin: Vec3,
    pub begin: u32,
}

#[derive(Clone, Default)]
pub struct Channel {
    pub sfx_index: Option<usize>,
    pub entnum: i32,
    pub entchannel: i32,
    pub origin: Vec3,
    pub dist_mult: f32,
    pub master_vol: i32,
    pub fixed_origin: bool,
    pub autosound: bool,

    // === Sound continuation during packet loss ===
    /// Last time this channel was confirmed by server (client realtime)
    pub last_confirmed_time: i32,
    /// Whether this is a looping sound that should continue during packet loss
    pub is_looping: bool,
    /// Timeout for continuing looping sounds (ms) - continue for up to 500ms
    pub continuation_timeout: i32,

    // === Sound position smoothing ===
    /// Previous origin for interpolation
    pub prev_origin: Vec3,
    /// Target origin for interpolation
    pub target_origin: Vec3,
    /// Time when position update was received (for interpolation)
    pub position_update_time: i32,
    /// Whether we're currently interpolating position
    pub interpolating: bool,

    // === Velocity extrapolation for packet loss ===
    /// Estimated velocity for extrapolation (units per second)
    pub velocity: Vec3,
    /// Whether velocity estimate is valid
    pub velocity_valid: bool,
    /// Time of last velocity calculation
    pub velocity_update_time: i32,

    // === Doppler effect ===
    /// Current Doppler pitch multiplier (1.0 = no shift)
    pub doppler_pitch: f32,
    /// Whether Doppler effect is enabled for this channel
    pub doppler_enabled: bool,
}

impl Channel {
    /// Duration for position interpolation in milliseconds.
    /// Set to half of SERVER_FRAMETIME_MS (50ms) for smooth audio without
    /// noticeable latency. Shorter values are more responsive but jittery,
    /// longer values are smoother but can cause audible position lag.
    const POSITION_LERP_MS: i32 = 50;

    /// Maximum extrapolation time during packet loss (ms)
    const MAX_EXTRAPOLATION_MS: i32 = 300;

    /// Update the position target and start interpolation
    pub fn set_position_target(&mut self, new_origin: &Vec3, current_time: i32) {
        if !self.interpolating {
            // First update - snap to position
            self.prev_origin = *new_origin;
            self.origin = *new_origin;
            self.target_origin = *new_origin;
            self.interpolating = true;
            self.position_update_time = current_time;
            self.velocity_valid = false;
        } else {
            // Calculate velocity from position change
            let dt = (current_time - self.position_update_time) as f32 / 1000.0;
            if dt > 0.001 && dt < 1.0 {
                // Calculate instantaneous velocity
                let new_velocity = [
                    (new_origin[0] - self.target_origin[0]) / dt,
                    (new_origin[1] - self.target_origin[1]) / dt,
                    (new_origin[2] - self.target_origin[2]) / dt,
                ];

                // Smooth velocity with exponential moving average
                if self.velocity_valid {
                    for i in 0..3 {
                        self.velocity[i] = self.velocity[i] * 0.7 + new_velocity[i] * 0.3;
                    }
                } else {
                    self.velocity = new_velocity;
                }
                self.velocity_valid = true;
                self.velocity_update_time = current_time;
            }

            // Subsequent update - start interpolation from current position
            self.prev_origin = self.origin;
            self.target_origin = *new_origin;
            self.position_update_time = current_time;
        }
    }

    /// Get extrapolated position during packet loss
    /// Uses velocity to predict where the sound source would be
    pub fn get_extrapolated_position(&self, current_time: i32) -> Vec3 {
        if !self.velocity_valid {
            return self.origin;
        }

        let dt = (current_time - self.position_update_time) as f32 / 1000.0;
        if dt <= 0.0 || dt > Self::MAX_EXTRAPOLATION_MS as f32 / 1000.0 {
            return self.origin;
        }

        // Extrapolate from target position using velocity
        [
            self.target_origin[0] + self.velocity[0] * dt,
            self.target_origin[1] + self.velocity[1] * dt,
            self.target_origin[2] + self.velocity[2] * dt,
        ]
    }

    /// Clear velocity tracking (for channel reuse)
    pub fn clear_velocity(&mut self) {
        self.velocity = [0.0; 3];
        self.velocity_valid = false;
        self.velocity_update_time = 0;
    }

    /// Get the smoothed position for audio spatialization
    pub fn get_smoothed_position(&self, current_time: i32) -> Vec3 {
        if !self.interpolating {
            return self.origin;
        }

        let elapsed = current_time - self.position_update_time;
        if elapsed >= Self::POSITION_LERP_MS {
            return self.target_origin;
        }

        let lerp = (elapsed as f32) / (Self::POSITION_LERP_MS as f32);
        [
            self.prev_origin[0] + (self.target_origin[0] - self.prev_origin[0]) * lerp,
            self.prev_origin[1] + (self.target_origin[1] - self.prev_origin[1]) * lerp,
            self.prev_origin[2] + (self.target_origin[2] - self.prev_origin[2]) * lerp,
        ]
    }

    /// Finalize interpolation (call after getting smoothed position)
    pub fn update_interpolation(&mut self, current_time: i32) {
        if self.interpolating && current_time - self.position_update_time >= Self::POSITION_LERP_MS {
            self.origin = self.target_origin;
        }
    }

    /// Speed of sound in game units per second
    /// Quake 2 uses 1 unit = ~1 inch, so ~8 units/meter
    /// Speed of sound ~343 m/s = ~2744 units/s
    /// We use a higher value for less extreme Doppler (more comfortable)
    const SPEED_OF_SOUND: f32 = 5000.0;

    /// Maximum Doppler pitch shift (prevents extreme warping)
    const MAX_DOPPLER_SHIFT: f32 = 0.3; // +/- 30% pitch shift

    /// Calculate Doppler pitch shift based on relative velocity
    /// listener_origin: position of the listener (player)
    /// listener_velocity: velocity of the listener (player movement)
    /// Returns pitch multiplier (1.0 = no shift, >1.0 = higher pitch, <1.0 = lower pitch)
    pub fn calculate_doppler(
        &mut self,
        listener_origin: &Vec3,
        listener_velocity: &Vec3,
    ) -> f32 {
        if !self.doppler_enabled || !self.velocity_valid {
            self.doppler_pitch = 1.0;
            return 1.0;
        }

        // Calculate direction from source to listener
        let dx = listener_origin[0] - self.origin[0];
        let dy = listener_origin[1] - self.origin[1];
        let dz = listener_origin[2] - self.origin[2];
        let distance = (dx * dx + dy * dy + dz * dz).sqrt();

        if distance < 1.0 {
            self.doppler_pitch = 1.0;
            return 1.0;
        }

        // Normalize direction
        let dir = [dx / distance, dy / distance, dz / distance];

        // Calculate velocity components along the line between source and listener
        // Positive = moving towards each other
        let listener_towards = listener_velocity[0] * dir[0]
            + listener_velocity[1] * dir[1]
            + listener_velocity[2] * dir[2];

        let source_towards = self.velocity[0] * dir[0]
            + self.velocity[1] * dir[1]
            + self.velocity[2] * dir[2];

        // Relative velocity: positive when approaching
        let relative_velocity = listener_towards - source_towards;

        // Calculate Doppler shift
        // When approaching: relative_velocity > 0, pitch increases
        // When receding: relative_velocity < 0, pitch decreases
        let pitch_shift = (relative_velocity / Self::SPEED_OF_SOUND)
            .clamp(-Self::MAX_DOPPLER_SHIFT, Self::MAX_DOPPLER_SHIFT);

        // Apply smoothing to prevent jarring pitch changes
        let target_pitch = 1.0 + pitch_shift;
        self.doppler_pitch = self.doppler_pitch * 0.8 + target_pitch * 0.2;

        self.doppler_pitch
    }

    /// Continue Doppler effect during packet loss (gradually return to normal)
    pub fn continue_doppler_during_packet_loss(&mut self) {
        if self.doppler_enabled {
            // Gradually return to normal pitch
            self.doppler_pitch = self.doppler_pitch * 0.95 + 1.0 * 0.05;
        }
    }
}

/// Tracked looping sound for continuation during packet loss
#[derive(Clone, Default)]
pub struct LoopingSound {
    /// Entity number this sound is attached to
    pub entnum: i32,
    /// Sound effect index
    pub sfx_index: Option<usize>,
    /// Last time confirmed by server
    pub last_confirmed_time: i32,
    /// Sound origin
    pub origin: Vec3,
    /// Whether currently active
    pub active: bool,
}

/// Maximum tracked looping sounds
pub const MAX_LOOPING_SOUNDS: usize = 64;

/// Looping sound continuation state
pub struct LoopingSoundState {
    pub sounds: [LoopingSound; MAX_LOOPING_SOUNDS],
    /// Timeout for sound continuation (default 500ms)
    pub continuation_timeout: i32,
    /// Whether sound continuation is enabled
    pub enabled: bool,
}

impl Default for LoopingSoundState {
    fn default() -> Self {
        Self {
            sounds: std::array::from_fn(|_| LoopingSound::default()),
            // Continue sounds for up to 500ms (5 server frames at 10Hz) during packet loss.
            // This prevents abrupt audio cutoff during brief network hiccups.
            continuation_timeout: 500,
            enabled: true,
        }
    }
}

impl LoopingSoundState {
    /// Register a looping sound from server update
    pub fn register(&mut self, entnum: i32, sfx_index: Option<usize>, origin: Vec3, current_time: i32) {
        // Find existing or empty slot
        let mut slot = None;
        let mut empty_slot = None;

        for i in 0..MAX_LOOPING_SOUNDS {
            if self.sounds[i].entnum == entnum && self.sounds[i].sfx_index == sfx_index {
                slot = Some(i);
                break;
            }
            if empty_slot.is_none() && !self.sounds[i].active {
                empty_slot = Some(i);
            }
        }

        let idx = slot.or(empty_slot);
        if let Some(i) = idx {
            self.sounds[i].entnum = entnum;
            self.sounds[i].sfx_index = sfx_index;
            self.sounds[i].origin = origin;
            self.sounds[i].last_confirmed_time = current_time;
            self.sounds[i].active = true;
        }
    }

    /// Get sounds that should continue playing during packet loss
    pub fn get_continuing_sounds(&self, current_time: i32) -> Vec<&LoopingSound> {
        if !self.enabled {
            return Vec::new();
        }

        self.sounds.iter()
            .filter(|s| {
                s.active &&
                current_time - s.last_confirmed_time > 0 &&
                current_time - s.last_confirmed_time < self.continuation_timeout
            })
            .collect()
    }

    /// Clear sounds that have timed out
    pub fn cleanup(&mut self, current_time: i32) {
        for sound in self.sounds.iter_mut() {
            if sound.active && current_time - sound.last_confirmed_time > self.continuation_timeout {
                sound.active = false;
            }
        }
    }

    /// Clear all tracked sounds
    pub fn clear(&mut self) {
        for sound in self.sounds.iter_mut() {
            *sound = LoopingSound::default();
        }
    }
}

#[derive(Clone, Default)]
pub struct WavInfo {
    pub rate: i32,
    pub width: i32,
    pub channels: i32,
    pub loopstart: i32,
    pub samples: i32,
    pub dataofs: i32,
}

// ============================================================
// Sound system state
// ============================================================

pub struct SoundState {
    pub s_registration_sequence: i32,
    pub channels: [Channel; MAX_CHANNELS],
    pub snd_initialized: bool,
    pub sound_started: bool,

    pub listener_origin: Vec3,
    pub listener_forward: Vec3,
    pub listener_right: Vec3,
    pub listener_up: Vec3,
    /// Listener velocity for Doppler effect calculation
    pub listener_velocity: Vec3,
    /// Previous listener origin for velocity calculation
    pub prev_listener_origin: Vec3,
    /// Time of last listener update
    pub listener_update_time: i32,
    /// Whether Doppler effect is globally enabled
    pub doppler_enabled: bool,

    pub s_registering: bool,

    pub known_sfx: Vec<Sfx>,
    pub num_sfx: usize,

    pub s_playsounds: Vec<Playsound>,
    pub s_freeplays_head: usize,
    pub s_pendingplays_head: usize,

    // Cvar values
    pub s_verbose: bool,
    pub s_volume: f32,
    pub s_volume_modified: bool,
    pub s_loadas8bit: bool,
    pub s_khz: i32,
    pub s_show: bool,

    // === Sound continuation during packet loss ===
    pub looping_sounds: LoopingSoundState,
}

impl Default for SoundState {
    fn default() -> Self {
        Self {
            s_registration_sequence: 0,
            channels: std::array::from_fn(|_| Channel::default()),
            snd_initialized: false,
            sound_started: false,
            listener_origin: [0.0; 3],
            listener_forward: [0.0; 3],
            listener_right: [0.0; 3],
            listener_up: [0.0; 3],
            listener_velocity: [0.0; 3],
            prev_listener_origin: [0.0; 3],
            listener_update_time: 0,
            doppler_enabled: true,
            s_registering: false,
            known_sfx: Vec::with_capacity(MAX_SFX),
            num_sfx: 0,
            s_playsounds: vec![Playsound::default(); MAX_PLAYSOUNDS + 2],
            s_freeplays_head: MAX_PLAYSOUNDS,
            s_pendingplays_head: MAX_PLAYSOUNDS + 1,
            s_verbose: false,
            s_volume: 0.5,
            s_volume_modified: false,
            s_loadas8bit: false,
            s_khz: 22,
            s_show: false,
            looping_sounds: LoopingSoundState::default(),
        }
    }
}

// ============================================================
// Audio backend trait (implemented by OpenAL in myq2-sys)
// ============================================================

pub trait AudioBackend {
    fn init(&mut self) -> bool;
    fn shutdown(&mut self);
    fn play_sound(
        &mut self,
        channel: usize,
        sfx_data: &[u8],
        format: &AudioFormat,
        origin: &[f32; 3],
        volume: f32,
        attenuation: f32,
        looping: bool,
    );
    fn stop_channel(&mut self, channel: usize);
    fn update_listener(&mut self, origin: &[f32; 3], forward: &[f32; 3], up: &[f32; 3]);
    fn update_channel_position(&mut self, channel: usize, origin: &[f32; 3]);
    /// Update a channel's velocity for Doppler shift calculation.
    fn update_channel_velocity(&mut self, _channel: usize, _velocity: &[f32; 3]) {}
    /// Update a channel's pitch/playback rate for Doppler effect.
    /// pitch: 1.0 = normal, >1.0 = higher pitch (approaching), <1.0 = lower pitch (receding)
    fn update_channel_pitch(&mut self, _channel: usize, _pitch: f32) {}
    /// Set the reverb environment manually (0=generic, 1=underwater, 2=cave, etc.).
    fn set_environment(&mut self, _env: i32) {}
    /// Automatically detect and set reverb environment based on room analysis.
    ///
    /// # Arguments
    /// * `room_data` - Room analysis data computed from ray traces
    fn auto_detect_environment(&mut self, _room_data: &RoomAnalysis) {}
    fn is_channel_playing(&self, channel: usize) -> bool;
    fn activate(&mut self, active: bool);

    /// Get playing state of all channels at once (for parallel processing).
    /// Default implementation calls is_channel_playing for each channel.
    fn get_channel_states(&self) -> [bool; MAX_CHANNELS] {
        let mut states = [false; MAX_CHANNELS];
        for i in 0..MAX_CHANNELS {
            states[i] = self.is_channel_playing(i);
        }
        states
    }

    // ---- Streaming audio for cinematics ----

    /// Queue raw audio samples for streaming playback (used by cinematics).
    ///
    /// # Arguments
    /// * `samples` - PCM audio data (16-bit signed, native endian)
    /// * `rate` - Sample rate in Hz (e.g., 22050, 44100)
    /// * `channels` - Number of channels (1 = mono, 2 = stereo)
    fn queue_streaming_samples(&mut self, _samples: &[i16], _rate: i32, _channels: i32) {}

    /// Check if streaming audio is currently playing.
    fn is_streaming_active(&self) -> bool {
        false
    }

    /// Stop streaming audio playback and clear queued buffers.
    fn stop_streaming(&mut self) {}
}

// ============================================================
// Functions
// ============================================================

impl SoundState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn s_init(&mut self, backend: &mut dyn AudioBackend) {
        com_printf("\n------- sound initialization -------\n");

        // Register s_khz cvar — used by cinematic audio (cl_cin.rs) and sound menu (menu.rs).
        // Default 48 KHz for modern hardware (original was 22 KHz).
        myq2_common::cvar::cvar_get("s_khz", "48", myq2_common::q_shared::CVAR_ARCHIVE);

        if !backend.init() {
            com_printf("OpenAL: not initializing.\n");
            com_printf("------------------------------------\n");
            return;
        }

        self.sound_started = true;
        self.num_sfx = 0;

        self.s_stop_all_sounds(Some(backend));

        com_printf("------------------------------------\n");
    }

    pub fn s_shutdown(&mut self, backend: &mut dyn AudioBackend) {
        if !self.sound_started {
            return;
        }

        for i in 0..MAX_CHANNELS {
            backend.stop_channel(i);
        }

        backend.shutdown();
        self.sound_started = false;

        for sfx in self.known_sfx.iter_mut() {
            if sfx.name.is_empty() {
                continue;
            }
            sfx.cache = None;
            *sfx = Sfx::default();
        }
        self.num_sfx = 0;
    }

    pub fn s_find_name(&mut self, name: &str, create: bool) -> Option<usize> {
        if name.is_empty() {
            panic!("S_FindName: empty name");
        }
        if name.len() >= MAX_QPATH {
            panic!("Sound name too long: {}", name);
        }

        for i in 0..self.num_sfx {
            if self.known_sfx[i].name == name {
                return Some(i);
            }
        }

        if !create {
            return None;
        }

        let mut slot = None;
        for i in 0..self.num_sfx {
            if self.known_sfx[i].name.is_empty() {
                slot = Some(i);
                break;
            }
        }

        let idx = if let Some(i) = slot {
            i
        } else {
            if self.num_sfx >= MAX_SFX {
                panic!("S_FindName: out of sfx_t");
            }
            let i = self.num_sfx;
            self.known_sfx.push(Sfx::default());
            self.num_sfx += 1;
            i
        };

        self.known_sfx[idx] = Sfx::default();
        self.known_sfx[idx].name = name.to_string();
        self.known_sfx[idx].registration_sequence = self.s_registration_sequence;

        Some(idx)
    }

    pub fn s_alias_name(&mut self, aliasname: &str, truename: &str) -> Option<usize> {
        let mut slot = None;
        for i in 0..self.num_sfx {
            if self.known_sfx[i].name.is_empty() {
                slot = Some(i);
                break;
            }
        }

        let idx = if let Some(i) = slot {
            i
        } else {
            if self.num_sfx >= MAX_SFX {
                panic!("S_FindName: out of sfx_t");
            }
            let i = self.num_sfx;
            self.known_sfx.push(Sfx::default());
            self.num_sfx += 1;
            i
        };

        self.known_sfx[idx] = Sfx::default();
        self.known_sfx[idx].name = aliasname.to_string();
        self.known_sfx[idx].registration_sequence = self.s_registration_sequence;
        self.known_sfx[idx].truename = Some(truename.to_string());

        Some(idx)
    }

    pub fn s_begin_registration(&mut self) {
        self.s_registration_sequence += 1;
        self.s_registering = true;
    }

    pub fn s_register_sound(&mut self, name: &str, load_file: &dyn Fn(&str) -> Option<Vec<u8>>) -> Option<usize> {
        if !self.sound_started {
            return None;
        }

        let idx = self.s_find_name(name, true)?;
        self.known_sfx[idx].registration_sequence = self.s_registration_sequence;

        if !self.s_registering {
            crate::snd_mem::s_load_sound(&mut self.known_sfx[idx], load_file);
        }

        Some(idx)
    }

    pub fn s_register_sexed_sound(
        &mut self,
        ent_number: i32,
        base: &str,
        configstrings: &[String],
        load_file: &dyn Fn(&str) -> Option<Vec<u8>>,
    ) -> Option<usize> {
        let mut model = String::new();
        let n = myq2_common::q_shared::CS_PLAYERSKINS + (ent_number as usize) - 1;
        if n < configstrings.len() && !configstrings[n].is_empty() {
            if let Some(pos) = configstrings[n].find('\\') {
                let after = &configstrings[n][pos + 1..];
                if let Some(slash) = after.find('/') {
                    model = after[..slash].to_string();
                } else {
                    model = after.to_string();
                }
            }
        }

        if model.is_empty() {
            model = "male".to_string();
        }

        let sexed_filename = format!("#players/{}/{}", model, &base[1..]);

        if load_file(&format!("players/{}/{}", model, &base[1..])).is_some() {
            return self.s_register_sound(&sexed_filename, load_file);
        }

        if model.starts_with("female") {
            let female_filename = format!("player/female/{}", &base[1..]);
            if load_file(&female_filename).is_some() {
                return self.s_register_sound(&format!("#{}", female_filename), load_file);
            }
        }

        let male_filename = format!("player/male/{}", &base[1..]);
        self.s_register_sound(&format!("#{}", male_filename), load_file)
    }

    /// End sound registration and load all registered sounds.
    /// Uses parallel loading via rayon for improved performance.
    pub fn s_end_registration<F>(&mut self, load_file: F)
    where
        F: Fn(&str) -> Option<Vec<u8>> + Send + Sync,
    {
        // Phase 1: Clear stale sounds (sequential - modifies registration state)
        for i in 0..self.num_sfx {
            if self.known_sfx[i].name.is_empty() {
                continue;
            }
            if self.known_sfx[i].registration_sequence != self.s_registration_sequence {
                self.known_sfx[i].cache = None;
                self.known_sfx[i] = Sfx::default();
            }
        }

        // Phase 2: Load sound files in parallel
        // Each sound is independent - file I/O and WAV parsing can run concurrently
        self.known_sfx[..self.num_sfx]
            .par_iter_mut()
            .filter(|sfx| !sfx.name.is_empty())
            .for_each(|sfx| {
                crate::snd_mem::s_load_sound(sfx, &load_file);
            });

        self.s_registering = false;
    }

    pub fn s_pick_channel(&mut self, entnum: i32, entchannel: i32, playernum: i32) -> Option<usize> {
        if entchannel < 0 {
            panic!("S_PickChannel: entchannel<0");
        }

        let mut first_to_die: i32 = -1;

        for ch_idx in 0..MAX_CHANNELS {
            if entchannel != 0
                && self.channels[ch_idx].entnum == entnum
                && self.channels[ch_idx].entchannel == entchannel
            {
                first_to_die = ch_idx as i32;
                break;
            }

            if self.channels[ch_idx].sfx_index.is_none() {
                first_to_die = ch_idx as i32;
                break;
            }

            if self.channels[ch_idx].entnum == playernum + 1
                && entnum != playernum + 1
                && self.channels[ch_idx].sfx_index.is_some()
            {
                continue;
            }

            if first_to_die == -1 {
                first_to_die = ch_idx as i32;
            }
        }

        if first_to_die == -1 {
            return None;
        }

        self.channels[first_to_die as usize] = Channel::default();
        Some(first_to_die as usize)
    }

    pub fn s_alloc_playsound(&mut self) -> Option<usize> {
        let sentinel = self.s_freeplays_head;
        let ps_idx = self.s_playsounds[sentinel].next;
        if ps_idx == sentinel {
            return None;
        }

        let prev = self.s_playsounds[ps_idx].prev;
        let next = self.s_playsounds[ps_idx].next;
        self.s_playsounds[prev].next = next;
        self.s_playsounds[next].prev = prev;

        Some(ps_idx)
    }

    pub fn s_free_playsound(&mut self, ps_idx: usize) {
        let prev = self.s_playsounds[ps_idx].prev;
        let next = self.s_playsounds[ps_idx].next;
        self.s_playsounds[prev].next = next;
        self.s_playsounds[next].prev = prev;

        let sentinel = self.s_freeplays_head;
        let old_next = self.s_playsounds[sentinel].next;
        self.s_playsounds[ps_idx].next = old_next;
        self.s_playsounds[old_next].prev = ps_idx;
        self.s_playsounds[ps_idx].prev = sentinel;
        self.s_playsounds[sentinel].next = ps_idx;
    }

    pub fn s_issue_playsound(
        &mut self,
        ps_idx: usize,
        playernum: i32,
        backend: &mut dyn AudioBackend,
        load_file: &dyn Fn(&str) -> Option<Vec<u8>>,
    ) {
        if self.s_show {
            com_dprintf(&format!("Issue {}\n", self.s_playsounds[ps_idx].begin));
        }

        let ps = &self.s_playsounds[ps_idx];
        let entnum = ps.entnum;
        let entchannel = ps.entchannel;
        let attenuation = ps.attenuation;
        let volume = ps.volume;
        let sfx_index = ps.sfx_index;
        let origin = ps.origin;
        let fixed_origin = ps.fixed_origin;

        let ch_idx = match self.s_pick_channel(entnum, entchannel, playernum) {
            Some(idx) => idx,
            None => {
                self.s_free_playsound(ps_idx);
                return;
            }
        };

        backend.stop_channel(ch_idx);

        self.channels[ch_idx].entnum = entnum;
        self.channels[ch_idx].entchannel = entchannel;
        self.channels[ch_idx].sfx_index = sfx_index;
        self.channels[ch_idx].origin = origin;
        self.channels[ch_idx].fixed_origin = fixed_origin;
        self.channels[ch_idx].master_vol = volume as i32;
        self.channels[ch_idx].dist_mult = if attenuation == ATTN_STATIC {
            attenuation * 0.001
        } else {
            attenuation * 0.0005
        };
        self.channels[ch_idx].autosound = false;
        // Enable Doppler effect for non-fixed origin sounds (moving entities)
        self.channels[ch_idx].doppler_enabled = !fixed_origin;
        self.channels[ch_idx].doppler_pitch = 1.0;

        if let Some(sfx_idx) = sfx_index {
            crate::snd_mem::s_load_sound(&mut self.known_sfx[sfx_idx], load_file);
            if let Some(ref sc) = self.known_sfx[sfx_idx].cache {
                let format = AudioFormat {
                    sample_rate: sc.speed as u32,
                    bits_per_sample: (sc.width * 8) as u16,
                    channels: 1,
                };
                let looping = sc.loopstart >= 0;
                backend.play_sound(
                    ch_idx,
                    &sc.data,
                    &format,
                    &origin,
                    (volume / 255.0) * self.s_volume,
                    attenuation,
                    looping,
                );
            }
        }

        self.s_free_playsound(ps_idx);
    }

    pub fn s_start_sound(
        &mut self,
        origin: Option<Vec3>,
        entnum: i32,
        entchannel: i32,
        sfx_index: usize,
        fvol: f32,
        attenuation: f32,
        _timeofs: f32,
        _server_time: i32,
    ) {
        if !self.sound_started {
            return;
        }

        let vol = (fvol * 255.0) as i32;

        let ps_idx = match self.s_alloc_playsound() {
            Some(idx) => idx,
            None => return,
        };

        if let Some(orig) = origin {
            self.s_playsounds[ps_idx].origin = orig;
            self.s_playsounds[ps_idx].fixed_origin = true;
        } else {
            self.s_playsounds[ps_idx].fixed_origin = false;
        }

        self.s_playsounds[ps_idx].entnum = entnum;
        self.s_playsounds[ps_idx].entchannel = entchannel;
        self.s_playsounds[ps_idx].attenuation = attenuation;
        self.s_playsounds[ps_idx].volume = vol as f32;
        self.s_playsounds[ps_idx].sfx_index = Some(sfx_index);
        self.s_playsounds[ps_idx].begin = 0;

        // Insert into pending list
        let sentinel = self.s_pendingplays_head;
        let sort = self.s_playsounds[sentinel].next;
        let sort_prev = self.s_playsounds[sort].prev;
        self.s_playsounds[ps_idx].next = sort;
        self.s_playsounds[ps_idx].prev = sort_prev;
        self.s_playsounds[sort].prev = ps_idx;
        self.s_playsounds[sort_prev].next = ps_idx;
    }

    pub fn s_start_local_sound(
        &mut self,
        sound: &str,
        playernum: i32,
        server_time: i32,
        load_file: &dyn Fn(&str) -> Option<Vec<u8>>,
    ) {
        if !self.sound_started {
            return;
        }

        let sfx_idx = match self.s_register_sound(sound, load_file) {
            Some(idx) => idx,
            None => {
                com_printf(&format!("S_StartLocalSound: can't cache {}\n", sound));
                return;
            }
        };
        self.s_start_sound(None, playernum + 1, 0, sfx_idx, 1.0, 1.0, 0.0, server_time);
    }

    pub fn s_stop_all_sounds(&mut self, backend: Option<&mut dyn AudioBackend>) {
        if !self.sound_started {
            return;
        }

        if let Some(be) = backend {
            for i in 0..MAX_CHANNELS {
                be.stop_channel(i);
            }
        }

        for ps in self.s_playsounds.iter_mut() {
            *ps = Playsound::default();
        }

        let free_sentinel = self.s_freeplays_head;
        self.s_playsounds[free_sentinel].next = free_sentinel;
        self.s_playsounds[free_sentinel].prev = free_sentinel;

        let pending_sentinel = self.s_pendingplays_head;
        self.s_playsounds[pending_sentinel].next = pending_sentinel;
        self.s_playsounds[pending_sentinel].prev = pending_sentinel;

        for i in 0..MAX_PLAYSOUNDS {
            self.s_playsounds[i].prev = free_sentinel;
            self.s_playsounds[i].next = self.s_playsounds[free_sentinel].next;
            let old_next = self.s_playsounds[free_sentinel].next;
            self.s_playsounds[old_next].prev = i;
            self.s_playsounds[free_sentinel].next = i;
        }

        for ch in self.channels.iter_mut() {
            *ch = Channel::default();
        }
    }

    pub fn s_add_loop_sounds(
        &mut self,
        paused: bool,
        active: bool,
        sound_prepped: bool,
        frame_num_entities: i32,
        frame_parse_entities: i32,
        parse_entities: &[EntitySoundInfo],
        sound_precache: &[Option<usize>],
        playernum: i32,
        backend: &mut dyn AudioBackend,
        load_file: &dyn Fn(&str) -> Option<Vec<u8>>,
        current_time: i32,
    ) {
        if paused || !active || !sound_prepped {
            return;
        }

        let max_parse = parse_entities.len();

        for i in 0..frame_num_entities as usize {
            let num = (frame_parse_entities as usize + i) & (max_parse - 1);
            let sound = parse_entities[num].sound;
            if sound == 0 {
                continue;
            }

            let sfx_idx = match sound_precache.get(sound as usize) {
                Some(Some(idx)) => *idx,
                _ => continue,
            };

            crate::snd_mem::s_load_sound(&mut self.known_sfx[sfx_idx], load_file);
            if self.known_sfx[sfx_idx].cache.is_none() {
                continue;
            }

            let origin = parse_entities[num].origin;

            // Register this looping sound for continuation during packet loss
            self.looping_sounds.register(num as i32, Some(sfx_idx), origin, current_time);

            let ch_idx = match self.s_pick_channel(0, 0, playernum) {
                Some(idx) => idx,
                None => return,
            };

            self.channels[ch_idx].autosound = true;
            self.channels[ch_idx].sfx_index = Some(sfx_idx);
            self.channels[ch_idx].origin = origin;
            self.channels[ch_idx].is_looping = true;
            self.channels[ch_idx].last_confirmed_time = current_time;

            if let Some(ref sc) = self.known_sfx[sfx_idx].cache {
                let format = AudioFormat {
                    sample_rate: sc.speed as u32,
                    bits_per_sample: (sc.width * 8) as u16,
                    channels: 1,
                };
                let looping = sc.loopstart >= 0;
                backend.play_sound(
                    ch_idx,
                    &sc.data,
                    &format,
                    &origin,
                    self.s_volume,
                    SOUND_LOOPATTENUATE,
                    looping,
                );
            }
        }

        // Clean up timed-out sounds and continue playing tracked looping sounds during packet loss
        self.looping_sounds.cleanup(current_time);
    }

    /// Continue playing looping sounds during packet loss.
    /// Call this when server updates are delayed to maintain ambient sound.
    pub fn s_continue_looping_sounds(
        &mut self,
        current_time: i32,
        playernum: i32,
        backend: &mut dyn AudioBackend,
    ) {
        if !self.looping_sounds.enabled {
            return;
        }

        // Collect data from continuing sounds first to avoid borrow conflicts
        let sounds_to_continue: Vec<(usize, [f32; 3])> = self.looping_sounds
            .get_continuing_sounds(current_time)
            .iter()
            .filter_map(|sound| {
                sound.sfx_index.map(|idx| (idx, sound.origin))
            })
            .collect();

        for (sfx_idx, origin) in sounds_to_continue {
            if sfx_idx >= self.known_sfx.len() {
                continue;
            }

            // Check if this sound is already playing
            let already_playing = self.channels.iter()
                .any(|ch| ch.sfx_index == Some(sfx_idx) && ch.autosound);

            if already_playing {
                continue;
            }

            // Re-add the looping sound
            let ch_idx = match self.s_pick_channel(0, 0, playernum) {
                Some(idx) => idx,
                None => return,
            };

            self.channels[ch_idx].autosound = true;
            self.channels[ch_idx].sfx_index = Some(sfx_idx);
            self.channels[ch_idx].origin = origin;
            self.channels[ch_idx].is_looping = true;

            if let Some(ref sc) = self.known_sfx[sfx_idx].cache {
                let format = AudioFormat {
                    sample_rate: sc.speed as u32,
                    bits_per_sample: (sc.width * 8) as u16,
                    channels: 1,
                };
                backend.play_sound(
                    ch_idx,
                    &sc.data,
                    &format,
                    &origin,
                    self.s_volume,
                    SOUND_LOOPATTENUATE,
                    true,
                );
            }
        }
    }

    /// Queue raw audio samples for streaming playback (used by cinematics).
    ///
    /// Converts incoming audio data to 16-bit signed PCM and resamples to 44100 Hz
    /// if necessary before queuing to the audio backend.
    ///
    /// # Arguments
    /// * `samples` - Number of samples (per channel)
    /// * `rate` - Source sample rate in Hz
    /// * `width` - Bytes per sample (1 = 8-bit, 2 = 16-bit)
    /// * `channels` - Number of channels (1 = mono, 2 = stereo)
    /// * `data` - Raw PCM audio data
    /// * `backend` - Audio backend for streaming playback
    pub fn s_raw_samples(
        &mut self,
        samples: i32,
        rate: i32,
        width: i32,
        channels: i32,
        data: &[u8],
        backend: &mut dyn AudioBackend,
    ) {
        if !self.sound_started || samples <= 0 {
            return;
        }

        // Target sample rate (OpenAL default)
        const TARGET_RATE: i32 = 44100;

        let samples = samples as usize;
        let channels = channels as usize;

        // Step 1: Convert to 16-bit signed samples
        let samples_16: Vec<i16> = if width == 1 {
            // 8-bit unsigned -> 16-bit signed
            // 8-bit PCM is unsigned (0-255), center is 128
            data.iter()
                .take(samples * channels)
                .map(|&s| ((s as i16 - 128) * 256) as i16)
                .collect()
        } else {
            // 16-bit signed (little endian)
            data.chunks_exact(2)
                .take(samples * channels)
                .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
                .collect()
        };

        // Step 2: Resample to target rate if needed
        let resampled: Vec<i16> = if rate != TARGET_RATE && rate > 0 {
            // Linear interpolation resampling
            let ratio = rate as f64 / TARGET_RATE as f64;
            let output_samples = ((samples as f64 / ratio) as usize).max(1);
            let mut output = Vec::with_capacity(output_samples * channels);

            for i in 0..output_samples {
                let src_pos = i as f64 * ratio;
                let src_idx = src_pos as usize;
                let frac = (src_pos - src_idx as f64) as f32;

                for ch in 0..channels {
                    let idx0 = src_idx * channels + ch;
                    let idx1 = ((src_idx + 1).min(samples - 1)) * channels + ch;

                    if idx0 < samples_16.len() && idx1 < samples_16.len() {
                        let s0 = samples_16[idx0] as f32;
                        let s1 = samples_16[idx1] as f32;
                        let interpolated = s0 + (s1 - s0) * frac;
                        output.push(interpolated as i16);
                    } else if idx0 < samples_16.len() {
                        output.push(samples_16[idx0]);
                    }
                }
            }
            output
        } else {
            samples_16
        };

        // Step 3: Queue to backend
        backend.queue_streaming_samples(&resampled, TARGET_RATE, channels as i32);
    }

    /// Two-phase sound update.
    /// Phase 1 (parallel): Compute channel state changes
    /// Phase 2 (sequential): Apply changes to backend
    pub fn s_update(
        &mut self,
        origin: Vec3,
        forward: Vec3,
        right: Vec3,
        up: Vec3,
        playernum: i32,
        _disable_screen: bool,
        backend: &mut dyn AudioBackend,
        get_entity_origin: &dyn Fn(i32) -> Vec3,
        load_file: &dyn Fn(&str) -> Option<Vec<u8>>,
        current_time: i32,
        packet_loss_frames: i32,
    ) {
        if !self.sound_started {
            return;
        }

        // Calculate listener velocity for Doppler effect
        if self.listener_update_time > 0 {
            let dt = (current_time - self.listener_update_time) as f32 / 1000.0;
            if dt > 0.001 && dt < 0.5 {
                let new_velocity = [
                    (origin[0] - self.prev_listener_origin[0]) / dt,
                    (origin[1] - self.prev_listener_origin[1]) / dt,
                    (origin[2] - self.prev_listener_origin[2]) / dt,
                ];
                for i in 0..3 {
                    self.listener_velocity[i] = self.listener_velocity[i] * 0.7 + new_velocity[i] * 0.3;
                }
            }
        }

        // Update listener state
        self.prev_listener_origin = self.listener_origin;
        self.listener_origin = origin;
        self.listener_forward = forward;
        self.listener_right = right;
        self.listener_up = up;
        self.listener_update_time = current_time;
        backend.update_listener(&origin, &forward, &up);

        // Issue all pending playsounds (sequential - modifies state)
        let sentinel = self.s_pendingplays_head;
        loop {
            let ps_idx = self.s_playsounds[sentinel].next;
            if ps_idx == sentinel {
                break;
            }
            self.s_issue_playsound(ps_idx, playernum, backend, load_file);
        }

        // During packet loss, continue looping sounds
        if packet_loss_frames > 0 {
            self.s_continue_looping_sounds(current_time, playernum, backend);
        }

        // Update active channels (parallel two-phase approach)
        self.s_update_channels(backend, get_entity_origin, current_time, packet_loss_frames);

        // Debug output (sequential)
        if self.s_show {
            let mut total = 0;
            for i in 0..MAX_CHANNELS {
                if self.channels[i].sfx_index.is_some() && backend.is_channel_playing(i) {
                    if let Some(sfx_idx) = self.channels[i].sfx_index {
                        com_printf(&format!(
                            "{:3} {}\n",
                            self.channels[i].master_vol, self.known_sfx[sfx_idx].name
                        ));
                    }
                    total += 1;
                }
            }
            com_printf(&format!("----({})----\n", total));
        }
    }

    /// Channel update using two-phase approach.
    fn s_update_channels(
        &mut self,
        backend: &mut dyn AudioBackend,
        get_entity_origin: &dyn Fn(i32) -> Vec3,
        current_time: i32,
        packet_loss_frames: i32,
    ) {
        // Pre-fetch all channel playing states (single batch call to backend)
        let playing_states = backend.get_channel_states();

        // Collect channel data needed for computation (sequential phase)
        // Also pre-fetch entity origins since the closure may not be thread-safe
        let channel_data: Vec<_> = (0..MAX_CHANNELS)
            .filter_map(|i| {
                if self.channels[i].sfx_index.is_none() {
                    return None;
                }
                // Pre-fetch entity origin if needed (sequential, before parallel)
                let entity_origin = if !self.channels[i].fixed_origin {
                    Some(get_entity_origin(self.channels[i].entnum))
                } else {
                    None
                };
                Some((
                    i,
                    self.channels[i].autosound,
                    self.channels[i].fixed_origin,
                    entity_origin,
                    self.channels[i].velocity_valid,
                ))
            })
            .collect();

        // Parallel computation of channel actions (all data is now local/Sync)
        let preserve_autosounds = packet_loss_frames > 0;
        let is_packet_loss = packet_loss_frames > 0;
        let actions: Vec<_> = channel_data
            .par_iter()
            .map(|(i, autosound, fixed_origin, entity_origin, velocity_valid)| {
                if *autosound {
                    // During packet loss, preserve autosounds to prevent ambient cutout
                    if preserve_autosounds {
                        (*i, ChannelAction::KeepPlaying)
                    } else {
                        (*i, ChannelAction::StopAndClear)
                    }
                } else if !playing_states[*i] {
                    (*i, ChannelAction::Clear)
                } else if !*fixed_origin {
                    // During packet loss with valid velocity, use extrapolation
                    if is_packet_loss && *velocity_valid {
                        (*i, ChannelAction::ExtrapolatePosition)
                    } else {
                        (*i, ChannelAction::UpdatePosition(entity_origin.unwrap()))
                    }
                } else {
                    (*i, ChannelAction::KeepPlaying)
                }
            })
            .collect();

        // Phase 2: Sequential - apply actions to backend
        for (i, action) in actions {
            match action {
                ChannelAction::StopAndClear => {
                    backend.stop_channel(i);
                    self.channels[i] = Channel::default();
                }
                ChannelAction::Clear => {
                    self.channels[i] = Channel::default();
                }
                ChannelAction::UpdatePosition(new_origin) => {
                    // Use smooth position interpolation
                    self.channels[i].set_position_target(&new_origin, current_time);
                    let smoothed_origin = self.channels[i].get_smoothed_position(current_time);
                    self.channels[i].origin = smoothed_origin;
                    backend.update_channel_position(i, &smoothed_origin);
                    self.channels[i].update_interpolation(current_time);

                    // Apply Doppler effect
                    if self.doppler_enabled && self.channels[i].doppler_enabled {
                        self.channels[i].calculate_doppler(
                            &self.listener_origin,
                            &self.listener_velocity,
                        );
                        backend.update_channel_pitch(i, self.channels[i].doppler_pitch);
                    }
                }
                ChannelAction::ExtrapolatePosition => {
                    // Use velocity extrapolation during packet loss
                    let position = self.channels[i].get_extrapolated_position(current_time);
                    self.channels[i].origin = position;
                    backend.update_channel_position(i, &position);

                    // During packet loss, gradually return Doppler to normal
                    if self.doppler_enabled && self.channels[i].doppler_enabled {
                        self.channels[i].continue_doppler_during_packet_loss();
                        backend.update_channel_pitch(i, self.channels[i].doppler_pitch);
                    }
                }
                ChannelAction::KeepPlaying => {}
            }
        }
    }

    pub fn s_play(
        &mut self,
        args: &[String],
        playernum: i32,
        server_time: i32,
        load_file: &dyn Fn(&str) -> Option<Vec<u8>>,
    ) {
        for arg in args.iter().skip(1) {
            let name = if !arg.contains('.') {
                format!("{}.wav", arg)
            } else {
                arg.clone()
            };
            if let Some(sfx_idx) = self.s_register_sound(&name, load_file) {
                self.s_start_sound(None, playernum + 1, 0, sfx_idx, 1.0, 1.0, 0.0, server_time);
            }
        }
    }

    pub fn s_sound_list(&self) {
        let mut total = 0;
        for i in 0..self.num_sfx {
            let sfx = &self.known_sfx[i];
            if sfx.registration_sequence == 0 {
                continue;
            }
            if let Some(ref sc) = sfx.cache {
                let size = sc.length * sc.width * (sc.stereo + 1);
                total += size;
                if sc.loopstart >= 0 {
                    com_printf("L");
                } else {
                    com_printf(" ");
                }
                com_printf(&format!("({:2}b) {:6} : {}\n", sc.width * 8, size, sfx.name));
            } else if sfx.name.starts_with('*') {
                com_printf(&format!("  placeholder : {}\n", sfx.name));
            } else {
                com_printf(&format!("  not loaded  : {}\n", sfx.name));
            }
        }
        com_printf(&format!("Total resident: {}\n", total));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== Channel position smoothing ==========

    #[test]
    fn channel_set_position_target_first_update_snaps() {
        let mut ch = Channel::default();
        let pos = [100.0, 200.0, 300.0];
        ch.set_position_target(&pos, 1000);

        assert_eq!(ch.origin, pos);
        assert_eq!(ch.prev_origin, pos);
        assert_eq!(ch.target_origin, pos);
        assert!(ch.interpolating);
        assert!(!ch.velocity_valid);
        assert_eq!(ch.position_update_time, 1000);
    }

    #[test]
    fn channel_set_position_target_second_update_starts_interpolation() {
        let mut ch = Channel::default();
        let pos1 = [0.0, 0.0, 0.0];
        let pos2 = [100.0, 0.0, 0.0];
        ch.set_position_target(&pos1, 1000);
        ch.set_position_target(&pos2, 1100);

        // prev_origin should be set to the old origin (the snap position)
        assert_eq!(ch.prev_origin, pos1);
        assert_eq!(ch.target_origin, pos2);
        assert!(ch.velocity_valid);
        assert_eq!(ch.position_update_time, 1100);
        // Velocity should be ~1000 units/sec (100 units in 0.1 sec)
        assert!((ch.velocity[0] - 1000.0).abs() < 1.0);
    }

    #[test]
    fn channel_set_position_target_velocity_smoothing() {
        let mut ch = Channel::default();
        ch.set_position_target(&[0.0, 0.0, 0.0], 1000);
        ch.set_position_target(&[100.0, 0.0, 0.0], 1100); // 1000 u/s
        let v1 = ch.velocity[0];

        ch.set_position_target(&[300.0, 0.0, 0.0], 1200); // raw 2000 u/s
        let v2 = ch.velocity[0];

        // With EMA (0.7 old + 0.3 new), v2 should be between v1 and 2000
        assert!(v2 > v1);
        assert!(v2 < 2000.0);
    }

    #[test]
    fn channel_set_position_target_ignores_tiny_dt() {
        let mut ch = Channel::default();
        ch.set_position_target(&[0.0, 0.0, 0.0], 1000);
        ch.set_position_target(&[100.0, 0.0, 0.0], 1000); // dt = 0, should skip velocity calc
        assert!(!ch.velocity_valid);
    }

    #[test]
    fn channel_set_position_target_ignores_large_dt() {
        let mut ch = Channel::default();
        ch.set_position_target(&[0.0, 0.0, 0.0], 1000);
        ch.set_position_target(&[100.0, 0.0, 0.0], 2001); // dt > 1.0 sec
        assert!(!ch.velocity_valid);
    }

    // ========== Smoothed position ==========

    #[test]
    fn channel_get_smoothed_position_not_interpolating_returns_origin() {
        let ch = Channel {
            origin: [10.0, 20.0, 30.0],
            interpolating: false,
            ..Channel::default()
        };
        let pos = ch.get_smoothed_position(5000);
        assert_eq!(pos, [10.0, 20.0, 30.0]);
    }

    #[test]
    fn channel_get_smoothed_position_after_lerp_returns_target() {
        let ch = Channel {
            prev_origin: [0.0, 0.0, 0.0],
            target_origin: [100.0, 200.0, 300.0],
            interpolating: true,
            position_update_time: 1000,
            ..Channel::default()
        };
        // At or after POSITION_LERP_MS (50), returns target
        let pos = ch.get_smoothed_position(1050);
        assert_eq!(pos, [100.0, 200.0, 300.0]);
    }

    #[test]
    fn channel_get_smoothed_position_midway_interpolates() {
        let ch = Channel {
            prev_origin: [0.0, 0.0, 0.0],
            target_origin: [100.0, 0.0, 0.0],
            interpolating: true,
            position_update_time: 1000,
            ..Channel::default()
        };
        // At 25ms (half of 50ms POSITION_LERP_MS)
        let pos = ch.get_smoothed_position(1025);
        assert!((pos[0] - 50.0).abs() < 0.01);
        assert_eq!(pos[1], 0.0);
        assert_eq!(pos[2], 0.0);
    }

    // ========== Extrapolated position ==========

    #[test]
    fn channel_get_extrapolated_position_no_velocity_returns_origin() {
        let ch = Channel {
            origin: [50.0, 60.0, 70.0],
            velocity_valid: false,
            ..Channel::default()
        };
        let pos = ch.get_extrapolated_position(5000);
        assert_eq!(pos, [50.0, 60.0, 70.0]);
    }

    #[test]
    fn channel_get_extrapolated_position_with_velocity() {
        let ch = Channel {
            origin: [0.0, 0.0, 0.0],
            target_origin: [100.0, 0.0, 0.0],
            velocity: [1000.0, 0.0, 0.0],
            velocity_valid: true,
            position_update_time: 1000,
            ..Channel::default()
        };
        // At 100ms after last update
        let pos = ch.get_extrapolated_position(1100);
        // target + velocity * dt = 100 + 1000 * 0.1 = 200
        assert!((pos[0] - 200.0).abs() < 0.1);
    }

    #[test]
    fn channel_get_extrapolated_position_exceeds_max_returns_origin() {
        let ch = Channel {
            origin: [50.0, 0.0, 0.0],
            target_origin: [100.0, 0.0, 0.0],
            velocity: [1000.0, 0.0, 0.0],
            velocity_valid: true,
            position_update_time: 1000,
            ..Channel::default()
        };
        // MAX_EXTRAPOLATION_MS is 300, so 400ms should return origin
        let pos = ch.get_extrapolated_position(1400);
        assert_eq!(pos, [50.0, 0.0, 0.0]);
    }

    #[test]
    fn channel_get_extrapolated_position_negative_dt_returns_origin() {
        let ch = Channel {
            origin: [50.0, 0.0, 0.0],
            velocity: [1000.0, 0.0, 0.0],
            velocity_valid: true,
            position_update_time: 2000,
            ..Channel::default()
        };
        let pos = ch.get_extrapolated_position(1000); // time before update
        assert_eq!(pos, [50.0, 0.0, 0.0]);
    }

    // ========== Doppler effect ==========

    #[test]
    fn channel_calculate_doppler_disabled_returns_one() {
        let mut ch = Channel {
            doppler_enabled: false,
            velocity_valid: true,
            ..Channel::default()
        };
        let pitch = ch.calculate_doppler(&[0.0; 3], &[0.0; 3]);
        assert_eq!(pitch, 1.0);
        assert_eq!(ch.doppler_pitch, 1.0);
    }

    #[test]
    fn channel_calculate_doppler_no_velocity_returns_one() {
        let mut ch = Channel {
            doppler_enabled: true,
            velocity_valid: false,
            ..Channel::default()
        };
        let pitch = ch.calculate_doppler(&[0.0; 3], &[0.0; 3]);
        assert_eq!(pitch, 1.0);
    }

    #[test]
    fn channel_calculate_doppler_very_close_returns_one() {
        let mut ch = Channel {
            doppler_enabled: true,
            velocity_valid: true,
            origin: [0.0, 0.0, 0.0],
            velocity: [100.0, 0.0, 0.0],
            ..Channel::default()
        };
        // Listener at origin, distance < 1.0
        let pitch = ch.calculate_doppler(&[0.5, 0.0, 0.0], &[0.0; 3]);
        assert_eq!(pitch, 1.0);
    }

    #[test]
    fn channel_calculate_doppler_listener_moving_away_raises_pitch() {
        let mut ch = Channel {
            doppler_enabled: true,
            velocity_valid: true,
            origin: [0.0, 0.0, 0.0],
            velocity: [0.0, 0.0, 0.0], // source stationary
            doppler_pitch: 1.0,
            ..Channel::default()
        };
        // Listener at [1000,0,0], moving away from source [+500,0,0].
        // dir = normalized(listener - source) = [1,0,0].
        // listener_towards = dot([500,0,0], [1,0,0]) = 500 (positive).
        // relative_velocity = 500 > 0 -> pitch_shift > 0 -> pitch increases.
        let pitch = ch.calculate_doppler(&[1000.0, 0.0, 0.0], &[500.0, 0.0, 0.0]);
        assert!(pitch > 1.0, "Listener moving along dir should raise pitch, got {}", pitch);
    }

    #[test]
    fn channel_calculate_doppler_listener_moving_towards_lowers_pitch() {
        let mut ch = Channel {
            doppler_enabled: true,
            velocity_valid: true,
            origin: [0.0, 0.0, 0.0],
            velocity: [0.0, 0.0, 0.0], // source stationary
            doppler_pitch: 1.0,
            ..Channel::default()
        };
        // Listener at [1000,0,0], moving towards source [-500,0,0].
        // dir = [1,0,0], listener_towards = -500, relative_velocity = -500 -> pitch decreases.
        let pitch = ch.calculate_doppler(&[1000.0, 0.0, 0.0], &[-500.0, 0.0, 0.0]);
        assert!(pitch < 1.0, "Listener moving against dir should lower pitch, got {}", pitch);
    }

    #[test]
    fn channel_calculate_doppler_clamped_to_max_shift() {
        let mut ch = Channel {
            doppler_enabled: true,
            velocity_valid: true,
            origin: [0.0, 0.0, 0.0],
            velocity: [0.0, 0.0, 0.0],
            doppler_pitch: 1.0,
            ..Channel::default()
        };
        // Extreme velocity towards source - should be clamped
        let pitch = ch.calculate_doppler(&[1000.0, 0.0, 0.0], &[-50000.0, 0.0, 0.0]);
        // With smoothing (0.8 * 1.0 + 0.2 * target), pitch won't exceed 1.0 + MAX_DOPPLER_SHIFT immediately
        // But the raw pitch_shift should be clamped to +/- 0.3
        assert!(pitch <= 1.0 + Channel::MAX_DOPPLER_SHIFT + 0.01);
    }

    #[test]
    fn channel_continue_doppler_during_packet_loss() {
        let mut ch = Channel {
            doppler_enabled: true,
            doppler_pitch: 1.2, // shifted up
            ..Channel::default()
        };
        ch.continue_doppler_during_packet_loss();
        // Should move towards 1.0: 1.2 * 0.95 + 1.0 * 0.05 = 1.14 + 0.05 = 1.19
        assert!((ch.doppler_pitch - 1.19).abs() < 0.001);
        // Continue more - with 0.95 decay, convergence is slow
        for _ in 0..100 {
            ch.continue_doppler_during_packet_loss();
        }
        // After 101 total iterations from 1.2: should be very close to 1.0
        // 0.95^101 * 0.2 ~ 0.000012, so within 0.001
        assert!((ch.doppler_pitch - 1.0).abs() < 0.01);
    }

    #[test]
    fn channel_continue_doppler_disabled_no_change() {
        let mut ch = Channel {
            doppler_enabled: false,
            doppler_pitch: 1.5,
            ..Channel::default()
        };
        ch.continue_doppler_during_packet_loss();
        assert_eq!(ch.doppler_pitch, 1.5); // unchanged
    }

    // ========== Channel velocity ==========

    #[test]
    fn channel_clear_velocity() {
        let mut ch = Channel {
            velocity: [100.0, 200.0, 300.0],
            velocity_valid: true,
            velocity_update_time: 5000,
            ..Channel::default()
        };
        ch.clear_velocity();
        assert_eq!(ch.velocity, [0.0; 3]);
        assert!(!ch.velocity_valid);
        assert_eq!(ch.velocity_update_time, 0);
    }

    // ========== Update interpolation ==========

    #[test]
    fn channel_update_interpolation_finalizes() {
        let mut ch = Channel {
            interpolating: true,
            position_update_time: 1000,
            origin: [50.0, 0.0, 0.0],
            target_origin: [100.0, 0.0, 0.0],
            ..Channel::default()
        };
        ch.update_interpolation(1050); // At or past POSITION_LERP_MS
        assert_eq!(ch.origin, [100.0, 0.0, 0.0]);
    }

    #[test]
    fn channel_update_interpolation_no_change_before_completion() {
        let mut ch = Channel {
            interpolating: true,
            position_update_time: 1000,
            origin: [50.0, 0.0, 0.0],
            target_origin: [100.0, 0.0, 0.0],
            ..Channel::default()
        };
        ch.update_interpolation(1020); // Before POSITION_LERP_MS
        assert_eq!(ch.origin, [50.0, 0.0, 0.0]); // unchanged
    }

    // ========== RoomAnalysis ==========

    #[test]
    fn room_analysis_from_traces_computes_volume() {
        let distances = [100.0, 100.0, 200.0, 200.0, 50.0, 50.0];
        let ra = RoomAnalysis::from_traces(false, distances, SurfaceMaterial::Stone, false);

        // room_volume = (100+100) * (200+200) * (50+50) = 200 * 400 * 100 = 8_000_000
        assert_eq!(ra.room_volume, 8_000_000.0);
        assert_eq!(ra.room_height, 100.0);
        assert!(!ra.underwater);
        assert!(!ra.outdoors);
        assert_eq!(ra.surface_material, SurfaceMaterial::Stone);
    }

    #[test]
    fn room_analysis_from_traces_counts_close_walls() {
        let distances = [50.0, 50.0, 600.0, 600.0, 30.0, 30.0];
        let ra = RoomAnalysis::from_traces(false, distances, SurfaceMaterial::Concrete, false);
        // Walls < 512: 50, 50, 30, 30 = 4
        assert_eq!(ra.walls_detected, 4);
    }

    #[test]
    fn room_analysis_from_traces_all_close_walls() {
        let distances = [100.0; 6];
        let ra = RoomAnalysis::from_traces(false, distances, SurfaceMaterial::Metal, false);
        assert_eq!(ra.walls_detected, 6);
    }

    #[test]
    fn room_analysis_from_traces_no_close_walls() {
        let distances = [1000.0; 6];
        let ra = RoomAnalysis::from_traces(true, distances, SurfaceMaterial::Water, true);
        assert_eq!(ra.walls_detected, 0);
        assert!(ra.underwater);
        assert!(ra.outdoors);
    }

    // ========== LoopingSoundState ==========

    #[test]
    fn looping_sound_state_register_and_retrieve() {
        let mut lss = LoopingSoundState::default();
        lss.register(5, Some(10), [100.0, 200.0, 300.0], 1000);

        let continuing = lss.get_continuing_sounds(1100);
        assert_eq!(continuing.len(), 1);
        assert_eq!(continuing[0].entnum, 5);
        assert_eq!(continuing[0].sfx_index, Some(10));
    }

    #[test]
    fn looping_sound_state_no_continuing_when_disabled() {
        let mut lss = LoopingSoundState::default();
        lss.enabled = false;
        lss.register(5, Some(10), [0.0; 3], 1000);

        let continuing = lss.get_continuing_sounds(1100);
        assert!(continuing.is_empty());
    }

    #[test]
    fn looping_sound_state_timeout() {
        let mut lss = LoopingSoundState::default();
        lss.register(5, Some(10), [0.0; 3], 1000);

        // After 500ms (continuation_timeout), should not continue
        let continuing = lss.get_continuing_sounds(1501);
        assert!(continuing.is_empty());
    }

    #[test]
    fn looping_sound_state_cleanup_removes_timed_out() {
        let mut lss = LoopingSoundState::default();
        lss.register(5, Some(10), [0.0; 3], 1000);
        lss.register(6, Some(11), [0.0; 3], 1400);

        lss.cleanup(1600); // 600ms after first, 200ms after second
        // First should be deactivated (>500ms), second still active
        assert!(!lss.sounds[0].active);
        assert!(lss.sounds[1].active);
    }

    #[test]
    fn looping_sound_state_clear() {
        let mut lss = LoopingSoundState::default();
        lss.register(5, Some(10), [0.0; 3], 1000);
        lss.clear();

        assert!(!lss.sounds[0].active);
        assert_eq!(lss.sounds[0].entnum, 0);
    }

    #[test]
    fn looping_sound_state_register_reuses_existing_slot() {
        let mut lss = LoopingSoundState::default();
        lss.register(5, Some(10), [100.0, 0.0, 0.0], 1000);
        lss.register(5, Some(10), [200.0, 0.0, 0.0], 1100);

        // Should update existing slot, not create new
        let count = lss.sounds.iter().filter(|s| s.active).count();
        assert_eq!(count, 1);
        assert_eq!(lss.sounds[0].origin[0], 200.0);
        assert_eq!(lss.sounds[0].last_confirmed_time, 1100);
    }

    #[test]
    fn looping_sound_state_register_different_sounds() {
        let mut lss = LoopingSoundState::default();
        lss.register(5, Some(10), [0.0; 3], 1000);
        lss.register(6, Some(11), [0.0; 3], 1000);

        let count = lss.sounds.iter().filter(|s| s.active).count();
        assert_eq!(count, 2);
    }

    #[test]
    fn looping_sound_state_get_continuing_excludes_just_confirmed() {
        let mut lss = LoopingSoundState::default();
        lss.register(5, Some(10), [0.0; 3], 1000);

        // At the same time as confirmation, current_time - last_confirmed_time = 0
        // The filter requires > 0, so this should be empty
        let continuing = lss.get_continuing_sounds(1000);
        assert!(continuing.is_empty());
    }

    // ========== SoundState: s_find_name ==========

    #[test]
    fn sound_state_s_find_name_create() {
        let mut ss = SoundState::new();
        let idx = ss.s_find_name("weapons/blaster.wav", true);
        assert!(idx.is_some());
        assert_eq!(ss.known_sfx[idx.unwrap()].name, "weapons/blaster.wav");
        assert_eq!(ss.num_sfx, 1);
    }

    #[test]
    fn sound_state_s_find_name_find_existing() {
        let mut ss = SoundState::new();
        let idx1 = ss.s_find_name("weapons/blaster.wav", true).unwrap();
        let idx2 = ss.s_find_name("weapons/blaster.wav", true).unwrap();
        assert_eq!(idx1, idx2);
        assert_eq!(ss.num_sfx, 1);
    }

    #[test]
    fn sound_state_s_find_name_no_create() {
        let mut ss = SoundState::new();
        let idx = ss.s_find_name("weapons/blaster.wav", false);
        assert!(idx.is_none());
    }

    #[test]
    #[should_panic(expected = "S_FindName: empty name")]
    fn sound_state_s_find_name_empty_panics() {
        let mut ss = SoundState::new();
        ss.s_find_name("", true);
    }

    #[test]
    fn sound_state_s_find_name_multiple() {
        let mut ss = SoundState::new();
        let idx1 = ss.s_find_name("a.wav", true).unwrap();
        let idx2 = ss.s_find_name("b.wav", true).unwrap();
        assert_ne!(idx1, idx2);
        assert_eq!(ss.num_sfx, 2);
    }

    // ========== SoundState: s_alias_name ==========

    #[test]
    fn sound_state_s_alias_name() {
        let mut ss = SoundState::new();
        let idx = ss.s_alias_name("alias.wav", "real.wav").unwrap();
        assert_eq!(ss.known_sfx[idx].name, "alias.wav");
        assert_eq!(ss.known_sfx[idx].truename.as_deref(), Some("real.wav"));
    }

    // ========== SoundState: s_pick_channel ==========

    #[test]
    fn sound_state_s_pick_channel_empty() {
        let mut ss = SoundState::new();
        let idx = ss.s_pick_channel(1, 1, 0);
        assert!(idx.is_some());
    }

    #[test]
    fn sound_state_s_pick_channel_replaces_same_entity_channel() {
        let mut ss = SoundState::new();
        ss.channels[0].entnum = 5;
        ss.channels[0].entchannel = 2;
        ss.channels[0].sfx_index = Some(1);

        let idx = ss.s_pick_channel(5, 2, 0);
        assert_eq!(idx, Some(0));
    }

    #[test]
    fn sound_state_s_pick_channel_finds_empty_slot() {
        let mut ss = SoundState::new();
        ss.channels[0].sfx_index = Some(1);
        ss.channels[0].entnum = 1;
        ss.channels[1].sfx_index = None; // empty

        let idx = ss.s_pick_channel(5, 3, 0);
        assert_eq!(idx, Some(1));
    }

    #[test]
    #[should_panic(expected = "entchannel<0")]
    fn sound_state_s_pick_channel_negative_entchannel_panics() {
        let mut ss = SoundState::new();
        ss.s_pick_channel(1, -1, 0);
    }

    // ========== SoundState: s_begin_registration ==========

    #[test]
    fn sound_state_begin_registration_increments_sequence() {
        let mut ss = SoundState::new();
        assert_eq!(ss.s_registration_sequence, 0);
        ss.s_begin_registration();
        assert_eq!(ss.s_registration_sequence, 1);
        assert!(ss.s_registering);
        ss.s_begin_registration();
        assert_eq!(ss.s_registration_sequence, 2);
    }

    // ========== SoundState: s_stop_all_sounds ==========

    #[test]
    fn sound_state_stop_all_sounds_clears_channels() {
        let mut ss = SoundState::new();
        ss.sound_started = true;
        ss.channels[0].sfx_index = Some(1);
        ss.channels[0].entnum = 5;
        ss.channels[5].sfx_index = Some(2);

        ss.s_stop_all_sounds(None);

        assert!(ss.channels[0].sfx_index.is_none());
        assert_eq!(ss.channels[0].entnum, 0);
        assert!(ss.channels[5].sfx_index.is_none());
    }

    #[test]
    fn sound_state_stop_all_sounds_not_started_noop() {
        let mut ss = SoundState::new();
        ss.sound_started = false;
        ss.channels[0].sfx_index = Some(1);
        ss.s_stop_all_sounds(None);
        // Should not clear because sound_started is false
        assert!(ss.channels[0].sfx_index.is_some());
    }

    // ========== Playsound allocation ==========

    #[test]
    fn sound_state_playsound_alloc_and_free() {
        let mut ss = SoundState::new();
        ss.sound_started = true;
        ss.s_stop_all_sounds(None); // Initialize linked lists

        let ps = ss.s_alloc_playsound();
        assert!(ps.is_some());
        let ps_idx = ps.unwrap();

        // Free it back
        ss.s_free_playsound(ps_idx);

        // Should be able to allocate again
        let ps2 = ss.s_alloc_playsound();
        assert!(ps2.is_some());
    }

    // ========== SurfaceMaterial defaults ==========

    #[test]
    fn surface_material_default_is_stone() {
        let mat = SurfaceMaterial::default();
        assert_eq!(mat, SurfaceMaterial::Stone);
    }

    // ========== Constants ==========

    #[test]
    fn sound_constants() {
        assert_eq!(SOUND_FULLVOLUME, 80.0);
        assert_eq!(SOUND_LOOPATTENUATE, 0.003);
        assert_eq!(MAX_CHANNELS, 32);
        assert_eq!(MAX_PLAYSOUNDS, 128);
        assert_eq!(MAX_LOOPING_SOUNDS, 64);
    }

    // ========== SoundState default ==========

    #[test]
    fn sound_state_default_values() {
        let ss = SoundState::new();
        assert!(!ss.snd_initialized);
        assert!(!ss.sound_started);
        assert_eq!(ss.s_volume, 0.5);
        assert_eq!(ss.s_khz, 22);
        assert!(!ss.s_registering);
        assert!(ss.doppler_enabled);
        assert_eq!(ss.num_sfx, 0);
        assert_eq!(ss.s_playsounds.len(), MAX_PLAYSOUNDS + 2);
    }

    // ========== WavInfo default ==========

    #[test]
    fn wavinfo_default() {
        let wi = WavInfo::default();
        assert_eq!(wi.rate, 0);
        assert_eq!(wi.width, 0);
        assert_eq!(wi.channels, 0);
        assert_eq!(wi.loopstart, 0);
        assert_eq!(wi.samples, 0);
        assert_eq!(wi.dataofs, 0);
    }

    // ========== Channel constants ==========

    #[test]
    fn channel_constants() {
        assert_eq!(Channel::POSITION_LERP_MS, 50);
        assert_eq!(Channel::MAX_EXTRAPOLATION_MS, 300);
        assert_eq!(Channel::SPEED_OF_SOUND, 5000.0);
        assert_eq!(Channel::MAX_DOPPLER_SHIFT, 0.3);
    }
}

pub fn snd_load_file(filename: &str) -> Option<Vec<u8>> {
    myq2_common::files::fs_load_file(filename)
}

/// Minimal entity sound info needed for loop sounds
#[derive(Clone, Default)]
pub struct EntitySoundInfo {
    pub origin: Vec3,
    pub sound: i32,
}
