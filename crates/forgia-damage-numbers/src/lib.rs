//! forgia-damage-numbers — Floating damage numbers, 3D world-space billboards.
//!
//! Listens to `DamageEvent` from forgia-damage. For each damage, spawns a
//! `Text2d`-equivalent (text mesh) at the target position that drifts up and
//! fades out over `lifetime` seconds.
//!
//! V1 lesson : use a single `Text2d`-style billboard, NOT one entity per char.

use bevy::prelude::*;
use forgia_damage::{DamageEvent, DamageKind};

#[derive(Component)]
pub struct FloatingNumber {
    pub ttl: f32,
    pub initial_ttl: f32,
    pub vel_y: f32,
}

pub struct ForgiaDamageNumbersPlugin;

impl Plugin for ForgiaDamageNumbersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (spawn_on_damage, tick_numbers));
    }
}

fn spawn_on_damage(
    mut events: MessageReader<DamageEvent>,
    mut commands: Commands,
    targets: Query<&GlobalTransform>,
) {
    for ev in events.read() {
        let Ok(xf) = targets.get(ev.target) else { continue };
        let pos = xf.translation() + Vec3::Y * 1.6;

        let color = match ev.kind {
            DamageKind::Fire => Color::srgb(1.0, 0.4, 0.1),
            DamageKind::Poison => Color::srgb(0.3, 1.0, 0.3),
            DamageKind::Explosion => Color::srgb(1.0, 0.6, 0.0),
            _ => Color::srgb(1.0, 1.0, 1.0),
        };

        commands.spawn((
            FloatingNumber { ttl: 1.0, initial_ttl: 1.0, vel_y: 1.6 },
            Text2d::new(format!("{:.0}", ev.amount)),
            TextFont { font_size: 22.0, ..default() },
            TextColor(color),
            Transform::from_translation(pos),
            GlobalTransform::default(),
        ));
    }
}

fn tick_numbers(
    time: Res<Time>,
    mut q: Query<(Entity, &mut FloatingNumber, &mut Transform, &mut TextColor)>,
    mut commands: Commands,
) {
    let dt = time.delta_secs();
    for (e, mut fn_, mut xf, mut color) in &mut q {
        fn_.ttl -= dt;
        xf.translation.y += fn_.vel_y * dt;
        fn_.vel_y = (fn_.vel_y - 2.0 * dt).max(0.2);
        let alpha = (fn_.ttl / fn_.initial_ttl).clamp(0.0, 1.0);
        let c = color.0.to_linear();
        color.0 = Color::linear_rgba(c.red, c.green, c.blue, alpha);
        if fn_.ttl <= 0.0 {
            commands.entity(e).despawn();
        }
    }
}
