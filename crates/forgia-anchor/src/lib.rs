//! forgia-anchor — Generic anchor-point system (Component + helpers + sensor).
//!
//! Story-483 V7 P0 (NEW crate). Mode-agnostic spawn-zone markers consumed by
//! `forgia-stage-arena`, `forgia-mode-roguelite`, `forgia-mode-fps-arena` (M4),
//! `forgia-rpg` (POI signposts), etc.
//!
//! Pattern :
//! - `AnchorPoint` Component porte `kind: AnchorKind` + `slot_index: u32`
//! - `AnchorKind` enum à 11 variantes : `PlayerSpawn`, `PoiSlot`, `Landmark`,
//!   `BossPad`, `Teleporter`, `LootZone` (story-483, indices [0..6) stables),
//!   `CoverLow`, `CoverHigh`, `SniperPerch`, `MeleePit`, `FlankRoute` (story-485)
//! - Helpers déterministes pour placement (`layout_circle`, `layout_grid`)
//! - Sensor `forgia2_anchor.json` 1Hz expose counts par kind + severity/next_step
//!
//! **Invariant rétro-compat (story-485)** : indices [0..6) jamais renumérotés.
//! Variants ajoutés en fin d'enum, indices [6..11) sont les nouveaux cover/lane
//! types. Sensor JSON nouveaux champs additifs uniquement.
//!
//! Sources convention pattern :
//! - Unreal Engine `ActorSpawn` markers + Anchor sockets
//! - Bevy ECS Component idiom (Bevy 0.18 Cheat Book)
//! - Forgia memory `[[reference-pattern-genome-driven-plugin-with-sensor]]`

use bevy::prelude::*;
use serde::Serialize;
use std::fs;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const SENSOR_PATH: &str = "forgia2_anchor.json";
const SENSOR_WRITE_PERIOD_SEC: f64 = 1.0;

// ─── AnchorKind ─────────────────────────────────────────────────────────────

/// Type d'anchor. 11 variantes couvrent l'usage cross-mode V7 :
/// - [0..6) : story-483 baseline (spawn/POI/decor/boss/teleporter/loot)
/// - [6..11) : story-485 arena spatial identity (cover/lane structure)
///
/// **Indices [0..6) jamais renumérotés** — sensor JSON et downstream tooling
/// (consumers `forgia-stage-arena`, `forgia-mode-roguelite`) s'y fient.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnchorKind {
    /// Spawn player position. Typiquement 1 par stage.
    PlayerSpawn,
    /// POI placement (chest, shrine, elite-pad, etc.). N par stage (6-10 typique).
    PoiSlot,
    /// Décoration anchor (pillar, banner, torche). Pas de gameplay.
    Landmark,
    /// Anchor du boss arena floor. Typiquement 0 ou 1 par stage.
    BossPad,
    /// Stage-exit portal. Typiquement 1 par stage.
    Teleporter,
    /// Loot drop magnet radius center. N par stage.
    LootZone,
    /// Cover bas 1.0–1.25 m (Uncharted 4 standard). Crouch behind, peek over.
    CoverLow,
    /// Cover haut ≥ 1.75 m. Full body conceal, blocks ranged sight-line.
    CoverHigh,
    /// Overlook perch ~6 m verticality (Resistance pattern). Sniper vantage point.
    SniperPerch,
    /// Close-quarter brawl zone (cramped, low cover). Favors Tank/melee archetype.
    MeleePit,
    /// Flank path waypoint (AI navigation hint). Pas de mesh — pur AI marker.
    FlankRoute,
}

impl AnchorKind {
    /// Index stable [0..11) pour AtomicU32 array. Ne **jamais renuméroter** —
    /// indice persisté dans sensor JSON, downstream tooling peut s'y fier.
    pub const fn index(self) -> usize {
        match self {
            AnchorKind::PlayerSpawn => 0,
            AnchorKind::PoiSlot => 1,
            AnchorKind::Landmark => 2,
            AnchorKind::BossPad => 3,
            AnchorKind::Teleporter => 4,
            AnchorKind::LootZone => 5,
            AnchorKind::CoverLow => 6,
            AnchorKind::CoverHigh => 7,
            AnchorKind::SniperPerch => 8,
            AnchorKind::MeleePit => 9,
            AnchorKind::FlankRoute => 10,
        }
    }

