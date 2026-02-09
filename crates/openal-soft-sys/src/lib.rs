//! Low-level FFI bindings to OpenAL Soft, built from source.
//!
//! This crate builds OpenAL Soft 1.25.1 from source via CMake and exposes
//! the C API as Rust FFI functions. Only the functions needed by the engine
//! are declared here — extend as needed.

#![allow(non_camel_case_types, non_upper_case_globals)]

use std::ffi::c_void;

// ============================================================================
// Types
// ============================================================================

pub type ALboolean = i8;
pub type ALchar = i8;
pub type ALint = i32;
pub type ALuint = u32;
pub type ALfloat = f32;
pub type ALenum = i32;
pub type ALsizei = i32;

/// Opaque device handle.
pub enum ALCdevice {}
/// Opaque context handle.
pub enum ALCcontext {}

pub type ALCboolean = i8;
pub type ALCchar = i8;
pub type ALCint = i32;
pub type ALCenum = i32;
pub type ALCsizei = i32;

// ============================================================================
// AL constants
// ============================================================================

pub const AL_NONE: ALenum = 0;
pub const AL_FALSE: ALboolean = 0;
pub const AL_TRUE: ALboolean = 1;
pub const AL_NO_ERROR: ALenum = 0;

// Source properties
pub const AL_PITCH: ALenum = 0x1003;
pub const AL_POSITION: ALenum = 0x1004;
pub const AL_DIRECTION: ALenum = 0x1005;
pub const AL_VELOCITY: ALenum = 0x1006;
pub const AL_LOOPING: ALenum = 0x1007;
pub const AL_BUFFER: ALenum = 0x1009;
pub const AL_GAIN: ALenum = 0x100A;
pub const AL_SOURCE_STATE: ALenum = 0x1010;
pub const AL_PLAYING: ALenum = 0x1012;
pub const AL_PAUSED: ALenum = 0x1013;
pub const AL_STOPPED: ALenum = 0x1014;
pub const AL_BUFFERS_QUEUED: ALenum = 0x1015;
pub const AL_BUFFERS_PROCESSED: ALenum = 0x1016;
pub const AL_SOURCE_RELATIVE: ALenum = 0x0202;
pub const AL_REFERENCE_DISTANCE: ALenum = 0x1020;
pub const AL_ROLLOFF_FACTOR: ALenum = 0x1021;
pub const AL_MAX_DISTANCE: ALenum = 0x1023;

// Listener properties
pub const AL_ORIENTATION: ALenum = 0x100F;

// Buffer formats
pub const AL_FORMAT_MONO8: ALenum = 0x1100;
pub const AL_FORMAT_MONO16: ALenum = 0x1101;
pub const AL_FORMAT_STEREO8: ALenum = 0x1102;
pub const AL_FORMAT_STEREO16: ALenum = 0x1103;

// Distance models
pub const AL_INVERSE_DISTANCE_CLAMPED: ALenum = 0xD002;

// Doppler
pub const AL_SPEED_OF_SOUND: ALenum = 0xC003;

// ============================================================================
// EFX extension constants
// ============================================================================

// Effect types
pub const AL_EFFECT_TYPE: ALenum = 0x8001;
pub const AL_EFFECT_REVERB: ALenum = 0x0001;

// Reverb parameters
pub const AL_REVERB_DENSITY: ALenum = 0x0001;
pub const AL_REVERB_DIFFUSION: ALenum = 0x0002;
pub const AL_REVERB_GAIN: ALenum = 0x0003;
pub const AL_REVERB_GAINHF: ALenum = 0x0004;
pub const AL_REVERB_DECAY_TIME: ALenum = 0x0005;
pub const AL_REVERB_DECAY_HFRATIO: ALenum = 0x0006;
pub const AL_REVERB_REFLECTIONS_GAIN: ALenum = 0x0007;
pub const AL_REVERB_REFLECTIONS_DELAY: ALenum = 0x0008;
pub const AL_REVERB_LATE_REVERB_GAIN: ALenum = 0x0009;
pub const AL_REVERB_LATE_REVERB_DELAY: ALenum = 0x000A;
pub const AL_REVERB_AIR_ABSORPTION_GAINHF: ALenum = 0x000B;
pub const AL_REVERB_ROOM_ROLLOFF_FACTOR: ALenum = 0x000C;
pub const AL_REVERB_DECAY_HFLIMIT: ALenum = 0x000D;

// Filter types
pub const AL_FILTER_TYPE: ALenum = 0x8001;
pub const AL_FILTER_NULL: ALenum = 0x0000;
pub const AL_FILTER_LOWPASS: ALenum = 0x0001;
pub const AL_LOWPASS_GAIN: ALenum = 0x0001;
pub const AL_LOWPASS_GAINHF: ALenum = 0x0002;

// Auxiliary effect slot
pub const AL_EFFECTSLOT_EFFECT: ALenum = 0x0001;
pub const AL_EFFECTSLOT_GAIN: ALenum = 0x0002;
pub const AL_EFFECTSLOT_AUXILIARY_SEND_AUTO: ALenum = 0x0003;

// Source aux send filter
pub const AL_AUXILIARY_SEND_FILTER: ALenum = 0x20006;

// ============================================================================
// ALC constants
// ============================================================================

