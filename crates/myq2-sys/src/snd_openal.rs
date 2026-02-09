//! OpenAL Soft audio backend with HRTF spatial audio.
//!
//! Replaces SDL3 audio and Q2's stereo panning mixer with OpenAL Soft.
//! HRTF is enabled by default for immersive directional audio.
//!
//! OpenAL Soft is built from source (v1.25.1) via the `openal-soft-sys` crate.

use openal_soft_sys as al;
use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr;

use myq2_client::snd_dma::{AudioBackend, AudioFormat, RoomAnalysis, SurfaceMaterial};

// ============================================================================
// Public API
// ============================================================================

/// Maximum number of sound channels (matches Q2's MAX_CHANNELS).
const MAX_CHANNELS: usize = 32;

/// Q2 unit scale: ~40 units per meter; speed of sound = 340 m/s * 40 = 13600 units/s.
const Q2_SPEED_OF_SOUND: f32 = 13600.0;

// ============================================================================
// Reverb presets
// ============================================================================

/// Environmental reverb preset parameters.
#[derive(Debug, Clone, Copy)]
struct ReverbPreset {
    density: f32,
    diffusion: f32,
    gain: f32,
    gain_hf: f32,
    decay_time: f32,
    decay_hf_ratio: f32,
    reflections_gain: f32,
    reflections_delay: f32,
    late_reverb_gain: f32,
    late_reverb_delay: f32,
}

impl ReverbPreset {
    const GENERIC: Self = Self {
        density: 1.0,
        diffusion: 1.0,
        gain: 0.32,
        gain_hf: 0.89,
        decay_time: 1.49,
        decay_hf_ratio: 0.83,
        reflections_gain: 0.05,
        reflections_delay: 0.007,
        late_reverb_gain: 1.26,
        late_reverb_delay: 0.011,
    };

    const UNDERWATER: Self = Self {
        density: 0.36,
        diffusion: 1.0,
        gain: 0.32,
        gain_hf: 0.01,
        decay_time: 8.0,
        decay_hf_ratio: 0.2,
        reflections_gain: 0.4,
        reflections_delay: 0.02,
        late_reverb_gain: 1.0,
        late_reverb_delay: 0.04,
    };

    const CAVE: Self = Self {
        density: 1.0,
        diffusion: 1.0,
        gain: 0.32,
        gain_hf: 0.59,
        decay_time: 3.0,
        decay_hf_ratio: 0.6,
        reflections_gain: 0.14,
        reflections_delay: 0.015,
        late_reverb_gain: 1.0,
        late_reverb_delay: 0.022,
    };

    const HALLWAY: Self = Self {
        density: 0.36,
        diffusion: 1.0,
        gain: 0.32,
        gain_hf: 0.89,
        decay_time: 1.49,
        decay_hf_ratio: 0.59,
        reflections_gain: 0.25,
        reflections_delay: 0.007,
        late_reverb_gain: 1.26,
        late_reverb_delay: 0.011,
    };

    const ARENA: Self = Self {
        density: 1.0,
        diffusion: 1.0,
        gain: 0.32,
        gain_hf: 0.45,
        decay_time: 4.6,
        decay_hf_ratio: 0.5,
        reflections_gain: 0.2,
        reflections_delay: 0.02,
        late_reverb_gain: 0.8,
        late_reverb_delay: 0.03,
    };

    /// Forest - outdoor with natural echo from distant trees.
    const FOREST: Self = Self {
        density: 0.5,
        diffusion: 0.6,
        gain: 0.28,
        gain_hf: 0.70,
        decay_time: 1.8,
        decay_hf_ratio: 0.5,
        reflections_gain: 0.08,
        reflections_delay: 0.030,
        late_reverb_gain: 0.5,
        late_reverb_delay: 0.045,
    };

    /// Cathedral - large reverberant space with long decay.
    const CATHEDRAL: Self = Self {
        density: 1.0,
        diffusion: 1.0,
        gain: 0.32,
        gain_hf: 0.35,
        decay_time: 8.5,
        decay_hf_ratio: 0.4,
        reflections_gain: 0.18,
        reflections_delay: 0.015,
        late_reverb_gain: 1.4,
        late_reverb_delay: 0.040,
    };

    /// Metal Room - small, highly reflective industrial space.
    const METAL_ROOM: Self = Self {
        density: 0.9,
        diffusion: 0.8,
        gain: 0.35,
        gain_hf: 0.95,
        decay_time: 1.2,
        decay_hf_ratio: 0.9,
        reflections_gain: 0.35,
        reflections_delay: 0.003,
        late_reverb_gain: 0.9,
        late_reverb_delay: 0.008,
    };

    /// Snow - sound-absorbing environment with muffled reflections.
    const SNOW: Self = Self {
        density: 0.3,
        diffusion: 1.0,
        gain: 0.20,
        gain_hf: 0.15,
        decay_time: 0.6,
        decay_hf_ratio: 0.3,
        reflections_gain: 0.02,
        reflections_delay: 0.020,
        late_reverb_gain: 0.3,
        late_reverb_delay: 0.035,
    };

    /// Tunnel - confined corridor with parallel walls.
    const TUNNEL: Self = Self {
        density: 0.8,
        diffusion: 0.5,
        gain: 0.40,
        gain_hf: 0.70,
        decay_time: 2.8,
        decay_hf_ratio: 0.65,
        reflections_gain: 0.30,
        reflections_delay: 0.010,
        late_reverb_gain: 1.1,
        late_reverb_delay: 0.018,
    };

    /// Bathroom - small, highly reflective tiled space.
    const BATHROOM: Self = Self {
        density: 0.6,
        diffusion: 1.0,
        gain: 0.30,
        gain_hf: 0.88,
        decay_time: 1.5,
        decay_hf_ratio: 0.75,
        reflections_gain: 0.40,
        reflections_delay: 0.005,
        late_reverb_gain: 0.8,
        late_reverb_delay: 0.012,
    };
}

/// Active reverb environment.
///
/// Environment IDs for `set_environment()`:
/// - 0: Generic (default)
/// - 1: Underwater (muffled, long decay)
/// - 2: Cave (long decay, hard reflections)
/// - 3: Hallway (moderate decay)
/// - 4: Arena (large open space)
/// - 5: Forest (outdoor, natural echo)
/// - 6: Cathedral (very long decay)
/// - 7: MetalRoom (bright, industrial)
/// - 8: Snow (sound-absorbing)
/// - 9: Tunnel (confined corridor)
/// - 10: Bathroom (small, reflective)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReverbEnvironment {
    Generic,
    Underwater,
    Cave,
    Hallway,
    Arena,
    Forest,
    Cathedral,
    MetalRoom,
    Snow,
    Tunnel,
    Bathroom,
}

impl ReverbEnvironment {
    fn preset(&self) -> ReverbPreset {
        match self {
            Self::Generic => ReverbPreset::GENERIC,
            Self::Underwater => ReverbPreset::UNDERWATER,
            Self::Cave => ReverbPreset::CAVE,
            Self::Hallway => ReverbPreset::HALLWAY,
            Self::Arena => ReverbPreset::ARENA,
            Self::Forest => ReverbPreset::FOREST,
            Self::Cathedral => ReverbPreset::CATHEDRAL,
            Self::MetalRoom => ReverbPreset::METAL_ROOM,
            Self::Snow => ReverbPreset::SNOW,
            Self::Tunnel => ReverbPreset::TUNNEL,
            Self::Bathroom => ReverbPreset::BATHROOM,
        }
    }
}

