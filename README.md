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
- **Async client** — decouple render FPS from network packet rate via `cl_async`
- **Chat enhancements** — word filter, ignore list, chat logging
- **Health-based crosshair** — crosshair color changes based on health via `ch_health`

### Network Smoothing System
- **Adaptive interpolation** — adjusts buffer size based on network jitter
- **Dead reckoning** — predicts player positions between server updates
- **Weapon fire prediction** — immediate muzzle flash before server confirmation
- **Input buffering** — smooths local movement prediction
- **Spline interpolation** — Catmull-Rom curves for smooth entity movement
- **Prediction error smoothing** — smooth server corrections over 100ms
- **Frame time smoothing** — reduces jitter from variable frame rates
- **Effect continuation** — continues rendering effects during packet loss

### HUD Customization
- Configurable HUD elements: health, armor, ammo, timer, FPS counter, speed meter, network stats
- HUD scaling and alpha via `hud_scale`, `hud_alpha`
- Minimal HUD mode via `hud_minimal`

### Crosshair Customization
- 5 procedural styles: Cross, Dot, Circle, CrossDot, XShape
- Configurable size, color, alpha, gap, thickness
- Dynamic expansion on movement/firing

### Server Browser
- Master server queries and LAN broadcast discovery
- Sorting by name, map, players, ping
- Filtering by name/map/ping/empty/full
- Favorites list saved to disk

### HTTP Downloads
- Async HTTP downloads via `tokio` — game continues while downloading
- Progress polling, non-blocking I/O

### Performance Optimizations
- **Parallelization** via `rayon` across 15+ subsystems (PVS merging, entity saves, particle physics, BSP parsing, etc.)
- **O(1) lookups** via HashMap for commands, cvars, pack files, items, entities, fields, spawns
- **Batched GPU uploads** and **parallel command buffer recording** in the Vulkan renderer
- **Deferred pipeline creation** and **parallel shader loading**

## Building

Requires Rust 1.70+ and the Vulkan SDK.

```sh
cargo build           # debug build
cargo build --release # release build
cargo run             # run the engine
cargo test            # run all tests
cargo clippy          # lint
```

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
