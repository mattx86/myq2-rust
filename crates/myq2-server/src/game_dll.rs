// game_dll.rs — Dynamic game DLL loading
// Handles loading external C game DLLs (gamex86.dll) for mod compatibility

use libloading::{Library, Symbol};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::path::Path;

use myq2_common::game_api::*;

// ============================================================
// GameDll — wrapper for loaded game DLL
// ============================================================

/// Wrapper for a dynamically loaded game DLL
///
/// This struct manages the lifecycle of an external game DLL (gamex86.dll).
/// It handles loading, calling GetGameApi, and provides safe(r) wrappers
/// for calling game export functions.
pub struct GameDll {
    /// The loaded library handle - must be kept alive
    _library: Library,
    /// Pointer to the game export structure returned by GetGameApi
    pub export: *mut game_export_t,
}

impl GameDll {
    /// Load a game DLL from the given path
    ///
    /// # Arguments
    /// * `path` - Path to the game DLL (e.g., "baseq2/gamex86.dll")
    /// * `import` - Pointer to our game_import_t structure with engine functions
    ///
    /// # Safety
    /// The import table must remain valid for the lifetime of the game DLL.
    /// The caller is responsible for ensuring the path points to a valid Q2 game DLL.
    ///
    /// # Errors
    /// Returns an error if:
    /// - The DLL cannot be loaded
    /// - GetGameApi symbol is not found
    /// - GetGameApi returns null
    /// - API version doesn't match GAME_API_VERSION (3)
    pub unsafe fn load(path: &str, import: *mut game_import_t) -> Result<Self, String> {
        // Check if file exists
        if !Path::new(path).exists() {
            return Err(format!("Game DLL not found: {}", path));
        }

        // Load the library
        let library = Library::new(path)
            .map_err(|e| format!("Failed to load game DLL '{}': {}", path, e))?;

        // Get the GetGameApi symbol
        let get_game_api: Symbol<GetGameApiFn> = library
            .get(b"GetGameApi")
            .map_err(|e| format!("GetGameApi not found in '{}': {}", path, e))?;

        // Call GetGameApi with our import table
        let export = get_game_api(import);

        if export.is_null() {
            return Err(format!("GetGameApi returned null for '{}'", path));
        }

        // Verify API version
        let api_version = (*export).apiversion;
        if api_version != GAME_API_VERSION {
            return Err(format!(
                "Game DLL '{}' has API version {} (expected {})",
                path, api_version, GAME_API_VERSION
            ));
        }

        println!("Loaded game DLL: {} (API version {})", path, api_version);

        Ok(Self {
            _library: library,
            export,
        })
    }

    // ============================================================
    // Safe wrappers for game export functions
    // ============================================================

    /// Call ge->Init()
    pub unsafe fn init(&self) {
        if let Some(init_fn) = (*self.export).Init {
            init_fn();
        }
    }

    /// Call ge->Shutdown()
    pub unsafe fn shutdown(&self) {
        if let Some(shutdown_fn) = (*self.export).Shutdown {
            shutdown_fn();
        }
    }

    /// Call ge->SpawnEntities(mapname, entstring, spawnpoint)
    pub unsafe fn spawn_entities(&self, mapname: &str, entstring: &str, spawnpoint: &str) {
        if let Some(spawn_fn) = (*self.export).SpawnEntities {
            let mapname_c = CString::new(mapname).unwrap_or_default();
            let entstring_c = CString::new(entstring).unwrap_or_default();
            let spawnpoint_c = CString::new(spawnpoint).unwrap_or_default();
            spawn_fn(
                mapname_c.as_ptr(),
                entstring_c.as_ptr(),
                spawnpoint_c.as_ptr(),
            );
        }
    }

    /// Call ge->WriteGame(filename, autosave)
    pub unsafe fn write_game(&self, filename: &str, autosave: bool) {
        if let Some(write_fn) = (*self.export).WriteGame {
            let filename_c = CString::new(filename).unwrap_or_default();
            write_fn(filename_c.as_ptr(), if autosave { 1 } else { 0 });
        }
    }