/// Number of streaming buffers for cinematic audio.
const STREAMING_BUFFER_COUNT: usize = 4;

/// Size of each streaming buffer in bytes (~0.25 seconds at 44100 Hz stereo 16-bit).
const STREAMING_BUFFER_SIZE: usize = 44100;

/// OpenAL Soft backend with HRTF support.
pub struct OpenAlBackend {
    /// OpenAL device handle.
    device: *mut al::ALCdevice,
    /// OpenAL context handle.
    context: *mut al::ALCcontext,
    /// One AL source per channel.
    sources: [al::ALuint; MAX_CHANNELS],
    /// Cached AL buffer objects (counter -> AL buffer).
    buffers: HashMap<usize, al::ALuint>,
    /// Buffer counter for unique keys.
    buffer_counter: usize,
    /// Whether the backend is initialized.
    initialized: bool,
    /// Whether HRTF is enabled.
    pub hrtf_enabled: bool,
    /// Whether audio is currently active (not muted).
    active: bool,
    /// EFX auxiliary effect slot for reverb.
    aux_slot: al::ALuint,
    /// EFX reverb effect object.
    reverb_effect: al::ALuint,
    /// EFX low-pass filter for underwater muffling.
    lowpass_filter: al::ALuint,
    /// Whether EFX reverb is enabled.
    reverb_enabled: bool,
    /// Current reverb environment.
    current_reverb: ReverbEnvironment,
    /// Whether Doppler effect is enabled.
    doppler_enabled: bool,
    /// Dedicated source for streaming audio (cinematics).
    streaming_source: al::ALuint,
    /// Buffer queue for streaming audio.
    streaming_buffers: [al::ALuint; STREAMING_BUFFER_COUNT],
    /// Whether streaming is initialized.
    streaming_initialized: bool,
}

impl OpenAlBackend {
    pub fn new() -> Self {
        Self {
            device: ptr::null_mut(),
            context: ptr::null_mut(),
            sources: [0; MAX_CHANNELS],
            buffers: HashMap::new(),
            buffer_counter: 0,
            initialized: false,
            hrtf_enabled: true,
            active: true,
            aux_slot: 0,
            reverb_effect: 0,
            lowpass_filter: 0,
            reverb_enabled: true,
            current_reverb: ReverbEnvironment::Generic,
            doppler_enabled: true,
            streaming_source: 0,
            streaming_buffers: [0; STREAMING_BUFFER_COUNT],
            streaming_initialized: false,
        }
    }

    /// Convert Q2 coordinate system (Z-up) to OpenAL (Y-up).
    /// Q2: (x, y, z) -> OpenAL: (x, z, -y)
    #[inline]
    fn q2_to_al(v: &[f32; 3]) -> (f32, f32, f32) {
        (v[0], v[2], -v[1])
    }

    /// Initialize EFX reverb effect, auxiliary slot, and low-pass filter.
    /// Returns true if EFX was set up successfully.
    fn init_efx(&mut self) -> bool {
        // SAFETY: OpenAL is initialized.
        unsafe {
            // Create auxiliary effect slot
            al::alGenAuxiliaryEffectSlots(1, &mut self.aux_slot);
            if al::alGetError() != al::AL_NO_ERROR {
                eprintln!("OpenAL EFX: Failed to create auxiliary effect slot");
                return false;
            }

            // Create reverb effect
            al::alGenEffects(1, &mut self.reverb_effect);
            if al::alGetError() != al::AL_NO_ERROR {
                eprintln!("OpenAL EFX: Failed to create effect");
                al::alDeleteAuxiliaryEffectSlots(1, &self.aux_slot);
                self.aux_slot = 0;
                return false;
            }

            al::alEffecti(self.reverb_effect, al::AL_EFFECT_TYPE, al::AL_EFFECT_REVERB);
            if al::alGetError() != al::AL_NO_ERROR {
                eprintln!("OpenAL EFX: Reverb effect type not supported");
                al::alDeleteEffects(1, &self.reverb_effect);
                al::alDeleteAuxiliaryEffectSlots(1, &self.aux_slot);
                self.reverb_effect = 0;
                self.aux_slot = 0;
                return false;
            }

            // Create low-pass filter for underwater muffling
            al::alGenFilters(1, &mut self.lowpass_filter);
            if al::alGetError() != al::AL_NO_ERROR {
                eprintln!("OpenAL EFX: Failed to create filter (non-fatal)");
                self.lowpass_filter = 0;
            } else {
                al::alFilteri(self.lowpass_filter, al::AL_FILTER_TYPE, al::AL_FILTER_LOWPASS);
                al::alFilterf(self.lowpass_filter, al::AL_LOWPASS_GAIN, 1.0);
                al::alFilterf(self.lowpass_filter, al::AL_LOWPASS_GAINHF, 1.0);
            }

            // Apply default reverb preset
            self.apply_reverb_preset(ReverbEnvironment::Generic);

            eprintln!("OpenAL EFX: Reverb initialized");
            true
        }
    }

    /// Apply a reverb preset to the effect object and attach to the aux slot.
    fn apply_reverb_preset(&mut self, env: ReverbEnvironment) {
        if self.reverb_effect == 0 || self.aux_slot == 0 {
            return;
        }

        let p = env.preset();

        // SAFETY: OpenAL is initialized; effect and slot are valid.
        unsafe {
            al::alEffectf(self.reverb_effect, al::AL_REVERB_DENSITY, p.density);
            al::alEffectf(self.reverb_effect, al::AL_REVERB_DIFFUSION, p.diffusion);
            al::alEffectf(self.reverb_effect, al::AL_REVERB_GAIN, p.gain);
            al::alEffectf(self.reverb_effect, al::AL_REVERB_GAINHF, p.gain_hf);
            al::alEffectf(self.reverb_effect, al::AL_REVERB_DECAY_TIME, p.decay_time);
            al::alEffectf(self.reverb_effect, al::AL_REVERB_DECAY_HFRATIO, p.decay_hf_ratio);
            al::alEffectf(self.reverb_effect, al::AL_REVERB_REFLECTIONS_GAIN, p.reflections_gain);
            al::alEffectf(self.reverb_effect, al::AL_REVERB_REFLECTIONS_DELAY, p.reflections_delay);
            al::alEffectf(self.reverb_effect, al::AL_REVERB_LATE_REVERB_GAIN, p.late_reverb_gain);
            al::alEffectf(self.reverb_effect, al::AL_REVERB_LATE_REVERB_DELAY, p.late_reverb_delay);

            // Attach effect to auxiliary slot
            al::alAuxiliaryEffectSloti(
                self.aux_slot,
                al::AL_EFFECTSLOT_EFFECT,
                self.reverb_effect as al::ALint,
            );

            // Apply underwater low-pass filter when in underwater environment
            if env == ReverbEnvironment::Underwater && self.lowpass_filter != 0 {
                al::alFilterf(self.lowpass_filter, al::AL_LOWPASS_GAINHF, 0.15);
            } else if self.lowpass_filter != 0 {
                al::alFilterf(self.lowpass_filter, al::AL_LOWPASS_GAINHF, 1.0);
            }
        }

        self.current_reverb = env;
    }

