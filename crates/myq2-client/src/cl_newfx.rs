// cl_newfx.rs -- MORE entity effects parsing and management
// Converted from: myq2-original/client/cl_newfx.c

use std::f32::consts::PI;

use myq2_common::q_shared::*;

use crate::cl_fx::*;
use crate::client::{PARTICLE_GRAVITY, INSTANT_PARTICLE};

use crate::cl_fx::{qrand, frand, crand};

// ============================================================
// All cl_newfx functions operate on ClFxState
// ============================================================

impl ClFxState {
    // ============================================================
    // CL_Flashlight
    // ============================================================

    pub fn cl_flashlight(&mut self, ent: i32, pos: &Vec3, cl_time: f32) {
        let idx = self.cl_alloc_dlight(ent, cl_time);
        let dl = &mut self.cl_dlights[idx];
        dl.origin = *pos;
        dl.radius = 400.0;
        dl.minlight = 250.0;
        dl.die = cl_time + 100.0;
        dl.color[0] = 1.0;
        dl.color[1] = 1.0;
        dl.color[2] = 1.0;
    }

    // ============================================================
    // CL_ColorFlash — flash of light
    // ============================================================

    pub fn cl_color_flash(
        &mut self,
        pos: &Vec3,
        ent: i32,
        intensity: f32,
        r: f32,
        g: f32,
        b: f32,
        cl_time: f32,
    ) {
        let idx = self.cl_alloc_dlight(ent, cl_time);
        let dl = &mut self.cl_dlights[idx];
        dl.origin = *pos;
        dl.radius = intensity;
        dl.minlight = 250.0;
        dl.die = cl_time + 100.0;
        dl.color[0] = r;
        dl.color[1] = g;
        dl.color[2] = b;
    }

    // ============================================================
    // CL_DebugTrail
    // ============================================================

