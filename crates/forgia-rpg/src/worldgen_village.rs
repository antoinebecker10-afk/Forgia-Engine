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
use forgia_foliage::{FoliageExclusionDisc, VegetationTree};
use forgia_terrain::VillageFlattenZone;
use forgia_worldgen::hex::{hex_spiral, Hex};
use forgia_worldgen::seed::{derive, SeededRng};
use std::collections::HashMap;

use crate::RpgVillageAnchor;

const KIT: &str = "models/kaykit/hexagon";

/// Render scale of every KayKit hex piece (tile 2 m → `2·S` m flat-to-flat). Sized so buildings tower
/// over the ~2 m characters (Rex = reference) — a home ≈ `0.93·S·BUILDING_SCALE_MUL` ≈ 5.4 m.
const HEX_SCALE: f32 = 4.5;
/// Buildings (homes / civic / well / towers) are rendered a bit larger than the tile scale so they
/// read as real buildings next to the characters, without overflowing their tile.
const BUILDING_SCALE_MUL: f32 = 1.2;
/// Hexagon grid radius in rings for the inhabited area (`1 + 3R(R+1)` tiles → R=4 = 61 tiles). A
/// bigger town so buildings are spread out, not cramped.
const HEX_RADIUS: i32 = 4;
/// Fortification ring (one ring beyond the buildings): walls + corner/gate towers + 3 gates.
const WALL_RING: i32 = HEX_RADIUS + 1;
/// Center-to-vertex of a KayKit tile at scale 1 (= 2/√3). Drives tessellation spacing.
const HEX_SIZE_NATIVE: f32 = 1.154_700_5;
/// Tiny lift so the tile top sits just above the flattened terrain (no z-fight).
const TILE_LIFT: f32 = 0.05;
/// Fraction of ring ≥ 2 tiles that carry a building (rest = decoration / grass). Lower = less
/// cramped + more decoration trees inside the town.
const BUILD_DENSITY: f32 = 0.42;

/// Outer extent (m) of a given hex ring: furthest hex center (`ring · √3 · size`) + one tile
/// half-width (`size`).
const fn ring_extent(ring: i32) -> f32 {
    let size = HEX_SIZE_NATIVE * HEX_SCALE;
    ring as f32 * 1.732_050_8 * size + size
}

/// Flat disc radii (m) — cover the **walled town** (out to [`WALL_RING`]) and the player spawn
/// (world origin, ~22.6 m from the village center) so the player lands on flat ground.
pub(crate) const VILLAGE_FLATTEN_INNER: f32 = ring_extent(WALL_RING) + 3.0;
/// Short falloff so the natural (forested) terrain resumes close to the walls.
pub(crate) const VILLAGE_FLATTEN_FALLOFF: f32 = 10.0;
/// Streamed trees are despawned only inside the walls, so the forest grows right up to the
/// fortifications. Audit 2026-06-08: an over-wide clear left a bare Forest moat around the village.
const FOLIAGE_CLEAR_RADIUS: f32 = ring_extent(WALL_RING);

/// Village seed (deterministic layout).
const VILLAGE_SEED: u64 = 1310;

/// Grass tile — the village floor.
const TILE_GRASS: &str = "tiles/base/hex_grass.gltf";

