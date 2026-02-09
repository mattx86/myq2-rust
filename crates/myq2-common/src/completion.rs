//! Console command autocompletion system.
//!
//! Provides bash-style tab completion for:
//! - Commands, aliases, and cvars (first word)
//! - Filenames for commands that take file arguments (map, exec, etc.)

use crate::cmd::with_cmd_ctx;
use crate::cvar::with_cvar_ctx;
use crate::files::with_fs_ctx;

/// What type of argument a command expects at a given position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgType {
    /// No specific completion
    None,
    /// Map files (maps/*.bsp, strip extension)
    MapName,
    /// Config files (*.cfg in all search paths)
    ConfigFile,
    /// Demo files (demos/*.dm2)
    DemoFile,
    /// Save directory names (save/*/)
    SaveDir,
}

/// Result of a completion operation.
#[derive(Debug, Default)]
pub struct CompletionResult {
    /// All matches found (sorted, deduplicated)
    pub matches: Vec<String>,
    /// Longest common prefix of all matches
    pub common_prefix: String,
}

/// Determine what argument type a command expects at a given position.
fn get_arg_type(command: &str, arg_index: usize) -> ArgType {
    let cmd_lower = command.to_lowercase();
    match (cmd_lower.as_str(), arg_index) {
        ("map" | "gamemap" | "demomap", 0) => ArgType::MapName,
        ("exec", 0) => ArgType::ConfigFile,
        ("record" | "playdemo", 0) => ArgType::DemoFile,
        ("loadgame" | "savegame", 0) => ArgType::SaveDir,
        _ => ArgType::None,
    }
}

/// Parse a console input line to determine what's being completed.
/// Returns (command_name, arg_index, partial_text).
/// - arg_index 0 means we're completing the first word (command/cvar/alias)
/// - arg_index > 0 means we're completing an argument to the command
fn parse_line(line: &str) -> (String, usize, String) {
    let line = line.trim_start();

    // Handle empty line
    if line.is_empty() {
        return (String::new(), 0, String::new());
    }

    let parts: Vec<&str> = line.split_whitespace().collect();

    if parts.is_empty() {
        return (String::new(), 0, String::new());
    }

    // If no trailing space and only one word, we're completing the command itself
    if !line.ends_with(' ') && !line.ends_with('\t') && parts.len() == 1 {
        return (String::new(), 0, parts[0].to_string());
    }

    let command = parts[0].to_string();

    // Determine which argument position we're at
    // If line ends with space, we're starting a new argument (empty partial)
    // Otherwise, we're in the middle of typing an argument
    let (arg_index, partial) = if line.ends_with(' ') || line.ends_with('\t') {
        (parts.len(), String::new())
    } else {
        (parts.len() - 1, parts.last().unwrap_or(&"").to_string())
    };

    (command, arg_index, partial)
}

/// Find the longest common prefix of a list of strings.
fn find_common_prefix(strings: &[String]) -> String {
    if strings.is_empty() {
        return String::new();
    }
    if strings.len() == 1 {
        return strings[0].clone();
    }

    let first = &strings[0];
    let mut prefix_len = first.len();

    for s in &strings[1..] {
        let common = first
            .chars()
            .zip(s.chars())
            .take(prefix_len)
            .take_while(|(a, b)| a.eq_ignore_ascii_case(b))
            .count();
        prefix_len = common;
        if prefix_len == 0 {
            break;
        }
    }

    first[..prefix_len].to_string()
}

/// Complete commands, aliases, and cvars (for first word completion).
fn complete_command_or_cvar(partial: &str) -> Vec<String> {
    let mut matches = Vec::new();

    // Get matching commands and aliases
    if let Some(cmd_matches) = with_cmd_ctx(|ctx| {
        let mut m: Vec<String> = ctx
            .complete_all_commands(partial)
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        m.extend(
            ctx.complete_all_aliases(partial)
                .into_iter()
                .map(|s| s.to_string()),
        );
        m
    }) {
        matches.extend(cmd_matches);
    }

    // Get matching cvars
    if let Some(cvar_matches) = with_cvar_ctx(|ctx| {
        ctx.complete_all_variables(partial)
            .into_iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
    }) {
        matches.extend(cvar_matches);
    }

    matches.sort();
    matches.dedup();
    matches
}

