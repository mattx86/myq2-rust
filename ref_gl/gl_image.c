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

#include "gl_local.h"
#include "../lib/png.h"

image_t		gltextures[MAX_GLTEXTURES];
int			numgltextures;
int			base_textureid;		// gltextures[i] = base_textureid+i

static byte			 intensitytable[256];
static unsigned char gammatable[256];

cvar_t		*intensity;

unsigned	d_8to24table[256];

qboolean GL_Upload8 (byte *data, int width, int height, qboolean mipmap, image_t *image);
qboolean GL_Upload32 (unsigned *data, int width, int height, qboolean mipmap, int bpp, image_t *image);

int		gl_solid_format = 3;
int		gl_alpha_format = 4;

int		gl_tex_solid_format = 3;
int		gl_tex_alpha_format = 4;

int		gl_filter_min = GL_LINEAR_MIPMAP_LINEAR;
int		gl_filter_max = GL_LINEAR;

void GL_EnableMultitexture(qboolean enable)
{
	if (!qglSelectTextureSGIS && !qglActiveTextureARB)
		return;

#ifdef __linux__
	GL_SelectTexture(QGL_TEXTURE3);
#else
	GL_SelectTexture(GL_TEXTURE3);
#endif

	if (enable)
		qglEnable(GL_TEXTURE_2D);
	else
		qglDisable(GL_TEXTURE_2D);

#ifdef __linux__
	GL_SelectTexture(QGL_TEXTURE2);
#else
	GL_SelectTexture(GL_TEXTURE2);
#endif

	if (enable)
		qglEnable(GL_TEXTURE_2D);
	else
		qglDisable(GL_TEXTURE_2D);

#ifdef __linux__
	GL_SelectTexture(QGL_TEXTURE1);
#else
	GL_SelectTexture(GL_TEXTURE1);
#endif

	if (enable)
		qglEnable(GL_TEXTURE_2D);
	else
		qglDisable(GL_TEXTURE_2D);

	GL_TexEnv(GL_REPLACE);

#ifdef __linux__
	GL_SelectTexture(QGL_TEXTURE0);
#else
	GL_SelectTexture(GL_TEXTURE0);
#endif

	GL_TexEnv(GL_REPLACE);
}

void GL_SelectTexture( GLenum texture )
{
	int tmu;

	if ( !qglSelectTextureSGIS && !qglActiveTextureARB )
		return;

	if ( texture == GL_TEXTURE0 )
	{
		tmu = 0;
	}
	//ep::multitexturing
	else if ( texture == GL_TEXTURE2 )
	{
		tmu = 2;
	}
	else if ( texture == GL_TEXTURE3 )
	{
		tmu = 3;
	}
	//ep::multitexturing
	else
	{
		tmu = 1;
	}

	if ( tmu == gl_state.currenttmu )
	{
		return;
	}

	gl_state.currenttmu = tmu;

	if ( qglSelectTextureSGIS )
	{
		qglSelectTextureSGIS( texture );
	}
	else if ( qglActiveTextureARB )
	{
		qglActiveTextureARB( texture );
		qglClientActiveTextureARB( texture );
	}
}

void GL_TexEnv( GLenum mode )
{
	static int lastmodes[4] = { -1, -1, -1, -1 };

	if ( mode != lastmodes[gl_state.currenttmu] )
	{
		qglTexEnvf( GL_TEXTURE_ENV, GL_TEXTURE_ENV_MODE, mode );
		lastmodes[gl_state.currenttmu] = mode;
	}
}

void GL_Bind (int texnum)
{
	extern	image_t	*draw_chars;

	if (gl_nobind->value && draw_chars)		// performance evaluation option
		texnum = draw_chars->texnum;
	if ( gl_state.currenttextures[gl_state.currenttmu] == texnum)
		return;
	gl_state.currenttextures[gl_state.currenttmu] = texnum;
	qglBindTexture (GL_TEXTURE_2D, texnum);
}

void GL_MBind( GLenum target, int texnum )
{
	GL_SelectTexture( target );
	if ( target == GL_TEXTURE0 )
	{
		if ( gl_state.currenttextures[0] == texnum )
			return;
	}
	//ep::multitexturing
	
	else if ( target == GL_TEXTURE2 )
	{
		if ( gl_state.currenttextures[2] == texnum )
			return;
	}
	else if ( target == GL_TEXTURE3 )
	{
		if ( gl_state.currenttextures[3] == texnum )
			return;
	}
	//ep::multitexturing
	else
	{
		if ( gl_state.currenttextures[1] == texnum )
			return;
	}
	GL_Bind( texnum );
}

typedef struct
{
	char *name;
	int	minimize, maximize;
} glmode_t;

glmode_t modes[] = {
	{"GL_NEAREST", GL_NEAREST, GL_NEAREST},
	{"GL_LINEAR", GL_LINEAR, GL_LINEAR},
	{"GL_NEAREST_MIPMAP_NEAREST", GL_NEAREST_MIPMAP_NEAREST, GL_NEAREST},
	{"GL_LINEAR_MIPMAP_NEAREST", GL_LINEAR_MIPMAP_NEAREST, GL_LINEAR},
	{"GL_NEAREST_MIPMAP_LINEAR", GL_NEAREST_MIPMAP_LINEAR, GL_NEAREST},
	{"GL_LINEAR_MIPMAP_LINEAR", GL_LINEAR_MIPMAP_LINEAR, GL_LINEAR}
};

#define NUM_GL_MODES (sizeof(modes) / sizeof (glmode_t))

typedef struct
{
	char *name;
	int mode;
} gltmode_t;

gltmode_t gl_alpha_modes[] = {
	{"default", 4},
	{"GL_RGBA", GL_RGBA},
	{"GL_RGBA8", GL_RGBA8},
	{"GL_RGB5_A1", GL_RGB5_A1},
	{"GL_RGBA4", GL_RGBA4},
	{"GL_RGBA2", GL_RGBA2},
};

#define NUM_GL_ALPHA_MODES (sizeof(gl_alpha_modes) / sizeof (gltmode_t))

gltmode_t gl_solid_modes[] = {
	{"default", 3},
	{"GL_RGB", GL_RGB},
	{"GL_RGB8", GL_RGB8},
	{"GL_RGB5", GL_RGB5},
	{"GL_RGB4", GL_RGB4},
	{"GL_R3_G3_B2", GL_R3_G3_B2},
#ifdef GL_RGB2_EXT
	{"GL_RGB2", GL_RGB2_EXT},
#endif
};

#define NUM_GL_SOLID_MODES (sizeof(gl_solid_modes) / sizeof (gltmode_t))