/// KayKit road tiles by connection signature: the hex edge-slots their road touches at the tile's
/// default rotation (decoded from the meshes). Slot `s` = the edge at angle `s·60°` (s=0 → +X).
/// Every connection mask of degree 1..6 is reachable by one of these rotated (see `road_tile_for`).
const ROAD_TILES: &[(&str, u8)] = &[
    ("tiles/roads/hex_road_A.gltf", 0b00_1001), // straight  {0,3}
    ("tiles/roads/hex_road_B.gltf", 0b10_1000), // curve 120 {3,5}
    ("tiles/roads/hex_road_C.gltf", 0b01_1000), // curve 60  {3,4}
    ("tiles/roads/hex_road_D.gltf", 0b10_1010), // 3-way Y   {1,3,5}
    ("tiles/roads/hex_road_E.gltf", 0b10_1001), // 3-way     {0,3,5}
    ("tiles/roads/hex_road_F.gltf", 0b00_1011), // 3-way     {0,1,3}
    ("tiles/roads/hex_road_G.gltf", 0b01_1100), // 3-way T   {2,3,4}
    ("tiles/roads/hex_road_H.gltf", 0b01_1101), // 4-way     {0,2,3,4}
    ("tiles/roads/hex_road_I.gltf", 0b11_0110), // 4-way     {1,2,4,5}
    ("tiles/roads/hex_road_J.gltf", 0b00_1111), // 4-way     {0,1,2,3}
    ("tiles/roads/hex_road_K.gltf", 0b11_1110), // 5-way     {1,2,3,4,5}
    ("tiles/roads/hex_road_L.gltf", 0b11_1111), // 6-way     {0..5}
    ("tiles/roads/hex_road_M.gltf", 0b00_1000), // dead-end  {3}
];
/// Yaw per +1 slot of rotation (degrees), shared by roads + walls (same decode convention).
/// Negative: a +θ yaw moves an edge from angle α to α−θ (glam `from_rotation_y`). Flip the sign if
/// curves / corners / dead-ends face the wrong way in-game.
const TILE_YAW_STEP_DEG: f32 = -60.0;

/// Town wall segments, autotiled exactly like roads (a wall spanning a hex edge-pair). Decoded from
/// the meshes: straight {0,3}, corner-A 120° {3,5}, corner-B 60° {3,4}.
const WALL_TILES: &[(&str, u8)] = &[
    ("buildings/neutral/wall_straight.gltf", 0b00_1001),
    ("buildings/neutral/wall_corner_A_outside.gltf", 0b10_1000),
    ("buildings/neutral/wall_corner_B_outside.gltf", 0b01_1000),
];
/// Front-face direction of the straight wall mesh at yaw=0, in the slot-angle
/// convention (degrees; slot 0 = +X). A straight wall spans {0,3} (the +X/−X axis)
/// so its faces are ±Z → front at 90°. EMPIRICAL: if every straight wall ends up
/// facing inward in-game, flip the sign to −90.0.
const WALL_FRONT_BASE_DEG: f32 = 90.0;

/// Defensive tower (town corners + gate posts).
const TOWER: &str = "buildings/red/building_tower_base_red.gltf";
/// Radial outward shift (m) of corner/gate towers so they sit flush on the wall line
/// (slight bastion projection) instead of recessed at the hex centre — "déplace les
/// tours pour qu'elles soient bien intégrées aux remparts". EMPIRICAL: increase to
/// project further out, lower toward 0 to centre on the hex.
const TOWER_OUTWARD_OFFSET_M: f32 = 2.5;

/// Plaza centerpiece.
const WELL: &str = "buildings/blue/building_well_blue.gltf";

/// Fixed civic buildings placed at known hexes so an NPC can stand in front of "their" building.
/// `(q, r, building, npc-name)`. The well + its NPC are handled separately.
const CIVIC_LANDMARKS: &[(i32, i32, &str, &str)] = &[
    (2, -1, "buildings/blue/building_blacksmith_blue.gltf", "MaitreForgeron"),
    (-1, 2, "buildings/red/building_market_red.gltf", "Mira"),
    (1, 1, "buildings/green/building_tavern_green.gltf", "Dorin"),
];
/// Well plaza hex + the NPC stationed there.
const WELL_HEX: (i32, i32) = (0, 1);
const WELL_NPC: &str = "Apprenti";
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

/// Spoke directions of the road network: 3 streets radiating from the center, 120° apart (dir 0, 2,
/// 4). Keeps the center + ring-1 plaza mostly open (only the 3 spoke tiles there are road).
const ROAD_SPOKES: [(i32, i32); 3] = [(1, 0), (0, -1), (-1, 1)];

/// The six canonical hex directions (index = [`Hex::neighbors`] order).
const HEX_DIRS: [(i32, i32); 6] = [(1, 0), (1, -1), (0, -1), (-1, 0), (-1, 1), (0, 1)];

/// Road network = center + the 3 radial spokes extended to the wall ring (so each road exits
/// through a gate). The 3 spoke tips on the wall ring are the gates.
fn is_road_hex(hex: Hex) -> bool {
    if hex == Hex::ZERO {
        return true;
    }
    ROAD_SPOKES
        .iter()
        .any(|&(dq, dr)| (1..=WALL_RING).any(|n| hex == Hex::new(dq * n, dr * n)))
}

