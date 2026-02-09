// g_target.rs — Target entity functions
// Converted from: myq2-original/game/g_target.c
//
// Copyright (C) 1997-2001 Id Software, Inc.
// Licensed under the GNU General Public License v2 or later.

use crate::g_local::*;
use crate::game::*;
use crate::g_utils::*;
use crate::game_import::*;
use myq2_common::q_shared::{
    CHAN_RELIABLE, CHAN_VOICE,
    DF_ALLOW_EXIT, EF_BLASTER, EF_HYPERBLASTER, RF_BEAM, RF_TRANSLUCENT,
    vec3_origin, vector_normalize,
    DmFlags,
};

// ============================================================
// target_temp_entity
// ============================================================

/// Use_Target_Tent — fire an origin-based temp entity event to clients.
pub fn use_target_tent(game: &mut GameContext, ent_idx: i32, _other_idx: i32, _activator_idx: i32) {
    let ent = &game.edicts[ent_idx as usize];
    let _style = ent.s.skinnum; // ent->style maps to style field
    let origin = ent.s.origin;
    let _style_byte = ent.style;

    gi_write_byte(SVC_TEMP_ENTITY);
    gi_write_byte(_style_byte);
    gi_write_position(&origin);
    gi_multicast(&origin, MULTICAST_PVS);
}

/// SP_target_temp_entity — spawn function.
pub fn sp_target_temp_entity(game: &mut GameContext, ent_idx: i32) {
    let ent = &mut game.edicts[ent_idx as usize];
    ent.use_fn = Some(USE_TARGET_TENT);
}

// ============================================================
// target_speaker
// ============================================================

/// Use_Target_Speaker — play or toggle a sound.
pub fn use_target_speaker(game: &mut GameContext, ent_idx: i32, _other_idx: i32, _activator_idx: i32) {
    let ent = &mut game.edicts[ent_idx as usize];

    if ent.spawnflags & 3 != 0 {
        // looping sound toggles
        if ent.s.sound != 0 {
            ent.s.sound = 0; // turn it off
        } else {
            ent.s.sound = ent.noise_index; // start it
        }
    } else {
        // normal sound
        let chan = if ent.spawnflags & 4 != 0 {
            CHAN_VOICE | CHAN_RELIABLE
        } else {
            CHAN_VOICE
        };
        let origin = ent.s.origin;
        let noise_index = ent.noise_index;
        let volume = ent.volume;
        let attenuation = ent.attenuation;
        gi_positioned_sound(&origin, ent_idx, chan, noise_index, volume, attenuation, 0.0);
    }
}

/// SP_target_speaker — spawn function.
pub fn sp_target_speaker(game: &mut GameContext, ent_idx: i32, st: &SpawnTemp) {
    if st.noise.is_empty() {
        let origin = vtos(&game.edicts[ent_idx as usize].s.origin);
        gi_dprintf(&format!("target_speaker with no noise set at {}\n", origin));
        return;
    }

    let buffer = if !st.noise.contains(".wav") {
        format!("{}.wav", st.noise)
    } else {
        st.noise.clone()
    };

    let noise_index = gi_soundindex(&buffer);

    let ent = &mut game.edicts[ent_idx as usize];
    ent.noise_index = noise_index;

    if ent.volume == 0.0 {
        ent.volume = 1.0;
    }

    if ent.attenuation == 0.0 {
        ent.attenuation = 1.0;
    } else if ent.attenuation == -1.0 {
        // use -1 so 0 defaults to 1
        ent.attenuation = 0.0;
    }

    // check for prestarted looping sound
    if ent.spawnflags & 1 != 0 {
        ent.s.sound = ent.noise_index;
    }

    ent.use_fn = Some(USE_TARGET_SPEAKER);

    // must link the entity so we get areas and clusters so
    // the server can determine who to send updates to
    gi_linkentity(ent_idx);
}

// ============================================================
// target_help
// ============================================================

/// Use_Target_Help — set help message on the personal computer.
pub fn use_target_help(
    game: &mut GameContext,
    game_locals: &mut GameLocals,
    ent_idx: i32,
    _other_idx: i32,
    _activator_idx: i32,
) {
    let ent = &game.edicts[ent_idx as usize];
    if ent.spawnflags & 1 != 0 {
        game_locals.helpmessage1 = ent.message.clone();
    } else {
        game_locals.helpmessage2 = ent.message.clone();
    }
    game_locals.helpchanged += 1;
}

/// SP_target_help — spawn function.
pub fn sp_target_help(game: &mut GameContext, ent_idx: i32, deathmatch_value: f32) {
    if deathmatch_value != 0.0 {
        // auto-remove for deathmatch
        g_free_edict(game, ent_idx as usize);
        return;
    }

    let ent = &game.edicts[ent_idx as usize];
    if ent.message.is_empty() {
        let classname = ent.classname.clone();
        let origin = vtos(&ent.s.origin);
        gi_dprintf(&format!("{} with no message at {}\n", classname, origin));
        g_free_edict(game, ent_idx as usize);
        return;
    }

    let ent = &mut game.edicts[ent_idx as usize];
    ent.use_fn = Some(USE_TARGET_HELP);
}

// ============================================================
// target_secret
// ============================================================

/// use_target_secret — counts a secret found.
pub fn use_target_secret(
    game: &mut GameContext,
    level: &mut LevelLocals,
    ent_idx: i32,
    _other_idx: i32,
    activator_idx: i32,
) {
    let ent = &game.edicts[ent_idx as usize];
    let noise_index = ent.noise_index;
    gi_sound(ent_idx, CHAN_VOICE, noise_index, 1.0, ATTN_NORM as f32, 0.0);

    level.found_secrets += 1;

    g_use_targets(game, ent_idx as usize, activator_idx as usize);
    g_free_edict(game, ent_idx as usize);
}

/// SP_target_secret — spawn function.
pub fn sp_target_secret(
    game: &mut GameContext,
    level: &mut LevelLocals,
    ent_idx: i32,
    st: &mut SpawnTemp,
    deathmatch_value: f32,
) {
    if deathmatch_value != 0.0 {
        // auto-remove for deathmatch
        g_free_edict(game, ent_idx as usize);
        return;
    }

    let ent = &mut game.edicts[ent_idx as usize];
    ent.use_fn = Some(USE_TARGET_SECRET);

    if st.noise.is_empty() {
        st.noise = "misc/secret.wav".to_string();
    }

    ent.noise_index = gi_soundindex(&st.noise);

    ent.svflags = SVF_NOCLIENT;
    level.total_secrets += 1;

    // map bug hack
    if level.mapname.eq_ignore_ascii_case("mine3")
        && ent.s.origin[0] == 280.0
        && ent.s.origin[1] == -2048.0
        && ent.s.origin[2] == -624.0
    {
        ent.message = "You have found a secret area.".to_string();
    }
}

// ============================================================
// target_goal
// ============================================================

/// use_target_goal — counts a goal completed.
pub fn use_target_goal(
    game: &mut GameContext,
    level: &mut LevelLocals,
    ent_idx: i32,
    _other_idx: i32,
    activator_idx: i32,
) {
    let ent = &game.edicts[ent_idx as usize];
    let noise_index = ent.noise_index;
    gi_sound(ent_idx, CHAN_VOICE, noise_index, 1.0, ATTN_NORM as f32, 0.0);

    level.found_goals += 1;

    if level.found_goals == level.total_goals {
        gi_configstring(CS_CDTRACK as i32, "0");
    }

    g_use_targets(game, ent_idx as usize, activator_idx as usize);
    g_free_edict(game, ent_idx as usize);
}

/// SP_target_goal — spawn function.
pub fn sp_target_goal(
    game: &mut GameContext,
    level: &mut LevelLocals,
    ent_idx: i32,
    st: &mut SpawnTemp,
    deathmatch_value: f32,
) {
    if deathmatch_value != 0.0 {
        // auto-remove for deathmatch
        g_free_edict(game, ent_idx as usize);
        return;
    }

    let ent = &mut game.edicts[ent_idx as usize];
    ent.use_fn = Some(USE_TARGET_GOAL);

    if st.noise.is_empty() {
        st.noise = "misc/secret.wav".to_string();
    }

    ent.noise_index = gi_soundindex(&st.noise);

    ent.svflags = SVF_NOCLIENT;
    level.total_goals += 1;
}

// ============================================================
// target_explosion
// ============================================================

