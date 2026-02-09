// cvar.rs — dynamic variable tracking
// Converted from: myq2-original/qcommon/cvar.c

use crate::common::{com_printf, com_dprintf};
use crate::q_shared::{
    CVAR_ARCHIVE, CVAR_LATCH, CVAR_NOSET, CVAR_SERVERINFO, CVAR_USERINFO,
    MAX_INFO_STRING, info_set_value_for_key,
};
use crate::wildcards::wildcardfit;

use std::collections::HashMap;

/// A console variable.
#[derive(Clone)]
pub struct Cvar {
    pub name: String,
    pub string: String,
    pub latched_string: Option<String>,
    pub flags: i32,
    pub modified: bool,
    pub value: f32,
}

/// Callback type for FS_SetGamedir.
pub type FsSetGamedirFn = Box<dyn Fn(&str) + Send>;

/// Callback type for FS_ExecAutoexec.
pub type FsExecAutoexecFn = Box<dyn Fn() + Send>;

/// The full cvar system context.
pub struct CvarContext {
    pub cvar_vars: Vec<Cvar>,
    /// O(1) cvar lookup by name -> index in cvar_vars
    cvar_index: HashMap<String, usize>,
    pub userinfo_modified: bool,
    /// Callback to set the game directory (wired to FS_SetGamedir).
    pub fs_set_gamedir: Option<FsSetGamedirFn>,
    /// Callback to execute autoexec.cfg (wired to FS_ExecAutoexec).
    pub fs_exec_autoexec: Option<FsExecAutoexecFn>,
}

impl CvarContext {
    pub fn new() -> Self {
        Self {
            cvar_vars: Vec::new(),
            cvar_index: HashMap::new(),
            userinfo_modified: false,
            fs_set_gamedir: None,
            fs_exec_autoexec: None,
        }
    }

    /// Validate that a string doesn't contain characters invalid in info strings.
    pub fn info_validate(s: &str) -> bool {
        !s.contains('\\') && !s.contains('"') && !s.contains(';')
    }

    /// Find a cvar by name, returning its index. O(1) via HashMap.
    pub fn find_var_index(&self, name: &str) -> Option<usize> {
        self.cvar_index.get(name).copied()
    }

    /// Find a cvar by name. O(1) via HashMap.
    pub fn find_var(&self, name: &str) -> Option<&Cvar> {
        self.cvar_index.get(name).map(|&idx| &self.cvar_vars[idx])
    }

    /// Find a cvar by name (mutable). O(1) via HashMap.
    pub fn find_var_mut(&mut self, name: &str) -> Option<&mut Cvar> {
        if let Some(&idx) = self.cvar_index.get(name) {
            Some(&mut self.cvar_vars[idx])
        } else {
            None
        }
    }

    /// Get the floating-point value of a cvar. Returns 0 if not found.
    pub fn variable_value(&self, name: &str) -> f32 {
        match self.find_var(name) {
            Some(var) => var.value,
            None => 0.0,
        }
    }

    /// Get the string value of a cvar. Returns "" if not found.
    pub fn variable_string(&self, name: &str) -> &str {
        match self.find_var(name) {
            Some(var) => &var.string,
            None => "",
        }
    }

    /// Attempt to match a partial variable name for auto-completion.
    pub fn complete_variable(&self, partial: &str) -> Option<&str> {
        if partial.is_empty() {
            return None;
        }

        // Check exact match
        for var in &self.cvar_vars {
            if var.name == partial {
                return Some(&var.name);
            }
        }

        // Check partial match
        for var in &self.cvar_vars {
            if var.name.starts_with(partial) {
                return Some(&var.name);
            }
        }

        None
    }

    /// Get all cvars matching a prefix (for multi-completion).
    pub fn complete_all_variables(&self, partial: &str) -> Vec<&str> {
        self.cvar_vars
            .iter()
            .filter(|v| v.name.starts_with(partial))
            .map(|v| v.name.as_str())
            .collect()
    }

