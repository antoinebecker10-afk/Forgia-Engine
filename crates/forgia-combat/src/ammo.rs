//! # Ammo state machine — per-weapon clip + reserve + reload
//!
//! Story-455 Phase A (2026-05-18). Remplace l'infinite-ammo stub V1
//! (`EquippedWeapons.ammo_rifle = 999` no-op) par un vrai système gameplay :
//!
//! - `AmmoSlot` : état mémoire par arme (current_mag, reserve, reload_state).
//! - `ReloadKind` : `Mag` (rifle/shotgun batch) ou `ShellPerShell` (pump).
//! - `ReloadState` : `Idle` ou `Reloading { remaining_secs, kind }`.
//! - `AmmoChanged` Message : event-driven UI consumer (HUD ammo, sensor).
//! - `AmmoConfig` : snapshot genome (mag_size, reserve_max, reload_time, kind, infinite).
//!
//! Pas de hardcode : valeurs viennent de `ViewmodelGenomeEntry` (assets/genomes/viewmodel_arena.toml).
//! Le producteur des `AmmoConfig` vit dans `forgia-fps` (qui parse le genome) et les
//! pousse via [`sync_ammo_slot_from_config`] / [`AmmoSlot::apply_config`].

use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use crate::weapons::WeaponType;

// ─── Types publics ─────────────────────────────────────────────────────────

/// Manière de recharger (depuis genome `reload_kind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReloadKind {
    /// Mag entière : à la fin du timer, transfert `min(mag_size - current, reserve)`.
    /// Rifle/SMG/sniper. Cancelable mais sans progrès partiel.
    #[default]
    Mag,
    /// Une cartouche à la fois : timer court répété, interrompable à tout shot.
    /// Pump shotgun (Boucherie). Tirer interrompt le reload entre 2 shells.
    ShellPerShell,
}

impl ReloadKind {
    /// Parse depuis string genome. Inconnu → fallback Mag + warn.
    pub fn from_genome_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "mag" => ReloadKind::Mag,
            "shell_per_shell" | "shell" | "pump" => ReloadKind::ShellPerShell,
            other => {
                warn!("[ammo] reload_kind inconnu '{}' — fallback Mag", other);
                ReloadKind::Mag
            }
        }
    }
}

/// État courant du reload.
#[derive(Debug, Clone, Copy, Default)]
pub enum ReloadState {
    #[default]
    Idle,
    Reloading {
        remaining_secs: f32,
        kind: ReloadKind,
    },
}

impl ReloadState {
    pub fn is_reloading(&self) -> bool {
        matches!(self, ReloadState::Reloading { .. })
    }
    /// Progress [0..1] inverse du temps restant. 0 = juste commencé, 1 = fini.
    /// Hors Reloading → 0.
    pub fn progress(&self, total_secs: f32) -> f32 {
        match *self {
            ReloadState::Reloading { remaining_secs, .. } if total_secs > 0.0 => {
                (1.0 - (remaining_secs / total_secs)).clamp(0.0, 1.0)
            }
            _ => 0.0,
        }
    }
}

/// Snapshot des valeurs ammo issues du genome viewmodel. Producteur = `forgia-fps`
/// (qui parse `ViewmodelGenomeEntry`). Consommateur = `AmmoSlot::apply_config`.
#[derive(Debug, Clone, Copy)]
pub struct AmmoConfig {
    pub mag_size: u32,
    pub reserve_max: u32,
    pub reload_time_secs: f32,
    pub reload_kind: ReloadKind,
    pub infinite_ammo: bool,
    pub low_ammo_threshold: f32,
}

impl Default for AmmoConfig {
    fn default() -> Self {
        Self {
            mag_size: 30,
            reserve_max: 120,
            reload_time_secs: 1.8,
            reload_kind: ReloadKind::Mag,
            infinite_ammo: false,
            low_ammo_threshold: 0.25,
        }
    }
}

/// Per-weapon ammo state. Stocké dans `EquippedWeapons.slots`.
#[derive(Debug, Clone, Copy)]
pub struct AmmoSlot {
    pub current_mag: u32,
    pub reserve: u32,
    pub reload_state: ReloadState,
    pub config: AmmoConfig,
}

impl Default for AmmoSlot {
    fn default() -> Self {
        let config = AmmoConfig::default();
        Self {
            current_mag: config.mag_size,
            reserve: config.reserve_max,
            reload_state: ReloadState::Idle,
            config,
        }
    }
}

impl AmmoSlot {
    /// Construit un slot plein depuis une config.
    pub fn full_from_config(config: AmmoConfig) -> Self {
        Self {
            current_mag: config.mag_size,
            reserve: config.reserve_max,
            reload_state: ReloadState::Idle,
            config,
        }
    }

