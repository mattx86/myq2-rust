// cl_hud.rs -- Enhanced HUD customization (R1Q2/Q2Pro feature)
//
// Features:
// - HUD scaling
// - Alpha transparency
// - Visibility toggles for elements
// - Additional displays: FPS counter, speed meter, timer
// - Minimal HUD mode

use std::sync::{LazyLock, Mutex};
use std::time::Instant;

use myq2_common::common::com_printf;
use myq2_common::cvar::cvar_variable_value;
use crate::console::draw_string;

/// HUD anchor points for element positioning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudAnchor {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
}

/// HUD element position configuration.
#[derive(Clone)]
pub struct HudPosition {
    pub anchor: HudAnchor,
    pub offset_x: i32,
    pub offset_y: i32,
}

impl Default for HudPosition {
    fn default() -> Self {
        Self {
            anchor: HudAnchor::TopLeft,
            offset_x: 0,
            offset_y: 0,
        }
    }
}

/// HUD configuration and state.
#[derive(Clone)]
pub struct HudConfig {
    /// Global scale factor (0.5-2.0).
    pub scale: f32,
    /// Global alpha transparency (0.0-1.0).
    pub alpha: f32,
    /// Show health display.
    pub show_health: bool,
    /// Show armor display.
    pub show_armor: bool,
    /// Show ammo display.
    pub show_ammo: bool,
    /// Show match timer.
    pub show_timer: bool,
    /// Show FPS counter.
    pub show_fps: bool,
    /// Show velocity/speed meter.
    pub show_speed: bool,
    /// Show network statistics (ping, jitter, packet loss).
    pub show_netstats: bool,
    /// Minimal HUD mode (only essential info).
    pub minimal_mode: bool,
    /// Position for FPS display.
    pub fps_position: HudPosition,
    /// Position for speed display.
    pub speed_position: HudPosition,
    /// Position for timer display.
    pub timer_position: HudPosition,
    /// Position for network stats display.
    pub netstats_position: HudPosition,
}

impl Default for HudConfig {
    fn default() -> Self {
        Self {
            scale: 1.0,
            alpha: 1.0,
            show_health: true,
            show_armor: true,
            show_ammo: true,
            show_timer: false,
            show_fps: false,
            show_speed: false,
            show_netstats: false,
            minimal_mode: false,
            fps_position: HudPosition {
                anchor: HudAnchor::TopRight,
                offset_x: -8,
                offset_y: 8,
            },
            speed_position: HudPosition {
                anchor: HudAnchor::BottomRight,
                offset_x: -8,
                offset_y: -24,
            },
            timer_position: HudPosition {
                anchor: HudAnchor::TopRight,
                offset_x: -8,
                offset_y: 24,
            },
            netstats_position: HudPosition {
                anchor: HudAnchor::TopLeft,
                offset_x: 8,
                offset_y: 8,
            },
        }
    }
}

impl HudConfig {
    /// Update configuration from cvars.
    pub fn update_from_cvars(&mut self) {
        self.scale = cvar_variable_value("hud_scale").clamp(0.5, 2.0);
        self.alpha = cvar_variable_value("hud_alpha").clamp(0.0, 1.0);
        self.show_health = cvar_variable_value("hud_show_health") != 0.0;
        self.show_armor = cvar_variable_value("hud_show_armor") != 0.0;
        self.show_ammo = cvar_variable_value("hud_show_ammo") != 0.0;
        self.show_timer = cvar_variable_value("hud_show_timer") != 0.0;
        self.show_fps = cvar_variable_value("hud_show_fps") != 0.0;
        self.show_speed = cvar_variable_value("hud_show_speed") != 0.0;
        self.show_netstats = cvar_variable_value("hud_show_netstats") != 0.0;
        self.minimal_mode = cvar_variable_value("hud_minimal") != 0.0;
    }

