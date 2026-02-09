// cmd.rs — Quake script command processing module
// Converted from: myq2-original/qcommon/cmd.c

use crate::common::{com_printf, ComArgs};
use crate::q_shared::{MAX_STRING_CHARS, MAX_STRING_TOKENS};
use crate::wildcards::wildcardfit;

use std::collections::HashMap;

pub const MAX_ALIAS_NAME: usize = 32;
pub const ALIAS_LOOP_COUNT: i32 = 16;

/// Execution timing for Cbuf_ExecuteText.
pub const EXEC_NOW: i32 = 0;
pub const EXEC_INSERT: i32 = 1;
pub const EXEC_APPEND: i32 = 2;

/// A command alias.
#[derive(Clone)]
pub struct CmdAlias {
    pub name: String,
    pub value: String,
}

/// A registered command.
pub struct CmdFunction {
    pub name: String,
    /// If None, the command is forwarded to the server as clc_stringcmd.
    pub function: Option<Box<dyn Fn(&mut CmdContext) + Send>>,
}

/// Command buffer state.
pub struct CmdTextBuf {
    pub data: Vec<u8>,
    pub cursize: usize,
    pub maxsize: usize,
}

impl CmdTextBuf {
    pub fn new(maxsize: usize) -> Self {
        Self {
            data: vec![0u8; maxsize],
            cursize: 0,
            maxsize,
        }
    }

    pub fn clear(&mut self) {
        self.cursize = 0;
    }

    pub fn write(&mut self, src: &[u8]) {
        if self.cursize + src.len() > self.maxsize {
            return; // overflow
        }
        self.data[self.cursize..self.cursize + src.len()].copy_from_slice(src);
        self.cursize += src.len();
    }
}

/// Callback type for loading a file. Returns file contents or None.
/// Mirrors FS_LoadFile in the C code.
pub type FsLoadFileFn = Box<dyn Fn(&str) -> Option<Vec<u8>> + Send>;

/// Callback type for looking up a cvar's string value.
/// Mirrors Cvar_VariableString in the C code.
pub type CvarVariableStringFn = Box<dyn Fn(&str) -> String + Send>;

/// Callback type for handling cvar commands (e.g. "varname value" sets the cvar).
/// Returns true if handled.
pub type CvarCommandFn = Box<dyn Fn(&mut CmdContext) -> bool + Send>;

/// Callback for forwarding unknown commands to the server.
pub type ForwardToServerFn = Box<dyn Fn(&mut CmdContext) + Send>;

/// The full command system context, holding all state that was global in C.
pub struct CmdContext {
    // Command buffer
    pub cmd_text: CmdTextBuf,
    pub defer_text_buf: Vec<u8>,
    pub cmd_wait: bool,

    // Aliases
    pub cmd_alias: Vec<CmdAlias>,
    /// O(1) alias lookup by name (lowercase) -> index in cmd_alias
    cmd_alias_index: HashMap<String, usize>,
    pub alias_count: i32,

    // Tokenized command line
    pub cmd_argc: usize,
    pub cmd_argv: Vec<String>,
    pub cmd_args: String,

    // Registered commands
    pub cmd_functions: Vec<CmdFunction>,
    /// O(1) command lookup by name (lowercase) -> index in cmd_functions
    cmd_functions_index: HashMap<String, usize>,

    // External callbacks (set by the engine after init)
    pub fs_load_file: Option<FsLoadFileFn>,
    pub cvar_variable_string: Option<CvarVariableStringFn>,
    pub cvar_command: Option<CvarCommandFn>,
    pub forward_to_server: Option<ForwardToServerFn>,
}

impl CmdContext {
    pub fn new() -> Self {
        Self {
            cmd_text: CmdTextBuf::new(65536),
            defer_text_buf: vec![0u8; 65536],
            cmd_wait: false,
            cmd_alias: Vec::new(),
            cmd_alias_index: HashMap::new(),
            alias_count: 0,
            cmd_argc: 0,
            cmd_argv: Vec::new(),
            cmd_args: String::new(),
            cmd_functions: Vec::new(),
            cmd_functions_index: HashMap::new(),
            fs_load_file: None,
            cvar_variable_string: None,
            cvar_command: None,
            forward_to_server: None,
        }
    }

    // ========================================================
    // Command buffer operations (Cbuf_*)
    // ========================================================

    /// Add command text at the end of the buffer.
    pub fn cbuf_add_text(&mut self, text: &str) {
        let bytes = text.as_bytes();
        if self.cmd_text.cursize + bytes.len() >= self.cmd_text.maxsize {
            com_printf("Cbuf_AddText: overflow\n");
            return;
        }
        self.cmd_text.write(bytes);
    }

