// Copyright (C) 1997-2001 Id Software, Inc.
//
// This program is free software; you can redistribute it and/or
// modify it under the terms of the GNU General Public License
// as published by the Free Software Foundation; either version 2
// of the License, or (at your option) any later version.
//
// Converted from myq2-original/client/ref.h

// ============================================================
// Re-exports from myq2-common — canonical repr(C) definitions
// ============================================================

/// Opaque model handle (equivalent to `struct model_s *`).
pub use myq2_common::q_shared::RefModel as Model;

/// Opaque image handle (equivalent to `struct image_s *`).
pub use myq2_common::q_shared::RefImage as Image;

/// entity_t — passed to the renderer for drawing.
pub use myq2_common::q_shared::RefEntity as Entity;

/// refdef_t — rendering parameters for a frame.
pub use myq2_common::q_shared::RefRefDef as RefDef;

pub use myq2_common::q_shared::{DLight, StainType, DStain};

pub use myq2_common::q_shared::{Particle, LightStyle};
