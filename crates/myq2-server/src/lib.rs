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

// Server module — converted from myq2-original/server/

pub mod server;
pub mod sv_ccmds;
pub mod sv_ents;
pub mod sv_game;
pub mod sv_init;
pub mod sv_main;
pub mod sv_send;
pub mod sv_user;
pub mod sv_world;
pub mod server_game_import;
pub mod game_dll;
pub mod game_ffi;
pub mod sv_lag_compensation;