    /// Calculate screen position from anchor and offset.
    pub fn calc_position(&self, pos: &HudPosition, width: i32, height: i32, text_width: i32, text_height: i32) -> (i32, i32) {
        let (base_x, base_y) = match pos.anchor {
            HudAnchor::TopLeft => (0, 0),
            HudAnchor::TopRight => (width - text_width, 0),
            HudAnchor::BottomLeft => (0, height - text_height),
            HudAnchor::BottomRight => (width - text_width, height - text_height),
            HudAnchor::Center => ((width - text_width) / 2, (height - text_height) / 2),
        };
        (base_x + pos.offset_x, base_y + pos.offset_y)
    }
}

/// FPS counter state.
pub struct FpsCounter {
    /// Frame times for averaging.
    frame_times: [f32; 60],
    /// Current index in frame_times array.
    current_index: usize,
    /// Last frame timestamp.
    last_frame: Instant,
    /// Cached FPS value.
    cached_fps: f32,
    /// Frames since last FPS update.
    frames_since_update: i32,
}

impl Default for FpsCounter {
    fn default() -> Self {
        Self {
            frame_times: [0.0; 60],
            current_index: 0,
            last_frame: Instant::now(),
            cached_fps: 0.0,
            frames_since_update: 0,
        }
    }
}

impl FpsCounter {
    /// Update FPS counter with current frame.
    pub fn update(&mut self) {
        let now = Instant::now();
        let delta = now.duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;

        if delta > 0.0 {
            self.frame_times[self.current_index] = delta;
            self.current_index = (self.current_index + 1) % self.frame_times.len();
        }

        self.frames_since_update += 1;

        // Update cached FPS every 10 frames to avoid flicker
        if self.frames_since_update >= 10 {
            let total: f32 = self.frame_times.iter().sum();
            let avg = total / self.frame_times.len() as f32;
            self.cached_fps = if avg > 0.0 { 1.0 / avg } else { 0.0 };
            self.frames_since_update = 0;
        }
    }

    /// Get current FPS value.
    pub fn get_fps(&self) -> f32 {
        self.cached_fps
    }
}

/// Speed meter state.
pub struct SpeedMeter {
    /// Current speed value.
    current_speed: f32,
    /// Maximum speed recorded this session.
    max_speed: f32,
}

impl Default for SpeedMeter {
    fn default() -> Self {
        Self {
            current_speed: 0.0,
            max_speed: 0.0,
        }
    }
}

impl SpeedMeter {
    /// Update speed from velocity vector.
    pub fn update(&mut self, velocity: &[f32; 3]) {
        // Calculate horizontal speed (XY plane only, Z is vertical)
        let speed = (velocity[0] * velocity[0] + velocity[1] * velocity[1]).sqrt();
        self.current_speed = speed;
        if speed > self.max_speed {
            self.max_speed = speed;
        }
    }

    /// Reset max speed tracking.
    pub fn reset_max(&mut self) {
        self.max_speed = 0.0;
    }

    /// Get current speed.
    pub fn get_speed(&self) -> f32 {
        self.current_speed
    }

    /// Get max speed.
    pub fn get_max_speed(&self) -> f32 {
        self.max_speed
    }
}

/// Timer state for match timing.
pub struct MatchTimer {
    /// Start time of the match (server time in ms).
    start_time: i32,
    /// Whether timer is running.
    running: bool,
    /// Elapsed time in seconds.
    elapsed: f32,
}

impl Default for MatchTimer {
    fn default() -> Self {
        Self {
            start_time: 0,
            running: false,
            elapsed: 0.0,
        }
    }
}

impl MatchTimer {
    /// Start or restart the timer.
    pub fn start(&mut self, server_time: i32) {
        self.start_time = server_time;
        self.running = true;
    }

    /// Stop the timer.
    pub fn stop(&mut self) {
        self.running = false;
    }

    /// Update elapsed time.
    pub fn update(&mut self, server_time: i32) {
        if self.running {
            self.elapsed = (server_time - self.start_time) as f32 / 1000.0;
        }
    }

    /// Format elapsed time as M:SS.
    pub fn format_time(&self) -> String {
        let total_secs = self.elapsed as i32;
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        format!("{}:{:02}", mins, secs)
    }

