// cl_demo.rs -- Enhanced demo playback with seeking, pause, and speed control
//
// R1Q2/Q2Pro-style demo enhancements:
// - Demo seeking via keyframe index
// - Pause/resume playback
// - Variable playback speed (0.25x - 4.0x)
// - Demo info display (duration, current time, frame count)

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::sync::{LazyLock, Mutex};

use myq2_common::q_shared::{EntityState, MAX_CONFIGSTRINGS, MAX_EDICTS};
use myq2_common::common::{com_printf, msg_read_byte, msg_read_short, msg_read_long, msg_read_string};
use myq2_common::qcommon::SizeBuf;
use myq2_common::qcommon::{
    SVC_SERVERDATA, SVC_CONFIGSTRING, SVC_SPAWNBASELINE, SVC_FRAME,
    SVC_SOUND, SVC_PRINT, SVC_STUFFTEXT, SVC_LAYOUT, SVC_INVENTORY,
    SVC_TEMP_ENTITY, SVC_MUZZLEFLASH, SVC_MUZZLEFLASH2, SVC_CENTERPRINT,
    SVC_NOP, SVC_DISCONNECT, SVC_RECONNECT, SVC_DOWNLOAD, SVC_PLAYERINFO,
    SVC_PACKETENTITIES, SVC_DELTAPACKETENTITIES,
};

/// Maximum playback speed multiplier
pub const MAX_DEMO_SPEED: f32 = 4.0;
/// Minimum playback speed multiplier
pub const MIN_DEMO_SPEED: f32 = 0.25;
/// Interval between keyframes in server frames (~5 seconds at 10fps)
pub const KEYFRAME_INTERVAL: u32 = 50;

/// A keyframe snapshot for seeking within demos.
/// Stores enough state to resume playback from this point.
#[derive(Clone)]
pub struct DemoKeyframe {
    /// Server time at this keyframe
    pub servertime: i32,
    /// File offset to start reading from
    pub file_offset: u64,
    /// Frame number in the demo
    pub frame_number: u32,
    /// Snapshot of configstrings that have changed up to this point
    pub configstrings: HashMap<i32, String>,
    /// Entity baselines at this keyframe
    pub baselines: Vec<EntityState>,
}

impl Default for DemoKeyframe {
    fn default() -> Self {
        Self {
            servertime: 0,
            file_offset: 0,
            frame_number: 0,
            configstrings: HashMap::new(),
            baselines: Vec::new(),
        }
    }
}

/// Index of keyframes for a demo file, enabling fast seeking.
pub struct DemoIndex {
    /// List of keyframes at regular intervals
    pub keyframes: Vec<DemoKeyframe>,
    /// Total duration in seconds
    pub duration: f32,
    /// Total number of frames
    pub total_frames: u32,
    /// File size in bytes
    pub file_size: u64,
}

impl DemoIndex {
    pub fn new() -> Self {
        Self {
            keyframes: Vec::new(),
            duration: 0.0,
            total_frames: 0,
            file_size: 0,
        }
    }

    /// Find the keyframe nearest to (but not after) the given time.
    pub fn find_keyframe_for_time(&self, time_seconds: f32) -> Option<&DemoKeyframe> {
        if self.keyframes.is_empty() {
            return None;
        }

        // Convert to server time (assuming 10fps = 100ms per frame)
        let target_servertime = (time_seconds * 1000.0) as i32;

        // Binary search for the nearest keyframe before or at the target time
        let mut best_idx = 0;
        for (i, kf) in self.keyframes.iter().enumerate() {
            if kf.servertime <= target_servertime {
                best_idx = i;
            } else {
                break;
            }
        }

        self.keyframes.get(best_idx)
    }

    /// Find the keyframe nearest to the given percentage (0.0 - 1.0).
    pub fn find_keyframe_for_percent(&self, percent: f32) -> Option<&DemoKeyframe> {
        let time = self.duration * percent.clamp(0.0, 1.0);
        self.find_keyframe_for_time(time)
    }
}

