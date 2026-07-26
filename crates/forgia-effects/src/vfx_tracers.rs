//! forgia-vfx-tracers — Bullet tracers with **cached mesh** (Forgia gunfeel V5).
//!
//! V1 lesson : building tracer mesh per shot caused -55% freeze. Solution :
//! pre-build a single oriented quad mesh + emissive material at Startup, then
//! reuse for every tracer (spawn cheap, mark lifetime, despawn).
//!
//! Spawn via `TracerSpawn` message — emitted by the active forgia-fps firing
//! pipeline.

use bevy::prelude::*;

#[derive(Message, Debug, Clone, Copy)]
pub struct TracerSpawn {
    pub origin: Vec3,
    pub end: Vec3,
    /// Lifetime in seconds. Tracer despawns + fades over this duration.
    pub lifetime: f32,
}

#[derive(Component)]
pub struct Tracer {
    pub ttl: f32,
    pub initial_ttl: f32,
}

/// Cached shared mesh + material for all tracers.
#[derive(Resource)]
pub struct TracerCache {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
}

pub struct ForgiaVfxTracersPlugin;

impl Plugin for ForgiaVfxTracersPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<TracerSpawn>()
            .add_systems(Startup, build_cache)
            .add_systems(Update, (spawn_tracers, tick_tracers));
    }
}

fn build_cache(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // 1m-long cuboid 0.03 wide on X/Z, Y is forward (will be rotated).
    let mesh = meshes.add(Cuboid::new(0.025, 0.025, 1.0));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.85, 0.4),
        emissive: LinearRgba::new(8.0, 5.0, 1.5, 1.0),
        unlit: true,
        ..default()
    });
    commands.insert_resource(TracerCache { mesh, material });
}

fn spawn_tracers(
    mut events: MessageReader<TracerSpawn>,
    mut commands: Commands,
    cache: Option<Res<TracerCache>>,
) {
    let Some(cache) = cache else { return };
    for ev in events.read() {
        let mid = (ev.origin + ev.end) * 0.5;
        let dir = ev.end - ev.origin;
        let len = dir.length().max(0.01);
        let dir_n = dir / len;

        // Mesh is z-aligned (length 1 along Z). Rotate to face dir, scale Z by len.
        let rot = Quat::from_rotation_arc(Vec3::Z, dir_n);
        let mut xf = Transform::from_translation(mid);
        xf.rotation = rot;
        xf.scale.z = len;

        commands.spawn((
            Tracer {
                ttl: ev.lifetime,
                initial_ttl: ev.lifetime,
            },
            Mesh3d(cache.mesh.clone()),
            MeshMaterial3d(cache.material.clone()),
            xf,
            GlobalTransform::default(),
        ));
    }
}

fn tick_tracers(time: Res<Time>, mut q: Query<(Entity, &mut Tracer)>, mut commands: Commands) {
    let dt = time.delta_secs();
    for (e, mut t) in &mut q {
        t.ttl -= dt;
        if t.ttl <= 0.0 {
            commands.entity(e).despawn();
        }
    }
}
