//! # forgia-ui-damage-direction
//!
//! Story-455 Phase E — Damage Direction Indicator (DDI) FPS.
//!
//! Pattern AAA (research §7 Jasper Stephenson UX analysis) :
//! - **2D arcs autour du crosshair** (CoD / Left 4 Dead) — le plus lisible
//! - Multi-attackers : **arcs distincts** (pas cumul Apex stacking, anti-pattern)
//! - Intensity alpha lerp `[0.5, 1.0]` selon `damage / player_max_hp`
//! - Fade ease-out 1.2s
//! - Cap 4 arcs simultanés (genome-driven)
//!
//! Consume `CombatHitEvent` filter `target == player` && `attacker.is_some()` :
//! calcule angle horizontal `atan2(attacker_xz - player_xz)` projeté caméra.
//! Genome `assets/genomes/damage_dir_tuning.toml`. Sensor `forgia_damage_dir.json`.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_combat::prelude::*;
use forgia_core::prelude::*;
use forgia_genome_core::{Genome, GenomeLoader};
use forgia_player::{FpsCamera, Player};
use forgia_ui_style::*;
use std::collections::VecDeque;
use std::fs;

mod tuning;

pub use tuning::DamageDirTuning;

pub mod prelude {
    pub use crate::{DamageDirTuning, ForgiaUiDamageDirectionPlugin};
}

// ─── State ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct DamageArc {
    /// Angle radian dans le plan écran : 0 = devant, π/2 = droite, π = derrière, -π/2 = gauche.
    pub angle_rad: f32,
    /// Intensity lerp alpha basé sur damage / max_hp [0.5, 1.0].
    pub intensity: f32,
    /// Age depuis spawn (secs).
    pub age_secs: f32,
}

/// BUG-455-02 fix : `VecDeque` au lieu de `Vec` — `pop_front()` O(1) vs `Vec::remove(0)` O(n).
/// Cohérence avec `forgia-killfeed::KillFeedState` qui utilise déjà VecDeque.
#[derive(Resource, Default)]
pub struct DamageArcsState {
    pub arcs: VecDeque<DamageArc>,
}

impl DamageArcsState {
    pub fn new() -> Self {
        Self {
            arcs: VecDeque::with_capacity(8),
        }
    }
}

#[derive(Resource, Default)]
pub struct DamageDirSensor {
    pub last_write_secs: f32,
    pub events_received: u32,
    pub last_angle_deg: f32,
}

// ─── Helpers ───────────────────────────────────────────────────────────

/// Convertit la position monde de l'attaquant en angle radian dans le plan caméra.
/// Source : pattern CoD/Apex DDI — projection horizontale XZ + atan2 relatif au
/// forward de la caméra. Y est ignoré (DDI 2D).
///
/// Return: angle ∈ [-π, π] avec 0 = directement devant le joueur,
///         positif = droite, négatif = gauche, ±π = derrière.
fn world_to_screen_angle(
    attacker_world: Vec3,
    player_world: Vec3,
    cam_forward: Vec3,
    cam_right: Vec3,
) -> f32 {
    // Vecteur du joueur vers l'attaquant, projeté XZ (DDI 2D plan horizontal).
    let mut to_attacker = attacker_world - player_world;
    to_attacker.y = 0.0;
    if to_attacker.length_squared() < 1e-6 {
        return 0.0;
    }
    to_attacker = to_attacker.normalize();

    let mut fwd = cam_forward;
    fwd.y = 0.0;
    let fwd_h = if fwd.length_squared() < 1e-6 {
        Vec3::Z * -1.0
    } else {
        fwd.normalize()
    };

    let mut right = cam_right;
    right.y = 0.0;
    let right_h = if right.length_squared() < 1e-6 {
        Vec3::X
    } else {
        right.normalize()
    };

    let dot_fwd = to_attacker.dot(fwd_h).clamp(-1.0, 1.0);
    let dot_right = to_attacker.dot(right_h);
    // atan2(side, forward) : 0 = devant, positif = droite, négatif = gauche, π = derrière
    dot_right.atan2(dot_fwd)
}

// ─── Ingest events ─────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub(crate) fn ingest_damage_for_player(
    mut events: MessageReader<CombatHitEvent>,
    mut arcs: ResMut<DamageArcsState>,
    mut sensor: ResMut<DamageDirSensor>,
    tuning: Res<DamageDirTuning>,
    q_player: Query<(Entity, &Transform), With<Player>>,
    q_cam: Query<&GlobalTransform, With<FpsCamera>>,
    q_health: Query<&Health, With<Player>>,
    q_attacker_tf: Query<&GlobalTransform>,
) {
    let Ok((player_entity, player_tf)) = q_player.single() else {
        return;
    };
    let Ok(cam_tf) = q_cam.single() else { return };
    let max_hp = q_health.single().map(|h| h.max).unwrap_or(100.0);
    let fwd = cam_tf.forward().as_vec3();
    let right = cam_tf.right().as_vec3();

    for hit in events.read() {
        // Filter : seul le player en target déclenche le DDI.
        if hit.target != player_entity {
            continue;
        }
        let Some(attacker) = hit.attacker else {
            continue;
        };
        if attacker == player_entity {
            // Self-damage (fall, splash propre) : skip.
            continue;
        }
        // BUG-455-01 fix : query GlobalTransform de l'attaquant pour position correcte.
        // Fallback sur hit_world_pos uniquement si l'attaquant n'a plus de Transform
        // (despawned au moment du process — rare mais possible).
        let attacker_world = q_attacker_tf
            .get(attacker)
            .map(|tf| tf.translation())
            .unwrap_or(hit.hit_world_pos);
        let angle = world_to_screen_angle(attacker_world, player_tf.translation, fwd, right);
        let intensity = tuning
            .intensity_min
            .lerp(tuning.intensity_max, (hit.damage / max_hp).clamp(0.0, 1.0));
        // BUG-455-02 fix : pop_front() O(1) au lieu de remove(0) O(n).
        if arcs.arcs.len() >= tuning.max_arcs.max(1) {
            arcs.arcs.pop_front();
        }
        arcs.arcs.push_back(DamageArc {
            angle_rad: angle,
            intensity,
            age_secs: 0.0,
        });
        sensor.events_received = sensor.events_received.saturating_add(1);
        sensor.last_angle_deg = angle.to_degrees();
    }
}

