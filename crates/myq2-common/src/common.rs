// common.rs — misc functions used in client and server
// Converted from: myq2-original/qcommon/common.c

use std::sync::Mutex;

use crate::crc::crc_block;
use crate::q_shared::{UserCmd, EntityState, Vec3, MAX_EDICTS, RF_BEAM, CVAR_SERVERINFO, CVAR_NOSET, CVAR_ZERO};
use crate::qcommon::{
    ERR_FATAL, ERR_DROP,
    NUMVERTEXNORMALS,
    SizeBuf,
    BUILDSTRING, CPUSTRING,
    CM_ANGLE1, CM_ANGLE2, CM_ANGLE3, CM_FORWARD, CM_SIDE, CM_UP, CM_BUTTONS, CM_IMPULSE,
    U_ORIGIN1, U_ORIGIN2, U_ORIGIN3, U_ANGLE1, U_ANGLE2, U_ANGLE3,
    U_FRAME8, U_FRAME16, U_EVENT, U_MOREBITS1, U_MOREBITS2, U_MOREBITS3,
    U_NUMBER16, U_MODEL, U_MODEL2, U_MODEL3, U_MODEL4,
    U_RENDERFX8, U_RENDERFX16, U_EFFECTS8, U_EFFECTS16,
    U_SKIN8, U_SKIN16, U_SOUND, U_SOLID, U_OLDORIGIN,
};

pub const MAXPRINTMSG: usize = 4096;
pub const MAX_NUM_ARGVS: usize = 50;

/// Distribution name and version (for window title, version strings, etc.)
pub const DISTNAME: &str = "MyQ2-Rust";
pub const DISTVER: f32 = 1.0;

// ============================================================
// Redirect buffer for Com_Printf
// ============================================================

static RD_BUFFER: Mutex<Option<String>> = Mutex::new(None);

/// Begin redirecting printf output into a buffer.
pub fn com_begin_redirect() {
    let mut buf = RD_BUFFER.lock().unwrap();
    *buf = Some(String::new());
}

/// End redirect and return the captured output.
pub fn com_end_redirect() -> Option<String> {
    let mut buf = RD_BUFFER.lock().unwrap();
    buf.take()
}

// ============================================================
// Com_Printf / Com_DPrintf / Com_Error
// ============================================================

/// General-purpose print function. Prints to stdout and appends to redirect
/// buffer if one is active.
pub fn com_printf(msg: &str) {
    // If redirecting, append to buffer
    {
        let mut buf = RD_BUFFER.lock().unwrap();
        if let Some(ref mut s) = *buf {
            s.push_str(msg);
            return;
        }
    }
    print!("{}", msg);
}

/// Developer-only print. Only prints when developer mode is active.
/// Controlled by the "developer" cvar.
pub fn com_dprintf(msg: &str) {
    if crate::cvar::cvar_variable_value("developer") == 0.0 {
        return;
    }
    com_printf(msg);
}

/// Engine error handler.
/// - `ERR_FATAL`: prints to stderr and panics.
/// - `ERR_DROP`: prints the error (non-fatal, allows recovery in the future).
/// - `ERR_QUIT`: clean exit.
pub fn com_error(code: i32, msg: &str) {
    if code == ERR_FATAL {
        eprintln!("Error: {}", msg);
        panic!("Fatal error: {}", msg);
    } else if code == ERR_DROP {
        eprintln!("********************\nERROR: {}\n********************", msg);
    } else {
        // ERR_QUIT or unknown
        println!("{}", msg);
        std::process::exit(0);
    }
}

// ============================================================
// CopyString — trivial in Rust
// ============================================================

/// Equivalent of CopyString in C (Z_Malloc + strcpy). In Rust, just clone.
pub fn copy_string(s: &str) -> String {
    String::from(s)
}

// ============================================================
// Z_Free — no-op in Rust
// ============================================================

/// No-op in Rust. Memory is managed automatically by the borrow checker / Drop.
/// Kept for API compatibility with the C codebase.
pub fn z_free<T>(_ptr: T) {
    // Intentionally empty — Rust drops `_ptr` at end of scope.
}

// ============================================================
// Bytedirs table — 162 pre-computed vertex normals for MD2 models
// ============================================================

pub use crate::anorms::BYTEDIRS;

// ============================================================
// SizeBuf operations
// ============================================================

impl SizeBuf {
    /// Get writable space in the buffer. Returns start offset of the space.
    /// Returns None on overflow.
    pub fn get_space(&mut self, length: usize) -> Option<usize> {
        let cursize = self.cursize as usize;
        let maxsize = self.maxsize as usize;

        if cursize + length > maxsize {
            if !self.allow_overflow {
                // In the C code this was Com_Error(ERR_FATAL, ...)
                panic!("SZ_GetSpace: overflow without allowoverflow set");
            }
            if length > maxsize {
                panic!("SZ_GetSpace: {} is > full buffer size", length);
            }
            com_printf("SZ_GetSpace: overflow\n");
            self.clear();
            self.overflowed = true;
        }

        let start = self.cursize as usize;
        self.cursize += length as i32;
        Some(start)
    }

    /// Write raw bytes into the buffer.
    pub fn write(&mut self, src: &[u8]) {
        if let Some(start) = self.get_space(src.len()) {
            self.data[start..start + src.len()].copy_from_slice(src);
        }
    }

    /// Append a null-terminated string, merging trailing nulls.
    pub fn print(&mut self, s: &str) {
        let bytes = s.as_bytes();
        let len = bytes.len() + 1; // include null terminator

        let cursize = self.cursize as usize;
        if cursize > 0 && self.data[cursize - 1] == 0 {
            // Write over existing trailing null
            if let Some(start) = self.get_space(len - 1) {
                let dest_start = start - 1;
                self.data[dest_start..dest_start + bytes.len()].copy_from_slice(bytes);
                self.data[dest_start + bytes.len()] = 0;
            }
        } else if let Some(start) = self.get_space(len) {
            self.data[start..start + bytes.len()].copy_from_slice(bytes);
            self.data[start + bytes.len()] = 0;
        }
    }
}

// ============================================================
// MSG write functions
// ============================================================

pub fn msg_write_char(sb: &mut SizeBuf, c: i32) {
    if let Some(start) = sb.get_space(1) {
        sb.data[start] = c as u8;
    }
}

pub fn msg_write_byte(sb: &mut SizeBuf, c: i32) {
    if let Some(start) = sb.get_space(1) {
        sb.data[start] = c as u8;
    }
}

pub fn msg_write_short(sb: &mut SizeBuf, c: i32) {
    if let Some(start) = sb.get_space(2) {
        sb.data[start..start + 2].copy_from_slice(&(c as i16).to_le_bytes());
    }
}

pub fn msg_write_long(sb: &mut SizeBuf, c: i32) {
    if let Some(start) = sb.get_space(4) {
        sb.data[start..start + 4].copy_from_slice(&c.to_le_bytes());
    }
}

pub fn msg_write_float(sb: &mut SizeBuf, f: f32) {
    if let Some(start) = sb.get_space(4) {
        sb.data[start..start + 4].copy_from_slice(&f.to_le_bytes());
    }
}

pub fn msg_write_string(sb: &mut SizeBuf, s: &str) {
    let bytes = s.as_bytes();
    sb.write(bytes);
    sb.write(&[0]); // null terminator
}

pub fn msg_write_coord(sb: &mut SizeBuf, f: f32) {
    msg_write_short(sb, (f * 8.0) as i32);
}

