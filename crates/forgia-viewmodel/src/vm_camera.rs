//! vm_camera.rs — **FOV viewmodel séparé** (story-618).
//!
//! Pattern Bevy officiel `first-person-view-model` : une 2e caméra ne rendant que
//! le viewmodel (arme + bras) sur `RenderLayers::layer(1)`, à un FOV fixe (~68°)
//! indépendant du FOV monde. Résout la déformation « gros tubes » au FOV élevé.
//!
//! - `ViewmodelCamera` (forgia-player) : enfant de FpsCamera, `order:1`,
//!   `ClearColorConfig::None`, depth séparé → l'arme dessine TOUJOURS au-dessus
//!   du monde (jamais de clip dans les murs).
//! - Le viewmodel (root + descendants GLB/primitives) est propagé sur layer 1 →
//!   invisible pour la caméra monde (layer 0).
//! - Exclue du toon (roguelite) + du skybox (forgia-player) pour éviter le crash
//!   render-graph documenté + un skybox plein écran.
//! - `enabled=false` (hot-reload) : despawn caméra + retire les RenderLayers →
//!   retour propre au comportement pré-618.

use bevy::camera::visibility::RenderLayers;
use bevy::camera::ClearColorConfig;
use bevy::prelude::*;
use forgia_core::prelude::{GameMode, GameplayHudVisible};
use forgia_player::prelude::{FpsCamera, ViewmodelCamera};

use crate::arms::ViewmodelArms;
use crate::attach::WeaponViewmodel;

/// Layer dédié au viewmodel. La caméra monde (layer 0 par défaut) ne le voit pas.
pub const VIEWMODEL_LAYER: usize = 1;

/// Tuning FOV viewmodel (hot-reload `fps_tuning.toml [viewmodel_fov]`).
#[derive(Resource, Debug, Clone, Copy)]
pub struct ViewmodelFovTuning {
    pub enabled: bool,
    pub fov_deg: f32,
}

impl Default for ViewmodelFovTuning {
    fn default() -> Self {
        Self {
            enabled: true,
            fov_deg: 68.0,
        }
    }
}

/// Spawn / despawn la caméra viewmodel selon `enabled`.
pub fn manage_viewmodel_camera(
    mut commands: Commands,
    tuning: Res<ViewmodelFovTuning>,
    q_fps: Query<Entity, With<FpsCamera>>,
    q_vm_cam: Query<Entity, With<ViewmodelCamera>>,
) {
    let exists = !q_vm_cam.is_empty();
    if tuning.enabled && !exists {
        let Ok(fps) = q_fps.single() else {
            return;
        };
        let cam = commands
            .spawn((
                ViewmodelCamera,
                Camera3d::default(),
                Camera {
                    order: 1,
                    clear_color: ClearColorConfig::None,
                    ..default()
                },
                Projection::from(PerspectiveProjection {
                    fov: tuning.fov_deg.to_radians(),
                    near: 0.01,
                    far: 100.0,
                    ..default()
                }),
                RenderLayers::layer(VIEWMODEL_LAYER),
                Transform::IDENTITY,
                Name::new("ViewmodelCamera"),
            ))
            .id();
        commands.entity(fps).add_child(cam);

        // Lumière dédiée viewmodel (layer 1) : dans Bevy les lumières respectent les
        // RenderLayers → le soleil (layer 0) n'éclaire PAS le viewmodel (layer 1), il
        // restait sombre au crépuscule. Cette directional sur layer 1, enfant de la
        // caméra (direction view-relative), éclaire arme + mains de façon CONSTANTE
        // quelle que soit l'heure du jeu (pattern viewmodel AAA). Pas d'ombres (inutile
        // + coûteux sur un layer isolé). Auto-despawn avec la caméra (enfant).
        let light = commands
            .spawn((
                DirectionalLight {
                    illuminance: 9000.0,
                    shadows_enabled: false,
                    ..default()
                },
                // Vient d'en haut-avant-droite (modelé doux sur le poing + l'arme).
                Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.85, 0.5, 0.0)),
                RenderLayers::layer(VIEWMODEL_LAYER),
                Name::new("ViewmodelLight"),
            ))
            .id();
        commands.entity(cam).add_child(light);
        info!(
            "[forgia-viewmodel] ViewmodelCamera + light spawnées (FOV {:.0}°, layer {VIEWMODEL_LAYER})",
            tuning.fov_deg
        );
    } else if !tuning.enabled && exists {
        for e in &q_vm_cam {
            commands.entity(e).despawn();
        }
        info!("[forgia-viewmodel] ViewmodelCamera despawnée (viewmodel_fov.enabled=false)");
    }
}

/// Sync le FOV de la caméra viewmodel quand le tuning change (hot-reload).
pub fn update_viewmodel_camera_fov(
    tuning: Res<ViewmodelFovTuning>,
    mut q: Query<&mut Projection, With<ViewmodelCamera>>,
) {
    if !tuning.is_changed() {
        return;
    }
    for mut proj in &mut q {
        if let Projection::Perspective(p) = proj.as_mut() {
            p.fov = tuning.fov_deg.to_radians();
        }
    }
}

/// Propage `RenderLayers::layer(1)` sur tout le viewmodel (root + descendants
/// GLB/primitives, spawnés async). Si `enabled=false`, retire les RenderLayers →
/// le viewmodel repasse layer 0 (caméra monde), comportement pré-618.
#[allow(clippy::type_complexity)]
pub fn propagate_viewmodel_layer(
    tuning: Res<ViewmodelFovTuning>,
    roots: Query<Entity, Or<(With<WeaponViewmodel>, With<ViewmodelArms>)>>,
    q_children: Query<&Children>,
    q_layers: Query<&RenderLayers>,
    mut commands: Commands,
) {
    let target = RenderLayers::layer(VIEWMODEL_LAYER);
    for root in &roots {
        let mut stack = vec![root];
        while let Some(e) = stack.pop() {
            if tuning.enabled {
                if q_layers.get(e).map(|l| *l != target).unwrap_or(true) {
                    commands.entity(e).insert(target.clone());
                }
            } else if q_layers.get(e).is_ok() {
                commands.entity(e).remove::<RenderLayers>();
            }
            if let Ok(children) = q_children.get(e) {
                for c in children.iter() {
                    stack.push(c);
                }
            }
        }
    }
}

/// Coupe la caméra viewmodel quand le HUD de gameplay est masqué (ex. Lobby
/// Roguelite, home-hub P2.1) : l'arme + les bras (layer 1) disparaissent, MAIS le
/// monde ET l'aperçu 3D du hub (layer 0, caméra principale) restent visibles.
/// `is_active` set-if-different ; lue depuis `GameplayHudVisible` (forgia-core).
pub fn sync_viewmodel_camera_active(
    hud_visible: Option<Res<GameplayHudVisible>>,
    mut q: Query<&mut Camera, With<ViewmodelCamera>>,
) {
    let want = hud_visible.map(|h| h.0).unwrap_or(true);
    for mut cam in &mut q {
        if cam.is_active != want {
            cam.is_active = want;
        }
    }
}

/// Plugin FOV viewmodel séparé. Gated FPS + Roguelite.
pub struct ForgiaViewmodelCameraPlugin;

impl Plugin for ForgiaViewmodelCameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ViewmodelFovTuning>().add_systems(
            Update,
            (
                manage_viewmodel_camera,
                update_viewmodel_camera_fov,
                propagate_viewmodel_layer,
                sync_viewmodel_camera_active,
            )
                .run_if(in_state(GameMode::Fps).or(in_state(GameMode::Roguelite))),
        );
    }
}