    /// Get or create a cvar. If it already exists, the value is not changed
    /// but flags are OR'd in. O(1) lookup via HashMap.
    pub fn get(&mut self, name: &str, value: Option<&str>, flags: i32) -> Option<usize> {
        if flags & (CVAR_USERINFO | CVAR_SERVERINFO) != 0
            && !Self::info_validate(name) {
                com_printf("invalid info cvar name\n");
                return None;
            }

        // O(1) lookup
        if let Some(&idx) = self.cvar_index.get(name) {
            self.cvar_vars[idx].flags |= flags;
            return Some(idx);
        }

        let value = value?;

        if flags & (CVAR_USERINFO | CVAR_SERVERINFO) != 0
            && !Self::info_validate(value) {
                com_printf("invalid info cvar value\n");
                return None;
            }

        let float_val = value.parse::<f32>().unwrap_or(0.0);
        let idx = self.cvar_vars.len();
        self.cvar_vars.push(Cvar {
            name: name.to_string(),
            string: value.to_string(),
            latched_string: None,
            flags,
            modified: true,
            value: float_val,
        });
        self.cvar_index.insert(name.to_string(), idx);

        Some(idx)
    }

    /// Convenience: get or create a cvar, returning its index.
    /// Panics if creation fails (invalid info string).
    pub fn get_or_create(&mut self, name: &str, value: &str, flags: i32) -> usize {
        self.get(name, Some(value), flags)
            .expect("failed to create cvar")
    }

    /// Internal set implementation.
    fn set2(&mut self, name: &str, value: &str, force: bool, server_state: i32) -> Option<usize> {
        let idx = match self.find_var_index(name) {
            Some(idx) => idx,
            None => return self.get(name, Some(value), 0),
        };

        if self.cvar_vars[idx].flags & (CVAR_USERINFO | CVAR_SERVERINFO) != 0
            && !Self::info_validate(value) {
                com_printf("invalid info cvar value\n");
                return Some(idx);
            }

        if !force {
            if self.cvar_vars[idx].flags & CVAR_NOSET != 0 {
                com_printf(&format!("{} is write protected.\n", name));
                return Some(idx);
            }

            if self.cvar_vars[idx].flags & CVAR_LATCH != 0 {
                if let Some(ref latched) = self.cvar_vars[idx].latched_string {
                    if value == latched {
                        return Some(idx);
                    }
                } else if value == self.cvar_vars[idx].string {
                    return Some(idx);
                }

                if server_state != 0 {
                    com_printf(&format!("{} will be changed for next game.\n", name));
                    self.cvar_vars[idx].latched_string = Some(value.to_string());
                } else {
                    self.cvar_vars[idx].string = value.to_string();
                    self.cvar_vars[idx].value = value.parse::<f32>().unwrap_or(0.0);
                    if name == "game" {
                        if let Some(ref fs_sgd) = self.fs_set_gamedir {
                            fs_sgd(value);
                        }
                        if let Some(ref fs_ea) = self.fs_exec_autoexec {
                            fs_ea();
                        }
                    }
                }
                return Some(idx);
            }
        } else {
            self.cvar_vars[idx].latched_string = None;
        }

        if value == self.cvar_vars[idx].string {
            return Some(idx); // not changed
        }

        self.cvar_vars[idx].modified = true;

        if self.cvar_vars[idx].flags & CVAR_USERINFO != 0 {
            self.userinfo_modified = true;
        }

        self.cvar_vars[idx].string = value.to_string();
        self.cvar_vars[idx].value = value.parse::<f32>().unwrap_or(0.0);

        Some(idx)
    }

    /// Set a cvar value (respects NOSET and LATCH flags).
    pub fn set(&mut self, name: &str, value: &str) -> Option<usize> {
        self.set2(name, value, false, 0)
    }

