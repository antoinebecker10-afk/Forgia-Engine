# Story-578 — `forgia-worldgen` : moteur de génération procédurale (villes / villages / maps)

> **Statut** : EN COURS — **P0 + P1 + P2 DONE** (2026-06-07), P3→P6 PLANNED
> **Niveau BMAD** : Enterprise (nouveau crate, multi-module, cross-cutting, ≥10 fichiers)
> **Origine** : besoin user « générer des villes, villages, maps ». Audit web 2026-06-07
> (Unreal PCG, Houdini/PDG, tensor-field roads, ECS data-oriented, WFC) → archi enrichie.

---

## ✅ P0 — Registre d'assets (LIVRÉ 2026-06-07)

**Crate** `forgia-worldgen` créé (pure-data, 0 dép Bevy/forgia — `serde` + `ron` only,
crate-feuille immunisée au churn multi-terminal). Ajouté aux workspace members (1 ligne).

**Livrables :**
- `crates/forgia-worldgen/src/registry.rs` — types `AssetMeta` / `AssetRole` (Platform/
  Pillar/Wall/Ramp/Prop/Decal/Backdrop/Loot/Hazard/Unknown) / `ColliderKind` (Cuboid/
  Cylinder/ConvexHull/TriMesh/NoCollider) + loader RON (`AssetRegistry::from_ron`) +
  helper de grounding `placement_y(ground_y, scale)` (le fix anti-enfoncé/flottant).
- `assets/registry/asset_meta.ron` — **107 modules** ithappy catalogués semi-auto depuis
  `One_file_assets.glb` (parseur GLB pur-Python). Géométrie `aabb_min/max` + `ground_offset`
  **EXACTE** (accessor min/max). Rôle/collider = passe géométrique (18 Platform, 14 Pillar,
  2 Wall, 3 Backdrop, 18 Decal, 3 **Unknown** = file de revue P1, 49 Prop).
- `assets/models/environment/platformer/one_file_assets.glb` — copie source (spawn P1).
- **9 tests** (preuve P0 : `every_module_grounds_correctly` = 107 modules × 4 scales ×
  4 hauteurs → bottom == ground, jamais enfoncé/flottant ; `solid_roles_have_colliders` =
  invariant anti-walkable-sans-collider). `cargo test` vert, clippy 0 warning.

**Audit qa-lead (BUG REPORT) — suivis P1 :**
- 🟠 QA-03 (Majeur, traité non-spéculativement) : `Cube.035`/`Cube.042` (modules longs 27m/
  25m) → marqués `Unknown` au lieu de deviner Platform/Wall. **P1 : voir le mesh → trancher.**
- 🟡 QA-01 (Mineur) : `from_file` collapse silencieux des ids dupliqués (doc explicite ajoutée ;
  loader faillible = P1 si 2ᵉ consumer).
- ⚪ QA-02/04/05 (Cosmétique) : ground_offset négatif (maths prouvées), `include_str!` test-only
  (commenté), variantes `Cube.001/002/090` (géométrie identique = variantes kit probables).

**Vérification** (pas de runtime — pure data) : `cargo test -p forgia-worldgen` → 9 passed.

---

## ✅ P1 — Points + spawn instancié (LIVRÉ 2026-06-07)

La crate `forgia-worldgen` devient un **plugin Bevy**. Modèle points+attributs + spawn
budgété + sensor + debug viz. **Découplé** : ground height = `GroundSampler` injecté
(défaut plat), **0 dép forgia-terrain / gameplay** (confirmé : l'autre terminal édite
forgia-terrain ET forgia-stage → couplage évité).

**Livrables :**
- `points.rs` — `Point` / `PointCloud` (modèle central), `GroundSampler` (Box<dyn Fn> injecté),
  `generate_row` + `generate_showcase_row` (modules variés, déterministe par id).