    /// Insert command text immediately after the current command.
    pub fn cbuf_insert_text(&mut self, text: &str) {
        // Copy off any commands still remaining in the exec buffer
        let templen = self.cmd_text.cursize;
        let temp = if templen > 0 {
            let mut t = vec![0u8; templen];
            t.copy_from_slice(&self.cmd_text.data[..templen]);
            self.cmd_text.clear();
            Some(t)
        } else {
            None
        };

        // Add the entire text of the file
        self.cbuf_add_text(text);

        // Add the copied off data
        if let Some(t) = temp {
            self.cmd_text.write(&t);
        }
    }

    /// Copy command buffer to defer buffer.
    pub fn cbuf_copy_to_defer(&mut self) {
        let cursize = self.cmd_text.cursize;
        self.defer_text_buf[..cursize].copy_from_slice(&self.cmd_text.data[..cursize]);
        self.defer_text_buf[cursize] = 0;
        self.cmd_text.cursize = 0;
    }

    /// Insert deferred commands back into the command buffer.
    pub fn cbuf_insert_from_defer(&mut self) {
        // Find the null terminator in defer_text_buf
        let len = self.defer_text_buf.iter().position(|&b| b == 0).unwrap_or(0);
        if len > 0 {
            let text = String::from_utf8_lossy(&self.defer_text_buf[..len]).to_string();
            self.cbuf_insert_text(&text);
        }
        self.defer_text_buf[0] = 0;
    }

    /// Adds command line parameters as script statements.
    /// Commands lead with a +, and continue until another +.
    /// Set commands are added early, before client and server initialize.
    /// If `clear` is true, the consumed argv entries are cleared.
    pub fn cbuf_add_early_commands(&mut self, args: &mut ComArgs, clear: bool) {
        let argc = args.com_argc();
        let mut i = 0;
        while i < argc {
            let s = args.com_argv(i).to_string();
            if s != "+set" {
                i += 1;
                continue;
            }
            let arg1 = args.com_argv(i + 1).to_string();
            let arg2 = args.com_argv(i + 2).to_string();
            let text = format!("set {} {}\n", arg1, arg2);
            self.cbuf_add_text(&text);
            if clear {
                args.com_clear_argv(i);
                args.com_clear_argv(i + 1);
                args.com_clear_argv(i + 2);
            }
            i += 3;
        }
    }

    /// Adds command line parameters as script statements.
    /// Commands lead with a + and continue until another + or -.
    /// Returns true if any late commands were added.
    pub fn cbuf_add_late_commands(&mut self, args: &ComArgs) -> bool {
        // Build the combined string to parse from
        let argc = args.com_argc();
        let mut s = 0usize;
        for i in 1..argc {
            s += args.com_argv(i).len() + 1;
        }
        if s == 0 {
            return false;
        }

        let mut text = String::with_capacity(s + 1);
        for i in 1..argc {
            text.push_str(args.com_argv(i));
            if i != argc - 1 {
                text.push(' ');
            }
        }

        // Pull out the + commands
        let text_bytes = text.as_bytes();
        let text_len = text_bytes.len();
        let mut build = String::new();

        let mut i = 0;
        while i < text_len.saturating_sub(1) {
            if text_bytes[i] == b'+' {
                i += 1;

                let mut j = i;
                while j < text_len
                    && text_bytes[j] != b'+'
                    && text_bytes[j] != b'-'
                {
                    j += 1;
                }

                build.push_str(&text[i..j]);
                build.push('\n');
                i = j;
            } else {
                i += 1;
            }
        }

        let ret = !build.is_empty();
        if ret {
            self.cbuf_add_text(&build);
        }
        ret
    }

    /// Execute text based on timing: EXEC_NOW, EXEC_INSERT, or EXEC_APPEND.
    pub fn cbuf_execute_text(&mut self, exec_when: i32, text: &str) {
        match exec_when {
            EXEC_NOW => self.cmd_execute_string(text),
            EXEC_INSERT => self.cbuf_insert_text(text),
            EXEC_APPEND => self.cbuf_add_text(text),
            _ => panic!("Cbuf_ExecuteText: bad exec_when"),
        }
    }

