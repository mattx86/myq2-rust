// cl_loc.rs -- Location system for named map positions
//
// R1Q2/Q2Pro-style location system:
// - Load location files (.loc) for each map
// - Track player's current location
// - Provide $loc_here macro expansion for chat
//
// Location file format (locs/<mapname>.loc):
// ```
// # Comments start with #
// x y z Location Name
// -512 1024 128 Red Armor
// 256 -384 64 Railgun
// ```

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::{LazyLock, Mutex};

use myq2_common::common::com_printf;
use myq2_common::q_shared::Vec3;

/// A named location on a map.
#[derive(Debug, Clone)]
pub struct Location {
    /// Position in world coordinates
    pub origin: Vec3,
    /// Name of the location
    pub name: String,
}

/// Location database for the current map.
pub struct LocationDb {
    /// Map name this database is for
    pub mapname: String,
    /// All locations on this map
    pub locations: Vec<Location>,
    /// Cached nearest location for performance (origin -> location name)
    location_cache: HashMap<[i32; 3], String>,
}

impl LocationDb {
    pub fn new() -> Self {
        Self {
            mapname: String::new(),
            locations: Vec::new(),
            location_cache: HashMap::new(),
        }
    }

    /// Load locations for a map.
    pub fn load(&mut self, mapname: &str, gamedir: &str) {
        self.clear();
        self.mapname = mapname.to_string();

        // Try to load from locs/<mapname>.loc
        let path = format!("{}/locs/{}.loc", gamedir, mapname);

        let file = match File::open(&path) {
            Ok(f) => f,
            Err(_) => {
                // Try baseq2 as fallback
                let fallback_path = format!("baseq2/locs/{}.loc", mapname);
                match File::open(&fallback_path) {
                    Ok(f) => f,
                    Err(_) => {
                        // No location file for this map - that's fine
                        return;
                    }
                }
            }
        };

        let reader = BufReader::new(file);
        let mut count = 0;

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => continue,
            };

            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
                continue;
            }

            // Parse: x y z name
            if let Some(loc) = Self::parse_location_line(line) {
                self.locations.push(loc);
                count += 1;
            }
        }

        if count > 0 {
            com_printf(&format!("Loaded {} locations for {}\n", count, mapname));
        }
    }

    /// Parse a location line: "x y z Location Name"
    fn parse_location_line(line: &str) -> Option<Location> {
        let parts: Vec<&str> = line.splitn(4, char::is_whitespace)
            .filter(|s| !s.is_empty())
            .collect();

        if parts.len() < 4 {
            return None;
        }

        let x: f32 = parts[0].parse().ok()?;
        let y: f32 = parts[1].parse().ok()?;
        let z: f32 = parts[2].parse().ok()?;

        // Name is everything after the coordinates
        let name = parts[3].trim().to_string();
        if name.is_empty() {
            return None;
        }

        Some(Location {
            origin: [x, y, z],
            name,
        })
    }

    /// Find the nearest location to the given position.
    pub fn find_nearest(&mut self, pos: Vec3) -> Option<String> {
        if self.locations.is_empty() {
            return None;
        }

        // Check cache first (quantized to 32 units)
        let cache_key = [
            (pos[0] / 32.0) as i32,
            (pos[1] / 32.0) as i32,
            (pos[2] / 32.0) as i32,
        ];

        if let Some(name) = self.location_cache.get(&cache_key) {
            return Some(name.clone());
        }

        // Find nearest location
        let mut best_dist = f32::MAX;
        let mut best_idx: Option<usize> = None;

        for (i, loc) in self.locations.iter().enumerate() {
            let dx = pos[0] - loc.origin[0];
            let dy = pos[1] - loc.origin[1];
            let dz = pos[2] - loc.origin[2];
            let dist = dx * dx + dy * dy + dz * dz;

            if dist < best_dist {
                best_dist = dist;
                best_idx = Some(i);
            }
        }

        // Cache and return the result
        if let Some(idx) = best_idx {
            let name = self.locations[idx].name.clone();
            self.location_cache.insert(cache_key, name.clone());
            Some(name)
        } else {
            None
        }
    }

    /// Clear all locations.
    pub fn clear(&mut self) {
        self.mapname.clear();
        self.locations.clear();
        self.location_cache.clear();
    }

    /// Get count of locations.
    pub fn count(&self) -> usize {
        self.locations.len()
    }
}