impl Default for DemoIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Demo playback state for enhanced features.
pub struct DemoPlayback {
    /// True if demo playback is paused
    pub paused: bool,
    /// Playback speed multiplier (1.0 = normal, 0.5 = half, 2.0 = double)
    pub speed: f32,
    /// Time accumulator for frame timing at variable speed
    pub time_accumulator: f32,
    /// Current keyframe index (for seeking)
    pub current_keyframe: usize,
    /// Demo index for seeking
    pub index: Option<DemoIndex>,
    /// Demo file reader (if playing a demo)
    pub reader: Option<BufReader<File>>,
    /// Current server time in the demo
    pub current_time: i32,
    /// True if we're currently playing a demo
    pub playing: bool,
}

impl DemoPlayback {
    pub fn new() -> Self {
        Self {
            paused: false,
            speed: 1.0,
            time_accumulator: 0.0,
            current_keyframe: 0,
            index: None,
            reader: None,
            current_time: 0,
            playing: false,
        }
    }

    /// Start playing a demo from the given file path.
    pub fn start(&mut self, path: &str) -> Result<(), String> {
        // Build the keyframe index first (this scans the file)
        let index = build_demo_index(path)?;

        // Re-open file for playback
        let file = File::open(path)
            .map_err(|e| format!("Failed to open demo: {}", e))?;

        self.reader = Some(BufReader::new(file));
        self.playing = true;
        self.paused = false;
        self.speed = 1.0;
        self.time_accumulator = 0.0;
        self.current_time = 0;
        self.current_keyframe = 0;
        self.index = Some(index);

        Ok(())
    }

    /// Stop demo playback.
    pub fn stop(&mut self) {
        self.reader = None;
        self.playing = false;
        self.paused = false;
        self.index = None;
        self.current_time = 0;
    }

    /// Toggle pause state.
    pub fn toggle_pause(&mut self) {
        if self.playing {
            self.paused = !self.paused;
        }
    }

    /// Set playback speed, clamped to valid range.
    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed.clamp(MIN_DEMO_SPEED, MAX_DEMO_SPEED);
    }

    /// Seek to a specific time in seconds.
    pub fn seek_to_time(&mut self, time_seconds: f32) -> bool {
        if let Some(ref index) = self.index {
            if let Some(keyframe) = index.find_keyframe_for_time(time_seconds) {
                return self.seek_to_keyframe(keyframe.file_offset, keyframe.servertime);
            }
        }
        false
    }

    /// Seek by a relative number of seconds (+/-).
    pub fn seek_relative(&mut self, delta_seconds: f32) -> bool {
        let current_seconds = self.current_time as f32 / 1000.0;
        let target_seconds = (current_seconds + delta_seconds).max(0.0);
        self.seek_to_time(target_seconds)
    }

    /// Seek to a percentage of the demo (0-100).
    pub fn seek_to_percent(&mut self, percent: f32) -> bool {
        if let Some(ref index) = self.index {
            if let Some(keyframe) = index.find_keyframe_for_percent(percent / 100.0) {
                return self.seek_to_keyframe(keyframe.file_offset, keyframe.servertime);
            }
        }
        false
    }

    /// Internal: seek to a specific file offset.
    fn seek_to_keyframe(&mut self, offset: u64, servertime: i32) -> bool {
        if let Some(ref mut reader) = self.reader {
            if reader.seek(SeekFrom::Start(offset)).is_ok() {
                self.current_time = servertime;
                self.time_accumulator = 0.0;
                return true;
            }
        }
        false
    }

    /// Get current playback position as a percentage.
    pub fn get_percent(&self) -> f32 {
        if let Some(ref index) = self.index {
            if index.duration > 0.0 {
                return (self.current_time as f32 / 1000.0 / index.duration * 100.0).clamp(0.0, 100.0);
            }
        }
        0.0
    }

    /// Get current playback time in seconds.
    pub fn get_time_seconds(&self) -> f32 {
        self.current_time as f32 / 1000.0
    }

    /// Get duration in seconds.
    pub fn get_duration(&self) -> f32 {
        self.index.as_ref().map(|i| i.duration).unwrap_or(0.0)
    }

    /// Check if a frame should be processed based on speed and accumulator.
    pub fn should_process_frame(&mut self, frame_msec: i32) -> bool {
        if self.paused {
            return false;
        }

        // Accumulate time scaled by playback speed
        self.time_accumulator += frame_msec as f32 * self.speed;

        // Process frame if we've accumulated enough time
        // (at 1.0x speed, this is 1:1; at 2.0x, we process twice as fast)
        if self.time_accumulator >= frame_msec as f32 {
            self.time_accumulator -= frame_msec as f32;
            return true;
        }

        false
    }
}

