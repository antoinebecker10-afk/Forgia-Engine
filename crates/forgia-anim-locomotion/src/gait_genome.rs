//! Gait genome — paramètres data-driven du cycle de marche, **per-personnage**.
//!
//! Story-579 incr.1 (audit anim 2026-06-07, finding #1 hardcode). Sort les 7 paires
//! WALK/RUN lerpées par [`crate::proc_walk::GaitTunables::for_speed`] du code Rust
//! vers `config/genomes/anim/gait_<id>.toml`. Permet d'animer un personnage aux
//! proportions/style différents **sans recompiler**.
//!
//! ## Zéro régression
//! Tous les défauts serde = miroir EXACT des const `proc_walk` au moment de l'extraction.
//! Un TOML absent/partiel → valeurs identiques au comportement hardcodé d'avant
//! (test `default_mirrors_proc_walk_consts`). Rex bouge à l'identique.
//!
//! ## Lecture hot-path (pattern story-576)
//! [`GaitTunables::for_speed`] est une fn PURE appelée par frame (3 systèmes) sans
//! accès ECS. Elle lit un global lazy-chargé via [`gait`] (RwLock, valeur `Copy`).
//! Chargement disque 1× au premier accès ; [`reload_gait`] permet la relecture
//! (Shift+F12 / re-enter = incr.2). Pas d'alloc, pas de lecture fichier par frame.
//!
//! ## Binding per-personnage (incr.1b — différé)
//! Aujourd'hui un seul global = le gait de Rex (`gait_biped_lizard.toml`), car seul
//! Rex est animé. Pour N personnages animés simultanément : registry par
//! `SkeletonTemplateId` + résolution au build du cache (comme `SkeletonTemplateRegistry`).

use serde::Deserialize;
use std::path::Path;
use std::sync::RwLock;

use crate::proc_walk;

/// Chemin du gait de Rex (biped lizard), relatif au cwd du jeu (racine workspace).
pub const GAIT_BIPED_LIZARD_PATH: &str = "config/genomes/anim/gait_biped_lizard.toml";

// ── Défauts serde = miroir EXACT des const proc_walk (zéro régression) ──────────
fn d_stride_walk() -> f32 { proc_walk::STRIDE_PER_M_WALK }
fn d_stride_run() -> f32 { proc_walk::STRIDE_PER_M_RUN }
fn d_stance_walk() -> f32 { proc_walk::STANCE_FRAC_WALK }
fn d_stance_run() -> f32 { proc_walk::STANCE_FRAC_RUN }
fn d_thigh_walk() -> f32 { proc_walk::AMP_THIGH_WALK }
fn d_thigh_run() -> f32 { proc_walk::AMP_THIGH_RUN }
fn d_arm_walk() -> f32 { proc_walk::AMP_ARM_WALK }
fn d_arm_run() -> f32 { proc_walk::AMP_ARM_RUN }
fn d_knee_walk() -> f32 { proc_walk::KNEE_FLEX_PEAK_WALK }
fn d_knee_run() -> f32 { proc_walk::KNEE_FLEX_PEAK_RUN }
fn d_pyaw_walk() -> f32 { proc_walk::PELVIC_YAW_AMP_WALK }
fn d_pyaw_run() -> f32 { proc_walk::PELVIC_YAW_AMP_RUN }
fn d_pbob_walk() -> f32 { proc_walk::PELVIC_BOB_AMP_WALK }
fn d_pbob_run() -> f32 { proc_walk::PELVIC_BOB_AMP_RUN }

/// Paires WALK/RUN du cycle de marche, lerpées par `for_speed`. Tous les champs ont
/// un défaut serde (miroir const) → un TOML partiel/absent ne casse jamais.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct GaitGenome {
    /// Cycles par mètre (cadence). Walk ~0.55, run ~0.40.
    #[serde(default = "d_stride_walk")] pub stride_per_m_walk: f32,
    #[serde(default = "d_stride_run")] pub stride_per_m_run: f32,
    /// Ratio stance/swing.
    #[serde(default = "d_stance_walk")] pub stance_frac_walk: f32,
    #[serde(default = "d_stance_run")] pub stance_frac_run: f32,
    /// Amplitude swing cuisse (rad).
    #[serde(default = "d_thigh_walk")] pub amp_thigh_walk: f32,
    #[serde(default = "d_thigh_run")] pub amp_thigh_run: f32,
    /// Amplitude swing bras / pitch épaule (rad).
    #[serde(default = "d_arm_walk")] pub amp_arm_walk: f32,
    #[serde(default = "d_arm_run")] pub amp_arm_run: f32,
    /// Flexion genou peak (rad). Anatomie : digitigrade ~92° vs plantigrade différent.
    #[serde(default = "d_knee_walk")] pub knee_flex_peak_walk: f32,
    #[serde(default = "d_knee_run")] pub knee_flex_peak_run: f32,
    /// Yaw bassin (rad).
    #[serde(default = "d_pyaw_walk")] pub pelvic_yaw_amp_walk: f32,
    #[serde(default = "d_pyaw_run")] pub pelvic_yaw_amp_run: f32,
    /// Bob vertical bassin (m).
    #[serde(default = "d_pbob_walk")] pub pelvic_bob_amp_walk: f32,
    #[serde(default = "d_pbob_run")] pub pelvic_bob_amp_run: f32,
}

