//! Couche **pose** — Aim Down Sight (ADS), interpolation FOV / transform / rotation
//! viewmodel + propagation de la sensibilité souris et vitesse mouvement en ADS.
//!
//! Pipeline canonique (cf Source SDK `cl_ads*` family, CoD style sight align) :
//! 1. `track_right_mouse_state` : `MessageReader<MouseButtonInput>` → `RightMouseState.held`
//! 2. `update_ads_progress` : lerp progress vers 1.0 (held) ou 0.0 (relâché) à `ads_lerp_speed`
//! 3. `apply_ads_camera_fov` : interpole `Projection::Perspective.fov` entre le FOV
//!    hipfire joueur (`forgia_player::CameraFov`, slider menu ESC) et `ads_fov` (genome)
//! 4. `apply_ads_viewmodel` : interpole Transform.translation viewmodel entre hipfire et ADS offset
//!
//! Pas de hardcode : `ads_fov` vient du `ViewmodelGenome` per-weapon ; lerp speed et
//! atténuation FOV punch viennent de `AdsTuning` (genome `fps_tuning.toml`) ; le FOV
//! hipfire est une préf joueur (`CameraFov`), plus un gène genome (story-615).

use bevy::camera::Projection;
use bevy::input::mouse::{MouseButtonInput, MouseMotion};
use bevy::input::ButtonState;
use bevy::prelude::*;
use forgia_combat::weapons::EquippedWeapons;
use forgia_core::prelude::GameMode;
use forgia_genome_core::Genome;
use forgia_input::prelude::MouseSensitivityMultiplier;
use forgia_juice_lib::fov_punch::FovPunchState;
use forgia_player::prelude::{CameraFov, FpsCamera, MovementSpeedMultiplier, Player};
use forgia_ui::prelude::CrosshairMode;

use crate::attach::{NeedsAutoScale, ViewmodelBaseScale, WeaponViewmodel};
use crate::calibration::{viewmodel_rotation_ads, viewmodel_rotation_hipfire, viewmodel_transform};
use crate::genome::{lookup_genome_entry, ViewmodelGenome, ViewmodelGenomeHandle};

#[derive(Resource, Default)]
pub struct RightMouseState {
    pub held: bool,
}

#[derive(Resource, Default)]
pub struct AdsState {
    /// 0.0 = hipfire (normal), 1.0 = full ADS. Lerp continu.
    pub progress: f32,
}

/// Tuning ADS (lerp speed, atténuation FOV punch en ADS).
/// Synced depuis `fps_tuning.toml` côté `forgia-fps::sync_fps_tuning`.
/// Story-615 : le FOV hipfire n'est plus ici — c'est une préf joueur partagée via
/// `forgia_player::CameraFov` (slider menu ESC), lue par `apply_ads_camera_fov`.
#[derive(Resource, Debug, Clone, Copy)]
pub struct AdsTuning {
    pub lerp_speed: f32,
    pub punch_attenuation: f32,
}

impl Default for AdsTuning {
    fn default() -> Self {
        Self {
            lerp_speed: 12.0,
            punch_attenuation: 0.7,
        }
    }
}

/// Tuning « présence » viewmodel (story-617) : sway (l'arme traîne quand on tourne
/// la souris), bob de marche, respiration idle. Couche ADDITIVE par-dessus la pose
/// ADS. Synced depuis `fps_tuning.toml [viewmodel_motion]` (hot-reload).
#[derive(Resource, Debug, Clone, Copy)]
pub struct ViewmodelMotionTuning {
    /// Offset positionnel (m) par pixel de delta souris.
    pub sway_pos_per_px: f32,
    /// Clamp de l'offset positionnel de sway (m).
    pub sway_pos_max: f32,
    /// Offset rotationnel (deg) par pixel de delta souris.
    pub sway_rot_per_px_deg: f32,
    /// Clamp de l'offset rotationnel de sway (deg).
    pub sway_rot_max_deg: f32,
    /// Vitesse de lissage du sway (1/s) — retour au centre quand la souris s'arrête.
    pub sway_smooth: f32,
    /// Amplitude du bob de marche (m) à pleine vitesse.
    pub bob_pos: f32,
    /// Fréquence du bob (cycles/s) à pleine vitesse.
    pub bob_freq: f32,
    /// Vitesse de déplacement (m/s) = bob plein.
    pub bob_speed_ref: f32,
    /// Amplitude de la respiration idle (m).
    pub idle_amp: f32,
    /// Fréquence de la respiration idle (cycles/s).
    pub idle_freq: f32,
}

impl Default for ViewmodelMotionTuning {
    fn default() -> Self {
        // Valeurs conservatrices (veille : « sway = garder, faible »).
        Self {
            sway_pos_per_px: 0.0009,
            sway_pos_max: 0.03,
            sway_rot_per_px_deg: 0.03,
            sway_rot_max_deg: 2.5,
            sway_smooth: 9.0,
            bob_pos: 0.014,
            bob_freq: 8.0,
            bob_speed_ref: 6.0,
            idle_amp: 0.004,
            idle_freq: 1.1,
        }
    }
}

