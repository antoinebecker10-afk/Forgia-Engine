//! Layout sensor — story-485 phase 5/6.
//!
//! Lit `LayoutResult` Resource + écrit `forgia2_stage_layout.json` 1Hz avec
//! métriques placement modules + severity + next_step.
//!
//! Pattern : `[[reference-pattern-genome-driven-plugin-with-sensor]]`.

use crate::layout::{count_by_kind, SIGHTLINE_MAX_M};
use crate::LayoutResult;
use bevy::prelude::*;
use forgia_core::layout::{
    covers_expected, disc_area, min_spacing_m, open_radius_m, sightline_profile,
    COVER_SPACING_MAX_M, COVER_SPACING_MIN_M,
};
use forgia_level_presets::ModuleKind;
use serde::Serialize;
use web_time::{SystemTime, UNIX_EPOCH};

const SENSOR_PATH: &str = "forgia2_stage_layout.json";
const SENSOR_WRITE_PERIOD_SEC: f64 = 1.0;

// ─── Severity / Next-step (purs, testables headless) ────────────────────────

/// Severity layout — sourcé story §5.5 :
/// - `error` si `longest_sightline_m > 40.0` OR `min_cover_spacing_m < 3.0`
///   OR `cover_low_count == 0` quand palette inclut cover_low_cluster
/// - `warn` si `longest_sightline_m > 35.0` (proche limite) OR
///   `instances_skipped > 0`
/// - `info` si layout vide (transitoire startup)
/// - `ok` sinon
pub fn severity_for_layout(
    longest_sightline_m: f32,
    min_cover_spacing_m: f32,
    cover_low_count: u32,
    palette_expects_cover_low: bool,
    instances_skipped: u32,
    modules_placed: usize,
) -> &'static str {
    if modules_placed == 0 {
        return "info";
    }
    if longest_sightline_m > SIGHTLINE_MAX_M {
        return "error";
    }
    if min_cover_spacing_m.is_finite() && min_cover_spacing_m < 3.0 {
        return "error";
    }
    if palette_expects_cover_low && cover_low_count == 0 {
        return "error";
    }
    if longest_sightline_m > 35.0 {
        return "warn";
    }
    if instances_skipped > 0 {
        return "warn";
    }
    "ok"
}

/// Next-step actionnable — toujours pointer vers TOML edit + sensor.
pub fn next_step_for_layout(
    longest_sightline_m: f32,
    min_cover_spacing_m: f32,
    cover_low_count: u32,
    palette_expects_cover_low: bool,
    instances_skipped: u32,
    modules_placed: usize,
) -> &'static str {
    if modules_placed == 0 {
        return "No modules placed yet. If stage Ready and palette non-empty, check `forgia2_stage.json` state — palette may be empty in `roguelite_stages.toml`.";
    }
    if longest_sightline_m > SIGHTLINE_MAX_M {
        return "Sight-line PlayerSpawn↔BossPad exceeds 40m (COD WW2 engagement comfort). Increase `cover_high_wall.count` in current stage palette in `assets/genomes/roguelite_stages.toml`.";
    }
    if min_cover_spacing_m.is_finite() && min_cover_spacing_m < 3.0 {
        return "Cover spacing violated (< 3.0m, Level Design Book rule). Reduce `cover_low_cluster.count` or increase `arena_extent_m` in stage def.";
    }
    if palette_expects_cover_low && cover_low_count == 0 {
        return "Palette expected cover_low but 0 placed. Check `forgia2_stage_layout.json instances_skipped` — palette may be too dense for extent.";
    }
    if longest_sightline_m > 35.0 {
        return "Sight-line approaching 40m limit. Consider adding 1 CoverHigh between PlayerSpawn and BossPad — edit `assets/genomes/roguelite_stages.toml`.";
    }
    if instances_skipped > 0 {
        return "Some module instances failed placement (dart-throw exhausted). Reduce `count` or increase `arena_extent_m`.";
    }
    "Layout healthy. Stage spatial identity active."
}

