# Fixes Summary: Slow Movement & Audio Stuttering

## Issues Fixed

### 1. Slow Movement at Map Start
**Problem:** Player character moves very slowly when starting a map, then speeds up to normal after a few seconds.

**Root Cause:** 
- Client used variable `frametime` (based on actual frame timing) for user command `msec` field
- Server uses fixed physics simulation at 10Hz (100ms frametime)
- This caused desynchronization between client prediction and server physics

**Solution:**
- Modified `cl_finish_move()` in `crates/myq2-client/src/cl_input.rs`
- Now uses physics frametime (`1.0 / cl_physics_fps`) instead of variable render frametime
- Default `cl_physics_fps` is 125.0 (8ms timestep), matching classic competitive Quake 2 settings
- This ensures consistent movement physics that matches server simulation

**Files Changed:**
- `crates/myq2-client/src/cl_input.rs`: Updated `cl_finish_move()` function
- `crates/myq2-client/src/cl_input.rs`: Updated tests to reflect new behavior

### 2. Audio Stuttering
**Problem:** Audio sounds stutter during playback, especially during cinematics.

**Root Cause:**
- OpenAL streaming buffer management had limited buffer count (4 buffers)
- Buffer queue logic only queued new data if queued count < STREAMING_BUFFER_COUNT
- If all buffers were queued and processing was slow, new audio data couldn't be queued

**Solution:**
- Increased `STREAMING_BUFFER_COUNT` from 4 to 16 in `crates/myq2-sys/src/snd_openal.rs`
- Improved buffer queue logic to force unqueue processed buffers when queue is full
- Added buffer underrun detection and recovery mechanism
- Added tracking for underrun count for debugging

**Files Changed:**
- `crates/myq2-sys/src/snd_openal.rs`: Increased buffer count
- `crates/myq2-sys/src/snd_openal.rs`: Improved queue_streaming_samples() logic
- `crates/myq2-sys/src/snd_openal.rs`: Added underrun detection and recovery

## Testing

All tests pass:
- `myq2-sys`: 207 tests passed
- `myq2-client`: 926 tests passed

## Build Status

✅ Release build successful
- Executable: `target/release/myq2-rust.exe`
- Size: 14,082,048 bytes

## Expected Results

After applying these fixes:
1. Player movement speed should be normal immediately when starting a map
2. No "slow start" behavior where character moves slowly then speeds up
3. Audio should play smoothly without stuttering or buffer underruns
4. Cinematic audio should be synchronized with video playback

## Performance Impact

- Minimal memory increase: +12 streaming buffers (16 total vs 4 previously)
- Positive: Smoother audio playback and more consistent movement
- No negative performance impact expected