/*
===============
GL_TextureMode
===============
*/
void GL_TextureMode( char *string )
{
	int		i;
	image_t	*glt;

	for (i=0 ; i< NUM_GL_MODES ; i++)
	{
		if ( !Q_strcasecmp( modes[i].name, string ) )
			break;
	}

	if (i == NUM_GL_MODES)
	{
		VID_Printf (PRINT_ALL, "bad filter name\n");
		return;
	}

	gl_filter_min = modes[i].minimize;
	gl_filter_max = modes[i].maximize;

	// change all the existing mipmap texture objects
	for (i=0, glt=gltextures ; i<numgltextures ; i++, glt++)
	{
		if (glt->type != it_pic && glt->type != it_sky )
		{
			GL_Bind (glt->texnum);
			qglTexParameterf(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, gl_filter_min);
			qglTexParameterf(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, gl_filter_max);
		}
	}
}

/*
===============
GL_TextureAlphaMode
===============
*/
void GL_TextureAlphaMode( char *string )
{
	int		i;

	for (i=0 ; i< NUM_GL_ALPHA_MODES ; i++)
	{
		if ( !Q_strcasecmp( gl_alpha_modes[i].name, string ) )
			break;
	}

	if (i == NUM_GL_ALPHA_MODES)
	{
		VID_Printf (PRINT_ALL, "bad alpha texture mode name\n");
		return;
	}

	gl_tex_alpha_format = gl_alpha_modes[i].mode;
}

/*
===============
GL_TextureSolidMode
===============
*/
void GL_TextureSolidMode( char *string )
{
	int		i;

	for (i=0 ; i< NUM_GL_SOLID_MODES ; i++)
	{
		if ( !Q_strcasecmp( gl_solid_modes[i].name, string ) )
			break;
	}

	if (i == NUM_GL_SOLID_MODES)
	{
		VID_Printf (PRINT_ALL, "bad solid texture mode name\n");
		return;
	}

	gl_tex_solid_format = gl_solid_modes[i].mode;
}

/*
===============
GL_ImageList_f
===============
*/
void	GL_ImageList_f (void)
{
	int		i;
	image_t	*image;
	int		texels;

	VID_Printf (PRINT_ALL, "------------------\n");
	texels = 0;

	for (i=0, image=gltextures ; i<numgltextures ; i++, image++)
	{
		if (image->texnum <= 0)
			continue;
		texels += image->upload_width*image->upload_height;
		switch (image->type)
		{
		case it_skin:
			VID_Printf (PRINT_ALL, "M");
			break;
		case it_sprite:
			VID_Printf (PRINT_ALL, "S");
			break;
		case it_wall:
			VID_Printf (PRINT_ALL, "W");
			break;
		case it_pic:
			VID_Printf (PRINT_ALL, "P");
			break;
		default:
			VID_Printf (PRINT_ALL, " ");
			break;
		}

		VID_Printf (PRINT_ALL,  " %3i %3i: %s\n",
			image->upload_width, image->upload_height, image->name);
	}
	VID_Printf (PRINT_ALL, "Total texel count (not counting mipmaps): %i\n", texels);
}


/*
=============================================================================

  scrap allocation

  Allocate all the little status bar obejcts into a single texture
  to crutch up inefficient hardware / drivers

=============================================================================
*/

#define	MAX_SCRAPS		1
#define	BLOCK_WIDTH		256
#define	BLOCK_HEIGHT	256

int			scrap_allocated[MAX_SCRAPS][BLOCK_WIDTH];
byte		scrap_texels[MAX_SCRAPS][BLOCK_WIDTH*BLOCK_HEIGHT];
qboolean	scrap_dirty;

// returns a texture number and the position inside it
int Scrap_AllocBlock (int w, int h, int *x, int *y)
{
	int		i, j;
	int		best, best2;
	int		texnum;

	for (texnum=0 ; texnum<MAX_SCRAPS ; texnum++)
	{
		best = BLOCK_HEIGHT;

		for (i=0 ; i<BLOCK_WIDTH-w ; i++)
		{
			best2 = 0;

			for (j=0 ; j<w ; j++)
			{
				if (scrap_allocated[texnum][i+j] >= best)
					break;
				if (scrap_allocated[texnum][i+j] > best2)
					best2 = scrap_allocated[texnum][i+j];
			}
			if (j == w)
			{	// this is a valid spot
				*x = i;
				*y = best = best2;
			}
		}

		if (best + h > BLOCK_HEIGHT)
			continue;

		for (i=0 ; i<w ; i++)
			scrap_allocated[texnum][*x + i] = best + h;

		return texnum;
	}

	return -1;
//	Sys_Error ("Scrap_AllocBlock: full");
}

int	scrap_uploads;

void Scrap_Upload (void)
{
	scrap_uploads++;
	GL_Bind(TEXNUM_SCRAPS);
	GL_Upload8 (scrap_texels[0], BLOCK_WIDTH, BLOCK_HEIGHT, false, NULL );
	scrap_dirty = false;
}

/*
=================================================================

PCX LOADING

=================================================================
*/


/*
==============
LoadPCX
==============
*/
void LoadPCX (char *filename, byte **pic, byte **palette, int *width, int *height)
{
	byte	*raw;
	pcx_t	*pcx;
	int		x, y;
	int		len;
	int		dataByte, runLength;
	byte	*out, *pix;

	*pic = NULL;
	*palette = NULL;

	//
	// load the file
	//
	len = FS_LoadFile (filename, (void **)&raw);
	if (!raw)
	{
		VID_Printf (PRINT_DEVELOPER, "Bad pcx file %s\n", filename);
		return;
	}

	//
	// parse the PCX file
	//
	pcx = (pcx_t *)raw;

    pcx->xmin = LittleShort(pcx->xmin);
    pcx->ymin = LittleShort(pcx->ymin);
    pcx->xmax = LittleShort(pcx->xmax);
    pcx->ymax = LittleShort(pcx->ymax);
    pcx->hres = LittleShort(pcx->hres);
    pcx->vres = LittleShort(pcx->vres);
    pcx->bytes_per_line = LittleShort(pcx->bytes_per_line);
    pcx->palette_type = LittleShort(pcx->palette_type);

	raw = &pcx->data;

	if (pcx->manufacturer != 0x0a
		|| pcx->version != 5
		|| pcx->encoding != 1
		|| pcx->bits_per_pixel != 8
		|| pcx->xmax >= 640
		|| pcx->ymax >= 480)
	{
		VID_Printf (PRINT_ALL, "Bad pcx file %s\n", filename);
		return;
	}

	out = malloc ( (pcx->ymax+1) * (pcx->xmax+1) );

	*pic = out;

	pix = out;

	if (palette)
	{
		*palette = malloc(768);
		memcpy (*palette, (byte *)pcx + len - 768, 768);
	}

	if (width)
		*width = pcx->xmax+1;
	if (height)
		*height = pcx->ymax+1;

	for (y=0 ; y<=pcx->ymax ; y++, pix += pcx->xmax+1)
	{
		for (x=0 ; x<=pcx->xmax ; )
		{
			dataByte = *raw++;

			if((dataByte & 0xC0) == 0xC0)
			{
				runLength = dataByte & 0x3F;
				dataByte = *raw++;
			}
			else
				runLength = 1;

			while(runLength-- > 0)
				pix[x++] = dataByte;
		}

	}

	if ( raw - (byte *)pcx > len)
	{
		VID_Printf (PRINT_DEVELOPER, "PCX file %s was malformed", filename);
		free (*pic);
		*pic = NULL;
	}

	FS_FreeFile (pcx);
}