    /// Call ge->ReadGame(filename)
    pub unsafe fn read_game(&self, filename: &str) {
        if let Some(read_fn) = (*self.export).ReadGame {
            let filename_c = CString::new(filename).unwrap_or_default();
            read_fn(filename_c.as_ptr());
        }
    }

    /// Call ge->WriteLevel(filename)
    pub unsafe fn write_level(&self, filename: &str) {
        if let Some(write_fn) = (*self.export).WriteLevel {
            let filename_c = CString::new(filename).unwrap_or_default();
            write_fn(filename_c.as_ptr());
        }
    }

    /// Call ge->ReadLevel(filename)
    pub unsafe fn read_level(&self, filename: &str) {
        if let Some(read_fn) = (*self.export).ReadLevel {
            let filename_c = CString::new(filename).unwrap_or_default();
            read_fn(filename_c.as_ptr());
        }
    }

    /// Call ge->ClientConnect(ent, userinfo)
    /// Returns true if client is allowed to connect
    pub unsafe fn client_connect(&self, ent_index: c_int, userinfo: &mut String) -> bool {
        if let Some(connect_fn) = (*self.export).ClientConnect {
            let ent = self.edict_num(ent_index);
            // Create a mutable buffer for userinfo (game may modify it)
            let mut userinfo_buf = [0u8; 512];
            let userinfo_bytes = userinfo.as_bytes();
            let copy_len = userinfo_bytes.len().min(userinfo_buf.len() - 1);
            userinfo_buf[..copy_len].copy_from_slice(&userinfo_bytes[..copy_len]);

            let result = connect_fn(ent, userinfo_buf.as_mut_ptr() as *mut c_char);

            // Copy modified userinfo back
            if let Ok(s) = CStr::from_ptr(userinfo_buf.as_ptr() as *const c_char).to_str() {
                *userinfo = s.to_string();
            }

            result != 0
        } else {
            false
        }
    }

    /// Call ge->ClientBegin(ent)
    pub unsafe fn client_begin(&self, ent_index: c_int) {
        if let Some(begin_fn) = (*self.export).ClientBegin {
            let ent = self.edict_num(ent_index);
            begin_fn(ent);
        }
    }

    /// Call ge->ClientUserinfoChanged(ent, userinfo)
    pub unsafe fn client_userinfo_changed(&self, ent_index: c_int, userinfo: &str) {
        if let Some(changed_fn) = (*self.export).ClientUserinfoChanged {
            let ent = self.edict_num(ent_index);
            // Create mutable buffer for userinfo
            let mut userinfo_buf = [0u8; 512];
            let userinfo_bytes = userinfo.as_bytes();
            let copy_len = userinfo_bytes.len().min(userinfo_buf.len() - 1);
            userinfo_buf[..copy_len].copy_from_slice(&userinfo_bytes[..copy_len]);
            changed_fn(ent, userinfo_buf.as_mut_ptr() as *mut c_char);
        }
    }

    /// Call ge->ClientDisconnect(ent)
    pub unsafe fn client_disconnect(&self, ent_index: c_int) {
        if let Some(disconnect_fn) = (*self.export).ClientDisconnect {
            let ent = self.edict_num(ent_index);
            disconnect_fn(ent);
        }
    }

    /// Call ge->ClientCommand(ent)
    pub unsafe fn client_command(&self, ent_index: c_int) {
        if let Some(command_fn) = (*self.export).ClientCommand {
            let ent = self.edict_num(ent_index);
            command_fn(ent);
        }
    }

    /// Call ge->ClientThink(ent, cmd)
    pub unsafe fn client_think(&self, ent_index: c_int, cmd: &usercmd_t) {
        if let Some(think_fn) = (*self.export).ClientThink {
            let ent = self.edict_num(ent_index);
            // Copy cmd to mutable buffer
            let mut cmd_copy = *cmd;
            think_fn(ent, &mut cmd_copy);
        }
    }

    /// Call ge->RunFrame()
    pub unsafe fn run_frame(&self) {
        if let Some(run_fn) = (*self.export).RunFrame {
            run_fn();
        }
    }

