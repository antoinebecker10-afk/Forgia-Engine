//! ADS (Aim Down Sight) — clic droit maintenu = zoom FOV + viewmodel rapproché.
//!
//! Pipeline :
//! 1. `track_right_mouse_state` : MessageReader<MouseButtonInput> → RightMouseState.held
//! 2. `update_ads_progress` : lerp progress vers 1.0 (held) ou 0.0 (relâché) à ads_lerp_speed
//! 3. `apply_ads_camera_fov` : interpole `Projection::Perspective.fov` entre default et ads_fov
//! 4. `apply_ads_viewmodel` : interpole Transform.translation viewmodel entre hipfire et ADS offset
//!
//! Pas de hardcode : tout vient du `ViewmodelGenome` per-weapon.

use bevy::prelude::*;
use bevy::camera::Projection;
use bevy::input::mouse::MouseButtonInput;
use bevy::input::ButtonState;
use forgia_combat::weapons::EquippedWeapons;
use forgia_genome_core::Genome;
use forgia_input::prelude::MouseSensitivityMultiplier;
use forgia_juice_fov_punch::FovPunchState;
use forgia_player::prelude::{FpsCamera, MovementSpeedMultiplier};
use forgia_ui::prelude::CrosshairMode;

use crate::{
    lookup_genome_entry, viewmodel_rotation_ads, viewmodel_rotation_hipfire, viewmodel_transform,
    NeedsAutoScale, ViewmodelGenome, ViewmodelGenomeHandle, WeaponViewmodel,
};

const DEFAULT_FOV_DEG: f32 = 45.0; // Bevy PerspectiveProjection default (~0.785 rad)
const ADS_LERP_SPEED: f32 = 12.0;  // 1/12 sec ≈ 80ms full transition

#[derive(Resource, Default)]
pub struct RightMouseState {
    pub held: bool,
}

#[derive(Resource, Default)]
pub struct AdsState {
    /// 0.0 = hipfire (normal), 1.0 = full ADS. Lerp continu.
    pub progress: f32,
}

/// Update RightMouseState depuis MessageReader (anti-trap egui-consume).
pub fn track_right_mouse_state(
    mut mouse_evs: MessageReader<MouseButtonInput>,
    mut state: ResMut<RightMouseState>,
) {
    for ev in mouse_evs.read() {
        if ev.button == MouseButton::Right {
            state.held = ev.state == ButtonState::Pressed;
        }
    }
}

/// Lerp progress vers cible (1.0 si held, 0.0 sinon) à vitesse fixe.
/// Pousse aussi :
/// - CrosshairMode : pour que forgia-ui switche croix blanche → red dot
/// - MovementSpeedMultiplier : ralentit le déplacement en ADS (style CoD)
pub fn update_ads_progress(
    time: Res<Time>,
    right: Res<RightMouseState>,
    mut ads: ResMut<AdsState>,
    mut crosshair: ResMut<CrosshairMode>,
    mut speed_mul: ResMut<MovementSpeedMultiplier>,
    mut sens_mul: ResMut<MouseSensitivityMultiplier>,
    equipped: Res<EquippedWeapons>,
    genome_handle: Option<Res<ViewmodelGenomeHandle>>,
    genome_assets: Res<Assets<Genome<ViewmodelGenome>>>,
) {
    let target = if right.held { 1.0 } else { 0.0 };
    let delta = ADS_LERP_SPEED * time.delta_secs();
    if (ads.progress - target).abs() < delta {
        ads.progress = target;
    } else if ads.progress < target {
        ads.progress += delta;
    } else {
        ads.progress -= delta;
    }
    crosshair.ads_progress = ads.progress;

    let entry_opt = genome_handle
        .as_deref()
        .and_then(|h| lookup_genome_entry(&genome_assets, h, equipped.current));

    // Lerp vitesse player + sensibilité souris + propagation flag sniper scope fullscreen
    let (ads_speed_f, ads_sens_f, sniper_fullscreen) = match entry_opt {
        Some(e) => (
            e.ads_move_speed_factor,
            e.ads_mouse_sensitivity_factor,
            e.sniper_scope_fullscreen,
        ),
        None => (0.65, 0.7, false),
    };
    speed_mul.0 = 1.0_f32.lerp(ads_speed_f, ads.progress);
    // Sensibilité souris : 1.0 hipfire → ads_mouse_sensitivity_factor en full ADS.
    // Reset à 1.0 garanti quand progress=0 → pas de drift hipfire après ADS.
    sens_mul.factor = 1.0_f32.lerp(ads_sens_f, ads.progress);
    crosshair.sniper_fullscreen = sniper_fullscreen;
}