/// État local lissé du sway/bob (par-frame, pas de Resource).
#[derive(Default)]
pub struct SwayBobState {
    sway: Vec2,
    sway_rot: Vec2,
    bob_phase: f32,
    last_player_pos: Option<Vec3>,
}

/// Offset sway/bob/idle partagé (translation locale + rotation), calculé une fois
/// par frame. Réutilisable par l'arme ET les bras (story-617 inc.2) pour une
/// présence cohérente.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct ViewmodelMotionOffset {
    pub translation: Vec3,
    pub rotation: Quat,
}

/// Story-617 — couche additive sway/bob/idle. Tourne APRÈS `apply_ads_viewmodel`
/// (qui réécrit la base chaque frame) → l'offset s'ajoute sans accumuler. Ne touche
/// QUE le rendu du viewmodel — zéro effet sur la direction de tir (= caméra) ou l'ADS.
pub fn apply_viewmodel_sway_bob(
    time: Res<Time>,
    mut motion: MessageReader<MouseMotion>,
    tuning: Res<ViewmodelMotionTuning>,
    q_player: Query<&Transform, (With<Player>, Without<WeaponViewmodel>)>,
    // Without<NeedsAutoScale> : miroir d'apply_ads_viewmodel — pendant la fenêtre
    // d'auto-scale la base n'est pas réécrite, donc on n'ajoute pas (anti-drift).
    mut q_vm: Query<&mut Transform, (With<WeaponViewmodel>, Without<NeedsAutoScale>)>,
    mut offset_out: ResMut<ViewmodelMotionOffset>,
    mut state: Local<SwayBobState>,
) {
    let dt = time.delta_secs().max(1e-5);

    // ── Sway depuis le delta souris (l'arme traîne à l'opposé du mouvement) ──
    let mut md = Vec2::ZERO;
    for ev in motion.read() {
        md += ev.delta;
    }
    let target_pos =
        (Vec2::new(-md.x, md.y) * tuning.sway_pos_per_px).clamp_length_max(tuning.sway_pos_max);
    let target_rot = (Vec2::new(-md.x, md.y) * tuning.sway_rot_per_px_deg.to_radians())
        .clamp_length_max(tuning.sway_rot_max_deg.to_radians());
    let a = (tuning.sway_smooth * dt).min(1.0);
    state.sway = state.sway.lerp(target_pos, a);
    state.sway_rot = state.sway_rot.lerp(target_rot, a);

    // ── Bob de marche (amplitude ∝ vitesse horizontale, phase figée à l'arrêt) ──
    let speed = if let Ok(ptf) = q_player.single() {
        let p = ptf.translation;
        let s = state
            .last_player_pos
            .map(|lp| {
                let d = p - lp;
                Vec2::new(d.x, d.z).length() / dt
            })
            .unwrap_or(0.0);
        state.last_player_pos = Some(p);
        s
    } else {
        0.0
    };
    let speed_frac = (speed / tuning.bob_speed_ref.max(0.01)).clamp(0.0, 1.0);
    if speed_frac > 0.05 {
        state.bob_phase += tuning.bob_freq * speed_frac * dt * std::f32::consts::TAU;
        // Wrap pour éviter la perte de précision f32 en très longue session.
        state.bob_phase %= std::f32::consts::TAU;
    }
    let bob_y = state.bob_phase.sin() * tuning.bob_pos * speed_frac;
    let bob_x = (state.bob_phase * 0.5).cos() * tuning.bob_pos * 0.5 * speed_frac;

    // ── Respiration idle (toujours active, très subtile) ──
    let idle_y =
        (time.elapsed_secs() * tuning.idle_freq * std::f32::consts::TAU).sin() * tuning.idle_amp;

    let offset_t = Vec3::new(state.sway.x + bob_x, state.sway.y + bob_y + idle_y, 0.0);
    let offset_rot =
        Quat::from_rotation_y(state.sway_rot.x) * Quat::from_rotation_x(state.sway_rot.y);

    // Expose pour les bras (inc.2) + applique à l'arme (additif sur la base ADS).
    offset_out.translation = offset_t;
    offset_out.rotation = offset_rot;
    for mut tf in &mut q_vm {
        tf.translation += offset_t;
        tf.rotation = offset_rot * tf.rotation;
    }
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
/// - MouseSensitivityMultiplier : ralentit la souris en ADS (precision)
#[allow(clippy::too_many_arguments)]
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
    tuning: Res<AdsTuning>,
) {
    let target = if right.held { 1.0 } else { 0.0 };
    let delta = tuning.lerp_speed * time.delta_secs();
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

    let (ads_speed_f, ads_sens_f, sniper_fullscreen) = match entry_opt {
        Some(e) => (
            e.ads_move_speed_factor,
            e.ads_mouse_sensitivity_factor,
            e.sniper_scope_fullscreen,
        ),
        None => (0.65, 0.7, false),
    };
    speed_mul.0 = 1.0_f32.lerp(ads_speed_f, ads.progress);
    // Reset à 1.0 garanti quand progress=0 → pas de drift hipfire après ADS.
    sens_mul.factor = 1.0_f32.lerp(ads_sens_f, ads.progress);
    crosshair.sniper_fullscreen = sniper_fullscreen;
}

