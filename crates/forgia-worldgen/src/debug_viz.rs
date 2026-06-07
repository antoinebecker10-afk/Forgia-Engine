//! Debug visualization — draw each spawned module's world AABB as a wireframe box.
//!
//! The worldgen equivalent of Unreal PCG's per-node debug (story-578 §8). Toggled with F8.

use crate::spawn::WorldgenModule;
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
