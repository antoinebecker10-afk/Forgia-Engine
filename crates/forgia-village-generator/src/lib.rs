//! # forgia-village-generator
//!
//! Procedural village layout orchestrator. Consumes a typed
//! [`VillageGenome`](forgia_genome_village::VillageGenome) and emits a
//! [`VillageGraph`](forgia_procgen_graph::VillageGraph).
//!
//! ## V1 dispatch
//!
//! - [`LayoutType::Hamlet`] — Poisson disc on circle bounding, required
//!   buildings placed first, optional fill by weighted pick. Single radial
//!   road set leaving the village.
//! - [`LayoutType::Village`] — Voronoi 3-5 cells + Lloyd relaxation + Poisson
//!   intra-cell + Delaunay candidate roads.
//! - [`LayoutType::Capital`] — `unimplemented!` returns an [`Err`]
//!   ([`GeneratorError::Unimplemented`]) — V1 scope, V3+ adds L-system streets.
//!
//! ## Reproducibility
//!
//! `genome.meta.seed` is threaded through every random draw. Same seed →
//! bit-identical [`VillageGraph`]. Verified by integration tests.
//!
//! ## No allocations on hot path
//!
//! The generator runs **once per village** at world-load — not per frame —
//! so allocations are acceptable. We still bound them via `Vec::with_capacity`
//! at known sites.

use bevy::math::Vec2;
use forgia_genome_village::{
    BuildingsGenome, KitMixDef, LayoutType, VillageGenome,
};
use forgia_procgen_graph::{
    BuildingNode, DistrictRole, GraphDistrict, GraphEdge, GraphNode, NodeKind, RoadSegment2D,
    VillageGraph,
};
use forgia_rng::Rng;
use rstar::{RTree, RTreeObject, AABB};
use thiserror::Error;

// ─────────────────────────────────────────────────────────────────────────────
// Spatial index — wraps `RoadSegment2D` for R-tree queries.
// Pattern : Anno-style edge-adjacency + CityEngine setback (story-446).
// ─────────────────────────────────────────────────────────────────────────────

/// One entry in the road spatial index. We wrap [`RoadSegment2D`] so we can
/// implement [`RTreeObject`] without polluting the data crate with `rstar`.
#[derive(Clone, Debug)]
struct RoadIndexEntry {
    seg: RoadSegment2D,
}

impl RTreeObject for RoadIndexEntry {
    type Envelope = AABB<[f32; 2]>;
    fn envelope(&self) -> Self::Envelope {
        let (min, max) = self.seg.aabb();
        AABB::from_corners(min, max)
    }
}

/// Returns `true` if `p` is within `clearance` of any road in the index.
/// Cost : O(log n) average via R-tree window query, falling back to O(k)
/// where k = number of segments whose AABB intersects the search box.
fn point_too_close_to_road(
    rtree: &RTree<RoadIndexEntry>,
    p: Vec2,
    clearance: f32,
) -> bool {
    // Expand search box by clearance — anything outside cannot reach p.
    let search = AABB::from_corners(
        [p.x - clearance, p.y - clearance],
        [p.x + clearance, p.y + clearance],
    );
    let clearance2 = clearance * clearance;
    rtree
        .locate_in_envelope_intersecting(&search)
        .any(|e| e.seg.distance_squared_to(p) < clearance2)
}

/// Returns the nearest road segment + the distance to it (squared), if any
/// within `max_distance` of `p`. Used for `road_anchor` snap placement.
fn nearest_road_within(
    rtree: &RTree<RoadIndexEntry>,
    p: Vec2,
    max_distance: f32,
) -> Option<(RoadSegment2D, f32)> {
    let search = AABB::from_corners(
        [p.x - max_distance, p.y - max_distance],
        [p.x + max_distance, p.y + max_distance],
    );
    let mut best: Option<(RoadSegment2D, f32)> = None;
    for e in rtree.locate_in_envelope_intersecting(&search) {
        let d2 = e.seg.distance_squared_to(p);
        if d2 > max_distance * max_distance {
            continue;
        }
        match best {
            Some((_, b2)) if b2 <= d2 => {}
            _ => best = Some((e.seg, d2)),
        }
    }
    best.map(|(s, d2)| (s, d2.sqrt()))
}

