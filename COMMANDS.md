# MyQ2 Rust — Console Command Reference

Complete list of all console commands. Open the console with `~` (tilde) and type any command.

---

## General

| Command | Description |
|---------|-------------|
| `alias` | Create or list command aliases. Accepts wildcard search. |
| `aliaslist` | List all defined aliases. Accepts wildcard search. |
| `cmdlist` | List all registered commands. Accepts wildcard search. |
| `echo` | Print text to the console. |
| `exec` | Execute a config file (.cfg extension optional). |
| `quit` | Exit the game. |
| `wait` | Wait one frame before executing the next command. |

## Client — Connection

| Command | Description |
|---------|-------------|
| `changing` | Server is changing maps; prepare client for level transition. |
| `cmd` | Forward a command string to the server. |
| `connect` | Connect to a server by address (e.g., `connect 192.168.1.1`). |
| `disconnect` | Disconnect from the current server. |
| `download` | Manually initiate a file download from the server. |
| `precache` | Load and cache all resources for the current map. |
| `reconnect` | Reconnect to the last server. |

## Client — Configuration

| Command | Description |
|---------|-------------|
| `cl_netstats` | Display network statistics (packet loss, latency, bandwidth). |
| `cl_smooth` | Display network smoothing system statistics. |
| `pingservers` | Ping all servers in the address book. |
| `rcon` | Send a remote console command to the server. |
| `savecfg` | Save current configuration to a .cfg file (filename optional). |
| `setenv` | Set an environment variable. |
| `skins` | Force a reload of all player model skins. |
| `snd_restart` | Restart the sound subsystem. |
| `userinfo` | Display current userinfo string. |

## Client — Game Actions (forwarded to server)

| Command | Description |
|---------|-------------|
| `drop` | Drop the current item. |
| `give` | Give yourself an item (cheat). |
| `god` | Toggle god mode (cheat). |
| `info` | Request server information. |
| `inven` | Open the inventory screen. |
| `invdrop` | Drop the selected inventory item. |
| `invnext` | Select the next inventory item. |
| `invprev` | Select the previous inventory item. |
| `invuse` | Use the selected inventory item. |
| `kill` | Kill yourself (respawn). |
| `noclip` | Toggle noclip mode (cheat). |
| `notarget` | Toggle notarget mode (cheat). |
| `prog` | Display game DLL information. |
| `say` | Send a chat message to all players. |
| `say_team` | Send a chat message to your team. |
| `use` | Use/equip a named item (e.g., `use shotgun`). |
| `wave` | Perform a wave gesture (0-4). |
| `weapnext` | Switch to the next weapon. |
| `weapprev` | Switch to the previous weapon. |

## Client — Input (bind to keys)

These are movement/action commands. Prefix `+` starts the action, `-` stops it (released automatically when key is released).

| Command | Description |
|---------|-------------|
| `+attack` | Fire weapon |
| `+back` | Move backward |
| `+forward` | Move forward |
| `+klook` | Keyboard look mode |
| `+left` | Turn left |
| `+lookdown` | Look down |
| `+lookup` | Look up |
| `+mlook` | Mouse look mode |
| `+moveleft` | Strafe left |
| `+moveright` | Strafe right |
| `+moveup` | Jump |
| `+movedown` | Crouch |
| `+right` | Turn right |
| `+speed` | Run (hold) |
| `+strafe` | Strafe mode (hold) |
| `+use` | Use/interact |
| `centerview` | Center the view |
| `impulse` | Send an impulse command |

## Demo Playback

| Command | Description |
|---------|-------------|
| `demo_info` | Display information about the currently playing demo. |
| `demo_pause` | Toggle demo pause. |
| `demo_speed` | Set demo playback speed (0.25 - 4.0). |
| `playdemo` | Play a demo file (.dm2). |
| `record` | Start recording a demo (auto-names with timestamp if no argument). |
| `record_from_demo` | Re-record while watching a demo. |
| `seek` | Seek to a time position in the current demo (in seconds). |
| `seekpercent` | Seek to a percentage position in the current demo (0-100). |
| `stop` | Stop recording/playing a demo. |

## Location System

| Command | Description |
|---------|-------------|
| `loc` | Show the current location name (based on loaded .loc file). |
| `locadd` | Add a new location point at the current position. |
| `locdel` | Delete the nearest location point. |
| `loclist` | List all loaded location points. |
| `locsave` | Save location points to a .loc file. |

## Chat Enhancements

| Command | Description |
|---------|-------------|
| `filter_reload` | Reload the chat word filter (filter.txt). |
| `ignore` | Ignore chat messages from a player (e.g., `ignore PlayerName`). |
| `ignorelist` | List all ignored players. |
| `unignore` | Stop ignoring a player. |