// ─── La géométrie POSÉE — story-690 ─────────────────────────────────────────
//
// Tout ce qui précède ne mesurait que le solveur de modules. Or il ne produit
// rien : les deux cartes autorées le coupent, les deux autres n'ont pas de
// palette. Le capteur rendait donc `ok` avec `longest_sightline_m: 0.0`,
// `min_cover_spacing_m: -1.0` et tous les compteurs à zéro — exactement le
// capteur aveugle que `map-design-patterns.md` §13 interdit.
//
// Ce bloc mesure la géométrie RÉELLEMENT posée (`ArenaGeometry`), quel que soit
// le générateur qui l'a produite, et refuse de conclure quand il n'a rien vu.

/// Combien de rayons pour le profil de portées.
///
/// C'est une RÉSOLUTION DE MESURE, pas un réglage de gameplay : 64 rayons =
/// un tous les 5,6°, soit plus fin que la largeur angulaire d'un abri de 2 m vu
/// à 20 m (5,7°). Aucun abri ne peut donc se glisser entre deux rayons.
const PROFILE_RAYS: u32 = 64;

/// Au-delà de cette distance, l'arsenal perd 40 % de ses dégâts
/// (`reference_weapon_stats_real_source_viewmodel_arena`). Une carte dont la
/// plupart des lignes dépassent ce seuil se joue en pénalité permanente.
const WEAPON_FULL_DAMAGE_M: f32 = 30.0;

/// Ce que le capteur a mesuré sur la géométrie posée. Séparé du reste pour que
/// la taille d'échantillon voyage AVEC les valeurs qu'elle qualifie.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeometryVerdict {
    /// Nombre de solides mesurés. **Zéro = aveugle, jamais vert.**
    pub solids: usize,
    pub covers: usize,
    pub covers_expected: f32,
    /// `expected / covers`. 3,0 = sous-couverte d'un facteur 3.
    pub deficit_factor: f32,
    pub min_spacing_m: f32,
    pub open_radius_m: f32,
    pub sightline_median_m: f32,
    pub sightline_max_m: f32,
    pub frac_over_falloff: f32,
}

/// Severity de la géométrie posée, dans l'ordre de gravité.
///
/// ⚠️ **On ne regagne jamais le vert en abaissant un seuil** (§13). Les trois
/// seuils utilisés sont sourcés ailleurs et ne s'ajustent pas ici :
/// `COVER_SPACING_MIN_M` / `COVER_SPACING_MAX_M` (Watch Dogs, Gears),
/// `SIGHTLINE_MAX_M` (COD WW2 / TF2).
pub fn severity_for_geometry(v: &GeometryVerdict, authored_pending: u32) -> &'static str {
    if v.solids == 0 {
        return "info"; // aveugle — voir next_step
    }
    if authored_pending > 0 {
        return "info"; // échantillon incomplet, GLB en vol
    }
    // Un point d'apparition sans aucun repli à portée : la salle est un stand de
    // tir. 10 m = le maximum de la bande d'espacement sourcée — au-delà, aucune
    // doctrine ne considère qu'un abri est « à portée ».
    if v.open_radius_m > COVER_SPACING_MAX_M {
        return "error";
    }
    // Moins de la MOITIÉ du couvert attendu. Le seuil est volontairement lâche :
    // il ne prétend pas trancher « bien couvert », seulement « manifestement pas ».
    if v.deficit_factor >= 2.0 {
        return "error";
    }
    if v.min_spacing_m.is_finite() && v.min_spacing_m < COVER_SPACING_MIN_M {
        return "error";
    }
    if v.sightline_max_m > SIGHTLINE_MAX_M {
        return "warn";
    }
    // Une carte dont la majorité des lignes est hors pleine puissance se joue en
    // pénalité de dégâts permanente.
    if v.frac_over_falloff > 0.5 {
        return "warn";
    }
    "ok"
}