    /// Get elapsed time in seconds.
    pub fn get_elapsed(&self) -> f32 {
        self.elapsed
    }
}

/// Cached network statistics for HUD display.
#[derive(Clone, Default)]
pub struct CachedNetStats {
    /// Ping in milliseconds.
    pub ping: i32,
    /// Jitter in milliseconds.
    pub jitter: i32,
    /// Packet loss percentage (0-100).
    pub loss: f32,
    /// Interpolation buffer size in ms.
    pub interp: i32,
    /// Network quality assessment string ("Excellent", "Good", "Fair", "Poor", "Critical").
    pub quality: String,
    /// Last update time for refresh throttling.
    pub last_update: i32,
}

// ============================================================
// Stat Value Smoothing
// ============================================================

/// Smoothed stat value for gradual HUD transitions.
/// Uses exponential moving average (EMA) for smooth value changes.
#[derive(Clone)]
pub struct SmoothedStat {
    /// Current smoothed display value
    pub display_value: f32,
    /// Target value from server
    pub target_value: f32,
    /// Smoothing factor (0.0-1.0, higher = faster response)
    pub smoothing: f32,
    /// Whether this stat is enabled for smoothing
    pub enabled: bool,
    /// Last time the stat was updated
    pub last_update_time: i32,
}

impl Default for SmoothedStat {
    fn default() -> Self {
        Self {
            display_value: 0.0,
            target_value: 0.0,
            smoothing: 0.15, // ~150ms smoothing at 60fps
            enabled: true,
            last_update_time: 0,
        }
    }
}

impl SmoothedStat {
    /// Create with custom smoothing factor
    pub fn with_smoothing(smoothing: f32) -> Self {
        Self {
            smoothing: smoothing.clamp(0.01, 1.0),
            ..Default::default()
        }
    }

    /// Update the stat with a new target value
    pub fn update(&mut self, new_value: i32, current_time: i32) {
        self.target_value = new_value as f32;

        if !self.enabled {
            self.display_value = self.target_value;
            return;
        }

        // Initialize on first update
        if self.last_update_time == 0 {
            self.display_value = self.target_value;
            self.last_update_time = current_time;
            return;
        }

        // Calculate time-based smoothing for framerate independence
        let dt = (current_time - self.last_update_time) as f32 / 1000.0;
        self.last_update_time = current_time;

        // Clamp dt to prevent huge jumps after pause
        let dt = dt.clamp(0.0, 0.1);

        // Calculate EMA factor based on time delta
        // Higher dt = more smoothing towards target
        let factor = 1.0 - (1.0 - self.smoothing).powf(dt * 60.0);

        // Handle large changes (damage/healing) differently
        let diff = (self.target_value - self.display_value).abs();
        if diff > 50.0 {
            // Large change - use faster smoothing to show damage/healing quickly
            self.display_value += (self.target_value - self.display_value) * factor * 2.0;
        } else {
            // Normal change - smooth gradually
            self.display_value += (self.target_value - self.display_value) * factor;
        }

        // Snap to target if very close
        if (self.target_value - self.display_value).abs() < 0.5 {
            self.display_value = self.target_value;
        }
    }

    /// Get the display value as an integer
    pub fn get(&self) -> i32 {
        self.display_value.round() as i32
    }

    /// Get the fractional part for bar animations
    pub fn get_fraction(&self) -> f32 {
        self.display_value - self.display_value.floor()
    }

    /// Check if currently animating (display != target)
    pub fn is_animating(&self) -> bool {
        (self.target_value - self.display_value).abs() > 0.5
    }

    /// Reset to a specific value (for level changes, respawns)
    pub fn reset(&mut self, value: i32) {
        self.display_value = value as f32;
        self.target_value = value as f32;
    }
}

/// Collection of smoothed stats for HUD display.
#[derive(Clone, Default)]
pub struct StatSmoothing {
    /// Health smoothing
    pub health: SmoothedStat,
    /// Armor smoothing
    pub armor: SmoothedStat,
    /// Current weapon ammo smoothing
    pub ammo: SmoothedStat,
    /// Frag count smoothing
    pub frags: SmoothedStat,
    /// Whether stat smoothing is globally enabled
    pub enabled: bool,
}

