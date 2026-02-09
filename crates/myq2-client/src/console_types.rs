// Console types — converted from myq2-original/client/console.h

/// mattx86: console_notifylines (from myq2opts.h)
pub const NUM_CON_TIMES: usize = 5;

/// mattx86: console_textsize (from myq2opts.h)
pub const CON_TEXTSIZE: usize = 131072;

#[repr(C)]
pub struct Console {
    pub initialized: bool,

    pub text: [u8; CON_TEXTSIZE],
    /// Line where next message will be printed
    pub current: i32,
    /// Offset in current line for next print
    pub x: i32,
    /// Bottom of console displays this line
    pub display: i32,

    /// High bit mask for colored characters
    pub ormask: i32,

    /// Characters across screen
    pub linewidth: i32,
    /// Total lines in console scrollback
    pub totallines: i32,

    pub cursorspeed: f32,

    pub vislines: i32,

    /// cls.realtime time the line was generated, for transparent notify lines
    pub times: [f32; NUM_CON_TIMES],
}

impl Default for Console {
    fn default() -> Self {
        Self {
            initialized: false,
            text: [b' '; CON_TEXTSIZE],
            current: 0,
            x: 0,
            display: 0,
            ormask: 0,
            linewidth: 0,
            totallines: 0,
            cursorspeed: 0.0,
            vislines: 0,
            times: [0.0; NUM_CON_TIMES],
        }
    }
}
