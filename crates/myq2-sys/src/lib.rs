#![allow(dead_code, unused_variables, unused_assignments, unused_mut)]
#![allow(clippy::needless_return, clippy::too_many_arguments, clippy::collapsible_if,
         clippy::collapsible_else_if, clippy::field_reassign_with_default,
         clippy::manual_range_contains, clippy::single_match, clippy::comparison_chain,
         clippy::identity_op, clippy::float_cmp, clippy::needless_range_loop,
         clippy::match_single_binding, clippy::if_same_then_else,
         clippy::missing_safety_doc, clippy::manual_clamp, clippy::ptr_arg,
         clippy::type_complexity, clippy::needless_late_init, clippy::manual_strip,
         clippy::manual_is_ascii_check, clippy::redundant_locals, clippy::while_let_loop,
         clippy::unnecessary_unwrap, clippy::unnecessary_cast, clippy::nonminimal_bool,
         clippy::manual_memcpy, clippy::manual_find, clippy::redundant_guards,
         clippy::wildcard_in_or_patterns, clippy::empty_line_after_doc_comments)]

// Platform layer — converted from myq2-original/win32/

// Shared network constants
pub const MAX_LOOPBACK: usize = 4;

pub mod conproc;
pub mod glw_imp;
pub mod net_common;
pub mod net_tcp;
pub mod net_udp;
pub mod q_shwin;
pub mod qvk_win;
pub mod snd_openal;

pub mod sys_win;
pub mod in_win;
pub mod vid_dll;
pub mod vid_menu;
pub mod platform_register;
pub mod renderer_bridge;
pub mod net_io_thread;
