//! forgia-damage — Health + DamageEvent + DeathEvent atomic crate.
//!
//! No deps on weapons/AI/UI — they emit `DamageEvent`, this crate consumes
//! them and mutates `Health`, then emits `DeathEvent` when HP <= 0.
//!
//! Bevy 0.18.1 — `Event` renamed to `Message`.

use bevy::prelude::*;

/// Per-entity health. Add to any entity that can take damage.
#[derive(Component, Debug, Clone, Copy)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Health {
    pub fn new(max: f32) -> Self {
        Self { current: max, max }
    }

    pub fn is_alive(&self) -> bool {
        self.current > 0.0
    }

    pub fn fraction(&self) -> f32 {
        if self.max <= 0.0 { 0.0 } else { (self.current / self.max).clamp(0.0, 1.0) }
    }
}

/// Marker for entities whose death should trigger respawn / cleanup elsewhere.
#[derive(Component, Default)]
pub struct Mortal;

#[derive(Message, Debug, Clone, Copy)]
pub struct DamageEvent {
    pub target: Entity,
    pub source: Option<Entity>,
    pub amount: f32,
    pub kind: DamageKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageKind {
    Physical,
    Fire,
    Poison,
    Explosion,
    Fall,
    Other,
}

#[derive(Message, Debug, Clone, Copy)]
pub struct DeathEvent {
    pub target: Entity,
    pub source: Option<Entity>,
    pub final_kind: DamageKind,
}

pub struct ForgiaDamagePlugin;

impl Plugin for ForgiaDamagePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<DamageEvent>()
            .add_message::<DeathEvent>()
            .add_systems(Update, apply_damage);
    }
}

fn apply_damage(
    mut events: MessageReader<DamageEvent>,
    mut healths: Query<&mut Health>,
    mut deaths: MessageWriter<DeathEvent>,
) {
    for ev in events.read() {
        let Ok(mut hp) = healths.get_mut(ev.target) else { continue };
        if !hp.is_alive() { continue; }
        hp.current = (hp.current - ev.amount).max(0.0);
        if hp.current <= 0.0 {
            deaths.write(DeathEvent {
                target: ev.target,
                source: ev.source,
                final_kind: ev.kind,
            });
        }
    }
}