impl Default for GaitGenome {
    fn default() -> Self {
        Self {
            stride_per_m_walk: d_stride_walk(), stride_per_m_run: d_stride_run(),
            stance_frac_walk: d_stance_walk(), stance_frac_run: d_stance_run(),
            amp_thigh_walk: d_thigh_walk(), amp_thigh_run: d_thigh_run(),
            amp_arm_walk: d_arm_walk(), amp_arm_run: d_arm_run(),
            knee_flex_peak_walk: d_knee_walk(), knee_flex_peak_run: d_knee_run(),
            pelvic_yaw_amp_walk: d_pyaw_walk(), pelvic_yaw_amp_run: d_pyaw_run(),
            pelvic_bob_amp_walk: d_pbob_walk(), pelvic_bob_amp_run: d_pbob_run(),
        }
    }
}

impl GaitGenome {
    /// Charge depuis le TOML, ou défaut (miroir const) si absent/invalide. Robuste :
    /// un TOML cassé logge un warn et tombe sur le défaut (pas de crash).
    pub fn load_or_default(path: impl AsRef<Path>) -> Self {
        let p = path.as_ref();
        match std::fs::read_to_string(p) {
            Ok(s) => match toml::from_str::<Self>(&s) {
                Ok(g) => {
                    bevy::log::info!(
                        "[gait-genome] chargé {} : stride={:.2}/{:.2} amp_thigh={:.2}/{:.2}",
                        p.display(), g.stride_per_m_walk, g.stride_per_m_run,
                        g.amp_thigh_walk, g.amp_thigh_run
                    );
                    g
                }
                Err(e) => {
                    bevy::log::warn!("[gait-genome] parse {} échoué ({e}) — défaut (miroir const)", p.display());
                    Self::default()
                }
            },
            Err(_) => {
                bevy::log::info!("[gait-genome] pas de {} — défaut (miroir const)", p.display());
                Self::default()
            }
        }
    }
}

// ── Global lazy-chargé (lecture hot-path sans alloc / sans fs par frame) ─────────
static GAIT: RwLock<Option<GaitGenome>> = RwLock::new(None);

/// Le gait courant (chargé du disque au 1er accès, puis mémorisé). `Copy` → retour
/// par valeur, pas d'alloc. Appelé par `for_speed` (fn pure hot-path).
///
/// Poison recovery (fix audit 2026-07-19) : un panic isolé pendant la section
/// critique empoisonnerait le lock POUR TOUJOURS → `.expect` = crash permanent
/// de l'anim sur toutes les frames suivantes. La donnée est `Copy` et toujours
/// cohérente (écriture atomique d'un `Option`) → récupérer via `into_inner`.
pub fn gait() -> GaitGenome {
    if let Some(g) = *GAIT.read().unwrap_or_else(|e| e.into_inner()) {
        return g;
    }
    let loaded = GaitGenome::load_or_default(GAIT_BIPED_LIZARD_PATH);
    *GAIT.write().unwrap_or_else(|e| e.into_inner()) = Some(loaded);
    loaded
}

/// Relit le TOML depuis le disque (hot-reload Shift+F12 / re-enter — câblé en incr.2).
pub fn reload_gait() {
    let g = GaitGenome::load_or_default(GAIT_BIPED_LIZARD_PATH);
    *GAIT.write().unwrap_or_else(|e| e.into_inner()) = Some(g);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Zéro régression : chaque défaut == la const proc_walk correspondante.
    #[test]
    fn default_mirrors_proc_walk_consts() {
        let g = GaitGenome::default();
        assert_eq!(g.stride_per_m_walk, proc_walk::STRIDE_PER_M_WALK);
        assert_eq!(g.stride_per_m_run, proc_walk::STRIDE_PER_M_RUN);
        assert_eq!(g.stance_frac_walk, proc_walk::STANCE_FRAC_WALK);
        assert_eq!(g.stance_frac_run, proc_walk::STANCE_FRAC_RUN);
        assert_eq!(g.amp_thigh_walk, proc_walk::AMP_THIGH_WALK);
        assert_eq!(g.amp_thigh_run, proc_walk::AMP_THIGH_RUN);
        assert_eq!(g.amp_arm_walk, proc_walk::AMP_ARM_WALK);
        assert_eq!(g.amp_arm_run, proc_walk::AMP_ARM_RUN);
        assert_eq!(g.knee_flex_peak_walk, proc_walk::KNEE_FLEX_PEAK_WALK);
        assert_eq!(g.knee_flex_peak_run, proc_walk::KNEE_FLEX_PEAK_RUN);
        assert_eq!(g.pelvic_yaw_amp_walk, proc_walk::PELVIC_YAW_AMP_WALK);
        assert_eq!(g.pelvic_yaw_amp_run, proc_walk::PELVIC_YAW_AMP_RUN);
        assert_eq!(g.pelvic_bob_amp_walk, proc_walk::PELVIC_BOB_AMP_WALK);
        assert_eq!(g.pelvic_bob_amp_run, proc_walk::PELVIC_BOB_AMP_RUN);
    }

    /// Un TOML partiel remplit le reste avec les défauts (miroir const).
    #[test]
    fn partial_toml_fills_defaults() {
        let g: GaitGenome = toml::from_str("amp_arm_walk = 0.9\n").unwrap();
        assert_eq!(g.amp_arm_walk, 0.9); // overridé
        assert_eq!(g.stride_per_m_walk, proc_walk::STRIDE_PER_M_WALK); // défaut
        assert_eq!(g.knee_flex_peak_run, proc_walk::KNEE_FLEX_PEAK_RUN); // défaut
    }

    /// Fichier absent → défaut (miroir const), pas de crash.
    #[test]
    fn missing_file_returns_default() {
        let g = GaitGenome::load_or_default("this/does/not/exist.toml");
        assert_eq!(g, GaitGenome::default());
    }
}