/*
=========================================================

PNG LOADING

=========================================================
*/

typedef struct {
    BYTE *Buffer;
    int Pos;
} TPngFileBuffer;

void __cdecl PngReadFunc(png_struct *Png, png_bytep buf, png_size_t size)
{
    TPngFileBuffer *PngFileBuffer=(TPngFileBuffer*)png_get_io_ptr(Png);
    memcpy(buf,PngFileBuffer->Buffer+PngFileBuffer->Pos,size);
    PngFileBuffer->Pos+=size;
}

/*
=============
LoadPNG
=============
*/

void LoadPNG (char *name, byte **pic, int *width, int *height)
{
	int				i, rowptr;
	png_structp		png_ptr;
	png_infop		info_ptr;
	png_infop		end_info;

	unsigned char	**row_pointers;

	TPngFileBuffer	PngFileBuffer = {NULL,0};

	*pic = NULL;

	FS_LoadFile (name, &PngFileBuffer.Buffer);

    if (!PngFileBuffer.Buffer)
		return;

	if ((png_check_sig(PngFileBuffer.Buffer, 8)) == 0)
	{
		FS_FreeFile (PngFileBuffer.Buffer); 
		VID_Printf (PRINT_ALL, "Not a PNG file: %s\n", name);
		return;
    }

	PngFileBuffer.Pos=0;

    png_ptr = png_create_read_struct (PNG_LIBPNG_VER_STRING, NULL,  NULL, NULL);

    if (!png_ptr)
	{
		FS_FreeFile (PngFileBuffer.Buffer);
		VID_Printf (PRINT_ALL, "Bad PNG file: %s\n", name);
		return;
	}

    info_ptr = png_create_info_struct(png_ptr);
    if (!info_ptr)
	{
        png_destroy_read_struct(&png_ptr, (png_infopp)NULL, (png_infopp)NULL);
		FS_FreeFile (PngFileBuffer.Buffer);
		VID_Printf (PRINT_ALL, "Bad PNG file: %s\n", name);
		return;
    }
    
	end_info = png_create_info_struct(png_ptr);
    if (!end_info)
	{
        png_destroy_read_struct(&png_ptr, &info_ptr, (png_infopp)NULL);
		FS_FreeFile (PngFileBuffer.Buffer);
		VID_Printf (PRINT_ALL, "Bad PNG file: %s\n", name);
		return;
    }

	png_set_read_fn (png_ptr,(png_voidp)&PngFileBuffer,(png_rw_ptr)PngReadFunc);

	png_read_png(png_ptr, info_ptr, PNG_TRANSFORM_IDENTITY, NULL);

	row_pointers = png_get_rows(png_ptr, info_ptr);

	rowptr = 0;

	*pic = malloc (info_ptr->width * info_ptr->height * sizeof(int));

	if (info_ptr->channels == 4)
	{
		for (i = 0; i < info_ptr->height; i++)
		{
			memcpy (*pic + rowptr, row_pointers[i], info_ptr->rowbytes);
			rowptr += info_ptr->rowbytes;
		}
	}
	else
	{
		int j, x;
		memset (*pic, 255, info_ptr->width * info_ptr->height * sizeof(int));
		x = 0;
		for (i = 0; i < info_ptr->height; i++)
		{
			for (j = 0; j < info_ptr->rowbytes; j+=info_ptr->channels)
			{
				memcpy (*pic + x, row_pointers[i] + j, info_ptr->channels);
				x+= sizeof(int);
			}
			rowptr += info_ptr->rowbytes;
		}
	}

	*width = info_ptr->width;
	*height = info_ptr->height;

	png_destroy_read_struct(&png_ptr, &info_ptr, &end_info);

	FS_FreeFile (PngFileBuffer.Buffer);
}


/*
=========================================================

TARGA LOADING

=========================================================
*/

typedef struct _TargaHeader {
	unsigned char 	id_length, colormap_type, image_type;
	unsigned short	colormap_index, colormap_length;
	unsigned char	colormap_size;
	unsigned short	x_origin, y_origin, width, height;
	unsigned char	pixel_size, attributes;
} TargaHeader;

// Definitions for image types
#define TGA_Null		0	// no image data
#define TGA_Map			1	// Uncompressed, color-mapped images
#define TGA_RGB			2	// Uncompressed, RGB images
#define TGA_Mono		3	// Uncompressed, black and white images
#define TGA_RLEMap		9	// Runlength encoded color-mapped images
#define TGA_RLERGB		10	// Runlength encoded RGB images
#define TGA_RLEMono		11	// Compressed, black and white images
#define TGA_CompMap		32	// Compressed color-mapped data, using Huffman, Delta, and runlength encoding
#define TGA_CompMap4	33	// Compressed color-mapped data, using Huffman, Delta, and runlength encoding. 4-pass quadtree-type process
// Definitions for interleave flag
#define TGA_IL_None		0	// non-interleaved
#define TGA_IL_Two		1	// two-way (even/odd) interleaving
#define TGA_IL_Four		2	// four way interleaving
#define TGA_IL_Reserved	3	// reserved
// Definitions for origin flag
#define TGA_O_UPPER		0	// Origin in lower left-hand corner
#define TGA_O_LOWER		1	// Origin in upper left-hand corner
#define MAXCOLORS 16384