    /// Execute all commands in the buffer.
    pub fn cbuf_execute(&mut self) {
        self.alias_count = 0;

        while self.cmd_text.cursize > 0 {
            // Find a \n or ; line break
            let mut quotes = 0;
            let mut i = 0;
            while i < self.cmd_text.cursize {
                let ch = self.cmd_text.data[i];
                if ch == b'"' {
                    quotes += 1;
                }
                if (quotes & 1) == 0 && ch == b';' {
                    break;
                }
                if ch == b'\n' {
                    break;
                }
                i += 1;
            }

            // Extract the line
            let line = String::from_utf8_lossy(&self.cmd_text.data[..i]).to_string();

            // Delete the text from the command buffer
            if i == self.cmd_text.cursize {
                self.cmd_text.cursize = 0;
            } else {
                let skip = i + 1;
                self.cmd_text.cursize -= skip;
                self.cmd_text.data.copy_within(skip..skip + self.cmd_text.cursize, 0);
            }

            // Execute the command line
            self.cmd_execute_string(&line);

            if self.cmd_wait {
                self.cmd_wait = false;
                break;
            }
        }
    }

    // ========================================================
    // Macro expansion
    // ========================================================

    /// Expand $macros in a command string using cvar values.
    /// Returns None if the string is invalid (too long, unmatched quote, expansion loop).
    pub fn cmd_macro_expand_string(&self, text: &str) -> Option<String> {
        let mut scan = text.to_string();

        if scan.len() >= MAX_STRING_CHARS {
            com_printf(&format!("Line exceeded {} chars, discarded.\n", MAX_STRING_CHARS));
            return None;
        }

        let mut count = 0;
        let mut i = 0;

        loop {
            if i >= scan.len() {
                break;
            }

            // Check for unbalanced quotes — skip inside quotes
            let scan_bytes = scan.as_bytes();
            let mut inquote = false;
            let mut found_dollar = false;
            let mut dollar_pos = 0;

            // Re-scan from beginning to find next $ outside quotes
            inquote = false;
            found_dollar = false;
            for (ci, &b) in scan_bytes.iter().enumerate() {
                if b == b'"' {
                    inquote = !inquote;
                }
                if !inquote && b == b'$' && ci >= i {
                    dollar_pos = ci;
                    found_dollar = true;
                    break;
                }
            }

            if !found_dollar {
                break;
            }

            // Parse the token after $
            let after_dollar = &scan[dollar_pos + 1..];
            let (token, token_end) = com_parse_inline(after_dollar.as_bytes(), 0);
            if token.is_empty() {
                i = dollar_pos + 1;
                continue;
            }

            // Look up cvar value
            let value = if let Some(ref cvar_fn) = self.cvar_variable_string {
                cvar_fn(&token)
            } else {
                String::new()
            };

            let total_consumed = dollar_pos + 1 + token_end;
            let new_len = scan.len() - (total_consumed - dollar_pos) + value.len();
            if new_len >= MAX_STRING_CHARS {
                com_printf(&format!("Expanded line exceeded {} chars, discarded.\n", MAX_STRING_CHARS));
                return None;
            }

            // Replace $token with value
            let mut new_scan = String::with_capacity(new_len);
            new_scan.push_str(&scan[..dollar_pos]);
            new_scan.push_str(&value);
            new_scan.push_str(&scan[total_consumed..]);

            scan = new_scan;
            // Don't advance i — re-check from the same position in case of nested expansion
            count += 1;
            if count == 100 {
                com_printf("Macro expansion loop, discarded.\n");
                return None;
            }
        }

        // Check for unmatched quotes
        let quote_count = scan.bytes().filter(|&b| b == b'"').count();
        if quote_count % 2 != 0 {
            com_printf("Line has unmatched quote, discarded.\n");
            return None;
        }

        Some(scan)
    }

    // ========================================================
    // Command tokenization
    // ========================================================

    /// Parse the given string into command line tokens.
    /// $Cvars will be expanded unless they are in a quoted token.
    pub fn cmd_tokenize_string(&mut self, text: &str, macro_expand: bool) {
        self.cmd_argc = 0;
        self.cmd_argv.clear();
        self.cmd_args.clear();

        // Macro expand the text
        let expanded;
        let text = if macro_expand {
            if let Some(s) = self.cmd_macro_expand_string(text) {
                expanded = s;
                expanded.as_str()
            } else {
                return;
            }
        } else {
            text
        };

        let bytes = text.as_bytes();
        let mut pos = 0;

        loop {
            // Skip whitespace up to a \n
            while pos < bytes.len() && bytes[pos] <= b' ' && bytes[pos] != b'\n' {
                pos += 1;
            }

            if pos >= bytes.len() {
                return;
            }

            if bytes[pos] == b'\n' {
                break;
            }

            // Set cmd_args to everything after the first arg
            if self.cmd_argc == 1 {
                let args_text = String::from_utf8_lossy(&bytes[pos..]).to_string();
                self.cmd_args = args_text.trim_end().to_string();
            }

            // Parse a token
            let (token, new_pos) = com_parse_inline(bytes, pos);
            if new_pos == pos && token.is_empty() {
                return;
            }
            pos = new_pos;

            if self.cmd_argc < MAX_STRING_TOKENS {
                self.cmd_argv.push(token);
                self.cmd_argc += 1;
            }
        }
    }

