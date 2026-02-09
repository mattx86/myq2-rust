// cl_cin.rs — Client cinematics
// Converted from: myq2-original/client/cl_cin.c

use std::fs::File;
use std::io::Read;
use myq2_common::common::{com_printf, com_dprintf, msg_write_byte};

use crate::client::{ClientState, ClientStatic, KeyDest, ConnState};


// ============================================================
// Types
// ============================================================

pub struct CBlock {
    pub data: Vec<u8>,
    pub count: usize,
}

pub struct Cinematics {
    pub restart_sound: bool,
    pub s_rate: i32,
    pub s_width: i32,
    pub s_channels: i32,

    pub width: i32,
    pub height: i32,
    pub pic: Option<Vec<u8>>,
    pub pic_pending: Option<Vec<u8>>,

    // order 1 huffman stuff
    // hnodes1: [256][256][2] flattened
    pub hnodes1: Option<Vec<i32>>,
    pub numhnodes1: [i32; 256],

    pub h_used: [i32; 512],
    pub h_count: [i32; 512],
}

impl Cinematics {
    pub fn new() -> Self {
        Self {
            restart_sound: false,
            s_rate: 0,
            s_width: 0,
            s_channels: 0,
            width: 0,
            height: 0,
            pic: None,
            pic_pending: None,
            hnodes1: None,
            numhnodes1: [0; 256],
            h_used: [0; 512],
            h_count: [0; 512],
        }
    }
}

impl Default for Cinematics {
    fn default() -> Self {
        Self::new()
    }
}

/// Global cinematic state
static mut CIN: Option<Cinematics> = None;

/// Get a mutable reference to the global cinematics state.
///
/// # Safety
/// Must only be called from the main thread (single-threaded engine).
pub unsafe fn cin() -> &'static mut Cinematics {
    if CIN.is_none() {
        CIN = Some(Cinematics::new());
    }
    CIN.as_mut().unwrap()
}

// ============================================================
// PCX Loading
// ============================================================

/// SCR_LoadPCX — Load a PCX image file.
///
/// Returns (pic, palette, width, height). pic or palette may be None on failure.
pub fn scr_load_pcx(filename: &str) -> (Option<Vec<u8>>, Option<Vec<u8>>, i32, i32) {
    // load the file
    let raw = match fs_load_file(filename) {
        Some(data) => data,
        None => return (None, None, 0, 0),
    };

    // Use the unified PCX decoder from myq2-common
    match myq2_common::qfiles::pcx_decode(&raw) {
        Some(result) => {
            (
                Some(result.pixels),
                Some(result.palette.to_vec()),
                result.width as i32,
                result.height as i32,
            )
        }
        None => {
            com_printf(&format!("Bad pcx file {}\n", filename));
            (None, None, 0, 0)
        }
    }
}

// ============================================================
// Cinematic playback
// ============================================================

/// SCR_StopCinematic
pub fn scr_stop_cinematic(cl: &mut ClientState, _cls: &mut ClientStatic) {
    cl.cinematictime = 0;

    // SAFETY: single-threaded engine
    let cin = unsafe { cin() };

    cin.pic = None;
    cin.pic_pending = None;

    if cl.cinematicpalette_active {
        r_set_palette(None);
        cl.cinematicpalette_active = false;
    }

    if cl.cinematic_file.is_some() {
        cl.cinematic_file = None;
    }

    cin.hnodes1 = None;

    // switch back down to 11 khz sound if necessary
    if cin.restart_sound {
        cin.restart_sound = false;
        crate::cl_main::cl_snd_restart_f();
    }
}

/// SCR_FinishCinematic — Called when either the cinematic completes, or it is aborted
pub fn scr_finish_cinematic(cls: &mut ClientStatic, cl: &ClientState) {
    // tell the server to advance to the next map / cinematic
    msg_write_byte(&mut cls.netchan.message, CLC_STRINGCMD as i32);
    cls.netchan.message.print(
        &format!("nextserver {}\n", cl.servercount),
    );
}

// ============================================================
// Huffman decompression
// ============================================================

/// SmallestNode1
fn smallest_node1(cin: &mut Cinematics, numhnodes: i32) -> i32 {
    let mut best = 99999999i32;
    let mut bestnode: i32 = -1;

    for i in 0..numhnodes {
        if cin.h_used[i as usize] != 0 {
            continue;
        }
        if cin.h_count[i as usize] == 0 {
            continue;
        }
        if cin.h_count[i as usize] < best {
            best = cin.h_count[i as usize];
            bestnode = i;
        }
    }

    if bestnode == -1 {
        return -1;
    }

    cin.h_used[bestnode as usize] = 1; // true
    bestnode
}

