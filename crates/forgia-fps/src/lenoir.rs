//! Madame Lenoir — stats de précision + sensor (story-533 AC10).
//!
//! Lenoir = `WeaponType::Shotgun` (slot Digit3 — l'enum est héritée Arena,
//! l'arme est le SNIPER précision, cf shockwave.rs). AC1 : les stats actuelles
//! (50 body / ×2 head = one-shot tête, mag 5) sont CONSERVÉES — divergence
//! délibérée vs GDD littéral (80 HS / 40 body) : le one-shot tête est la
//! récompense « ouverture parfaite », tuning validé en jeu (leçon Bourrasque v1).
//!
//! Observe le fire path sans le toucher : `ShotResolved` + `CombatHitEvent`
//! (multicast). Le HS ratio est LA métrique persona (cible GDD : >40 % gamers).

use bevy::prelude::*;
use forgia_combat::combat_juice::CombatHitEvent;
use forgia_combat::confidence::ShotResolved;
use forgia_combat::weapons::WeaponType;
use forgia_core::prelude::{GameMode, GameSet};
use std::fs;

/// L'arme persona « Madame Lenoir » dans l'enum héritée Arena.
pub const LENOIR_WEAPON: WeaponType = WeaponType::Shotgun;

const SENSOR_PATH: &str = "forgia2_lenoir.json";

/// Stats de la run courante — Resource globale (1 joueur).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct LenoirStats {
    pub shots_run: u32,
    /// Tirs qui ont touché un adversaire.
    pub hits_run: u32,
    /// Touches en pleine tête (zone Head).
    pub headshots_run: u32,
    pub kills_run: u32,
}

impl LenoirStats {
    pub fn reset_run(&mut self) {
        *self = Self::default();
    }
}

fn sys_update_stats(
    mut shots: MessageReader<ShotResolved>,
    mut hits: MessageReader<CombatHitEvent>,
    mut stats: ResMut<LenoirStats>,
) {
    for shot in shots.read() {
        if shot.weapon == LENOIR_WEAPON {
            stats.shots_run += 1;
        }
    }
    for hit in hits.read() {
        if hit.weapon == Some(LENOIR_WEAPON) {
            stats.hits_run += 1;
            if hit.is_headshot {
                stats.headshots_run += 1;
            }
            if hit.is_kill {
                stats.kills_run += 1;
            }
        }
    }
}

/// Nouvelle run = compteurs à zéro (OnEnter Roguelite, miroir Pépin/Bourrasque).
fn sys_reset_stats(mut stats: ResMut<LenoirStats>) {
    stats.reset_run();
}

/// Sensor `forgia2_lenoir.json` 1Hz — story-533 AC10.
fn sys_write_lenoir_sensor(time: Res<Time>, mut accum: Local<f32>, stats: Res<LenoirStats>) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let accuracy = if stats.shots_run > 0 {
        f64::from(stats.hits_run) / f64::from(stats.shots_run)
    } else {
        0.0
    };
    let hs_ratio = if stats.hits_run > 0 {
        f64::from(stats.headshots_run) / f64::from(stats.hits_run)
    } else {
        0.0
    };
    let json = format!(
        r#"{{"id":"lenoir","severity":"ok","next_step":"","timestamp_secs":{:.1},"shots_run":{},"hits_run":{},"accuracy":{:.3},"headshots_run":{},"hs_ratio":{:.3},"kills_run":{}}}"#,
        time.elapsed_secs(),
        stats.shots_run,
        stats.hits_run,
        accuracy,
        stats.headshots_run,
        hs_ratio,
        stats.kills_run,
    );
    if let Err(e) = fs::write(SENSOR_PATH, &json) {
        warn!("[forgia-fps/lenoir] sensor write failed: {e}");
    }
}

pub struct ForgiaLenoirPlugin;

impl Plugin for ForgiaLenoirPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LenoirStats>()
            .add_systems(OnEnter(GameMode::Roguelite), sys_reset_stats)
            .add_systems(Update, sys_update_stats.in_set(GameSet::Combat))
            .add_systems(Update, sys_write_lenoir_sensor.in_set(GameSet::Sensors));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgia_damage::HitZone;

    /// Headless : shots/hits/HS/kills comptés pour Lenoir seule.
    #[test]
    fn stats_track_lenoir_headshots() {
        let mut app = App::new();
        app.init_resource::<LenoirStats>()
            .add_message::<ShotResolved>()
            .add_message::<CombatHitEvent>()
            .add_systems(Update, sys_update_stats);

        for (weapon, hit) in [(LENOIR_WEAPON, true), (LENOIR_WEAPON, false)] {
            app.world_mut().write_message(ShotResolved {
                weapon,
                hit_enemy: hit,
            });
        }
        // 1 hit tête fatal Lenoir + 1 hit corps Bourrasque (ignoré).
        for (weapon, zone, kill) in [
            (LENOIR_WEAPON, HitZone::Head, true),
            (WeaponType::AssaultRifle, HitZone::Body, false),
        ] {
            app.world_mut().write_message(CombatHitEvent {
                target: Entity::PLACEHOLDER,
                attacker: None,
                damage: 100.0,
                is_kill: kill,
                is_headshot: zone == HitZone::Head,
                hit_world_pos: Vec3::ZERO,
                weapon: Some(weapon),
                body_zone: zone,
            });
        }
        app.update();

        let s = app.world().resource::<LenoirStats>();
        assert_eq!(s.shots_run, 2);
        assert_eq!(s.hits_run, 1);
        assert_eq!(s.headshots_run, 1);
        assert_eq!(s.kills_run, 1);
        let mut s2 = *s;
        s2.reset_run();
        assert_eq!(s2.shots_run, 0);
    }
}