    /// Set a cvar value with a specific server state (for LATCH handling).
    pub fn set_with_server_state(&mut self, name: &str, value: &str, server_state: i32) -> Option<usize> {
        self.set2(name, value, false, server_state)
    }

    /// Force-set a cvar value (ignores NOSET and LATCH).
    pub fn force_set(&mut self, name: &str, value: &str) -> Option<usize> {
        self.set2(name, value, true, 0)
    }

    /// Set a cvar with explicit flags (FullSet).
    pub fn full_set(&mut self, name: &str, value: &str, flags: i32) -> Option<usize> {
        let idx = match self.find_var_index(name) {
            Some(idx) => idx,
            None => return self.get(name, Some(value), flags),
        };

        self.cvar_vars[idx].modified = true;

        if self.cvar_vars[idx].flags & CVAR_USERINFO != 0 {
            self.userinfo_modified = true;
        }

        self.cvar_vars[idx].string = value.to_string();
        self.cvar_vars[idx].value = value.parse::<f32>().unwrap_or(0.0);
        self.cvar_vars[idx].flags = flags;

        Some(idx)
    }

    /// Set a cvar from a float value.
    pub fn set_value(&mut self, name: &str, value: f32) {
        let val_str = if value == (value as i32) as f32 {
            format!("{}", value as i32)
        } else {
            format!("{}", value)
        };
        self.set(name, &val_str);
    }

    /// Apply all latched variable changes.
    pub fn get_latched_vars(&mut self) {
        for var in &mut self.cvar_vars {
            if let Some(latched) = var.latched_string.take() {
                var.string = latched;
                var.value = var.string.parse::<f32>().unwrap_or(0.0);
                if var.name == "game" {
                    if let Some(ref fs_sgd) = self.fs_set_gamedir {
                        fs_sgd(&var.string);
                    }
                    if let Some(ref fs_ea) = self.fs_exec_autoexec {
                        fs_ea();
                    }
                }
            }
        }
    }

    /// Check for modified cvars and queue auto-restart commands.
    /// Returns a list of commands to execute (e.g., "snd_restart\n", "vid_restart\n").
    pub fn check_modified(&mut self) -> Vec<String> {
        let mut commands = Vec::new();

        for var in &mut self.cvar_vars {
            if !var.modified {
                continue;
            }
            var.modified = false;

            if wildcardfit("s_*", &var.name)
                && !var.name.eq_ignore_ascii_case("s_verbose")
                && !var.name.eq_ignore_ascii_case("s_volume")
            {
                commands.push("snd_restart\n".to_string());
                com_dprintf(&format!("var = \"{}\"; event = \"snd_restart\"\n", var.name));
            } else if (wildcardfit("vk_*", &var.name)
                || wildcardfit("vid_*", &var.name)
                || wildcardfit("r_*", &var.name)
                || var.name.eq_ignore_ascii_case("intensity"))
                && !wildcardfit("vk_refl_*", &var.name)
                && !var.name.eq_ignore_ascii_case("r_celshading")
                && !var.name.eq_ignore_ascii_case("r_fog")
                && !var.name.eq_ignore_ascii_case("r_timebasedfx")
                && !var.name.eq_ignore_ascii_case("r_verbose")
            {
                commands.push("vid_restart\n".to_string());
                com_dprintf(&format!("var = \"{}\"; event = \"vid_restart\"\n", var.name));
            } else if var.name.eq_ignore_ascii_case("cl_defaultskin")
                || var.name.eq_ignore_ascii_case("cl_noskins")
            {
                commands.push("skins\n".to_string());
                com_dprintf(&format!("var = \"{}\"; event = \"skins\"\n", var.name));
            }
        }

        commands
    }

    /// Handle variable inspection/changing from the console.
    /// Returns true if the command was a cvar reference.
    pub fn command(&mut self, argv0: &str, argc: usize, argv1: Option<&str>) -> bool {
        let idx = match self.find_var_index(argv0) {
            Some(idx) => idx,
            None => return false,
        };

        if argc == 1 {
            com_printf(&format!(
                "\"{}\" is \"{}\"\n",
                self.cvar_vars[idx].name, self.cvar_vars[idx].string
            ));
            return true;
        }

        if let Some(value) = argv1 {
            let name = self.cvar_vars[idx].name.clone();
            self.set(&name, value);
        }
        true
    }

