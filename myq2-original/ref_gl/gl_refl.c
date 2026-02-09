// gl_refl.c
// by Matt Ownby

// adds reflective water to the Quake2 engine

#include "../qcommon/myq2opts.h" // mattx86: myq2opts.h
#ifdef DO_REFLECTIVE_WATER // mattx86: reflective_water


#include "gl_local.h"
#include "gl_refl.h"


// width and height of the texture we are gonna use to capture our reflection
#define REFL_TEXW 512
#define REFL_TEXH 512

unsigned int g_reflTexW = REFL_TEXW;	// dynamic size of reflective texture
unsigned int g_reflTexH = REFL_TEXH;

int g_num_refl = 0;	// how many reflections we need to generate
float g_refl_Z[MAX_REFL];	// the Z (vertical) value of each reflection
int g_tex_num[MAX_REFL];	// corresponding texture numbers for each reflection
int g_active_refl = 0;	// which reflection is being rendered at the moment

// whether we are actively rendering a reflection of the world
// (instead of the world itself)
qboolean g_drawing_refl = false;
qboolean g_refl_enabled = true;	// whether reflections should be drawn at all

float g_last_known_fov = 0.0;	// this is no longer necessary

// one-time initialization function
void R_init_refl()
{
	unsigned char *buf = NULL;
	int i = 0;
	
	for (i = 0; i < MAX_REFL; i++)
	{
		buf = (unsigned char *) malloc(REFL_TEXW * REFL_TEXH * 3);	// create empty buffer for texture
		if (buf)
		{
			memset(buf, 255, (REFL_TEXW * REFL_TEXH * 3));	// fill it with white color so we can easily see where our tex border is
			g_tex_num[i] = txm_genTexObject(buf, REFL_TEXW, REFL_TEXH, GL_RGB,false,true);	// make this texture
			free(buf);	// once we've made texture memory, we don't need the sys ram anymore
		}
		// else malloc failed, so this texture was not created
		else
		{
			fprintf(stderr, "Malloc failed?\n");
			exit(1);	// unsafe exit, but we don't ever expect malloc to fail anyway
		}
	}

	// if screen dimensions are smaller than texture size, we have to use screen dimensions instead (doh!)
	g_reflTexW = (vid.width < REFL_TEXW) ? vid.width : REFL_TEXW;
	g_reflTexH = (vid.height < REFL_TEXH) ? vid.height : REFL_TEXH;

	VID_Printf(PRINT_INFO, "Reflective water textures initialized\n");
}

// clears our reflection array
void R_clear_refl()
{
	g_num_refl = 0;
}

// adds a reflection to our array as long as it hasn't already been added,
// and the array still has room.
// If a reflection isn't added it, you will be able to visually detect it in
// the game :)
void R_add_refl(float Z)
{
	int i = 0;

	// make sure this isn't a duplicate entry
	// (I expect a lot of duplicates, which is why I put this check first)
	for (; i < g_num_refl; i++)
	{
		// if this is a duplicate entry then we don't want to add anything
		if (g_refl_Z[i] == Z)
		{
			return;
		}
	}

	// make sure we have room to add
	if (g_num_refl < MAX_REFL)
	{
		g_refl_Z[g_num_refl] = Z;
//		printf("Added reflection at %f\n", Z);	// debug only
		g_num_refl++;
	}
	// else we're at our limit, so just ignore
}

/////

// this routine taken from the glBase package
static int txm_genTexObject(unsigned char *texData, int w, int h,
								int format, qboolean repeat, qboolean mipmap)
{
	unsigned int texNum;

	qglGenTextures(1, &texNum);

	repeat = false;
	mipmap = false;

	if (texData) {

		qglBindTexture(GL_TEXTURE_2D, texNum);
		qglPixelStorei(GL_UNPACK_ALIGNMENT, 1);

		/* Set the tiling mode */
		if (repeat) {
			qglTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_REPEAT);
			qglTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_REPEAT);
		}
		else {
			qglTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP);
			qglTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP);
		}

		/* Set the filtering */
		if (mipmap) {
//			qglTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
//			qglTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER,
//													GL_LINEAR_MIPMAP_LINEAR);
//			if (gl_config.sgismipmap)
//			{
//				qglTexParameteri(GL_TEXTURE_2D, GL_GENERATE_MIPMAP_SGIS, GL_TRUE);
				// an nvidia DynamicTexturing.pdf document told me to use this instead of gluBuild2DMipMaps
//			}
//			else
//			{
//				gluBuild2DMipmaps(GL_TEXTURE_2D, format, w, h,
//						format, GL_UNSIGNED_BYTE, texData);
//			}

		}
		else {
			qglTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
			qglTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
			qglTexImage2D(GL_TEXTURE_2D, 0, format, w, h, 0,
						format, GL_UNSIGNED_BYTE, texData);
		}
	}
	return texNum;
}

