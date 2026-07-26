//! # castle_envmap — éclairage par image du Hall de Forgia
//!
//! Forgia n'avait **aucun éclairage par image** : `EnvironmentMapLight` n'était
//! employé nulle part dans le projet. Une surface PBR avec une carte de rugosité
//! et rien à réfléchir tombe sur un reflet plat et uniforme — c'est ce qui donne
//! au sol du Hall son aspect de « pierre mouillée » face à la capture du créateur.
//!
//! ## D'où vient la cubemap
//!
//! Le pack contient ses **32 sondes de réflexion cuites**. Ce sont des mesures de
//! ce que sa pierre réfléchit vraiment : sombre et franchement orangé (rapport
//! rouge/bleu jusqu'à 25). `tools/unity/build_castle_envmap.py` reconnaît les 27
//! sondes d'intérieur — les 5 autres regardent le ciel — et les moyenne en une
//! cubemap, en appliquant le miroir sur X que porte le château.
//!
//! ## Une seule cubemap, pas 31 sondes localisées
//!
//! Ses 31 sondes ont chacune leur volume et sa scène ne dit **pas** quel fichier
//! EXR va avec quelle sonde : l'association vit dans `LightingData.asset`, un
//! binaire Unity. Poser des sondes localisées sur une association devinée serait
//! pire que pas de sonde du tout — un reflet de cour extérieure dans une cave se
//! voit tout de suite. On pose donc une ambiance unique, juste par construction,
//! et les sondes localisées attendront que l'association soit établie.
//!
//! ## Pourquoi `GeneratedEnvironmentMapLight`
//!
//! `EnvironmentMapLight` réclame un couple diffus/spéculaire **préfiltré** ; le
//! générateur, lui, ne veut qu'une cubemap brute et fait le filtrage sur GPU. On
//! livre donc les faces telles que le créateur les a cuites, sans étape de bake
//! intermédiaire de notre côté.
//!
//! Réglages : section `[environment]` de `assets/genomes/castle_hub_lighting.toml`,
//! relue à chaud. Contexte : `docs/audits/audit-2026-07-26-comparaison-interieur-createur.md`.

use bevy::asset::LoadState;
use bevy::image::Image;
use bevy::light::GeneratedEnvironmentMapLight;
use bevy::prelude::*;
use bevy::render::render_resource::{TextureViewDescriptor, TextureViewDimension};
use forgia_core::prelude::*;

use crate::castle_flames::CastleLighting;

/// Cubemap d'ambiance, générée par `tools/unity/build_castle_envmap.py`.
/// Image empilée verticalement : 6 faces de 128, soit 128 × 768.
const ENV_MAP_PATH: &str = "hdri/castle_hub_env.hdr";
const ENV_MAP_FACES: u32 = 6;
const SENSOR_PATH: &str = "forgia2_castle_envmap.json";
const SENSOR_PERIOD_SECS: f32 = 1.0;
/// Délai laissé au chargement du fichier avant de considérer qu'il ne viendra pas.
const LOAD_GRACE_SECS: f32 = 10.0;

#[derive(Resource, Default)]
struct CastleEnvMap {
    handle: Handle<Image>,
    /// L'image a été réinterprétée en cubemap (opération à faire **une** fois).
    prepared: bool,
    /// Le chargement a échoué — inutile de réessayer en boucle.
    failed: bool,
    elapsed_secs: f32,
    /// Nombre de caméras portant l'éclairage par image.
    attached: u32,
}

pub struct CastleEnvMapPlugin;

impl Plugin for CastleEnvMapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CastleEnvMap>()
            .add_systems(OnEnter(GameMode::CastleHub), sys_load_env_map)
            .add_systems(OnExit(GameMode::CastleHub), sys_cleanup_env_map)
            .add_systems(
                Update,
                (sys_prepare_cubemap, sys_apply_env_map)
                    .chain()
                    .run_if(in_state(GameMode::CastleHub)),
            )
            .add_systems(Update, sys_write_sensor.in_set(GameSet::Sensors));
    }
}

fn sys_load_env_map(
    mut env: ResMut<CastleEnvMap>,
    assets: Res<AssetServer>,
    tuning: Res<CastleLighting>,
) {
    *env = CastleEnvMap::default();
    if !tuning.env_enabled {
        return;
    }
    env.handle = assets.load(ENV_MAP_PATH);
}

/// Réinterprète l'image empilée en cubemap, dès qu'elle est chargée.
///
/// Le fichier est une image 2D de 128 × 768 : les six faces sont empilées. Bevy ne
/// devine pas qu'il s'agit d'une cubemap — il faut le lui dire, et une seule fois,
/// sans quoi la seconde réinterprétation échouerait sur une image déjà en tableau.
fn sys_prepare_cubemap(
    mut env: ResMut<CastleEnvMap>,
    mut images: ResMut<Assets<Image>>,
    assets: Res<AssetServer>,
    time: Res<Time<Real>>,
) {
    env.elapsed_secs += time.delta_secs();
    if env.prepared || env.failed || env.handle == Handle::default() {
        return;
    }
    match assets.get_load_state(&env.handle) {
        Some(LoadState::Failed(_)) => {
            env.failed = true;
            warn!("[castle-envmap] {ENV_MAP_PATH} illisible — pas d'éclairage par image");
            return;
        }
        Some(LoadState::Loaded) => {}
        _ => return,
    }
    let Some(image) = images.get_mut(&env.handle) else {
        return;
    };
    if let Err(error) = image.reinterpret_stacked_2d_as_array(ENV_MAP_FACES) {
        env.failed = true;
        warn!("[castle-envmap] {ENV_MAP_PATH} n'est pas une pile de 6 faces ({error:?})");
        return;
    }
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    env.prepared = true;
}