/// target_explosion_explode — the actual explosion.
pub fn target_explosion_explode(game: &mut GameContext, level: &mut LevelLocals, self_idx: i32) {
    let ent = &game.edicts[self_idx as usize];
    let origin = ent.s.origin;
    let dmg = ent.dmg;
    let activator_idx = ent.activator;
    let _delay = ent.delay;

    gi_write_byte(SVC_TEMP_ENTITY);
    gi_write_byte(TE_EXPLOSION1);
    gi_write_position(&origin);
    gi_multicast(&origin, MULTICAST_PHS);

    // T_RadiusDamage(self, self->activator, self->dmg, NULL, self->dmg+40, MOD_EXPLOSIVE)
    crate::g_combat::t_radius_damage(
        self_idx as usize,
        activator_idx as usize,
        dmg as f32,
        None,
        (dmg + 40) as f32,
        MOD_EXPLOSIVE,
        &mut game.edicts,
        level,
    );

    let ent = &mut game.edicts[self_idx as usize];
    let save = ent.delay;
    ent.delay = 0.0;
    g_use_targets(game, self_idx as usize, activator_idx as usize);
    game.edicts[self_idx as usize].delay = save;
}

/// use_target_explosion — use callback for target_explosion.
pub fn use_target_explosion(
    game: &mut GameContext,
    level: &mut LevelLocals,
    self_idx: i32,
    _other_idx: i32,
    activator_idx: i32,
) {
    game.edicts[self_idx as usize].activator = activator_idx;

    if game.edicts[self_idx as usize].delay == 0.0 {
        target_explosion_explode(game, level, self_idx);
        return;
    }

    game.edicts[self_idx as usize].think_fn = Some(THINK_TARGET_EXPLOSION_EXPLODE);
    game.edicts[self_idx as usize].nextthink =
        level.time + game.edicts[self_idx as usize].delay;
}

/// SP_target_explosion — spawn function.
pub fn sp_target_explosion(game: &mut GameContext, ent_idx: i32) {
    let ent = &mut game.edicts[ent_idx as usize];
    ent.use_fn = Some(USE_TARGET_EXPLOSION);
    ent.svflags = SVF_NOCLIENT;
}

// ============================================================
// target_changelevel
// ============================================================

/// use_target_changelevel — changes level when fired.
pub fn use_target_changelevel(
    game: &mut GameContext,
    game_locals: &mut GameLocals,
    level: &mut LevelLocals,
    clients: &[GClient],
    self_idx: i32,
    other_idx: i32,
    activator_idx: i32,
    deathmatch_value: f32,
    coop_value: f32,
    dmflags_value: f32,
) {
    if level.intermissiontime != 0.0 {
        return; // already activated
    }

    if deathmatch_value == 0.0 && coop_value == 0.0 {
        // g_edicts[1] is the first player
        if game.edicts[1].health <= 0 {
            return;
        }
    }

    // if noexit, do a ton of damage to other
    if deathmatch_value != 0.0
        && !DmFlags::from_bits_truncate(dmflags_value as i32).intersects(DF_ALLOW_EXIT)
        && other_idx != 0
    // other != world (world is entity 0)
    {
        let other = &game.edicts[other_idx as usize];
        let max_health = other.max_health;
        let other_origin = other.s.origin;
        // T_Damage(other, self, self, vec3_origin, other->s.origin, vec3_origin, 10*max_health, 1000, 0, MOD_EXIT)
        crate::g_combat::t_damage(
            other_idx as usize,
            self_idx as usize,
            self_idx as usize,
            vec3_origin,
            other_origin,
            vec3_origin,
            10 * max_health,
            1000,
            DamageFlags::empty(),
            MOD_EXIT,
            &mut game.edicts,
            level,
        );
        return;
    }

    // if multiplayer, let everyone know who hit the exit
    if deathmatch_value != 0.0
        && activator_idx > 0 {
            let activator = &game.edicts[activator_idx as usize];
            if let Some(client_idx) = activator.client {
                let netname = &clients[client_idx].pers.netname;
                gi_bprintf(PRINT_HIGH, &format!("{} exited the level.\n", netname));
            }
        }

    // if going to a new unit, clear cross triggers
    let self_map = game.edicts[self_idx as usize].map.clone();
    if self_map.contains('*') {
        game_locals.serverflags &= !(SFL_CROSS_TRIGGER_MASK);
    }

    // BeginIntermission deferred: p_hud::GameContext differs from g_target::GameContext
}

/// SP_target_changelevel — spawn function.
pub fn sp_target_changelevel(game: &mut GameContext, level: &LevelLocals, ent_idx: i32) {
    {
        let ent = &game.edicts[ent_idx as usize];
        if ent.map.is_empty() {
            let origin = vtos(&ent.s.origin);
            gi_dprintf(&format!("target_changelevel with no map at {}\n", origin));
            g_free_edict(game, ent_idx as usize);
            return;
        }
    }

    // ugly hack because *SOMEBODY* screwed up their map
    if level.mapname.eq_ignore_ascii_case("fact1")
        && game.edicts[ent_idx as usize].map.eq_ignore_ascii_case("fact3")
    {
        game.edicts[ent_idx as usize].map = "fact3$secret1".to_string();
    }

    let ent = &mut game.edicts[ent_idx as usize];
    ent.use_fn = Some(USE_TARGET_CHANGELEVEL);
    ent.svflags = SVF_NOCLIENT;
}

// ============================================================
// target_splash
// ============================================================

/// use_target_splash — creates a particle splash effect when used.
pub fn use_target_splash(
    game: &mut GameContext,
    level: &mut LevelLocals,
    self_idx: i32,
    _other_idx: i32,
    activator_idx: i32,
) {
    let ent = &game.edicts[self_idx as usize];
    let count = ent.count;
    let origin = ent.s.origin;
    let movedir = ent.movedir;
    let sounds = ent.sounds;
    let dmg = ent.dmg;

    gi_write_byte(SVC_TEMP_ENTITY);
    gi_write_byte(TE_SPLASH);
    gi_write_byte(count);
    gi_write_position(&origin);
    gi_write_dir(&movedir);
    gi_write_byte(sounds);
    gi_multicast(&origin, MULTICAST_PVS);

    if dmg != 0 {
        // T_RadiusDamage(self, activator, self->dmg, NULL, self->dmg+40, MOD_SPLASH)
        crate::g_combat::t_radius_damage(
            self_idx as usize,
            activator_idx as usize,
            dmg as f32,
            None,
            (dmg + 40) as f32,
            MOD_SPLASH,
            &mut game.edicts,
            level,
        );
    }
}

/// SP_target_splash — spawn function.
pub fn sp_target_splash(game: &mut GameContext, self_idx: i32) {
    let ent = &mut game.edicts[self_idx as usize];
    ent.use_fn = Some(USE_TARGET_SPLASH);

    // G_SetMovedir(self->s.angles, self->movedir)
    let angles = ent.s.angles;
    let mut movedir = ent.movedir;
    g_set_movedir(&angles, &mut movedir);
    let ent = &mut game.edicts[self_idx as usize];
    ent.movedir = movedir;
    ent.s.angles = [0.0, 0.0, 0.0];

    if ent.count == 0 {
        ent.count = 32;
    }

    ent.svflags = SVF_NOCLIENT;
}

// ============================================================
// target_spawner
// ============================================================

/// use_target_spawner — spawns entities of type self->target.
pub fn use_target_spawner(
    game: &mut GameContext,
    self_idx: i32,
    _other_idx: i32,
    _activator_idx: i32,
) {
    let self_ent = &game.edicts[self_idx as usize];
    let target = self_ent.target.clone();
    let origin = self_ent.s.origin;
    let angles = self_ent.s.angles;
    let speed = self_ent.speed;
    let movedir = self_ent.movedir;

    let new_idx = g_spawn(game);
    {
        let new_ent = &mut game.edicts[new_idx];
        new_ent.classname = target;
        new_ent.s.origin = origin;
        new_ent.s.angles = angles;
    }

    // ED_CallSpawn deferred: requires g_spawn context integration
    gi_unlinkentity(new_idx as i32);
    killbox(game, new_idx);
    gi_linkentity(new_idx as i32);

    if speed != 0.0 {
        game.edicts[new_idx].velocity = movedir;
    }
}

/// SP_target_spawner — spawn function.
pub fn sp_target_spawner(game: &mut GameContext, self_idx: i32) {
    let ent = &mut game.edicts[self_idx as usize];
    ent.use_fn = Some(USE_TARGET_SPAWNER);
    ent.svflags = SVF_NOCLIENT;

    if ent.speed != 0.0 {
        let angles = ent.s.angles;
        let mut movedir = ent.movedir;
        g_set_movedir(&angles, &mut movedir);
        // VectorScale(self->movedir, self->speed, self->movedir)
        let speed = ent.speed;
        movedir[0] *= speed;
        movedir[1] *= speed;
        movedir[2] *= speed;
        let ent = &mut game.edicts[self_idx as usize];
        ent.movedir = movedir;
        ent.s.angles = [0.0, 0.0, 0.0];
    }
}

// ============================================================
// target_blaster
// ============================================================