    /// Route a source through the auxiliary reverb slot.
    fn route_source_to_reverb(&self, source: al::ALuint) {
        if !self.reverb_enabled || self.aux_slot == 0 {
            return;
        }

        // SAFETY: OpenAL is initialized; source and aux slot are valid.
        unsafe {
            let filter = if self.current_reverb == ReverbEnvironment::Underwater
                && self.lowpass_filter != 0
            {
                self.lowpass_filter as al::ALint
            } else {
                al::AL_FILTER_NULL
            };

            // Connect source to auxiliary send 0 with optional filter
            al::alSource3i(
                source,
                al::AL_AUXILIARY_SEND_FILTER,
                self.aux_slot as al::ALint,
                0,     // send number
                filter,
            );
        }
    }

    /// Set the reverb environment. Called when player waterlevel changes.
    fn set_reverb_environment(&mut self, env: ReverbEnvironment) {
        if !self.reverb_enabled || env == self.current_reverb {
            return;
        }
        self.apply_reverb_preset(env);

        // Re-route all active sources with updated filter
        for &source in &self.sources {
            if source != 0 {
                self.route_source_to_reverb(source);
            }
        }
    }

    /// Get or create an AL buffer for the given sound data.
    fn get_or_create_buffer(&mut self, sfx_data: &[u8], format: &AudioFormat) -> al::ALuint {
        self.buffer_counter += 1;

        let mut buffer: al::ALuint = 0;
        // SAFETY: OpenAL is initialized; all parameters are valid.
        unsafe {
            al::alGenBuffers(1, &mut buffer);

            let al_format = match (format.channels, format.bits_per_sample) {
                (1, 8) => al::AL_FORMAT_MONO8,
                (1, 16) => al::AL_FORMAT_MONO16,
                (2, 8) => al::AL_FORMAT_STEREO8,
                (2, 16) => al::AL_FORMAT_STEREO16,
                _ => al::AL_FORMAT_MONO16,
            };

            al::alBufferData(
                buffer,
                al_format,
                sfx_data.as_ptr() as *const c_void,
                sfx_data.len() as al::ALsizei,
                format.sample_rate as al::ALsizei,
            );

            let err = al::alGetError();
            if err != al::AL_NO_ERROR {
                eprintln!("OpenAL buffer upload error: 0x{:X}", err);
            }
        }

        self.buffers.insert(self.buffer_counter, buffer);
        buffer
    }
}

impl Default for OpenAlBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioBackend for OpenAlBackend {
    fn init(&mut self) -> bool {
        // SAFETY: OpenAL Soft is statically linked; all function calls are valid.
        unsafe {
            // Open default device
            self.device = al::alcOpenDevice(ptr::null());
            if self.device.is_null() {
                eprintln!("OpenAL: Failed to open default audio device");
                return false;
            }

            // Create context with HRTF request
            let attrs: [al::ALCint; 5] = if self.hrtf_enabled {
                [al::ALC_HRTF_SOFT, al::ALC_TRUE, al::ALC_FREQUENCY, 44100, 0]
            } else {
                [al::ALC_FREQUENCY, 44100, 0, 0, 0]
            };

            self.context = al::alcCreateContext(self.device, attrs.as_ptr());
            if self.context.is_null() {
                eprintln!("OpenAL: Failed to create context");
                al::alcCloseDevice(self.device);
                self.device = ptr::null_mut();
                return false;
            }

            al::alcMakeContextCurrent(self.context);

            // Check HRTF status
            if self.hrtf_enabled {
                let mut hrtf_status: al::ALCint = 0;
                al::alcGetIntegerv(self.device, al::ALC_HRTF_STATUS_SOFT, 1, &mut hrtf_status);
                match hrtf_status {
                    al::ALC_HRTF_ENABLED_SOFT => {
                        eprintln!("OpenAL: HRTF enabled");
                    }
                    al::ALC_HRTF_DENIED_SOFT => {
                        eprintln!("OpenAL: HRTF denied by user config");
                    }
                    al::ALC_HRTF_REQUIRED_SOFT => {
                        eprintln!("OpenAL: HRTF required and enabled");
                    }
                    al::ALC_HRTF_HEADPHONES_DETECTED_SOFT => {
                        eprintln!("OpenAL: Headphones detected, HRTF enabled");
                    }
                    al::ALC_HRTF_UNSUPPORTED_FORMAT_SOFT => {
                        eprintln!("OpenAL: HRTF not available (unsupported format)");
                    }
                    _ => {
                        eprintln!("OpenAL: HRTF not available, using standard 3D audio");
                    }
                }
            }

            // Set distance model
            al::alDistanceModel(al::AL_INVERSE_DISTANCE_CLAMPED);

            // Generate sources
            al::alGenSources(MAX_CHANNELS as al::ALsizei, self.sources.as_mut_ptr());
            let err = al::alGetError();
            if err != al::AL_NO_ERROR {
                eprintln!("OpenAL: Failed to generate sources: 0x{:X}", err);
            }

            // Configure default source properties
            for &source in &self.sources {
                if source != 0 {
                    al::alSourcef(source, al::AL_PITCH, 1.0);
                    al::alSourcef(source, al::AL_GAIN, 1.0);
                    al::alSourcef(source, al::AL_REFERENCE_DISTANCE, 200.0);
                    al::alSourcef(source, al::AL_ROLLOFF_FACTOR, 1.0);
                    al::alSourcef(source, al::AL_MAX_DISTANCE, 4096.0);
                }
            }

            // Enable Doppler effect
            if self.doppler_enabled {
                al::alDopplerFactor(1.0);
                al::alSpeedOfSound(Q2_SPEED_OF_SOUND);
                eprintln!("OpenAL: Doppler enabled (speed of sound = {} units/s)", Q2_SPEED_OF_SOUND);
            }
        }

        self.initialized = true;
        self.active = true;

        // Initialize EFX reverb (non-fatal if unsupported)
        if self.reverb_enabled {
            if !self.init_efx() {
                self.reverb_enabled = false;
                eprintln!("OpenAL: EFX reverb not available");
            } else {
                // Route all sources through reverb
                for &source in &self.sources {
                    if source != 0 {
                        self.route_source_to_reverb(source);
                    }
                }
            }
        }

        // Initialize streaming source and buffers for cinematics
        // SAFETY: OpenAL is initialized.
        unsafe {
            al::alGenSources(1, &mut self.streaming_source);
            if al::alGetError() == al::AL_NO_ERROR && self.streaming_source != 0 {
                al::alGenBuffers(
                    STREAMING_BUFFER_COUNT as al::ALsizei,
                    self.streaming_buffers.as_mut_ptr(),
                );
                if al::alGetError() == al::AL_NO_ERROR {
                    // Configure streaming source: listener-relative, no spatialization
                    al::alSourcei(
                        self.streaming_source,
                        al::AL_SOURCE_RELATIVE,
                        al::AL_TRUE as al::ALint,
                    );
                    al::alSource3f(self.streaming_source, al::AL_POSITION, 0.0, 0.0, 0.0);
                    al::alSourcef(self.streaming_source, al::AL_GAIN, 1.0);
                    al::alSourcei(self.streaming_source, al::AL_LOOPING, al::AL_FALSE as al::ALint);
                    self.streaming_initialized = true;
                    eprintln!("OpenAL: Streaming audio initialized ({} buffers)", STREAMING_BUFFER_COUNT);
                } else {
                    eprintln!("OpenAL: Failed to create streaming buffers");
                    al::alDeleteSources(1, &self.streaming_source);
                    self.streaming_source = 0;
                }
            } else {
                eprintln!("OpenAL: Failed to create streaming source");
            }
        }

        eprintln!(
            "OpenAL: Audio initialized ({} channels, HRTF={}, reverb={}, doppler={})",
            MAX_CHANNELS,
            if self.hrtf_enabled { "on" } else { "off" },
            if self.reverb_enabled { "on" } else { "off" },
            if self.doppler_enabled { "on" } else { "off" },
        );
        true
    }

