# MyQ2 Rust

> **Alpha Software** — This project is under active development and not yet feature-complete. Expect bugs, missing functionality, and general unusability.

A complete rewrite of [MyQ2](myq2-original/readme.txt) (a Quake II engine fork by Matt Smith) in Rust, featuring a modern Vulkan renderer, R1Q2/Q2Pro-inspired client enhancements, and extensive parallelization.

Based on id Software's Quake II engine (v3.21), licensed under the GPL.

## Features

### Vulkan Renderer (replaces OpenGL)
- **Modern rendering pipeline** with Vulkan backend via `ash`
- **MSAA** anti-aliasing (2x, 4x, 8x) via `r_msaa`
- **Anisotropic filtering** (1x-16x) via `r_anisotropy`
- **FXAA** post-process anti-aliasing via `r_fxaa`
- **SSAO** (Screen Space Ambient Occlusion) via `r_ssao`
- **Bloom** post-process effect via `r_bloom`
- **FSR** (FidelityFX Super Resolution) upscaling via `r_fsr`
- **Overbright rendering** via `r_overbrightbits`
- **Detail textures** and **water caustics** via `r_detailtexture`, `r_caustics`
- **Stain maps** via `r_stainmap`
- **Cel shading** via `r_celshading`
- **Fog** (normal and underwater) via `r_fog`
- **Time-based lighting effects** via `r_timebasedfx`
- **Dynamic lights** toggle via `vk_dynamic`
- **Stencil shadows** via `vk_shadows`
- **Screenshot support** (TGA/PNG/JPG) via `screenshot` command
- **Configurable texture filtering** and **swap interval** (vsync)

### R1Q2/Q2Pro Client Enhancements
- **Demo seeking** — seek to any point in .dm2 demos with keyframe indexing
  - Commands: `playdemo`, `seek`, `seekpercent`, `demo_pause`, `demo_speed`, `demo_info`
  - Variable speed playback (0.25x - 4.0x)
- **Demo recording** — auto-naming with timestamps, `cl_autorecord` for automatic recording
- **Location system** — load .loc files, `$loc_here` chat macro expansion
- **Auto-reconnect** — automatic reconnection on timeout with exponential backoff
- **Packet duplication** — send duplicate packets for lossy connections (WiFi, satellite)
- **FPS-independent strafe jumping** — consistent strafe jump gains at any FPS
- **Async client** — decouple render FPS from network packet rate
  - `cl_async` enables independent timing (1=decoupled, 0=legacy)
  - `r_maxfps` controls render FPS (0=unlimited/vsync)
  - `cl_maxpackets` controls network packet rate (default: 30/sec)
  - Allows high FPS rendering while maintaining optimal network timing
- **Chat enhancements** — word filter, ignore list, chat logging
- **Health-based crosshair** — crosshair color changes based on player health
  - Enabled via `ch_health` cvar (0=off, 1=on)
  - Color smoothly transitions from green (full health) to red (low health)
  - Works with all crosshair styles (1-5)
- **Weapon fire prediction** — immediate visual feedback before server confirmation
  - Instant muzzle flash on attack button press
  - Supports: MuzzleFlash, Tracer, BulletImpact, RocketTrail, RailTrail effects
  - Confirmed when server sends SVC_MuzzleFlash
  - Eliminates perceived input lag on weapon firing

### Network Smoothing System
- **Adaptive interpolation** — adjusts buffer size based on network jitter
  - Records packet arrivals, calculates jitter statistics
  - Target buffer auto-adjusts between 50-200ms
- **Dead reckoning** — predicts player positions between server updates
  - Uses velocity + acceleration for prediction
  - Confidence decays over time (2x per second)
  - High confidence (>0.5): pure prediction; Medium: blended with last known position
- **Snapshot-based interpolation** — uses buffered snapshots for smoother lerpfrac
  - Records server snapshots with arrival times
  - Blends 70% snapshot-based + 30% standard timing for stability
