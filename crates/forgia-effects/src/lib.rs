//! # forgia-effects
//!
//! Hanabi VFX + audio combat + HitFlashCache.
//!
//! **Pattern obligatoire Phase 0** : pre-spawn dummy `ParticleEffect` `Visibility::Hidden`
//! au Startup pour payer le shader compile lazy AVANT le 1er tir.
//! Sinon freeze 25s confirmé V1 (story-436).
//!
//! Modules portés V1 :
//! - `weapon_vfx/` : muzzle flash (5-layer), impact (3-layer), tracers cache
//! - `arena_feedback` : SFX kill confirm + damage player (story-427)
//! - (combat_juice vit dans forgia-combat car couplé à CombatHitEvent)

use bevy::prelude::*;
use forgia_core::prelude::*;

pub mod arena_feedback;
pub mod weapon_vfx;

// TODO: port from V1 effects/ — les modules suivants nécessitent des deps supplémentaires
// pub mod fireball_vfx;   // needs forgia-player (Fireball, IceBolt, GoblinOnFire, etc.)
// pub mod ice_vfx;        // needs forgia-player
// pub mod shield_vfx;     // needs forgia-player
// pub mod levelup_vfx;    // needs forgia-combat (LevelUpEvent)
// pub mod boss_vfx;       // needs forgia-player (GoblinBoss)
// pub mod biome_particles; // needs forgia-terrain (BiomeMap)
// pub mod weather;         // needs forgia-terrain (BiomeMap)
// pub mod biome_ambiance;  // needs forgia-terrain (BiomeMap)
// pub mod particles;       // needs forgia-terrain (VillageBuildingInstance)
// pub mod vignette;        // needs forgia-core pp
// pub mod wind;            // needs genome
// pub mod fade_in;         // needs forgia-core (FadeIn component)

pub mod prelude {
    pub use crate::arena_feedback::{ArenaFeedbackPlugin, ArenaFeedbackStats};
    pub use crate::weapon_vfx::tracer::{spawn_hitscan_tracer, EmissiveFade, TracerResources};
    pub use crate::weapon_vfx::{
        spawn_impact_vfx, spawn_muzzle_flash, ImpactVfxMarker, Lifetime, MuzzleVfxMarker,
        WeaponVfxEffects,
    };
    pub use crate::ForgiaEffectsPlugin;
}

pub struct ForgiaEffectsPlugin;

impl Plugin for ForgiaEffectsPlugin {
    fn build(&self, app: &mut App) {
        // Pre-spawn hanabi dummies at Startup to pay shader compile cost
        // BEFORE first shot (pattern story-436 / reference_hanabi_shader_compile_lazy_pattern.md)
        app.add_systems(Startup, prespawn_hanabi_dummies)
            .add_systems(Startup, weapon_vfx::setup_weapon_vfx)
            .add_systems(Startup, weapon_vfx::tracer::setup_tracer_resources)
            .add_plugins(arena_feedback::ArenaFeedbackPlugin)
            .add_systems(
                Update,
                (
                    effects_tick,
                    emissive_fade_tick,
                    lifetime_tick,
                    weapon_vfx::tracer::tick_bullets_in_flight,
                )
                    .in_set(GameSet::Effects),
            );
    }
}

/// Tick `Lifetime` timers — despawn entity à expiration (Muzzle/Impact VFX).
fn lifetime_tick(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut weapon_vfx::Lifetime)>,
) {
    for (entity, mut life) in &mut q {
        life.0.tick(time.delta());
        if life.0.is_finished() {
            if let Ok(mut ec) = commands.get_entity(entity) {
                ec.try_despawn();
            }
        }
    }
}

/// Tick `EmissiveFade` timers — interpolate emissive vers 0 puis despawn entité.
fn emissive_fade_tick(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut q: Query<(
        Entity,
        &mut weapon_vfx::tracer::EmissiveFade,
        &MeshMaterial3d<StandardMaterial>,
    )>,
) {
    for (entity, mut fade, mat) in &mut q {
        fade.timer.tick(time.delta());
        let pct = 1.0 - fade.timer.fraction();
        if let Some(material) = materials.get_mut(&mat.0) {
            material.emissive = LinearRgba::new(
                fade.initial.red * pct,
                fade.initial.green * pct,
                fade.initial.blue * pct,
                fade.initial.alpha,
            );
        }
        if fade.timer.is_finished() {
            if let Ok(mut ec) = commands.get_entity(entity) {
                ec.try_despawn();
            }
        }
    }
}

fn prespawn_hanabi_dummies(mut _commands: Commands) {
    // TODO: Phase 2 — spawn 8 dummy `ParticleEffect` `Visibility::Hidden` at Y=-10000
    // to pay shader compile cost BEFORE first shot.
    // Pattern: memory `reference_hanabi_shader_compile_lazy_pattern.md`
    info!("[forgia-effects] prespawn_hanabi_dummies — placeholder (Phase 2 adds 8 dummies)");
}

fn effects_tick() {
    // TODO: Phase 2 — Lifetime cleanup tick for MuzzleVfxMarker + ImpactVfxMarker entities
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_constructible() {
        let _p = ForgiaEffectsPlugin;
    }
}
