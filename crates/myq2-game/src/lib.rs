#![allow(dead_code, unused_variables, unused_assignments, unused_mut, unused_doc_comments)]
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
// Game module — converted from myq2-original/game/

pub mod ai_wrappers;
pub mod entity_adapters;
pub mod dispatch;
pub mod game_import;
pub mod game;
pub mod g_local;
pub mod m_player_frames;
pub mod g_utils;
pub mod g_combat;
pub mod g_weapon;
pub mod g_phys;
pub mod g_ai;
pub mod m_flash;
pub mod g_items;
pub mod g_main;
pub mod g_trigger;
pub mod g_target;
pub mod g_spawn;
pub mod g_svcmds;
pub mod g_chase;
pub mod g_turret;
pub mod g_monster;
pub mod g_cmds;
pub mod p_trail;
pub mod g_save;
pub mod g_misc;
pub mod p_hud;
pub mod p_view;
pub mod p_weapon;
pub mod p_client;
pub mod m_move;
pub mod m_berserk;
pub mod m_brain;
pub mod m_gladiator;
pub mod m_actor;
pub mod m_flipper;
pub mod m_flyer;
pub mod g_func;
pub mod m_gunner;
pub mod m_hover;
pub mod m_parasite;
pub mod m_float;
pub mod m_chick;
pub mod m_boss3;
pub mod m_infantry;
pub mod m_insane;
pub mod m_mutant;
pub mod m_medic;
pub mod m_boss2;
pub mod m_boss31;
pub mod m_boss32;
pub mod m_supertank;
pub mod m_tank;
pub mod m_soldier;
