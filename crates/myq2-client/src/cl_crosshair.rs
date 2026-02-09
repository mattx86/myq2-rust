// cl_crosshair.rs -- Enhanced crosshair customization (R1Q2/Q2Pro feature)
//
// Features:
// - Multiple crosshair styles (0=none, 1=cross, 2=dot, 3=circle, 4=cross+dot, 5=X)
// - Size scaling
// - Color selection (palette index)
// - Alpha transparency
// - Gap and thickness configuration
// - Dynamic crosshair (expand on movement/firing)

use std::sync::{LazyLock, Mutex};

use myq2_common::common::com_printf;
use crate::console::draw_fill;

/// Crosshair styles
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum CrosshairStyle {
    /// No crosshair
    None = 0,
    /// Classic + crosshair
    Cross = 1,
    /// Simple dot
    Dot = 2,
    /// Circle outline (approximated with rectangles)
    Circle = 3,
    /// Cross with center dot
    CrossDot = 4,
    /// X shape
    XShape = 5,
    /// Use image (ch1, ch2, etc.)
    Image = 6,
}

impl From<i32> for CrosshairStyle {
    fn from(value: i32) -> Self {
        match value {
            0 => CrosshairStyle::None,
            1 => CrosshairStyle::Cross,
            2 => CrosshairStyle::Dot,
            3 => CrosshairStyle::Circle,
            4 => CrosshairStyle::CrossDot,
            5 => CrosshairStyle::XShape,
            _ => CrosshairStyle::Image,
        }
    }
}

/// Crosshair configuration
#[derive(Clone)]
pub struct CrosshairConfig {
    /// Style (0-6)
    pub style: CrosshairStyle,
    /// Size multiplier (1.0 = default, 2.0 = double size)
    pub size: f32,
    /// Color (Q2 palette index, 0-255)
    pub color: i32,
    /// Alpha transparency (0.0-1.0)
    pub alpha: f32,
    /// Center gap in pixels
    pub gap: i32,
    /// Line thickness in pixels
    pub thickness: i32,
    /// Enable dynamic crosshair (expand on movement)
    pub dynamic: bool,
    /// Current dynamic expansion (0.0 = none, 1.0 = max)
    pub expansion: f32,
    /// Enable health-based crosshair color (R1Q2/Q2Pro ch_health)
    pub ch_health: bool,
}

impl Default for CrosshairConfig {
    fn default() -> Self {
        Self {
            style: CrosshairStyle::Cross,
            size: 1.0,
            color: 0xf0, // Bright white in Q2 palette
            alpha: 1.0,
            gap: 2,
            thickness: 2,
            dynamic: false,
            expansion: 0.0,
            ch_health: false,
        }
    }
}

impl CrosshairConfig {
    /// Update configuration from cvar values
    pub fn update_from_cvars(&mut self) {
        use myq2_common::cvar::cvar_variable_value;

        let style_val = cvar_variable_value("crosshair") as i32;

        // If style is 1-5, use procedural; if > 5, use image-based
        if style_val >= 1 && style_val <= 5 {
            self.style = CrosshairStyle::from(style_val);
        } else if style_val > 5 {
            self.style = CrosshairStyle::Image;
        } else {
            self.style = CrosshairStyle::None;
        }

        self.size = cvar_variable_value("crosshair_size").clamp(0.5, 4.0);
        self.color = cvar_variable_value("crosshair_color") as i32;
        self.alpha = cvar_variable_value("crosshair_alpha").clamp(0.0, 1.0);
        self.gap = cvar_variable_value("crosshair_gap") as i32;
        self.thickness = (cvar_variable_value("crosshair_thickness") as i32).clamp(1, 8);
        self.dynamic = cvar_variable_value("crosshair_dynamic") != 0.0;
        self.ch_health = cvar_variable_value("ch_health") != 0.0;
    }

    /// Get the effective color, optionally based on player health
    /// Health-based colors (R1Q2/Q2Pro ch_health):
    /// - Green (0xd0) when health > 66
    /// - Yellow (0xe0) when health 33-66
    /// - Red (0xf2) when health < 33
    pub fn get_effective_color(&self, health: Option<i32>) -> i32 {
        if !self.ch_health {
            return self.color;
        }

        match health {
            Some(h) if h > 66 => 0xd0,  // Green
            Some(h) if h >= 33 => 0xe0, // Yellow
            Some(_) => 0xf2,             // Red
            None => self.color,          // Fallback to configured color
        }
    }