- `spawn.rs` — `sys_spawn_drain` budgété (**8 modules/frame**, anti-freeze pilier perf #3) :
  mesh par nom (`gltf.named_meshes`), transform **grounded** via `placement_y` (pivot P0),
  collider per `ColliderKind` (Cuboid/Cylinder depuis l'AABB ; ConvexHull/TriMesh via
  `Collider::from_bevy_mesh`). `WorldgenModule` marker + `SpawnQueue` + `WorldgenStats`.
- `sensor.rs` — `forgia2_worldgen.json` (1 Hz) : registry_modules, spawned, pending, last_row +
  severity (warn si registre vide).
- `debug_viz.rs` — gizmos AABB monde (F8 toggle).
- `lib.rs` — `ForgiaWorldgenPlugin` : load registry (fs) + GLB (asset_server, **+1 call-site
  L1**, noté) au Startup ; **F7** = spawn rangée showcase devant la caméra ; F8 viz ; sensor
  in `GameSet::Sensors`.
- **Wiring** `forgia-game` : dep workspace + `Cargo.toml` + `ForgiaWorldgenPlugin` dans le
  tuple mode-specific (lib.rs:97). `cargo check -p forgia` OK.
- **10 tests** (9 P0 + sensor severity), clippy 0 warning.

**Demo observable (F7)** : une rangée de ~8 modules variés (2 Platform / 2 Pillar / 2 Wall /
2 Prop — pivots très différents) apparaît au sol devant la caméra, **tous posés flush** sur la
même ligne de sol → preuve visuelle du grounding P0. Re-F7 = nouvelle rangée (clear auto).

**Vérification runtime** : `forgia.exe` (release-fast) → entrer en Roguelite → **F7**.

**Suivis P2+** : brancher un vrai `GroundSampler` terrain (conformance pentes, story P4) ;
remplacer la rangée demo par un layout recette-driven (P2) ; instancing GPU explicite (P6).

---

## ✅ P2 — Recette grille → hameau data-driven (LIVRÉ 2026-06-07)

Couche **recette** (génome TOML hot-reload) + layout grille simple → la rangée demo P1 est
remplacée par un **petit hameau généré depuis une recette**. La variété vient de la donnée.

**Livrables :**
- `recipe.rs` — `HamletRecipe` (seed, grid_cols/rows, cell_size, jitter, scale, fill_chance,
  yaw_random, building_roles, border, border_role) + `load_recipe` (TOML, `#[serde(default)]`,
  fallback intégré). Rôles désérialisés directement en `AssetRole` (`["Prop","Pillar"]`).
- `assets/genomes/worldgen/hamlet.toml` — la recette (commentée, hot-éditable).
- `points.rs::generate_hamlet` — grille jitterée (bâtiments intérieurs `fill_chance` + ceinture
  de murs) → `PointCloud` (modèle P1 réutilisé). **RNG splitmix64 inline déterministe** (seed) ;
  P4 passera à `forgia-rng` (seeds hiérarchiques). Filtre hauteur ≤16 (pas de spire de 50 m).
- `lib.rs` — `sys_worldgen_input` : **F7** = nouveau hameau devant la caméra ; **Shift+F12** =
  re-lit la recette + régénère sur place (**hot-reload**). `LastHamletPlacement` mémorise le spot.
- **15 tests** (5 nouveaux : recette sane/parse, hameau déterministe, seed→variété, grounding).
  clippy 0.

**Demo (F7 / Shift+F12)** : un hameau (grille 6×5, ceinture de murs + bâtiments props/piliers)
apparaît au sol devant la caméra. Édite `seed`/`grid`/`roles` dans `hamlet.toml` → **Shift+F12**
→ disposition différente. Déterministe (même seed → même hameau, testé).

**Vérification runtime** : `forgia.exe` → Roguelite → **F7** (hameau) → éditer `hamlet.toml` →
**Shift+F12** (régénère). Sensor `forgia2_worldgen.json::last_row` = nb modules du hameau.

**Suivis P3+** : routes (tensor field) + parcelles (subdivision) ; seeds hiérarchiques + chunk
streaming (P4) ; grammaire bâtiments + injection POI hand-crafted (P5) ; bake + LOD (P6).

---

## 1. Contexte & objectif

Forgia = moteur **IA-native** : le créateur décrit (« une ville-forge dans un cratère
volcanique »), l'IA construit. Le worldgen est la brique qui **transforme une recette
en monde** : villes, villages, maps, donjons. Cible : un système **du niveau du marché**
(Unreal PCG production-ready 2025) mais idiomatique Bevy/ECS et data-driven (génome).

**Non-objectif** : refaire la procgen de TERRAIN (`forgia-terrain` existe — biomes,
chunks, heightmap). Le worldgen **se branche dessus** (conformance sol, streaming).

---

## 2. Décisions d'architecture (issues de l'audit)

### 2.1 Trois couches
| Couche | Rôle | Équivalent marché |
|---|---|---|
| **Recette** (génome TOML, hot-reload) | params + règles : pools, densité, poids, biome→archi, seed | Unreal *Graph Parameters* / Houdini params promus |
| **Registre d'assets** (RON versionné) | métadonnées par module : `pivot_offset`, `aabb`, `role` (walkable/deco/loot), `collider_kind`, `target_size`, `category` | modules pré-modélisés + métadonnées (standard film/jeu) |
| **Moteur** (`forgia-worldgen`, Rust) | lit recette + registre → génère | algorithme de génération |

> Crate : **démarrer avec `forgia-worldgen` seul** (modules `registry` / `recipe` /
> `layout` / `spawn`). **Extraire `forgia-asset-registry`** quand un 2ᵉ consommateur
> (terrain/décor/éditeur) le réclame. Pas de sur-ingénierie tant qu'il y a 1 client.

### 2.2 🔑 Modèle de données central : POINTS + ATTRIBUTS (insight n°1 Unreal PCG)
On ne place **pas** les bâtiments directement. Pipeline :
```
LAYOUT → nuage de POINTS (transform + attributs: density, seed, steepness, bounds,
         category, biome, ...) → FILTRE/TRANSFORME → SPAWN (instancié)
```
Découplage = composabilité + perf (points cheap, traités en masse data-oriented, puis
instanciés). Mappe parfaitement ECS (`par_iter`, Changed<T>).

### 2.3 Moteur en 3 étapes
1. **LAYOUT** : routes (tensor field) → subdivision de blocs en parcelles convexes →
   **points + attributs** par parcelle/cellule.
2. **RÉSOLUTION** : grammaire / templates → quel **module** pour chaque point (selon
   attributs + seed hiérarchique). Sélectionne dans les pools du registre.
3. **SPAWN** : instancie les modules (groupés par mesh+matériau → GPU instancing),
   **par chunk**, **async**, **conformance terrain** (pose sur `heightmap_at` + FlattenZones).

---

## 3. Les 3 piliers PERFORMANCE (non négociables)

1. **Chunk-streaming** : générer/décharger par chunk autour du joueur. **RÉUTILISER le
   streaming de `forgia-terrain`** (même grille de chunks) — ne pas réinventer.
2. **GPU instancing** : concevoir le spawn pour produire des **batches instanciables**
   (Bevy auto-instancie même mesh+matériau). Grouper par module avant spawn.
3. **Génération async / par lots** : jamais bloquer la frame (cf colliders incrémentaux
   du loot_room, `~20/frame`). Génération en tâche de fond ou étalée sur N frames.

---

## 4. Déterminisme — SEEDS HIÉRARCHIQUES
`world_seed → region → block → parcel → building`. Chaque niveau dérive du parent
(hash). Permet : régénération à l'identique (roguelite seedé), variation locale, et
re-génération partielle d'un chunk sans tout refaire. Utiliser `forgia-rng` existant.

---

## 5. Techniques de layout (combo AAA validé)
- **Routes** : **tensor fields** (eigenvecteurs major/minor ⊥ → réseaux réalistes,
  contrôle global + local, conformance terrain/côte) — base de CityEngine.
- **Parcelles** : **subdivision récursive de blocs** convexes le long des arêtes
  parallèles les plus longues jusqu'à une taille cible.
- **Bâtiments** : **grammaire / templates** (footprint → étages → toit). L-system pour
  les variations.
- **Zones denses contraintes** (optionnel) : **WFC** ⚠️ SEULEMENT sur petites surfaces
  tuilées (lent + peut ne pas terminer sur grande ville → jamais la ville entière).

---

## 6. Hybride hand-crafted + procédural (best-practice n°1)
La recette doit pouvoir **injecter des landmarks** : « place la forge ICI, génère le
reste autour ». S'appuyer sur le système **POI de `forgia-stage`** (anchors). Réserve
des parcelles pour les pièces hand-crafted avant le remplissage procédural.

---

## 7. Bake/cache vs runtime
Supporter les deux (comme Unreal/Houdini) :
- **Runtime** : génération à la volée (IA-native, exploration infinie).
- **Bake/cache** : générer une fois une map statique → sérialiser le résultat (points
  résolus) → recharger sans regénérer. Gain perf sur maps fixes.

---

## 8. Observabilité & validation (règles Forgia)
- **Sensor** `forgia2_worldgen.json` : compteurs (parcels, modules spawnés, chunks
  actifs, temps de génération/chunk, points par catégorie) + health alert si génération
  > budget ms ou overlaps détectés.
- **Debug viz** (touche dédiée, ex. F11) : afficher routes / parcelles / points /
  bounds — l'équivalent du debug per-node Unreal.
- **Validation** : pas d'overlap entre modules, parcelles convexes valides,
  accessibilité (routes connectées), rien qui flotte/s'enfonce (le piège pivot),
  conformance terrain. Cohérent avec `QUALITY_GATE.md`.

---

## 9. Structure du crate `forgia-worldgen`
```
crates/forgia-worldgen/
  src/
    lib.rs            # WorldgenPlugin, registres de systèmes
    registry.rs       # AssetMeta (pivot/aabb/role/collider/target/category) + loader RON
    recipe.rs         # CityRecipe (génome TOML) + loader (forgia-genome-core)
    points.rs         # PointCloud + Attributs (le modèle central)
    layout/
      roads.rs        # tensor fields
      parcels.rs      # subdivision de blocs
    resolve.rs        # grammaire/templates → module par point (seed)
    spawn.rs          # instancing + async + conformance terrain
    seed.rs           # seeds hiérarchiques
    cache.rs          # bake/sérialisation
    sensor.rs         # forgia2_worldgen.json
    debug_viz.rs      # gizmos routes/parcelles
assets/genomes/worldgen/    # recettes (city_forge.toml, hamlet_*.toml...)
assets/registry/asset_meta.ron   # métadonnées modules
```

### Dépendances
`bevy`, `bevy_rapier3d` (colliders), `forgia-core`, `forgia-genome-core` (loader),
`forgia-rng` (seeds), `forgia-terrain` (conformance + streaming). Pas de dép sur les
crates gameplay (roguelite/rpg) → réutilisable partout.

---

## 10. Invariants / Locks à protéger
- **Genome-driven** : 0 hardcode (règle `no-hardcode.md`). Toute valeur dans la recette.
- **Async hot path** : 0 blocage de frame, 0 alloc dans les boucles chaudes (`scalability.md`).
- **Observable** : sensor + health (règle `observability-required.md`).
- **Réutiliser forgia-terrain** : streaming + conformance, ne PAS dupliquer.
- **Découplage** : worldgen ne dépend PAS du gameplay (réutilisable RPG/Roguelite/éditeur).
- **Déterministe** : même seed → même monde (testable).

---

## 11. Phases (incrémentales, preuve par la valeur à chaque étape)

| Phase | Livrable | Preuve |
|---|---|---|
| **P0 — Registre** | `registry.rs` + `asset_meta.ron` des ~92 pièces ithappy (semi-auto via parseur GLB) | test : chaque pièce posée au bon endroit (plus de gate enfoncée) |
| **P1 — Points + spawn** | `points.rs` + `spawn.rs` : spawn instancié d'1 module sur le terrain (conformance) + sensor + debug viz | une rangée de modules instanciés sur le sol, perf OK (sensor) |
| **P2 — Recette grille** | `recipe.rs` + layout grille simple → petit hameau data-driven | hameau hot-reloadable (Shift+F12), variété par recette |
| **P3 — Routes + parcelles** | `layout/roads.rs` (tensor field) + `parcels.rs` | layout de ville cohérent (routes + parcelles) en debug viz |
| **P4 — Streaming + seeds** | `seed.rs` hiérarchique + branchement chunk-streaming forgia-terrain | ville qui stream autour du joueur + reproductible (même seed) |
| **P5 — Grammaire + hand-crafted** | `resolve.rs` (templates bâtiments) + injection POI (forgia-stage) | variété de bâtiments + landmarks placés (la forge) |
| **P6 — Bake + LOD + perf** | `cache.rs` + LOD distant + tuning | map bakée rechargée instantanément, budget frame tenu |

Chaque phase = `cargo check` + clippy 0 + sensor + récap test in-game (règle in-game-test-recap).

---

## 12. Critères d'acceptation (globaux)
- AC1 : génère une ville/village data-driven depuis une recette TOML hot-reload.
- AC2 : déterministe (même seed → même monde, test unitaire).
- AC3 : streaming par chunk, 0 freeze, budget frame respecté (sensor le prouve).
- AC4 : modules posés correctement (pivot/role/collider — 0 enfoncé/flottant/bloquant).
- AC5 : injection de landmarks hand-crafted dans le procédural.
- AC6 : conformance terrain (pose sur la hauteur réelle + aplanit les parcelles).
- AC7 : sensor `forgia2_worldgen.json` + debug viz + validation (0 overlap).
- AC8 : 0 dép gameplay (réutilisable RPG/Roguelite/éditeur).

---

## 13. Risques
- **Perf** : 1000s de modules → instancing + chunk + LOD obligatoires dès P1 (pas après).
- **Conformance terrain** : pente raide → bâtiments penchés (cf bug récurrent assets-sous-map). Mitiger via FlattenZones par parcelle.
- **Coordination** : créer le crate = toucher `Cargo.toml` workspace `members` → **accord user + signaler** (règle multi-terminal, l'autre terminal y travaille).
- **WFC** : tentation de l'utiliser partout → le cantonner aux petites zones tuilées.
- **Scope creep** : viser P0→P2 d'abord (preuve de valeur), pas tout d'un coup.

---

## 14. Sources (audit 2026-06-07)
- [Unreal PCG Overview](https://dev.epicgames.com/documentation/en-us/unreal-engine/procedural-content-generation-overview) (modèle points+attributs, graphes, World Partition)
- [PCG GPU Processing](https://dev.epicgames.com/documentation/en-us/unreal-engine/using-pcg-with-gpu-processing-in-unreal-engine)
- [Survey of Procedural City Generation (citygen.net)](http://www.citygen.net/files/Procedural_City_Generation_Survey.pdf)
- [Tensor-field road networks (SIGGRAPH 2008)](https://www.cs.drexel.edu/~deb39/Classes/ICG/Assignments_new/cardillo_presentation.pdf)
- [Extend WFC to large-scale (arXiv 2308.07307)](https://arxiv.org/pdf/2308.07307)
- [Houdini Build a City with PDG (SideFX)](https://www.sidefx.com/tutorials/foundations-build-a-city-with-pdg/)
- [ECS data-oriented for persistent worlds](https://www.daydreamsoft.com/blog/ecs-2-0-data-oriented-micro-kernel-architectures-for-massive-persistent-game-worlds)
- [Optimizing procedural worlds (Wayline)](https://www.wayline.io/blog/optimizing-game-performance-procedural-content-customization)

---

## 15. Cross-refs Forgia
- `forgia-terrain` (streaming + conformance à réutiliser)
- `forgia-stage` (POI/anchors pour l'injection hand-crafted)
- `forgia-genome-core` (loader recette), `forgia-rng` (seeds)
- `reference_ithappy_demo_level_integration` (parseur GLB, pivots, registre — la base de P0)
- Règles : `no-hardcode.md`, `observability-required.md`, `scalability.md`, `concept-first.md`, `multi-terminal-coordination.md`