    /// Synchronise un slot existant avec une nouvelle config (hot-reload genome).
    /// - Clamp `current_mag` au nouveau `mag_size`.
    /// - Clamp `reserve` au nouveau `reserve_max`.
    /// - Si reloading en cours et `reload_time_secs` changé, scale `remaining_secs`
    ///   proportionnellement pour ne pas casser l'animation.
    pub fn apply_config(&mut self, new_config: AmmoConfig) {
        let old_reload_total = self.config.reload_time_secs.max(0.001);
        if let ReloadState::Reloading {
            remaining_secs,
            kind,
        } = &mut self.reload_state
        {
            let new_reload_total = new_config.reload_time_secs.max(0.001);
            if (old_reload_total - new_reload_total).abs() > f32::EPSILON {
                let progress = 1.0 - (*remaining_secs / old_reload_total).clamp(0.0, 1.0);
                *remaining_secs = new_reload_total * (1.0 - progress);
            }
            // Si le genome a changé le kind en plein reload, on conserve l'ancien kind
            // jusqu'à la prochaine reload pour cohérence visuelle.
            let _ = kind; // suppress unused, on conserve volontairement
        }
        self.current_mag = self.current_mag.min(new_config.mag_size);
        self.reserve = self.reserve.min(new_config.reserve_max);
        self.config = new_config;
    }

    /// True si on peut tirer (mag non vide ou infinite).
    pub fn can_fire(&self) -> bool {
        self.config.infinite_ammo || self.current_mag > 0
    }

    /// Consomme 1 shot (no-op si infinite). Retourne true si shot consommé.
    /// **N'émet PAS d'event** : c'est le caller (`fire_weapon_minimal`) qui décide
    /// d'émettre `AmmoChanged` pour batcher avec multi-pellets.
    pub fn consume_shot(&mut self) -> bool {
        if self.config.infinite_ammo {
            return true;
        }
        if self.current_mag == 0 {
            return false;
        }
        self.current_mag -= 1;
        true
    }

    /// Démarre un reload si pas déjà en cours et si utile (mag non plein + reserve > 0).
    /// Retourne true si reload effectivement démarré.
    pub fn try_start_reload(&mut self) -> bool {
        self.try_start_reload_at_speed(1.0)
    }

    /// Idem, mais à une vitesse de rechargement donnée (`> 1` = plus rapide).
    ///
    /// 2026-08-04 — les atouts d'**entretien** passent par ici. Le multiplicateur
    /// n'est PAS écrit dans `config` : celui-ci est le miroir du génome et se
    /// fait réécrire à chaque hot-reload, ce qui effacerait le bonus en silence.
    /// Il s'applique au moment où le timer démarre, là où il est observable.
    ///
    /// Borné à `0.1` en bas : une vitesse nulle ou négative gèlerait le
    /// rechargement pour toujours, et un atout ne doit jamais pouvoir casser
    /// l'arme qu'il améliore.
    pub fn try_start_reload_at_speed(&mut self, speed_mul: f32) -> bool {
        if self.config.infinite_ammo {
            return false;
        }
        if self.reload_state.is_reloading() {
            return false;
        }
        if self.current_mag >= self.config.mag_size {
            return false; // mag déjà plein
        }
        if self.reserve == 0 {
            return false; // pas de munition en réserve
        }
        self.reload_state = ReloadState::Reloading {
            remaining_secs: self.config.reload_time_secs / speed_mul.max(0.1),
            kind: self.config.reload_kind,
        };
        true
    }

    /// Cancel le reload en cours (weapon switch / mort).
    pub fn cancel_reload(&mut self) {
        self.reload_state = ReloadState::Idle;
    }

    /// Tick une frame de reload. Retourne `Some(AmmoChangeKind::Reload {..})` si transfer
    /// de munitions eu lieu cette frame, `None` sinon.
    pub fn tick_reload(&mut self, dt: f32) -> Option<AmmoChangeKind> {
        let ReloadState::Reloading {
            remaining_secs,
            kind,
        } = &mut self.reload_state
        else {
            return None;
        };
        *remaining_secs -= dt;
        if *remaining_secs > 0.0 {
            return None;
        }
        // Reload tick complete.
        let kind = *kind;
        self.reload_state = ReloadState::Idle;
        match kind {
            ReloadKind::Mag => {
                let needed = self.config.mag_size.saturating_sub(self.current_mag);
                let transferred = needed.min(self.reserve);
                self.current_mag += transferred;
                self.reserve -= transferred;
                Some(AmmoChangeKind::Reload { transferred })
            }
            ReloadKind::ShellPerShell => {
                let transferred = if self.reserve > 0 { 1 } else { 0 };
                if transferred > 0 {
                    self.current_mag += 1;
                    self.reserve -= 1;
                }
                // Si on n'a pas fini, on redémarre un cycle.
                if self.current_mag < self.config.mag_size && self.reserve > 0 {
                    self.reload_state = ReloadState::Reloading {
                        remaining_secs: self.config.reload_time_secs,
                        kind: ReloadKind::ShellPerShell,
                    };
                }
                Some(AmmoChangeKind::Reload { transferred })
            }
        }
    }