    /// Write all archived cvars to a writer.
    pub fn write_variables(&self, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        for var in &self.cvar_vars {
            if var.flags & CVAR_ARCHIVE != 0 {
                writeln!(writer, "set {} \"{}\"", var.name, var.string)?;
            }
        }
        Ok(())
    }

    /// Write address book cvars (adr0-adr99) to a writer.
    pub fn write_address_book(&self, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        for i in 0..=99 {
            let name = format!("adr{}", i);
            let value = self.variable_string(&name);
            writeln!(writer, "set {} \"{}\"", name, value)?;
        }
        Ok(())
    }

    /// Build an info string from all cvars with the given flag bit set.
    pub fn bit_info(&self, bit: i32) -> String {
        let mut info = String::with_capacity(MAX_INFO_STRING);
        for var in &self.cvar_vars {
            if var.flags & bit != 0 {
                info_set_value_for_key(&mut info, &var.name, &var.string);
            }
        }
        info
    }

    /// Get userinfo string.
    pub fn userinfo(&self) -> String {
        self.bit_info(CVAR_USERINFO)
    }

    /// Get serverinfo string.
    pub fn serverinfo(&self) -> String {
        self.bit_info(CVAR_SERVERINFO)
    }

    /// Console command handler for "set <variable> <value> [u / s]".
    /// `args` should contain all arguments after "set" (i.e., argv[1..]).
    pub fn set_f(&mut self, argc: usize, argv: &[&str]) {
        if argc != 3 && argc != 4 {
            com_printf("usage: set <variable> <value> [u / s]\n");
            return;
        }

        if argc == 4 {
            let flags = match argv[2] {
                "u" => CVAR_USERINFO,
                "s" => CVAR_SERVERINFO,
                _ => {
                    com_printf("flags can only be 'u' or 's'\n");
                    return;
                }
            };
            self.full_set(argv[0], argv[1], flags);
        } else {
            self.set(argv[0], argv[1]);
        }
    }

    /// Console command handler for "unset <variable>".
    /// Simply sets the value of the cvar to "".
    pub fn unset_f(&mut self, argc: usize, argv: &[&str]) {
        if argc != 2 {
            com_printf("usage: unset <variable>\n");
            return;
        }

        self.set(argv[0], "");
    }

    /// Console command handler for "cvarlist [wildcard]".
    pub fn list_f(&self, argc: usize, argv: &[&str]) {
        if argc != 1 && argc != 2 {
            com_printf("usage: cvarlist [wildcard]\n");
            return;
        }

        let pattern = if argc == 2 { Some(argv[0]) } else { None };
        self.list(pattern);
    }

    /// Register cvar-related console commands.
    /// Returns the command names that should be registered with the command system.
    pub fn init() -> Vec<&'static str> {
        vec!["set", "unset", "cvarlist"]
    }

    /// List cvars matching a pattern (Quake 3-style cvarlist).
    pub fn list(&self, pattern: Option<&str>) -> (usize, usize) {
        let wc = pattern.unwrap_or("*");
        let mut total = 0;
        let mut matching = 0;

        for var in &self.cvar_vars {
            total += 1;
            if wildcardfit(wc, &var.name) {
                matching += 1;
                let archive = if var.flags & CVAR_ARCHIVE != 0 { '*' } else { ' ' };
                let userinfo = if var.flags & CVAR_USERINFO != 0 { 'U' } else { ' ' };
                let serverinfo = if var.flags & CVAR_SERVERINFO != 0 { 'S' } else { ' ' };
                let noset = if var.flags & CVAR_NOSET != 0 {
                    '-'
                } else if var.flags & CVAR_LATCH != 0 {
                    'L'
                } else {
                    ' '
                };
                com_printf(&format!(
                    "{}{}{}{} {} \"{}\"\n",
                    archive, userinfo, serverinfo, noset, var.name, var.string
                ));
            }
        }

        com_printf(&format!("{} cvars, {} matching\n", total, matching));
        (total, matching)
    }
}

