//! Sight-line solver — story-485 phase 4.
//!
//! Pure module placement algorithm. Pas de dépendance Bevy `Commands` /
//! `AssetServer` — fully headless testable.
//!
//! ## Invariants enforced (cf story §5.4)
//!
//! 1. Sight-line PlayerSpawn ↔ BossPad **brisée** par ≥ 1 CoverHigh proche de
//!    la droite (COD WW2 engagement comfort < 40 m).
//! 2. Cover spacing : tout couple (CoverLow, CoverLow) ≥ `min_spacing_m`
//!    (Level Design Book "less cover is better").
//! 3. ≥ 1 SniperPerch placé en bord (distance > 0.6 * extent_m du centre)
//!    si palette l'inclut.
//! 4. MeleePit central (distance < 0.3 * extent_m du centre) si palette inclut.
//! 5. Aucun module dans cercle 8m autour PlayerSpawn (anti-cheese spawn camp).
//! 6. Tous modules dans cercle inscrit hex `0.866 * extent_m`.
//!
//! ## Algorithme
//!
//! - Seed deterministe via `splitmix64` (cf parent crate).
//! - Dart-throw rejection sampling (max 200 essais/module).
//! - Pour SniperPerch et MeleePit (singletons par convention `density=0`) :
//!   placement contraint (edge/center) plutôt que random.
//! - Pour CoverHigh, si BossPad présent, premier exemplaire placé proche du
//!   midpoint Player↔Boss pour casser la sight-line.

use crate::splitmix64;
use bevy::prelude::*;
use forgia_anchor::AnchorKind;
use forgia_level_presets::{
    parse_anchor_kind, HeightClass, ModuleDef, ModuleKind, ModulePaletteEntry,
};
use std::collections::HashMap;

/// Demi-largeur du cercle inscrit dans un hexagone régulier de "rayon" R
/// (R = distance centre→sommet). cos(π/6) = √3/2 ≈ 0.866.
pub const HEX_INSCRIBED_RATIO: f32 = 0.866_025_4;

/// Rayon de safety autour du PlayerSpawn — anti-cheese spawn camp.
pub const SPAWN_SAFETY_RADIUS_M: f32 = 8.0;

/// Tentatives max de dart-throw par module avant abandon.
pub const MAX_PLACEMENT_ATTEMPTS: u32 = 200;

/// Seuil engagement distance comfort (COD WW2 / TF2 doctrine).
pub const SIGHTLINE_MAX_M: f32 = 40.0;

/// Ratio de distance min au centre pour SniperPerch (edge constraint).
pub const SNIPER_PERCH_MIN_DIST_RATIO: f32 = 0.6;

/// Ratio de distance max au centre pour MeleePit (center constraint).
pub const MELEE_PIT_MAX_DIST_RATIO: f32 = 0.3;

/// Distance perpendiculaire au segment Player↔Boss en-dessous de laquelle un
/// blocker High/Tall est considéré comme cassant la sight-line. Valeur dérivée
/// de la largeur d'épaule joueur (~0.6 m) × facteur sécurité ≈ 5x. À tuner si
/// stages compacts <30m introduits (story-485 BUG-485-03).
pub const SIGHTLINE_BREAK_PERP_M: f32 = 3.0;

// ─── ModulePlacement ────────────────────────────────────────────────────────

/// Résultat d'un placement individuel. Consumer (`spawn_stage_arena_on_request`
/// Phase 5) spawn le prefab GLB à `position` + record `AnchorPoint` + `AnchorStats`.
#[derive(Debug, Clone)]
pub struct ModulePlacement {
    /// Position monde (X, Y=0 pour V1, Z).
    pub position: Vec3,
    /// Id du module dans `LevelModulesGenome` (e.g. "cover_low_cluster").
    pub module_id: String,
    /// Kind résolu depuis `ModuleDef.kind`.
    pub kind: ModuleKind,
    /// Index dans `ModuleDef.prop_palette` du prop choisi (weighted RNG).
    pub prop_index: usize,
    /// Anchor primaire émis — résolu depuis `prop_palette[prop_index].anchor_kind`.
    pub anchor_kind: AnchorKind,
    /// Hauteur — utilisée par sight-line check + downstream AI.
    pub height_class: HeightClass,
}

// ─── place_modules — main entry point ───────────────────────────────────────

