# MyQ2 Rust — Project Guide

## Goal

Full 1-to-1 conversion of MyQ2 (a Quake 2 engine fork) from C/C++ to Rust, with a modern Vulkan renderer replacing the original OpenGL backend. The original C source lives in `myq2-original/` and must be preserved unmodified as the reference implementation. The Rust port must reproduce the same behavior, data layouts, network protocol, and file format compatibility.

## Project Layout

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
  myq2-sys/             # platform layer (win32 cross-platform)
  openal-soft-sys/      # OpenAL audio backend bindings
```

### Original C Source (`myq2-original/`)

| Directory   | Purpose                                      |
|-------------|----------------------------------------------|
| `client/`   | Client-side logic: rendering, input, sound, menus, console |
| `server/`   | Dedicated and listen server                  |
| `game/`     | Game DLL: entities, AI, physics, weapons, monsters |
| `qcommon/`  | Shared engine code: filesystem, networking, command system, cvar |
| `ref_gl/`   | OpenGL renderer                              |
| `win32/`    | Win32 platform layer (window, input, sound, GL context) |

## Conversion Rules

- **1-to-1 fidelity**: Every C function gets a Rust equivalent. Do not skip, stub, or simplify game logic. Preserve original algorithms, constants, and magic numbers.
- **File mapping**: Each `.c` file should map to a corresponding `.rs` module. Keep the same logical grouping.
- **Data compatibility**: Structs that are serialized (save files, network packets, BSP data) must have identical binary layouts. Use `#[repr(C)]` where needed.
- **Global/static state**: Convert C globals to appropriate Rust patterns (e.g., module-level statics behind `Mutex`, or passed-through context structs). Prefer context structs.
- **Unsafe**: Use `unsafe` only where strictly required (FFI, raw pointer math for BSP traversal, etc.). Document every `unsafe` block with a `// SAFETY:` comment.
- **Dependencies**: Prefer well-maintained crates for platform abstractions (e.g., `winit`, `ash`, `rodio`). Do not rewrite platform code from scratch where a crate suffices.
- **C string handling**: Use `CStr`/`CString` at boundaries. Internally prefer `&str`/`String`.

## Build & Run

```sh
cargo build           # debug build
cargo build --release # release build
cargo run             # run the engine
cargo test            # run all tests
cargo clippy          # lint
```

## Deduplication & Crate Usage

- **Deduplicate shared code**: If the same logic (e.g., math helpers, string utilities, parsing routines) appears in multiple crates, extract it into `myq2-common` or an appropriate shared module rather than duplicating it.
- **Use existing Rust crates** where they provide a 100% faithful replacement for C utility code, while keeping the engine's overall behavior identical. Examples: `byteorder` for endian conversions, `bitflags` for flag types, `glam` or inline math for vector/matrix ops (only if the results are bit-identical). Do not use crates that change game logic, algorithms, or data layouts.
- **Do not over-abstract**: Only deduplicate when the duplicated code is truly identical in purpose. Two functions that happen to look similar but serve different subsystems should remain separate if their evolution paths may diverge.

## Code Quality

- **Warnings are errors**: All compiler warnings must be treated as errors and fixed immediately. Do not leave warnings in the codebase. Use `#![deny(warnings)]` or fix all warnings before considering a crate complete.

## Workflow

1. Convert one module (`.c` file) at a time, starting from the leaf dependencies (`qcommon/`) and working up.
2. After converting each file, verify it compiles and passes any applicable tests.
3. Do not modify anything under `myq2-original/`.

## Documentation

- [README.md](README.md) — Project overview and features
- [COMMANDS.md](COMMANDS.md) — Complete console command reference
- [CVARS.md](CVARS.md) — Complete cvar (configuration variable) reference
