//! ultimate.rs — État "Ultime" partagé (touche F).
//!
//! Design (validé 2026-06-30) : F n'est plus un cast instantané (cf. ancien
//! `shockwave.rs`) mais l'**entrée dans un état Ultime de 10 s** pendant lequel
//! l'arme équipée débloque sa **technique signature** (explosion Pépin, chaîne
//! électrique Bourrasque, perforation+poison Lenoir, gel de zone Pompe).
//!
//! Ce module ne porte QUE l'état (timer + cooldown + compteur sensor), neutre et
//! testable headless — calqué sur [`crate::combat_mods::PlayerCombatMods`].
//! L'input F (gaté par `AppMode::InGame` + run) et le branchement
//! technique-par-arme vivent dans `forgia-mode-roguelite` (cross-crate : combat
//! est dep root). Aucun hardcode gameplay enfoui : `duration`/`cooldown` sont des
//! champs (genome-ready), `Default` = miroir des constantes de référence.

use bevy::prelude::*;

/// Durée de référence de l'état Ultime (s) — fenêtre où la technique est active.
pub const ULTIMATE_DURATION_SECS: f32 = 10.0;
/// Cooldown de référence APRÈS la fin de l'Ultime (s) avant réactivation.
pub const ULTIMATE_COOLDOWN_SECS: f32 = 25.0;

/// État de l'Ultime joueur (touche F).
///
/// Cycle : `Prêt` → (F) `Actif` pendant `duration` → `Cooldown` pendant
/// `cooldown` → `Prêt`. Le verrou total entre deux activations = `duration +
/// cooldown` (le cd ne court qu'une fois l'Ultime terminé).
///
/// `Default` neutre = aucune activation, prêt à l'emploi. Tunables en champs pour
/// override genome ultérieur (story de suivi) sans toucher la logique.
#[derive(Resource, Debug, Clone, Copy)]
pub struct UltimateState {
    /// Durée d'un Ultime (s) — recopiée depuis le genome au load.
    pub duration: f32,
    /// Cooldown après la fin de l'Ultime (s).
    pub cooldown: f32,
    /// Temps restant d'Ultime ACTIF (s). `> 0` ⇒ techniques débloquées.
    pub active_left: f32,
    /// Temps restant avant de pouvoir réactiver (s). `> 0` ⇒ verrouillé.
    pub cd_left: f32,
    /// Nombre total d'activations (sensor / diagnostic).
    pub activations: u32,
}

impl Default for UltimateState {
    fn default() -> Self {
        Self {
            duration: ULTIMATE_DURATION_SECS,
            cooldown: ULTIMATE_COOLDOWN_SECS,
            active_left: 0.0,
            cd_left: 0.0,
            activations: 0,
        }
    }
}

impl UltimateState {
    /// `true` si l'Ultime est ACTIF (la technique de l'arme s'applique).
    pub fn is_active(&self) -> bool {
        self.active_left > 0.0
    }

    /// `true` si F peut activer l'Ultime maintenant (ni actif, ni en cooldown).
    pub fn can_activate(&self) -> bool {
        self.active_left <= 0.0 && self.cd_left <= 0.0
    }

    /// Fraction de progression de l'état courant pour le HUD :
    /// - en Ultime : `1.0 → 0.0` (jauge qui se vide pendant les 10 s),
    /// - en cooldown : `0.0 → 1.0` (jauge qui se recharge),
    /// - prêt : `1.0`.
    pub fn hud_fill(&self) -> f32 {
        if self.is_active() {
            (self.active_left / self.duration).clamp(0.0, 1.0)
        } else if self.cd_left > 0.0 {
            (1.0 - self.cd_left / self.cooldown).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }

    /// Tente d'activer l'Ultime. Retourne `true` si activé (sinon : déjà actif ou
    /// en cooldown). Verrou total = `duration + cooldown` ; le cd ne court qu'une
    /// fois l'Ultime terminé (d'où `cd_left = duration + cooldown`).
    pub fn try_activate(&mut self) -> bool {
        if !self.can_activate() {
            return false;
        }
        self.active_left = self.duration;
        self.cd_left = self.duration + self.cooldown;
        self.activations = self.activations.saturating_add(1);
        true
    }

    /// Avance les timers d'`dt` secondes (clampés à 0).
    pub fn tick(&mut self, dt: f32) {
        self.active_left = (self.active_left - dt).max(0.0);
        self.cd_left = (self.cd_left - dt).max(0.0);
    }

    /// Libellé d'état pour le sensor / HUD : `ready` / `active` / `cooldown`.
    pub fn state_str(&self) -> &'static str {
        if self.is_active() {
            "active"
        } else if self.cd_left > 0.0 {
            "cooldown"
        } else {
            "ready"
        }
    }

    /// Remet à neutre (OnExit Roguelite — comme `PlayerCombatMods::reset`).
    pub fn reset(&mut self) {
        let (d, c) = (self.duration, self.cooldown);
        *self = Self {
            duration: d,
            cooldown: c,
            ..Self::default()
        };
    }
}