    pub const fn all() -> [AnchorKind; 11] {
        [
            AnchorKind::PlayerSpawn,
            AnchorKind::PoiSlot,
            AnchorKind::Landmark,
            AnchorKind::BossPad,
            AnchorKind::Teleporter,
            AnchorKind::LootZone,
            AnchorKind::CoverLow,
            AnchorKind::CoverHigh,
            AnchorKind::SniperPerch,
            AnchorKind::MeleePit,
            AnchorKind::FlankRoute,
        ]
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            AnchorKind::PlayerSpawn => "player_spawn",
            AnchorKind::PoiSlot => "poi_slot",
            AnchorKind::Landmark => "landmark",
            AnchorKind::BossPad => "boss_pad",
            AnchorKind::Teleporter => "teleporter",
            AnchorKind::LootZone => "loot_zone",
            AnchorKind::CoverLow => "cover_low",
            AnchorKind::CoverHigh => "cover_high",
            AnchorKind::SniperPerch => "sniper_perch",
            AnchorKind::MeleePit => "melee_pit",
            AnchorKind::FlankRoute => "flank_route",
        }
    }
}

// ─── AnchorPoint Component ──────────────────────────────────────────────────

/// Component attaché à une entité marker (spawnée par `forgia-stage-arena` ou
/// autre loader mode). Pas de mesh ni collider — c'est juste une position
/// taggée que les consumers requêtent via `Query<(&Transform, &AnchorPoint)>`.
///
/// `slot_index` : ordinal stable pour reproductibilité (run replay, debug).
#[derive(Component, Debug, Clone, Copy)]
pub struct AnchorPoint {
    pub kind: AnchorKind,
    pub slot_index: u32,
}

impl AnchorPoint {
    pub const fn new(kind: AnchorKind, slot_index: u32) -> Self {
        Self { kind, slot_index }
    }
}

// ─── Layout helpers (pure, déterministes, testables headless) ───────────────

/// Distribue `n` positions également sur un cercle horizontal (XZ plane) de
/// rayon `radius_m` centré sur `center`. Y des positions = center.y.
///
/// Utilisé pour POI slots et Landmark anchors en arène circulaire.
pub fn layout_circle(n: u32, radius_m: f32, center: Vec3) -> Vec<Vec3> {
    if n == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(n as usize);
    let step = std::f32::consts::TAU / n as f32;
    for i in 0..n {
        let angle = step * i as f32;
        out.push(Vec3::new(
            center.x + radius_m * angle.cos(),
            center.y,
            center.z + radius_m * angle.sin(),
        ));
    }
    out
}

/// Distribue positions en grille `rows × cols` avec `spacing_m` entre cellules,
/// centrée sur `center`. Utilisé pour rangées de Landmarks (colonnes, banners).
pub fn layout_grid(rows: u32, cols: u32, spacing_m: f32, center: Vec3) -> Vec<Vec3> {
    if rows == 0 || cols == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity((rows * cols) as usize);
    let x_offset = (cols as f32 - 1.0) * 0.5 * spacing_m;
    let z_offset = (rows as f32 - 1.0) * 0.5 * spacing_m;
    for r in 0..rows {
        for c in 0..cols {
            out.push(Vec3::new(
                center.x + c as f32 * spacing_m - x_offset,
                center.y,
                center.z + r as f32 * spacing_m - z_offset,
            ));
        }
    }
    out
}

// ─── Stats / Sensor ─────────────────────────────────────────────────────────

/// Resource ECS — Counters par AnchorKind, mirrored to `forgia2_anchor.json` 1Hz.
///
/// Incremented when consumer spawns an `AnchorPoint`. Reset via
/// `reset_anchor_stats` on OnExit(GameMode::Roguelite) (caller responsibility).
#[derive(Resource, Default)]
pub struct AnchorStats {
    counts: [AtomicU32; 11],
}

impl AnchorStats {
    pub fn record(&self, kind: AnchorKind) {
        self.counts[kind.index()].fetch_add(1, Ordering::Relaxed);
    }

    pub fn count(&self, kind: AnchorKind) -> u32 {
        self.counts[kind.index()].load(Ordering::Relaxed)
    }

    pub fn total(&self) -> u32 {
        self.counts.iter().map(|a| a.load(Ordering::Relaxed)).sum()
    }
}

/// Reset counters — call from caller's OnExit cleanup system. Sans ça, les
/// runs successifs cumulent (régression #BUG-441-02 capitalisée memory
/// `[[feedback-stats-reset-between-sessions]]`).
pub fn reset_anchor_stats(stats: &AnchorStats) {
    for c in &stats.counts {
        c.store(0, Ordering::Relaxed);
    }
}

// ─── Plugin ─────────────────────────────────────────────────────────────────

/// Bevy Plugin. Add via `app.add_plugins(ForgiaAnchorPlugin)`.
pub struct ForgiaAnchorPlugin;

impl Plugin for ForgiaAnchorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AnchorStats>()
            .add_systems(Update, write_anchor_sensor);
    }
}

// ─── Sensor writer 1Hz ──────────────────────────────────────────────────────

