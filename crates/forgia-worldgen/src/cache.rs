//! Bake / cache — serialize a resolved city and reload it instantly (story-578 P6).
//!
//! Generating a city runs roads + parcels + grammar; baking writes the **resolved points** to
//! RON so the same map can be reloaded without regenerating (Unreal/Houdini bake parity). The
//! points are stored as a flat DTO (`[f32;…]`) to avoid a glam-serde dependency.

use crate::points::Point;
use bevy::math::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
struct BakedPoint {
    id: String,
    x: f32,
    y: f32,
    z: f32,
    yaw: f32,
    scale: f32,
}

#[derive(Serialize, Deserialize, Default)]
struct BakedCity {
    points: Vec<BakedPoint>,
}

fn to_baked(points: &[Point]) -> BakedCity {
    BakedCity {
        points: points
            .iter()
            .map(|p| BakedPoint {
                id: p.module_id.clone(),
                x: p.ground_pos.x,
                y: p.ground_pos.y,
                z: p.ground_pos.z,
                yaw: p.yaw,
                scale: p.scale,
            })
            .collect(),
    }
}

fn to_points(baked: BakedCity) -> Vec<Point> {
    baked
        .points
        .into_iter()
        .map(|p| Point {
            module_id: p.id,
            ground_pos: Vec3::new(p.x, p.y, p.z),
            yaw: p.yaw,
            scale: p.scale,
        })
        .collect()
}

/// Bake the resolved points to `path` (RON). Returns the number of points written.
pub fn bake(points: &[Point], path: &str) -> std::io::Result<usize> {
    let s = ron::to_string(&to_baked(points)).map_err(std::io::Error::other)?;
    std::fs::write(path, s)?;
    Ok(points.len())
}

/// Load a baked city from `path`. `None` if the file is missing or invalid.
pub fn load(path: &str) -> Option<Vec<Point>> {
    let s = std::fs::read_to_string(path).ok()?;
    let baked: BakedCity = ron::from_str(&s).ok()?;
    Some(to_points(baked))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bake_roundtrips_through_ron() {
        let points = vec![
            Point {
                module_id: "Cube.001".to_string(),
                ground_pos: Vec3::new(1.0, 2.5, -3.0),
                yaw: 0.75,
                scale: 0.6,
            },
            Point {
                module_id: "Cube.017".to_string(),
                ground_pos: Vec3::new(10.0, 0.0, 4.0),
                yaw: 0.0,
                scale: 1.0,
            },
        ];
        let s = ron::to_string(&to_baked(&points)).expect("serialize");
        let back: BakedCity = ron::from_str(&s).expect("deserialize");
        let restored = to_points(back);
        assert_eq!(restored.len(), points.len());
        for (a, b) in restored.iter().zip(&points) {
            assert_eq!(a.module_id, b.module_id);
            assert!(a.ground_pos.distance(b.ground_pos) < 1e-5);
            assert!((a.yaw - b.yaw).abs() < 1e-6);
            assert!((a.scale - b.scale).abs() < 1e-6);
        }
    }
}