/// Place les modules dans l'arène selon palette + invariants.
///
/// `extent_m` : demi-côté de l'hex (= distance centre→sommet).
/// `modules` : HashMap référence depuis `LevelModulesGenome.modules`.
/// `palette` : liste ordonnée d'instructions (id + count).
/// `player_spawn`, `boss_pad` : positions monde (Y ignoré, projection XZ).
/// `rng_seed` : seed déterministe pour reproductibilité run replay.
///
/// Retourne placements valides. Modules dont la palette référence un id
/// inconnu, ou dont count == 0, sont silently skipped (log côté consumer).
pub fn place_modules(
    extent_m: f32,
    modules: &HashMap<String, ModuleDef>,
    palette: &[ModulePaletteEntry],
    player_spawn: Vec3,
    boss_pad: Option<Vec3>,
    rng_seed: u64,
) -> Vec<ModulePlacement> {
    if extent_m <= 0.0 || palette.is_empty() {
        return Vec::new();
    }

    let max_radius = HEX_INSCRIBED_RATIO * extent_m;
    // Arena centrée sur l'origine par convention story-483 (Vec3::ZERO implicite).
    let mut placements: Vec<ModulePlacement> = Vec::with_capacity(palette.len() * 4);
    let mut rng_state: u64 = rng_seed;

    // Pre-flag : doit-on forcer le 1er CoverHigh sur le midpoint player↔boss ?
    let mut sightline_break_pending = boss_pad.is_some();
    let player_xz = Vec3::new(player_spawn.x, 0.0, player_spawn.z);
    let boss_xz_opt = boss_pad.map(|b| Vec3::new(b.x, 0.0, b.z));

    for entry in palette {
        let Some(def) = modules.get(&entry.id) else {
            continue; // unknown id — skip
        };
        for instance_idx in 0..entry.count {
            let pos_opt = match def.kind {
                ModuleKind::SniperPerch => sample_edge(
                    max_radius,
                    &mut rng_state,
                    player_xz,
                    &placements,
                    def,
                ),
                ModuleKind::MeleePit => sample_center(
                    max_radius,
                    &mut rng_state,
                    player_xz,
                    &placements,
                    def,
                ),
                ModuleKind::CoverWall if sightline_break_pending && instance_idx == 0 => {
                    let mp = midpoint_player_boss(player_xz, boss_xz_opt.unwrap());
                    if is_position_valid(mp, max_radius, player_xz, &placements, def) {
                        sightline_break_pending = false;
                        Some(mp)
                    } else {
                        // fallback dart-throw près du midpoint
                        sample_near(
                            mp,
                            8.0,
                            max_radius,
                            &mut rng_state,
                            player_xz,
                            &placements,
                            def,
                        )
                        .inspect(|_| {
                            sightline_break_pending = false;
                        })
                    }
                }
                _ => sample_dart_throw(
                    max_radius,
                    &mut rng_state,
                    player_xz,
                    &placements,
                    def,
                ),
            };

            if let Some(pos) = pos_opt {
                let (prop_index, height_class) = choose_prop(def, &mut rng_state);
                let anchor_kind = def
                    .prop_palette
                    .get(prop_index)
                    .and_then(|p| parse_anchor_kind(&p.anchor_kind))
                    .unwrap_or(AnchorKind::Landmark);
                placements.push(ModulePlacement {
                    position: pos,
                    module_id: entry.id.clone(),
                    kind: def.kind,
                    prop_index,
                    anchor_kind,
                    height_class,
                });
            }
            // sinon : abandon silent (sensor rejection_attempts surfacera côté Phase 5)
        }
    }

    placements
}

// ─── Sampling helpers ───────────────────────────────────────────────────────

/// Dart-throw uniforme dans le disque max_radius.
fn sample_dart_throw(
    max_radius: f32,
    rng: &mut u64,
    player_xz: Vec3,
    placed: &[ModulePlacement],
    def: &ModuleDef,
) -> Option<Vec3> {
    for _ in 0..MAX_PLACEMENT_ATTEMPTS {
        let p = random_disk_point(max_radius, rng);
        if is_position_valid(p, max_radius, player_xz, placed, def) {
            return Some(p);
        }
    }
    None
}

/// Sampling en bord (distance > 0.6 * max_radius) pour SniperPerch.
fn sample_edge(
    max_radius: f32,
    rng: &mut u64,
    player_xz: Vec3,
    placed: &[ModulePlacement],
    def: &ModuleDef,
) -> Option<Vec3> {
    let min_dist = SNIPER_PERCH_MIN_DIST_RATIO * max_radius;
    for _ in 0..MAX_PLACEMENT_ATTEMPTS {
        let p = random_annulus_point(min_dist, max_radius, rng);
        if is_position_valid(p, max_radius, player_xz, placed, def) {
            return Some(p);
        }
    }
    None
}

