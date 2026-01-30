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
/*				cl_part.c				*/
/* - particle stuff on the client side	*/

#include "client.h"
#include "../qcommon/particles.h"

extern cparticle_t	*active_particles, *free_particles;
extern cparticle_t	particles[MAX_PARTICLES];
extern int			cl_numparticles;

/*
===============
makePart
===============
*/

cparticle_t	*makePart	(float org0,			float org1,				float org2,
						 float vel0,			float vel1,				float vel2,
						 float accel0,			float accel1,			float accel2,
						 float color,			float colorvel,
						 float alpha,			float alphavel)
{
	cparticle_t		*p;

	if (!free_particles)
		return NULL;

	p = free_particles;
	free_particles = p->next;
	p->next = active_particles;
	active_particles = p;

	p->time = cl.time;

	p->org[0] = org0;
	p->org[1] = org1;
	p->org[2] = org2;

	p->vel[0] = vel0;
	p->vel[1] = vel1;
	p->vel[2] = vel2;
	
	p->accel[0] = accel0;
	p->accel[1] = accel1;
	p->accel[2] = accel2;

	p->color = color;
	p->colorvel = colorvel;

	p->alpha = alpha;
	p->alphavel = alphavel;

	p->type = PT_DEFAULT;               //ep::particles

	return p;
}