/// Next-step actionnable pour la géométrie posée.
pub fn next_step_for_geometry(v: &GeometryVerdict, authored_pending: u32) -> String {
    if v.solids == 0 {
        return "AVEUGLE : aucun solide mesure. Le capteur ne dit PAS que l'arene est vide, il dit qu'il n'a rien vu. Verifier que le stage est Ready (forgia2_stage.json) et que le decor a ete planifie (log '[decor] planned N GLB props').".to_string();
    }
    if authored_pending > 0 {
        return format!(
            "ECHANTILLON INCOMPLET : {authored_pending} piece(s) autoree(s) dont le GLB n'est pas arrive. Mesure reprise des chargement termine — ne rien conclure d'ici la."
        );
    }
    if v.open_radius_m > COVER_SPACING_MAX_M {
        return format!(
            "STAND DE TIR : aucun abri a moins de {:.0} m du point d'apparition (bande sourcee 3-10 m). Baisser `decor_cover_radius_min_m` dans assets/genomes/roguelite/roguelite_decor.toml sous le rayon de l'anneau d'apparition ennemi le plus proche.",
            v.open_radius_m
        );
    }
    if v.deficit_factor >= 2.0 {
        return format!(
            "SOUS-COUVERTE d'un facteur {:.1} : {} abris pour {:.0} attendus a l'espacement le plus LACHE de la bande sourcee (10 m). Ce n'est pas un ecart de gout. Baisser `decor_cover_spacing_m` ou elargir l'anneau de couvert (roguelite_decor.toml).",
            v.deficit_factor, v.covers, v.covers_expected
        );
    }
    if v.min_spacing_m.is_finite() && v.min_spacing_m < COVER_SPACING_MIN_M {
        return format!(
            "ABRIS ENTASSES : {:.1} m entre deux abris, minimum source {COVER_SPACING_MIN_M:.0} m (Level Design Book, 'less cover is better'). Augmenter `decor_cover_spacing_m`.",
            v.min_spacing_m
        );
    }
    if v.sightline_max_m > SIGHTLINE_MAX_M {
        return format!(
            "LIGNE DE {:.0} m depuis l'apparition, confort d'engagement {SIGHTLINE_MAX_M:.0} m (COD WW2 / TF2). Ajouter un solide haut sur cet axe, ou assumer la carte longue et le documenter.",
            v.sightline_max_m
        );
    }
    if v.frac_over_falloff > 0.5 {
        return format!(
            "{:.0} % des lignes depassent {WEAPON_FULL_DAMAGE_M:.0} m — l'arsenal y perd 40 % de ses degats. La carte se joue en penalite permanente : densifier le couvert ou reduire l'extent du stage.",
            v.frac_over_falloff * 100.0
        );
    }
    format!(
        "Geometrie saine sur {} solides mesures : {} abris, portee mediane {:.0} m, repli a {:.1} m de l'apparition.",
        v.solids, v.covers, v.sightline_median_m, v.open_radius_m
    )
}

/// Mesure la géométrie posée. Pur sur les données de la ressource.
pub fn measure(geometry: &crate::ArenaGeometry) -> GeometryVerdict {
    let (sx, sz) = geometry.player_spawn;
    let radius = geometry.playable_radius_m;
    let profile = sightline_profile(
        sx,
        sz,
        &geometry.discs,
        &geometry.segs,
        radius,
        if geometry.measured() == 0 {
            0
        } else {
            PROFILE_RAYS
        },
        WEAPON_FULL_DAMAGE_M,
    );
    let cover_pts: Vec<(f32, f32)> = geometry.covers().map(|d| (d.x, d.z)).collect();
    // Le compte attendu se dérive de l'aire, à l'espacement le plus LÂCHE de la
    // bande sourcée : c'est la borne la plus conservatrice, donc celle qu'on ne
    // peut pas contester. Si on est court à 10 m, on l'est à 6 m aussi.
    let expected = covers_expected(disc_area(radius), COVER_SPACING_MAX_M);
    let covers = cover_pts.len();
    GeometryVerdict {
        solids: geometry.measured(),
        covers,
        covers_expected: expected,
        deficit_factor: if covers == 0 {
            if expected > 0.0 {
                f32::INFINITY
            } else {
                0.0
            }
        } else {
            expected / covers as f32
        },
        min_spacing_m: min_spacing_m(&cover_pts),
        open_radius_m: open_radius_m(sx, sz, &geometry.discs, &geometry.segs, radius),
        sightline_median_m: profile.median_m,
        sightline_max_m: profile.max_m,
        frac_over_falloff: profile.frac_over_threshold,
    }
}