/// Sampling central (distance < 0.3 * max_radius) pour MeleePit.
fn sample_center(
    max_radius: f32,
    rng: &mut u64,
    player_xz: Vec3,
    placed: &[ModulePlacement],
    def: &ModuleDef,
) -> Option<Vec3> {
    let max_dist = MELEE_PIT_MAX_DIST_RATIO * max_radius;
    for _ in 0..MAX_PLACEMENT_ATTEMPTS {
        let p = random_disk_point(max_dist, rng);
        if is_position_valid(p, max_radius, player_xz, placed, def) {
            return Some(p);
        }
    }
    None
}

/// Sampling dans une zone autour d'un point cible (pour CoverHigh forced sight-line break).
fn sample_near(
    target: Vec3,
    radius: f32,
    max_radius: f32,
    rng: &mut u64,
    player_xz: Vec3,
    placed: &[ModulePlacement],
    def: &ModuleDef,
) -> Option<Vec3> {
    for _ in 0..MAX_PLACEMENT_ATTEMPTS {
        let offset = random_disk_point(radius, rng);
        let p = Vec3::new(target.x + offset.x, 0.0, target.z + offset.z);
        if is_position_valid(p, max_radius, player_xz, placed, def) {
            return Some(p);
        }
    }
    None
}

// ─── Validation (pure) ──────────────────────────────────────────────────────

/// Check toutes les contraintes de placement pour `pos`.
fn is_position_valid(
    pos: Vec3,
    max_radius: f32,
    player_xz: Vec3,
    placed: &[ModulePlacement],
    def: &ModuleDef,
) -> bool {
    // Invariant 6 — dans cercle inscrit hex
    let dist_center = (pos.x * pos.x + pos.z * pos.z).sqrt();
    if dist_center > max_radius {
        return false;
    }
    // Invariant 5 — spawn safety
    let dx = pos.x - player_xz.x;
    let dz = pos.z - player_xz.z;
    let dist_player = (dx * dx + dz * dz).sqrt();
    if dist_player < SPAWN_SAFETY_RADIUS_M {
        return false;
    }
    // Invariant 1 (cover spacing) + footprint exclusion
    for p in placed {
        let ddx = pos.x - p.position.x;
        let ddz = pos.z - p.position.z;
        let d = (ddx * ddx + ddz * ddz).sqrt();
        // Footprint exclusion (rayon module précédent). BUG-485-01 fix :
        // `def.footprint_radius_m` est requis (pas de #[serde(default)]) et
        // toujours >= 1.5 dans la palette TOML, donc le .max(height/2) qui
        // existait était dead logic.
        if d < def.footprint_radius_m {
            return false;
        }
        // Cover spacing : entre 2 CoverLow ou 2 CoverHigh, distance ≥ min_spacing_m
        if p.kind == def.kind && d < def.min_spacing_m {
            return false;
        }
    }
    true
}

// ─── Geometry helpers ───────────────────────────────────────────────────────

/// Sample uniforme dans un disque rayon `r`. XZ plane, Y=0.
fn random_disk_point(r: f32, rng: &mut u64) -> Vec3 {
    let u1 = u64_to_unit_f32(splitmix64(rng));
    let u2 = u64_to_unit_f32(splitmix64(rng));
    let radius = r * u1.sqrt(); // uniform area
    let theta = std::f32::consts::TAU * u2;
    Vec3::new(radius * theta.cos(), 0.0, radius * theta.sin())
}

/// Sample uniforme dans un anneau [r_min, r_max].
fn random_annulus_point(r_min: f32, r_max: f32, rng: &mut u64) -> Vec3 {
    let u1 = u64_to_unit_f32(splitmix64(rng));
    let u2 = u64_to_unit_f32(splitmix64(rng));
    // CDF inverse area-uniform
    let r2 = r_min * r_min + u1 * (r_max * r_max - r_min * r_min);
    let radius = r2.sqrt();
    let theta = std::f32::consts::TAU * u2;
    Vec3::new(radius * theta.cos(), 0.0, radius * theta.sin())
}

/// Convertit u64 vers f32 dans [0.0, 1.0).
fn u64_to_unit_f32(n: u64) -> f32 {
    // 24 bits mantissa pour f32
    ((n >> 40) as u32) as f32 / (1u32 << 24) as f32
}

/// Midpoint XZ entre player et boss.
fn midpoint_player_boss(player: Vec3, boss: Vec3) -> Vec3 {
    Vec3::new(
        (player.x + boss.x) * 0.5,
        0.0,
        (player.z + boss.z) * 0.5,
    )
}