/// A gate = a spoke tip on the wall ring (the road passes through; no wall there).
fn is_gate(hex: Hex) -> bool {
    hex.ring() == WALL_RING
        && ROAD_SPOKES
            .iter()
            .any(|&(dq, dr)| hex == Hex::new(dq * WALL_RING, dr * WALL_RING))
}

/// A tower = a non-gate corner of the wall ring, or a wall hex flanking a gate (gate post).
fn is_tower(hex: Hex) -> bool {
    if hex.ring() != WALL_RING || is_gate(hex) {
        return false;
    }
    let is_corner = HEX_DIRS
        .iter()
        .any(|&(dq, dr)| hex == Hex::new(dq * WALL_RING, dr * WALL_RING));
    is_corner || hex.neighbors().iter().any(|&nb| is_gate(nb))
}

/// Wall network = everything on the wall ring except the gates (walls + towers). Walls join to it.
fn is_wall_network(hex: Hex) -> bool {
    hex.ring() == WALL_RING && !is_gate(hex)
}

/// The wall tile + yaw for a non-gate ring hex, for a **continuous** enclosure (only the gates are
/// open). Gate-post hexes (a single wall neighbour) are straightened so the wall reaches the gate.
fn wall_at(hex: Hex) -> Option<(&'static str, f32)> {
    let mut m = wall_mask(hex);
    if m.count_ones() == 1 {
        let slot = m.trailing_zeros();
        m |= 1u8 << ((slot + 3) % 6);
    }
    wall_tile_for(m)
}

/// Orient a wall's "outside" face radially outward from the town centre. The straight
/// wall tile is 180°-symmetric in its connection mask, so [`wall_tile_for`] cannot
/// choose which face is outside — it would point inward on half the ring. We flip the
/// yaw 180° (which keeps the same {0,3} connection) when the front would face inward.
/// Corners ({3,5}/{3,4}) have a unique rotation → left untouched. `outward` = the hex's
/// XZ offset from the town centre. Pure → testable headless.
fn orient_wall_outward(path: &str, yaw: f32, outward: Vec2) -> f32 {
    if path != WALL_TILES[0].0 || outward.length_squared() < 1e-6 {
        return yaw;
    }
    // After a yaw, a feature at base slot-angle α appears at α − yaw (glam from_rotation_y,
    // cf TILE_YAW_STEP_DEG note). Slot angle β maps to the XZ direction (cos β, sin β).
    let front_ang = WALL_FRONT_BASE_DEG.to_radians() - yaw;
    let front = Vec2::new(front_ang.cos(), front_ang.sin());
    if front.dot(outward.normalize_or_zero()) < 0.0 {
        yaw + std::f32::consts::PI
    } else {
        yaw
    }
}

/// Connection mask of a hex within a network: bit `slot` set when the neighbour across that edge is
/// a member. Slot for neighbour direction `i` is `(6 - i) % 6` (the edge geometry).
fn network_mask(hex: Hex, member: impl Fn(Hex) -> bool) -> u8 {
    let mut m = 0u8;
    for (i, nb) in hex.neighbors().into_iter().enumerate() {
        if member(nb) {
            m |= 1 << ((6 - i as u32) % 6);
        }
    }
    m
}

fn road_mask(hex: Hex) -> u8 {
    network_mask(hex, is_road_hex)
}
fn wall_mask(hex: Hex) -> u8 {
    network_mask(hex, is_wall_network)
}

/// Rotate a 6-bit slot mask left by `k` (circularly within 6 bits).
fn rot6(mask: u8, k: u32) -> u8 {
    let m = u32::from(mask) & 0x3f;
    (((m << k) | (m >> (6 - k))) & 0x3f) as u8
}