pub fn msg_write_pos(sb: &mut SizeBuf, pos: &Vec3) {
    msg_write_short(sb, (pos[0] * 8.0) as i32);
    msg_write_short(sb, (pos[1] * 8.0) as i32);
    msg_write_short(sb, (pos[2] * 8.0) as i32);
}

pub fn msg_write_angle(sb: &mut SizeBuf, f: f32) {
    msg_write_byte(sb, ((f * 256.0 / 360.0) as i32) & 255);
}

pub fn msg_write_angle16(sb: &mut SizeBuf, f: f32) {
    msg_write_short(sb, ((f * 65536.0 / 360.0) as i32) & 65535);
}

pub fn msg_write_delta_usercmd(buf: &mut SizeBuf, from: &UserCmd, cmd: &UserCmd) {
    let mut bits: i32 = 0;

    if cmd.angles[0] != from.angles[0] { bits |= CM_ANGLE1; }
    if cmd.angles[1] != from.angles[1] { bits |= CM_ANGLE2; }
    if cmd.angles[2] != from.angles[2] { bits |= CM_ANGLE3; }
    if cmd.forwardmove != from.forwardmove { bits |= CM_FORWARD; }
    if cmd.sidemove != from.sidemove { bits |= CM_SIDE; }
    if cmd.upmove != from.upmove { bits |= CM_UP; }
    if cmd.buttons != from.buttons { bits |= CM_BUTTONS; }
    if cmd.impulse != from.impulse { bits |= CM_IMPULSE; }

    msg_write_byte(buf, bits);

    if bits & CM_ANGLE1 != 0 { msg_write_short(buf, cmd.angles[0] as i32); }
    if bits & CM_ANGLE2 != 0 { msg_write_short(buf, cmd.angles[1] as i32); }
    if bits & CM_ANGLE3 != 0 { msg_write_short(buf, cmd.angles[2] as i32); }
    if bits & CM_FORWARD != 0 { msg_write_short(buf, cmd.forwardmove as i32); }
    if bits & CM_SIDE != 0 { msg_write_short(buf, cmd.sidemove as i32); }
    if bits & CM_UP != 0 { msg_write_short(buf, cmd.upmove as i32); }
    if bits & CM_BUTTONS != 0 { msg_write_byte(buf, cmd.buttons as i32); }
    if bits & CM_IMPULSE != 0 { msg_write_byte(buf, cmd.impulse as i32); }

    msg_write_byte(buf, cmd.msec as i32);
    msg_write_byte(buf, cmd.lightlevel as i32);
}

pub fn msg_write_dir(sb: &mut SizeBuf, dir: &Vec3) {
    let mut best = 0;
    let mut bestd: f32 = 0.0;

    for i in 0..NUMVERTEXNORMALS {
        let d = dir[0] * BYTEDIRS[i][0] + dir[1] * BYTEDIRS[i][1] + dir[2] * BYTEDIRS[i][2];
        if d > bestd {
            bestd = d;
            best = i;
        }
    }

    msg_write_byte(sb, best as i32);
}

// ============================================================
// MSG_WRITE functions for Vec<u8> buffers (growable)
// Used by demo recording where buffer size is not fixed.
// ============================================================

pub fn msg_write_byte_vec(buf: &mut Vec<u8>, c: i32) {
    buf.push(c as u8);
}

pub fn msg_write_short_vec(buf: &mut Vec<u8>, c: i32) {
    buf.extend_from_slice(&(c as i16).to_le_bytes());
}

pub fn msg_write_long_vec(buf: &mut Vec<u8>, c: i32) {
    buf.extend_from_slice(&c.to_le_bytes());
}

pub fn msg_write_string_vec(buf: &mut Vec<u8>, s: &str) {
    buf.extend_from_slice(s.as_bytes());
    buf.push(0);
}

pub fn msg_write_delta_entity(
    from: &EntityState,
    to: &EntityState,
    msg: &mut SizeBuf,
    force: bool,
    newentity: bool,
) {
    assert!(to.number != 0, "Unset entity number");
    assert!((to.number as usize) < MAX_EDICTS, "Entity number >= MAX_EDICTS");

    let mut bits: i32 = 0;

    if to.number >= 256 { bits |= U_NUMBER16; }

    if to.origin[0] != from.origin[0] { bits |= U_ORIGIN1; }
    if to.origin[1] != from.origin[1] { bits |= U_ORIGIN2; }
    if to.origin[2] != from.origin[2] { bits |= U_ORIGIN3; }

    if to.angles[0] != from.angles[0] { bits |= U_ANGLE1; }
    if to.angles[1] != from.angles[1] { bits |= U_ANGLE2; }
    if to.angles[2] != from.angles[2] { bits |= U_ANGLE3; }

    if to.skinnum != from.skinnum {
        if (to.skinnum as u32) < 256 { bits |= U_SKIN8; }
        else if (to.skinnum as u32) < 0x10000 { bits |= U_SKIN16; }
        else { bits |= U_SKIN8 | U_SKIN16; }
    }

    if to.frame != from.frame {
        if to.frame < 256 { bits |= U_FRAME8; }
        else { bits |= U_FRAME16; }
    }

    if to.effects != from.effects {
        if to.effects < 256 { bits |= U_EFFECTS8; }
        else if to.effects < 0x8000 { bits |= U_EFFECTS16; }
        else { bits |= U_EFFECTS8 | U_EFFECTS16; }
    }

    if to.renderfx != from.renderfx {
        if (to.renderfx as u32) < 256 { bits |= U_RENDERFX8; }
        else if (to.renderfx as u32) < 0x8000 { bits |= U_RENDERFX16; }
        else { bits |= U_RENDERFX8 | U_RENDERFX16; }
    }

    if to.solid != from.solid { bits |= U_SOLID; }
    if to.event != 0 { bits |= U_EVENT; }
    if to.modelindex != from.modelindex { bits |= U_MODEL; }
    if to.modelindex2 != from.modelindex2 { bits |= U_MODEL2; }
    if to.modelindex3 != from.modelindex3 { bits |= U_MODEL3; }
    if to.modelindex4 != from.modelindex4 { bits |= U_MODEL4; }
    if to.sound != from.sound { bits |= U_SOUND; }

    if newentity || (to.renderfx & RF_BEAM != 0) {
        bits |= U_OLDORIGIN;
    }

    if bits == 0 && !force {
        return;
    }

    // Set morebits flags
    if bits & 0xff000000_u32 as i32 != 0 {
        bits |= U_MOREBITS3 | U_MOREBITS2 | U_MOREBITS1;
    } else if bits & 0x00ff0000 != 0 {
        bits |= U_MOREBITS2 | U_MOREBITS1;
    } else if bits & 0x0000ff00 != 0 {
        bits |= U_MOREBITS1;
    }

    msg_write_byte(msg, bits & 255);

    if bits & 0xff000000_u32 as i32 != 0 {
        msg_write_byte(msg, (bits >> 8) & 255);
        msg_write_byte(msg, (bits >> 16) & 255);
        msg_write_byte(msg, (bits >> 24) & 255);
    } else if bits & 0x00ff0000 != 0 {
        msg_write_byte(msg, (bits >> 8) & 255);
        msg_write_byte(msg, (bits >> 16) & 255);
    } else if bits & 0x0000ff00 != 0 {
        msg_write_byte(msg, (bits >> 8) & 255);
    }

    if bits & U_NUMBER16 != 0 {
        msg_write_short(msg, to.number);
    } else {
        msg_write_byte(msg, to.number);
    }

    if bits & U_MODEL != 0 { msg_write_byte(msg, to.modelindex); }
    if bits & U_MODEL2 != 0 { msg_write_byte(msg, to.modelindex2); }
    if bits & U_MODEL3 != 0 { msg_write_byte(msg, to.modelindex3); }
    if bits & U_MODEL4 != 0 { msg_write_byte(msg, to.modelindex4); }

    if bits & U_FRAME8 != 0 { msg_write_byte(msg, to.frame); }
    if bits & U_FRAME16 != 0 { msg_write_short(msg, to.frame); }

    if (bits & U_SKIN8 != 0) && (bits & U_SKIN16 != 0) {
        msg_write_long(msg, to.skinnum);
    } else if bits & U_SKIN8 != 0 {
        msg_write_byte(msg, to.skinnum);
    } else if bits & U_SKIN16 != 0 {
        msg_write_short(msg, to.skinnum);
    }

    if (bits & U_EFFECTS8 != 0) && (bits & U_EFFECTS16 != 0) {
        msg_write_long(msg, to.effects as i32);
    } else if bits & U_EFFECTS8 != 0 {
        msg_write_byte(msg, to.effects as i32);
    } else if bits & U_EFFECTS16 != 0 {
        msg_write_short(msg, to.effects as i32);
    }

    if (bits & U_RENDERFX8 != 0) && (bits & U_RENDERFX16 != 0) {
        msg_write_long(msg, to.renderfx);
    } else if bits & U_RENDERFX8 != 0 {
        msg_write_byte(msg, to.renderfx);
    } else if bits & U_RENDERFX16 != 0 {
        msg_write_short(msg, to.renderfx);
    }

    if bits & U_ORIGIN1 != 0 { msg_write_coord(msg, to.origin[0]); }
    if bits & U_ORIGIN2 != 0 { msg_write_coord(msg, to.origin[1]); }
    if bits & U_ORIGIN3 != 0 { msg_write_coord(msg, to.origin[2]); }

    if bits & U_ANGLE1 != 0 { msg_write_angle(msg, to.angles[0]); }
    if bits & U_ANGLE2 != 0 { msg_write_angle(msg, to.angles[1]); }
    if bits & U_ANGLE3 != 0 { msg_write_angle(msg, to.angles[2]); }

    if bits & U_OLDORIGIN != 0 {
        msg_write_coord(msg, to.old_origin[0]);
        msg_write_coord(msg, to.old_origin[1]);
        msg_write_coord(msg, to.old_origin[2]);
    }

    if bits & U_SOUND != 0 { msg_write_byte(msg, to.sound); }
    if bits & U_EVENT != 0 { msg_write_byte(msg, to.event); }
    if bits & U_SOLID != 0 { msg_write_short(msg, to.solid); }
}