impl Default for DemoPlayback {
    fn default() -> Self {
        Self::new()
    }
}

/// Global demo playback state
pub static DEMO_PLAYBACK: LazyLock<Mutex<DemoPlayback>> =
    LazyLock::new(|| Mutex::new(DemoPlayback::new()));

// ============================================================
// Console Commands
// ============================================================

/// Parse a time string like "1:30" or "90" into seconds.
fn parse_time_string(s: &str) -> Option<f32> {
    // Check for minute:second format
    if let Some(colon_pos) = s.find(':') {
        let minutes: f32 = s[..colon_pos].parse().ok()?;
        let seconds: f32 = s[colon_pos + 1..].parse().ok()?;
        return Some(minutes * 60.0 + seconds);
    }

    // Check for relative seek (+10, -5)
    if s.starts_with('+') || s.starts_with('-') {
        return s.parse().ok();
    }

    // Plain seconds
    s.parse().ok()
}

/// seek <time> - Jump to time (e.g., "seek 1:30", "seek 90", "seek +10")
pub fn cmd_seek(args: &str) {
    let mut playback = DEMO_PLAYBACK.lock().unwrap();

    if !playback.playing {
        com_printf("Not playing a demo.\n");
        return;
    }

    let time_str = args.trim();
    if time_str.is_empty() {
        com_printf("Usage: seek <time> (e.g., seek 1:30, seek 90, seek +10)\n");
        return;
    }

    // Check for relative seek
    if time_str.starts_with('+') || time_str.starts_with('-') {
        if let Some(delta) = parse_time_string(time_str) {
            if playback.seek_relative(delta) {
                com_printf(&format!("Seeked to {:.1}s\n", playback.get_time_seconds()));
            } else {
                com_printf("Failed to seek.\n");
            }
        } else {
            com_printf("Invalid time format.\n");
        }
    } else {
        // Absolute seek
        if let Some(time) = parse_time_string(time_str) {
            if playback.seek_to_time(time) {
                com_printf(&format!("Seeked to {:.1}s\n", playback.get_time_seconds()));
            } else {
                com_printf("Failed to seek.\n");
            }
        } else {
            com_printf("Invalid time format.\n");
        }
    }
}

/// seekpercent <0-100> - Jump to percentage of demo
pub fn cmd_seekpercent(args: &str) {
    let mut playback = DEMO_PLAYBACK.lock().unwrap();

    if !playback.playing {
        com_printf("Not playing a demo.\n");
        return;
    }

    let percent_str = args.trim();
    if let Ok(percent) = percent_str.parse::<f32>() {
        if playback.seek_to_percent(percent) {
            com_printf(&format!("Seeked to {:.1}%\n", playback.get_percent()));
        } else {
            com_printf("Failed to seek.\n");
        }
    } else {
        com_printf("Usage: seekpercent <0-100>\n");
    }
}

/// demo_pause / demo_resume - Toggle pause
pub fn cmd_demo_pause() {
    let mut playback = DEMO_PLAYBACK.lock().unwrap();

    if !playback.playing {
        com_printf("Not playing a demo.\n");
        return;
    }

    playback.toggle_pause();
    if playback.paused {
        com_printf("Demo paused.\n");
    } else {
        com_printf("Demo resumed.\n");
    }
}