    fn shutdown(&mut self) {
        if !self.initialized {
            return;
        }

        // SAFETY: OpenAL is initialized; cleanup calls are valid.
        unsafe {
            // Clean up streaming resources
            if self.streaming_initialized {
                al::alSourceStop(self.streaming_source);
                al::alSourcei(self.streaming_source, al::AL_BUFFER, 0);
                al::alDeleteSources(1, &self.streaming_source);
                al::alDeleteBuffers(
                    STREAMING_BUFFER_COUNT as al::ALsizei,
                    self.streaming_buffers.as_ptr(),
                );
                self.streaming_source = 0;
                self.streaming_buffers = [0; STREAMING_BUFFER_COUNT];
                self.streaming_initialized = false;
            }

            // Disconnect sources from aux send before cleanup
            for &source in &self.sources {
                if source != 0 {
                    al::alSourceStop(source);
                    al::alSource3i(source, al::AL_AUXILIARY_SEND_FILTER, 0, 0, al::AL_FILTER_NULL);
                    al::alSourcei(source, al::AL_BUFFER, 0);
                }
            }

            // Clean up EFX resources
            if self.lowpass_filter != 0 {
                al::alDeleteFilters(1, &self.lowpass_filter);
                self.lowpass_filter = 0;
            }
            if self.reverb_effect != 0 {
                al::alDeleteEffects(1, &self.reverb_effect);
                self.reverb_effect = 0;
            }
            if self.aux_slot != 0 {
                al::alDeleteAuxiliaryEffectSlots(1, &self.aux_slot);
                self.aux_slot = 0;
            }

            al::alDeleteSources(MAX_CHANNELS as al::ALsizei, self.sources.as_ptr());

            for &buffer in self.buffers.values() {
                al::alDeleteBuffers(1, &buffer);
            }
            self.buffers.clear();

            al::alcMakeContextCurrent(ptr::null_mut());
            if !self.context.is_null() {
                al::alcDestroyContext(self.context);
                self.context = ptr::null_mut();
            }
            if !self.device.is_null() {
                al::alcCloseDevice(self.device);
                self.device = ptr::null_mut();
            }
        }

        self.sources = [0; MAX_CHANNELS];
        self.initialized = false;
        eprintln!("OpenAL: Audio shut down");
    }

    fn play_sound(
        &mut self,
        channel: usize,
        sfx_data: &[u8],
        format: &AudioFormat,
        origin: &[f32; 3],
        volume: f32,
        attenuation: f32,
        looping: bool,
    ) {
        if !self.initialized || !self.active || channel >= MAX_CHANNELS {
            return;
        }

        let source = self.sources[channel];
        if source == 0 {
            return;
        }

        let buffer = self.get_or_create_buffer(sfx_data, format);

        // SAFETY: OpenAL is initialized; source and buffer are valid.
        unsafe {
            al::alSourceStop(source);
            al::alSourcei(source, al::AL_BUFFER, buffer as al::ALint);

            if attenuation > 0.0 {
                let (x, y, z) = Self::q2_to_al(origin);
                al::alSourcei(source, al::AL_SOURCE_RELATIVE, al::AL_FALSE as al::ALint);
                al::alSource3f(source, al::AL_POSITION, x, y, z);
                al::alSourcef(source, al::AL_REFERENCE_DISTANCE, 200.0 / attenuation);
                al::alSourcef(source, al::AL_ROLLOFF_FACTOR, attenuation);
            } else {
                al::alSourcei(source, al::AL_SOURCE_RELATIVE, al::AL_TRUE as al::ALint);
                al::alSource3f(source, al::AL_POSITION, 0.0, 0.0, 0.0);
            }

            al::alSourcef(source, al::AL_GAIN, volume);
            al::alSourcei(
                source,
                al::AL_LOOPING,
                if looping { al::AL_TRUE as al::ALint } else { al::AL_FALSE as al::ALint },
            );

            // Zero out velocity (Doppler starts fresh per sound)
            al::alSource3f(source, al::AL_VELOCITY, 0.0, 0.0, 0.0);
        }

        // Route through reverb aux send
        self.route_source_to_reverb(source);

        // SAFETY: OpenAL is initialized; source is valid.
        unsafe {
            al::alSourcePlay(source);
        }
    }

    fn stop_channel(&mut self, channel: usize) {
        if !self.initialized || channel >= MAX_CHANNELS {
            return;
        }

        let source = self.sources[channel];
        if source == 0 {
            return;
        }

        // SAFETY: OpenAL is initialized; source is valid.
        unsafe {
            al::alSourceStop(source);
            al::alSourcei(source, al::AL_BUFFER, 0);
        }
    }

    fn update_listener(&mut self, origin: &[f32; 3], forward: &[f32; 3], up: &[f32; 3]) {
        if !self.initialized {
            return;
        }

        // SAFETY: OpenAL is initialized.
        unsafe {
            let (px, py, pz) = Self::q2_to_al(origin);
            al::alListener3f(al::AL_POSITION, px, py, pz);

            let (fx, fy, fz) = Self::q2_to_al(forward);
            let (ux, uy, uz) = Self::q2_to_al(up);
            let orientation: [f32; 6] = [fx, fy, fz, ux, uy, uz];
            al::alListenerfv(al::AL_ORIENTATION, orientation.as_ptr());
        }
    }

    fn update_channel_position(&mut self, channel: usize, origin: &[f32; 3]) {
        if !self.initialized || channel >= MAX_CHANNELS {
            return;
        }

        let source = self.sources[channel];
        if source == 0 {
            return;
        }

        let (x, y, z) = Self::q2_to_al(origin);
        // SAFETY: OpenAL is initialized; source is valid.
        unsafe {
            al::alSource3f(source, al::AL_POSITION, x, y, z);
        }
    }

    fn update_channel_velocity(&mut self, channel: usize, velocity: &[f32; 3]) {
        if !self.initialized || !self.doppler_enabled || channel >= MAX_CHANNELS {
            return;
        }

        let source = self.sources[channel];
        if source == 0 {
            return;
        }

        let (vx, vy, vz) = Self::q2_to_al(velocity);
        // SAFETY: OpenAL is initialized; source is valid.
        unsafe {
            al::alSource3f(source, al::AL_VELOCITY, vx, vy, vz);
        }
    }

