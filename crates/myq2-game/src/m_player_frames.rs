// m_player_frames.rs — Player model animation frame constants
// Converted from: myq2-original/game/m_player.h
//
// These constants define the animation frame indices for the player model.
// They are shared across multiple modules (g_cmds, p_weapon, p_view, p_client).
//
// Copyright (C) 1997-2001 Id Software, Inc.
// Licensed under the GNU General Public License v2+

// Stand frames (0-39)
pub const FRAME_STAND01: i32 = 0;
pub const FRAME_STAND40: i32 = 39;

// Run frames (40-45)
pub const FRAME_RUN1: i32 = 40;
pub const FRAME_RUN6: i32 = 45;

// Attack frames (46-53)
pub const FRAME_ATTACK1: i32 = 46;
pub const FRAME_ATTACK8: i32 = 53;

// Pain group 1 frames (54-57)
pub const FRAME_PAIN101: i32 = 54;
pub const FRAME_PAIN104: i32 = 57;

// Pain group 2 frames (58-61)
pub const FRAME_PAIN201: i32 = 58;
pub const FRAME_PAIN204: i32 = 61;

// Pain group 3 frames (62-65)
pub const FRAME_PAIN301: i32 = 62;
pub const FRAME_PAIN304: i32 = 65;

// Jump frames (66-71)
pub const FRAME_JUMP1: i32 = 66;
pub const FRAME_JUMP2: i32 = 67;
pub const FRAME_JUMP3: i32 = 68;
pub const FRAME_JUMP6: i32 = 71;

// Flip gesture frames (72-83)
pub const FRAME_FLIP01: i32 = 72;
pub const FRAME_FLIP12: i32 = 83;

// Salute gesture frames (84-94)
pub const FRAME_SALUTE01: i32 = 84;
pub const FRAME_SALUTE11: i32 = 94;

// Taunt gesture frames (95-111)
pub const FRAME_TAUNT01: i32 = 95;
pub const FRAME_TAUNT17: i32 = 111;

// Wave gesture frames (112-122)
pub const FRAME_WAVE01: i32 = 112;
pub const FRAME_WAVE08: i32 = 119;
pub const FRAME_WAVE11: i32 = 122;

// Point gesture frames (123-134)
pub const FRAME_POINT01: i32 = 123;
pub const FRAME_POINT12: i32 = 134;

// Crouch stand frames (135-153)
pub const FRAME_CRSTND01: i32 = 135;
pub const FRAME_CRSTND19: i32 = 153;

// Crouch walk frames (154-159)
pub const FRAME_CRWALK1: i32 = 154;
pub const FRAME_CRWALK6: i32 = 159;

// Crouch attack frames (160-168)
pub const FRAME_CRATTAK1: i32 = 160;
pub const FRAME_CRATTAK3: i32 = 162;
pub const FRAME_CRATTAK9: i32 = 168;

// Crouch pain frames (169-172)
pub const FRAME_CRPAIN1: i32 = 169;
pub const FRAME_CRPAIN4: i32 = 172;

// Crouch death frames (173-177)
pub const FRAME_CRDEATH1: i32 = 173;
pub const FRAME_CRDEATH5: i32 = 177;

// Death group 1 frames (178-183)
pub const FRAME_DEATH101: i32 = 178;
pub const FRAME_DEATH106: i32 = 183;

// Death group 2 frames (184-189)
pub const FRAME_DEATH201: i32 = 184;
pub const FRAME_DEATH206: i32 = 189;

// Death group 3 frames (190-197)
pub const FRAME_DEATH301: i32 = 190;
pub const FRAME_DEATH308: i32 = 197;
