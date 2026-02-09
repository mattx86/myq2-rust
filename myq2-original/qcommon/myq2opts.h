/*
  MyQ2 - Build Options
*/
#ifndef MYQ2OPTS_H
#define MYQ2OPTS_H


#define DISTNAME			"MyQ2"
#define DISTVER				0.01

// ---- Enable / Disable Section ----
#define HIRES_TEX_SCALING							// mattx86: hires_scaling
#define TGAPNG_TEX_LOADING							// mattx86: tgapng_loading
#define SEED_RANDOM									// mattx86: seed_random
#define CONSOLE_INIT_EARLY							// mattx86: console_init
#define AUTO_CVAR									// mattx86: auto_cvar
#define USE_WSAECONNRESET_FIX						// mattx86: wsaeconnreset_fix
#define ENABLE_BOBBING_ITEMS						// mattx86: bobbing_items
#define AUTO_MOUSE_XPFIX							// mattx86: mouse_xpfix
#define DLIGHT_SURFACE_FIX							// mattx86: dlight_surface_fix
#define TASKBAR_FIX									// mattx86: taskbar_fix
#define PLAYER_MENU_FIX								// mattx86: player_menu_fix
#define USE_CONSOLE_IN_DEMOS						// mattx86: console_demos
#define TRANS_CONSOLE								// mattx86: trans_console
#define BETTER_DLIGHT_FALLOFF						// mattx86: dlight_falloff
//#define DO_WATER_WAVES								// mattx86: water_waves
//#define PRED_OUT_OF_DATE_FREEZE						// mattx86: out_of_date_freeze
#define USE_UGLY_SKIN_FIX							// mattx86: ugly_skin_fix
#define ENABLE_MOUSE4_MOUSE5						// mattx86: mouse4_mouse5
#define PLAYER_OVERFLOW_FIX							// mattx86: player_overflow_fix
#define VISIBLE_GUN_WIDEANGLE						// mattx86: gun_wideangle
#define CENTERED_GUN								// mattx86: centered_gun
#define DISABLE_STARTUP_DEMO						// mattx86: startup_demo
//#define SWAP_UDP_FOR_TCP							// mattx86: udp_tcp
//#define DO_REFLECTIVE_WATER							// mattx86: reflective_water


// ---- Settings Section ----
#define NOTIFY_INDENT		2						// mattx86: console_indent_notifylines
#define NOTIFY_LINEWIDTH	(((viddef.width*0.45)/8)-2)	// mattx86: console_linewidth_notifylines
#define NOTIFY_VERTPOS		viddef.height * 0.675	// mattx86: console_verticalpos_noftylines
#define NUM_CON_TIMES		5						// mattx86: console_notifylines
#define CON_TEXTSIZE		131072					// mattx86: console_textsize
#define PORT_CLIENT			(rand()%16383)+49152	// mattx86: port_client
#define TRANS_CONSOLE_VALUE	0.675					// mattx86: trans_console
#define SKYBOX_SIZE			4600					// mattx86: skybox_size
#define	DLIGHT_CUTOFF		16						// mattx86: dlight_cutoff
#define OUTLINEDROPOFF		1000.0					// mattx86: cel_shading
#define CEL_WIDTH			1.50					// mattx86: cel_shading


// ---- fixme ----


// ---- etc ----


#endif
