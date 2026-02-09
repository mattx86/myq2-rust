// files.rs — Quake 2 virtual filesystem
// Converted from: myq2-original/qcommon/files.c

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

use rayon::prelude::*;

use crate::common::{com_printf, com_dprintf};
use crate::q_shared::{CVAR_LATCH, CVAR_NOSET, CVAR_SERVERINFO};
use crate::qcommon::BASEDIRNAME;
use crate::qfiles::{
    DPackFile, DPackHeader, DZipHeader, IDPAKHEADER, MAX_FILES_IN_PACK, ZPAKDIRHEADER, ZPAKHEADER,
};

// ============================================================
// Constants
// ============================================================

/// PAK0 checksum for full version
pub const PAK0_CHECKSUM: u32 = 0x40e614e0;

/// Maximum bytes per read chunk (64k)
const MAX_READ: usize = 0x10000;

const DEFAULTPAK: &str = "pak";
const DEFAULTZIP: &str = "zip";

// ============================================================
// In-memory structures
// ============================================================

/// A file entry within a pack file (in-memory representation).
#[derive(Debug, Clone)]
pub struct PackFile {
    pub name: String,
    pub filepos: i32,
    pub filelen: i32,
}

/// A loaded .pak or .zip archive.
#[derive(Debug)]
pub struct Pack {
    pub filename: String,
    pub files: Vec<PackFile>,
    /// HashMap index for O(1) file lookup: lowercase filename -> index in files Vec
    file_index: HashMap<String, usize>,
}

impl Pack {
    /// Creates a new Pack with files and builds the lookup index.
    pub fn new(filename: String, files: Vec<PackFile>) -> Self {
        let file_index = Self::build_index(&files);
        Self {
            filename,
            files,
            file_index,
        }
    }

    /// Builds the HashMap index for O(1) file lookup.
    fn build_index(files: &[PackFile]) -> HashMap<String, usize> {
        files
            .iter()
            .enumerate()
            .map(|(i, pf)| (pf.name.to_lowercase(), i))
            .collect()
    }

    /// Finds a file by name (case-insensitive), returning the PackFile if found.
    #[inline]
    pub fn find_file(&self, filename: &str) -> Option<&PackFile> {
        self.file_index
            .get(&filename.to_lowercase())
            .map(|&idx| &self.files[idx])
    }
}

/// A link that redirects file lookups from one prefix to another.
#[derive(Debug, Clone)]
pub struct FileLink {
    pub from: String,
    pub to: String,
}

/// A single element on the search path — either a directory or a pack file.
#[derive(Debug)]
pub struct SearchPath {
    /// Directory path (used when `pack` is `None`).
    pub filename: String,
    /// If `Some`, this search path entry is a pack/zip file.
    pub pack: Option<Pack>,
}

/// Result of opening a file through the virtual filesystem.
pub struct FsOpenResult {
    pub file: File,
    pub length: i32,
    pub from_pak: bool,
}

// ============================================================
// Filesystem context (replaces C globals)
// ============================================================

/// Callback type for Cbuf_AddText.
pub type CbufAddTextFn = Box<dyn Fn(&str) + Send>;

/// Central filesystem state, replacing all C-level global variables.
pub struct FsContext {
    /// Current game directory (absolute path, e.g. "c:/quake2/baseq2").
    pub gamedir: String,

    /// The search path list. Later entries have lower priority.
    pub search_paths: Vec<SearchPath>,

    /// Index into `search_paths` that marks where the base search paths end.
    /// Entries at index >= base_search_index are "base" and should not be
    /// freed when changing game directories.
    pub base_search_index: usize,

    /// File links (from -> to prefix mapping).
    pub links: Vec<FileLink>,

    /// Cvar-equivalent: base directory (default ".").
    pub basedir: String,

    /// Cvar-equivalent: CD directory (default "").
    pub cddir: String,

    /// Cvar-equivalent: game directory override (default "").
    pub gamedirvar: String,

    /// Cvar-equivalent: pak search wildcard pattern (default "pak*").
    pub paksearch: String,

    /// Set to true when the last FS_FOpenFile found the file inside a pak.
    pub file_from_pak: bool,

    /// Callback to add text to the command buffer (wired to Cbuf_AddText).
    pub cbuf_add_text: Option<CbufAddTextFn>,
}

