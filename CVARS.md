# MyQ2 Rust — Cvar Reference

Complete list of all configuration variables (cvars). Set from the console with `cvarname value`, or in config files.

**Flag legend:**
- **ARCHIVE** — Saved to config.cfg automatically
- **NOSET** — Read-only, cannot be changed after initialization
- **USERINFO** — Sent to the server as part of the player's userinfo string
- **SERVERINFO** — Included in server status queries
- **LATCH** — Change takes effect on next map load

---

## General / Engine

| Cvar | Default | Flags | Description |
|------|---------|-------|-------------|
| `dedicated` | `0` | NOSET | Set to 1 when running as a dedicated server |
| `developer` | `0` | — | Enable developer debug messages (com_dprintf output) |
| `game` | _(empty)_ | LATCH, SERVERINFO | Active game directory (mod name) |
| `noudp` | `0` | NOSET | Disable UDP networking |
| `qport` | _(random)_ | NOSET | Random port for NAT traversal in connect strings |
| `version` | _(auto)_ | SERVERINFO, NOSET | Engine version string |

## Client — Player Identity

| Cvar | Default | Flags | Description |
|------|---------|-------|-------------|
| `fov` | `90` | USERINFO, ARCHIVE | Field of view in degrees |
| `gender` | `male` | USERINFO, ARCHIVE | Player gender (affects pain sounds) |
| `gender_auto` | `1` | ARCHIVE | Automatically set gender based on model |
| `hand` | `2` | USERINFO, ARCHIVE | Weapon hand: 0=right, 1=left, 2=center |
| `msg` | `1` | USERINFO, ARCHIVE | Message level filter |
| `name` | `Player` | USERINFO, ARCHIVE | Player name |
| `password` | _(empty)_ | USERINFO | Server password |
| `rate` | `25000` | USERINFO, ARCHIVE | Network rate in bytes/sec |
| `skin` | `male/grunt` | USERINFO, ARCHIVE | Player model/skin |
| `spectator` | `0` | USERINFO | Spectator mode |

## Client — Gameplay

| Cvar | Default | Flags | Description |
|------|---------|-------|-------------|
| `cl_add_blend` / `cl_blend` | `1` | ARCHIVE | Enable screen blend effects (damage flash, underwater tint) |
| `cl_add_entities` / `cl_entities` | `1` | ARCHIVE | Enable entity rendering |
| `cl_add_lights` / `cl_lights` | `1` | ARCHIVE | Enable dynamic light rendering |
| `cl_add_particles` / `cl_particles` | `1` | ARCHIVE | Enable particle effects |
| `cl_defaultskin` | `male/grunt` | ARCHIVE | Default skin for players with missing skins |
| `cl_footsteps` | `1` | ARCHIVE | Enable footstep sounds |
| `cl_gun` | `1` | ARCHIVE | Show the weapon model (1=on, 0=off) |
| `cl_noskins` | `0` | ARCHIVE | Force all players to use `cl_defaultskin` |
| `cl_predict` | `1` | ARCHIVE | Enable client-side movement prediction |
| `cl_vwep` | `1` | ARCHIVE | Enable visible weapon models on other players |

## Client — Timing & Network

| Cvar | Default | Flags | Description |
|------|---------|-------|-------------|
| `cl_async` | `1` | ARCHIVE | Decouple render FPS from network packet rate |
| `cl_maxfps` | `90` | ARCHIVE | Maximum client frame rate |
| `cl_maxpackets` | `30` | ARCHIVE | Maximum network packets per second (when cl_async=1) |
| `cl_packetdup` | `0` | ARCHIVE | Duplicate outgoing packets (0-2) for lossy connections |
| `cl_timeout` | `120` | — | Seconds before disconnecting an unresponsive server |
| `paused` | `0` | — | Game pause state |
| `r_maxfps` | `0` | ARCHIVE | Maximum render FPS (0=unlimited, when cl_async=1) |
| `timedemo` | `0` | — | Benchmark mode: render as fast as possible |

## Client — Mouse & Look

| Cvar | Default | Flags | Description |
|------|---------|-------|-------------|
| `freelook` | `1` | ARCHIVE | Enable mouse look |
| `lookspring` | `0` | ARCHIVE | Auto-center view when freelook is off |
| `lookstrafe` | `0` | ARCHIVE | Mouse strafes instead of turns when +strafe is held |
| `m_forward` | `1` | ARCHIVE | Mouse forward/back sensitivity scale |
| `m_pitch` | `0.022` | ARCHIVE | Mouse pitch (up/down) sensitivity |
| `m_side` | `1` | ARCHIVE | Mouse strafe sensitivity scale |
| `m_yaw` | `0.022` | ARCHIVE | Mouse yaw (left/right) sensitivity |
| `sensitivity` | `5` | ARCHIVE | Overall mouse sensitivity |