/// Huff1TableInit — Reads the 64k counts table and initializes the node trees
pub fn huff1_table_init(cl: &mut ClientState) {
    // SAFETY: single-threaded engine
    let cin = unsafe { cin() };

    cin.hnodes1 = Some(vec![0i32; 256 * 256 * 2]);

    for prev in 0..256usize {
        cin.h_count = [0; 512];
        cin.h_used = [0; 512];

        // read a row of counts
        let mut counts = [0u8; 256];
        if let Some(ref mut file) = cl.cinematic_file {
            let _ = file.read_exact(&mut counts);
        }
        for j in 0..256usize {
            cin.h_count[j] = counts[j] as i32;
        }

        // build the nodes
        let mut numhnodes: i32 = 256;

        while numhnodes != 511 {
            let node_offset = prev * 256 * 2 + (numhnodes as usize - 256) * 2;

            // pick two lowest counts
            let n0 = smallest_node1(cin, numhnodes);
            if n0 == -1 {
                break;
            }
            let n1 = smallest_node1(cin, numhnodes);
            if n1 == -1 {
                break;
            }

            if let Some(ref mut hnodes) = cin.hnodes1 {
                hnodes[node_offset] = n0;
                hnodes[node_offset + 1] = n1;
            }

            cin.h_count[numhnodes as usize] =
                cin.h_count[n0 as usize] + cin.h_count[n1 as usize];
            numhnodes += 1;
        }

        cin.numhnodes1[prev] = numhnodes - 1;
    }
}

/// Huff1Decompress
pub fn huff1_decompress(in_block: &CBlock) -> CBlock {
    if in_block.data.len() < 4 {
        return CBlock {
            data: Vec::new(),
            count: 0,
        };
    }

    // get decompressed count
    let count_total = in_block.data[0] as i32
        | ((in_block.data[1] as i32) << 8)
        | ((in_block.data[2] as i32) << 16)
        | ((in_block.data[3] as i32) << 24);

    let mut input_pos: usize = 4;
    let mut out_data: Vec<u8> = Vec::with_capacity(count_total as usize);

    // SAFETY: single-threaded engine
    let cin = unsafe { cin() };

    let hnodes = match cin.hnodes1 {
        Some(ref h) => h,
        None => {
            return CBlock {
                data: Vec::new(),
                count: 0,
            }
        }
    };

    // hnodesbase = cin.hnodes1 - 256*2; nodes 0-255 aren't stored
    // We simulate this offset by adjusting indexing: hnodesbase[i] = hnodes[i - 256*2]
    // So when accessing hnodes_base[nodenum*2 + bit], we use hnodes[nodenum*2 + bit - 512]
    // And for hnodes_base + (nodenum<<9), base offset = nodenum*256*2 - 512

    let mut count = count_total;
    let mut nodenum = cin.numhnodes1[0] as i32;
    // Current huffman table base: prev_byte * 256 * 2
    // "hnodesbase" in C = hnodes1 - 256*2, so hnodes = hnodesbase + (prev<<9) means
    // the effective base for lookups is (prev * 512 - 512) in the flat array.
    // Access: hnodes[effective_base + nodenum*2 + bit]
    let mut hnodes_offset: i32 = -512; // start: hnodesbase = hnodes1 - 512

    while count > 0 {
        if input_pos >= in_block.data.len() {
            break;
        }
        let mut inbyte = in_block.data[input_pos] as i32;
        input_pos += 1;

        // Unrolled 8 bits per byte, matching C code
        macro_rules! decode_bit {
            () => {
                if nodenum < 256 {
                    // hnodes = hnodesbase + (nodenum<<9)
                    hnodes_offset = (nodenum << 9) - 512;
                    out_data.push(nodenum as u8);
                    count -= 1;
                    if count == 0 {
                        break;
                    }
                    nodenum = cin.numhnodes1[nodenum as usize] as i32;
                }
                let idx = (hnodes_offset + nodenum * 2 + (inbyte & 1)) as usize;
                nodenum = hnodes[idx];
                inbyte >>= 1;
            };
        }

        decode_bit!();
        decode_bit!();
        decode_bit!();
        decode_bit!();
        decode_bit!();
        decode_bit!();
        decode_bit!();
        decode_bit!();
    }

    let diff = input_pos as i64 - in_block.count as i64;
    if diff != 0 && diff != 1 {
        com_dprintf(&format!(
            "Decompression overread by {}\n",
            input_pos as i64 - in_block.count as i64
        ));
    }

    let out_count = out_data.len();
    CBlock {
        data: out_data,
        count: out_count,
    }
}

