# Story-447 — Village V2 W2 : Terrain Leveling & Debug Gizmos (Niveau A)

**Status** : DONE (2026-05-18 matin)
**Created** : 2026-05-18
**Scale** : BMAD Standard (7 fichiers, 1 NEW)
**Blocks** : story-448 (segmented roads), story-449 (foundations/plinths)
**Builds on** : story-442 (procgen V1), story-446 (road clearance / red-only)

## Contexte

Le village procgen V2 (story-446 livrée 2026-05-17) spawne 6 buildings KayKit + 4 routes radiales. Les bâtiments sont posés sur le heightmap brut → **inclinés, semi-enfouis, ou flottants** selon la pente. Aucun debug visuel : impossible de diagnostiquer overlaps/placements sans relancer le binaire et tourner la cam.

## Objectif (Niveau A — Quick Win)

1. **Terrain leveling local** : flatten le heightmap dans un disc autour du village center AVANT spawn buildings, avec falloff smoothstep en bordure.
2. **Buildings Y cohérent** : Y des bâtiments = Y leveled (depuis FlattenZones), pas heightmap brut.
3. **Debug gizmos** : circles (bounding_radius, clearance), lines (road axes, setbacks), AABB (footprints) — visibles en debug.
4. **Sensor `forgia_village_debug.json`** : footprints + leveling zone + delta Y avant/après pour diagnostic AI.

## Acceptance Criteria

- [ ] Buildings ne flottent/s'enfoncent plus visuellement (Y stable plateau leveled)
- [ ] Mesh terrain visible sous village = plat (chunk mesh + collider heightfield modifiés)
- [ ] Section `[terrain_leveling]` dans `starter_hamlet.toml` (enabled / radius_m / falloff_m)
- [ ] Sensor `forgia_village_debug.json` écrit 1Hz pendant le mode RPG
- [ ] Gizmos visibles via cam debug (gizmos activés en RPG mode, conditionnel `dbg_village_gizmos`)
- [ ] Aucune regression : `cargo check --workspace` + `cargo clippy --workspace -- -D warnings` clean
- [ ] Foliage exclusion disc inchangée (les arbres restent excluded par le bounding_radius existant)
- [ ] 0 Path not found dans `forgia2_run_retest3.log` après runtime test

## Hors scope (Niveau B+)

- Foliage Y query via FlattenZones (arbres bordure restent sur heightmap raw — différentiel négligeable car déjà excluded par disc 31m)
- Roads segmented graph nodes (toujours ribbon)
- Foundations plinths visuelles sous buildings
- NPCs intra-village, props decoratifs

## Plan d'attaque (7 fichiers)

| # | Fichier | Type | Changement |
|---|---|---|---|
| 1 | `crates/forgia-terrain/src/flatten.rs` | NEW | `FlattenZones` Resource + `VillageFlattenZone { center, target_y, inner_radius, falloff_radius }` + `sample(x, z, raw_y) -> f32` avec smoothstep |
| 2 | `crates/forgia-terrain/src/lib.rs` | EDIT | `pub mod flatten;` + re-export `FlattenZones, VillageFlattenZone` |
| 3 | `crates/forgia-terrain/src/meshing_heightmap.rs` | EDIT | `build_chunk_mesh` accepte `Option<&FlattenZones>` ; appliqué pass 1 (positions Y) + heights (collider heightfield) |
| 4 | `crates/forgia-genome-village/src/lib.rs` | EDIT | `TerrainLevelingDef { enabled: bool, radius_m: f32, falloff_m: f32 }` avec serde default |
| 5 | `config/genomes/villages/starter_hamlet.toml` | EDIT | Section `[terrain_leveling] enabled=true, radius_m=18.0, falloff_m=10.0` |
| 6 | `crates/forgia-rpg/src/lib.rs` | EDIT | Avant `build_chunk_mesh` : load genome TOML, calc target_y = `heightmap_at(village_world_center)`, insert FlattenZones Resource, passer à build_chunk_mesh |
| 7 | `crates/forgia-village-loader/src/lib.rs` | EDIT | Buildings Y = `flatten_zones.sample(...)` ; NEW system `village_debug_gizmos` + write `forgia_village_debug.json` |

## Risques & mitigations

- **Heightmap mutation** : `heightmap_at` reste pure ; flatten = post-process via `FlattenZones::sample(raw_y)`. Aucune mutation in-place. Resource scoped au mode RPG, cleanup OnExit via `RpgWorldMarker`.
- **Foliage Y mismatch** : trees bordure entre `radius_m` et `bounding_radius` (31m disc exclusion) restent sur heightmap raw. Différentiel = falloff smoothstep, max ~quelques cm en bord. Acceptable V1, documenté Niveau B.
- **Chunk(0,0) build order** : `setup_rpg_world` insère `FlattenZones` **avant** `build_chunk_mesh`. Single chunk pour V2 minimal, pas de streaming.

## Architecture

```
forgia-rpg::setup_rpg_world
   │
   ├─→ load VillageGenome (TOML)
   ├─→ compute target_y = heightmap_at(village_center, raw)
   ├─→ insert FlattenZones [{center, target_y, radius_m, falloff_m}]
   ├─→ build_chunk_mesh(coord, ..., Option<&FlattenZones>)  // pass 1 applique sample
   └─→ insert LoadVillageGenomeRequest

forgia-village-loader::process_village_genome_request
   │
   ├─→ generate village procgen
   ├─→ for each building : Y = flatten_zones.sample(world_xz, raw_y)
   └─→ spawn building entities

forgia-village-loader::village_debug_gizmos (Update RPG)
   │
   ├─→ gizmos.circle(village_center, bounding_radius, color: GREEN)
   ├─→ gizmos.circle(village_center, flatten_radius, color: YELLOW)
   ├─→ for each building : gizmos.aabb(footprint, color: BLUE)
   ├─→ for each road : gizmos.line(start, end, color: WHITE)
   └─→ for each road_anchored building : gizmos.line(building, nearest road, color: RED)
```

## Sensor schema `forgia_village_debug.json`

```json
{
  "timestamp_secs": 12.3,
  "village_id": "starter_hamlet",
  "leveling_enabled": true,
  "leveling_target_y": 18.2,
  "leveling_inner_radius": 18.0,
  "leveling_falloff_radius": 10.0,
  "buildings": [
    {"id": "building_well", "world_xz": [16.0, 16.0], "y": 18.2, "footprint_half_m": 2.5}
  ],
  "roads": [
    {"start": [16.0, 16.0], "end": [76.0, 16.0], "tier": "urban"}
  ]
}
```

## Source de vérité patterns AAA

- **Skyrim (Burgess GDC 2013)** : local flattening disc autour POI
- **Cities Skylines** : per-segment elevation curve, terrain blend distance
- **Anno 1800** : foundation/plinth (différé Niveau B)

## Test plan

1. `cargo check -p forgia-terrain -p forgia-genome-village -p forgia-rpg -p forgia-village-loader`
2. `cargo clippy --workspace -- -D warnings`
3. `cargo test -p forgia-terrain` (no regression heightmap pure)
4. Lance binaire, mode RPG, screenshot village
5. Vérif `forgia_village_debug.json` content + 0 Path not found dans log