impl Default for LocationDb {
    fn default() -> Self {
        Self::new()
    }
}

/// Global location database.
pub static LOCATION_DB: LazyLock<Mutex<LocationDb>> =
    LazyLock::new(|| Mutex::new(LocationDb::new()));

// ============================================================
// Public API
// ============================================================

/// Load locations for the given map.
/// Call this when a new map is loaded.
pub fn loc_load_map(mapname: &str, gamedir: &str) {
    let mut db = LOCATION_DB.lock().unwrap();
    db.load(mapname, gamedir);
}

/// Get the player's current location name.
/// Returns None if no location file exists or player isn't near any location.
pub fn loc_get_current(pos: Vec3) -> Option<String> {
    let mut db = LOCATION_DB.lock().unwrap();
    db.find_nearest(pos)
}

/// Clear the location database.
/// Call this on disconnect.
pub fn loc_clear() {
    let mut db = LOCATION_DB.lock().unwrap();
    db.clear();
}

/// Expand $loc_here macro in a message.
/// Replaces $loc_here with the player's current location name.
pub fn loc_expand_macros(msg: &str, player_pos: Vec3) -> String {
    if !msg.contains("$loc_here") {
        return msg.to_string();
    }

    let location = loc_get_current(player_pos)
        .unwrap_or_else(|| "unknown".to_string());

    msg.replace("$loc_here", &location)
}

// ============================================================
// Console Commands
// ============================================================

/// loc - Show location info
pub fn cmd_loc(player_pos: Vec3) {
    let mut db = LOCATION_DB.lock().unwrap();

    if db.locations.is_empty() {
        com_printf(&format!("No locations loaded for map '{}'\n", db.mapname));
        return;
    }

    let current = db.find_nearest(player_pos)
        .unwrap_or_else(|| "none".to_string());

    com_printf(&format!(
        "Location: {} ({} locations on map {})\n",
        current,
        db.locations.len(),
        db.mapname
    ));
}

/// loclist - List all locations on current map
pub fn cmd_loclist() {
    let db = LOCATION_DB.lock().unwrap();

    if db.locations.is_empty() {
        com_printf(&format!("No locations loaded for map '{}'\n", db.mapname));
        return;
    }

    com_printf(&format!("Locations for {}:\n", db.mapname));
    for (i, loc) in db.locations.iter().enumerate() {
        com_printf(&format!(
            "  {:3}: {:>8.0} {:>8.0} {:>8.0}  {}\n",
            i + 1,
            loc.origin[0],
            loc.origin[1],
            loc.origin[2],
            loc.name
        ));
    }
    com_printf(&format!("Total: {} locations\n", db.locations.len()));
}

/// locadd <name> - Add a location at the player's current position
pub fn cmd_locadd(name: &str, player_pos: Vec3, gamedir: &str) {
    if name.is_empty() {
        com_printf("Usage: locadd <name>\n");
        return;
    }

    let mut db = LOCATION_DB.lock().unwrap();

    // Add to database
    db.locations.push(Location {
        origin: player_pos,
        name: name.to_string(),
    });
    db.location_cache.clear();

    com_printf(&format!(
        "Added location '{}' at ({:.0}, {:.0}, {:.0})\n",
        name, player_pos[0], player_pos[1], player_pos[2]
    ));

    // Save to file
    let path = format!("{}/locs/{}.loc", gamedir, db.mapname);
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        use std::io::Write;
        let _ = writeln!(
            file,
            "{:.0} {:.0} {:.0} {}",
            player_pos[0], player_pos[1], player_pos[2], name
        );
        com_printf(&format!("Saved to {}\n", path));
    }
}

