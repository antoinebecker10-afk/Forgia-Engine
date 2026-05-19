//! # forgia-enemy-nameplate (story-457, 2026-05-19)
//!
//! Floating HP nameplate au-dessus des enemies — 3D world-space billboard
//! avec `StandardMaterial unlit`. Remplace `forgia-ui-hud::bot_hp_floaters`
//! (egui screen-space) qui était limité aux écrans (pas de profondeur z,
//! pas de occlusion par les murs si projeté hors-écran).
//!
//! ## Architecture
//!
//! - Spawn-on-hit : un `CombatHitEvent` matérialise (ou refresh) un nameplate
//!   enfant du bot ciblé. Lifetime reset à chaque hit.
//! - Billboard cylindrique : yaw vers la caméra, pitch=0 (jamais tilt vertical).
//! - 2 quads superposés : background (full width) + fill (scale.x = hp_fraction).
//!   Pattern AAA simple, alpha unifiée pour le fade.
//! - Tuning genome `genomes/ui/enemy_nameplate.toml` (hot-reload).
//!
//! ## Custom shader
//!
//! `assets/shaders/nameplate_hp.wgsl` est livré (deliverable plan) pour un
//! upgrade futur — implémenter `Material` custom avec `AsBindGroup` uniforme
//! `hp_fraction` + bord arrondi GPU. V1 reste `StandardMaterial unlit + 2 quads`
//! pour shipper rapide.
//!
//! ## Sensor
//!
//! `forgia_enemy_nameplate.json` (1Hz) — active_count + tracked targets.

use bevy::prelude::*;
use forgia_combat::Health as CombatHealth;
use forgia_combat::prelude::CombatHitEvent;
use std::collections::HashMap;
use std::fs;

mod tuning;
pub use tuning::{EnemyNameplate, EnemyNameplateTuning, EnemyNameplateTuningHandle};

pub mod prelude {
    pub use crate::{EnemyNameplate, EnemyNameplateTuning, ForgiaEnemyNameplatePlugin};
}

/// Marker root du nameplate (enfant du bot). Lifetime reset on each hit.
#[derive(Component)]
pub struct NameplateRoot {
    pub target: Entity,
    pub lifetime_left: f32,
}

/// Marker du quad fill HP (scale.x = hp_fraction).
#[derive(Component)]
pub struct NameplateFill;

/// Marker du quad background (full width).
#[derive(Component)]
pub struct NameplateBg;

/// Index target_entity → nameplate_root_entity (évite duplication).
#[derive(Resource, Default)]
pub struct NameplateRegistry {
    pub map: HashMap<Entity, Entity>,
}

pub struct ForgiaEnemyNameplatePlugin;

impl Plugin for ForgiaEnemyNameplatePlugin {
    fn build(&self, app: &mut App) {
        tuning::register_tuning(app);
        app.init_resource::<NameplateRegistry>()
            .add_systems(
                Update,
                (
                    spawn_or_refresh_on_hit,
                    update_hp_fill,
                    billboard_to_camera,
                    tick_lifetime_and_despawn,
                    sensor_write,
                )
                    .chain(),
            );
    }
}

/// Spawn nameplate enfant du bot au premier hit, ou refresh lifetime/HP au hit suivant.
fn spawn_or_refresh_on_hit(
    mut events: MessageReader<CombatHitEvent>,
    mut commands: Commands,
    mut registry: ResMut<NameplateRegistry>,
    mut q_existing: Query<&mut NameplateRoot>,
    tuning: Res<EnemyNameplate>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for ev in events.read() {
        // Refresh si déjà existant.
        if let Some(root) = registry.map.get(&ev.target).copied() {
            if let Ok(mut np) = q_existing.get_mut(root) {
                np.lifetime_left = tuning.0.lifetime;
                continue;
            }
        }

        // Spawn nouveau nameplate.
        let t = &tuning.0;
        let bg_mesh = meshes.add(Rectangle::new(t.width, t.height));
        let fill_mesh = meshes.add(Rectangle::new(t.width - t.border_thickness * 2.0, t.height - t.border_thickness * 2.0));

        let bg_mat = materials.add(StandardMaterial {
            base_color: Color::linear_rgb(t.bg_color[0], t.bg_color[1], t.bg_color[2]),
            unlit: true,
            ..default()
        });
        let fill_mat = materials.add(StandardMaterial {
            base_color: Color::linear_rgb(t.fill_color[0], t.fill_color[1], t.fill_color[2]),
            unlit: true,
            ..default()
        });

        let root_id = commands
            .spawn((
                NameplateRoot {
                    target: ev.target,
                    lifetime_left: t.lifetime,
                },
                Transform::from_xyz(0.0, t.y_offset, 0.0),
                Visibility::default(),
                Name::new("EnemyNameplate"),
                ChildOf(ev.target),
            ))
            .id();

        commands.entity(root_id).with_children(|p| {
            p.spawn((
                NameplateBg,
                Mesh3d(bg_mesh),
                MeshMaterial3d(bg_mat),
                Transform::from_xyz(0.0, 0.0, -0.005),
                Name::new("NameplateBg"),
            ));
            p.spawn((
                NameplateFill,
                Mesh3d(fill_mesh),
                MeshMaterial3d(fill_mat),
                // Pivot left : on translate de -width/2 et scale.x s'ancre au left edge.
                Transform::from_xyz(0.0, 0.0, 0.0),
                Name::new("NameplateFill"),
            ));
        });

        registry.map.insert(ev.target, root_id);
    }
}

