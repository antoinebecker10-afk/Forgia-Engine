//! RPG worldgen village — a **KayKit Medieval Hexagon** settlement, laid on a flattened terrain
//! disc using the `forgia-worldgen` **hex layout primitive** (story-578: the toolbox in real use).
//!
//! The KayKit hex tiles tessellate edge-to-edge (clean transitions by construction — they only fit
//! on a flat plane), so we **flatten a disc** of terrain under the village ([`FlattenZones`],
//! registered in `spawn_world` before the chunks mesh) and lay a hex grid on top:
//! - center hex + ring 1 = open **plaza** (player arrives here, the 4 on-brand NPCs spawn in an arc),
//!   with a **well** on one plaza tile,
//! - ring ≥ 2 = **buildings** (homes + civic: tavern, blacksmith, market, church, windmill…) and
//!   **decoration** (trees, rocks, barrels) on the rest,
//! - everything else is grass tiles.
//!
//! Buildings get a deferred `AsyncSceneCollider` so the player can't walk through them. Trees and
//! the village footprint are kept clear of streamed foliage.

use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use bevy::scene::SceneRoot;
use bevy_rapier3d::prelude::{AsyncSceneCollider, ComputedColliderShape, RigidBody};
use forgia_foliage::VegetationTree;
use forgia_terrain::VillageFlattenZone;
use forgia_worldgen::hex::{hex_spiral, Hex};
use forgia_worldgen::seed::{derive, SeededRng};
use std::collections::HashMap;

use crate::RpgVillageAnchor;

const KIT: &str = "models/kaykit/hexagon";

/// Render scale of every KayKit hex piece (tile 2 m → `2·S` m flat-to-flat; home ≈ `0.93·S` m tall).
const HEX_SCALE: f32 = 3.0;
/// Hexagon grid radius in rings (`1 + 3R(R+1)` tiles → R=3 = 37 tiles).
const HEX_RADIUS: i32 = 3;
/// Center-to-vertex of a KayKit tile at scale 1 (= 2/√3). Drives tessellation spacing.
const HEX_SIZE_NATIVE: f32 = 1.154_700_5;
/// Tiny lift so the tile top sits just above the flattened terrain (no z-fight).
const TILE_LIFT: f32 = 0.05;
/// Fraction of ring ≥ 2 tiles that carry a building (rest = decoration / grass).
const BUILD_DENSITY: f32 = 0.62;

/// Flat disc radii (m) — must cover the tiled village **and** the player spawn (world origin, ~22.6 m
/// from the village center at (16,16)) so the player lands on flat ground.
pub(crate) const VILLAGE_FLATTEN_INNER: f32 = 26.0;
pub(crate) const VILLAGE_FLATTEN_FALLOFF: f32 = 16.0;
/// Streamed trees are despawned within this radius of the village center.
const FOLIAGE_CLEAR_RADIUS: f32 = VILLAGE_FLATTEN_INNER + VILLAGE_FLATTEN_FALLOFF * 0.4;

/// Village seed (deterministic layout).
const VILLAGE_SEED: u64 = 1310;

/// Grass tile — the village floor.
const TILE_GRASS: &str = "tiles/base/hex_grass.gltf";
/// Plaza centerpiece.
const WELL: &str = "buildings/blue/building_well_blue.gltf";
/// Homes (mixed colors → cheerful village).
const HOMES: &[&str] = &[
    "buildings/red/building_home_A_red.gltf",
    "buildings/red/building_home_B_red.gltf",
    "buildings/green/building_home_A_green.gltf",
    "buildings/green/building_home_B_green.gltf",
    "buildings/blue/building_home_A_blue.gltf",
    "buildings/blue/building_home_B_blue.gltf",
    "buildings/yellow/building_home_A_yellow.gltf",
    "buildings/yellow/building_home_B_yellow.gltf",
];
/// Civic buildings (rarer accents).
const CIVIC: &[&str] = &[
    "buildings/green/building_tavern_green.gltf",
    "buildings/blue/building_blacksmith_blue.gltf",
    "buildings/red/building_market_red.gltf",
    "buildings/yellow/building_windmill_yellow.gltf",
    "buildings/green/building_church_green.gltf",
    "buildings/red/building_lumbermill_red.gltf",
    "buildings/blue/building_watermill_blue.gltf",
];
/// Trees + rocks (large decoration, gets a collider).
const TREES: &[&str] = &[
    "decoration/nature/tree_single_A.gltf",
    "decoration/nature/tree_single_B.gltf",
    "decoration/nature/trees_A_small.gltf",
    "decoration/nature/trees_B_small.gltf",
];
const ROCKS: &[&str] = &[
    "decoration/nature/rock_single_A.gltf",
    "decoration/nature/rock_single_B.gltf",
    "decoration/nature/rock_single_C.gltf",
];
/// Small props (no collider — dressing).
const PROPS: &[&str] = &[
    "decoration/props/barrel.gltf",
    "decoration/props/crate_A_small.gltf",
    "decoration/props/sack.gltf",
    "decoration/props/wheelbarrow.gltf",
];