/// use_target_blaster — fires a blaster bolt in the set direction.
pub fn use_target_blaster(
    game: &mut GameContext,
    self_idx: i32,
    _other_idx: i32,
    _activator_idx: i32,
) {
    let ent = &game.edicts[self_idx as usize];
    let _effect: u32 = if ent.spawnflags & 2 != 0 {
        0
    } else if ent.spawnflags & 1 != 0 {
        EF_HYPERBLASTER
    } else {
        EF_BLASTER
    };

    let origin = ent.s.origin;
    let movedir = ent.movedir;
    let dmg = ent.dmg;
    let speed = ent.speed;
    let noise_index = ent.noise_index;
    let hyper = (ent.spawnflags & 1) != 0;

    // fire_blaster(self, self->s.origin, self->movedir, self->dmg, self->speed, effect, hyper)
    {
        let mut temp_level = LevelLocals::default();
        temp_level.time = game.level.time;
        crate::g_weapon::fire_blaster(
            self_idx as usize, &mut game.edicts, &mut temp_level,
            &origin,
            &movedir,
            dmg,
            speed as i32,
            _effect as i32,
            hyper,
        );
    }

    gi_sound(self_idx, CHAN_VOICE, noise_index, 1.0, ATTN_NORM as f32, 0.0);
}

/// SP_target_blaster — spawn function.
pub fn sp_target_blaster(game: &mut GameContext, self_idx: i32) {
    let ent = &mut game.edicts[self_idx as usize];
    ent.use_fn = Some(USE_TARGET_BLASTER);

    let angles = ent.s.angles;
    let mut movedir = ent.movedir;
    g_set_movedir(&angles, &mut movedir);
    let ent = &mut game.edicts[self_idx as usize];
    ent.movedir = movedir;
    ent.s.angles = [0.0, 0.0, 0.0];

    ent.noise_index = gi_soundindex("weapons/laser2.wav");

    if ent.dmg == 0 {
        ent.dmg = 15;
    }
    if ent.speed == 0.0 {
        ent.speed = 1000.0;
    }

    ent.svflags = SVF_NOCLIENT;
}

// ============================================================
// target_crosslevel_trigger
// ============================================================

/// trigger_crosslevel_trigger_use — sets server flags and frees self.
pub fn trigger_crosslevel_trigger_use(
    game: &mut GameContext,
    game_locals: &mut GameLocals,
    self_idx: i32,
    _other_idx: i32,
    _activator_idx: i32,
) {
    game_locals.serverflags |= game.edicts[self_idx as usize].spawnflags;
    g_free_edict(game, self_idx as usize);
}

/// SP_target_crosslevel_trigger — spawn function.
pub fn sp_target_crosslevel_trigger(game: &mut GameContext, self_idx: i32) {
    let ent = &mut game.edicts[self_idx as usize];
    ent.svflags = SVF_NOCLIENT;
    ent.use_fn = Some(USE_TRIGGER_CROSSLEVEL_TRIGGER);
}

// ============================================================
// target_crosslevel_target
// ============================================================

/// target_crosslevel_target_think — check if cross-level triggers are satisfied.
pub fn target_crosslevel_target_think(
    game: &mut GameContext,
    game_locals: &GameLocals,
    self_idx: i32,
) {
    let spawnflags = game.edicts[self_idx as usize].spawnflags;
    if spawnflags == (game_locals.serverflags & SFL_CROSS_TRIGGER_MASK & spawnflags) {
        g_use_targets(game, self_idx as usize, self_idx as usize);
        g_free_edict(game, self_idx as usize);
    }
}

/// SP_target_crosslevel_target — spawn function.
pub fn sp_target_crosslevel_target(game: &mut GameContext, level: &LevelLocals, self_idx: i32) {
    let ent = &mut game.edicts[self_idx as usize];
    if ent.delay == 0.0 {
        ent.delay = 1.0;
    }
    ent.svflags = SVF_NOCLIENT;
    ent.think_fn = Some(THINK_TARGET_CROSSLEVEL_TARGET);
    ent.nextthink = level.time + ent.delay;
}

// ============================================================
// target_laser
// ============================================================

/// target_laser_think — trace laser beam and deal damage.
pub fn target_laser_think(game: &mut GameContext, level: &mut LevelLocals, self_idx: i32) {
    let count;
    {
        let ent = &game.edicts[self_idx as usize];
        count = if ent.spawnflags & 0x80000000u32 as i32 != 0 {
            8
        } else {
            4
        };
    }

    // If we have an enemy, update movedir to point at it
    let enemy_idx = game.edicts[self_idx as usize].enemy;
    if enemy_idx > 0 {
        let last_movedir = game.edicts[self_idx as usize].movedir;
        let enemy = &game.edicts[enemy_idx as usize];
        let point = [
            enemy.absmin[0] + 0.5 * enemy.size[0],
            enemy.absmin[1] + 0.5 * enemy.size[1],
            enemy.absmin[2] + 0.5 * enemy.size[2],
        ];
        let self_origin = game.edicts[self_idx as usize].s.origin;
        let mut movedir = [
            point[0] - self_origin[0],
            point[1] - self_origin[1],
            point[2] - self_origin[2],
        ];
        vector_normalize(&mut movedir);
        game.edicts[self_idx as usize].movedir = movedir;
        if movedir != last_movedir {
            game.edicts[self_idx as usize].spawnflags |= 0x80000000u32 as i32;
        }
    }

    let ent = &game.edicts[self_idx as usize];
    let start = ent.s.origin;
    let movedir = ent.movedir;
    let end = [
        start[0] + 2048.0 * movedir[0],
        start[1] + 2048.0 * movedir[1],
        start[2] + 2048.0 * movedir[2],
    ];
    let dmg = ent.dmg;
    let _skinnum = ent.s.skinnum;
    let _spawnflags = ent.spawnflags;
    let activator_idx = ent.activator;

    let mut ignore_idx = self_idx;
    let mut trace_start = start;
    let mut last_endpos = end;

    loop {
        let tr = gi_trace(&trace_start, &VEC3_ORIGIN, &VEC3_ORIGIN, &end, ignore_idx,
            CONTENTS_SOLID | CONTENTS_MONSTER | CONTENTS_DEADMONSTER);

        if tr.ent_index < 0 || (tr.ent_index as usize) >= game.edicts.len() {
            last_endpos = tr.endpos;
            break;
        }
        let tr_ent = tr.ent_index as usize;
        last_endpos = tr.endpos;

        // hurt it if we can
        if game.edicts[tr_ent].takedamage != 0
            && !game.edicts[tr_ent].flags.intersects(FL_IMMUNE_LASER)
        {
            crate::g_combat::t_damage(
                tr_ent, self_idx as usize, activator_idx as usize,
                movedir, tr.endpos, VEC3_ORIGIN,
                dmg, 1, DAMAGE_ENERGY, MOD_TARGET_LASER,
                &mut game.edicts, level,
            );
        }

        // if we hit something that's not a monster or player, we're done
        if (game.edicts[tr_ent].svflags & SVF_MONSTER) == 0 && game.edicts[tr_ent].client.is_none() {
            if (game.edicts[self_idx as usize].spawnflags & (0x80000000u32 as i32)) != 0 {
                game.edicts[self_idx as usize].spawnflags &= !(0x80000000u32 as i32);
                gi_write_byte(SVC_TEMP_ENTITY);
                gi_write_byte(TE_LASER_SPARKS);
                gi_write_byte(count);
                gi_write_position(&tr.endpos);
                gi_write_dir(&tr.plane.normal);
                gi_write_byte(game.edicts[self_idx as usize].s.skinnum);
                gi_multicast(&tr.endpos, MULTICAST_PVS);
            }
            break;
        }

        ignore_idx = tr_ent as i32;
        trace_start = tr.endpos;
    }

    game.edicts[self_idx as usize].s.old_origin = last_endpos;
    game.edicts[self_idx as usize].nextthink = level.time + FRAMETIME;
}

/// target_laser_on — activate the laser.
pub fn target_laser_on(game: &mut GameContext, level: &mut LevelLocals, self_idx: i32) {
    if game.edicts[self_idx as usize].activator == 0 {
        game.edicts[self_idx as usize].activator = self_idx;
    }
    game.edicts[self_idx as usize].spawnflags |= 0x80000000u32 as i32 | 1;
    game.edicts[self_idx as usize].svflags &= !SVF_NOCLIENT;
    target_laser_think(game, level, self_idx);
}

/// target_laser_off — deactivate the laser.
pub fn target_laser_off(game: &mut GameContext, self_idx: i32) {
    let ent = &mut game.edicts[self_idx as usize];
    ent.spawnflags &= !1;
    ent.svflags |= SVF_NOCLIENT;
    ent.nextthink = 0.0;
}

