//! Couche **attach** — équivalent `CBaseViewModel::SetWeaponModel` du Source SDK
//! (cf `developer.valvesoftware.com/wiki/Viewmodel`). Spawn / despawn / swap du
//! viewmodel attaché à la caméra FPS.
//!
//! Pipeline :
//! 1. `attach_viewmodel_to_camera` : spawn enfant de `FpsCamera` si absent.
//! 2. `update_viewmodel_on_switch` : swap `SceneRoot` quand `EquippedWeapons.current` change.
//! 3. `auto_scale_viewmodel` : mesure AABB BFS descendants, applique scale = target/max_extent.
//! 4. `ensure_camera_shake_component` : insert `CameraShake` sur `FpsCamera` (1×).
//! 5. `despawn_viewmodel` : sur OnExit(Fps), retire le viewmodel pour mode RPG.

use bevy::prelude::*;
use forgia_combat::weapons::{EquippedWeapons, WeaponType};
use forgia_core::prelude::GameMode;
use forgia_genome_core::Genome;
use forgia_juice_camera_shake::{CameraShake, CameraShakeTuning};
use forgia_player::prelude::FpsCamera;

use crate::calibration::{viewmodel_fallback_scale, viewmodel_target_size, viewmodel_transform};
use crate::genome::{lookup_genome_entry, ViewmodelGenome, ViewmodelGenomeHandle};

/// Viewmodel 1P enfant de FpsCamera. Stocke l'arme actuellement rendue
/// pour détecter les changements et swap le SceneRoot.
#[derive(Component)]
pub struct WeaponViewmodel {
    pub current: WeaponType,
}

/// Scene handles pré-chargés pour les 4 armes V1 Arena (load_weapon_models au Startup).
#[derive(Resource)]
pub struct WeaponModelAssets {
    pub pepin: Handle<Scene>,
    pub bourrasque: Handle<Scene>,
    pub madame_lenoir: Handle<Scene>,
    pub boucherie: Handle<Scene>,
}

/// Marker : viewmodel attend la mesure AABB pour calculer son scale réel.
/// Pattern porté de V1 `combat/viewmodel.rs` (auto_scale_system).
#[derive(Component)]
pub struct NeedsAutoScale {
    pub target_size: f32,
}

/// Scale "de base" du viewmodel après auto-calibration AABB (hipfire).
/// Lu par `pose::apply_ads_viewmodel` pour lerp scale en ADS sans drift par frame.
#[derive(Component, Debug, Clone, Copy)]
pub struct ViewmodelBaseScale(pub f32);

fn scene_for_weapon(a: &WeaponModelAssets, w: WeaponType) -> Handle<Scene> {
    match w {
        WeaponType::ModernAR => a.pepin.clone(),
        WeaponType::AssaultRifle => a.bourrasque.clone(),
        WeaponType::Shotgun => a.madame_lenoir.clone(),
        WeaponType::RocketLauncher => a.boucherie.clone(),
        _ => a.pepin.clone(),
    }
}

/// Startup : pré-charge les 4 GLB viewmodel.
pub fn load_weapon_models(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(WeaponModelAssets {
        pepin: asset_server.load("models/weapons/forgia/pepin.glb#Scene0"),
        bourrasque: asset_server.load("models/weapons/forgia/bourrasque.glb#Scene0"),
        madame_lenoir: asset_server.load("models/weapons/forgia/madame_lenoir.glb#Scene0"),
        boucherie: asset_server.load("models/weapons/forgia/boucherie.glb#Scene0"),
    });
}

/// Attache un viewmodel enfant de FpsCamera s'il n'en a pas encore.
pub fn attach_viewmodel_to_camera(
    mut commands: Commands,
    q_cam: Query<(Entity, Option<&Children>), With<FpsCamera>>,
    q_viewmodel: Query<&WeaponViewmodel>,
    assets: Option<Res<WeaponModelAssets>>,
    equipped: Res<EquippedWeapons>,
    genome_handle: Option<Res<ViewmodelGenomeHandle>>,
    genome_assets: Res<Assets<Genome<ViewmodelGenome>>>,
) {
    let Some(assets) = assets else { return };
    for (cam, children) in &q_cam {
        let has_vm = children
            .map(|c| c.iter().any(|child| q_viewmodel.get(child).is_ok()))
            .unwrap_or(false);
        if has_vm {
            continue;
        }
        let entry = genome_handle
            .as_deref()
            .and_then(|h| lookup_genome_entry(&genome_assets, h, equipped.current));
        let scene = scene_for_weapon(&assets, equipped.current);
        let tf = viewmodel_transform(equipped.current, entry);
        let target = viewmodel_target_size(equipped.current, entry);
        // Hidden tant qu'AABB pas mesurée pour ne pas voir 1 frame d'arme géante.
        let vm = commands
            .spawn((
                WeaponViewmodel {
                    current: equipped.current,
                },
                SceneRoot(scene),
                tf,
                Visibility::Hidden,
                NeedsAutoScale {
                    target_size: target,
                },
                Name::new("WeaponViewmodel"),
            ))
            .id();
        commands.entity(cam).add_child(vm);
        info!(
            "[forgia-viewmodel] viewmodel spawned ({:?}, target {:.2}m, awaiting AABB)",
            equipped.current, target
        );
    }
}

