// game.rs — Game DLL interface visible to server
// Converted from: myq2-original/game/game.h


pub use myq2_common::game_api::{GAME_API_VERSION, SVF_NOCLIENT, SVF_DEADMONSTER, SVF_MONSTER};
pub use myq2_common::q_shared::Multicast;

// edict->svflags (SVF_PROJECTILE is MyQ2 extension, not in base game_api)
pub const SVF_PROJECTILE: i32 = 0x00000008;

// edict->solid values
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum Solid {
    #[default]
    Not = 0,
    Trigger,
    Bbox,
    Bsp,
}

// MAX_ENT_CLUSTERS comes from myq2_common::q_shared

/// Area link (doubly-linked list node for spatial partitioning).
/// In the C code this was `link_t` with raw prev/next pointers.
/// In Rust, entity area linking will be managed by index-based references.
#[derive(Debug, Clone, Copy, Default)]
pub struct AreaLink {
    pub prev: i32, // entity index, -1 = none
    pub next: i32, // entity index, -1 = none
}

// ============================================================
// Multicast enum — re-exported from myq2_common::q_shared above
// ============================================================