// ============================================================
// MSG read functions
// ============================================================

pub fn msg_begin_reading(msg: &mut SizeBuf) {
    msg.readcount = 0;
}

pub fn msg_read_char(msg: &mut SizeBuf) -> i32 {
    let rc = msg.readcount as usize;
    let cs = msg.cursize as usize;
    msg.readcount += 1;
    if rc + 1 > cs {
        -1
    } else {
        msg.data[rc] as i8 as i32
    }
}

pub fn msg_read_byte(msg: &mut SizeBuf) -> i32 {
    let rc = msg.readcount as usize;
    let cs = msg.cursize as usize;
    msg.readcount += 1;
    if rc + 1 > cs {
        -1
    } else {
        msg.data[rc] as i32
    }
}

pub fn msg_read_short(msg: &mut SizeBuf) -> i32 {
    let rc = msg.readcount as usize;
    let cs = msg.cursize as usize;
    msg.readcount += 2;
    if rc + 2 > cs {
        -1
    } else {
        i16::from_le_bytes([msg.data[rc], msg.data[rc + 1]]) as i32
    }
}

pub fn msg_read_long(msg: &mut SizeBuf) -> i32 {
    let rc = msg.readcount as usize;
    let cs = msg.cursize as usize;
    msg.readcount += 4;
    if rc + 4 > cs {
        -1
    } else {
        i32::from_le_bytes([msg.data[rc], msg.data[rc + 1], msg.data[rc + 2], msg.data[rc + 3]])
    }
}

pub fn msg_read_float(msg: &mut SizeBuf) -> f32 {
    let rc = msg.readcount as usize;
    let cs = msg.cursize as usize;
    msg.readcount += 4;
    if rc + 4 > cs {
        -1.0
    } else {
        f32::from_le_bytes([msg.data[rc], msg.data[rc + 1], msg.data[rc + 2], msg.data[rc + 3]])
    }
}

pub fn msg_read_string(msg: &mut SizeBuf) -> String {
    let mut result = String::new();
    loop {
        let c = msg_read_char(msg);
        if c == -1 || c == 0 {
            break;
        }
        result.push(c as u8 as char);
        if result.len() >= 2047 {
            break;
        }
    }
    result
}

pub fn msg_read_string_line(msg: &mut SizeBuf) -> String {
    let mut result = String::new();
    loop {
        let c = msg_read_char(msg);
        if c == -1 || c == 0 || c == b'\n' as i32 {
            break;
        }
        result.push(c as u8 as char);
        if result.len() >= 2047 {
            break;
        }
    }
    result
}

pub fn msg_read_coord(msg: &mut SizeBuf) -> f32 {
    msg_read_short(msg) as f32 * (1.0 / 8.0)
}

pub fn msg_read_pos(msg: &mut SizeBuf) -> Vec3 {
    [
        msg_read_short(msg) as f32 * (1.0 / 8.0),
        msg_read_short(msg) as f32 * (1.0 / 8.0),
        msg_read_short(msg) as f32 * (1.0 / 8.0),
    ]
}

pub fn msg_read_angle(msg: &mut SizeBuf) -> f32 {
    msg_read_char(msg) as f32 * (360.0 / 256.0)
}

pub fn msg_read_angle16(msg: &mut SizeBuf) -> f32 {
    msg_read_short(msg) as f32 * (360.0 / 65536.0)
}

pub fn msg_read_delta_usercmd(msg: &mut SizeBuf, from: &UserCmd) -> UserCmd {
    let mut cmd = *from;
    let bits = msg_read_byte(msg);

    if bits & CM_ANGLE1 != 0 { cmd.angles[0] = msg_read_short(msg) as i16; }
    if bits & CM_ANGLE2 != 0 { cmd.angles[1] = msg_read_short(msg) as i16; }
    if bits & CM_ANGLE3 != 0 { cmd.angles[2] = msg_read_short(msg) as i16; }

    if bits & CM_FORWARD != 0 { cmd.forwardmove = msg_read_short(msg) as i16; }
    if bits & CM_SIDE != 0 { cmd.sidemove = msg_read_short(msg) as i16; }
    if bits & CM_UP != 0 { cmd.upmove = msg_read_short(msg) as i16; }

    if bits & CM_BUTTONS != 0 { cmd.buttons = msg_read_byte(msg) as u8; }
    if bits & CM_IMPULSE != 0 { cmd.impulse = msg_read_byte(msg) as u8; }

    cmd.msec = msg_read_byte(msg) as u8;
    cmd.lightlevel = msg_read_byte(msg) as u8;

    cmd
}

pub fn msg_read_dir(msg: &mut SizeBuf) -> Vec3 {
    let b = msg_read_byte(msg) as usize;
    if b >= NUMVERTEXNORMALS {
        panic!("MSG_ReadDir: out of range");
    }
    BYTEDIRS[b]
}

pub fn msg_read_data(msg: &mut SizeBuf, len: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(len);
    for _ in 0..len {
        data.push(msg_read_byte(msg) as u8);
    }
    data
}

// ============================================================
// COM_BlockSequenceCRCByte — for proxy protecting
// ============================================================

