//! Bourrasque — stats de tir + sensor (story-532 AC9).
//!
//! Bourrasque = `WeaponType::AssaultRifle` (slot Digit2). AC1 (7 pellets cone
//! 20°, 8 dmg/pellet, 10 m, mag 5, 1.5/s) vit dans la couche definition :
//! `assets/genomes/viewmodel_arena.toml [weapons.bourrasque]` — rien à coder.
//!
//! Ce module observe le fire path SANS le toucher : `ShotResolved` (1/tir) +
//! `CombatHitEvent` (1/pellet qui touche) sont MULTICAST — lecteurs killfeed/
//! confidence/barks non affectés. Le sort F « Coup de Bourrasque » (gust,
//! story-572) reste dans forgia-mode-roguelite/shockwave.rs.

use bevy::prelude::*;
use forgia_combat::combat_juice::CombatHitEvent;
use forgia_combat::confidence::ShotResolved;
use forgia_combat::weapons::WeaponType;
use forgia_core::prelude::{GameMode, GameSet};
use std::fs;

/// L'arme persona « Bourrasque » dans l'enum héritée Arena.
pub const BOURRASQUE_WEAPON: WeaponType = WeaponType::AssaultRifle;

const SENSOR_PATH: &str = "forgia2_bourrasque.json";
/// Doit suivre `pellets` du genome (sensor only — le gameplay lit le genome).
/// v2 lance-rafales 2026-06-11 : 5 pellets (le pump 7 « faisait trop pompe »).
const PELLETS_PER_SHOT: u32 = 5;

/// Stats de la run courante — Resource globale (1 joueur).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct BourrasqueStats {
    /// Tirs engagés (1 par pression, pas par pellet).
    pub shots_run: u32,
    /// Tirs dont ≥1 pellet a touché un ennemi.
    pub shots_hit_run: u32,
    /// Pellets individuels ayant touché un ennemi.
    pub pellet_hits_run: u32,
    /// Kills (pellets fatals).
    pub kills_run: u32,
}

impl BourrasqueStats {
    pub fn reset_run(&mut self) {
        *self = Self::default();
    }
}

/// Application pure d'un tir résolu + des hits pellets — testable headless.
pub fn apply_shot(stats: &mut BourrasqueStats, hit_enemy: bool) {
    stats.shots_run += 1;
    if hit_enemy {
        stats.shots_hit_run += 1;
    }
}

fn sys_update_stats(
    mut shots: MessageReader<ShotResolved>,
    mut hits: MessageReader<CombatHitEvent>,
    mut stats: ResMut<BourrasqueStats>,
) {
    for shot in shots.read() {
        if shot.weapon == BOURRASQUE_WEAPON {
            apply_shot(&mut stats, shot.hit_enemy);
        }
    }
    for hit in hits.read() {
        if hit.weapon == Some(BOURRASQUE_WEAPON) {
            stats.pellet_hits_run += 1;
            if hit.is_kill {
                stats.kills_run += 1;
            }
        }
    }
}

/// Nouvelle run = compteurs à zéro (OnEnter Roguelite, miroir Pépin).
fn sys_reset_stats(mut stats: ResMut<BourrasqueStats>) {
    stats.reset_run();
}

/// Sensor `forgia2_bourrasque.json` 1Hz — story-532 AC9.
fn sys_write_bourrasque_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    stats: Res<BourrasqueStats>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let pellets_fired = stats.shots_run * PELLETS_PER_SHOT;
    let pellet_ratio = if pellets_fired > 0 {
        f64::from(stats.pellet_hits_run) / f64::from(pellets_fired)
    } else {
        0.0
    };
    let shot_accuracy = if stats.shots_run > 0 {
        f64::from(stats.shots_hit_run) / f64::from(stats.shots_run)
    } else {
        0.0
    };
    let json = format!(
        r#"{{"id":"bourrasque","severity":"ok","next_step":"","timestamp_secs":{:.1},"shots_run":{},"shots_hit_run":{},"shot_accuracy":{:.3},"pellets_fired":{},"pellet_hits_run":{},"pellet_hit_ratio":{:.3},"kills_run":{}}}"#,
        time.elapsed_secs(),
        stats.shots_run,
        stats.shots_hit_run,
        shot_accuracy,
        pellets_fired,
        stats.pellet_hits_run,
        pellet_ratio,
        stats.kills_run,
    );
    if let Err(e) = fs::write(SENSOR_PATH, &json) {
        warn!("[forgia-fps/bourrasque] sensor write failed: {e}");
    }
}

pub struct ForgiaBourrasquePlugin;

impl Plugin for ForgiaBourrasquePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BourrasqueStats>()
            .add_systems(OnEnter(GameMode::Roguelite), sys_reset_stats)
            .add_systems(Update, sys_update_stats.in_set(GameSet::Combat))
            .add_systems(Update, sys_write_bourrasque_sensor.in_set(GameSet::Sensors));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgia_damage::HitZone;

    #[test]
    fn apply_shot_counts_shots_and_hits() {
        let mut s = BourrasqueStats::default();
        apply_shot(&mut s, true);
        apply_shot(&mut s, false);
        apply_shot(&mut s, true);
        assert_eq!(s.shots_run, 3);
        assert_eq!(s.shots_hit_run, 2);
        s.reset_run();
        assert_eq!(s.shots_run, 0);
    }

    /// Headless : seuls les tirs/pellets Bourrasque comptent (multicast,
    /// ModernAR ignoré).
    #[test]
    fn stats_track_bourrasque_only() {
        let mut app = App::new();
        app.init_resource::<BourrasqueStats>()
            .add_message::<ShotResolved>()
            .add_message::<CombatHitEvent>()
            .add_systems(Update, sys_update_stats);

        app.world_mut().write_message(ShotResolved {
            weapon: BOURRASQUE_WEAPON,
            hit_enemy: true,
        });
        app.world_mut().write_message(ShotResolved {
            weapon: WeaponType::ModernAR,
            hit_enemy: true,
        });
        // 2 pellets touchent, dont 1 fatal + 1 hit Pépin (ignoré).
        for (weapon, kill) in [
            (BOURRASQUE_WEAPON, false),
            (BOURRASQUE_WEAPON, true),
            (WeaponType::ModernAR, true),
        ] {
            app.world_mut().write_message(CombatHitEvent {
                target: Entity::PLACEHOLDER,
                attacker: None,
                damage: 8.0,
                is_kill: kill,
                is_headshot: false,
                hit_world_pos: Vec3::ZERO,
                weapon: Some(weapon),
                body_zone: HitZone::Body,
            });
        }
        app.update();

        let s = app.world().resource::<BourrasqueStats>();
        assert_eq!(s.shots_run, 1, "le tir Pépin ne compte pas");
        assert_eq!(s.shots_hit_run, 1);
        assert_eq!(s.pellet_hits_run, 2);
        assert_eq!(s.kills_run, 1);
    }
}