/// Pick the tile from `tiles` (+ yaw) whose rotated connection signature matches `mask`. `None` only
/// for an isolated hex (mask 0).
fn autotile(tiles: &[(&'static str, u8)], mask: u8) -> Option<(&'static str, f32)> {
    if mask == 0 {
        return None;
    }
    for &(path, base) in tiles {
        for k in 0..6u32 {
            if rot6(base, k) == mask {
                return Some((path, (k as f32 * TILE_YAW_STEP_DEG).to_radians()));
            }
        }
    }
    None
}
fn road_tile_for(mask: u8) -> Option<(&'static str, f32)> {
    autotile(ROAD_TILES, mask)
}
fn wall_tile_for(mask: u8) -> Option<(&'static str, f32)> {
    autotile(WALL_TILES, mask)
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

/// Yaw so a piece at local offset `local` (from the center) faces the plaza center (Bevy fwd = -Z).
fn face_center_yaw(local: Vec2) -> f32 {
    local.x.atan2(local.y)
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
    let building_scale = HEX_SCALE * BUILDING_SCALE_MUL;
    let mut cache: HashMap<&'static str, Handle<Scene>> = HashMap::new();
    let well_hex = Hex::new(WELL_HEX.0, WELL_HEX.1);

    // Keep the player's spawn hex (+ neighbours) clear of buildings — the player spawns at world
    // origin and the bigger town now reaches there.
    let player_hex = Hex::from_world(-center, size);
    let spawn_clear = |h: Hex| h == player_hex || player_hex.neighbors().contains(&h);

    let mut buildings = 0u32;
    let mut tiles = 0u32;
    for hex in hex_spiral(WALL_RING) {
        let local = hex.to_world(size);
        let pos = Vec3::new(center.x + local.x, base_y, center.y + local.y);
        let mut rng = SeededRng::new(derive(VILLAGE_SEED, hash_hex(hex)));
        let ring = hex.ring();

        // Ground tile: road tile on the road network (autotiled), else grass.
        let road_tile = is_road_hex(hex)
            .then(|| road_tile_for(road_mask(hex)))
            .flatten();
        let road = road_tile.is_some();
        let (tile_path, tile_yaw) = match road_tile {
            Some((p, yaw)) => (p, yaw),
            None => (TILE_GRASS, (rng.below(6) as f32) * std::f32::consts::FRAC_PI_3),
        };
        commands.spawn((
            RpgVillagePiece,
            Name::new("village:tile"),
            SceneRoot(scene(&asset_server, &mut cache, tile_path)),
            Transform::from_translation(pos)
                .with_rotation(Quat::from_rotation_y(tile_yaw))
                .with_scale(tile_scale),
        ));
        tiles += 1;

        // Roads carry nothing else (the gate tiles too).
        if road {
            continue;
        }

        // Fortification ring: towers (corners + gate posts) and autotiled wall segments.
        if ring == WALL_RING {
            // Continuous wall on every non-gate ring hex (gates were roads, already skipped) so the
            // town is fully enclosed; a tower sits on top at the corners + gate posts.
            if let Some((wpath, wyaw)) = wall_at(hex) {
                // Force the wall's "outside" face radially outward (the straight tile is
                // 180°-symmetric → autotiler can't choose → "sens" flipped on half the ring).
                let wyaw = orient_wall_outward(wpath, wyaw, local);
                let h = scene(&asset_server, &mut cache, wpath);
                spawn_prop(&mut commands, h, pos, wyaw, HEX_SCALE, true);
            }
            if is_tower(hex) {
                // Push the tower radially outward so it sits flush on / projects slightly
                // past the wall line (bastion) instead of recessed at the hex centre.
                let outward = local.normalize_or_zero();
                let tower_pos = pos + Vec3::new(outward.x, 0.0, outward.y) * TOWER_OUTWARD_OFFSET_M;
                let h = scene(&asset_server, &mut cache, TOWER);
                spawn_prop(&mut commands, h, tower_pos, 0.0, building_scale, true);
            }
            continue;
        }

        // Well on its plaza tile.
        if hex == well_hex {
            let h = scene(&asset_server, &mut cache, WELL);
            spawn_prop(&mut commands, h, pos, 0.0, building_scale, true);
            continue;
        }
        // Fixed civic buildings (for NPC stations), facing the plaza center.
        if let Some(&(_, _, path, _)) =
            CIVIC_LANDMARKS.iter().find(|&&(q, r, _, _)| hex == Hex::new(q, r))
        {
            let h = scene(&asset_server, &mut cache, path);
            spawn_prop(&mut commands, h, pos, face_center_yaw(local), building_scale, true);
            buildings += 1;
            continue;
        }

        match role_for(ring, rng.next_f32()) {
            TileRole::Building if !spawn_clear(hex) => {
                let path = if rng.next_f32() < 0.82 {
                    HOMES[rng.below(HOMES.len())]
                } else {
                    CIVIC[rng.below(CIVIC.len())]
                };
                let yaw = (rng.below(6) as f32) * std::f32::consts::FRAC_PI_3;
                let s = building_scale * (0.92 + rng.next_f32() * 0.16);
                spawn_prop(&mut commands, scene(&asset_server, &mut cache, path), pos, yaw, s, true);
                buildings += 1;
            }
            TileRole::Decoration if !spawn_clear(hex) => {
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
            _ => {}
        }
    }

    state.spawned = true;
    // R7 (story-586) : exclusion foliage PERSISTANTE à la source. Le système B legacy
    // (village-loader) qui posait `FoliageExclusionDisc` est débranché du RPG → c'est
    // désormais le village hex qui le pose. forgia-foliage skippe le spawn d'arbres
    // dans ce disque (plus de flicker/churn vs le clear réactif `sys_clear_village_foliage`
    // seul — ce dernier reste un filet de sécurité pour la race streaming/foliage).
    commands.insert_resource(FoliageExclusionDisc {
        center,
        radius: FOLIAGE_CLEAR_RADIUS,
    });
    info!(
        "[rpg] village hex KayKit fortifié : {tiles} tuiles, {buildings} bâtiments, centre ({:.0}, {:.0})",
        center.x, center.y
    );
}

/// World positions where the on-brand NPCs stand (in front of "their" building, facing it). Read by
/// `character::spawn_character_lineup`. Returns `(npc-name, xz, yaw)`; `yaw` faces the building.
pub(crate) fn npc_stations(anchor: &RpgVillageAnchor) -> Vec<(&'static str, Vec2, f32)> {
    let center = Vec2::new(anchor.center.x, anchor.center.z);
    let size = HEX_SIZE_NATIVE * HEX_SCALE;
    let mut out = Vec::new();
    let mut push = |q: i32, r: i32, npc: &'static str| {
        let local = Hex::new(q, r).to_world(size);
        let bpos = center + local;
        // Stand between the building and the plaza center, facing the building (away from center).
        let to_center = (center - bpos).normalize_or_zero();
        let station = bpos + to_center * (size * 0.85);
        let yaw = face_center_yaw(local) + std::f32::consts::PI; // face the building, not the plaza
        out.push((npc, station, yaw));
    };
    for &(q, r, _, npc) in CIVIC_LANDMARKS {
        push(q, r, npc);
    }
    push(WELL_HEX.0, WELL_HEX.1, WELL_NPC);
    out
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
    // story-588 FIX : `Transform` (local) et NON `GlobalTransform`. Les arbres sont
    // spawnés dans le même Update que ce système ; leur `GlobalTransform` n'est propagé
    // qu'en PostUpdate → il vaut l'identité (0,0,0) la frame du spawn. Lu via
    // GlobalTransform, CHAQUE arbre paraissait à 22.6m du centre village (= dist
    // origine→centre) donc DANS le disque de 50m → tous despawnés avant d'être rendus
    // (régression invisible). Les arbres étant des entités racine, `Transform.translation`
    // = position monde, correcte immédiatement (cohérent avec `spawn_village_paths`).
    q_trees: Query<(Entity, &Transform), With<VegetationTree>>,
    mut commands: Commands,
) {
    if !state.spawned {
        return;
    }
    let r2 = FOLIAGE_CLEAR_RADIUS * FOLIAGE_CLEAR_RADIUS;
    for (e, tf) in &q_trees {
        let p = tf.translation;
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
    fn straight_wall_faces_outward() {
        let straight = WALL_TILES[0].0;
        // Front at yaw=0 = +Z (WALL_FRONT_BASE_DEG=90). Outward +Z → keep ; outward −Z → flip 180°.
        assert_eq!(orient_wall_outward(straight, 0.0, Vec2::new(0.0, 1.0)), 0.0);
        assert!(
            (orient_wall_outward(straight, 0.0, Vec2::new(0.0, -1.0)) - std::f32::consts::PI).abs()
                < 1e-4
        );
        // Corners ({3,5}/{3,4}) have a unique rotation → left untouched.
        let corner = WALL_TILES[1].0;
        assert_eq!(orient_wall_outward(corner, 1.234, Vec2::new(0.0, -1.0)), 1.234);
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
    fn road_network_is_center_plus_three_spokes() {
        assert!(is_road_hex(Hex::ZERO)); // center crossroads
        assert!(is_road_hex(Hex::new(3, 0))); // spoke dir0 tip
        assert!(is_road_hex(Hex::new(0, -2))); // spoke dir2
        assert!(is_road_hex(Hex::new(-2, 2))); // spoke dir4
        assert!(!is_road_hex(Hex::new(-2, 0))); // opposite side: not a spoke
        assert!(!is_road_hex(Hex::new(0, 1))); // plaza (well)
        assert!(!is_road_hex(Hex::new(2, 1))); // building / deco
    }

    #[test]
    fn rot6_is_circular() {
        assert_eq!(rot6(0b001001, 0), 0b001001);
        assert_eq!(rot6(0b001000, 1), 0b010000); // dead-end slot3 → slot4
        assert_eq!(rot6(0b100000, 1), 0b000001); // wrap slot5 → slot0
        assert_eq!(rot6(0b111111, 4), 0b111111); // 6-way is rotation-invariant
    }

    #[test]
    fn autotiler_matches_signatures() {
        assert_eq!(road_tile_for(0b001001).unwrap().0, "tiles/roads/hex_road_A.gltf"); // straight
        assert_eq!(road_tile_for(0b111111).unwrap().0, "tiles/roads/hex_road_L.gltf"); // 6-way
        assert_eq!(road_tile_for(0b001000).unwrap().0, "tiles/roads/hex_road_M.gltf"); // dead-end
        assert!(road_tile_for(0).is_none());
        // 3-spoke center = neighbours dir 0,2,4 → slots {0,2,4} → D (3-way Y).
        assert_eq!(road_mask(Hex::ZERO), 0b010101);
        assert_eq!(road_tile_for(road_mask(Hex::ZERO)).unwrap().0, "tiles/roads/hex_road_D.gltf");
    }

    #[test]
    fn autotiler_covers_every_mask() {
        // Completeness: every non-zero 6-bit connection mask maps to some tile.
        for mask in 1u8..0b100_0000 {
            assert!(road_tile_for(mask).is_some(), "no tile for mask {mask:#08b}");
        }
    }

    #[test]
    fn fortification_ring_classification() {
        // Gates = the 3 spoke tips on the wall ring (WALL_RING = 5); the road exits through them.
        assert!(is_gate(Hex::new(5, 0)));
        assert!(is_gate(Hex::new(0, -5)));
        assert!(is_road_hex(Hex::new(5, 0)));
        // Non-gate corners = towers.
        assert!(is_tower(Hex::new(5, -5)));
        assert!(is_tower(Hex::new(-5, 0)));
        // Gate posts (the gate's wall-ring neighbours) = towers.
        let posts = Hex::new(5, 0).neighbors().iter().filter(|h| is_tower(**h)).count();
        assert!(posts >= 1, "a gate must be flanked by towers");
        // A non-gate ring hex resolves to a wall; inner hexes are not fortifications.
        assert!(wall_at(Hex::new(5, -2)).is_some());
        assert!(!is_tower(Hex::ZERO) && !is_gate(Hex::ZERO));
    }

    #[test]
    fn enclosure_is_continuous_except_gates() {
        // Every non-gate hex on the wall ring resolves to a wall tile → the town is fully enclosed.
        for hex in forgia_worldgen::hex::hex_ring(WALL_RING) {
            if !is_gate(hex) {
                assert!(wall_at(hex).is_some(), "gap in the wall at {hex:?}");
            }
        }
    }

    #[test]
    fn npc_stations_cover_all_lineup_spots() {
        let anchor = RpgVillageAnchor { center: Vec3::new(16.0, 5.0, 16.0) };
        let stations = npc_stations(&anchor);
        assert_eq!(stations.len(), CIVIC_LANDMARKS.len() + 1); // civic buildings + the well
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
