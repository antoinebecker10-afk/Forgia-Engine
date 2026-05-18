//! # forgia-camera-orbit
//!
//! Orbit camera 3P (RPG mode) qui suit une entité cible.
//!
//! - Yaw : récupéré depuis la rotation Y du target (réutilise `Player.yaw` de forgia-player,
//!   pas de double input handling).
//! - Pitch : géré par mouse Y motion (interne à OrbitCamera).
//! - Distance : mouse wheel zoom in/out, clampée.
//! - Position camera = target_pos + arrière(target.yaw, pitch) * distance + height_offset.
//!
//! ## Usage
//!
//! ```ignore
//! commands.spawn((
//!     Camera3d::default(),
//!     OrbitCamera::new(player_entity),
//!     Transform::default(),
//! ));
//! ```

use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::input::ButtonInput;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use forgia_core::prelude::*;

pub mod prelude {
    pub use crate::{ForgiaCameraOrbitPlugin, OrbitCamera};
}

/// Composant à attacher à un `Camera3d` pour en faire une orbit cam 3P.
#[derive(Component)]
pub struct OrbitCamera {
    pub target: Entity,
    /// Distance camera ↔ target (m). Mouse wheel scrollable.
    pub distance: f32,
    pub min_distance: f32,
    pub max_distance: f32,
    /// Pitch local en radians, mouse-driven.
    pub pitch: f32,
    pub min_pitch: f32,
    pub max_pitch: f32,
    /// Hauteur d'épaule (m) — caméra vise target + height_offset, pas target.center.
    pub height_offset: f32,
    /// Sensibilité mouse Y → pitch.
    pub pitch_sensitivity: f32,
    /// Sensibilité mouse wheel → distance.
    pub zoom_sensitivity: f32,
    /// Yaw additionnel manuel (rad), contrôlé par LMB drag (mouse X). S'ajoute
    /// au yaw du target → permet à l'utilisateur de tourner autour du perso
    /// sans tourner le perso lui-même. Wraps automatiquement.
    pub yaw_offset: f32,
    /// Sensibilité mouse X → yaw_offset quand LMB held.
    pub yaw_sensitivity: f32,
}

impl OrbitCamera {
    /// Defaults RPG cinématique style Forgia V1 / Witcher / GTA :
    /// 7m back, 1.8m hauteur épaule, pitch piqué pour cadrer le character entier.
    pub fn new(target: Entity) -> Self {
        Self {
            target,
            distance: 7.0,
            min_distance: 3.0,
            max_distance: 18.0,
            pitch: -0.30, // ~17° vers le bas
            min_pitch: -1.2,
            max_pitch: 0.6,
            height_offset: 1.8,
            pitch_sensitivity: 0.002,
            zoom_sensitivity: 0.5,
            yaw_offset: 0.0,
            yaw_sensitivity: 0.005,
        }
    }
}

pub struct ForgiaCameraOrbitPlugin;

impl Plugin for ForgiaCameraOrbitPlugin {
    fn build(&self, app: &mut App) {
        // BUG-ANIMQA-06 fix : in_set(GameSet::Camera) pour ordering canonique V2
        app.add_systems(
            Update,
            (
                orbit_input,
                orbit_auto_recenter_on_move,
                orbit_follow,
                orbit_cursor_grab,
            )
                .chain()
                .in_set(GameSet::Camera)
                .run_if(any_with_component::<OrbitCamera>),
        );
    }
}

/// WoW pattern : cursor caché tant qu'un bouton souris (LMB ou RMB) est tenu,
/// restauré au release. Hold-to-grab, pas toggle (cf research §1.4).
///
/// `CursorOptions` est un Component séparé de Window depuis Bevy 0.16
/// (PR #19668) — query directe sur PrimaryWindow.
fn orbit_cursor_grab(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut q: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    let Ok(mut cursor) = q.single_mut() else {
        return;
    };
    let any_held = mouse_buttons.pressed(MouseButton::Left)
        || mouse_buttons.pressed(MouseButton::Right);
    let desired_grab = if any_held {
        CursorGrabMode::Locked
    } else {
        CursorGrabMode::None
    };
    let desired_visible = !any_held;
    if cursor.grab_mode != desired_grab {
        cursor.grab_mode = desired_grab;
    }
    if cursor.visible != desired_visible {
        cursor.visible = desired_visible;
    }
}

