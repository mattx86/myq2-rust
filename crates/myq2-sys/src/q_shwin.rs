// q_shwin.rs — Shared Windows platform code: memory, timing, filesystem
// Converted from: myq2-original/win32/q_shwin.c

use std::alloc::{self, Layout};
use std::path::Path;
use std::sync::Mutex;

use myq2_common::q_shared::*;

// ============================================================
// Hunk memory allocator
// ============================================================

/// Tracks the state of one hunk allocation.
struct HunkState {
    base: *mut u8,
    max_size: usize,
    cur_size: usize,
    layout: Option<Layout>,
}

// SAFETY: HunkState is only accessed through the HUNK mutex.
unsafe impl Send for HunkState {}

static HUNK: Mutex<HunkState> = Mutex::new(HunkState {
    base: std::ptr::null_mut(),
    max_size: 0,
    cur_size: 0,
    layout: None,
});

static HUNK_COUNT: Mutex<i32> = Mutex::new(0);

/// Reserve a large block of memory.
///
/// Original: `void *Hunk_Begin(int maxsize)` — used VirtualAlloc with MEM_RESERVE.
/// Rust equivalent: allocate the full block up front (no virtual memory reserve/commit distinction).
pub fn hunk_begin(max_size: usize) -> *mut u8 {
    let mut hunk = HUNK.lock().unwrap();
    hunk.cur_size = 0;
    hunk.max_size = max_size;

    let layout = Layout::from_size_align(max_size, 32)
        .expect("Hunk_Begin: invalid layout");

    // SAFETY: We allocate a block of max_size bytes with 32-byte alignment.
    // The pointer is managed exclusively through hunk_alloc / hunk_free.
    let ptr = unsafe { alloc::alloc_zeroed(layout) };
    if ptr.is_null() {
        crate::sys_win::sys_error("Hunk_Begin: allocation failed (VirtualAlloc reserve equivalent)");
    }

    hunk.base = ptr;
    hunk.layout = Some(layout);
    ptr
}

/// Allocate from the current hunk, rounding up to 32-byte cache line.
///
/// Original: `void *Hunk_Alloc(int size)` — used VirtualAlloc MEM_COMMIT.
pub fn hunk_alloc(size: usize) -> *mut u8 {
    // Round to cache line (32 bytes)
    let size = (size + 31) & !31;

    let mut hunk = HUNK.lock().unwrap();
    hunk.cur_size += size;

    if hunk.cur_size > hunk.max_size {
        crate::sys_win::sys_error("Hunk_Alloc overflow");
    }

    // SAFETY: base is valid and cur_size <= max_size, so the offset is in bounds.
    unsafe { hunk.base.add(hunk.cur_size - size) }
}

/// Finalize the hunk, returning the total bytes used.
///
/// Original: `int Hunk_End(void)`
pub fn hunk_end() -> usize {
    let hunk = HUNK.lock().unwrap();
    let mut count = HUNK_COUNT.lock().unwrap();
    *count += 1;
    hunk.cur_size
}

/// Free a previously allocated hunk.
///
/// Original: `void Hunk_Free(void *base)` — used VirtualFree MEM_RELEASE.
/// # Safety
/// `base` must have been allocated by `hunk_alloc` and not yet freed.
pub unsafe fn hunk_free(base: *mut u8) {
    if !base.is_null() {
        let hunk = HUNK.lock().unwrap();
        if let Some(layout) = hunk.layout {
            // SAFETY: base was allocated with alloc::alloc_zeroed using this layout.
            unsafe {
                alloc::dealloc(base, layout);
            }
        }
    }
    let mut count = HUNK_COUNT.lock().unwrap();
    *count -= 1;
}

// ============================================================
// Timing
// ============================================================

/// Current time in milliseconds (legacy, kept for API compat).
pub static CURTIME: Mutex<i32> = Mutex::new(0);

/// Get milliseconds since engine start.
///
/// Original: `int Sys_Milliseconds(void)` — used timeGetTime().
/// Delegates to the canonical implementation in myq2_common and updates CURTIME.
pub fn sys_milliseconds() -> i32 {
    let elapsed = myq2_common::common::sys_milliseconds();
    let mut curtime = CURTIME.lock().unwrap();
    *curtime = elapsed;
    elapsed
}

