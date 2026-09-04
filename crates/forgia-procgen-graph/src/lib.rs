//! # forgia-procgen-graph
//!
//! Pure data structures describing a procgen village's logical graph :
//! [`VillageGraph`] = nodes (intersections + building anchors) + edges
//! (road segments) + optional districts (Voronoi cells).
//!
//! The [`forgia_village_generator`] crate consumes this graph and emits a
//! `VillageDef` (positions, scales, kit pieces) for `forgia-village-loader`.
//!
//! ## Why a separate crate
//!
//! The graph is independent of any algorithm — hamlet/village/capital can all
//! emit this same shape. Sensors, validators, and future navmesh consumers
//! read it without coupling to the generator's internal RNG or kit choices.
//!
//! ## Pure data — no Bevy ECS
//!
//! Only `bevy::math::Vec2` is used (lightweight, ubiquitous). No Plugin, no
//! Resource, no Component. The caller (Bevy or pure logic) owns lifecycle.

use bevy::math::Vec2;

/// Contrats PCG génériques partagés avec les graphes spécialisés (village,
/// donjon, véhicule). `VillageGraph` reste son IR métier historique ; les
/// nouveaux générateurs peuvent transiter par ces types purs avant adaptation.
pub use forgia_pcg_core::{ContentSpec, GenerationContext, LogicalPlan, SpatialPlan};

// ─────────────────────────────────────────────────────────────────────────────
// RoadSegment2D — capsule 2D for road-vs-building validation (story-446)
// ─────────────────────────────────────────────────────────────────────────────

/// One road as a 2D **capsule** : line segment `[a, b]` with `half_width`.
/// Used by [`VillageGraph`] generators for spatial collision checks against
/// candidate building positions (Anno-style edge-adjacency + CityEngine
/// setback rule).
///
/// `parry2d::shape::Capsule` is the canonical primitive ; we store the raw
/// data here so this crate stays Bevy-only (no rapier dep).
#[derive(Clone, Copy, Debug)]
pub struct RoadSegment2D {
    pub a: Vec2,
    pub b: Vec2,
    pub half_width: f32,
}

impl RoadSegment2D {
    /// Squared distance from a point to this segment (XZ plane). Used by the
    /// generator to test `dist² > (half_width + clearance)²`.
    pub fn distance_squared_to(&self, p: Vec2) -> f32 {
        let ab = self.b - self.a;
        let len2 = ab.length_squared();
        if len2 < 1e-6 {
            return (p - self.a).length_squared();
        }
        let t = ((p - self.a).dot(ab) / len2).clamp(0.0, 1.0);
        let proj = self.a + ab * t;
        (p - proj).length_squared()
    }

