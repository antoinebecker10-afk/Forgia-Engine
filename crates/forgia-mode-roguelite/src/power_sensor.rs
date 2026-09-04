//! power_sensor.rs — `forgia2_power.json` 1 Hz : la puissance RÉELLE du joueur,
//! décomposée, face à la menace du round (V0, 2026-08-04).
//!
//! ## Pourquoi ce capteur existe
//!
//! Cinq systèmes alimentent `PlayerCombatMods.damage_mul` — les atouts, l'Enclume,
//! la maîtrise d'arme, la Trempe et l'équipement. La composition est propre : un
//! seul écrivain, une ligne par source (`boons_apply::sys_recompute_boon_mods`).
//!
//! Mais le modèle de difficulté (`rounds.rs`) ne connaît d'eux **qu'un seul chiffre
//! agrégé**, `power_gain_per_round`, posé à la main — et il est *linéaire*
//! (`1 + gain × round`) là où la menace est *géométrique*. Pire : il suppose le
//! joueur à ×1,0 au round 0, alors qu'un compte avec de la méta démarre bien
//! au-dessus (Enclume ×1,40 · maîtrise ×1,20 = ×1,68 avant le premier tir).
//!
//! Autrement dit : **le mur est calculé, mais pas contre le bon adversaire.** Tant
//! que l'écart n'est pas mesuré, tout réglage de la courbe est un pari.
//!
//! ## Ce qu'il ne fait pas
//!
//! Il ne **recalcule rien**. La décomposition est écrite par le compositeur
//! lui-même ([`PowerBreakdown`]), et le temps de nettoyage passe par
//! `RoundsConfig::time_to_clear_at_power` — la même formule que le modèle. Un
//! capteur qui refait le calcul finit toujours par diverger de la vérité qu'il
//! prétend mesurer (`map-design-patterns.md` §14).
//!
//! ## Schéma
//!
//! - `power.{total,boons,perm,mastery,trempe,equip}` — la décomposition
//! - `power.modeled` — ce que le modèle du mur suppose au round courant
//! - `boon_count` / `boon_damage_count` — atouts actifs, et ceux qui portent un
//!   `DamageMul`. Les deux sont nécessaires : `boons = 1.000` avec 2 atouts actifs
//!   est normal s'ils donnent de la cadence ou du soin, et un défaut s'ils sont
//!   inertes. Un seul des deux compteurs ne permet pas de trancher.
//! - `mastery_weapon` — l'arme dont la maîtrise est appliquée. À comparer à
//!   `forgia2_trempe.current_weapon` : un écart signalait le désync corrigé le
//!   2026-08-04, et le signalerait à nouveau s'il revenait.
//! - `model_ratio` — `total / modeled`. **1,0 = le modèle dit vrai.**
//! - `margin.{modeled,real}` — `1 − ttk/budget`. Négatif = le mur est franchi.
//! - `wall.{lazy,diligent}` — le mur selon le modèle, joueur qui ne prend rien /
//!   qui prend tout. `null` = hors de portée (≠ « pas de mur »).
//! - `routing.{unknown_stats,over_cap_boons}` — les atouts qui ne font rien, ou qui
//!   en font trop pour une seule arme.

use bevy::prelude::*;

use crate::boons_apply::{BoonRoutingIssues, PowerBreakdown};
use crate::rounds::{RoundPace, RoundsConfig};

const SENSOR_PATH: &str = "forgia2_power.json";
const SENSOR_PERIOD_SECS: f32 = 1.0;

/// Au-delà, la puissance réelle écrase tellement le modèle que le mur ne tombera
/// jamais : la difficulté annoncée n'est pas celle qui est jouée.
const MODEL_RATIO_TOO_HIGH: f32 = 2.0;
/// En-dessous, le joueur est bien plus faible que ce que le modèle suppose — le mur
/// tombera plus tôt qu'annoncé, et la run sera vécue comme injuste.
const MODEL_RATIO_TOO_LOW: f32 = 0.5;

/// Écrit le capteur 1 Hz. Gaté sur `GameMode::Roguelite` par le plugin.
#[allow(clippy::too_many_arguments)]
/// Le point le plus PROFOND atteint depuis le lancement, et la puissance qu'on
/// y avait.
///
/// ## Pourquoi il existe
///
/// Le capteur de puissance décrit l'instant présent. Dès qu'une run se termine,
/// tout retombe au round 1 — et les seuls chiffres qui décident de la
/// calibration (la puissance au round 10, face au boss) sont perdus. Trois runs
/// gagnées d'affilée n'ont donc rien appris : à chaque « regarde », la salve
/// arrivait après la remise à zéro.
///
/// Il ne se réinitialise pas : c'est un record de session, comme
/// `los_checks_session` chez les bots. Un pic qu'on remet à zéro ne répond plus
/// à « où en était-il au bout ? ».
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct PowerPeak {
    /// Round 1-basé le plus profond atteint. 0 = jamais entré en run.
    pub round: u32,
    pub total: f32,
    pub boons: f32,
    pub mastery: f32,
    pub trempe: f32,
    pub equip: f32,
    pub boon_damage_count: u32,
    /// Marge réelle à ce round — négative = le budget de temps était dépassé.
    pub margin_real: f32,
}