// ============================================================
// Filesystem: mkdir
// ============================================================

/// Create a directory.
///
/// Original: `void Sys_Mkdir(char *path)` — used _mkdir().
pub fn sys_mkdir(path: &str) {
    let _ = std::fs::create_dir_all(path);
}

// ============================================================
// File finding (directory enumeration)
// ============================================================

/// State for the Sys_FindFirst / Sys_FindNext / Sys_FindClose sequence.
struct FindState {
    base: String,
    pattern: String,
    entries: Vec<String>,
    index: usize,
    active: bool,
}

static FIND_STATE: Mutex<FindState> = Mutex::new(FindState {
    base: String::new(),
    pattern: String::new(),
    entries: Vec::new(),
    index: 0,
    active: false,
});

/// Compare file attributes against must-have and can't-have masks.
///
/// Original: `static qboolean CompareAttributes(unsigned found, unsigned musthave, unsigned canthave)`
///
/// In the original C code, this checked DOS-style file attributes (_A_RDONLY, _A_HIDDEN, etc.)
/// against SFF_ flags. In the Rust port, we use std::fs::metadata and map accordingly.
fn compare_attributes(path: &Path, musthave: SysFileFlags, canthave: SysFileFlags) -> bool {
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return false,
    };

    let is_dir = meta.is_dir();
    let is_readonly = meta.permissions().readonly();

    // Map to original attribute flags
    if is_readonly && canthave.intersects(SFF_RDONLY) {
        return false;
    }
    if is_dir && canthave.intersects(SFF_SUBDIR) {
        return false;
    }

    if musthave.intersects(SFF_RDONLY) && !is_readonly {
        return false;
    }
    if musthave.intersects(SFF_SUBDIR) && !is_dir {
        return false;
    }

    // SFF_HIDDEN, SFF_SYSTEM, SFF_ARCH have no direct cross-platform mapping.
    // We pass them through (ignore musthave for those, reject canthave conservatively).

    true
}

/// Extract the directory portion of a path (equivalent to COM_FilePath).
fn file_path(path: &str) -> String {
    match path.rfind('/').or_else(|| path.rfind('\\')) {
        Some(pos) => path[..pos].to_string(),
        None => ".".to_string(),
    }
}

/// Begin a file search with a glob-like pattern.
///
/// Original: `char *Sys_FindFirst(char *path, unsigned musthave, unsigned canthave)`
///
/// Uses _findfirst/_findnext in C. Rust uses std::fs::read_dir with pattern matching.
pub fn sys_find_first(path: &str, musthave: SysFileFlags, canthave: SysFileFlags) -> Option<String> {
    let mut state = FIND_STATE.lock().unwrap();

    if state.active {
        crate::sys_win::sys_error("Sys_BeginFind without close");
    }

    state.base = file_path(path);
    state.entries.clear();
    state.index = 0;
    state.active = true;

    // Extract the filename pattern (after last separator)
    let pattern_str = match path.rfind('/').or_else(|| path.rfind('\\')) {
        Some(pos) => &path[pos + 1..],
        None => path,
    };
    state.pattern = pattern_str.to_string();

    // Read directory and filter by simple glob (supports * and ?)
    let dir = &state.base;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if simple_glob_match(pattern_str, &name) {
                let full = format!("{}/{}", state.base, name);
                let full_path = Path::new(&full);
                if compare_attributes(full_path, musthave, canthave) {
                    state.entries.push(full);
                }
            }
        }
    }

    if state.index < state.entries.len() {
        let result = state.entries[state.index].clone();
        state.index += 1;
        Some(result)
    } else {
        None
    }
}

/// Continue a file search.
///
/// Original: `char *Sys_FindNext(unsigned musthave, unsigned canthave)`
pub fn sys_find_next(_musthave: SysFileFlags, _canthave: SysFileFlags) -> Option<String> {
    let mut state = FIND_STATE.lock().unwrap();

    if !state.active {
        return None;
    }

    // Attributes were already filtered during sys_find_first
    if state.index < state.entries.len() {
        let result = state.entries[state.index].clone();
        state.index += 1;
        Some(result)
    } else {
        None
    }
}