    /// Pickup munitions (futur loot system).
    pub fn pickup(&mut self, amount: u32) -> u32 {
        let before = self.reserve;
        self.reserve = (self.reserve + amount).min(self.config.reserve_max);
        self.reserve - before
    }

    /// Fraction `current_mag / mag_size` ∈ [0, 1]. 0 si mag_size = 0.
    pub fn mag_fraction(&self) -> f32 {
        if self.config.mag_size == 0 {
            return 0.0;
        }
        self.current_mag as f32 / self.config.mag_size as f32
    }

    /// True si en seuil low-ammo (genome-driven threshold).
    pub fn is_low(&self) -> bool {
        !self.config.infinite_ammo && self.mag_fraction() <= self.config.low_ammo_threshold
    }
}

// ─── Event ─────────────────────────────────────────────────────────────────

/// Nature de la mutation ammo. Consommé par HUD, sensors, audio (futur).
#[derive(Debug, Clone, Copy)]
pub enum AmmoChangeKind {
    /// N shots consommés ce frame (multi-pellets batché en 1 event = 1).
    /// shots = nombre de pulls de gâchette (pas de pellets).
    Fire { shots: u32 },
    /// Reload tick complete (mag→reserve transfer ou shell-per-shell +1).
    Reload { transferred: u32 },
    /// Switch arme → snapshot état nouvelle arme.
    WeaponSwitch,
    /// Pickup loot world (futur).
    Pickup { amount: u32 },
    /// Genome hot-reload (clamp values).
    GenomeApplied,
}

/// Event émis dès qu'un `AmmoSlot` change. Consommateurs : forgia-ui-hud-ammo (Phase B),
/// sensor ammo (Phase A), audio (futur), achievements (futur).
#[derive(Message, Debug, Clone, Copy)]
pub struct AmmoChanged {
    pub weapon: WeaponType,
    pub current_mag: u32,
    pub reserve: u32,
    pub mag_size: u32,
    pub kind: AmmoChangeKind,
    pub is_low: bool,
    pub is_reloading: bool,
}

impl AmmoChanged {
    pub fn snapshot(weapon: WeaponType, slot: &AmmoSlot, kind: AmmoChangeKind) -> Self {
        Self {
            weapon,
            current_mag: slot.current_mag,
            reserve: slot.reserve,
            mag_size: slot.config.mag_size,
            kind,
            is_low: slot.is_low(),
            is_reloading: slot.reload_state.is_reloading(),
        }
    }
}

// ─── Helper : sync slot depuis config + emit event ─────────────────────────