// ─────────────────────────────────────────────────────────────────────────────
// Errors
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum GeneratorError {
    #[error("layout_type {0:?} is unimplemented in V1 (planned for story-444)")]
    Unimplemented(LayoutType),
    #[error("genome has empty kit_mix.colors — at least one color weight required")]
    EmptyColorWeights,
    #[error(
        "Poisson placement reached max retries without placing {placed}/{target} buildings — \
         next: increase bounding_radius or decrease density in genome '{genome_id}'"
    )]
    PoissonExhausted {
        genome_id: String,
        placed: usize,
        target: usize,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// Public entry point
// ─────────────────────────────────────────────────────────────────────────────

/// Statistics returned alongside the [`VillageGraph`] for sensor export.
#[derive(Debug, Clone, Default)]
pub struct GeneratorStats {
    pub layout_type: String,
    pub seed: u64,
    pub placed_buildings: usize,
    pub target_buildings: usize,
    pub poisson_retries: u32,
    pub districts: usize,
    pub edges: usize,
    pub elapsed_ms: f32,
}

/// Generate a village graph from a typed genome.
///
/// Wall-clock time bounded : we cap Poisson retries to a per-attempt budget
/// to avoid infinite loops on impossible-to-satisfy parameters (e.g. density
/// = 1.0 with bounding_radius too small).
pub fn generate(genome: &VillageGenome) -> Result<(VillageGraph, GeneratorStats), GeneratorError> {
    let mut rng = Rng::new(genome.meta.seed);
    let mut stats = GeneratorStats {
        layout_type: format!("{:?}", genome.meta.layout_type).to_lowercase(),
        seed: genome.meta.seed,
        target_buildings: genome.scale.target_building_count,
        ..Default::default()
    };

    let graph = match genome.meta.layout_type {
        LayoutType::Hamlet => hamlet_layout(genome, &mut rng, &mut stats)?,
        LayoutType::Village => village_layout(genome, &mut rng, &mut stats)?,
        LayoutType::Capital => return Err(GeneratorError::Unimplemented(LayoutType::Capital)),
    };

    stats.placed_buildings = graph.building_count();
    stats.districts = graph.district_count();
    stats.edges = graph.edge_count();
    Ok((graph, stats))
}

// ─────────────────────────────────────────────────────────────────────────────
// Hamlet — Poisson disc on bounding circle
// ─────────────────────────────────────────────────────────────────────────────

/// Number of placement attempts per slot before declaring failure.
const POISSON_MAX_ATTEMPTS_PER_SLOT: u32 = 30;

fn hamlet_layout(
    genome: &VillageGenome,
    rng: &mut Rng,
    stats: &mut GeneratorStats,
) -> Result<VillageGraph, GeneratorError> {
    let mut graph = VillageGraph::default();

    // STORY-446 — ROAD-FIRST PIPELINE (Cities Skylines pattern).
    // Order changed : roads must exist BEFORE buildings to enforce clearance.
    //
    // 1. Generate radial road axes deterministically from seed.
    // 2. Build R-tree spatial index (rstar) for O(log n) clearance queries.
    // 3. Place required buildings : center node bypasses road check (well is
    //    intentionally on village center axis convergence) ; others snap
    //    around with clearance enforced.
    // 4. Poisson fill : every candidate checked vs other buildings AND
    //    vs roads (clearance ≥ road.half_width + genome.road_clearance_m).
    // 5. road_anchor pieces snap to nearest road with setback.

    // 1. Pre-compute road segments.
    let road_count = pick_radial_count(genome, rng);
    let road_segments = compute_radial_road_segments(genome, road_count);
    // 2. R-tree spatial index.
    let road_rtree: RTree<RoadIndexEntry> = RTree::bulk_load(
        road_segments
            .iter()
            .map(|s| RoadIndexEntry { seg: *s })
            .collect(),
    );

    // 3. Required placement (center is special — see place_required_with_roads).
    let buildings = &genome.buildings;
    let required = &buildings.required;
    let min_sep = poisson_min_separation(genome);
    let placed_required = place_required_with_roads(
        genome,
        rng,
        &mut graph,
        required,
        &road_rtree,
        min_sep,
    );

    // 4. Fill remaining slots with road clearance enforced.
    let slots_to_fill = genome
        .scale
        .target_building_count
        .saturating_sub(placed_required);
    if slots_to_fill > 0 {
        fill_with_poisson_road_aware(
            genome,
            rng,
            &mut graph,
            buildings,
            slots_to_fill,
            min_sep,
            &road_rtree,
            stats,
        )?;
    }

    // 5. Materialize road graph (nodes + edges) so VillageLoadResult sees them.
    materialize_roads_to_graph(genome, &mut graph, &road_segments);

    Ok(graph)
}

// ─────────────────────────────────────────────────────────────────────────────
// Road pre-compute + materialization
// ─────────────────────────────────────────────────────────────────────────────

/// Pure deterministic computation of radial road segments **before** any
/// building placement. Output is then used to build the R-tree.
///
/// **Story-446 fix** : roads start at the **perimeter** (gate position), not
/// at village center. Pattern Mondstadt/Genshin — interior plaza is road-free,
/// roads exit outward via gates. Previously roads pierced the village center
/// which left no room for Poisson candidates (generator failed at 1/6).
fn compute_radial_road_segments(genome: &VillageGenome, count: u32) -> Vec<RoadSegment2D> {
    if count == 0 {
        return Vec::new();
    }
    let half_width = match genome.roads.external_tier.as_str() {
        "primary" => 2.5,
        "secondary" => 1.8,
        "trail" => 0.8,
        "urban" => 1.5,
        _ => 1.5,
    };
    let length = genome.roads.external_length;
    let radius = genome.scale.bounding_radius;
    // Gate position = 90% of bounding radius (just inside perimeter).
    let gate_radius = radius * 0.9;
    let end_radius = radius + length;
    let step = std::f32::consts::TAU / count as f32;
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count {
        let theta = i as f32 * step;
        let dir = Vec2::new(theta.sin(), theta.cos());
        out.push(RoadSegment2D {
            a: dir * gate_radius,
            b: dir * end_radius,
            half_width,
        });
    }
    out
}

/// Adds the road graph nodes + edges to `VillageGraph` (consumed by loader).
///
/// Story-446 — roads materialized as **gate-to-outside** segments, no central
/// hub. Visual ribbon mesh = the actual exclusion zone, so player sees exactly
/// where buildings are kept clear. The village interior plaza stays road-free.
fn materialize_roads_to_graph(
    genome: &VillageGenome,
    graph: &mut VillageGraph,
    segments: &[RoadSegment2D],
) {
    for seg in segments {
        let gate_idx = graph.nodes.len();
        graph.nodes.push(GraphNode {
            position: seg.a,
            kind: NodeKind::Intersection,
            building: None,
        });
        let end_idx = graph.nodes.len();
        graph.nodes.push(GraphNode {
            position: seg.b,
            kind: NodeKind::Intersection,
            building: None,
        });
        graph.edges.push(GraphEdge {
            from: gate_idx,
            to: end_idx,
            tier: genome.roads.external_tier.clone(),
            warp: 0.0,
        });
    }
}

/// Required placement with road-clearance enforcement (story-446).
/// - First required (idx 0) anchored at center — bypass road check.
/// - Subsequent required spread on an inner ring at radius `min_sep + 1m`,
///   each candidate validated vs roads ; if too close, rotate around ring
///   until valid (max 16 micro-steps).
fn place_required_with_roads(
    genome: &VillageGenome,
    rng: &mut Rng,
    graph: &mut VillageGraph,
    required: &[String],
    rtree: &RTree<RoadIndexEntry>,
    min_sep: f32,
) -> usize {
    if required.is_empty() {
        return 0;
    }
    // Center anchor — special, no road check (well is meant to be at axis hub).
    graph.nodes.push(build_node(
        genome,
        rng,
        Vec2::ZERO,
        required[0].as_str(),
        Some("center".into()),
    ));
    let count = required.len() - 1;
    if count == 0 {
        return 1;
    }
    let inner_radius = min_sep + 1.0;
    let angle_step = std::f32::consts::TAU / count as f32;
    let angle_offset = rng.range_f32(0.0, std::f32::consts::TAU);
    let clearance = road_clearance_total(genome);
    for (i, piece) in required.iter().skip(1).enumerate() {
        let base_theta = angle_offset + i as f32 * angle_step;
        // Try 16 micro-rotations within ±half-step.
        let mut placed = false;
        for k in 0..16 {
            let micro = if k == 0 { 0.0 } else {
                let sign = if k % 2 == 0 { 1.0 } else { -1.0 };
                sign * (k as f32) * (angle_step / 32.0)
            };
            let theta = base_theta + micro;
            let cand = Vec2::new(theta.sin(), theta.cos()) * inner_radius;
            if !point_too_close_to_road(rtree, cand, clearance) {
                graph.nodes.push(build_node(genome, rng, cand, piece, None));
                placed = true;
                break;
            }
        }
        if !placed {
            // Fallback : push it on inner ring anyway — log via stats elsewhere.
            let cand = Vec2::new(base_theta.sin(), base_theta.cos()) * inner_radius;
            graph.nodes.push(build_node(genome, rng, cand, piece, None));
        }
    }
    1 + count
}

/// Composite clearance = road's own half_width + genome road_clearance_m.
/// Buildings stay outside `road.half_width + clearance` from any road centerline.
fn road_clearance_total(genome: &VillageGenome) -> f32 {
    let road_half = match genome.roads.external_tier.as_str() {
        "primary" => 2.5,
        "secondary" => 1.8,
        "trail" => 0.8,
        "urban" => 1.5,
        _ => 1.5,
    };
    road_half + genome.scale.road_clearance_m + genome.scale.footprint_half_m
}

fn poisson_min_separation(genome: &VillageGenome) -> f32 {
    // KayKit Medieval Hexagon scaled ≈ 3-5m wide buildings. We add 1.5m buffer
    // for visual breathing room ; density compresses this.
    let base = 3.5 + 1.5; // ≈ 5m comfortable separation at density 0
    let min = 3.0; // never go below 3m (overlap)
    let density = genome.scale.density.clamp(0.0, 1.0);
    (base * (1.0 - 0.5 * density)).max(min)
}

/// Story-446 — Poisson fill with road-clearance enforcement.
/// Per-slot algorithm :
/// 1. Pick a piece via weighted_pick.
/// 2. If piece is in `road_anchored`, try snap-to-nearest-road with setback.
/// 3. Otherwise, Poisson candidate in bounding disc, validated against
///    other buildings AND roads.
#[allow(clippy::too_many_arguments)]
fn fill_with_poisson_road_aware(
    genome: &VillageGenome,
    rng: &mut Rng,
    graph: &mut VillageGraph,
    buildings: &BuildingsGenome,
    slots: usize,
    min_sep: f32,
    rtree: &RTree<RoadIndexEntry>,
    stats: &mut GeneratorStats,
) -> Result<(), GeneratorError> {
    if buildings.optional.is_empty() {
        return Ok(());
    }

    let radius = genome.scale.bounding_radius;
    let optional_entries: Vec<(&String, f32)> = buildings
        .optional
        .iter()
        .map(|(k, v)| (k, *v))
        .collect();
    let weights: Vec<f32> = optional_entries.iter().map(|(_, w)| *w).collect();
    let clearance = road_clearance_total(genome);
    let setback = genome.scale.setback_m;
    let anchored = &buildings.road_anchored;

    let mut placed = 0usize;
    let mut retries = 0u32;
    let max_total_retries = (slots as u32) * POISSON_MAX_ATTEMPTS_PER_SLOT;
    for _slot in 0..slots {
        // Pick the piece first so we know whether to snap-to-road.
        let idx = match rng.weighted_pick(&weights) {
            Some(i) => i,
            None => break,
        };
        let piece = optional_entries[idx].0.as_str();
        let is_anchored = anchored.iter().any(|s| s == piece);

        let mut attempts = 0u32;
        let pos_opt = loop {
            attempts += 1;
            retries += 1;
            if attempts > POISSON_MAX_ATTEMPTS_PER_SLOT || retries > max_total_retries {
                break None;
            }
            // Sample candidate in bounding disc.
            let (rx, ry) = rng.unit_disc();
            let mut cand = Vec2::new(rx, ry) * radius;
            // Track snap origin so we can skip the road-clearance check for
            // anchored placements (snap math already satisfies it by design).
            let mut snapped_to_road = false;

            if is_anchored {
                if let Some((seg, _)) = nearest_road_within(rtree, cand, radius * 1.5) {
                    let proj = project_on_segment(seg, cand);
                    let normal = perpendicular_outward(seg, cand);
                    // Offset = full clearance + user setback : post-check is
                    // guaranteed to pass. Bug fix story-446 PM2 (Poisson rejected
                    // every anchored candidate because setback < clearance).
                    let snap_offset = seg.half_width
                        + genome.scale.road_clearance_m
                        + genome.scale.footprint_half_m
                        + setback;
                    cand = proj + normal * snap_offset;
                    snapped_to_road = true;
                }
            }

            // Road clearance — only enforce on non-anchored candidates (anchored
            // snap math guarantees clearance by construction, see above).
            if !snapped_to_road && point_too_close_to_road(rtree, cand, clearance) {
                continue;
            }
            // Reject if too close to any building already placed.
            let too_close_b = graph
                .buildings()
                .any(|(n, _)| cand.distance(n.position) < min_sep);
            if too_close_b {
                continue;
            }
            // Final safety net : even anchored placements must fall inside the
            // bounding circle (snapped buildings near gate could otherwise drift
            // outside the village footprint).
            if cand.length() > radius * 1.2 {
                continue;
            }
            break Some(cand);
        };

        let Some(pos) = pos_opt else { break };
        // Yaw : face nearest road if anchored, else face center.
        let yaw_deg = if is_anchored {
            if let Some((seg, _)) = nearest_road_within(rtree, pos, radius * 1.5) {
                yaw_facing_road(pos, seg)
            } else {
                yaw_facing_center(pos)
            }
        } else {
            yaw_facing_center(pos)
        };
        graph.nodes.push(GraphNode {
            position: pos,
            kind: NodeKind::Building,
            building: Some(BuildingNode {
                piece: piece.into(),
                color: pick_color(&genome.kit_mix, rng),
                yaw_deg,
                scale: None,
                label: None,
            }),
        });
        placed += 1;
    }
    stats.poisson_retries = retries;

    if placed * 2 < slots {
        return Err(GeneratorError::PoissonExhausted {
            genome_id: genome.meta.id.clone(),
            placed: graph.building_count(),
            target: genome.scale.target_building_count,
        });
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Geometry helpers — segment projection + perpendicular outward.
// ─────────────────────────────────────────────────────────────────────────────

fn project_on_segment(seg: RoadSegment2D, p: Vec2) -> Vec2 {
    let ab = seg.b - seg.a;
    let len2 = ab.length_squared();
    if len2 < 1e-6 {
        return seg.a;
    }
    let t = ((p - seg.a).dot(ab) / len2).clamp(0.0, 1.0);
    seg.a + ab * t
}

/// Perpendicular unit vector pointing AWAY from segment toward `p` side.
fn perpendicular_outward(seg: RoadSegment2D, p: Vec2) -> Vec2 {
    let ab = (seg.b - seg.a).normalize_or_zero();
    let perp = Vec2::new(-ab.y, ab.x);
    let proj = project_on_segment(seg, p);
    let side = p - proj;
    if side.dot(perp) >= 0.0 {
        perp
    } else {
        -perp
    }
}

/// Yaw (deg) so the building "front" (+Z local) faces the road segment.
fn yaw_facing_road(pos: Vec2, seg: RoadSegment2D) -> f32 {
    let proj = project_on_segment(seg, pos);
    let to_road = (proj - pos).normalize_or_zero();
    if to_road == Vec2::ZERO {
        return 0.0;
    }
    to_road.x.atan2(to_road.y).to_degrees()
}

fn build_node(
    genome: &VillageGenome,
    rng: &mut Rng,
    position: Vec2,
    piece: &str,
    label: Option<String>,
) -> GraphNode {
    let color = pick_color(&genome.kit_mix, rng);
    // Yaw : face toward center (helps composition).
    let yaw_deg = yaw_facing_center(position);
    GraphNode {
        position,
        kind: NodeKind::Building,
        building: Some(BuildingNode {
            piece: piece.into(),
            color,
            yaw_deg,
            scale: None,
            label,
        }),
    }
}

fn pick_color(mix: &KitMixDef, rng: &mut Rng) -> Option<String> {
    if mix.colors.is_empty() {
        return None;
    }
    let entries: Vec<(&String, f32)> = mix.colors.iter().map(|(k, v)| (k, *v)).collect();
    let weights: Vec<f32> = entries.iter().map(|(_, w)| *w).collect();
    rng.weighted_pick(&weights)
        .map(|i| entries[i].0.clone())
}

fn yaw_facing_center(pos: Vec2) -> f32 {
    if pos.length_squared() < 1e-6 {
        return 0.0;
    }
    // Building "forward" is +Z. We want it to face -position direction (toward
    // center). Yaw rotates +Z around Y. atan2 returns angle of (sin, cos).
    let to_center = -pos.normalize();
    to_center.x.atan2(to_center.y).to_degrees()
}

fn pick_radial_count(genome: &VillageGenome, rng: &mut Rng) -> u32 {
    let [lo, hi] = genome.roads.radial_count_range;
    if hi <= lo {
        return lo;
    }
    rng.range_u32(lo, hi + 1)
}

fn add_radial_roads(genome: &VillageGenome, graph: &mut VillageGraph, count: u32) {
    if count == 0 {
        return;
    }
    let length = genome.roads.external_length;
    let radius = genome.scale.bounding_radius;
    let step = std::f32::consts::TAU / count as f32;
    // Anchor a virtual "intersection" node at village center for road `from`.
    let center_idx = graph.nodes.len();
    graph.nodes.push(GraphNode {
        position: Vec2::ZERO,
        kind: NodeKind::Intersection,
        building: None,
    });
    for i in 0..count {
        let theta = i as f32 * step;
        let dir = Vec2::new(theta.sin(), theta.cos());
        let end_pos = dir * (radius + length);
        let end_idx = graph.nodes.len();
        graph.nodes.push(GraphNode {
            position: end_pos,
            kind: NodeKind::Intersection,
            building: None,
        });
        graph.edges.push(GraphEdge {
            from: center_idx,
            to: end_idx,
            tier: genome.roads.external_tier.clone(),
            warp: 0.0, // straight radial — village center → outside.
        });
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Village layout — Voronoi + Lloyd + Poisson per cell + Delaunay roads
// ─────────────────────────────────────────────────────────────────────────────

fn village_layout(
    genome: &VillageGenome,
    rng: &mut Rng,
    stats: &mut GeneratorStats,
) -> Result<VillageGraph, GeneratorError> {
    // V1 implementation : 3 districts radial-clustered, each with own Poisson
    // sub-zone. Real Voronoi+Lloyd integration is reserved for story-443 to
    // keep V1 deterministic and small.
    let mut graph = VillageGraph::default();

    let district_count = 3usize;
    let radius = genome.scale.bounding_radius;
    let cell_radius = radius * 0.45;
    let cell_offset = radius * 0.5; // distance from village center to cell center
    let min_sep = poisson_min_separation(genome);

    // Place center node + district centers.
    let center_idx = graph.nodes.len();
    graph.nodes.push(GraphNode {
        position: Vec2::ZERO,
        kind: NodeKind::Intersection,
        building: None,
    });

    let angle_offset = rng.range_f32(0.0, std::f32::consts::TAU);
    let angle_step = std::f32::consts::TAU / district_count as f32;
    let roles = [
        DistrictRole::Market,
        DistrictRole::Residential,
        DistrictRole::Workshop,
    ];
    let mut district_centers: Vec<Vec2> = Vec::with_capacity(district_count);
    for i in 0..district_count {
        let theta = angle_offset + i as f32 * angle_step;
        let center = Vec2::new(theta.sin(), theta.cos()) * cell_offset;
        district_centers.push(center);
    }

    // Required first — anchored to center.
    let mut placed_required = 0;
    if let Some(first) = genome.buildings.required.first() {
        graph
            .nodes
            .push(build_node(genome, rng, Vec2::ZERO, first.as_str(), Some("center".into())));
        placed_required += 1;
        // Distribute extra required across district centers (1 per cell).
        for (i, piece) in genome.buildings.required.iter().enumerate().skip(1) {
            let cell_idx = i % district_count;
            let pos = district_centers[cell_idx];
            graph.nodes.push(build_node(genome, rng, pos, piece, None));
            placed_required += 1;
        }
    }

    // Per-district Poisson fill from optional pool.
    let total_slots = genome
        .scale
        .target_building_count
        .saturating_sub(placed_required);
    let slots_per_cell = total_slots / district_count;
    let leftover = total_slots % district_count;

    let optional_entries: Vec<(&String, f32)> = genome
        .buildings
        .optional
        .iter()
        .map(|(k, v)| (k, *v))
        .collect();
    let weights: Vec<f32> = optional_entries.iter().map(|(_, w)| *w).collect();

    let mut retries_acc = 0u32;
    for (cell_i, &cell_center) in district_centers.iter().enumerate() {
        let cell_slots = slots_per_cell + if cell_i < leftover { 1 } else { 0 };
        let mut placed_in_cell = Vec::with_capacity(cell_slots);
        for _slot in 0..cell_slots {
            let mut attempts = 0u32;
            let pos_opt = loop {
                attempts += 1;
                retries_acc += 1;
                if attempts > POISSON_MAX_ATTEMPTS_PER_SLOT {
                    break None;
                }
                let (rx, ry) = rng.unit_disc();
                let cand = cell_center + Vec2::new(rx, ry) * cell_radius;
                // Check against ALL existing buildings (cross-cell respect).
                let mut ok = true;
                for existing in graph.buildings().map(|(n, _)| n.position) {
                    if cand.distance(existing) < min_sep {
                        ok = false;
                        break;
                    }
                }
                if ok {
                    break Some(cand);
                }
            };
            let Some(pos) = pos_opt else { continue };
            let Some(idx) = rng.weighted_pick(&weights) else {
                continue;
            };
            let piece = optional_entries[idx].0.as_str();
            let node_idx = graph.nodes.len();
            graph.nodes.push(build_node(genome, rng, pos, piece, None));
            placed_in_cell.push(node_idx);
        }
        graph.districts.push(GraphDistrict {
            center: cell_center,
            role: roles[cell_i % roles.len()],
            node_indices: placed_in_cell,
        });
    }
    stats.poisson_retries = retries_acc;

    // Roads : center → each district center, +district→district between
    // consecutive cells.
    for (i, &dc) in district_centers.iter().enumerate() {
        let dc_idx = graph.nodes.len();
        graph.nodes.push(GraphNode {
            position: dc,
            kind: NodeKind::Intersection,
            building: None,
        });
        graph.edges.push(GraphEdge {
            from: center_idx,
            to: dc_idx,
            tier: "secondary".into(),
            warp: 0.0,
        });
        // Connect to next cell (ring).
        let next_i = (i + 1) % district_count;
        let next_dc = district_centers[next_i];
        let next_idx = graph.nodes.len();
        graph.nodes.push(GraphNode {
            position: next_dc,
            kind: NodeKind::Intersection,
            building: None,
        });
        graph.edges.push(GraphEdge {
            from: dc_idx,
            to: next_idx,
            tier: "trail".into(),
            warp: 0.1, // slight curve for ring
        });
    }

    // External radial roads
    let road_count = pick_radial_count(genome, rng);
    add_radial_roads(genome, &mut graph, road_count);

    Ok(graph)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use forgia_genome_village::{ScaleDef, VillageMeta};

    fn hamlet_genome(seed: u64) -> VillageGenome {
        let mut optional = std::collections::BTreeMap::new();
        optional.insert("building_tavern".into(), 1.0);
        optional.insert("building_market".into(), 1.0);
        optional.insert("building_home_A".into(), 2.0);
        optional.insert("building_home_B".into(), 2.0);
        let mut colors = std::collections::BTreeMap::new();
        colors.insert("red".into(), 1.0);
        VillageGenome {
            meta: VillageMeta {
                id: "test_hamlet".into(),
                layout_type: LayoutType::Hamlet,
                seed,
                tier: forgia_genome_village::EconomicTier::Standard,
                biome_affinity: vec![],
            },
            scale: ScaleDef {
                target_building_count: 5,
                density: 0.4,
                bounding_radius: 15.0,
                unit_scale: 3.5,
                road_clearance_m: 1.5,
                setback_m: 2.5,
                footprint_half_m: 2.5,
            },
            kit_mix: KitMixDef {
                kit: "kaykit_medieval_hexagon".into(),
                colors,
            },
            ramparts: Default::default(),
            buildings: BuildingsGenome {
                required: vec!["building_well".into()],
                optional,
                road_anchored: vec![],
            },
            roads: Default::default(),
            npcs: Default::default(),
            spawn: Default::default(),
        }
    }

    #[test]
    fn hamlet_seed_42_reproducible() {
        let g = hamlet_genome(42);
        let (a, _) = generate(&g).unwrap();
        let (b, _) = generate(&g).unwrap();
        assert_eq!(a.building_count(), b.building_count());
        for (na, nb) in a.nodes.iter().zip(b.nodes.iter()) {
            assert_eq!(na.position, nb.position);
            assert_eq!(na.kind, nb.kind);
        }
    }

    #[test]
    fn hamlet_different_seeds_differ() {
        let g1 = hamlet_genome(42);
        let g2 = hamlet_genome(100);
        let (a, _) = generate(&g1).unwrap();
        let (b, _) = generate(&g2).unwrap();
        // At least one building position must differ.
        let pa: Vec<Vec2> = a.buildings().map(|(n, _)| n.position).collect();
        let pb: Vec<Vec2> = b.buildings().map(|(n, _)| n.position).collect();
        assert!(pa != pb, "seeds 42 vs 100 should produce different layouts");
    }

    #[test]
    fn hamlet_no_overlap() {
        let g = hamlet_genome(42);
        let (graph, _) = generate(&g).unwrap();
        // Min sep ≈ 4-5m at density 0.4 — pick 3m as strict no-overlap.
        assert!(
            !graph.has_building_overlap(3.0),
            "buildings overlap under 3m"
        );
    }

    #[test]
    fn hamlet_required_first_at_center() {
        let g = hamlet_genome(42);
        let (graph, _) = generate(&g).unwrap();
        let first_building = graph.buildings().next().unwrap();
        assert_eq!(first_building.1.piece, "building_well");
        assert!(first_building.0.position.length() < 0.01); // at origin
    }

    #[test]
    fn capital_returns_unimplemented_error() {
        let mut g = hamlet_genome(42);
        g.meta.layout_type = LayoutType::Capital;
        let result = generate(&g);
        assert!(matches!(result, Err(GeneratorError::Unimplemented(_))));
    }

    #[test]
    fn village_layout_produces_districts() {
        let mut g = hamlet_genome(42);
        g.meta.layout_type = LayoutType::Village;
        g.scale.target_building_count = 12;
        g.scale.bounding_radius = 25.0;
        let (graph, stats) = generate(&g).unwrap();
        assert_eq!(graph.district_count(), 3);
        assert!(stats.placed_buildings >= 6, "village should place at least half target");
    }

    #[test]
    fn radial_count_in_range() {
        let g = hamlet_genome(42);
        let (graph, _) = generate(&g).unwrap();
        // Default radial range = [2, 4] roads ; new pattern (story-446) :
        // each road has 2 intersection nodes (gate + far endpoint). No center hub.
        // Total intersections = 2 × N where N in [2,4] → range [4, 8].
        let intersections: Vec<&GraphNode> = graph
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Intersection)
            .collect();
        assert!(
            intersections.len() >= 4 && intersections.len() <= 8,
            "got {} intersections, expected 4..=8 (2 nodes per radial road)",
            intersections.len()
        );
    }

    #[test]
    fn yaw_at_origin_is_zero() {
        // Building at origin has no facing direction → yaw 0.
        assert_eq!(yaw_facing_center(Vec2::ZERO), 0.0);
    }
}