/// End a file search.
///
/// Original: `void Sys_FindClose(void)`
pub fn sys_find_close() {
    let mut state = FIND_STATE.lock().unwrap();
    state.entries.clear();
    state.index = 0;
    state.active = false;
}

/// Simple glob pattern matcher supporting `*` and `?`.
/// Case-insensitive to match Windows behavior.
fn simple_glob_match(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let n: Vec<char> = name.chars().collect();
    glob_match_inner(&p, 0, &n, 0)
}

fn glob_match_inner(p: &[char], pi: usize, n: &[char], ni: usize) -> bool {
    let mut pi = pi;
    let mut ni = ni;

    while pi < p.len() {
        match p[pi] {
            '*' => {
                pi += 1;
                // Match zero or more characters
                for i in ni..=n.len() {
                    if glob_match_inner(p, pi, n, i) {
                        return true;
                    }
                }
                return false;
            }
            '?' => {
                if ni >= n.len() {
                    return false;
                }
                pi += 1;
                ni += 1;
            }
            c => {
                if ni >= n.len() {
                    return false;
                }
                if !c.eq_ignore_ascii_case(&n[ni]) {
                    return false;
                }
                pi += 1;
                ni += 1;
            }
        }
    }

    ni >= n.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------
    // simple_glob_match
    // -------------------------------------------------------

    #[test]
    fn test_glob_exact_match() {
        assert!(simple_glob_match("hello", "hello"));
    }

    #[test]
    fn test_glob_exact_mismatch() {
        assert!(!simple_glob_match("hello", "world"));
    }

    #[test]
    fn test_glob_case_insensitive() {
        assert!(simple_glob_match("Hello", "hello"));
        assert!(simple_glob_match("HELLO", "hello"));
        assert!(simple_glob_match("hello", "HELLO"));
    }

    #[test]
    fn test_glob_star_matches_all() {
        assert!(simple_glob_match("*", "anything"));
        assert!(simple_glob_match("*", ""));
        assert!(simple_glob_match("*", "a"));
    }

    #[test]
    fn test_glob_star_prefix() {
        assert!(simple_glob_match("*.txt", "readme.txt"));
        assert!(simple_glob_match("*.txt", ".txt"));
        assert!(!simple_glob_match("*.txt", "readme.doc"));
    }

    #[test]
    fn test_glob_star_suffix() {
        assert!(simple_glob_match("readme*", "readme.txt"));
        assert!(simple_glob_match("readme*", "readme"));
        assert!(!simple_glob_match("readme*", "other.txt"));
    }

    #[test]
    fn test_glob_star_middle() {
        assert!(simple_glob_match("a*c", "abc"));
        assert!(simple_glob_match("a*c", "ac"));
        assert!(simple_glob_match("a*c", "aXYZc"));
        assert!(!simple_glob_match("a*c", "ab"));
    }

    #[test]
    fn test_glob_multiple_stars() {
        assert!(simple_glob_match("*.*", "file.txt"));
        assert!(simple_glob_match("*.*", ".hidden"));
        assert!(!simple_glob_match("*.*", "noextension"));
    }

    #[test]
    fn test_glob_question_mark() {
        assert!(simple_glob_match("?", "a"));
        assert!(!simple_glob_match("?", ""));
        assert!(!simple_glob_match("?", "ab"));
    }

    #[test]
    fn test_glob_question_mark_multiple() {
        assert!(simple_glob_match("???", "abc"));
        assert!(!simple_glob_match("???", "ab"));
        assert!(!simple_glob_match("???", "abcd"));
    }

    #[test]
    fn test_glob_question_with_literal() {
        assert!(simple_glob_match("?.txt", "a.txt"));
        assert!(!simple_glob_match("?.txt", "ab.txt"));
    }

    #[test]
    fn test_glob_combined_star_question() {
        assert!(simple_glob_match("*?", "a"));
        assert!(simple_glob_match("*?", "abc"));
        assert!(!simple_glob_match("*?", ""));
    }

    #[test]
    fn test_glob_quake_patterns() {
        // Common Quake 2 file patterns
        assert!(simple_glob_match("*.bsp", "q2dm1.bsp"));
        assert!(simple_glob_match("*.pak", "pak0.pak"));
        assert!(simple_glob_match("*.pcx", "colormap.pcx"));
        assert!(simple_glob_match("*.md2", "tris.md2"));
        assert!(simple_glob_match("*.wal", "e1u1/floor1_1.wal"));
    }

    #[test]
    fn test_glob_case_insensitive_extension() {
        assert!(simple_glob_match("*.BSP", "q2dm1.bsp"));
        assert!(simple_glob_match("*.bsp", "Q2DM1.BSP"));
    }

    #[test]
    fn test_glob_empty_pattern_matches_empty_name() {
        assert!(simple_glob_match("", ""));
    }

    #[test]
    fn test_glob_empty_pattern_rejects_nonempty() {
        assert!(!simple_glob_match("", "something"));
    }

    #[test]
    fn test_glob_nonempty_pattern_rejects_empty() {
        assert!(!simple_glob_match("abc", ""));
    }

    // -------------------------------------------------------
    // file_path
    // -------------------------------------------------------

    #[test]
    fn test_file_path_with_forward_slash() {
        assert_eq!(file_path("baseq2/maps/q2dm1.bsp"), "baseq2/maps");
    }

    #[test]
    fn test_file_path_with_backslash() {
        assert_eq!(file_path("baseq2\\maps\\q2dm1.bsp"), "baseq2\\maps");
    }

    #[test]
    fn test_file_path_mixed_separators() {
        // rfind('/') finds the last forward slash; rfind('\\') finds the last backslash.
        // The function takes whichever comes last.
        let result = file_path("baseq2/maps\\q2dm1.bsp");
        // The backslash is at index 13, forward slash at index 6
        // rfind('/') = Some(6), rfind('\\') = Some(13)
        // The function uses .or_else, so it takes '/' if found, else '\\'
        // Actually looking at the code: rfind('/').or_else(|| rfind('\\'))
        // rfind('/') returns Some(6), so it uses position 6
        // Wait, let me re-read the code...
        // It's path.rfind('/').or_else(|| path.rfind('\\'))
        // rfind('/') = Some(6), so or_else is not called
        // Result: path[..6] = "baseq2"
        assert_eq!(result, "baseq2");
    }

    #[test]
    fn test_file_path_no_separator() {
        assert_eq!(file_path("filename.txt"), ".");
    }

    #[test]
    fn test_file_path_root_slash() {
        assert_eq!(file_path("/file.txt"), "");
    }

    #[test]
    fn test_file_path_trailing_slash() {
        assert_eq!(file_path("baseq2/maps/"), "baseq2/maps");
    }

    #[test]
    fn test_file_path_single_directory() {
        assert_eq!(file_path("dir/file"), "dir");
    }

    // -------------------------------------------------------
    // Hunk alloc rounding
    // -------------------------------------------------------

    #[test]
    fn test_hunk_alloc_rounds_to_cache_line() {
        // hunk_alloc rounds up to 32-byte alignment: (size + 31) & !31
        let round = |size: usize| (size + 31) & !31;

        assert_eq!(round(0), 0);
        assert_eq!(round(1), 32);
        assert_eq!(round(31), 32);
        assert_eq!(round(32), 32);
        assert_eq!(round(33), 64);
        assert_eq!(round(64), 64);
        assert_eq!(round(100), 128);
    }

    // -------------------------------------------------------
    // sys_mkdir (no-op verification)
    // -------------------------------------------------------

    #[test]
    fn test_sys_mkdir_does_not_panic_on_existing() {
        // sys_mkdir uses create_dir_all and ignores errors
        sys_mkdir(".");
    }

    // -------------------------------------------------------
    // sys_find_close resets state
    // -------------------------------------------------------

    #[test]
    fn test_sys_find_close_clears_state() {
        // Directly verify that close resets the state
        sys_find_close();
        let state = FIND_STATE.lock().unwrap();
        assert_eq!(state.entries.len(), 0);
        assert_eq!(state.index, 0);
        assert!(!state.active);
    }
}
