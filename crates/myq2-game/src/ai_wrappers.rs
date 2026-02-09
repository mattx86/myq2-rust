// ai_wrappers.rs — Thread-local AI context and wrapper functions for MFrame callbacks
//
// The MFrame struct stores `ai_fn: fn(&mut Edict, f32)` but the real AI functions
// in g_ai.rs require an AiContext (edicts vec, level, etc.). This module bridges
// the gap by storing a pointer to the AiContext in a thread-local before running
// monster frames, then providing wrapper functions that retrieve it.

use std::cell::Cell;
use crate::g_ai::AiContext;
use crate::g_local::Edict;

/// Raw pointer to the current AiContext, set before executing monster AI frames.
///
/// SAFETY: The game logic runs single-threaded. The pointer is set just before
/// calling ai_fn callbacks and cleared immediately after. The AiContext must
/// outlive the frame execution.
thread_local! {
    static AI_CTX: Cell<*mut AiContext> = const { Cell::new(std::ptr::null_mut()) };
}

/// Set the active AiContext for the duration of a closure.
/// This must wrap any code that calls MFrame ai_fn callbacks.
pub fn with_ai_context<F, R>(ctx: &mut AiContext, f: F) -> R
where
    F: FnOnce() -> R,
{
    AI_CTX.with(|cell| {
        let old = cell.get();
        cell.set(ctx as *mut AiContext);
        let result = f();
        cell.set(old);
        result
    })
}

/// Retrieve the current AiContext. Panics if called outside of `with_ai_context`.
///
/// SAFETY: The returned reference borrows from the AiContext pointer set by
/// `with_ai_context`. The caller must not hold this reference across a yield
/// point or past the `with_ai_context` scope. Since the game is single-threaded
/// and frame callbacks are synchronous, this is safe.
#[inline]
fn get_ctx() -> &'static mut AiContext {
    AI_CTX.with(|cell| {
        let ptr = cell.get();
        assert!(!ptr.is_null(), "AI wrapper called outside of with_ai_context");
        // SAFETY: The pointer is valid for the duration of the with_ai_context call.
        // Game logic is single-threaded and callbacks are synchronous.
        unsafe { &mut *ptr }
    })
}

/// Wrapper for `g_ai::ai_stand`. Signature matches `fn(&mut Edict, f32)`.
pub fn ai_stand(ent: &mut Edict, dist: f32) {
    let self_idx = ent.s.number;
    let ctx = get_ctx();
    crate::g_ai::ai_stand(ctx, self_idx, dist);
}

/// Wrapper for `g_ai::ai_walk`. Signature matches `fn(&mut Edict, f32)`.
pub fn ai_walk(ent: &mut Edict, dist: f32) {
    let self_idx = ent.s.number;
    let ctx = get_ctx();
    crate::g_ai::ai_walk(ctx, self_idx, dist);
}

/// Wrapper for `g_ai::ai_run`. Signature matches `fn(&mut Edict, f32)`.
pub fn ai_run(ent: &mut Edict, dist: f32) {
    let self_idx = ent.s.number;
    let ctx = get_ctx();
    crate::g_ai::ai_run(ctx, self_idx, dist);
}

/// Wrapper for `g_ai::ai_charge`. Signature matches `fn(&mut Edict, f32)`.
pub fn ai_charge(ent: &mut Edict, dist: f32) {
    let self_idx = ent.s.number;
    let ctx = get_ctx();
    crate::g_ai::ai_charge(ctx, self_idx, dist);
}

/// Wrapper for `g_ai::ai_move`. Signature matches `fn(&mut Edict, f32)`.
pub fn ai_move(ent: &mut Edict, dist: f32) {
    let self_idx = ent.s.number;
    let ctx = get_ctx();
    crate::g_ai::ai_move(ctx, self_idx, dist);
}

/// Wrapper for `g_ai::ai_turn`. Signature matches `fn(&mut Edict, f32)`.
pub fn ai_turn(ent: &mut Edict, dist: f32) {
    let self_idx = ent.s.number;
    let ctx = get_ctx();
    crate::g_ai::ai_turn(ctx, self_idx, dist);
}
