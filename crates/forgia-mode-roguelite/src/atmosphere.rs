//! Incrément 4 — atmosphère « Cratère de la Forge » : **brume volcanique**
//! (`DistanceFog` rouge-orangé) + **lumière ambiante chaude et sombre**, toutes
//! deux posées sur la caméra FPS. Gated Roguelite : insérées tant qu'on joue,
//! retirées OnExit (les Components quittent la caméra → les autres modes
//! retrouvent leur rendu normal automatiquement). Embers hanabi = différé (les
//! braseros + l'anneau de feu donnent déjà la lueur ; à ajouter si voulu).
//!
//! API Bevy 0.18.1 vérifiée en source : `DistanceFog { color,
//! directional_light_color, directional_light_exponent, falloff }`,
//! `FogFalloff::Exponential { density }`, et `AmbientLight` = **Component**
//! `#[require(Camera)]` (override de `GlobalAmbientLight`), champs `color,
//! brightness, affects_lightmapped_meshes`.

use crate::ambiances::{Ambiance, AmbiancesConfig, CurrentAmbiance};
use crate::render_quality::RogueliteRenderConfig;
use bevy::prelude::*;
use forgia_core::prelude::*;
use forgia_player::FpsCamera;

// Story-676 — les COULEURS viennent de l'ambiance du round
// (`roguelite_ambiances.toml`). Avant, elles étaient des consts volcaniques
// appliquées aux 4 arènes, Hauts Pâturages compris.
//
// `roguelite_render.toml` garde la main sur la DENSITÉ et la LUMINOSITÉ : ce
// sont des réglages de confort/perf transverses, pas d'identité d'univers. Quand
// il les fournit, ils écrasent ceux de l'ambiance — un joueur qui baisse la brume
// la baisse partout.

/// Halo de la lumière directionnelle perçue à travers la brume. Exposant fixe :
/// c'est la forme du halo, pas sa couleur — l'identité est portée par les RGB.
const FOG_SUN_EXPONENT: f32 = 30.0;

fn ambiance_fog(a: &Ambiance, density: f32) -> DistanceFog {
    DistanceFog {
        color: Color::srgb(a.fog_rgb[0], a.fog_rgb[1], a.fog_rgb[2]),
        directional_light_color: Color::srgb(a.fog_sun_rgb[0], a.fog_sun_rgb[1], a.fog_sun_rgb[2]),
        directional_light_exponent: FOG_SUN_EXPONENT,
        falloff: FogFalloff::Exponential { density },
    }
}

fn ambiance_ambient(a: &Ambiance, brightness: f32) -> AmbientLight {
    AmbientLight {
        color: Color::srgb(a.ambient_rgb[0], a.ambient_rgb[1], a.ambient_rgb[2]),
        brightness,
        ..default()
    }
}

pub struct RogueliteAtmospherePlugin;

impl Plugin for RogueliteAtmospherePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnExit(GameMode::Roguelite), sys_remove_atmosphere)
            .add_systems(
                Update,
                sys_ensure_atmosphere
                    .in_set(GameSet::Effects)
                    .run_if(in_state(GameMode::Roguelite)),
            );
    }
}

/// Pose la brume + l'ambiante chaude sur la caméra FPS tant qu'on est en
/// Roguelite (robuste à un éventuel respawn caméra). N'insère que si absente.
fn sys_ensure_atmosphere(
    mut commands: Commands,
    cfg: Option<Res<RogueliteRenderConfig>>,
    amb_cfg: Option<Res<AmbiancesConfig>>,
    current: Option<Res<CurrentAmbiance>>,
    q_cam: Query<Entity, With<FpsCamera>>,
    q_has_fog: Query<(), (With<FpsCamera>, With<DistanceFog>)>,
) {
    // L'univers du round donne les COULEURS ; `roguelite_render.toml` garde la
    // densité et la luminosité. Génome absent → forge historique, comme avant.
    let fallback = Ambiance::forge_ardente();
    let ambiance: &Ambiance = match (amb_cfg.as_deref(), current.as_deref()) {
        (Some(cfg), Some(cur)) => cfg.ambiance(&cur.id),
        _ => &fallback,
    };
    let (fog_enabled, density, brightness) = match cfg.as_deref() {
        Some(c) => (c.fog_enabled, c.fog_density, c.ambient_brightness),
        None => (true, ambiance.fog_density, ambiance.ambient_brightness),
    };
    // Ré-appliquer aussi quand l'UNIVERS change : sinon le round 2 garderait la
    // brume du round 1 (l'insertion n'a lieu que si le composant est absent).
    let cfg_changed = cfg.as_ref().map(|c| c.is_changed()).unwrap_or(false)
        || current.as_ref().map(|c| c.is_changed()).unwrap_or(false)
        || amb_cfg.as_ref().map(|c| c.is_changed()).unwrap_or(false);
    for cam in &q_cam {
        if !fog_enabled {
            commands
                .entity(cam)
                .remove::<DistanceFog>()
                .remove::<AmbientLight>();
            continue;
        }
        // Insère si absent, OU ré-applique si config changée (hot-reload densité).
        if cfg_changed || q_has_fog.get(cam).is_err() {
            commands.entity(cam).insert((
                ambiance_fog(ambiance, density),
                ambiance_ambient(ambiance, brightness),
            ));
        }
    }
}

/// Retire la brume + l'ambiante de la caméra (retour au rendu normal hors Roguelite).
fn sys_remove_atmosphere(mut commands: Commands, q_cam: Query<Entity, With<FpsCamera>>) {
    for cam in &q_cam {
        commands
            .entity(cam)
            .remove::<DistanceFog>()
            .remove::<AmbientLight>();
    }
}
