=====================
         MyQ2
    By Matt Smith
=====================

MyQ2 is a clean, simple, and compatible, yet customizable, Win32/OpenGL Quake II engine.


v0.01 Changes
~~~~~~~~~~~~~
- Merged ref_gl.dll into the Quake II EXE, and removed everything that had to do with the Software DLL. (mattx86)
- Now tries to load PNGs, then TGAs for texture/pic replacement. (mattx86)
   - High resolution PNGs and TGAs get scaled down to the correct size. (NiceAss, Nexus, Idle)
- srand() is now called, to make random numbers "truely" random. (mattx86)
- Console is initialized earlier on, so you'll now see messages that before, didn't show up. (mattx86)
- Console's buffer size has been quadrupled.  Can now fit the text from say, 4 or 5 maps on a plain/OSP style DM game. (mattx86)
- Q2Msgs exploit fix (psychospaz)
- Automatic snd/vid_restart'ing code added.  You may want to combine multiple commands/cvar changes on one line from now on. (mattx86)
- Merged VID_Error() into VID_Printf(). (mattx86)
- All of the sound and video output to the console, now uses a special PRINT_INFO print_level. (mattx86)
   - If r_verbose is 0 - you don't see the video output, 1 - you'll see it just as before.
   - If s_verbose is 0 - you don't see the sound output, 1 - you'll see it just as before.
   - I don't care to see all of this myself, so I have them set to 0.  As well as, because of the automatic snd/vid_restart's, it can get spammy when you first start it up.
- Fixed the problem where trying to do 44khz would revert back to 11khz. (mattx86)
- Replaced all instances of Q_stricmp() with Q_strcasecmp(), for a speed-up. (jitspoe)
- Console editing like in notepad added. (pooy, Vic)
- Fixed the WSAECONNRESET problem. (jitspoe)
- Updated some of the WinSock stuff. (mattx86)
   - Now tries to initialize WinSock v2.2.
   - Error strings are now more detailed.
- "Bonus" items now bob, something like in Quake III. (Vic)
- Console can now be scrolled via the mouse wheel. (mattx86)
- Automatic NT 5.0+ (Win2k, XP and up on the 5.x series) fix for mouse acceleration. (NiceAss)
- Added dynamic light surface fix. (Discoloda, psychospaz)
- Tweaked the DLIGHT_CUTOFF so dynamic lights look a bit better. (Echon)
- Added taskbar fix, where switching resolutions on-the-fly caused an empty window to be left on the taskbar. (Tomaz)
- Added a fix for the player setup menu, where sometimes it would not display all of the models that are installed. (AnthonyJ)
- Added Quake III-style cvarlist update. (Riot)
- Console now works in demos. (NiceAss)
- Console is transparent. (NiceAss)
- Increased skybox size. (David Pochron)
- Added stencil shadows. (MrG, psychospaz, Echon)
- Changed overall color depth to 32bit. (psychospaz)
- Added a better dynamic light falloff update. (Riot)
- Pak files can now be .PAK or (uncompressed) .ZIP files. (Nanook, psychospaz, Evilpope, mattx86)
   - .PAK files load first, then .ZIP files.
   - Pak files to be loaded, are the ones that match the paksearch cvar.  "pak*" will match, for instance, "pak4_my_custom_pak.zip" and "pak0.pak"
- Added cel shading code.  Toggle it on/off (1/0) with the r_celshading cvar. (Discoloda, psychospaz, icculus.org)
- Updated rcon cmd so you can now set rcon_address if you're connected to a server already or not, and be able to control the server you've specified. (mattx86)
- Added r_displayrefresh cvar to fix the WinXP refresh rate problem. (Evilpope, mattx86)
   - Set it to 0, and it'll grab the refresh rate from Windows.
   - Set it to anything else, and it'll be used as the refresh rate you want.
- Added hardware gamma ramp code.  Use the r_hwgamma cvar to toggle it on/off. (MrG)
- Added detail textures and water caustics (Evilpope, Echon, mattx86)
   - r_detailtexture to set which detail texture to use. (1 - 8, or set to 0 to turn them off)
   - r_caustics to toggle the use of water caustics.
- Updated LoadTGA() to support more TGA formats. (Idle, NiceAss)
- Added PNG support. (Evilpope)
- Added stainmaps. (Evilpope, mattx86)
   - r_stainmap, toggle them on/off (1/0)
- There are now 100 address book cvars, adr[0-99], though the server menu doesn't display any more than the first 8 or so. (mattx86)
- Updated alias, cvarlist, bind, and bindlist. (mattx86)
   - Added aliaslist cmd.
   - alias and bind now list aliases/binds by default, but still function of course, to be able to setup a new bind or alias.
   - alias/aliaslist now lists like binds/cvars
   - alias/aliaslist, cvarlist, cmdlist, and bind/bindlist now accept a wildcard to search through them
   - bindlist now lists the empty binds
