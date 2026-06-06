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

use bevy::prelude::*;
use forgia_core::prelude::*;
use forgia_player::FpsCamera;

// ── Réglages (tunables — me demander pour ajuster ; genome possible plus tard) ──
/// Couleur de la brume = ce vers quoi le lointain se fond (fumée volcanique).
const FOG_COLOR: (f32, f32, f32) = (0.30, 0.10, 0.07);
/// Densité exponentielle : ~1/density ≈ distance de demi-brume. 0.008 → ~125 m :
/// le cratère/pics (110-205 m) se voilent, la ville (52-76 m) est légèrement
/// brumeuse, l'aire de combat (0-50 m) reste lisible.
const FOG_DENSITY: f32 = 0.008;
/// Halo chaud de la lumière directionnelle perçue à travers la brume.
const FOG_SUN_COLOR: (f32, f32, f32) = (1.0, 0.55, 0.25);
/// Lumière ambiante chaude volcanique (teinte tout en orange sombre).
const AMBIENT_COLOR: (f32, f32, f32) = (1.0, 0.45, 0.22);
const AMBIENT_BRIGHTNESS: f32 = 300.0;

fn volcanic_fog() -> DistanceFog {
    DistanceFog {
        color: Color::srgb(FOG_COLOR.0, FOG_COLOR.1, FOG_COLOR.2),
        directional_light_color: Color::srgb(FOG_SUN_COLOR.0, FOG_SUN_COLOR.1, FOG_SUN_COLOR.2),
        directional_light_exponent: 30.0,
        falloff: FogFalloff::Exponential {
            density: FOG_DENSITY,
        },
    }
}

fn volcanic_ambient() -> AmbientLight {
    AmbientLight {
        color: Color::srgb(AMBIENT_COLOR.0, AMBIENT_COLOR.1, AMBIENT_COLOR.2),
        brightness: AMBIENT_BRIGHTNESS,
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
    q_cam: Query<Entity, (With<FpsCamera>, Without<DistanceFog>)>,
) {
    for cam in &q_cam {
        commands
            .entity(cam)
            .insert((volcanic_fog(), volcanic_ambient()));
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
