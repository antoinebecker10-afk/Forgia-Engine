//! Debug visualization — draw each spawned module's world AABB as a wireframe box.
//!
//! The worldgen equivalent of Unreal PCG's per-node debug (story-578 §8). Toggled with F8.

use crate::layout::RoadKind;
use crate::spawn::WorldgenModule;
use crate::CityLayout;
use bevy::prelude::*;

/// Whether worldgen debug gizmos are drawn. Toggle with F8.
#[derive(Resource, Default)]
pub struct WorldgenDebugViz(pub bool);

/// F8 toggles the debug viz.
pub fn sys_toggle_viz(keys: Res<ButtonInput<KeyCode>>, mut viz: ResMut<WorldgenDebugViz>) {
    if keys.just_pressed(KeyCode::F8) {
        viz.0 = !viz.0;
        info!("[worldgen] debug viz: {}", if viz.0 { "ON" } else { "OFF" });
    }
}

/// Draw the world-space AABB of every spawned module.
pub fn sys_worldgen_gizmos(
    viz: Res<WorldgenDebugViz>,
    mut gizmos: Gizmos,
    q: Query<(&GlobalTransform, &WorldgenModule)>,
) {
    if !viz.0 {
        return;
    }
    for (gt, m) in &q {
        let world = gt.compute_transform();
        let box_tf = Transform {
            translation: gt.transform_point(m.local_center),
            rotation: world.rotation,
            scale: world.scale * (m.local_half * 2.0),
        };
        gizmos.cube(box_tf, Color::srgb(0.15, 1.0, 0.4));
    }
}

/// Draw the city layout (P3): roads (gold = major, grey = minor) + parcel outlines (green).
/// Same F8 toggle as the AABB gizmos.
pub fn sys_layout_gizmos(viz: Res<WorldgenDebugViz>, layout: Res<CityLayout>, mut gizmos: Gizmos) {
    if !viz.0 || !layout.present {
        return;
    }
    let to_world =
        |p: Vec2| layout.origin + layout.axis_x * p.x + layout.axis_z * p.y + Vec3::Y * 0.15;

    for s in &layout.roads.segments {
        let color = match s.kind {
            RoadKind::Major => Color::srgb(0.96, 0.78, 0.20),
            RoadKind::Minor => Color::srgb(0.55, 0.55, 0.62),
        };
        gizmos.line(to_world(s.a), to_world(s.b), color);
    }

    let green = Color::srgb(0.20, 0.92, 0.42);
    for parcel in &layout.parcels {
        let loop_pts: Vec<Vec3> = parcel
            .polygon
            .iter()
            .map(|p| to_world(*p))
            .chain(parcel.polygon.first().map(|p| to_world(*p)))
            .collect();
        gizmos.linestrip(loop_pts, green);
        let c = to_world(parcel.center);
        gizmos.line(c, c + Vec3::Y * 1.2, green);
    }
}
