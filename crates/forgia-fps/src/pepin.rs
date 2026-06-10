//! Pépin — jauge de confiance (story-531 AC9/AC10, GDD §2.2).
//!
//! Pépin = `WeaponType::ModernAR` (mapping slot Digit1, cf forgia-mode-roguelite
//! audio.rs `weapon_fire_def`). La jauge vit dans `forgia_combat::confidence`
//! (state partagé fps↔HUD) ; ce module fait : genome tuning hot-reload, mise à
//! jour depuis [`ShotResolved`], payoff dégâts, reset entre runs, sensor.
//!
//! Payoff base (genome) : +`per_stack_damage` de dégâts PAR stack, uniquement
//! quand Pépin est l'arme du tir — défaut +2 %/stack = +20 % à pleine confiance.
//! Les boons story-530 ajouteront leurs effets par-dessus la même Resource.

use bevy::prelude::*;
use forgia_combat::confidence::{apply_shot, PepinConfidence, ShotResolved};
use forgia_combat::weapons::WeaponType;
use forgia_core::prelude::{GameMode, GameSet};
use forgia_genome_core::{Genome, GenomeLoader};
use serde::Deserialize;
use std::fs;

/// L'arme persona « Pépin » dans l'enum héritée Arena.
pub const PEPIN_WEAPON: WeaponType = WeaponType::ModernAR;

const SENSOR_PATH: &str = "forgia2_pepin.json";

// ─── Genome tuning ────────────────────────────────────────────────────────────

/// Tuning de la jauge — couche definition (assets/genomes/roguelite/
/// pepin_confidence.toml, hot-reload file_watcher). Défauts = GDD §2.2.
#[derive(Resource, Deserialize, TypePath, Clone, Debug)]
#[serde(default)]
pub struct PepinTuning {
    /// Coupe-circuit : false = jauge figée à 0, payoff neutre (debug/balance).
    pub enabled: bool,
    /// Stacks max (GDD : 10).
    pub max_stacks: u8,
    /// Bonus de dégâts par stack (0.02 = +2 %/stack, +20 % à 10).
    pub per_stack_damage: f32,
}

impl Default for PepinTuning {
    fn default() -> Self {
        Self {
            enabled: true,
            max_stacks: 10,
            per_stack_damage: 0.02,
        }
    }
}

#[derive(Resource)]
pub struct PepinTuningHandle(pub Handle<Genome<PepinTuning>>);

fn load_pepin_tuning(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handle: Handle<Genome<PepinTuning>> =
        asset_server.load("genomes/roguelite/pepin_confidence.toml");
    commands.insert_resource(PepinTuningHandle(handle));
}

fn sync_pepin_tuning(
    handle: Option<Res<PepinTuningHandle>>,
    assets: Res<Assets<Genome<PepinTuning>>>,
    mut tuning: ResMut<PepinTuning>,
) {
    let Some(g) = handle.as_deref().and_then(|h| assets.get(&h.0)) else {
        return;
    };
    *tuning = g.data.clone();
}

// ─── Payoff (pur, appelé par le fire path) ────────────────────────────────────

/// Multiplicateur de dégâts de confiance pour UN tir de `weapon`.
/// Neutre (1.0) pour toute arme ≠ Pépin ou si la jauge est désactivée.
pub fn confidence_damage_mul(
    conf: &PepinConfidence,
    tuning: &PepinTuning,
    weapon: WeaponType,
) -> f32 {
    if !tuning.enabled || weapon != PEPIN_WEAPON {
        return 1.0;
    }
    1.0 + f32::from(conf.stacks) * tuning.per_stack_damage
}

// ─── Update + reset + sensor ──────────────────────────────────────────────────

/// Consomme [`ShotResolved`] : seuls les tirs de Pépin font bouger la jauge.
fn sys_update_confidence(
    mut shots: MessageReader<ShotResolved>,
    mut conf: ResMut<PepinConfidence>,
    tuning: Res<PepinTuning>,
    time: Res<Time>,
) {
    for shot in shots.read() {
        if shot.weapon != PEPIN_WEAPON || !tuning.enabled {
            continue;
        }
        let before = conf.stacks;
        conf.stacks = apply_shot(conf.stacks, shot.hit_enemy, tuning.max_stacks);
        if shot.hit_enemy {
            conf.hits_run += 1;
        } else {
            conf.misses_run += 1;
        }
        conf.peak_run = conf.peak_run.max(conf.stacks);
        if conf.stacks != before {
            conf.last_change_secs = time.elapsed_secs();
        }
    }
}

