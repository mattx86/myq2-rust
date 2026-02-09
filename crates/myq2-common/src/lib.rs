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

pub mod q_shared;
pub mod qfiles;
pub mod crc;
pub mod md4;
pub mod wildcards;
pub mod qcommon;
pub mod cmd;
pub mod cvar;
pub mod common;
mod anorms;
pub mod net_chan;
pub mod files;
pub mod pmove;
pub mod completion;
pub mod cmodel;
pub mod net;
pub mod keys;
pub mod net_queue;
pub mod game_api;
pub mod compression;