    /// Returns the 2D AABB envelope including `half_width` — useful for
    /// inserting into an R-tree (`rstar`) without a full capsule shape.
    pub fn aabb(&self) -> ([f32; 2], [f32; 2]) {
        let min_x = self.a.x.min(self.b.x) - self.half_width;
        let max_x = self.a.x.max(self.b.x) + self.half_width;
        let min_y = self.a.y.min(self.b.y) - self.half_width;
        let max_y = self.a.y.max(self.b.y) + self.half_width;
        ([min_x, min_y], [max_x, max_y])
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// VillageGraph — the procgen output shape
// ─────────────────────────────────────────────────────────────────────────────

/// Full procgen output for a village. Generators populate this ; loaders +
/// sensors consume it.
#[derive(Debug, Clone, Default)]
pub struct VillageGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub districts: Vec<GraphDistrict>,
}

/// One logical point in the village — building anchor, intersection, or POI.
#[derive(Debug, Clone)]
pub struct GraphNode {
    /// XZ position local to village center.
    pub position: Vec2,
    pub kind: NodeKind,
    /// Optional reference to a building piece (filled if `kind == Building`).
    pub building: Option<BuildingNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    /// Anchored building (well, tavern, home, …).
    Building,
    /// Path intersection — no entity spawned here, just road crossing.
    Intersection,
    /// Point of interest reserved for future use (quest marker, NPC spawn).
    Poi,
}

/// Building-specific data for a [`NodeKind::Building`] node.
#[derive(Debug, Clone)]
pub struct BuildingNode {
    /// KayKit piece name (e.g. `building_well`, `building_tavern`).
    pub piece: String,
    /// Color variant for color-coded kits (e.g. `red`, `blue`).
    pub color: Option<String>,
    /// Yaw in degrees (0 = facing +Z).
    pub yaw_deg: f32,
    /// Per-building scale override. `None` → use genome `unit_scale` directly.
    pub scale: Option<f32>,
    /// Friendly label for HUD/interact prompts.
    pub label: Option<String>,
}

/// One road segment between two nodes (by index into [`VillageGraph::nodes`]).
#[derive(Debug, Clone)]
pub struct GraphEdge {
    pub from: usize,
    pub to: usize,
    /// `urban` | `primary` | `secondary` | `trail` — passed through to mesh
    /// builder.
    pub tier: String,
    /// Curvature warp factor passed to `forgia-spline::arc_control_from_chord`.
    /// `0.0` = straight line. `0.25` = mild curve (matches forgia-terrain default).
    pub warp: f32,
}

/// Optional district (Voronoi cell) — used by Village/Capital, not Hamlet.
#[derive(Debug, Clone)]
pub struct GraphDistrict {
    pub center: Vec2,
    pub role: DistrictRole,
    /// Indices of nodes that belong to this district.
    pub node_indices: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistrictRole {
    Market,
    Residential,
    Workshop,
    /// Reserved for future use.
    Sacred,
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

impl VillageGraph {
    /// Iterator over building nodes only — convenience for loaders.
    pub fn buildings(&self) -> impl Iterator<Item = (&GraphNode, &BuildingNode)> {
        self.nodes
            .iter()
            .filter_map(|n| n.building.as_ref().map(|b| (n, b)))
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
    pub fn building_count(&self) -> usize {
        self.buildings().count()
    }
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
    pub fn district_count(&self) -> usize {
        self.districts.len()
    }

    /// Returns true if any two building nodes are within `min_separation` of
    /// each other — useful for post-placement validation. O(N²) — keep N small.
    pub fn has_building_overlap(&self, min_separation: f32) -> bool {
        let buildings: Vec<&GraphNode> = self.buildings().map(|(n, _)| n).collect();
        for i in 0..buildings.len() {
            for j in (i + 1)..buildings.len() {
                let d = buildings[i].position.distance(buildings[j].position);
                if d < min_separation {
                    return true;
                }
            }
        }
        false
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn building(x: f32, z: f32, piece: &str) -> GraphNode {
        GraphNode {
            position: Vec2::new(x, z),
            kind: NodeKind::Building,
            building: Some(BuildingNode {
                piece: piece.into(),
                color: None,
                yaw_deg: 0.0,
                scale: None,
                label: None,
            }),
        }
    }

    #[test]
    fn empty_graph_has_zero_counts() {
        let g = VillageGraph::default();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.building_count(), 0);
        assert_eq!(g.edge_count(), 0);
        assert_eq!(g.district_count(), 0);
    }

    #[test]
    fn building_iter_filters_intersections() {
        let mut g = VillageGraph::default();
        g.nodes.push(building(0.0, 0.0, "building_well"));
        g.nodes.push(GraphNode {
            position: Vec2::new(5.0, 5.0),
            kind: NodeKind::Intersection,
            building: None,
        });
        g.nodes.push(building(10.0, 0.0, "building_tavern"));
        assert_eq!(g.node_count(), 3);
        assert_eq!(g.building_count(), 2);
    }

    #[test]
    fn overlap_detection_works() {
        let mut g = VillageGraph::default();
        g.nodes.push(building(0.0, 0.0, "a"));
        g.nodes.push(building(2.0, 0.0, "b")); // 2m apart
        assert!(g.has_building_overlap(3.0)); // 2 < 3 → overlap
        assert!(!g.has_building_overlap(1.5)); // 2 > 1.5 → no overlap
    }

    #[test]
    fn road_segment_distance_to_perpendicular_point() {
        // Segment along +X, point above midpoint.
        let seg = RoadSegment2D {
            a: Vec2::ZERO,
            b: Vec2::new(10.0, 0.0),
            half_width: 1.5,
        };
        let d2 = seg.distance_squared_to(Vec2::new(5.0, 3.0));
        assert!(
            (d2 - 9.0).abs() < 1e-3,
            "perpendicular distance² should be 9, got {d2}"
        );
    }

    #[test]
    fn road_segment_distance_to_endpoint() {
        let seg = RoadSegment2D {
            a: Vec2::ZERO,
            b: Vec2::new(10.0, 0.0),
            half_width: 1.5,
        };
        // Beyond endpoint b → distance from b.
        let d2 = seg.distance_squared_to(Vec2::new(13.0, 4.0));
        assert!((d2 - 25.0).abs() < 1e-3, "expected 25, got {d2}");
    }

    #[test]
    fn road_segment_aabb_includes_half_width() {
        let seg = RoadSegment2D {
            a: Vec2::new(0.0, 5.0),
            b: Vec2::new(10.0, 5.0),
            half_width: 1.5,
        };
        let (min, max) = seg.aabb();
        assert!((min[0] - (-1.5)).abs() < 1e-3);
        assert!((min[1] - 3.5).abs() < 1e-3);
        assert!((max[0] - 11.5).abs() < 1e-3);
        assert!((max[1] - 6.5).abs() < 1e-3);
    }

    #[test]
    fn overlap_with_intersection_ignores_non_building() {
        let mut g = VillageGraph::default();
        g.nodes.push(building(0.0, 0.0, "a"));
        // An intersection 1m away — should NOT count as overlap.
        g.nodes.push(GraphNode {
            position: Vec2::new(1.0, 0.0),
            kind: NodeKind::Intersection,
            building: None,
        });
        assert!(!g.has_building_overlap(2.0));
    }
}