    pub fn cl_debug_trail(&mut self, start: &Vec3, end: &Vec3, cl_time: f32) {
        let mut mov = *start;
        let mut vec = vector_subtract(end, start);
        let mut len = vector_normalize(&mut vec);

        let (mut _right, mut _up) = ([0.0f32; 3], [0.0f32; 3]);
        make_normal_vectors(&vec, &mut _right, &mut _up);

        let dec: f32 = 3.0;
        vec = vector_scale(&vec, dec);
        mov = *start;

        while len > 0.0 {
            len -= dec;

            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];

            p.time = cl_time;
            vector_clear(&mut p.accel);
            vector_clear(&mut p.vel);
            p.alpha = 1.0;
            p.alphavel = -0.1;
            p.color = (0x74 + (qrand() & 7)) as f32;
            p.particle_type = PT_DEFAULT;
            p.org = mov;

            mov = vector_add(&mov, &vec);
        }
    }

    // ============================================================
    // CL_SmokeTrail
    // ============================================================

    pub fn cl_smoke_trail(
        &mut self,
        start: &Vec3,
        end: &Vec3,
        color_start: i32,
        color_run: i32,
        spacing: i32,
        cl_time: f32,
    ) {
        let mut mov = *start;
        let mut vec = vector_subtract(end, start);
        let mut len = vector_normalize(&mut vec);

        vec = vector_scale(&vec, spacing as f32);

        while len > 0.0 {
            len -= spacing as f32;

            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            vector_clear(&mut p.accel);

            p.time = cl_time;
            p.alpha = 1.0;
            p.alphavel = -1.0 / (1.0 + frand() * 0.5);
            p.color = (color_start + (qrand() % color_run)) as f32;
            p.particle_type = PT_DEFAULT;
            for j in 0..3 {
                p.org[j] = mov[j] + crand() * 3.0;
                p.accel[j] = 0.0;
            }
            p.vel[2] = 20.0 + crand() * 5.0;

            mov = vector_add(&mov, &vec);
        }
    }

    // ============================================================
    // CL_ForceWall
    // ============================================================

    pub fn cl_force_wall(&mut self, start: &Vec3, end: &Vec3, color: i32, cl_time: f32) {
        let mut mov = *start;
        let mut vec = vector_subtract(end, start);
        let mut len = vector_normalize(&mut vec);

        vec = vector_scale(&vec, 4.0);

        while len > 0.0 {
            len -= 4.0;

            if self.free_particles.is_none() {
                return;
            }

            if frand() > 0.3 {
                let idx = match self.alloc_particle() {
                    Some(i) => i,
                    None => return,
                };
                let p = &mut self.particles[idx];
                vector_clear(&mut p.accel);

                p.time = cl_time;
                p.alpha = 1.0;
                p.alphavel = -1.0 / (3.0 + frand() * 0.5);
                p.color = color as f32;
                p.particle_type = PT_DEFAULT;
                for j in 0..3 {
                    p.org[j] = mov[j] + crand() * 3.0;
                    p.accel[j] = 0.0;
                }
                p.vel[0] = 0.0;
                p.vel[1] = 0.0;
                p.vel[2] = -40.0 - (crand() * 10.0);
            }

            mov = vector_add(&mov, &vec);
        }
    }

    // ============================================================
    // CL_FlameEffects
    // ============================================================

    pub fn cl_flame_effects(&mut self, _ent: &CEntity, origin: &Vec3, cl_time: f32) {
        let count = qrand() & 0xf;

        for _ in 0..count {
            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            vector_clear(&mut p.accel);
            p.time = cl_time;

            p.alpha = 1.0;
            p.alphavel = -1.0 / (1.0 + frand() * 0.2);
            p.color = (226 + (qrand() % 4)) as f32;
            p.particle_type = PT_DEFAULT;
            for j in 0..3 {
                p.org[j] = origin[j] + crand() * 5.0;
                p.vel[j] = crand() * 5.0;
            }
            p.vel[2] = crand() * -10.0;
            p.accel[2] = -PARTICLE_GRAVITY;
        }

        let count = qrand() & 0x7;

        for _ in 0..count {
            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            vector_clear(&mut p.accel);

            p.time = cl_time;
            p.alpha = 1.0;
            p.alphavel = -1.0 / (1.0 + frand() * 0.5);
            p.color = (qrand() % 4) as f32;
            p.particle_type = PT_DEFAULT;
            for j in 0..3 {
                p.org[j] = origin[j] + crand() * 3.0;
            }
            p.vel[2] = 20.0 + crand() * 5.0;
        }
    }

    // ============================================================
    // CL_GenericParticleEffect
    // ============================================================

    pub fn cl_generic_particle_effect(
        &mut self,
        org: &Vec3,
        dir: &Vec3,
        color: i32,
        count: i32,
        numcolors: i32,
        dirspread: i32,
        alphavel: f32,
        cl_time: f32,
    ) {
        for _ in 0..count {
            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];

            p.time = cl_time;
            if numcolors > 1 {
                p.color = (color + (qrand() & numcolors)) as f32;
            } else {
                p.color = color as f32;
            }
            p.particle_type = PT_DEFAULT;

            let d = (qrand() & dirspread) as f32;
            for j in 0..3 {
                p.org[j] = org[j] + ((qrand() & 7) - 4) as f32 + d * dir[j];
                p.vel[j] = crand() * 20.0;
            }

            p.accel[0] = 0.0;
            p.accel[1] = 0.0;
            p.accel[2] = -PARTICLE_GRAVITY;
            p.alpha = 1.0;
            p.alphavel = -1.0 / (0.5 + frand() * alphavel);
        }
    }

    // ============================================================
    // CL_BubbleTrail2
    // ============================================================

    pub fn cl_bubble_trail2(&mut self, start: &Vec3, end: &Vec3, dist: i32, cl_time: f32) {
        let mut mov = *start;
        let mut vec = vector_subtract(end, start);
        let len = vector_normalize(&mut vec);

        let dec = dist as f32;
        vec = vector_scale(&vec, dec);

        let mut i: f32 = 0.0;
        while i < len {
            i += dec;

            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            vector_clear(&mut p.accel);
            p.time = cl_time;

            p.alpha = 1.0;
            p.alphavel = -1.0 / (1.0 + frand() * 0.1);
            p.color = (4 + (qrand() & 7)) as f32;
            p.particle_type = PT_BUBBLE;
            for j in 0..3 {
                p.org[j] = mov[j] + crand() * 2.0;
                p.vel[j] = crand() * 10.0;
            }
            p.org[2] -= 4.0;
            p.vel[2] += 20.0;

            mov = vector_add(&mov, &vec);
        }
    }

    // ============================================================
    // CL_Heatbeam (RINGS variant — the one compiled by default)
    // ============================================================

    pub fn cl_heatbeam(
        &mut self,
        start: &Vec3,
        forward: &Vec3,
        v_right: &Vec3,
        v_up: &Vec3,
        cl_time: f32,
    ) {
        let mut end = [0.0f32; 3];
        vector_ma_to(start, 4096.0, forward, &mut end);

        let mut mov = *start;
        let mut vec = vector_subtract(&end, start);
        let len = vector_normalize(&mut vec);

        let right = *v_right;
        let up = *v_up;
        let tmp = mov;
        vector_ma_to(&tmp, -0.5, &right, &mut mov);
        let tmp = mov;
        vector_ma_to(&tmp, -0.5, &up, &mut mov);

        let ltime = cl_time / 1000.0;
        let step: f32 = 32.0;
        let start_pt = (ltime * 96.0) % step;
        mov = vector_ma(start, start_pt, &vec);

        let vec_scaled = vector_scale(&vec, step);

        let rstep = PI / 10.0;
        let mut i = start_pt;
        while i < len {
            if i > step * 5.0 {
                // don't bother after the 5th ring
                break;
            }

            let mut rot: f32 = 0.0;
            while rot < PI * 2.0 {
                let idx = match self.alloc_particle() {
                    Some(i) => i,
                    None => return,
                };
                let p = &mut self.particles[idx];

                p.time = cl_time;
                vector_clear(&mut p.accel);

                let variance = 0.5;
                let c = rot.cos() * variance;
                let s = rot.sin() * variance;

                let dir;
                // trim it so it looks like it's starting at the origin
                if i < 10.0 {
                    let scaled_right = vector_scale(&right, c * (i / 10.0));
                    dir = vector_ma(&scaled_right, s * (i / 10.0), &up);
                } else {
                    let scaled_right = vector_scale(&right, c);
                    dir = vector_ma(&scaled_right, s, &up);
                }

                p.alpha = 0.5;
                p.alphavel = -1000.0;
                p.color = (223 - (qrand() & 7)) as f32;
                p.particle_type = PT_DEFAULT;
                for j in 0..3 {
                    p.org[j] = mov[j] + dir[j] * 3.0;
                    p.vel[j] = 0.0;
                }

                rot += rstep;
            }
            mov = vector_add(&mov, &vec_scaled);
            i += step;
        }
    }

    // ============================================================
    // CL_ParticleSteamEffect
    // ============================================================

    pub fn cl_particle_steam_effect(
        &mut self,
        org: &Vec3,
        dir: &Vec3,
        color: i32,
        count: i32,
        magnitude: i32,
        cl_time: f32,
    ) {
        let mut r = [0.0f32; 3];
        let mut u = [0.0f32; 3];
        make_normal_vectors(dir, &mut r, &mut u);

        for _ in 0..count {
            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];

            p.time = cl_time;
            p.color = (color + (qrand() & 7)) as f32;
            p.particle_type = PT_DEFAULT;

            for j in 0..3 {
                p.org[j] = org[j] + magnitude as f32 * 0.1 * crand();
            }
            vector_scale_to(dir, magnitude as f32, &mut p.vel);
            let d = crand() * magnitude as f32 / 3.0;
            p.vel = vector_ma(&p.vel, d, &r);
            let d = crand() * magnitude as f32 / 3.0;
            p.vel = vector_ma(&p.vel, d, &u);

            p.accel[0] = 0.0;
            p.accel[1] = 0.0;
            p.accel[2] = -PARTICLE_GRAVITY / 2.0;
            p.alpha = 1.0;
            p.alphavel = -1.0 / (0.5 + frand() * 0.3);
        }
    }

    // ============================================================
    // CL_ParticleSteamEffect2
    // ============================================================

    pub fn cl_particle_steam_effect2(&mut self, sustain: &mut ClSustain, cl_time: f32) {
        let dir = sustain.dir;
        let mut r = [0.0f32; 3];
        let mut u = [0.0f32; 3];
        make_normal_vectors(&dir, &mut r, &mut u);

        for _ in 0..sustain.count {
            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];

            p.time = cl_time;
            p.color = (sustain.color + (qrand() & 7)) as f32;
            p.particle_type = PT_DEFAULT;

            for j in 0..3 {
                p.org[j] = sustain.org[j] + sustain.magnitude as f32 * 0.1 * crand();
            }
            vector_scale_to(&dir, sustain.magnitude as f32, &mut p.vel);
            let d = crand() * sustain.magnitude as f32 / 3.0;
            p.vel = vector_ma(&p.vel, d, &r);
            let d = crand() * sustain.magnitude as f32 / 3.0;
            p.vel = vector_ma(&p.vel, d, &u);

            p.accel[0] = 0.0;
            p.accel[1] = 0.0;
            p.accel[2] = -PARTICLE_GRAVITY / 2.0;
            p.alpha = 1.0;
            p.alphavel = -1.0 / (0.5 + frand() * 0.3);
        }
        sustain.nextthink += sustain.thinkinterval;
    }

    // ============================================================
    // CL_TrackerTrail
    // ============================================================

    pub fn cl_tracker_trail(&mut self, start: &Vec3, end: &Vec3, particle_color: i32, cl_time: f32) {
        let mut mov = *start;
        let mut vec = vector_subtract(end, start);
        let mut len = vector_normalize(&mut vec);

        let forward = vec;
        let mut angle_dir = [0.0f32; 3];
        vectoangles2(&forward, &mut angle_dir);
        let mut fwd = [0.0f32; 3];
        let mut right = [0.0f32; 3];
        let mut up = [0.0f32; 3];
        angle_vectors(&angle_dir, Some(&mut fwd), Some(&mut right), Some(&mut up));

        let dec = 3;
        vec = vector_scale(&vec, 3.0);

        while len > 0.0 {
            len -= dec as f32;

            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            vector_clear(&mut p.accel);

            p.time = cl_time;
            p.alpha = 1.0;
            p.alphavel = -2.0;
            p.color = particle_color as f32;
            p.particle_type = PT_DEFAULT;
            let dist = dot_product(&mov, &forward);
            p.org = vector_ma(&mov, 8.0 * dist.cos(), &up);
            for j in 0..3 {
                p.vel[j] = 0.0;
                p.accel[j] = 0.0;
            }
            p.vel[2] = 5.0;

            mov = vector_add(&mov, &vec);
        }
    }

    // ============================================================
    // CL_Tracker_Shell
    // ============================================================

    pub fn cl_tracker_shell(&mut self, origin: &Vec3, cl_time: f32) {
        for _ in 0..300 {
            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            vector_clear(&mut p.accel);

            p.time = cl_time;
            p.alpha = 1.0;
            p.alphavel = INSTANT_PARTICLE;
            p.color = 0.0;
            p.particle_type = PT_DEFAULT;

            let mut dir = [crand(), crand(), crand()];
            vector_normalize(&mut dir);

            p.org = vector_ma(origin, 40.0, &dir);
        }
    }

    // ============================================================
    // CL_MonsterPlasma_Shell
    // ============================================================

    pub fn cl_monster_plasma_shell(&mut self, origin: &Vec3, cl_time: f32) {
        for _ in 0..40 {
            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            vector_clear(&mut p.accel);

            p.time = cl_time;
            p.alpha = 1.0;
            p.alphavel = INSTANT_PARTICLE;
            p.color = 0xe0 as f32;
            p.particle_type = PT_DEFAULT;

            let mut dir = [crand(), crand(), crand()];
            vector_normalize(&mut dir);

            p.org = vector_ma(origin, 10.0, &dir);
        }
    }

    // ============================================================
    // CL_Widowbeamout
    // ============================================================

    pub fn cl_widowbeamout(&mut self, org: &Vec3, ratio: f32, cl_time: f32) {
        let colortable: [i32; 4] = [2 * 8, 13 * 8, 21 * 8, 18 * 8];

        for _ in 0..300 {
            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            vector_clear(&mut p.accel);

            p.time = cl_time;
            p.alpha = 1.0;
            p.alphavel = INSTANT_PARTICLE;
            p.color = colortable[(qrand() & 3) as usize] as f32;
            p.particle_type = PT_DEFAULT;

            let mut dir = [crand(), crand(), crand()];
            vector_normalize(&mut dir);

            p.org = vector_ma(org, 45.0 * ratio, &dir);
        }
    }

    // ============================================================
    // CL_Nukeblast
    // ============================================================

    pub fn cl_nukeblast(&mut self, org: &Vec3, ratio: f32, cl_time: f32) {
        let colortable: [i32; 4] = [110, 112, 114, 116];

        for _ in 0..700 {
            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            vector_clear(&mut p.accel);

            p.time = cl_time;
            p.alpha = 1.0;
            p.alphavel = INSTANT_PARTICLE;
            p.color = colortable[(qrand() & 3) as usize] as f32;
            p.particle_type = PT_DEFAULT;

            let mut dir = [crand(), crand(), crand()];
            vector_normalize(&mut dir);

            p.org = vector_ma(org, 200.0 * ratio, &dir);
        }
    }

    // ============================================================
    // CL_WidowSplash
    // ============================================================

    pub fn cl_widow_splash(&mut self, org: &Vec3, cl_time: f32) {
        let colortable: [i32; 4] = [2 * 8, 13 * 8, 21 * 8, 18 * 8];

        for _ in 0..256 {
            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];

            p.time = cl_time;
            p.color = colortable[(qrand() & 3) as usize] as f32;
            p.particle_type = PT_DEFAULT;

            let mut dir = [crand(), crand(), crand()];
            vector_normalize(&mut dir);
            p.org = vector_ma(org, 45.0, &dir);
            vector_ma_to(&vec3_origin, 40.0, &dir, &mut p.vel);

            p.accel[0] = 0.0;
            p.accel[1] = 0.0;
            p.alpha = 1.0;
            p.alphavel = -0.8 / (0.5 + frand() * 0.3);
        }
    }

    // ============================================================
    // CL_Tracker_Explode
    // ============================================================

    pub fn cl_tracker_explode(&mut self, origin: &Vec3, cl_time: f32) {
        for _ in 0..300 {
            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            vector_clear(&mut p.accel);

            p.time = cl_time;
            p.alpha = 1.0;
            p.alphavel = -1.0;
            p.color = 0.0;
            p.particle_type = PT_DEFAULT;

            let mut dir = [crand(), crand(), crand()];
            vector_normalize(&mut dir);
            let backdir = vector_scale(&dir, -1.0);

            p.org = vector_ma(origin, 64.0, &dir);
            vector_scale_to(&backdir, 64.0, &mut p.vel);
        }
    }

    // ============================================================
    // CL_TagTrail
    // ============================================================

    pub fn cl_tag_trail(&mut self, start: &Vec3, end: &Vec3, color: f32, cl_time: f32) {
        let mut mov = *start;
        let mut vec = vector_subtract(end, start);
        let mut len = vector_normalize(&mut vec);

        let dec = 5;
        vec = vector_scale(&vec, 5.0);

        while len >= 0.0 {
            len -= dec as f32;

            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            vector_clear(&mut p.accel);

            p.time = cl_time;
            p.alpha = 1.0;
            p.alphavel = -1.0 / (0.8 + frand() * 0.2);
            p.color = color;
            p.particle_type = PT_DEFAULT;
            for j in 0..3 {
                p.org[j] = mov[j] + crand() * 16.0;
                p.vel[j] = crand() * 5.0;
                p.accel[j] = 0.0;
            }

            mov = vector_add(&mov, &vec);
        }
    }

    // ============================================================
    // CL_ColorExplosionParticles
    // ============================================================

    pub fn cl_color_explosion_particles(
        &mut self,
        org: &Vec3,
        color: i32,
        run: i32,
        cl_time: f32,
    ) {
        for _ in 0..128 {
            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];

            p.time = cl_time;
            p.color = (color + (qrand() % run)) as f32;
            p.particle_type = PT_DEFAULT;

            for j in 0..3 {
                p.org[j] = org[j] + ((qrand() % 32) - 16) as f32;
                p.vel[j] = ((qrand() % 256) - 128) as f32;
            }

            p.accel[0] = 0.0;
            p.accel[1] = 0.0;
            p.accel[2] = -PARTICLE_GRAVITY;
            p.alpha = 1.0;
            p.alphavel = -0.4 / (0.6 + frand() * 0.2);
        }
    }

    // ============================================================
    // CL_ParticleSmokeEffect — like steam but unaffected by gravity
    // ============================================================

    pub fn cl_particle_smoke_effect(
        &mut self,
        org: &Vec3,
        dir: &Vec3,
        color: i32,
        count: i32,
        magnitude: i32,
        cl_time: f32,
    ) {
        let mut r = [0.0f32; 3];
        let mut u = [0.0f32; 3];
        make_normal_vectors(dir, &mut r, &mut u);

        for _ in 0..count {
            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];

            p.time = cl_time;
            p.color = (color + (qrand() & 7)) as f32;
            p.particle_type = PT_DEFAULT;

            for j in 0..3 {
                p.org[j] = org[j] + magnitude as f32 * 0.1 * crand();
            }
            vector_scale_to(dir, magnitude as f32, &mut p.vel);
            let d = crand() * magnitude as f32 / 3.0;
            p.vel = vector_ma(&p.vel, d, &r);
            let d = crand() * magnitude as f32 / 3.0;
            p.vel = vector_ma(&p.vel, d, &u);

            p.accel[0] = 0.0;
            p.accel[1] = 0.0;
            p.accel[2] = 0.0; // no gravity
            p.alpha = 1.0;
            p.alphavel = -1.0 / (0.5 + frand() * 0.3);
        }
    }

    // ============================================================
    // CL_BlasterParticles2 — Wall impact puffs (Green)
    // ============================================================

    pub fn cl_blaster_particles2(
        &mut self,
        org: &Vec3,
        dir: &Vec3,
        color: u32,
        cl_time: f32,
    ) {
        let count = 40;
        for _ in 0..count {
            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];

            p.time = cl_time;
            p.color = (color as i32 + (qrand() & 7)) as f32;
            p.particle_type = PT_DEFAULT;

            let d = (qrand() & 15) as f32;
            for j in 0..3 {
                p.org[j] = org[j] + ((qrand() & 7) - 4) as f32 + d * dir[j];
                p.vel[j] = dir[j] * 30.0 + crand() * 40.0;
            }

            p.accel[0] = 0.0;
            p.accel[1] = 0.0;
            p.accel[2] = -PARTICLE_GRAVITY;
            p.alpha = 1.0;
            p.alphavel = -1.0 / (0.5 + frand() * 0.3);
        }
    }

    // ============================================================
    // CL_BlasterTrail2 — Green!
    // ============================================================

    pub fn cl_blaster_trail2(&mut self, start: &Vec3, end: &Vec3, cl_time: f32) {
        let mut mov = *start;
        let mut vec = vector_subtract(end, start);
        let mut len = vector_normalize(&mut vec);

        let dec = 5;
        vec = vector_scale(&vec, 5.0);

        while len > 0.0 {
            len -= dec as f32;

            let idx = match self.alloc_particle() {
                Some(i) => i,
                None => return,
            };
            let p = &mut self.particles[idx];
            vector_clear(&mut p.accel);

            p.time = cl_time;
            p.alpha = 1.0;
            p.alphavel = -1.0 / (0.3 + frand() * 0.2);
            p.color = 0xd0 as f32;
            p.particle_type = PT_DEFAULT;
            for j in 0..3 {
                p.org[j] = mov[j] + crand();
                p.vel[j] = crand() * 5.0;
                p.accel[j] = 0.0;
            }

            mov = vector_add(&mov, &vec);
        }
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use myq2_common::q_shared::*;

    /// Create a fresh ClFxState with particles initialized for allocation.
    fn make_fx_state() -> ClFxState {
        let mut state = ClFxState::new();
        state.cl_clear_particles();
        state.cl_clear_dlights();
        state
    }

    /// Count active particles by walking the linked list.
    fn count_active_particles(state: &ClFxState) -> usize {
        let mut count = 0;
        let mut idx = state.active_particles;
        while let Some(i) = idx {
            count += 1;
            idx = state.particles[i].next;
        }
        count
    }

    // ============================================================
    // Flashlight tests
    // ============================================================

    #[test]
    fn test_cl_flashlight_properties() {
        let mut state = make_fx_state();
        let pos = [100.0, 200.0, 300.0];
        let cl_time = 10.0;

        state.cl_flashlight(42, &pos, cl_time);

        // Find the allocated dlight
        let dl = state.cl_dlights.iter().find(|d| d.key == 42).unwrap();
        assert_eq!(dl.origin, [100.0, 200.0, 300.0]);
        assert_eq!(dl.radius, 400.0);
        assert_eq!(dl.minlight, 250.0);
        assert_eq!(dl.die, cl_time + 100.0);
        assert_eq!(dl.color, [1.0, 1.0, 1.0]);
    }

    #[test]
    fn test_cl_flashlight_same_key_reuses_slot() {
        let mut state = make_fx_state();
        let pos1 = [10.0, 20.0, 30.0];
        let pos2 = [40.0, 50.0, 60.0];

        state.cl_flashlight(5, &pos1, 1.0);
        state.cl_flashlight(5, &pos2, 2.0);

        // Should only have one dlight with key 5
        let count = state.cl_dlights.iter().filter(|d| d.key == 5).count();
        assert_eq!(count, 1);

        let dl = state.cl_dlights.iter().find(|d| d.key == 5).unwrap();
        assert_eq!(dl.origin, [40.0, 50.0, 60.0]);
    }

    // ============================================================
    // Color flash tests
    // ============================================================

    #[test]
    fn test_cl_color_flash_properties() {
        let mut state = make_fx_state();
        let pos = [50.0, 60.0, 70.0];

        state.cl_color_flash(&pos, 10, 300.0, 1.0, 0.5, 0.2, 5.0);

        let dl = state.cl_dlights.iter().find(|d| d.key == 10).unwrap();
        assert_eq!(dl.origin, pos);
        assert_eq!(dl.radius, 300.0);
        assert_eq!(dl.minlight, 250.0);
        assert_eq!(dl.die, 105.0);
        assert_eq!(dl.color[0], 1.0);
        assert_eq!(dl.color[1], 0.5);
        assert_eq!(dl.color[2], 0.2);
    }

    // ============================================================
    // Debug trail tests
    // ============================================================

    #[test]
    fn test_cl_debug_trail_creates_particles() {
        let mut state = make_fx_state();

        let start = [0.0, 0.0, 0.0];
        let end = [30.0, 0.0, 0.0];
        // Distance = 30 units, step = 3 units, so 10 particles
        state.cl_debug_trail(&start, &end, 1.0);

        assert_eq!(count_active_particles(&state), 10);
    }

    #[test]
    fn test_cl_debug_trail_particle_properties() {
        let mut state = make_fx_state();

        let start = [0.0, 0.0, 0.0];
        let end = [6.0, 0.0, 0.0]; // 2 particles at step=3
        let cl_time = 5.0;
        state.cl_debug_trail(&start, &end, cl_time);

        // Check first active particle properties (linked list head is most recent)
        let idx = state.active_particles.unwrap();
        let p = &state.particles[idx];
        assert_eq!(p.time, cl_time);
        assert_eq!(p.alpha, 1.0);
        assert_eq!(p.alphavel, -0.1);
        assert_eq!(p.accel, [0.0, 0.0, 0.0]);
        assert_eq!(p.vel, [0.0, 0.0, 0.0]);
        assert_eq!(p.particle_type, PT_DEFAULT);
    }

    #[test]
    fn test_cl_debug_trail_color_range() {
        let mut state = make_fx_state();

        let start = [0.0, 0.0, 0.0];
        let end = [30.0, 0.0, 0.0];
        state.cl_debug_trail(&start, &end, 1.0);

        // Color should be in range 0x74..=0x7B (0x74 + (qrand() & 7))
        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let color = state.particles[i].color as i32;
            assert!(color >= 0x74 && color <= 0x7B,
                "color {} out of range [0x74, 0x7B]", color);
            idx = state.particles[i].next;
        }
    }

    #[test]
    fn test_cl_debug_trail_zero_length() {
        let mut state = make_fx_state();
        let start = [5.0, 5.0, 5.0];
        let end = [5.0, 5.0, 5.0]; // same point

        state.cl_debug_trail(&start, &end, 1.0);

        // Length is 0, so no particles should be created
        assert!(state.active_particles.is_none());
    }

    // ============================================================
    // Smoke trail tests
    // ============================================================

    #[test]
    fn test_cl_smoke_trail_creates_particles() {
        let mut state = make_fx_state();

        let start = [0.0, 0.0, 0.0];
        let end = [50.0, 0.0, 0.0];
        let color_start = 4;
        let color_run = 8;
        let spacing = 5;

        state.cl_smoke_trail(&start, &end, color_start, color_run, spacing, 1.0);

        // 50 / 5 = 10 particles
        assert_eq!(count_active_particles(&state), 10);
    }

    #[test]
    fn test_cl_smoke_trail_particle_velocity() {
        let mut state = make_fx_state();

        let start = [0.0, 0.0, 0.0];
        let end = [10.0, 0.0, 0.0];

        state.cl_smoke_trail(&start, &end, 4, 8, 5, 1.0);

        // Each particle should have vel[2] = 20.0 + crand() * 5.0
        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let p = &state.particles[i];
            // vel[2] should be roughly in [15, 25] range
            assert!(p.vel[2] > 14.0 && p.vel[2] < 26.0,
                "vel[2] = {} out of expected range", p.vel[2]);
            assert_eq!(p.accel, [0.0, 0.0, 0.0]);
            idx = p.next;
        }
    }

    #[test]
    fn test_cl_smoke_trail_color_range() {
        let mut state = make_fx_state();

        let start = [0.0, 0.0, 0.0];
        let end = [50.0, 0.0, 0.0];
        let color_start = 10;
        let color_run = 4;

        state.cl_smoke_trail(&start, &end, color_start, color_run, 5, 1.0);

        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let color = state.particles[i].color as i32;
            assert!(color >= color_start && color < color_start + color_run,
                "color {} out of range [{}, {})", color, color_start, color_start + color_run);
            idx = state.particles[i].next;
        }
    }

    // ============================================================
    // Generic particle effect tests
    // ============================================================

    #[test]
    fn test_cl_generic_particle_effect_count() {
        let mut state = make_fx_state();

        let org = [100.0, 100.0, 100.0];
        let dir = [0.0, 0.0, 1.0];

        state.cl_generic_particle_effect(&org, &dir, 0xE0, 50, 1, 15, 0.5, 1.0);

        assert_eq!(count_active_particles(&state), 50);
    }

    #[test]
    fn test_cl_generic_particle_effect_gravity() {
        let mut state = make_fx_state();

        let org = [0.0, 0.0, 0.0];
        let dir = [0.0, 0.0, 1.0];

        state.cl_generic_particle_effect(&org, &dir, 0xE0, 10, 1, 15, 0.5, 1.0);

        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let p = &state.particles[i];
            assert_eq!(p.accel[0], 0.0);
            assert_eq!(p.accel[1], 0.0);
            assert_eq!(p.accel[2], -PARTICLE_GRAVITY);
            assert_eq!(p.alpha, 1.0);
            idx = p.next;
        }
    }

    #[test]
    fn test_cl_generic_particle_effect_single_color() {
        let mut state = make_fx_state();

        let org = [0.0, 0.0, 0.0];
        let dir = [0.0, 1.0, 0.0];

        // numcolors = 1, so color should always be exactly 'color'
        state.cl_generic_particle_effect(&org, &dir, 0x50, 20, 1, 15, 0.5, 1.0);

        let mut idx = state.active_particles;
        while let Some(i) = idx {
            assert_eq!(state.particles[i].color, 0x50 as f32);
            idx = state.particles[i].next;
        }
    }

    #[test]
    fn test_cl_generic_particle_effect_multi_color() {
        let mut state = make_fx_state();

        let org = [0.0, 0.0, 0.0];
        let dir = [0.0, 1.0, 0.0];

        // numcolors > 1, color should be in range [color, color + numcolors]
        // (note: uses & not %, so range is color + (qrand() & numcolors))
        state.cl_generic_particle_effect(&org, &dir, 0x50, 100, 7, 15, 0.5, 1.0);

        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let color = state.particles[i].color as i32;
            // color = 0x50 + (qrand() & 7), range [0x50, 0x57]
            assert!(color >= 0x50 && color <= 0x57,
                "color {} not in expected range", color);
            idx = state.particles[i].next;
        }
    }

    // ============================================================
    // Bubble trail tests
    // ============================================================

    #[test]
    fn test_cl_bubble_trail2_creates_particles() {
        let mut state = make_fx_state();

        let start = [0.0, 0.0, 0.0];
        let end = [100.0, 0.0, 0.0];
        // dist = 10, length = 100, so 10 particles
        state.cl_bubble_trail2(&start, &end, 10, 1.0);

        assert_eq!(count_active_particles(&state), 10);
    }

    #[test]
    fn test_cl_bubble_trail2_particle_type() {
        let mut state = make_fx_state();

        let start = [0.0, 0.0, 0.0];
        let end = [50.0, 0.0, 0.0];
        state.cl_bubble_trail2(&start, &end, 5, 1.0);

        let mut idx = state.active_particles;
        while let Some(i) = idx {
            assert_eq!(state.particles[i].particle_type, PT_BUBBLE);
            idx = state.particles[i].next;
        }
    }

    #[test]
    fn test_cl_bubble_trail2_upward_velocity() {
        let mut state = make_fx_state();

        let start = [0.0, 0.0, 0.0];
        let end = [50.0, 0.0, 0.0];
        state.cl_bubble_trail2(&start, &end, 5, 1.0);

        // Bubbles should have upward velocity bias: vel[2] += 20.0
        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let p = &state.particles[i];
            // vel[2] = crand() * 10.0 + 20.0, so in [10.0, 30.0]
            assert!(p.vel[2] > 9.0, "bubble vel[2] = {} should be > 9", p.vel[2]);
            idx = p.next;
        }
    }

    // ============================================================
    // Tracker shell tests
    // ============================================================

    #[test]
    fn test_cl_tracker_shell_creates_300_particles() {
        let mut state = make_fx_state();

        let origin = [100.0, 200.0, 300.0];
        state.cl_tracker_shell(&origin, 1.0);

        assert_eq!(count_active_particles(&state), 300);
    }

    #[test]
    fn test_cl_tracker_shell_particle_properties() {
        let mut state = make_fx_state();

        let origin = [0.0, 0.0, 0.0];
        state.cl_tracker_shell(&origin, 5.0);

        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let p = &state.particles[i];
            assert_eq!(p.alpha, 1.0);
            assert_eq!(p.alphavel, INSTANT_PARTICLE);
            assert_eq!(p.color, 0.0);
            assert_eq!(p.particle_type, PT_DEFAULT);
            assert_eq!(p.time, 5.0);
            assert_eq!(p.accel, [0.0, 0.0, 0.0]);
            idx = p.next;
        }
    }

    #[test]
    fn test_cl_tracker_shell_particles_at_radius_40() {
        let mut state = make_fx_state();

        let origin = [10.0, 20.0, 30.0];
        state.cl_tracker_shell(&origin, 1.0);

        // Each particle position = origin + 40 * normalized_dir
        // Distance from origin should be approximately 40
        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let p = &state.particles[i];
            let dx = p.org[0] - origin[0];
            let dy = p.org[1] - origin[1];
            let dz = p.org[2] - origin[2];
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
            assert!((dist - 40.0).abs() < 1.0,
                "particle dist from origin = {}, expected ~40.0", dist);
            idx = p.next;
        }
    }

    // ============================================================
    // Monster plasma shell tests
    // ============================================================

    #[test]
    fn test_cl_monster_plasma_shell_creates_40_particles() {
        let mut state = make_fx_state();

        let origin = [0.0, 0.0, 0.0];
        state.cl_monster_plasma_shell(&origin, 1.0);

        assert_eq!(count_active_particles(&state), 40);
    }

    #[test]
    fn test_cl_monster_plasma_shell_color() {
        let mut state = make_fx_state();

        let origin = [0.0, 0.0, 0.0];
        state.cl_monster_plasma_shell(&origin, 1.0);

        let mut idx = state.active_particles;
        while let Some(i) = idx {
            assert_eq!(state.particles[i].color, 0xe0 as f32);
            idx = state.particles[i].next;
        }
    }

    #[test]
    fn test_cl_monster_plasma_shell_radius_10() {
        let mut state = make_fx_state();

        let origin = [0.0, 0.0, 0.0];
        state.cl_monster_plasma_shell(&origin, 1.0);

        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let p = &state.particles[i];
            let dist = (p.org[0] * p.org[0] + p.org[1] * p.org[1] + p.org[2] * p.org[2]).sqrt();
            assert!((dist - 10.0).abs() < 1.0,
                "plasma shell particle dist = {}, expected ~10.0", dist);
            idx = p.next;
        }
    }

    // ============================================================
    // Color explosion particles tests
    // ============================================================

    #[test]
    fn test_cl_color_explosion_particles_count() {
        let mut state = make_fx_state();

        let org = [0.0, 0.0, 0.0];
        state.cl_color_explosion_particles(&org, 0xE0, 8, 1.0);

        assert_eq!(count_active_particles(&state), 128);
    }

    #[test]
    fn test_cl_color_explosion_particles_color_range() {
        let mut state = make_fx_state();

        let org = [0.0, 0.0, 0.0];
        let color = 0xE0i32;
        let run = 8;
        state.cl_color_explosion_particles(&org, color, run, 1.0);

        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let c = state.particles[i].color as i32;
            assert!(c >= color && c < color + run,
                "color {} not in range [{}, {})", c, color, color + run);
            idx = state.particles[i].next;
        }
    }

    #[test]
    fn test_cl_color_explosion_particles_gravity() {
        let mut state = make_fx_state();

        let org = [0.0, 0.0, 0.0];
        state.cl_color_explosion_particles(&org, 0xE0, 8, 1.0);

        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let p = &state.particles[i];
            assert_eq!(p.accel[0], 0.0);
            assert_eq!(p.accel[1], 0.0);
            assert_eq!(p.accel[2], -PARTICLE_GRAVITY);
            idx = p.next;
        }
    }

    // ============================================================
    // Widowbeamout tests
    // ============================================================

    #[test]
    fn test_cl_widowbeamout_creates_300_particles() {
        let mut state = make_fx_state();

        let org = [0.0, 0.0, 0.0];
        state.cl_widowbeamout(&org, 1.0, 1.0);

        assert_eq!(count_active_particles(&state), 300);
    }

    #[test]
    fn test_cl_widowbeamout_color_table() {
        let mut state = make_fx_state();

        let org = [0.0, 0.0, 0.0];
        state.cl_widowbeamout(&org, 1.0, 1.0);

        let colortable: [i32; 4] = [2 * 8, 13 * 8, 21 * 8, 18 * 8];
        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let c = state.particles[i].color as i32;
            assert!(colortable.contains(&c),
                "color {} not in colortable {:?}", c, colortable);
            idx = state.particles[i].next;
        }
    }

    #[test]
    fn test_cl_widowbeamout_ratio_scales_radius() {
        let mut state = make_fx_state();

        let org = [0.0, 0.0, 0.0];
        let ratio = 2.0;
        state.cl_widowbeamout(&org, ratio, 1.0);

        // Particles at origin + 45 * ratio * dir = 90 units from origin
        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let p = &state.particles[i];
            let dist = (p.org[0] * p.org[0] + p.org[1] * p.org[1] + p.org[2] * p.org[2]).sqrt();
            assert!((dist - 90.0).abs() < 2.0,
                "widowbeamout dist = {}, expected ~90.0", dist);
            idx = p.next;
        }
    }

    // ============================================================
    // Nukeblast tests
    // ============================================================

    #[test]
    fn test_cl_nukeblast_creates_700_particles() {
        let mut state = make_fx_state();

        let org = [0.0, 0.0, 0.0];
        state.cl_nukeblast(&org, 1.0, 1.0);

        assert_eq!(count_active_particles(&state), 700);
    }

    #[test]
    fn test_cl_nukeblast_radius_200() {
        let mut state = make_fx_state();

        let org = [0.0, 0.0, 0.0];
        state.cl_nukeblast(&org, 1.0, 1.0);

        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let p = &state.particles[i];
            let dist = (p.org[0] * p.org[0] + p.org[1] * p.org[1] + p.org[2] * p.org[2]).sqrt();
            assert!((dist - 200.0).abs() < 2.0,
                "nukeblast dist = {}, expected ~200.0", dist);
            idx = p.next;
        }
    }

    #[test]
    fn test_cl_nukeblast_color_table() {
        let mut state = make_fx_state();

        let org = [0.0, 0.0, 0.0];
        state.cl_nukeblast(&org, 1.0, 1.0);

        let colortable: [i32; 4] = [110, 112, 114, 116];
        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let c = state.particles[i].color as i32;
            assert!(colortable.contains(&c),
                "nukeblast color {} not in {:?}", c, colortable);
            idx = state.particles[i].next;
        }
    }

    // ============================================================
    // Tracker explode tests
    // ============================================================

    #[test]
    fn test_cl_tracker_explode_creates_300_particles() {
        let mut state = make_fx_state();

        let origin = [0.0, 0.0, 0.0];
        state.cl_tracker_explode(&origin, 1.0);

        assert_eq!(count_active_particles(&state), 300);
    }

    #[test]
    fn test_cl_tracker_explode_velocity_inward() {
        let mut state = make_fx_state();

        let origin = [0.0, 0.0, 0.0];
        state.cl_tracker_explode(&origin, 1.0);

        // Particles are placed at origin + 64*dir, velocity is -64*dir
        // So velocity should point back towards origin
        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let p = &state.particles[i];
            // vel should be -dir * 64, and org is origin + dir * 64
            // So vel dot org should be negative (pointing inward)
            let dot = p.vel[0] * p.org[0] + p.vel[1] * p.org[1] + p.vel[2] * p.org[2];
            assert!(dot < 0.0, "tracker_explode velocity should point inward, dot={}", dot);
            idx = p.next;
        }
    }

    // ============================================================
    // Tag trail tests
    // ============================================================

    #[test]
    fn test_cl_tag_trail_creates_particles() {
        let mut state = make_fx_state();

        let start = [0.0, 0.0, 0.0];
        let end = [25.0, 0.0, 0.0];
        // Length = 25, step = 5, so 6 particles (while len >= 0)
        state.cl_tag_trail(&start, &end, 0xDC as f32, 1.0);

        assert_eq!(count_active_particles(&state), 6); // len goes 25->20->15->10->5->0, all >= 0
    }

    #[test]
    fn test_cl_tag_trail_color() {
        let mut state = make_fx_state();

        let start = [0.0, 0.0, 0.0];
        let end = [10.0, 0.0, 0.0];
        let color = 0xDC as f32;
        state.cl_tag_trail(&start, &end, color, 1.0);

        let mut idx = state.active_particles;
        while let Some(i) = idx {
            assert_eq!(state.particles[i].color, color);
            idx = state.particles[i].next;
        }
    }

    // ============================================================
    // Blaster particles2 tests
    // ============================================================

    #[test]
    fn test_cl_blaster_particles2_count() {
        let mut state = make_fx_state();

        let org = [0.0, 0.0, 0.0];
        let dir = [0.0, 0.0, 1.0];
        state.cl_blaster_particles2(&org, &dir, 0xD0, 1.0);

        assert_eq!(count_active_particles(&state), 40); // hardcoded count = 40
    }

    #[test]
    fn test_cl_blaster_particles2_velocity_direction() {
        let mut state = make_fx_state();

        let org = [0.0, 0.0, 0.0];
        let dir = [0.0, 0.0, 1.0]; // upward impact
        state.cl_blaster_particles2(&org, &dir, 0xD0, 1.0);

        // Each particle vel[j] = dir[j] * 30.0 + crand() * 40.0
        // For dir=[0,0,1], vel[2] = 30.0 + crand()*40.0, in [-10, 70]
        let mut idx = state.active_particles;
        let mut found_positive_z = false;
        while let Some(i) = idx {
            let p = &state.particles[i];
            if p.vel[2] > 0.0 {
                found_positive_z = true;
            }
            assert_eq!(p.accel[2], -PARTICLE_GRAVITY);
            idx = p.next;
        }
        // Most particles should have positive z velocity given dir=[0,0,1]*30
        assert!(found_positive_z);
    }

    // ============================================================
    // Blaster trail2 tests
    // ============================================================

    #[test]
    fn test_cl_blaster_trail2_creates_particles() {
        let mut state = make_fx_state();

        let start = [0.0, 0.0, 0.0];
        let end = [25.0, 0.0, 0.0];
        // Length = 25, dec = 5, so 5 particles
        state.cl_blaster_trail2(&start, &end, 1.0);

        assert_eq!(count_active_particles(&state), 5);
    }

    #[test]
    fn test_cl_blaster_trail2_color() {
        let mut state = make_fx_state();

        let start = [0.0, 0.0, 0.0];
        let end = [10.0, 0.0, 0.0];
        state.cl_blaster_trail2(&start, &end, 1.0);

        let mut idx = state.active_particles;
        while let Some(i) = idx {
            assert_eq!(state.particles[i].color, 0xd0 as f32);
            idx = state.particles[i].next;
        }
    }

    #[test]
    fn test_cl_blaster_trail2_no_accel() {
        let mut state = make_fx_state();

        let start = [0.0, 0.0, 0.0];
        let end = [10.0, 0.0, 0.0];
        state.cl_blaster_trail2(&start, &end, 1.0);

        let mut idx = state.active_particles;
        while let Some(i) = idx {
            assert_eq!(state.particles[i].accel, [0.0, 0.0, 0.0]);
            idx = state.particles[i].next;
        }
    }

    // ============================================================
    // Particle smoke effect tests (no gravity variant)
    // ============================================================

    #[test]
    fn test_cl_particle_smoke_effect_no_gravity() {
        let mut state = make_fx_state();

        let org = [0.0, 0.0, 0.0];
        let dir = [0.0, 0.0, 1.0];
        state.cl_particle_smoke_effect(&org, &dir, 0x10, 20, 50, 1.0);

        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let p = &state.particles[i];
            // Smoke effect has NO gravity (accel[2] = 0.0)
            assert_eq!(p.accel, [0.0, 0.0, 0.0]);
            idx = p.next;
        }
    }

    // ============================================================
    // Particle steam effect tests (half gravity variant)
    // ============================================================

    #[test]
    fn test_cl_particle_steam_effect_half_gravity() {
        let mut state = make_fx_state();

        let org = [0.0, 0.0, 0.0];
        let dir = [0.0, 0.0, 1.0];
        state.cl_particle_steam_effect(&org, &dir, 0x10, 20, 50, 1.0);

        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let p = &state.particles[i];
            assert_eq!(p.accel[0], 0.0);
            assert_eq!(p.accel[1], 0.0);
            assert_eq!(p.accel[2], -PARTICLE_GRAVITY / 2.0);
            idx = p.next;
        }
    }

    #[test]
    fn test_cl_particle_steam_effect_count() {
        let mut state = make_fx_state();

        let org = [0.0, 0.0, 0.0];
        let dir = [0.0, 0.0, 1.0];
        state.cl_particle_steam_effect(&org, &dir, 0x10, 30, 50, 1.0);

        assert_eq!(count_active_particles(&state), 30);
    }

    // ============================================================
    // Widow splash tests
    // ============================================================

    #[test]
    fn test_cl_widow_splash_count() {
        let mut state = make_fx_state();

        let org = [0.0, 0.0, 0.0];
        state.cl_widow_splash(&org, 1.0);

        assert_eq!(count_active_particles(&state), 256);
    }

    #[test]
    fn test_cl_widow_splash_outward_velocity() {
        let mut state = make_fx_state();

        let org = [0.0, 0.0, 0.0];
        state.cl_widow_splash(&org, 1.0);

        // Velocity = vec3_origin + 40.0 * dir
        // Speed should be approximately 40.0
        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let p = &state.particles[i];
            let speed = (p.vel[0] * p.vel[0] + p.vel[1] * p.vel[1] + p.vel[2] * p.vel[2]).sqrt();
            assert!((speed - 40.0).abs() < 1.0,
                "widow splash velocity magnitude = {}, expected ~40.0", speed);
            idx = p.next;
        }
    }

    // ============================================================
    // Particle exhaustion tests
    // ============================================================

    #[test]
    fn test_particle_exhaustion_graceful() {
        // MAX_PARTICLES = 4096, create an effect that would want more
        let mut state = make_fx_state();

        // Nukeblast creates 700 particles, call it 6 times = 4200 > 4096
        let org = [0.0, 0.0, 0.0];
        for _ in 0..6 {
            state.cl_nukeblast(&org, 1.0, 1.0);
        }

        // Should have exactly MAX_PARTICLES (4096) allocated
        assert_eq!(count_active_particles(&state), 4096);
        // Free list should be exhausted
        assert!(state.free_particles.is_none());
    }

    // ============================================================
    // Alpha/lifetime calculations tests
    // ============================================================

    #[test]
    fn test_alphavel_ranges() {
        // Many effects use alphavel = -1.0 / (base + frand() * range)
        // For smoke trail: -1.0 / (1.0 + frand() * 0.5) => [-1.0, -0.667]
        let min_alpha = -1.0f32 / (1.0 + 0.0 * 0.5);
        let max_alpha = -1.0f32 / (1.0 + 1.0 * 0.5);
        assert_eq!(min_alpha, -1.0);
        assert!((max_alpha - (-0.6667)).abs() < 0.01);

        // For blaster trail2: -1.0 / (0.3 + frand() * 0.2) => [-3.333, -2.0]
        let min_alpha = -1.0f32 / (0.3 + 0.0 * 0.2);
        let max_alpha = -1.0f32 / (0.3 + 1.0 * 0.2);
        assert!((min_alpha - (-3.333)).abs() < 0.01);
        assert_eq!(max_alpha, -2.0);

        // For color explosion: -0.4 / (0.6 + frand() * 0.2) => [-0.667, -0.5]
        let min_alpha = -0.4f32 / (0.6 + 0.0 * 0.2);
        let max_alpha = -0.4f32 / (0.6 + 1.0 * 0.2);
        assert!((min_alpha - (-0.6667)).abs() < 0.01);
        assert_eq!(max_alpha, -0.5);
    }

    // ============================================================
    // Heatbeam ring spacing test
    // ============================================================

    #[test]
    fn test_heatbeam_ring_parameters() {
        // Heatbeam uses step=32, rstep=PI/10, starts at ltime*96 % step
        let step: f32 = 32.0;
        let rstep = PI / 10.0;

        // Number of points per ring = floor(2*PI / rstep) = floor(20) = 20
        let points_per_ring = (2.0 * PI / rstep).floor() as i32;
        assert_eq!(points_per_ring, 20);

        // Max 5 rings before stopping
        let max_dist = step * 5.0;
        assert_eq!(max_dist, 160.0);

        // Start position modulated by time
        let cl_time = 1500.0f32; // 1.5 seconds
        let ltime = cl_time / 1000.0;
        let start_pt = (ltime * 96.0) % step;
        assert!((start_pt - (144.0 % 32.0)).abs() < 0.001);
        assert!((start_pt - 16.0).abs() < 0.001);
    }

    // ============================================================
    // Sustain effect test (steam effect 2)
    // ============================================================

    #[test]
    fn test_cl_particle_steam_effect2_updates_nextthink() {
        let mut state = make_fx_state();

        let mut sustain = ClSustain {
            id: 0,
            sustain_type: 0,
            endtime: 10000,
            nextthink: 1000,
            thinkinterval: 100,
            org: [0.0, 0.0, 0.0],
            dir: [0.0, 0.0, 1.0],
            color: 0x10,
            count: 5,
            magnitude: 50,
            original_endtime: 10000,
            extended: false,
        };

        state.cl_particle_steam_effect2(&mut sustain, 1.0);

        assert_eq!(sustain.nextthink, 1100); // 1000 + 100
    }

    // ============================================================
    // Force wall tests
    // ============================================================

    #[test]
    fn test_cl_force_wall_particle_downward_velocity() {
        let mut state = make_fx_state();

        let start = [0.0, 0.0, 100.0];
        let end = [100.0, 0.0, 100.0];
        state.cl_force_wall(&start, &end, 0x74, 1.0);

        // Particles should have negative z velocity (falling)
        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let p = &state.particles[i];
            // vel[2] = -40.0 - (crand() * 10.0), range [-50, -30]
            assert!(p.vel[2] < -29.0, "force wall vel[2] = {} should be < -29", p.vel[2]);
            assert_eq!(p.vel[0], 0.0);
            assert_eq!(p.vel[1], 0.0);
            idx = p.next;
        }
    }

    // ============================================================
    // Vector math used by effects
    // ============================================================

    #[test]
    fn test_vector_subtract() {
        let a = [10.0, 20.0, 30.0];
        let b = [1.0, 2.0, 3.0];
        let result = vector_subtract(&a, &b);
        assert_eq!(result, [9.0, 18.0, 27.0]);
    }

    #[test]
    fn test_vector_normalize_unit() {
        let mut v = [3.0f32, 4.0, 0.0];
        let len = vector_normalize(&mut v);
        assert!((len - 5.0).abs() < 0.001);
        assert!((v[0] - 0.6).abs() < 0.001);
        assert!((v[1] - 0.8).abs() < 0.001);
        assert!(v[2].abs() < 0.001);
    }

    #[test]
    fn test_vector_scale() {
        let v = [1.0, 2.0, 3.0];
        let result = vector_scale(&v, 2.0);
        assert_eq!(result, [2.0, 4.0, 6.0]);
    }

    #[test]
    fn test_vector_add() {
        let a = [1.0, 2.0, 3.0];
        let b = [4.0, 5.0, 6.0];
        let result = vector_add(&a, &b);
        assert_eq!(result, [5.0, 7.0, 9.0]);
    }

    #[test]
    fn test_vector_ma() {
        let veca = [1.0, 0.0, 0.0];
        let vecb = [0.0, 1.0, 0.0];
        let result = vector_ma(&veca, 3.0, &vecb);
        assert_eq!(result, [1.0, 3.0, 0.0]);
    }

    #[test]
    fn test_dot_product() {
        let a = [1.0, 2.0, 3.0];
        let b = [4.0, 5.0, 6.0];
        let result = dot_product(&a, &b);
        assert_eq!(result, 32.0); // 4 + 10 + 18
    }

    // ============================================================
    // RNG tests (qrand, frand, crand)
    // ============================================================

    #[test]
    fn test_qrand_range() {
        for _ in 0..1000 {
            let v = qrand();
            assert!(v >= 0 && v <= 0x7fff, "qrand() = {} out of range", v);
        }
    }

    #[test]
    fn test_frand_range() {
        for _ in 0..1000 {
            let v = frand();
            assert!(v >= 0.0 && v < 1.0, "frand() = {} out of [0,1)", v);
        }
    }

    #[test]
    fn test_crand_range() {
        for _ in 0..1000 {
            let v = crand();
            assert!(v >= -1.0 && v < 1.0, "crand() = {} out of [-1,1)", v);
        }
    }

    // ============================================================
    // Trail spacing calculation tests
    // ============================================================

    #[test]
    fn test_smoke_trail_spacing_5() {
        let mut state = make_fx_state();
        let start = [0.0, 0.0, 0.0];
        let end = [25.0, 0.0, 0.0];
        // length=25, spacing=5, expected 5 particles
        state.cl_smoke_trail(&start, &end, 4, 8, 5, 1.0);
        assert_eq!(count_active_particles(&state), 5);
    }

    #[test]
    fn test_smoke_trail_spacing_10() {
        let mut state = make_fx_state();
        let start = [0.0, 0.0, 0.0];
        let end = [100.0, 0.0, 0.0];
        // length=100, spacing=10, expected 10 particles
        state.cl_smoke_trail(&start, &end, 4, 8, 10, 1.0);
        assert_eq!(count_active_particles(&state), 10);
    }

    #[test]
    fn test_smoke_trail_spacing_1() {
        let mut state = make_fx_state();
        let start = [0.0, 0.0, 0.0];
        let end = [5.0, 0.0, 0.0];
        // length=5, spacing=1, expected 5 particles
        state.cl_smoke_trail(&start, &end, 4, 8, 1, 1.0);
        assert_eq!(count_active_particles(&state), 5);
    }

    #[test]
    fn test_debug_trail_spacing_3() {
        let mut state = make_fx_state();
        let start = [0.0, 0.0, 0.0];
        let end = [9.0, 0.0, 0.0];
        // length=9, step=3, expected 3 particles
        state.cl_debug_trail(&start, &end, 1.0);
        assert_eq!(count_active_particles(&state), 3);
    }

    #[test]
    fn test_blaster_trail2_spacing_5() {
        let mut state = make_fx_state();
        let start = [0.0, 0.0, 0.0];
        let end = [15.0, 0.0, 0.0];
        // length=15, dec=5, expected 3 particles
        state.cl_blaster_trail2(&start, &end, 1.0);
        assert_eq!(count_active_particles(&state), 3);
    }

    #[test]
    fn test_tag_trail_spacing_5() {
        let mut state = make_fx_state();
        let start = [0.0, 0.0, 0.0];
        let end = [20.0, 0.0, 0.0];
        // length=20, dec=5, particles at: 20,15,10,5,0 (while len >= 0) = 5 particles
        state.cl_tag_trail(&start, &end, 0xDC as f32, 1.0);
        assert_eq!(count_active_particles(&state), 5);
    }

    // ============================================================
    // Diagonal trail tests (non-axis-aligned)
    // ============================================================

    #[test]
    fn test_smoke_trail_diagonal() {
        let mut state = make_fx_state();
        let start = [0.0, 0.0, 0.0];
        let end = [30.0, 40.0, 0.0]; // length = 50
        state.cl_smoke_trail(&start, &end, 4, 8, 10, 1.0);
        assert_eq!(count_active_particles(&state), 5);
    }

    #[test]
    fn test_debug_trail_3d_diagonal() {
        let mut state = make_fx_state();
        let start = [0.0, 0.0, 0.0];
        // length = sqrt(10^2 + 20^2 + 20^2) = sqrt(900) = 30
        let end = [10.0, 20.0, 20.0];
        state.cl_debug_trail(&start, &end, 1.0);
        assert_eq!(count_active_particles(&state), 10);
    }

    // ============================================================
    // Impact effect direction tests
    // ============================================================

    #[test]
    fn test_blaster_particles2_impact_spread() {
        let mut state = make_fx_state();
        let org = [100.0, 100.0, 100.0];
        let dir = [1.0, 0.0, 0.0]; // impact from right
        state.cl_blaster_particles2(&org, &dir, 0xD0, 1.0);

        // All particles should be near the origin, spread by d*dir and random offset
        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let p = &state.particles[i];
            // With dir=[1,0,0], vel[0] should be biased positive: dir[0]*30 + crand*40
            // vel[0] is in range [-10, 70] (30 + [-40,40])
            // Y and Z should be unbiased: crand*40 in [-40,40]
            assert!(p.vel[1].abs() <= 41.0, "y velocity {} too large", p.vel[1]);
            assert!(p.vel[2].abs() <= 41.0 || p.accel[2] == -PARTICLE_GRAVITY,
                "z velocity {} too large with no gravity compensation", p.vel[2]);
            idx = p.next;
        }
    }

    #[test]
    fn test_blaster_particles2_impact_upward() {
        let mut state = make_fx_state();
        let org = [0.0, 0.0, 0.0];
        let dir = [0.0, 0.0, 1.0]; // upward-facing surface
        state.cl_blaster_particles2(&org, &dir, 0xD0, 1.0);

        // Count particles with positive z velocity
        let mut positive_z = 0;
        let mut total = 0;
        let mut idx = state.active_particles;
        while let Some(i) = idx {
            total += 1;
            if state.particles[i].vel[2] > 0.0 {
                positive_z += 1;
            }
            idx = state.particles[i].next;
        }
        // Most should have positive Z velocity due to dir[2]*30 bias
        assert!(positive_z as f32 / total as f32 > 0.5,
            "expected majority positive z, got {}/{}", positive_z, total);
    }

    // ============================================================
    // Steam vs smoke effect parameter comparison tests
    // ============================================================

    #[test]
    fn test_steam_vs_smoke_gravity_difference() {
        let mut state_steam = make_fx_state();
        let mut state_smoke = make_fx_state();

        let org = [0.0, 0.0, 0.0];
        let dir = [0.0, 0.0, 1.0];

        state_steam.cl_particle_steam_effect(&org, &dir, 0x10, 10, 50, 1.0);
        state_smoke.cl_particle_smoke_effect(&org, &dir, 0x10, 10, 50, 1.0);

        // Steam has half gravity
        let mut idx = state_steam.active_particles;
        while let Some(i) = idx {
            assert_eq!(state_steam.particles[i].accel[2], -PARTICLE_GRAVITY / 2.0);
            idx = state_steam.particles[i].next;
        }

        // Smoke has no gravity
        let mut idx = state_smoke.active_particles;
        while let Some(i) = idx {
            assert_eq!(state_smoke.particles[i].accel[2], 0.0);
            idx = state_smoke.particles[i].next;
        }
    }

    #[test]
    fn test_steam_effect_velocity_along_direction() {
        let mut state = make_fx_state();
        let org = [0.0, 0.0, 0.0];
        let dir = [0.0, 0.0, 1.0]; // upward
        let magnitude = 100;

        state.cl_particle_steam_effect(&org, &dir, 0x10, 20, magnitude, 1.0);

        // Main velocity component should be along dir (z-axis) with magnitude
        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let p = &state.particles[i];
            // vel[2] should be approximately magnitude (100) + crand offsets
            // The base is vector_scale(dir, magnitude) = [0, 0, 100]
            // Plus random perpendicular offsets from r and u vectors
            assert!(p.vel[2] > 0.0,
                "steam vel[2] should be positive with upward dir, got {}", p.vel[2]);
            idx = p.next;
        }
    }

    #[test]
    fn test_smoke_effect_velocity_along_direction() {
        let mut state = make_fx_state();
        let org = [0.0, 0.0, 0.0];
        let dir = [1.0, 0.0, 0.0]; // rightward
        let magnitude = 50;

        state.cl_particle_smoke_effect(&org, &dir, 0x10, 20, magnitude, 1.0);

        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let p = &state.particles[i];
            // vel[0] should be positive (magnitude=50 along x)
            // With random perpendicular offsets, x component should dominate
            assert!(p.vel[0] > 0.0,
                "smoke vel[0] should be positive with rightward dir, got {}", p.vel[0]);
            idx = p.next;
        }
    }

    #[test]
    fn test_steam_effect_particle_origin_spread() {
        let mut state = make_fx_state();
        let org = [100.0, 200.0, 300.0];
        let dir = [0.0, 0.0, 1.0];
        let magnitude = 50;

        state.cl_particle_steam_effect(&org, &dir, 0x10, 50, magnitude, 1.0);

        // Particles should be near org, spread by magnitude * 0.1 * crand
        // Max spread = 50 * 0.1 * 1.0 = 5.0
        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let p = &state.particles[i];
            for j in 0..3 {
                let dist = (p.org[j] - org[j]).abs();
                assert!(dist <= 5.1,
                    "particle org[{}] dist {} from origin, expected <= 5.0", j, dist);
            }
            idx = p.next;
        }
    }

    // ============================================================
    // Nuke flash radius scaling tests
    // ============================================================

    #[test]
    fn test_nukeblast_ratio_half() {
        let mut state = make_fx_state();
        let org = [0.0, 0.0, 0.0];
        state.cl_nukeblast(&org, 0.5, 1.0);

        // Particles at 200 * 0.5 = 100 units from origin
        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let p = &state.particles[i];
            let dist = (p.org[0] * p.org[0] + p.org[1] * p.org[1] + p.org[2] * p.org[2]).sqrt();
            assert!((dist - 100.0).abs() < 2.0,
                "nukeblast dist at ratio=0.5 should be ~100, got {}", dist);
            idx = p.next;
        }
    }

    #[test]
    fn test_nukeblast_ratio_2() {
        let mut state = make_fx_state();
        let org = [0.0, 0.0, 0.0];
        state.cl_nukeblast(&org, 2.0, 1.0);

        // Particles at 200 * 2.0 = 400 units from origin
        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let p = &state.particles[i];
            let dist = (p.org[0] * p.org[0] + p.org[1] * p.org[1] + p.org[2] * p.org[2]).sqrt();
            assert!((dist - 400.0).abs() < 2.0,
                "nukeblast dist at ratio=2.0 should be ~400, got {}", dist);
            idx = p.next;
        }
    }

    #[test]
    fn test_nukeblast_instant_particle() {
        let mut state = make_fx_state();
        let org = [0.0, 0.0, 0.0];
        state.cl_nukeblast(&org, 1.0, 1.0);

        // All nuke particles should have INSTANT_PARTICLE alphavel
        let mut idx = state.active_particles;
        while let Some(i) = idx {
            assert_eq!(state.particles[i].alphavel, INSTANT_PARTICLE);
            idx = state.particles[i].next;
        }
    }

    // ============================================================
    // Widowbeamout ratio scaling tests
    // ============================================================

    #[test]
    fn test_widowbeamout_ratio_half() {
        let mut state = make_fx_state();
        let org = [0.0, 0.0, 0.0];
        state.cl_widowbeamout(&org, 0.5, 1.0);

        // Particles at 45 * 0.5 = 22.5 units
        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let p = &state.particles[i];
            let dist = (p.org[0] * p.org[0] + p.org[1] * p.org[1] + p.org[2] * p.org[2]).sqrt();
            assert!((dist - 22.5).abs() < 2.0,
                "widowbeamout dist at ratio=0.5 should be ~22.5, got {}", dist);
            idx = p.next;
        }
    }

    // ============================================================
    // Widow splash radius and velocity tests
    // ============================================================

    #[test]
    fn test_widow_splash_particle_radius() {
        let mut state = make_fx_state();
        let org = [50.0, 50.0, 50.0];
        state.cl_widow_splash(&org, 1.0);

        // Particles at org + 45 * dir, so distance from org = 45
        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let p = &state.particles[i];
            let dx = p.org[0] - org[0];
            let dy = p.org[1] - org[1];
            let dz = p.org[2] - org[2];
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
            assert!((dist - 45.0).abs() < 1.5,
                "widow splash dist should be ~45, got {}", dist);
            idx = p.next;
        }
    }

    #[test]
    fn test_widow_splash_color_table() {
        let mut state = make_fx_state();
        let org = [0.0, 0.0, 0.0];
        state.cl_widow_splash(&org, 1.0);

        let colortable: [i32; 4] = [2 * 8, 13 * 8, 21 * 8, 18 * 8];
        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let c = state.particles[i].color as i32;
            assert!(colortable.contains(&c),
                "widow splash color {} not in colortable {:?}", c, colortable);
            idx = state.particles[i].next;
        }
    }

    // ============================================================
    // Force wall direction and step tests
    // ============================================================

    #[test]
    fn test_force_wall_step_4() {
        let mut state = make_fx_state();
        // Force wall uses step = 4.0
        let start = [0.0, 0.0, 0.0];
        let end = [40.0, 0.0, 0.0]; // length = 40
        state.cl_force_wall(&start, &end, 0x74, 1.0);

        // 40 / 4 = 10 potential particles, but some are skipped (frand() > 0.3)
        let count = count_active_particles(&state);
        assert!(count <= 10, "force wall should have <= 10 particles, got {}", count);
        assert!(count > 0, "force wall should have at least 1 particle");
    }

    #[test]
    fn test_force_wall_zero_length() {
        let mut state = make_fx_state();
        let pos = [50.0, 50.0, 50.0];
        state.cl_force_wall(&pos, &pos, 0x74, 1.0);
        assert_eq!(count_active_particles(&state), 0);
    }

    // ============================================================
    // Bubble trail underwater behavior tests
    // ============================================================

    #[test]
    fn test_bubble_trail2_org_z_offset() {
        let mut state = make_fx_state();
        let start = [0.0, 0.0, 100.0];
        let end = [50.0, 0.0, 100.0];
        state.cl_bubble_trail2(&start, &end, 10, 1.0);

        // Bubble particles have org[2] -= 4.0, so they should be below trail line
        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let p = &state.particles[i];
            // org[2] = base_z + crand*2.0 - 4.0
            // base_z is around 100.0, so org[2] should be around 96.0 +/- 2.0
            assert!(p.org[2] < 100.0,
                "bubble org[2] {} should be < 100 due to -4.0 offset", p.org[2]);
            idx = p.next;
        }
    }

    // ============================================================
    // Tracker trail cosine modulation test
    // ============================================================

    #[test]
    fn test_tracker_trail_creates_particles() {
        let mut state = make_fx_state();
        let start = [0.0, 0.0, 0.0];
        let end = [30.0, 0.0, 0.0]; // 30 / 3 = 10 particles
        state.cl_tracker_trail(&start, &end, 0, 1.0);
        assert_eq!(count_active_particles(&state), 10);
    }

    #[test]
    fn test_tracker_trail_upward_velocity() {
        let mut state = make_fx_state();
        let start = [0.0, 0.0, 0.0];
        let end = [30.0, 0.0, 0.0];
        state.cl_tracker_trail(&start, &end, 0, 1.0);

        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let p = &state.particles[i];
            assert_eq!(p.vel[2], 5.0, "tracker trail vel[2] should be 5.0");
            assert_eq!(p.vel[0], 0.0);
            assert_eq!(p.vel[1], 0.0);
            idx = p.next;
        }
    }

    // ============================================================
    // Flame effect random count tests
    // ============================================================

    #[test]
    fn test_flame_effects_particle_count_bounded() {
        // CL_FlameEffects creates (qrand() & 0xf) fire particles + (qrand() & 0x7) smoke
        // Fire: [0..15], Smoke: [0..7], Total: [0..22]
        let mut state = make_fx_state();
        let origin = [0.0, 0.0, 0.0];

        // Create a dummy CEntity (only origin matters)
        let ce = CEntity::default();
        state.cl_flame_effects(&ce, &origin, 1.0);

        let count = count_active_particles(&state);
        assert!(count <= 22, "flame effects max 22 particles, got {}", count);
    }

    // ============================================================
    // Generic particle effect direction spread tests
    // ============================================================

    #[test]
    fn test_generic_particle_effect_dir_spread() {
        let mut state = make_fx_state();
        let org = [0.0, 0.0, 0.0];
        let dir = [0.0, 0.0, 1.0];
        let dirspread = 31; // mask for d = qrand() & 31, so d in [0, 31]

        state.cl_generic_particle_effect(&org, &dir, 0xE0, 100, 1, dirspread, 0.5, 1.0);

        // With dir=[0,0,1] and d up to 31:
        // org[2] = org[2] + (qrand()&7-4) + d*dir[2] = -4..3 + 0..31 = -4..34
        let mut idx = state.active_particles;
        let mut max_z: f32 = f32::MIN;
        while let Some(i) = idx {
            let p = &state.particles[i];
            if p.org[2] > max_z {
                max_z = p.org[2];
            }
            idx = p.next;
        }
        // Max z should be up to 34 (3 + 31*1)
        assert!(max_z > 0.0, "some particles should have positive z offset");
    }

    // ============================================================
    // Color explosion velocity range tests
    // ============================================================

    #[test]
    fn test_color_explosion_velocity_range() {
        let mut state = make_fx_state();
        let org = [0.0, 0.0, 0.0];
        state.cl_color_explosion_particles(&org, 0xE0, 8, 1.0);

        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let p = &state.particles[i];
            for j in 0..3 {
                // vel[j] = (qrand() % 256) - 128, range [-128, 127]
                assert!(p.vel[j] >= -128.0 && p.vel[j] <= 127.0,
                    "color explosion vel[{}]={} out of range", j, p.vel[j]);
            }
            idx = p.next;
        }
    }

    #[test]
    fn test_color_explosion_org_spread() {
        let mut state = make_fx_state();
        let org = [500.0, 500.0, 500.0];
        state.cl_color_explosion_particles(&org, 0xE0, 8, 1.0);

        let mut idx = state.active_particles;
        while let Some(i) = idx {
            let p = &state.particles[i];
            for j in 0..3 {
                // org[j] = org[j] + (qrand()%32 - 16), range: org-16..org+15
                let offset = p.org[j] - org[j];
                assert!(offset >= -16.0 && offset <= 15.0,
                    "color explosion org[{}] offset {} out of [-16,15]", j, offset);
            }
            idx = p.next;
        }
    }
}