impl StatSmoothing {
    pub fn new() -> Self {
        Self {
            health: SmoothedStat::with_smoothing(0.12), // Slightly slower for health
            armor: SmoothedStat::with_smoothing(0.15),
            ammo: SmoothedStat::with_smoothing(0.20),   // Faster for ammo (rapid fire)
            frags: SmoothedStat::with_smoothing(0.25),  // Fast for score updates
            enabled: true,
        }
    }

    /// Update all stats from playerstate
    pub fn update(&mut self, health: i32, armor: i32, ammo: i32, frags: i32, current_time: i32) {
        if !self.enabled {
            self.health.display_value = health as f32;
            self.armor.display_value = armor as f32;
            self.ammo.display_value = ammo as f32;
            self.frags.display_value = frags as f32;
            return;
        }

        self.health.update(health, current_time);
        self.armor.update(armor, current_time);
        self.ammo.update(ammo, current_time);
        self.frags.update(frags, current_time);
    }

    /// Reset all stats (on respawn, level change)
    pub fn reset(&mut self, health: i32, armor: i32, ammo: i32, frags: i32) {
        self.health.reset(health);
        self.armor.reset(armor);
        self.ammo.reset(ammo);
        self.frags.reset(frags);
    }

    /// Update during packet loss - continue smoothing towards last known target
    /// but don't accept new values that might be stale/corrupt.
    /// This prevents flickering to 0 or other invalid values during network issues.
    pub fn continue_during_packet_loss(&mut self, current_time: i32) {
        if !self.enabled {
            return;
        }

        // Continue smoothing towards the last known target values
        // This keeps animations smooth but doesn't accept potentially corrupt data
        let dt = (current_time - self.health.last_update_time) as f32 / 1000.0;
        if dt > 0.0 && dt < 0.5 {
            // Update time but don't change targets - just continue the animation
            self.health.last_update_time = current_time;
            self.armor.last_update_time = current_time;
            self.ammo.last_update_time = current_time;
            self.frags.last_update_time = current_time;

            // Apply smoothing towards current targets
            let factor = 1.0 - (1.0 - 0.15_f32).powf(dt * 60.0);
            self.health.display_value += (self.health.target_value - self.health.display_value) * factor;
            self.armor.display_value += (self.armor.target_value - self.armor.display_value) * factor;
            self.ammo.display_value += (self.ammo.target_value - self.ammo.display_value) * factor;
            self.frags.display_value += (self.frags.target_value - self.frags.display_value) * factor;
        }
    }
}

/// Global HUD state.
pub struct HudState {
    pub config: HudConfig,
    pub fps_counter: FpsCounter,
    pub speed_meter: SpeedMeter,
    pub timer: MatchTimer,
    pub net_stats: CachedNetStats,
    /// Smoothed stat values for gradual HUD transitions
    pub stat_smoothing: StatSmoothing,
}

impl Default for HudState {
    fn default() -> Self {
        Self {
            config: HudConfig::default(),
            fps_counter: FpsCounter::default(),
            speed_meter: SpeedMeter::default(),
            timer: MatchTimer::default(),
            net_stats: CachedNetStats::default(),
            stat_smoothing: StatSmoothing::new(),
        }
    }
}

/// Global HUD state instance.
pub static HUD_STATE: LazyLock<Mutex<HudState>> = LazyLock::new(|| Mutex::new(HudState::default()));

// ============================================================
// Public API
// ============================================================

/// Update HUD configuration from cvars.
pub fn hud_update_config() {
    let mut state = HUD_STATE.lock().unwrap();
    state.config.update_from_cvars();
}

/// Update FPS counter (call each frame).
pub fn hud_update_fps() {
    let mut state = HUD_STATE.lock().unwrap();
    state.fps_counter.update();
}