/// target_laser_use — toggle the laser on/off.
pub fn target_laser_use(
    game: &mut GameContext,
    level: &mut LevelLocals,
    self_idx: i32,
    _other_idx: i32,
    activator_idx: i32,
) {
    game.edicts[self_idx as usize].activator = activator_idx;
    if game.edicts[self_idx as usize].spawnflags & 1 != 0 {
        target_laser_off(game, self_idx);
    } else {
        target_laser_on(game, level, self_idx);
    }
}

/// target_laser_start — deferred setup for laser entity.
pub fn target_laser_start(game: &mut GameContext, level: &mut LevelLocals, self_idx: i32) {
    {
        let ent = &mut game.edicts[self_idx as usize];
        ent.movetype = MoveType::None;
        ent.solid = Solid::Not;
        ent.s.renderfx |= RF_BEAM | RF_TRANSLUCENT;
        ent.s.modelindex = 1; // must be non-zero

        // set the beam diameter
        if ent.spawnflags & 64 != 0 {
            ent.s.frame = 16;
        } else {
            ent.s.frame = 4;
        }

        // set the color
        if ent.spawnflags & 2 != 0 {
            ent.s.skinnum = 0xf2f2f0f0u32 as i32;
        } else if ent.spawnflags & 4 != 0 {
            ent.s.skinnum = 0xd0d1d2d3u32 as i32;
        } else if ent.spawnflags & 8 != 0 {
            ent.s.skinnum = 0xf3f3f1f1u32 as i32;
        } else if ent.spawnflags & 16 != 0 {
            ent.s.skinnum = 0xdcdddedf_u32 as i32;
        } else if ent.spawnflags & 32 != 0 {
            ent.s.skinnum = 0xe0e1e2e3_u32 as i32;
        }
    }

    let enemy_idx = game.edicts[self_idx as usize].enemy;
    if enemy_idx <= 0 {
        let target = game.edicts[self_idx as usize].target.clone();
        if !target.is_empty() {
            let found = g_find(game, 0, "targetname", &target);
            if let Some(found_idx) = found {
                game.edicts[self_idx as usize].enemy = found_idx as i32;
            } else {
                let classname = game.edicts[self_idx as usize].classname.clone();
                let origin = vtos(&game.edicts[self_idx as usize].s.origin);
                gi_dprintf(&format!("{} at {}: {} is a bad target\n", classname, origin, target));
            }
        } else {
            let angles = game.edicts[self_idx as usize].s.angles;
            let mut movedir = game.edicts[self_idx as usize].movedir;
            g_set_movedir(&angles, &mut movedir);
            game.edicts[self_idx as usize].movedir = movedir;
            game.edicts[self_idx as usize].s.angles = [0.0, 0.0, 0.0];
        }
    }

    {
        let ent = &mut game.edicts[self_idx as usize];
        ent.use_fn = Some(USE_TARGET_LASER);
        ent.think_fn = Some(THINK_TARGET_LASER);

        if ent.dmg == 0 {
            ent.dmg = 1;
        }

        ent.mins = [-8.0, -8.0, -8.0];
        ent.maxs = [8.0, 8.0, 8.0];
    }

    gi_linkentity(self_idx);

    if game.edicts[self_idx as usize].spawnflags & 1 != 0 {
        target_laser_on(game, level, self_idx);
    } else {
        target_laser_off(game, self_idx);
    }
}

/// SP_target_laser — spawn function (defers to target_laser_start).
pub fn sp_target_laser(game: &mut GameContext, level: &LevelLocals, self_idx: i32) {
    // let everything else get spawned before we start firing
    let ent = &mut game.edicts[self_idx as usize];
    ent.think_fn = Some(THINK_TARGET_LASER_START);
    ent.nextthink = level.time + 1.0;
}

// ============================================================
// target_lightramp
// ============================================================

/// target_lightramp_think — interpolate light level over time.
pub fn target_lightramp_think(game: &mut GameContext, level: &LevelLocals, self_idx: i32) {
    let ent = &game.edicts[self_idx as usize];
    let movedir = ent.movedir;
    let timestamp = ent.timestamp;
    let speed = ent.speed;
    let spawnflags = ent.spawnflags;
    let enemy_idx = ent.enemy;

    let char_val =
        (b'a' as f32 + movedir[0] + (level.time - timestamp) / FRAMETIME * movedir[2]) as u8;
    let style_str = format!("{}", char_val as char);

    if enemy_idx > 0 {
        let enemy_style = game.edicts[enemy_idx as usize].style;
        gi_configstring((CS_LIGHTS as i32) + enemy_style, &style_str);
    }

    if (level.time - timestamp) < speed {
        game.edicts[self_idx as usize].nextthink = level.time + FRAMETIME;
    } else if spawnflags & 1 != 0 {
        // TOGGLE: swap start/end and reverse direction
        let ent = &mut game.edicts[self_idx as usize];
        ent.movedir.swap(0, 1);
        ent.movedir[2] *= -1.0;
    }
}

/// target_lightramp_use — start the light ramp.
pub fn target_lightramp_use(
    game: &mut GameContext,
    level: &LevelLocals,
    self_idx: i32,
    _other_idx: i32,
    _activator_idx: i32,
) {
    if game.edicts[self_idx as usize].enemy <= 0 {
        // find the target light entity
        let target = game.edicts[self_idx as usize].target.clone();
        let mut search_from: usize = 0;
        loop {
            let found = g_find(game, search_from, "targetname", &target);
            match found {
                None => break,
                Some(e_idx) => {
                    let e = &game.edicts[e_idx];
                    if e.classname != "light" {
                        let classname = game.edicts[self_idx as usize].classname.clone();
                        let self_origin =
                            vtos(&game.edicts[self_idx as usize].s.origin);
                        let e_classname = e.classname.clone();
                        let e_origin = vtos(&e.s.origin);
                        gi_dprintf(&format!(
                            "{} at {} target {} ({} at {}) is not a light\n",
                            classname, self_origin, target, e_classname, e_origin
                        ));
                    } else {
                        game.edicts[self_idx as usize].enemy = e_idx as i32;
                    }
                    search_from = e_idx + 1;
                }
            }
        }

        if game.edicts[self_idx as usize].enemy <= 0 {
            let classname = game.edicts[self_idx as usize].classname.clone();
            let origin = vtos(&game.edicts[self_idx as usize].s.origin);
            gi_dprintf(&format!("{} target {} not found at {}\n", classname, target, origin));
            g_free_edict(game, self_idx as usize);
            return;
        }
    }

    game.edicts[self_idx as usize].timestamp = level.time;
    target_lightramp_think(game, level, self_idx);
}

/// SP_target_lightramp — spawn function.
pub fn sp_target_lightramp(
    game: &mut GameContext,
    self_idx: i32,
    deathmatch_value: f32,
) {
    let ent = &game.edicts[self_idx as usize];

    // Validate message
    let valid = if let Some(msg) = ent.message.as_bytes().get(..2) {
        ent.message.len() == 2
            && msg[0] >= b'a'
            && msg[0] <= b'z'
            && msg[1] >= b'a'
            && msg[1] <= b'z'
            && msg[0] != msg[1]
    } else {
        false
    };

    if !valid {
        let message = ent.message.clone();
        let origin = vtos(&ent.s.origin);
        gi_dprintf(&format!(
            "target_lightramp has bad ramp ({}) at {}\n",
            message, origin
        ));
        g_free_edict(game, self_idx as usize);
        return;
    }

    if deathmatch_value != 0.0 {
        g_free_edict(game, self_idx as usize);
        return;
    }

    if game.edicts[self_idx as usize].target.is_empty() {
        let classname = game.edicts[self_idx as usize].classname.clone();
        let origin = vtos(&game.edicts[self_idx as usize].s.origin);
        gi_dprintf(&format!("{} with no target at {}\n", classname, origin));
        g_free_edict(game, self_idx as usize);
        return;
    }

    let ent = &mut game.edicts[self_idx as usize];
    ent.svflags |= SVF_NOCLIENT;
    ent.use_fn = Some(USE_TARGET_LIGHTRAMP);
    ent.think_fn = Some(THINK_TARGET_LIGHTRAMP);

    let msg_bytes = ent.message.as_bytes();
    ent.movedir[0] = (msg_bytes[0] - b'a') as f32;
    ent.movedir[1] = (msg_bytes[1] - b'a') as f32;
    ent.movedir[2] = (ent.movedir[1] - ent.movedir[0]) / (ent.speed / FRAMETIME);
}

// ============================================================
// target_earthquake
// ============================================================