- **Input buffering** — smooths local movement prediction
  - Buffers 2 frames of input commands
  - Weighted average blending (newer commands = higher weight)
- **Spline interpolation** — Catmull-Rom curves for smooth entity movement
  - Requires 4+ position samples
  - Falls back to linear when insufficient data
- **Prediction error smoothing** — smooth server corrections over 100ms
  - Prevents jarring position snaps on misprediction
- **Frame time smoothing** — reduces jitter from variable frame rates
  - Weighted average of 8 recent frame times
- **Effect continuation** — continues rendering effects during packet loss
  - Registers significant effects (explosions, blood, debris)
  - Auto-cleanup after timeout

### HUD Customization
- Configurable HUD elements: health, armor, ammo, timer, FPS counter, speed meter, network stats
- HUD scaling and alpha via `hud_scale`, `hud_alpha`
- Minimal HUD mode via `hud_minimal`

### Crosshair Customization
- **5 procedural styles:**
  1. **Cross** — Traditional + crosshair
  2. **Dot** — Center dot only
  3. **Circle** — Circular crosshair
  4. **CrossDot** — Cross + center dot
  5. **XShape** — Diagonal X crosshair
  - Styles 6+ use image files
- Configurable size, color, alpha, gap, thickness
- Dynamic expansion on movement/firing via `crosshair_dynamic`
- Health-based color via `ch_health`

### Server Browser
- Master server queries and LAN broadcast discovery
- Sorting by name, map, players, ping
- Filtering by name/map/ping/empty/full
- Favorites list saved to disk

### HTTP Downloads
- Async HTTP downloads via `tokio` — game continues while downloading
- Progress polling, non-blocking I/O

### File Formats & Configuration

#### Location Files (.loc)
- Store map location names for chat macros (`$loc_here`)
- Format: `locs/<mapname>.loc`
- Commands: `locadd`, `locdel`, `locsave`, `loclist`
- Auto-loaded on map change

#### Chat Filter
- Word filter: `filter.txt` in base directory
- Format: One word per line (case-insensitive)
- Filtered words replaced with asterisks
- Reload: `filter_reload` command

#### Chat Logging
- Enabled via `cl_chat_log` cvar
- Format: `logs/chat-YYYY-MM-DD.log`
- Logs all chat messages with timestamps

#### Favorites List
- Server browser favorites saved to `favorites.txt`
- One server address per line
- Commands: `addfavorite`, `browser_clear`

### Performance Optimizations

#### CPU Parallelization (via rayon)
Multi-threaded parallelization across 15 subsystems with tuned thresholds:
- **PVS/PHS merging** (threshold: 64 longs) — parallel bitwise OR for visibility data
- **Entity save serialization** (threshold: 32 entities) — parallel entity→buffer, sequential write
- **Entity visibility extraction** — data extraction pattern with EntityVisData struct
- **Radius damage** (threshold: 8 entities) — parallel damage calculations
- **Client ping calculation** (threshold: 8 clients) — parallel ping statistics
- **Client timeout checking** (threshold: 8 clients) — parallel connection monitoring
- **Sound channel updates** (threshold: 16 channels) — parallel audio processing
- **Particle physics** (threshold: 256 particles) — parallel particle simulation
- **WAV loading** — parallel audio file loading during registration
- **Client message sending** (threshold: 8 clients) — parallel message preparation
- **Edict initialization** — parallel initialization of 1024 edicts during level load
- **Client entity state init** — parallel init of MAX_EDICTS + MAX_PARSE_ENTITIES
- **BSP lump parsing** (threshold: 64 elements) — parallel parsing of surfaces, nodes, leafs, planes, brushes
- **Pack file entry parsing** (threshold: 64 files) — parallel .pak/.zip file reading
- **Directory wildcard matching** (threshold: 64 entries) — parallel file pattern matching

#### Async I/O
Non-blocking I/O for background operations:
- **HTTP downloads** via AsyncHttpDownloadManager with tokio
  - Non-blocking downloads via `cl_http_async_download()`
  - Progress polling via `cl_http_async_poll()`
  - Game continues running during downloads

