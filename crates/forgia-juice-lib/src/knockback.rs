//! # knockback — poussée physique des ennemis à l'impact (story-650)
//!
//! Trick Vlambeer fondamental : chaque balle qui touche pousse l'ennemi dans la
//! direction du tir ; le kill projette plus fort (+ léger « pop » vertical).
//! Transmet la MASSE de l'arme : Boucherie projette, Pépin picote.
//!
//! **Contrainte Forgia** : les ennemis sont `RigidBody::KinematicPositionBased`
//! (waves.rs) → une `ExternalImpulse` Rapier est SANS EFFET. Le knockback est
//! donc un composant `Knockback` à vélocité décroissante qui déplace le
//! `Transform` chaque frame — ADDITIF, il compose avec le mouvement incrémental
//! des bots (forgia-ai-arena-bot) sans le combattre.
//!
//! Tuning genome plat `roguelite_gamefeel.toml` (hot-reload 1Hz, même fichier
//! que les paliers hitstop — watcher mtime séparé, doublon assumé et borné).
//! Capteur : `forgia2_knockback.json`.

use bevy::prelude::*;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

pub mod prelude {
    pub use super::{ForgiaJuiceKnockbackPlugin, Knockback, KnockbackStats, KnockbackTuning};
}

const GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_gamefeel.toml";
const SENSOR_PATH: &str = "forgia2_knockback.json";
const POLL_PERIOD_SEC: f32 = 1.0;

/// Taux de décroissance exponentielle de la vélocité (1/s) — forme de courbe,
/// pas du tuning gameplay : ~63 % du déplacement dans les 125 premières ms
/// (poussée sèche façon arcade, pas une glissade).
const DECAY_RATE: f32 = 8.0;
/// En-dessous de cette vitesse (m/s), le knockback est terminé (retrait composant).
const MIN_SPEED: f32 = 0.05;

/// Tuning knockback (genome plat hot-reload). Les déplacements sont exprimés en
/// MÈTRES totaux (intégrale de la vélocité décroissante) — intuitif à tuner.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct KnockbackTuning {
    /// Déplacement par hit (m), avant multiplicateur d'arme.
    pub base_m: f32,
    /// Multiplicateur au kill (le corps part en arrière).
    pub kill_mult: f32,
    /// Composante verticale ajoutée au kill (m) — le « pop ».
    pub pop_up_m: f32,
    /// Déplacement cumulé max (m) — anti-éjection hors arène (full-auto stack).
    pub max_m: f32,
    /// Multiplicateurs par arme (Pépin picote, Boucherie projette).
    pub mult_pepin: f32,
    pub mult_bourrasque: f32,
    pub mult_lenoir: f32,
    pub mult_boucherie: f32,
    pub mult_default: f32,
}

impl Default for KnockbackTuning {
    fn default() -> Self {
        Self {
            base_m: 0.30,
            kill_mult: 3.0,
            pop_up_m: 0.25,
            max_m: 2.5,
            mult_pepin: 0.6,
            mult_bourrasque: 0.45,
            mult_lenoir: 1.6,
            mult_boucherie: 2.2,
            mult_default: 1.0,
        }
    }
}

#[derive(Deserialize)]
struct GeneToml {
    id: String,
    #[serde(default)]
    default: f32,
}

#[derive(Deserialize)]
struct GamefeelGenomeToml {
    #[serde(default)]
    genes: Vec<GeneToml>,
}

impl KnockbackTuning {
    /// Pur — genes plats, ids préfixés `knockback_`.
    pub fn parse_toml(content: &str) -> Self {
        let parsed: GamefeelGenomeToml = match toml::from_str(content) {
            Ok(v) => v,
            Err(_) => return Self::default(),
        };
        let mut t = Self::default();
        for gene in &parsed.genes {
            let v = gene.default;
            match gene.id.as_str() {
                "knockback_base_m" => t.base_m = v.clamp(0.0, 2.0),
                "knockback_kill_mult" => t.kill_mult = v.clamp(1.0, 8.0),
                "knockback_pop_up_m" => t.pop_up_m = v.clamp(0.0, 1.5),
                "knockback_max_m" => t.max_m = v.clamp(0.5, 8.0),
                "knockback_mult_pepin" => t.mult_pepin = v.clamp(0.0, 5.0),
                "knockback_mult_bourrasque" => t.mult_bourrasque = v.clamp(0.0, 5.0),
                "knockback_mult_lenoir" => t.mult_lenoir = v.clamp(0.0, 5.0),
                "knockback_mult_boucherie" => t.mult_boucherie = v.clamp(0.0, 5.0),
                "knockback_mult_default" => t.mult_default = v.clamp(0.0, 5.0),
                _ => {}
            }
        }
        t
    }