/// Update speed meter with player velocity.
pub fn hud_update_speed(velocity: &[f32; 3]) {
    let mut state = HUD_STATE.lock().unwrap();
    state.speed_meter.update(velocity);
}

/// Update match timer with server time.
pub fn hud_update_timer(server_time: i32) {
    let mut state = HUD_STATE.lock().unwrap();
    state.timer.update(server_time);
}

/// Start the match timer.
pub fn hud_start_timer(server_time: i32) {
    let mut state = HUD_STATE.lock().unwrap();
    state.timer.start(server_time);
}

/// Stop the match timer.
pub fn hud_stop_timer() {
    let mut state = HUD_STATE.lock().unwrap();
    state.timer.stop();
}

/// Reset speed meter max speed.
pub fn hud_reset_speed_max() {
    let mut state = HUD_STATE.lock().unwrap();
    state.speed_meter.reset_max();
}

/// Update network stats from client smoothing state.
/// Called periodically (not every frame) to avoid lock contention.
pub fn hud_update_netstats(ping: i32, jitter: i32, loss: f32, interp: i32, quality: &str, current_time: i32) {
    let mut state = HUD_STATE.lock().unwrap();
    // Only update every 250ms to reduce overhead
    if current_time - state.net_stats.last_update < 250 {
        return;
    }
    state.net_stats.ping = ping;
    state.net_stats.jitter = jitter;
    state.net_stats.loss = loss;
    state.net_stats.interp = interp;
    state.net_stats.quality = quality.to_string();
    state.net_stats.last_update = current_time;
}

/// Update smoothed stat values for HUD display.
/// Call this each frame with current playerstate values.
pub fn hud_update_stats(health: i32, armor: i32, ammo: i32, frags: i32, current_time: i32) {
    let mut state = HUD_STATE.lock().unwrap();
    state.stat_smoothing.update(health, armor, ammo, frags, current_time);
}

/// Continue stat smoothing during packet loss.
/// This prevents HUD flickering by holding last known values
/// while continuing smooth animation towards targets.
pub fn hud_continue_stats_during_packet_loss(current_time: i32) {
    let mut state = HUD_STATE.lock().unwrap();
    state.stat_smoothing.continue_during_packet_loss(current_time);
}

/// Get smoothed health value for HUD display.
pub fn hud_get_smoothed_health() -> i32 {
    let state = HUD_STATE.lock().unwrap();
    state.stat_smoothing.health.get()
}

/// Get smoothed armor value for HUD display.
pub fn hud_get_smoothed_armor() -> i32 {
    let state = HUD_STATE.lock().unwrap();
    state.stat_smoothing.armor.get()
}

/// Get smoothed ammo value for HUD display.
pub fn hud_get_smoothed_ammo() -> i32 {
    let state = HUD_STATE.lock().unwrap();
    state.stat_smoothing.ammo.get()
}

/// Get smoothed frag count for HUD display.
pub fn hud_get_smoothed_frags() -> i32 {
    let state = HUD_STATE.lock().unwrap();
    state.stat_smoothing.frags.get()
}

/// Reset smoothed stats (on respawn, level change).
pub fn hud_reset_stats(health: i32, armor: i32, ammo: i32, frags: i32) {
    let mut state = HUD_STATE.lock().unwrap();
    state.stat_smoothing.reset(health, armor, ammo, frags);
}

/// Enable or disable stat smoothing.
pub fn hud_set_stat_smoothing(enabled: bool) {
    let mut state = HUD_STATE.lock().unwrap();
    state.stat_smoothing.enabled = enabled;
}

/// Check if any stat is currently animating (for bar effects).
pub fn hud_stats_animating() -> bool {
    let state = HUD_STATE.lock().unwrap();
    state.stat_smoothing.health.is_animating() ||
    state.stat_smoothing.armor.is_animating() ||
    state.stat_smoothing.ammo.is_animating()
}