/// `INFINITY` / `NaN` → `None`. Un échantillon vide se sérialise en `null`, pas
/// en `-1` : `-1` se lit comme une mesure, `null` se lit comme une absence.
fn finite_or_none(v: f32) -> Option<f32> {
    v.is_finite().then_some(v)
}

// ─── Sensor writer 1Hz ──────────────────────────────────────────────────────

#[derive(Serialize)]
struct LayoutSensorJson<'a> {
    id: &'a str,
    severity: &'a str,
    timestamp_secs: f64,
    stage_id: &'a str,
    // ── Géométrie POSÉE (story-690) — la mesure qui compte ──
    /// Taille de l'échantillon. **0 = le capteur n'a rien vu**, pas « arène vide ».
    solids_measured: usize,
    /// Pièces autorées dont le GLB n'est pas encore arrivé (> 0 = incomplet).
    authored_pending: u32,
    rays: u32,
    playable_radius_m: f32,
    covers_count: usize,
    covers_expected: f32,
    /// `expected / count` — 3,0 = sous-couverte d'un facteur 3.
    cover_deficit_factor: Option<f32>,
    /// Espacement minimal réel entre deux abris. `null` si moins de deux.
    cover_min_spacing_m: Option<f32>,
    /// Rayon sans aucun abri autour du point d'apparition.
    open_radius_at_spawn_m: f32,
    sightline_median_m: f32,
    sightline_max_m: f32,
    /// Part des lignes au-delà de la pleine puissance de l'arsenal (30 m).
    sightline_frac_over_falloff: f32,
    // ── Diagnostic de PALETTE (story-485) — sous-système, pas le verdict ──
    modules_placed: usize,
    cover_low_count: u32,
    cover_high_count: u32,
    sniper_perch_count: u32,
    melee_pit_count: u32,
    flank_route_count: u32,
    longest_sightline_m: f32,
    min_cover_spacing_m: f32,
    instances_skipped: u32,
    module_palette_used: &'a [String],
    /// Story-625 - provenance du layout : "authored" | "procedural".
    layout_source: &'a str,
    /// Story-625 - pieces authored posees (coquille data-driven).
    authored_pieces: u32,
    /// Story-625 - sections authored distinctes (bible).
    authored_sections: u32,
    next_step: &'a str,
    /// Diagnostic de la palette de modules, quand elle a servi. `null` sinon —
    /// une palette absente n'est pas une palette saine.
    palette_note: Option<&'a str>,
}