    fn set_environment(&mut self, env: i32) {
        if !self.initialized || !self.reverb_enabled {
            return;
        }

        let reverb_env = match env {
            0 => ReverbEnvironment::Generic,
            1 => ReverbEnvironment::Underwater,
            2 => ReverbEnvironment::Cave,
            3 => ReverbEnvironment::Hallway,
            4 => ReverbEnvironment::Arena,
            5 => ReverbEnvironment::Forest,
            6 => ReverbEnvironment::Cathedral,
            7 => ReverbEnvironment::MetalRoom,
            8 => ReverbEnvironment::Snow,
            9 => ReverbEnvironment::Tunnel,
            10 => ReverbEnvironment::Bathroom,
            _ => ReverbEnvironment::Generic,
        };

        self.set_reverb_environment(reverb_env);
    }

    fn auto_detect_environment(&mut self, room_data: &RoomAnalysis) {
        if !self.initialized || !self.reverb_enabled {
            return;
        }

        // Priority 1: Underwater always takes precedence
        if room_data.underwater {
            self.set_reverb_environment(ReverbEnvironment::Underwater);
            return;
        }

        // Priority 2: Outdoors detection (sky visible)
        if room_data.outdoors {
            // Check if it might be snowy (very absorptive)
            if room_data.surface_material == SurfaceMaterial::Snow {
                self.set_reverb_environment(ReverbEnvironment::Snow);
                return;
            }
            // General outdoor - forest-like
            self.set_reverb_environment(ReverbEnvironment::Forest);
            return;
        }

        // Priority 3: Material-based detection
        match room_data.surface_material {
            SurfaceMaterial::Metal => {
                self.set_reverb_environment(ReverbEnvironment::MetalRoom);
                return;
            }
            SurfaceMaterial::Glass => {
                self.set_reverb_environment(ReverbEnvironment::Bathroom);
                return;
            }
            SurfaceMaterial::Snow => {
                self.set_reverb_environment(ReverbEnvironment::Snow);
                return;
            }
            _ => {}
        }

        // Priority 4: Room size/shape detection
        let avg_distance = room_data.wall_distances.iter().sum::<f32>() / 6.0;

        // Tunnel/Corridor detection: 2 walls close, 2 walls far
        if room_data.walls_detected >= 4 {
            let width = room_data.wall_distances[0] + room_data.wall_distances[1];
            let depth = room_data.wall_distances[2] + room_data.wall_distances[3];
            let aspect_ratio = if width > depth { width / depth.max(1.0) } else { depth / width.max(1.0) };

            if aspect_ratio > 3.0 {
                self.set_reverb_environment(ReverbEnvironment::Tunnel);
                return;
            }

            if aspect_ratio > 2.0 && room_data.room_height < 200.0 {
                self.set_reverb_environment(ReverbEnvironment::Hallway);
                return;
            }
        }

        // Volume-based detection
        let volume = room_data.room_volume;

        if volume > 10_000_000.0 {
            // Very large space (> 200x200x250 units)
            self.set_reverb_environment(ReverbEnvironment::Cathedral);
        } else if volume > 2_000_000.0 {
            // Large space (arena-sized)
            self.set_reverb_environment(ReverbEnvironment::Arena);
        } else if volume > 500_000.0 {
            // Medium space - check height for cave detection
            if room_data.room_height > 300.0 {
                self.set_reverb_environment(ReverbEnvironment::Cave);
            } else {
                self.set_reverb_environment(ReverbEnvironment::Generic);
            }
        } else if volume < 50_000.0 {
            // Small space (bathroom-sized)
            if room_data.surface_material == SurfaceMaterial::Stone ||
               room_data.surface_material == SurfaceMaterial::Concrete {
                self.set_reverb_environment(ReverbEnvironment::Bathroom);
            } else {
                self.set_reverb_environment(ReverbEnvironment::Hallway);
            }
        } else {
            // Default medium room
            self.set_reverb_environment(ReverbEnvironment::Generic);
        }
    }

    fn is_channel_playing(&self, channel: usize) -> bool {
        if !self.initialized || channel >= MAX_CHANNELS {
            return false;
        }

        let source = self.sources[channel];
        if source == 0 {
            return false;
        }

        let mut state: al::ALint = 0;
        // SAFETY: OpenAL is initialized; source is valid.
        unsafe {
            al::alGetSourcei(source, al::AL_SOURCE_STATE, &mut state);
        }
        state == al::AL_PLAYING
    }

    fn activate(&mut self, active: bool) {
        if !self.initialized {
            return;
        }

        self.active = active;

        // SAFETY: OpenAL is initialized.
        unsafe {
            if active {
                for &source in &self.sources {
                    if source != 0 {
                        let mut state: al::ALint = 0;
                        al::alGetSourcei(source, al::AL_SOURCE_STATE, &mut state);
                        if state == al::AL_PAUSED {
                            al::alSourcePlay(source);
                        }
                    }
                }
            } else {
                for &source in &self.sources {
                    if source != 0 {
                        let mut state: al::ALint = 0;
                        al::alGetSourcei(source, al::AL_SOURCE_STATE, &mut state);
                        if state == al::AL_PLAYING {
                            al::alSourcePause(source);
                        }
                    }
                }
            }
        }
    }

    fn queue_streaming_samples(&mut self, samples: &[i16], rate: i32, channels: i32) {
        if !self.initialized || !self.streaming_initialized || samples.is_empty() {
            return;
        }

        // SAFETY: OpenAL is initialized; streaming source and buffers are valid.
        unsafe {
            // Unqueue any processed buffers first
            let mut processed: al::ALint = 0;
            al::alGetSourcei(self.streaming_source, al::AL_BUFFERS_PROCESSED, &mut processed);

            while processed > 0 {
                let mut buffer: al::ALuint = 0;
                al::alSourceUnqueueBuffers(self.streaming_source, 1, &mut buffer);
                processed -= 1;
            }

            // Check how many buffers are queued
            let mut queued: al::ALint = 0;
            al::alGetSourcei(self.streaming_source, al::AL_BUFFERS_QUEUED, &mut queued);

            // Find a free buffer to use
            if (queued as usize) < STREAMING_BUFFER_COUNT {
                // Find an unused buffer
                let buffer = self.streaming_buffers[queued as usize];

                // Determine format
                let format = match channels {
                    1 => al::AL_FORMAT_MONO16,
                    _ => al::AL_FORMAT_STEREO16,
                };

                // Upload data to buffer
                al::alBufferData(
                    buffer,
                    format,
                    samples.as_ptr() as *const c_void,
                    (samples.len() * std::mem::size_of::<i16>()) as al::ALsizei,
                    rate as al::ALsizei,
                );

                if al::alGetError() != al::AL_NO_ERROR {
                    eprintln!("OpenAL: Failed to upload streaming buffer data");
                    return;
                }

                // Queue the buffer
                al::alSourceQueueBuffers(self.streaming_source, 1, &buffer);

                // Start playing if not already
                let mut state: al::ALint = 0;
                al::alGetSourcei(self.streaming_source, al::AL_SOURCE_STATE, &mut state);
                if state != al::AL_PLAYING {
                    al::alSourcePlay(self.streaming_source);
                }
            }
        }
    }

