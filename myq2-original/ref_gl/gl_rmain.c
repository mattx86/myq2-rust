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
// r_main.c
#include "gl_local.h"
#include "../qcommon/myq2opts.h" // mattx86: myq2opts.h
#ifdef DO_REFLECTIVE_WATER // mattx86: reflective_water
	#include "gl_refl.h"
#endif

int				fogType = 3;
float			fogDensity = 0.0f;

void R_Clear (void);

viddef_t	vid;

int GL_TEXTURE0, GL_TEXTURE1, GL_TEXTURE2, GL_TEXTURE3;

model_t		*r_worldmodel;

float		gldepthmin, gldepthmax;

glconfig_t gl_config;
glstate_t  gl_state;

image_t		*r_notexture;		// use for bad textures
image_t		*r_particletexture[PT_MAX];	// little dot for particles

entity_t	*currententity;
model_t		*currentmodel;

cplane_t	frustum[4];

int			r_visframecount;	// bumped when going to a new PVS
int			r_framecount;		// used for dlight push checking

int			c_brush_polys, c_alias_polys;

float		v_blend[4];			// final blending color

void GL_Strings_f( void );

// opengl queries
int		max_aniso;
int		max_tsize;

//
// view origin
//
vec3_t	vup;
vec3_t	vpn;
vec3_t	vright;
vec3_t	r_origin;

float	r_world_matrix[16];
float	r_base_world_matrix[16];

//
// screen size info
//
refdef_t	r_newrefdef;

int		r_viewcluster, r_viewcluster2, r_oldviewcluster, r_oldviewcluster2;

cvar_t	*r_norefresh;
cvar_t	*r_drawentities;
cvar_t	*r_drawworld;
cvar_t	*r_speeds;
cvar_t	*r_fullbright;
cvar_t	*r_novis;
cvar_t	*r_nocull;
cvar_t	*r_lerpmodels;
cvar_t	*r_lefthand;

cvar_t	*r_lightlevel;	// FIXME: This is a HACK to get the client's light level

cvar_t	*r_overbrightbits; // Vic - overbrightbits

cvar_t	*gl_nosubimage;
cvar_t	*gl_allow_software;

cvar_t	*gl_vertex_arrays;

cvar_t	*gl_particle_min_size;
cvar_t	*gl_particle_max_size;
cvar_t	*gl_particle_size;
cvar_t	*gl_particle_att_a;
cvar_t	*gl_particle_att_b;
cvar_t	*gl_particle_att_c;

cvar_t	*gl_ext_swapinterval;
cvar_t	*gl_ext_multitexture;
//cvar_t	*gl_ext_pointparameters;
cvar_t	*gl_ext_compiled_vertex_array;

cvar_t	*gl_log;
cvar_t	*gl_bitdepth;
cvar_t	*gl_drawbuffer;
cvar_t  *gl_driver;
cvar_t	*gl_lightmap;
cvar_t	*gl_shadows;
cvar_t	*gl_mode;
cvar_t	*gl_dynamic;
cvar_t  *gl_monolightmap;
cvar_t	*gl_modulate;
cvar_t	*gl_nobind;
cvar_t	*gl_round_down;
cvar_t	*gl_picmip;
cvar_t	*gl_skymip;
cvar_t	*gl_showtris;
cvar_t	*gl_ztrick;
cvar_t	*gl_finish;
cvar_t	*gl_clear;
cvar_t	*gl_cull;
cvar_t	*gl_polyblend;
cvar_t	*gl_flashblend;
cvar_t	*gl_playermip;
cvar_t  *gl_saturatelighting;
cvar_t	*gl_swapinterval;
cvar_t	*gl_texturemode;
cvar_t	*gl_texturealphamode;
cvar_t	*gl_texturesolidmode;
cvar_t	*gl_lockpvs;

cvar_t	*gl_ext_texture_filter_anisotropic;
cvar_t	*gl_sgis_generate_mipmap;
cvar_t	*r_celshading; // mattx86: cel_shading
cvar_t	*r_fog; // mattx86: engine_fog
cvar_t	*r_timebasedfx;
cvar_t	*r_detailtexture;  //ep::detail textures
cvar_t	*r_caustics;
cvar_t	*r_displayrefresh; // mattx86: display_refresh
cvar_t	*r_hwgamma; // MrG - BeefQuake - hardware gammaramp
cvar_t	*r_stainmap;
cvar_t	*r_verbose;

#ifdef DO_REFLECTIVE_WATER // mattx86: reflective_water
cvar_t	*gl_refl_alpha;	// MPO	alpha transparency, 1.0 is full bright
cvar_t	*gl_refl_debug;	// MPO
#endif

cvar_t	*gl_3dlabs_broken;

cvar_t	*vid_fullscreen;
cvar_t	*vid_gamma;
cvar_t	*vid_ref;

/*
=================
R_CullBox

Returns true if the box is completely outside the frustom
=================
*/
qboolean R_CullBox (vec3_t mins, vec3_t maxs)
{
	int		i;

	if (r_nocull->value)
		return false;

	for (i=0 ; i<4 ; i++)
		if ( BOX_ON_PLANE_SIDE(mins, maxs, &frustum[i]) == 2)
			return true;
	return false;
}


void R_RotateForEntity (entity_t *e)
{
    qglTranslatef (e->origin[0],  e->origin[1],  e->origin[2]);

    qglRotatef (e->angles[1],  0, 0, 1);
    qglRotatef (-e->angles[0],  0, 1, 0);
    qglRotatef (-e->angles[2],  1, 0, 0);
}

/*
=============================================================

  SPRITE MODELS

=============================================================
*/


/*
=================
R_DrawSpriteModel

=================
*/
void R_DrawSpriteModel (entity_t *e)
{
	float alpha = 1.0F;
	vec3_t	point;
	dsprframe_t	*frame;
	float		*up, *right;
	dsprite_t		*psprite;

	// don't even bother culling, because it's just a single
	// polygon without a surface cache

	psprite = (dsprite_t *)currentmodel->extradata;

#if 0
	if (e->frame < 0 || e->frame >= psprite->numframes)
	{
		VID_Printf (PRINT_ALL, "no such sprite frame %i\n", e->frame);
		e->frame = 0;
	}
#endif
	e->frame %= psprite->numframes;

	frame = &psprite->frames[e->frame];

#if 0
	if (psprite->type == SPR_ORIENTED)
	{	// bullet marks on walls
	vec3_t		v_forward, v_right, v_up;

	AngleVectors (currententity->angles, v_forward, v_right, v_up);
		up = v_up;
		right = v_right;
	}
	else
#endif
	{	// normal sprite
		up = vup;
		right = vright;
	}

	if ( e->flags & RF_TRANSLUCENT )
		alpha = e->alpha;

	if ( alpha != 1.0F )
		qglEnable( GL_BLEND );

	qglColor4f( 1, 1, 1, alpha );

    GL_Bind(currentmodel->skins[e->frame]->texnum);

	if (!gl_config.mtexcombine || !r_overbrightbits->value)
	{
		GL_TexEnv (GL_MODULATE);
	}
	else
	{
		qglTexEnvi(GL_TEXTURE_ENV, GL_TEXTURE_ENV_MODE, GL_COMBINE_EXT);
		qglTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_RGB_EXT, GL_MODULATE);
		qglTexEnvi(GL_TEXTURE_ENV, GL_RGB_SCALE_ARB, r_overbrightbits->value);
		qglTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_ALPHA_ARB, GL_MODULATE);
		GL_TexEnv( GL_COMBINE_EXT );
	}

	if ( alpha == 1.0 )
		qglEnable (GL_ALPHA_TEST);
	else
		qglDisable( GL_ALPHA_TEST );

	qglBegin (GL_QUADS);

	qglTexCoord2f (0, 1);
	VectorMA (e->origin, -frame->origin_y, up, point);
	VectorMA (point, -frame->origin_x, right, point);
	qglVertex3fv (point);

	qglTexCoord2f (0, 0);
	VectorMA (e->origin, frame->height - frame->origin_y, up, point);
	VectorMA (point, -frame->origin_x, right, point);
	qglVertex3fv (point);

	qglTexCoord2f (1, 0);
	VectorMA (e->origin, frame->height - frame->origin_y, up, point);
	VectorMA (point, frame->width - frame->origin_x, right, point);
	qglVertex3fv (point);

	qglTexCoord2f (1, 1);
	VectorMA (e->origin, -frame->origin_y, up, point);
	VectorMA (point, frame->width - frame->origin_x, right, point);
	qglVertex3fv (point);
	
	qglEnd ();

	qglDisable (GL_ALPHA_TEST);
	GL_TexEnv( GL_REPLACE );
	if (gl_config.mtexcombine && r_overbrightbits->value)
		qglTexEnvi(GL_TEXTURE_ENV, GL_RGB_SCALE_ARB, 1);

	if ( alpha != 1.0F )
		qglDisable( GL_BLEND );

	qglColor4f( 1, 1, 1, 1 );
}

//==================================================================================

/*
=============
R_DrawNullModel
=============
*/
void R_DrawNullModel (void)
{
	vec3_t	shadelight;
	int		i;

	if ( currententity->flags & RF_FULLBRIGHT )
		shadelight[0] = shadelight[1] = shadelight[2] = 1.0F;
	else
		R_LightPoint (currententity->origin, shadelight);

    qglPushMatrix ();
	R_RotateForEntity (currententity);

	qglDisable (GL_TEXTURE_2D);
	qglColor3fv (shadelight);

	qglBegin (GL_TRIANGLE_FAN);
	qglVertex3f (0, 0, -16);
	for (i=0 ; i<=4 ; i++)
		qglVertex3f (16*cos(i*M_PI/2), 16*sin(i*M_PI/2), 0);
	qglEnd ();

	qglBegin (GL_TRIANGLE_FAN);
	qglVertex3f (0, 0, 16);
	for (i=4 ; i>=0 ; i--)
		qglVertex3f (16*cos(i*M_PI/2), 16*sin(i*M_PI/2), 0);
	qglEnd ();

	qglColor3f (1,1,1);
	qglPopMatrix ();
	qglEnable (GL_TEXTURE_2D);
}