/// Weighted RNG selection dans `def.prop_palette`. Retourne (index, height_class).
fn choose_prop(def: &ModuleDef, rng: &mut u64) -> (usize, HeightClass) {
    if def.prop_palette.is_empty() {
        return (0, HeightClass::Low);
    }
    let total: f32 = def.prop_palette.iter().map(|p| p.weight.max(0.0)).sum();
    if total <= 0.0 {
        return (0, def.prop_palette[0].height_class);
    }
    let pick = u64_to_unit_f32(splitmix64(rng)) * total;
    let mut acc = 0.0;
    for (i, p) in def.prop_palette.iter().enumerate() {
        acc += p.weight.max(0.0);
        if pick <= acc {
            return (i, p.height_class);
        }
    }
    let last = def.prop_palette.len() - 1;
    (last, def.prop_palette[last].height_class)
}

// ─── Metrics (sensor consumers) ─────────────────────────────────────────────

/// Plus longue sight-line résiduelle entre PlayerSpawn et BossPad qui n'est PAS
/// brisée par un CoverHigh / SniperPerch / module Tall ou High. Retourne
/// la distance euclidienne XZ si non-brisée, sinon 0.0.
///
/// Approximation : sight-line considérée "brisée" si distance perpendiculaire
/// au segment Player↔Boss < 3.0 m ET dans le segment (projection).
pub fn longest_unbroken_sightline_m(
    player: Vec3,
    boss: Option<Vec3>,
    placed: &[ModulePlacement],
) -> f32 {
    let Some(boss) = boss else {
        return 0.0;
    };
    let p_xz = Vec3::new(player.x, 0.0, player.z);
    let b_xz = Vec3::new(boss.x, 0.0, boss.z);
    let seg = b_xz - p_xz;
    let len = (seg.x * seg.x + seg.z * seg.z).sqrt();
    if len < 1e-3 {
        return 0.0;
    }
    let dir_x = seg.x / len;
    let dir_z = seg.z / len;
    // Find any blocker
    for m in placed {
        // Only High/Tall break sight-lines
        if matches!(m.height_class, HeightClass::Low) {
            continue;
        }
        let dx = m.position.x - p_xz.x;
        let dz = m.position.z - p_xz.z;
        let t = dx * dir_x + dz * dir_z;
        if t < 0.0 || t > len {
            continue;
        }
        let perp_x = dx - t * dir_x;
        let perp_z = dz - t * dir_z;
        let perp = (perp_x * perp_x + perp_z * perp_z).sqrt();
        if perp < SIGHTLINE_BREAK_PERP_M {
            return 0.0; // broken
        }
    }
    len
}

/// Distance minimale entre tout couple de modules `CoverCluster` placés.
/// Retourne `f32::INFINITY` si < 2 instances. Sensor : alerte si < 3.0
/// (Level Design Book "less cover is better" rule).
///
/// **Note nommage (BUG-485-04 fix)** : ne mesure que `CoverCluster` kind, pas
/// les anchors `cover_low` émis par d'autres kinds. Pour story-486 cover-aware
/// AI, derive depuis `Query<&AnchorPoint>` filtré par `kind == CoverLow`.
pub fn min_cover_cluster_spacing_m(placed: &[ModulePlacement]) -> f32 {
    let lows: Vec<&ModulePlacement> = placed
        .iter()
        .filter(|m| m.kind == ModuleKind::CoverCluster)
        .collect();
    if lows.len() < 2 {
        return f32::INFINITY;
    }
    let mut min_d = f32::INFINITY;
    for i in 0..lows.len() {
        for j in (i + 1)..lows.len() {
            let dx = lows[i].position.x - lows[j].position.x;
            let dz = lows[i].position.z - lows[j].position.z;
            let d = (dx * dx + dz * dz).sqrt();
            if d < min_d {
                min_d = d;
            }
        }
    }
    min_d
}

/// Count modules d'un kind donné.
pub fn count_by_kind(placed: &[ModulePlacement], kind: ModuleKind) -> u32 {
    placed.iter().filter(|m| m.kind == kind).count() as u32
}

