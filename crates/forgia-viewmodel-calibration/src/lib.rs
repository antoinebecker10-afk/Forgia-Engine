//! forgia-viewmodel-calibration
//!
//! Sensor cumulatif d'orientation viewmodel **par arme**.
//!
//! Le sensor V1 (`forgia-fps::viewmodel_debug`) ne mesure qu'une arme à la fois
//! (celle équipée). Pour calibrer les 4 armes V1 Arena (Pépin / Bourrasque /
//! Madame Lenoir / Boucherie), il fallait cycler manuellement et lire le JSON 4x.
//!
//! Ce crate :
//! 1. Maintient `CalibrationStore`: `HashMap<WeaponType, WeaponMeasurement>`.
//! 2. Mesure à chaque tick (1Hz) + sur changement d'arme.
//! 3. Écrit `forgia_viewmodel_calibration.json` cumulatif (toutes armes vues).
//! 4. Calcule `suggested_rotation_y_deg` quand l'angle barrel↔cam est ~90°.
//!
//! Workflow : tu cycles Digit1→2→3→4 en jeu, ADS 1× chacune, je relis le JSON,
//! je patche `viewmodel_arena.toml` d'un coup avec preuve nommée par arme.
//!
//! Story-384 : Quality Gate observability. Concept-first §3 étape 0 : framework
//! (sensor) + definition (TOML patches après mesure). Aucun fix spéculatif.

use bevy::camera::primitives::Aabb;
use bevy::prelude::*;
use forgia_combat::weapons::{EquippedWeapons, WeaponType};
use forgia_player::prelude::FpsCamera;
use forgia_viewmodel::WeaponViewmodel;
use std::collections::HashMap;
use std::fs;

/// Mesure individuelle pour une arme.
#[derive(Clone, Debug)]
pub struct WeaponMeasurement {
    pub mesh_local_size: [f32; 3],
    pub mesh_aabb_min: [f32; 3],
    pub mesh_aabb_max: [f32; 3],
    pub principal_axis_local: String,
    pub applied_transform_local_euler_deg: [f32; 3],
    pub barrel_direction_world: [f32; 3],
    pub cam_forward_world: [f32; 3],
    pub angle_barrel_to_cam_forward_deg: f32,
    pub interpretation: String,
    pub suggested_rotation_y_deg: Option<f32>,
    pub measured_at_secs: f32,
}

#[derive(Resource, Default)]
pub struct CalibrationStore {
    pub per_weapon: HashMap<WeaponType, WeaponMeasurement>,
}

#[derive(Resource)]
pub struct CalibrationTimer(pub Timer);

impl Default for CalibrationTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(1.0, TimerMode::Repeating))
    }
}

pub fn viewmodel_calibration_sensor(
    time: Res<Time>,
    mut timer: ResMut<CalibrationTimer>,
    mut store: ResMut<CalibrationStore>,
    q_vm: Query<(Entity, &GlobalTransform, &Transform), With<WeaponViewmodel>>,
    q_cam: Query<&GlobalTransform, With<FpsCamera>>,
    q_children: Query<&Children>,
    q_aabb: Query<&Aabb>,
    equipped: Res<EquippedWeapons>,
) {
    timer.0.tick(time.delta());
    let on_change = equipped.is_changed();
    if !timer.0.just_finished() && !on_change {
        return;
    }

    let Ok((vm_entity, vm_global, vm_local)) = q_vm.single() else {
        return;
    };
    let Ok(cam_global) = q_cam.single() else {
        return;
    };

    // BFS — agrège AABB local mesh
    let mut g_min = Vec3::splat(f32::MAX);
    let mut g_max = Vec3::splat(f32::MIN);
    let mut found = false;
    let mut stack = vec![vm_entity];
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
        return;
    }
    let size = g_max - g_min;

    // Axe principal local (le plus long = canon attendu)
    let (axis_name, axis_local) = if size.x >= size.y && size.x >= size.z {
        ("X", Vec3::X)
    } else if size.y >= size.z {
        ("Y", Vec3::Y)
    } else {
        ("Z", Vec3::Z)
    };

    let rot = vm_global.rotation();
    let cam_forward = cam_global.forward().as_vec3();
    let dir_pos = rot * axis_local;
    let (barrel_dir, sign) = if dir_pos.dot(cam_forward) > (-dir_pos).dot(cam_forward) {
        (dir_pos, "+")
    } else {
        (-dir_pos, "-")
    };
    let dot = barrel_dir.dot(cam_forward).clamp(-1.0, 1.0);
    let angle_deg = dot.acos().to_degrees();
    let local_euler = vm_local.rotation.to_euler(EulerRot::XYZ);
    let applied = [
        local_euler.0.to_degrees(),
        local_euler.1.to_degrees(),
        local_euler.2.to_degrees(),
    ];

    let (interpretation, suggested_y) = interpret(angle_deg, barrel_dir, cam_forward, applied[1]);

    let measurement = WeaponMeasurement {
        mesh_local_size: [size.x, size.y, size.z],
        mesh_aabb_min: [g_min.x, g_min.y, g_min.z],
        mesh_aabb_max: [g_max.x, g_max.y, g_max.z],
        principal_axis_local: format!("{}{}", sign, axis_name),
        applied_transform_local_euler_deg: applied,
        barrel_direction_world: [barrel_dir.x, barrel_dir.y, barrel_dir.z],
        cam_forward_world: [cam_forward.x, cam_forward.y, cam_forward.z],
        angle_barrel_to_cam_forward_deg: angle_deg,
        interpretation,
        suggested_rotation_y_deg: suggested_y,
        measured_at_secs: time.elapsed_secs(),
    };
    store.per_weapon.insert(equipped.current, measurement);

    write_json(&store);
}

