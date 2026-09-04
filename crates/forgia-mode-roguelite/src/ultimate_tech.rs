//! ultimate_tech.rs — Logique PURE des techniques d'Ultime (story-596 T2).
//!
//! Pendant l'État Ultime (10 s, cf. `forgia_combat::ultimate::UltimateState`),
//! l'arme équipée applique sa **technique signature**. Ce module ne contient que
//! la **décision pure et testable headless** de deux d'entre elles — le
//! branchement runtime (insertion des components, lecture genome, intégration au
//! mouvement des bots, VFX) vit en T3/T4 :
//!
//! - **Bourrasque** : *chaîne électrique* → [`chain_targets`] choisit les ennemis
//!   vers qui l'arc rebondit depuis le point d'impact.
//! - **Pompe** : *gel de zone* → [`frozen_speed_factor`] / [`freeze_secs_after_tick`]
//!   modélisent l'immobilisation décroissante.
//!
//! Les constantes sont des **tunables de référence** (genome-ready, migration TOML
//! en T3 — même pattern interim que `shockwave.rs`). Aucun hardcode enfoui dans la
//! logique : tout passe en paramètres.

use bevy::math::Vec3;

// ─── Tunables de référence (genome story de suivi) ──────────────────────────

/// Bourrasque — nb maximal de rebonds d'arc APRÈS l'ennemi touché (l'impact
/// primaire est exclu des candidats par l'appelant ; cf. `chain_extra_targets`).
pub const ULTIMATE_CHAIN_MAX_EXTRA: usize = 3;
/// Bourrasque — distance maximale (m) d'un rebond d'arc vers l'ennemi suivant.
pub const ULTIMATE_CHAIN_HOP_RADIUS: f32 = 4.0;
/// Bourrasque — fraction des dégâts du tir transmise à chaque ennemi de la chaîne.
pub const ULTIMATE_CHAIN_DAMAGE_FACTOR: f32 = 0.6;

/// Pompe — durée du gel (s) appliqué à un ennemi pris dans la zone.
pub const ULTIMATE_FREEZE_SECS: f32 = 2.5;
/// Pompe — facteur de vitesse pendant le gel (0 = immobilisation totale).
pub const ULTIMATE_FREEZE_SLOW: f32 = 0.0;
/// Pompe — rayon (m) de la zone de gel autour de l'impact.
pub const ULTIMATE_FREEZE_AOE_RADIUS: f32 = 5.0;

/// Pépin — rayon (m) du splash d'explosion autour de l'impact.
pub const ULTIMATE_EXPLOSION_RADIUS: f32 = 4.5;
/// Pépin — fraction des dégâts du tir transmise aux voisins de l'explosion.
pub const ULTIMATE_EXPLOSION_DAMAGE_FACTOR: f32 = 0.8;

/// Mme Lenoir — portée (m) de la perforation derrière la cible touchée.
pub const ULTIMATE_PIERCE_RANGE: f32 = 25.0;
/// Mme Lenoir — demi-largeur (m) du couloir de perforation.
pub const ULTIMATE_PIERCE_HALF_WIDTH: f32 = 1.5;
/// Mme Lenoir — fraction des dégâts du tir transmise aux ennemis perforés.
pub const ULTIMATE_PIERCE_DAMAGE_FACTOR: f32 = 0.7;

// ─── Bourrasque : chaîne électrique (sélection des cibles) ──────────────────