/// List map files matching a partial name.
fn list_maps(partial: &str) -> Vec<String> {
    let mut maps = Vec::new();

    with_fs_ctx(|ctx| {
        // Search in pak files
        for sp in &ctx.search_paths {
            if let Some(ref pack) = sp.pack {
                for pf in &pack.files {
                    let name_lower = pf.name.to_lowercase();
                    if name_lower.starts_with("maps/") && name_lower.ends_with(".bsp") {
                        // Extract map name without path and extension
                        let map_name = &pf.name[5..pf.name.len() - 4];
                        if map_name.to_lowercase().starts_with(&partial.to_lowercase()) {
                            maps.push(map_name.to_string());
                        }
                    }
                }
            }
        }

        // Also search directories for loose .bsp files
        for sp in &ctx.search_paths {
            if sp.pack.is_none() {
                let maps_dir = format!("{}/maps", sp.filename);
                if let Ok(entries) = std::fs::read_dir(&maps_dir) {
                    for entry in entries.flatten() {
                        let fname = entry.file_name().to_string_lossy().to_string();
                        if fname.to_lowercase().ends_with(".bsp") {
                            let map_name = &fname[..fname.len() - 4];
                            if map_name.to_lowercase().starts_with(&partial.to_lowercase()) {
                                maps.push(map_name.to_string());
                            }
                        }
                    }
                }
            }
        }
    });

    maps.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    maps.dedup();
    maps
}

/// List config files matching a partial name.
fn list_configs(partial: &str) -> Vec<String> {
    let mut configs = Vec::new();
    let partial_lower = partial.to_lowercase();

    with_fs_ctx(|ctx| {
        // Search in pak files
        for sp in &ctx.search_paths {
            if let Some(ref pack) = sp.pack {
                for pf in &pack.files {
                    let name_lower = pf.name.to_lowercase();
                    if name_lower.ends_with(".cfg") {
                        // Use the full path from pak, but check if it starts with partial
                        if name_lower.starts_with(&partial_lower) {
                            configs.push(pf.name.clone());
                        }
                        // Also check just the filename
                        if let Some(fname) = pf.name.rsplit('/').next() {
                            if fname.to_lowercase().starts_with(&partial_lower) {
                                configs.push(fname.to_string());
                            }
                        }
                    }
                }
            }
        }

        // Also search directories
        for sp in &ctx.search_paths {
            if sp.pack.is_none() {
                if let Ok(entries) = std::fs::read_dir(&sp.filename) {
                    for entry in entries.flatten() {
                        let fname = entry.file_name().to_string_lossy().to_string();
                        if fname.to_lowercase().ends_with(".cfg")
                            && fname.to_lowercase().starts_with(&partial_lower)
                        {
                            configs.push(fname);
                        }
                    }
                }
            }
        }
    });

    configs.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    configs.dedup();
    configs
}

/// List demo files matching a partial name.
fn list_demos(partial: &str) -> Vec<String> {
    let mut demos = Vec::new();
    let partial_lower = partial.to_lowercase();

    with_fs_ctx(|ctx| {
        // Search in pak files
        for sp in &ctx.search_paths {
            if let Some(ref pack) = sp.pack {
                for pf in &pack.files {
                    let name_lower = pf.name.to_lowercase();
                    if name_lower.starts_with("demos/") && name_lower.ends_with(".dm2") {
                        let demo_name = &pf.name[6..pf.name.len() - 4];
                        if demo_name.to_lowercase().starts_with(&partial_lower) {
                            demos.push(demo_name.to_string());
                        }
                    }
                }
            }
        }

        // Also search directories
        for sp in &ctx.search_paths {
            if sp.pack.is_none() {
                let demos_dir = format!("{}/demos", sp.filename);
                if let Ok(entries) = std::fs::read_dir(&demos_dir) {
                    for entry in entries.flatten() {
                        let fname = entry.file_name().to_string_lossy().to_string();
                        if fname.to_lowercase().ends_with(".dm2") {
                            let demo_name = &fname[..fname.len() - 4];
                            if demo_name.to_lowercase().starts_with(&partial_lower) {
                                demos.push(demo_name.to_string());
                            }
                        }
                    }
                }
            }
        }
    });

    demos.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    demos.dedup();
    demos
}