// based off of R_RecursiveWorldNode,
// this locates all reflective surfaces and their associated height
void R_RecursiveFindRefl (mnode_t *node)
{
	int			c, side, sidebit;
	cplane_t	*plane;
	msurface_t	*surf, **mark;
	mleaf_t		*pleaf;
	float		dot;
	//image_t		*image;

	if (node->contents == CONTENTS_SOLID)
		return;		// solid

	if (node->visframe != r_visframecount)
		return;

	// MPO : if this function returns true, it means that the polygon is not visible
	// in the frustum, therefore drawing it would be a waste of resources
	if (R_CullBox (node->minmaxs, node->minmaxs+3))
		return;

// if a leaf node, draw stuff
	if (node->contents != -1)
	{
		pleaf = (mleaf_t *)node;

		// check for door connected areas
		if (r_newrefdef.areabits)
		{
			if (! (r_newrefdef.areabits[pleaf->area>>3] & (1<<(pleaf->area&7)) ) )
				return;		// not visible
		}

		mark = pleaf->firstmarksurface;
		c = pleaf->nummarksurfaces;

		if (c)
		{
			do
			{
				(*mark)->visframe = r_framecount;
				mark++;
			} while (--c);
		}

		return;
	}

// node is just a decision point, so go down the apropriate sides

// find which side of the node we are on
	plane = node->plane;

	switch (plane->type)
	{
	case PLANE_X:
		dot = r_newrefdef.vieworg[0] - plane->dist;
		break;
	case PLANE_Y:
		dot = r_newrefdef.vieworg[1] - plane->dist;
		break;
	case PLANE_Z:
		dot = r_newrefdef.vieworg[2] - plane->dist;
		break;
	default:
		dot = DotProduct (r_newrefdef.vieworg, plane->normal) - plane->dist;
		break;
	}

	if (dot >= 0)
	{
		side = 0;
		sidebit = 0;
	}
	else
	{
		side = 1;
		sidebit = SURF_PLANEBACK;
	}

// recurse down the children, front side first
	R_RecursiveFindRefl (node->children[side]);

	// draw stuff
	for ( c = node->numsurfaces, surf = r_worldmodel->surfaces + node->firstsurface; c ; c--, surf++)
	{
		if (surf->visframe != r_framecount)
			continue;

		if ( (surf->flags & SURF_PLANEBACK) != sidebit )
			continue;		// wrong side

		// MPO : from this point onward, we should be dealing with visible surfaces
		// start MPO
		
		// if this is a reflective surface ...
		if ((surf->flags & SURF_DRAWTURB & (SURF_TRANS33|SURF_TRANS66) ) &&
			(surf->texinfo->flags & (SURF_TRANS33|SURF_TRANS66)) && 
			!( r_newrefdef.rdflags & RDF_UNDERWATER ))

		{
			// and if it is flat on the Z plane ...
			if (plane->type == PLANE_Z)
			{
				R_add_refl(surf->polys->verts[0][2]);	// add it!
			}
		}
		// stop MPO
	}

	// recurse down the back side
	R_RecursiveFindRefl (node->children[!side]);
}

// it helps to see what the texture looks like so we can debug it
// NOTE : you must call R_SetGL2D first before calling this function
void R_DrawDebugReflTexture()
{
	qglBindTexture(GL_TEXTURE_2D, g_tex_num[0]);	// do the first texture
	qglBegin(GL_QUADS);
	qglTexCoord2f(1, 1); qglVertex3f(0, 0, 0);
	qglTexCoord2f(0, 1); qglVertex3f(200, 0, 0);
	qglTexCoord2f(0, 0); qglVertex3f(200, 200, 0);
	qglTexCoord2f(1, 0); qglVertex3f(0, 200, 0);
	qglEnd();
}

void R_UpdateReflTex(refdef_t *fd)
{
	g_drawing_refl = true;	// begin drawing reflection

	g_last_known_fov = fd->fov_y;
	
	// go through each reflection and render it
	for (g_active_refl = 0; g_active_refl < g_num_refl; g_active_refl++){

		/* Clear Screen & Depth Buffer */
		//glClearColor(0.2f, 0.5f, 1.0f, 1.0f);		/* Background */
		qglClearColor(0, 0, 0, 1);	// this looks better :)
		qglClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);

		//printf("Updating texture %d for a Z of %f...\n", g_active_refl, g_refl_Z[g_active_refl]);

		// draw stuff here

		R_RenderView( fd );	// draw the scene here!


		qglBindTexture(GL_TEXTURE_2D, g_tex_num[g_active_refl]);
		qglCopyTexSubImage2D(GL_TEXTURE_2D, 0,
			(REFL_TEXW - g_reflTexW) >> 1,
			(REFL_TEXH - g_reflTexH) >> 1,
			0, 0, g_reflTexW, g_reflTexH);
		
	} //for




	g_drawing_refl = false;	// done drawing refl

	qglClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
}