/// Construit la **chaîne d'arc électrique** depuis `start` (point d'impact du
/// tir) : saute vers l'ennemi le plus proche non encore touché, à ≤ `hop_radius`
/// du point courant, et répète depuis celui-ci, jusqu'à `max_extra` rebonds.
///
/// L'ennemi touché en premier (l'impact) doit être **exclu de `candidates`** par
/// l'appelant ; le résultat est la liste des cibles SUPPLÉMENTAIRES touchées par
/// l'arc, dans l'ordre de propagation. `out` est vidé puis rempli (buffer réutilisé
/// — 0 alloc hot path, règle scalability §hot). Générique sur l'id de nœud pour
/// rester testable sans `Entity`.
pub fn chain_targets<T: Copy + PartialEq>(
    start: Vec3,
    candidates: &[(T, Vec3)],
    max_extra: usize,
    hop_radius: f32,
    out: &mut Vec<T>,
) {
    out.clear();
    if max_extra == 0 || hop_radius <= 0.0 {
        return;
    }
    let r2 = hop_radius * hop_radius;
    let mut cur = start;
    while out.len() < max_extra {
        // Ennemi le plus proche du point courant, non déjà dans la chaîne, à portée.
        let mut best: Option<(usize, f32)> = None;
        for (i, (id, pos)) in candidates.iter().enumerate() {
            if out.contains(id) {
                continue;
            }
            let d2 = (*pos - cur).length_squared();
            if d2 <= r2 && best.is_none_or(|(_, bd)| d2 < bd) {
                best = Some((i, d2));
            }
        }
        let Some((i, _)) = best else {
            break; // plus aucun ennemi à portée → l'arc s'éteint
        };
        out.push(candidates[i].0);
        cur = candidates[i].1;
    }
}

// ─── Mme Lenoir : perforation (couloir de ligne) ────────────────────────────

/// Sélectionne les ennemis **perforés** : ceux dans un couloir de longueur
/// `range` et demi-largeur `half_width` partant de `origin` le long de `dir`
/// (projeté sur le plan XZ). Mêmes maths que l'ancien `line_strike` (shockwave)
/// mais pur et générique (testable sans `Entity`). `out` est vidé puis rempli
/// (buffer réutilisé — 0 alloc hot path). L'appelant exclut la cible primaire.
pub fn pierce_targets<T: Copy>(
    origin: Vec3,
    dir: Vec3,
    range: f32,
    half_width: f32,
    candidates: &[(T, Vec3)],
    out: &mut Vec<T>,
) {
    out.clear();
    let fwd = Vec3::new(dir.x, 0.0, dir.z).normalize_or_zero();
    if fwd == Vec3::ZERO || range <= 0.0 {
        return;
    }
    // Perpendiculaire au forward dans le plan XZ (rotation -90°).
    let perp = Vec3::new(fwd.z, 0.0, -fwd.x);
    for (id, pos) in candidates {
        let rel = Vec3::new(pos.x - origin.x, 0.0, pos.z - origin.z);
        let along = rel.dot(fwd);
        if along < 0.0 || along > range {
            continue;
        }
        if rel.dot(perp).abs() > half_width {
            continue;
        }
        out.push(*id);
    }
}

// ─── Pompe : gel de zone (modèle d'immobilisation) ──────────────────────────

/// Facteur multiplicatif appliqué à la vitesse d'un ennemi : `slow_factor`
/// (clampé `0..1`) tant qu'il reste du gel, sinon `1.0` (vitesse normale).
pub fn frozen_speed_factor(secs_left: f32, slow_factor: f32) -> f32 {
    if secs_left > 0.0 {
        slow_factor.clamp(0.0, 1.0)
    } else {
        1.0
    }
}