/// What a tile carries (decided per hex from its ring + a seeded roll).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TileRole {
    /// Open plaza (center + ring 1): grass only, kept clear for player + NPCs.
    Plaza,
    /// A building stands on this tile.
    Building,
    /// A tree / rock / prop stands on this tile.
    Decoration,
    /// Bare grass.
    Empty,
}

/// Pure role decision (testable). Center + ring 1 are always plaza; ring ≥ 2 rolls building vs
/// decoration vs bare grass.
fn role_for(ring: i32, roll: f32) -> TileRole {
    if ring <= 1 {
        TileRole::Plaza
    } else if roll < BUILD_DENSITY {
        TileRole::Building
    } else if roll < BUILD_DENSITY + 0.22 {
        TileRole::Decoration
    } else {
        TileRole::Empty
    }
}

/// Village runtime state (generated once per RPG session + where it landed).
#[derive(Resource, Default)]
pub(crate) struct RpgVillageState {
    spawned: bool,
    center: Vec2,
}

/// Marker on every spawned village piece (tile / building / decoration) — for OnExit cleanup.
#[derive(Component)]
pub(crate) struct RpgVillagePiece;

/// The flatten disc for the hex village (call from `spawn_world` so the terrain is flat **before**
/// the chunks mesh). `center` = village world XZ, `target_y` = `heightmap_at(center)` raw.
pub(crate) fn village_flatten_zone(center: Vec2, target_y: f32) -> VillageFlattenZone {
    VillageFlattenZone {
        center,
        target_y,
        inner_radius: VILLAGE_FLATTEN_INNER,
        falloff_radius: VILLAGE_FLATTEN_FALLOFF,
    }
}

/// Load (and cache) a GLTF scene handle for a KayKit hex asset (path relative to [`KIT`]).
fn scene(
    asset_server: &AssetServer,
    cache: &mut HashMap<&'static str, Handle<Scene>>,
    rel: &'static str,
) -> Handle<Scene> {
    cache
        .entry(rel)
        .or_insert_with(|| asset_server.load(GltfAssetLabel::Scene(0).from_asset(format!("{KIT}/{rel}"))))
        .clone()
}

/// Generate the hex village once, after the village anchor (center + flat Y) exists.
pub(crate) fn sys_spawn_worldgen_village(
    mut state: ResMut<RpgVillageState>,
    anchor: Option<Res<RpgVillageAnchor>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    if state.spawned {
        return;
    }
    let Some(anchor) = anchor else {
        return; // wait for spawn_world to publish the anchor
    };

    let center = Vec2::new(anchor.center.x, anchor.center.z);
    let base_y = anchor.center.y + TILE_LIFT;
    state.center = center;

    let size = HEX_SIZE_NATIVE * HEX_SCALE;
    let tile_scale = Vec3::splat(HEX_SCALE);
    let mut cache: HashMap<&'static str, Handle<Scene>> = HashMap::new();
    // The well sits on the north plaza tile, not dead-center (player + NPCs occupy the center).
    let well_hex = Hex::new(0, -1);

    let mut buildings = 0u32;
    let mut tiles = 0u32;
    for hex in hex_spiral(HEX_RADIUS) {
        let local = hex.to_world(size);
        let pos = Vec3::new(center.x + local.x, base_y, center.y + local.y);
        let mut rng = SeededRng::new(derive(VILLAGE_SEED, hash_hex(hex)));

        // Ground tile (always), yaw snapped to the hex grid for a tidy tessellation.
        let tile_yaw = (rng.below(6) as f32) * std::f32::consts::FRAC_PI_3;
        commands.spawn((
            RpgVillagePiece,
            Name::new("village:tile"),
            SceneRoot(scene(&asset_server, &mut cache, TILE_GRASS)),
            Transform::from_translation(pos)
                .with_rotation(Quat::from_rotation_y(tile_yaw))
                .with_scale(tile_scale),
        ));
        tiles += 1;

        // Well on its dedicated plaza tile.
        if hex == well_hex {
            spawn_prop(&mut commands, scene(&asset_server, &mut cache, WELL), pos, 0.0, HEX_SCALE, true);
            continue;
        }

        match role_for(hex.ring(), rng.next_f32()) {
            TileRole::Plaza | TileRole::Empty => {}
            TileRole::Building => {
                let path = if rng.next_f32() < 0.82 {
                    HOMES[rng.below(HOMES.len())]
                } else {
                    CIVIC[rng.below(CIVIC.len())]
                };
                let yaw = (rng.below(6) as f32) * std::f32::consts::FRAC_PI_3;
                let s = HEX_SCALE * (0.92 + rng.next_f32() * 0.16);
                spawn_prop(&mut commands, scene(&asset_server, &mut cache, path), pos, yaw, s, true);
                buildings += 1;
            }
            TileRole::Decoration => {
                let r = rng.next_f32();
                let (path, collide, sfac) = if r < 0.5 {
                    (TREES[rng.below(TREES.len())], true, 1.0)
                } else if r < 0.78 {
                    (ROCKS[rng.below(ROCKS.len())], true, 0.9)
                } else {
                    (PROPS[rng.below(PROPS.len())], false, 0.8)
                };
                let yaw = rng.next_f32() * std::f32::consts::TAU;
                let handle = scene(&asset_server, &mut cache, path);
                spawn_prop(&mut commands, handle, pos, yaw, HEX_SCALE * sfac, collide);
            }
        }
    }

    state.spawned = true;
    info!(
        "[rpg] village hex KayKit : {tiles} tuiles, {buildings} bâtiments, centre ({:.0}, {:.0})",
        center.x, center.y
    );
}