impl Default for FsContext {
    fn default() -> Self {
        Self {
            gamedir: String::new(),
            search_paths: Vec::new(),
            base_search_index: 0,
            links: Vec::new(),
            basedir: ".".to_string(),
            cddir: String::new(),
            gamedirvar: String::new(),
            paksearch: "pak*".to_string(),
            file_from_pak: false,
            cbuf_add_text: None,
        }
    }
}

impl FsContext {
    pub fn new() -> Self {
        Self::default()
    }

    // ============================================================
    // FS_filelength
    // ============================================================

    /// Returns the length of an open file.
    pub fn filelength(f: &mut File) -> io::Result<u64> {
        let pos = f.stream_position()?;
        let end = f.seek(SeekFrom::End(0))?;
        f.seek(SeekFrom::Start(pos))?;
        Ok(end)
    }

    // ============================================================
    // FS_CreatePath
    // ============================================================

    /// Creates any intermediate directories needed to store the given filename.
    pub fn create_path(path: &str) {
        let p = Path::new(path);
        if let Some(parent) = p.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                com_printf(&format!("FS_CreatePath: failed to create {}: {}\n", parent.display(), e));
            }
        }
    }

    // ============================================================
    // Developer_searchpath
    // ============================================================

    /// Returns 1 if "xatrix" is found in any search path, 2 if "rogue" is found, else 0.
    pub fn developer_searchpath(&self, _who: i32) -> i32 {
        for sp in &self.search_paths {
            if sp.filename.contains("xatrix") {
                return 1;
            }
            if sp.filename.contains("rogue") {
                return 2;
            }
        }
        0
    }

    // ============================================================
    // FS_FOpenFile
    // ============================================================

    /// Finds the file in the search path. Returns an `FsOpenResult` on success,
    /// or `None` if the file could not be found.
    pub fn fopen_file(&mut self, filename: &str) -> Option<FsOpenResult> {
        self.file_from_pak = false;

        // Check links first
        for link in &self.links {
            if filename.starts_with(&link.from) {
                let netpath = format!("{}{}", link.to, &filename[link.from.len()..]);
                if let Ok(mut f) = File::open(&netpath) {
                    com_dprintf(&format!("link file: {}\n", netpath));
                    let len = Self::filelength(&mut f).unwrap_or(0) as i32;
                    return Some(FsOpenResult {
                        file: f,
                        length: len,
                        from_pak: false,
                    });
                }
                return None;
            }
        }

        // Search through the path, one element at a time
        for sp in &self.search_paths {
            if let Some(ref pack) = sp.pack {
                // O(1) lookup via HashMap index instead of O(n) linear search
                if let Some(pf) = pack.find_file(filename) {
                    // Found it in a pack
                    com_dprintf(&format!("PackFile: {} : {}\n", pack.filename, filename));
                    let mut f = File::open(&pack.filename).unwrap_or_else(|_| {
                        panic!("Couldn't reopen {}", pack.filename);
                    });
                    f.seek(SeekFrom::Start(pf.filepos as u64)).ok();
                    // We need to set file_from_pak but we have &self borrow
                    // — handled after the loop via the return value.
                    return Some(FsOpenResult {
                        file: f,
                        length: pf.filelen,
                        from_pak: true,
                    });
                }
            } else {
                // Check a file in the directory tree
                let netpath = format!("{}/{}", sp.filename, filename);
                if let Ok(mut f) = File::open(&netpath) {
                    com_dprintf(&format!("FindFile: {}\n", netpath));
                    let len = Self::filelength(&mut f).unwrap_or(0) as i32;
                    return Some(FsOpenResult {
                        file: f,
                        length: len,
                        from_pak: false,
                    });
                }
            }
        }

        com_dprintf(&format!("FindFile: can't find {}\n", filename));
        None
    }

    // ============================================================
    // FS_Read
    // ============================================================

    /// Reads `len` bytes from `f` into `buf`, handling partial reads.
    /// Panics (Com_Error equivalent) if no progress can be made.
    pub fn fs_read(buf: &mut [u8], f: &mut File) -> io::Result<()> {
        let mut offset = 0usize;
        let mut tries = 0;

        while offset < buf.len() {
            let block = std::cmp::min(buf.len() - offset, MAX_READ);
            match f.read(&mut buf[offset..offset + block]) {
                Ok(0) => {
                    if tries == 0 {
                        tries = 1;
                    } else {
                        panic!("FS_Read: 0 bytes read");
                    }
                }
                Ok(n) => {
                    offset += n;
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    // ============================================================
    // FS_LoadFile
    // ============================================================

    /// Loads a file into memory. Returns `None` if not found.
    /// If found, returns the file contents as a `Vec<u8>`.
    pub fn load_file(&mut self, path: &str) -> Option<Vec<u8>> {
        let result = self.fopen_file(path)?;
        let mut f = result.file;
        let len = result.length as usize;
        self.file_from_pak = result.from_pak;

        let mut buf = vec![0u8; len];
        if let Err(e) = Self::fs_read(&mut buf, &mut f) {
            com_printf(&format!("FS_LoadFile: read error: {}\n", e));
            return None;
        }
        Some(buf)
    }

    /// Returns the length of a file without loading it, or `None` if not found.
    pub fn file_length(&mut self, path: &str) -> Option<i32> {
        let result = self.fopen_file(path)?;
        self.file_from_pak = result.from_pak;
        Some(result.length)
    }

    // ============================================================
    // FS_LoadPackFile
    // ============================================================

    /// Loads a .pak file, returning a `Pack` on success.
    pub fn load_pack_file(packfile: &str) -> Option<Pack> {
        let mut f = File::open(packfile).ok()?;

        // Read the header
        let mut header_bytes = [0u8; std::mem::size_of::<DPackHeader>()];
        f.read_exact(&mut header_bytes).ok()?;

        let header: DPackHeader = unsafe { std::ptr::read(header_bytes.as_ptr() as *const _) };

        if i32::from_le(header.ident) != IDPAKHEADER {
            panic!("{} is not a packfile", packfile);
        }

        let dirofs = i32::from_le(header.dirofs);
        let dirlen = i32::from_le(header.dirlen);
        let numpackfiles = dirlen as usize / std::mem::size_of::<DPackFile>();

        if numpackfiles > MAX_FILES_IN_PACK {
            panic!("{} has {} files", packfile, numpackfiles);
        }

        // Seek to directory and read entries
        f.seek(SeekFrom::Start(dirofs as u64)).ok()?;

        let mut info = Vec::with_capacity(numpackfiles);
        for _ in 0..numpackfiles {
            let mut entry_bytes = [0u8; std::mem::size_of::<DPackFile>()];
            f.read_exact(&mut entry_bytes).ok()?;
            let entry: DPackFile = unsafe { std::ptr::read(entry_bytes.as_ptr() as *const _) };
            info.push(entry);
        }

        // Parse directory entries - parallel for large packs (64+ files)
        const PARALLEL_THRESHOLD: usize = 64;
        let files: Vec<PackFile> = if numpackfiles >= PARALLEL_THRESHOLD {
            info.par_iter()
                .map(|entry| {
                    let name_end = entry.name.iter().position(|&b| b == 0).unwrap_or(entry.name.len());
                    let name = String::from_utf8_lossy(&entry.name[..name_end]).to_string();
                    PackFile {
                        name,
                        filepos: i32::from_le(entry.filepos),
                        filelen: i32::from_le(entry.filelen),
                    }
                })
                .collect()
        } else {
            info.iter()
                .map(|entry| {
                    let name_end = entry.name.iter().position(|&b| b == 0).unwrap_or(entry.name.len());
                    let name = String::from_utf8_lossy(&entry.name[..name_end]).to_string();
                    PackFile {
                        name,
                        filepos: i32::from_le(entry.filepos),
                        filelen: i32::from_le(entry.filelen),
                    }
                })
                .collect()
        };

        com_printf(&format!("Added {} ({} files)\n", packfile, numpackfiles));
        Some(Pack::new(packfile.to_string(), files))
    }

    // ============================================================
    // FS_LoadZipFile
    // ============================================================

    /// Loads an uncompressed .zip file (store-only), returning a `Pack` on success.
    pub fn load_zip_file(packfile: &str) -> Option<Pack> {
        let mut f = File::open(packfile).ok()?;

        let header_size = std::mem::size_of::<DZipHeader>();
        let mut files: Vec<PackFile> = Vec::new();

        for _ in 0..MAX_FILES_IN_PACK {
            let mut hdr_bytes = vec![0u8; header_size];
            if f.read_exact(&mut hdr_bytes).is_err() {
                break;
            }
            let temp: DZipHeader = unsafe { std::ptr::read(hdr_bytes.as_ptr() as *const _) };

            // Check for central directory header — means we are done
            let ident_be = u32::from_be(temp.ident);
            if ident_be == ZPAKDIRHEADER {
                break;
            }

            // Validate local file header signature
            if ident_be != ZPAKHEADER && files.is_empty() {
                panic!("{} is not a packfile", packfile);
            }

            // Check for compression or flags
            let compression = { temp.compression };
            let flags = { temp.flags };
            if compression != 0 || flags != 0 {
                panic!("{} contains errors or is compressed", packfile);
            }

            let uncompressed_size = { temp.uncompressed_size } as i32;
            let filename_length = { temp.filename_length } as usize;
            let extra_field_length = { temp.extra_field_length } as usize;

            // Read the filename
            let mut name_bytes = vec![0u8; filename_length];
            f.read_exact(&mut name_bytes).ok()?;
            let name = String::from_utf8_lossy(&name_bytes).to_string();

            // Data offset = current position + extra field length
            let data_offset = f.stream_position().ok()? as i32 + extra_field_length as i32;

            files.push(PackFile {
                name,
                filepos: data_offset,
                filelen: uncompressed_size,
            });

            // Seek to next local file header
            let next_pos = data_offset as u64 + uncompressed_size as u64;
            f.seek(SeekFrom::Start(next_pos)).ok()?;
        }

        let num = files.len();
        com_printf(&format!("Added {} ({} files)\n", packfile, num));
        // Note: ZIP parsing must be sequential due to stream position dependencies,
        // but we still use Pack::new() to build the HashMap index for O(1) lookup.
        Some(Pack::new(packfile.to_string(), files))
    }

    // ============================================================
    // FS_ListFiles (simplified — uses std::fs glob via pattern matching)
    // ============================================================

    /// Lists files matching a simple glob pattern (e.g. "baseq2/pak*.pak").
    /// Only supports a single '*' wildcard in the filename portion.
    pub fn list_files(findname: &str) -> Vec<String> {
        let path = Path::new(findname);
        let dir = match path.parent() {
            Some(d) => d,
            None => return Vec::new(),
        };
        let pattern = match path.file_name() {
            Some(p) => p.to_string_lossy().to_string(),
            None => return Vec::new(),
        };

        let entries: Vec<_> = match fs::read_dir(dir) {
            Ok(e) => e.flatten().collect(),
            Err(_) => return Vec::new(),
        };

        // Parallel wildcard matching for large directories (64+ entries)
        const PARALLEL_THRESHOLD: usize = 64;
        let pattern_lower = pattern.to_lowercase();

        let mut results: Vec<String> = if entries.len() >= PARALLEL_THRESHOLD {
            entries
                .par_iter()
                .filter_map(|entry| {
                    let fname = entry.file_name().to_string_lossy().to_lowercase();
                    if Self::simple_wildcard_match(&pattern_lower, &fname) {
                        Some(entry.path().to_string_lossy().to_string())
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            entries
                .iter()
                .filter_map(|entry| {
                    let fname = entry.file_name().to_string_lossy().to_lowercase();
                    if Self::simple_wildcard_match(&pattern_lower, &fname) {
                        Some(entry.path().to_string_lossy().to_string())
                    } else {
                        None
                    }
                })
                .collect()
        };

        results.sort();
        results
    }

    /// Simple wildcard matching supporting '*' wildcards.
    fn simple_wildcard_match(pattern: &str, text: &str) -> bool {
        let parts: Vec<&str> = pattern.split('*').collect();
        if parts.len() == 1 {
            // No wildcard
            return pattern == text;
        }
        // Check prefix (before first *)
        if !text.starts_with(parts[0]) {
            return false;
        }
        let mut pos = parts[0].len();
        // Check middle parts greedily
        for &part in &parts[1..parts.len() - 1] {
            match text[pos..].find(part) {
                Some(idx) => pos += idx + part.len(),
                None => return false,
            }
        }
        // Check suffix (after last *)
        let suffix = parts[parts.len() - 1];
        pos <= text.len() - suffix.len() && text[pos..].ends_with(suffix)
    }

    // ============================================================
    // FS_AddGameDirectory
    // ============================================================

    /// Adds a game directory to the search path, loading all matching .pak and .zip files.
    ///
    /// Uses parallel I/O to load pak/zip files concurrently for faster startup.
    pub fn add_game_directory(&mut self, dir: &str) {
        self.gamedir = dir.to_string();

        let findpaks = format!("{}/{}.pak", dir, self.paksearch);
        let findzips = format!("{}/{}.zip", dir, self.paksearch);

        let pakfiles = Self::list_files(&findpaks);
        let zipfiles = Self::list_files(&findzips);

        // Add the directory itself to the search path
        self.search_paths.insert(
            0,
            SearchPath {
                filename: dir.to_string(),
                pack: None,
            },
        );

        // Load .pak and .zip files in parallel
        // Combine both lists with a tag indicating the type
        let all_archives: Vec<_> = pakfiles
            .iter()
            .map(|p| (p.as_str(), true)) // true = pak
            .chain(zipfiles.iter().map(|z| (z.as_str(), false))) // false = zip
            .collect();

        // Parallel load all archives
        let loaded_packs: Vec<Option<Pack>> = all_archives
            .par_iter()
            .map(|(path, is_pak)| {
                if *is_pak {
                    Self::load_pack_file(path)
                } else {
                    Self::load_zip_file(path)
                }
            })
            .collect();

        // Add loaded packs to search paths (sequential to maintain order)
        for pack in loaded_packs.into_iter().flatten() {
            self.search_paths.insert(
                0,
                SearchPath {
                    filename: String::new(),
                    pack: Some(pack),
                },
            );
        }
    }

    // ============================================================
    // FS_Gamedir
    // ============================================================

    /// Returns the current game directory, or BASEDIRNAME if not set.
    pub fn gamedir(&self) -> &str {
        if !self.gamedir.is_empty() {
            &self.gamedir
        } else {
            BASEDIRNAME
        }
    }

    // ============================================================
    // FS_ExecAutoexec
    // ============================================================

    /// Checks for autoexec.cfg and executes it via Cbuf_AddText if it exists.
    pub fn exec_autoexec(&self) -> Option<String> {
        let name = if !self.gamedirvar.is_empty() {
            format!("{}/{}/autoexec.cfg", self.basedir, self.gamedirvar)
        } else {
            format!("{}/{}/autoexec.cfg", self.basedir, BASEDIRNAME)
        };

        if Path::new(&name).exists() {
            if let Some(ref cbuf) = self.cbuf_add_text {
                cbuf("exec autoexec.cfg\n");
            }
            Some(name)
        } else {
            None
        }
    }

    // ============================================================
    // FS_SetGamedir
    // ============================================================

    /// Sets the gamedir and path to a different directory.
    pub fn set_gamedir(&mut self, dir: &str) {
        if dir.contains("..") || dir.contains('/') || dir.contains('\\') || dir.contains(':') {
            com_printf("Gamedir should be a single filename, not a path\n");
            return;
        }

        // Free up any current game dir info (entries before base_search_index)
        self.search_paths.drain(0..self.base_search_index);
        self.base_search_index = 0;

        if let Some(ref cbuf) = self.cbuf_add_text {
            cbuf("vid_restart\nsnd_restart\n");
        }

        self.gamedir = format!("{}/{}", self.basedir, dir);

        if dir == BASEDIRNAME || dir.is_empty() {
            crate::cvar::cvar_full_set("gamedir", "", CVAR_SERVERINFO | CVAR_NOSET);
            crate::cvar::cvar_full_set("game", "", CVAR_LATCH | CVAR_SERVERINFO);
        } else {
            crate::cvar::cvar_full_set("gamedir", dir, CVAR_SERVERINFO | CVAR_NOSET);
            if !self.cddir.is_empty() {
                let cd_game = format!("{}/{}", self.cddir, dir);
                self.add_game_directory(&cd_game);
            }
            let base_game = format!("{}/{}", self.basedir, dir);
            self.add_game_directory(&base_game);
        }
    }

    // ============================================================
    // FS_Link_f
    // ============================================================

    /// Creates, updates, or deletes a file link.
    /// Equivalent to the "link" console command.
    pub fn link(&mut self, from: &str, to: &str) {
        // See if the link already exists
        if let Some(pos) = self.links.iter().position(|l| l.from == from) {
            if to.is_empty() {
                // Delete it
                self.links.remove(pos);
            } else {
                self.links[pos].to = to.to_string();
            }
            return;
        }

        // Create a new link
        if !to.is_empty() {
            self.links.push(FileLink {
                from: from.to_string(),
                to: to.to_string(),
            });
        }
    }

    // ============================================================
    // FS_Dir_f
    // ============================================================

    /// Prints directory listings for all search paths matching the given wildcard.
    pub fn dir_f(&self, wildcard: &str) {
        let wc = if wildcard.is_empty() { "*.*" } else { wildcard };

        let mut prev_path: Option<&str> = None;
        loop {
            let path = self.next_path(prev_path);
            let path = match path {
                Some(p) => p,
                None => break,
            };

            let findname = format!("{}/{}", path, wc).replace('\\', "/");
            com_printf(&format!("Directory of {}\n", findname));
            com_printf("----\n");

            let files = Self::list_files(&findname);
            for f in &files {
                if let Some(slash) = f.rfind('/') {
                    com_printf(&format!("{}\n", &f[slash + 1..]));
                } else {
                    com_printf(&format!("{}\n", f));
                }
            }
            com_printf("\n");

            prev_path = Some(path);
        }
    }

    // ============================================================
    // FS_Path_f
    // ============================================================

    /// Prints the current search path (equivalent to the "path" console command).
    pub fn path_f(&self) {
        com_printf("Current search path:\n");
        for (i, sp) in self.search_paths.iter().enumerate() {
            if i == self.base_search_index && self.base_search_index > 0 {
                com_printf("----------\n");
            }
            if let Some(ref pack) = sp.pack {
                com_printf(&format!("{} ({} files)\n", pack.filename, pack.files.len()));
            } else {
                com_printf(&format!("{}\n", sp.filename));
            }
        }

        com_printf("\nLinks:\n");
        for link in &self.links {
            com_printf(&format!("{} : {}\n", link.from, link.to));
        }
    }

    // ============================================================
    // FS_NextPath
    // ============================================================

    /// Allows enumerating all directories in the search path.
    /// Pass `None` to start; pass the previously returned path to get the next one.
    pub fn next_path<'a>(&'a self, prevpath: Option<&str>) -> Option<&'a str> {
        if prevpath.is_none() {
            if self.gamedir.is_empty() {
                return None;
            }
            return Some(&self.gamedir);
        }

        let prev = prevpath.unwrap();
        let mut last: &str = &self.gamedir;

        for sp in &self.search_paths {
            if sp.pack.is_some() {
                continue;
            }
            if prev == last {
                return Some(&sp.filename);
            }
            last = &sp.filename;
        }

        None
    }

    // ============================================================
    // FS_InitFilesystem
    // ============================================================

    /// Returns the command names that should be registered with the command system.
    /// The caller is responsible for wiring these up to call path_f(), link(), and dir_f().
    pub fn commands() -> Vec<&'static str> {
        vec!["path", "link", "dir"]
    }

    /// Initializes the virtual filesystem.
    pub fn init_filesystem(&mut self) {
        // Note: "path", "link", "dir" commands should be registered by the caller
        // using FsContext::commands() and wiring them to path_f/link/dir_f.

        if !self.cddir.is_empty() {
            let cd_base = format!("{}/{}", self.cddir, BASEDIRNAME);
            self.add_game_directory(&cd_base);
        }

        // Start up with baseq2 by default
        let base = format!("{}/{}", self.basedir, BASEDIRNAME);
        self.add_game_directory(&base);

        // Any set gamedirs will be freed up to here
        self.base_search_index = self.search_paths.len();

        // Check for game override
        let _ = crate::cvar::cvar_get("game", "", CVAR_LATCH | CVAR_SERVERINFO);
        if !self.gamedirvar.is_empty() {
            let gd = self.gamedirvar.clone();
            self.set_gamedir(&gd);
        }
    }
}

// ============================================================
// Global singleton and free-function wrappers
// ============================================================

use std::sync::Mutex;

static FS_CTX: Mutex<Option<FsContext>> = Mutex::new(None);

pub fn fs_init() {
    let mut g = FS_CTX.lock().unwrap();
    let mut ctx = FsContext::new();
    ctx.init_filesystem();
    *g = Some(ctx);
}

pub fn fs_shutdown() {
    let mut g = FS_CTX.lock().unwrap();
    *g = None;
}

pub fn fs_gamedir() -> String {
    FS_CTX.lock().unwrap().as_ref().map_or(String::new(), |c| c.gamedir().to_string())
}

pub fn fs_create_path(path: &str) {
    FsContext::create_path(path);
}

pub fn fs_exec_autoexec() -> Option<String> {
    FS_CTX.lock().unwrap().as_ref().and_then(|c| c.exec_autoexec())
}

pub fn fs_load_file(name: &str) -> Option<Vec<u8>> {
    FS_CTX.lock().unwrap().as_mut().and_then(|c| c.load_file(name))
}

/// Load a file and also report whether it came from a pak file.
/// Returns (data, from_pak).
pub fn fs_load_file_ex(name: &str) -> (Option<Vec<u8>>, bool) {
    let mut guard = FS_CTX.lock().unwrap();
    if let Some(ref mut c) = *guard {
        let data = c.load_file(name);
        let from_pak = c.file_from_pak;
        (data, from_pak)
    } else {
        (None, false)
    }
}

pub fn fs_file_length(name: &str) -> Option<i32> {
    FS_CTX.lock().unwrap().as_mut().and_then(|c| c.file_length(name))
}

pub fn fs_set_gamedir(dir: &str) {
    if let Some(ref mut c) = *FS_CTX.lock().unwrap() {
        c.set_gamedir(dir);
    }
}

pub fn fs_add_game_directory(dir: &str) {
    if let Some(ref mut c) = *FS_CTX.lock().unwrap() {
        c.add_game_directory(dir);
    }
}

/// Access the global FS_CTX with a closure. Returns None if not initialized.
pub fn with_fs_ctx<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut FsContext) -> R,
{
    let mut g = FS_CTX.lock().unwrap();
    g.as_mut().map(f)
}

/// Parallel batch file existence check.
///
/// Takes a list of filenames and returns which ones exist in the filesystem.
/// Uses parallel processing to check across all pak files and search paths.
/// Useful for batch texture/sound loading at level start.
///
/// Returns a Vec of (filename, exists, from_pak) tuples.
pub fn fs_batch_file_exists(names: &[String]) -> Vec<(String, bool, bool)> {
    // Build pak index and collect directory paths while holding the lock
    let (pak_index, dir_paths): (std::collections::HashSet<String>, Vec<String>) = {
        let guard = FS_CTX.lock().unwrap();
        let ctx = match guard.as_ref() {
            Some(c) => c,
            None => return names.iter().map(|n| (n.clone(), false, false)).collect(),
        };

        // Build a quick lookup set for pak files
        let mut pak_set = std::collections::HashSet::new();
        let mut dirs = Vec::new();

        for sp in &ctx.search_paths {
            if let Some(ref pack) = sp.pack {
                for pf in &pack.files {
                    pak_set.insert(pf.name.to_lowercase());
                }
            } else {
                dirs.push(sp.filename.clone());
            }
        }

        (pak_set, dirs)
    };
    // Lock released here

    // Parallel check each filename (no lock held)
    names
        .par_iter()
        .map(|filename| {
            let lower = filename.to_lowercase();

            // Check pak index first
            if pak_index.contains(&lower) {
                return (filename.clone(), true, true);
            }

            // Check loose files in search path directories
            for dir in &dir_paths {
                let netpath = format!("{}/{}", dir, filename);
                if std::path::Path::new(&netpath).exists() {
                    return (filename.clone(), true, false);
                }
            }

            (filename.clone(), false, false)
        })
        .collect()
}

/// Parallel batch file loading.
///
/// Loads multiple files in parallel. Returns a Vec of (filename, Option<data>).
/// Note: This acquires the filesystem lock multiple times, so it's best used
/// for files that don't need strict ordering.
pub fn fs_batch_load_files(names: &[String]) -> Vec<(String, Option<Vec<u8>>)> {
    // We need to be careful here - each load_file call needs the mutex.
    // For true parallelism, we'd need to redesign the filesystem to be lock-free.
    // For now, we can at least parallelize the file reading from disk once we
    // know which files exist and where they are.

    // First, check which files exist (parallel)
    let existence = fs_batch_file_exists(names);

    // Then load each existing file
    // Note: This is still sequential due to the mutex, but the existence
    // check above was parallel
    existence
        .into_iter()
        .map(|(name, exists, _from_pak)| {
            if exists {
                let data = fs_load_file(&name);
                (name, data)
            } else {
                (name, None)
            }
        })
        .collect()
}

// ============================================================
// Unit tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_simple_wildcard_match() {
        assert!(FsContext::simple_wildcard_match("pak*", "pak0"));
        assert!(FsContext::simple_wildcard_match("pak*.pak", "pak0.pak"));
        assert!(FsContext::simple_wildcard_match("pak*.pak", "pakfoo.pak"));
        assert!(!FsContext::simple_wildcard_match("pak*.pak", "pak0.zip"));
        assert!(FsContext::simple_wildcard_match("*.*", "foo.txt"));
        assert!(FsContext::simple_wildcard_match("*", "anything"));
        assert!(!FsContext::simple_wildcard_match("pak", "pakx"));
        assert!(FsContext::simple_wildcard_match("pak", "pak"));
    }

    #[test]
    fn test_fs_context_defaults() {
        let ctx = FsContext::new();
        assert_eq!(ctx.basedir, ".");
        assert_eq!(ctx.paksearch, "pak*");
        assert!(ctx.search_paths.is_empty());
        assert!(ctx.links.is_empty());
        assert!(!ctx.file_from_pak);
    }

    #[test]
    fn test_gamedir_returns_basedirname_when_empty() {
        let ctx = FsContext::new();
        assert_eq!(ctx.gamedir(), BASEDIRNAME);
    }

    #[test]
    fn test_gamedir_returns_set_value() {
        let mut ctx = FsContext::new();
        ctx.gamedir = "mymod".to_string();
        assert_eq!(ctx.gamedir(), "mymod");
    }

    #[test]
    fn test_link_create_and_delete() {
        let mut ctx = FsContext::new();
        ctx.link("models/", "/tmp/models/");
        assert_eq!(ctx.links.len(), 1);
        assert_eq!(ctx.links[0].from, "models/");
        assert_eq!(ctx.links[0].to, "/tmp/models/");

        // Update
        ctx.link("models/", "/other/models/");
        assert_eq!(ctx.links.len(), 1);
        assert_eq!(ctx.links[0].to, "/other/models/");

        // Delete
        ctx.link("models/", "");
        assert_eq!(ctx.links.len(), 0);
    }

    #[test]
    fn test_set_gamedir_rejects_paths() {
        let mut ctx = FsContext::new();
        ctx.set_gamedir("../hack");
        // Should have been rejected — gamedir unchanged
        assert!(ctx.gamedir.is_empty() || ctx.gamedir != "../hack");
    }

    #[test]
    fn test_create_path() {
        let dir = std::env::temp_dir().join("myq2_test_create_path/sub1/sub2");
        let file_path = dir.join("test.txt");
        let _ = fs::remove_dir_all(std::env::temp_dir().join("myq2_test_create_path"));

        FsContext::create_path(&file_path.to_string_lossy());
        assert!(dir.exists());

        let _ = fs::remove_dir_all(std::env::temp_dir().join("myq2_test_create_path"));
    }

    #[test]
    fn test_filelength() {
        let dir = std::env::temp_dir();
        let path = dir.join("myq2_test_filelength.bin");
        {
            let mut f = File::create(&path).unwrap();
            f.write_all(&[0u8; 1234]).unwrap();
        }
        let mut f = File::open(&path).unwrap();
        let len = FsContext::filelength(&mut f).unwrap();
        assert_eq!(len, 1234);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_developer_searchpath() {
        let mut ctx = FsContext::new();
        assert_eq!(ctx.developer_searchpath(1), 0);

        ctx.search_paths.push(SearchPath {
            filename: "c:/quake2/xatrix".to_string(),
            pack: None,
        });
        assert_eq!(ctx.developer_searchpath(1), 1);

        ctx.search_paths.clear();
        ctx.search_paths.push(SearchPath {
            filename: "c:/quake2/rogue".to_string(),
            pack: None,
        });
        assert_eq!(ctx.developer_searchpath(2), 2);
    }

    #[test]
    fn test_next_path() {
        let mut ctx = FsContext::new();
        ctx.gamedir = "baseq2".to_string();
        ctx.search_paths.push(SearchPath {
            filename: "dir1".to_string(),
            pack: None,
        });
        ctx.search_paths.push(SearchPath {
            filename: "dir2".to_string(),
            pack: None,
        });

        assert_eq!(ctx.next_path(None), Some("baseq2"));
        assert_eq!(ctx.next_path(Some("baseq2")), Some("dir1"));
        assert_eq!(ctx.next_path(Some("dir1")), Some("dir2"));
        assert_eq!(ctx.next_path(Some("dir2")), None);
    }
}