    /// Update dynamic expansion based on player state
    pub fn update_dynamic(&mut self, moving: bool, attacking: bool, delta_time: f32) {
        if !self.dynamic {
            self.expansion = 0.0;
            return;
        }

        // Target expansion based on activity
        let target = if attacking {
            1.0
        } else if moving {
            0.5
        } else {
            0.0
        };

        // Smooth interpolation
        let speed = if target > self.expansion { 15.0 } else { 8.0 };
        self.expansion += (target - self.expansion) * speed * delta_time;
        self.expansion = self.expansion.clamp(0.0, 1.0);
    }

    /// Get the effective gap (including dynamic expansion)
    fn effective_gap(&self) -> i32 {
        let base = (self.gap as f32 * self.size) as i32;
        let expansion_add = (self.expansion * 8.0 * self.size) as i32;
        base + expansion_add
    }

    /// Get the effective thickness
    fn effective_thickness(&self) -> i32 {
        ((self.thickness as f32) * self.size).max(1.0) as i32
    }

    /// Get the effective arm length (for cross/X shapes)
    fn effective_length(&self) -> i32 {
        ((8.0 * self.size) as i32).max(2)
    }

    /// Draw the crosshair at the given center position
    pub fn draw(&self, center_x: i32, center_y: i32) {
        self.draw_with_health(center_x, center_y, None);
    }

    /// Draw the crosshair at the given center position with optional health-based coloring
    pub fn draw_with_health(&self, center_x: i32, center_y: i32, health: Option<i32>) {
        if self.style == CrosshairStyle::None || self.style == CrosshairStyle::Image {
            return; // None or use the existing image-based system
        }

        let effective_color = self.get_effective_color(health);

        match self.style {
            CrosshairStyle::Cross => self.draw_cross_with_color(center_x, center_y, effective_color),
            CrosshairStyle::Dot => self.draw_dot_with_color(center_x, center_y, effective_color),
            CrosshairStyle::Circle => self.draw_circle_with_color(center_x, center_y, effective_color),
            CrosshairStyle::CrossDot => {
                self.draw_cross_with_color(center_x, center_y, effective_color);
                self.draw_dot_with_color(center_x, center_y, effective_color);
            }
            CrosshairStyle::XShape => self.draw_x_with_color(center_x, center_y, effective_color),
            _ => {}
        }
    }

    /// Draw a + shaped crosshair
    fn draw_cross(&self, cx: i32, cy: i32) {
        self.draw_cross_with_color(cx, cy, self.color);
    }

    /// Draw a + shaped crosshair with specified color
    fn draw_cross_with_color(&self, cx: i32, cy: i32, color: i32) {
        let gap = self.effective_gap();
        let thickness = self.effective_thickness();
        let length = self.effective_length();
        let half_t = thickness / 2;

        // Top arm
        draw_fill(cx - half_t, cy - gap - length, thickness, length, color, self.alpha);
        // Bottom arm
        draw_fill(cx - half_t, cy + gap, thickness, length, color, self.alpha);
        // Left arm
        draw_fill(cx - gap - length, cy - half_t, length, thickness, color, self.alpha);
        // Right arm
        draw_fill(cx + gap, cy - half_t, length, thickness, color, self.alpha);
    }

    /// Draw a dot crosshair
    fn draw_dot(&self, cx: i32, cy: i32) {
        self.draw_dot_with_color(cx, cy, self.color);
    }

    /// Draw a dot crosshair with specified color
    fn draw_dot_with_color(&self, cx: i32, cy: i32, color: i32) {
        let dot_size = (self.thickness as f32 * self.size * 1.5).max(2.0) as i32;
        let half = dot_size / 2;
        draw_fill(cx - half, cy - half, dot_size, dot_size, color, self.alpha);
    }

    /// Draw a circle crosshair (approximated with rectangles)
    fn draw_circle(&self, cx: i32, cy: i32) {
        self.draw_circle_with_color(cx, cy, self.color);
    }