/// Draw HUD overlay elements (FPS, speed, timer).
pub fn hud_draw_overlays(screen_width: i32, screen_height: i32) {
    let state = HUD_STATE.lock().unwrap();
    let config = &state.config;

    if config.alpha <= 0.0 {
        return;
    }

    // Draw FPS counter
    if config.show_fps {
        let fps_text = format!("{:.0} FPS", state.fps_counter.get_fps());
        let text_width = fps_text.len() as i32 * 8; // Approximate char width
        let (x, y) = config.calc_position(&config.fps_position, screen_width, screen_height, text_width, 8);
        draw_hud_string(x, y, &fps_text, 0xf0, config.alpha); // Bright white
    }

    // Draw speed meter
    if config.show_speed {
        let speed_text = format!("{:.0} ups", state.speed_meter.get_speed());
        let text_width = speed_text.len() as i32 * 8;
        let (x, y) = config.calc_position(&config.speed_position, screen_width, screen_height, text_width, 8);
        draw_hud_string(x, y, &speed_text, 0xd0, config.alpha); // Yellow-ish
    }

    // Draw timer
    if config.show_timer {
        let timer_text = state.timer.format_time();
        let text_width = timer_text.len() as i32 * 8;
        let (x, y) = config.calc_position(&config.timer_position, screen_width, screen_height, text_width, 8);
        draw_hud_string(x, y, &timer_text, 0xf0, config.alpha);
    }

    // Draw network stats
    if config.show_netstats {
        let ns = &state.net_stats;
        // Color based on connection quality
        let ping_color = if ns.ping < 50 { 0xd0 } // Green
            else if ns.ping < 100 { 0xe0 } // Yellow
            else { 0xf2 }; // Red

        let loss_color = if ns.loss < 1.0 { 0xd0 } // Green
            else if ns.loss < 5.0 { 0xe0 } // Yellow
            else { 0xf2 }; // Red

        // Color for quality string
        let quality_color = match ns.quality.as_str() {
            "Excellent" => 0xd0, // Green
            "Good" => 0xd2,      // Light green
            "Fair" => 0xe0,      // Yellow
            "Poor" => 0xf2,      // Red
            "Critical" => 0xf4,  // Bright red
            _ => 0xd0,
        };

        // Line 1: Ping and jitter
        let line1 = format!("Ping: {}ms (±{})", ns.ping, ns.jitter);
        let text_width1 = line1.len() as i32 * 8;
        let (x1, y1) = config.calc_position(&config.netstats_position, screen_width, screen_height, text_width1, 32);
        draw_hud_string(x1, y1, &line1, ping_color, config.alpha);

        // Line 2: Packet loss
        let line2 = format!("Loss: {:.1}%", ns.loss);
        draw_hud_string(x1, y1 + 8, &line2, loss_color, config.alpha);

        // Line 3: Interpolation buffer
        let line3 = format!("Interp: {}ms", ns.interp);
        draw_hud_string(x1, y1 + 16, &line3, 0xd0, config.alpha);

        // Line 4: Network quality assessment
        if !ns.quality.is_empty() {
            let line4 = format!("Quality: {}", ns.quality);
            draw_hud_string(x1, y1 + 24, &line4, quality_color, config.alpha);
        }
    }
}

/// Draw a HUD string with color and alpha.
fn draw_hud_string(x: i32, y: i32, text: &str, _color: i32, alpha: f32) {
    // For now, use the standard draw_string with color index
    // Alpha is stored but may not be used by all renderers
    if alpha >= 1.0 {
        draw_string(x, y, text);
    } else if alpha > 0.0 {
        // Draw with alpha - use draw_fill for background if needed
        // Most Quake 2 renderers don't support per-character alpha,
        // so we just draw normally for now
        draw_string(x, y, text);
    }
}

/// Get the current HUD scale factor.
pub fn hud_get_scale() -> f32 {
    let state = HUD_STATE.lock().unwrap();
    state.config.scale
}

/// Get the current HUD alpha.
pub fn hud_get_alpha() -> f32 {
    let state = HUD_STATE.lock().unwrap();
    state.config.alpha
}

