//! forgia-weapon-hitscan — Instant raycast weapon with damage falloff.
//!
//! Emits `DamageEvent` from `forgia-damage` on hit. Uses rapier3d raycast.
//!
//! Usage :
//! ```ignore
//! commands.spawn((Hitscan::new(35.0, 80.0, 25.0), TryFire::default()));
//! // ... per-frame :
//! try_fire.pull = true;  // user pulled trigger
//! ```

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use forgia_damage::{DamageEvent, DamageKind};

/// Hitscan weapon attached to a shooter entity (player / bot).
#[derive(Component, Debug, Clone, Copy)]
pub struct Hitscan {
    /// Base damage at point-blank range.
    pub damage_base: f32,
    /// Maximum effective range (meters). Past this, damage falls to 0.
    pub range_max: f32,
    /// Start of falloff (meters). Damage = damage_base in [0, falloff_start].
    pub falloff_start: f32,
    /// Seconds between shots (rate of fire).
    pub cooldown: f32,
    /// Internal cooldown timer (counts down each frame).
    pub cooldown_left: f32,
}

impl Hitscan {
    pub fn new(damage_base: f32, range_max: f32, falloff_start: f32) -> Self {
        Self {
            damage_base,
            range_max,
            falloff_start,
            cooldown: 0.12,
            cooldown_left: 0.0,
        }
    }

    pub fn with_cooldown(mut self, cooldown: f32) -> Self {
        self.cooldown = cooldown;
        self
    }

    /// Damage at distance `d`. Linear falloff between `falloff_start` and `range_max`.
    pub fn damage_at(&self, d: f32) -> f32 {
        if d <= self.falloff_start { return self.damage_base; }
        if d >= self.range_max { return 0.0; }
        let t = (d - self.falloff_start) / (self.range_max - self.falloff_start);
        self.damage_base * (1.0 - t)
    }
}

/// Per-frame trigger pull state. Set `pull = true` when input fires.
#[derive(Component, Debug, Default)]
pub struct TryFire {
    pub pull: bool,
}

/// Emitted when a hitscan actually fires (regardless of hit). Useful for VFX/audio.
#[derive(Message, Debug, Clone, Copy)]
pub struct HitscanFired {
    pub shooter: Entity,
    pub origin: Vec3,
    pub direction: Vec3,
    pub hit: Option<Entity>,
    pub hit_point: Option<Vec3>,
    pub damage_applied: f32,
}

pub struct ForgiaWeaponHitscanPlugin;

impl Plugin for ForgiaWeaponHitscanPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<HitscanFired>()
            .add_systems(Update, tick_cooldown)
            .add_systems(Update, fire_hitscan.after(tick_cooldown));
    }
}

fn tick_cooldown(time: Res<Time>, mut q: Query<&mut Hitscan>) {
    let dt = time.delta_secs();
    for mut h in &mut q {
        h.cooldown_left = (h.cooldown_left - dt).max(0.0);
    }
}

fn fire_hitscan(
    mut q: Query<(Entity, &mut Hitscan, &mut TryFire, &GlobalTransform)>,
    rapier: ReadRapierContext,
    mut dmg_w: MessageWriter<DamageEvent>,
    mut fired_w: MessageWriter<HitscanFired>,
) {
    let Ok(ctx) = rapier.single() else { return };
    for (shooter, mut hitscan, mut try_fire, xf) in &mut q {
        if !try_fire.pull { continue; }
        try_fire.pull = false;
        if hitscan.cooldown_left > 0.0 { continue; }
        hitscan.cooldown_left = hitscan.cooldown;

        let origin = xf.translation();
        let direction: Vec3 = (*xf.forward()).into();

        let filter = QueryFilter::default().exclude_collider(shooter);
        let hit = ctx.cast_ray_and_get_normal(
            origin,
            direction,
            hitscan.range_max,
            true,
            filter,
        );

        match hit {
            Some((target, intersect)) => {
                let dist = intersect.time_of_impact;
                let dmg = hitscan.damage_at(dist);
                if dmg > 0.0 {
                    dmg_w.write(DamageEvent {
                        target,
                        source: Some(shooter),
                        amount: dmg,
                        kind: DamageKind::Physical,
                    });
                }
                fired_w.write(HitscanFired {
                    shooter,
                    origin,
                    direction,
                    hit: Some(target),
                    hit_point: Some(intersect.point),
                    damage_applied: dmg,
                });
            }
            None => {
                fired_w.write(HitscanFired {
                    shooter,
                    origin,
                    direction,
                    hit: None,
                    hit_point: None,
                    damage_applied: 0.0,
                });
            }
        }
    }
}