/// demo_speed <0.25-4.0> - Set playback speed
pub fn cmd_demo_speed(args: &str) {
    let mut playback = DEMO_PLAYBACK.lock().unwrap();

    if !playback.playing {
        com_printf("Not playing a demo.\n");
        return;
    }

    let speed_str = args.trim();
    if let Ok(speed) = speed_str.parse::<f32>() {
        playback.set_speed(speed);
        com_printf(&format!("Demo speed set to {:.2}x\n", playback.speed));
    } else {
        com_printf(&format!("Usage: demo_speed <{}-{}>\n", MIN_DEMO_SPEED, MAX_DEMO_SPEED));
    }
}

/// demo_info - Show demo information
pub fn cmd_demo_info() {
    let playback = DEMO_PLAYBACK.lock().unwrap();

    if !playback.playing {
        com_printf("Not playing a demo.\n");
        return;
    }

    let current = playback.get_time_seconds();
    let duration = playback.get_duration();
    let percent = playback.get_percent();

    com_printf(&format!(
        "Demo Info:\n  Time: {:.1}s / {:.1}s ({:.1}%)\n  Speed: {:.2}x\n  Paused: {}\n",
        current,
        duration,
        percent,
        playback.speed,
        if playback.paused { "Yes" } else { "No" }
    ));

    if let Some(ref index) = playback.index {
        com_printf(&format!(
            "  Keyframes: {}\n  Total frames: {}\n",
            index.keyframes.len(),
            index.total_frames
        ));
    }
}

// ============================================================
// Demo Indexing
// ============================================================

/// State accumulated while scanning a demo for indexing.
struct DemoScanState {
    /// All configstrings accumulated so far
    configstrings: HashMap<i32, String>,
    /// All entity baselines accumulated so far
    baselines: Vec<EntityState>,
    /// Current servertime (from SVC_FRAME)
    servertime: i32,
    /// Frame counter
    frame_count: u32,
    /// Last servertime we created a keyframe at
    last_keyframe_time: i32,
}

impl DemoScanState {
    fn new() -> Self {
        Self {
            configstrings: HashMap::new(),
            baselines: vec![EntityState::default(); MAX_EDICTS],
            servertime: 0,
            frame_count: 0,
            last_keyframe_time: i32::MIN,
        }
    }

    /// Create a keyframe snapshot from current state.
    fn create_keyframe(&self, file_offset: u64) -> DemoKeyframe {
        DemoKeyframe {
            servertime: self.servertime,
            file_offset,
            frame_number: self.frame_count,
            configstrings: self.configstrings.clone(),
            baselines: self.baselines.iter()
                .filter(|e| e.modelindex != 0)
                .cloned()
                .collect(),
        }
    }
}

/// Skip over variable-length data in a message buffer without fully parsing it.
/// This is used to skip messages we don't care about for indexing.
fn skip_demo_message(cmd: i32, msg: &mut SizeBuf) {
    match cmd {
        x if x == SVC_NOP => {}
        x if x == SVC_DISCONNECT || x == SVC_RECONNECT => {}

        x if x == SVC_PRINT => {
            let _ = msg_read_byte(msg); // level
            let _ = msg_read_string(msg);
        }

        x if x == SVC_CENTERPRINT || x == SVC_STUFFTEXT || x == SVC_LAYOUT => {
            let _ = msg_read_string(msg);
        }

        x if x == SVC_SOUND => {
            let flags = msg_read_byte(msg);
            let _ = msg_read_byte(msg); // sound_num
            if flags & 1 != 0 { let _ = msg_read_byte(msg); } // volume
            if flags & 2 != 0 { let _ = msg_read_byte(msg); } // attenuation
            if flags & 8 != 0 { let _ = msg_read_byte(msg); } // offset
            if flags & 4 != 0 { let _ = msg_read_short(msg); } // ent+channel
            if flags & 16 != 0 {
                let _ = msg_read_short(msg); // pos x
                let _ = msg_read_short(msg); // pos y
                let _ = msg_read_short(msg); // pos z
            }
        }

        x if x == SVC_TEMP_ENTITY => {
            // TempEntity is complex; skip to end of message for safety
            // In practice, we just let it run off the end which is safe
            // since we're scanning, not playing back
            msg.readcount = msg.cursize;
        }

        x if x == SVC_MUZZLEFLASH || x == SVC_MUZZLEFLASH2 => {
            let _ = msg_read_short(msg); // entity
            let _ = msg_read_byte(msg); // weapon/flash
        }

        x if x == SVC_DOWNLOAD => {
            let size = msg_read_short(msg);
            let _ = msg_read_byte(msg); // percent
            if size > 0 {
                msg.readcount += size;
            }
        }

        x if x == SVC_INVENTORY => {
            // 256 shorts
            for _ in 0..256 {
                let _ = msg_read_short(msg);
            }
        }

        _ => {
            // Unknown command - can't skip safely, abort message
            msg.readcount = msg.cursize;
        }
    }
}