#[derive(Serialize)]
struct AnchorSensorJson<'a> {
    id: &'a str,
    severity: &'a str,
    timestamp_secs: f64,
    counts: AnchorCountsJson,
    total: u32,
    next_step: &'a str,
}

#[derive(Serialize)]
struct AnchorCountsJson {
    player_spawn: u32,
    poi_slot: u32,
    landmark: u32,
    boss_pad: u32,
    teleporter: u32,
    loot_zone: u32,
    // story-485 — additif uniquement (downstream JSON consumers default to 0)
    cover_low: u32,
    cover_high: u32,
    sniper_perch: u32,
    melee_pit: u32,
    flank_route: u32,
}

fn write_anchor_sensor(stats: Res<AnchorStats>, mut last_write: Local<f64>) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);

    if now - *last_write < SENSOR_WRITE_PERIOD_SEC {
        return;
    }
    *last_write = now;

    let counts = AnchorCountsJson {
        player_spawn: stats.count(AnchorKind::PlayerSpawn),
        poi_slot: stats.count(AnchorKind::PoiSlot),
        landmark: stats.count(AnchorKind::Landmark),
        boss_pad: stats.count(AnchorKind::BossPad),
        teleporter: stats.count(AnchorKind::Teleporter),
        loot_zone: stats.count(AnchorKind::LootZone),
        cover_low: stats.count(AnchorKind::CoverLow),
        cover_high: stats.count(AnchorKind::CoverHigh),
        sniper_perch: stats.count(AnchorKind::SniperPerch),
        melee_pit: stats.count(AnchorKind::MeleePit),
        flank_route: stats.count(AnchorKind::FlankRoute),
    };
    let total = stats.total();
    let severity = severity_for_anchors(total, counts.player_spawn);
    let next_step = next_step_for_anchors(total, counts.player_spawn);

    let payload = AnchorSensorJson {
        id: "anchor",
        severity,
        timestamp_secs: now,
        counts,
        total,
        next_step,
    };

    if let Ok(json) = serde_json::to_string_pretty(&payload) {
        let _ = fs::write(SENSOR_PATH, json);
    }
}

// ─── Severity / Next-step (pure, testable) ──────────────────────────────────

/// Severity pure — `ok` si stage minimalement peuplé, `warn` si vide, `info`
/// pour states transitoires.
pub fn severity_for_anchors(total: u32, player_spawn: u32) -> &'static str {
    if total == 0 {
        "info" // pas encore spawné (menu, transitions)
    } else if player_spawn == 0 {
        "warn" // anchors présents mais pas de spawn player = config cassée
    } else {
        "ok"
    }
}

/// Next-step actionnable — toujours pointer vers un fichier ou une action concrète.
pub fn next_step_for_anchors(total: u32, player_spawn: u32) -> &'static str {
    if total == 0 {
        "No anchors yet. Awaiting stage load (Read forgia2_stage.json for stage state)."
    } else if player_spawn == 0 {
        "Anchors present but no PlayerSpawn anchor. Check stage definition in roguelite_stages.toml — anchor_slots must include player spawn."
    } else {
        "Anchors active. Stage map ready."
    }
}

