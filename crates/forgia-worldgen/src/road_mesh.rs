//! Road meshing — turn a [`RoadNetwork`] into a flat ground ribbon mesh (story-578 P6+).
//!
//! P3 only drew roads as debug gizmos (invisible to players). This builds a real `Mesh`: one
//! flat quad per road segment, laid on the ground, major roads wider than minor. The mesh is
//! built **relative to `origin`** (so the spawning entity sits at the city center → correct LOD).
//! Pure-ish (takes the injected `GroundSampler` for height). Authoring API.

use crate::layout::{RoadKind, RoadNetwork};
use crate::points::GroundSampler;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

/// Minor road width (meters); major roads are wider.
pub const ROAD_WIDTH_M: f32 = 4.0;
/// Major-over-minor width ratio.
const MAJOR_RATIO: f32 = 1.7;
/// Small lift to avoid z-fighting with the ground.
const ROAD_Y_EPS: f32 = 0.06;
/// Subdivision step (meters): long segments are split so the ribbon follows the terrain.
const ROAD_STEP_M: f32 = 3.0;

/// Build a single mesh for the whole road network, in coordinates **relative to `origin`**
/// (spawn the resulting entity at `Transform::from_translation(origin)`). `axis_x` / `axis_z` are
/// the horizontal layout axes in world space; `minor_width` the minor road width.
pub fn build_road_mesh(
    roads: &RoadNetwork,
    origin: Vec3,
    axis_x: Vec3,
    axis_z: Vec3,
    ground: &GroundSampler,
    minor_width: f32,
) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    // Layout point -> position relative to `origin`, sampling the ground for height.
    let local = |p: Vec2| {
        let world_x = origin.x + axis_x.x * p.x + axis_z.x * p.y;
        let world_z = origin.z + axis_x.z * p.x + axis_z.z * p.y;
        let h = ground.height(world_x, world_z) + ROAD_Y_EPS;
        Vec3::new(
            axis_x.x * p.x + axis_z.x * p.y,
            h,
            axis_x.z * p.x + axis_z.z * p.y,
        )
    };

    for seg in &roads.segments {
        let width = if seg.kind == RoadKind::Major {
            minor_width * MAJOR_RATIO
        } else {
            minor_width
        };
        let len = seg.a.distance(seg.b);
        if len < 1e-3 {
            continue;
        }
        // Perpendicular in layout space (so each edge vertex samples its own terrain height).
        let dir2 = (seg.b - seg.a) / len;
        let perp2 = Vec2::new(-dir2.y, dir2.x) * (width * 0.5);
        // Subdivide so the ribbon follows the terrain along its length.
        let steps = (len / ROAD_STEP_M).ceil().max(1.0) as usize;

        let (mut prev_l, mut prev_r) = (Vec3::ZERO, Vec3::ZERO);
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let p = seg.a.lerp(seg.b, t);
            let l = local(p - perp2);
            let r = local(p + perp2);
            if i > 0 {
                let base = positions.len() as u32;
                for v in [prev_l, prev_r, r, l] {
                    positions.push([v.x, v.y, v.z]);
                    normals.push([0.0, 1.0, 0.0]);
                }
                uvs.extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
                indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            }
            prev_l = l;
            prev_r = r;
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// A dark, double-sided road material (so winding never hides the ribbon).
pub fn road_material() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgb(0.13, 0.12, 0.14),
        perceptual_roughness: 0.95,
        cull_mode: None,
        ..default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::RoadSegment;

    #[test]
    fn segment_is_subdivided_into_quads() {
        let roads = RoadNetwork {
            segments: vec![RoadSegment {
                a: Vec2::new(0.0, 0.0),
                b: Vec2::new(12.0, 0.0), // 12 m / 3 m step → 4 quads → 16 verts
                kind: RoadKind::Major,
            }],
        };
        let mesh = build_road_mesh(
            &roads,
            Vec3::ZERO,
            Vec3::X,
            Vec3::Z,
            &GroundSampler::default(),
            ROAD_WIDTH_M,
        );
        assert_eq!(mesh.count_vertices(), 16, "12 m / 3 m = 4 quads × 4 verts");
        assert_eq!(mesh.indices().map(bevy::mesh::Indices::len), Some(24));
        // Each ribbon quad is 4 verts → vertex count is a multiple of 4.
        assert_eq!(mesh.count_vertices() % 4, 0);
    }

    #[test]
    fn empty_network_makes_empty_mesh() {
        let mesh = build_road_mesh(
            &RoadNetwork::default(),
            Vec3::ZERO,
            Vec3::X,
            Vec3::Z,
            &GroundSampler::default(),
            ROAD_WIDTH_M,
        );
        assert_eq!(mesh.count_vertices(), 0);
    }

    #[test]
    fn degenerate_segment_skipped() {
        let roads = RoadNetwork {
            segments: vec![RoadSegment {
                a: Vec2::new(5.0, 5.0),
                b: Vec2::new(5.0, 5.0), // zero length
                kind: RoadKind::Minor,
            }],
        };
        let mesh = build_road_mesh(
            &roads,
            Vec3::ZERO,
            Vec3::X,
            Vec3::Z,
            &GroundSampler::default(),
            ROAD_WIDTH_M,
        );
        assert_eq!(
            mesh.count_vertices(),
            0,
            "zero-length segment produces no quad"
        );
    }
}