/*
=============
R_DrawEntitiesOnList
=============
*/
void R_DrawEntitiesOnList (void)
{
	int		i;

	if ( !r_drawentities->value )
		return;

	// draw non-transparent first
	for (i = 0; i < r_newrefdef.num_entities; i++)
	{
		currententity = &r_newrefdef.entities[i];
		if (currententity->flags & RF_TRANSLUCENT)
			continue;

		if (currententity->flags & RF_BEAM)
			R_DrawBeam( currententity );
		else
		{
			currentmodel = currententity->model;
			if ( !currentmodel )
			{
				R_DrawNullModel();
				continue;
			}

			switch (currentmodel->type)
			{
				case mod_alias:
					R_DrawAliasModel( currententity, false );
					break;
				case mod_brush:
					R_DrawBrushModel( currententity );
					break;
				case mod_sprite:
					R_DrawSpriteModel( currententity );
					break;
				default:
					VID_Printf(ERR_DROP, "Bad modeltype");
					break;
			}
		}
	}

	// draw transparent entities
	// we could sort these if it ever becomes a problem...
	qglDepthMask (0);		// no z writes
	for (i = 0; i < r_newrefdef.num_entities; i++)
	{
		currententity = &r_newrefdef.entities[i];
		if ( !(currententity->flags & RF_TRANSLUCENT) )
			continue;

		if (currententity->flags & RF_BEAM)
			R_DrawBeam( currententity );
		else
		{
			currentmodel = currententity->model;
			if ( !currentmodel )
			{
				R_DrawNullModel();
				continue;
			}

			switch (currentmodel->type)
			{
				case mod_alias:
					R_DrawAliasModel( currententity, true );
					break;
				case mod_brush:
					R_DrawBrushModel( currententity );
					break;
				case mod_sprite:
					R_DrawSpriteModel( currententity );
					break;
				default:
					VID_Printf(ERR_DROP, "Bad modeltype");
					break;
			}
		}
	}
	qglDepthMask (1);		// back to writing
}

// c14 added this function to draw spirals
extern void MakeNormalVectors (vec3_t forward, vec3_t right, vec3_t up);

/*
===============
R_DrawParticles
===============
*/
void R_DrawParticles (void)
{
	if (fogDensity > 0.0f)
		qglDisable(GL_FOG);

#if 0
	if ( gl_ext_pointparameters->value && qglPointParameterfEXT )
	{
		int i;
		unsigned char color[4];
		const particle_t *p;

		//qglDepthMask( GL_FALSE ); //majparticle
		qglEnable( GL_BLEND );
		qglDisable( GL_TEXTURE_2D );

		if (!gl_config.mtexcombine || !r_overbrightbits->value)
		{
			GL_TexEnv (GL_MODULATE);
		}
		else
		{
			qglTexEnvi(GL_TEXTURE_ENV, GL_TEXTURE_ENV_MODE, GL_COMBINE_EXT);
			qglTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_RGB_EXT, GL_MODULATE);
			qglTexEnvi(GL_TEXTURE_ENV, GL_RGB_SCALE_ARB, r_overbrightbits->value);
			qglTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_ALPHA_ARB, GL_MODULATE);
			GL_TexEnv( GL_COMBINE_EXT );
		}

		qglPointSize( gl_particle_size->value );
		qglBegin( GL_POINTS );
		for ( i = 0, p = r_newrefdef.particles; i < r_newrefdef.num_particles; i++, p++ )
		{
			*(int *)color = d_8to24table[p->color];
			color[3] = p->alpha*255;

			qglColor4ubv( color );

			qglVertex3fv( p->origin );
		}
		qglEnd();

		qglDisable( GL_BLEND );
		qglColor4f( 1.0F, 1.0F, 1.0F, 1.0F );
		//qglDepthMask( GL_TRUE ); //majparticle
		qglEnable( GL_TEXTURE_2D );
		if (gl_config.mtexcombine && r_overbrightbits->value)
			qglTexEnvi(GL_TEXTURE_ENV, GL_RGB_SCALE_ARB, 1);
	}
	else
#endif

	{
		particle_t	*p;
		vec3_t		up, right;
		byte		color[4];
		float		scale;
		int			i;


// code for default
		GL_Bind(r_particletexture[PT_DEFAULT]->texnum);
		qglDepthMask (GL_FALSE);
		qglEnable (GL_BLEND);
		GL_TexEnv (GL_MODULATE); // mattx86: default particles get double-modulated  =)
		if (!gl_config.mtexcombine || !r_overbrightbits->value)
		{
			GL_TexEnv (GL_MODULATE);
		}
		else
		{
			qglTexEnvi(GL_TEXTURE_ENV, GL_TEXTURE_ENV_MODE, GL_COMBINE_EXT);
			qglTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_RGB_EXT, GL_MODULATE);
			qglTexEnvi(GL_TEXTURE_ENV, GL_RGB_SCALE_ARB, r_overbrightbits->value);
			qglTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_ALPHA_ARB, GL_MODULATE);
			GL_TexEnv( GL_COMBINE_EXT );
		}

		qglBegin (GL_QUADS);
		for (p = r_newrefdef.particles, i=0 ; i < r_newrefdef.num_particles ; i++,p++)
		{
			if (p->type != PT_DEFAULT) continue; //ep::particles

			VectorScale (vup, .667, up);
			VectorScale (vright, .667, right);

			// hack a scale up to keep particles from disapearing
			scale = ( p->origin[0] - r_origin[0] ) * vpn[0] + 
					( p->origin[1] - r_origin[1] ) * vpn[1] +
					( p->origin[2] - r_origin[2] ) * vpn[2];
			scale = (scale<20)?1:1+scale*0.004;

			*(int *)color = d_8to24table[p->color];
			color[3] = p->alpha*255;

			qglColor4ubv (color);

			qglTexCoord2f (0.0, 0.0);
			qglVertex3f (	p->origin[0] - right[0]*scale - up[0]*scale,
							p->origin[1] - right[1]*scale - up[1]*scale,
							p->origin[2] - right[2]*scale - up[2]*scale);

			qglTexCoord2f (0.0, 1.0);
			qglVertex3f (	p->origin[0] - right[0]*scale + up[0]*scale,
							p->origin[1] - right[1]*scale + up[1]*scale,
							p->origin[2] - right[2]*scale + up[2]*scale);

			qglTexCoord2f (1.0, 1.0);
			qglVertex3f (	p->origin[0] + right[0]*scale + up[0]*scale,
							p->origin[1] + right[1]*scale + up[1]*scale,
							p->origin[2] + right[2]*scale + up[2]*scale);

			qglTexCoord2f (1.0, 0.0);
			qglVertex3f (	p->origin[0] + right[0]*scale - up[0]*scale,
							p->origin[1] + right[1]*scale - up[1]*scale,
							p->origin[2] + right[2]*scale - up[2]*scale);
		}
		qglEnd ();

		qglDisable (GL_BLEND);
		qglColor4f (1,1,1,1);
		qglDepthMask (GL_TRUE);		// back to normal Z buffering
		GL_TexEnv (GL_REPLACE);
		if (gl_config.mtexcombine && r_overbrightbits->value)
			qglTexEnvi(GL_TEXTURE_ENV, GL_RGB_SCALE_ARB, 1);


// code for fire
		GL_Bind(r_particletexture[PT_FIRE]->texnum);
		qglDepthMask (GL_FALSE);
		qglEnable (GL_BLEND);
		if (!gl_config.mtexcombine || !r_overbrightbits->value)
		{
			GL_TexEnv (GL_MODULATE);
		}
		else
		{
			qglTexEnvi(GL_TEXTURE_ENV, GL_TEXTURE_ENV_MODE, GL_COMBINE_EXT);
			qglTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_RGB_EXT, GL_MODULATE);
			qglTexEnvi(GL_TEXTURE_ENV, GL_RGB_SCALE_ARB, r_overbrightbits->value);
			qglTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_ALPHA_ARB, GL_MODULATE);
			GL_TexEnv( GL_COMBINE_EXT );
		}

		VectorScale (vup, .8, up);
		VectorScale (vright, .8, right);

		qglBegin (GL_QUADS);
		for (p = r_newrefdef.particles, i=0 ; i < r_newrefdef.num_particles ; i++,p++)
		{
			if (p->type != PT_FIRE) continue; //ep::particles

			// hack a scale up to keep particles from disapearing
			scale = ( p->origin[0] - r_origin[0] ) * vpn[0] + 
					( p->origin[1] - r_origin[1] ) * vpn[1] +
					( p->origin[2] - r_origin[2] ) * vpn[2];
			scale = (scale<20)?1:1+scale*0.004;

			*(int *)color = d_8to24table[p->color];
			color[3] = p->alpha*255;

			qglColor4ubv (color);

			qglTexCoord2f (0.0, 0.0);
			qglVertex3f (	p->origin[0] - right[0]*scale - up[0]*scale,
							p->origin[1] - right[1]*scale - up[1]*scale,
							p->origin[2] - right[2]*scale - up[2]*scale);

			qglTexCoord2f (0.0, 1.0);
			qglVertex3f (	p->origin[0] - right[0]*scale + up[0]*scale,
							p->origin[1] - right[1]*scale + up[1]*scale,
							p->origin[2] - right[2]*scale + up[2]*scale);

			qglTexCoord2f (1.0, 1.0);
			qglVertex3f (	p->origin[0] + right[0]*scale + up[0]*scale,
							p->origin[1] + right[1]*scale + up[1]*scale,
							p->origin[2] + right[2]*scale + up[2]*scale);

			qglTexCoord2f (1.0, 0.0);
			qglVertex3f (	p->origin[0] + right[0]*scale - up[0]*scale,
							p->origin[1] + right[1]*scale - up[1]*scale,
							p->origin[2] + right[2]*scale - up[2]*scale);
		}
		qglEnd ();

		qglDisable (GL_BLEND);
		qglColor4f (1,1,1,1);
		qglDepthMask (GL_TRUE);		// back to normal Z buffering
		GL_TexEnv (GL_REPLACE);
		if (gl_config.mtexcombine && r_overbrightbits->value)
			qglTexEnvi(GL_TEXTURE_ENV, GL_RGB_SCALE_ARB, 1);


