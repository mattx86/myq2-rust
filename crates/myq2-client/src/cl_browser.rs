// cl_browser.rs -- Server Browser (R1Q2/Q2Pro feature)
//
// Features:
// - Master server queries
// - LAN broadcast discovery
// - Server list with sorting and filtering
// - Favorites management
// - Extended server info queries

use std::collections::HashSet;
use std::net::{UdpSocket, SocketAddr, ToSocketAddrs};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use myq2_common::common::com_printf;
use myq2_common::cvar::cvar_variable_string;

/// Maximum servers to track.
const MAX_SERVERS: usize = 1024;
/// Query timeout in milliseconds.
const QUERY_TIMEOUT_MS: u64 = 2000;
/// LAN broadcast address.
const LAN_BROADCAST: &str = "255.255.255.255:27910";

/// Sort column for server list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    Name,
    Map,
    Players,
    Ping,
    GameType,
}

impl Default for SortColumn {
    fn default() -> Self {
        SortColumn::Ping
    }
}

/// Server filter configuration.
#[derive(Clone, Default)]
pub struct ServerFilter {
    /// Filter by server name (contains).
    pub name_contains: String,
    /// Filter by map name (contains).
    pub map_contains: String,
    /// Filter by game type.
    pub gametype: String,
    /// Hide empty servers.
    pub not_empty: bool,
    /// Hide full servers.
    pub not_full: bool,
    /// Maximum ping (0 = no limit).
    pub max_ping: i32,
}

impl ServerFilter {
    /// Check if a server passes the filter.
    pub fn matches(&self, server: &ServerEntry) -> bool {
        if !self.name_contains.is_empty() &&
           !server.name.to_lowercase().contains(&self.name_contains.to_lowercase()) {
            return false;
        }
        if !self.map_contains.is_empty() &&
           !server.map.to_lowercase().contains(&self.map_contains.to_lowercase()) {
            return false;
        }
        if !self.gametype.is_empty() &&
           !server.gametype.to_lowercase().contains(&self.gametype.to_lowercase()) {
            return false;
        }
        if self.not_empty && server.players == 0 {
            return false;
        }
        if self.not_full && server.players >= server.max_players {
            return false;
        }
        if self.max_ping > 0 && server.ping > self.max_ping {
            return false;
        }
        true
    }
}

/// A server entry in the browser list.
#[derive(Clone)]
pub struct ServerEntry {
    /// Server address (ip:port).
    pub address: String,
    /// Server name.
    pub name: String,
    /// Current map.
    pub map: String,
    /// Current player count.
    pub players: i32,
    /// Maximum players.
    pub max_players: i32,
    /// Ping in milliseconds.
    pub ping: i32,
    /// Game type (CTF, DM, etc.).
    pub gametype: String,
    /// Is this server a favorite.
    pub is_favorite: bool,
    /// Last query timestamp.
    pub last_query: Instant,
    /// Last response timestamp.
    pub last_response: Option<Instant>,
    /// Query sequence number.
    pub query_seq: u32,
}

impl Default for ServerEntry {
    fn default() -> Self {
        Self {
            address: String::new(),
            name: String::new(),
            map: String::new(),
            players: 0,
            max_players: 0,
            ping: 9999,
            gametype: String::new(),
            is_favorite: false,
            last_query: Instant::now(),
            last_response: None,
            query_seq: 0,
        }
    }
}

/// Pending query state.
#[derive(Clone)]
pub struct PendingQuery {
    /// Server address.
    pub address: String,
    /// Query send timestamp.
    pub sent_time: Instant,
    /// Query type.
    pub query_type: QueryType,
}

/// Query type.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum QueryType {
    /// Basic info query.
    Info,
    /// Full status query (includes player list).
    Status,
}

/// Server browser state.
pub struct ServerBrowser {
    /// All known servers.
    pub servers: Vec<ServerEntry>,
    /// Favorite server addresses.
    pub favorites: HashSet<String>,
    /// Master server addresses.
    pub masters: Vec<String>,
    /// Current filter settings.
    pub filter: ServerFilter,
    /// Sort column.
    pub sort_column: SortColumn,
    /// Sort ascending.
    pub sort_ascending: bool,
    /// Currently selected server index.
    pub selected: Option<usize>,
    /// Pending queries.
    pub pending_queries: Vec<PendingQuery>,
    /// Query sequence counter.
    pub query_seq: u32,
    /// UDP socket for queries.
    socket: Option<UdpSocket>,
    /// Last refresh timestamp.
    pub last_refresh: Option<Instant>,
    /// Is currently refreshing.
    pub refreshing: bool,
}