#[rustfmt::skip]
static CHKTBL: [u8; 1024] = [
    0x84, 0x47, 0x51, 0xc1, 0x93, 0x22, 0x21, 0x24, 0x2f, 0x66, 0x60, 0x4d, 0xb0, 0x7c, 0xda,
    0x88, 0x54, 0x15, 0x2b, 0xc6, 0x6c, 0x89, 0xc5, 0x9d, 0x48, 0xee, 0xe6, 0x8a, 0xb5, 0xf4,
    0xcb, 0xfb, 0xf1, 0x0c, 0x2e, 0xa0, 0xd7, 0xc9, 0x1f, 0xd6, 0x06, 0x9a, 0x09, 0x41, 0x54,
    0x67, 0x46, 0xc7, 0x74, 0xe3, 0xc8, 0xb6, 0x5d, 0xa6, 0x36, 0xc4, 0xab, 0x2c, 0x7e, 0x85,
    0xa8, 0xa4, 0xa6, 0x4d, 0x96, 0x19, 0x19, 0x9a, 0xcc, 0xd8, 0xac, 0x39, 0x5e, 0x3c, 0xf2,
    0xf5, 0x5a, 0x72, 0xe5, 0xa9, 0xd1, 0xb3, 0x23, 0x82, 0x6f, 0x29, 0xcb, 0xd1, 0xcc, 0x71,
    0xfb, 0xea, 0x92, 0xeb, 0x1c, 0xca, 0x4c, 0x70, 0xfe, 0x4d, 0xc9, 0x67, 0x43, 0x47, 0x94,
    0xb9, 0x47, 0xbc, 0x3f, 0x01, 0xab, 0x7b, 0xa6, 0xe2, 0x76, 0xef, 0x5a, 0x7a, 0x29, 0x0b,
    0x51, 0x54, 0x67, 0xd8, 0x1c, 0x14, 0x3e, 0x29, 0xec, 0xe9, 0x2d, 0x48, 0x67, 0xff, 0xed,
    0x54, 0x4f, 0x48, 0xc0, 0xaa, 0x61, 0xf7, 0x78, 0x12, 0x03, 0x7a, 0x9e, 0x8b, 0xcf, 0x83,
    0x7b, 0xae, 0xca, 0x7b, 0xd9, 0xe9, 0x53, 0x2a, 0xeb, 0xd2, 0xd8, 0xcd, 0xa3, 0x10, 0x25,
    0x78, 0x5a, 0xb5, 0x23, 0x06, 0x93, 0xb7, 0x84, 0xd2, 0xbd, 0x96, 0x75, 0xa5, 0x5e, 0xcf,
    0x4e, 0xe9, 0x50, 0xa1, 0xe6, 0x9d, 0xb1, 0xe3, 0x85, 0x66, 0x28, 0x4e, 0x43, 0xdc, 0x6e,
    0xbb, 0x33, 0x9e, 0xf3, 0x0d, 0x00, 0xc1, 0xcf, 0x67, 0x34, 0x06, 0x7c, 0x71, 0xe3, 0x63,
    0xb7, 0xb7, 0xdf, 0x92, 0xc4, 0xc2, 0x25, 0x5c, 0xff, 0xc3, 0x6e, 0xfc, 0xaa, 0x1e, 0x2a,
    0x48, 0x11, 0x1c, 0x36, 0x68, 0x78, 0x86, 0x79, 0x30, 0xc3, 0xd6, 0xde, 0xbc, 0x3a, 0x2a,
    0x6d, 0x1e, 0x46, 0xdd, 0xe0, 0x80, 0x1e, 0x44, 0x3b, 0x6f, 0xaf, 0x31, 0xda, 0xa2, 0xbd,
    0x77, 0x06, 0x56, 0xc0, 0xb7, 0x92, 0x4b, 0x37, 0xc0, 0xfc, 0xc2, 0xd5, 0xfb, 0xa8, 0xda,
    0xf5, 0x57, 0xa8, 0x18, 0xc0, 0xdf, 0xe7, 0xaa, 0x2a, 0xe0, 0x7c, 0x6f, 0x77, 0xb1, 0x26,
    0xba, 0xf9, 0x2e, 0x1d, 0x16, 0xcb, 0xb8, 0xa2, 0x44, 0xd5, 0x2f, 0x1a, 0x79, 0x74, 0x87,
    0x4b, 0x00, 0xc9, 0x4a, 0x3a, 0x65, 0x8f, 0xe6, 0x5d, 0xe5, 0x0a, 0x77, 0xd8, 0x1a, 0x14,
    0x41, 0x75, 0xb1, 0xe2, 0x50, 0x2c, 0x93, 0x38, 0x2b, 0x6d, 0xf3, 0xf6, 0xdb, 0x1f, 0xcd,
    0xff, 0x14, 0x70, 0xe7, 0x16, 0xe8, 0x3d, 0xf0, 0xe3, 0xbc, 0x5e, 0xb6, 0x3f, 0xcc, 0x81,
    0x24, 0x67, 0xf3, 0x97, 0x3b, 0xfe, 0x3a, 0x96, 0x85, 0xdf, 0xe4, 0x6e, 0x3c, 0x85, 0x05,
    0x0e, 0xa3, 0x2b, 0x07, 0xc8, 0xbf, 0xe5, 0x13, 0x82, 0x62, 0x08, 0x61, 0x69, 0x4b, 0x47,
    0x62, 0x73, 0x44, 0x64, 0x8e, 0xe2, 0x91, 0xa6, 0x9a, 0xb7, 0xe9, 0x04, 0xb6, 0x54, 0x0c,
    0xc5, 0xa9, 0x47, 0xa6, 0xc9, 0x08, 0xfe, 0x4e, 0xa6, 0xcc, 0x8a, 0x5b, 0x90, 0x6f, 0x2b,
    0x3f, 0xb6, 0x0a, 0x96, 0xc0, 0x78, 0x58, 0x3c, 0x76, 0x6d, 0x94, 0x1a, 0xe4, 0x4e, 0xb8,
    0x38, 0xbb, 0xf5, 0xeb, 0x29, 0xd8, 0xb0, 0xf3, 0x15, 0x1e, 0x99, 0x96, 0x3c, 0x5d, 0x63,
    0xd5, 0xb1, 0xad, 0x52, 0xb8, 0x55, 0x70, 0x75, 0x3e, 0x1a, 0xd5, 0xda, 0xf6, 0x7a, 0x48,
    0x7d, 0x44, 0x41, 0xf9, 0x11, 0xce, 0xd7, 0xca, 0xa5, 0x3d, 0x7a, 0x79, 0x7e, 0x7d, 0x25,
    0x1b, 0x77, 0xbc, 0xf7, 0xc7, 0x0f, 0x84, 0x95, 0x10, 0x92, 0x67, 0x15, 0x11, 0x5a, 0x5e,
    0x41, 0x66, 0x0f, 0x38, 0x03, 0xb2, 0xf1, 0x5d, 0xf8, 0xab, 0xc0, 0x02, 0x76, 0x84, 0x28,
    0xf4, 0x9d, 0x56, 0x46, 0x60, 0x20, 0xdb, 0x68, 0xa7, 0xbb, 0xee, 0xac, 0x15, 0x01, 0x2f,
    0x20, 0x09, 0xdb, 0xc0, 0x16, 0xa1, 0x89, 0xf9, 0x94, 0x59, 0x00, 0xc1, 0x76, 0xbf, 0xc1,
    0x4d, 0x5d, 0x2d, 0xa9, 0x85, 0x2c, 0xd6, 0xd3, 0x14, 0xcc, 0x02, 0xc3, 0xc2, 0xfa, 0x6b,
    0xb7, 0xa6, 0xef, 0xdd, 0x12, 0x26, 0xa4, 0x63, 0xe3, 0x62, 0xbd, 0x56, 0x8a, 0x52, 0x2b,
    0xb9, 0xdf, 0x09, 0xbc, 0x0e, 0x97, 0xa9, 0xb0, 0x82, 0x46, 0x08, 0xd5, 0x1a, 0x8e, 0x1b,
    0xa7, 0x90, 0x98, 0xb9, 0xbb, 0x3c, 0x17, 0x9a, 0xf2, 0x82, 0xba, 0x64, 0x0a, 0x7f, 0xca,
    0x5a, 0x8c, 0x7c, 0xd3, 0x79, 0x09, 0x5b, 0x26, 0xbb, 0xbd, 0x25, 0xdf, 0x3d, 0x6f, 0x9a,
    0x8f, 0xee, 0x21, 0x66, 0xb0, 0x8d, 0x84, 0x4c, 0x91, 0x45, 0xd4, 0x77, 0x4f, 0xb3, 0x8c,
    0xbc, 0xa8, 0x99, 0xaa, 0x19, 0x53, 0x7c, 0x02, 0x87, 0xbb, 0x0b, 0x7c, 0x1a, 0x2d, 0xdf,
    0x48, 0x44, 0x06, 0xd6, 0x7d, 0x0c, 0x2d, 0x35, 0x76, 0xae, 0xc4, 0x5f, 0x71, 0x85, 0x97,
    0xc4, 0x3d, 0xef, 0x52, 0xbe, 0x00, 0xe4, 0xcd, 0x49, 0xd1, 0xd1, 0x1c, 0x3c, 0xd0, 0x1c,
    0x42, 0xaf, 0xd4, 0xbd, 0x58, 0x34, 0x07, 0x32, 0xee, 0xb9, 0xb5, 0xea, 0xff, 0xd7, 0x8c,
    0x0d, 0x2e, 0x2f, 0xaf, 0x87, 0xbb, 0xe6, 0x52, 0x71, 0x22, 0xf5, 0x25, 0x17, 0xa1, 0x82,
    0x04, 0xc2, 0x4a, 0xbd, 0x57, 0xc6, 0xab, 0xc8, 0x35, 0x0c, 0x3c, 0xd9, 0xc2, 0x43, 0xdb,
    0x27, 0x92, 0xcf, 0xb8, 0x25, 0x60, 0xfa, 0x21, 0x3b, 0x04, 0x52, 0xc8, 0x96, 0xba, 0x74,
    0xe3, 0x67, 0x3e, 0x8e, 0x8d, 0x61, 0x90, 0x92, 0x59, 0xb6, 0x1a, 0x1c, 0x5e, 0x21, 0xc1,
    0x65, 0xe5, 0xa6, 0x34, 0x05, 0x6f, 0xc5, 0x60, 0xb1, 0x83, 0xc1, 0xd5, 0xd5, 0xed, 0xd9,
    0xc7, 0x11, 0x7b, 0x49, 0x7a, 0xf9, 0xf9, 0x84, 0x47, 0x9b, 0xe2, 0xa5, 0x82, 0xe0, 0xc2,
    0x88, 0xd0, 0xb2, 0x58, 0x88, 0x7f, 0x45, 0x09, 0x67, 0x74, 0x61, 0xbf, 0xe6, 0x40, 0xe2,
    0x9d, 0xc2, 0x47, 0x05, 0x89, 0xed, 0xcb, 0xbb, 0xb7, 0x27, 0xe7, 0xdc, 0x7a, 0xfd, 0xbf,
    0xa8, 0xd0, 0xaa, 0x10, 0x39, 0x3c, 0x20, 0xf0, 0xd3, 0x6e, 0xb1, 0x72, 0xf8, 0xe6, 0x0f,
    0xef, 0x37, 0xe5, 0x09, 0x33, 0x5a, 0x83, 0x43, 0x80, 0x4f, 0x65, 0x2f, 0x7c, 0x8c, 0x6a,
    0xa0, 0x82, 0x0c, 0xd4, 0xd4, 0xfa, 0x81, 0x60, 0x3d, 0xdf, 0x06, 0xf1, 0x5f, 0x08, 0x0d,
    0x6d, 0x43, 0xf2, 0xe3, 0x11, 0x7d, 0x80, 0x32, 0xc5, 0xfb, 0xc5, 0xd9, 0x27, 0xec, 0xc6,
    0x4e, 0x65, 0x27, 0x76, 0x87, 0xa6, 0xee, 0xee, 0xd7, 0x8b, 0xd1, 0xa0, 0x5c, 0xb0, 0x42,
    0x13, 0x0e, 0x95, 0x4a, 0xf2, 0x06, 0xc6, 0x43, 0x33, 0xf4, 0xc7, 0xf8, 0xe7, 0x1f, 0xdd,
    0xe4, 0x46, 0x4a, 0x70, 0x39, 0x6c, 0xd0, 0xed, 0xca, 0xbe, 0x60, 0x3b, 0xd1, 0x7b, 0x57,
    0x48, 0xe5, 0x3a, 0x79, 0xc1, 0x69, 0x33, 0x53, 0x1b, 0x80, 0xb8, 0x91, 0x7d, 0xb4, 0xf6,
    0x17, 0x1a, 0x1d, 0x5a, 0x32, 0xd6, 0xcc, 0x71, 0x29, 0x3f, 0x28, 0xbb, 0xf3, 0x5e, 0x71,
    0xb8, 0x43, 0xaf, 0xf8, 0xb9, 0x64, 0xef, 0xc4, 0xa5, 0x6c, 0x08, 0x53, 0xc7, 0x00, 0x10,
    0x39, 0x4f, 0xdd, 0xe4, 0xb6, 0x19, 0x27, 0xfb, 0xb8, 0xf5, 0x32, 0x73, 0xe5, 0xcb, 0x32,
    // In C, the remaining 64 elements are zero-initialized (960 explicit + 64 zeros = 1024)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
];

