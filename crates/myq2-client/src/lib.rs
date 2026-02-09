#![allow(dead_code, unused_variables, unused_assignments, unused_mut, unused_imports, static_mut_refs, unpredictable_function_pointer_comparisons, non_snake_case, private_interfaces)]
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
// Client module — converted from myq2-original/client/
pub mod client;
pub mod cl_cin;
pub mod cl_main;
pub mod cl_part;
pub mod console;
pub mod console_types;
pub mod keys;
pub mod menu;
pub mod ref_def;
pub mod sound_types;
pub mod cl_scrn;
pub mod cl_view;
pub mod cl_inv;
pub mod cl_ents;
pub mod cl_input;
pub mod cl_pred;
pub mod cl_parse;
pub mod cl_tent;
pub mod cl_fx;
pub mod cl_newfx;
pub mod qmenu;
pub mod snd_dma;
pub mod snd_mem;
pub mod cl_timing;
pub mod cl_demo;

pub mod platform;
pub mod cl_http;
pub mod cl_loc;
pub mod cl_browser;
pub mod cl_chat;
pub mod cl_crosshair;
pub mod cl_hud;
pub mod cl_smooth;