/*
=============
LoadTGA
NiceAss: LoadTGA() from Q2Ice, it supports more formats
=============
*/
void LoadTGA( char *filename, byte **pic, int *width, int *height )
{
	int			w, h, x, y, i, temp1, temp2;
	int			realrow, truerow, baserow, size, interleave, origin;
	int			pixel_size, map_idx, mapped, rlencoded, RLE_count, RLE_flag;
	TargaHeader	header;
	byte		tmp[2], r, g, b, a, j, k, l;
	byte		*dst, *ColorMap, *data, *pdata;

	// load file
	FS_LoadFile( filename, &data );

	if( !data )
		return;

	pdata = data;

	header.id_length = *pdata++;
	header.colormap_type = *pdata++;
	header.image_type = *pdata++;
	
	tmp[0] = pdata[0];
	tmp[1] = pdata[1];
	header.colormap_index = LittleShort( *((short *)tmp) );
	pdata+=2;
	tmp[0] = pdata[0];
	tmp[1] = pdata[1];
	header.colormap_length = LittleShort( *((short *)tmp) );
	pdata+=2;
	header.colormap_size = *pdata++;
	header.x_origin = LittleShort( *((short *)pdata) );
	pdata+=2;
	header.y_origin = LittleShort( *((short *)pdata) );
	pdata+=2;
	header.width = LittleShort( *((short *)pdata) );
	pdata+=2;
	header.height = LittleShort( *((short *)pdata) );
	pdata+=2;
	header.pixel_size = *pdata++;
	header.attributes = *pdata++;

	if( header.id_length )
		pdata += header.id_length;

	// validate TGA type
	switch( header.image_type ) {
		case TGA_Map:
		case TGA_RGB:
		case TGA_Mono:
		case TGA_RLEMap:
		case TGA_RLERGB:
		case TGA_RLEMono:
			break;
		default:
			VID_Printf ( ERR_DROP, "LoadTGA: Only type 1 (map), 2 (RGB), 3 (mono), 9 (RLEmap), 10 (RLERGB), 11 (RLEmono) TGA images supported\n" );
			return;
	}

	// validate color depth
	switch( header.pixel_size ) {
		case 8:
		case 15:
		case 16:
		case 24:
		case 32:
			break;
		default:
			VID_Printf ( ERR_DROP, "LoadTGA: Only 8, 15, 16, 24 and 32 bit images (with colormaps) supported\n" );
			return;
	}

	r = g = b = a = l = 0;

	// if required, read the color map information
	ColorMap = NULL;
	mapped = ( header.image_type == TGA_Map || header.image_type == TGA_RLEMap || header.image_type == TGA_CompMap || header.image_type == TGA_CompMap4 ) && header.colormap_type == 1;
	if( mapped ) {
		// validate colormap size
		switch( header.colormap_size ) {
			case 8:
			case 16:
			case 32:
			case 24:
				break;
			default:
				VID_Printf ( ERR_DROP, "LoadTGA: Only 8, 16, 24 and 32 bit colormaps supported\n" );
				return;
		}

		temp1 = header.colormap_index;
		temp2 = header.colormap_length;
		if( (temp1 + temp2 + 1) >= MAXCOLORS ) {
			FS_FreeFile( data );
			return;
		}
		ColorMap = (byte *)malloc( MAXCOLORS * 4 );
		map_idx = 0;
		for( i = temp1; i < temp1 + temp2; ++i, map_idx += 4 ) {
			// read appropriate number of bytes, break into rgb & put in map
			switch( header.colormap_size ) {
				case 8:
					r = g = b = *pdata++;
					a = 255;
					break;
				case 15:
					j = *pdata++;
					k = *pdata++;
					l = ((unsigned int) k << 8) + j;
					r = (byte) ( ((k & 0x7C) >> 2) << 3 );
					g = (byte) ( (((k & 0x03) << 3) + ((j & 0xE0) >> 5)) << 3 );
					b = (byte) ( (j & 0x1F) << 3 );
					a = 255;
					break;
				case 16:
					j = *pdata++;
					k = *pdata++;
					l = ((unsigned int) k << 8) + j;
					r = (byte) ( ((k & 0x7C) >> 2) << 3 );
					g = (byte) ( (((k & 0x03) << 3) + ((j & 0xE0) >> 5)) << 3 );
					b = (byte) ( (j & 0x1F) << 3 );
					a = (k & 0x80) ? 255 : 0;
					break;
				case 24:
					b = *pdata++;
					g = *pdata++;
					r = *pdata++;
					a = 255;
					l = 0;
					break;
				case 32:
					b = *pdata++;
					g = *pdata++;
					r = *pdata++;
					a = *pdata++;
					l = 0;
					break;
			}
			ColorMap[map_idx + 0] = r;
			ColorMap[map_idx + 1] = g;
			ColorMap[map_idx + 2] = b;
			ColorMap[map_idx + 3] = a;
		}
	}

	// check run-length encoding
	rlencoded = header.image_type == TGA_RLEMap || header.image_type == TGA_RLERGB || header.image_type == TGA_RLEMono;
	RLE_count = 0;
	RLE_flag = 0;

	w = header.width;
	h = header.height;

	if( width )
		*width = w;
	if( height )
		*height = h;

	size = w * h * 4;
	*pic = (byte *)malloc( size );

	memset( *pic, 0, size );

	// read the Targa file body and convert to portable format
	pixel_size = header.pixel_size;
	origin = (header.attributes & 0x20) >> 5;
	interleave = (header.attributes & 0xC0) >> 6;
	truerow = 0;
	baserow = 0;
	for( y = 0; y < h; y++ ) {
		realrow = truerow;
		if( origin == TGA_O_UPPER )
			realrow = h - realrow - 1;

		dst = *pic + realrow * w * 4;

		for( x = 0; x < w; x++ ) {
			// check if run length encoded
			if( rlencoded ) {
				if( !RLE_count ) {
					// have to restart run
					i = *pdata++;
					RLE_flag = (i & 0x80);
					if( !RLE_flag ) {
						// stream of unencoded pixels
						RLE_count = i + 1;
					} else {
						// single pixel replicated
						RLE_count = i - 127;
					}
					// decrement count & get pixel
					--RLE_count;
				} else {
					// have already read count & (at least) first pixel
					--RLE_count;
					if( RLE_flag )
						// replicated pixels
						goto PixEncode;
				}
			}

			// read appropriate number of bytes, break into RGB
			switch( pixel_size ) {
				case 8:
					r = g = b = l = *pdata++;
					a = 255;
					break;
				case 15:
					j = *pdata++;
					k = *pdata++;
					l = ((unsigned int) k << 8) + j;
					r = (byte) ( ((k & 0x7C) >> 2) << 3 );
					g = (byte) ( (((k & 0x03) << 3) + ((j & 0xE0) >> 5)) << 3 );
					b = (byte) ( (j & 0x1F) << 3 );
					a = 255;
					break;
				case 16:
					j = *pdata++;
					k = *pdata++;
					l = ((unsigned int) k << 8) + j;
					r = (byte) ( ((k & 0x7C) >> 2) << 3 );
					g = (byte) ( (((k & 0x03) << 3) + ((j & 0xE0) >> 5)) << 3 );
					b = (byte) ( (j & 0x1F) << 3 );
					a = 255;
					break;
				case 24:
					b = *pdata++;
					g = *pdata++;
					r = *pdata++;
					a = 255;
					l = 0;
					break;
				case 32:
					b = *pdata++;
					g = *pdata++;
					r = *pdata++;
					a = *pdata++;
					l = 0;
					break;
				default:
					VID_Printf( ERR_DROP, "Illegal pixel_size '%d' in file '%s'\n", filename );
					return;
			}

PixEncode:
			if ( mapped )
			{
				map_idx = l * 4;
				*dst++ = ColorMap[map_idx + 0];
				*dst++ = ColorMap[map_idx + 1];
				*dst++ = ColorMap[map_idx + 2];
				*dst++ = ColorMap[map_idx + 3];
			}
			else
			{
				*dst++ = r;
				*dst++ = g;
				*dst++ = b;
				*dst++ = a;
			}
		}

		if (interleave == TGA_IL_Four)
			truerow += 4;
		else if (interleave == TGA_IL_Two)
			truerow += 2;
		else
			truerow++;

		if (truerow >= h)
			truerow = ++baserow;
	}

	if (mapped)
		free( ColorMap );
	
	FS_FreeFile( data );
}