## Client — Network Smoothing

| Cvar | Default | Flags | Description |
|------|---------|-------|-------------|
| `cl_adaptive_interp` | `1` | ARCHIVE | Adaptive interpolation buffer based on jitter |
| `cl_anim_continue` | `1` | ARCHIVE | Continue entity animations during packet loss |
| `cl_cubic_interp` | `1` | ARCHIVE | Use Catmull-Rom spline interpolation for entities |
| `cl_extrapolate` | `1` | ARCHIVE | Extrapolate entity positions beyond last server update |
| `cl_extrapolate_max` | `50` | ARCHIVE | Maximum extrapolation time in milliseconds |
| `cl_projectile_predict` | `1` | ARCHIVE | Predict projectile positions client-side |
| `cl_timenudge` | `0` | ARCHIVE | Adjust interpolation timing (negative=ahead, positive=behind) |
| `cl_view_smooth` | `1` | ARCHIVE | Smooth view angle changes |

## Client — Strafe Jumping

| Cvar | Default | Flags | Description |
|------|---------|-------|-------------|
| `cl_physics_fps` | `125` | ARCHIVE | Reference FPS for strafe jump normalization |
| `cl_strafejump_fix` | `1` | ARCHIVE | Normalize strafe jump gains across different FPS |

## Client — Auto-Reconnect

| Cvar | Default | Flags | Description |
|------|---------|-------|-------------|
| `cl_autoreconnect` | `0` | ARCHIVE | Enable automatic reconnection on timeout |
| `cl_autoreconnect_delay` | `3000` | ARCHIVE | Delay between reconnect attempts (ms) |
| `cl_autoreconnect_max` | `3` | ARCHIVE | Maximum reconnect attempts |

## Client — Map/Disconnect Hooks

| Cvar | Default | Flags | Description |
|------|---------|-------|-------------|
| `cl_beginmapcmd` | _(empty)_ | ARCHIVE | Command to execute when a new map begins |
| `cl_changemapcmd` | _(empty)_ | ARCHIVE | Command to execute on map change |
| `cl_disconnectcmd` | _(empty)_ | ARCHIVE | Command to execute on disconnect |

## Client — Demo Recording

| Cvar | Default | Flags | Description |
|------|---------|-------|-------------|
| `cl_autorecord` | `0` | ARCHIVE | Automatically record demos on map enter |

## Client — HTTP Downloads

| Cvar | Default | Flags | Description |
|------|---------|-------|-------------|
| `cl_http_downloads` | `1` | ARCHIVE | Enable HTTP downloads from servers |

## Client — Chat

| Cvar | Default | Flags | Description |
|------|---------|-------|-------------|
| `cl_chat_log` | `0` | ARCHIVE | Log chat messages to logs/chat-YYYY-MM-DD.log |
| `cl_filter_chat` | `1` | ARCHIVE | Enable chat word filter (loads filter.txt) |

## Client — Debug

| Cvar | Default | Flags | Description |
|------|---------|-------|-------------|
| `cl_showmiss` | `0` | — | Show prediction miss debugging info |
| `cl_shownet` | `0` | — | Show network packet debugging info |
| `cl_stereo` | `0` | ARCHIVE | Enable stereo rendering |
| `cl_stereo_separation` | `0.4` | ARCHIVE | Stereo eye separation distance |
| `cl_testblend` | `0` | — | Test screen blend effect |
| `cl_testentities` | `0` | — | Test entity rendering |
| `cl_testlights` | `0` | — | Test dynamic lights |
| `cl_testparticles` | `0` | — | Test particle effects |
| `cl_stats` | `0` | — | Show frame timing statistics |
| `rcon_address` | _(empty)_ | — | Remote console server address |
| `rcon_password` | _(empty)_ | — | Remote console password |
| `showclamp` | `0` | — | Show time clamping debug info |

## Crosshair

| Cvar | Default | Flags | Description |
|------|---------|-------|-------------|
| `ch_health` | `0` | ARCHIVE | Color crosshair based on health (R1Q2/Q2Pro) |
| `crosshair` | `1` | ARCHIVE | Crosshair style (0=off, 1-5=procedural, 6+=image) |
| `crosshair_alpha` | `1.0` | ARCHIVE | Crosshair opacity (0.0-1.0) |
| `crosshair_color` | `240` | ARCHIVE | Crosshair color index (Q2 palette) |
| `crosshair_dynamic` | `0` | ARCHIVE | Dynamic crosshair expansion on movement/firing |
| `crosshair_gap` | `2` | ARCHIVE | Gap between crosshair lines (pixels) |
| `crosshair_size` | `1.0` | ARCHIVE | Crosshair scale factor |
| `crosshair_thickness` | `2` | ARCHIVE | Crosshair line thickness (pixels) |