impl Default for ServerBrowser {
    fn default() -> Self {
        Self {
            servers: Vec::new(),
            favorites: HashSet::new(),
            masters: vec![
                "master.q2servers.com:27900".to_string(),
                "master.quake2.com:27900".to_string(),
            ],
            filter: ServerFilter::default(),
            sort_column: SortColumn::Ping,
            sort_ascending: true,
            selected: None,
            pending_queries: Vec::new(),
            query_seq: 0,
            socket: None,
            last_refresh: None,
            refreshing: false,
        }
    }
}

impl ServerBrowser {
    /// Initialize the browser socket.
    pub fn init(&mut self) -> Result<(), String> {
        if self.socket.is_some() {
            return Ok(());
        }

        let socket = UdpSocket::bind("0.0.0.0:0")
            .map_err(|e| format!("Failed to create browser socket: {}", e))?;

        socket.set_nonblocking(true)
            .map_err(|e| format!("Failed to set nonblocking: {}", e))?;

        socket.set_broadcast(true)
            .map_err(|e| format!("Failed to set broadcast: {}", e))?;

        self.socket = Some(socket);
        Ok(())
    }

    /// Add a server to the list if not already present.
    fn add_server(&mut self, address: &str) -> Option<usize> {
        // Check if server already exists
        for (i, server) in self.servers.iter().enumerate() {
            if server.address == address {
                return Some(i);
            }
        }

        // Add new server
        if self.servers.len() >= MAX_SERVERS {
            return None;
        }

        let mut entry = ServerEntry::default();
        entry.address = address.to_string();
        entry.is_favorite = self.favorites.contains(address);
        self.servers.push(entry);
        Some(self.servers.len() - 1)
    }

    /// Query a specific server for info.
    pub fn query_server(&mut self, address: &str, query_type: QueryType) {
        let Some(ref socket) = self.socket else {
            return;
        };

        let addr: SocketAddr = match address.to_socket_addrs() {
            Ok(mut addrs) => match addrs.next() {
                Some(a) => a,
                None => return,
            },
            Err(_) => return,
        };

        let query: &[u8] = match query_type {
            QueryType::Info => b"\xff\xff\xff\xffinfo",
            QueryType::Status => b"\xff\xff\xff\xffstatus",
        };

        if socket.send_to(query, addr).is_ok() {
            // Track pending query
            self.pending_queries.push(PendingQuery {
                address: address.to_string(),
                sent_time: Instant::now(),
                query_type,
            });

            // Update server query time
            if let Some(idx) = self.add_server(address) {
                self.servers[idx].last_query = Instant::now();
                self.servers[idx].query_seq = self.query_seq;
                self.query_seq = self.query_seq.wrapping_add(1);
            }
        }
    }

    /// Query all servers in the list.
    pub fn refresh_all(&mut self) {
        let addresses: Vec<String> = self.servers.iter().map(|s| s.address.clone()).collect();
        for addr in addresses {
            self.query_server(&addr, QueryType::Info);
        }
        self.last_refresh = Some(Instant::now());
        self.refreshing = true;
    }

    /// Query master servers for server list.
    pub fn query_masters(&mut self) {
        let Some(ref socket) = self.socket else {
            return;
        };

        let masters = self.masters.clone();
        for master in masters {
            let addr: SocketAddr = match master.to_socket_addrs() {
                Ok(mut addrs) => match addrs.next() {
                    Some(a) => a,
                    None => continue,
                },
                Err(_) => continue,
            };

            // Query format: "query" for most masters
            let _ = socket.send_to(b"query", addr);
        }

        self.refreshing = true;
    }

    /// Broadcast LAN query.
    pub fn query_lan(&mut self) {
        let Some(ref socket) = self.socket else {
            return;
        };

        let addr: SocketAddr = match LAN_BROADCAST.to_socket_addrs() {
            Ok(mut addrs) => match addrs.next() {
                Some(a) => a,
                None => return,
            },
            Err(_) => return,
        };

        // Standard Q2 info query
        let _ = socket.send_to(b"\xff\xff\xff\xffinfo", addr);
    }