/// Swap SceneRoot du viewmodel quand EquippedWeapons.current change.
pub fn update_viewmodel_on_switch(
    mut commands: Commands,
    assets: Option<Res<WeaponModelAssets>>,
    equipped: Res<EquippedWeapons>,
    genome_handle: Option<Res<ViewmodelGenomeHandle>>,
    genome_assets: Res<Assets<Genome<ViewmodelGenome>>>,
    mut q: Query<(
        Entity,
        &mut SceneRoot,
        &mut Transform,
        &mut Visibility,
        &mut WeaponViewmodel,
    )>,
) {
    if !equipped.is_changed() {
        return;
    }
    let Some(assets) = assets else { return };
    let entry = genome_handle
        .as_deref()
        .and_then(|h| lookup_genome_entry(&genome_assets, h, equipped.current));
    for (entity, mut scene, mut tf, mut vis, mut vm) in &mut q {
        if vm.current == equipped.current {
            continue;
        }
        scene.0 = scene_for_weapon(&assets, equipped.current);
        *tf = viewmodel_transform(equipped.current, entry);
        *vis = Visibility::Hidden;
        commands.entity(entity).insert(NeedsAutoScale {
            target_size: viewmodel_target_size(equipped.current, entry),
        });
        vm.current = equipped.current;
    }
}

/// Auto-scale BFS pattern V1 (`combat/viewmodel.rs` auto_scale_system).
/// Combine les Aabb de tous les descendants Mesh3d, calcule scale = target / max_extent,
/// puis applique au Transform + retire NeedsAutoScale + révèle (Visibility::Inherited).
pub fn auto_scale_viewmodel(
    mut commands: Commands,
    q_needs: Query<(Entity, &NeedsAutoScale, &Transform, &WeaponViewmodel)>,
    q_children: Query<&Children>,
    genome_handle: Option<Res<ViewmodelGenomeHandle>>,
    genome_assets: Res<Assets<Genome<ViewmodelGenome>>>,
    q_aabb: Query<&bevy::camera::primitives::Aabb>,
) {
    for (entity, auto, tf, vm) in q_needs.iter() {
        let mut g_min = Vec3::splat(f32::MAX);
        let mut g_max = Vec3::splat(f32::MIN);
        let mut found = false;

        let mut stack = vec![entity];
        while let Some(e) = stack.pop() {
            if let Ok(aabb) = q_aabb.get(e) {
                let c: Vec3 = aabb.center.into();
                let h: Vec3 = aabb.half_extents.into();
                g_min = g_min.min(c - h);
                g_max = g_max.max(c + h);
                found = true;
            }
            if let Ok(children) = q_children.get(e) {
                for child in children.iter() {
                    stack.push(child);
                }
            }
        }

        if !found {
            // Scene encore en load — Aabb pas calculé. Retry next frame.
            continue;
        }

        let size = g_max - g_min;
        let max_extent = size.x.max(size.y).max(size.z);
        if max_extent < 0.001 {
            continue;
        }
        // AABB corrompu (e.g. i16 quantization) : utiliser fallback scale.
        let new_scale = if max_extent > 100.0 {
            let entry = genome_handle
                .as_deref()
                .and_then(|h| lookup_genome_entry(&genome_assets, h, vm.current));
            let fallback = viewmodel_fallback_scale(vm.current, entry);
            warn!(
                "[forgia-viewmodel] AABB CORROMPU ({:.0}m) {:?} → fallback scale {:.4}",
                max_extent, vm.current, fallback
            );
            fallback
        } else {
            auto.target_size / max_extent
        };
        info!(
            "[forgia-viewmodel] AABB ({:.2},{:.2},{:.2}) max {:.2}m → scale {:.4}",
            size.x, size.y, size.z, max_extent, new_scale
        );
        commands
            .entity(entity)
            .remove::<NeedsAutoScale>()
            .insert(Transform {
                scale: Vec3::splat(new_scale),
                ..*tf
            })
            .insert(ViewmodelBaseScale(new_scale))
            .insert(Visibility::Inherited);
    }
}

/// Insert `CameraShake` Component sur FpsCamera si pas déjà présent.
/// Single-shot effectif (Without<CameraShake>). Évite ajouter dep forgia-juice-camera-shake
/// dans forgia-player (upstream).
pub fn ensure_camera_shake_component(
    mut commands: Commands,
    q: Query<Entity, (With<FpsCamera>, Without<CameraShake>)>,
    cs_tuning: Res<CameraShakeTuning>,
) {
    for e in &q {
        commands.entity(e).insert(CameraShake {
            decay: cs_tuning.default_decay,
            max_rotation: cs_tuning.default_max_rotation,
            ..CameraShake::default()
        });
        info!(
            "[forgia-viewmodel] CameraShake attaché (decay={:.1}, max_rot={:.4})",
            cs_tuning.default_decay, cs_tuning.default_max_rotation
        );
    }
}

/// OnExit(Fps) : despawn le viewmodel pour ne pas l'avoir en mode RPG.
pub fn despawn_viewmodel(mut commands: Commands, q: Query<Entity, With<WeaponViewmodel>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

/// Plugin attache : Startup load assets, Update attach/switch/auto-scale/shake,
/// OnExit(Fps) cleanup.
pub struct ForgiaViewmodelAttachPlugin;

impl Plugin for ForgiaViewmodelAttachPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_weapon_models)
            .add_systems(OnExit(GameMode::Fps), despawn_viewmodel)
            .add_systems(
                Update,
                (
                    attach_viewmodel_to_camera,
                    update_viewmodel_on_switch,
                    auto_scale_viewmodel,
                    ensure_camera_shake_component,
                )
                    .run_if(in_state(GameMode::Fps).or(in_state(GameMode::Roguelite))),
            );
    }
}