/// **WoW camera pattern** (sources Blizzard/wowpedia §research) :
///
/// - **LMB held + mouse X** : `yaw_offset` orbite la caméra autour du player.
///   Player yaw inchangé. "Look mode".
/// - **RMB held + mouse X** : `yaw_offset` reset à 0 (la caméra suit le player
///   directement). Mouse X consumé par `forgia-player` qui tourne le player
///   yaw. "Mouselook" — caméra et perso steer ensemble.
/// - **Pitch** : actif si LMB OU RMB tenu (gate hold-to-look WoW).
/// - **Mouse wheel** : zoom in/out toujours actif (même sans bouton tenu).
fn orbit_input(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut motion: MessageReader<MouseMotion>,
    mut wheel: MessageReader<MouseWheel>,
    mut q: Query<&mut OrbitCamera>,
) {
    let lmb_held = mouse_buttons.pressed(MouseButton::Left);
    let rmb_held = mouse_buttons.pressed(MouseButton::Right);
    let any_held = lmb_held || rmb_held;

    let mut delta_x = 0.0;
    let mut delta_y = 0.0;
    for ev in motion.read() {
        delta_x += ev.delta.x;
        delta_y += ev.delta.y;
    }
    let mut delta_wheel = 0.0;
    for ev in wheel.read() {
        delta_wheel += ev.y;
    }
    if delta_x.abs() < f32::EPSILON
        && delta_y.abs() < f32::EPSILON
        && delta_wheel.abs() < f32::EPSILON
    {
        return;
    }
    for mut cam in &mut q {
        // Pitch : seulement si un bouton souris est tenu (pattern WoW).
        if any_held && delta_y.abs() > f32::EPSILON {
            cam.pitch = (cam.pitch - delta_y * cam.pitch_sensitivity)
                .clamp(cam.min_pitch, cam.max_pitch);
        }
        // RMB tenu : la cam suit le yaw du player → reset yaw_offset à 0
        // smooth-lerp pour éviter snap brutal au passage LMB → RMB.
        if rmb_held {
            cam.yaw_offset *= 0.85;
            if cam.yaw_offset.abs() < 0.001 {
                cam.yaw_offset = 0.0;
            }
        } else if lmb_held && delta_x.abs() > f32::EPSILON {
            // LMB only : orbit cam-only (player yaw inchangé).
            // ⚠️ Pas de rem_euclid (range [0, TAU]) — sinon le shortest-path
            // decay de l'auto-recenter ne peut pas raccourcir un offset > π en
            // partant via -π. Laisser flotter, cos/sin wrap naturellement.
            cam.yaw_offset -= delta_x * cam.yaw_sensitivity;
        }
        if delta_wheel.abs() > f32::EPSILON {
            cam.distance = (cam.distance - delta_wheel * cam.zoom_sensitivity)
                .clamp(cam.min_distance, cam.max_distance);
        }
    }
}

/// WoW pattern — auto-recenter caméra derrière le perso quand il se déplace
/// ET qu'aucun bouton souris n'est tenu. Decay smooth de `yaw_offset` vers 0
/// (~0.5s à 60fps). Si l'utilisateur tient LMB/RMB, son intent override (pas
/// d'auto-recenter). Pattern Blizzard wowpedia `cameraDistanceMoveSpeed`.
/// Durée totale d'une transition de recenter (ease-out quad).
/// 1.2s = WoW cam follow snappy. Ease-out = vitesse rapide visible au début
/// puis ralentissement doux à l'arrivée (ressort sous-amorti, pattern AAA).
/// Tuning user 2026-05-18 : 2.0s trop lent → 1.2s.
const RECENTER_DURATION_SEC: f32 = 1.2;

/// État interne du recenter (tracked entre frames). Reset au touch souris ou
/// quand la transition est terminée. `initial_yaw` est l'offset au moment où
/// le user a commencé à bouger (snapshot).
#[derive(Default)]
struct RecenterState {
    initial_yaw: f32,
    elapsed: f32,
    active: bool,
}