/// Spawn one KayKit piece at `pos`, with optional deferred collider (buildings / trees / rocks).
fn spawn_prop(
    commands: &mut Commands,
    handle: Handle<Scene>,
    pos: Vec3,
    yaw: f32,
    scale: f32,
    collide: bool,
) {
    let mut e = commands.spawn((
        RpgVillagePiece,
        Name::new("village:piece"),
        SceneRoot(handle),
        Transform::from_translation(pos)
            .with_rotation(Quat::from_rotation_y(yaw))
            .with_scale(Vec3::splat(scale)),
    ));
    if collide {
        e.insert((
            RigidBody::Fixed,
            AsyncSceneCollider {
                shape: Some(ComputedColliderShape::TriMesh(default())),
                ..default()
            },
        ));
    }
}

/// Stable per-hex hash for seeding (q,r packed).
fn hash_hex(hex: Hex) -> u64 {
    let q = i64::from(hex.q) as u64;
    let r = i64::from(hex.r) as u64;
    (q << 32) ^ (r & 0xffff_ffff)
}

/// Keep the village footprint clear of streamed trees (foliage streams continuously, so this runs
/// each frame and despawns any tree that lands inside the village disc).
pub(crate) fn sys_clear_village_foliage(
    state: Res<RpgVillageState>,
    q_trees: Query<(Entity, &GlobalTransform), With<VegetationTree>>,
    mut commands: Commands,
) {
    if !state.spawned {
        return;
    }
    let r2 = FOLIAGE_CLEAR_RADIUS * FOLIAGE_CLEAR_RADIUS;
    for (e, gt) in &q_trees {
        let p = gt.translation();
        let (dx, dz) = (p.x - state.center.x, p.z - state.center.y);
        if dx * dx + dz * dz < r2 {
            commands.entity(e).despawn();
        }
    }
}

/// On RPG exit: despawn the worldgen village, reset for next entry.
pub(crate) fn sys_cleanup_worldgen_village(
    mut state: ResMut<RpgVillageState>,
    q_pieces: Query<Entity, With<RpgVillagePiece>>,
    mut commands: Commands,
) {
    for e in &q_pieces {
        commands.entity(e).despawn();
    }
    state.spawned = false;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plaza_is_center_and_ring_one() {
        assert_eq!(role_for(0, 0.0), TileRole::Plaza);
        assert_eq!(role_for(1, 0.99), TileRole::Plaza);
    }

    #[test]
    fn outer_rings_roll_building_then_deco_then_empty() {
        assert_eq!(role_for(2, 0.0), TileRole::Building);
        assert_eq!(role_for(2, BUILD_DENSITY - 0.01), TileRole::Building);
        assert_eq!(role_for(2, BUILD_DENSITY + 0.01), TileRole::Decoration);
        assert_eq!(role_for(3, 0.99), TileRole::Empty);
    }

    #[test]
    fn flatten_zone_covers_player_spawn() {
        // Player spawns at world origin; village center at (16,16) → 22.6 m away, inside inner disc.
        let z = village_flatten_zone(Vec2::new(16.0, 16.0), 5.0);
        let player = Vec2::new(0.0, 0.0);
        assert!(player.distance(z.center) < z.inner_radius, "player spawn must be on flat ground");
        assert_eq!(z.target_y, 5.0);
    }

    #[test]
    fn hash_hex_is_distinct_for_neighbours() {
        let a = hash_hex(Hex::new(1, 0));
        let b = hash_hex(Hex::new(0, 1));
        let c = hash_hex(Hex::new(-1, 0));
        assert_ne!(a, b);
        assert_ne!(b, c);
        assert_ne!(a, c);
    }
}
