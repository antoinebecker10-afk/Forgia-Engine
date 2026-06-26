//! progress.rs — Progression JOUEUR (niveau + XP), P4 (design home-hub 2026-06-26).
//!
//! XP de **participation** gagnée à CHAQUE fin de run, même en défaite (« toute run
//! paie ») : base fixe + temps de survie. Distincte des Âmes (méta-monnaie de
//! l'Enclume). Chaque montée de niveau octroie un **point de talent** (dépensé en P5).
//!
//! Auto-contenu : save séparée `player_progress_save.toml` (pattern `identity.rs` /
//! `meta_shop.rs`), courbe simple croissante, persistée à chaque fin de run.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::run::{RunState, RunTimer};

const SAVE_FILE: &str = "player_progress_save.toml";
const SAVE_VERSION: u32 = 1;

/// XP de base versée à chaque run terminée (participation — « toute run paie »).
/// Balance — à externaliser en genome plus tard (cf `run::SOULS_PER_BOSS`).
const RUN_BASE_XP: u32 = 40;

/// Progression persistée du joueur : niveau + XP vers le niveau suivant + points de
/// talent non dépensés (P5). Distincte de `MetaSouls` (méta-monnaie).
#[derive(Resource, Serialize, Deserialize, Clone, Debug)]
pub struct PlayerProgress {
    pub version: u32,
    pub level: u32,
    /// XP accumulée VERS le niveau suivant (pas un total cumulé).
    pub xp: u32,
    /// Points de talent non dépensés (1 par niveau ; dépensés en P5).
    pub talent_points: u32,
}

impl Default for PlayerProgress {
    fn default() -> Self {
        Self {
            version: SAVE_VERSION,
            level: 1,
            xp: 0,
            talent_points: 0,
        }
    }
}

impl PlayerProgress {
    /// XP requise pour passer du niveau `level` au suivant (courbe douce croissante).
    pub fn xp_to_next(level: u32) -> u32 {
        80 + level.saturating_sub(1) * 40
    }

    /// Ajoute de l'XP et gère les montées de niveau (+1 point de talent par niveau).
    /// Retourne le nombre de niveaux gagnés (pour le feedback / juice futur).
    pub fn add_xp(&mut self, amount: u32) -> u32 {
        self.xp = self.xp.saturating_add(amount);
        let mut gained = 0;
        while self.xp >= Self::xp_to_next(self.level) {
            self.xp -= Self::xp_to_next(self.level);
            self.level = self.level.saturating_add(1);
            self.talent_points = self.talent_points.saturating_add(1);
            gained += 1;
        }
        gained
    }

    /// Fraction [0,1] de la barre d'XP vers le niveau suivant.
    pub fn xp_fraction(&self) -> f32 {
        let need = Self::xp_to_next(self.level).max(1);
        (self.xp as f32 / need as f32).clamp(0.0, 1.0)
    }
}

// ── Persistence (pattern identity/meta_shop) ─────────────────────────────────

/// Walk-up depuis l'exe pour trouver `config/` (marqueur `config/biomes/`),
/// fallback `config/`. Réplique locale (évite une dépendance crate).
fn config_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        let mut cursor: Option<&Path> = exe.parent();
        while let Some(d) = cursor {
            if d.join("config").join("biomes").exists() {
                return d.join("config");
            }
            cursor = d.parent();
        }
    }
    PathBuf::from("config")
}

impl PlayerProgress {
    fn save_path() -> PathBuf {
        config_dir().join(SAVE_FILE)
    }

    fn load_or_default() -> Self {
        match std::fs::read_to_string(Self::save_path()) {
            Ok(c) => toml::from_str(&c).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    fn save(&self) {
        crate::persist::save_toml_atomic(&Self::save_path(), self, "player-progress");
    }
}

// ── Systèmes ─────────────────────────────────────────────────────────────────

/// Startup — charge la progression disque (1×).
fn sys_load_progress(mut commands: Commands) {
    let progress = PlayerProgress::load_or_default();
    info!(
        "[progress] chargé — niveau {} ({} XP, {} pts talent)",
        progress.level, progress.xp, progress.talent_points
    );
    commands.insert_resource(progress);
}

/// Fin de run (Defeat/Victory) → XP de participation (base + temps de survie),
/// même en défaite. Persiste. RunTimer.secs = durée de la run (reset au start).
fn sys_award_run_xp(mut progress: ResMut<PlayerProgress>, timer: Option<Res<RunTimer>>) {
    let secs = timer.as_ref().map(|t| t.secs.max(0.0) as u32).unwrap_or(0);
    let xp = RUN_BASE_XP + secs;
    let gained = progress.add_xp(xp);
    progress.save();
    info!(
        "[progress] +{xp} XP (run {secs}s) → niveau {} ({} pts talent){}",
        progress.level,
        progress.talent_points,
        if gained > 0 {
            format!(" — +{gained} niveau(x) !")
        } else {
            String::new()
        }
    );
}

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct PlayerProgressPlugin;

impl Plugin for PlayerProgressPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, sys_load_progress)
            // XP versée à chaque fin de run (les deux issues — « toute run paie »).
            .add_systems(OnEnter(RunState::Defeat), sys_award_run_xp)
            .add_systems(OnEnter(RunState::Victory), sys_award_run_xp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_starts_level_1() {
        let p = PlayerProgress::default();
        assert_eq!(p.level, 1);
        assert_eq!(p.xp, 0);
        assert_eq!(p.talent_points, 0);
    }

    #[test]
    fn add_xp_levels_up_and_grants_talent_points() {
        let mut p = PlayerProgress::default();
        // Niveau 1→2 requiert 80 XP.
        let gained = p.add_xp(80);
        assert_eq!(gained, 1);
        assert_eq!(p.level, 2);
        assert_eq!(p.xp, 0);
        assert_eq!(p.talent_points, 1);
    }

    #[test]
    fn add_xp_handles_multiple_levels() {
        let mut p = PlayerProgress::default();
        // 80 (→2) + 120 (→3) = 200 XP d'un coup → 2 niveaux.
        let gained = p.add_xp(200);
        assert_eq!(gained, 2);
        assert_eq!(p.level, 3);
        assert_eq!(p.talent_points, 2);
    }

    #[test]
    fn xp_fraction_in_range() {
        let mut p = PlayerProgress::default();
        p.xp = 40; // 40/80 = 0.5
        assert!((p.xp_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn xp_curve_increases_with_level() {
        assert!(PlayerProgress::xp_to_next(2) > PlayerProgress::xp_to_next(1));
    }
}
