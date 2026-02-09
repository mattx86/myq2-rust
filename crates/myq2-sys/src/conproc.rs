// conproc.rs — QHOST console process support
// Converted from: myq2-original/win32/conproc.c
//
// QHOST was an external tool that could hook into the Quake 2 dedicated
// server console via shared memory and events. On modern systems this is
// unused, but the API surface is preserved for 1-to-1 conversion fidelity.

use std::sync::Mutex;

use myq2_common::common::com_printf;

// ============================================================
// QHOST command codes
// ============================================================

const CCOM_WRITE_TEXT: i32 = 0x2;
// Param1 : Text

const CCOM_GET_TEXT: i32 = 0x3;
// Param1 : Begin line
// Param2 : End line

const CCOM_GET_SCR_LINES: i32 = 0x4;
// No params

const CCOM_SET_SCR_LINES: i32 = 0x5;
// Param1 : Number of lines

// ============================================================
// Win32 FFI (windows-only)
// ============================================================

#[cfg(target_os = "windows")]
#[allow(clippy::upper_case_acronyms)]
mod win32 {
    use std::ffi::c_void;

    pub type HANDLE = *mut c_void;
    pub type LPVOID = *mut c_void;
    pub type BOOL = i32;
    pub type DWORD = u32;
    pub type WORD = u16;
    pub type WCHAR = u16;
    pub type CHAR = i8;
    pub type SHORT = i16;
    pub type UINT = u32;

    pub const FALSE: BOOL = 0;
    pub const TRUE: BOOL = 1;
    pub const INFINITE: DWORD = 0xFFFFFFFF;
    pub const WAIT_OBJECT_0: DWORD = 0x00000000;
    pub const STD_OUTPUT_HANDLE: DWORD = 0xFFFFFFF5u32; // (DWORD)-11
    pub const STD_INPUT_HANDLE: DWORD = 0xFFFFFFF6u32;  // (DWORD)-10
    pub const FILE_MAP_READ: DWORD = 0x0004;
    pub const FILE_MAP_WRITE: DWORD = 0x0002;
    pub const KEY_EVENT: WORD = 0x0001;

    #[repr(C)]
    #[derive(Copy, Clone, Default)]
    pub struct COORD {
        pub x: SHORT,
        pub y: SHORT,
    }

    #[repr(C)]
    #[derive(Copy, Clone, Default)]
    pub struct SMALL_RECT {
        pub left: SHORT,
        pub top: SHORT,
        pub right: SHORT,
        pub bottom: SHORT,
    }

