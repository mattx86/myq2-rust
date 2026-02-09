#![allow(dead_code, unused_variables, unused_assignments, unused_mut, static_mut_refs, non_upper_case_globals, unused_unsafe)]
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
// Vulkan 1.3 renderer with ray tracing support

// Vulkan backend (replaces SDL3 GPU)
pub mod vulkan;

// Vulkan renderer modules (converted from legacy OpenGL code)
pub mod vk_bindings;
pub mod vk_model_types;
pub mod vk_local;
pub mod qvk;
pub mod vk_model;
pub mod vk_image;
pub mod vk_draw;
pub mod vk_light;
pub mod vk_warp;
pub mod vk_rsurf;
pub mod vk_rmain;
pub mod vk_rmisc;
pub mod platform;
pub mod modern;