    /// Draw a circle crosshair with specified color (approximated with rectangles)
    fn draw_circle_with_color(&self, cx: i32, cy: i32, color: i32) {
        let radius = ((8.0 * self.size) as i32).max(4);
        let thickness = self.effective_thickness();

        // Approximate circle with 8 segments
        // This creates an octagon-like shape
        let segments = 8;
        for i in 0..segments {
            let angle1 = (i as f32) * std::f32::consts::TAU / segments as f32;
            let angle2 = ((i + 1) as f32) * std::f32::consts::TAU / segments as f32;

            let x1 = cx + (angle1.cos() * radius as f32) as i32;
            let y1 = cy + (angle1.sin() * radius as f32) as i32;
            let x2 = cx + (angle2.cos() * radius as f32) as i32;
            let y2 = cy + (angle2.sin() * radius as f32) as i32;

            // Draw line segment as a thin rectangle
            self.draw_line_with_color(x1, y1, x2, y2, thickness, color);
        }
    }

    /// Draw an X shaped crosshair
    fn draw_x(&self, cx: i32, cy: i32) {
        self.draw_x_with_color(cx, cy, self.color);
    }

    /// Draw an X shaped crosshair with specified color
    fn draw_x_with_color(&self, cx: i32, cy: i32, color: i32) {
        let gap = self.effective_gap();
        let length = self.effective_length();
        let thickness = self.effective_thickness();

        // Calculate diagonal positions
        let offset = ((gap as f32) * 0.707) as i32; // cos(45) ≈ 0.707
        let arm_len = ((length as f32) * 0.707) as i32;

        // Four diagonal arms (approximated with small rectangles)
        // Top-left arm
        for i in 0..arm_len {
            let x = cx - offset - i;
            let y = cy - offset - i;
            draw_fill(x - thickness/2, y - thickness/2, thickness, thickness, color, self.alpha);
        }
        // Top-right arm
        for i in 0..arm_len {
            let x = cx + offset + i;
            let y = cy - offset - i;
            draw_fill(x - thickness/2, y - thickness/2, thickness, thickness, color, self.alpha);
        }
        // Bottom-left arm
        for i in 0..arm_len {
            let x = cx - offset - i;
            let y = cy + offset + i;
            draw_fill(x - thickness/2, y - thickness/2, thickness, thickness, color, self.alpha);
        }
        // Bottom-right arm
        for i in 0..arm_len {
            let x = cx + offset + i;
            let y = cy + offset + i;
            draw_fill(x - thickness/2, y - thickness/2, thickness, thickness, color, self.alpha);
        }
    }

    /// Draw a line using small rectangles (Bresenham-like)
    fn draw_line(&self, x1: i32, y1: i32, x2: i32, y2: i32, thickness: i32) {
        self.draw_line_with_color(x1, y1, x2, y2, thickness, self.color);
    }

    /// Draw a line using small rectangles (Bresenham-like) with specified color
    fn draw_line_with_color(&self, x1: i32, y1: i32, x2: i32, y2: i32, thickness: i32, color: i32) {
        let dx = (x2 - x1).abs();
        let dy = (y2 - y1).abs();
        let steps = dx.max(dy);

        if steps == 0 {
            draw_fill(x1 - thickness/2, y1 - thickness/2, thickness, thickness, color, self.alpha);
            return;
        }

        let x_inc = (x2 - x1) as f32 / steps as f32;
        let y_inc = (y2 - y1) as f32 / steps as f32;

        let mut x = x1 as f32;
        let mut y = y1 as f32;

        for _ in 0..=steps {
            draw_fill(
                x as i32 - thickness/2,
                y as i32 - thickness/2,
                thickness,
                thickness,
                color,
                self.alpha,
            );
            x += x_inc;
            y += y_inc;
        }
    }
}

/// Global crosshair configuration
pub static CROSSHAIR_CONFIG: LazyLock<Mutex<CrosshairConfig>> =
    LazyLock::new(|| Mutex::new(CrosshairConfig::default()));

/// Check if the current crosshair style is procedural (not image-based)
pub fn crosshair_is_procedural() -> bool {
    let config = CROSSHAIR_CONFIG.lock().unwrap();
    config.style != CrosshairStyle::None && config.style != CrosshairStyle::Image
}

