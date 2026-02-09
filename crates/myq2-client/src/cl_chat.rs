// cl_chat.rs -- Chat enhancements (R1Q2/Q2Pro feature)
//
// Features:
// - Word filter: Load baseq2/filter.txt, replace filtered words
// - Ignore list: Ignore messages from specific players
// - Chat logging: Log all chat to daily files

use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

use myq2_common::common::com_printf;
use myq2_common::files::fs_gamedir;

/// Maximum length of a player name
const MAX_NAME_LEN: usize = 16;

/// Chat filter and ignore state
pub struct ChatState {
    /// Words to filter from chat messages
    pub filter_words: Vec<String>,
    /// Players to ignore (by name, lowercase)
    pub ignored_players: HashSet<String>,
    /// Whether chat filtering is enabled
    pub filter_enabled: bool,
    /// Whether chat logging is enabled
    pub log_enabled: bool,
    /// Current log file (if open)
    log_file: Option<File>,
    /// Current log date (YYYY-MM-DD) to detect day changes
    log_date: String,
}

impl ChatState {
    pub fn new() -> Self {
        Self {
            filter_words: Vec::new(),
            ignored_players: HashSet::new(),
            filter_enabled: true,
            log_enabled: false,
            log_file: None,
            log_date: String::new(),
        }
    }

    /// Load the word filter from filter.txt
    pub fn load_filter(&mut self, gamedir: &str) {
        self.filter_words.clear();

        let path = format!("{}/filter.txt", gamedir);
        let file = match File::open(&path) {
            Ok(f) => f,
            Err(_) => return, // No filter file, that's fine
        };

        let reader = BufReader::new(file);
        for line in reader.lines().map_while(Result::ok) {
            let word = line.trim().to_lowercase();
            if !word.is_empty() && !word.starts_with("//") {
                self.filter_words.push(word);
            }
        }

        if !self.filter_words.is_empty() {
            com_printf(&format!("Loaded {} filter words\n", self.filter_words.len()));
        }
    }

    /// Load the ignore list from ignore.txt
    pub fn load_ignore_list(&mut self, gamedir: &str) {
        self.ignored_players.clear();

        let path = format!("{}/ignore.txt", gamedir);
        let file = match File::open(&path) {
            Ok(f) => f,
            Err(_) => return, // No ignore file, that's fine
        };

        let reader = BufReader::new(file);
        for line in reader.lines().map_while(Result::ok) {
            let name = line.trim().to_lowercase();
            if !name.is_empty() && !name.starts_with("//") {
                self.ignored_players.insert(name);
            }
        }

        if !self.ignored_players.is_empty() {
            com_printf(&format!("Loaded {} ignored players\n", self.ignored_players.len()));
        }
    }

    /// Save the ignore list to ignore.txt
    pub fn save_ignore_list(&self, gamedir: &str) -> Result<(), String> {
        let path = format!("{}/ignore.txt", gamedir);

        let mut file = File::create(&path)
            .map_err(|e| format!("Failed to create ignore.txt: {}", e))?;

        writeln!(file, "// Ignored players list")
            .map_err(|e| format!("Failed to write: {}", e))?;
        writeln!(file, "// One player name per line")
            .map_err(|e| format!("Failed to write: {}", e))?;

        for name in &self.ignored_players {
            writeln!(file, "{}", name)
                .map_err(|e| format!("Failed to write: {}", e))?;
        }

        Ok(())
    }

    /// Add a player to the ignore list
    pub fn ignore_player(&mut self, name: &str, gamedir: &str) -> bool {
        let name_lower = name.to_lowercase();
        if name_lower.len() > MAX_NAME_LEN {
            return false;
        }

        if self.ignored_players.insert(name_lower.clone()) {
            // Save the updated list
            if let Err(e) = self.save_ignore_list(gamedir) {
                com_printf(&format!("Warning: {}\n", e));
            }
            true
        } else {
            false // Already ignored
        }
    }

    /// Remove a player from the ignore list
    pub fn unignore_player(&mut self, name: &str, gamedir: &str) -> bool {
        let name_lower = name.to_lowercase();

        if self.ignored_players.remove(&name_lower) {
            // Save the updated list
            if let Err(e) = self.save_ignore_list(gamedir) {
                com_printf(&format!("Warning: {}\n", e));
            }
            true
        } else {
            false // Wasn't ignored
        }
    }