/*
====================================================================

IMAGE FLOOD FILLING

====================================================================
*/


/*
=================
Mod_FloodFillSkin

Fill background pixels so mipmapping doesn't have haloes
=================
*/

typedef struct
{
	short		x, y;
} floodfill_t;

// must be a power of 2
#define FLOODFILL_FIFO_SIZE 0x1000
#define FLOODFILL_FIFO_MASK (FLOODFILL_FIFO_SIZE - 1)

#define FLOODFILL_STEP( off, dx, dy ) \
{ \
	if (pos[off] == fillcolor) \
	{ \
		pos[off] = 255; \
		fifo[inpt].x = x + (dx), fifo[inpt].y = y + (dy); \
		inpt = (inpt + 1) & FLOODFILL_FIFO_MASK; \
	} \
	else if (pos[off] != 255) fdc = pos[off]; \
}

void R_FloodFillSkin( byte *skin, int skinwidth, int skinheight )
{
	byte				fillcolor = *skin; // assume this is the pixel to fill
	floodfill_t			fifo[FLOODFILL_FIFO_SIZE];
	int					inpt = 0, outpt = 0;
	int					filledcolor = -1;
	int					i;

	if (filledcolor == -1)
	{
		filledcolor = 0;
		// attempt to find opaque black
		for (i = 0; i < 256; ++i)
			if (d_8to24table[i] == (255 << 0)) // alpha 1.0
			{
				filledcolor = i;
				break;
			}
	}

	// can't fill to filled color or to transparent color (used as visited marker)
	if ((fillcolor == filledcolor) || (fillcolor == 255))
	{
		//printf( "not filling skin from %d to %d\n", fillcolor, filledcolor );
		return;
	}

	fifo[inpt].x = 0, fifo[inpt].y = 0;
	inpt = (inpt + 1) & FLOODFILL_FIFO_MASK;

	while (outpt != inpt)
	{
		int			x = fifo[outpt].x, y = fifo[outpt].y;
		int			fdc = filledcolor;
		byte		*pos = &skin[x + skinwidth * y];

		outpt = (outpt + 1) & FLOODFILL_FIFO_MASK;

		if (x > 0)				FLOODFILL_STEP( -1, -1, 0 );
		if (x < skinwidth - 1)	FLOODFILL_STEP( 1, 1, 0 );
		if (y > 0)				FLOODFILL_STEP( -skinwidth, 0, -1 );
		if (y < skinheight - 1)	FLOODFILL_STEP( skinwidth, 0, 1 );
		skin[x + skinwidth * y] = fdc;
	}
}

//=======================================================


/*
================
GL_ResampleTexture
================
*/
void GL_ResampleTexture (unsigned *in, int inwidth, int inheight, unsigned *out,  int outwidth, int outheight)
{
	int			i, j;
	unsigned	*inrow, *inrow2;
	unsigned	frac, fracstep;
	static unsigned	*p1 = NULL, *p2 = NULL;
	byte		*pix1, *pix2, *pix3, *pix4;

	if (!p1)
	{
		p1 = malloc(max_tsize * sizeof(int));
		p2 = malloc (max_tsize * sizeof(int));
	}

	fracstep = inwidth*0x10000/outwidth;

	frac = fracstep>>2;
	for (i=0 ; i<outwidth ; i++)
	{
		p1[i] = 4*(frac>>16);
		frac += fracstep;
	}

	frac = 3*(fracstep>>2);
	for (i=0 ; i<outwidth ; i++)
	{
		p2[i] = 4*(frac>>16);
		frac += fracstep;
	}

	for (i=0 ; i<outheight ; i++, out += outwidth)
	{
		inrow = in + inwidth*(int)((i+0.25)*inheight/outheight);
		inrow2 = in + inwidth*(int)((i+0.75)*inheight/outheight);
		frac = fracstep >> 1;
		for (j=0 ; j<outwidth ; j++)
		{
			pix1 = (byte *)inrow + p1[j];
			pix2 = (byte *)inrow + p2[j];
			pix3 = (byte *)inrow2 + p1[j];
			pix4 = (byte *)inrow2 + p2[j];

			((byte *)(out+j))[0] = (pix1[0] + pix2[0] + pix3[0] + pix4[0])>>2;
			((byte *)(out+j))[1] = (pix1[1] + pix2[1] + pix3[1] + pix4[1])>>2;
			((byte *)(out+j))[2] = (pix1[2] + pix2[2] + pix3[2] + pix4[2])>>2;
			((byte *)(out+j))[3] = (pix1[3] + pix2[3] + pix3[3] + pix4[3])>>2;
		}
	}
}

/*
================
GL_LightScaleTexture

Scale up the pixel values in a texture to increase the
lighting range
================
*/
void GL_LightScaleTexture (unsigned *in, int inwidth, int inheight, qboolean only_gamma, int bpp)
{
	int		inc = (bpp == 24)?3:4;

	if ( only_gamma )
	{
		int		i, c;
		byte	*p;

		p = (byte *)in;

		c = inwidth*inheight;
		for (i=0 ; i<c ; i++, p+=inc)
		{
			p[0] = gammatable[p[0]];
			p[1] = gammatable[p[1]];
			p[2] = gammatable[p[2]];
		}
	}
	else
	{
		int		i, c;
		byte	*p;

		p = (byte *)in;

		c = inwidth*inheight;
		for (i=0 ; i<c ; i++, p+=inc)
		{
			p[0] = gammatable[intensitytable[p[0]]];
			p[1] = gammatable[intensitytable[p[1]]];
			p[2] = gammatable[intensitytable[p[2]]];
		}
	}
}

