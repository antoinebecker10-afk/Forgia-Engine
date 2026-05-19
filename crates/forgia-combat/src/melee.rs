#![allow(dead_code, unused_imports)]
//! Port verbatim de `forgia-game/src/combat/melee.rs` (V1).
//! RPG-3: Player melee combat system.
//!
//! Left-click when a melee weapon is equipped → short-range sphere cast (3m)
//! → damage closest enemy in cone → floating damage text → death if HP <= 0.
//!
//! Melee weapons: IronSword, SteelSword, BattleAxe, MithrilSword, HammerForgia.
//! Ranged weapons (FlameScepter, FrostWand, HuntingBow, Crossbow) use hitscan in weapons.rs.

use bevy::prelude::*;

// TODO: port from V1 — components
// use forgia_player::components::{FloatingText, GoblinGuard, GoblinOnFire, LocalPlayer, Player};

// TODO: port from V1 — inventory::equip::compute_weapon_bonus
// use forgia_inventory::equip::compute_weapon_bonus;

// TODO: port from V1 — inventory items (ItemId)
// use forgia_inventory::items::ItemId;

// TODO: port from V1 — inventory::storage::EquippedItems
// use forgia_inventory::storage::EquippedItems;

// TODO: port from V1 — resources (DamageBoost, FpsTuning, GlobalCooldown, InputBlockers)
// use forgia_core::resources::{DamageBoost, FpsTuning, GlobalCooldown, InputBlockers};

// ─── Cooldown Resource ───────────────────────────────────────────────────────

/// Presence = melee attack on cooldown.
#[derive(Resource)]
pub struct MeleeCooldown {
    pub timer: Timer,
}

// ─── Melee cooldown tick (no external deps — portable as-is) ─────────────────

/// Tick melee cooldown timer, remove when done.
pub fn melee_cooldown_tick_system(
    time: Res<Time>,
    mut cd: ResMut<MeleeCooldown>,
    mut commands: Commands,
) {
    cd.timer.tick(time.delta());
    if cd.timer.is_finished() {
        commands.remove_resource::<MeleeCooldown>();
    }
}

// ─── TODO: melee_attack_system requires cross-module deps ────────────────────
//
// pub fn melee_attack_system(
//     buttons: Res<ButtonInput<MouseButton>>,
//     blockers: Res<InputBlockers>,           // TODO: forgia-core
//     tuning: Res<FpsTuning>,                // TODO: forgia-core
//     equipped: Res<EquippedItems>,          // TODO: forgia-inventory
//     damage_boost: Option<Res<DamageBoost>>, // TODO: forgia-core
//     gcd: Option<Res<GlobalCooldown>>,      // TODO: forgia-core
//     melee_cd: Option<Res<MeleeCooldown>>,
//     camera_mode: Option<Res<CameraMode>>,  // TODO: forgia-camera-fps
//     mut commands: Commands,
//     q_player: Query<&GlobalTransform, (With<Player>, With<LocalPlayer>)>, // TODO: forgia-player
//     mut q_goblins: Query<(Entity, &mut GoblinGuard, &GlobalTransform)>,    // TODO: forgia-player
//     level: Res<PlayerLevel>,               // TODO: forgia-combat (from lib.rs)
//     mut combat_state: Option<ResMut<CombatState>>, // TODO: forgia-core
// )

#[cfg(test)]
mod tests {
    use super::*;

    // Helper : insère Time<()> manuel sans TimePlugin pour que `advance_by`
    // ne soit pas écrasé par `time_system` à chaque frame (Bevy 0.18 trap).
    fn app_with_manual_time() -> App {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default());
        app
    }

    #[test]
    fn melee_cooldown_tick_advances_and_keeps_resource() {
        let mut app = app_with_manual_time();
        app.insert_resource(MeleeCooldown {
            timer: Timer::from_seconds(2.0, TimerMode::Once),
        });
        app.add_systems(
            Update,
            melee_cooldown_tick_system.run_if(resource_exists::<MeleeCooldown>),
        );
        // Simulate 500ms
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(500));
        app.update();
        let cd = app.world().get_resource::<MeleeCooldown>()
            .expect("MeleeCooldown must remain at 0.5s/2s");
        assert!(cd.timer.fraction() > 0.20 && cd.timer.fraction() < 0.30);
    }

    #[test]
    fn melee_cooldown_tick_removes_resource_when_finished() {
        let mut app = app_with_manual_time();
        app.insert_resource(MeleeCooldown {
            timer: Timer::from_seconds(0.1, TimerMode::Once),
        });
        app.add_systems(
            Update,
            melee_cooldown_tick_system.run_if(resource_exists::<MeleeCooldown>),
        );
        // 5 × 100ms = 500ms, well past 0.1s
        for _ in 0..5 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_millis(100));
            app.update();
        }
        assert!(
            app.world().get_resource::<MeleeCooldown>().is_none(),
            "MeleeCooldown must be removed when timer finishes"
        );
    }
}
