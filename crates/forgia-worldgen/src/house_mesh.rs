//! House meshing — generate a simple low-poly house (story-578 toolbox).
//!
//! The found building GLBs were AI hero models (50–190 MB each) — unusable for an instanced
//! village. Instead we **generate** lightweight cartoon houses: a colored box body + a hip
//! (pyramid) roof, with per-vertex colors so one shared white material renders every variant.
//! Pure mesh generation, no ECS. Authoring API.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

/// Build a low-poly house mesh centered at XZ origin, base at y = 0. `w`/`d` footprint,
/// `body_h` wall height, `roof_h` roof rise, `overhang` roof eave overhang. Walls get
/// `wall_color`, roof gets `roof_color` (per-vertex; use with a white double-sided material).
pub fn build_house_mesh(
    w: f32,
    d: f32,
    body_h: f32,
    roof_h: f32,
    overhang: f32,
    wall_color: [f32; 4],
    roof_color: [f32; 4],
) -> Mesh {
    let mut pos: Vec<[f32; 3]> = Vec::new();
    let mut nrm: Vec<[f32; 3]> = Vec::new();
    let mut col: Vec<[f32; 4]> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();

    let hw = w * 0.5;
    let hd = d * 0.5;

    let mut quad = |a: Vec3, b: Vec3, c: Vec3, e: Vec3, color: [f32; 4]| {
        let n = (b - a).cross(c - a).normalize_or_zero();
        let base = pos.len() as u32;
        for v in [a, b, c, e] {
            pos.push([v.x, v.y, v.z]);
            nrm.push([n.x, n.y, n.z]);
            col.push(color);
        }
        idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    };

    // Four walls (base y=0 → body_h), outward normals.
    let y0 = 0.0;
    let y1 = body_h;
    // +Z
    quad(
        Vec3::new(-hw, y0, hd),
        Vec3::new(hw, y0, hd),
        Vec3::new(hw, y1, hd),
        Vec3::new(-hw, y1, hd),
        wall_color,
    );
    // -Z
    quad(
        Vec3::new(hw, y0, -hd),
        Vec3::new(-hw, y0, -hd),
        Vec3::new(-hw, y1, -hd),
        Vec3::new(hw, y1, -hd),
        wall_color,
    );
    // +X
    quad(
        Vec3::new(hw, y0, hd),
        Vec3::new(hw, y0, -hd),
        Vec3::new(hw, y1, -hd),
        Vec3::new(hw, y1, hd),
        wall_color,
    );
    // -X
    quad(
        Vec3::new(-hw, y0, -hd),
        Vec3::new(-hw, y0, hd),
        Vec3::new(-hw, y1, hd),
        Vec3::new(-hw, y1, -hd),
        wall_color,
    );

    // Hip roof: 4 triangles from the (overhanging) eave corners to an apex.
    let ehw = hw + overhang;
    let ehd = hd + overhang;
    let apex = Vec3::new(0.0, body_h + roof_h, 0.0);
    let corners = [
        Vec3::new(-ehw, body_h, ehd),
        Vec3::new(ehw, body_h, ehd),
        Vec3::new(ehw, body_h, -ehd),
        Vec3::new(-ehw, body_h, -ehd),
    ];
    let mut tri = |a: Vec3, b: Vec3, c: Vec3, color: [f32; 4]| {
        let n = (b - a).cross(c - a).normalize_or_zero();
        let base = pos.len() as u32;
        for v in [a, b, c] {
            pos.push([v.x, v.y, v.z]);
            nrm.push([n.x, n.y, n.z]);
            col.push(color);
        }
        idx.extend_from_slice(&[base, base + 1, base + 2]);
    };
    for i in 0..4 {
        tri(corners[i], corners[(i + 1) % 4], apex, roof_color);
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nrm);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    mesh.insert_indices(Indices::U32(idx));
    mesh
}

/// A white, double-sided material that lets per-vertex house colors show (and never culls a
/// wall by winding).
pub fn house_material() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.9,
        cull_mode: None,
        ..default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn house_has_walls_and_roof() {
        let m = build_house_mesh(6.0, 5.0, 4.0, 2.5, 0.4, [0.8, 0.7, 0.5, 1.0], [0.6, 0.2, 0.15, 1.0]);
        // 4 walls (4 verts each) + 4 roof tris (3 verts each) = 16 + 12 = 28 verts.
        assert_eq!(m.count_vertices(), 28);
        // 4 walls (6 idx) + 4 tris (3 idx) = 24 + 12 = 36 indices.
        assert_eq!(m.indices().map(bevy::mesh::Indices::len), Some(36));
    }
}