    fn load_or_default() -> Self {
        match fs::read_to_string(PathBuf::from(GENOME_PATH)) {
            Ok(content) => Self::parse_toml(&content),
            Err(_) => Self::default(),
        }
    }
}

/// Composant : vélocité de poussée décroissante, appliquée au `Transform`.
/// `vel` initial = déplacement_total (m) × `DECAY_RATE` (intégrale exp = disp).
#[derive(Component, Debug, Clone, Copy)]
pub struct Knockback {
    pub vel: Vec3,
}

/// Pur — vélocité initiale d'un hit : direction horizontale × déplacement voulu,
/// converti en vélocité (× DECAY_RATE), + pop vertical au kill.
pub fn knockback_velocity(
    dir_horizontal: Vec3,
    tuning: &KnockbackTuning,
    weapon_mult: f32,
    is_kill: bool,
) -> Vec3 {
    let flat = Vec3::new(dir_horizontal.x, 0.0, dir_horizontal.z).normalize_or_zero();
    let kill_factor = if is_kill { tuning.kill_mult } else { 1.0 };
    let disp = tuning.base_m * weapon_mult.max(0.0) * kill_factor;
    let mut v = flat * disp * DECAY_RATE;
    if is_kill {
        v.y = tuning.pop_up_m * DECAY_RATE;
    }
    v
}

/// Pur — déplacement total (m) qu'une vélocité de knockback produira
/// (intégrale de la décroissance exp). Pour capteurs/logs.
pub fn displacement_m(vel: Vec3) -> f32 {
    vel.length() / DECAY_RATE
}

/// Pur — cumule un nouveau knockback sur un existant, borné par `max_m`
/// (anti-éjection en full-auto : la vitesse cumulée équivaut à ≤ max_m de
/// déplacement restant).
pub fn accumulate_knockback(current: Vec3, added: Vec3, tuning: &KnockbackTuning) -> Vec3 {
    let sum = current + added;
    let max_speed = tuning.max_m * DECAY_RATE;
    sum.clamp_length_max(max_speed)
}

/// Compteurs capteur.
#[derive(Resource, Default, Debug)]
pub struct KnockbackStats {
    pub pushes: u64,
    pub kill_pushes: u64,
    pub last_displacement_m: f32,
}

/// Suivi mtime genome.
#[derive(Resource, Default)]
pub struct KnockbackGenomeWatch {
    pub last_mtime: Option<SystemTime>,
    pub reload_count: u32,
}

pub struct ForgiaJuiceKnockbackPlugin;

impl Plugin for ForgiaJuiceKnockbackPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<KnockbackStats>()
            .add_systems(Startup, sys_init_knockback_genome)
            .add_systems(
                Update,
                (
                    sys_tick_knockback,
                    sys_hot_reload_knockback_genome,
                    sys_write_knockback_sensor,
                ),
            );
    }
}

/// Déplace les entités poussées + décroissance exp + retrait quand fini.
/// `Res<Time>` (virtuel) → gèle en pause et pendant le hitstop (relâche
/// dramatique après le freeze du kill — voulu).
fn sys_tick_knockback(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Transform, &mut Knockback)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let decay = (-DECAY_RATE * dt).exp();
    for (e, mut tf, mut kb) in &mut q {
        tf.translation += kb.vel * dt;
        kb.vel *= decay;
        if kb.vel.length_squared() < MIN_SPEED * MIN_SPEED {
            commands.entity(e).remove::<Knockback>();
        }
    }
}

fn sys_init_knockback_genome(mut commands: Commands) {
    let tuning = KnockbackTuning::load_or_default();
    let mtime = fs::metadata(GENOME_PATH).and_then(|m| m.modified()).ok();
    commands.insert_resource(tuning);
    commands.insert_resource(KnockbackGenomeWatch {
        last_mtime: mtime,
        ..Default::default()
    });
    info!(
        "[juice-knockback] genome loaded — base {:.2}m kill×{:.1} pop {:.2}m (pépin×{:.2} boucherie×{:.2})",
        tuning.base_m, tuning.kill_mult, tuning.pop_up_m, tuning.mult_pepin, tuning.mult_boucherie
    );
}

fn sys_hot_reload_knockback_genome(
    time: Res<Time>,
    mut accum: Local<f32>,
    tuning: Option<ResMut<KnockbackTuning>>,
    watch: Option<ResMut<KnockbackGenomeWatch>>,
) {
    let (Some(mut tuning), Some(mut watch)) = (tuning, watch) else {
        return;
    };
    *accum += time.delta_secs();
    if *accum < POLL_PERIOD_SEC {
        return;
    }
    *accum = 0.0;
    let Ok(meta) = fs::metadata(GENOME_PATH) else {
        return;
    };
    let Ok(mtime) = meta.modified() else {
        return;
    };
    if watch.last_mtime == Some(mtime) {
        return;
    }
    let Ok(content) = fs::read_to_string(GENOME_PATH) else {
        return;
    };
    let new_tuning = KnockbackTuning::parse_toml(&content);
    watch.last_mtime = Some(mtime);
    if new_tuning != *tuning {
        *tuning = new_tuning;
        watch.reload_count = watch.reload_count.saturating_add(1);
        info!(
            "[juice-knockback] HOT-RELOADED — base {:.2}m kill×{:.1}",
            new_tuning.base_m, new_tuning.kill_mult
        );
    }
}