// ─── Tests purs ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_kind_index_stability_legacy_indices() {
        // story-483 baseline — indices [0..6) JAMAIS renumérotés, lock invariant
        assert_eq!(AnchorKind::PlayerSpawn.index(), 0);
        assert_eq!(AnchorKind::PoiSlot.index(), 1);
        assert_eq!(AnchorKind::Landmark.index(), 2);
        assert_eq!(AnchorKind::BossPad.index(), 3);
        assert_eq!(AnchorKind::Teleporter.index(), 4);
        assert_eq!(AnchorKind::LootZone.index(), 5);
    }

    #[test]
    fn anchor_kind_index_story_485_new_variants() {
        // story-485 — indices [6..11) cover/lane structure
        assert_eq!(AnchorKind::CoverLow.index(), 6);
        assert_eq!(AnchorKind::CoverHigh.index(), 7);
        assert_eq!(AnchorKind::SniperPerch.index(), 8);
        assert_eq!(AnchorKind::MeleePit.index(), 9);
        assert_eq!(AnchorKind::FlankRoute.index(), 10);
    }

    #[test]
    fn anchor_kind_all_covers_11_variants() {
        assert_eq!(AnchorKind::all().len(), 11);
        // Tous distincts
        let mut seen = [false; 11];
        for k in AnchorKind::all() {
            seen[k.index()] = true;
        }
        assert!(seen.iter().all(|b| *b));
    }

    #[test]
    fn anchor_kind_as_str_snake_case() {
        assert_eq!(AnchorKind::PlayerSpawn.as_str(), "player_spawn");
        assert_eq!(AnchorKind::BossPad.as_str(), "boss_pad");
        assert_eq!(AnchorKind::LootZone.as_str(), "loot_zone");
        // story-485
        assert_eq!(AnchorKind::CoverLow.as_str(), "cover_low");
        assert_eq!(AnchorKind::CoverHigh.as_str(), "cover_high");
        assert_eq!(AnchorKind::SniperPerch.as_str(), "sniper_perch");
        assert_eq!(AnchorKind::MeleePit.as_str(), "melee_pit");
        assert_eq!(AnchorKind::FlankRoute.as_str(), "flank_route");
    }

    #[test]
    fn anchor_stats_new_kinds_recordable() {
        let s = AnchorStats::default();
        s.record(AnchorKind::CoverLow);
        s.record(AnchorKind::CoverLow);
        s.record(AnchorKind::CoverHigh);
        s.record(AnchorKind::SniperPerch);
        s.record(AnchorKind::MeleePit);
        s.record(AnchorKind::FlankRoute);
        assert_eq!(s.count(AnchorKind::CoverLow), 2);
        assert_eq!(s.count(AnchorKind::CoverHigh), 1);
        assert_eq!(s.count(AnchorKind::SniperPerch), 1);
        assert_eq!(s.count(AnchorKind::MeleePit), 1);
        assert_eq!(s.count(AnchorKind::FlankRoute), 1);
        assert_eq!(s.total(), 6);
    }

    #[test]
    fn layout_circle_zero_yields_empty() {
        assert!(layout_circle(0, 10.0, Vec3::ZERO).is_empty());
    }

    #[test]
    fn layout_circle_distributes_evenly() {
        let pts = layout_circle(6, 10.0, Vec3::ZERO);
        assert_eq!(pts.len(), 6);
        // Tous à distance ~radius du center
        for p in &pts {
            let d = (p.x * p.x + p.z * p.z).sqrt();
            assert!((d - 10.0).abs() < 1e-4, "point off-radius: {:?} d={}", p, d);
        }
        // Premier point sur axe +X (angle 0)
        assert!((pts[0].x - 10.0).abs() < 1e-4);
        assert!(pts[0].z.abs() < 1e-4);
    }

    #[test]
    fn layout_circle_respects_center() {
        let center = Vec3::new(5.0, 2.0, -3.0);
        let pts = layout_circle(4, 1.0, center);
        for p in &pts {
            assert!((p.y - 2.0).abs() < 1e-6, "y drift {:?}", p);
        }
    }

    #[test]
    fn layout_grid_zero_yields_empty() {
        assert!(layout_grid(0, 5, 1.0, Vec3::ZERO).is_empty());
        assert!(layout_grid(5, 0, 1.0, Vec3::ZERO).is_empty());
    }

    #[test]
    fn layout_grid_count_and_center() {
        let pts = layout_grid(3, 4, 2.0, Vec3::ZERO);
        assert_eq!(pts.len(), 12);
        // Moyenne ~ center (grid centré)
        let avg = pts.iter().fold(Vec3::ZERO, |a, p| a + *p) / 12.0;
        assert!(avg.x.abs() < 1e-4 && avg.z.abs() < 1e-4, "avg {:?}", avg);
    }

    #[test]
    fn anchor_stats_record_and_count() {
        let s = AnchorStats::default();
        s.record(AnchorKind::PoiSlot);
        s.record(AnchorKind::PoiSlot);
        s.record(AnchorKind::PlayerSpawn);
        assert_eq!(s.count(AnchorKind::PoiSlot), 2);
        assert_eq!(s.count(AnchorKind::PlayerSpawn), 1);
        assert_eq!(s.count(AnchorKind::BossPad), 0);
        assert_eq!(s.total(), 3);
    }

    #[test]
    fn anchor_stats_reset() {
        let s = AnchorStats::default();
        s.record(AnchorKind::PoiSlot);
        s.record(AnchorKind::Landmark);
        reset_anchor_stats(&s);
        assert_eq!(s.total(), 0);
    }

    #[test]
    fn severity_empty_is_info() {
        assert_eq!(severity_for_anchors(0, 0), "info");
    }

    #[test]
    fn severity_missing_player_spawn_is_warn() {
        assert_eq!(severity_for_anchors(5, 0), "warn");
    }

    #[test]
    fn severity_healthy_is_ok() {
        assert_eq!(severity_for_anchors(7, 1), "ok");
    }

    #[test]
    fn next_step_empty_points_to_stage_sensor() {
        let s = next_step_for_anchors(0, 0);
        assert!(s.contains("forgia2_stage.json"));
    }

    #[test]
    fn next_step_missing_spawn_points_to_toml() {
        let s = next_step_for_anchors(5, 0);
        assert!(s.contains("roguelite_stages.toml"));
    }
}