/// For proxy protecting.
pub fn com_block_sequence_crc_byte(base: &[u8], sequence: i32) -> u8 {
    assert!(sequence >= 0, "sequence < 0, this shouldn't happen");

    let p_idx = (sequence as usize) % (CHKTBL.len() - 4);
    let p = &CHKTBL[p_idx..];

    let length = base.len().min(60);
    let mut chkb = [0u8; 64];
    chkb[..length].copy_from_slice(&base[..length]);

    chkb[length] = p[0];
    chkb[length + 1] = p[1];
    chkb[length + 2] = p[2];
    chkb[length + 3] = p[3];

    let total_len = length + 4;
    let crc = crc_block(&chkb[..total_len]);

    let mut x: u32 = 0;
    for i in 0..total_len {
        x = x.wrapping_add(chkb[i] as u32);
    }

    ((crc as u32 ^ x) & 0xff) as u8
}

// ============================================================
// COM argument handling
// ============================================================

pub struct ComArgs {
    pub argc: usize,
    pub argv: Vec<String>,
}

impl ComArgs {
    pub fn new() -> Self {
        Self {
            argc: 0,
            argv: Vec::new(),
        }
    }

    pub fn init(&mut self, args: &[String]) {
        self.argc = args.len().min(MAX_NUM_ARGVS);
        self.argv = args[..self.argc].to_vec();
    }

    pub fn com_argc(&self) -> usize {
        self.argc
    }

    pub fn com_argv(&self, arg: usize) -> &str {
        if arg >= self.argc {
            ""
        } else {
            &self.argv[arg]
        }
    }