    // ========================================================
    // Command registration
    // ========================================================

    /// Register a new command. O(1) lookup via HashMap.
    pub fn cmd_add_command(&mut self, name: &str, function: Option<Box<dyn Fn(&mut CmdContext) + Send>>) {
        let key = name.to_ascii_lowercase();

        // Check if command already exists using O(1) HashMap lookup
        if self.cmd_functions_index.contains_key(&key) {
            com_printf(&format!("Cmd_AddCommand: {} already defined\n", name));
            return;
        }

        let idx = self.cmd_functions.len();
        self.cmd_functions.push(CmdFunction {
            name: name.to_string(),
            function,
        });
        self.cmd_functions_index.insert(key, idx);
    }

    /// Remove a command by name. O(1) lookup via HashMap.
    pub fn cmd_remove_command(&mut self, name: &str) {
        let key = name.to_ascii_lowercase();

        if let Some(&idx) = self.cmd_functions_index.get(&key) {
            // Remove from vec and update HashMap
            self.cmd_functions.remove(idx);
            self.cmd_functions_index.remove(&key);

            // Update indices for all commands after the removed one
            for (k, v) in self.cmd_functions_index.iter_mut() {
                if *v > idx {
                    *v -= 1;
                }
            }
        } else {
            com_printf(&format!("Cmd_RemoveCommand: {} not added\n", name));
        }
    }

    /// Check if a command exists. O(1) lookup via HashMap.
    pub fn cmd_exists(&self, name: &str) -> bool {
        self.cmd_functions_index.contains_key(&name.to_ascii_lowercase())
    }

    /// Get the number of arguments.
    pub fn cmd_argc(&self) -> usize {
        self.cmd_argc
    }

    /// Get argument by index. Returns empty string if out of range.
    pub fn cmd_argv(&self, arg: usize) -> &str {
        if arg >= self.cmd_argc {
            ""
        } else {
            &self.cmd_argv[arg]
        }
    }

    /// Get all arguments after the first as a single string.
    pub fn cmd_args(&self) -> &str {
        &self.cmd_args
    }

    /// Attempt to match a partial command name for auto-completion.
    pub fn cmd_complete_command(&self, partial: &str) -> Option<&str> {
        if partial.is_empty() {
            return None;
        }

        // Check for exact match
        for cmd in &self.cmd_functions {
            if cmd.name == partial {
                return Some(&cmd.name);
            }
        }
        for alias in &self.cmd_alias {
            if alias.name == partial {
                return Some(&alias.name);
            }
        }

        // Check for partial match
        for cmd in &self.cmd_functions {
            if cmd.name.starts_with(partial) {
                return Some(&cmd.name);
            }
        }
        for alias in &self.cmd_alias {
            if alias.name.starts_with(partial) {
                return Some(&alias.name);
            }
        }

        None
    }

    /// Get all commands matching a prefix (for multi-completion).
    pub fn complete_all_commands(&self, partial: &str) -> Vec<&str> {
        self.cmd_functions
            .iter()
            .filter(|cmd| cmd.name.starts_with(partial))
            .map(|cmd| cmd.name.as_str())
            .collect()
    }

    /// Get all aliases matching a prefix (for multi-completion).
    pub fn complete_all_aliases(&self, partial: &str) -> Vec<&str> {
        self.cmd_alias
            .iter()
            .filter(|a| a.name.starts_with(partial))
            .map(|a| a.name.as_str())
            .collect()
    }

    // ========================================================
    // Command execution
    // ========================================================

