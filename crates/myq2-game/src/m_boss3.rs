// m_boss3.rs — Boss3 (Jorg/Makron teleport trigger)
// Converted from: myq2-original/game/m_boss3.c
// Frame definitions from: myq2-original/game/m_boss32.h

use crate::g_local::*;
use crate::game::*;
use crate::game_import::*;

// ============================================================
// Frame definitions (from m_boss32.h, subset used by boss3)
// ============================================================

pub const FRAME_STAND201: i32 = 414;
pub const FRAME_STAND260: i32 = 473;

// ============================================================
// Use_Boss3 — triggered when boss3_stand is used; sends a
// TE_BOSSTPORT temp entity effect and frees the edict.
// ============================================================

/// Corresponds to `Use_Boss3` in the C source.
/// `ent_idx`: index of the entity being used.
/// `_other_idx`: index of the other entity (unused).
/// `_activator_idx`: index of the activator entity (unused).
pub fn use_boss3(ctx: &mut GameCtx, ent_idx: usize, _other_idx: usize, _activator_idx: usize) {
    let origin = ctx.edicts[ent_idx].s.origin;

    gi_write_byte(SVC_TEMP_ENTITY);
    gi_write_byte(TE_BOSSTPORT);
    gi_write_position(&origin);
    gi_multicast(&origin, MULTICAST_PVS);

    crate::g_utils::g_free_edict(ctx, ent_idx);
}

// ============================================================
// Think_Boss3Stand — cycles the standing animation frames
// ============================================================

/// Corresponds to `Think_Boss3Stand` in the C source.
pub fn think_boss3_stand(ctx: &mut GameCtx, ent_idx: usize) {
    if ctx.edicts[ent_idx].s.frame == FRAME_STAND260 {
        ctx.edicts[ent_idx].s.frame = FRAME_STAND201;
    } else {
        ctx.edicts[ent_idx].s.frame += 1;
    }
    ctx.edicts[ent_idx].nextthink = ctx.level.time + FRAMETIME;
}

// ============================================================
// SP_monster_boss3_stand — spawn function
//
// QUAKED monster_boss3_stand (1 .5 0) (-32 -32 0) (32 32 90)
// Just stands and cycles in one place until targeted, then
// teleports away.
// ============================================================

/// Corresponds to `SP_monster_boss3_stand` in the C source.
pub fn sp_monster_boss3_stand(ctx: &mut GameCtx, self_idx: usize) {
    if ctx.deathmatch != 0.0 {
        crate::g_utils::g_free_edict(ctx, self_idx);
        return;
    }

    ctx.edicts[self_idx].movetype = MoveType::Step;
    ctx.edicts[self_idx].solid = Solid::Bbox;
    ctx.edicts[self_idx].model = "models/monsters/boss3/rider/tris.md2".to_string();

    ctx.edicts[self_idx].s.modelindex = gi_modelindex("models/monsters/boss3/rider/tris.md2");

    ctx.edicts[self_idx].s.frame = FRAME_STAND201;

    gi_soundindex("misc/bigtele.wav");

    ctx.edicts[self_idx].mins = [-32.0, -32.0, 0.0];
    ctx.edicts[self_idx].maxs = [32.0, 32.0, 90.0];

    ctx.edicts[self_idx].use_fn = Some(crate::dispatch::USE_BOSS3);
    ctx.edicts[self_idx].think_fn = Some(crate::dispatch::THINK_BOSS3_STAND);

    ctx.edicts[self_idx].nextthink = ctx.level.time + FRAMETIME;

    gi_linkentity(self_idx as i32);
}