    #[repr(C)]
    #[derive(Copy, Clone, Default)]
    pub struct CONSOLE_SCREEN_BUFFER_INFO {
        pub dw_size: COORD,
        pub dw_cursor_position: COORD,
        pub w_attributes: WORD,
        pub sr_window: SMALL_RECT,
        pub dw_maximum_window_size: COORD,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    pub struct KEY_EVENT_RECORD {
        pub b_key_down: BOOL,
        pub w_repeat_count: WORD,
        pub w_virtual_key_code: WORD,
        pub w_virtual_scan_code: WORD,
        pub u_char: KEY_EVENT_CHAR,
        pub dw_control_key_state: DWORD,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    pub union KEY_EVENT_CHAR {
        pub unicode_char: WCHAR,
        pub ascii_char: CHAR,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    pub struct INPUT_RECORD {
        pub event_type: WORD,
        pub _padding: WORD,
        pub event: INPUT_RECORD_EVENT,
    }

    #[repr(C)]
    #[derive(Copy, Clone)]
    pub union INPUT_RECORD_EVENT {
        pub key_event: KEY_EVENT_RECORD,
        pub _pad: [u8; 16], // enough space for largest event union member
    }

    pub const NULL_HANDLE: HANDLE = std::ptr::null_mut();

    extern "system" {
        pub fn CreateEventA(
            lp_event_attributes: LPVOID,
            b_manual_reset: BOOL,
            b_initial_state: BOOL,
            lp_name: *const u8,
        ) -> HANDLE;

        pub fn SetEvent(h_event: HANDLE) -> BOOL;
        pub fn CloseHandle(h_object: HANDLE) -> BOOL;

        pub fn GetStdHandle(n_std_handle: DWORD) -> HANDLE;

        pub fn WaitForMultipleObjects(
            n_count: DWORD,
            lp_handles: *const HANDLE,
            b_wait_all: BOOL,
            dw_milliseconds: DWORD,
        ) -> DWORD;

        pub fn MapViewOfFile(
            h_file_mapping_object: HANDLE,
            dw_desired_access: DWORD,
            dw_file_offset_high: DWORD,
            dw_file_offset_low: DWORD,
            dw_number_of_bytes_to_map: usize,
        ) -> LPVOID;

        pub fn UnmapViewOfFile(lp_base_address: LPVOID) -> BOOL;

        pub fn GetConsoleScreenBufferInfo(
            h_console_output: HANDLE,
            lp_console_screen_buffer_info: *mut CONSOLE_SCREEN_BUFFER_INFO,
        ) -> BOOL;

        pub fn GetLargestConsoleWindowSize(h_console_output: HANDLE) -> COORD;

        pub fn SetConsoleWindowInfo(
            h_console_output: HANDLE,
            b_absolute: BOOL,
            lp_console_window: *const SMALL_RECT,
        ) -> BOOL;

        pub fn SetConsoleScreenBufferSize(
            h_console_output: HANDLE,
            dw_size: COORD,
        ) -> BOOL;

        pub fn ReadConsoleOutputCharacterA(
            h_console_output: HANDLE,
            lp_character: *mut u8,
            n_length: DWORD,
            dw_read_coord: COORD,
            lp_number_of_chars_read: *mut DWORD,
        ) -> BOOL;

        pub fn WriteConsoleInputA(
            h_console_input: HANDLE,
            lp_buffer: *const INPUT_RECORD,
            n_length: DWORD,
            lp_number_of_events_written: *mut DWORD,
        ) -> BOOL;

        pub fn _beginthreadex(
            security: LPVOID,
            stack_size: UINT,
            start_address: unsafe extern "system" fn(LPVOID) -> u32,
            arg_list: LPVOID,
            init_flag: UINT,
            thread_addr: *mut UINT,
        ) -> usize;

        pub fn _endthreadex(retval: UINT);
    }
}

// ============================================================
// Global state
// ============================================================

#[cfg(target_os = "windows")]
static HANDLES: Mutex<ConProcHandles> = Mutex::new(ConProcHandles::new());

#[cfg(target_os = "windows")]
struct ConProcHandles {
    hevent_done: win32::HANDLE,
    hfile_buffer: win32::HANDLE,
    hevent_child_send: win32::HANDLE,
    hevent_parent_send: win32::HANDLE,
    h_stdout: win32::HANDLE,
    h_stdin: win32::HANDLE,
}

#[cfg(target_os = "windows")]
// SAFETY: Win32 HANDLEs are thread-safe when used with proper synchronization.
// We protect all access behind a Mutex.
unsafe impl Send for ConProcHandles {}
unsafe impl Sync for ConProcHandles {}

#[cfg(target_os = "windows")]
impl ConProcHandles {
    const fn new() -> Self {
        Self {
            hevent_done: std::ptr::null_mut(),
            hfile_buffer: std::ptr::null_mut(),
            hevent_child_send: std::ptr::null_mut(),
            hevent_parent_send: std::ptr::null_mut(),
            h_stdout: std::ptr::null_mut(),
            h_stdin: std::ptr::null_mut(),
        }
    }
}

/// Stored command-line args for QHOST parameter checking.
static CCOM_ARGS: Mutex<Vec<String>> = Mutex::new(Vec::new());

// ============================================================
// CCheckParm
// ============================================================

/// Returns the position (1 to argc-1) in the program's argument list
/// where the given parameter appears, or 0 if not present.
///
/// Original: `int CCheckParm(char *parm)`
fn c_check_parm(parm: &str, args: &[String]) -> usize {
    for i in 1..args.len() {
        if args[i] == parm {
            return i;
        }
    }
    0
}

// ============================================================
// InitConProc
// ============================================================

/// Initialize QHOST console process hooks.
///
/// Original: `void InitConProc(int argc, char **argv)`
pub fn init_con_proc(args: &[String]) {
    let mut stored = CCOM_ARGS.lock().unwrap();
    *stored = args.to_vec();

    let t_hfile = c_check_parm("-HFILE", args);
    let t_hparent = c_check_parm("-HPARENT", args);
    let t_hchild = c_check_parm("-HCHILD", args);

    // Ignore if we don't have all the events.
    if t_hfile == 0 || t_hparent == 0 || t_hchild == 0 {
        com_printf("Qhost not present.\n");
        return;
    }

    #[cfg(target_os = "windows")]
    {
        let h_file: win32::HANDLE = if t_hfile + 1 < args.len() {
            args[t_hfile + 1]
                .parse::<isize>()
                .unwrap_or(0) as win32::HANDLE
        } else {
            win32::NULL_HANDLE
        };

        let hevent_parent: win32::HANDLE = if t_hparent + 1 < args.len() {
            args[t_hparent + 1]
                .parse::<isize>()
                .unwrap_or(0) as win32::HANDLE
        } else {
            win32::NULL_HANDLE
        };

        let hevent_child: win32::HANDLE = if t_hchild + 1 < args.len() {
            args[t_hchild + 1]
                .parse::<isize>()
                .unwrap_or(0) as win32::HANDLE
        } else {
            win32::NULL_HANDLE
        };

        // ignore if we don't have all the events.
        if h_file.is_null() || hevent_parent.is_null() || hevent_child.is_null() {
            com_printf("Qhost not present.\n");
            return;
        }

        com_printf("Initializing for qhost.\n");

        let mut handles = HANDLES.lock().unwrap();
        handles.hfile_buffer = h_file;
        handles.hevent_parent_send = hevent_parent;
        handles.hevent_child_send = hevent_child;

        // so we'll know when to go away.
        // SAFETY: Calling CreateEventA with null attributes and name to create
        // an anonymous auto-reset event.
        let hevent_done = unsafe {
            win32::CreateEventA(
                std::ptr::null_mut(),
                win32::FALSE,
                win32::FALSE,
                std::ptr::null(),
            )
        };

        if hevent_done.is_null() {
            com_printf("Couldn't create heventDone\n");
            return;
        }

        handles.hevent_done = hevent_done;

        // SAFETY: Spawning the QHOST request processing thread via _beginthreadex.
        // The thread function (request_proc_thread) follows the expected calling convention.
        let mut thread_addr: win32::UINT = 0;
        let thread_handle = unsafe {
            win32::_beginthreadex(
                std::ptr::null_mut(),
                0,
                request_proc_thread,
                std::ptr::null_mut(),
                0,
                &mut thread_addr,
            )
        };

        if thread_handle == 0 {
            // SAFETY: Closing the event handle on failure.
            unsafe { win32::CloseHandle(hevent_done); }
            handles.hevent_done = win32::NULL_HANDLE;
            com_printf("Couldn't create QHOST thread\n");
            return;
        }

        // save off the input/output handles.
        // SAFETY: GetStdHandle returns a handle to the standard I/O device.
        handles.h_stdout = unsafe { win32::GetStdHandle(win32::STD_OUTPUT_HANDLE) };
        handles.h_stdin = unsafe { win32::GetStdHandle(win32::STD_INPUT_HANDLE) };

        let h_stdout = handles.h_stdout;
        drop(handles);

        // force 80 character width, at least 25 character height
        set_console_cxcy(h_stdout, 80, 25);
    }

    #[cfg(not(target_os = "windows"))]
    {
        com_printf("Initializing for qhost.\n");
        com_printf("Qhost not supported on this platform.\n");
    }
}

// ============================================================
// DeinitConProc
// ============================================================

/// Shut down QHOST hooks.
///
/// Original: `void DeinitConProc(void)`
pub fn deinit_con_proc() {
    #[cfg(target_os = "windows")]
    {
        let handles = HANDLES.lock().unwrap();
        if !handles.hevent_done.is_null() {
            // SAFETY: Signaling the heventDone event to tell RequestProc to exit.
            unsafe { win32::SetEvent(handles.hevent_done); }
        }
    }
}

// ============================================================
// RequestProc — QHOST worker thread
// ============================================================

/// QHOST request processing thread entry point.
///
/// Original: `unsigned _stdcall RequestProc(void *arg)`
#[cfg(target_os = "windows")]
unsafe extern "system" fn request_proc_thread(_arg: win32::LPVOID) -> u32 {
    request_proc();
    // SAFETY: _endthreadex terminates the current thread.
    unsafe { win32::_endthreadex(0); }
    0
}

#[cfg(target_os = "windows")]
fn request_proc() {
    let (hevent_parent_send, hevent_done, hfile_buffer, hevent_child_send, h_stdout, h_stdin) = {
        let handles = HANDLES.lock().unwrap();
        (
            handles.hevent_parent_send,
            handles.hevent_done,
            handles.hfile_buffer,
            handles.hevent_child_send,
            handles.h_stdout,
            handles.h_stdin,
        )
    };

    let hevent_wait: [win32::HANDLE; 2] = [hevent_parent_send, hevent_done];

    loop {
        // SAFETY: Waiting on the two event handles. Both are valid Win32 event handles
        // initialized in init_con_proc.
        let dw_ret = unsafe {
            win32::WaitForMultipleObjects(
                2,
                hevent_wait.as_ptr(),
                win32::FALSE,
                win32::INFINITE,
            )
        };

        // heventDone fired, so we're exiting.
        if dw_ret == win32::WAIT_OBJECT_0 + 1 {
            break;
        }

        let p_buffer = get_mapped_buffer(hfile_buffer);

        // hfileBuffer is invalid. Just leave.
        if p_buffer.is_null() {
            com_printf("Invalid hfileBuffer\n");
            break;
        }

        // SAFETY: p_buffer points to a valid shared memory region mapped by
        // MapViewOfFile. The region contains at least 3 i32 values followed
        // by text data, as per the QHOST protocol.
        unsafe {
            let buf = p_buffer as *mut i32;
            match *buf {
                CCOM_WRITE_TEXT => {
                    // Param1 : Text (starts at buf+1)
                    let text_ptr = buf.add(1) as *const u8;
                    let result = write_text_raw(h_stdin, text_ptr);
                    *buf = result as i32;
                }
                CCOM_GET_TEXT => {
                    // Param1 : Begin line, Param2 : End line
                    let i_begin_line = *buf.add(1);
                    let i_end_line = *buf.add(2);
                    let text_ptr = buf.add(1) as *mut u8;
                    let result = read_text_raw(h_stdout, text_ptr, i_begin_line, i_end_line);
                    *buf = result as i32;
                }
                CCOM_GET_SCR_LINES => {
                    // No params, result in buf[1]
                    let result = get_screen_buffer_lines_raw(h_stdout, buf.add(1));
                    *buf = result as i32;
                }
                CCOM_SET_SCR_LINES => {
                    // Param1 : Number of lines
                    let i_lines = *buf.add(1);
                    let result = set_console_cxcy(h_stdout, 80, i_lines as i16);
                    *buf = result as i32;
                }
                _ => {}
            }
        }

        release_mapped_buffer(p_buffer);

        // SAFETY: Signaling the child event to notify QHOST that the command is done.
        unsafe { win32::SetEvent(hevent_child_send); }
    }
}

// ============================================================
// Shared memory helpers
// ============================================================

/// Map a file-mapping object into memory.
///
/// Original: `LPVOID GetMappedBuffer(HANDLE hfileBuffer)`
#[cfg(target_os = "windows")]
fn get_mapped_buffer(hfile_buffer: win32::HANDLE) -> win32::LPVOID {
    // SAFETY: MapViewOfFile maps the file mapping object into the address space.
    // hfile_buffer is a valid file mapping handle passed from QHOST.
    unsafe {
        win32::MapViewOfFile(
            hfile_buffer,
            win32::FILE_MAP_READ | win32::FILE_MAP_WRITE,
            0,
            0,
            0,
        )
    }
}

/// Unmap a previously mapped buffer.
///
/// Original: `void ReleaseMappedBuffer(LPVOID pBuffer)`
#[cfg(target_os = "windows")]
fn release_mapped_buffer(p_buffer: win32::LPVOID) {
    // SAFETY: p_buffer was returned by MapViewOfFile and is a valid mapped view.
    unsafe { win32::UnmapViewOfFile(p_buffer); }
}

// ============================================================
// Console screen buffer helpers
// ============================================================

/// Get the number of lines in the console screen buffer, writing result to *pi_lines.
///
/// Original: `BOOL GetScreenBufferLines(int *piLines)`
#[cfg(target_os = "windows")]
fn get_screen_buffer_lines_raw(h_stdout: win32::HANDLE, pi_lines: *mut i32) -> win32::BOOL {
    let mut info = win32::CONSOLE_SCREEN_BUFFER_INFO::default();

    // SAFETY: GetConsoleScreenBufferInfo fills info for a valid console handle.
    let b_ret = unsafe { win32::GetConsoleScreenBufferInfo(h_stdout, &mut info) };

    if b_ret != 0 {
        // SAFETY: pi_lines points into the shared memory buffer which is valid.
        unsafe { *pi_lines = info.dw_size.y as i32; }
    }

    b_ret
}

/// Read text from the console screen buffer into raw buffer.
///
/// Original: `BOOL ReadText(LPTSTR pszText, int iBeginLine, int iEndLine)`
#[cfg(target_os = "windows")]
fn read_text_raw(
    h_stdout: win32::HANDLE,
    psz_text: *mut u8,
    i_begin_line: i32,
    i_end_line: i32,
) -> win32::BOOL {
    let coord = win32::COORD {
        x: 0,
        y: i_begin_line as win32::SHORT,
    };
    let mut dw_read: win32::DWORD = 0;
    let n_length = (80 * (i_end_line - i_begin_line + 1)) as win32::DWORD;

    // SAFETY: ReadConsoleOutputCharacterA reads characters from the console
    // screen buffer. psz_text points to valid shared memory with enough space.
    let b_ret = unsafe {
        win32::ReadConsoleOutputCharacterA(h_stdout, psz_text, n_length, coord, &mut dw_read)
    };

    // Make sure it's null terminated.
    if b_ret != 0 {
        // SAFETY: dw_read <= n_length, and the buffer is large enough.
        unsafe { *psz_text.add(dw_read as usize) = 0; }
    }

    b_ret
}

/// Write text to the console input buffer by synthesizing key events.
///
/// Original: `BOOL WriteText(LPCTSTR szText)`
#[cfg(target_os = "windows")]
fn write_text_raw(h_stdin: win32::HANDLE, sz_text: *const u8) -> win32::BOOL {
    let mut dw_written: win32::DWORD = 0;
    let mut sz = sz_text;

    // SAFETY: sz points into valid shared memory. We iterate until we find a
    // null terminator, reading one byte at a time.
    unsafe {
        while *sz != 0 {
            let mut c = *sz;

            // 13 is the code for a carriage return (\n) instead of 10.
            if c == 10 {
                c = 13;
            }

            let upper = c.to_ascii_uppercase();

            let mut rec: win32::INPUT_RECORD = std::mem::zeroed();
            rec.event_type = win32::KEY_EVENT;
            rec.event.key_event.b_key_down = win32::TRUE;
            rec.event.key_event.w_repeat_count = 1;
            rec.event.key_event.w_virtual_key_code = upper as win32::WORD;
            rec.event.key_event.w_virtual_scan_code = char_to_code(c) as win32::WORD;
            rec.event.key_event.u_char.ascii_char = c as win32::CHAR;
            rec.event.key_event.u_char.unicode_char = c as win32::WCHAR;
            rec.event.key_event.dw_control_key_state =
                if (*sz).is_ascii_uppercase() { 0x80 } else { 0x0 };

            // SAFETY: Writing a single INPUT_RECORD to the console input buffer.
            win32::WriteConsoleInputA(h_stdin, &rec, 1, &mut dw_written);

            rec.event.key_event.b_key_down = win32::FALSE;

            // SAFETY: Writing the key-up event.
            win32::WriteConsoleInputA(h_stdin, &rec, 1, &mut dw_written);

            sz = sz.add(1);
        }
    }

    win32::TRUE
}

/// Convert a character to a keyboard scan code.
///
/// Original: `int CharToCode(char c)`
fn char_to_code(c: u8) -> i32 {
    let upper = c.to_ascii_uppercase();

    match c {
        13 => 28,
        _ if c.is_ascii_alphabetic() => 30 + (upper as i32) - 65,
        _ if c.is_ascii_digit() => 1 + (upper as i32) - 47,
        _ => c as i32,
    }
}

/// Set the console window and buffer size.
///
/// Original: `BOOL SetConsoleCXCY(HANDLE hStdout, int cx, int cy)`
#[cfg(target_os = "windows")]
fn set_console_cxcy(h_stdout: win32::HANDLE, cx: i16, cy: i16) -> bool {
    // SAFETY: GetLargestConsoleWindowSize returns the largest possible console
    // window size for the given handle.
    let coord_max = unsafe { win32::GetLargestConsoleWindowSize(h_stdout) };

    let mut cy = cy;
    let mut cx = cx;

    if cy > coord_max.y {
        cy = coord_max.y;
    }
    if cx > coord_max.x {
        cx = coord_max.x;
    }

    let mut info = win32::CONSOLE_SCREEN_BUFFER_INFO::default();

    // SAFETY: GetConsoleScreenBufferInfo fills info for a valid console handle.
    if unsafe { win32::GetConsoleScreenBufferInfo(h_stdout, &mut info) } == 0 {
        return false;
    }

    // height
    info.sr_window.left = 0;
    info.sr_window.right = info.dw_size.x - 1;
    info.sr_window.top = 0;
    info.sr_window.bottom = cy - 1;

    if cy < info.dw_size.y {
        // SAFETY: SetConsoleWindowInfo sets the window rect for a valid console handle.
        if unsafe { win32::SetConsoleWindowInfo(h_stdout, win32::TRUE, &info.sr_window) } == 0 {
            return false;
        }

        info.dw_size.y = cy;

        // SAFETY: SetConsoleScreenBufferSize sets the buffer size for a valid console handle.
        if unsafe { win32::SetConsoleScreenBufferSize(h_stdout, info.dw_size) } == 0 {
            return false;
        }
    } else if cy > info.dw_size.y {
        info.dw_size.y = cy;

        // SAFETY: SetConsoleScreenBufferSize sets the buffer size for a valid console handle.
        if unsafe { win32::SetConsoleScreenBufferSize(h_stdout, info.dw_size) } == 0 {
            return false;
        }

        // SAFETY: SetConsoleWindowInfo sets the window rect for a valid console handle.
        if unsafe { win32::SetConsoleWindowInfo(h_stdout, win32::TRUE, &info.sr_window) } == 0 {
            return false;
        }
    }

    // SAFETY: Re-reading console info after height adjustment.
    if unsafe { win32::GetConsoleScreenBufferInfo(h_stdout, &mut info) } == 0 {
        return false;
    }

    // width
    info.sr_window.left = 0;
    info.sr_window.right = cx - 1;
    info.sr_window.top = 0;
    info.sr_window.bottom = info.dw_size.y - 1;

    if cx < info.dw_size.x {
        // SAFETY: SetConsoleWindowInfo sets the window rect for a valid console handle.
        if unsafe { win32::SetConsoleWindowInfo(h_stdout, win32::TRUE, &info.sr_window) } == 0 {
            return false;
        }

        info.dw_size.x = cx;

        // SAFETY: SetConsoleScreenBufferSize sets the buffer size for a valid console handle.
        if unsafe { win32::SetConsoleScreenBufferSize(h_stdout, info.dw_size) } == 0 {
            return false;
        }
    } else if cx > info.dw_size.x {
        info.dw_size.x = cx;

        // SAFETY: SetConsoleScreenBufferSize sets the buffer size for a valid console handle.
        if unsafe { win32::SetConsoleScreenBufferSize(h_stdout, info.dw_size) } == 0 {
            return false;
        }

        // SAFETY: SetConsoleWindowInfo sets the window rect for a valid console handle.
        if unsafe { win32::SetConsoleWindowInfo(h_stdout, win32::TRUE, &info.sr_window) } == 0 {
            return false;
        }
    }

    true
}

#[cfg(not(target_os = "windows"))]
fn set_console_cxcy(_h_stdout: *mut std::ffi::c_void, _cx: i16, _cy: i16) -> bool {
    false
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // CCOM command code constant tests
    // ============================================================

    #[test]
    fn test_ccom_write_text() {
        assert_eq!(CCOM_WRITE_TEXT, 0x2);
    }

    #[test]
    fn test_ccom_get_text() {
        assert_eq!(CCOM_GET_TEXT, 0x3);
    }

    #[test]
    fn test_ccom_get_scr_lines() {
        assert_eq!(CCOM_GET_SCR_LINES, 0x4);
    }

    #[test]
    fn test_ccom_set_scr_lines() {
        assert_eq!(CCOM_SET_SCR_LINES, 0x5);
    }

    #[test]
    fn test_ccom_codes_are_distinct() {
        let codes = [CCOM_WRITE_TEXT, CCOM_GET_TEXT, CCOM_GET_SCR_LINES, CCOM_SET_SCR_LINES];
        for i in 0..codes.len() {
            for j in (i + 1)..codes.len() {
                assert_ne!(codes[i], codes[j], "CCOM codes must be distinct");
            }
        }
    }

    // ============================================================
    // c_check_parm tests
    // ============================================================

    #[test]
    fn test_c_check_parm_found() {
        let args = vec![
            "myq2.exe".to_string(),
            "-HFILE".to_string(),
            "123".to_string(),
        ];
        assert_eq!(c_check_parm("-HFILE", &args), 1);
    }

    #[test]
    fn test_c_check_parm_found_later() {
        let args = vec![
            "myq2.exe".to_string(),
            "-HFILE".to_string(),
            "123".to_string(),
            "-HPARENT".to_string(),
            "456".to_string(),
        ];
        assert_eq!(c_check_parm("-HPARENT", &args), 3);
    }

    #[test]
    fn test_c_check_parm_not_found() {
        let args = vec![
            "myq2.exe".to_string(),
            "-HFILE".to_string(),
            "123".to_string(),
        ];
        assert_eq!(c_check_parm("-HCHILD", &args), 0);
    }

    #[test]
    fn test_c_check_parm_empty_args() {
        let args: Vec<String> = Vec::new();
        assert_eq!(c_check_parm("-HFILE", &args), 0);
    }

    #[test]
    fn test_c_check_parm_only_exe() {
        let args = vec!["myq2.exe".to_string()];
        assert_eq!(c_check_parm("-HFILE", &args), 0);
    }

    #[test]
    fn test_c_check_parm_skips_index_0() {
        // The exe name at index 0 should NOT be matched
        let args = vec!["-HFILE".to_string()];
        // Only one arg at index 0 (program name), so search starts at 1 which doesn't exist
        assert_eq!(c_check_parm("-HFILE", &args), 0);
    }

    #[test]
    fn test_c_check_parm_case_sensitive() {
        let args = vec![
            "myq2.exe".to_string(),
            "-hfile".to_string(),
        ];
        assert_eq!(c_check_parm("-HFILE", &args), 0);
    }

    #[test]
    fn test_c_check_parm_returns_first_occurrence() {
        let args = vec![
            "myq2.exe".to_string(),
            "-flag".to_string(),
            "-flag".to_string(),
        ];
        assert_eq!(c_check_parm("-flag", &args), 1);
    }

    #[test]
    fn test_c_check_parm_all_qhost_params() {
        let args = vec![
            "myq2.exe".to_string(),
            "-HFILE".to_string(),
            "100".to_string(),
            "-HPARENT".to_string(),
            "200".to_string(),
            "-HCHILD".to_string(),
            "300".to_string(),
        ];
        assert_eq!(c_check_parm("-HFILE", &args), 1);
        assert_eq!(c_check_parm("-HPARENT", &args), 3);
        assert_eq!(c_check_parm("-HCHILD", &args), 5);
    }

    // ============================================================
    // char_to_code tests
    // ============================================================

    #[test]
    fn test_char_to_code_enter() {
        assert_eq!(char_to_code(13), 28);
    }

    #[test]
    fn test_char_to_code_letter_a() {
        // Uppercase A: 30 + (65 - 65) = 30
        assert_eq!(char_to_code(b'A'), 30);
    }

    #[test]
    fn test_char_to_code_letter_a_lowercase() {
        // Lowercase a: upper = A = 65, so 30 + (65 - 65) = 30
        assert_eq!(char_to_code(b'a'), 30);
    }

    #[test]
    fn test_char_to_code_letter_z() {
        // Z: 30 + (90 - 65) = 30 + 25 = 55
        assert_eq!(char_to_code(b'Z'), 55);
    }

    #[test]
    fn test_char_to_code_letter_z_lowercase() {
        assert_eq!(char_to_code(b'z'), 55);
    }

    #[test]
    fn test_char_to_code_digit_0() {
        // '0' = 48: 1 + (48 - 47) = 2
        assert_eq!(char_to_code(b'0'), 2);
    }

    #[test]
    fn test_char_to_code_digit_1() {
        // '1' = 49: 1 + (49 - 47) = 3
        assert_eq!(char_to_code(b'1'), 3);
    }

    #[test]
    fn test_char_to_code_digit_9() {
        // '9' = 57: 1 + (57 - 47) = 11
        assert_eq!(char_to_code(b'9'), 11);
    }

    #[test]
    fn test_char_to_code_letters_sequential() {
        // Letters should map to sequential scan codes
        for (i, c) in (b'A'..=b'Z').enumerate() {
            assert_eq!(char_to_code(c), 30 + i as i32);
        }
    }

    #[test]
    fn test_char_to_code_digits_sequential() {
        // Digits should map to sequential scan codes
        for (i, c) in (b'0'..=b'9').enumerate() {
            assert_eq!(char_to_code(c), 2 + i as i32);
        }
    }

    #[test]
    fn test_char_to_code_other_chars_passthrough() {
        // Non-alphanumeric, non-enter characters should just return the char value
        assert_eq!(char_to_code(b' '), b' ' as i32);
        assert_eq!(char_to_code(b'.'), b'.' as i32);
        assert_eq!(char_to_code(b'-'), b'-' as i32);
        assert_eq!(char_to_code(b'/'), b'/' as i32);
    }

    #[test]
    fn test_char_to_code_case_insensitive() {
        // Lowercase and uppercase should produce the same scan code
        for c in b'a'..=b'z' {
            let upper = c.to_ascii_uppercase();
            assert_eq!(char_to_code(c), char_to_code(upper),
                "scan code for '{}' should match '{}'", c as char, upper as char);
        }
    }

    // ============================================================
    // Win32 type sanity tests (compile-time layout verification)
    // ============================================================

    #[cfg(target_os = "windows")]
    mod win32_tests {
        use super::super::win32::*;

        #[test]
        fn test_coord_default() {
            let c = COORD::default();
            assert_eq!(c.x, 0);
            assert_eq!(c.y, 0);
        }

        #[test]
        fn test_small_rect_default() {
            let r = SMALL_RECT::default();
            assert_eq!(r.left, 0);
            assert_eq!(r.top, 0);
            assert_eq!(r.right, 0);
            assert_eq!(r.bottom, 0);
        }

        #[test]
        fn test_console_screen_buffer_info_default() {
            let info = CONSOLE_SCREEN_BUFFER_INFO::default();
            assert_eq!(info.dw_size.x, 0);
            assert_eq!(info.dw_size.y, 0);
            assert_eq!(info.w_attributes, 0);
        }

        #[test]
        fn test_null_handle() {
            assert!(NULL_HANDLE.is_null());
        }

        #[test]
        fn test_std_handle_constants() {
            // These are (DWORD)-11 and (DWORD)-10 in C
            assert_eq!(STD_OUTPUT_HANDLE, 0xFFFFFFF5);
            assert_eq!(STD_INPUT_HANDLE, 0xFFFFFFF6);
        }

        #[test]
        fn test_key_event_constant() {
            assert_eq!(KEY_EVENT, 0x0001);
        }
    }
}