/// Interpole le FOV camera entre default (45°) et ads_fov_deg du genome.
/// Ajoute par-dessus l'offset FOV punch courant (forgia-juice-fov-punch) — punch décay
/// indépendamment, base ADS recalculée chaque frame, somme appliquée au render.
pub fn apply_ads_camera_fov(
    ads: Res<AdsState>,
    equipped: Res<EquippedWeapons>,
    genome_handle: Option<Res<ViewmodelGenomeHandle>>,
    genome_assets: Res<Assets<Genome<ViewmodelGenome>>>,
    fov_punch: Res<FovPunchState>,
    mut q_cam: Query<&mut Projection, With<FpsCamera>>,
) {
    let ads_fov = genome_handle
        .as_deref()
        .and_then(|h| lookup_genome_entry(&genome_assets, h, equipped.current))
        .map(|e| e.ads_fov_deg)
        .unwrap_or(25.0);
    let base_fov = DEFAULT_FOV_DEG.lerp(ads_fov, ads.progress);
    // Atténue le punch en ADS pour éviter zoom oscillant pendant visée précise.
    let punch_attenuation = 1.0 - ads.progress * 0.7; // -70% punch en full ADS
    let final_fov = base_fov + fov_punch.current_deg * punch_attenuation;
    for mut proj in &mut q_cam {
        if let Projection::Perspective(p) = proj.as_mut() {
            p.fov = final_fov.to_radians();
        }
    }
}

/// Interpole le Transform.translation viewmodel entre hipfire et ADS.
///
/// Si `sight_local_*` non-nul dans le genome → mode **sight-align** (style CoD) :
/// formule `ads_translation = (0, 0, -sight_distance) - rotation × scale × sight_local`
/// → garantit que le point `sight_local` du mesh se projette pile sur l'axe cam à
///   `sight_distance` devant → red dot aligné au viseur de l'arme.
///
/// Sinon → fallback manuel `ads_offset_*` (V1 simple).
pub fn apply_ads_viewmodel(
    ads: Res<AdsState>,
    equipped: Res<EquippedWeapons>,
    genome_handle: Option<Res<ViewmodelGenomeHandle>>,
    genome_assets: Res<Assets<Genome<ViewmodelGenome>>>,
    // Without<NeedsAutoScale> : ne touche pas visibility tant que auto_scale_viewmodel
    // n'a pas mesuré l'AABB et révélé l'arme. Évite race condition d'affichage prématuré.
    mut q_vm: Query<(&mut Transform, &mut Visibility), (With<WeaponViewmodel>, Without<NeedsAutoScale>)>,
) {
    let entry = genome_handle
        .as_deref()
        .and_then(|h| lookup_genome_entry(&genome_assets, h, equipped.current));
    let Some(entry) = entry else { return };

    let hipfire = viewmodel_transform(equipped.current, Some(entry));
    let hipfire_rot = viewmodel_rotation_hipfire(entry);
    let ads_rot = viewmodel_rotation_ads(entry);

    let sight_local = Vec3::new(entry.sight_local_x, entry.sight_local_y, entry.sight_local_z);
    let use_sight_align = sight_local.length_squared() > 0.0001;
    // Sniper scope fullscreen : hide complet viewmodel quand ADS engaged (juste le viseur visible).
    let hide_for_sniper = entry.sniper_scope_fullscreen && ads.progress > 0.5;

    for (mut tf, mut vis) in &mut q_vm {
        let ads_target = if use_sight_align {
            let scale = tf.scale.x;
            let sight_offset = ads_rot * (sight_local * scale); // rotation ADS (droite)
            Vec3::new(0.0, 0.0, -entry.sight_distance) - sight_offset
        } else {
            Vec3::new(entry.ads_offset_x, entry.ads_offset_y, entry.ads_offset_z)
        };
        tf.translation = hipfire.translation.lerp(ads_target, ads.progress);
        // Slerp rotation hipfire (avec tilt) → ADS (droite) selon progress
        tf.rotation = hipfire_rot.slerp(ads_rot, ads.progress);

        // Hide viewmodel en scope sniper fullscreen — sinon Inherited.
        // Query filtre Without<NeedsAutoScale> → on n'écrase pas la phase init.
        *vis = if hide_for_sniper { Visibility::Hidden } else { Visibility::Inherited };
    }
}

pub struct AdsPlugin;

impl Plugin for AdsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RightMouseState>()
            .init_resource::<AdsState>()
            .add_systems(
                Update,
                (
                    track_right_mouse_state,
                    update_ads_progress,
                    apply_ads_camera_fov,
                    apply_ads_viewmodel,
                )
                    .chain()
                    .run_if(in_state(forgia_core::prelude::GameMode::Fps)),
            );
    }
}