/// Parse SVC_SERVERDATA message for demo indexing.
fn parse_demo_serverdata(msg: &mut SizeBuf, state: &mut DemoScanState) {
    let _protocol = msg_read_long(msg);
    let _servercount = msg_read_long(msg);
    let _attractloop = msg_read_byte(msg);
    let _gamedir = msg_read_string(msg);
    let _playernum = msg_read_short(msg);
    let mapname = msg_read_string(msg);

    // Reset state for new map
    state.configstrings.clear();
    state.baselines = vec![EntityState::default(); MAX_EDICTS];
    state.servertime = 0;
    state.frame_count = 0;

    // Store mapname as CS_NAME (configstring 0)
    state.configstrings.insert(0, mapname);
}

/// Parse SVC_CONFIGSTRING message for demo indexing.
fn parse_demo_configstring(msg: &mut SizeBuf, state: &mut DemoScanState) {
    let i = msg_read_short(msg);
    let s = msg_read_string(msg);

    if i >= 0 && (i as usize) < MAX_CONFIGSTRINGS {
        state.configstrings.insert(i, s);
    }
}

/// Parse SVC_SPAWNBASELINE message for demo indexing.
/// Returns the entity number if successfully parsed.
fn parse_demo_baseline(msg: &mut SizeBuf, state: &mut DemoScanState) {
    // Parse the entity state delta
    // Format: entity number (with bits), then delta fields based on bits

    // Read entity number with bits
    let mut bits = msg_read_byte(msg) as u32;
    if bits & 0x80 != 0 {
        bits |= (msg_read_byte(msg) as u32) << 8;
    }

    let number = if bits & 0x100 != 0 {
        msg_read_short(msg) as usize
    } else {
        msg_read_byte(msg) as usize
    };

    if number >= MAX_EDICTS {
        return;
    }

    // Parse delta fields - simplified, just skip the right number of bytes
    // This matches the U_* flags from q_shared
    let ent = &mut state.baselines[number];
    ent.number = number as i32;

    if bits & 0x0001 != 0 { ent.origin[0] = msg_read_short(msg) as f32 / 8.0; }
    if bits & 0x0002 != 0 { ent.origin[1] = msg_read_short(msg) as f32 / 8.0; }
    if bits & 0x0200 != 0 { ent.origin[2] = msg_read_short(msg) as f32 / 8.0; }
    if bits & 0x0400 != 0 { ent.angles[0] = msg_read_byte(msg) as f32 * (360.0 / 256.0); }
    if bits & 0x0004 != 0 { ent.angles[1] = msg_read_byte(msg) as f32 * (360.0 / 256.0); }
    if bits & 0x0800 != 0 { ent.angles[2] = msg_read_byte(msg) as f32 * (360.0 / 256.0); }
    if bits & 0x1000 != 0 { ent.old_origin[0] = msg_read_short(msg) as f32 / 8.0; }
    if bits & 0x2000 != 0 { ent.old_origin[1] = msg_read_short(msg) as f32 / 8.0; }
    if bits & 0x4000 != 0 { ent.old_origin[2] = msg_read_short(msg) as f32 / 8.0; }
    if bits & 0x0008 != 0 { ent.modelindex = msg_read_byte(msg) as i32; }
    if bits & 0x10000 != 0 { ent.modelindex2 = msg_read_byte(msg) as i32; }
    if bits & 0x20000 != 0 { ent.modelindex3 = msg_read_byte(msg) as i32; }
    if bits & 0x40000 != 0 { ent.modelindex4 = msg_read_byte(msg) as i32; }
    if bits & 0x0010 != 0 { ent.frame = msg_read_byte(msg) as i32; }
    if bits & 0x80000 != 0 { ent.frame = (ent.frame & 0xFF) | ((msg_read_byte(msg) as i32) << 8); }
    if bits & 0x0020 != 0 { ent.skinnum = msg_read_byte(msg) as i32; }
    if bits & 0x100000 != 0 { ent.skinnum = (ent.skinnum & 0xFF) | ((msg_read_byte(msg) as i32) << 8); }
    if bits & 0x0040 != 0 { ent.effects = msg_read_byte(msg) as u32; }
    if bits & 0x200000 != 0 { ent.effects = (ent.effects & 0xFF) | ((msg_read_byte(msg) as u32) << 8); }
    if bits & 0x0080 != 0 { ent.renderfx = msg_read_byte(msg) as i32; }
    if bits & 0x400000 != 0 { ent.renderfx = (ent.renderfx & 0xFF) | ((msg_read_byte(msg) as i32) << 8); }
    if bits & 0x8000 != 0 { ent.solid = msg_read_short(msg) as i32; }
    if bits & 0x800000 != 0 { ent.event = msg_read_byte(msg) as i32; }
    if bits & 0x1000000 != 0 { ent.sound = msg_read_byte(msg) as i32; }
}