// ─── Tick arcs (age + cleanup) ────────────────────────────────────────

pub(crate) fn tick_damage_arcs(
    time: Res<Time>,
    mut arcs: ResMut<DamageArcsState>,
    tuning: Res<DamageDirTuning>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for a in &mut arcs.arcs {
        a.age_secs += dt;
    }
    arcs.arcs.retain(|a| a.age_secs < tuning.duration_secs);
}

// ─── Draw arcs ─────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_damage_arcs(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    arcs: Res<DamageArcsState>,
    tuning: Res<DamageDirTuning>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Fps {
        return;
    }
    if arcs.arcs.is_empty() {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("forgia_damage_dir"),
    ));
    let center = screen.center();
    let radius = tuning.radius_px;
    let arc_span = tuning.arc_span_deg.to_radians();

    for arc in &arcs.arcs {
        // Fade ease-out : alpha = intensity × (1 - t)² avec t = age/duration
        let t = (arc.age_secs / tuning.duration_secs).clamp(0.0, 1.0);
        let fade = (1.0 - t).powi(2);
        let alpha = (arc.intensity * fade).clamp(0.0, 1.0);
        if alpha < 0.01 {
            continue;
        }
        let alpha_u8 = (alpha * 255.0) as u8;
        let color = egui::Color32::from_rgba_unmultiplied(
            C_AMMO_LOW.r(),
            C_AMMO_LOW.g(),
            C_AMMO_LOW.b(),
            alpha_u8,
        );
        // Convention angle_rad : 0 = devant (top de l'écran en repère egui), positif = droite.
        // Egui paint coord : (0,0) top-left, x→ droite, y→ bas. "Devant" = au-dessus du crosshair = -y.
        // Donc on transforme angle_rad → angle_screen avec rotation -π/2 (devant = -y).
        let theta_screen = arc.angle_rad - std::f32::consts::FRAC_PI_2;
        let start = theta_screen - arc_span * 0.5;
        // Arc filled : on dessine multiple segments épaisseur tuning.thickness.
        arc_stroke(
            &painter,
            center,
            radius,
            start,
            arc_span,
            1.0, // progress complet (l'arc entier est dessiné)
            color,
            tuning.thickness,
            tuning.segments,
        );
    }
}

// ─── Sensor JSON ───────────────────────────────────────────────────────

pub(crate) fn write_damage_dir_sensor(
    time: Res<Time>,
    tuning: Res<DamageDirTuning>,
    arcs: Res<DamageArcsState>,
    mut sensor: ResMut<DamageDirSensor>,
) {
    let now = time.elapsed_secs();
    if now - sensor.last_write_secs < tuning.sensor_period_secs.max(0.1) {
        return;
    }
    sensor.last_write_secs = now;
    let mut arcs_json = String::with_capacity(arcs.arcs.len() * 80);
    let mut first = true;
    for a in &arcs.arcs {
        if !first {
            arcs_json.push(',');
        }
        first = false;
        arcs_json.push_str(&format!(
            r#"{{"angle_deg":{:.1},"intensity":{:.3},"age":{:.2}}}"#,
            a.angle_rad.to_degrees(),
            a.intensity,
            a.age_secs,
        ));
    }
    let json = format!(
        r#"{{"timestamp_secs":{:.2},"active_arcs":{},"max_arcs":{},"events_received":{},"last_angle_deg":{:.1},"arcs":[{}]}}"#,
        now,
        arcs.arcs.len(),
        tuning.max_arcs,
        sensor.events_received,
        sensor.last_angle_deg,
        arcs_json,
    );
    let _ = fs::write("forgia_damage_dir.json", json);
}

// ─── Plugin ────────────────────────────────────────────────────────────

pub struct ForgiaUiDamageDirectionPlugin;

impl Plugin for ForgiaUiDamageDirectionPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DamageArcsState::new())
            .init_resource::<DamageDirSensor>()
            .init_resource::<DamageDirTuning>()
            .init_asset::<Genome<DamageDirTuning>>()
            .register_asset_loader(GenomeLoader::<DamageDirTuning>::default())
            .add_systems(Startup, tuning::load_damage_dir_tuning)
            .add_systems(
                Update,
                (
                    tuning::sync_damage_dir_tuning,
                    ingest_damage_for_player,
                    tick_damage_arcs,
                    write_damage_dir_sensor,
                )
                    .chain(),
            )
            .add_systems(EguiPrimaryContextPass, draw_damage_arcs);
    }
}