    /// Execute a single command string.
    /// A complete command line has been parsed, so try to execute it.
    /// Uses O(1) HashMap lookup for commands and aliases.
    pub fn cmd_execute_string(&mut self, text: &str) {
        self.cmd_tokenize_string(text, true);

        if self.cmd_argc == 0 {
            return;
        }

        let cmd_name = self.cmd_argv[0].clone();
        let key = cmd_name.to_ascii_lowercase();

        // Check registered commands using O(1) HashMap lookup
        if let Some(&idx) = self.cmd_functions_index.get(&key) {
            // Take the function pointer out temporarily
            let func = self.cmd_functions[idx].function.take();
            match func {
                Some(f) => {
                    // Call the function with mutable access to self
                    f(self);
                    // Put the function back
                    self.cmd_functions[idx].function = Some(f);
                }
                None => {
                    // No function — forward to server command
                    let forwarded = format!("cmd {}", text);
                    self.cmd_execute_string(&forwarded);
                }
            }
            return;
        }

        // Check aliases using O(1) HashMap lookup
        if let Some(&idx) = self.cmd_alias_index.get(&key) {
            let alias_value = self.cmd_alias[idx].value.clone();
            self.alias_count += 1;
            if self.alias_count == ALIAS_LOOP_COUNT {
                com_printf("ALIAS_LOOP_COUNT\n");
                return;
            }
            let insert = format!("{}\n", alias_value);
            self.cbuf_insert_text(&insert);
            return;
        }

        // Check cvars
        if let Some(cvar_cmd) = self.cvar_command.take() {
            let handled = cvar_cmd(self);
            self.cvar_command = Some(cvar_cmd);
            if handled {
                return;
            }
        }

        // Forward to server
        if let Some(fwd) = self.forward_to_server.take() {
            fwd(self);
            self.forward_to_server = Some(fwd);
        }
    }

    // ========================================================
    // Built-in command handlers
    // ========================================================

    /// Cmd_Wait_f — causes execution of the remainder of the command buffer
    /// to be delayed until the next frame.
    pub fn cmd_wait_f(&mut self) {
        self.cmd_wait = true;
    }

    /// Cmd_Echo_f — just prints the rest of the line to the console.
    pub fn cmd_echo_f(&self) {
        for i in 1..self.cmd_argc {
            com_printf(&format!("{} ", self.cmd_argv(i)));
        }
        com_printf("\n");
    }

    /// Cmd_Exec_f — execute a script file.
    pub fn cmd_exec_f(&mut self) {
        if self.cmd_argc != 2 {
            com_printf("exec <filename> : execute a script file\n");
            return;
        }

        let mut filename = self.cmd_argv(1).to_string();
        if !wildcardfit("*.cfg", &filename) {
            filename.push_str(".cfg");
        }

        // Load the file via the filesystem callback
        let file_data = if let Some(ref load_fn) = self.fs_load_file {
            load_fn(&filename)
        } else {
            com_printf(&format!("couldn't exec {} (no filesystem)\n", filename));
            return;
        };

        match file_data {
            Some(data) => {
                com_printf(&format!("execing {}\n", filename));
                // The file doesn't have a trailing 0, so convert to string
                let text = String::from_utf8_lossy(&data).to_string();
                self.cbuf_insert_text(&text);
            }
            None => {
                com_printf(&format!("couldn't exec {}\n", filename));
            }
        }
    }

    /// Cmd_Alias_f — creates a new command that executes a command string.
    pub fn cmd_alias_f(&mut self) {
        let c = self.cmd_argc;

        if c <= 2 {
            // List aliases
            self.cmd_alias_list_f_inner();
            return;
        }

        let name = self.cmd_argv(1).to_string();

        if name.len() >= MAX_ALIAS_NAME {
            com_printf("Alias name is too long\n");
            return;
        }

        // Build the command string from remaining args
        let mut cmd = String::new();
        for i in 2..c {
            cmd.push_str(self.cmd_argv(i));
            if i != c - 1 {
                cmd.push(' ');
            }
        }

        self.cmd_alias_set(&name, &cmd);
    }

    /// Cmd_AliasList_f — lists all aliases, optionally filtered by wildcard.
    pub fn cmd_alias_list_f(&self) {
        self.cmd_alias_list_f_inner();
    }

    fn cmd_alias_list_f_inner(&self) {
        let pattern = if self.cmd_argc == 2 {
            Some(self.cmd_argv(1))
        } else {
            None
        };
        self.cmd_alias_list(pattern);
    }

    /// Cmd_List_f — lists all registered commands.
    pub fn cmd_list_f(&self) {
        let pattern = if self.cmd_argc == 2 {
            Some(self.cmd_argv(1))
        } else {
            None
        };
        self.cmd_list(pattern);
    }

    // ========================================================
    // Alias management
    // ========================================================

