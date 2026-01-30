/*
Copyright (C) 1997-2001 Id Software, Inc.

This program is free software; you can redistribute it and/or
modify it under the terms of the GNU General Public License
as published by the Free Software Foundation; either version 2
of the License, or (at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  

See the GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program; if not, write to the Free Software
Foundation, Inc., 59 Temple Place - Suite 330, Boston, MA  02111-1307, USA.

*/
// r_misc.c

#include "gl_local.h"

byte	notexture[16][16] =
{
	
	{1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1},
	{1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1},
	{1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1},
	{1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1},
	{1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1},
	{1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1},
	{1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1},
	{1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1},
	{1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1},
	{1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1},
	{1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1},
	{1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1},
	{1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1},
	{1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1},
	{1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1},
	{1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1},
	
};

/*
byte	dottexture[16][16] =
{
	{  0,  0,  0,  0,  0,  0, 27, 31, 31, 27,  0,  0,  0,  0,  0,  0},
	{  0,  0,  0, 31, 39, 47, 55, 63, 63, 55, 47, 39, 31,  0,  0,  0},
	{  0,  0, 35, 47, 59, 71, 83, 95, 95, 83, 71, 59, 47, 35,  0,  0},
	{  0, 31, 47, 63, 79, 95,111,127,127,111, 95, 79, 63, 47, 31,  0},
	{  0, 39, 59, 79, 99,119,139,159,159,139,119, 99, 79, 59, 39,  0},
	{  0, 47, 71, 95,119,143,167,191,191,167,143,119, 95, 71, 47,  0},
	{ 27, 55, 83,111,139,167,195,195,195,195,167,139,111, 83, 55, 27},
	{ 31, 63, 95,127,159,191,195,195,195,195,191,159,127, 95, 63, 31},
	{ 31, 63, 95,127,159,191,195,195,195,195,191,159,127, 95, 63, 31},
	{ 27, 55, 83,111,139,167,195,195,195,195,167,139,111, 83, 55, 27},
	{  0, 47, 71, 95,119,143,167,191,191,167,143,119, 95, 71, 47,  0},
	{  0, 39, 59, 79, 99,119,139,159,159,139,119, 99, 79, 59, 39,  0},
	{  0, 31, 47, 63, 79, 95,111,127,127,111, 95, 79, 63, 47, 31,  0},
	{  0,  0, 35, 47, 59, 71, 83, 95, 95, 83, 71, 59, 47, 35,  0,  0},
	{  0,  0,  0, 31, 39, 47, 55, 63, 63, 55, 47, 39, 31,  0,  0,  0},
	{  0,  0,  0,  0,  0,  0, 27, 31, 31, 27,  0,  0,  0,  0,  0,  0},
};
*/

/*
==================
R_InitParticleTexture
==================
*/

extern image_t *Draw_FindPic(char *name);  //c14

void R_InitParticleTexture (void)
{
	int		x, y, xx, alpha;
	byte	data[16][16][4];

	//===================================================================================
	// PARTICLE TEXTURES
	//===================================================================================
	for (x = 0; x < 16; x++)
	{
		xx = (x - 8) * (x - 8);

		for (y = 0; y < 16; y++)
		{
			alpha = 255 - 4 * (xx + ((y-8) * (y-8)));

			if (alpha <= 0)
				alpha = data[y][x][0] = data[y][x][1] = data[y][x][2] = 0;
			else
				data[y][x][0] = data[y][x][1] = data[y][x][2] = 255;

			data[y][x][3] = alpha;
		}
	}

	r_particletexture[PT_DEFAULT] = Draw_FindPic("particles/default");
	r_particletexture[PT_FIRE] = Draw_FindPic("particles/fire");
	r_particletexture[PT_SMOKE] = Draw_FindPic("particles/smoke");
	r_particletexture[PT_BUBBLE] = Draw_FindPic("particles/bubble");
	r_particletexture[PT_BLOOD] = Draw_FindPic("particles/blood");

	for (x = 0; x < PT_MAX; x++)
	{
		if (!r_particletexture[x])
			r_particletexture[x] = GL_LoadPic ("***particle***", (byte *)data, 16, 16, it_sprite, 32);
    }


	//===================================================================================
	// NO_TEXTURE TEXTURE
	//===================================================================================
	for (x = 0; x < 16; x++)
	{
		for (y = 0; y < 16; y++)
		{
			data[y][x][0] = notexture[x&3][y&3] * 255;
			data[y][x][1] = 0;
			data[y][x][2] = 0;
			data[y][x][3] = 255;
		}
	}
	r_notexture = GL_LoadPic ("***r_notexture***", (byte *)data, 16, 16, it_wall, 32);
}


/* 
============================================================================== 
 
						SCREEN SHOTS 
 
============================================================================== 
*/ 

typedef struct _TargaHeader {
	unsigned char 	id_length, colormap_type, image_type;
	unsigned short	colormap_index, colormap_length;
	unsigned char	colormap_size;
	unsigned short	x_origin, y_origin, width, height;
	unsigned char	pixel_size, attributes;
} TargaHeader;