/// List save directories matching a partial name.
fn list_save_dirs(partial: &str) -> Vec<String> {
    let mut saves = Vec::new();
    let partial_lower = partial.to_lowercase();

    with_fs_ctx(|ctx| {
        let save_dir = format!("{}/save", ctx.gamedir);
        if let Ok(entries) = std::fs::read_dir(&save_dir) {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    let dname = entry.file_name().to_string_lossy().to_string();
                    // Check that this save dir contains server.ssv (valid save)
                    let ssv_path = entry.path().join("server.ssv");
                    if ssv_path.exists() && dname.to_lowercase().starts_with(&partial_lower) {
                        saves.push(dname);
                    }
                }
            }
        }
    });

    saves.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    saves.dedup();
    saves
}

/// Complete an argument based on its expected type.
fn complete_argument(command: &str, arg_index: usize, partial: &str) -> Vec<String> {
    let arg_type = get_arg_type(command, arg_index - 1); // arg_index is 1-based here

    match arg_type {
        ArgType::MapName => list_maps(partial),
        ArgType::ConfigFile => list_configs(partial),
        ArgType::DemoFile => list_demos(partial),
        ArgType::SaveDir => list_save_dirs(partial),
        ArgType::None => Vec::new(),
    }
}

/// Main entry point for console line completion.
/// Call this with the current input line (without the leading ']' prompt).
pub fn complete_line(line: &str) -> CompletionResult {
    let (command, arg_index, partial) = parse_line(line);

    let matches = if arg_index == 0 {
        // Completing command/cvar/alias
        complete_command_or_cvar(&partial)
    } else {
        // Completing argument
        complete_argument(&command, arg_index, &partial)
    };

    let common_prefix = if matches.is_empty() {
        partial
    } else {
        find_common_prefix(&matches)
    };

    CompletionResult {
        matches,
        common_prefix,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_line_empty() {
        let (cmd, idx, partial) = parse_line("");
        assert_eq!(cmd, "");
        assert_eq!(idx, 0);
        assert_eq!(partial, "");
    }

    #[test]
    fn test_parse_line_partial_command() {
        let (cmd, idx, partial) = parse_line("ma");
        assert_eq!(cmd, "");
        assert_eq!(idx, 0);
        assert_eq!(partial, "ma");
    }

    #[test]
    fn test_parse_line_command_with_space() {
        let (cmd, idx, partial) = parse_line("map ");
        assert_eq!(cmd, "map");
        assert_eq!(idx, 1);
        assert_eq!(partial, "");
    }

    #[test]
    fn test_parse_line_command_with_partial_arg() {
        let (cmd, idx, partial) = parse_line("map ba");
        assert_eq!(cmd, "map");
        assert_eq!(idx, 1);
        assert_eq!(partial, "ba");
    }

    #[test]
    fn test_find_common_prefix_empty() {
        assert_eq!(find_common_prefix(&[]), "");
    }

    #[test]
    fn test_find_common_prefix_single() {
        assert_eq!(
            find_common_prefix(&["hello".to_string()]),
            "hello"
        );
    }

    #[test]
    fn test_find_common_prefix_multiple() {
        assert_eq!(
            find_common_prefix(&[
                "map".to_string(),
                "maxclients".to_string(),
                "mapname".to_string()
            ]),
            "ma"
        );
    }

    #[test]
    fn test_find_common_prefix_identical() {
        assert_eq!(
            find_common_prefix(&["test".to_string(), "test".to_string()]),
            "test"
        );
    }
}