/// Pousse une config dans le slot d'une arme. Émet `AmmoChanged::GenomeApplied`. Idempotent.
///
/// - **Slot absent** : crée un slot plein (`full_from_config`) avec la config genome.
///   → `current_mag = mag_size`, `reserve = reserve_max`, état Idle.
/// - **Slot existant** (hot-reload) : `apply_config` clamp values + scale in-progress reload.
///   → préserve `current_mag` (clampé), `reserve` (clampé), `reload_state`.
pub fn sync_ammo_slot_from_config(
    slots: &mut HashMap<WeaponType, AmmoSlot>,
    weapon: WeaponType,
    config: AmmoConfig,
    events: &mut MessageWriter<AmmoChanged>,
) {
    let slot = match slots.entry(weapon) {
        bevy::platform::collections::hash_map::Entry::Occupied(mut entry) => {
            entry.get_mut().apply_config(config);
            entry.into_mut()
        }
        bevy::platform::collections::hash_map::Entry::Vacant(entry) => {
            entry.insert(AmmoSlot::full_from_config(config))
        }
    };
    events.write(AmmoChanged::snapshot(
        weapon,
        slot,
        AmmoChangeKind::GenomeApplied,
    ));
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(mag: u32, reserve: u32, reload_t: f32) -> AmmoConfig {
        AmmoConfig {
            mag_size: mag,
            reserve_max: reserve,
            reload_time_secs: reload_t,
            reload_kind: ReloadKind::Mag,
            infinite_ammo: false,
            low_ammo_threshold: 0.25,
        }
    }

    #[test]
    fn fresh_slot_full_mag_and_reserve() {
        let s = AmmoSlot::full_from_config(cfg(30, 120, 1.8));
        assert_eq!(s.current_mag, 30);
        assert_eq!(s.reserve, 120);
        assert!(s.can_fire());
        assert!(!s.is_low());
    }

    #[test]
    fn consume_until_empty_blocks_fire() {
        let mut s = AmmoSlot::full_from_config(cfg(3, 10, 1.0));
        assert!(s.consume_shot());
        assert!(s.consume_shot());
        assert!(s.consume_shot());
        assert_eq!(s.current_mag, 0);
        assert!(!s.can_fire());
        assert!(!s.consume_shot());
    }

    #[test]
    fn infinite_ammo_never_decrements() {
        let mut c = cfg(5, 10, 1.0);
        c.infinite_ammo = true;
        let mut s = AmmoSlot::full_from_config(c);
        for _ in 0..100 {
            assert!(s.consume_shot());
        }
        assert_eq!(s.current_mag, 5);
        assert!(s.can_fire());
    }

    #[test]
    fn mag_reload_transfers_from_reserve() {
        let mut s = AmmoSlot::full_from_config(cfg(10, 30, 1.0));
        for _ in 0..7 {
            s.consume_shot();
        }
        assert_eq!(s.current_mag, 3);
        assert!(s.try_start_reload());
        s.tick_reload(0.5); // mid reload
        assert!(s.reload_state.is_reloading());
        assert_eq!(s.current_mag, 3); // pas encore appliqué
        let change = s.tick_reload(0.6); // fini (0.5 + 0.6 > 1.0)
        assert!(matches!(
            change,
            Some(AmmoChangeKind::Reload { transferred: 7 })
        ));
        assert_eq!(s.current_mag, 10);
        assert_eq!(s.reserve, 23);
        assert!(!s.reload_state.is_reloading());
    }

    #[test]
    fn shell_per_shell_loops_until_full() {
        let mut c = cfg(4, 10, 0.3);
        c.reload_kind = ReloadKind::ShellPerShell;
        let mut s = AmmoSlot::full_from_config(c);
        s.consume_shot();
        s.consume_shot();
        s.consume_shot();
        assert_eq!(s.current_mag, 1);
        assert!(s.try_start_reload());
        // Tick 1 shell.
        let c1 = s.tick_reload(0.31);
        assert!(matches!(
            c1,
            Some(AmmoChangeKind::Reload { transferred: 1 })
        ));
        assert_eq!(s.current_mag, 2);
        assert!(s.reload_state.is_reloading()); // relance auto
                                                // Tick 2.
        s.tick_reload(0.31);
        assert_eq!(s.current_mag, 3);
        // Tick 3 → mag plein, fin.
        s.tick_reload(0.31);
        assert_eq!(s.current_mag, 4);
        assert!(!s.reload_state.is_reloading());
    }

    #[test]
    fn cancel_reload_keeps_partial_mag() {
        let mut c = cfg(4, 10, 0.3);
        c.reload_kind = ReloadKind::ShellPerShell;
        let mut s = AmmoSlot::full_from_config(c);
        s.consume_shot();
        s.consume_shot();
        s.consume_shot();
        s.try_start_reload();
        s.tick_reload(0.31); // +1 shell, current=2
        s.cancel_reload();
        assert_eq!(s.current_mag, 2); // progrès gardé
        assert!(!s.reload_state.is_reloading());
    }

    #[test]
    fn reload_blocked_when_mag_full() {
        let mut s = AmmoSlot::full_from_config(cfg(5, 10, 1.0));
        assert!(!s.try_start_reload());
    }

    #[test]
    fn reload_blocked_when_no_reserve() {
        let mut s = AmmoSlot::full_from_config(cfg(5, 0, 1.0));
        s.consume_shot();
        assert!(!s.try_start_reload());
    }

    #[test]
    fn low_ammo_flag_below_threshold() {
        let mut s = AmmoSlot::full_from_config(cfg(20, 0, 1.0));
        // threshold 0.25 → low si mag <= 5
        for _ in 0..15 {
            s.consume_shot();
        }
        assert_eq!(s.current_mag, 5);
        assert!(s.is_low());
        s.consume_shot();
        assert!(s.is_low());
    }

    #[test]
    fn hot_reload_clamps_overflow() {
        let mut s = AmmoSlot::full_from_config(cfg(50, 200, 1.0));
        // mag_size shrink 50 → 20 → current clamp
        s.apply_config(cfg(20, 200, 1.0));
        assert_eq!(s.current_mag, 20);
        assert_eq!(s.config.mag_size, 20);
    }

    #[test]
    fn hot_reload_scales_in_progress_reload_time() {
        let mut s = AmmoSlot::full_from_config(cfg(10, 10, 2.0));
        s.consume_shot();
        s.try_start_reload();
        s.tick_reload(0.5); // 25% done, remaining 1.5s
                            // New genome : reload_time 4.0s. Restant doit scaler à 3.0s (75% restant).
        s.apply_config(cfg(10, 10, 4.0));
        if let ReloadState::Reloading { remaining_secs, .. } = s.reload_state {
            assert!(
                (remaining_secs - 3.0).abs() < 0.01,
                "remaining={}",
                remaining_secs
            );
        } else {
            panic!("should still be reloading");
        }
    }

    #[test]
    fn pickup_caps_at_reserve_max() {
        let mut s = AmmoSlot::full_from_config(cfg(10, 20, 1.0));
        s.reserve = 18;
        let added = s.pickup(10);
        assert_eq!(added, 2);
        assert_eq!(s.reserve, 20);
    }

    #[test]
    fn reload_kind_parses_aliases() {
        assert_eq!(ReloadKind::from_genome_str("mag"), ReloadKind::Mag);
        assert_eq!(
            ReloadKind::from_genome_str("Shell_Per_Shell"),
            ReloadKind::ShellPerShell
        );
        assert_eq!(
            ReloadKind::from_genome_str("pump"),
            ReloadKind::ShellPerShell
        );
        // Unknown → Mag fallback (warn).
        assert_eq!(ReloadKind::from_genome_str("foobar"), ReloadKind::Mag);
    }
}