fn sys_write_knockback_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    stats: Option<Res<KnockbackStats>>,
    tuning: Option<Res<KnockbackTuning>>,
    watch: Option<Res<KnockbackGenomeWatch>>,
    q_active: Query<(), With<Knockback>>,
) {
    *accum += time.delta_secs();
    if *accum < POLL_PERIOD_SEC {
        return;
    }
    *accum = 0.0;
    let (Some(stats), Some(tuning), Some(watch)) = (stats, tuning, watch) else {
        return;
    };
    let json = format!(
        r#"{{"id":"knockback","severity":"ok","next_step":"Tuning dans roguelite_gamefeel.toml (knockback_*, hot-reload 1Hz).","timestamp_secs":{:.1},"pushes":{},"kill_pushes":{},"active_now":{},"last_displacement_m":{:.2},"base_m":{:.2},"kill_mult":{:.1},"pop_up_m":{:.2},"max_m":{:.1},"reload_count":{}}}"#,
        time.elapsed_secs(),
        stats.pushes,
        stats.kill_pushes,
        q_active.iter().count(),
        stats.last_displacement_m,
        tuning.base_m,
        tuning.kill_mult,
        tuning.pop_up_m,
        tuning.max_m,
        watch.reload_count,
    );
    if let Err(e) = forgia_core::sensor_io::enqueue(SENSOR_PATH, json) {
        warn!("[juice-knockback] sensor write failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_pushes_horizontally_no_vertical() {
        let t = KnockbackTuning::default();
        let v = knockback_velocity(Vec3::new(0.3, 0.5, -0.8), &t, 1.0, false);
        assert_eq!(v.y, 0.0, "pas de pop vertical sur simple hit");
        assert!(v.length() > 0.0);
        // Déplacement total = |v| / DECAY_RATE = base_m × mult.
        assert!((v.length() / DECAY_RATE - t.base_m).abs() < 1e-5);
    }

    #[test]
    fn kill_pushes_harder_and_pops_up() {
        let t = KnockbackTuning::default();
        let dir = Vec3::NEG_Z;
        let hit = knockback_velocity(dir, &t, 1.0, false);
        let kill = knockback_velocity(dir, &t, 1.0, true);
        assert!(kill.length() > hit.length(), "kill > hit");
        assert!(kill.y > 0.0, "pop vertical au kill");
        assert!((kill.y / DECAY_RATE - t.pop_up_m).abs() < 1e-5);
    }

    #[test]
    fn weapon_mult_scales_push() {
        let t = KnockbackTuning::default();
        let dir = Vec3::NEG_Z;
        let pepin = knockback_velocity(dir, &t, t.mult_pepin, false);
        let boucherie = knockback_velocity(dir, &t, t.mult_boucherie, false);
        assert!(
            boucherie.length() > 2.0 * pepin.length(),
            "Boucherie doit PROJETER, Pépin picoter"
        );
    }

    #[test]
    fn accumulation_is_capped() {
        let t = KnockbackTuning::default();
        let big = Vec3::NEG_Z * t.max_m * DECAY_RATE * 3.0;
        let sum = accumulate_knockback(big, big, &t);
        assert!(
            sum.length() <= t.max_m * DECAY_RATE + 1e-4,
            "cumul borné par max_m (anti-éjection)"
        );
    }

    #[test]
    fn zero_direction_is_safe() {
        let t = KnockbackTuning::default();
        let v = knockback_velocity(Vec3::ZERO, &t, 1.0, false);
        assert_eq!(v, Vec3::ZERO);
    }

    #[test]
    fn parse_overrides_and_clamps() {
        let toml = r#"
[[genes]]
id = "knockback_base_m"
default = 0.5
[[genes]]
id = "knockback_kill_mult"
default = 99.0
"#;
        let t = KnockbackTuning::parse_toml(toml);
        assert!((t.base_m - 0.5).abs() < 1e-6);
        assert!((t.kill_mult - 8.0).abs() < 1e-6, "clamp 99 -> 8");
        assert!((t.mult_boucherie - 2.2).abs() < 1e-6, "absent = défaut");
    }

    #[test]
    fn parse_empty_returns_default() {
        assert_eq!(KnockbackTuning::parse_toml(""), KnockbackTuning::default());
    }
}