pub const ALC_FALSE: ALCint = 0;
pub const ALC_TRUE: ALCint = 1;
pub const ALC_FREQUENCY: ALCenum = 0x1007;
pub const ALC_NO_ERROR: ALCenum = 0;

// HRTF extension (ALC_SOFT_HRTF)
pub const ALC_HRTF_SOFT: ALCenum = 0x1992;
pub const ALC_HRTF_STATUS_SOFT: ALCenum = 0x1993;
pub const ALC_HRTF_DISABLED_SOFT: ALCenum = 0x0000;
pub const ALC_HRTF_ENABLED_SOFT: ALCenum = 0x0001;
pub const ALC_HRTF_DENIED_SOFT: ALCenum = 0x0002;
pub const ALC_HRTF_REQUIRED_SOFT: ALCenum = 0x0003;
pub const ALC_HRTF_HEADPHONES_DETECTED_SOFT: ALCenum = 0x0004;
pub const ALC_HRTF_UNSUPPORTED_FORMAT_SOFT: ALCenum = 0x0005;

extern "C" {
    // ========================================================================
    // Core AL functions
    // ========================================================================

    pub fn alGetError() -> ALenum;
    pub fn alDistanceModel(model: ALenum);

    // Sources
    pub fn alGenSources(n: ALsizei, sources: *mut ALuint);
    pub fn alDeleteSources(n: ALsizei, sources: *const ALuint);
    pub fn alSourcei(source: ALuint, param: ALenum, value: ALint);
    pub fn alSourcef(source: ALuint, param: ALenum, value: ALfloat);
    pub fn alSource3f(
        source: ALuint,
        param: ALenum,
        v1: ALfloat,
        v2: ALfloat,
        v3: ALfloat,
    );
    pub fn alGetSourcei(source: ALuint, param: ALenum, value: *mut ALint);
    pub fn alSourcePlay(source: ALuint);
    pub fn alSourceStop(source: ALuint);
    pub fn alSourcePause(source: ALuint);

    // Buffers
    pub fn alGenBuffers(n: ALsizei, buffers: *mut ALuint);
    pub fn alDeleteBuffers(n: ALsizei, buffers: *const ALuint);
    pub fn alBufferData(
        buffer: ALuint,
        format: ALenum,
        data: *const c_void,
        size: ALsizei,
        freq: ALsizei,
    );

    // Buffer queue (for streaming audio)
    pub fn alSourceQueueBuffers(source: ALuint, n: ALsizei, buffers: *const ALuint);
    pub fn alSourceUnqueueBuffers(source: ALuint, n: ALsizei, buffers: *mut ALuint);

    // Listener
    pub fn alListenerf(param: ALenum, value: ALfloat);
    pub fn alListener3f(param: ALenum, v1: ALfloat, v2: ALfloat, v3: ALfloat);
    pub fn alListenerfv(param: ALenum, values: *const ALfloat);

    // ========================================================================
    // ALC (context/device) functions
    // ========================================================================

    pub fn alcOpenDevice(devicename: *const ALCchar) -> *mut ALCdevice;
    pub fn alcCloseDevice(device: *mut ALCdevice) -> ALCboolean;
    pub fn alcCreateContext(
        device: *mut ALCdevice,
        attrlist: *const ALCint,
    ) -> *mut ALCcontext;
    pub fn alcDestroyContext(context: *mut ALCcontext);
    pub fn alcMakeContextCurrent(context: *mut ALCcontext) -> ALCboolean;
    pub fn alcGetIntegerv(
        device: *mut ALCdevice,
        param: ALCenum,
        size: ALCsizei,
        values: *mut ALCint,
    );
    pub fn alcGetError(device: *mut ALCdevice) -> ALCenum;
    pub fn alcGetString(device: *mut ALCdevice, param: ALCenum) -> *const ALCchar;

    // ========================================================================
    // Doppler functions
    // ========================================================================

    pub fn alDopplerFactor(value: ALfloat);
    pub fn alSpeedOfSound(value: ALfloat);

    // ========================================================================
    // EFX extension functions
    // ========================================================================

    // Effects
    pub fn alGenEffects(n: ALsizei, effects: *mut ALuint);
    pub fn alDeleteEffects(n: ALsizei, effects: *const ALuint);
    pub fn alEffecti(effect: ALuint, param: ALenum, value: ALint);
    pub fn alEffectf(effect: ALuint, param: ALenum, value: ALfloat);

    // Filters
    pub fn alGenFilters(n: ALsizei, filters: *mut ALuint);
    pub fn alDeleteFilters(n: ALsizei, filters: *const ALuint);
    pub fn alFilteri(filter: ALuint, param: ALenum, value: ALint);
    pub fn alFilterf(filter: ALuint, param: ALenum, value: ALfloat);

    // Auxiliary effect slots
    pub fn alGenAuxiliaryEffectSlots(n: ALsizei, slots: *mut ALuint);
    pub fn alDeleteAuxiliaryEffectSlots(n: ALsizei, slots: *const ALuint);
    pub fn alAuxiliaryEffectSloti(slot: ALuint, param: ALenum, value: ALint);
    pub fn alAuxiliaryEffectSlotf(slot: ALuint, param: ALenum, value: ALfloat);

    // Source aux send (uses alSource3i which takes 3 ints)
    pub fn alSource3i(
        source: ALuint,
        param: ALenum,
        v1: ALint,
        v2: ALint,
        v3: ALint,
    );
}