/// locdel <index> - Delete a location by index (1-based)
pub fn cmd_locdel(index_str: &str) {
    let index: usize = match index_str.parse() {
        Ok(i) => i,
        Err(_) => {
            com_printf("Usage: locdel <index> (see loclist for indices)\n");
            return;
        }
    };

    let mut db = LOCATION_DB.lock().unwrap();

    if index == 0 || index > db.locations.len() {
        com_printf(&format!(
            "Invalid index {}. Valid range: 1-{}\n",
            index,
            db.locations.len()
        ));
        return;
    }

    let removed = db.locations.remove(index - 1);
    db.location_cache.clear();

    com_printf(&format!("Removed location '{}'\n", removed.name));
    com_printf("Note: Run 'locsave' to save changes to file.\n");
}

/// locsave - Save all locations to file
pub fn cmd_locsave(gamedir: &str) {
    let db = LOCATION_DB.lock().unwrap();

    if db.mapname.is_empty() {
        com_printf("No map loaded.\n");
        return;
    }

    let path = format!("{}/locs/{}.loc", gamedir, db.mapname);

    // Create locs directory if needed
    let dir = format!("{}/locs", gamedir);
    let _ = std::fs::create_dir_all(&dir);

    let file = match std::fs::File::create(&path) {
        Ok(f) => f,
        Err(e) => {
            com_printf(&format!("Failed to create {}: {}\n", path, e));
            return;
        }
    };

    use std::io::Write;
    let mut writer = std::io::BufWriter::new(file);

    let _ = writeln!(writer, "# Location file for {}", db.mapname);
    let _ = writeln!(writer, "# Format: x y z Name");
    let _ = writeln!(writer, "# Generated by MyQ2");
    let _ = writeln!(writer);

    for loc in &db.locations {
        let _ = writeln!(
            writer,
            "{:.0} {:.0} {:.0} {}",
            loc.origin[0], loc.origin[1], loc.origin[2], loc.name
        );
    }

    com_printf(&format!("Saved {} locations to {}\n", db.locations.len(), path));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_location_line() {
        let loc = LocationDb::parse_location_line("100 200 300 Test Location").unwrap();
        assert_eq!(loc.origin, [100.0, 200.0, 300.0]);
        assert_eq!(loc.name, "Test Location");

        let loc = LocationDb::parse_location_line("-512 1024 128 Red Armor").unwrap();
        assert_eq!(loc.origin, [-512.0, 1024.0, 128.0]);
        assert_eq!(loc.name, "Red Armor");
    }

    #[test]
    fn test_parse_invalid_lines() {
        assert!(LocationDb::parse_location_line("").is_none());
        assert!(LocationDb::parse_location_line("# comment").is_none());
        assert!(LocationDb::parse_location_line("100 200").is_none()); // Missing z and name
        assert!(LocationDb::parse_location_line("100 200 300").is_none()); // Missing name
    }

    #[test]
    fn test_find_nearest() {
        let mut db = LocationDb::new();
        db.locations.push(Location {
            origin: [0.0, 0.0, 0.0],
            name: "Origin".to_string(),
        });
        db.locations.push(Location {
            origin: [100.0, 0.0, 0.0],
            name: "East".to_string(),
        });
        db.locations.push(Location {
            origin: [0.0, 100.0, 0.0],
            name: "North".to_string(),
        });

        assert_eq!(db.find_nearest([10.0, 0.0, 0.0]).as_deref(), Some("Origin"));
        assert_eq!(db.find_nearest([90.0, 0.0, 0.0]).as_deref(), Some("East"));
        assert_eq!(db.find_nearest([0.0, 90.0, 0.0]).as_deref(), Some("North"));
    }

    #[test]
    fn test_expand_macros() {
        let mut db = LOCATION_DB.lock().unwrap();
        db.locations.push(Location {
            origin: [0.0, 0.0, 0.0],
            name: "Spawn".to_string(),
        });
        drop(db);

        let expanded = loc_expand_macros("I'm at $loc_here", [0.0, 0.0, 0.0]);
        assert_eq!(expanded, "I'm at Spawn");

        let expanded = loc_expand_macros("No macro here", [0.0, 0.0, 0.0]);
        assert_eq!(expanded, "No macro here");
    }
}