// ─── Entretien : la vitesse de rechargement (2026-08-04) ─────────────────────

#[cfg(test)]
mod reload_speed_tests {
    use super::*;

    fn slot_vide() -> AmmoSlot {
        let mut s = AmmoSlot::default();
        s.current_mag = 0;
        s.reserve = 100;
        s
    }

    fn restant(s: &AmmoSlot) -> f32 {
        match s.reload_state {
            ReloadState::Reloading { remaining_secs, .. } => remaining_secs,
            ReloadState::Idle => f32::NAN,
        }
    }

    /// Un atout d'entretien raccourcit VRAIMENT le rechargement.
    #[test]
    fn a_maintenance_boon_actually_shortens_the_reload() {
        let mut nu = slot_vide();
        assert!(nu.try_start_reload());
        let mut boosté = slot_vide();
        assert!(boosté.try_start_reload_at_speed(1.5));
        assert!(
            restant(&boosté) < restant(&nu),
            "{} devrait être < {}",
            restant(&boosté),
            restant(&nu)
        );
        // +50 % de vitesse = deux tiers du temps.
        assert!((restant(&boosté) - restant(&nu) / 1.5).abs() < 1e-4);
    }

    /// Sans atout, rien ne change : `try_start_reload` reste l'ancien comportement.
    #[test]
    fn no_boon_means_the_previous_behaviour_exactly() {
        let mut a = slot_vide();
        let mut b = slot_vide();
        a.try_start_reload();
        b.try_start_reload_at_speed(1.0);
        assert_eq!(restant(&a), restant(&b));
        assert_eq!(restant(&a), a.config.reload_time_secs);
    }

    /// Un atout ne doit JAMAIS pouvoir casser l'arme qu'il améliore : une vitesse
    /// nulle ou négative gèlerait le rechargement pour toujours.
    #[test]
    fn a_degenerate_speed_can_never_freeze_the_weapon() {
        for vitesse in [0.0, -3.0, f32::MIN] {
            let mut s = slot_vide();
            assert!(s.try_start_reload_at_speed(vitesse));
            let r = restant(&s);
            assert!(r.is_finite() && r > 0.0, "vitesse {vitesse} → {r}");
            // Borné : au pire 10× le temps nominal, jamais l'infini.
            assert!(r <= s.config.reload_time_secs * 10.0 + 1e-3);
        }
    }

    /// Le bonus n'écrit PAS dans `config` — sinon le prochain hot-reload de
    /// génome l'effacerait en silence.
    #[test]
    fn the_boon_never_writes_into_the_genome_mirror() {
        let mut s = slot_vide();
        let avant = s.config.reload_time_secs;
        s.try_start_reload_at_speed(3.0);
        assert_eq!(s.config.reload_time_secs, avant);
    }
}