/// Parse SVC_FRAME header to get servertime.
fn parse_demo_frame_header(msg: &mut SizeBuf) -> i32 {
    let serverframe = msg_read_long(msg);
    let _deltaframe = msg_read_long(msg);
    let _suppres = msg_read_byte(msg);

    // Skip areabits
    let areabytes = msg_read_byte(msg);
    msg.readcount += areabytes;

    // Return servertime (frame * 100ms for 10fps server)
    serverframe * 100
}

/// Build a keyframe index for a demo file.
/// This scans the demo file and creates snapshots at regular intervals.
pub fn build_demo_index(path: &str) -> Result<DemoIndex, String> {
    let file = File::open(path)
        .map_err(|e| format!("Failed to open demo: {}", e))?;

    let file_size = file.metadata()
        .map(|m| m.len())
        .unwrap_or(0);

    let mut reader = BufReader::new(file);
    let mut index = DemoIndex::new();
    index.file_size = file_size;

    let mut state = DemoScanState::new();
    let mut msg_buffer = vec![0u8; 0x10000]; // 64KB buffer for messages
    let mut first_servertime = 0i32;
    let mut last_servertime = 0i32;

    // Create initial keyframe at file start
    index.keyframes.push(DemoKeyframe::default());

    loop {
        // Remember position before reading this message block
        let block_offset = reader.stream_position().unwrap_or(0);

        // Read message length (4 bytes, little endian)
        let mut len_bytes = [0u8; 4];
        if reader.read_exact(&mut len_bytes).is_err() {
            break; // End of file
        }
        let msg_len = i32::from_le_bytes(len_bytes);

        if msg_len == -1 {
            break; // End of demo marker
        }

        if msg_len <= 0 || msg_len > 0x10000 {
            break; // Invalid length
        }

        // Read message data
        let msg_len_usize = msg_len as usize;
        if reader.read_exact(&mut msg_buffer[..msg_len_usize]).is_err() {
            break;
        }

        // Parse the message
        let mut msg = SizeBuf::new(msg_len);
        msg.data[..msg_len_usize].copy_from_slice(&msg_buffer[..msg_len_usize]);
        msg.cursize = msg_len;

        while msg.readcount < msg.cursize {
            let cmd = msg_read_byte(&mut msg);

            if cmd == -1 {
                break; // End of message
            }

            match cmd {
                x if x == SVC_SERVERDATA => {
                    parse_demo_serverdata(&mut msg, &mut state);
                    // Create a keyframe at serverdata
                    let kf = state.create_keyframe(block_offset);
                    if index.keyframes.is_empty() || index.keyframes[0].configstrings.is_empty() {
                        index.keyframes[0] = kf;
                    }
                }

                x if x == SVC_CONFIGSTRING => {
                    parse_demo_configstring(&mut msg, &mut state);
                }

                x if x == SVC_SPAWNBASELINE => {
                    parse_demo_baseline(&mut msg, &mut state);
                }

                x if x == SVC_FRAME => {
                    state.servertime = parse_demo_frame_header(&mut msg);
                    state.frame_count += 1;

                    if first_servertime == 0 && state.servertime > 0 {
                        first_servertime = state.servertime;
                    }
                    last_servertime = state.servertime;

                    // Create keyframe every KEYFRAME_INTERVAL frames (or ~5 seconds)
                    let time_since_keyframe = state.servertime - state.last_keyframe_time;
                    if time_since_keyframe >= 5000 {
                        // ~5 seconds
                        let kf = state.create_keyframe(block_offset);
                        index.keyframes.push(kf);
                        state.last_keyframe_time = state.servertime;
                    }

                    // Skip the rest of the frame (playerinfo, entities)
                    msg.readcount = msg.cursize;
                }

                x if x == SVC_PLAYERINFO || x == SVC_PACKETENTITIES || x == SVC_DELTAPACKETENTITIES => {
                    // These are complex to parse; skip to end of message
                    msg.readcount = msg.cursize;
                }

                _ => {
                    skip_demo_message(cmd, &mut msg);
                }
            }
        }
    }

    // Calculate duration
    if last_servertime > first_servertime {
        index.duration = (last_servertime - first_servertime) as f32 / 1000.0;
    } else {
        // Fallback estimate
        index.duration = (file_size as f32 / 10000.0).max(1.0);
    }
    index.total_frames = state.frame_count;

    com_printf(&format!(
        "Demo indexed: {:.1}s, {} frames, {} keyframes\n",
        index.duration,
        index.total_frames,
        index.keyframes.len()
    ));

    Ok(index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_time_string() {
        assert_eq!(parse_time_string("90"), Some(90.0));
        assert_eq!(parse_time_string("1:30"), Some(90.0));
        assert_eq!(parse_time_string("+10"), Some(10.0));
        assert_eq!(parse_time_string("-5"), Some(-5.0));
        assert_eq!(parse_time_string("invalid"), None);
    }

    #[test]
    fn test_demo_playback_speed() {
        let mut playback = DemoPlayback::new();
        playback.set_speed(2.0);
        assert_eq!(playback.speed, 2.0);

        playback.set_speed(10.0); // Should clamp to MAX
        assert_eq!(playback.speed, MAX_DEMO_SPEED);

        playback.set_speed(0.01); // Should clamp to MIN
        assert_eq!(playback.speed, MIN_DEMO_SPEED);
    }

    #[test]
    fn test_demo_index_find_keyframe() {
        let mut index = DemoIndex::new();
        index.duration = 100.0;

        // Add some keyframes
        for i in 0..5 {
            index.keyframes.push(DemoKeyframe {
                servertime: i * 20000, // Every 20 seconds
                file_offset: i as u64 * 1000,
                frame_number: i as u32 * 200,
                ..Default::default()
            });
        }

        // Find keyframe for 25 seconds - should get the 20s keyframe
        let kf = index.find_keyframe_for_time(25.0).unwrap();
        assert_eq!(kf.servertime, 20000);

        // Find keyframe for 50% - should get around 50s
        let kf = index.find_keyframe_for_percent(0.5).unwrap();
        assert_eq!(kf.servertime, 40000);
    }
}