// ─── Tests purs ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use forgia_level_presets::PropEntry;

    fn make_module(
        kind: ModuleKind,
        anchor_kind: &str,
        height: HeightClass,
        min_spacing: f32,
        footprint: f32,
    ) -> ModuleDef {
        ModuleDef {
            kind,
            prop_palette: vec![PropEntry {
                prefab: "models/test.glb".into(),
                weight: 1.0,
                anchor_kind: anchor_kind.into(),
                height_class: height,
            }],
            density_per_m2: 0.0,
            min_spacing_m: min_spacing,
            footprint_radius_m: footprint,
            allowed_biomes: vec![],
            anchor_kinds_emitted: vec![anchor_kind.into()],
        }
    }

    fn make_palette() -> (HashMap<String, ModuleDef>, Vec<ModulePaletteEntry>) {
        let mut modules = HashMap::new();
        modules.insert(
            "cover_low".into(),
            make_module(ModuleKind::CoverCluster, "cover_low", HeightClass::Low, 3.0, 1.5),
        );
        modules.insert(
            "cover_high".into(),
            make_module(ModuleKind::CoverWall, "cover_high", HeightClass::High, 5.0, 2.5),
        );
        modules.insert(
            "sniper".into(),
            make_module(ModuleKind::SniperPerch, "sniper_perch", HeightClass::Tall, 0.0, 4.0),
        );
        modules.insert(
            "pit".into(),
            make_module(ModuleKind::MeleePit, "melee_pit", HeightClass::Low, 0.0, 6.0),
        );
        let palette = vec![
            ModulePaletteEntry { id: "cover_low".into(), count: 6 },
            ModulePaletteEntry { id: "cover_high".into(), count: 2 },
            ModulePaletteEntry { id: "sniper".into(), count: 1 },
            ModulePaletteEntry { id: "pit".into(), count: 1 },
        ];
        (modules, palette)
    }

    #[test]
    fn place_modules_empty_palette_yields_empty() {
        let modules = HashMap::new();
        let r = place_modules(90.0, &modules, &[], Vec3::ZERO, None, 42);
        assert!(r.is_empty());
    }

    #[test]
    fn place_modules_zero_extent_yields_empty() {
        let (modules, palette) = make_palette();
        let r = place_modules(0.0, &modules, &palette, Vec3::ZERO, None, 42);
        assert!(r.is_empty());
    }

    #[test]
    fn place_modules_respects_min_spacing() {
        let (modules, palette) = make_palette();
        let placements = place_modules(
            90.0,
            &modules,
            &palette,
            Vec3::new(0.0, 0.0, -85.0),
            Some(Vec3::new(0.0, 0.0, 85.0)),
            12345,
        );
        // Tous couples CoverCluster doivent respecter min_spacing_m = 3.0
        let lows: Vec<&ModulePlacement> = placements
            .iter()
            .filter(|m| m.kind == ModuleKind::CoverCluster)
            .collect();
        for i in 0..lows.len() {
            for j in (i + 1)..lows.len() {
                let dx = lows[i].position.x - lows[j].position.x;
                let dz = lows[i].position.z - lows[j].position.z;
                let d = (dx * dx + dz * dz).sqrt();
                assert!(
                    d >= 3.0 - 0.01,
                    "CoverCluster spacing violated: {:.2}m (lows={})",
                    d,
                    lows.len()
                );
            }
        }
    }

    #[test]
    fn place_modules_within_hex_inscribed_circle() {
        let (modules, palette) = make_palette();
        let extent = 90.0;
        let placements = place_modules(
            extent,
            &modules,
            &palette,
            Vec3::new(0.0, 0.0, -85.0),
            Some(Vec3::new(0.0, 0.0, 85.0)),
            42,
        );
        let max_r = HEX_INSCRIBED_RATIO * extent;
        for p in &placements {
            let d = (p.position.x * p.position.x + p.position.z * p.position.z).sqrt();
            assert!(
                d <= max_r + 0.01,
                "module outside inscribed hex: d={:.2} max={:.2}",
                d,
                max_r
            );
        }
    }

    #[test]
    fn place_modules_no_module_in_spawn_safety() {
        let (modules, palette) = make_palette();
        let player = Vec3::new(0.0, 0.0, -85.0);
        let placements = place_modules(
            90.0,
            &modules,
            &palette,
            player,
            Some(Vec3::new(0.0, 0.0, 85.0)),
            99,
        );
        for p in &placements {
            let dx = p.position.x - player.x;
            let dz = p.position.z - player.z;
            let d = (dx * dx + dz * dz).sqrt();
            assert!(
                d >= SPAWN_SAFETY_RADIUS_M - 0.01,
                "module within spawn safety: d={:.2} kind={:?}",
                d,
                p.kind
            );
        }
    }

    #[test]
    fn place_modules_sniper_perch_at_edge() {
        let (modules, palette) = make_palette();
        let extent = 90.0;
        let placements = place_modules(
            extent,
            &modules,
            &palette,
            Vec3::new(0.0, 0.0, -85.0),
            Some(Vec3::new(0.0, 0.0, 85.0)),
            7,
        );
        let max_r = HEX_INSCRIBED_RATIO * extent;
        let min_dist = SNIPER_PERCH_MIN_DIST_RATIO * max_r;
        let snipers: Vec<&ModulePlacement> = placements
            .iter()
            .filter(|m| m.kind == ModuleKind::SniperPerch)
            .collect();
        assert!(!snipers.is_empty(), "expected ≥ 1 SniperPerch");
        for s in &snipers {
            let d = (s.position.x * s.position.x + s.position.z * s.position.z).sqrt();
            assert!(
                d >= min_dist - 0.01,
                "SniperPerch not at edge: d={:.2} min={:.2}",
                d,
                min_dist
            );
        }
    }

    #[test]
    fn place_modules_melee_pit_central() {
        let (modules, palette) = make_palette();
        let extent = 90.0;
        let placements = place_modules(
            extent,
            &modules,
            &palette,
            Vec3::new(0.0, 0.0, -85.0),
            Some(Vec3::new(0.0, 0.0, 85.0)),
            13,
        );
        let max_r = HEX_INSCRIBED_RATIO * extent;
        let max_dist = MELEE_PIT_MAX_DIST_RATIO * max_r;
        let pits: Vec<&ModulePlacement> = placements
            .iter()
            .filter(|m| m.kind == ModuleKind::MeleePit)
            .collect();
        assert!(!pits.is_empty(), "expected ≥ 1 MeleePit");
        for p in &pits {
            let d = (p.position.x * p.position.x + p.position.z * p.position.z).sqrt();
            assert!(
                d <= max_dist + 0.01,
                "MeleePit not central: d={:.2} max={:.2}",
                d,
                max_dist
            );
        }
    }

    #[test]
    fn place_modules_deterministic_same_seed() {
        let (modules, palette) = make_palette();
        let r1 = place_modules(
            90.0,
            &modules,
            &palette,
            Vec3::new(0.0, 0.0, -85.0),
            Some(Vec3::new(0.0, 0.0, 85.0)),
            42,
        );
        let r2 = place_modules(
            90.0,
            &modules,
            &palette,
            Vec3::new(0.0, 0.0, -85.0),
            Some(Vec3::new(0.0, 0.0, 85.0)),
            42,
        );
        assert_eq!(r1.len(), r2.len(), "len mismatch");
        for (a, b) in r1.iter().zip(r2.iter()) {
            assert_eq!(a.module_id, b.module_id);
            assert_eq!(a.kind, b.kind);
            assert!(
                (a.position.x - b.position.x).abs() < 1e-4
                    && (a.position.z - b.position.z).abs() < 1e-4,
                "position drift {:?} vs {:?}",
                a.position,
                b.position
            );
        }
    }

    #[test]
    fn place_modules_different_seed_different_layout() {
        let (modules, palette) = make_palette();
        let r1 = place_modules(
            90.0,
            &modules,
            &palette,
            Vec3::new(0.0, 0.0, -85.0),
            Some(Vec3::new(0.0, 0.0, 85.0)),
            42,
        );
        let r2 = place_modules(
            90.0,
            &modules,
            &palette,
            Vec3::new(0.0, 0.0, -85.0),
            Some(Vec3::new(0.0, 0.0, 85.0)),
            999,
        );
        // Au moins une position cluster doit différer (sniper/pit peuvent coincider rarement)
        let any_diff = r1.iter().zip(r2.iter()).any(|(a, b)| {
            (a.position.x - b.position.x).abs() > 0.1 || (a.position.z - b.position.z).abs() > 0.1
        });
        assert!(any_diff, "expected variation between seeds");
    }

    #[test]
    fn place_modules_unknown_id_skipped() {
        let (modules, _) = make_palette();
        let palette = vec![
            ModulePaletteEntry { id: "does_not_exist".into(), count: 3 },
            ModulePaletteEntry { id: "cover_low".into(), count: 2 },
        ];
        let r = place_modules(
            90.0,
            &modules,
            &palette,
            Vec3::new(0.0, 0.0, -85.0),
            None,
            1,
        );
        // Seuls les 2 cover_low (max) placés
        assert!(r.len() <= 2);
        assert!(r.iter().all(|p| p.module_id == "cover_low"));
    }

    #[test]
    fn place_modules_breaks_sightline_player_boss() {
        let (modules, palette) = make_palette();
        let player = Vec3::new(0.0, 0.0, -85.0);
        let boss = Vec3::new(0.0, 0.0, 85.0);
        let placements = place_modules(90.0, &modules, &palette, player, Some(boss), 42);
        let unbroken = longest_unbroken_sightline_m(player, Some(boss), &placements);
        // Soit la sight-line est brisée (=0), soit elle est < SIGHTLINE_MAX_M
        assert!(
            unbroken < SIGHTLINE_MAX_M + 5.0,
            "sight-line not broken: {:.2}m (CoverHigh should sit on midpoint)",
            unbroken
        );
    }

    #[test]
    fn longest_unbroken_no_boss_is_zero() {
        let placements = vec![];
        let r = longest_unbroken_sightline_m(Vec3::ZERO, None, &placements);
        assert_eq!(r, 0.0);
    }

    #[test]
    fn min_cover_cluster_spacing_less_than_two_returns_infinity() {
        let placements = vec![];
        assert_eq!(min_cover_cluster_spacing_m(&placements), f32::INFINITY);
    }

    #[test]
    fn count_by_kind_correct() {
        let (modules, palette) = make_palette();
        let placements = place_modules(
            90.0,
            &modules,
            &palette,
            Vec3::new(0.0, 0.0, -85.0),
            Some(Vec3::new(0.0, 0.0, 85.0)),
            42,
        );
        // Each placement should be counted exactly once
        let total: u32 = ModuleKind::all()
            .iter()
            .map(|&k| count_by_kind(&placements, k))
            .sum();
        assert_eq!(total as usize, placements.len());
    }

    #[test]
    fn place_modules_no_boss_pad_coverwalls_placed_freely() {
        // BUG-485-05 fix : stage sans boss_pad → sightline_break_pending = false,
        // CoverWalls placés via dart-throw normal (pas de midpoint forcé).
        let (modules, palette) = make_palette();
        let placements = place_modules(
            90.0,
            &modules,
            &palette,
            Vec3::new(0.0, 0.0, -85.0),
            None, // no boss
            42,
        );
        // CoverWalls quand même placés (au moins 1 si dart-throw réussit)
        let walls_count = placements
            .iter()
            .filter(|m| m.kind == ModuleKind::CoverWall)
            .count();
        // Pas d'invariant midpoint, juste que la fonction ne crash pas
        assert!(walls_count <= 2, "palette demande 2 CoverWalls max");
        // longest_unbroken_sightline_m doit retourner 0 quand boss absent
        assert_eq!(
            longest_unbroken_sightline_m(Vec3::new(0.0, 0.0, -85.0), None, &placements),
            0.0
        );
    }

    #[test]
    fn weighted_prop_selection_picks_in_range() {
        let mut rng = 42u64;
        let def = ModuleDef {
            kind: ModuleKind::CoverCluster,
            prop_palette: vec![
                PropEntry {
                    prefab: "a.glb".into(),
                    weight: 0.6,
                    anchor_kind: "cover_low".into(),
                    height_class: HeightClass::Low,
                },
                PropEntry {
                    prefab: "b.glb".into(),
                    weight: 0.4,
                    anchor_kind: "cover_low".into(),
                    height_class: HeightClass::Low,
                },
            ],
            density_per_m2: 0.0,
            min_spacing_m: 3.0,
            footprint_radius_m: 1.5,
            allowed_biomes: vec![],
            anchor_kinds_emitted: vec!["cover_low".into()],
        };
        for _ in 0..50 {
            let (idx, _) = choose_prop(&def, &mut rng);
            assert!(idx < 2, "prop index out of range: {}", idx);
        }
    }

    /// AC6 story-485 — assure que la palette prod `crypts_of_anvil`
    /// (cover_low_cluster footprint=6m count=6 dans extent=80m) place bien
    /// 6 CoverLow sans instances_skipped. Détecte une régression palette TOML.
    #[test]
    fn place_modules_crypts_palette_ac6_no_skips() {
        let mut modules = HashMap::new();
        // Mirroring assets/genomes/level_modules.toml prod values
        modules.insert(
            "cover_low_cluster".into(),
            make_module(ModuleKind::CoverCluster, "cover_low", HeightClass::Low, 3.0, 6.0),
        );
        modules.insert(
            "cover_high_wall".into(),
            make_module(ModuleKind::CoverWall, "cover_high", HeightClass::High, 5.0, 8.0),
        );
        modules.insert(
            "sniper_perch".into(),
            make_module(ModuleKind::SniperPerch, "sniper_perch", HeightClass::Tall, 0.0, 4.0),
        );
        modules.insert(
            "melee_pit".into(),
            make_module(ModuleKind::MeleePit, "melee_pit", HeightClass::Low, 0.0, 6.0),
        );
        // Mirroring assets/genomes/roguelite_stages.toml crypts_of_anvil
        let palette = vec![
            ModulePaletteEntry { id: "cover_low_cluster".into(), count: 6 },
            ModulePaletteEntry { id: "cover_high_wall".into(),   count: 2 },
            ModulePaletteEntry { id: "sniper_perch".into(),      count: 1 },
            ModulePaletteEntry { id: "melee_pit".into(),         count: 1 },
        ];
        // 5 seeds — robustesse multi-run replay
        for seed in [1u64, 42, 9876, 12345, 99999] {
            let placements = place_modules(
                80.0,
                &modules,
                &palette,
                Vec3::new(0.0, 0.0, -76.0),
                Some(Vec3::new(0.0, 0.0, 76.0)),
                seed,
            );
            let cover_low = count_by_kind(&placements, ModuleKind::CoverCluster);
            let sniper = count_by_kind(&placements, ModuleKind::SniperPerch);
            let melee = count_by_kind(&placements, ModuleKind::MeleePit);
            assert_eq!(
                cover_low, 6,
                "AC6 cover_low shortfall on seed {} : got {}/6",
                seed, cover_low
            );
            assert!(sniper >= 1, "AC6 sniper shortfall on seed {}", seed);
            assert!(melee >= 1, "AC6 melee shortfall on seed {}", seed);
        }
    }

    /// AC7 story-485 — assure que la palette `forge_sanctum` produit un layout
    /// signature distincte de `crypts_of_anvil` (no sniper + 2 melee + moins
    /// de cover bas). Détecte régression palette TOML qui aplatirait l'identity
    /// spatiale d'un stage.
    #[test]
    fn place_modules_forge_distinct_from_crypts() {
        let mut modules = HashMap::new();
        modules.insert(
            "cover_low_cluster".into(),
            make_module(ModuleKind::CoverCluster, "cover_low", HeightClass::Low, 3.0, 6.0),
        );
        modules.insert(
            "cover_high_wall".into(),
            make_module(ModuleKind::CoverWall, "cover_high", HeightClass::High, 5.0, 8.0),
        );
        modules.insert(
            "sniper_perch".into(),
            make_module(ModuleKind::SniperPerch, "sniper_perch", HeightClass::Tall, 0.0, 4.0),
        );
        modules.insert(
            "melee_pit".into(),
            make_module(ModuleKind::MeleePit, "melee_pit", HeightClass::Low, 0.0, 6.0),
        );

        let crypts_palette = vec![
            ModulePaletteEntry { id: "cover_low_cluster".into(), count: 6 },
            ModulePaletteEntry { id: "cover_high_wall".into(),   count: 2 },
            ModulePaletteEntry { id: "sniper_perch".into(),      count: 1 },
            ModulePaletteEntry { id: "melee_pit".into(),         count: 1 },
        ];
        let forge_palette = vec![
            ModulePaletteEntry { id: "cover_low_cluster".into(), count: 4 },
            ModulePaletteEntry { id: "cover_high_wall".into(),   count: 1 },
            ModulePaletteEntry { id: "melee_pit".into(),         count: 2 },
        ];

        let seed = 42u64;
        let crypts = place_modules(
            80.0,
            &modules,
            &crypts_palette,
            Vec3::new(0.0, 0.0, -76.0),
            Some(Vec3::new(0.0, 0.0, 76.0)),
            seed,
        );
        let forge = place_modules(
            80.0,
            &modules,
            &forge_palette,
            Vec3::new(0.0, 0.0, -76.0),
            None,
            seed,
        );

        // Crypts identity = sniper edge + dense low cover + 1 melee
        assert!(count_by_kind(&crypts, ModuleKind::SniperPerch) >= 1);
        assert_eq!(count_by_kind(&crypts, ModuleKind::MeleePit), 1);

        // Forge identity = no sniper + double melee + lighter cover
        assert_eq!(count_by_kind(&forge, ModuleKind::SniperPerch), 0);
        assert_eq!(count_by_kind(&forge, ModuleKind::MeleePit), 2);

        // Signatures (kind, count) doivent différer
        let crypts_sig = (
            count_by_kind(&crypts, ModuleKind::CoverCluster),
            count_by_kind(&crypts, ModuleKind::CoverWall),
            count_by_kind(&crypts, ModuleKind::SniperPerch),
            count_by_kind(&crypts, ModuleKind::MeleePit),
        );
        let forge_sig = (
            count_by_kind(&forge, ModuleKind::CoverCluster),
            count_by_kind(&forge, ModuleKind::CoverWall),
            count_by_kind(&forge, ModuleKind::SniperPerch),
            count_by_kind(&forge, ModuleKind::MeleePit),
        );
        assert_ne!(
            crypts_sig, forge_sig,
            "AC7: forge_sanctum layout signature identique à crypts_of_anvil — perte d'identity spatiale"
        );
    }
}