## HUD

| Cvar | Default | Flags | Description |
|------|---------|-------|-------------|
| `hud_alpha` | `1.0` | ARCHIVE | HUD element opacity (0.0-1.0) |
| `hud_minimal` | `0` | ARCHIVE | Enable minimal HUD mode |
| `hud_scale` | `1.0` | ARCHIVE | HUD scale factor |
| `hud_show_ammo` | `1` | ARCHIVE | Show ammo counter |
| `hud_show_armor` | `1` | ARCHIVE | Show armor value |
| `hud_show_fps` | `0` | ARCHIVE | Show FPS counter |
| `hud_show_health` | `1` | ARCHIVE | Show health value |
| `hud_show_netstats` | `0` | ARCHIVE | Show network statistics overlay |
| `hud_show_speed` | `0` | ARCHIVE | Show speed meter (units/sec) |
| `hud_show_timer` | `0` | ARCHIVE | Show match timer |
| `hud_stat_smoothing` | `1` | ARCHIVE | Smooth HUD stat value changes |

## Screen

| Cvar | Default | Flags | Description |
|------|---------|-------|-------------|
| `debuggraph` | `0` | — | Show debug graph |
| `graphheight` | `32` | — | Debug/net graph height in pixels |
| `graphscale` | `1` | — | Debug/net graph vertical scale |
| `graphshift` | `0` | — | Debug/net graph horizontal shift |
| `netgraph` | `0` | — | Show network latency graph |
| `scr_centertime` | `2.5` | — | Duration of center-screen messages (seconds) |
| `scr_conspeed` | `3` | — | Console open/close animation speed |
| `scr_drawall` | `0` | — | Force redraw of all screen elements every frame |
| `scr_printspeed` | `8` | — | Typewriter-style text print speed |
| `scr_showpause` | `1` | — | Show "PAUSED" text when paused |
| `scr_showturtle` | `0` | — | Show turtle icon when frame rate drops |
| `timegraph` | `0` | — | Show frame timing graph |
| `viewsize` | `100` | ARCHIVE | View size as percentage of screen (30-120) |

## Sound

| Cvar | Default | Flags | Description |
|------|---------|-------|-------------|
| `s_khz` | `48` | ARCHIVE | Sound sample rate in KHz (11, 22, 44, 48, 96) |

## Renderer — Core

| Cvar | Default | Flags | Description |
|------|---------|-------|-------------|
| `r_celshading` | `0` | ARCHIVE | Enable cel shading effect |
| `r_caustics` | `1` | ARCHIVE | Enable underwater caustic texture overlay |
| `r_detailtexture` | `7` | ARCHIVE | Detail texture selection (0=off, 1-8=texture index) |
| `r_drawentities` | `1` | — | Draw entities (0=hide all entities) |
| `r_drawworld` | `1` | — | Draw the world/BSP (0=empty world) |
| `r_fog` | `0` | ARCHIVE | Enable fog effects (normal + underwater) |
| `r_fullbright` | `0` | — | Render all surfaces at full brightness (no lightmaps) |
| `r_hwgamma` | `0` | ARCHIVE | Enable hardware gamma ramp |
| `r_lightlevel` | `0` | — | Current surface light level (set by engine) |
| `r_nocull` | `0` | — | Disable frustum culling |
| `r_norefresh` | `0` | — | Disable screen refresh |
| `r_novis` | `0` | — | Disable PVS (render everything) |
| `r_overbrightbits` | `2` | ARCHIVE | Overbright lighting multiplier (0=off, 1-4) |
| `r_speeds` | `0` | — | Show renderer performance stats (poly counts, etc.) |
| `r_stainmap` | `1` | ARCHIVE | Enable stain maps (blood/explosion marks on walls) |
| `r_timebasedfx` | `1` | ARCHIVE | Time-of-day lighting effects |
| `r_verbose` | `0` | — | Verbose renderer output during initialization |
| `flushmap` | `0` | — | Force model cache flush on map load |
| `intensity` | `2` | — | Texture color intensity multiplier |

## Renderer — Post-Processing