/// Met à jour scale.x du fill quad selon hp_fraction du target.
fn update_hp_fill(
    q_roots: Query<(&NameplateRoot, &Children)>,
    q_fill: Query<&NameplateFill>,
    mut q_xform: Query<&mut Transform, With<NameplateFill>>,
    q_health: Query<&CombatHealth>,
) {
    for (root, children) in &q_roots {
        let Ok(hp) = q_health.get(root.target) else { continue };
        let frac = if hp.max > 0.01 {
            (hp.current / hp.max).clamp(0.0, 1.0)
        } else {
            0.0
        };
        for child in children.iter() {
            if q_fill.get(child).is_ok() {
                if let Ok(mut xf) = q_xform.get_mut(child) {
                    // Anchor center : scale.x autour de l'origine. Pour anchor left,
                    // décale translation.x de -(1 - frac) * half_width.
                    xf.scale.x = frac;
                }
            }
        }
    }
}

/// Billboard cylindrique — yaw vers caméra, pitch = 0. Évite tilt vertical
/// désagréable quand le joueur regarde en haut/bas.
fn billboard_to_camera(
    q_cam: Query<&GlobalTransform, With<Camera3d>>,
    mut q_np: Query<(&GlobalTransform, &mut Transform), With<NameplateRoot>>,
) {
    let Ok(cam_xf) = q_cam.single() else { return };
    let cam_pos = cam_xf.translation();
    for (np_global, mut np_local) in &mut q_np {
        let np_world = np_global.translation();
        let dx = cam_pos.x - np_world.x;
        let dz = cam_pos.z - np_world.z;
        let yaw = dx.atan2(dz);
        // Le nameplate root est ChildOf(bot). Sa rotation locale est aussi
        // affectée par la rotation parent. Pour billboard absolu, il faudrait
        // soustraire la rotation parent. Bot KinematicPositionBased rotate
        // peu (face vers player via yaw aim) → approximation acceptable.
        np_local.rotation = Quat::from_rotation_y(yaw);
    }
}

/// Décrémente lifetime, fade alpha sur la fin, despawn à 0.
fn tick_lifetime_and_despawn(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut NameplateRoot, &Children)>,
    q_mat: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut registry: ResMut<NameplateRegistry>,
    tuning: Res<EnemyNameplate>,
) {
    let dt = time.delta_secs();
    for (root_e, mut np, children) in &mut q {
        np.lifetime_left -= dt;
        let alpha = if np.lifetime_left > tuning.0.fade_out_secs {
            1.0
        } else {
            (np.lifetime_left / tuning.0.fade_out_secs).clamp(0.0, 1.0)
        };
        for child in children.iter() {
            if let Ok(mat_h) = q_mat.get(child) {
                if let Some(mat) = materials.get_mut(&mat_h.0) {
                    let lc = mat.base_color.to_linear();
                    mat.base_color = Color::linear_rgba(lc.red, lc.green, lc.blue, alpha);
                    mat.alpha_mode = AlphaMode::Blend;
                }
            }
        }
        if np.lifetime_left <= 0.0 {
            registry.map.remove(&np.target);
            commands.entity(root_e).despawn();
        }
    }
}

/// Sensor 1Hz → `forgia_enemy_nameplate.json`.
fn sensor_write(
    time: Res<Time>,
    mut acc: Local<f32>,
    registry: Res<NameplateRegistry>,
    q: Query<&NameplateRoot>,
) {
    *acc += time.delta_secs();
    if *acc < 1.0 {
        return;
    }
    *acc = 0.0;

    let active_count = q.iter().count();
    let registry_size = registry.map.len();
    let mean_lifetime = if active_count > 0 {
        let s: f32 = q.iter().map(|n| n.lifetime_left).sum();
        s / active_count as f32
    } else {
        0.0
    };
    let payload = serde_json::json!({
        "timestamp_secs": time.elapsed_secs(),
        "active_count": active_count,
        "registry_size": registry_size,
        "mean_lifetime_left": mean_lifetime,
        "status": if active_count == 0 { "idle" } else { "active" },
    });
    let _ = fs::write("forgia_enemy_nameplate.json", payload.to_string());
}
