//! forgia-damage-numbers — Floating damage numbers, 3D world-space billboards.
//!
//! Story-457 (2026-05-19) — refacto : lit `CombatHitEvent` (au lieu de
//! `DamageEvent`) car c'est l'event source-of-truth pour le fire path FPS V2.
//! Couleur + taille pilotées par `HitFeedbackTuning` (genome hot-reload) selon
//! `body_zone`. Position depuis `hit_world_pos` (vs `target.translation + Y`).

use bevy::prelude::*;
use forgia_combat::prelude::CombatHitEvent;
use forgia_damage::{HitFeedback, HitZone};

#[derive(Component)]
pub struct FloatingNumber {
    pub ttl: f32,
    pub initial_ttl: f32,
    pub vel_y: f32,
}

pub struct ForgiaDamageNumbersPlugin;

impl Plugin for ForgiaDamageNumbersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (spawn_on_hit, tick_numbers));
    }
}

fn spawn_on_hit(
    mut events: MessageReader<CombatHitEvent>,
    mut commands: Commands,
    feedback: Res<HitFeedback>,
) {
    for ev in events.read() {
        let rgb = feedback.0.color(ev.body_zone);
        let font_size = feedback.0.font_size(ev.body_zone);
        let color = Color::linear_rgb(rgb[0], rgb[1], rgb[2]);

        // Headshot pop : monte plus vite (lecture "important").
        let vel_y = match ev.body_zone {
            HitZone::Head => 2.4,
            HitZone::Body => 1.6,
            HitZone::Limb => 1.2,
        };

        commands.spawn((
            FloatingNumber { ttl: 1.0, initial_ttl: 1.0, vel_y },
            Text2d::new(format!("{:.0}", ev.damage)),
            TextFont { font_size, ..default() },
            TextColor(color),
            Transform::from_translation(ev.hit_world_pos + Vec3::Y * 0.1),
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
            if let Ok(mut ec) = commands.get_entity(e) {
                ec.try_despawn();
            }
        }
    }
}
