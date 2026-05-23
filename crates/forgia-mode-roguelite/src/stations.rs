//! stations.rs — V7 M3 step 3 : stations heal + ammo dans l'arène Roguelite.
//!
//! Pattern roguelite classique (Diablo shrine / Hadès fountain / RoR2 chest) :
//! - 4 health stations + 4 ammo stations placés au scene spawn (4 coins arène)
//! - Walk-over (radius 3m) → effet appliqué + station "used" (visuel dimm)
//! - Single-use per stage : `sys_reset_stations_on_stage_change` ré-active toutes
//!   les stations à chaque transition de stage (StageGraph traversal).
//! - Cooldown visuel : color shift used=true → desaturé/faible emissive.
//!
//! Heal : +30 HP (forgia_damage::Health.current clamp max).
//! Ammo : refill mag courante de EquippedWeapons.current (max = config.mag_size).

use crate::run::{RogueliteRunMarker, RunState};
use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;
use bevy_rapier3d::prelude::{Collider, RigidBody, Sensor};
use forgia_combat::weapons::EquippedWeapons;
use forgia_core::prelude::*;
use forgia_loot_tables::PickupCollector;

/// HP rendus par une heal station active.
pub const HEAL_AMOUNT: f32 = 30.0;
/// Rayon walk-over (m) pour activer une station.
pub const STATION_TRIGGER_RADIUS: f32 = 3.0;
/// Hauteur du pillar visuel (m).
const STATION_HEIGHT: f32 = 1.6;
const STATION_RADIUS: f32 = 0.35;