pub fn sys_write_power_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    breakdown: Res<PowerBreakdown>,
    issues: Res<BoonRoutingIssues>,
    cfg: Res<RoundsConfig>,
    pace: Option<Res<RoundPace>>,
    mastery: Res<crate::meta_shop::WeaponMasteryMods>,
    mut peak: ResMut<PowerPeak>,
) {
    *accum += time.delta_secs();
    if *accum < SENSOR_PERIOD_SECS {
        return;
    }
    *accum = 0.0;

    // `RoundPace.round` porte le ROUND canonique (le combat) depuis le
    // 2026-08-04 ; il portait l'index d'ARÈNE avant, et ce capteur annonçait
    // donc `round: 0` alors que le joueur était au round 1. Le modèle attend un
    // index 0-basé, comme `enemy_scaling` et le capteur de rounds.
    let round_1based = pace.as_ref().map(|p| p.round).unwrap_or(0);
    let round = round_1based.saturating_sub(1);
    let threat = cfg.threat(round);
    let modeled = cfg.player_power(round, 1.0);
    let model_ratio = if modeled.abs() > f32::EPSILON {
        breakdown.total / modeled
    } else {
        f32::INFINITY
    };
    let margin_modeled = cfg.margin(round, 1.0);
    let margin_real = cfg.margin_at_power(round, breakdown.total);

    // Le pic se met à jour AVANT l'écriture : la salve qui suit la fin d'une run
    // doit déjà porter le point le plus profond.
    if round_1based > peak.round {
        *peak = PowerPeak {
            round: round_1based,
            total: breakdown.total,
            boons: breakdown.boons,
            mastery: breakdown.mastery,
            trempe: breakdown.trempe,
            equip: breakdown.equip,
            boon_damage_count: breakdown.boon_damage_count,
            margin_real,
        };
    }

    let (severity, next_step) = severity_for_power(issues.is_clean(), model_ratio, margin_real);

    let json = format!(
        r#"{{"id":"power","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"round":{round_1based},"threat_hp":{:.2},"power":{{"total":{:.3},"boons":{:.3},"perm":{:.3},"mastery":{:.3},"trempe":{:.3},"equip":{:.3},"modeled":{:.3}}},"boon_count":{},"boon_damage_count":{},"mastery_weapon":"{:?}","model_ratio":{:.3},"margin":{{"modeled":{:.3},"real":{:.3}}},"wall":{{"lazy":{},"diligent":{}}},"routing":{{"unknown_stats":[{}],"over_cap_boons":[{}]}},"deepest":{{"round":{},"total":{:.3},"boons":{:.3},"mastery":{:.3},"trempe":{:.3},"equip":{:.3},"boon_damage_count":{},"margin_real":{:.3}}}}}"#,
        time.elapsed_secs(),
        threat.hp,
        breakdown.total,
        breakdown.boons,
        breakdown.perm,
        breakdown.mastery,
        breakdown.trempe,
        breakdown.equip,
        modeled,
        breakdown.boon_count,
        breakdown.boon_damage_count,
        mastery.current,
        model_ratio,
        margin_modeled,
        margin_real,
        opt_round_json(cfg.wall_round(0.0)),
        opt_round_json(cfg.wall_round(1.0)),
        str_list_json(&issues.unknown_stats),
        str_list_json(&issues.over_cap_boons),
        peak.round,
        peak.total,
        peak.boons,
        peak.mastery,
        peak.trempe,
        peak.equip,
        peak.boon_damage_count,
        peak.margin_real,
    );

    if let Err(e) = forgia_core::sensor_io::enqueue(SENSOR_PATH, json) {
        warn!("[forgia-mode-roguelite] power sensor write failed: {e}");
    }
}

/// `null` distingue « le mur est hors de portée » de « le mur est au round 0 ».
/// Les confondre ferait lire un `0` comme un succès (`map-design-patterns.md` §13).
fn opt_round_json(r: Option<u32>) -> String {
    r.map(|v| v.to_string()).unwrap_or_else(|| "null".into())
}