fn interpret(
    angle_deg: f32,
    barrel: Vec3,
    cam_fwd: Vec3,
    current_y_deg: f32,
) -> (String, Option<f32>) {
    if angle_deg < 10.0 {
        ("OK : canon aligné forward (<10° écart)".to_string(), None)
    } else if angle_deg < 30.0 {
        (
            "ACCEPTABLE : canon proche forward, léger angle".to_string(),
            None,
        )
    } else if (80.0..100.0).contains(&angle_deg) {
        let right = cam_fwd.cross(Vec3::Y);
        let points_right = (barrel - right).length() < 0.5;
        let points_left = (barrel + right).length() < 0.5;
        if points_right {
            (
                "WRONG : canon pointe DROITE (suggested = current - 90°)".to_string(),
                Some(current_y_deg - 90.0),
            )
        } else if points_left {
            (
                "WRONG : canon pointe GAUCHE (suggested = current + 90°)".to_string(),
                Some(current_y_deg + 90.0),
            )
        } else if barrel.y > 0.7 {
            (
                "WRONG : canon pointe HAUT (rotation_x_deg à corriger)".to_string(),
                None,
            )
        } else if barrel.y < -0.7 {
            (
                "WRONG : canon pointe BAS (rotation_x_deg à corriger)".to_string(),
                None,
            )
        } else {
            (
                "WRONG : canon perpendiculaire à forward (90° écart)".to_string(),
                None,
            )
        }
    } else if angle_deg > 170.0 {
        (
            "WRONG : canon pointe DERRIERE (suggested = current + 180°)".to_string(),
            Some(current_y_deg + 180.0),
        )
    } else {
        (
            "WRONG : canon mal aligné, ajuster rotation_*_deg dans TOML".to_string(),
            None,
        )
    }
}

fn weapon_toml_key(w: WeaponType) -> &'static str {
    match w {
        WeaponType::ModernAR => "pepin",
        WeaponType::AssaultRifle => "bourrasque",
        WeaponType::Shotgun => "madame_lenoir",
        WeaponType::RocketLauncher => "boucherie",
        WeaponType::AK47 => "ak47",
        WeaponType::PlasmaRifle => "plasma_rifle",
        WeaponType::Chainsaw => "chainsaw",
    }
}

fn write_json(store: &CalibrationStore) {
    let mut weapons_json = String::new();
    let mut first = true;
    // Sortie ordre stable
    let ordered = [
        WeaponType::ModernAR,
        WeaponType::AssaultRifle,
        WeaponType::Shotgun,
        WeaponType::RocketLauncher,
    ];
    for w in ordered {
        let Some(m) = store.per_weapon.get(&w) else {
            continue;
        };
        if !first {
            weapons_json.push(',');
        }
        first = false;
        let suggested = match m.suggested_rotation_y_deg {
            Some(v) => format!("{:.1}", v),
            None => "null".to_string(),
        };
        let weapon_label = format!("{:?}", w);
        weapons_json.push_str(&format!(
            r#"
    "{}": {{
      "toml_key": "{}",
      "mesh_local_size": [{:.3}, {:.3}, {:.3}],
      "mesh_aabb_min": [{:.3}, {:.3}, {:.3}],
      "mesh_aabb_max": [{:.3}, {:.3}, {:.3}],
      "principal_axis_local": "{}",
      "applied_transform_local_euler_deg": [{:.2}, {:.2}, {:.2}],
      "barrel_direction_world": [{:.3}, {:.3}, {:.3}],
      "cam_forward_world": [{:.3}, {:.3}, {:.3}],
      "angle_barrel_to_cam_forward_deg": {:.2},
      "interpretation": "{}",
      "suggested_rotation_y_deg": {},
      "measured_at_secs": {:.1}
    }}"#,
            weapon_label,
            weapon_toml_key(w),
            m.mesh_local_size[0],
            m.mesh_local_size[1],
            m.mesh_local_size[2],
            m.mesh_aabb_min[0],
            m.mesh_aabb_min[1],
            m.mesh_aabb_min[2],
            m.mesh_aabb_max[0],
            m.mesh_aabb_max[1],
            m.mesh_aabb_max[2],
            m.principal_axis_local,
            m.applied_transform_local_euler_deg[0],
            m.applied_transform_local_euler_deg[1],
            m.applied_transform_local_euler_deg[2],
            m.barrel_direction_world[0],
            m.barrel_direction_world[1],
            m.barrel_direction_world[2],
            m.cam_forward_world[0],
            m.cam_forward_world[1],
            m.cam_forward_world[2],
            m.angle_barrel_to_cam_forward_deg,
            m.interpretation,
            suggested,
            m.measured_at_secs,
        ));
    }
    let json = format!(
        r#"{{
  "weapons_measured": {},
  "weapons": {{{}
  }}
}}
"#,
        store.per_weapon.len(),
        weapons_json
    );
    let _ = fs::write("forgia_viewmodel_calibration.json", json);
}

pub struct ForgiaViewmodelCalibrationPlugin;

impl Plugin for ForgiaViewmodelCalibrationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CalibrationStore>()
            .init_resource::<CalibrationTimer>()
            .add_systems(Update, viewmodel_calibration_sensor);
    }
}