    /// Create or update an alias. O(1) lookup via HashMap.
    pub fn cmd_alias_set(&mut self, name: &str, value: &str) {
        if name.len() >= MAX_ALIAS_NAME {
            com_printf("Alias name is too long\n");
            return;
        }

        let key = name.to_ascii_lowercase();

        // Update existing alias if found using O(1) HashMap lookup
        if let Some(&idx) = self.cmd_alias_index.get(&key) {
            self.cmd_alias[idx].value = value.to_string();
            return;
        }

        // Create new alias
        let idx = self.cmd_alias.len();
        self.cmd_alias.push(CmdAlias {
            name: name.to_string(),
            value: value.to_string(),
        });
        self.cmd_alias_index.insert(key, idx);
    }

    /// Write all aliases to a writer (for config saving).
    pub fn cmd_write_aliases(&self, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        for alias in &self.cmd_alias {
            writeln!(writer, "alias {} \"{}\"", alias.name, alias.value)?;
        }
        Ok(())
    }

    /// List aliases matching an optional wildcard pattern.
    pub fn cmd_alias_list(&self, pattern: Option<&str>) -> (usize, usize) {
        let pat = pattern.unwrap_or("*");
        let mut total = 0;
        let mut matching = 0;

        for alias in &self.cmd_alias {
            total += 1;
            if wildcardfit(pat, &alias.name) {
                com_printf(&format!("{} \"{}\"\n", alias.name, alias.value));
                matching += 1;
            }
        }

        com_printf(&format!("{} aliases, {} matching\n", total, matching));
        (total, matching)
    }

    /// List commands matching an optional wildcard pattern.
    pub fn cmd_list(&self, pattern: Option<&str>) -> (usize, usize) {
        let pat = pattern.unwrap_or("*");
        let mut total = 0;
        let mut matching = 0;

        for cmd in &self.cmd_functions {
            total += 1;
            if wildcardfit(pat, &cmd.name) {
                com_printf(&format!("{}\n", cmd.name));
                matching += 1;
            }
        }

        com_printf(&format!("{} commands, {} matching\n", total, matching));
        (total, matching)
    }

    // ========================================================
    // Initialization
    // ========================================================

    /// Register built-in commands: cmdlist, exec, echo, alias, aliaslist, wait.
    pub fn cmd_init(&mut self) {
        self.cmd_add_command("cmdlist", Some(Box::new(|ctx: &mut CmdContext| {
            ctx.cmd_list_f();
        })));
        self.cmd_add_command("exec", Some(Box::new(|ctx: &mut CmdContext| {
            ctx.cmd_exec_f();
        })));
        self.cmd_add_command("echo", Some(Box::new(|ctx: &mut CmdContext| {
            ctx.cmd_echo_f();
        })));
        self.cmd_add_command("alias", Some(Box::new(|ctx: &mut CmdContext| {
            ctx.cmd_alias_f();
        })));
        self.cmd_add_command("aliaslist", Some(Box::new(|ctx: &mut CmdContext| {
            ctx.cmd_alias_list_f();
        })));
        self.cmd_add_command("wait", Some(Box::new(|ctx: &mut CmdContext| {
            ctx.cmd_wait_f();
        })));
    }
}