// sets modelview to reflection instead of normal view
void R_DoReflTransform()
{
/*
	    qglRotatef (-r_newrefdef.viewangles[2],  1, 0, 0);	// MPO : this doesn't seem to matter at all
	    qglRotatef (-r_newrefdef.viewangles[0],  0, 1, 0);	// MPO : this handles up/down rotation
	    qglRotatef (-r_newrefdef.viewangles[1],  0, 0, 1);	// MPO : this handles left/right rotation
	    qglTranslatef (-r_newrefdef.vieworg[0],  -r_newrefdef.vieworg[1],  -r_newrefdef.vieworg[2]);

return;
*/

	qglRotatef(180, 1, 0, 0);	// flip upside down (X-axis is forward)
    qglRotatef (r_newrefdef.viewangles[2],  1, 0, 0);
    qglRotatef (r_newrefdef.viewangles[0],  0, 1, 0);	// up/down rotation (reversed)
    qglRotatef (-r_newrefdef.viewangles[1],  0, 0, 1);	// left/right rotation
    qglTranslatef (-r_newrefdef.vieworg[0],
    	-r_newrefdef.vieworg[1],
    	-((2*g_refl_Z[g_active_refl]) - r_newrefdef.vieworg[2]));

/*
    printf("Did reflective translate to Z of %f, regular position was %f, water is %f\n",
    	-((2*g_refl_Z[g_active_refl]) - r_newrefdef.vieworg[2]), -r_newrefdef.vieworg[2],
    	g_refl_Z[g_active_refl]);
*/
}

///////////

void print_matrix(int which_matrix, const char *desc)
{
	GLfloat m[16];	// receives our matrix
	qglGetFloatv(which_matrix, m);	// snag the matrix
	
	printf("[%s]\n", desc);
	printf("%0.3f %0.3f %0.3f %0.3f\n", m[0], m[4], m[8], m[12]);
	printf("%0.3f %0.3f %0.3f %0.3f\n", m[1], m[5], m[9], m[13]);
	printf("%0.3f %0.3f %0.3f %0.3f\n", m[2], m[6], m[10], m[14]);
	printf("%0.3f %0.3f %0.3f %0.3f\n", m[3], m[7], m[11], m[15]);
}

double calc_wav(GLfloat x, GLfloat y, double time)
{
	return ((sin(x + (time*10)) + (cos(y + (time*7)))) * 1.0);
}

// alters texture matrix to handle our reflection
void R_LoadReflMatrix()
{
//	float aspect = (float)g_reflTexW / (float)g_reflTexH;
	float aspect = (float)r_newrefdef.width/r_newrefdef.height;

	extern void MYgluPerspective( GLdouble fovy, GLdouble aspect, GLdouble zNear, GLdouble zFar );

	qglMatrixMode(GL_TEXTURE);
	qglLoadIdentity();

	qglTranslatef(0.5, 0.5, 0);				/* Center texture */

	qglScalef(0.5f *(float)g_reflTexW / REFL_TEXW,
			 0.5f * (float)g_reflTexH / REFL_TEXH,
			 1.0);								/* Scale and bias */

	MYgluPerspective(g_last_known_fov, aspect, 4, 4096);

	qglRotatef (-90,  1, 0, 0);	    // put Z going up
	qglRotatef (90,  0, 0, 1);	    // put Z going up

	// do transform
	R_DoReflTransform();
	
	qglTranslatef(0, 0, REFL_MAGIC_NUMBER);
	
	qglMatrixMode(GL_MODELVIEW);
}

/*
 * Load identity into texture matrix
 */
void R_ClearReflMatrix()
{
	qglMatrixMode(GL_TEXTURE);
	qglLoadIdentity();
	qglMatrixMode(GL_MODELVIEW);
}

// the frustum function from the Mesa3D Library
// Apparently the regular glFrustum function can be broken in certain instances?
void mesa_frustum(GLdouble left, GLdouble right,
        GLdouble bottom, GLdouble top, 
        GLdouble nearval, GLdouble farval)
{
   GLdouble x, y, a, b, c, d;
   GLdouble m[16];

   x = (2.0 * nearval) / (right - left);
   y = (2.0 * nearval) / (top - bottom);
   a = (right + left) / (right - left);
   b = (top + bottom) / (top - bottom);
   c = -(farval + nearval) / ( farval - nearval);
   d = -(2.0 * farval * nearval) / (farval - nearval);

#define M(row,col)  m[col*4+row]
   M(0,0) = x;     M(0,1) = 0.0F;  M(0,2) = a;      M(0,3) = 0.0F;
   M(1,0) = 0.0F;  M(1,1) = y;     M(1,2) = b;      M(1,3) = 0.0F;
   M(2,0) = 0.0F;  M(2,1) = 0.0F;  M(2,2) = c;      M(2,3) = d;
   M(3,0) = 0.0F;  M(3,1) = 0.0F;  M(3,2) = -1.0F;  M(3,3) = 0.0F;
#undef M

   qglMultMatrixd(m);
}


#endif // mattx86: reflective_water