/// Interpole le FOV camera entre default et ads_fov_deg du genome.
/// Ajoute par-dessus l'offset FOV punch courant (forgia-juice-fov-punch).
pub fn apply_ads_camera_fov(
    ads: Res<AdsState>,
    equipped: Res<EquippedWeapons>,
    genome_handle: Option<Res<ViewmodelGenomeHandle>>,
    genome_assets: Res<Assets<Genome<ViewmodelGenome>>>,
    fov_punch: Res<FovPunchState>,
    tuning: Res<AdsTuning>,
    // Story-615 — FOV hipfire = préf joueur (slider menu ESC), plus un gène genome.
    camera_fov: Res<CameraFov>,
    mut q_cam: Query<&mut Projection, With<FpsCamera>>,
) {
    let ads_fov = genome_handle
        .as_deref()
        .and_then(|h| lookup_genome_entry(&genome_assets, h, equipped.current))
        .map(|e| e.ads_fov_deg)
        .unwrap_or(25.0);
    let base_fov = camera_fov.hipfire_deg.lerp(ads_fov, ads.progress);
    let punch_attenuation = 1.0 - ads.progress * tuning.punch_attenuation;
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
/// `ads_translation = (0, 0, -sight_distance) - rotation × scale × sight_local`
/// → garantit que le point `sight_local` du mesh se projette pile sur l'axe cam
/// à `sight_distance` devant → red dot aligné au viseur de l'arme.
///
/// Sinon → fallback manuel `ads_offset_*` (V1 simple).
pub fn apply_ads_viewmodel(
    ads: Res<AdsState>,
    equipped: Res<EquippedWeapons>,
    genome_handle: Option<Res<ViewmodelGenomeHandle>>,
    genome_assets: Res<Assets<Genome<ViewmodelGenome>>>,
    // Without<NeedsAutoScale> : on n'écrase pas la phase init `auto_scale_viewmodel`.
    mut q_vm: Query<
        (&mut Transform, &mut Visibility, Option<&ViewmodelBaseScale>),
        (With<WeaponViewmodel>, Without<NeedsAutoScale>),
    >,
) {
    let entry = genome_handle
        .as_deref()
        .and_then(|h| lookup_genome_entry(&genome_assets, h, equipped.current));
    let Some(entry) = entry else { return };

    let hipfire = viewmodel_transform(equipped.current, Some(entry));
    let hipfire_rot = viewmodel_rotation_hipfire(entry);
    let ads_rot = viewmodel_rotation_ads(entry);

    let sight_local = Vec3::new(
        entry.sight_local_x,
        entry.sight_local_y,
        entry.sight_local_z,
    );
    let use_sight_align = sight_local.length_squared() > 0.0001;
    let hide_for_sniper = entry.sniper_scope_fullscreen && ads.progress > 0.5;

    for (mut tf, mut vis, base_scale) in &mut q_vm {
        // ADS scale shrink — base_scale stocké par auto_scale_viewmodel.
        if let Some(bs) = base_scale {
            let scale_mul = 1.0_f32.lerp(entry.ads_scale_factor, ads.progress);
            let new_scale = bs.0 * scale_mul;
            tf.scale = Vec3::splat(new_scale);
        }

        let ads_target = if use_sight_align {
            let scale = tf.scale.x;
            let sight_offset = ads_rot * (sight_local * scale);
            Vec3::new(0.0, 0.0, -entry.sight_distance) - sight_offset
        } else {
            Vec3::new(entry.ads_offset_x, entry.ads_offset_y, entry.ads_offset_z)
        };
        tf.translation = hipfire.translation.lerp(ads_target, ads.progress);
        tf.rotation = hipfire_rot.slerp(ads_rot, ads.progress);

        *vis = if hide_for_sniper {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
}

/// Plugin pose : ADS state + lerp FOV/transform/rotation/speed/sensitivity.
/// Gated `run_if(in_state(GameMode::Fps))` — la pose n'a aucun sens hors FPS.
pub struct ForgiaViewmodelPosePlugin;

impl Plugin for ForgiaViewmodelPosePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RightMouseState>()
            .init_resource::<AdsState>()
            .init_resource::<AdsTuning>()
            .init_resource::<ViewmodelMotionTuning>()
            .init_resource::<ViewmodelMotionOffset>()
            .add_systems(
                Update,
                (
                    track_right_mouse_state,
                    update_ads_progress,
                    apply_ads_camera_fov,
                    apply_ads_viewmodel,
                    // Story-617 — couche présence additive APRÈS la pose ADS.
                    apply_viewmodel_sway_bob,
                )
                    .chain()
                    .run_if(in_state(GameMode::Fps).or(in_state(GameMode::Roguelite))),
            );
    }
}