/// Check if minimal HUD mode is enabled.
pub fn hud_is_minimal() -> bool {
    let state = HUD_STATE.lock().unwrap();
    state.config.minimal_mode
}

/// Check if health should be shown.
pub fn hud_show_health() -> bool {
    let state = HUD_STATE.lock().unwrap();
    state.config.show_health && !state.config.minimal_mode
}

/// Check if armor should be shown.
pub fn hud_show_armor() -> bool {
    let state = HUD_STATE.lock().unwrap();
    state.config.show_armor && !state.config.minimal_mode
}

/// Check if ammo should be shown.
pub fn hud_show_ammo() -> bool {
    let state = HUD_STATE.lock().unwrap();
    state.config.show_ammo && !state.config.minimal_mode
}

/// Print HUD configuration info.
pub fn cmd_hud_info() {
    let state = HUD_STATE.lock().unwrap();
    let config = &state.config;
    com_printf(&format!(
        "HUD Info:\n\
         Scale: {:.1}x\n\
         Alpha: {:.2}\n\
         Show Health: {}\n\
         Show Armor: {}\n\
         Show Ammo: {}\n\
         Show FPS: {}\n\
         Show Speed: {}\n\
         Show Timer: {}\n\
         Minimal Mode: {}\n\
         Current FPS: {:.1}\n\
         Current Speed: {:.0} ups\n",
        config.scale,
        config.alpha,
        if config.show_health { "yes" } else { "no" },
        if config.show_armor { "yes" } else { "no" },
        if config.show_ammo { "yes" } else { "no" },
        if config.show_fps { "yes" } else { "no" },
        if config.show_speed { "yes" } else { "no" },
        if config.show_timer { "yes" } else { "no" },
        if config.minimal_mode { "yes" } else { "no" },
        state.fps_counter.get_fps(),
        state.speed_meter.get_speed(),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hud_config_defaults() {
        let config = HudConfig::default();
        assert_eq!(config.scale, 1.0);
        assert_eq!(config.alpha, 1.0);
        assert!(config.show_health);
        assert!(config.show_armor);
        assert!(config.show_ammo);
        assert!(!config.show_fps);
        assert!(!config.show_speed);
        assert!(!config.minimal_mode);
    }

    #[test]
    fn test_fps_counter() {
        let mut fps = FpsCounter::default();
        // Just verify it doesn't panic and returns reasonable values
        fps.update();
        std::thread::sleep(std::time::Duration::from_millis(16));
        fps.update();
        // After two updates, should have some FPS value (could be very high if fast)
        let current_fps = fps.get_fps();
        // FPS counter needs 10 frames to update cached value, so it may still be 0
        assert!(current_fps >= 0.0);
    }

    #[test]
    fn test_speed_meter() {
        let mut meter = SpeedMeter::default();
        meter.update(&[300.0, 400.0, 0.0]);
        assert_eq!(meter.get_speed(), 500.0); // 3-4-5 triangle
        assert_eq!(meter.get_max_speed(), 500.0);

        meter.update(&[0.0, 0.0, 0.0]);
        assert_eq!(meter.get_speed(), 0.0);
        assert_eq!(meter.get_max_speed(), 500.0); // Max should persist

        meter.reset_max();
        assert_eq!(meter.get_max_speed(), 0.0);
    }

    #[test]
    fn test_match_timer() {
        let mut timer = MatchTimer::default();
        timer.start(0);
        timer.update(5000);
        assert_eq!(timer.get_elapsed(), 5.0);
        assert_eq!(timer.format_time(), "0:05");

        timer.update(65000);
        assert_eq!(timer.format_time(), "1:05");
    }

    #[test]
    fn test_calc_position() {
        let config = HudConfig::default();
        let pos = HudPosition {
            anchor: HudAnchor::TopRight,
            offset_x: -10,
            offset_y: 5,
        };
        let (x, y) = config.calc_position(&pos, 800, 600, 100, 16);
        assert_eq!(x, 800 - 100 - 10); // width - text_width + offset
        assert_eq!(y, 0 + 5); // top + offset
    }
}