/*
================
GL_MipMap

Operates in place, quartering the size of the texture
================
*/
void GL_MipMap (byte *in, int width, int height)
{
	int		i, j;
	byte	*out;

	width <<=2;
	height >>= 1;
	out = in;
	for (i=0 ; i<height ; i++, in+=width)
	{
		for (j=0 ; j<width ; j+=8, out+=4, in+=8)
		{
			out[0] = (in[0] + in[4] + in[width+0] + in[width+4])>>2;
			out[1] = (in[1] + in[5] + in[width+1] + in[width+5])>>2;
			out[2] = (in[2] + in[6] + in[width+2] + in[width+6])>>2;
			out[3] = (in[3] + in[7] + in[width+3] + in[width+7])>>2;
		}
	}
}

/*
===============
GL_Upload32

Returns has_alpha
===============
*/

int			upload_width, upload_height;

qboolean GL_Upload32 (unsigned *data, int width, int height, qboolean mipmap, int bpp, image_t *image)
{
	int			samples;
	unsigned	*scaled;
	int			scaled_width, scaled_height;
	int			i, c;
	byte		*scan;
	int			comp;

	for (scaled_width = 1 ; scaled_width < width ; scaled_width<<=1)
		;
	if (gl_round_down->value && scaled_width > width && mipmap)
		scaled_width >>= 1;
	for (scaled_height = 1 ; scaled_height < height ; scaled_height<<=1)
		;
	if (gl_round_down->value && scaled_height > height && mipmap)
		scaled_height >>= 1;

	// let people sample down the world textures for speed
	if (mipmap)
	{
		scaled_width >>= (int)gl_picmip->value;
		scaled_height >>= (int)gl_picmip->value;
	}

	// check size
	scaled_width = (scaled_width > max_tsize)?max_tsize:(scaled_width < 1)?1:scaled_width;
	scaled_height = (scaled_height > max_tsize)?max_tsize:(scaled_height < 1)?1:scaled_height;

	// scan the texture for any non-255 alpha
	samples = gl_solid_format;
	if (bpp != 24)
	{
		c = width*height;
		scan = ((byte *)data) + 3;
		for (i=0 ; i<c ; i++, scan += 4)
		{
			if ( *scan != 255 )
			{
				samples = gl_alpha_format;
				break;
			}
		}
	}

	comp = (samples == gl_solid_format) ? gl_tex_solid_format : gl_tex_alpha_format;

	if (scaled_width == width && scaled_height == height)
	{
		scaled = data;
		if (!mipmap)
		{
			qglTexImage2D (GL_TEXTURE_2D, 0, comp, scaled_width, scaled_height, 0, GL_RGBA, GL_UNSIGNED_BYTE, data);
			goto done;
		}
		if (scaled != data)
			memcpy (scaled, data, width * height * sizeof(int));
	}
	else
	{
		scaled = malloc(scaled_width * scaled_height * sizeof(int));
		if (!scaled)
			VID_Printf (ERR_DROP, "GL_Upload32: out of memory");

		GL_ResampleTexture (data, width, height, scaled, scaled_width, scaled_height);
	}

	if (image->type != it_pic)
	{
		if (!strstr(image->name, "fx/caustic")/*|| !strstr(image->name, "fx/detail")*/)
			GL_LightScaleTexture (scaled, scaled_width, scaled_height, !mipmap, bpp);
	}

	if (gl_config.sgismipmap)
	{
		int j;
		for (j=0 ; j<NUM_GL_MODES ; j++)
			if ( !Q_strcasecmp( modes[j].name, gl_texturemode->string ) )
				break;

		qglTexParameteri (GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, modes[j].minimize);
		qglTexParameteri (GL_TEXTURE_2D, GL_GENERATE_MIPMAP_SGIS, GL_TRUE);
	}

	if (gl_config.anisotropy)
	{
		if (gl_ext_texture_filter_anisotropic->value > (float)max_aniso)
			qglTexParameterf (GL_TEXTURE_2D, GL_TEXTURE_MAX_ANISOTROPY_EXT, max_aniso);
		else
			qglTexParameterf (GL_TEXTURE_2D, GL_TEXTURE_MAX_ANISOTROPY_EXT, (int)gl_ext_texture_filter_anisotropic->value);
	}

	qglTexImage2D (GL_TEXTURE_2D, 0, comp, scaled_width, scaled_height, 0, GL_RGBA, GL_UNSIGNED_BYTE, scaled);

	if (mipmap && !(gl_config.sgismipmap))
	{
		int		miplevel = 0;

		while (scaled_width > 1 || scaled_height > 1)
		{
			GL_MipMap ((byte *)scaled, scaled_width, scaled_height);

			scaled_width >>= 1;
			scaled_height >>= 1;

			if (scaled_width < 1) scaled_width = 1;
			if (scaled_height < 1) scaled_height = 1;

			miplevel++;

			qglTexImage2D (GL_TEXTURE_2D, miplevel, comp, scaled_width, scaled_height, 0, GL_RGBA, GL_UNSIGNED_BYTE, scaled);
		}
	}
done: ;

	upload_width = scaled_width; upload_height = scaled_height;

	qglTexParameterf(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, (mipmap)?gl_filter_min:gl_filter_max);
	qglTexParameterf(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, gl_filter_max);

	if (scaled && scaled != data)
		free (scaled);

	return (samples == gl_alpha_format);
}

/*
===============
GL_Upload8

Returns has_alpha
===============
*/

qboolean GL_Upload8 (byte *data, int width, int height, qboolean mipmap, image_t *image)
{
	unsigned int *trans;
	int			i, s;
	int			p;

	s = width*height;
	trans = malloc(width * height * sizeof(int));

	for (i=0 ; i<s ; i++)
	{
		p = data[i];
		trans[i] = d_8to24table[p];
		
		if (p == 255)
		{	// transparent, so scan around for another color
			// to avoid alpha fringes
			// FIXME: do a full flood fill so mips work...
			if (i > width && data[i-width] != 255)
				p = data[i-width];
			else if (i < s-width && data[i+width] != 255)
				p = data[i+width];
			else if (i > 0 && data[i-1] != 255)
				p = data[i-1];
			else if (i < s-1 && data[i+1] != 255)
				p = data[i+1];
			else
				p = 0;
			// copy rgb components
			((byte *)&trans[i])[0] = ((byte *)&d_8to24table[p])[0];
			((byte *)&trans[i])[1] = ((byte *)&d_8to24table[p])[1];
			((byte *)&trans[i])[2] = ((byte *)&d_8to24table[p])[2];
		}
	}

	return GL_Upload32 (trans, width, height, mipmap, 8, image);
}