impl Default for CvarContext {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// Global singleton and free-function wrappers
// ============================================================

use std::sync::Mutex;

static CVAR_CTX: Mutex<Option<CvarContext>> = Mutex::new(None);

pub fn cvar_init() {
    let mut g = CVAR_CTX.lock().unwrap();
    *g = Some(CvarContext::new());
}

pub fn cvar_shutdown() {
    let mut g = CVAR_CTX.lock().unwrap();
    *g = None;
}

pub fn cvar_get(name: &str, value: &str, flags: i32) -> Option<usize> {
    CVAR_CTX.lock().unwrap().as_mut().and_then(|c| c.get(name, Some(value), flags))
}

pub fn cvar_set(name: &str, value: &str) {
    if let Some(ref mut c) = *CVAR_CTX.lock().unwrap() {
        c.set(name, value);
    }
}

pub fn cvar_set_value(name: &str, value: f32) {
    if let Some(ref mut c) = *CVAR_CTX.lock().unwrap() {
        c.set_value(name, value);
    }
}

pub fn cvar_force_set(name: &str, value: &str) {
    if let Some(ref mut c) = *CVAR_CTX.lock().unwrap() {
        c.force_set(name, value);
    }
}

pub fn cvar_variable_value(name: &str) -> f32 {
    CVAR_CTX.lock().unwrap().as_ref().map_or(0.0, |c| c.variable_value(name))
}

pub fn cvar_variable_string(name: &str) -> String {
    CVAR_CTX.lock().unwrap().as_ref().map_or(String::new(), |c| c.variable_string(name).to_string())
}

pub fn cvar_userinfo() -> String {
    CVAR_CTX.lock().unwrap().as_ref().map_or(String::new(), |c| c.userinfo())
}

pub fn cvar_serverinfo() -> String {
    CVAR_CTX.lock().unwrap().as_ref().map_or(String::new(), |c| c.serverinfo())
}

pub fn cvar_write_variables(f: &mut dyn std::io::Write) {
    if let Some(ref c) = *CVAR_CTX.lock().unwrap() {
        let _ = c.write_variables(f);
    }
}

pub fn cvar_write_address_book(f: &mut dyn std::io::Write) {
    if let Some(ref c) = *CVAR_CTX.lock().unwrap() {
        let _ = c.write_address_book(f);
    }
}

pub fn cvar_full_set(name: &str, value: &str, flags: i32) {
    if let Some(ref mut c) = *CVAR_CTX.lock().unwrap() {
        c.full_set(name, value, flags);
    }
}

pub fn cvar_get_latched_vars() {
    if let Some(ref mut c) = *CVAR_CTX.lock().unwrap() {
        c.get_latched_vars();
    }
}

/// Access the global CVAR_CTX with a closure. Returns None if not initialized.
pub fn with_cvar_ctx<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut CvarContext) -> R,
{
    let mut g = CVAR_CTX.lock().unwrap();
    g.as_mut().map(f)
}

/// Get a cvar's float value by handle (index). Returns 0.0 if invalid.
pub fn cvar_value_by_handle(handle: usize) -> f32 {
    CVAR_CTX.lock().unwrap().as_ref().map_or(0.0, |c| {
        c.cvar_vars.get(handle).map_or(0.0, |v| v.value)
    })
}

/// Check if a cvar has been modified, by handle (index). Returns false if invalid.
pub fn cvar_modified_by_handle(handle: usize) -> bool {
    CVAR_CTX.lock().unwrap().as_ref().is_some_and(|c| {
        c.cvar_vars.get(handle).is_some_and(|v| v.modified)
    })
}