/* 
================== 
GL_ScreenShot_f
================== 
*/  
void GL_ScreenShot_f (void) 
{
	byte		*buffer;
	char		picname[80]; 
	char		checkname[MAX_OSPATH];
	int			i, c, temp;
	FILE		*f;

	// create the scrnshots directory if it doesn't exist
	Com_sprintf (checkname, sizeof(checkname), "%s/scrnshot", FS_Gamedir());
	Sys_Mkdir (checkname);

// 
// find a file name to save it to 
// 
	strcpy(picname,"quake00.tga");

	for (i=0 ; i<=99 ; i++) 
	{ 
		picname[5] = i/10 + '0';
		picname[6] = i%10 + '0';
		Com_sprintf (checkname, sizeof(checkname), "%s/scrnshot/%s", FS_Gamedir(), picname);
		f = fopen (checkname, "rb");
		if (!f)
			break;	// file doesn't exist
		fclose (f);
	} 
	if (i==100) 
	{
		VID_Printf (PRINT_ALL, "SCR_ScreenShot_f: Couldn't create a file\n"); 
		return;
 	}


	buffer = malloc(vid.width*vid.height*3 + 18);
	memset (buffer, 0, 18);
	buffer[2] = 2;		// uncompressed type
	buffer[12] = vid.width&255;
	buffer[13] = vid.width>>8;
	buffer[14] = vid.height&255;
	buffer[15] = vid.height>>8;
	buffer[16] = 24;	// pixel size

	qglReadPixels (0, 0, vid.width, vid.height, GL_RGB, GL_UNSIGNED_BYTE, buffer+18 );
	// apply gamma correction if necessary
	if (gl_config.gammaramp)
	{
		byte gammaTable[256], *ptr = buffer + 18;

		for (i=0; i<256; i++)
		{
			signed int v;
			v = 255 * pow ( (i+0.5) * 0.0039138943248532289628180039138943, vid_gamma->value ) + 0.5;
			if (v > 255) v=255;
			if (v < 0) v=0;
			gammaTable[i]=v;
		}

		c = vid.width * vid.height * 3;
		for (i=0; i<c; i++)
		{
			*ptr = gammaTable[*ptr];
			ptr++;
		}
	}

	// swap rgb to bgr
	c = 18+vid.width*vid.height*3;
	for (i=18 ; i<c ; i+=3)
	{
		temp = buffer[i];
		buffer[i] = buffer[i+2];
		buffer[i+2] = temp;
	}

	f = fopen (checkname, "wb");
	fwrite (buffer, 1, c, f);
	fclose (f);

	free (buffer);
	VID_Printf (PRINT_ALL, "Wrote %s\n", picname);
} 

/*
** GL_Strings_f
*/
void GL_Strings_f( void )
{
	VID_Printf (PRINT_ALL, "GL_VENDOR: %s\n", gl_config.vendor_string );
	VID_Printf (PRINT_ALL, "GL_RENDERER: %s\n", gl_config.renderer_string );
	VID_Printf (PRINT_ALL, "GL_VERSION: %s\n", gl_config.version_string );
	VID_Printf (PRINT_ALL, "GL_EXTENSIONS: %s\n", gl_config.extensions_string );
}

/*
** GL_SetDefaultState
*/
void GL_SetDefaultState( void )
{
	qglClearColor (1,0, 0.5 , 0.5);
	qglCullFace(GL_FRONT);
	qglEnable(GL_TEXTURE_2D);

	qglEnable(GL_ALPHA_TEST);
	qglAlphaFunc(GL_GREATER, 0.666);

	qglDisable (GL_DEPTH_TEST);
	qglDisable (GL_CULL_FACE);
	qglDisable (GL_BLEND);

	qglDisable(GL_FOG);

	qglColor4f (1,1,1,1);

	qglPolygonMode (GL_FRONT_AND_BACK, GL_FILL);
	qglShadeModel (GL_FLAT);

	GL_TextureMode( gl_texturemode->string );
	GL_TextureAlphaMode( gl_texturealphamode->string );
	GL_TextureSolidMode( gl_texturesolidmode->string );

	qglTexParameterf(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, gl_filter_min);
	qglTexParameterf(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, gl_filter_max);

	qglTexParameterf(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_REPEAT);
	qglTexParameterf(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_REPEAT);

	qglBlendFunc (GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);

	GL_TexEnv( GL_REPLACE );

#if 0
	if ( qglPointParameterfEXT )
	{
		float attenuations[3];

		attenuations[0] = gl_particle_att_a->value;
		attenuations[1] = gl_particle_att_b->value;
		attenuations[2] = gl_particle_att_c->value;

		qglEnable( GL_POINT_SMOOTH );
		qglHint(GL_POINT_SMOOTH_HINT, GL_NICEST); // majparticle
		qglPointParameterfEXT( GL_POINT_SIZE_MIN_EXT, gl_particle_min_size->value );
		qglPointParameterfEXT( GL_POINT_SIZE_MAX_EXT, gl_particle_max_size->value );
		qglPointParameterfvEXT( GL_DISTANCE_ATTENUATION_EXT, attenuations );
	}
#endif

	GL_UpdateSwapInterval();
}

void GL_UpdateSwapInterval( void )
{
	if ( gl_swapinterval->modified )
	{
		gl_swapinterval->modified = false;

		if ( !gl_state.stereo_enabled ) 
		{
#ifdef _WIN32
			if ( qwglSwapIntervalEXT )
				qwglSwapIntervalEXT( gl_swapinterval->value );
#endif
		}
	}
}