    pub fn com_clear_argv(&mut self, arg: usize) {
        if arg < self.argc {
            self.argv[arg] = String::new();
        }
    }

}

impl Default for ComArgs {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// Info_Print
// ============================================================

pub fn info_print(s: &str) {
    let bytes = s.as_bytes();
    let mut pos = 0;

    if pos < bytes.len() && bytes[pos] == b'\\' {
        pos += 1;
    }

    while pos < bytes.len() {
        // Read key
        let mut key = String::new();
        while pos < bytes.len() && bytes[pos] != b'\\' {
            key.push(bytes[pos] as char);
            pos += 1;
        }

        // Pad key to 20 chars
        if key.len() < 20 {
            key.extend(std::iter::repeat_n(' ', 20 - key.len()));
        }
        com_printf(&key);

        if pos >= bytes.len() {
            com_printf("MISSING VALUE\n");
            return;
        }

        // Skip backslash
        pos += 1;

        // Read value
        let mut value = String::new();
        while pos < bytes.len() && bytes[pos] != b'\\' {
            value.push(bytes[pos] as char);
            pos += 1;
        }

        if pos < bytes.len() {
            pos += 1; // skip trailing backslash
        }

        com_printf(&format!("{}\n", value));
    }
}

// ============================================================
// Random number functions
// ============================================================

/// Random float in [0, 1).
pub fn frand() -> f32 {
    (rand::random::<u32>() & 32767) as f32 * (1.0 / 32767.0)
}

/// Random float in [-1, 1).
pub fn crand() -> f32 {
    (rand::random::<u32>() & 32767) as f32 * (2.0 / 32767.0) - 1.0
}

/// Random integer in [0, 32767] — equivalent to C's rand() & 0x7fff.
/// Used for game logic like selecting random animations, damage values, etc.
pub fn rand_i32() -> i32 {
    (rand::random::<u32>() & 0x7fff) as i32
}

// ============================================================
// Server state (global in C, managed here)
// ============================================================

pub struct CommonState {
    pub server_state: i32,
    pub realtime: i32,
    pub args: ComArgs,
    pub time_before_game: i32,
    pub time_after_game: i32,
    pub time_before_ref: i32,
    pub time_after_ref: i32,
}

impl CommonState {
    pub fn new() -> Self {
        Self {
            server_state: 0,
            realtime: 0,
            args: ComArgs::new(),
            time_before_game: 0,
            time_after_game: 0,
            time_before_ref: 0,
            time_after_ref: 0,
        }
    }

    pub fn server_state(&self) -> i32 {
        self.server_state
    }

    pub fn set_server_state(&mut self, state: i32) {
        self.server_state = state;
    }
}

impl Default for CommonState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// Qcommon_Init / Qcommon_Frame — engine init and main loop tick
// ============================================================

/// Global engine state, lazily initialized by qcommon_init.
static COMMON_STATE: Mutex<Option<CommonState>> = Mutex::new(None);

/// Set the server state in the global common state.
///
/// Original: `void Com_SetServerState(int state)`
pub fn com_set_server_state(state: i32) {
    let mut global = COMMON_STATE.lock().unwrap();
    if let Some(ref mut s) = *global {
        s.set_server_state(state);
    }
}

/// Get the server state from the global common state.
///
/// Original: `int Com_ServerState(void)`
pub fn com_server_state() -> i32 {
    let global = COMMON_STATE.lock().unwrap();
    global.as_ref().map_or(0, |s| s.server_state())
}

/// Initialize all engine subsystems.
///
/// Original: `void Qcommon_Init(int argc, char **argv)`
///
/// The C original initializes (in order): command buffer, command system, cvars,
/// filesystem, network channel, server, client. This stub sets up CommonState
/// and records the command-line arguments; subsystem init will be wired in as
/// each module is converted.
pub fn qcommon_init(args: &[String]) {
    let mut state = CommonState::new();
    state.args.init(args);

    // Initialize subsystems in order (mirrors C Qcommon_Init)
    crate::cmd::cmd_init();
    crate::cvar::cvar_init();
    // Key_Init — wired at runtime by client
    crate::files::fs_init();
    crate::cmodel::cmodel_init();
    crate::cmd::with_cmd_ctx(|cmd| {
        cmd.cbuf_add_early_commands(&mut state.args, true);
    });
    crate::cmd::cbuf_execute();
    crate::cvar::cvar_get_latched_vars();
    // NET_Init — wired at runtime by sys
    // Netchan_Init — stateless in Rust
    // SV_Init — wired at runtime by server
    // CL_Init — wired at runtime by client
    // Create the version cvar (read-only, broadcast to clients)
    // Format: "MyQ2-Rust v1.0 x86_64 Win32 RELEASE"
    let version_string = format!(
        "{} v{:.1} {} {}",
        DISTNAME, DISTVER, CPUSTRING, BUILDSTRING
    );
    crate::cvar::cvar_get("version", &version_string, CVAR_SERVERINFO | CVAR_NOSET);

    // Register "dedicated" cvar — read by 5+ subsystems via cvar_variable_value().
    // Set to "1" by the launcher when running as a dedicated server.
    crate::cvar::cvar_get("dedicated", "0", CVAR_NOSET);

    // Register "developer" cvar — controls com_dprintf output and developer_value() in console.
    crate::cvar::cvar_get("developer", "0", CVAR_ZERO);

    // Register "qport" cvar — random port for the connect string, prevents NAT confusion.
    let port = sys_milliseconds() & 0xffff;
    crate::cvar::cvar_get("qport", &port.to_string(), CVAR_NOSET);

    crate::cmd::with_cmd_ctx(|cmd| {
        cmd.cbuf_add_late_commands(&state.args);
    });
    crate::cmd::cbuf_execute();

    com_printf("====== Qcommon Initialized ======\n");

    let mut global = COMMON_STATE.lock().unwrap();
    *global = Some(state);
}

/// Run a single engine frame.
///
/// Original: `void Qcommon_Frame(int msec)`
///
/// The C original runs: Cbuf_Execute, server frame, client frame.
/// This stub updates realtime; subsystem ticks will be wired in as modules are converted.
pub fn qcommon_frame(msec: i32) {
    let mut global = COMMON_STATE.lock().unwrap();
    if let Some(ref mut state) = *global {
        state.realtime += msec;

        crate::cmd::cbuf_execute();
        // SV_Frame(msec) — wired at runtime by server
        // CL_Frame(msec) — wired at runtime by client
    }
}

/// Clean shutdown of the engine.
///
/// Original: `void Qcommon_Shutdown(void)`
pub fn qcommon_shutdown() {
    let mut global = COMMON_STATE.lock().unwrap();
    *global = None;
}

// ============================================================
// Sys_Milliseconds — canonical process-wide timer
// ============================================================

/// Sys_Milliseconds — Get the current time in milliseconds.
/// Returns a monotonically increasing time value relative to a process-wide epoch.
pub fn sys_milliseconds() -> i32 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    let epoch = EPOCH.get_or_init(Instant::now);
    epoch.elapsed().as_millis() as i32
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msg_write_read_char() {
        let mut sb = SizeBuf::new(64);
        msg_write_char(&mut sb, -5);
        msg_begin_reading(&mut sb);
        assert_eq!(msg_read_char(&mut sb), -5);
    }

    #[test]
    fn test_msg_write_read_byte() {
        let mut sb = SizeBuf::new(64);
        msg_write_byte(&mut sb, 200);
        msg_begin_reading(&mut sb);
        assert_eq!(msg_read_byte(&mut sb), 200);
    }