/// Clear the modified flag on a cvar, by handle (index).
pub fn cvar_clear_modified_by_handle(handle: usize) {
    if let Some(ref mut c) = *CVAR_CTX.lock().unwrap() {
        if let Some(v) = c.cvar_vars.get_mut(handle) {
            v.modified = false;
        }
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cvar_get_and_find() {
        let mut ctx = CvarContext::new();
        ctx.get("test_var", Some("42"), 0);
        assert_eq!(ctx.variable_value("test_var"), 42.0);
        assert_eq!(ctx.variable_string("test_var"), "42");
    }

    #[test]
    fn test_cvar_set() {
        let mut ctx = CvarContext::new();
        ctx.get("test_var", Some("10"), 0);
        ctx.set("test_var", "20");
        assert_eq!(ctx.variable_value("test_var"), 20.0);
    }

    #[test]
    fn test_cvar_noset() {
        let mut ctx = CvarContext::new();
        ctx.get("test_var", Some("10"), CVAR_NOSET);
        ctx.set("test_var", "20"); // should be blocked
        assert_eq!(ctx.variable_value("test_var"), 10.0);
    }

    #[test]
    fn test_cvar_force_set() {
        let mut ctx = CvarContext::new();
        ctx.get("test_var", Some("10"), CVAR_NOSET);
        ctx.force_set("test_var", "20");
        assert_eq!(ctx.variable_value("test_var"), 20.0);
    }

    #[test]
    fn test_cvar_set_value() {
        let mut ctx = CvarContext::new();
        ctx.get("test_var", Some("0"), 0);
        ctx.set_value("test_var", 3.14);
        assert!((ctx.variable_value("test_var") - 3.14).abs() < 0.001);
    }

    #[test]
    fn test_cvar_complete() {
        let mut ctx = CvarContext::new();
        ctx.get("vk_mode", Some("3"), 0);
        ctx.get("vk_driver", Some("opengl32"), 0);
        assert_eq!(ctx.complete_variable("vk_m"), Some("vk_mode"));
        assert_eq!(ctx.complete_variable("xyz"), None);
    }

    #[test]
    fn test_cvar_not_found() {
        let ctx = CvarContext::new();
        assert_eq!(ctx.variable_value("nonexistent"), 0.0);
        assert_eq!(ctx.variable_string("nonexistent"), "");
    }

    #[test]
    fn test_cvar_get_creates_once() {
        let mut ctx = CvarContext::new();
        ctx.get("test", Some("1"), 0);
        ctx.get("test", Some("2"), 0); // should NOT change value
        assert_eq!(ctx.variable_string("test"), "1");
    }

    #[test]
    fn test_cvar_latch() {
        let mut ctx = CvarContext::new();
        ctx.get("game", Some("baseq2"), CVAR_LATCH);
        // With server running (server_state != 0), latched string is set
        ctx.set_with_server_state("game", "ctf", 1);
        assert_eq!(ctx.variable_string("game"), "baseq2"); // not changed yet
        assert_eq!(ctx.cvar_vars[0].latched_string.as_deref(), Some("ctf"));
        // Apply latched
        ctx.get_latched_vars();
        assert_eq!(ctx.variable_string("game"), "ctf");
    }

    #[test]
    fn test_cvar_info_validate() {
        let mut ctx = CvarContext::new();
        // Names with backslash should be rejected for info cvars
        let result = ctx.get("bad\\name", Some("value"), CVAR_USERINFO);
        assert!(result.is_none());
    }

    #[test]
    fn test_write_variables() {
        let mut ctx = CvarContext::new();
        ctx.get("archived_var", Some("hello"), CVAR_ARCHIVE);
        ctx.get("normal_var", Some("world"), 0);
        let mut buf = Vec::new();
        ctx.write_variables(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("set archived_var \"hello\""));
        assert!(!output.contains("normal_var"));
    }
}