    /// Call ge->ServerCommand()
    pub unsafe fn server_command(&self) {
        if let Some(cmd_fn) = (*self.export).ServerCommand {
            cmd_fn();
        }
    }

    // ============================================================
    // Edict accessors
    // ============================================================

    /// Get pointer to edict N using proper stride
    #[inline]
    pub unsafe fn edict_num(&self, n: c_int) -> *mut edict_t {
        let edicts = (*self.export).edicts;
        let edict_size = (*self.export).edict_size;
        myq2_common::game_api::edict_num(edicts, edict_size, n)
    }

    /// Get the index of an edict
    #[inline]
    pub unsafe fn num_for_edict(&self, ent: *mut edict_t) -> c_int {
        let edicts = (*self.export).edicts;
        let edict_size = (*self.export).edict_size;
        myq2_common::game_api::num_for_edict(edicts, edict_size, ent)
    }

    /// Get current number of edicts
    #[inline]
    pub unsafe fn num_edicts(&self) -> c_int {
        (*self.export).num_edicts
    }

    /// Get maximum number of edicts
    #[inline]
    pub unsafe fn max_edicts(&self) -> c_int {
        (*self.export).max_edicts
    }

    /// Get edict size in bytes
    #[inline]
    pub unsafe fn edict_size(&self) -> c_int {
        (*self.export).edict_size
    }

    /// Get base edicts pointer
    #[inline]
    pub unsafe fn edicts(&self) -> *mut edict_t {
        (*self.export).edicts
    }
}

impl Drop for GameDll {
    fn drop(&mut self) {
        // Call shutdown before unloading
        unsafe {
            self.shutdown();
        }
        println!("Game DLL unloaded");
    }
}

// ============================================================
// DLL path resolution
// ============================================================

/// Find the game DLL path for a given game directory
///
/// Searches for platform-specific DLL name in the game directory.
/// On Windows: gamex86.dll
/// On Linux: gamei386.so (32-bit) or gamex86_64.so (64-bit)
pub fn find_game_dll(gamedir: &str) -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        #[cfg(target_pointer_width = "64")]
        let dll_name = "gamex86_64.dll";
        #[cfg(target_pointer_width = "32")]
        let dll_name = "gamex86.dll";

        let path = format!("{}/{}", gamedir, dll_name);
        if Path::new(&path).exists() {
            return Some(path);
        }

        // Fallback to gamex86.dll on 64-bit Windows (for 32-bit DLLs)
        #[cfg(target_pointer_width = "64")]
        {
            let path = format!("{}/gamex86.dll", gamedir);
            if Path::new(&path).exists() {
                return Some(path);
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        #[cfg(target_pointer_width = "64")]
        let so_name = "gamex86_64.so";
        #[cfg(target_pointer_width = "32")]
        let so_name = "gamei386.so";

        let path = format!("{}/{}", gamedir, so_name);
        if Path::new(&path).exists() {
            return Some(path);
        }
    }

    #[cfg(target_os = "macos")]
    {
        let dylib_name = "game.dylib";
        let path = format!("{}/{}", gamedir, dylib_name);
        if Path::new(&path).exists() {
            return Some(path);
        }
    }

    None
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_game_dll_nonexistent() {
        // Should return None for non-existent directory
        assert!(find_game_dll("nonexistent_directory").is_none());
    }

    #[test]
    fn test_edict_pointer_math() {
        // Test the edict pointer arithmetic
        unsafe {
            // Simulate an edict array with custom size
            let edict_size: c_int = 256; // Typical game edict is larger than server's view
            let max_edicts = 10;
            let buffer = vec![0u8; (edict_size * max_edicts) as usize];
            let edicts = buffer.as_ptr() as *mut edict_t;

            // Verify edict_num calculation
            let ent5 = edict_num(edicts, edict_size, 5);
            let expected = (edicts as *mut u8).add(5 * 256) as *mut edict_t;
            assert_eq!(ent5, expected);

            // Verify num_for_edict calculation
            let index = num_for_edict(edicts, edict_size, ent5);
            assert_eq!(index, 5);
        }
    }
}