    /// Process incoming responses.
    pub fn process_responses(&mut self) {
        // Collect responses first to avoid borrow conflicts
        let mut responses: Vec<(Vec<u8>, SocketAddr)> = Vec::new();

        if let Some(ref socket) = self.socket {
            let mut buf = [0u8; 4096];
            loop {
                match socket.recv_from(&mut buf) {
                    Ok((len, from)) => {
                        responses.push((buf[..len].to_vec(), from));
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        break;
                    }
                    Err(_) => {
                        break;
                    }
                }
            }
        }

        // Process collected responses
        for (data, from) in responses {
            self.process_response(&data, from);
        }

        // Clean up timed-out queries
        let timeout = Duration::from_millis(QUERY_TIMEOUT_MS);
        self.pending_queries.retain(|q| q.sent_time.elapsed() < timeout);

        // Check if refresh is complete
        if self.pending_queries.is_empty() {
            self.refreshing = false;
        }
    }

    /// Process a single response.
    fn process_response(&mut self, data: &[u8], from: SocketAddr) {
        // Check for connectionless prefix
        if data.len() < 4 || &data[0..4] != b"\xff\xff\xff\xff" {
            return;
        }

        let data = &data[4..];
        let address = from.to_string();

        // Parse response type
        if data.starts_with(b"info\n") || data.starts_with(b"infoResponse\n") {
            self.parse_info_response(&address, data);
        } else if data.starts_with(b"statusResponse\n") || data.starts_with(b"print\n") {
            self.parse_status_response(&address, data);
        } else if data.starts_with(b"servers ") || data.starts_with(b"servers\n") {
            self.parse_master_response(data);
        }
    }

    /// Parse info response.
    fn parse_info_response(&mut self, address: &str, data: &[u8]) {
        let idx = match self.add_server(address) {
            Some(i) => i,
            None => return,
        };

        let now = Instant::now();
        let ping = self.servers[idx].last_query.elapsed().as_millis() as i32;
        self.servers[idx].ping = ping;
        self.servers[idx].last_response = Some(now);

        // Parse info string (key=value pairs)
        let info_str = String::from_utf8_lossy(data);
        for line in info_str.lines() {
            let line = line.trim();
            if line.is_empty() || line == "info" || line == "infoResponse" {
                continue;
            }

            // Parse backslash-separated key-value pairs
            if line.starts_with('\\') {
                let pairs: Vec<&str> = line[1..].split('\\').collect();
                for chunk in pairs.chunks(2) {
                    if chunk.len() == 2 {
                        self.set_server_info(idx, chunk[0], chunk[1]);
                    }
                }
            }
        }

        // Remove from pending
        self.pending_queries.retain(|q| q.address != address);
    }

    /// Parse status response (includes player list).
    fn parse_status_response(&mut self, address: &str, data: &[u8]) {
        // Status response is similar to info but may include player list
        self.parse_info_response(address, data);
    }

