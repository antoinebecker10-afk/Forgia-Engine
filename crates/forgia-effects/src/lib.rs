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
// Story-523 fusion : 4 VFX-adjacent crates intégrées comme modules.
pub mod damage_numbers;
pub mod hitmarker;
pub mod mesh_fader;
pub mod vfx_tracers;

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
    // Re-export pour les consommateurs (ex. forgia-mode-roguelite status_vfx) qui
    // spawnent une aura sans dépendre directement de bevy_hanabi.
    // Story-647 : EffectMaterial aussi — obligatoire sur toute entité ParticleEffect
    // depuis que les EffectAsset déclarent un slot texture.
    pub use bevy_hanabi::{EffectMaterial, ParticleEffect};
    pub use crate::ForgiaEffectsPlugin;
    // Story-523 re-exports des modules fusionnés.
    pub use crate::damage_numbers::ForgiaDamageNumbersPlugin;
    pub use crate::hitmarker::{ForgiaHitmarkerPlugin, HitmarkerState};
    pub use crate::mesh_fader::{MaterialFader, MaterialFaderCloned, MeshFaderPlugin};
    pub use crate::vfx_tracers::ForgiaVfxTracersPlugin;
}

pub struct ForgiaEffectsPlugin;

impl Plugin for ForgiaEffectsPlugin {
    fn build(&self, app: &mut App) {
        // Pre-spawn hanabi dummies pour payer le shader compile AVANT le 1er tir
        // (pattern story-436 / reference_hanabi_shader_compile_lazy_pattern.md).
        // PostStartup : setup_weapon_vfx (Startup) doit avoir inséré WeaponVfxEffects.
        app.add_systems(PostStartup, prespawn_hanabi_dummies)
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

/// Pré-spawn 1 dummy `ParticleEffect` caché par EffectAsset (8) à Y=-10000 :
/// hanabi compile ses shaders à la première préparation d'un effet — sans ce
/// warmup, le PREMIER tir de la session paie la compile (freeze 25 s confirmé
/// V1, story-436). Les dummies vivent 5 s puis `lifetime_tick` les despawn.
/// Était un placeholder « TODO Phase 2 » depuis Phase 0 — implémenté story-594
/// (audit 2026-06-10 P1, M2-B5).
fn prespawn_hanabi_dummies(mut commands: Commands, effects: Res<weapon_vfx::WeaponVfxEffects>) {
    // Story-647 : chaque dummy binde aussi sa texture (EffectMaterial) — un effet
    // à slot texture sans material ne prépare pas son bind group, le warmup
    // shader serait incomplet.
    let handles = [
        (&effects.muzzle_core_flash, &effects.tex_muzzle_flash),
        (&effects.muzzle_sparks, &effects.tex_spark),
        (&effects.muzzle_smoke, &effects.tex_smoke),
        (&effects.muzzle_heat_glow, &effects.tex_glow),
        (&effects.muzzle_forward_flash, &effects.tex_muzzle_tongue),
        (&effects.impact_sparks, &effects.tex_spark),
        (&effects.impact_dust, &effects.tex_dust),
        (&effects.impact_flash, &effects.tex_flare),
        (&effects.status_flame, &effects.tex_flame),
        (&effects.status_poison_cloud, &effects.tex_poison),
    ];
    for (handle, tex) in handles {
        commands.spawn((
            bevy_hanabi::ParticleEffect::new((*handle).clone()),
            bevy_hanabi::EffectMaterial {
                images: vec![(*tex).clone()],
            },
            Transform::from_xyz(0.0, -10_000.0, 0.0),
            Visibility::Hidden,
            weapon_vfx::Lifetime(Timer::from_seconds(5.0, TimerMode::Once)),
        ));
    }
    info!("[forgia-effects] prespawn 10 hanabi dummies (shader warmup, story-594)");
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