fn orbit_auto_recenter_on_move(
    time: Res<Time>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut q_cam: Query<&mut OrbitCamera>,
    targets: Query<&GlobalTransform, Without<OrbitCamera>>,
    mut last_pos: Local<Option<Vec3>>,
    mut state: Local<RecenterState>,
) {
    // Si user tient un bouton, il steer la cam — annule la transition courante
    // et save la position pour calculer le delta au release suivant.
    let any_held = mouse_buttons.pressed(MouseButton::Left)
        || mouse_buttons.pressed(MouseButton::Right);
    if any_held {
        state.active = false;
        if let Some(orbit) = q_cam.iter().next() {
            if let Ok(t) = targets.get(orbit.target) {
                *last_pos = Some(t.translation());
            }
        }
        return;
    }
    for mut orbit in &mut q_cam {
        if orbit.yaw_offset.abs() < 0.001 {
            orbit.yaw_offset = 0.0;
            state.active = false;
            continue;
        }
        let Ok(target_gt) = targets.get(orbit.target) else { continue };
        let pos = target_gt.translation();
        let moved = match *last_pos {
            // Seuil 1e-4 m² (~10mm) : ignore les micro-jitters de la physique.
            Some(prev) => (pos - prev).length_squared() > 1.0e-4,
            None => false,
        };
        *last_pos = Some(pos);

        // Shortest-path normalize sur le yaw_offset COURANT pour qu'à chaque
        // (re)trigger on parte du chemin le plus court vers 0.
        use std::f32::consts::{PI, TAU};
        let mut a = orbit.yaw_offset.rem_euclid(TAU);
        if a > PI {
            a -= TAU;
        }

        // Trigger : la transition démarre au premier mouvement. Une fois
        // active, elle continue à recentrer MÊME SI le player s'arrête (pas
        // de pause-on-stop : sinon le cam reste figée à mi-chemin, frustrant).
        if !state.active {
            if !moved {
                continue;
            }
            state.initial_yaw = a;
            state.elapsed = 0.0;
            state.active = true;
        }
        state.elapsed += time.delta_secs();
        let t = (state.elapsed / RECENTER_DURATION_SEC).clamp(0.0, 1.0);
        // Ease-out quad : t * (2 - t) → visible début + doux fin (WoW-like).
        let smooth_t = t * (2.0 - t);
        orbit.yaw_offset = state.initial_yaw * (1.0 - smooth_t);
        if t >= 1.0 {
            orbit.yaw_offset = 0.0;
            state.active = false;
        }
    }
}

/// Place chaque OrbitCamera derrière son target, à `distance` mètres, vise target+height.
/// Le yaw vient du target lui-même (réutilise Player.yaw managé par forgia-player).
fn orbit_follow(
    targets: Query<&GlobalTransform, Without<OrbitCamera>>,
    mut cams: Query<(&OrbitCamera, &mut Transform), With<Camera3d>>,
) {
    for (orbit, mut cam_tf) in &mut cams {
        let Ok(target_gt) = targets.get(orbit.target) else {
            continue;
        };
        let target_pos = target_gt.translation();
        // Extrait le yaw du target depuis sa rotation (en Y).
        let (yaw, _, _) = target_gt
            .compute_transform()
            .rotation
            .to_euler(EulerRot::YXZ);

        // Vecteur "derrière" : opposé au forward du target, modulé par pitch.
        // Forward V2 = -Z après rotation Y(yaw) → forward = (-sin(yaw), 0, -cos(yaw))
        // On veut camera derrière, donc on inverse + on ajoute la composante pitch (Y).
        // `yaw_offset` ajoute la rotation manuelle (LMB drag) au yaw du target.
        let cos_pitch = orbit.pitch.cos();
        let sin_pitch = orbit.pitch.sin();
        let total_yaw = yaw + orbit.yaw_offset;
        let back = Vec3::new(
            total_yaw.sin() * cos_pitch,
            -sin_pitch,
            total_yaw.cos() * cos_pitch,
        );

        let look_target = target_pos + Vec3::Y * orbit.height_offset;
        cam_tf.translation = look_target + back * orbit.distance;
        cam_tf.look_at(look_target, Vec3::Y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orbit_camera_defaults_reasonable() {
        let mut world = World::new();
        let target = world.spawn_empty().id();
        let cam = OrbitCamera::new(target);
        assert!(cam.distance >= cam.min_distance);
        assert!(cam.distance <= cam.max_distance);
        assert!(cam.pitch >= cam.min_pitch);
        assert!(cam.pitch <= cam.max_pitch);
    }

    #[test]
    fn plugin_builds_without_panic() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(ForgiaCameraOrbitPlugin);
    }
}