/// SCR_ReadNextFrame
pub fn scr_read_next_frame(cl: &mut ClientState) -> Option<Vec<u8>> {
    // SAFETY: single-threaded engine
    let cin = unsafe { cin() };

    let file = match cl.cinematic_file {
        Some(ref mut f) => f,
        None => return None,
    };

    // read the next frame
    let mut command_bytes = [0u8; 4];
    let mut r = file.read(&mut command_bytes).unwrap_or(0);
    if r == 0 {
        // give it one more chance
        r = file.read(&mut command_bytes).unwrap_or(0);
    }
    if r < 4 {
        return None;
    }

    let command = i32::from_le_bytes(command_bytes);
    if command == 2 {
        return None; // last frame marker
    }

    if command == 1 {
        // read palette
        let _ = file.read_exact(&mut cl.cinematicpalette);
        cl.cinematicpalette_active = false; // dubious.... exposes an edge case
    }

    // decompress the next frame
    let mut size_bytes = [0u8; 4];
    let _ = file.read_exact(&mut size_bytes);
    let size = i32::from_le_bytes(size_bytes);
    if !(1..=0x20000).contains(&size) {
        com_printf("Bad compressed frame size\n");
        return None; // ERR_DROP in original
    }

    let mut compressed = vec![0u8; size as usize];
    let _ = file.read_exact(&mut compressed);

    // read sound
    let start = cl.cinematicframe * cin.s_rate / 14;
    let end = (cl.cinematicframe + 1) * cin.s_rate / 14;
    let count = end - start;

    let sample_size = (count * cin.s_width * cin.s_channels) as usize;
    let mut samples = vec![0u8; sample_size];
    let _ = file.read_exact(&mut samples);

    crate::cl_main::cl_s_raw_samples(count, cin.s_rate, cin.s_width, cin.s_channels, &samples);

    let in_block = CBlock {
        data: compressed,
        count: size as usize,
    };

    let huf1 = huff1_decompress(&in_block);

    cl.cinematicframe += 1;

    Some(huf1.data)
}

/// SCR_RunCinematic
pub fn scr_run_cinematic(cl: &mut ClientState, cls: &mut ClientStatic) {
    if cl.cinematictime <= 0 {
        scr_stop_cinematic(cl, cls);
        return;
    }

    if cl.cinematicframe == -1 {
        return; // static image
    }

    if cls.key_dest != KeyDest::Game {
        // pause if menu or console is up
        cl.cinematictime = cls.realtime - cl.cinematicframe * 1000 / 14;
        return;
    }

    let frame = ((cls.realtime - cl.cinematictime) as f64 * 14.0 / 1000.0) as i32;
    if frame <= cl.cinematicframe {
        return;
    }
    if frame > cl.cinematicframe + 1 {
        com_dprintf(&format!(
            "Dropped frame: {} > {}\n",
            frame,
            cl.cinematicframe + 1
        ));
        cl.cinematictime = cls.realtime - cl.cinematicframe * 1000 / 14;
    }

    // SAFETY: single-threaded engine
    let cin = unsafe { cin() };

    cin.pic = cin.pic_pending.take();
    cin.pic_pending = scr_read_next_frame(cl);

    if cin.pic_pending.is_none() {
        scr_stop_cinematic(cl, cls);
        scr_finish_cinematic(cls, cl);
        cl.cinematictime = 1; // hack to get the black screen behind loading
        scr_begin_loading_plaque();
        cl.cinematictime = 0;
    }
}

/// SCR_DrawCinematic — Returns true if a cinematic is active, meaning the view
/// rendering should be skipped.
pub fn scr_draw_cinematic(cl: &mut ClientState, cls: &ClientStatic) -> bool {
    if cl.cinematictime <= 0 {
        return false;
    }

    if cls.key_dest == KeyDest::Menu {
        // blank screen and pause if menu is up
        r_set_palette(None);
        cl.cinematicpalette_active = false;
        return true;
    }

    if !cl.cinematicpalette_active {
        r_set_palette(Some(&cl.cinematicpalette));
        cl.cinematicpalette_active = true;
    }

    // SAFETY: single-threaded engine
    let cin = unsafe { cin() };

    if cin.pic.is_none() {
        return true;
    }

    if let Some(ref pic) = cin.pic {
        draw_stretch_raw(0, 0, viddef_width(), viddef_height(), cin.width, cin.height, pic);
    }

    true
}