    /// Check if a player is ignored
    pub fn is_ignored(&self, name: &str) -> bool {
        self.ignored_players.contains(&name.to_lowercase())
    }

    /// Filter a chat message, replacing filtered words with asterisks
    pub fn filter_message(&self, message: &str) -> String {
        if !self.filter_enabled || self.filter_words.is_empty() {
            return message.to_string();
        }

        let mut result = message.to_string();
        let lower = message.to_lowercase();

        for word in &self.filter_words {
            if let Some(pos) = lower.find(word) {
                // Replace the word with asterisks
                let replacement = "*".repeat(word.len());
                // Need to handle case-insensitive replacement
                let end = pos + word.len();
                result = format!("{}{}{}", &result[..pos], replacement, &result[end..]);
            }
        }

        result
    }

    /// Log a chat message to the daily log file
    pub fn log_message(&mut self, sender: &str, message: &str, gamedir: &str) {
        if !self.log_enabled {
            return;
        }

        // Get current date
        let now = chrono_lite_date();

        // Check if we need a new log file
        if self.log_date != now || self.log_file.is_none() {
            self.log_date = now.clone();
            self.log_file = None;

            // Create logs directory if needed
            let logs_dir = format!("{}/logs", gamedir);
            let _ = std::fs::create_dir_all(&logs_dir);

            // Open new log file
            let log_path = format!("{}/chat-{}.log", logs_dir, now);
            match OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
            {
                Ok(f) => self.log_file = Some(f),
                Err(e) => {
                    com_printf(&format!("Failed to open chat log: {}\n", e));
                    return;
                }
            }
        }

        // Write the log entry
        if let Some(ref mut file) = self.log_file {
            let timestamp = chrono_lite_time();
            let _ = writeln!(file, "[{}] {}: {}", timestamp, sender, message);
        }
    }

    /// Close the current log file
    pub fn close_log(&mut self) {
        self.log_file = None;
        self.log_date.clear();
    }
}

impl Default for ChatState {
    fn default() -> Self {
        Self::new()
    }
}

/// Global chat state
pub static CHAT_STATE: LazyLock<Mutex<ChatState>> =
    LazyLock::new(|| Mutex::new(ChatState::new()));

// ============================================================
// Chat Message Queue (for packet loss resilience)
// ============================================================

/// A queued outgoing chat message
#[derive(Debug, Clone)]
pub struct QueuedChatMessage {
    /// The message content
    pub message: String,
    /// Time the message was queued (client realtime)
    pub queue_time: i32,
    /// Number of send attempts
    pub attempts: i32,
    /// Whether this is a team message
    pub team: bool,
}

/// Queue for outgoing chat messages during packet loss
/// Messages are queued when network issues are detected and
/// automatically sent when connection is restored.
#[derive(Debug, Clone)]
pub struct ChatMessageQueue {
    /// Queued outgoing messages
    pub queue: Vec<QueuedChatMessage>,
    /// Whether queuing is enabled
    pub enabled: bool,
    /// Maximum messages to queue
    pub max_queue_size: usize,
    /// Maximum age for a queued message (ms) before discarding
    pub max_message_age_ms: i32,
    /// Maximum send attempts before discarding
    pub max_attempts: i32,
}

impl Default for ChatMessageQueue {
    fn default() -> Self {
        Self {
            queue: Vec::new(),
            enabled: true,
            max_queue_size: 8,         // Don't queue too many
            max_message_age_ms: 10000, // 10 second timeout
            max_attempts: 3,
        }
    }
}

impl ChatMessageQueue {
    /// Queue a message for sending
    /// Returns true if queued, false if queue is full/disabled
    pub fn queue_message(&mut self, message: &str, team: bool, current_time: i32) -> bool {
        if !self.enabled || self.queue.len() >= self.max_queue_size {
            return false;
        }

        // Don't queue empty messages
        if message.trim().is_empty() {
            return false;
        }

        self.queue.push(QueuedChatMessage {
            message: message.to_string(),
            queue_time: current_time,
            attempts: 0,
            team,
        });

        true
    }