/// Nouvelle run = Pépin repart timide (OnEnter Roguelite).
fn sys_reset_confidence(mut conf: ResMut<PepinConfidence>) {
    conf.reset_run();
}

/// Sensor `forgia2_pepin.json` 1Hz — story-531 AC10.
fn sys_write_pepin_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    conf: Res<PepinConfidence>,
    tuning: Res<PepinTuning>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let total = conf.hits_run + conf.misses_run;
    let accuracy = if total > 0 {
        f64::from(conf.hits_run) / f64::from(total)
    } else {
        0.0
    };
    let (severity, next_step) = if tuning.enabled {
        ("ok", "")
    } else {
        ("info", "jauge desactivee (enabled=false dans pepin_confidence.toml)")
    };
    let json = format!(
        r#"{{"id":"pepin","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"stacks":{},"max_stacks":{},"peak_run":{},"hits_run":{},"misses_run":{},"accuracy":{:.3},"damage_mul":{:.3}}}"#,
        time.elapsed_secs(),
        conf.stacks,
        tuning.max_stacks,
        conf.peak_run,
        conf.hits_run,
        conf.misses_run,
        accuracy,
        confidence_damage_mul(&conf, &tuning, PEPIN_WEAPON),
    );
    if let Err(e) = fs::write(SENSOR_PATH, &json) {
        warn!("[forgia-fps/pepin] sensor write failed: {e}");
    }
}

// ─── Plugin ───────────────────────────────────────────────────────────────────

pub struct ForgiaPepinPlugin;

impl Plugin for ForgiaPepinPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PepinTuning>()
            .init_asset::<Genome<PepinTuning>>()
            .register_asset_loader(GenomeLoader::<PepinTuning>::default())
            .add_systems(Startup, load_pepin_tuning)
            .add_systems(OnEnter(GameMode::Roguelite), sys_reset_confidence)
            .add_systems(
                Update,
                (sync_pepin_tuning, sys_update_confidence).in_set(GameSet::Combat),
            )
            .add_systems(Update, sys_write_pepin_sensor.in_set(GameSet::Sensors));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tuning_defaults_match_gdd() {
        let t = PepinTuning::default();
        assert!(t.enabled);
        assert_eq!(t.max_stacks, 10, "GDD : jauge 0-10");
        assert_eq!(t.per_stack_damage, 0.02);
    }

    #[test]
    fn damage_mul_scales_with_stacks_for_pepin_only() {
        let t = PepinTuning::default();
        let mut c = PepinConfidence::default();
        assert_eq!(confidence_damage_mul(&c, &t, PEPIN_WEAPON), 1.0);
        c.stacks = 10;
        let full = confidence_damage_mul(&c, &t, PEPIN_WEAPON);
        assert!((full - 1.2).abs() < 1e-5, "+20 % à pleine confiance, got {full}");
        // Les autres armes ne profitent JAMAIS de la jauge.
        assert_eq!(confidence_damage_mul(&c, &t, WeaponType::Shotgun), 1.0);
    }

    #[test]
    fn damage_mul_neutral_when_disabled() {
        let t = PepinTuning {
            enabled: false,
            ..Default::default()
        };
        let c = PepinConfidence {
            stacks: 10,
            ..Default::default()
        };
        assert_eq!(confidence_damage_mul(&c, &t, PEPIN_WEAPON), 1.0);
    }

    /// Régression : la jauge ne bouge QUE sur les tirs de Pépin.
    #[test]
    fn confidence_updates_only_on_pepin_shots() {
        let mut app = App::new();
        app.init_resource::<PepinConfidence>()
            .init_resource::<PepinTuning>()
            .init_resource::<Time>()
            .add_message::<ShotResolved>()
            .add_systems(Update, sys_update_confidence);

        // 2 hits Pépin, 1 miss Pépin, 1 hit Shotgun (ignoré).
        for (weapon, hit) in [
            (PEPIN_WEAPON, true),
            (PEPIN_WEAPON, true),
            (WeaponType::Shotgun, true),
            (PEPIN_WEAPON, false),
        ] {
            app.world_mut().write_message(ShotResolved {
                weapon,
                hit_enemy: hit,
            });
        }
        app.update();

        let c = app.world().resource::<PepinConfidence>();
        assert_eq!(c.stacks, 1, "+1+1-1, le tir Shotgun n'a pas compté");
        assert_eq!(c.hits_run, 2);
        assert_eq!(c.misses_run, 1);
        assert_eq!(c.peak_run, 2);
    }
}