pub fn write_layout_sensor(
    layout: Res<LayoutResult>,
    geometry: Res<crate::ArenaGeometry>,
    mut last_write: Local<f64>,
) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);

    if now - *last_write < SENSOR_WRITE_PERIOD_SEC {
        return;
    }
    *last_write = now;

    let placed = &layout.placements;
    let cover_low_count = count_by_kind(placed, ModuleKind::CoverCluster);
    let cover_high_count = count_by_kind(placed, ModuleKind::CoverWall);
    let sniper_perch_count = count_by_kind(placed, ModuleKind::SniperPerch);
    let melee_pit_count = count_by_kind(placed, ModuleKind::MeleePit);
    let flank_route_count = count_by_kind(placed, ModuleKind::FlankRoute);

    // BUG-485-06 — Heuristique substring : on considère qu'un id de module
    // contenant "cover_low" promet d'émettre des anchors CoverCluster. C'est un
    // **contrat de nommage** (convention `cover_low_*` dans level_modules.toml),
    // pas une dérivation depuis `ModuleDef.anchor_kinds_emitted`. Refactor vers
    // une dérivation stricte coûterait passer `LevelModulesGenome` au sensor —
    // disproportionné tant que le set de modules reste petit (~4 en V7). Si la
    // palette TOML explose (>20 modules) ou si un module "cover_low_pillar_*"
    // n'émet PAS un CoverCluster, migrer vers query du genome.
    let palette_expects_cover_low = layout
        .module_palette_used
        .iter()
        .any(|s| s.contains("cover_low"));

    let min_spacing_finite = if layout.min_cover_spacing_m.is_finite() {
        layout.min_cover_spacing_m
    } else {
        -1.0 // sentinel JSON-friendly pour "< 2 CoverLow placés"
    };

    // Story-690 — LE VERDICT vient de la géométrie posée, plus de la palette.
    //
    // Avant : un stage autoré rendait « ok » sur le seul fait d'avoir posé des
    // pièces, un stage sans palette rendait « info » — dans les deux cas sans
    // avoir mesuré un seul mètre. Les 4 cartes du jeu passaient par l'une de ces
    // deux branches, donc AUCUNE n'était mesurée.
    let verdict = measure(&geometry);
    let severity = severity_for_geometry(&verdict, geometry.authored_pending);
    let next_step_owned = next_step_for_geometry(&verdict, geometry.authored_pending);

    // Le diagnostic de palette DESCEND au rang de note : il reste utile quand le
    // solveur tourne, il ne décide plus de la santé de l'arène.
    let palette_note: Option<&str> = if layout.layout_source == "authored" {
        Some("Coquille autoree : le solveur de modules est coupe (suppress_procedural_modules). Le verdict porte sur la geometrie posee.")
    } else if placed.is_empty() {
        Some("Aucun module place : le stage n'a pas de module_palette dans roguelite_stages.toml. Le verdict porte sur la geometrie posee (murs de pieces + decor).")
    } else {
        Some(next_step_for_layout(
            layout.longest_sightline_m,
            layout.min_cover_spacing_m,
            cover_low_count,
            palette_expects_cover_low,
            layout.instances_skipped,
            placed.len(),
        ))
    };

    let payload = LayoutSensorJson {
        id: "stage_layout",
        severity,
        timestamp_secs: now,
        stage_id: &layout.stage_id,
        solids_measured: verdict.solids,
        authored_pending: geometry.authored_pending,
        rays: if verdict.solids == 0 { 0 } else { PROFILE_RAYS },
        playable_radius_m: geometry.playable_radius_m,
        covers_count: verdict.covers,
        covers_expected: verdict.covers_expected,
        cover_deficit_factor: finite_or_none(verdict.deficit_factor),
        cover_min_spacing_m: finite_or_none(verdict.min_spacing_m),
        open_radius_at_spawn_m: verdict.open_radius_m,
        sightline_median_m: verdict.sightline_median_m,
        sightline_max_m: verdict.sightline_max_m,
        sightline_frac_over_falloff: verdict.frac_over_falloff,
        modules_placed: placed.len(),
        cover_low_count,
        cover_high_count,
        sniper_perch_count,
        melee_pit_count,
        flank_route_count,
        longest_sightline_m: layout.longest_sightline_m,
        min_cover_spacing_m: min_spacing_finite,
        instances_skipped: layout.instances_skipped,
        module_palette_used: &layout.module_palette_used,
        layout_source: if layout.layout_source.is_empty() {
            "procedural"
        } else {
            &layout.layout_source
        },
        authored_pieces: layout.authored_pieces,
        authored_sections: layout.authored_sections,
        next_step: &next_step_owned,
        palette_note,
    };

    if let Ok(json) = serde_json::to_string_pretty(&payload) {
        let _ = forgia_core::sensor_io::enqueue(SENSOR_PATH, json);
    }
}