/// Pose l'éclairage par image sur les caméras 3D, et suit le réglage à chaud.
fn sys_apply_env_map(
    mut commands: Commands,
    mut env: ResMut<CastleEnvMap>,
    tuning: Res<CastleLighting>,
    q_cameras: Query<(Entity, Option<&GeneratedEnvironmentMapLight>), With<Camera3d>>,
) {
    if !env.prepared {
        return;
    }
    let mut attached = 0;
    for (entity, existing) in &q_cameras {
        if !tuning.env_enabled {
            if existing.is_some() {
                commands
                    .entity(entity)
                    .remove::<GeneratedEnvironmentMapLight>();
            }
            continue;
        }
        // Rien à faire tant que l'intensité n'a pas bougé : ré-insérer chaque
        // frame relancerait le filtrage GPU de la cubemap.
        if existing.is_some_and(|light| light.intensity == tuning.env_intensity) {
            attached += 1;
            continue;
        }
        commands.entity(entity).insert(GeneratedEnvironmentMapLight {
            environment_map: env.handle.clone(),
            intensity: tuning.env_intensity,
            ..default()
        });
        attached += 1;
    }
    env.attached = attached;
}

fn sys_cleanup_env_map(
    mut commands: Commands,
    mut env: ResMut<CastleEnvMap>,
    q_cameras: Query<Entity, With<GeneratedEnvironmentMapLight>>,
) {
    for entity in &q_cameras {
        commands
            .entity(entity)
            .remove::<GeneratedEnvironmentMapLight>();
    }
    *env = CastleEnvMap::default();
}

// ─── Observabilité ───────────────────────────────────────────────────────────

fn sys_write_sensor(
    time: Res<Time<Real>>,
    mut next_write: Local<f32>,
    game_mode: Res<State<GameMode>>,
    tuning: Res<CastleLighting>,
    env: Res<CastleEnvMap>,
) {
    let now = time.elapsed_secs();
    if now < *next_write {
        return;
    }
    *next_write = now + SENSOR_PERIOD_SECS;

    let in_hub = matches!(game_mode.get(), GameMode::CastleHub);
    let (severity, next_step) = severity_for_env_map(
        in_hub,
        tuning.env_enabled,
        env.prepared,
        env.failed,
        env.attached,
        env.elapsed_secs,
    );
    let json = format!(
        r#"{{"id":"castle_envmap","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{now:.1},"in_hub":{in_hub},"enabled":{},"prepared":{},"failed":{},"cameras":{},"intensity":{:.0}}}"#,
        tuning.env_enabled, env.prepared, env.failed, env.attached, tuning.env_intensity,
    );
    let _ = forgia_core::sensor_io::enqueue(SENSOR_PATH, json);
}

/// Sévérité + action de remédiation. Extraite pour être testable sans app Bevy.
fn severity_for_env_map(
    in_hub: bool,
    enabled: bool,
    prepared: bool,
    failed: bool,
    cameras: u32,
    elapsed_secs: f32,
) -> (&'static str, &'static str) {
    if !in_hub {
        return ("info", "hors du Hall — pas d'éclairage par image");
    }
    if !enabled {
        return ("info", "désactivé dans [environment] du genome");
    }
    if failed {
        return (
            "critical",
            "cubemap illisible : régénérer assets/hdri/castle_hub_env.hdr \
             via tools/unity/build_castle_envmap.py",
        );
    }
    if !prepared {
        // Le chargement de l'image n'est pas instantané.
        if elapsed_secs < LOAD_GRACE_SECS {
            return ("ok", "chargement de la cubemap en cours");
        }
        return (
            "critical",
            "cubemap jamais chargée : vérifier la présence de assets/hdri/castle_hub_env.hdr",
        );
    }
    if cameras == 0 {
        return (
            "warn",
            "aucune caméra 3D ne porte l'éclairage par image — la pierre restera plate",
        );
    }
    ("ok", "aucune action")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Sans le fichier, tout le module est inerte : il doit être versionné.
    #[test]
    fn the_ambience_cubemap_ships_with_the_game() {
        let path = Path::new("../../assets").join(ENV_MAP_PATH);
        assert!(
            path.exists(),
            "cubemap d'ambiance absente : la lancer via tools/unity/build_castle_envmap.py"
        );
    }

    #[test]
    fn a_missing_cubemap_is_reported_as_critical() {
        let (severity, _) = severity_for_env_map(true, true, false, true, 0, 1.0);
        assert_eq!(severity, "critical");
        // Jamais préparée passé le délai : même verdict, autre cause.
        let (severity, _) =
            severity_for_env_map(true, true, false, false, 0, LOAD_GRACE_SECS + 1.0);
        assert_eq!(severity, "critical");
    }

    /// Ne pas crier pendant que l'image charge.
    #[test]
    fn loading_is_not_an_error() {
        let (severity, _) = severity_for_env_map(true, true, false, false, 0, 1.0);
        assert_eq!(severity, "ok");
    }

    /// Le cas qui rendrait le correctif silencieusement inutile : la cubemap est
    /// prête mais aucune caméra ne la porte.
    #[test]
    fn a_prepared_map_on_no_camera_warns() {
        let (severity, _) = severity_for_env_map(true, true, true, false, 0, 30.0);
        assert_eq!(severity, "warn");
        let (severity, _) = severity_for_env_map(true, true, true, false, 1, 30.0);
        assert_eq!(severity, "ok");
    }

    #[test]
    fn disabling_it_is_not_a_failure() {
        let (severity, _) = severity_for_env_map(true, false, false, false, 0, 30.0);
        assert_eq!(severity, "info");
    }
}