- Added unset cmd for cvars. (mattx86)
- Added savecfg cmd. (mattx86)
   - Accepts a filename parameter, which if not specified, will attempt to save it as config.cfg.
   - You can append the .cfg to it or not - and it will add it for you.
- Config's generated with MyQ2 are timestamped.  Address Book entries are placed at the top, then the rest of your variables, then aliases, and finally, your binds. (mattx86)
- The exec cmd now also allows you to append the .cfg to it or not - and it will be added for you. (mattx86)
- The condump cmd now also allows you to append the .txt to it or not - and it will be added for you. (mattx86)
- Particles with gl_ext_pointparameters set to 0, should be bigger and smoother. (Carbon14, mattx86)
- Netgraph is now drawn transparently. (mattx86)
- GL_Upload32 updated. (Echon, Evilpope)
   - gl_sgis_generate_mipmap, toggle this mipmapping on/off (1/0)
   - gl_ext_texture_filter_anisotropic, anisotropic filtering
      - Set it to 0, and it'll be disabled.
      - Set it to anything else, and it'll be used as the anisotropic filtering value you want.  However, if this is set higher than the max your card can do, it will be set to that max.
   - Texture sizes now limited to the max your card can do.
- Added two extra messagemodes. (mattx86)
   - messagemode3, bind a key to this and when hit, you'll type in <who> you want to say the message to <space> and then the message.  This is OSP-specific I believe. [tell]
   - messagemode4, bind a key to this and when hit, you'll type in <who> or even LIKE <partial> <space> and then the message.  This is Q2Admin-specific I believe. [say_person]
- Out of date predictions no longer freeze.  This helps with the occasional packet loss. (mattx86)
- Added ugly skin fix, where if someone had female/../cyborg/ps9000 as their skin, you'd see and hear the cyborg/ps9000 skin/sounds on/from the female model. (mattx86)
- Added MOUSE4 and MOUSE5. (jitspoe, backslash, Echon, mattx86)
- Added a player overflow fix, having to do with skins. (flossie, mattx86)
- Added normal, water, slime, and lava fog. (GGodis, Echon, Evilpope, mattx86)
   - Normal and water fog can be toggled on/off (1/0) with the r_fog cvar; slime and lava fog are always on.
- Fixed some depth masking problems. (mattx86)
   - Particles can now be seen through alpha surfaces.
   - You can now see things through shadows.
- Added overbright rendering (Vic, Evilpope, mattx86)
   - r_overbrightbits cvar
      - Set it to 0, and it'll be disabled.
      - Set it to anything else, and it'll be used as the overbright value you want.
- You can now change the default skin, the default being male/grunt. (Chattanoo, mattx86)
   - cl_defaultskin, change this to female/athena for example, and every time now that someone's using a skin that you don't have, they'll show up as this.
   - cl_noskins updated, so that when you've set cl_defaultskin, and turn cl_noskins on, you'll see everyone as the skin you set in cl_defaultskin.
   - if cl_defaultskin or cl_noskins are modified, the skins cmd will automatically be executed.
- Gun is now visible with an FOV > 90, as well as when the cvar hand is set to 2, the gun is visible and centered, provided you have cl_gun set to 1, to actually see the gun. (Riot, mattx86)
- Game gets darker/brighter based on the hour of the day.  Normal and water fog comes out at night, providing you have r_fog set to 1. (mattx86)
   - r_timebasedfx cvar, toggle it on (1) or off (0)
- Added some code to the server to check whether or not clients are connecting from a local network (loopback, LAN/wLAN/vLAN), and if so, to *not* do a rate drop.  This allows the game to be played as smoothly as possible when you're LANning or otherwise. (mattx86)
- Updated particles. (Carbon14, Evilpope, mattx86)
   - Point Parameter type particles are no longer used, so the following variables are now useless:
      - gl_particle_*
      - gl_ext_pointparameters
- Demo "d1" no longer plays at startup. (mattx86)
- default.cfg is no longer executed at startup, only config.cfg and autoexec.cfg get executed now. (mattx86)
- Updated the EXE's icon a little bit and other little updates/fixes/additions.



Current cvars
~~~~~~~~~~~~~
adr[0-99]
cl_defaultskin
gl_ext_texture_filter_anisotropic
gl_sgis_generate_mipmap
paksearch
r_caustics
r_celshading
r_detailtexture
r_displayrefresh
r_fog
r_hwgamma
r_overbrightbits
r_stainmap
r_timebasedfx
r_verbose
s_verbose


Current cmds
~~~~~~~~~~~~
aliaslist
messagemode3
messagemode4
savecfg
unset



Stuff that needs to be done
===========================
- Fix lighting on alpha surfaces.
- Add volumetric shadows.
- Add per pixel lighting.
- Make it so you can toggle the new particles on/off.
- Make the rail trail actually spiral out.
- Add/update some of the Q2Advance/NoCheat Eye Candy.