/*
================
GL_LoadPic

This is also used as an entry point for the generated r_notexture
================
*/
image_t *GL_LoadPic (char *name, byte *pic, int width, int height, imagetype_t type, int bits)
{
	image_t		*image;
	int			i;
	qboolean	mipmap = false;

	// NiceAss: Nexus'added vars for texture scaling
	// mattx86: hires_scaling
#ifdef HIRES_TEX_SCALING
	miptex_t *mt;
	int len;
	char s[128];
#endif

	// find a free image_t
	for (i=0, image=gltextures ; i<numgltextures ; i++,image++)
	{
		if (!image->texnum)
			break;
	}
	if (i == numgltextures)
	{
		if (numgltextures == MAX_GLTEXTURES)
			VID_Printf (ERR_DROP, "MAX_GLTEXTURES");
		numgltextures++;
	}
	image = &gltextures[i];

	if (strlen(name) >= sizeof(image->name))
		VID_Printf (ERR_DROP, "Draw_LoadPic: \"%s\" is too long", name);
	strcpy (image->name, name);
	image->registration_sequence = registration_sequence;

	image->width = width;
	image->height = height;
	image->type = type;

//	if (type == it_skin && bits == 8)
//		R_FloodFillSkin(pic, width, height);

	// <q2ice: scale high resolution textures properly>
	// mattx86: hires_scaling
#ifdef HIRES_TEX_SCALING
	len = strlen(name);
	strcpy(s,name);

	if (!strcmp(s+len-4, ".tga") || !strcmp(s+len-4, ".png"))
	{
		s[len-3] = 'w';
		s[len-2] = 'a';
		s[len-1] = 'l';
		FS_LoadFile (s, (void **)&mt);	//load .wal file

		if (!mt)
			goto nomatch;	// no luck, dont mess with the size

		image->width = LittleLong (mt->width);
		image->height = LittleLong (mt->height);
		FS_FreeFile ((void *)mt);
	}
	nomatch:
	if (type == it_skin && bits == 8)
		R_FloodFillSkin(pic, width, height);
#endif
	// </q2ice: scale high resolution textures properly>

	// load little pics into the scrap
	if (image->type == it_pic && image->width < 64 && image->height < 64)
	{
		if (bits == 8)
		{
			int		x, y;
			int		i, j, k;
			int		texnum;

			texnum = Scrap_AllocBlock (image->width, image->height, &x, &y);
			if (texnum == -1)
				goto nonscrap;
			scrap_dirty = true;

			// copy the texels into the scrap block
			k = 0;
			for (i=0 ; i<image->height ; i++)
				for (j=0 ; j<image->width ; j++, k++)
					scrap_texels[texnum][(y+i)*BLOCK_WIDTH + x + j] = pic[k];
			image->texnum = TEXNUM_SCRAPS + texnum;
			image->has_alpha = true;
			image->sl = (x+0.01)/(float)BLOCK_WIDTH;
			image->sh = (x+image->width-0.01)/(float)BLOCK_WIDTH;
			image->tl = (y+0.01)/(float)BLOCK_WIDTH;
			image->th = (y+image->height-0.01)/(float)BLOCK_WIDTH;	
			return image;
		}
	}
nonscrap:
	image->texnum = TEXNUM_IMAGES + (image - gltextures);
	GL_Bind(image->texnum);

	mipmap = ((image->type != it_pic) && (image->type != it_sky))?true:false;

	if (bits == 8)
		image->has_alpha = GL_Upload8 (pic, width, height, mipmap, image);
	else
		image->has_alpha = GL_Upload32 ((unsigned *)pic, width, height, mipmap, bits, image );

	image->upload_width = upload_width;		// after power of 2 and scales
	image->upload_height = upload_height;
	image->sl = 0;
	image->sh = 1;
	image->tl = 0;
	image->th = 1;

	return image;
}


/*
================
GL_LoadWal
================
*/
image_t *GL_LoadWal (char *name)
{
	miptex_t	*mt;
	int			width, height, ofs;
	image_t		*image;

	FS_LoadFile (name, (void **)&mt);
	if (!mt)
	{
		VID_Printf (PRINT_ALL, "GL_FindImage: can't load %s\n", name);
		return r_notexture;
	}

	width = LittleLong (mt->width);
	height = LittleLong (mt->height);
	ofs = LittleLong (mt->offsets[0]);

	image = GL_LoadPic (name, (byte *)mt + ofs, width, height, it_wall, 8);

	FS_FreeFile ((void *)mt);

	return image;
}

/*
===============
GL_FindImage

Finds or loads the given image
===============
*/
image_t	*GL_FindImage (char *name, imagetype_t type)
{
#ifdef TGAPNG_TEX_LOADING
	char	tganame[MAX_QPATH]; // mattx86: tgapng_loading
	char	pngname[MAX_QPATH]; // mattx86: tgapng_loading
#endif
	image_t	*image;
	int		i, len;
	byte	*pic, *palette;
	int		width, height;

	if (!name)
		return NULL;	//	VID_Printf (ERR_DROP, "GL_FindImage: NULL name");
	len = strlen(name);
	if (len<5)
		return NULL;	//	VID_Printf (ERR_DROP, "GL_FindImage: bad name: %s", name);

#ifdef TGAPNG_TEX_LOADING // mattx86: tgapng_loading
	COM_StripExtension(name, tganame);
	COM_StripExtension(name, pngname);
	strcat(tganame, ".tga");
	strcat(pngname, ".png");
#endif

	// look for it
	for (i = 0, image = gltextures; i < numgltextures; i++, image++)
	{
#ifdef TGAPNG_TEX_LOADING // mattx86: tgapng_loading
		if (!strcmp(pngname, image->name))
		{
			image->registration_sequence = registration_sequence;
			return image;
		}
		else if (!strcmp(tganame, image->name))
		{
			image->registration_sequence = registration_sequence;
			return image;
		}
		else if (!strcmp(name, image->name))
		{
			image->registration_sequence = registration_sequence;
			return image;
		}
#else
		if (!strcmp(name, image->name))
		{
			image->registration_sequence = registration_sequence;
			return image;
		}
#endif
	}

	//
	// load the pic from disk
	//
	pic = NULL;
	palette = NULL;

////mattx86: tgapng_loading - BEGIN
#ifdef TGAPNG_TEX_LOADING
	LoadPNG(pngname, &pic, &width, &height);
	if ( !pic )
	{
		LoadTGA(tganame, &pic, &width, &height);
		if ( !pic )
		{
			if ( !strcmp(name + (len - 4), ".pcx") )
			{
				LoadPCX(name, &pic, &palette, &width, &height);
				if ( !pic )
					return NULL; // VID_Printf (ERR_DROP, "GL_FindImage: can't load %s", name);
				image = GL_LoadPic(name, pic, width, height, type, 8);
			}
			else if ( !strcmp(name + (len - 4), ".wal") )
			{
				image = GL_LoadWal(name);
			}
			else
				return NULL;	//	VID_Printf (ERR_DROP, "GL_FindImage: bad extension on: %s", name);
		}
		else
			image = GL_LoadPic(tganame, pic, width, height, type, 32);
	}
	else
		image = GL_LoadPic(pngname, pic, width, height, type, 32);
#else
	if (!strcmp(name+len-4, ".pcx"))
	{
		LoadPCX (name, &pic, &palette, &width, &height);
		if (!pic)
			return NULL; // ri.Sys_Error (ERR_DROP, "GL_FindImage: can't load %s", name);
		image = GL_LoadPic (name, pic, width, height, type, 8);
	}
	else if (!strcmp(name+len-4, ".wal"))
	{
		image = GL_LoadWal (name);
	}
	else if (!strcmp(name+len-4, ".tga"))
	{
		LoadTGA (name, &pic, &width, &height);
		if (!pic)
			return NULL; // ri.Sys_Error (ERR_DROP, "GL_FindImage: can't load %s", name);
		image = GL_LoadPic (name, pic, width, height, type, 32);
	}
	else
		return NULL;	//	ri.Sys_Error (ERR_DROP, "GL_FindImage: bad extension on: %s", name);
#endif
////mattx86: tgapng_loading - END

	if (pic)
		free(pic);
	if (palette)
		free(palette);

	return image;
}