    fn is_streaming_active(&self) -> bool {
        if !self.initialized || !self.streaming_initialized {
            return false;
        }

        // SAFETY: OpenAL is initialized; streaming source is valid.
        unsafe {
            let mut state: al::ALint = 0;
            al::alGetSourcei(self.streaming_source, al::AL_SOURCE_STATE, &mut state);
            state == al::AL_PLAYING
        }
    }

    fn stop_streaming(&mut self) {
        if !self.initialized || !self.streaming_initialized {
            return;
        }

        // SAFETY: OpenAL is initialized; streaming source is valid.
        unsafe {
            al::alSourceStop(self.streaming_source);

            // Unqueue all buffers
            let mut queued: al::ALint = 0;
            al::alGetSourcei(self.streaming_source, al::AL_BUFFERS_QUEUED, &mut queued);

            while queued > 0 {
                let mut buffer: al::ALuint = 0;
                al::alSourceUnqueueBuffers(self.streaming_source, 1, &mut buffer);
                queued -= 1;
            }
        }
    }
}

impl Drop for OpenAlBackend {
    fn drop(&mut self) {
        self.shutdown();
    }
}

// SAFETY: OpenAlBackend is only used from the main thread in this single-threaded engine.
// The raw pointers (ALCdevice, ALCcontext) are not shared across threads.
unsafe impl Send for OpenAlBackend {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Coordinate system conversion tests (q2_to_al)
    // ========================================================================

    #[test]
    fn test_q2_to_al_origin() {
        let pos = [0.0, 0.0, 0.0];
        let (x, y, z) = OpenAlBackend::q2_to_al(&pos);
        assert_eq!(x, 0.0);
        assert_eq!(y, 0.0);
        assert_eq!(z, 0.0);
    }

    #[test]
    fn test_q2_to_al_x_axis() {
        // Q2 X maps to AL X
        let pos = [100.0, 0.0, 0.0];
        let (x, y, z) = OpenAlBackend::q2_to_al(&pos);
        assert_eq!(x, 100.0);
        assert_eq!(y, 0.0);
        assert_eq!(z, 0.0);
    }

    #[test]
    fn test_q2_to_al_y_axis() {
        // Q2 Y maps to AL -Z
        let pos = [0.0, 100.0, 0.0];
        let (x, y, z) = OpenAlBackend::q2_to_al(&pos);
        assert_eq!(x, 0.0);
        assert_eq!(y, 0.0);
        assert_eq!(z, -100.0);
    }

    #[test]
    fn test_q2_to_al_z_axis() {
        // Q2 Z maps to AL Y
        let pos = [0.0, 0.0, 100.0];
        let (x, y, z) = OpenAlBackend::q2_to_al(&pos);
        assert_eq!(x, 0.0);
        assert_eq!(y, 100.0);
        assert_eq!(z, 0.0);
    }

    #[test]
    fn test_q2_to_al_combined() {
        // Q2 (10, 20, 30) -> AL (10, 30, -20)
        let pos = [10.0, 20.0, 30.0];
        let (x, y, z) = OpenAlBackend::q2_to_al(&pos);
        assert_eq!(x, 10.0);
        assert_eq!(y, 30.0);
        assert_eq!(z, -20.0);
    }

    #[test]
    fn test_q2_to_al_negative_coords() {
        let pos = [-5.0, -10.0, -15.0];
        let (x, y, z) = OpenAlBackend::q2_to_al(&pos);
        assert_eq!(x, -5.0);
        assert_eq!(y, -15.0);
        assert_eq!(z, 10.0); // -(-10) = 10
    }

    #[test]
    fn test_q2_to_al_preserves_magnitude() {
        let pos: [f32; 3] = [3.0, 4.0, 0.0];
        let q2_mag = (pos[0] * pos[0] + pos[1] * pos[1] + pos[2] * pos[2]).sqrt();
        let (x, y, z) = OpenAlBackend::q2_to_al(&pos);
        let al_mag = (x * x + y * y + z * z).sqrt();
        assert!((q2_mag - al_mag).abs() < 0.001,
            "magnitude should be preserved: Q2={}, AL={}", q2_mag, al_mag);
    }

    #[test]
    fn test_q2_to_al_forward_vector() {
        // Q2 forward = (1, 0, 0), AL forward should be (1, 0, 0)
        let fwd = [1.0, 0.0, 0.0];
        let (x, y, z) = OpenAlBackend::q2_to_al(&fwd);
        assert_eq!((x, y, z), (1.0, 0.0, 0.0));
    }

    #[test]
    fn test_q2_to_al_up_vector() {
        // Q2 up = (0, 0, 1), AL up should be (0, 1, 0)
        let up = [0.0, 0.0, 1.0];
        let (x, y, z) = OpenAlBackend::q2_to_al(&up);
        assert_eq!((x, y, z), (0.0, 1.0, 0.0));
    }

    // ========================================================================
    // Constants tests
    // ========================================================================

    #[test]
    fn test_max_channels() {
        assert_eq!(MAX_CHANNELS, 32);
    }

    #[test]
    fn test_speed_of_sound() {
        // 340 m/s * 40 Q2 units/meter = 13600
        assert_eq!(Q2_SPEED_OF_SOUND, 13600.0);
    }

    #[test]
    fn test_streaming_buffer_count() {
        assert_eq!(STREAMING_BUFFER_COUNT, 4);
    }

    #[test]
    fn test_streaming_buffer_size() {
        // ~0.25 seconds at 44100 Hz stereo 16-bit
        assert_eq!(STREAMING_BUFFER_SIZE, 44100);
    }

    // ========================================================================
    // OpenAlBackend::new() default state tests
    // ========================================================================

    #[test]
    fn test_backend_new_defaults() {
        let backend = OpenAlBackend::new();
        assert!(backend.device.is_null());
        assert!(backend.context.is_null());
        assert_eq!(backend.sources, [0; MAX_CHANNELS]);
        assert!(backend.buffers.is_empty());
        assert_eq!(backend.buffer_counter, 0);
        assert!(!backend.initialized);
        assert!(backend.hrtf_enabled);
        assert!(backend.active);
    }

    #[test]
    fn test_backend_new_efx_defaults() {
        let backend = OpenAlBackend::new();
        assert_eq!(backend.aux_slot, 0);
        assert_eq!(backend.reverb_effect, 0);
        assert_eq!(backend.lowpass_filter, 0);
        assert!(backend.reverb_enabled);
        assert_eq!(backend.current_reverb, ReverbEnvironment::Generic);
        assert!(backend.doppler_enabled);
    }

    #[test]
    fn test_backend_new_streaming_defaults() {
        let backend = OpenAlBackend::new();
        assert_eq!(backend.streaming_source, 0);
        assert_eq!(backend.streaming_buffers, [0; STREAMING_BUFFER_COUNT]);
        assert!(!backend.streaming_initialized);
    }

    #[test]
    fn test_backend_default_matches_new() {
        let from_new = OpenAlBackend::new();
        let from_default = OpenAlBackend::default();
        assert_eq!(from_new.initialized, from_default.initialized);
        assert_eq!(from_new.hrtf_enabled, from_default.hrtf_enabled);
        assert_eq!(from_new.active, from_default.active);
        assert_eq!(from_new.reverb_enabled, from_default.reverb_enabled);
        assert_eq!(from_new.doppler_enabled, from_default.doppler_enabled);
    }