#### GPU Parallelization (Vulkan)
Batching and parallel execution on the GPU:
- **Batched GPU uploads** via `flush_uploads()` (threshold: 4 uploads) — single staging buffer
- **Parallel command buffer recording** via `record_secondary_parallel()` (threshold: 4 buffers)
- **Lightmap batch upload** — parallel CPU prep, sequential GPU upload
- **Batch descriptor updates** — single `update_descriptor_sets()` call for multiple descriptors
- **Parallel shader loading** via `load_shaders_parallel()` (threshold: 4 shaders)
- **Deferred pipeline creation** — queue via `queue_pipeline()`, flush via `flush_pending_pipelines()`
- **VBO/IBO staging uploads** — staging buffer pattern for vertex/index buffers
- **Render command batching** — RenderCommandQueue with SurfaceBatch for texture sorting

#### O(1) Lookup Optimizations
HashMap-based lookups instead of linear scans:
- **Command lookup** ([cmd.rs](crates/myq2-common/src/cmd.rs)) — HashMap instead of Vec
- **Cvar lookup** ([cvar.rs](crates/myq2-common/src/cvar.rs)) — HashMap instead of Vec
- **Pack file lookup** ([files.rs](crates/myq2-common/src/files.rs)) — HashMap for instant pak/zip search
- **Item lookup** ([g_items.rs](crates/myq2-game/src/g_items.rs)) — HashMaps for classname and pickup name
- **FIELDS/SPAWNS lookup** ([g_spawn.rs](crates/myq2-game/src/g_spawn.rs)) — OnceLock HashMaps
- **Entity lookup** ([g_utils.rs](crates/myq2-game/src/g_utils.rs)) — HashMaps for targetname and classname
- **Team linking** ([g_spawn.rs](crates/myq2-game/src/g_spawn.rs)) — HashMap grouping for O(n) instead of O(n²)

## Building

Requires Rust 1.70+ and the Vulkan SDK.

```sh
cargo build           # debug build
cargo build --release # release build
cargo run             # run the engine
cargo test            # run all tests
cargo clippy          # lint
```

## Runtime Dependencies

### Required
- **Vulkan SDK** — Required for renderer
- **OpenAL** — Required for audio (OpenAL Soft recommended)

### Performance Libraries
- **rayon** — Multi-threaded parallelization for CPU-bound tasks
- **tokio** — Async I/O for HTTP downloads and background tasks
- **parking_lot** — High-performance mutexes
- **crossbeam** — Lock-free data structures for concurrent operations

### Rendering
- **ash** — Vulkan bindings for Rust
- **gpu-allocator** — Vulkan memory management

## Project Structure

```
Cargo.toml              # workspace root
myq2-original/          # original C source (read-only reference)
crates/
  myq2-common/          # qcommon: net, filesystem, cmd, cvar, shared types
  myq2-client/          # client module
  myq2-server/          # server module
  myq2-game/            # game logic (game DLL equivalent)
  myq2-game-dll/        # game DLL loader
  myq2-renderer/        # Vulkan renderer (replaces ref_gl)
  myq2-sys/             # platform layer
  openal-soft-sys/      # OpenAL audio backend bindings
```

## Reference Documentation

- [COMMANDS.md](COMMANDS.md) — Complete console command reference
- [CVARS.md](CVARS.md) — Complete cvar (configuration variable) reference

## Original Credits

MyQ2 by Matt Smith (mattx86), based on Quake II by id Software (John Carmack). See [myq2-original/readme.txt](myq2-original/readme.txt) for the original MyQ2 changelog and credits for community contributions (NiceAss, psychospaz, Echon, Evilpope, Vic, Riot, jitspoe, Carbon14, MrG, and many others).

## License

This source code is licensed under the [GNU General Public License v2](gnu.txt), the same license as the original Quake II source release. All Quake II data files remain copyrighted by id Software under their original terms.