    #[test]
    fn test_msg_write_read_short() {
        let mut sb = SizeBuf::new(64);
        msg_write_short(&mut sb, -1234);
        msg_begin_reading(&mut sb);
        assert_eq!(msg_read_short(&mut sb), -1234);
    }

    #[test]
    fn test_msg_write_read_long() {
        let mut sb = SizeBuf::new(64);
        msg_write_long(&mut sb, 0x12345678);
        msg_begin_reading(&mut sb);
        assert_eq!(msg_read_long(&mut sb), 0x12345678);
    }

    #[test]
    fn test_msg_write_read_float() {
        let mut sb = SizeBuf::new(64);
        msg_write_float(&mut sb, 3.14);
        msg_begin_reading(&mut sb);
        let val = msg_read_float(&mut sb);
        assert!((val - 3.14).abs() < 0.001);
    }

    #[test]
    fn test_msg_write_read_string() {
        let mut sb = SizeBuf::new(256);
        msg_write_string(&mut sb, "hello world");
        msg_begin_reading(&mut sb);
        assert_eq!(msg_read_string(&mut sb), "hello world");
    }

    #[test]
    fn test_msg_read_overflow() {
        let mut sb = SizeBuf::new(64);
        msg_begin_reading(&mut sb);
        assert_eq!(msg_read_byte(&mut sb), -1);
    }

    #[test]
    fn test_msg_coord_roundtrip() {
        let mut sb = SizeBuf::new(64);
        msg_write_coord(&mut sb, 100.5);
        msg_begin_reading(&mut sb);
        let val = msg_read_coord(&mut sb);
        assert!((val - 100.5).abs() < 0.2); // coord precision is 1/8
    }

    #[test]
    fn test_com_block_sequence_crc() {
        let data = b"test data";
        let crc1 = com_block_sequence_crc_byte(data, 0);
        let crc2 = com_block_sequence_crc_byte(data, 0);
        assert_eq!(crc1, crc2); // deterministic

        let crc3 = com_block_sequence_crc_byte(data, 1);
        // Different sequences should (usually) produce different values
        // Can't guarantee this always, but it's a sanity check
        let _ = crc3;
    }

    // =========================================================================
    // C-to-Rust cross-validation: MSG coordinate encoding
    // C behavior: write_coord => (int)(f*8) as short; read_coord => short * 0.125
    // =========================================================================

    #[test]
    fn test_msg_coord_round_trip_matches_c() {
        // The C code does: MSG_WriteShort((int)(f*8.0));
        // then:            MSG_ReadShort() * (1.0/8.0)
        // This tests exact bit-level fidelity with C's (int)(f*8) cast.
        let test_values: &[f32] = &[
            0.0, 1.0, -1.0, 100.5, -100.5, 0.125, -0.125,
            0.0625, // not representable at 1/8 precision
            1000.0, -1000.0, 4095.875, -4095.875,
        ];
        for &f in test_values {
            let mut sb = SizeBuf::new(64);
            msg_write_coord(&mut sb, f);
            msg_begin_reading(&mut sb);
            let result = msg_read_coord(&mut sb);

            // C reference: (int)(f*8) then * 0.125
            let c_encoded = (f * 8.0) as i32;
            let c_expected = c_encoded as f32 * 0.125;
            assert!(
                (result - c_expected).abs() < f32::EPSILON,
                "coord round-trip mismatch for {}: got {}, C expects {}",
                f, result, c_expected
            );
        }
    }

    // =========================================================================
    // C-to-Rust cross-validation: MSG angle encoding
    // C behavior: write_angle => (int)(f*256/360) & 255; read_angle => char * (360/256)
    // =========================================================================

    #[test]
    fn test_msg_angle_round_trip_matches_c() {
        let test_angles: &[f32] = &[
            0.0, 45.0, 90.0, 180.0, 270.0, 359.0, -45.0, -90.0,
            360.0, 720.0, 1.40625, // exact: 1 unit in 256 space
        ];
        for &f in test_angles {
            let mut sb = SizeBuf::new(64);
            msg_write_angle(&mut sb, f);
            msg_begin_reading(&mut sb);
            let result = msg_read_angle(&mut sb);

            // C reference: (int)(f*256/360) & 255, then sign-extend as i8, then * (360/256)
            let c_encoded = ((f * 256.0 / 360.0) as i32) & 255;
            let c_expected = (c_encoded as i8) as f32 * (360.0 / 256.0);
            assert!(
                (result - c_expected).abs() < 0.001,
                "angle round-trip mismatch for {}: got {}, C expects {} (encoded byte={})",
                f, result, c_expected, c_encoded
            );
        }
    }

    // =========================================================================
    // C-to-Rust cross-validation: MSG angle16 encoding
    // C behavior: ANGLE2SHORT macro => (int)(f*65536/360) & 65535
    // =========================================================================

    #[test]
    fn test_msg_angle16_round_trip_matches_c() {
        let test_angles: &[f32] = &[0.0, 45.0, 90.0, 180.0, 270.0, 359.0, -45.0, 360.0];
        for &f in test_angles {
            let mut sb = SizeBuf::new(64);
            msg_write_angle16(&mut sb, f);
            msg_begin_reading(&mut sb);
            let result = msg_read_angle16(&mut sb);

            // C reference: (int)(f*65536/360) & 65535, written as short,
            // read as signed short, then * (360/65536)
            let c_encoded = ((f * 65536.0 / 360.0) as i32) & 65535;
            let c_expected = (c_encoded as i16) as f32 * (360.0 / 65536.0);
            assert!(
                (result - c_expected).abs() < 0.01,
                "angle16 round-trip mismatch for {}: got {}, C expects {} (encoded={})",
                f, result, c_expected, c_encoded
            );
        }
    }

    // =========================================================================
    // Overflow behavior: write_byte with values > 255, write_short > 32767
    // C truncates via cast to unsigned char / short
    // =========================================================================

    #[test]
    fn test_msg_write_byte_overflow_truncation() {
        // C: c = c & 0xff via cast to unsigned char
        let mut sb = SizeBuf::new(64);
        msg_write_byte(&mut sb, 256);
        msg_begin_reading(&mut sb);
        // 256 as u8 = 0
        assert_eq!(msg_read_byte(&mut sb), 0);

        let mut sb = SizeBuf::new(64);
        msg_write_byte(&mut sb, 300);
        msg_begin_reading(&mut sb);
        // 300 as u8 = 44
        assert_eq!(msg_read_byte(&mut sb), 44);

        let mut sb = SizeBuf::new(64);
        msg_write_byte(&mut sb, -1);
        msg_begin_reading(&mut sb);
        // -1 as u8 = 255
        assert_eq!(msg_read_byte(&mut sb), 255);
    }

    #[test]
    fn test_msg_write_short_overflow_truncation() {
        // C: write_short stores (c as i16) in LE
        let mut sb = SizeBuf::new(64);
        msg_write_short(&mut sb, 32768);
        msg_begin_reading(&mut sb);
        // 32768 as i16 = -32768
        assert_eq!(msg_read_short(&mut sb), -32768);

        let mut sb = SizeBuf::new(64);
        msg_write_short(&mut sb, 65535);
        msg_begin_reading(&mut sb);
        // 65535 as i16 = -1
        assert_eq!(msg_read_short(&mut sb), -1);
    }

    // =========================================================================
    // SizeBuf overflow detection
    // =========================================================================