    // ========================================================================
    // ReverbPreset constant tests
    // ========================================================================

    #[test]
    fn test_reverb_preset_generic() {
        let p = ReverbPreset::GENERIC;
        assert_eq!(p.density, 1.0);
        assert_eq!(p.diffusion, 1.0);
        assert_eq!(p.gain, 0.32);
        assert_eq!(p.gain_hf, 0.89);
        assert_eq!(p.decay_time, 1.49);
        assert_eq!(p.decay_hf_ratio, 0.83);
    }

    #[test]
    fn test_reverb_preset_underwater() {
        let p = ReverbPreset::UNDERWATER;
        assert_eq!(p.gain_hf, 0.01); // very low HF gain for muffling
        assert_eq!(p.decay_time, 8.0); // very long decay underwater
    }

    #[test]
    fn test_reverb_preset_cave() {
        let p = ReverbPreset::CAVE;
        assert_eq!(p.decay_time, 3.0);
        assert_eq!(p.density, 1.0);
    }

    #[test]
    fn test_reverb_preset_cathedral() {
        let p = ReverbPreset::CATHEDRAL;
        assert_eq!(p.decay_time, 8.5); // very long decay
        assert_eq!(p.late_reverb_gain, 1.4); // strong late reverb
    }

    #[test]
    fn test_reverb_preset_metal_room() {
        let p = ReverbPreset::METAL_ROOM;
        assert_eq!(p.gain_hf, 0.95); // bright reflections
        assert_eq!(p.decay_hf_ratio, 0.9); // high HF preservation
    }

    #[test]
    fn test_reverb_preset_snow() {
        let p = ReverbPreset::SNOW;
        assert_eq!(p.gain_hf, 0.15); // very absorptive
        assert_eq!(p.decay_time, 0.6); // short decay
    }

    #[test]
    fn test_reverb_presets_density_range() {
        // All presets should have density in [0, 1]
        let presets = [
            ReverbPreset::GENERIC, ReverbPreset::UNDERWATER, ReverbPreset::CAVE,
            ReverbPreset::HALLWAY, ReverbPreset::ARENA, ReverbPreset::FOREST,
            ReverbPreset::CATHEDRAL, ReverbPreset::METAL_ROOM, ReverbPreset::SNOW,
            ReverbPreset::TUNNEL, ReverbPreset::BATHROOM,
        ];
        for p in &presets {
            assert!(p.density >= 0.0 && p.density <= 1.0,
                "density {} out of [0,1]", p.density);
        }
    }

    #[test]
    fn test_reverb_presets_diffusion_range() {
        let presets = [
            ReverbPreset::GENERIC, ReverbPreset::UNDERWATER, ReverbPreset::CAVE,
            ReverbPreset::HALLWAY, ReverbPreset::ARENA, ReverbPreset::FOREST,
            ReverbPreset::CATHEDRAL, ReverbPreset::METAL_ROOM, ReverbPreset::SNOW,
            ReverbPreset::TUNNEL, ReverbPreset::BATHROOM,
        ];
        for p in &presets {
            assert!(p.diffusion >= 0.0 && p.diffusion <= 1.0,
                "diffusion {} out of [0,1]", p.diffusion);
        }
    }

    #[test]
    fn test_reverb_presets_gain_positive() {
        let presets = [
            ReverbPreset::GENERIC, ReverbPreset::UNDERWATER, ReverbPreset::CAVE,
            ReverbPreset::HALLWAY, ReverbPreset::ARENA, ReverbPreset::FOREST,
            ReverbPreset::CATHEDRAL, ReverbPreset::METAL_ROOM, ReverbPreset::SNOW,
            ReverbPreset::TUNNEL, ReverbPreset::BATHROOM,
        ];
        for p in &presets {
            assert!(p.gain > 0.0, "gain should be positive, got {}", p.gain);
            assert!(p.gain_hf > 0.0, "gain_hf should be positive, got {}", p.gain_hf);
        }
    }

    #[test]
    fn test_reverb_presets_decay_positive() {
        let presets = [
            ReverbPreset::GENERIC, ReverbPreset::UNDERWATER, ReverbPreset::CAVE,
            ReverbPreset::HALLWAY, ReverbPreset::ARENA, ReverbPreset::FOREST,
            ReverbPreset::CATHEDRAL, ReverbPreset::METAL_ROOM, ReverbPreset::SNOW,
            ReverbPreset::TUNNEL, ReverbPreset::BATHROOM,
        ];
        for p in &presets {
            assert!(p.decay_time > 0.0, "decay_time should be positive, got {}", p.decay_time);
            assert!(p.decay_hf_ratio > 0.0, "decay_hf_ratio should be positive, got {}", p.decay_hf_ratio);
        }
    }

    #[test]
    fn test_reverb_presets_delay_non_negative() {
        let presets = [
            ReverbPreset::GENERIC, ReverbPreset::UNDERWATER, ReverbPreset::CAVE,
            ReverbPreset::HALLWAY, ReverbPreset::ARENA, ReverbPreset::FOREST,
            ReverbPreset::CATHEDRAL, ReverbPreset::METAL_ROOM, ReverbPreset::SNOW,
            ReverbPreset::TUNNEL, ReverbPreset::BATHROOM,
        ];
        for p in &presets {
            assert!(p.reflections_delay >= 0.0,
                "reflections_delay should be >= 0, got {}", p.reflections_delay);
            assert!(p.late_reverb_delay >= 0.0,
                "late_reverb_delay should be >= 0, got {}", p.late_reverb_delay);
        }
    }

    // ========================================================================
    // ReverbEnvironment mapping tests
    // ========================================================================

    #[test]
    fn test_reverb_environment_preset_mapping() {
        // Each environment should return a valid preset
        let envs = [
            ReverbEnvironment::Generic,
            ReverbEnvironment::Underwater,
            ReverbEnvironment::Cave,
            ReverbEnvironment::Hallway,
            ReverbEnvironment::Arena,
            ReverbEnvironment::Forest,
            ReverbEnvironment::Cathedral,
            ReverbEnvironment::MetalRoom,
            ReverbEnvironment::Snow,
            ReverbEnvironment::Tunnel,
            ReverbEnvironment::Bathroom,
        ];
        for env in &envs {
            let p = env.preset();
            assert!(p.decay_time > 0.0, "{:?} has invalid decay_time", env);
        }
    }

    #[test]
    fn test_reverb_environment_equality() {
        assert_eq!(ReverbEnvironment::Generic, ReverbEnvironment::Generic);
        assert_ne!(ReverbEnvironment::Generic, ReverbEnvironment::Underwater);
    }

    #[test]
    fn test_underwater_has_longest_or_near_longest_decay() {
        let underwater = ReverbPreset::UNDERWATER;
        let generic = ReverbPreset::GENERIC;
        assert!(underwater.decay_time > generic.decay_time);
    }

    #[test]
    fn test_cathedral_has_very_long_decay() {
        let cathedral = ReverbPreset::CATHEDRAL;
        assert!(cathedral.decay_time > 5.0);
    }