// code for smoke
		GL_Bind(r_particletexture[PT_SMOKE]->texnum);
		qglDepthMask (GL_FALSE);
		qglEnable (GL_BLEND);
		if (!gl_config.mtexcombine || !r_overbrightbits->value)
		{
			GL_TexEnv (GL_MODULATE);
		}
		else
		{
			qglTexEnvi(GL_TEXTURE_ENV, GL_TEXTURE_ENV_MODE, GL_COMBINE_EXT);
			qglTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_RGB_EXT, GL_MODULATE);
			qglTexEnvi(GL_TEXTURE_ENV, GL_RGB_SCALE_ARB, r_overbrightbits->value);
			qglTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_ALPHA_ARB, GL_MODULATE);
			GL_TexEnv( GL_COMBINE_EXT );
		}

		VectorScale (vup, 3.667, up);
		VectorScale (vright, 3.667, right);

		qglBegin (GL_QUADS);
		for (p = r_newrefdef.particles, i=0 ; i < r_newrefdef.num_particles ; i++,p++)
		{
			if (p->type != PT_SMOKE) continue; //ep::particles

			// hack a scale up to keep particles from disapearing
			scale = ( p->origin[0] - r_origin[0] ) * vpn[0] + 
					( p->origin[1] - r_origin[1] ) * vpn[1] +
					( p->origin[2] - r_origin[2] ) * vpn[2];
			scale = (scale<20)?1:1+scale*0.004;

			*(int *)color = d_8to24table[p->color];
			color[3] = p->alpha*255;

			qglColor4ubv (color);

			qglTexCoord2f (0.0, 0.0);
			qglVertex3f (	p->origin[0] - right[0]*scale - up[0]*scale,
							p->origin[1] - right[1]*scale - up[1]*scale,
							p->origin[2] - right[2]*scale - up[2]*scale);

			qglTexCoord2f (0.0, 1.0);
			qglVertex3f (	p->origin[0] - right[0]*scale + up[0]*scale,
							p->origin[1] - right[1]*scale + up[1]*scale,
							p->origin[2] - right[2]*scale + up[2]*scale);

			qglTexCoord2f (1.0, 1.0);
			qglVertex3f (	p->origin[0] + right[0]*scale + up[0]*scale,
							p->origin[1] + right[1]*scale + up[1]*scale,
							p->origin[2] + right[2]*scale + up[2]*scale);

			qglTexCoord2f (1.0, 0.0);
			qglVertex3f (	p->origin[0] + right[0]*scale - up[0]*scale,
							p->origin[1] + right[1]*scale - up[1]*scale,
							p->origin[2] + right[2]*scale - up[2]*scale);
		}
		qglEnd ();

		qglDisable (GL_BLEND);
		qglColor4f (1,1,1,1);
		qglDepthMask (GL_TRUE);		// back to normal Z buffering
		GL_TexEnv (GL_REPLACE);
		if (gl_config.mtexcombine && r_overbrightbits->value)
			qglTexEnvi(GL_TEXTURE_ENV, GL_RGB_SCALE_ARB, 1);


// code for bubbles
		GL_Bind(r_particletexture[PT_BUBBLE]->texnum);
		qglDepthMask (GL_FALSE);
		qglEnable (GL_BLEND);
		if (!gl_config.mtexcombine || !r_overbrightbits->value)
		{
			GL_TexEnv (GL_MODULATE);
		}
		else
		{
			qglTexEnvi(GL_TEXTURE_ENV, GL_TEXTURE_ENV_MODE, GL_COMBINE_EXT);
			qglTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_RGB_EXT, GL_MODULATE);
			qglTexEnvi(GL_TEXTURE_ENV, GL_RGB_SCALE_ARB, r_overbrightbits->value);
			qglTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_ALPHA_ARB, GL_MODULATE);
			GL_TexEnv( GL_COMBINE_EXT );
		}

		qglBegin (GL_QUADS);
		for (p = r_newrefdef.particles, i=0 ; i < r_newrefdef.num_particles ; i++,p++)
		{
			if (p->type != PT_BUBBLE) continue; //ep::particles

			VectorScale (vup, .667, up);
			VectorScale (vright, .667, right);

			// hack a scale up to keep particles from disapearing
			scale = ( p->origin[0] - r_origin[0] ) * vpn[0] + 
					( p->origin[1] - r_origin[1] ) * vpn[1] +
					( p->origin[2] - r_origin[2] ) * vpn[2];
			scale = (scale<20)?1:1+scale*0.004;

			*(int *)color = d_8to24table[p->color];
			color[3] = p->alpha*255;

			qglColor4ubv (color);

			qglTexCoord2f (0.0, 0.0);
			qglVertex3f (	p->origin[0] - right[0]*scale - up[0]*scale,
							p->origin[1] - right[1]*scale - up[1]*scale,
							p->origin[2] - right[2]*scale - up[2]*scale);

			qglTexCoord2f (0.0, 1.0);
			qglVertex3f (	p->origin[0] - right[0]*scale + up[0]*scale,
							p->origin[1] - right[1]*scale + up[1]*scale,
							p->origin[2] - right[2]*scale + up[2]*scale);

			qglTexCoord2f (1.0, 1.0);
			qglVertex3f (	p->origin[0] + right[0]*scale + up[0]*scale,
							p->origin[1] + right[1]*scale + up[1]*scale,
							p->origin[2] + right[2]*scale + up[2]*scale);

			qglTexCoord2f (1.0, 0.0);
			qglVertex3f (	p->origin[0] + right[0]*scale - up[0]*scale,
							p->origin[1] + right[1]*scale - up[1]*scale,
							p->origin[2] + right[2]*scale - up[2]*scale);
		}
		qglEnd ();

		qglDisable (GL_BLEND);
		qglColor4f (1,1,1,1);
		qglDepthMask (GL_TRUE);		// back to normal Z buffering
		GL_TexEnv (GL_REPLACE);
		if (gl_config.mtexcombine && r_overbrightbits->value)
			qglTexEnvi(GL_TEXTURE_ENV, GL_RGB_SCALE_ARB, 1);


// code for blood
		GL_Bind(r_particletexture[PT_BLOOD]->texnum);
		qglDepthMask (GL_FALSE);
		qglEnable (GL_BLEND);
		if (!gl_config.mtexcombine || !r_overbrightbits->value)
		{
			GL_TexEnv (GL_MODULATE);
		}
		else
		{
			qglTexEnvi(GL_TEXTURE_ENV, GL_TEXTURE_ENV_MODE, GL_COMBINE_EXT);
			qglTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_RGB_EXT, GL_MODULATE);
			qglTexEnvi(GL_TEXTURE_ENV, GL_RGB_SCALE_ARB, r_overbrightbits->value);
			qglTexEnvi(GL_TEXTURE_ENV, GL_COMBINE_ALPHA_ARB, GL_MODULATE);
			GL_TexEnv( GL_COMBINE_EXT );
		}

		VectorScale (vup, 1.667, up);
		VectorScale (vright, 1.667, right);
	
		qglBegin (GL_QUADS);
		for (p = r_newrefdef.particles, i=0 ; i < r_newrefdef.num_particles ; i++,p++)
		{
			if (p->type != PT_BLOOD) continue; //ep::particles

			// hack a scale up to keep particles from disapearing
			scale = ( p->origin[0] - r_origin[0] ) * vpn[0] + 
					( p->origin[1] - r_origin[1] ) * vpn[1] +
					( p->origin[2] - r_origin[2] ) * vpn[2];
			scale = (scale<20)?1:1+scale*0.004;

			*(int *)color = d_8to24table[p->color];
			color[3] = p->alpha*255;

			qglColor4ubv (color);

			qglTexCoord2f (0.0, 0.0);
			qglVertex3f (	p->origin[0] - right[0]*scale - up[0]*scale,
							p->origin[1] - right[1]*scale - up[1]*scale,
							p->origin[2] - right[2]*scale - up[2]*scale);

			qglTexCoord2f (0.0, 1.0);
			qglVertex3f (	p->origin[0] - right[0]*scale + up[0]*scale,
							p->origin[1] - right[1]*scale + up[1]*scale,
							p->origin[2] - right[2]*scale + up[2]*scale);

			qglTexCoord2f (1.0, 1.0);
			qglVertex3f (	p->origin[0] + right[0]*scale + up[0]*scale,
							p->origin[1] + right[1]*scale + up[1]*scale,
							p->origin[2] + right[2]*scale + up[2]*scale);

			qglTexCoord2f (1.0, 0.0);
			qglVertex3f (	p->origin[0] + right[0]*scale - up[0]*scale,
							p->origin[1] + right[1]*scale - up[1]*scale,
							p->origin[2] + right[2]*scale - up[2]*scale);
		}
		qglEnd ();

		qglDisable (GL_BLEND);
		qglColor4f (1,1,1,1);
		qglDepthMask (GL_TRUE);		// back to normal Z buffering
		GL_TexEnv (GL_REPLACE);
		if (gl_config.mtexcombine && r_overbrightbits->value)
			qglTexEnvi(GL_TEXTURE_ENV, GL_RGB_SCALE_ARB, 1);

	} // end main else

	if (fogDensity > 0.0f)
		qglEnable(GL_FOG);
}

/*
============
R_PolyBlend
============
*/
void R_PolyBlend (void)
{
	if (!gl_polyblend->value)
		return;
	if (!v_blend[3])
		return;

	qglDisable (GL_ALPHA_TEST);
	qglEnable (GL_BLEND);
	qglDisable (GL_DEPTH_TEST);
	qglDisable (GL_TEXTURE_2D);

    qglLoadIdentity ();

	// FIXME: get rid of these
    qglRotatef (-90,  1, 0, 0);	    // put Z going up
    qglRotatef (90,  0, 0, 1);	    // put Z going up

	qglColor4fv (v_blend);

	qglBegin (GL_QUADS);

	qglVertex3f (10, 100, 100);
	qglVertex3f (10, -100, 100);
	qglVertex3f (10, -100, -100);
	qglVertex3f (10, 100, -100);
	qglEnd ();

	qglDisable (GL_BLEND);
	qglEnable (GL_TEXTURE_2D);
	qglEnable (GL_ALPHA_TEST);

	qglColor4f(1,1,1,1);
}

//=======================================================================

int SignbitsForPlane (cplane_t *out)
{
	int	bits, j;

	// for fast box on planeside test

	bits = 0;
	for (j=0 ; j<3 ; j++)
	{
		if (out->normal[j] < 0)
			bits |= 1<<j;
	}
	return bits;
}


void R_SetFrustum (void)
{
	int		i;

#if 0
	/*
	** this code is wrong, since it presume a 90 degree FOV both in the
	** horizontal and vertical plane
	*/
	// front side is visible
	VectorAdd (vpn, vright, frustum[0].normal);
	VectorSubtract (vpn, vright, frustum[1].normal);
	VectorAdd (vpn, vup, frustum[2].normal);
	VectorSubtract (vpn, vup, frustum[3].normal);

	// we theoretically don't need to normalize these vectors, but I do it
	// anyway so that debugging is a little easier
	VectorNormalize( frustum[0].normal );
	VectorNormalize( frustum[1].normal );
	VectorNormalize( frustum[2].normal );
	VectorNormalize( frustum[3].normal );
#else
	// rotate VPN right by FOV_X/2 degrees
	RotatePointAroundVector( frustum[0].normal, vup, vpn, -(90-r_newrefdef.fov_x / 2 ) );
	// rotate VPN left by FOV_X/2 degrees
	RotatePointAroundVector( frustum[1].normal, vup, vpn, 90-r_newrefdef.fov_x / 2 );
	// rotate VPN up by FOV_X/2 degrees
	RotatePointAroundVector( frustum[2].normal, vright, vpn, 90-r_newrefdef.fov_y / 2 );
	// rotate VPN down by FOV_X/2 degrees
	RotatePointAroundVector( frustum[3].normal, vright, vpn, -( 90 - r_newrefdef.fov_y / 2 ) );
#endif

	for (i=0 ; i<4 ; i++)
	{
		frustum[i].type = PLANE_ANYZ;
		frustum[i].dist = DotProduct (r_origin, frustum[i].normal);
		frustum[i].signbits = SignbitsForPlane (&frustum[i]);
	}
}