| Cvar | Default | Flags | Description |
|------|---------|-------|-------------|
| `r_bloom` | `1` | ARCHIVE | Enable bloom post-process effect |
| `r_bloom_intensity` | `0.3` | ARCHIVE | Bloom effect strength |
| `r_bloom_threshold` | `0.8` | ARCHIVE | Brightness threshold for bloom |
| `r_fsr` | `1` | ARCHIVE | Enable AMD FSR upscaling |
| `r_fsr_scale` | `0.75` | ARCHIVE | FSR render scale (lower=more upscaling) |
| `r_fsr_sharpness` | `0.2` | ARCHIVE | FSR sharpening strength |
| `r_fxaa` | `1` | ARCHIVE | Enable FXAA anti-aliasing |
| `r_ssao` | `1` | ARCHIVE | Enable Screen Space Ambient Occlusion |
| `r_ssao_intensity` | `1.0` | ARCHIVE | SSAO effect strength |
| `r_ssao_radius` | `0.5` | ARCHIVE | SSAO sample radius |

## Renderer — Quality

| Cvar | Default | Flags | Description |
|------|---------|-------|-------------|
| `r_anisotropy` | `8` | ARCHIVE | Anisotropic filtering level (1-16, clamped to device max) |
| `r_msaa` | `0` | ARCHIVE | Multisample anti-aliasing (0, 2, 4, 8; clamped to device max) |

## Renderer — Vulkan

| Cvar | Default | Flags | Description |
|------|---------|-------|-------------|
| `vk_3dlabs_broken` | `0` | ARCHIVE | Workaround for 3DLabs GPU issues |
| `vk_clear` | `0` | — | Clear the framebuffer each frame |
| `vk_cull` | `1` | ARCHIVE | Enable backface culling |
| `vk_drawbuffer` | `VK_BACK` | ARCHIVE | Draw buffer selection |
| `vk_driver` | `opengl32` | ARCHIVE | Graphics driver name (legacy, unused in Vulkan) |
| `vk_dynamic` | `1` | ARCHIVE | Enable dynamic lights on world surfaces |
| `vk_ext_multitexture` | `1` | ARCHIVE | Enable multitexture extension |
| `vk_ext_texture_filter_anisotropic` | `1` | ARCHIVE | Enable anisotropic filtering extension |
| `vk_finish` | `0` | ARCHIVE | Call device finish after each frame |
| `vk_flashblend` | `0` | ARCHIVE | Use additive blending for dynamic lights (instead of lightmaps) |
| `vk_lightmap` | `0` | — | Debug: render lightmaps only (no diffuse textures) |
| `vk_lockpvs` | `0` | — | Lock the PVS to the current leaf (debug) |
| `vk_log` | `0` | — | Enable Vulkan validation/debug logging |
| `vk_mode` | `4` | ARCHIVE | Video mode index |
| `vk_modulate` | `1.5` | ARCHIVE | Lightmap brightness multiplier |
| `vk_monolightmap` | `0` | — | Force monochrome lightmaps |
| `vk_picmip` | `0` | ARCHIVE | Texture quality reduction (0=best, higher=lower quality) |
| `vk_polyblend` | `1` | ARCHIVE | Enable fullscreen color blends (underwater, damage) |
| `vk_saturatelighting` | `0` | ARCHIVE | Clamp lightmap values to prevent overbright |
| `vk_screenshot_format` | `tga` | ARCHIVE | Screenshot format: tga, png, or jpg |
| `vk_screenshot_quality` | `85` | ARCHIVE | JPEG screenshot quality (1-100) |
| `vk_sgis_generate_mipmap` | `0` | ARCHIVE | Use hardware mipmap generation |
| `vk_shadows` | `1` | ARCHIVE | Enable stencil shadows |
| `vk_showtris` | `0` | — | Debug: render wireframe triangles |
| `vk_skymip` | `0` | ARCHIVE | Sky texture quality reduction |
| `vk_swapinterval` | `1` | ARCHIVE | VSync: 0=off, 1=on |
| `vk_texturealphamode` | `default` | — | Alpha texture blend mode |
| `vk_texturemode` | `VK_LINEAR_MIPMAP_LINEAR` | ARCHIVE | Texture filtering mode |
| `vk_texturesolidmode` | `default` | — | Solid texture blend mode |
| `vk_ztrick` | `0` | ARCHIVE | Z-buffer optimization trick |

## Video

| Cvar | Default | Flags | Description |
|------|---------|-------|-------------|
| `vid_fullscreen` | `1` | ARCHIVE | Fullscreen mode (0=windowed, 1=fullscreen) |
| `vid_gamma` | `0.6` | ARCHIVE | Display gamma correction (lower=brighter) |
| `vid_ref` | `gl` | ARCHIVE | Renderer reference (legacy, always Vulkan now) |

## Server

| Cvar | Default | Flags | Description |
|------|---------|-------|-------------|
| `sv_projectiles` | `1` | — | Enable server-side projectile entities |