/// Tick des timers de l'Ultime (GameSet::Combat). Léger, inconditionnel.
pub fn sys_tick_ultimate(time: Res<Time>, mut ult: ResMut<UltimateState>) {
    ult.tick(time.delta_secs());
}

// ─── Sensor forgia2_ultimate.json ───────────────────────────────────────────

const SENSOR_PATH: &str = "forgia2_ultimate.json";
const SENSOR_PERIOD_SECS: f32 = 1.0;

/// Écrit `forgia2_ultimate.json` 1Hz : état (ready/active/cooldown), timers,
/// nombre d'activations, remplissage HUD. Permet à un agent de diagnostiquer
/// l'Ultime sans lancer le rendu (point de validation T1b).
pub fn sys_write_ultimate_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    ult: Res<UltimateState>,
) {
    *accum += time.delta_secs();
    if *accum < SENSOR_PERIOD_SECS {
        return;
    }
    *accum = 0.0;

    let json = format!(
        r#"{{"id":"ultimate","severity":"ok","next_step":"","state":"{}","active":{},"active_left":{:.2},"cd_left":{:.2},"duration":{:.1},"cooldown":{:.1},"activations":{},"hud_fill":{:.3}}}"#,
        ult.state_str(),
        ult.is_active(),
        ult.active_left,
        ult.cd_left,
        ult.duration,
        ult.cooldown,
        ult.activations,
        ult.hud_fill(),
    );

    if let Err(e) = std::fs::write(SENSOR_PATH, &json) {
        warn!("[ultimate] sensor write failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_ready_and_neutral() {
        let u = UltimateState::default();
        assert!(u.can_activate());
        assert!(!u.is_active());
        assert_eq!(u.activations, 0);
        assert_eq!(u.duration, ULTIMATE_DURATION_SECS);
        assert_eq!(u.cooldown, ULTIMATE_COOLDOWN_SECS);
    }

    #[test]
    fn activate_sets_active_and_locks() {
        let mut u = UltimateState::default();
        assert!(u.try_activate());
        assert!(u.is_active());
        assert!(!u.can_activate(), "ne peut pas se réactiver pendant l'Ultime");
        assert_eq!(u.activations, 1);
        // Re-tenter pendant l'actif = refusé, pas de double comptage.
        assert!(!u.try_activate());
        assert_eq!(u.activations, 1);
    }

    #[test]
    fn active_window_lasts_duration_then_cooldown() {
        let mut u = UltimateState::default();
        u.try_activate();
        // Juste avant la fin de l'Ultime : encore actif.
        u.tick(ULTIMATE_DURATION_SECS - 0.1);
        assert!(u.is_active());
        assert!(!u.can_activate());
        // Après la fin de l'Ultime : plus actif, mais cooldown encore en cours.
        u.tick(0.2);
        assert!(!u.is_active());
        assert!(!u.can_activate(), "cooldown court après la fenêtre active");
        // Une fois le cooldown écoulé : de nouveau prêt.
        u.tick(ULTIMATE_COOLDOWN_SECS);
        assert!(u.can_activate());
    }

    #[test]
    fn total_lockout_is_duration_plus_cooldown() {
        let mut u = UltimateState::default();
        u.try_activate();
        // À duration + cooldown - epsilon : toujours verrouillé.
        u.tick(ULTIMATE_DURATION_SECS + ULTIMATE_COOLDOWN_SECS - 0.05);
        assert!(!u.can_activate());
        u.tick(0.1);
        assert!(u.can_activate());
    }

    #[test]
    fn hud_fill_drains_when_active_recharges_on_cd() {
        let mut u = UltimateState::default();
        assert_eq!(u.hud_fill(), 1.0, "prêt = plein");
        u.try_activate();
        assert!((u.hud_fill() - 1.0).abs() < 1e-3, "début Ultime = plein");
        u.tick(u.duration * 0.5);
        assert!((u.hud_fill() - 0.5).abs() < 1e-2, "mi-Ultime ≈ moitié");
        u.tick(u.duration * 0.5); // fin de l'Ultime → cooldown plein
        assert!(u.hud_fill() < 0.1, "début cooldown = quasi vide");
        u.tick(u.cooldown); // cooldown écoulé
        assert!((u.hud_fill() - 1.0).abs() < 1e-3, "cd fini = rechargé");
    }

    #[test]
    fn state_str_tracks_cycle() {
        let mut u = UltimateState::default();
        assert_eq!(u.state_str(), "ready");
        u.try_activate();
        assert_eq!(u.state_str(), "active");
        u.tick(u.duration); // fin de la fenêtre active → cooldown
        assert_eq!(u.state_str(), "cooldown");
        u.tick(u.cooldown);
        assert_eq!(u.state_str(), "ready");
    }

    #[test]
    fn reset_clears_timers_keeps_tunables() {
        let mut u = UltimateState::default();
        u.try_activate();
        u.tick(2.0);
        u.reset();
        assert!(u.can_activate());
        assert_eq!(u.activations, 0);
        assert_eq!(u.active_left, 0.0);
        assert_eq!(u.cd_left, 0.0);
    }
}