/// target_earthquake_think — shake the level.
pub fn target_earthquake_think(game: &mut GameContext, level: &LevelLocals, self_idx: i32) {
    let ent = &game.edicts[self_idx as usize];
    let self_origin = ent.s.origin;
    let noise_index = ent.noise_index;
    let speed = ent.speed;
    let timestamp = ent.timestamp;
    let last_move_time = ent.last_move_time;

    if last_move_time < level.time {
        gi_positioned_sound(&self_origin, self_idx, CHAN_AUTO, noise_index, 1.0, ATTN_NONE as f32, 0.0);
        game.edicts[self_idx as usize].last_move_time = level.time + 0.5;
    }

    // Iterate all edicts, shake players on the ground
    let num_edicts = game.num_edicts as usize;
    for i in 1..num_edicts {
        let e = &game.edicts[i];
        if !e.inuse {
            continue;
        }
        if e.client.is_none() {
            continue;
        }
        if e.groundentity <= 0 {
            continue;
        }

        let mass = e.mass;
        // crandom() returns [-1, 1)
        let cr1 = 2.0 * (rand::random::<f32>() - 0.5);
        let cr2 = 2.0 * (rand::random::<f32>() - 0.5);

        let e = &mut game.edicts[i];
        e.groundentity = -1; // NULL
        e.velocity[0] += cr1 * 150.0;
        e.velocity[1] += cr2 * 150.0;
        e.velocity[2] = speed * (100.0 / mass as f32);
    }

    if level.time < timestamp {
        game.edicts[self_idx as usize].nextthink = level.time + FRAMETIME;
    }
}

/// target_earthquake_use — start the earthquake.
pub fn target_earthquake_use(
    game: &mut GameContext,
    level: &LevelLocals,
    self_idx: i32,
    _other_idx: i32,
    activator_idx: i32,
) {
    let ent = &mut game.edicts[self_idx as usize];
    ent.timestamp = level.time + ent.count as f32;
    ent.nextthink = level.time + FRAMETIME;
    ent.activator = activator_idx;
    ent.last_move_time = 0.0;
}

/// SP_target_earthquake — spawn function.
pub fn sp_target_earthquake(game: &mut GameContext, self_idx: i32) {
    let ent = &game.edicts[self_idx as usize];
    if ent.targetname.is_empty() {
        let classname = ent.classname.clone();
        let origin = vtos(&ent.s.origin);
        gi_dprintf(&format!("untargeted {} at {}\n", classname, origin));
    }

    let ent = &mut game.edicts[self_idx as usize];
    if ent.count == 0 {
        ent.count = 5;
    }
    if ent.speed == 0.0 {
        ent.speed = 200.0;
    }

    ent.svflags |= SVF_NOCLIENT;
    ent.think_fn = Some(THINK_TARGET_EARTHQUAKE);
    ent.use_fn = Some(USE_TARGET_EARTHQUAKE);

    ent.noise_index = gi_soundindex("world/quake.wav");
}

// ============================================================
// Function dispatch table indices
//
// These constants serve as indices for the Option<usize> callback
// fields (use_fn, think_fn, etc.) on Edict. The actual dispatch
// is handled by a central match in the game loop.
// ============================================================

pub use crate::dispatch::USE_TARGET_TENT;
pub use crate::dispatch::USE_TARGET_SPEAKER;
pub use crate::dispatch::USE_TARGET_HELP;
pub use crate::dispatch::USE_TARGET_SECRET;
pub use crate::dispatch::USE_TARGET_GOAL;
pub use crate::dispatch::USE_TARGET_EXPLOSION;
pub use crate::dispatch::USE_TARGET_CHANGELEVEL;
pub use crate::dispatch::USE_TARGET_SPLASH;
pub use crate::dispatch::USE_TARGET_SPAWNER;
pub use crate::dispatch::USE_TARGET_BLASTER;
pub use crate::dispatch::USE_TRIGGER_CROSSLEVEL_TRIGGER;
pub use crate::dispatch::USE_TARGET_LASER;
pub use crate::dispatch::USE_TARGET_LIGHTRAMP;
pub use crate::dispatch::USE_TARGET_EARTHQUAKE;