    /// Parse master server response.
    fn parse_master_response(&mut self, data: &[u8]) {
        let response = String::from_utf8_lossy(data);

        // Parse server addresses from response
        // Format varies by master: "ip:port" per line or packed binary
        for line in response.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("servers") {
                continue;
            }

            // Try to parse as ip:port
            if line.contains(':') && line.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                if let Some(_) = self.add_server(line) {
                    // Query the newly added server
                    self.query_server(line, QueryType::Info);
                }
            }
        }
    }

    /// Set server info field.
    fn set_server_info(&mut self, idx: usize, key: &str, value: &str) {
        let server = &mut self.servers[idx];
        match key.to_lowercase().as_str() {
            "hostname" | "sv_hostname" => server.name = value.to_string(),
            "mapname" | "map" => server.map = value.to_string(),
            "clients" | "numplayers" => server.players = value.parse().unwrap_or(0),
            "maxclients" | "sv_maxclients" | "maxplayers" => server.max_players = value.parse().unwrap_or(0),
            "gametype" | "gamemode" | "g_gametype" => server.gametype = value.to_string(),
            _ => {}
        }
    }

    /// Sort the server list.
    pub fn sort(&mut self) {
        let ascending = self.sort_ascending;
        match self.sort_column {
            SortColumn::Name => {
                self.servers.sort_by(|a, b| {
                    let cmp = a.name.to_lowercase().cmp(&b.name.to_lowercase());
                    if ascending { cmp } else { cmp.reverse() }
                });
            }
            SortColumn::Map => {
                self.servers.sort_by(|a, b| {
                    let cmp = a.map.to_lowercase().cmp(&b.map.to_lowercase());
                    if ascending { cmp } else { cmp.reverse() }
                });
            }
            SortColumn::Players => {
                self.servers.sort_by(|a, b| {
                    let cmp = a.players.cmp(&b.players);
                    if ascending { cmp } else { cmp.reverse() }
                });
            }
            SortColumn::Ping => {
                self.servers.sort_by(|a, b| {
                    let cmp = a.ping.cmp(&b.ping);
                    if ascending { cmp } else { cmp.reverse() }
                });
            }
            SortColumn::GameType => {
                self.servers.sort_by(|a, b| {
                    let cmp = a.gametype.to_lowercase().cmp(&b.gametype.to_lowercase());
                    if ascending { cmp } else { cmp.reverse() }
                });
            }
        }
    }

    /// Get filtered and sorted server list.
    pub fn get_filtered_servers(&self) -> Vec<&ServerEntry> {
        self.servers
            .iter()
            .filter(|s| self.filter.matches(s))
            .collect()
    }

    /// Add a server to favorites.
    pub fn add_favorite(&mut self, address: &str) {
        self.favorites.insert(address.to_string());

        // Update existing entry
        for server in &mut self.servers {
            if server.address == address {
                server.is_favorite = true;
                break;
            }
        }
    }

    /// Remove a server from favorites.
    pub fn remove_favorite(&mut self, address: &str) {
        self.favorites.remove(address);

        // Update existing entry
        for server in &mut self.servers {
            if server.address == address {
                server.is_favorite = false;
                break;
            }
        }
    }

    /// Save favorites to file.
    pub fn save_favorites(&self, gamedir: &str) {
        let path = format!("{}/favorites.txt", gamedir);
        if let Ok(mut file) = std::fs::File::create(&path) {
            use std::io::Write;
            for addr in &self.favorites {
                let _ = writeln!(file, "{}", addr);
            }
        }
    }

    /// Load favorites from file.
    pub fn load_favorites(&mut self, gamedir: &str) {
        let path = format!("{}/favorites.txt", gamedir);
        if let Ok(contents) = std::fs::read_to_string(&path) {
            for line in contents.lines() {
                let addr = line.trim();
                if !addr.is_empty() {
                    self.favorites.insert(addr.to_string());
                }
            }
        }
    }

    /// Add a server manually by address.
    pub fn add_manual(&mut self, address: &str) {
        if let Some(_) = self.add_server(address) {
            self.query_server(address, QueryType::Info);
        }
    }

    /// Clear all servers.
    pub fn clear(&mut self) {
        self.servers.clear();
        self.pending_queries.clear();
        self.selected = None;
    }

    /// Get server count.
    pub fn server_count(&self) -> usize {
        self.servers.len()
    }

    /// Get filtered server count.
    pub fn filtered_count(&self) -> usize {
        self.servers.iter().filter(|s| self.filter.matches(s)).count()
    }
}

/// Global server browser instance.
pub static BROWSER: LazyLock<Mutex<ServerBrowser>> = LazyLock::new(|| Mutex::new(ServerBrowser::default()));

// ============================================================
// Public API
// ============================================================

/// Initialize the server browser.
pub fn browser_init() {
    let mut browser = BROWSER.lock().unwrap();
    if let Err(e) = browser.init() {
        com_printf(&format!("Server browser init failed: {}\n", e));
    }

    // Load favorites
    let gamedir = cvar_variable_string("game");
    let gamedir = if gamedir.is_empty() { "baseq2".to_string() } else { gamedir };
    browser.load_favorites(&gamedir);
}

/// Refresh the server list (query masters and existing servers).
pub fn browser_refresh() {
    let mut browser = BROWSER.lock().unwrap();
    browser.query_masters();
    browser.query_lan();
    browser.refresh_all();
}

