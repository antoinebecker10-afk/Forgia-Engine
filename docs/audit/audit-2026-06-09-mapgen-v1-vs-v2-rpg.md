# Audit comparatif — Génération de map : Forgia V1 « classique » vs V2 Rewrite (mode RPG)

> **Date** : 2026-06-09 · **Type** : audit read-only comparatif · **Méthode** : 2 explorations terrain-specialist parallèles (V1 sur `d:\Forgia`, V2 sur `Desktop\Forgia Rewrite`).
> **Contexte priorité** : SHIP = Roguelite. Le RPG = track FORGE (outils qui refluent). Les recommandations sont pondérées par « ça sert le ship ? ».

---

## TL;DR — le verdict en 3 phrases

1. **Technique de fond différente** : V1 = **SDF voxel 3D + Surface Nets** (grottes, surplombs, falaises verticales possibles, pipeline 19 couches). V2 RPG = **heightmap-grid 2D** (pattern Skyrim/Witcher, 1 vert/m, `Collider::heightfield`), volontairement plus simple.
2. **Le V2 RPG est une simplification ASSUMÉE** — mais une grande partie de la richesse V1 est **portée dans le code V2 et non branchée** (érosion, redistribution, micro-roughness, SDF, caves existent mais le chemin de rendu RPG utilise `heightmap_at` qui les ignore).
3. **Les vrais trous du V2 RPG ne sont pas "le manque de SDF" mais 4 dettes concrètes** : génération **synchrone** (stutter), `pipeline_diag` **stub** (zéro observabilité terrain = viole la règle), `biome_at` **O(N seeds)** sans index spatial, **LOD2 sans collider** (chute infinie hors ring LOD0).

---

## Comparaison côte à côte

| Axe | V1 « classique » (d:\Forgia) | V2 Rewrite RPG (Desktop) |
|---|---|---|
| **Mesh** | SDF voxel 32×128×32 + Surface Nets (`fast-surface-nets`) | Heightmap-grid 33×33 verts/chunk, `Collider::heightfield` |
| **Surplombs / grottes** | ✅ possibles (volumes creusés dans le SDF) | ❌ impossibles (heightmap pur) |
| **Pipeline gen** | 19 couches (noise→warp→redistribution→**érosion hydro/thermique/vallée**→flatten→SDF→**caves**) | `heightmap_at` = FBm + domain warp + amplitude biome. **Érosion/redistribution/micro-roughness codées mais NON branchées** (vivent dans `heightmap_at_gen_ext`, inutilisé par le mesh RPG) |
| **Biomes** | 10, Voronoi + **multi-noise layers par biome** (TOML), blend 240 m | 10, Voronoi + **amplitude_mult data-driven** (story-576), blend couleur 60 m / forme 200 m |
| **biome lookup** | BiomeMap partagée `Arc`, weights 4-voisins | `biome_at` **O(N seeds) linéaire** (~1500 seeds × 1089 verts/chunk) — pas d'index spatial |
| **Streaming** | ✅ **async** (`AsyncComputeTaskPool`) gen + mesh, 3 tiers détail (Full/Fast/Distant) | ❌ **synchrone** sur le thread principal (le pipeline async est documenté "W2+" mais pas activé) |
| **LOD** | 3 niveaux (plein / ½-rés / mega-tile) | 3 niveaux (plein / visibilité / LOD2 mega-tile 128 m) — **LOD2 sans collider** |
| **Cache** | LRU 128, quantize i16 + zstd | LRU 128, quantize i16 + zstd (identique) |
| **Data-driven** | 11 genomes biome TOML (recettes noise complètes) + MapGenConfig JSON | `terrain_shape.toml` (octaves/warp/max_height) + `biome_*.toml` (amplitude). **MapGenConfig construit inline = PAS hot-reload** |
| **Observabilité** | ✅ `pipeline_diag` **19 couches** + 6 sensors + health checks (`VEGETATION ZERO`, holes) | ❌ `pipeline_diag.rs` = **STUB no-op**. Sensors : LOD coverage + chunks snapshot. **Aucun health check terrain** |
| **Caves** | ✅ actives (Perlin 3D + worms + réseau + village caves) | ⚠️ code présent, **désactivé** ("removed 2026-05-17") |
| **Eau** | plan `bevy_water` à sea_level + skip chunks sous-marins | plan `bevy_water` à sea_level 4.0 (`SeaLevel` Resource) |
| **Rivières/lacs** | features heightmap + drainage érosion | `TerrainFeature::River/Lake` creuse le heightmap, **pas de mesh eau dédié** |
| **Village/routes** | VillageNetwork Poisson + PathNetwork MST/Bézier + château | Village hex KayKit (worldgen) + PathNetwork ribbon |
| **Tests** | benches criterion | ✅ **denses** (chunk 16, flatten 6, biomes 8, sampling 5 proptests…) |

---

## Lecture : ce qui distingue vraiment les deux

**V1 = maximaliste procédural.** Un SDF 3D par chunk (150 k f32) permet des grottes/surplombs réels et une érosion physique (hydraulique Beyer/Lague, thermique talus). Le prix : ~270 MB de SDF actif sur un monde 4 km, un pipeline de 19 couches, et une contrainte structurelle (heightmap→SDF extrudé ⇒ pentes >45° font dériver Surface Nets ⇒ `clamp_extreme_slopes`).

