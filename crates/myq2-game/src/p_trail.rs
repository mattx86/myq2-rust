// p_trail.rs — Player Trail system for AI pursuit
// Converted from: myq2-original/game/p_trail.c
//
// This is a circular list containing points of where the player has been
// recently. It is used by monsters for pursuit.
//
// .origin      the spot
// .owner       forward link
// .aiment      backward link

use crate::g_local::*;
// TRAIL_LENGTH imported from g_local via wildcard

/// Circular index helpers (mask-based wrap for power-of-two length).
#[inline]
fn next(n: usize) -> usize {
    (n + 1) & (TRAIL_LENGTH - 1)
}

#[inline]
fn prev(n: usize) -> usize {
    (n.wrapping_sub(1)) & (TRAIL_LENGTH - 1)
}

// ============================================================
// PlayerTrail functions
// ============================================================

/// Initialize the player trail by spawning TRAIL_LENGTH invisible marker
/// entities. Skipped in deathmatch mode.
///
/// Corresponds to: PlayerTrail_Init (p_trail.c)
pub fn player_trail_init(ctx: &mut GameCtx) {
    if ctx.deathmatch != 0.0 {
        return;
    }

    for n in 0..TRAIL_LENGTH {
        let ent_idx = crate::g_utils::g_spawn(ctx);
        ctx.edicts[ent_idx].classname = "player_trail".to_string();
        ctx.player_trail.trail[n] = ent_idx as i32;
    }

    ctx.player_trail.trail_head = 0;
    ctx.player_trail.trail_active = true;
}

/// Add a new point to the player trail at the current head position.
///
/// Corresponds to: PlayerTrail_Add (p_trail.c)
pub fn player_trail_add(ctx: &mut GameCtx, spot: Vec3) {
    if !ctx.player_trail.trail_active {
        return;
    }

    let head = ctx.player_trail.trail_head;
    let head_idx = ctx.player_trail.trail[head] as usize;

    // VectorCopy(spot, trail[trail_head]->s.origin)
    ctx.edicts[head_idx].s.origin = spot;

    // trail[trail_head]->timestamp = level.time
    ctx.edicts[head_idx].timestamp = ctx.level.time;

    // VectorSubtract(spot, trail[PREV(trail_head)]->s.origin, temp)
    let prev_head = prev(head);
    let prev_idx = ctx.player_trail.trail[prev_head] as usize;
    let temp = [
        spot[0] - ctx.edicts[prev_idx].s.origin[0],
        spot[1] - ctx.edicts[prev_idx].s.origin[1],
        spot[2] - ctx.edicts[prev_idx].s.origin[2],
    ];

    // trail[trail_head]->s.angles[1] = vectoyaw(temp)
    ctx.edicts[head_idx].s.angles[1] = vectoyaw(&temp);

    ctx.player_trail.trail_head = next(head);
}


/// Pick the first trail marker that is more recent than the monster's
/// trail_time. Falls back to the previous marker if the best one is
/// not visible.
///
/// Returns an entity index, or None if the trail is not active.
///
/// Corresponds to: PlayerTrail_PickFirst (p_trail.c)
pub fn player_trail_pick_first(ctx: &GameCtx, self_idx: usize) -> Option<usize> {
    if !ctx.player_trail.trail_active {
        return None;
    }

    let mut marker = ctx.player_trail.trail_head;
    for _ in 0..TRAIL_LENGTH {
        let marker_ent_idx = ctx.player_trail.trail[marker] as usize;
        if ctx.edicts[marker_ent_idx].timestamp <= ctx.edicts[self_idx].monsterinfo.trail_time {
            marker = next(marker);
        } else {
            break;
        }
    }

    let marker_ent_idx = ctx.player_trail.trail[marker] as usize;
    if crate::g_ai::visible(&ctx.edicts[self_idx], &ctx.edicts[marker_ent_idx]) {
        return Some(marker_ent_idx);
    }

    let prev_marker = prev(marker);
    let prev_ent_idx = ctx.player_trail.trail[prev_marker] as usize;
    if crate::g_ai::visible(&ctx.edicts[self_idx], &ctx.edicts[prev_ent_idx]) {
        return Some(prev_ent_idx);
    }

    Some(marker_ent_idx)
}

/// Pick the next trail marker that is more recent than the monster's
/// trail_time.
///
/// Returns an entity index, or None if the trail is not active.
///
/// Corresponds to: PlayerTrail_PickNext (p_trail.c)
pub fn player_trail_pick_next(ctx: &GameCtx, self_idx: usize) -> Option<usize> {
    if !ctx.player_trail.trail_active {
        return None;
    }

    let mut marker = ctx.player_trail.trail_head;
    for _ in 0..TRAIL_LENGTH {
        let marker_ent_idx = ctx.player_trail.trail[marker] as usize;
        if ctx.edicts[marker_ent_idx].timestamp <= ctx.edicts[self_idx].monsterinfo.trail_time {
            marker = next(marker);
        } else {
            break;
        }
    }

    Some(ctx.player_trail.trail[marker] as usize)
}

/// Return the entity index of the most recently placed trail spot.
///
/// Corresponds to: PlayerTrail_LastSpot (p_trail.c)
pub fn player_trail_last_spot(ctx: &GameCtx) -> usize {
    let idx = prev(ctx.player_trail.trail_head);
    ctx.player_trail.trail[idx] as usize
}

// ============================================================
// Cross-module helpers
// ============================================================


use myq2_common::q_shared::vectoyaw;


// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_ctx() -> GameCtx {
        GameCtx {
            deathmatch: 0.0,
            ..GameCtx::default()
        }
    }

    #[test]
    fn test_next_prev_wrap() {
        assert_eq!(next(0), 1);
        assert_eq!(next(7), 0);
        assert_eq!(prev(0), 7);
        assert_eq!(prev(1), 0);
    }

    #[test]
    fn test_player_trail_init() {
        let mut ctx = make_test_ctx();
        player_trail_init(&mut ctx);
        assert!(ctx.player_trail.trail_active);
        assert_eq!(ctx.player_trail.trail_head, 0);
        // Should have spawned TRAIL_LENGTH entities
        assert_eq!(ctx.edicts.len(), TRAIL_LENGTH);
        for n in 0..TRAIL_LENGTH {
            let idx = ctx.player_trail.trail[n] as usize;
            assert_eq!(ctx.edicts[idx].classname, "player_trail");
        }
    }

    #[test]
    fn test_player_trail_init_deathmatch_skips() {
        let mut ctx = make_test_ctx();
        ctx.deathmatch = 1.0;
        player_trail_init(&mut ctx);
        assert!(!ctx.player_trail.trail_active);
    }

    #[test]
    fn test_player_trail_add_advances_head() {
        let mut ctx = make_test_ctx();
        player_trail_init(&mut ctx);
        assert_eq!(ctx.player_trail.trail_head, 0);

        player_trail_add(&mut ctx, [100.0, 200.0, 0.0]);
        assert_eq!(ctx.player_trail.trail_head, 1);

        let ent_idx = ctx.player_trail.trail[0] as usize;
        assert_eq!(ctx.edicts[ent_idx].s.origin, [100.0, 200.0, 0.0]);
    }

    #[test]
    fn test_player_trail_last_spot() {
        let mut ctx = make_test_ctx();
        player_trail_init(&mut ctx);

        player_trail_add(&mut ctx, [10.0, 20.0, 30.0]);
        let last = player_trail_last_spot(&ctx);
        assert_eq!(ctx.edicts[last].s.origin, [10.0, 20.0, 30.0]);
    }
}
