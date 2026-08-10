//! # forgia-effects
//!
//! Hanabi VFX + audio combat.
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
pub mod pipeline_ready;
pub mod weapon_vfx;
// Story-523 fusion : 4 VFX-adjacent crates intégrées comme modules.
pub mod damage_numbers;
// L'envol des morts (2026-08-05) — reprise du `goblin_death_system` du RPG V1,
// avec une vraie traînée lumineuse. Ici parce que forgia-effects est la
// dépendance commune de forgia-fps (qui laisse passer) et des modes (qui
// déclenchent).
pub mod death_ascension;
pub mod hitmarker;
pub mod mesh_fader;
pub mod vfx_tracers;

// Modules V1 non portés : voir docs/audits/v1-nonported-classification-2026-08-05.md.
// Ils ne constituent pas une liste de TODO implicite : chacun doit être justifié
// par le scope Roguelite et son architecture V2 avant d'être réintroduit.
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
        spawn_impact_vfx, spawn_kill_burst, spawn_muzzle_flash, ImpactVfxMarker, Lifetime,
        MuzzleVfxMarker, VfxTuning, WeaponVfxEffects,
    };
    // Re-export pour les consommateurs (ex. forgia-mode-roguelite status_vfx) qui
    // spawnent une aura sans dépendre directement de bevy_hanabi.
    // Story-647 : EffectMaterial aussi — obligatoire sur toute entité ParticleEffect
    // depuis que les EffectAsset déclarent un slot texture.
    pub use crate::ForgiaEffectsPlugin;
    pub use bevy_hanabi::{EffectMaterial, ParticleEffect};
    // Audit fire-path 2026-07-20 — détecteur générique de compilation de pipelines
    // (anti-freeze de specialization au 1er affichage, consommé par le warmup décor).
    pub use crate::pipeline_ready::{PipelinesReady, PipelinesReadyPlugin};
    // Story-523 re-exports des modules fusionnés.
    pub use crate::damage_numbers::ForgiaDamageNumbersPlugin;
    pub use crate::death_ascension::{
        AscendPhase, Ascending, AscendsOnDeath, DeathAscensionPlugin, DeathAscensionStats,
        DeathAscensionTuning,
    };
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
        // Audit fire-path 2026-07-20 — détecteur « pipelines compilés » (RenderApp),
        // consommé par le warmup PBR du Lobby roguelite. Idempotent (skip si RenderApp absent).
        app.add_plugins(pipeline_ready::PipelinesReadyPlugin);
        // L'envol des morts + sa traînée (2026-08-05). Inerte tant qu'aucune
        // entité ne porte `Ascending` : aucun coût pour les modes qui ne
        // l'utilisent pas.
        app.add_plugins(death_ascension::DeathAscensionPlugin);
        app.add_systems(PostStartup, prespawn_hanabi_dummies)
            .add_systems(Startup, weapon_vfx::setup_weapon_vfx)
            .add_systems(Startup, weapon_vfx::tracer::setup_tracer_resources)
            .add_plugins(arena_feedback::ArenaFeedbackPlugin)
            .add_systems(
                Update,
                (
                    emissive_fade_tick,
                    lifetime_tick,
                    // LOT B (audit fire-path 2026-07-20) — decay des lumières des
                    // pools persistants (muzzle/impact/kill), sans despawn.
                    tick_pooled_light_fade,
                    weapon_vfx::tracer::tick_bullets_in_flight,
                    // Story-652 — tuning VFX hot-reload (rebuild assets) + capteur.
                    weapon_vfx::sys_hot_reload_vfx_genome,
                    weapon_vfx::sys_write_weapon_vfx_sensor,
                )
                    .in_set(GameSet::Effects),
            );
    }
}

/// LOT B (audit fire-path 2026-07-20) — decay d'intensité SANS despawn, pour
/// les `PointLight` des pools persistants muzzle/impact/kill (weapon_vfx). À
/// la différence de `Lifetime`/`lifetime_tick` (qui despawn), l'entité reste
/// vivante et attend la prochaine réutilisation par le round-robin. Réinsérer
/// ce composant (via `Commands::insert`) au prochain événement relance le decay.
#[derive(Component)]
pub(crate) struct PooledLightFade {
    pub timer: Timer,
    pub base_intensity: f32,
}

/// Tick `PooledLightFade` — decay l'intensité vers 0 sans jamais despawn.
fn tick_pooled_light_fade(time: Res<Time>, mut q: Query<(&mut PointLight, &mut PooledLightFade)>) {
    for (mut light, mut fade) in &mut q {
        if fade.timer.is_finished() {
            continue; // déjà éteinte — attend la prochaine réutilisation du slot
        }
        fade.timer.tick(time.delta());
        let k = (1.0 - fade.timer.fraction()).max(0.0);
        light.intensity = fade.base_intensity * k;
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

/// Pré-spawn 1 dummy `ParticleEffect` caché par EffectAsset restant à Y=-10000 :
/// hanabi compile ses shaders à la première préparation d'un effet — sans ce
/// warmup, le PREMIER tir de la session paie la compile (freeze 25 s confirmé
/// V1, story-436). Les dummies vivent 5 s puis `lifetime_tick` les despawn.
/// Était un placeholder « TODO Phase 2 » depuis Phase 0 — implémenté story-594
/// (audit 2026-06-10 P1, M2-B5).
///
/// LOT B (audit fire-path 2026-07-20) : les 8 handles muzzle (×5) + impact
/// sparks/dust + kill_burst n'ont PLUS besoin de dummy ici — leurs pools
/// persistants (`weapon_vfx::spawn_muzzle_pool/spawn_impact_pool/spawn_kill_pool`,
/// spawnés dans `setup_weapon_vfx`) émettent déjà UNE fois, cachés à
/// Y=-10000, ce qui paie le même warmup shader. Seuls les 4 handles SANS pool
/// dédié (impact_flash désactivé + les 3 status DoT continus) restent ici.
fn prespawn_hanabi_dummies(mut commands: Commands, effects: Res<weapon_vfx::WeaponVfxEffects>) {
    // Story-647 : chaque dummy binde aussi sa texture (EffectMaterial) — un effet
    // à slot texture sans material ne prépare pas son bind group, le warmup
    // shader serait incomplet.
    let handles = [
        (&effects.impact_flash, &effects.tex_flare),
        (&effects.status_flame, &effects.tex_flame),
        (&effects.status_poison_cloud, &effects.tex_poison),
        (&effects.status_shock, &effects.tex_spark),
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
    info!(
        "[forgia-effects] prespawn 4 hanabi dummies (shader warmup status/impact_flash, story-594/652/653) + 38 entités de pool persistant LOT B (muzzle/impact/kill, déjà warmées à leur spawn)"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_constructible() {
        let _p = ForgiaEffectsPlugin;
    }
}
