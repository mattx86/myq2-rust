// cl_timing.rs -- Decoupled frame timing (R1Q2/Q2Pro cl_async feature)
//
// This module provides decoupled timing for render, physics, and network
// operations. When cl_async is enabled (default), each subsystem runs at
// its own configurable rate:
//
// - r_maxfps: Render rate cap (0 = unlimited, follows vsync)
// - cl_maxfps: Physics/input rate (default 60)
// - cl_maxpackets: Network send rate (default 30)
//
// This allows high refresh rate rendering while maintaining consistent
// physics and network behavior.

use std::time::Instant;

/// Timing state for decoupled frame processing.
pub struct ClientTiming {
    /// Accumulated time for render frames (microseconds)
    pub render_accumulator: f64,
    /// Accumulated time for physics frames (microseconds)
    pub physics_accumulator: f64,
    /// Accumulated time for network frames (microseconds)
    pub network_accumulator: f64,
    /// Last frame timestamp
    pub last_frame_time: Instant,
    /// Whether cl_async is enabled
    pub async_enabled: bool,
}

impl ClientTiming {
    pub fn new() -> Self {
        Self {
            render_accumulator: 0.0,
            physics_accumulator: 0.0,
            network_accumulator: 0.0,
            last_frame_time: Instant::now(),
            async_enabled: true, // Enabled by default
        }
    }

    /// Calculate the time delta since the last frame and update accumulators.
    /// Returns the delta time in seconds.
    pub fn update(&mut self) -> f64 {
        let now = Instant::now();
        let delta = now.duration_since(self.last_frame_time);
        self.last_frame_time = now;

        let delta_us = delta.as_micros() as f64;

        if self.async_enabled {
            self.render_accumulator += delta_us;
            self.physics_accumulator += delta_us;
            self.network_accumulator += delta_us;
        }

        delta.as_secs_f64()
    }

    /// Check if a render frame should be processed.
    /// Returns true if enough time has passed for the target FPS.
    /// r_maxfps of 0 means unlimited (always returns true).
    pub fn should_render(&mut self, r_maxfps: f32) -> bool {
        if !self.async_enabled {
            return true; // Legacy mode: always render
        }

        if r_maxfps <= 0.0 {
            // Unlimited FPS
            self.render_accumulator = 0.0;
            return true;
        }

        let frame_time_us = 1_000_000.0 / r_maxfps as f64;
        if self.render_accumulator >= frame_time_us {
            self.render_accumulator -= frame_time_us;
            // Prevent accumulator from growing too large
            if self.render_accumulator > frame_time_us * 2.0 {
                self.render_accumulator = frame_time_us;
            }
            return true;
        }

        false
    }

    /// Check if a physics frame should be processed.
    /// Returns the number of physics frames to process (may be > 1 if we're behind).
    pub fn should_physics(&mut self, cl_maxfps: f32) -> u32 {
        if !self.async_enabled {
            return 1; // Legacy mode: one physics frame per render
        }

        if cl_maxfps <= 0.0 {
            return 1;
        }

        let frame_time_us = 1_000_000.0 / cl_maxfps as f64;
        let mut frames = 0u32;

        while self.physics_accumulator >= frame_time_us && frames < 5 {
            self.physics_accumulator -= frame_time_us;
            frames += 1;
        }

        // Prevent accumulator from growing too large
        if self.physics_accumulator > frame_time_us * 2.0 {
            self.physics_accumulator = 0.0;
        }

        frames
    }

    /// Check if a network packet should be sent.
    /// Returns true if enough time has passed.
    pub fn should_send_packet(&mut self, cl_maxpackets: f32) -> bool {
        if !self.async_enabled {
            return true; // Legacy mode: send with every frame
        }

        if cl_maxpackets <= 0.0 {
            return true;
        }

        let frame_time_us = 1_000_000.0 / cl_maxpackets as f64;
        if self.network_accumulator >= frame_time_us {
            self.network_accumulator -= frame_time_us;
            // Prevent accumulator from growing too large
            if self.network_accumulator > frame_time_us * 2.0 {
                self.network_accumulator = frame_time_us;
            }
            return true;
        }

        false
    }

    /// Get the physics frame time in seconds for a given target FPS.
    pub fn physics_frametime(&self, cl_maxfps: f32) -> f32 {
        if cl_maxfps <= 0.0 {
            1.0 / 60.0 // Default to 60fps physics
        } else {
            1.0 / cl_maxfps
        }
    }

    /// Reset all accumulators (used on map change, etc.)
    pub fn reset(&mut self) {
        self.render_accumulator = 0.0;
        self.physics_accumulator = 0.0;
        self.network_accumulator = 0.0;
        self.last_frame_time = Instant::now();
    }

    /// Set async mode enabled/disabled
    pub fn set_async(&mut self, enabled: bool) {
        self.async_enabled = enabled;
        if !enabled {
            self.reset();
        }
    }
}

impl Default for ClientTiming {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_timing_creation() {
        let timing = ClientTiming::new();
        assert!(timing.async_enabled);
        assert_eq!(timing.render_accumulator, 0.0);
        assert_eq!(timing.physics_accumulator, 0.0);
        assert_eq!(timing.network_accumulator, 0.0);
    }

    #[test]
    fn test_should_render_unlimited() {
        let mut timing = ClientTiming::new();
        timing.update();
        // r_maxfps = 0 should always allow rendering
        assert!(timing.should_render(0.0));
        assert!(timing.should_render(0.0));
    }

    #[test]
    fn test_should_render_capped() {
        let mut timing = ClientTiming::new();
        // Wait a bit to accumulate time
        thread::sleep(Duration::from_millis(20));
        timing.update();
        // At 60fps, frame time is ~16.67ms, so we should render after 20ms
        assert!(timing.should_render(60.0));
    }

    #[test]
    fn test_physics_frametime() {
        let timing = ClientTiming::new();
        assert!((timing.physics_frametime(60.0) - 1.0 / 60.0).abs() < 0.0001);
        assert!((timing.physics_frametime(0.0) - 1.0 / 60.0).abs() < 0.0001);
    }
}
