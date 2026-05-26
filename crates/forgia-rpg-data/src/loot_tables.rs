//! # forgia-loot-tables
//!
//! Drop tables + currency MVP pour V7 Roguelite.
//!
//! ## Scope M2 step 3 (cette release)
//!
//! - `Souls` Resource : currency principale (u32)
//! - `Pickup` Component : marker entité-pickup au sol (value + lifetime)
//! - `sys_tick_pickup_lifetime` : despawn auto après `lifetime_secs`
//! - `sys_collect_pickups` : Player walk-over → `Souls += value` + despawn
//!
//! Drop pools rarity (Common/Uncommon/Rare/Legendary) : reportés step 4+.
//!
//! Le **spawn** du Pickup (Observer<DeathEvent>) est géré côté caller
//! (`forgia-mode-roguelite`) pour permettre filtrage par EnemyArchetype
//! et control de la value drop.

use bevy::prelude::*;
use forgia_core::prelude::*;

/// Currency principale Roguelite. Persiste pendant la run (reset OnExit run state).
#[derive(Resource, Default, Debug, Clone)]
pub struct Souls {
    pub current: u32,
    pub total_collected: u32,
}

/// Marker pickup au sol. Value transférée à `Souls` au collect.
#[derive(Component, Debug, Clone, Copy)]
pub struct Pickup {
    pub value: u32,
    pub lifetime_secs: f32,
    pub collect_radius: f32,
}

impl Default for Pickup {
    fn default() -> Self {
        Self {
            value: 1,
            lifetime_secs: 30.0,
            collect_radius: 2.5,
        }
    }
}

/// Marker requis sur l'entité Player pour qu'elle puisse collect (1 par scène).
#[derive(Component, Default)]
pub struct PickupCollector;

pub struct ForgiaLootTablesPlugin;

impl Plugin for ForgiaLootTablesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Souls>().add_systems(
            Update,
            (sys_tick_pickup_lifetime, sys_collect_pickups).in_set(GameSet::Effects),
        );
    }
}

/// Decrémente `lifetime_secs` de chaque Pickup. Despawn quand <= 0.
pub fn sys_tick_pickup_lifetime(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Pickup)>,
) {
    let dt = time.delta_secs();
    for (e, mut p) in &mut q {
        p.lifetime_secs -= dt;
        if p.lifetime_secs <= 0.0 {
            if let Ok(mut ec) = commands.get_entity(e) {
                ec.try_despawn();
            }
        }
    }
}

/// Collect : Player Transform distance Pickup < collect_radius → +value + despawn.
pub fn sys_collect_pickups(
    mut commands: Commands,
    mut souls: ResMut<Souls>,
    q_player: Query<&Transform, With<PickupCollector>>,
    q_pickups: Query<(Entity, &Transform, &Pickup)>,
) {
    let Some(player_pos) = q_player.iter().next().map(|t| t.translation) else {
        return;
    };
    for (pickup_e, xf, pickup) in &q_pickups {
        let d_sq = (xf.translation - player_pos).length_squared();
        let r_sq = pickup.collect_radius * pickup.collect_radius;
        if d_sq <= r_sq {
            souls.current = souls.current.saturating_add(pickup.value);
            souls.total_collected = souls.total_collected.saturating_add(pickup.value);
            if let Ok(mut ec) = commands.get_entity(pickup_e) {
                ec.try_despawn();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn souls_default_zero() {
        let s = Souls::default();
        assert_eq!(s.current, 0);
        assert_eq!(s.total_collected, 0);
    }

    #[test]
    fn pickup_default_values() {
        let p = Pickup::default();
        assert_eq!(p.value, 1);
        assert!(p.lifetime_secs > 0.0);
        assert!(p.collect_radius > 0.0);
    }

    #[test]
    fn souls_saturating_add() {
        let mut s = Souls {
            current: u32::MAX - 2,
            total_collected: 0,
        };
        s.current = s.current.saturating_add(5);
        assert_eq!(s.current, u32::MAX);
    }
}