/// SCR_PlayCinematic
pub fn scr_play_cinematic(arg: &str, cl: &mut ClientState, cls: &mut ClientStatic) {
    cl.cinematicframe = 0;

    // check for static PCX image
    if let Some(dot_pos) = arg.rfind('.') {
        let ext = &arg[dot_pos..];
        if ext == ".pcx" {
            let name = format!("pics/{}", arg);
            let (pic, palette, w, h) = scr_load_pcx(&name);

            // SAFETY: single-threaded engine
            let cin = unsafe { cin() };
            cin.pic = pic;
            cin.width = w;
            cin.height = h;

            cl.cinematicframe = -1;
            cl.cinematictime = 1;
            scr_end_loading_plaque(true);
            cls.state = ConnState::Active;

            if cin.pic.is_none() {
                com_printf(&format!("{} not found.\n", name));
                cl.cinematictime = 0;
            } else if let Some(pal) = palette {
                let len = pal.len().min(cl.cinematicpalette.len());
                cl.cinematicpalette[..len].copy_from_slice(&pal[..len]);
            }
            return;
        }
    }

    let name = format!("video/{}", arg);
    let file = fs_fopen_file(&name);
    if file.is_none() {
        scr_finish_cinematic(cls, cl);
        cl.cinematictime = 0;
        return;
    }
    cl.cinematic_file = file;

    scr_end_loading_plaque(true);

    cls.state = ConnState::Active;

    // SAFETY: single-threaded engine
    let cin = unsafe { cin() };

    if let Some(ref mut file) = cl.cinematic_file {
        let mut buf4 = [0u8; 4];

        let _ = file.read_exact(&mut buf4);
        let width = i32::from_le_bytes(buf4);
        let _ = file.read_exact(&mut buf4);
        let height = i32::from_le_bytes(buf4);
        cin.width = width;
        cin.height = height;

        let _ = file.read_exact(&mut buf4);
        cin.s_rate = i32::from_le_bytes(buf4);
        let _ = file.read_exact(&mut buf4);
        cin.s_width = i32::from_le_bytes(buf4);
        let _ = file.read_exact(&mut buf4);
        cin.s_channels = i32::from_le_bytes(buf4);
    }

    huff1_table_init(cl);

    // switch up to 22 khz sound if necessary
    let old_khz = cvar_variable_value("s_khz") as i32;
    if old_khz != cin.s_rate / 1000 {
        cin.restart_sound = true;
        cvar_set_value("s_khz", (cin.s_rate / 1000) as f32);
        crate::cl_main::cl_snd_restart_f();
        cvar_set_value("s_khz", old_khz as f32);
    }

    cl.cinematicframe = 0;
    cin.pic = scr_read_next_frame(cl);
    cl.cinematictime = sys_milliseconds();
}

// ============================================================
// Placeholder functions — to be provided by other modules
// ============================================================

use myq2_common::qcommon::{SizeBuf, CLC_STRINGCMD};

// Direct imports — formerly wrappers
use myq2_common::files::fs_load_file;

fn fs_fopen_file(filename: &str) -> Option<File> {
    myq2_common::files::with_fs_ctx(|c| {
        c.fopen_file(filename).map(|r| r.file)
    }).flatten()
}

fn r_set_palette(palette: Option<&[u8]>) {
    // SAFETY: single-threaded engine
    unsafe { (crate::console::RENDERER_FNS.r_set_palette)(palette); }
}

fn draw_stretch_raw(
    x: i32, y: i32, w: i32, h: i32, cols: i32, rows: i32, data: &[u8],
) {
    // SAFETY: single-threaded engine
    unsafe {
        (crate::console::RENDERER_FNS.draw_stretch_raw)(x, y, w, h, cols, rows, data);
    }
}

fn viddef_width() -> i32 {
    // SAFETY: single-threaded engine
    unsafe { (crate::console::RENDERER_FNS.viddef_width)() }
}

fn viddef_height() -> i32 {
    // SAFETY: single-threaded engine
    unsafe { (crate::console::RENDERER_FNS.viddef_height)() }
}

fn scr_end_loading_plaque(clear: bool) {
    crate::console::scr_end_loading_plaque(clear);
}

fn scr_begin_loading_plaque() {
    crate::console::scr_begin_loading_plaque();
}

use myq2_common::cvar::{cvar_variable_value, cvar_set_value};