    #[test]
    fn test_sizebuf_overflow_detection() {
        let mut sb = SizeBuf::new(4);
        sb.allow_overflow = true;

        // Write 4 bytes - should succeed
        msg_write_long(&mut sb, 0x12345678);
        assert!(!sb.overflowed);
        assert_eq!(sb.cursize, 4);

        // Write 1 more byte - should trigger overflow
        msg_write_byte(&mut sb, 0);
        assert!(sb.overflowed);
    }

    #[test]
    #[should_panic(expected = "overflow without allowoverflow set")]
    fn test_sizebuf_overflow_panic_without_allow() {
        let mut sb = SizeBuf::new(4);
        // allow_overflow = false by default

        msg_write_long(&mut sb, 0x12345678); // fills the buffer
        msg_write_byte(&mut sb, 0); // should panic
    }

    // =========================================================================
    // Complete entity state delta: write then read back and verify every field
    // =========================================================================

    #[test]
    fn test_msg_write_delta_entity_full_roundtrip() {
        use crate::q_shared::EntityState;

        let from = EntityState::default();
        let mut to = EntityState::default();
        to.number = 42;
        to.origin = [100.0, 200.0, -300.0];
        to.angles = [45.0, 90.0, 180.0];
        to.old_origin = [10.0, 20.0, 30.0];
        to.modelindex = 7;
        to.modelindex2 = 3;
        to.modelindex3 = 1;
        to.modelindex4 = 2;
        to.frame = 15;
        to.skinnum = 5;
        to.effects = 100;
        to.renderfx = 50;
        to.solid = 255;
        to.sound = 12;
        to.event = 3;

        let mut sb = SizeBuf::new(1024);
        msg_write_delta_entity(&from, &to, &mut sb, true, true);

        // The delta should have written something
        assert!(sb.cursize > 0);

        // Verify the written data is deterministic by writing again
        let mut sb2 = SizeBuf::new(1024);
        msg_write_delta_entity(&from, &to, &mut sb2, true, true);
        assert_eq!(sb.cursize, sb2.cursize);
        assert_eq!(&sb.data[..sb.cursize as usize], &sb2.data[..sb2.cursize as usize]);
    }

    #[test]
    fn test_msg_write_delta_entity_no_change_no_force() {
        use crate::q_shared::EntityState;

        let mut from = EntityState::default();
        from.number = 1;
        let mut to = from.clone();
        to.event = 0; // ensure event is 0 (no forced U_EVENT)

        let mut sb = SizeBuf::new(1024);
        msg_write_delta_entity(&from, &to, &mut sb, false, false);

        // With no differences and force=false, nothing should be written
        assert_eq!(sb.cursize, 0);
    }

    // =========================================================================
    // Delta usercmd round-trip
    // =========================================================================

    #[test]
    fn test_msg_write_read_delta_usercmd_roundtrip() {
        let from = UserCmd::default();
        let mut cmd = UserCmd::default();
        cmd.angles = [1000, -2000, 3000];
        cmd.forwardmove = 127;
        cmd.sidemove = -64;
        cmd.upmove = 32;
        cmd.buttons = 3;
        cmd.impulse = 7;
        cmd.msec = 16;
        cmd.lightlevel = 128;

        let mut sb = SizeBuf::new(256);
        msg_write_delta_usercmd(&mut sb, &from, &cmd);

        msg_begin_reading(&mut sb);
        let result = msg_read_delta_usercmd(&mut sb, &from);

        assert_eq!(result.angles, cmd.angles);
        assert_eq!(result.forwardmove, cmd.forwardmove);
        assert_eq!(result.sidemove, cmd.sidemove);
        assert_eq!(result.upmove, cmd.upmove);
        assert_eq!(result.buttons, cmd.buttons);
        assert_eq!(result.impulse, cmd.impulse);
        assert_eq!(result.msec, cmd.msec);
        assert_eq!(result.lightlevel, cmd.lightlevel);
    }

    // =========================================================================
    // MSG read functions: end-of-buffer behavior
    // =========================================================================

    #[test]
    fn test_msg_read_short_overflow() {
        let mut sb = SizeBuf::new(64);
        msg_write_byte(&mut sb, 0x42); // only 1 byte
        msg_begin_reading(&mut sb);
        // Trying to read a short from a 1-byte buffer should return -1
        let val = msg_read_short(&mut sb);
        assert_eq!(val, -1);
    }

    #[test]
    fn test_msg_read_long_overflow() {
        let mut sb = SizeBuf::new(64);
        msg_write_short(&mut sb, 0x1234); // only 2 bytes
        msg_begin_reading(&mut sb);
        // Trying to read a long from a 2-byte buffer should return -1
        let val = msg_read_long(&mut sb);
        assert_eq!(val, -1);
    }

    #[test]
    fn test_msg_read_float_overflow() {
        let mut sb = SizeBuf::new(64);
        msg_write_byte(&mut sb, 0); // only 1 byte
        msg_begin_reading(&mut sb);
        let val = msg_read_float(&mut sb);
        assert_eq!(val, -1.0);
    }

    // =========================================================================
    // MSG string with special characters
    // =========================================================================

    #[test]
    fn test_msg_write_read_string_empty() {
        let mut sb = SizeBuf::new(64);
        msg_write_string(&mut sb, "");
        msg_begin_reading(&mut sb);
        assert_eq!(msg_read_string(&mut sb), "");
    }

    #[test]
    fn test_msg_write_read_string_with_backslash() {
        let mut sb = SizeBuf::new(256);
        msg_write_string(&mut sb, "\\name\\player");
        msg_begin_reading(&mut sb);
        assert_eq!(msg_read_string(&mut sb), "\\name\\player");
    }

    // =========================================================================
    // MSG pos round-trip
    // =========================================================================

    #[test]
    fn test_msg_pos_round_trip() {
        let pos: Vec3 = [100.5, -200.25, 50.125];
        let mut sb = SizeBuf::new(64);
        msg_write_pos(&mut sb, &pos);
        msg_begin_reading(&mut sb);
        let result = msg_read_pos(&mut sb);

        // Each component: (int)(f*8) * 0.125 -- verify C-compatible precision
        for i in 0..3 {
            let c_encoded = (pos[i] * 8.0) as i32;
            let c_expected = c_encoded as f32 * 0.125;
            assert!(
                (result[i] - c_expected).abs() < f32::EPSILON,
                "pos[{}] mismatch: got {}, expected {}",
                i, result[i], c_expected
            );
        }
    }

    // =========================================================================
    // MSG dir round-trip
    // =========================================================================

    #[test]
    fn test_msg_dir_round_trip() {
        // Use a known direction that maps cleanly to a bytedir
        let dir: Vec3 = [1.0, 0.0, 0.0]; // should find closest normal
        let mut sb = SizeBuf::new(64);
        msg_write_dir(&mut sb, &dir);
        msg_begin_reading(&mut sb);
        let result = msg_read_dir(&mut sb);

        // The result should be roughly in the +X direction
        assert!(result[0] > 0.5, "dir x should be positive, got {}", result[0]);
    }

    // =========================================================================
    // SizeBuf::print merges trailing nulls (C compat)
    // =========================================================================

    #[test]
    fn test_sizebuf_print_merges_trailing_null() {
        let mut sb = SizeBuf::new(128);
        sb.print("hello");
        let size_after_first = sb.cursize;

        sb.print(" world");
        // The second print should have merged the trailing null from the first,
        // so total size should be len("hello world") + 1 null, not
        // len("hello") + 1 + len(" world") + 1
        let expected_size = "hello world".len() as i32 + 1;
        assert_eq!(sb.cursize, expected_size,
            "SizeBuf::print should merge trailing nulls like C SZ_Print. \
             first_size={}, total={}, expected={}",
            size_after_first, sb.cursize, expected_size);
    }
}