pub use crate::dispatch::THINK_TARGET_EXPLOSION_EXPLODE;
pub use crate::dispatch::THINK_TARGET_CROSSLEVEL_TARGET;
pub const THINK_TARGET_LASER: usize = crate::dispatch::THINK_TARGET_LASER_THINK;
pub use crate::dispatch::THINK_TARGET_LASER_START;
pub const THINK_TARGET_LIGHTRAMP: usize = crate::dispatch::THINK_TARGET_LIGHTRAMP_THINK;
pub const THINK_TARGET_EARTHQUAKE: usize = crate::dispatch::THINK_TARGET_EARTHQUAKE_THINK;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::g_local::*;
    use crate::game::Solid;

    fn init_test_gi() {
        crate::game_import::set_gi(Box::new(crate::game_import::StubGameImport));
    }

    /// Helper: create a GameContext with N edicts (index 0 = world).
    fn make_ctx(num_edicts: usize) -> GameContext {
        init_test_gi();
        let mut ctx = GameContext::default();
        for _ in 0..num_edicts {
            ctx.edicts.push(Edict::default());
        }
        ctx.num_edicts = num_edicts as i32;
        ctx.max_edicts = (num_edicts + 10) as i32;
        ctx.game.maxclients = 1;
        ctx.maxclients = 1.0;
        ctx
    }

    // ============================================================
    // target_speaker tests
    // ============================================================

    #[test]
    fn test_use_target_speaker_looping_toggle_on() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].spawnflags = 1; // looping
        ctx.edicts[1].s.sound = 0; // currently off
        ctx.edicts[1].noise_index = 42;

        use_target_speaker(&mut ctx, 1, 0, 0);

        // Should toggle on: s.sound = noise_index
        assert_eq!(ctx.edicts[1].s.sound, 42);
    }

    #[test]
    fn test_use_target_speaker_looping_toggle_off() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].spawnflags = 1; // looping
        ctx.edicts[1].s.sound = 42; // currently on
        ctx.edicts[1].noise_index = 42;

        use_target_speaker(&mut ctx, 1, 0, 0);

        // Should toggle off: s.sound = 0
        assert_eq!(ctx.edicts[1].s.sound, 0);
    }

    #[test]
    fn test_use_target_speaker_looping_flag2() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].spawnflags = 2; // also looping (flags & 3 != 0)
        ctx.edicts[1].s.sound = 0;
        ctx.edicts[1].noise_index = 10;

        use_target_speaker(&mut ctx, 1, 0, 0);

        assert_eq!(ctx.edicts[1].s.sound, 10);
    }

    #[test]
    fn test_sp_target_speaker_defaults() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].volume = 0.0;
        ctx.edicts[1].attenuation = 0.0;
        ctx.edicts[1].spawnflags = 0;
        let st = SpawnTemp {
            noise: "sound/test.wav".to_string(),
            ..SpawnTemp::default()
        };

        sp_target_speaker(&mut ctx, 1, &st);

        // Default volume = 1.0
        assert!((ctx.edicts[1].volume - 1.0).abs() < f32::EPSILON);
        // Default attenuation = 1.0
        assert!((ctx.edicts[1].attenuation - 1.0).abs() < f32::EPSILON);
        assert_eq!(ctx.edicts[1].use_fn, Some(USE_TARGET_SPEAKER));
    }

    #[test]
    fn test_sp_target_speaker_negative_attenuation() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].volume = 0.5;
        ctx.edicts[1].attenuation = -1.0;
        ctx.edicts[1].spawnflags = 0;
        let st = SpawnTemp {
            noise: "test".to_string(),
            ..SpawnTemp::default()
        };

        sp_target_speaker(&mut ctx, 1, &st);

        // -1 attenuation means use 0 (no attenuation)
        assert!((ctx.edicts[1].attenuation - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_sp_target_speaker_prestart_looping() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].spawnflags = 1; // prestarted looping
        ctx.edicts[1].volume = 1.0;
        ctx.edicts[1].attenuation = 1.0;
        let st = SpawnTemp {
            noise: "ambient/loop.wav".to_string(),
            ..SpawnTemp::default()
        };

        sp_target_speaker(&mut ctx, 1, &st);

        // s.sound should be set to noise_index for pre-started looping
        assert_eq!(ctx.edicts[1].s.sound, ctx.edicts[1].noise_index);
    }

    #[test]
    fn test_sp_target_speaker_no_noise() {
        let mut ctx = make_ctx(3);
        let st = SpawnTemp {
            noise: String::new(),
            ..SpawnTemp::default()
        };

        sp_target_speaker(&mut ctx, 1, &st);

        // Should return early without setting use_fn
        assert!(ctx.edicts[1].use_fn.is_none());
    }

    #[test]
    fn test_sp_target_speaker_appends_wav() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].volume = 1.0;
        ctx.edicts[1].attenuation = 1.0;
        let st = SpawnTemp {
            noise: "sound/test".to_string(), // no .wav extension
            ..SpawnTemp::default()
        };

        sp_target_speaker(&mut ctx, 1, &st);

        // Should still set up successfully (buffer = "sound/test.wav")
        assert_eq!(ctx.edicts[1].use_fn, Some(USE_TARGET_SPEAKER));
    }

    // ============================================================
    // target_help tests
    // ============================================================

    #[test]
    fn test_use_target_help_message1() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].spawnflags = 1;
        ctx.edicts[1].message = "Test help msg".to_string();
        let mut game_locals = GameLocals::default();

        use_target_help(&mut ctx, &mut game_locals, 1, 0, 0);

        assert_eq!(game_locals.helpmessage1, "Test help msg");
        assert_eq!(game_locals.helpchanged, 1);
    }

    #[test]
    fn test_use_target_help_message2() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].spawnflags = 0;
        ctx.edicts[1].message = "Second message".to_string();
        let mut game_locals = GameLocals::default();

        use_target_help(&mut ctx, &mut game_locals, 1, 0, 0);

        assert_eq!(game_locals.helpmessage2, "Second message");
        assert_eq!(game_locals.helpchanged, 1);
    }

    #[test]
    fn test_sp_target_help_deathmatch_frees() {
        // Entity must have index > maxclients + BODY_QUEUE_SIZE (1+8=9) to be freed
        let mut ctx = make_ctx(12);
        ctx.edicts[10].inuse = true;
        ctx.edicts[10].message = "some help".to_string();

        sp_target_help(&mut ctx, 10, 1.0); // deathmatch != 0

        assert!(!ctx.edicts[10].inuse);
    }

    #[test]
    fn test_sp_target_help_no_message_frees() {
        let mut ctx = make_ctx(12);
        ctx.edicts[10].inuse = true;
        ctx.edicts[10].message = String::new();

        sp_target_help(&mut ctx, 10, 0.0);

        assert!(!ctx.edicts[10].inuse);
    }

    #[test]
    fn test_sp_target_help_success() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].message = "valid help".to_string();

        sp_target_help(&mut ctx, 1, 0.0);

        assert_eq!(ctx.edicts[1].use_fn, Some(USE_TARGET_HELP));
    }

    // ============================================================
    // target_secret tests
    // ============================================================

    #[test]
    fn test_sp_target_secret_deathmatch_frees() {
        let mut ctx = make_ctx(12);
        ctx.edicts[10].inuse = true;
        let mut level = LevelLocals::default();
        let mut st = SpawnTemp::default();

        sp_target_secret(&mut ctx, &mut level, 10, &mut st, 1.0);

        assert!(!ctx.edicts[10].inuse);
    }

    #[test]
    fn test_sp_target_secret_increments_total() {
        let mut ctx = make_ctx(3);
        let mut level = LevelLocals::default();
        let mut st = SpawnTemp::default();

        sp_target_secret(&mut ctx, &mut level, 1, &mut st, 0.0);

        assert_eq!(level.total_secrets, 1);
        assert_eq!(ctx.edicts[1].use_fn, Some(USE_TARGET_SECRET));
        assert_eq!(ctx.edicts[1].svflags, SVF_NOCLIENT);
    }

    #[test]
    fn test_sp_target_secret_default_noise() {
        let mut ctx = make_ctx(3);
        let mut level = LevelLocals::default();
        let mut st = SpawnTemp {
            noise: String::new(),
            ..SpawnTemp::default()
        };

        sp_target_secret(&mut ctx, &mut level, 1, &mut st, 0.0);

        // Default noise should be set to misc/secret.wav
        assert_eq!(st.noise, "misc/secret.wav");
    }

    #[test]
    fn test_sp_target_secret_mine3_hack() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].s.origin = [280.0, -2048.0, -624.0];
        let mut level = LevelLocals::default();
        level.mapname = "mine3".to_string();
        let mut st = SpawnTemp::default();

        sp_target_secret(&mut ctx, &mut level, 1, &mut st, 0.0);

        assert_eq!(ctx.edicts[1].message, "You have found a secret area.");
    }

    // ============================================================
    // target_goal tests
    // ============================================================

    #[test]
    fn test_sp_target_goal_increments_total() {
        let mut ctx = make_ctx(3);
        let mut level = LevelLocals::default();
        let mut st = SpawnTemp::default();

        sp_target_goal(&mut ctx, &mut level, 1, &mut st, 0.0);

        assert_eq!(level.total_goals, 1);
        assert_eq!(ctx.edicts[1].use_fn, Some(USE_TARGET_GOAL));
    }

    #[test]
    fn test_sp_target_goal_deathmatch_frees() {
        let mut ctx = make_ctx(12);
        ctx.edicts[10].inuse = true;
        let mut level = LevelLocals::default();
        let mut st = SpawnTemp::default();

        sp_target_goal(&mut ctx, &mut level, 10, &mut st, 1.0);

        assert!(!ctx.edicts[10].inuse);
    }

    // ============================================================
    // target_explosion tests
    // ============================================================

    #[test]
    fn test_sp_target_explosion_setup() {
        let mut ctx = make_ctx(3);

        sp_target_explosion(&mut ctx, 1);

        assert_eq!(ctx.edicts[1].use_fn, Some(USE_TARGET_EXPLOSION));
        assert_eq!(ctx.edicts[1].svflags, SVF_NOCLIENT);
    }

    #[test]
    fn test_use_target_explosion_with_delay() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].delay = 2.0;
        let mut level = LevelLocals::default();
        level.time = 5.0;

        use_target_explosion(&mut ctx, &mut level, 1, 0, 2);

        assert_eq!(ctx.edicts[1].activator, 2);
        // With delay, should set think and nextthink
        assert_eq!(ctx.edicts[1].think_fn, Some(THINK_TARGET_EXPLOSION_EXPLODE));
        assert!((ctx.edicts[1].nextthink - 7.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_use_target_explosion_no_delay_sets_activator() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].delay = 0.0;
        ctx.edicts[1].dmg = 50;
        let mut level = LevelLocals::default();
        level.time = 5.0;

        use_target_explosion(&mut ctx, &mut level, 1, 0, 2);

        // No delay => immediate explosion (activator set)
        assert_eq!(ctx.edicts[1].activator, 2);
    }

    // ============================================================
    // target_changelevel tests
    // ============================================================

    #[test]
    fn test_sp_target_changelevel_no_map_frees() {
        let mut ctx = make_ctx(12);
        ctx.edicts[10].map = String::new();
        ctx.edicts[10].inuse = true;
        let level = LevelLocals::default();

        sp_target_changelevel(&mut ctx, &level, 10);

        assert!(!ctx.edicts[10].inuse);
    }

    #[test]
    fn test_sp_target_changelevel_success() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].map = "base1".to_string();
        let level = LevelLocals::default();

        sp_target_changelevel(&mut ctx, &level, 1);

        assert_eq!(ctx.edicts[1].use_fn, Some(USE_TARGET_CHANGELEVEL));
        assert_eq!(ctx.edicts[1].svflags, SVF_NOCLIENT);
    }

    #[test]
    fn test_sp_target_changelevel_fact1_hack() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].map = "fact3".to_string();
        let mut level = LevelLocals::default();
        level.mapname = "fact1".to_string();

        sp_target_changelevel(&mut ctx, &level, 1);

        assert_eq!(ctx.edicts[1].map, "fact3$secret1");
    }

    // ============================================================
    // target_splash tests
    // ============================================================

    #[test]
    fn test_sp_target_splash_defaults() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].count = 0;
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0];

        sp_target_splash(&mut ctx, 1);

        assert_eq!(ctx.edicts[1].count, 32);
        assert_eq!(ctx.edicts[1].svflags, SVF_NOCLIENT);
        assert_eq!(ctx.edicts[1].use_fn, Some(USE_TARGET_SPLASH));
        // angles should be cleared
        assert_eq!(ctx.edicts[1].s.angles, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_sp_target_splash_custom_count() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].count = 64;
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0];

        sp_target_splash(&mut ctx, 1);

        assert_eq!(ctx.edicts[1].count, 64);
    }

    // ============================================================
    // target_blaster tests
    // ============================================================

    #[test]
    fn test_sp_target_blaster_defaults() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].dmg = 0;
        ctx.edicts[1].speed = 0.0;
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0];

        sp_target_blaster(&mut ctx, 1);

        assert_eq!(ctx.edicts[1].dmg, 15);
        assert!((ctx.edicts[1].speed - 1000.0).abs() < f32::EPSILON);
        assert_eq!(ctx.edicts[1].svflags, SVF_NOCLIENT);
        assert_eq!(ctx.edicts[1].use_fn, Some(USE_TARGET_BLASTER));
    }

    #[test]
    fn test_sp_target_blaster_custom_values() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].dmg = 50;
        ctx.edicts[1].speed = 2000.0;
        ctx.edicts[1].s.angles = [0.0, 90.0, 0.0];

        sp_target_blaster(&mut ctx, 1);

        assert_eq!(ctx.edicts[1].dmg, 50);
        assert!((ctx.edicts[1].speed - 2000.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_use_target_blaster_effect_selection() {
        let mut ctx = make_ctx(3);
        // Default: EF_BLASTER
        ctx.edicts[1].spawnflags = 0;
        let ent = &ctx.edicts[1];
        let effect: u32 = if ent.spawnflags & 2 != 0 { 0 }
                          else if ent.spawnflags & 1 != 0 { EF_HYPERBLASTER }
                          else { EF_BLASTER };
        assert_eq!(effect, EF_BLASTER);

        // Hyperblaster
        ctx.edicts[1].spawnflags = 1;
        let ent = &ctx.edicts[1];
        let effect: u32 = if ent.spawnflags & 2 != 0 { 0 }
                          else if ent.spawnflags & 1 != 0 { EF_HYPERBLASTER }
                          else { EF_BLASTER };
        assert_eq!(effect, EF_HYPERBLASTER);

        // No effect
        ctx.edicts[1].spawnflags = 2;
        let ent = &ctx.edicts[1];
        let effect: u32 = if ent.spawnflags & 2 != 0 { 0 }
                          else if ent.spawnflags & 1 != 0 { EF_HYPERBLASTER }
                          else { EF_BLASTER };
        assert_eq!(effect, 0);
    }

    // ============================================================
    // target_crosslevel tests
    // ============================================================

    #[test]
    fn test_sp_target_crosslevel_trigger() {
        let mut ctx = make_ctx(3);

        sp_target_crosslevel_trigger(&mut ctx, 1);

        assert_eq!(ctx.edicts[1].svflags, SVF_NOCLIENT);
        assert_eq!(ctx.edicts[1].use_fn, Some(USE_TRIGGER_CROSSLEVEL_TRIGGER));
    }

    #[test]
    fn test_trigger_crosslevel_trigger_use_sets_flags() {
        let mut ctx = make_ctx(12);
        ctx.edicts[10].spawnflags = 0x0F;
        ctx.edicts[10].inuse = true;
        let mut game_locals = GameLocals::default();
        game_locals.serverflags = 0;

        trigger_crosslevel_trigger_use(&mut ctx, &mut game_locals, 10, 0, 0);

        assert_eq!(game_locals.serverflags, 0x0F);
        // Entity should be freed
        assert!(!ctx.edicts[10].inuse);
    }

    #[test]
    fn test_sp_target_crosslevel_target_default_delay() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].delay = 0.0;
        let level = LevelLocals { time: 5.0, ..LevelLocals::default() };

        sp_target_crosslevel_target(&mut ctx, &level, 1);

        // Default delay = 1.0
        assert!((ctx.edicts[1].delay - 1.0).abs() < f32::EPSILON);
        assert_eq!(ctx.edicts[1].svflags, SVF_NOCLIENT);
        // nextthink = time + delay = 5.0 + 1.0 = 6.0
        assert!((ctx.edicts[1].nextthink - 6.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_target_crosslevel_target_think_matching_flags() {
        let mut ctx = make_ctx(12);
        ctx.edicts[10].spawnflags = 0x05;
        ctx.edicts[10].inuse = true;
        let mut game_locals = GameLocals::default();
        game_locals.serverflags = 0x07; // 0x05 & 0xFF & 0x05 = 0x05 == 0x05

        target_crosslevel_target_think(&mut ctx, &game_locals, 10);

        // Should fire targets and free itself
        assert!(!ctx.edicts[10].inuse);
    }

    #[test]
    fn test_target_crosslevel_target_think_no_match() {
        let mut ctx = make_ctx(12);
        ctx.edicts[10].spawnflags = 0x05;
        ctx.edicts[10].inuse = true;
        let game_locals = GameLocals {
            serverflags: 0x01, // 0x05 != 0x01 & 0xFF & 0x05 = 0x01
            ..GameLocals::default()
        };

        target_crosslevel_target_think(&mut ctx, &game_locals, 10);

        // Should NOT fire (entity remains in use)
        assert!(ctx.edicts[10].inuse);
    }

    // ============================================================
    // target_laser tests
    // ============================================================

    #[test]
    fn test_target_laser_off() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].spawnflags = 1;
        ctx.edicts[1].svflags = 0;
        ctx.edicts[1].nextthink = 10.0;

        target_laser_off(&mut ctx, 1);

        assert_eq!(ctx.edicts[1].spawnflags & 1, 0);
        assert_ne!(ctx.edicts[1].svflags & SVF_NOCLIENT, 0);
        assert_eq!(ctx.edicts[1].nextthink, 0.0);
    }

    #[test]
    fn test_target_laser_use_toggle_off() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].spawnflags = 1; // currently on
        ctx.edicts[1].svflags = 0;
        let mut level = LevelLocals::default();

        target_laser_use(&mut ctx, &mut level, 1, 0, 2);

        // Should turn off
        assert_eq!(ctx.edicts[1].activator, 2);
        assert_eq!(ctx.edicts[1].spawnflags & 1, 0);
        assert_ne!(ctx.edicts[1].svflags & SVF_NOCLIENT, 0);
    }

    #[test]
    fn test_target_laser_endpoint_calculation() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].s.origin = [100.0, 0.0, 0.0];
        ctx.edicts[1].movedir = [1.0, 0.0, 0.0];

        // The laser endpoint: start + 2048 * movedir
        let start = ctx.edicts[1].s.origin;
        let movedir = ctx.edicts[1].movedir;
        let end = [
            start[0] + 2048.0 * movedir[0],
            start[1] + 2048.0 * movedir[1],
            start[2] + 2048.0 * movedir[2],
        ];

        assert!((end[0] - 2148.0).abs() < f32::EPSILON);
        assert_eq!(end[1], 0.0);
        assert_eq!(end[2], 0.0);
    }

    #[test]
    fn test_target_laser_enemy_tracking() {
        let mut ctx = make_ctx(4);
        ctx.edicts[1].s.origin = [0.0, 0.0, 0.0];
        ctx.edicts[1].enemy = 2;
        ctx.edicts[1].movedir = [1.0, 0.0, 0.0];

        // Enemy is at center [50, 50, 50]
        ctx.edicts[2].absmin = [40.0, 40.0, 40.0];
        ctx.edicts[2].size = [20.0, 20.0, 20.0];

        // Calculate expected target point
        let point = [
            ctx.edicts[2].absmin[0] + 0.5 * ctx.edicts[2].size[0],
            ctx.edicts[2].absmin[1] + 0.5 * ctx.edicts[2].size[1],
            ctx.edicts[2].absmin[2] + 0.5 * ctx.edicts[2].size[2],
        ];

        assert!((point[0] - 50.0).abs() < f32::EPSILON);
        assert!((point[1] - 50.0).abs() < f32::EPSILON);
        assert!((point[2] - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_sp_target_laser_defers_start() {
        let mut ctx = make_ctx(3);
        let level = LevelLocals { time: 5.0, ..LevelLocals::default() };

        sp_target_laser(&mut ctx, &level, 1);

        assert_eq!(ctx.edicts[1].think_fn, Some(THINK_TARGET_LASER_START));
        assert!((ctx.edicts[1].nextthink - 6.0).abs() < f32::EPSILON);
    }

    // ============================================================
    // target_laser_start beam setup tests
    // ============================================================

    #[test]
    fn test_target_laser_start_beam_diameter_default() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].spawnflags = 0; // no 64 flag
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0];
        ctx.edicts[1].target = String::new();
        ctx.edicts[1].enemy = 0;
        ctx.edicts[1].dmg = 0;
        let mut level = LevelLocals { time: 0.0, ..LevelLocals::default() };

        target_laser_start(&mut ctx, &mut level, 1);

        assert_eq!(ctx.edicts[1].s.frame, 4); // default beam width
        assert_eq!(ctx.edicts[1].dmg, 1); // default damage
    }

    #[test]
    fn test_target_laser_start_beam_diameter_fat() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].spawnflags = 64; // fat beam
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0];
        ctx.edicts[1].target = String::new();
        ctx.edicts[1].enemy = 0;
        ctx.edicts[1].dmg = 10;
        let mut level = LevelLocals { time: 0.0, ..LevelLocals::default() };

        target_laser_start(&mut ctx, &mut level, 1);

        assert_eq!(ctx.edicts[1].s.frame, 16); // fat beam
    }

    #[test]
    fn test_target_laser_start_color_red() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].spawnflags = 2; // red
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0];
        ctx.edicts[1].target = String::new();
        ctx.edicts[1].enemy = 0;
        let mut level = LevelLocals { time: 0.0, ..LevelLocals::default() };

        target_laser_start(&mut ctx, &mut level, 1);

        assert_eq!(ctx.edicts[1].s.skinnum, 0xf2f2f0f0u32 as i32);
    }

    #[test]
    fn test_target_laser_start_color_blue() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].spawnflags = 4; // blue
        ctx.edicts[1].s.angles = [0.0, 0.0, 0.0];
        ctx.edicts[1].target = String::new();
        ctx.edicts[1].enemy = 0;
        let mut level = LevelLocals { time: 0.0, ..LevelLocals::default() };

        target_laser_start(&mut ctx, &mut level, 1);

        assert_eq!(ctx.edicts[1].s.skinnum, 0xd0d1d2d3u32 as i32);
    }

    // ============================================================
    // target_lightramp tests
    // ============================================================

    #[test]
    fn test_sp_target_lightramp_valid_message() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].message = "az".to_string();
        ctx.edicts[1].target = "light1".to_string();
        ctx.edicts[1].speed = 1.0;

        sp_target_lightramp(&mut ctx, 1, 0.0);

        assert_eq!(ctx.edicts[1].use_fn, Some(USE_TARGET_LIGHTRAMP));
        // movedir[0] = 'a' - 'a' = 0
        assert!((ctx.edicts[1].movedir[0] - 0.0).abs() < f32::EPSILON);
        // movedir[1] = 'z' - 'a' = 25
        assert!((ctx.edicts[1].movedir[1] - 25.0).abs() < f32::EPSILON);
        // movedir[2] = (25 - 0) / (1.0 / 0.1) = 25/10 = 2.5
        assert!((ctx.edicts[1].movedir[2] - 2.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_sp_target_lightramp_invalid_message_frees() {
        let mut ctx = make_ctx(12);
        ctx.edicts[10].inuse = true;
        ctx.edicts[10].message = "a".to_string(); // too short

        sp_target_lightramp(&mut ctx, 10, 0.0);

        assert!(!ctx.edicts[10].inuse);
    }

    #[test]
    fn test_sp_target_lightramp_same_chars_frees() {
        let mut ctx = make_ctx(12);
        ctx.edicts[10].inuse = true;
        ctx.edicts[10].message = "aa".to_string(); // same chars

        sp_target_lightramp(&mut ctx, 10, 0.0);

        assert!(!ctx.edicts[10].inuse);
    }

    #[test]
    fn test_sp_target_lightramp_deathmatch_frees() {
        let mut ctx = make_ctx(12);
        ctx.edicts[10].inuse = true;
        ctx.edicts[10].message = "az".to_string();
        ctx.edicts[10].target = "light1".to_string();

        sp_target_lightramp(&mut ctx, 10, 1.0); // deathmatch

        assert!(!ctx.edicts[10].inuse);
    }

    #[test]
    fn test_sp_target_lightramp_no_target_frees() {
        let mut ctx = make_ctx(12);
        ctx.edicts[10].inuse = true;
        ctx.edicts[10].message = "az".to_string();
        ctx.edicts[10].target = String::new();

        sp_target_lightramp(&mut ctx, 10, 0.0);

        assert!(!ctx.edicts[10].inuse);
    }

    #[test]
    fn test_target_lightramp_interpolation_values() {
        // Test the interpolation math in target_lightramp_think
        // char_val = 'a' + movedir[0] + (level.time - timestamp) / FRAMETIME * movedir[2]
        let movedir_start = 0.0_f32; // 'a'
        let movedir_end = 25.0_f32;  // 'z'
        let speed = 1.0_f32;
        let step = (movedir_end - movedir_start) / (speed / FRAMETIME);

        // At t=0: char = 'a' + 0 + 0 = 'a'
        let char_at_start = (b'a' as f32 + movedir_start + 0.0 / FRAMETIME * step) as u8;
        assert_eq!(char_at_start, b'a');

        // At t=speed (1.0): char = 'a' + 0 + (1.0/0.1) * 2.5 = 'a' + 25 = 'z'
        let char_at_end = (b'a' as f32 + movedir_start + (speed / FRAMETIME) * step) as u8;
        assert_eq!(char_at_end, b'z');
    }

    // ============================================================
    // target_earthquake tests
    // ============================================================

    #[test]
    fn test_sp_target_earthquake_defaults() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].count = 0;
        ctx.edicts[1].speed = 0.0;
        ctx.edicts[1].targetname = "quake1".to_string();

        sp_target_earthquake(&mut ctx, 1);

        assert_eq!(ctx.edicts[1].count, 5);
        assert!((ctx.edicts[1].speed - 200.0).abs() < f32::EPSILON);
        assert_ne!(ctx.edicts[1].svflags & SVF_NOCLIENT, 0);
        assert_eq!(ctx.edicts[1].think_fn, Some(THINK_TARGET_EARTHQUAKE));
        assert_eq!(ctx.edicts[1].use_fn, Some(USE_TARGET_EARTHQUAKE));
    }

    #[test]
    fn test_sp_target_earthquake_custom_values() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].count = 10;
        ctx.edicts[1].speed = 500.0;
        ctx.edicts[1].targetname = "quake1".to_string();

        sp_target_earthquake(&mut ctx, 1);

        assert_eq!(ctx.edicts[1].count, 10);
        assert!((ctx.edicts[1].speed - 500.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_target_earthquake_use_sets_timing() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].count = 5;
        let level = LevelLocals { time: 10.0, ..LevelLocals::default() };

        target_earthquake_use(&mut ctx, &level, 1, 0, 2);

        // timestamp = time + count = 10 + 5 = 15
        assert!((ctx.edicts[1].timestamp - 15.0).abs() < f32::EPSILON);
        // nextthink = time + FRAMETIME = 10.1
        assert!((ctx.edicts[1].nextthink - (10.0 + FRAMETIME)).abs() < f32::EPSILON);
        assert_eq!(ctx.edicts[1].activator, 2);
        assert_eq!(ctx.edicts[1].last_move_time, 0.0);
    }

    #[test]
    fn test_target_earthquake_shake_velocity() {
        // Test the earthquake velocity formula: vel[2] = speed * (100.0 / mass)
        let speed = 200.0_f32;
        let mass = 200_i32;
        let vel_z = speed * (100.0 / mass as f32);
        // 200 * (100/200) = 100
        assert!((vel_z - 100.0).abs() < f32::EPSILON);

        // Lighter entity gets more upward velocity
        let mass = 100_i32;
        let vel_z = speed * (100.0 / mass as f32);
        // 200 * (100/100) = 200
        assert!((vel_z - 200.0).abs() < f32::EPSILON);

        // Heavier entity gets less
        let mass = 400_i32;
        let vel_z = speed * (100.0 / mass as f32);
        // 200 * (100/400) = 50
        assert!((vel_z - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_target_earthquake_think_continues() {
        let mut ctx = make_ctx(4);
        ctx.edicts[1].s.origin = [0.0, 0.0, 0.0];
        ctx.edicts[1].noise_index = 5;
        ctx.edicts[1].speed = 200.0;
        ctx.edicts[1].timestamp = 20.0; // earthquake still active
        ctx.edicts[1].last_move_time = 0.0;
        ctx.num_edicts = 4;

        // No player entities to shake (no clients)
        let level = LevelLocals { time: 10.0, ..LevelLocals::default() };

        target_earthquake_think(&mut ctx, &level, 1);

        // Should schedule next think since time(10) < timestamp(20)
        assert!((ctx.edicts[1].nextthink - (10.0 + FRAMETIME)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_target_earthquake_think_expires() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].s.origin = [0.0, 0.0, 0.0];
        ctx.edicts[1].noise_index = 5;
        ctx.edicts[1].speed = 200.0;
        ctx.edicts[1].timestamp = 5.0; // already expired
        ctx.edicts[1].last_move_time = 0.0;
        ctx.num_edicts = 3;
        let level = LevelLocals { time: 10.0, ..LevelLocals::default() };

        target_earthquake_think(&mut ctx, &level, 1);

        // Should NOT schedule next think since time(10) >= timestamp(5)
        assert_eq!(ctx.edicts[1].nextthink, 0.0);
    }

    // ============================================================
    // target_spawner tests
    // ============================================================

    #[test]
    fn test_sp_target_spawner_setup() {
        let mut ctx = make_ctx(3);

        sp_target_spawner(&mut ctx, 1);

        assert_eq!(ctx.edicts[1].use_fn, Some(USE_TARGET_SPAWNER));
        assert_eq!(ctx.edicts[1].svflags, SVF_NOCLIENT);
    }

    #[test]
    fn test_sp_target_spawner_with_speed_computes_movedir() {
        let mut ctx = make_ctx(3);
        ctx.edicts[1].speed = 100.0;
        ctx.edicts[1].s.angles = [0.0, 90.0, 0.0]; // facing Y direction

        sp_target_spawner(&mut ctx, 1);

        // movedir should be computed from angles and scaled by speed
        // With yaw=90, forward ~ [0, 1, 0], scaled by 100 => [0, 100, 0]
        assert!(ctx.edicts[1].movedir[1].abs() > 90.0); // approximately 100
        // angles should be cleared
        assert_eq!(ctx.edicts[1].s.angles, [0.0, 0.0, 0.0]);
    }
}
