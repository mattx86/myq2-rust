// Integration tests for Linux/Wayland platform support.
//
// These tests verify that the platform layer compiles and behaves correctly
// on Linux. Most are gated behind #[cfg(target_os = "linux")] and will be
// skipped on Windows CI.

#[cfg(target_os = "linux")]
mod linux_tests {
    #[test]
    fn test_buildstring_is_linux() {
        assert_eq!(myq2_common::qcommon::BUILDSTRING, "Linux");
    }

    #[test]
    fn test_sys_milliseconds_monotonic() {
        let t1 = myq2_sys::q_shwin::sys_milliseconds();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let t2 = myq2_sys::q_shwin::sys_milliseconds();
        assert!(t2 > t1, "sys_milliseconds must be monotonically increasing");
    }

    #[test]
    fn test_console_input_returns_none_when_not_dedicated() {
        // When not in dedicated server mode, console input should always return None.
        let result = myq2_sys::sys_win::sys_console_input();
        assert!(result.is_none());
    }

    #[test]
    fn test_clipboard_does_not_panic() {
        // Verify clipboard access doesn't panic even without a running
        // Wayland compositor (e.g., in CI/Docker). It should return None
        // gracefully.
        let result = myq2_sys::sys_win::sys_get_clipboard_data();
        let _ = result;
    }

    #[test]
    fn test_gamma_ramp_disabled_on_linux() {
        let mut ctx = myq2_sys::glw_imp::GlImpContext::default();
        // On Linux, save_original_gamma should disable hw gamma
        // (Wayland has no gamma ramp API)
        ctx.save_original_gamma();
        assert!(!ctx.vk_config.gammaramp);
    }

    #[test]
    fn test_gamma_ramp_update_noop_when_disabled() {
        let mut ctx = myq2_sys::glw_imp::GlImpContext::default();
        ctx.save_original_gamma();
        // Should not panic; gammaramp is false so this early-returns
        ctx.update_gamma_ramp(2.0);
        // Ramp data should remain zeroed
        assert!(ctx.gamma.gamma_ramp[0].iter().all(|&v| v == 0));
    }
}

// Cross-platform tests that should pass on all platforms
mod cross_platform_tests {
    #[test]
    fn test_glw_imp_context_default() {
        let ctx = myq2_sys::glw_imp::GlImpContext::default();
        assert!(!ctx.vk_config.gammaramp);
        assert!(!ctx.vk_state.fullscreen);
        assert!(ctx.state.is_none());
    }

    #[test]
    fn test_gamma_ramp_computation_identity() {
        let mut ctx = myq2_sys::glw_imp::GlImpContext::default();
        ctx.vk_config.gammaramp = true;
        ctx.update_gamma_ramp(1.0);
        // With gamma=1.0, ramp should be approximately linear
        for i in 1..256 {
            let expected = (i as u16) << 8;
            let actual = ctx.gamma.gamma_ramp[0][i];
            let diff = (actual as i32 - expected as i32).unsigned_abs();
            assert!(
                diff <= 256,
                "gamma_ramp[0][{}] = {}, expected ~{} (diff={})",
                i,
                actual,
                expected,
                diff
            );
        }
    }

    #[test]
    fn test_gamma_ramp_computation_high_gamma() {
        let mut ctx = myq2_sys::glw_imp::GlImpContext::default();
        ctx.vk_config.gammaramp = true;
        ctx.update_gamma_ramp(2.0);
        // With gamma=2.0, the curve is v = ((i/255)^2) * 255, making
        // midtones darker. The midpoint (128) should be lower than 128<<8.
        let mid = ctx.gamma.gamma_ramp[0][128];
        assert!(
            mid < (128u16 << 8),
            "gamma=2.0: midpoint {} should be < {}",
            mid,
            128u16 << 8
        );
    }
}