    /// Get the next message to send (if any)
    /// Returns None if queue is empty or no messages are ready
    pub fn get_next(&mut self, current_time: i32) -> Option<QueuedChatMessage> {
        // Remove expired messages first
        self.queue.retain(|msg| {
            current_time - msg.queue_time < self.max_message_age_ms
        });

        if self.queue.is_empty() {
            return None;
        }

        // Get the first message (FIFO)
        let msg = self.queue.remove(0);
        Some(msg)
    }

    /// Mark a message as failed and re-queue if attempts remaining
    pub fn retry_message(&mut self, mut msg: QueuedChatMessage) {
        msg.attempts += 1;
        if msg.attempts < self.max_attempts {
            // Re-insert at front so it's tried next
            self.queue.insert(0, msg);
        }
        // Otherwise, message is dropped after max attempts
    }

    /// Check if there are queued messages
    pub fn has_pending(&self) -> bool {
        !self.queue.is_empty()
    }

    /// Get number of queued messages
    pub fn pending_count(&self) -> usize {
        self.queue.len()
    }

    /// Clear all queued messages
    pub fn clear(&mut self) {
        self.queue.clear();
    }
}

/// Global chat message queue
pub static CHAT_QUEUE: LazyLock<Mutex<ChatMessageQueue>> =
    LazyLock::new(|| Mutex::new(ChatMessageQueue::default()));

/// Queue an outgoing chat message (call during packet loss)
pub fn chat_queue_outgoing(message: &str, team: bool, current_time: i32) -> bool {
    let mut queue = CHAT_QUEUE.lock().unwrap();
    queue.queue_message(message, team, current_time)
}

/// Get next queued message to send (call when connection restored)
pub fn chat_get_queued(current_time: i32) -> Option<QueuedChatMessage> {
    let mut queue = CHAT_QUEUE.lock().unwrap();
    queue.get_next(current_time)
}

/// Re-queue a message after failed send attempt
pub fn chat_retry_message(msg: QueuedChatMessage) {
    let mut queue = CHAT_QUEUE.lock().unwrap();
    queue.retry_message(msg);
}

/// Check if there are queued chat messages
pub fn chat_has_queued() -> bool {
    let queue = CHAT_QUEUE.lock().unwrap();
    queue.has_pending()
}

/// Clear all queued chat messages
pub fn chat_clear_queue() {
    let mut queue = CHAT_QUEUE.lock().unwrap();
    queue.clear();
}

// ============================================================
// Simple date/time helpers (avoiding chrono dependency)
// ============================================================

/// Get current date as YYYY-MM-DD string
fn chrono_lite_date() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();

    // Days since Unix epoch
    let days = secs / 86400;

    // Calculate year, month, day (simplified, doesn't handle leap seconds perfectly)
    let mut year = 1970;
    let mut remaining_days = days as i64;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    // Calculate month and day
    let days_in_months: [i64; 12] = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1;
    for days_in_month in days_in_months {
        if remaining_days < days_in_month {
            break;
        }
        remaining_days -= days_in_month;
        month += 1;
    }

    let day = remaining_days + 1;

    format!("{:04}-{:02}-{:02}", year, month, day)
}

/// Get current time as HH:MM:SS string
fn chrono_lite_time() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();

    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

// ============================================================
// Public API
// ============================================================

/// Initialize the chat system. Call on client init.
pub fn chat_init() {
    let gamedir = fs_gamedir();
    let mut state = CHAT_STATE.lock().unwrap();
    state.load_filter(&gamedir);
    state.load_ignore_list(&gamedir);
}

/// Process an incoming chat message.
/// Returns None if the message should be ignored, or the filtered message otherwise.
pub fn chat_process_message(sender: &str, message: &str) -> Option<String> {
    let gamedir = fs_gamedir();
    let mut state = CHAT_STATE.lock().unwrap();

    // Check if sender is ignored
    if state.is_ignored(sender) {
        return None;
    }

    // Filter the message
    let filtered = state.filter_message(message);

    // Log the message
    state.log_message(sender, &filtered, &gamedir);

    Some(filtered)
}

