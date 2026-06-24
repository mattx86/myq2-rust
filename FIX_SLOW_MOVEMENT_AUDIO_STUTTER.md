# Fix Plan: Slow Movement at Map Start & Audio Stuttering

## Issue Analysis

### 1. Slow Movement at Map Start
**Root Cause:**
- Game server uses fixed `FRAMETIME` of 0.1 seconds (100ms) for physics simulation
- Client uses variable frametime based on actual frame timing
- This causes desynchronization between client prediction and server physics
- Player movement commands are sent with variable timing, but server physics runs at fixed 10Hz

**Evidence:**
- `crates/myq2-game/src/g_local.rs:63`: `pub const FRAMETIME: f32 = 0.1;`
- `crates/myq2-game/src/g_main.rs:337`: `ctx.level.time = ctx.level.framenum as f32 * FRAMETIME;`
- `crates/myq2-server/src/sv_main.rs:1057`: Server timing uses `sv_frametime` (default 100ms)
- `crates/myq2-client/src/cl_timing.rs:138-144`: Client uses variable physics frametime

### 2. Audio Stuttering
**Root Cause:**
- OpenAL streaming buffer management may not handle rapid audio data arrival properly
- Buffer queue logic only adds buffers if `queued < STREAMING_BUFFER_COUNT`
- Potential for buffer underruns if audio data arrives faster than buffers are processed

**Evidence:**
- `crates/myq2-sys/src/snd_openal.rs:1009`: `if (queued as usize) < STREAMING_BUFFER_COUNT`
- `crates/myq2-sys/src/snd_openal.rs:243`: Only 4 streaming buffers available
- `crates/myq2-client/src/cl_cin.rs:362-363`: Cinematic audio reads at 14 FPS

## Fix Strategy

### Phase 1: Fix Slow Movement at Map Start

#### 1.1. Synchronize Client and Server Frametime
**File:** `crates/myq2-client/src/cl_input.rs`
**Changes:**
- Ensure client movement commands use consistent frametime matching server physics
- Use the same fixed frametime (0.1s) that the server uses for physics simulation

**Implementation:**
```rust
// In cl_create_cmd function, use consistent frametime
let physics_frametime = 0.1; // Match server FRAMETIME
let ms = (physics_frametime * 1000.0) as i32;
let ms = if ms > 250 { 100 } else { ms };
cmd.msec = ms as u8;
```

#### 1.2. Fix Player Initialization Velocity
**File:** `crates/myq2-game/src/p_client.rs`
**Changes:**
- Verify player velocity is properly initialized when spawning
- Ensure velocity is set to zero on map start

**Implementation:**
- Already sets `ctx.edicts[ent_idx].velocity = vec3_origin;` (line 1074)
- Add debug logging to verify velocity initialization

#### 1.3. Add Debug Logging for Movement
**File:** `crates/myq2-game/src/p_client.rs`
**Changes:**
- Add debug logging to track player velocity and movement commands
- Log frametime values used in physics simulation

### Phase 2: Fix Audio Stuttering

#### 2.1. Improve Streaming Buffer Management
**File:** `crates/myq2-sys/src/snd_openal.rs`
**Changes:**
- Add more streaming buffers (increase from 4 to 8 or 16)
- Improve buffer queue logic to handle rapid audio data arrival
- Add buffer underrun detection and recovery

**Implementation:**
```rust
// Increase streaming buffer count
const STREAMING_BUFFER_COUNT: usize = 16; // Was 4

// Improve queue logic to handle buffer exhaustion
if queued as usize >= STREAMING_BUFFER_COUNT {
    // Force unqueue processed buffers to make room
    let mut processed: al::ALint = 0;
    al::alGetSourcei(self.streaming_source, al::AL_BUFFERS_PROCESSED, &mut processed);
    while processed > 0 {
        let mut buffer: al::ALuint = 0;
        al::alSourceUnqueueBuffers(self.streaming_source, 1, &mut buffer);
        processed -= 1;
    }
}
```

#### 2.2. Add Buffer Underrun Detection
**File:** `crates/myq2-sys/src/snd_openal.rs`
**Changes:**
- Monitor buffer queue state and detect underruns
- Add recovery mechanism when buffer underruns are detected

#### 2.3. Improve Cinematic Audio Timing
**File:** `crates/myq2-client/src/cl_cin.rs`
**Changes:**
- Ensure cinematic audio streaming is called at consistent intervals
- Add buffering to smooth out audio data delivery

### Phase 3: Testing and Verification

#### 3.1. Test Slow Movement Fix
- Start a new map and verify player movement speed is normal immediately
- Check that velocity is consistent between client and server
- Verify no desynchronization between prediction and physics

#### 3.2. Test Audio Stuttering Fix
- Play cinematic audio and verify no stuttering
- Monitor buffer queue state and ensure no underruns
- Test with various audio sample rates and formats

## Files to Modify

### High Priority
1. `crates/myq2-client/src/cl_input.rs` - Fix frametime consistency
2. `crates/myq2-sys/src/snd_openal.rs` - Improve streaming buffer management
3. `crates/myq2-game/src/p_client.rs` - Add debug logging for movement

### Medium Priority
4. `crates/myq2-client/src/cl_cin.rs` - Improve cinematic audio timing
5. `crates/myq2-client/src/snd_dma.rs` - Add buffer underrun detection

## Testing Plan

### Movement Testing
1. Start a new single-player game
2. Verify immediate normal movement speed
3. Check that strafe jumping works correctly
4. Verify server and client physics are synchronized

### Audio Testing
1. Play intro cinematic and verify no audio stuttering
2. Test various sound effects during gameplay
3. Monitor OpenAL buffer queue state
4. Verify audio quality at different sample rates

## Expected Results

### After Fix
- Player movement speed should be normal immediately when starting a map
- No "slow start" behavior where character moves slowly then speeds up
- Audio should play smoothly without stuttering or buffer underruns
- Cinematic audio should be synchronized with video playback

### Performance Impact
- Minimal: Increased streaming buffers may use slightly more memory
- Positive: Smoother audio playback and more consistent movement