impl Default for CmdContext {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// Inline token parser (simplified COM_Parse for tokenization)
// ============================================================

/// Parse a single token from the byte slice starting at `pos`.
/// Returns (token, new_pos).
fn com_parse_inline(data: &[u8], mut pos: usize) -> (String, usize) {
    // Skip whitespace
    while pos < data.len() && data[pos] <= b' ' {
        if data[pos] == b'\n' {
            return (String::new(), pos);
        }
        pos += 1;
    }

    if pos >= data.len() {
        return (String::new(), pos);
    }

    let mut token = String::new();

    // Handle quoted strings
    if data[pos] == b'"' {
        pos += 1; // skip opening quote
        while pos < data.len() && data[pos] != b'"' {
            token.push(data[pos] as char);
            pos += 1;
        }
        if pos < data.len() {
            pos += 1; // skip closing quote
        }
        return (token, pos);
    }

    // Regular token
    while pos < data.len() && data[pos] > b' ' {
        token.push(data[pos] as char);
        pos += 1;
    }

    (token, pos)
}

// ============================================================
// Global singleton and free-function wrappers
// ============================================================

use std::sync::Mutex;

static CMD_CTX: Mutex<Option<CmdContext>> = Mutex::new(None);

pub fn cmd_init() {
    let mut g = CMD_CTX.lock().unwrap();
    let mut ctx = CmdContext::new();
    ctx.cmd_init();
    *g = Some(ctx);
}

pub fn cmd_shutdown() {
    let mut g = CMD_CTX.lock().unwrap();
    *g = None;
}

pub fn cmd_argc() -> usize {
    CMD_CTX.lock().unwrap().as_ref().map_or(0, |c| c.cmd_argc())
}

pub fn cmd_argv(arg: usize) -> String {
    CMD_CTX.lock().unwrap().as_ref().map_or(String::new(), |c| c.cmd_argv(arg).to_string())
}

pub fn cmd_args() -> String {
    CMD_CTX.lock().unwrap().as_ref().map_or(String::new(), |c| c.cmd_args().to_string())
}

pub fn cmd_tokenize_string(text: &str, macro_expand: bool) {
    if let Some(ref mut c) = *CMD_CTX.lock().unwrap() {
        c.cmd_tokenize_string(text, macro_expand);
    }
}

pub fn cmd_add_command(name: &str, function: Option<Box<dyn Fn(&mut CmdContext) + Send>>) {
    if let Some(ref mut c) = *CMD_CTX.lock().unwrap() {
        c.cmd_add_command(name, function);
    }
}

/// Convenience wrapper: register a simple `fn()` command (always `Some`).
/// Adapts `fn()` to the `Option<Box<dyn Fn(&mut CmdContext) + Send>>` expected
/// by the core `cmd_add_command`.
pub fn cmd_add_command_simple(name: &str, func: fn()) {
    let boxed: Option<Box<dyn Fn(&mut CmdContext) + Send>> =
        Some(Box::new(move |_ctx: &mut CmdContext| func()));
    cmd_add_command(name, boxed);
}

/// Convenience wrapper: register an `Option<fn()>` command.
/// If `func` is `None`, the command is forwarded to the server as clc_stringcmd.
pub fn cmd_add_command_optional(name: &str, func: Option<fn()>) {
    let boxed = func.map(|f| {
        Box::new(move |_ctx: &mut CmdContext| f())
            as Box<dyn Fn(&mut CmdContext) + Send>
    });
    cmd_add_command(name, boxed);
}

pub fn cmd_remove_command(name: &str) {
    if let Some(ref mut c) = *CMD_CTX.lock().unwrap() {
        c.cmd_remove_command(name);
    }
}

pub fn cbuf_add_text(text: &str) {
    if let Some(ref mut c) = *CMD_CTX.lock().unwrap() {
        c.cbuf_add_text(text);
    }
}

pub fn cbuf_execute() {
    if let Some(ref mut c) = *CMD_CTX.lock().unwrap() {
        c.cbuf_execute();
    }
}

pub fn cmd_write_aliases(f: &mut dyn std::io::Write) {
    if let Some(ref c) = *CMD_CTX.lock().unwrap() {
        let _ = c.cmd_write_aliases(f);
    }
}

pub fn cmd_execute_string(text: &str) {
    if let Some(ref mut c) = *CMD_CTX.lock().unwrap() {
        c.cmd_execute_string(text);
    }
}

pub fn cbuf_copy_to_defer() {
    if let Some(ref mut c) = *CMD_CTX.lock().unwrap() {
        c.cbuf_copy_to_defer();
    }
}

pub fn cbuf_insert_from_defer() {
    if let Some(ref mut c) = *CMD_CTX.lock().unwrap() {
        c.cbuf_insert_from_defer();
    }
}

/// Access the global CMD_CTX with a closure. Returns None if not initialized.
pub fn with_cmd_ctx<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut CmdContext) -> R,
{
    let mut g = CMD_CTX.lock().unwrap();
    g.as_mut().map(f)
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cbuf_add_text() {
        let mut ctx = CmdContext::new();
        ctx.cbuf_add_text("echo hello\n");
        assert_eq!(ctx.cmd_text.cursize, 11);
    }

    #[test]
    fn test_cbuf_insert_text() {
        let mut ctx = CmdContext::new();
        ctx.cbuf_add_text("second\n");
        ctx.cbuf_insert_text("first\n");
        let text = String::from_utf8_lossy(&ctx.cmd_text.data[..ctx.cmd_text.cursize]).to_string();
        assert_eq!(text, "first\nsecond\n");
    }

    #[test]
    fn test_tokenize() {
        let mut ctx = CmdContext::new();
        ctx.cmd_tokenize_string("set name \"John Doe\"", false);
        assert_eq!(ctx.cmd_argc(), 3);
        assert_eq!(ctx.cmd_argv(0), "set");
        assert_eq!(ctx.cmd_argv(1), "name");
        assert_eq!(ctx.cmd_argv(2), "John Doe");
    }

    #[test]
    fn test_cmd_add_remove() {
        let mut ctx = CmdContext::new();
        ctx.cmd_add_command("test", None);
        assert!(ctx.cmd_exists("test"));
        ctx.cmd_remove_command("test");
        assert!(!ctx.cmd_exists("test"));
    }

    #[test]
    fn test_cmd_complete() {
        let mut ctx = CmdContext::new();
        ctx.cmd_add_command("echo", None);
        ctx.cmd_add_command("exec", None);
        assert_eq!(ctx.cmd_complete_command("echo"), Some("echo"));
        assert_eq!(ctx.cmd_complete_command("ec"), Some("echo"));
        assert_eq!(ctx.cmd_complete_command("xyz"), None);
    }

    #[test]
    fn test_alias() {
        let mut ctx = CmdContext::new();
        ctx.cmd_alias_set("test", "echo hello");
        assert_eq!(ctx.cmd_alias.len(), 1);
        assert_eq!(ctx.cmd_alias[0].value, "echo hello");

        // Update existing
        ctx.cmd_alias_set("test", "echo world");
        assert_eq!(ctx.cmd_alias.len(), 1);
        assert_eq!(ctx.cmd_alias[0].value, "echo world");
    }

    #[test]
    fn test_cmd_wait() {
        let mut ctx = CmdContext::new();
        ctx.cmd_init();
        ctx.cbuf_add_text("echo first\nwait\necho second\n");
        ctx.cbuf_execute();
        // After execute, the buffer should still have "echo second\n" left
        // because wait causes early exit
        assert!(ctx.cmd_text.cursize > 0);
    }

    #[test]
    fn test_cmd_execute_callback() {
        use std::sync::{Arc, Mutex};
        let called = Arc::new(Mutex::new(false));
        let called_clone = called.clone();

        let mut ctx = CmdContext::new();
        ctx.cmd_add_command("mytest", Some(Box::new(move |_ctx: &mut CmdContext| {
            *called_clone.lock().unwrap() = true;
        })));

        ctx.cmd_execute_string("mytest");
        assert!(*called.lock().unwrap());
    }

    #[test]
    fn test_cbuf_add_early_commands() {
        let mut ctx = CmdContext::new();
        let mut args = ComArgs::new();
        args.init(&[
            "myq2".to_string(),
            "+set".to_string(),
            "name".to_string(),
            "player".to_string(),
        ]);
        ctx.cbuf_add_early_commands(&mut args, true);
        let text = String::from_utf8_lossy(&ctx.cmd_text.data[..ctx.cmd_text.cursize]).to_string();
        assert_eq!(text, "set name player\n");
        // Args should be cleared
        assert_eq!(args.com_argv(1), "");
    }

    #[test]
    fn test_cbuf_add_late_commands() {
        let mut ctx = CmdContext::new();
        let args = {
            let mut a = ComArgs::new();
            a.init(&[
                "myq2".to_string(),
                "+map".to_string(),
                "amlev1".to_string(),
            ]);
            a
        };
        let ret = ctx.cbuf_add_late_commands(&args);
        assert!(ret);
        let text = String::from_utf8_lossy(&ctx.cmd_text.data[..ctx.cmd_text.cursize]).to_string();
        assert_eq!(text, "map amlev1\n");
    }

    #[test]
    fn test_macro_expand() {
        let mut ctx = CmdContext::new();
        ctx.cvar_variable_string = Some(Box::new(|name: &str| -> String {
            if name == "name" {
                "player".to_string()
            } else {
                String::new()
            }
        }));
        let result = ctx.cmd_macro_expand_string("echo $name");
        assert_eq!(result, Some("echo player".to_string()));
    }

    #[test]
    fn test_cbuf_copy_defer() {
        let mut ctx = CmdContext::new();
        ctx.cbuf_add_text("echo test\n");
        ctx.cbuf_copy_to_defer();
        assert_eq!(ctx.cmd_text.cursize, 0);
        ctx.cbuf_insert_from_defer();
        let text = String::from_utf8_lossy(&ctx.cmd_text.data[..ctx.cmd_text.cursize]).to_string();
        assert_eq!(text, "echo test\n");
    }

    #[test]
    fn test_cmd_write_aliases() {
        let mut ctx = CmdContext::new();
        ctx.cmd_alias_set("run", "echo running");
        ctx.cmd_alias_set("stop", "echo stopped");
        let mut buf = Vec::new();
        ctx.cmd_write_aliases(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("alias run \"echo running\""));
        assert!(output.contains("alias stop \"echo stopped\""));
    }
}