/*
===============
R_RegisterSkin
===============
*/
struct image_s *R_RegisterSkin (char *name)
{
	return GL_FindImage (name, it_skin);
}


/*
================
GL_FreeUnusedImages

Any image that was not touched on this registration sequence
will be freed.
================
*/
void GL_FreeUnusedImages (void)
{
	int		i;
	image_t	*image;

	// never free r_notexture or particle texture
	r_notexture->registration_sequence = registration_sequence;
	//r_particletexture->registration_sequence = registration_sequence;
	for (i = 0; i < PT_MAX; i++) {
		r_particletexture[i]->registration_sequence = registration_sequence;
	}

	for (i=0, image=gltextures ; i<numgltextures ; i++, image++)
	{
		if (image->registration_sequence == registration_sequence)
			continue;		// used this sequence
		if (!image->registration_sequence)
			continue;		// free image_t slot
		if (image->type == it_pic)
			continue;		// don't free pics
		// free it
		qglDeleteTextures (1, &image->texnum);
		memset (image, 0, sizeof(*image));
	}
}


/*
===============
Draw_GetPalette
===============
*/
int Draw_GetPalette (void)
{
	int		i;
	int		r, g, b;
	unsigned	v;
	byte	*pic, *pal;
	int		width, height;

	// get the palette

	LoadPCX ("pics/colormap.pcx", &pic, &pal, &width, &height);
	if (!pal)
		VID_Printf (ERR_FATAL, "Couldn't load pics/colormap.pcx");

	for (i=0 ; i<256 ; i++)
	{
		float avg, d1, d2, d3, dr, dg, db, sat;
		r = pal[i*3+0];
		g = pal[i*3+1];
		b = pal[i*3+2];
		
		//** DMP adjust saturation
		avg = (r + g + b + 2) / 3;			// first calc grey value we'll desaturate to
		dr = avg - r;						// calc distance from grey value to each gun
		dg = avg - g;
		db = avg - b;
		d1 = abs(r - g);					// find greatest distance between all the guns
		d2 = abs(g - b);
		d3 = abs(b - r);
		if (d1 > d2)						// we will use this for our existing saturation
			if (d1 > d3)
				sat = d1;
			else
				if (d2 > d3)
					sat = d2;
				else
					sat = d3;
		else
			if (d2 > d3)
				sat = d2;
			else
				sat = d3;
		sat /= 255.0;						// convert existing saturationn to ratio
		sat = 1.0 - sat;					// invert so the most saturation causes the desaturation to lessen
		sat *= (1.0 - 0.75/*r_saturation->value*/); // scale the desaturation value so our desaturation is non-linear (keeps lava looking good)
		dr *= sat;							// scale the differences by the amount we want to desaturate
		dg *= sat;
		db *= sat;
		r += dr;							// now move the gun values towards total grey by the amount we desaturated
		g += dg;
		b += db;
		//** DMP end saturation mod
		
		v = (255<<24) + (r<<0) + (g<<8) + (b<<16);
		d_8to24table[i] = LittleLong(v);
	}

	d_8to24table[255] &= LittleLong(0xffffff);	// 255 is transparent

	free (pic);
	free (pal);

	return 0;
}


/*
===============
GL_InitImages
===============
*/
void	GL_InitImages (void)
{
	int		i, j;
	float	g = vid_gamma->value;

	registration_sequence = 1;

	// init intensity conversions
// Vic - begin
	if ( gl_config.mtexcombine )
		intensity = Cvar_Get ("intensity", "1", CVAR_ARCHIVE);
	else
		intensity = Cvar_Get ("intensity", "2", CVAR_ARCHIVE);
// Vic - end
//	intensity = Cvar_Get ("intensity", "2", CVAR_ARCHIVE);

	if ( intensity->value < 1 )
		Cvar_Set( "intensity", "1" );

	gl_state.inverse_intensity = 1 / intensity->value;

	Draw_GetPalette ();

	if ( gl_config.renderer & ( GL_RENDERER_VOODOO | GL_RENDERER_VOODOO2 ) )
	{
		g = 1.0F;
	}

	for ( i = 0; i < 256; i++ )
	{
		// gammatable
		if ( g == 1 )
		{
			gammatable[i] = i;
		}
		else
		{
			float inf;

			inf = 255 * pow ( (i+0.5)/255.5 , g ) + 0.5;
			if (inf < 0)
				inf = 0;
			else if (inf > 255)
				inf = 255;
			gammatable[i] = inf;
		}

		// intensitytable
		j = i * (int)intensity->value;
		if (j > 255)
			j = 255;
		intensitytable[i] = j;
	}
}

/*
===============
GL_ShutdownImages
===============
*/
void	GL_ShutdownImages (void)
{
	int		i;
	image_t	*image;

	for (i=0, image=gltextures ; i<numgltextures ; i++, image++)
	{
		if (!image->registration_sequence)
			continue;		// free image_t slot
		// free it
		qglDeleteTextures (1, &image->texnum);
		memset (image, 0, sizeof(*image));
	}
}