**V2 RPG = pragmatique heightmap.** 1 vertex/mètre, collider heightfield natif Rapier, testé unitairement, LOD extent-aware robuste. C'est le bon choix pour un **overworld jouable** (le pattern AAA RPG). Mais le chemin de rendu (`heightmap_at`) reste **plus pauvre que ce que le code laisse croire** : il n'applique ni l'érosion ni la redistribution ni la micro-roughness qui sont pourtant portées dans `heightmap_at_gen_ext`.

> **Insight clé** : l'écart V1→V2-RPG n'est PAS surtout "il manque le SDF". C'est **(a) du code riche porté mais débranché** (érosion/redistribution/micro-roughness — quasi gratuit à activer) **+ (b) 4 dettes d'ingénierie** (sync gen, diag stub, biome O(N), LOD2 sans collider) **+ (c) un choix assumé** (heightmap vs SDF = pas de grottes).

---

## Trous du V2 RPG, classés par impact

| # | Trou | Gravité | Effort fix | Sert le ship ? |
|---|---|---|---|---|
| 1 | **`pipeline_diag` stub** → zéro observabilité terrain (viole `observability-required.md`) | Majeur | Moyen (porter le diag V1) | ⚠️ indirect (debug) |
| 2 | **LOD2 sans collider** → chute infinie hors du ring LOD0 | Majeur (bug jouable) | Faible | non (RPG) |
| 3 | **Génération synchrone** → stutter au streaming (le pipeline async V1 existe) | Majeur | Élevé (porter l'async) | non (RPG) |
| 4 | **`biome_at` O(N seeds)** sans index spatial → coût mesh ∝ nb seeds | Mineur→Majeur selon densité | Moyen (grid/kd-tree) | non |
| 5 | **`heightmap_at` ignore la gen riche** (érosion/redistribution/micro-roughness débranchées) | Mineur (terrain plus plat qu'il pourrait) | **Faible** (rebrancher le chemin ext) | non |
| 6 | **MapGenConfig pas hot-reload** (construit inline) | Cosmétique | Faible | non |
| 7 | Rivières/lacs sans mesh eau ; `LavaPool` sans visuel | Mineur | Moyen | non |
| 8 | Pas de grottes/surplombs (limite heightmap) | Design | Élevé (SDF) | non |

---

## Recommandations (pondérées SHIP = Roguelite)

**Principe** : le RPG est track FORGE. **Ne PAS réinvestir massivement dans le terrain procédural** (SDF/caves/érosion physique) — ça ne débloque pas le ship Roguelite. Mais 2 trous sont des **bugs/règles** à corriger quel que soit le track :

1. **À faire (cheap, dette réelle)** :
   - **#2 LOD2 collider** — un `Collider::heightfield` sur les mega-tiles (ou un plancher de sécurité) : empêche la chute infinie. Petit, bug jouable.
   - **#1 porter `pipeline_diag`** (au moins un sensor `forgia_terrain.json` minimal : counts, gen_ms, anomalies) — la règle `observability-required` l'exige et c'est aujourd'hui un angle mort total.
   - **#5 rebrancher la gen riche** sur `heightmap_at` (redistribution + micro-roughness + slope clamp) — quasi gratuit (le code est là), rend le relief RPG nettement plus expressif.

2. **À faire SI le RPG/monde devient prioritaire** (sinon différer) :
   - **#3 async gen** (porter `AsyncComputeTaskPool` de V1) — supprime le stutter de streaming.
   - **#4 index spatial biome** (grid 96 m → liste de seeds locale) — ÷~100 sur le coût biome.

3. **À NE PAS faire maintenant** (gros coût, zéro bénéfice ship) :
   - Activer le SDF/Surface Nets + grottes en RPG (#8). C'est la grande force de V1, mais un overworld RPG jouable n'en a pas besoin, et ça réintroduit la dette mémoire + la contrainte de pente. **Garder le heightmap-grid.**

> **Verdict** : le V2 RPG a fait le bon choix d'architecture (heightmap pragmatique). Il ne lui manque pas le SDF — il lui manque **l'observabilité (#1), un collider LOD2 (#2), et le rebranchement de sa propre gen riche déjà codée (#5)**. Ces trois-là, peu coûteux, ferment 80 % de l'écart ressenti avec V1 sans réintroduire sa complexité.

---

## Annexe — fichiers de référence

**V1** (`d:\Forgia\RUST\Forgia\Forgia\`):
- `forgia-terrain/src/generation/chunk_sdf.rs` — orchestrateur pipeline 19 couches
- `forgia-terrain/src/meshing.rs` — Surface Nets + vertex colors + async
- `forgia-terrain/src/pipeline_diag.rs` — 19 couches télémétrie
- `forgia-game/src/terrain/streaming.rs` — async ECS
- `config/genomes/biome_*.toml` — 11 genomes biome

**V2 RPG** (`C:\Users\Antoi\Desktop\Forgia Rewrite\crates\`):
- `forgia-rpg/src/lib.rs` — `spawn_world`, `stream_chunks_around_player` (gen sync)
- `forgia-terrain/src/meshing_heightmap.rs` — `build_chunk_mesh`, `Collider::heightfield`
- `forgia-terrain/src/generation/heightmap.rs` — `heightmap_at` (chemin W1) vs `heightmap_at_gen_ext` (riche, débranché)
- `forgia-terrain/src/biomes.rs` — `BiomeMap`, `biome_at` O(N)
- `forgia-terrain/src/lod.rs` — LOD2 (sans collider)
- `forgia-terrain/src/pipeline_diag.rs` — **STUB**