//=======================================================================

/*
===============
R_SetupFrame
===============
*/
void R_SetupFrame (void)
{
	int i;
	mleaf_t	*leaf;

	r_framecount++;

// build the transformation matrix for the given view angles
	VectorCopy (r_newrefdef.vieworg, r_origin);
	AngleVectors (r_newrefdef.viewangles, vpn, vright, vup);

#ifdef DO_REFLECTIVE_WATER // mattx86: reflective_water
	// start MPO
	// we want to look from the mirrored origin's perspective when drawing reflections
	if (g_drawing_refl)
	{
		vec3_t tmp;

		r_origin[2] = (2*g_refl_Z[g_active_refl]) - r_origin[2];	// flip
		
		VectorCopy(r_newrefdef.viewangles, tmp);
		tmp[0] *= -1.0;
		AngleVectors(tmp, vpn, vright, vup);

		if ( !( r_newrefdef.rdflags & RDF_NOWORLDMODEL ) )
		{
			vec3_t	temp;

//			r_oldviewcluster = r_viewcluster;
//			r_oldviewcluster2 = r_viewcluster2;
			leaf = Mod_PointInLeaf (r_origin, r_worldmodel);
			r_viewcluster = leaf->cluster;

			VectorCopy(r_origin, temp);
			temp[2] = g_refl_Z[g_active_refl] + 1;	// just above water level
//				tmp[0] = -45.0;	// HACK
//				AngleVectors(tmp, vpn, vright, vup);

			leaf = Mod_PointInLeaf (temp, r_worldmodel);
			if (!(leaf->contents & CONTENTS_SOLID) &&
				(leaf->cluster != r_viewcluster) )
			{
				r_viewcluster2 = leaf->cluster;
			}
		}
		return;
	}
	// stop MPO
#endif

// current viewcluster
	if ( !( r_newrefdef.rdflags & RDF_NOWORLDMODEL ) )
	{
		r_oldviewcluster = r_viewcluster;
		r_oldviewcluster2 = r_viewcluster2;
		leaf = Mod_PointInLeaf (r_origin, r_worldmodel);
		r_viewcluster = r_viewcluster2 = leaf->cluster;

		// check above and below so crossing solid water doesn't draw wrong
		if (!leaf->contents)
		{	// look down a bit
			vec3_t	temp;

			VectorCopy (r_origin, temp);
			temp[2] -= 16;
			leaf = Mod_PointInLeaf (temp, r_worldmodel);
			if ( !(leaf->contents & CONTENTS_SOLID) &&
				(leaf->cluster != r_viewcluster2) )
				r_viewcluster2 = leaf->cluster;
		}
		else
		{	// look up a bit
			vec3_t	temp;

			VectorCopy (r_origin, temp);
			temp[2] += 16;
			leaf = Mod_PointInLeaf (temp, r_worldmodel);
			if ( !(leaf->contents & CONTENTS_SOLID) &&
				(leaf->cluster != r_viewcluster2) )
				r_viewcluster2 = leaf->cluster;
		}
	}

	for (i=0 ; i<4 ; i++)
		v_blend[i] = r_newrefdef.blend[i];

	c_brush_polys = 0;
	c_alias_polys = 0;

	// clear out the portion of the screen that the NOWORLDMODEL defines
	if ( r_newrefdef.rdflags & RDF_NOWORLDMODEL )
	{
		qglEnable( GL_SCISSOR_TEST );
		qglClearColor( 0.3, 0.3, 0.3, 1 );
		qglScissor( r_newrefdef.x, vid.height - r_newrefdef.height - r_newrefdef.y, r_newrefdef.width, r_newrefdef.height );
		qglClear( GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT );
		qglClearColor( 1, 0, 0.5, 0.5 );
		qglDisable( GL_SCISSOR_TEST );
	}
}

void MYgluPerspective( GLdouble fovy, GLdouble aspect,
		     GLdouble zNear, GLdouble zFar )
{
   GLdouble xmin, xmax, ymin, ymax;

   ymax = zNear * tan( fovy * M_PI / 360.0 );
   ymin = -ymax;

   xmin = ymin * aspect;
   xmax = ymax * aspect;

   xmin += -( 2 * gl_state.camera_separation ) / zNear;
   xmax += -( 2 * gl_state.camera_separation ) / zNear;

#ifndef DO_REFLECTIVE_WATER // mattx86: reflective_water
	qglFrustum( xmin, xmax, ymin, ymax, zNear, zFar );
#else
   mesa_frustum( xmin, xmax, ymin, ymax, zNear, zFar ); //MPO
#endif
}


/*
=============
R_SetupGL
=============
*/
void R_SetupGL (void)
{
	float	screenaspect;
//	float	yfov;
	int		x, x2, y2, y, w, h;
	static qboolean runonce = false;
	static GLdouble farz;	// DMP skybox size change
	GLdouble boxsize;		// DMP skybox size change

	//
	// set up viewport
	//
	x = floor(r_newrefdef.x * vid.width / vid.width);
	x2 = ceil((r_newrefdef.x + r_newrefdef.width) * vid.width / vid.width);
	y = floor(vid.height - r_newrefdef.y * vid.height / vid.height);
	y2 = ceil(vid.height - (r_newrefdef.y + r_newrefdef.height) * vid.height / vid.height);

	w = x2 - x;
	h = y - y2;

#ifndef DO_REFLECTIVE_WATER // mattx86: reflective_water
	qglViewport (x, y2, w, h);
#else
	// start MPO
	// MPO : we only want to set the viewport if we aren't drawing the reflection
	if (!g_drawing_refl)
	{
		qglViewport (x, y2, w, h);	// MPO : note this happens every frame interestingly enough
	}
	else
	{
		qglViewport(0, 0, g_reflTexW, g_reflTexH);	// width/height of texture size, not screen size
	}
	// stop MPO
#endif

	// DMP: calc farz value from skybox size
	// mattx86: skybox_size
	if (!runonce)
	{
		runonce = true;

		boxsize = SKYBOX_SIZE;
		boxsize -= 252 * ceil(boxsize / 2300);
		farz = 1.0;
		while (farz < boxsize)		// DMP: make this value a power-of-2
		{
			farz *= 2.0;
			if (farz >= 65536.0)	// DMP: don't make it larger than this
				break;
	  	}
		farz *= 2.0;	// DMP: double since boxsize is distance from camera to
						//      edge of skybox - not total size of skybox
		VID_Printf(PRINT_DEVELOPER, "farz now set to %g\n", farz);
	}

	//
	// set up projection matrix
	//
    screenaspect = (float)r_newrefdef.width/r_newrefdef.height;
	qglMatrixMode(GL_PROJECTION);
    qglLoadIdentity ();

	MYgluPerspective (r_newrefdef.fov_y, screenaspect, 4, farz); // DMP skybox

	qglCullFace(GL_FRONT);

	qglMatrixMode(GL_MODELVIEW);
    qglLoadIdentity ();

    qglRotatef (-90,  1, 0, 0);	    // put Z going up
    qglRotatef (90,  0, 0, 1);	    // put Z going up
	// MPO : +Z is up, +X is forward, +Y is left according to my calculations

#ifndef DO_REFLECTIVE_WATER // mattx86: relfective_water
    qglRotatef (-r_newrefdef.viewangles[2],  1, 0, 0);
    qglRotatef (-r_newrefdef.viewangles[0],  0, 1, 0);
    qglRotatef (-r_newrefdef.viewangles[1],  0, 0, 1);
    qglTranslatef (-r_newrefdef.vieworg[0],  -r_newrefdef.vieworg[1],  -r_newrefdef.vieworg[2]);
#else
	// start MPO
	// standard transformation
	if (!g_drawing_refl)
	{
	    qglRotatef (-r_newrefdef.viewangles[2],  1, 0, 0);	// MPO : this handles rolling (ie when we strafe side to side we roll slightly)
	    qglRotatef (-r_newrefdef.viewangles[0],  0, 1, 0);	// MPO : this handles up/down rotation
	    qglRotatef (-r_newrefdef.viewangles[1],  0, 0, 1);	// MPO : this handles left/right rotation
	    qglTranslatef (-r_newrefdef.vieworg[0],  -r_newrefdef.vieworg[1],  -r_newrefdef.vieworg[2]);
	    // MPO : this translate call puts the player at the proper spot in the map
	    // MPO : The map is drawn to absolute coordinates
	}
	// mirrored transformation for reflection
	else
	{
		R_DoReflTransform();
		qglTranslatef(0, 0, -REFL_MAGIC_NUMBER);
	}
	// end MPO
#endif

//	if ( gl_state.camera_separation != 0 && gl_state.stereo_enabled )
//		qglTranslatef ( gl_state.camera_separation, 0, 0 );

	qglGetFloatv (GL_MODELVIEW_MATRIX, r_world_matrix);

	//
	// set drawing parms
	//
#ifndef DO_REFLECTIVE_WATER // mattx86: reflective_water
	if (gl_cull->value)
#else
	if ((gl_cull->value) && (!g_drawing_refl))	// MPO : we must disable culling when drawing the reflection
#endif
		qglEnable(GL_CULL_FACE);
	else
		qglDisable(GL_CULL_FACE);

	qglDisable(GL_BLEND);
	qglDisable(GL_ALPHA_TEST);
	qglEnable(GL_DEPTH_TEST);
}