use myq2_common::common::sys_milliseconds;

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // Cinematics struct tests
    // ============================================================

    #[test]
    fn test_cinematics_new_defaults() {
        let cin = Cinematics::new();
        assert!(!cin.restart_sound);
        assert_eq!(cin.s_rate, 0);
        assert_eq!(cin.s_width, 0);
        assert_eq!(cin.s_channels, 0);
        assert_eq!(cin.width, 0);
        assert_eq!(cin.height, 0);
        assert!(cin.pic.is_none());
        assert!(cin.pic_pending.is_none());
        assert!(cin.hnodes1.is_none());
        assert_eq!(cin.numhnodes1, [0; 256]);
        assert_eq!(cin.h_used, [0; 512]);
        assert_eq!(cin.h_count, [0; 512]);
    }

    #[test]
    fn test_cinematics_default_matches_new() {
        let cin_new = Cinematics::new();
        let cin_default = Cinematics::default();
        assert_eq!(cin_new.restart_sound, cin_default.restart_sound);
        assert_eq!(cin_new.s_rate, cin_default.s_rate);
        assert_eq!(cin_new.s_width, cin_default.s_width);
        assert_eq!(cin_new.s_channels, cin_default.s_channels);
        assert_eq!(cin_new.width, cin_default.width);
        assert_eq!(cin_new.height, cin_default.height);
        assert_eq!(cin_new.numhnodes1, cin_default.numhnodes1);
    }

    // ============================================================
    // CBlock tests
    // ============================================================

    #[test]
    fn test_cblock_creation() {
        let block = CBlock {
            data: vec![1, 2, 3, 4],
            count: 4,
        };
        assert_eq!(block.data.len(), 4);
        assert_eq!(block.count, 4);
    }

    #[test]
    fn test_cblock_empty() {
        let block = CBlock {
            data: Vec::new(),
            count: 0,
        };
        assert!(block.data.is_empty());
        assert_eq!(block.count, 0);
    }

    // ============================================================
    // Huffman decompression tests
    // ============================================================

    #[test]
    fn test_huff1_decompress_empty_input() {
        let block = CBlock {
            data: Vec::new(),
            count: 0,
        };
        let result = huff1_decompress(&block);
        assert!(result.data.is_empty());
        assert_eq!(result.count, 0);
    }

    #[test]
    fn test_huff1_decompress_too_short_input() {
        // Less than 4 bytes should return empty
        let block = CBlock {
            data: vec![1, 2, 3],
            count: 3,
        };
        let result = huff1_decompress(&block);
        assert!(result.data.is_empty());
        assert_eq!(result.count, 0);
    }

    #[test]
    fn test_huff1_decompress_zero_count() {
        // The first 4 bytes encode the decompressed count as little-endian i32
        // count = 0 means no data to decompress
        let block = CBlock {
            data: vec![0, 0, 0, 0],
            count: 4,
        };
        let result = huff1_decompress(&block);
        // count_total is 0, so no decompression should occur
        assert!(result.data.is_empty());
        assert_eq!(result.count, 0);
    }

    #[test]
    fn test_huff1_decompress_no_hnodes() {
        // When hnodes1 is None, decompress should return empty
        // First ensure the global CIN has no hnodes1
        unsafe {
            let c = cin();
            c.hnodes1 = None;
        }

        let block = CBlock {
            data: vec![5, 0, 0, 0, 0xFF], // count_total=5
            count: 5,
        };
        let result = huff1_decompress(&block);
        assert!(result.data.is_empty());
        assert_eq!(result.count, 0);
    }

    #[test]
    fn test_huff1_decompress_count_parsing() {
        // Verify the little-endian count parsing logic
        // count = 0x04030201
        let data = vec![0x01, 0x02, 0x03, 0x04];
        let count = data[0] as i32
            | ((data[1] as i32) << 8)
            | ((data[2] as i32) << 16)
            | ((data[3] as i32) << 24);
        assert_eq!(count, 0x04030201);

        // count = 256
        let data2 = vec![0x00, 0x01, 0x00, 0x00];
        let count2 = data2[0] as i32
            | ((data2[1] as i32) << 8)
            | ((data2[2] as i32) << 16)
            | ((data2[3] as i32) << 24);
        assert_eq!(count2, 256);

        // count = 1
        let data3 = vec![0x01, 0x00, 0x00, 0x00];
        let count3 = data3[0] as i32
            | ((data3[1] as i32) << 8)
            | ((data3[2] as i32) << 16)
            | ((data3[3] as i32) << 24);
        assert_eq!(count3, 1);
    }

    // ============================================================
    // smallest_node1 tests
    // ============================================================

    #[test]
    fn test_smallest_node1_finds_minimum() {
        let mut cin = Cinematics::new();
        cin.h_count[0] = 10;
        cin.h_count[1] = 5;
        cin.h_count[2] = 20;
        cin.h_used[0] = 0;
        cin.h_used[1] = 0;
        cin.h_used[2] = 0;

        let result = smallest_node1(&mut cin, 3);
        assert_eq!(result, 1); // h_count[1]=5 is smallest
        assert_eq!(cin.h_used[1], 1); // marked as used
    }

    #[test]
    fn test_smallest_node1_skips_used() {
        let mut cin = Cinematics::new();
        cin.h_count[0] = 10;
        cin.h_count[1] = 5;
        cin.h_count[2] = 20;
        cin.h_used[0] = 0;
        cin.h_used[1] = 1; // already used
        cin.h_used[2] = 0;

        let result = smallest_node1(&mut cin, 3);
        assert_eq!(result, 0); // h_count[0]=10 is smallest not used
        assert_eq!(cin.h_used[0], 1);
    }

    #[test]
    fn test_smallest_node1_skips_zero_count() {
        let mut cin = Cinematics::new();
        cin.h_count[0] = 0; // zero count
        cin.h_count[1] = 5;
        cin.h_count[2] = 0; // zero count
        cin.h_used[0] = 0;
        cin.h_used[1] = 0;
        cin.h_used[2] = 0;

        let result = smallest_node1(&mut cin, 3);
        assert_eq!(result, 1); // only node with non-zero count
    }

    #[test]
    fn test_smallest_node1_returns_neg1_all_used() {
        let mut cin = Cinematics::new();
        cin.h_count[0] = 10;
        cin.h_count[1] = 5;
        cin.h_used[0] = 1;
        cin.h_used[1] = 1;

        let result = smallest_node1(&mut cin, 2);
        assert_eq!(result, -1);
    }

    #[test]
    fn test_smallest_node1_returns_neg1_all_zero_count() {
        let mut cin = Cinematics::new();
        // All h_count values are zero by default
        let result = smallest_node1(&mut cin, 256);
        assert_eq!(result, -1);
    }

    #[test]
    fn test_smallest_node1_empty_range() {
        let mut cin = Cinematics::new();
        let result = smallest_node1(&mut cin, 0);
        assert_eq!(result, -1);
    }

    #[test]
    fn test_smallest_node1_consecutive_calls() {
        // Simulate how the Huffman tree builder calls this function
        let mut cin = Cinematics::new();
        cin.h_count[0] = 3;
        cin.h_count[1] = 1;
        cin.h_count[2] = 4;
        cin.h_count[3] = 1;
        cin.h_count[4] = 5;

        // First call should return node 1 (count=1, smallest)
        let n0 = smallest_node1(&mut cin, 5);
        assert_eq!(n0, 1);

        // Second call should return node 3 (count=1, next smallest, node 1 is used)
        let n1 = smallest_node1(&mut cin, 5);
        assert_eq!(n1, 3);

        // Third call should return node 0 (count=3)
        let n2 = smallest_node1(&mut cin, 5);
        assert_eq!(n2, 0);
    }

    // ============================================================
    // Frame timing tests
    // ============================================================

    #[test]
    fn test_cinematic_frame_timing_14fps() {
        // The cinematic system runs at 14 fps
        // frame = (realtime - cinematictime) * 14.0 / 1000.0
        let cinematictime = 1000;

        // At exactly the start time, frame = 0
        let realtime = 1000;
        let frame = ((realtime - cinematictime) as f64 * 14.0 / 1000.0) as i32;
        assert_eq!(frame, 0);

        // After ~71.4ms (one frame at 14fps), frame = 1
        let realtime = 1072;
        let frame = ((realtime - cinematictime) as f64 * 14.0 / 1000.0) as i32;
        assert_eq!(frame, 1);

        // After 500ms, frame = 7
        let realtime = 1500;
        let frame = ((realtime - cinematictime) as f64 * 14.0 / 1000.0) as i32;
        assert_eq!(frame, 7);

        // After 1000ms, frame = 14
        let realtime = 2000;
        let frame = ((realtime - cinematictime) as f64 * 14.0 / 1000.0) as i32;
        assert_eq!(frame, 14);
    }

    #[test]
    fn test_cinematic_audio_sample_calculation() {
        // Audio samples per frame: start = frame * s_rate / 14
        //                          end   = (frame+1) * s_rate / 14
        //                          count = end - start
        let s_rate = 22050;

        // Frame 0
        let start = 0 * s_rate / 14;
        let end = 1 * s_rate / 14;
        let count = end - start;
        assert_eq!(start, 0);
        assert_eq!(end, 1575);
        assert_eq!(count, 1575);

        // Frame 1
        let start = 1 * s_rate / 14;
        let end = 2 * s_rate / 14;
        let count = end - start;
        assert_eq!(start, 1575);
        assert_eq!(end, 3150);
        assert_eq!(count, 1575);

        // Frame 13 (last of a second)
        let start = 13 * s_rate / 14;
        let end = 14 * s_rate / 14;
        let count = end - start;
        // 13*22050/14 = 20475, 14*22050/14 = 22050
        assert_eq!(start, 20475);
        assert_eq!(end, 22050);
        assert_eq!(count, 1575);
    }

    #[test]
    fn test_cinematic_audio_sample_size() {
        // sample_size = count * s_width * s_channels
        let s_rate = 22050;
        let s_width = 2;   // 16-bit
        let s_channels = 1; // mono

        let frame = 0;
        let start = frame * s_rate / 14;
        let end = (frame + 1) * s_rate / 14;
        let count = end - start;
        let sample_size = (count * s_width * s_channels) as usize;

        // 1575 samples * 2 bytes/sample * 1 channel = 3150 bytes
        assert_eq!(sample_size, 3150);

        // Stereo 16-bit
        let s_channels = 2;
        let sample_size = (count * s_width * s_channels) as usize;
        assert_eq!(sample_size, 6300);

        // Mono 8-bit
        let s_width = 1;
        let s_channels = 1;
        let sample_size = (count * s_width * s_channels) as usize;
        assert_eq!(sample_size, 1575);
    }

    #[test]
    fn test_cinematic_audio_11khz() {
        // Test at 11025 Hz
        let s_rate = 11025;

        let start = 0 * s_rate / 14;
        let end = 1 * s_rate / 14;
        let count = end - start;
        // 11025/14 = 787 (integer division)
        assert_eq!(count, 787);
    }

    // ============================================================
    // Cinematic state machine tests
    // ============================================================

    #[test]
    fn test_stop_cinematic_resets_state() {
        let mut cl = ClientState::default();
        let mut cls = ClientStatic::default();

        cl.cinematictime = 5000;
        cl.cinematicpalette_active = false;
        cl.cinematic_file = None;

        // Set up global cin state
        unsafe {
            let c = cin();
            c.pic = Some(vec![1, 2, 3]);
            c.pic_pending = Some(vec![4, 5, 6]);
            c.hnodes1 = Some(vec![0; 256]);
            c.restart_sound = false;
        }

        scr_stop_cinematic(&mut cl, &mut cls);

        assert_eq!(cl.cinematictime, 0);
        unsafe {
            let c = cin();
            assert!(c.pic.is_none());
            assert!(c.pic_pending.is_none());
            assert!(c.hnodes1.is_none());
        }
    }

    #[test]
    fn test_draw_cinematic_returns_false_when_no_cinematic() {
        let mut cl = ClientState::default();
        let cls = ClientStatic::default();

        cl.cinematictime = 0;
        let result = scr_draw_cinematic(&mut cl, &cls);
        assert!(!result);
    }

    #[test]
    fn test_draw_cinematic_returns_true_when_active() {
        let mut cl = ClientState::default();
        let cls = ClientStatic::default();

        cl.cinematictime = 1000;
        cl.cinematicpalette_active = true;

        // No pic available but should still return true
        unsafe {
            let c = cin();
            c.pic = None;
        }

        let result = scr_draw_cinematic(&mut cl, &cls);
        assert!(result);
    }

    #[test]
    fn test_draw_cinematic_pauses_on_menu() {
        let mut cl = ClientState::default();
        let mut cls = ClientStatic::default();

        cl.cinematictime = 1000;
        cl.cinematicpalette_active = true;
        cls.key_dest = KeyDest::Menu;

        let result = scr_draw_cinematic(&mut cl, &cls);
        assert!(result);
        // When menu is up, palette should be deactivated
        assert!(!cl.cinematicpalette_active);
    }

    #[test]
    fn test_run_cinematic_stops_when_time_zero() {
        let mut cl = ClientState::default();
        let mut cls = ClientStatic::default();

        cl.cinematictime = 0;
        scr_run_cinematic(&mut cl, &mut cls);
        // Should have called stop cinematic, time stays 0
        assert_eq!(cl.cinematictime, 0);
    }

    #[test]
    fn test_run_cinematic_static_frame_noop() {
        let mut cl = ClientState::default();
        let mut cls = ClientStatic::default();

        cl.cinematictime = 1000;
        cl.cinematicframe = -1; // static image marker

        scr_run_cinematic(&mut cl, &mut cls);
        // Should return immediately, frame stays -1
        assert_eq!(cl.cinematicframe, -1);
    }

    #[test]
    fn test_run_cinematic_pauses_on_console() {
        let mut cl = ClientState::default();
        let mut cls = ClientStatic::default();

        cl.cinematictime = 1000;
        cl.cinematicframe = 5;
        cls.key_dest = KeyDest::Console;
        cls.realtime = 5000;

        scr_run_cinematic(&mut cl, &mut cls);
        // Should adjust cinematictime to pause
        // cl.cinematictime = cls.realtime - cl.cinematicframe * 1000 / 14
        let expected = cls.realtime - cl.cinematicframe * 1000 / 14;
        assert_eq!(cl.cinematictime, expected);
    }

    // ============================================================
    // Compressed frame size validation tests
    // ============================================================

    #[test]
    fn test_compressed_frame_size_validation() {
        // Valid range is 1..=0x20000
        assert!((1..=0x20000).contains(&1));
        assert!((1..=0x20000).contains(&0x20000));
        assert!(!(1..=0x20000).contains(&0));
        assert!(!(1..=0x20000).contains(&0x20001));
        assert!(!(1..=0x20000).contains(&-1));
    }

    // ============================================================
    // Huffman tree node offset calculation tests
    // ============================================================

    #[test]
    fn test_huffman_node_offset_calculation() {
        // hnodes1 is [256][256][2] = 256 * 256 * 2 = 131072 entries
        // For prev=0, node_offset = prev * 256 * 2 + (numhnodes - 256) * 2
        let prev = 0usize;
        let numhnodes = 256i32;
        let node_offset = prev * 256 * 2 + (numhnodes as usize - 256) * 2;
        assert_eq!(node_offset, 0);

        // For prev=1, numhnodes=257
        let prev = 1usize;
        let numhnodes = 257i32;
        let node_offset = prev * 256 * 2 + (numhnodes as usize - 256) * 2;
        assert_eq!(node_offset, 512 + 2);

        // For prev=255, numhnodes=510
        let prev = 255usize;
        let numhnodes = 510i32;
        let node_offset = prev * 256 * 2 + (numhnodes as usize - 256) * 2;
        assert_eq!(node_offset, 255 * 512 + 508);
    }

    #[test]
    fn test_huffman_hnodes_offset_calculation() {
        // hnodes_offset starts at -512 (hnodesbase = hnodes1 - 256*2)
        // When a leaf node is found (nodenum < 256):
        //   hnodes_offset = (nodenum << 9) - 512
        // Which is nodenum * 512 - 512

        // For nodenum=0: offset = 0 * 512 - 512 = -512
        assert_eq!((0i32 << 9) - 512, -512);

        // For nodenum=1: offset = 1 * 512 - 512 = 0
        assert_eq!((1i32 << 9) - 512, 0);

        // For nodenum=128: offset = 128 * 512 - 512 = 65024
        assert_eq!((128i32 << 9) - 512, 65024);

        // For nodenum=255: offset = 255 * 512 - 512 = 130048
        assert_eq!((255i32 << 9) - 512, 130048);
    }

    // ============================================================
    // Little-endian decoding tests
    // ============================================================

    #[test]
    fn test_le_i32_decode() {
        let bytes = [0x78u8, 0x56, 0x34, 0x12];
        let val = i32::from_le_bytes(bytes);
        assert_eq!(val, 0x12345678);
    }

    #[test]
    fn test_le_i32_decode_negative() {
        let bytes = [0xFF, 0xFF, 0xFF, 0xFF];
        let val = i32::from_le_bytes(bytes);
        assert_eq!(val, -1);
    }

    #[test]
    fn test_command_bytes_parsing() {
        // command == 2 means last frame marker
        let bytes = [2u8, 0, 0, 0];
        let command = i32::from_le_bytes(bytes);
        assert_eq!(command, 2);

        // command == 1 means read palette
        let bytes = [1, 0, 0, 0];
        let command = i32::from_le_bytes(bytes);
        assert_eq!(command, 1);
    }

    // ============================================================
    // Khz conversion tests
    // ============================================================

    #[test]
    fn test_khz_from_rate() {
        // s_rate / 1000 gives the khz value
        assert_eq!(22050 / 1000, 22);
        assert_eq!(11025 / 1000, 11);
        assert_eq!(44100 / 1000, 44);
    }
}