/// Extract sender name from a chat message.
/// Chat messages are typically formatted as "name: message" or "name says: message"
pub fn chat_extract_sender(message: &str) -> Option<&str> {
    // Look for "name: " pattern
    if let Some(colon_pos) = message.find(": ") {
        let name = &message[..colon_pos];
        // Validate it looks like a player name (not too long, no special chars at start)
        if name.len() <= MAX_NAME_LEN && !name.starts_with('[') {
            return Some(name);
        }
    }
    None
}

/// Set whether chat filtering is enabled
pub fn chat_set_filter_enabled(enabled: bool) {
    let mut state = CHAT_STATE.lock().unwrap();
    state.filter_enabled = enabled;
}

/// Set whether chat logging is enabled
pub fn chat_set_log_enabled(enabled: bool) {
    let mut state = CHAT_STATE.lock().unwrap();
    state.log_enabled = enabled;
    if !enabled {
        state.close_log();
    }
}

// ============================================================
// Console Commands
// ============================================================

/// ignore <name> - Add a player to the ignore list
pub fn cmd_ignore(args: &str) {
    let name = args.trim();
    if name.is_empty() {
        // List ignored players
        let state = CHAT_STATE.lock().unwrap();
        if state.ignored_players.is_empty() {
            com_printf("No players ignored.\n");
        } else {
            com_printf("Ignored players:\n");
            for player in &state.ignored_players {
                com_printf(&format!("  {}\n", player));
            }
        }
        return;
    }

    let gamedir = fs_gamedir();
    let mut state = CHAT_STATE.lock().unwrap();

    if state.ignore_player(name, &gamedir) {
        com_printf(&format!("Now ignoring '{}'\n", name));
    } else {
        com_printf(&format!("'{}' is already ignored\n", name));
    }
}

/// unignore <name> - Remove a player from the ignore list
pub fn cmd_unignore(args: &str) {
    let name = args.trim();
    if name.is_empty() {
        com_printf("Usage: unignore <name>\n");
        return;
    }

    let gamedir = fs_gamedir();
    let mut state = CHAT_STATE.lock().unwrap();

    if state.unignore_player(name, &gamedir) {
        com_printf(&format!("No longer ignoring '{}'\n", name));
    } else {
        com_printf(&format!("'{}' was not ignored\n", name));
    }
}

/// ignorelist - List all ignored players
pub fn cmd_ignorelist() {
    let state = CHAT_STATE.lock().unwrap();

    if state.ignored_players.is_empty() {
        com_printf("No players ignored.\n");
    } else {
        com_printf(&format!("Ignored players ({}):\n", state.ignored_players.len()));
        for player in &state.ignored_players {
            com_printf(&format!("  {}\n", player));
        }
    }
}

/// filter_reload - Reload the word filter
pub fn cmd_filter_reload() {
    let gamedir = fs_gamedir();
    let mut state = CHAT_STATE.lock().unwrap();
    state.load_filter(&gamedir);
    com_printf(&format!("Reloaded filter ({} words)\n", state.filter_words.len()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_message() {
        let mut state = ChatState::new();
        state.filter_enabled = true;
        state.filter_words.push("badword".to_string());
        state.filter_words.push("test".to_string());

        assert_eq!(state.filter_message("hello world"), "hello world");
        assert_eq!(state.filter_message("this is badword here"), "this is ******* here");
        assert_eq!(state.filter_message("test message"), "**** message");
    }

    #[test]
    fn test_ignore_player() {
        let mut state = ChatState::new();

        assert!(!state.is_ignored("player1"));

        state.ignored_players.insert("player1".to_string());
        assert!(state.is_ignored("player1"));
        assert!(state.is_ignored("PLAYER1")); // Case insensitive
    }

    #[test]
    fn test_extract_sender() {
        assert_eq!(chat_extract_sender("Player: hello"), Some("Player"));
        assert_eq!(chat_extract_sender("[Server]: message"), None); // Server messages
        assert_eq!(chat_extract_sender("no colon here"), None);
    }

    #[test]
    fn test_chrono_lite() {
        let date = chrono_lite_date();
        assert!(date.len() == 10); // YYYY-MM-DD
        assert!(date.contains('-'));

        let time = chrono_lite_time();
        assert!(time.len() == 8); // HH:MM:SS
        assert!(time.contains(':'));
    }
}
