//! combat_rng.rs — Keystone 0.1b (story-634, 2026-06-28).
//!
//! Resource `CombatRng` : flux RNG **déterministe** pour le combat, dérivé du
//! seed de run (`RunSeed`) — remplace les seeds ad-hoc basés sur l'horloge ou le
//! `toi` Rapier (non reproductibles ; cf. spike-fixedupdate-determinism §3).
//!
//! ## Pourquoi pas un seul `Rng` mutable partagé
//! Un `Rng` partagé entre N systèmes (crit, recoil, loot) exigerait un ordre
//! d'exécution global strict — fragile et cross-crate. À la place : un compteur
//! déterministe (`shot_counter`) + un sel par usage → seed dérivé → `Rng`
//! éphémère. La reproductibilité tient au COMPTEUR (stable par tir), pas au
//! timing ni à l'ordre des systèmes. Même pattern que `RunSeed::stage_seed`.
//!
//! ## Portée (slice 0.1b-1)
//! Le crit consomme déjà ce flux. Recoil + loot + AOE-order suivront (sels
//! dédiés) dans les slices 0.1b suivantes.
//!
//! Cross-crate safe : forgia-combat est dep root de forgia-fps (consomme) et de
//! forgia-mode-roguelite (reseed au StartRunEvent), comme `PlayerCombatMods`.

use bevy::prelude::*;

/// Sel du flux crit — sépare le crit des autres flux dérivés du même tir.
/// Valeur arbitraire fixe : seule la stabilité (jamais re-changée) compte.
pub const CRIT_SALT: u64 = 0x0000_0000_C217_0001;

/// Sel du flux recoil (yaw aléatoire du recul) — décorrélé du crit pour le même tir.
pub const RECOIL_SALT: u64 = 0x0000_0000_2EC0_1100;

/// État RNG combat déterministe. `Default` = seed 0 (déterministe hors run, ex.
/// FPS arena) ; reseedé depuis `RunSeed` au démarrage de run (Roguelite).
#[derive(Resource, Debug, Clone, Default)]
pub struct CombatRng {
    /// Seed maître (= `RunSeed.seed` en Roguelite, 0 sinon).
    pub seed: u64,
    /// Compteur monotone de tirs résolus (nonce déterministe). Reset au reseed.
    pub shot_counter: u64,
}

impl CombatRng {
    /// (Re)seed au démarrage d'une run. Remet le compteur de tirs à zéro.
    pub fn reseed(&mut self, seed: u64) {
        self.seed = seed;
        self.shot_counter = 0;
    }

    /// À appeler EXACTEMENT 1× par tir résolu (en tête de résolution), avant de
    /// tirer les flux dérivés du tir. Avance le nonce.
    pub fn begin_shot(&mut self) {
        self.shot_counter = self.shot_counter.wrapping_add(1);
    }

    /// `Rng` éphémère déterministe pour (tir courant, `sub_index`, `salt`).
    /// `sub_index` distingue les sous-événements d'un même tir (ex. `pellet_idx`).
    pub fn shot_stream(&self, sub_index: u32, salt: u64) -> forgia_rng::Rng {
        // Mix multiplicatif (constantes premières impaires) — chaque dimension
        // (tir / sous-index / usage) sur un sel distinct → flux décorrélés. Le
        // splitmix64 interne de `Rng::new` diffuse ensuite les bits.
        let mixed = self
            .seed
            ^ self.shot_counter.wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ u64::from(sub_index).wrapping_mul(0xBF58_476D_1CE4_E5B9)
            ^ salt.wrapping_mul(0x2545_F491_4F6C_DD1D);
        forgia_rng::Rng::new(mixed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reseed_resets_counter() {
        let mut r = CombatRng::default();
        r.begin_shot();
        r.begin_shot();
        assert_eq!(r.shot_counter, 2);
        r.reseed(123);
        assert_eq!(r.seed, 123);
        assert_eq!(r.shot_counter, 0);
    }

    #[test]
    fn same_seed_same_shot_reproducible() {
        let mut a = CombatRng::default();
        let mut b = CombatRng::default();
        a.reseed(42);
        b.reseed(42);
        a.begin_shot();
        b.begin_shot();
        assert_eq!(
            a.shot_stream(0, CRIT_SALT).next_f32(),
            b.shot_stream(0, CRIT_SALT).next_f32(),
            "même seed + même tir → même roll"
        );
    }

    #[test]
    fn different_shots_differ() {
        let mut r = CombatRng::default();
        r.reseed(42);
        r.begin_shot();
        let s1 = r.shot_stream(0, CRIT_SALT).next_f32();
        r.begin_shot();
        let s2 = r.shot_stream(0, CRIT_SALT).next_f32();
        assert_ne!(s1, s2, "deux tirs distincts → rolls distincts");
    }

    /// Distribution : `shot_stream(...).next_f32()` (1 Rng frais par tir, 1er tirage)
    /// est uniforme sur [0,1) → un seuil `chance` proc ~`chance` du temps. C'est ce
    /// qui garantit que le crit (0.1b-1) proc au taux du boon ; un biais du 1er
    /// tirage post-seeding casserait le feel. Vérifie pile l'usage runtime.
    #[test]
    fn crit_roll_distribution_matches_chance() {
        let mut r = CombatRng::default();
        r.reseed(1782672052101020100); // un vrai seed de run
        let chance = 0.20_f32;
        let n = 200_000;
        let mut crits = 0u32;
        for _ in 0..n {
            r.begin_shot();
            if r.shot_stream(0, CRIT_SALT).next_f32() < chance {
                crits += 1;
            }
        }
        let frac = crits as f32 / n as f32;
        assert!(
            (frac - chance).abs() < 0.01,
            "crit frac {frac} doit être ~{chance} (1er tirage post-seed uniforme)"
        );
    }

    #[test]
    fn different_sub_index_differ() {
        let mut r = CombatRng::default();
        r.reseed(42);
        r.begin_shot();
        let p0 = r.shot_stream(0, CRIT_SALT).next_f32();
        let p1 = r.shot_stream(1, CRIT_SALT).next_f32();
        assert_ne!(p0, p1, "pellets distincts → rolls distincts");
    }
}