fn str_list_json(items: &[String]) -> String {
    items
        .iter()
        .map(|s| format!(r#""{}""#, s.replace('"', "'")))
        .collect::<Vec<_>>()
        .join(",")
}

/// Pur — testable sans App ni World.
///
/// L'ordre des cas est une hiérarchie de gravité : un atout inerte est un défaut
/// **certain** (le joueur l'a payé) ; un mur franchi est un fait de la run en cours ;
/// une divergence de modèle est une dette de calibration.
pub fn severity_for_power(
    routing_clean: bool,
    model_ratio: f32,
    margin_real: f32,
) -> (&'static str, &'static str) {
    if !routing_clean {
        return (
            "error",
            "Un atout est INERTE (stat non routée) ou dépasse le plafond d'arme — \
             voir routing.unknown_stats / routing.over_cap_boons",
        );
    }
    if margin_real < 0.0 {
        return (
            "warn",
            "Le round courant ne se nettoie plus dans le budget à puissance réelle — \
             le mur est franchi (baisser la menace ou augmenter les récompenses)",
        );
    }
    if model_ratio > MODEL_RATIO_TOO_HIGH {
        return (
            "warn",
            "La puissance réelle dépasse largement le modèle du mur — la difficulté \
             annoncée n'est pas celle qui est jouée (recalibrer power_gain_per_round)",
        );
    }
    if model_ratio < MODEL_RATIO_TOO_LOW {
        return (
            "warn",
            "La puissance réelle est bien SOUS le modèle — le mur tombera plus tôt \
             qu'annoncé (recalibrer power_gain_per_round ou les récompenses)",
        );
    }
    ("ok", "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_atout_inerte_prime_sur_tout_le_reste() {
        // Même si la balance est parfaite : le joueur a payé pour rien, c'est le
        // défaut le plus grave parce qu'il est CERTAIN, pas modélisé.
        let (sev, _) = severity_for_power(false, 1.0, 0.5);
        assert_eq!(sev, "error");
    }

    #[test]
    fn un_mur_franchi_se_signale() {
        let (sev, step) = severity_for_power(true, 1.0, -0.2);
        assert_eq!(sev, "warn");
        assert!(step.contains("mur est franchi"));
    }

    #[test]
    fn un_modele_trop_optimiste_se_signale() {
        // Le socle méta seul (Enclume ×1.4 × maîtrise ×1.2 × trempe ×2.01 = ×3.38)
        // contre un modèle à ×1.0 au round 0 : exactement le cas qu'on cherche.
        let (sev, step) = severity_for_power(true, 3.38, 0.9);
        assert_eq!(sev, "warn");
        assert!(step.contains("power_gain_per_round"));
    }

    #[test]
    fn un_modele_trop_genereux_se_signale_aussi() {
        let (sev, _) = severity_for_power(true, 0.3, 0.4);
        assert_eq!(sev, "warn");
    }

    #[test]
    fn un_modele_juste_et_une_marge_saine_sont_ok() {
        let (sev, step) = severity_for_power(true, 1.1, 0.4);
        assert_eq!(sev, "ok");
        assert!(step.is_empty());
    }

    #[test]
    fn un_mur_hors_de_portee_nest_pas_le_round_zero() {
        // Confondre les deux ferait lire « aucun mur » comme « mur immédiat ».
        assert_eq!(opt_round_json(None), "null");
        assert_eq!(opt_round_json(Some(0)), "0");
    }

    #[test]
    fn la_liste_de_stats_est_du_json_valide() {
        assert_eq!(str_list_json(&[]), "");
        assert_eq!(
            str_list_json(&["reload_speed".into(), "ammo".into()]),
            r#""reload_speed","ammo""#
        );
    }

    #[test]
    fn un_guillemet_dans_un_id_ne_casse_pas_le_json() {
        assert_eq!(str_list_json(&[r#"a"b"#.into()]), r#""a'b""#);
    }
}

#[cfg(test)]
mod peak_tests {
    use super::*;

    /// Le pic ne recule pas : c'est ce qui le rend lisible APRÈS une run.
    /// Sans ça, la salve qui suit la fin d'un chapitre retombe au round 1 — et
    /// trois chapitres gagnés d'affilée n'ont rien appris.
    #[test]
    fn the_deepest_point_never_walks_back() {
        let mut peak = PowerPeak::default();
        for (round, total) in [(1u32, 1.9f32), (7, 2.4), (10, 2.6), (1, 1.9), (3, 2.0)] {
            if round > peak.round {
                peak = PowerPeak {
                    round,
                    total,
                    ..peak
                };
            }
        }
        assert_eq!(peak.round, 10, "le pic est retombé avec la run suivante");
        assert!((peak.total - 2.6).abs() < 1e-6, "puissance du pic écrasée");
    }

    /// Tant qu'aucune run n'a commencé, le pic doit se lire comme « rien »,
    /// pas comme un round 1 fictif.
    #[test]
    fn never_having_played_reads_as_zero_not_as_round_one() {
        assert_eq!(PowerPeak::default().round, 0);
    }
}