/*
=============
R_Clear
=============
*/
void R_Clear (void)
{
	if (gl_ztrick->value)
	{
		static int trickframe;

		if (gl_clear->value)
			qglClear (GL_COLOR_BUFFER_BIT);

		trickframe++;
		if (trickframe & 1)
		{
			gldepthmin = 0;
			gldepthmax = 0.49999;
			qglDepthFunc (GL_LEQUAL);
		}
		else
		{
			gldepthmin = 1;
			gldepthmax = 0.5;
			qglDepthFunc (GL_GEQUAL);
		}
	}
	else
	{
		if (gl_clear->value)
			qglClear (GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
		else
			qglClear (GL_DEPTH_BUFFER_BIT);
		gldepthmin = 0;
		gldepthmax = 1;
		qglDepthFunc (GL_LEQUAL);
	}

	qglDepthRange (gldepthmin, gldepthmax);

	// Stencil shadows - MrG
	if (gl_shadows->value)
	{
		qglClearStencil(1);
		qglClear(GL_STENCIL_BUFFER_BIT);
	}
}

void R_Flash( void )
{
	R_PolyBlend ();
}

#if 0
trace_t CL_Trace (vec3_t start, float size, int contentmask)
{
		vec3_t forward; // Echon
		vec3_t maxs, mins;

		VectorSet(maxs, size, size, size);
		VectorSet(mins, -size, -size, -size);
		VectorScale(r_newrefdef.vieworg, 8192, forward);	// Echon
		VectorAdd(forward, start, forward);					// Echon
		return CM_BoxTrace (start, forward, mins, maxs, size, contentmask);
}

float EndPosFrom(vec3_t org)
{
	trace_t	trace;
	float dist;
	trace = CL_Trace (org, 0, CONTENTS_SOLID); // Echon

	VectorSubtract(r_newrefdef.vieworg, trace.endpos, org);
	dist = DotProduct (org, org);
	return dist;
//	if (trace.fraction != 1)
//		return false;
//	return true;
}
#endif


/*
================
R_SetupFog

mattx86: engine_fog
================
*/
void R_SetupFog(void)
{
	// mattx86: timebasedfx - begin
	time_t			ctime;
	struct tm		*ltime;
	char			hour_s[3];
	int				hour_i, am;
	float			ampmarray[2][13] =
	{
		// X, hours 1 - 12 (fog from 10:00PM - 6:59AM)
		{ 0.00000, 0.00000, 0.00000, 0.00000, 0.00000, 0.00000, 0.00000, 0.00000, 0.00000, 0.00000, 0.00020, 0.00040, 0.00000 },	// PM - fog comes in heavily when it turns night
		{ 0.00000, 0.00050, 0.00040, 0.00030, 0.00020, 0.00010, 0.00005, 0.00000, 0.00000, 0.00000, 0.00000, 0.00000, 0.00060 }		// AM - fog slowly, smoothly, clears up..hopefully this will be a good transition
	};
	// mattx86: timebasedfx - end

	int				pointContents;
	float			fogColor[4][4] =
	{
		{ 0.00, 0.53, 1.00, 1.00 },	// water	(0)
		{ 0.65, 1.00, 0.00, 1.00 },	// slime	(1)
		{ 1.00, 0.25, 0.00, 1.00 },	// lava		(2)
		{ 0.45, 0.50, 0.50, 1.00 }	// norm fog	(3)
	};

	qglDisable(GL_FOG);

	pointContents = CM_PointContents(r_newrefdef.vieworg, 0);
	if (pointContents & CONTENTS_WATER)			fogType = 0;
	else if (pointContents & CONTENTS_SLIME)	fogType = 1;
	else if (pointContents & CONTENTS_LAVA)		fogType = 2;
	else										fogType = 3;

	if (r_fog->value || (!r_fog->value && (fogType == 1 || fogType == 2)))
	{
		if (r_timebasedfx->value && (fogType == 0 || fogType == 3))
		{
			ctime = time(NULL);
			ltime = localtime(&ctime);
			strftime(hour_s, sizeof(hour_s), "%H", ltime);
			if (hour_s[0] == '0') // trim zero off, otherwise int val. == 2
			{
				hour_s[0] = hour_s[1];
				hour_s[1] = 0;
			}
			hour_i = atoi(hour_s);

			// convert to 12-hour clock
			if ( hour_i <= 11 )		// AM
			{
				am = 1;
				if (hour_i == 0) // 0 = 12AM midnight
					hour_i = 12;
			}
			else					// PM
			{
				am = 0;
				if (hour_i > 12) // leave 12PM noon alone
					hour_i -= 12;
			}
			fogDensity = ampmarray[am][hour_i];
		}
		else
		{
			if (fogType == 1 || fogType == 2)
				fogDensity = 0.1200f;
			else
				fogDensity = 0.0675f;
		}

		if (fogDensity > 0.0f)
		{
			qglDisable(GL_FOG);
			qglFogi(GL_FOG_MODE, GL_LINEAR);
			qglFogfv(GL_FOG_COLOR, fogColor[fogType]);
			qglFogf(GL_FOG_START, 150);
			if (fogType == 3)
				qglFogf(GL_FOG_END, 2300);
			else
				qglFogf(GL_FOG_END, 1800);
			qglFogf(GL_FOG_DENSITY, fogDensity);
			qglEnable(GL_FOG);
			qglHint(GL_FOG_HINT, GL_NICEST);
		}
		else
			qglDisable(GL_FOG);
	}
	else
	{
		qglDisable(GL_FOG);
		fogDensity = 0.0f;
	}
}

/*
================
R_RenderView

r_newrefdef must be set before the first call
================
*/
void R_RenderView (refdef_t *fd)
{
	if (r_norefresh->value)
		return;

	r_newrefdef = *fd;

	if (!r_worldmodel && !( r_newrefdef.rdflags & RDF_NOWORLDMODEL ) )
		VID_Printf (ERR_DROP, "R_RenderView: NULL worldmodel");

	if (r_speeds->value)
	{
		c_brush_polys = 0;
		c_alias_polys = 0;
	}

	R_PushDlights ();

	if (gl_finish->value)
		qglFinish ();

	R_SetupFrame ();

	R_SetFrustum ();

	R_SetupGL ();

#ifdef DO_REFLECTIVE_WATER // mattx86: reflective_water
	// start MPO
	// if we are doing a reflection, we want to do a clip plane now, after
	// we've set up our projection/modelview matrices
	
	if (g_drawing_refl)
	{
		double clipPlane[] = { 0, 0, 1, -g_refl_Z[g_active_refl] };	    	
	    // we need clipping so we don't reflect objects underneath the water
		qglEnable(GL_CLIP_PLANE0);
		qglClipPlane(GL_CLIP_PLANE0, clipPlane);
	}
	// stop MPO
#endif

	R_MarkLeaves ();	// done here so we know if we're in water

	R_SetupFog();

	R_DrawWorld ();

	R_DrawEntitiesOnList ();

	R_RenderDlights ();

	R_DrawParticles ();

	R_DrawAlphaSurfaces ();

#ifndef DO_REFLECTIVE_WATER // mattx86: reflective_water
	R_Flash();

	if (r_speeds->value)
	{
		VID_Printf (PRINT_ALL, "%4i wpoly %4i epoly %i tex %i lmaps\n",
			c_brush_polys, 
			c_alias_polys, 
			c_visible_textures, 
			c_visible_lightmaps); 
	}
#else
	// start MPO
	// if we are doing a reflection, turn off clipping now
	if (g_drawing_refl)
	{
		qglDisable(GL_CLIP_PLANE0);
	}
	// if we aren't doing a reflection then we can do the flash and r speeds
	// (we don't want them showing up in our reflection)
	else
	{
		R_Flash();
	
		if (r_speeds->value)
		{
			VID_Printf (PRINT_ALL, "%4i wpoly %4i epoly %i tex %i lmaps\n",
				c_brush_polys, 
				c_alias_polys, 
				c_visible_textures, 
				c_visible_lightmaps); 
		}
	}
	// stop MPO
#endif
}


void	R_SetGL2D (void)
{
	// set 2D virtual screen size
	qglViewport (0,0, vid.width, vid.height);
	qglMatrixMode(GL_PROJECTION);
    qglLoadIdentity ();
	qglOrtho  (0, vid.width, vid.height, 0, -99999, 99999);
	qglMatrixMode(GL_MODELVIEW);
    qglLoadIdentity ();
	qglDisable (GL_DEPTH_TEST);
	qglDisable (GL_CULL_FACE);
	qglDisable (GL_BLEND);
	qglEnable (GL_ALPHA_TEST);
	qglColor4f (1,1,1,1);
	gl_state.transconsole = true; // mattx86: trans_console
}

static void GL_DrawColoredStereoLinePair( float r, float g, float b, float y )
{
	qglColor3f( r, g, b );
	qglVertex2f( 0, y );
	qglVertex2f( vid.width, y );
	qglColor3f( 0, 0, 0 );
	qglVertex2f( 0, y + 1 );
	qglVertex2f( vid.width, y + 1 );
}

static void GL_DrawStereoPattern( void )
{
	int i;

	if ( !( gl_config.renderer & GL_RENDERER_INTERGRAPH ) )
		return;

	if ( !gl_state.stereo_enabled )
		return;

	R_SetGL2D();

	qglDrawBuffer( GL_BACK_LEFT );

	for ( i = 0; i < 20; i++ )
	{
		qglBegin( GL_LINES );
			GL_DrawColoredStereoLinePair( 1, 0, 0, 0 );
			GL_DrawColoredStereoLinePair( 1, 0, 0, 2 );
			GL_DrawColoredStereoLinePair( 1, 0, 0, 4 );
			GL_DrawColoredStereoLinePair( 1, 0, 0, 6 );
			GL_DrawColoredStereoLinePair( 0, 1, 0, 8 );
			GL_DrawColoredStereoLinePair( 1, 1, 0, 10);
			GL_DrawColoredStereoLinePair( 1, 1, 0, 12);
			GL_DrawColoredStereoLinePair( 0, 1, 0, 14);
		qglEnd();
		
		GLimp_EndFrame();
	}
}


/*
====================
R_SetLightLevel

====================
*/
void R_SetLightLevel (void)
{
	vec3_t		shadelight;

	if (r_newrefdef.rdflags & RDF_NOWORLDMODEL)
		return;

	// save off light value for server to look at (BIG HACK!)

	R_LightPoint (r_newrefdef.vieworg, shadelight);

	// pick the greatest component, which should be the same
	// as the mono value returned by software
	if (shadelight[0] > shadelight[1])
	{
		if (shadelight[0] > shadelight[2])
			r_lightlevel->value = 150*shadelight[0];
		else
			r_lightlevel->value = 150*shadelight[2];
	}
	else
	{
		if (shadelight[1] > shadelight[2])
			r_lightlevel->value = 150*shadelight[1];
		else
			r_lightlevel->value = 150*shadelight[2];
	}

}

/*
@@@@@@@@@@@@@@@@@@@@@
R_RenderFrame

@@@@@@@@@@@@@@@@@@@@@
*/
void R_RenderFrame (refdef_t *fd)
{
#ifdef DO_REFLECTIVE_WATER // mattx86: reflective_water
	// start MPO
	// step 1 : detect all reflective surfaces
	static byte p = 0;
	p++;

	if((gl_refl_alpha->value > 0) && !(r_newrefdef.rdflags & RDF_UNDERWATER)) {

		if (g_refl_enabled && p%10==0) {
			R_clear_refl();	// clear out reflections we found last frame
			R_RecursiveFindRefl (r_worldmodel->nodes);
		}

		if (g_refl_enabled){
			R_UpdateReflTex(fd);	// render all reflections onto textures (this is slow)
		}

	}//if
	// end MPO
#endif

	R_RenderView( fd );
	R_SetLightLevel ();
	R_SetGL2D ();

#ifdef DO_REFLECTIVE_WATER // mattx86: reflective_water
	// start MPO
	// if debugging is enabled and reflections are enabled.. draw it
	if ((gl_refl_debug->value) && (g_refl_enabled))
	{
		R_DrawDebugReflTexture();
	}
	// end MPO
#endif
}


void R_Register( void )
{
	r_lefthand = Cvar_Get( "hand", "2", CVAR_USERINFO | CVAR_ARCHIVE );
	r_norefresh = Cvar_Get ("r_norefresh", "0", CVAR_ZERO);
	r_fullbright = Cvar_Get ("r_fullbright", "0", CVAR_ZERO);
	r_drawentities = Cvar_Get ("r_drawentities", "1", CVAR_ZERO);
	r_drawworld = Cvar_Get ("r_drawworld", "1", CVAR_ZERO);
	r_novis = Cvar_Get ("r_novis", "0", CVAR_ZERO);
	r_nocull = Cvar_Get ("r_nocull", "0", CVAR_ZERO);
	r_lerpmodels = Cvar_Get ("r_lerpmodels", "1", CVAR_ZERO);
	r_speeds = Cvar_Get ("r_speeds", "0", CVAR_ZERO);

	r_lightlevel = Cvar_Get ("r_lightlevel", "0", CVAR_ZERO);

	r_overbrightbits = Cvar_Get("r_overbrightbits", "2", CVAR_ARCHIVE); // Vic - overbrightbits

	gl_nosubimage = Cvar_Get( "gl_nosubimage", "0", CVAR_ARCHIVE);
	gl_allow_software = Cvar_Get( "gl_allow_software", "0", CVAR_ARCHIVE);

	gl_particle_min_size = Cvar_Get( "gl_particle_min_size", "2", CVAR_ARCHIVE );
	gl_particle_max_size = Cvar_Get( "gl_particle_max_size", "40", CVAR_ARCHIVE );
	gl_particle_size = Cvar_Get( "gl_particle_size", "40", CVAR_ARCHIVE );
	gl_particle_att_a = Cvar_Get( "gl_particle_att_a", "0.01", CVAR_ARCHIVE );
	gl_particle_att_b = Cvar_Get( "gl_particle_att_b", "0.0", CVAR_ARCHIVE );
	gl_particle_att_c = Cvar_Get( "gl_particle_att_c", "0.01", CVAR_ARCHIVE );

	gl_modulate = Cvar_Get ("gl_modulate", "1.5", CVAR_ARCHIVE );
	gl_log = Cvar_Get( "gl_log", "0", CVAR_ZERO);
	gl_bitdepth = Cvar_Get( "gl_bitdepth", "0", CVAR_ARCHIVE);
	gl_mode = Cvar_Get( "gl_mode", "4", CVAR_ARCHIVE );
	gl_lightmap = Cvar_Get ("gl_lightmap", "0", CVAR_ZERO);
	gl_shadows = Cvar_Get ("gl_shadows", "1", CVAR_ARCHIVE );
	gl_dynamic = Cvar_Get ("gl_dynamic", "1", CVAR_ARCHIVE);
	gl_nobind = Cvar_Get ("gl_nobind", "0", CVAR_ARCHIVE);
	gl_round_down = Cvar_Get ("gl_round_down", "1", CVAR_ARCHIVE);
	gl_picmip = Cvar_Get ("gl_picmip", "0", CVAR_ARCHIVE);
	gl_skymip = Cvar_Get ("gl_skymip", "0", CVAR_ARCHIVE);
	gl_showtris = Cvar_Get ("gl_showtris", "0", CVAR_ZERO);
	gl_ztrick = Cvar_Get ("gl_ztrick", "0", CVAR_ARCHIVE);
	gl_finish = Cvar_Get ("gl_finish", "0", CVAR_ARCHIVE);
	gl_clear = Cvar_Get ("gl_clear", "0", CVAR_ZERO);
	gl_cull = Cvar_Get ("gl_cull", "1", CVAR_ARCHIVE);
	gl_polyblend = Cvar_Get ("gl_polyblend", "0", CVAR_ARCHIVE);
	gl_flashblend = Cvar_Get ("gl_flashblend", "0", CVAR_ARCHIVE);
	gl_playermip = Cvar_Get ("gl_playermip", "0", CVAR_ARCHIVE);
	gl_monolightmap = Cvar_Get( "gl_monolightmap", "0", CVAR_ZERO);
	gl_driver = Cvar_Get( "gl_driver", "opengl32", CVAR_ARCHIVE );
	gl_texturemode = Cvar_Get( "gl_texturemode", "GL_LINEAR_MIPMAP_LINEAR", CVAR_ARCHIVE );
	gl_texturealphamode = Cvar_Get( "gl_texturealphamode", "default", CVAR_ZERO );
	gl_texturesolidmode = Cvar_Get( "gl_texturesolidmode", "default", CVAR_ZERO );
	gl_lockpvs = Cvar_Get( "gl_lockpvs", "0", CVAR_ZERO);

	gl_vertex_arrays = Cvar_Get( "gl_vertex_arrays", "0", CVAR_ARCHIVE );

	gl_ext_swapinterval = Cvar_Get( "gl_ext_swapinterval", "1", CVAR_ARCHIVE );
	gl_ext_multitexture = Cvar_Get( "gl_ext_multitexture", "1", CVAR_ARCHIVE );
	//gl_ext_pointparameters = Cvar_Get( "gl_ext_pointparameters", "1", CVAR_ARCHIVE );
	gl_ext_compiled_vertex_array = Cvar_Get( "gl_ext_compiled_vertex_array", "1", CVAR_ARCHIVE );

	gl_drawbuffer = Cvar_Get( "gl_drawbuffer", "GL_BACK", CVAR_ARCHIVE);
	gl_swapinterval = Cvar_Get( "gl_swapinterval", "1", CVAR_ARCHIVE );

	gl_saturatelighting = Cvar_Get( "gl_saturatelighting", "0", CVAR_ARCHIVE);

	gl_3dlabs_broken = Cvar_Get( "gl_3dlabs_broken", "0", CVAR_ARCHIVE );

	gl_ext_texture_filter_anisotropic =	Cvar_Get("gl_ext_texture_filter_anisotropic", "0", CVAR_ARCHIVE);
	gl_sgis_generate_mipmap	= Cvar_Get("gl_sgis_generate_mipmap", "0", CVAR_ARCHIVE);
	r_celshading = Cvar_Get("r_celshading", "0", CVAR_ARCHIVE); // mattx86: cel_shading
	r_fog = Cvar_Get("r_fog", "0", CVAR_ARCHIVE); // mattx86: engine_fog
	r_timebasedfx = Cvar_Get("r_timebasedfx", "1", CVAR_ARCHIVE);
	r_detailtexture = Cvar_Get("r_detailtexture", "7", CVAR_ARCHIVE);  //ep::detail textures
	r_caustics = Cvar_Get("r_caustics", "1", CVAR_ARCHIVE);
	r_displayrefresh = Cvar_Get("r_displayrefresh", "0", CVAR_ARCHIVE); // mattx86: display_refresh
	r_hwgamma = Cvar_Get("r_hwgamma", "0", CVAR_ARCHIVE); // MrG - BeefQuake - hardware gammaramp
	r_stainmap = Cvar_Get("r_stainmap", "1", CVAR_ARCHIVE);
	r_verbose = Cvar_Get("r_verbose", "0", CVAR_ZERO);

#ifdef DO_REFLECTIVE_WATER // mattx86: reflective_water
	gl_refl_alpha = Cvar_Get("gl_refl_alpha", "0", CVAR_ARCHIVE);
	gl_refl_debug = Cvar_Get("gl_refl_debug", "0", CVAR_ZERO);
#endif

	vid_fullscreen = Cvar_Get( "vid_fullscreen", "1", CVAR_ARCHIVE );
	vid_gamma = Cvar_Get( "vid_gamma", "0.6", CVAR_ARCHIVE );
	vid_ref = Cvar_Get( "vid_ref", "gl", CVAR_ZERO );

	Cmd_AddCommand( "imagelist", GL_ImageList_f );
	Cmd_AddCommand( "screenshot", GL_ScreenShot_f );
	Cmd_AddCommand( "modellist", Mod_Modellist_f );
	Cmd_AddCommand( "gl_strings", GL_Strings_f );
}

/*
==================
R_SetMode
==================
*/
qboolean R_SetMode (void)
{
	rserr_t err;
	qboolean fullscreen;

	if ( vid_fullscreen->modified && !gl_config.allow_cds )
	{
		VID_Printf( PRINT_ALL, "R_SetMode() - CDS not allowed with this driver\n" );
		Cvar_SetValue( "vid_fullscreen", !vid_fullscreen->value );
		vid_fullscreen->modified = false;
	}

	fullscreen = vid_fullscreen->value;

	vid_fullscreen->modified = false;
	gl_mode->modified = false;

	if ( ( err = GLimp_SetMode( &vid.width, &vid.height, gl_mode->value, fullscreen ) ) == rserr_ok )
	{
		gl_state.prev_mode = gl_mode->value;
	}
	else
	{
		if ( err == rserr_invalid_fullscreen )
		{
			Cvar_SetValue( "vid_fullscreen", 0);
			vid_fullscreen->modified = false;
			VID_Printf( PRINT_ALL, "ref_gl::R_SetMode() - fullscreen unavailable in this mode\n" );
			if ( ( err = GLimp_SetMode( &vid.width, &vid.height, gl_mode->value, false ) ) == rserr_ok )
				return true;
		}
		else if ( err == rserr_invalid_mode )
		{
			Cvar_SetValue( "gl_mode", gl_state.prev_mode );
			gl_mode->modified = false;
			VID_Printf( PRINT_ALL, "ref_gl::R_SetMode() - invalid mode\n" );
		}

		// try setting it back to something safe
		if ( ( err = GLimp_SetMode( &vid.width, &vid.height, gl_state.prev_mode, false ) ) != rserr_ok )
		{
			VID_Printf( PRINT_ALL, "ref_gl::R_SetMode() - could not revert to safe mode\n" );
			return false;
		}
	}
	return true;
}

/*
===============
R_Init
===============
*/
void PowerofTwo (unsigned int *var)
{
	int		i, powersoftwo[] = {2,4,8,16,32,64,128,256,512,1024,2048,4096,8192};

	for (i=0 ; i<14 ; i++)
	{
		if (powersoftwo[i] == *var)
		{
			break;
		}
		else if (powersoftwo[i+1] > *var)
		{
			*var = powersoftwo[i];
			break;
		}
	}
}

int R_Init( void *hinstance, void *hWnd )
{	
	char renderer_buffer[1000];
	char vendor_buffer[1000];
	int		err;
	int		j;
	extern float r_turbsin[256];

	for ( j = 0; j < 256; j++ )
	{
		r_turbsin[j] *= 0.5;
	}

	VID_Printf (PRINT_INFO, "ref_gl version: "REF_VERSION"\n");

	//reversed for saturation control
	R_Register();
	Draw_GetPalette ();

	// initialize our QGL dynamic bindings
	if ( !QGL_Init( gl_driver->string ) )
	{
		QGL_Shutdown();
        VID_Printf (PRINT_ALL, "ref_gl::R_Init() - could not load \"%s\"\n", gl_driver->string );
		return -1;
	}

	// initialize OS-specific parts of OpenGL
	if ( !GLimp_Init( hinstance, hWnd ) )
	{
		QGL_Shutdown();
		return -1;
	}

	// set our "safe" modes
	gl_state.prev_mode = 3;

	// create the window and set up the context
	if ( !R_SetMode () )
	{
		QGL_Shutdown();
        VID_Printf (PRINT_ALL, "ref_gl::R_Init() - could not R_SetMode()\n" );
		return -1;
	}

	VID_MenuInit();

	/*
	** get our various GL strings
	*/
	gl_config.vendor_string = qglGetString (GL_VENDOR);
	VID_Printf (PRINT_INFO, "GL_VENDOR: %s\n", gl_config.vendor_string );
	gl_config.renderer_string = qglGetString (GL_RENDERER);
	VID_Printf (PRINT_INFO, "GL_RENDERER: %s\n", gl_config.renderer_string );
	gl_config.version_string = qglGetString (GL_VERSION);
	VID_Printf (PRINT_INFO, "GL_VERSION: %s\n", gl_config.version_string );
	gl_config.extensions_string = qglGetString (GL_EXTENSIONS);
	VID_Printf (PRINT_INFO, "GL_EXTENSIONS: %s\n", gl_config.extensions_string );

	strcpy( renderer_buffer, gl_config.renderer_string );
	strlwr( renderer_buffer );

	strcpy( vendor_buffer, gl_config.vendor_string );
	strlwr( vendor_buffer );

	if ( strstr( renderer_buffer, "voodoo" ) )
	{
		if ( !strstr( renderer_buffer, "rush" ) )
			gl_config.renderer = GL_RENDERER_VOODOO;
		else
			gl_config.renderer = GL_RENDERER_VOODOO_RUSH;
	}
	else if ( strstr( vendor_buffer, "sgi" ) )
		gl_config.renderer = GL_RENDERER_SGI;
	else if ( strstr( renderer_buffer, "permedia" ) )
		gl_config.renderer = GL_RENDERER_PERMEDIA2;
	else if ( strstr( renderer_buffer, "glint" ) )
		gl_config.renderer = GL_RENDERER_GLINT_MX;
	else if ( strstr( renderer_buffer, "glzicd" ) )
		gl_config.renderer = GL_RENDERER_REALIZM;
	else if ( strstr( renderer_buffer, "gdi" ) )
		gl_config.renderer = GL_RENDERER_MCD;
	else if ( strstr( renderer_buffer, "pcx2" ) )
		gl_config.renderer = GL_RENDERER_PCX2;
	else if ( strstr( renderer_buffer, "verite" ) )
		gl_config.renderer = GL_RENDERER_RENDITION;
	else
		gl_config.renderer = GL_RENDERER_OTHER;

	if ( toupper( gl_monolightmap->string[1] ) != 'F' )
	{
		if ( gl_config.renderer == GL_RENDERER_PERMEDIA2 )
		{
			Cvar_Set( "gl_monolightmap", "A" );
			VID_Printf( PRINT_INFO, "...using gl_monolightmap 'a'\n" );
		}
		else if ( gl_config.renderer & GL_RENDERER_POWERVR ) 
		{
			Cvar_Set( "gl_monolightmap", "0" );
		}
		else
		{
			Cvar_Set( "gl_monolightmap", "0" );
		}
	}

	// power vr can't have anything stay in the framebuffer, so
	// the screen needs to redraw the tiled background every frame
	if ( gl_config.renderer & GL_RENDERER_POWERVR ) 
	{
		Cvar_Set( "scr_drawall", "1" );
	}
	else
	{
		Cvar_Set( "scr_drawall", "0" );
	}

#ifdef __linux__
	Cvar_SetValue( "gl_finish", 1 );
#endif

	// MCD has buffering issues
	if ( gl_config.renderer == GL_RENDERER_MCD )
	{
		Cvar_SetValue( "gl_finish", 1 );
	}

	if ( gl_config.renderer & GL_RENDERER_3DLABS )
	{
		if ( gl_3dlabs_broken->value )
			gl_config.allow_cds = false;
		else
			gl_config.allow_cds = true;
	}
	else
	{
		gl_config.allow_cds = true;
	}

	if ( gl_config.allow_cds )
		VID_Printf( PRINT_INFO, "...allowing CDS\n" );
	else
		VID_Printf( PRINT_INFO, "...disabling CDS\n" );

	/*
	** grab extensions
	*/
	if ( strstr( gl_config.extensions_string, "GL_EXT_compiled_vertex_array" ) || 
		 strstr( gl_config.extensions_string, "GL_SGI_compiled_vertex_array" ) )
	{
		VID_Printf( PRINT_INFO, "...enabling GL_EXT_compiled_vertex_array\n" );
		qglLockArraysEXT = ( void * ) qwglGetProcAddress( "glLockArraysEXT" );
		qglUnlockArraysEXT = ( void * ) qwglGetProcAddress( "glUnlockArraysEXT" );
	}
	else
	{
		VID_Printf( PRINT_INFO, "...GL_EXT_compiled_vertex_array not found\n" );
	}

#ifdef _WIN32
	if ( strstr( gl_config.extensions_string, "WGL_EXT_swap_control" ) )
	{
		qwglSwapIntervalEXT = ( BOOL (WINAPI *)(int)) qwglGetProcAddress( "wglSwapIntervalEXT" );
		VID_Printf( PRINT_INFO, "...enabling WGL_EXT_swap_control\n" );
	}
	else
	{
		VID_Printf( PRINT_INFO, "...WGL_EXT_swap_control not found\n" );
	}
#endif

#if 0
	if ( strstr( gl_config.extensions_string, "GL_EXT_point_parameters" ) )
	{
		if ( gl_ext_pointparameters->value )
		{
			qglPointParameterfEXT = ( void (APIENTRY *)( GLenum, GLfloat ) ) qwglGetProcAddress( "glPointParameterfEXT" );
			qglPointParameterfvEXT = ( void (APIENTRY *)( GLenum, const GLfloat * ) ) qwglGetProcAddress( "glPointParameterfvEXT" );
			VID_Printf( PRINT_INFO, "...using GL_EXT_point_parameters\n" );
		}
		else
		{
			VID_Printf( PRINT_INFO, "...ignoring GL_EXT_point_parameters\n" );
		}
	}
	else
	{
		VID_Printf( PRINT_INFO, "...GL_EXT_point_parameters not found\n" );
	}
#endif

	if ( strstr( gl_config.extensions_string, "GL_ARB_multitexture" ) )
	{
		if ( gl_ext_multitexture->value )
		{
			VID_Printf( PRINT_INFO, "...using GL_ARB_multitexture\n" );
			qglMTexCoord2fSGIS = ( void * ) qwglGetProcAddress( "glMultiTexCoord2fARB" );
			qglActiveTextureARB = ( void * ) qwglGetProcAddress( "glActiveTextureARB" );
			qglClientActiveTextureARB = ( void * ) qwglGetProcAddress( "glClientActiveTextureARB" );
			GL_TEXTURE0 = GL_TEXTURE0_ARB;
			GL_TEXTURE1 = GL_TEXTURE1_ARB;
			GL_TEXTURE2 = GL_TEXTURE2_ARB;
			GL_TEXTURE3 = GL_TEXTURE3_ARB;
		}
		else
		{
			VID_Printf( PRINT_INFO, "...ignoring GL_ARB_multitexture\n" );
		}
	}
	else
	{
		VID_Printf( PRINT_INFO, "...GL_ARB_multitexture not found\n" );
	}

	if ( strstr( gl_config.extensions_string, "GL_SGIS_multitexture" ) )
	{
		if ( qglActiveTextureARB )
		{
			VID_Printf( PRINT_INFO, "...GL_SGIS_multitexture deprecated in favor of ARB_multitexture\n" );
		}
		else if ( gl_ext_multitexture->value )
		{
			VID_Printf( PRINT_INFO, "...using GL_SGIS_multitexture\n" );
			qglMTexCoord2fSGIS = ( void * ) qwglGetProcAddress( "glMTexCoord2fSGIS" );
			qglSelectTextureSGIS = ( void * ) qwglGetProcAddress( "glSelectTextureSGIS" );
			GL_TEXTURE0 = GL_TEXTURE0_SGIS;
			GL_TEXTURE1 = GL_TEXTURE1_SGIS;
			GL_TEXTURE2 = GL_TEXTURE2_SGIS;
			GL_TEXTURE3 = GL_TEXTURE3_SGIS;
		}
		else
		{
			VID_Printf( PRINT_INFO, "...ignoring GL_SGIS_multitexture\n" );
		}
	}
	else
	{
		VID_Printf( PRINT_INFO, "...GL_SGIS_multitexture not found\n" );
	}

// Vic - begin
	gl_config.mtexcombine = false;
	if ( strstr( gl_config.extensions_string, "GL_ARB_texture_env_combine" ) )
	{
		if ( r_overbrightbits->value )
		{
			VID_Printf(PRINT_INFO, "...using GL_ARB_texture_env_combine\n");
			gl_config.mtexcombine = true;
		}
		else
		{
			VID_Printf(PRINT_INFO, "...ignoring GL_ARB_texture_env_combine\n");
		}
	}
	else
	{
		VID_Printf(PRINT_INFO, "...GL_ARB_texture_env_combine not found\n");
	}

	if ( !gl_config.mtexcombine )
	{
		if ( strstr( gl_config.extensions_string, "GL_EXT_texture_env_combine" ) )
		{
			if ( r_overbrightbits->value )
			{
				VID_Printf(PRINT_INFO, "...using GL_EXT_texture_env_combine\n");
				gl_config.mtexcombine = true;
			}
			else
			{
				VID_Printf(PRINT_INFO, "...ignoring GL_EXT_texture_env_combine\n");
			}
		}
		else
		{
			VID_Printf(PRINT_INFO, "...GL_EXT_texture_env_combine not found\n");
		}
	}
// Vic - end

	gl_config.anisotropy = false;
	if ( strstr( gl_config.extensions_string, "GL_EXT_texture_filter_anisotropic" ) )
	{
		if (gl_ext_texture_filter_anisotropic->value)
		{
			gl_config.anisotropy = true;
			VID_Printf( PRINT_INFO, "...using GL_EXT_texture_filter_anisotropic\n" );
		}
		else
		{
			VID_Printf( PRINT_INFO, "...ignoring GL_EXT_texture_filter_anisotropic\n" );
		}
	}
	else
	{
		VID_Printf( PRINT_INFO, "...GL_EXT_texture_filter_anisotropic not found\n" );
	}

	gl_config.sgismipmap = false;
	if ( strstr( gl_config.extensions_string, "GL_SGIS_generate_mipmap" ) )
	{
		if (gl_sgis_generate_mipmap->value)
		{
			gl_config.sgismipmap = true;
			VID_Printf( PRINT_INFO, "...using GL_SGIS_generate_mipmap\n" );
		}
		else
		{
			VID_Printf( PRINT_INFO, "...ignoring GL_SGIS_generate_mipmap\n" );
		}
	}
	else
	{
		VID_Printf( PRINT_INFO, "...GL_SGIS_generate_mipmap not found\n" );
	}

	// retreive information
	qglGetIntegerv (GL_MAX_TEXTURE_MAX_ANISOTROPY_EXT, &max_aniso);
	qglGetIntegerv (GL_MAX_TEXTURE_SIZE, &max_tsize);
	qglGetIntegerv (GL_MAX_TEXTURE_UNITS, &gl_state.num_tmu);
	PowerofTwo (&max_tsize);

	// display information
	VID_Printf (PRINT_INFO, "---------- OpenGL Queries ----------\n");
	VID_Printf( PRINT_INFO, "Maximum Anisotropy: %i\n", max_aniso);
	VID_Printf( PRINT_INFO, "Maximum Texture Size: %ix%i\n", max_tsize, max_tsize);
	VID_Printf( PRINT_INFO, "Maximum TMU: %i\n", gl_state.num_tmu);

	GL_SetDefaultState();

	/*
	** draw our stereo patterns
	*/
#if 0 // commented out until H3D pays us the money they owe us
	GL_DrawStereoPattern();
#endif

	GL_InitImages ();
	Mod_Init ();
	R_InitParticleTexture ();
	Draw_InitLocal ();

	err = qglGetError();
	if ( err != GL_NO_ERROR )
		VID_Printf (PRINT_ALL, "glGetError() = 0x%x\n", err);

#ifdef DO_REFLECTIVE_WATER // mattx86: reflective_water
	R_init_refl(); // MPO : init reflections
#endif
}

/*
===============
R_Shutdown
===============
*/
void R_Shutdown (void)
{	
	Cmd_RemoveCommand ("modellist");
	Cmd_RemoveCommand ("screenshot");
	Cmd_RemoveCommand ("imagelist");
	Cmd_RemoveCommand ("gl_strings");

	Mod_FreeAll ();

	GL_ShutdownImages ();

	/*
	** shut down OS specific OpenGL stuff like contexts, etc.
	*/
	GLimp_Shutdown();

	/*
	** shutdown our QGL subsystem
	*/
	QGL_Shutdown();
}



/*
@@@@@@@@@@@@@@@@@@@@@
R_BeginFrame
@@@@@@@@@@@@@@@@@@@@@
*/
void UpdateGammaRamp(void);
void R_BeginFrame( float camera_separation )
{

	gl_state.camera_separation = camera_separation;

	/*
	** change modes if necessary
	*/
#ifndef AUTO_CVAR // mattx86: auto_cvar
	if ( gl_mode->modified || vid_fullscreen->modified )
	{	// FIXME: only restart if CDS is required
	/*	cvar_t	*ref;

		ref = Cvar_Get ("vid_ref", "gl", CVAR_ARCHIVE);
		ref->modified = true;	*/
		Cbuf_AddText("vid_restart\n");
	}
#endif

	if ( gl_log->modified )
	{
		GLimp_EnableLogging( gl_log->value );
		gl_log->modified = false;
	}

	if ( gl_log->value )
	{
		GLimp_LogNewFrame();
	}

	/*
	** update 3Dfx gamma -- it is expected that a user will do a vid_restart
	** after tweaking this value
	*/
	if ( vid_gamma->modified )
	{
		vid_gamma->modified = false;

		if (gl_config.gammaramp)
		{	// MrG - BeefQuake - Hardware Gammaramp Support
			UpdateGammaRamp ();
		}

		if ( gl_config.renderer & ( GL_RENDERER_VOODOO ) )
		{
			char envbuffer[1024];
			float g;

			g = 2.00 * ( 0.8 - ( vid_gamma->value - 0.5 ) ) + 1.0F;
			Com_sprintf( envbuffer, sizeof(envbuffer), "SSTV2_GAMMA=%f", g );
			putenv( envbuffer );
			Com_sprintf( envbuffer, sizeof(envbuffer), "SST_GAMMA=%f", g );
			putenv( envbuffer );
		}
	}

	GLimp_BeginFrame( camera_separation );

	/*
	** go into 2D mode
	*/
	qglViewport (0,0, vid.width, vid.height);
	qglMatrixMode(GL_PROJECTION);
    qglLoadIdentity ();
	qglOrtho  (0, vid.width, vid.height, 0, -99999, 99999);
	qglMatrixMode(GL_MODELVIEW);
    qglLoadIdentity ();
	qglDisable (GL_DEPTH_TEST);
	qglDisable (GL_CULL_FACE);
	qglDisable (GL_BLEND);
	qglEnable (GL_ALPHA_TEST);
	qglColor4f (1,1,1,1);

	/*
	** draw buffer stuff
	*/
	if ( gl_drawbuffer->modified )
	{
		gl_drawbuffer->modified = false;

		if ( gl_state.camera_separation == 0 || !gl_state.stereo_enabled )
		{
			if ( Q_strcasecmp( gl_drawbuffer->string, "GL_FRONT" ) == 0 )
				qglDrawBuffer( GL_FRONT );
			else
				qglDrawBuffer( GL_BACK );
		}
	}

	/*
	** texturemode stuff
	*/
	if ( gl_texturemode->modified )
	{
		GL_TextureMode( gl_texturemode->string );
		gl_texturemode->modified = false;
	}

	if ( gl_texturealphamode->modified )
	{
		GL_TextureAlphaMode( gl_texturealphamode->string );
		gl_texturealphamode->modified = false;
	}

	if ( gl_texturesolidmode->modified )
	{
		GL_TextureSolidMode( gl_texturesolidmode->string );
		gl_texturesolidmode->modified = false;
	}

	/*
	** swapinterval stuff
	*/
	GL_UpdateSwapInterval();

	//
	// clear screen if desired
	//
	R_Clear ();
}

/*
=============
R_SetPalette
=============
*/
unsigned r_rawpalette[256];
void R_SetPalette ( const unsigned char *palette)
{
	int		i;

	byte *rp = ( byte * ) r_rawpalette;

	if ( palette )
	{
		for ( i = 0; i < 256; i++ )
		{
			rp[i*4+0] = palette[i*3+0];
			rp[i*4+1] = palette[i*3+1];
			rp[i*4+2] = palette[i*3+2];
			rp[i*4+3] = 0xff;
		}
	}
	else
	{
		for ( i = 0; i < 256; i++ )
		{
			rp[i*4+0] = d_8to24table[i] & 0xff;
			rp[i*4+1] = ( d_8to24table[i] >> 8 ) & 0xff;
			rp[i*4+2] = ( d_8to24table[i] >> 16 ) & 0xff;
			rp[i*4+3] = 0xff;
		}
	}

	qglClearColor (0,0,0,0);
	qglClear (GL_COLOR_BUFFER_BIT);
	qglClearColor (1,0, 0.5 , 0.5);
}

/*
** R_DrawBeam
*/
void R_DrawBeam( entity_t *e )
{
#define NUM_BEAM_SEGS 6

	int	i;
	float r, g, b;

	vec3_t perpvec;
	vec3_t direction, normalized_direction;
	vec3_t	start_points[NUM_BEAM_SEGS], end_points[NUM_BEAM_SEGS];
	vec3_t oldorigin, origin;

	oldorigin[0] = e->oldorigin[0];
	oldorigin[1] = e->oldorigin[1];
	oldorigin[2] = e->oldorigin[2];

	origin[0] = e->origin[0];
	origin[1] = e->origin[1];
	origin[2] = e->origin[2];

	normalized_direction[0] = direction[0] = oldorigin[0] - origin[0];
	normalized_direction[1] = direction[1] = oldorigin[1] - origin[1];
	normalized_direction[2] = direction[2] = oldorigin[2] - origin[2];

	if ( VectorNormalize( normalized_direction ) == 0 )
		return;

	PerpendicularVector( perpvec, normalized_direction );
	VectorScale( perpvec, e->frame / 2, perpvec );

	for ( i = 0; i < 6; i++ )
	{
		RotatePointAroundVector( start_points[i], normalized_direction, perpvec, (360.0/NUM_BEAM_SEGS)*i );
		VectorAdd( start_points[i], origin, start_points[i] );
		VectorAdd( start_points[i], direction, end_points[i] );
	}

	qglDisable( GL_TEXTURE_2D );
	qglEnable( GL_BLEND );
	qglDepthMask( GL_FALSE );

	r = ( d_8to24table[e->skinnum & 0xFF] ) & 0xFF;
	g = ( d_8to24table[e->skinnum & 0xFF] >> 8 ) & 0xFF;
	b = ( d_8to24table[e->skinnum & 0xFF] >> 16 ) & 0xFF;

	r *= 1/255.0F;
	g *= 1/255.0F;
	b *= 1/255.0F;

	qglColor4f( r, g, b, e->alpha );

	qglBegin( GL_TRIANGLE_STRIP );
	for ( i = 0; i < NUM_BEAM_SEGS; i++ )
	{
		qglVertex3fv( start_points[i] );
		qglVertex3fv( end_points[i] );
		qglVertex3fv( start_points[(i+1)%NUM_BEAM_SEGS] );
		qglVertex3fv( end_points[(i+1)%NUM_BEAM_SEGS] );
	}
	qglEnd();

	qglEnable( GL_TEXTURE_2D );
	qglDisable( GL_BLEND );
	qglDepthMask( GL_TRUE );
}

//===================================================================


void	R_BeginRegistration (char *map);
struct model_s	*R_RegisterModel (char *name);
struct image_s	*R_RegisterSkin (char *name);
void R_SetSky (char *name, float rotate, vec3_t axis);
void	R_EndRegistration (void);

void	R_RenderFrame (refdef_t *fd);

struct image_s	*Draw_FindPic (char *name);

void	Draw_Pic (int x, int y, char *name);
void	Draw_Char (int x, int y, int c);
void	Draw_TileClear (int x, int y, int w, int h, char *name);
void	Draw_Fill (int x, int y, int w, int h, int c, float alpha);
void	Draw_FadeScreen (void);