/// Décompte du gel : retourne le temps de gel restant après `dt` secondes
/// (clampé à 0). Le component est retiré par l'appelant quand le retour est `0`.
pub fn freeze_secs_after_tick(secs_left: f32, dt: f32) -> f32 {
    (secs_left - dt).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f32, z: f32) -> Vec3 {
        Vec3::new(x, 0.0, z)
    }

    #[test]
    fn chain_follows_consecutive_neighbours_in_order() {
        // Ennemis alignés espacés de 2 m ; hop 3 m → l'arc traverse les 3.
        let cands = [(1usize, v(2.0, 0.0)), (2, v(4.0, 0.0)), (3, v(6.0, 0.0))];
        let mut out = Vec::new();
        chain_targets(v(0.0, 0.0), &cands, 3, 3.0, &mut out);
        assert_eq!(out, vec![1, 2, 3], "l'arc rebondit de proche en proche");
    }

    #[test]
    fn chain_stops_at_a_gap_larger_than_hop() {
        // 1 à 2 m, puis trou de 5 m (> hop 3) → l'arc s'éteint après le 1er.
        let cands = [(1usize, v(2.0, 0.0)), (2, v(7.0, 0.0))];
        let mut out = Vec::new();
        chain_targets(v(0.0, 0.0), &cands, 3, 3.0, &mut out);
        assert_eq!(out, vec![1], "le trou > hop coupe la chaîne");
    }

    #[test]
    fn chain_respects_max_extra_cap() {
        let cands = [
            (1usize, v(1.0, 0.0)),
            (2, v(2.0, 0.0)),
            (3, v(3.0, 0.0)),
            (4, v(4.0, 0.0)),
        ];
        let mut out = Vec::new();
        chain_targets(v(0.0, 0.0), &cands, 2, 3.0, &mut out);
        assert_eq!(out.len(), 2, "cap à max_extra rebonds");
        assert_eq!(out, vec![1, 2]);
    }

    #[test]
    fn chain_never_repeats_a_target() {
        let cands = [(1usize, v(1.0, 0.0)), (2, v(1.2, 0.0))];
        let mut out = Vec::new();
        chain_targets(v(0.0, 0.0), &cands, 5, 3.0, &mut out);
        assert_eq!(out.len(), 2, "chaque ennemi touché une seule fois");
        // Pas de doublon malgré max_extra > nb d'ennemis.
        out.sort();
        out.dedup();
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn chain_empty_when_no_extra_or_no_radius() {
        let cands = [(1usize, v(1.0, 0.0))];
        let mut out = vec![42];
        chain_targets(v(0.0, 0.0), &cands, 0, 3.0, &mut out);
        assert!(out.is_empty(), "max_extra=0 → aucune cible, buffer vidé");
        chain_targets(v(0.0, 0.0), &cands, 3, 0.0, &mut out);
        assert!(out.is_empty(), "hop_radius=0 → aucune cible");
    }

    #[test]
    fn chain_picks_nearest_first_not_index_order() {
        // Le candidat le plus proche du start n'est pas le premier de la liste.
        let cands = [(1usize, v(2.9, 0.0)), (2, v(0.5, 0.0))];
        let mut out = Vec::new();
        chain_targets(v(0.0, 0.0), &cands, 2, 3.0, &mut out);
        assert_eq!(out[0], 2, "le plus proche d'abord (id 2 à 0.5 m)");
    }

    #[test]
    fn pierce_hits_enemies_in_corridor_only() {
        // Forward = +X. Trois ennemis : 2 alignés dans le couloir, 1 décalé hors largeur.
        let cands = [
            (1usize, v(5.0, 0.0)), // dans l'axe, à 5 m → touché
            (2, v(10.0, 1.0)),     // à 10 m, décalé 1 m < 1.5 → touché
            (3, v(8.0, 3.0)),      // décalé 3 m > 1.5 → raté
        ];
        let mut out = Vec::new();
        pierce_targets(v(0.0, 0.0), v(1.0, 0.0), 25.0, 1.5, &cands, &mut out);
        assert_eq!(
            out,
            vec![1, 2],
            "seuls les ennemis dans le couloir sont perforés"
        );
    }

    #[test]
    fn pierce_excludes_behind_and_out_of_range() {
        let cands = [
            (1usize, v(-3.0, 0.0)), // DERRIÈRE le tireur (along < 0) → raté
            (2, v(30.0, 0.0)),      // au-delà de range 25 → raté
        ];
        let mut out = Vec::new();
        pierce_targets(v(0.0, 0.0), v(1.0, 0.0), 25.0, 1.5, &cands, &mut out);
        assert!(out.is_empty(), "ni derrière, ni hors portée");
    }

    #[test]
    fn frozen_speed_factor_clamps_and_thaws() {
        assert_eq!(frozen_speed_factor(1.0, 0.0), 0.0, "gelé = immobile");
        assert_eq!(
            frozen_speed_factor(0.0, 0.0),
            1.0,
            "dégelé = vitesse normale"
        );
        assert!(
            (frozen_speed_factor(1.0, 0.3) - 0.3).abs() < 1e-6,
            "ralenti partiel"
        );
        assert_eq!(frozen_speed_factor(1.0, 1.5), 1.0, "clamp haut");
        assert_eq!(frozen_speed_factor(1.0, -0.2), 0.0, "clamp bas");
    }

    #[test]
    fn freeze_tick_counts_down_to_zero() {
        assert!((freeze_secs_after_tick(2.5, 0.5) - 2.0).abs() < 1e-6);
        assert_eq!(
            freeze_secs_after_tick(0.3, 1.0),
            0.0,
            "clamp à 0, pas de négatif"
        );
    }
}