## Crosshair & HUD

| Command | Description |
|---------|-------------|
| `crosshair_info` | Display current crosshair settings. |
| `hud_info` | Display current HUD settings. |
| `hud_reset_speed` | Reset the speed meter peak value. |
| `timer_start` | Start the match timer. |
| `timer_stop` | Stop the match timer. |

## Server Browser

| Command | Description |
|---------|-------------|
| `addfavorite` | Add the current or specified server to favorites. |
| `addserver` | Manually add a server by address. |
| `browser_clear` | Clear the server list. |
| `browser_filter` | Set server list filter criteria. |
| `browser_info` | Display server browser statistics. |
| `browser_sort` | Set server list sort column. |
| `refreshservers` | Query master servers and refresh the server list. |
| `serverlist` | Display the current server list. |

## Console

| Command | Description |
|---------|-------------|
| `clear` | Clear the console text buffer. |
| `condump` | Dump console contents to a .txt file (extension optional). |
| `messagemode` | Open chat input (say). |
| `messagemode2` | Open team chat input (say_team). |
| `messagemode3` | Open targeted chat input (tell — OSP-specific). |
| `messagemode4` | Open person chat input (say_person — Q2Admin-specific). |
| `togglechat` | Toggle the chat input overlay. |
| `toggleconsole` | Toggle the console open/closed. |

## Key Bindings

| Command | Description |
|---------|-------------|
| `bind` | Bind a key to a command (e.g., `bind mouse1 +attack`). Lists binds if no args. |
| `bindlist` | List all key bindings (including empty slots). Accepts wildcard search. |
| `unbind` | Unbind a key. |
| `unbindall` | Remove all key bindings. |

## Screen

| Command | Description |
|---------|-------------|
| `loading` | Display the loading screen. |
| `pause` | Toggle game pause. |
| `sizedown` | Decrease the view size. |
| `sizeup` | Increase the view size. |
| `sky` | Set the sky environment map (e.g., `sky night`). |
| `timerefresh` | Benchmark: render 128 frames rotating the view and report FPS. |

## View

| Command | Description |
|---------|-------------|
| `gun_model` | Set the weapon model path for testing. |
| `gun_next` | Cycle to the next weapon frame. |
| `gun_prev` | Cycle to the previous weapon frame. |
| `viewpos` | Display the current camera position and angles. |

## Renderer

| Command | Description |
|---------|-------------|
| `imagelist` | List all loaded textures with sizes and formats. |
| `modellist` | List all loaded models. |
| `screenshot` | Take a screenshot (saved to screenshots/). |
| `vk_strings` | Display Vulkan device/driver information strings. |

## Video / Platform

| Command | Description |
|---------|-------------|
| `vid_front` | Bring the game window to the front. |
| `vid_restart` | Restart the video subsystem (apply resolution/mode changes). |

## Server

| Command | Description |
|---------|-------------|
| `demomap` | Play a server-side demo. |
| `dumpuser` | Dump the userinfo for a client by slot number. |
| `gamemap` | Change the map (preserves game state across levels). |
| `heartbeat` | Send a heartbeat to the master server. |
| `kick` | Kick a client by slot number. |
| `killserver` | Shut down the server. |
| `load` | Load a saved game. |
| `map` | Change to a new map (resets game state). |
| `save` | Save the current game. |
| `serverinfo` | Display server configuration variables. |
| `serverrecord` | Start a server-side demo recording. |
| `serverstop` | Stop a server-side demo recording. |
| `setmaster` | Set the master server addresses. |
| `status` | Display server status (connected clients, addresses, pings). |
| `sv` | Server admin command prefix. |

## Menu

| Command | Description |
|---------|-------------|
| `menu_addressbook` | Open the address book menu. |
| `menu_credits` | Open the credits screen. |
| `menu_dmoptions` | Open the deathmatch options menu. |
| `menu_downloadoptions` | Open the download options menu. |
| `menu_game` | Open the single player menu. |
| `menu_joinserver` | Open the join server menu. |
| `menu_keys` | Open the key bindings menu. |
| `menu_loadgame` | Open the load game menu. |
| `menu_main` | Open the main menu. |
| `menu_multiplayer` | Open the multiplayer menu. |
| `menu_options` | Open the options menu. |
| `menu_playerconfig` | Open the player configuration menu. |
| `menu_quit` | Open the quit confirmation dialog. |
| `menu_savegame` | Open the save game menu. |
| `menu_startserver` | Open the start server menu. |
| `menu_video` | Open the video settings menu. |