// ─── Tests purs ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Story-690 : la géométrie posée ──────────────────────────────────────

    /// Un verdict « tout va bien », duquel on dévie champ par champ.
    fn healthy() -> GeometryVerdict {
        GeometryVerdict {
            solids: 120,
            covers: 60,
            covers_expected: 70.0,
            deficit_factor: 70.0 / 60.0,
            min_spacing_m: 7.5,
            open_radius_m: 6.0,
            sightline_median_m: 18.0,
            sightline_max_m: 33.0,
            frac_over_falloff: 0.2,
        }
    }

    #[test]
    fn a_healthy_arena_is_ok() {
        assert_eq!(severity_for_geometry(&healthy(), 0), "ok");
    }

    #[test]
    fn zero_solids_is_blind_never_ok() {
        // LE défaut d'origine : le capteur rendait « ok » sans avoir rien mesuré.
        let v = GeometryVerdict {
            solids: 0,
            covers: 0,
            covers_expected: 70.0,
            deficit_factor: f32::INFINITY,
            min_spacing_m: f32::INFINITY,
            open_radius_m: 0.0,
            sightline_median_m: 0.0,
            sightline_max_m: 0.0,
            frac_over_falloff: 0.0,
        };
        assert_eq!(severity_for_geometry(&v, 0), "info");
        assert!(next_step_for_geometry(&v, 0).contains("AVEUGLE"));
    }

    #[test]
    fn a_partially_loaded_arena_refuses_to_conclude() {
        // Une mesure prise pendant que les GLB arrivent conclurait « pas de
        // couvert » sur une carte qui en a — et ce serait un faux rouge.
        let mut v = healthy();
        v.covers = 0;
        v.deficit_factor = f32::INFINITY;
        assert_eq!(severity_for_geometry(&v, 3), "info");
        assert!(next_step_for_geometry(&v, 3).contains("INCOMPLET"));
    }

    #[test]
    fn a_spawn_with_no_cover_in_reach_is_an_error() {
        let mut v = healthy();
        v.open_radius_m = 20.0; // le plancher réel de l'anneau de couvert
        assert_eq!(severity_for_geometry(&v, 0), "error");
        assert!(next_step_for_geometry(&v, 0).contains("STAND DE TIR"));
    }

    #[test]
    fn cover_within_the_sourced_band_is_not_flagged() {
        // 10 m est le MAXIMUM de la bande sourcée : à 10 m pile, on ne râle pas.
        let mut v = healthy();
        v.open_radius_m = COVER_SPACING_MAX_M;
        assert_eq!(severity_for_geometry(&v, 0), "ok");
    }

    #[test]
    fn half_the_expected_cover_is_an_error_and_says_the_factor() {
        let mut v = healthy();
        v.covers = 20;
        v.covers_expected = 70.0;
        v.deficit_factor = 3.5;
        assert_eq!(severity_for_geometry(&v, 0), "error");
        let s = next_step_for_geometry(&v, 0);
        assert!(s.contains("SOUS-COUVERTE"), "{s}");
        assert!(s.contains("3.5"), "le facteur doit etre dit : {s}");
    }

    #[test]
    fn piled_up_covers_are_an_error() {
        let mut v = healthy();
        v.min_spacing_m = 1.5;
        assert_eq!(severity_for_geometry(&v, 0), "error");
    }

    #[test]
    fn a_single_cover_has_no_spacing_and_that_is_not_a_violation() {
        // `INFINITY` = échantillon trop petit, pas « espacement parfait » ni
        // « espacement nul ». Il ne doit déclencher aucune alerte d'entassement.
        let mut v = healthy();
        v.covers = 1;
        v.min_spacing_m = f32::INFINITY;
        v.deficit_factor = 1.2;
        assert_eq!(severity_for_geometry(&v, 0), "ok");
        assert_eq!(finite_or_none(f32::INFINITY), None);
    }

    #[test]
    fn a_long_line_warns_but_does_not_condemn() {
        // Une carte longue peut être un choix (Madame Lenoir, sniper 300 m).
        let mut v = healthy();
        v.sightline_max_m = SIGHTLINE_MAX_M + 5.0;
        assert_eq!(severity_for_geometry(&v, 0), "warn");
    }

    #[test]
    fn a_map_played_mostly_out_of_full_damage_range_warns() {
        let mut v = healthy();
        v.frac_over_falloff = 0.8;
        assert_eq!(severity_for_geometry(&v, 0), "warn");
        assert!(next_step_for_geometry(&v, 0).contains("80 %"));
    }

    #[test]
    fn an_empty_arena_geometry_measures_zero_rays() {
        // Le garde-fou : `measure` sur une ressource vide ne doit pas rendre un
        // profil « parfait » de rayons allant au bout.
        let g = crate::ArenaGeometry {
            playable_radius_m: 78.0,
            ..Default::default()
        };
        let v = measure(&g);
        assert_eq!(v.solids, 0);
        assert_eq!(v.sightline_max_m, 0.0);
        assert_eq!(severity_for_geometry(&v, 0), "info");
    }

    #[test]
    fn measure_counts_only_solids_tall_enough_to_hide_behind() {
        use forgia_core::layout::{SolidDisc, SIGHT_BREAK_H_M};
        let mut g = crate::ArenaGeometry {
            playable_radius_m: 60.0,
            ..Default::default()
        };
        g.discs.push(SolidDisc {
            x: 6.0,
            z: 0.0,
            r: 1.5,
            h: SIGHT_BREAK_H_M + 0.5,
        });
        g.discs.push(SolidDisc {
            x: -6.0,
            z: 0.0,
            r: 1.5,
            h: 1.2, // trop bas : masque le corps, pas la vue
        });
        let v = measure(&g);
        assert_eq!(v.solids, 2, "l'echantillon compte TOUT ce qui est pose");
        assert_eq!(v.covers, 1, "mais un seul est un abri");
        // Le repli se mesure à la surface de l'abri haut : 6 − 1,5 = 4,5 m.
        assert!((v.open_radius_m - 4.5).abs() < 0.01, "{}", v.open_radius_m);
    }

    // ── Diagnostic de palette (story-485, rétrogradé au rang de note) ────────

    #[test]
    fn severity_empty_layout_is_info() {
        assert_eq!(
            severity_for_layout(0.0, f32::INFINITY, 0, false, 0, 0),
            "info"
        );
    }

    #[test]
    fn severity_healthy_layout_is_ok() {
        assert_eq!(severity_for_layout(25.0, 4.5, 6, true, 0, 9), "ok");
    }

    #[test]
    fn severity_sightline_over_40_is_error() {
        assert_eq!(severity_for_layout(42.0, 4.0, 6, true, 0, 9), "error");
    }

    #[test]
    fn severity_sightline_35_to_40_is_warn() {
        assert_eq!(severity_for_layout(37.5, 4.0, 6, true, 0, 9), "warn");
    }

    #[test]
    fn severity_cover_spacing_under_3_is_error() {
        assert_eq!(severity_for_layout(25.0, 2.5, 6, true, 0, 9), "error");
    }

    #[test]
    fn severity_palette_expects_cover_low_but_zero_is_error() {
        assert_eq!(
            severity_for_layout(25.0, f32::INFINITY, 0, true, 2, 5),
            "error"
        );
    }

    #[test]
    fn severity_instances_skipped_is_warn() {
        assert_eq!(severity_for_layout(25.0, 4.0, 6, true, 3, 9), "warn");
    }

    #[test]
    fn next_step_sightline_over_40_mentions_toml() {
        let s = next_step_for_layout(42.0, 4.0, 6, true, 0, 9);
        assert!(s.contains("roguelite_stages.toml"));
        assert!(s.contains("cover_high"));
    }

    #[test]
    fn next_step_spacing_violation_mentions_count_or_extent() {
        let s = next_step_for_layout(25.0, 2.5, 6, true, 0, 9);
        assert!(s.contains("count") || s.contains("extent"));
    }

    #[test]
    fn next_step_empty_layout_mentions_stage_sensor() {
        let s = next_step_for_layout(0.0, f32::INFINITY, 0, false, 0, 0);
        assert!(s.contains("forgia2_stage.json") || s.contains("palette"));
    }
}