/// Process incoming server responses (call periodically).
pub fn browser_update() {
    let mut browser = BROWSER.lock().unwrap();
    browser.process_responses();
}

/// Add a server manually.
pub fn browser_add_server(address: &str) {
    let mut browser = BROWSER.lock().unwrap();
    browser.add_manual(address);
}

/// Toggle favorite status for selected server.
pub fn browser_toggle_favorite(address: &str) {
    let mut browser = BROWSER.lock().unwrap();
    if browser.favorites.contains(address) {
        browser.remove_favorite(address);
        com_printf(&format!("Removed {} from favorites.\n", address));
    } else {
        browser.add_favorite(address);
        com_printf(&format!("Added {} to favorites.\n", address));
    }

    // Save favorites
    let gamedir = cvar_variable_string("game");
    let gamedir = if gamedir.is_empty() { "baseq2".to_string() } else { gamedir };
    browser.save_favorites(&gamedir);
}

/// Set filter options.
pub fn browser_set_filter(filter: ServerFilter) {
    let mut browser = BROWSER.lock().unwrap();
    browser.filter = filter;
}

/// Set sort column.
pub fn browser_set_sort(column: SortColumn, ascending: bool) {
    let mut browser = BROWSER.lock().unwrap();
    browser.sort_column = column;
    browser.sort_ascending = ascending;
    browser.sort();
}

/// Print browser info.
pub fn cmd_browser_info() {
    let browser = BROWSER.lock().unwrap();
    com_printf(&format!(
        "Server Browser Info:\n\
         Total servers: {}\n\
         Filtered servers: {}\n\
         Favorites: {}\n\
         Pending queries: {}\n\
         Refreshing: {}\n",
        browser.server_count(),
        browser.filtered_count(),
        browser.favorites.len(),
        browser.pending_queries.len(),
        if browser.refreshing { "yes" } else { "no" },
    ));
}

/// Print server list.
pub fn cmd_serverlist() {
    let browser = BROWSER.lock().unwrap();
    let filtered = browser.get_filtered_servers();

    if filtered.is_empty() {
        com_printf("No servers found. Use 'refreshservers' to query.\n");
        return;
    }

    com_printf(&format!("--- Server List ({} servers) ---\n", filtered.len()));
    com_printf("  Ping  Players  Map              Name\n");
    com_printf("  ----  -------  ---              ----\n");

    for server in filtered {
        let fav = if server.is_favorite { "*" } else { " " };
        com_printf(&format!(
            "{}{:>4}  {:>3}/{:<3}  {:<16} {}\n",
            fav,
            server.ping,
            server.players,
            server.max_players,
            &server.map[..server.map.len().min(16)],
            &server.name[..server.name.len().min(40)],
        ));
    }
}

/// Clear server list.
pub fn cmd_browser_clear() {
    let mut browser = BROWSER.lock().unwrap();
    browser.clear();
    com_printf("Server list cleared.\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_filter() {
        let filter = ServerFilter {
            not_empty: true,
            max_ping: 100,
            ..Default::default()
        };

        let mut server = ServerEntry::default();
        server.players = 0;
        server.ping = 50;
        assert!(!filter.matches(&server)); // Empty server

        server.players = 4;
        assert!(filter.matches(&server)); // Has players, low ping

        server.ping = 200;
        assert!(!filter.matches(&server)); // High ping
    }

    #[test]
    fn test_server_sort() {
        let mut browser = ServerBrowser::default();

        let mut s1 = ServerEntry::default();
        s1.name = "Server A".to_string();
        s1.ping = 50;

        let mut s2 = ServerEntry::default();
        s2.name = "Server B".to_string();
        s2.ping = 25;

        browser.servers.push(s1);
        browser.servers.push(s2);

        browser.sort_column = SortColumn::Ping;
        browser.sort_ascending = true;
        browser.sort();

        assert_eq!(browser.servers[0].name, "Server B"); // Lower ping first
    }

    #[test]
    fn test_favorites() {
        let mut browser = ServerBrowser::default();

        browser.add_favorite("127.0.0.1:27910");
        assert!(browser.favorites.contains("127.0.0.1:27910"));

        browser.remove_favorite("127.0.0.1:27910");
        assert!(!browser.favorites.contains("127.0.0.1:27910"));
    }
}