/// Update crosshair configuration from cvars
pub fn crosshair_update_config() {
    let mut config = CROSSHAIR_CONFIG.lock().unwrap();
    config.update_from_cvars();
}

/// Update dynamic crosshair state
pub fn crosshair_update_dynamic(moving: bool, attacking: bool, delta_time: f32) {
    let mut config = CROSSHAIR_CONFIG.lock().unwrap();
    config.update_dynamic(moving, attacking, delta_time);
}

/// Draw the crosshair at the screen center
pub fn crosshair_draw(center_x: i32, center_y: i32) {
    let config = CROSSHAIR_CONFIG.lock().unwrap();
    config.draw(center_x, center_y);
}

/// Draw the crosshair at the screen center with health-based coloring
pub fn crosshair_draw_with_health(center_x: i32, center_y: i32, health: i32) {
    let config = CROSSHAIR_CONFIG.lock().unwrap();
    config.draw_with_health(center_x, center_y, Some(health));
}

/// Check if health-based crosshair coloring is enabled
pub fn crosshair_health_enabled() -> bool {
    let config = CROSSHAIR_CONFIG.lock().unwrap();
    config.ch_health
}

/// Print crosshair info
pub fn cmd_crosshair_info() {
    let config = CROSSHAIR_CONFIG.lock().unwrap();
    com_printf(&format!(
        "Crosshair Info:\n  Style: {:?}\n  Size: {:.1}x\n  Color: {}\n  Alpha: {:.2}\n  Gap: {}\n  Thickness: {}\n  Dynamic: {}\n  Health-based (ch_health): {}\n",
        config.style,
        config.size,
        config.color,
        config.alpha,
        config.gap,
        config.thickness,
        if config.dynamic { "yes" } else { "no" },
        if config.ch_health { "yes" } else { "no" }
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crosshair_style_from_int() {
        assert_eq!(CrosshairStyle::from(0), CrosshairStyle::None);
        assert_eq!(CrosshairStyle::from(1), CrosshairStyle::Cross);
        assert_eq!(CrosshairStyle::from(2), CrosshairStyle::Dot);
        assert_eq!(CrosshairStyle::from(5), CrosshairStyle::XShape);
        assert_eq!(CrosshairStyle::from(10), CrosshairStyle::Image);
    }

    #[test]
    fn test_effective_values() {
        let mut config = CrosshairConfig::default();

        // Default size
        assert_eq!(config.effective_gap(), 2);
        assert_eq!(config.effective_thickness(), 2);

        // Double size
        config.size = 2.0;
        assert_eq!(config.effective_gap(), 4);
        assert_eq!(config.effective_thickness(), 4);
    }

    #[test]
    fn test_dynamic_expansion() {
        let mut config = CrosshairConfig::default();
        config.dynamic = true;

        // Should expand when attacking
        config.update_dynamic(false, true, 0.1);
        assert!(config.expansion > 0.0);

        // Should contract when idle
        config.update_dynamic(false, false, 1.0);
        // After enough time, should be close to 0
    }

    #[test]
    fn test_ch_health_color() {
        let mut config = CrosshairConfig::default();

        // Without ch_health, should return configured color
        assert_eq!(config.get_effective_color(Some(100)), config.color);
        assert_eq!(config.get_effective_color(Some(50)), config.color);
        assert_eq!(config.get_effective_color(Some(10)), config.color);

        // Enable ch_health
        config.ch_health = true;

        // High health (>66) = green (0xd0)
        assert_eq!(config.get_effective_color(Some(100)), 0xd0);
        assert_eq!(config.get_effective_color(Some(67)), 0xd0);

        // Medium health (33-66) = yellow (0xe0)
        assert_eq!(config.get_effective_color(Some(66)), 0xe0);
        assert_eq!(config.get_effective_color(Some(50)), 0xe0);
        assert_eq!(config.get_effective_color(Some(33)), 0xe0);

        // Low health (<33) = red (0xf2)
        assert_eq!(config.get_effective_color(Some(32)), 0xf2);
        assert_eq!(config.get_effective_color(Some(10)), 0xf2);
        assert_eq!(config.get_effective_color(Some(0)), 0xf2);

        // None health should return configured color
        assert_eq!(config.get_effective_color(None), config.color);
    }
}
