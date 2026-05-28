//! physics_sensor.rs — Producteur `forgia2_physics.json` (1Hz, cross-mode).
//!
//! Comble le blind spot physique Rapier 0.33 : aujourd'hui 0 sensor exposait
//! l'état du monde physique (rigid bodies, colliders, sensors, KCC, joints).
//! Bug class concernée : story-540 player stuck KCC, story-545 raycast self-hit
//! (predicate vs exclude_rigid_body), collisions désynchronisées.
//!
//! ## Schema JSON
//!
//! ```json
//! {
//!   "id": "physics",
//!   "severity": "ok|warn",
//!   "next_step": "...",
//!   "timestamp_secs": 12.5,
//!   "gravity_y": -9.81,
//!   "rigid_bodies_total": 42,
//!   "rigid_bodies_dynamic": 28,
//!   "rigid_bodies_kinematic_pos": 5,
//!   "rigid_bodies_kinematic_vel": 0,
//!   "rigid_bodies_fixed": 9,
//!   "colliders_total": 60,
//!   "colliders_sensor": 12,
//!   "kcc_count": 1,
//!   "joints_count": 0
//! }
//! ```
//!
//! Pas de panic si Rapier non initialisé — early-return silencieux.

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

#[derive(Resource, Default)]
pub struct PhysicsSensorState {
    pub last_write_secs: f32,
}

const SENSOR_PATH: &str = "forgia2_physics.json";

#[allow(clippy::too_many_arguments)]
pub fn sys_write_physics_sensor(
    time: Res<Time>,
    q_config: Query<&RapierConfiguration>,
    mut state: ResMut<PhysicsSensorState>,
    q_rb: Query<&RigidBody>,
    q_collider: Query<&Collider>,
    q_sensor: Query<(), (With<Collider>, With<Sensor>)>,
    q_kcc: Query<(), With<KinematicCharacterController>>,
    q_joint_impulse: Query<(), With<ImpulseJoint>>,
    q_joint_multibody: Query<(), With<MultibodyJoint>>,
) {
    let now = time.elapsed_secs();
    if now - state.last_write_secs < 1.0 {
        return;
    }
    state.last_write_secs = now;

    let mut dynamic = 0u32;
    let mut kine_pos = 0u32;
    let mut kine_vel = 0u32;
    let mut fixed = 0u32;
    for rb in &q_rb {
        match *rb {
            RigidBody::Dynamic => dynamic += 1,
            RigidBody::KinematicPositionBased => kine_pos += 1,
            RigidBody::KinematicVelocityBased => kine_vel += 1,
            RigidBody::Fixed => fixed += 1,
        }
    }
    let rb_total = dynamic + kine_pos + kine_vel + fixed;

    let colliders_total = q_collider.iter().count() as u32;
    let colliders_sensor = q_sensor.iter().count() as u32;
    let kcc_count = q_kcc.iter().count() as u32;
    let joints_count = (q_joint_impulse.iter().count() + q_joint_multibody.iter().count()) as u32;

    let gravity_y = q_config
        .iter()
        .next()
        .map(|c| c.gravity.y)
        .unwrap_or(f32::NAN);

    let (severity, next_step) = if rb_total == 0 && colliders_total == 0 {
        (
            "warn",
            "Physics world empty — RapierPhysicsPlugin pas initialisé ou aucun corps actif",
        )
    } else if gravity_y.is_nan() {
        (
            "warn",
            "RapierConfiguration introuvable — gravity inconnue, simulation peut-être désactivée",
        )
    } else if kcc_count == 0 {
        (
            "ok",
            "Aucun KinematicCharacterController — normal hors InGame, à vérifier sinon",
        )
    } else {
        ("ok", "")
    };

    let json = format!(
        r#"{{"id":"physics","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"gravity_y":{},"rigid_bodies_total":{rb_total},"rigid_bodies_dynamic":{dynamic},"rigid_bodies_kinematic_pos":{kine_pos},"rigid_bodies_kinematic_vel":{kine_vel},"rigid_bodies_fixed":{fixed},"colliders_total":{colliders_total},"colliders_sensor":{colliders_sensor},"kcc_count":{kcc_count},"joints_count":{joints_count}}}"#,
        now,
        format_f32(gravity_y),
    );

    if let Err(e) = std::fs::write(SENSOR_PATH, &json) {
        warn!("[forgia-observability] physics_sensor write failed: {e}");
    }
}

fn format_f32(v: f32) -> String {
    if v.is_nan() {
        "null".to_string()
    } else {
        format!("{v:.3}")
    }
}