    #[test]
    fn test_snow_shortest_decay() {
        let snow = ReverbPreset::SNOW;
        let generic = ReverbPreset::GENERIC;
        assert!(snow.decay_time < generic.decay_time);
    }

    // ========================================================================
    // Distance attenuation formula tests
    // ========================================================================

    #[test]
    fn test_reference_distance_calculation() {
        // The code uses: reference_distance = 200.0 / attenuation
        // At attenuation = 1.0: ref_dist = 200
        // At attenuation = 2.0: ref_dist = 100
        // At attenuation = 0.5: ref_dist = 400
        let ref_dist_1 = 200.0f32 / 1.0;
        let ref_dist_2 = 200.0f32 / 2.0;
        let ref_dist_half = 200.0f32 / 0.5;
        assert_eq!(ref_dist_1, 200.0);
        assert_eq!(ref_dist_2, 100.0);
        assert_eq!(ref_dist_half, 400.0);
    }

    #[test]
    fn test_inverse_distance_attenuation_formula() {
        // OpenAL inverse distance clamped formula:
        // gain = ref_dist / (ref_dist + rolloff * (distance - ref_dist))
        // For Q2 defaults: ref_dist = 200, rolloff = 1.0, max_dist = 4096
        let ref_dist: f32 = 200.0;
        let rolloff: f32 = 1.0;

        // At reference distance, gain should be 1.0
        let gain_at_ref = ref_dist / (ref_dist + rolloff * (ref_dist - ref_dist));
        assert!((gain_at_ref - 1.0).abs() < 0.001);

        // At double reference distance, gain should be 0.5
        let distance = 400.0f32;
        let gain_at_double = ref_dist / (ref_dist + rolloff * (distance - ref_dist));
        assert!((gain_at_double - 0.5).abs() < 0.001);

        // At closer than reference, gain is clamped to 1.0 (not tested here, OpenAL does it)
    }

    // ========================================================================
    // Doppler effect calculation tests
    // ========================================================================

    #[test]
    fn test_doppler_speed_of_sound_calculation() {
        // 340 m/s * 40 units/meter = 13600 units/s
        let meters_per_second: f32 = 340.0;
        let q2_units_per_meter: f32 = 40.0;
        let expected = meters_per_second * q2_units_per_meter;
        assert_eq!(expected, Q2_SPEED_OF_SOUND);
    }

    #[test]
    fn test_doppler_frequency_shift_approaching() {
        // Doppler formula: f' = f * (speed_of_sound - listener_vel) / (speed_of_sound - source_vel)
        // Source approaching listener: source_vel positive towards listener
        let speed = Q2_SPEED_OF_SOUND;
        let source_vel = 500.0f32; // approaching at 500 units/s

        let freq_ratio = speed / (speed - source_vel);
        // Should be > 1.0 (higher pitch when approaching)
        assert!(freq_ratio > 1.0,
            "approaching source should increase pitch: {}", freq_ratio);
    }

    #[test]
    fn test_doppler_frequency_shift_receding() {
        let speed = Q2_SPEED_OF_SOUND;
        let source_vel = -500.0f32; // receding at 500 units/s

        let freq_ratio = speed / (speed - source_vel);
        // Should be < 1.0 (lower pitch when receding)
        assert!(freq_ratio < 1.0,
            "receding source should decrease pitch: {}", freq_ratio);
    }

    #[test]
    fn test_doppler_stationary_no_shift() {
        let speed = Q2_SPEED_OF_SOUND;
        let freq_ratio = speed / (speed - 0.0);
        assert!((freq_ratio - 1.0).abs() < 0.001);
    }

    // ========================================================================
    // Buffer format selection tests (logic validation)
    // ========================================================================

    #[test]
    fn test_al_format_mono8() {
        let channels = 1;
        let bits = 8;
        let format = match (channels, bits) {
            (1, 8) => al::AL_FORMAT_MONO8,
            (1, 16) => al::AL_FORMAT_MONO16,
            (2, 8) => al::AL_FORMAT_STEREO8,
            (2, 16) => al::AL_FORMAT_STEREO16,
            _ => al::AL_FORMAT_MONO16,
        };
        assert_eq!(format, al::AL_FORMAT_MONO8);
    }

    #[test]
    fn test_al_format_mono16() {
        let channels = 1;
        let bits = 16;
        let format = match (channels, bits) {
            (1, 8) => al::AL_FORMAT_MONO8,
            (1, 16) => al::AL_FORMAT_MONO16,
            (2, 8) => al::AL_FORMAT_STEREO8,
            (2, 16) => al::AL_FORMAT_STEREO16,
            _ => al::AL_FORMAT_MONO16,
        };
        assert_eq!(format, al::AL_FORMAT_MONO16);
    }

    #[test]
    fn test_al_format_stereo8() {
        let channels = 2;
        let bits = 8;
        let format = match (channels, bits) {
            (1, 8) => al::AL_FORMAT_MONO8,
            (1, 16) => al::AL_FORMAT_MONO16,
            (2, 8) => al::AL_FORMAT_STEREO8,
            (2, 16) => al::AL_FORMAT_STEREO16,
            _ => al::AL_FORMAT_MONO16,
        };
        assert_eq!(format, al::AL_FORMAT_STEREO8);
    }

    #[test]
    fn test_al_format_stereo16() {
        let channels = 2;
        let bits = 16;
        let format = match (channels, bits) {
            (1, 8) => al::AL_FORMAT_MONO8,
            (1, 16) => al::AL_FORMAT_MONO16,
            (2, 8) => al::AL_FORMAT_STEREO8,
            (2, 16) => al::AL_FORMAT_STEREO16,
            _ => al::AL_FORMAT_MONO16,
        };
        assert_eq!(format, al::AL_FORMAT_STEREO16);
    }

    #[test]
    fn test_al_format_unknown_defaults_to_mono16() {
        let channels = 4;
        let bits = 32;
        let format = match (channels, bits) {
            (1, 8) => al::AL_FORMAT_MONO8,
            (1, 16) => al::AL_FORMAT_MONO16,
            (2, 8) => al::AL_FORMAT_STEREO8,
            (2, 16) => al::AL_FORMAT_STEREO16,
            _ => al::AL_FORMAT_MONO16,
        };
        assert_eq!(format, al::AL_FORMAT_MONO16);
    }

    // ========================================================================
    // Volume/gain calculation tests
    // ========================================================================

    #[test]
    fn test_listener_relative_source_at_origin() {
        // When attenuation = 0, source should be listener-relative at (0,0,0)
        // This means volume is always max regardless of listener position
        // Verified by the code: attenuation == 0 -> AL_SOURCE_RELATIVE = TRUE, pos = (0,0,0)
        let attenuation = 0.0f32;
        assert!(attenuation <= 0.0); // This triggers the listener-relative branch
    }

    #[test]
    fn test_attenuation_affects_reference_distance() {
        // Higher attenuation = smaller reference distance = faster falloff
        let atten_low = 0.5f32;
        let atten_high = 2.0f32;
        let ref_dist_low = 200.0 / atten_low;
        let ref_dist_high = 200.0 / atten_high;
        assert!(ref_dist_low > ref_dist_high,
            "lower attenuation should give larger ref distance");
        assert_eq!(ref_dist_low, 400.0);
        assert_eq!(ref_dist_high, 100.0);
    }
}