#[derive(Component, Debug, Clone, Copy)]
pub struct HealthStation {
    pub used: bool,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct AmmoStation {
    pub used: bool,
}

/// Positions des 4 stations health + 4 stations ammo dans la map 300m.
/// Pattern : 4 coins du carré inner 60m (zone de combat tank+runner) +
/// 4 mid-distance 100m (zone sniper). Mix santé/munition alterné.
pub fn station_spawn_points() -> [(Vec3, StationKind); 8] {
    use StationKind::*;
    [
        // Inner ring (60m) — heal close to combat
        (Vec3::new(60.0, 0.0, 60.0), Health),
        (Vec3::new(-60.0, 0.0, 60.0), Ammo),
        (Vec3::new(-60.0, 0.0, -60.0), Health),
        (Vec3::new(60.0, 0.0, -60.0), Ammo),
        // Outer ring (110m) — exposition longue distance
        (Vec3::new(0.0, 0.0, 110.0), Ammo),
        (Vec3::new(110.0, 0.0, 0.0), Health),
        (Vec3::new(0.0, 0.0, -110.0), Ammo),
        (Vec3::new(-110.0, 0.0, 0.0), Health),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StationKind {
    Health,
    Ammo,
}

/// Spawn les 8 stations au début de la run (OnEnter Roguelite scene).
pub fn spawn_stations(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let mesh = meshes.add(Cylinder::new(STATION_RADIUS, STATION_HEIGHT));
    for (i, (pos, kind)) in station_spawn_points().iter().enumerate() {
        let (label, color, emissive) = match kind {
            StationKind::Health => (
                format!("RogueliteHealthStation_{i}"),
                Color::srgb(0.20, 0.95, 0.40),
                LinearRgba::new(0.30, 1.6, 0.50, 1.0),
            ),
            StationKind::Ammo => (
                format!("RogueliteAmmoStation_{i}"),
                Color::srgb(0.95, 0.85, 0.20),
                LinearRgba::new(1.5, 1.2, 0.30, 1.0),
            ),
        };
        let mat = materials.add(StandardMaterial {
            base_color: color,
            emissive,
            metallic: 0.3,
            perceptual_roughness: 0.5,
            ..default()
        });
        let world_pos = pos.with_y(STATION_HEIGHT * 0.5);
        let base = commands.spawn((
            Name::new(label),
            RogueliteRunMarker,
            DespawnOnExit(GameMode::Roguelite),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(mat),
            Transform::from_translation(world_pos),
            // Sensor collider : raycast hit (combat AI evite/detecte) sans bloquer player.
            RigidBody::Fixed,
            Collider::cylinder(STATION_HEIGHT * 0.5, STATION_RADIUS),
            Sensor,
        ));
        let mut ec = base;
        match kind {
            StationKind::Health => {
                ec.insert(HealthStation { used: false });
            }
            StationKind::Ammo => {
                ec.insert(AmmoStation { used: false });
            }
        }
    }
    info!("[roguelite] Stations spawned : 4 health + 4 ammo");
}

/// Walk-over collect health stations. Heal `HEAL_AMOUNT` HP clamp max.
pub fn sys_use_health_stations(
    q_player: Query<(Entity, &Transform), With<PickupCollector>>,
    mut q_stations: Query<
        (
            &Transform,
            &mut HealthStation,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        Without<PickupCollector>,
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    let Some((player_entity, player_xf)) = q_player.iter().next() else {
        return;
    };
    let player_pos = player_xf.translation;
    let r_sq = STATION_TRIGGER_RADIUS * STATION_TRIGGER_RADIUS;
    for (st_xf, mut station, mut mat_handle) in &mut q_stations {
        if station.used {
            continue;
        }
        let d_sq = (st_xf.translation - player_pos).length_squared();
        if d_sq > r_sq {
            continue;
        }
        // Heal Player via defer (évite conflit Query disjoint).
        commands.queue(move |world: &mut World| {
            if let Some(mut hp) = world.get_mut::<forgia_damage::Health>(player_entity) {
                let before = hp.current;
                hp.current = (hp.current + HEAL_AMOUNT).min(hp.max);
                info!(
                    "[roguelite] Health Station used : HP {:.0} → {:.0}",
                    before, hp.current
                );
            }
        });
        station.used = true;
        // Dim visuel : remplace material par version désaturée.
        let dimmed = materials.add(StandardMaterial {
            base_color: Color::srgb(0.30, 0.50, 0.32),
            emissive: LinearRgba::new(0.05, 0.20, 0.08, 1.0),
            metallic: 0.3,
            perceptual_roughness: 0.5,
            ..default()
        });
        mat_handle.0 = dimmed;
    }
}

/// Walk-over collect ammo stations. Refill mag de l'arme courante.
#[allow(clippy::too_many_arguments)]
pub fn sys_use_ammo_stations(
    q_player: Query<&Transform, With<PickupCollector>>,
    mut q_stations: Query<
        (
            &Transform,
            &mut AmmoStation,
            &mut MeshMaterial3d<StandardMaterial>,
        ),
        Without<PickupCollector>,
    >,
    mut equipped: Option<ResMut<EquippedWeapons>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(player_xf) = q_player.iter().next() else {
        return;
    };
    let player_pos = player_xf.translation;
    let r_sq = STATION_TRIGGER_RADIUS * STATION_TRIGGER_RADIUS;
    for (st_xf, mut station, mut mat_handle) in &mut q_stations {
        if station.used {
            continue;
        }
        let d_sq = (st_xf.translation - player_pos).length_squared();
        if d_sq > r_sq {
            continue;
        }
        if let Some(eq) = equipped.as_deref_mut() {
            let current = eq.current;
            if let Some(slot) = eq.slots.get_mut(&current) {
                let before = slot.current_mag;
                slot.current_mag = slot.config.mag_size;
                // Refill reserve aussi (généreux step 3 — calibrer plus tard).
                slot.reserve = slot.config.reserve_max;
                info!(
                    "[roguelite] Ammo Station used : {:?} mag {} → {}, reserve refilled",
                    current, before, slot.current_mag
                );
            }
        }
        station.used = true;
        let dimmed = materials.add(StandardMaterial {
            base_color: Color::srgb(0.45, 0.40, 0.20),
            emissive: LinearRgba::new(0.20, 0.15, 0.05, 1.0),
            metallic: 0.3,
            perceptual_roughness: 0.5,
            ..default()
        });
        mat_handle.0 = dimmed;
    }
}

/// Re-active toutes les stations quand on change de stage (RunState transition).
/// Restore l'emissive original via re-spawn de Material. Pattern simple : on flag
/// used=false ET on regenère un Material full color.
pub fn sys_reset_stations_on_stage_change(
    mut q_health: Query<
        (&mut HealthStation, &mut MeshMaterial3d<StandardMaterial>),
        Without<AmmoStation>,
    >,
    mut q_ammo: Query<
        (&mut AmmoStation, &mut MeshMaterial3d<StandardMaterial>),
        Without<HealthStation>,
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
    run_state: Res<State<RunState>>,
    mut last_state: Local<Option<RunState>>,
) {
    let current = run_state.get().clone();
    let prev = last_state.clone();
    *last_state = Some(current.clone());
    if prev.as_ref() == Some(&current) {
        return;
    }
    // Reset uniquement sur transitions entre stages combat (pas Lobby/Defeat/Victory).
    let in_combat = matches!(current, RunState::InRun { .. } | RunState::Boss { .. });
    if !in_combat {
        return;
    }
    let health_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.20, 0.95, 0.40),
        emissive: LinearRgba::new(0.30, 1.6, 0.50, 1.0),
        metallic: 0.3,
        perceptual_roughness: 0.5,
        ..default()
    });
    let ammo_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.85, 0.20),
        emissive: LinearRgba::new(1.5, 1.2, 0.30, 1.0),
        metallic: 0.3,
        perceptual_roughness: 0.5,
        ..default()
    });
    let mut reset_count = 0;
    for (mut station, mut mat) in &mut q_health {
        if station.used {
            station.used = false;
            mat.0 = health_mat.clone();
            reset_count += 1;
        }
    }
    for (mut station, mut mat) in &mut q_ammo {
        if station.used {
            station.used = false;
            mat.0 = ammo_mat.clone();
            reset_count += 1;
        }
    }
    if reset_count > 0 {
        info!("[roguelite] Stations reset ({reset_count}) on stage transition");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_points_count() {
        assert_eq!(station_spawn_points().len(), 8);
    }

    #[test]
    fn spawn_points_balanced_kinds() {
        let pts = station_spawn_points();
        let health = pts
            .iter()
            .filter(|(_, k)| *k == StationKind::Health)
            .count();
        let ammo = pts.iter().filter(|(_, k)| *k == StationKind::Ammo).count();
        assert_eq!(health, 4);
        assert_eq!(ammo, 4);
    }

    #[test]
    fn heal_amount_reasonable() {
        // Player max HP = 100. Heal 30 = 30% → meaningful but not infinite.
        assert!(HEAL_AMOUNT > 0.0);
        assert!(HEAL_AMOUNT <= 50.0, "Heal trop généreux");
    }

    #[test]
    fn trigger_radius_walkable() {
        // 3m = walk-over generous, pas besoin de précision pixel.
        assert!(STATION_TRIGGER_RADIUS >= 2.0);
        assert!(STATION_TRIGGER_RADIUS <= 5.0);
    }
}
