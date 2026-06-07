# Story-578 — `forgia-worldgen` : moteur de génération procédurale (villes / villages / maps)

> **Statut** : ✅ **DONE — P0 → P6 COMPLETS** (2026-06-07). Pipeline procédural de bout en bout.
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

## ✅ P3 — Routes (tensor field) + parcelles (subdivision) (LIVRÉ 2026-06-07)

Layout de **ville cohérent** : réseau de routes tracé depuis un **tensor field** + **blocs
subdivisés en lots convexes**, visualisé en debug viz. Nouveau module `layout/` (géométrie 2D
pure, Vec2, 0 ECS).

**Livrables :**
- `layout/roads.rs` — `TensorField` (grille + composante tangentielle/anneaux radiale, blend) →
  `generate_roads` = tracé de **streamlines avec séparation** (technique tensor-field roads,
  Chen 2008 simplifiée). `RoadNetwork { segments: [a,b,kind] }` (majeures/mineures). Déterministe.
- `layout/parcels.rs` — `generate_parcels` : tuile en blocs (taille = espacement routes) →
  `subdivide_rect` (**subdivision récursive le long du plus long côté** → lots convexes ≤ aire
  cible). `Parcel { polygon, center }`. Déterministe.
- `recipe.rs` — `CityRecipe` (size, base_angle, radial_strength/radius, road_spacing/step/sep,
  parcel_target_area/inset, building_scale/role) + `load_city_recipe` (TOML, fallback).
- `assets/genomes/worldgen/city.toml` — recette ville (hot-éditable).
- `lib.rs` — `CityLayout` Resource (routes + parcelles + transform monde) ; `sys_worldgen_input`
  étendu : **F9** = ville (routes+parcelles viz + 1 bâtiment grounded/parcelle), **Shift+F12**
  régénère le dernier (hameau OU ville). `build_city` déterministe.
- `debug_viz.rs` — `sys_layout_gizmos` : routes (or=majeures, gris=mineures) + contours parcelles
  (vert) + marqueurs centres. Toggle F8.
- `sensor.rs` — `forgia2_worldgen.json` + `city_roads` / `city_parcels`.
- **22 tests** (7 nouveaux : routes déterministes/in-bounds, field radial courbe, subdivision
  conserve l'aire/borne le lot, parcelles déterministes, target→densité, recettes parse). clippy 0.

**Demo (F9 + F8)** : presse **F9** devant toi → une ville apparaît : bâtiments posés sur une
grille de parcelles, **F8** révèle routes + parcelles en gizmos. Édite `city.toml`
(`radial_strength = 0.8` → routes en anneaux ; `road_spacing` → taille des blocs) + **Shift+F12**.

**Limites P3 (honnêtes)** : routes et parcelles partagent bounds+orientation mais le couplage
topologique exact (faces de routes → parcelles) est différé ; field radial courbe les routes
mais les parcelles restent rectangulaires (defaut `radial_strength=0` → aligné). RNG splitmix64
inline (seeds hiérarchiques = P4). Pas de mesh de route (gizmos only ; meshing = P5/P6).

**Suivis P4+** : seeds hiérarchiques (`forgia-rng`) + chunk streaming (forgia-terrain) ;
grammaire bâtiments + injection POI hand-crafted (P5) ; mesh routes + bake + LOD (P6).

---

## ✅ P4 — Seeds hiérarchiques + chunk streaming (LIVRÉ 2026-06-07)

Une ville **infinie qui stream autour du joueur**, **reproductible** (même seed → même monde).
**Découplé** : worldgen possède sa propre grille de chunks (PAS de dép `forgia-terrain` — l'autre
terminal l'édite + invariant §10), position joueur via `Camera3d`, sol via `GroundSampler`.

**Livrables :**
- `seed.rs` — seeds hiérarchiques `world → chunk` : `SeededRng` (splitmix64), `derive(parent,
  index)`, `chunk_seed(world, cx, cy)`. Remplace le RNG inline de P2/P3 (source unique).
- `points.rs::generate_chunk` — contenu d'un chunk depuis `chunk_seed` (coords MONDE absolues,
  grounded). Généré **en isolation** (pas besoin des voisins) → reproductible.
- `recipe.rs::StreamRecipe` + `load_stream_recipe` ; `assets/genomes/worldgen/stream.toml`.
- `spawn.rs::spawn_module` — **refacto** : le spawn d'1 module (grounded + children + collider)
  extrait en fn réutilisable, partagé par le drain de queue ET le streamer.
- `streaming.rs` — `CityStreaming` resource + `sys_toggle_streaming` (**F10**) +
  `sys_stream_city` : charge les chunks dans `view_radius` (nearest-first, **budget 1 chunk/frame**),
  décharge les sortants. HashMap chunk→entities.
- `lib.rs` — **gating Roguelite** : toute la démo (F7/F9/F8/F10 + streaming) sous
  `run_if(in_state(GameMode::Roguelite))` (décision user 2026-06-07). Les fonctions de génération
  restent mode-agnostiques/réutilisables.
- `sensor.rs` — `+ stream_chunks` (chunks actifs).
- **27 tests** (5 nouveaux : RNG déterministe + ranges, chunk_seed stable/distinct/sans collision,
  chunk reproductible + dans sa cellule). clippy 0.

**Demo (F10)** : en Roguelite, **F10** active le streaming → des bâtiments apparaissent par chunks
autour de toi ; déplace-toi → de nouveaux chunks chargent devant, les anciens se déchargent
derrière. **Reproductible** : même `world_seed` (stream.toml) → exactement la même ville. Re-toggle
F10 (off→on) recharge la recette (hot-reload).

**Mode** : worldgen gaté **Roguelite uniquement** (choix user). Crate réutilisable ailleurs plus tard.

**Suivis P5+** : grammaire de bâtiments (footprint→étages→toit) + injection POI hand-crafted
(forgia-stage) ; mesh de routes + bake/cache + LOD distant (P6) ; couplage chunk↔terrain réel.

---

## ✅ P5 — Grammaire de bâtiments + landmarks hand-crafted (LIVRÉ 2026-06-07)

Les parcelles deviennent des **bâtiments empilés** (grammaire) et la recette injecte des
**landmarks hand-crafted** (la forge), les parcelles autour étant réservées.

**Livrables :**
- `resolve.rs` — `resolve_building(template, registry, world_xz, base_y, seed, scale)` :
  grammaire **base → N étages → toit**, modules empilés en Y via leurs hauteurs registre (chacun
  repose sur le précédent). Nb d'étages **seedé** → variété par parcelle. Déterministe. Pur (0 ECS).
- `recipe.rs` — `BuildingTemplate` (base/body/cap_role + min/max_floors) + `Landmark`
  (module_id + x/z + scale). `CityRecipe` étendu : `building` + `landmarks` + `landmark_reserve_radius`
  (remplace l'ancien `building_role` unique).
- `assets/genomes/worldgen/city.toml` — table `[building]` + `[[landmarks]]` (la Forge =
  `Cube.001` tour ~33 m au centre). ⚠️ piège attrapé : `[[landmarks]]` (pluriel) doit matcher le
  champ serde — test renforcé pour le garantir.
- `lib.rs::build_city` — réécrit : place les landmarks d'abord (réserve leur footprint), puis
  **résout chaque parcelle non réservée** en bâtiment via la grammaire (seed parcelle =
  `derive(city_seed, index)`, hiérarchique P4).
- **30 tests** (3 nouveaux resolve : bâtiment déterministe, modules empilés vers le haut depuis
  la base, nb de modules dans la plage ; + test landmark renforcé). clippy 0.

**Demo (F9 + F8)** : presse **F9** → une ville avec **bâtiments à plusieurs étages** (1-3, variés)
sur les parcelles + **la Forge** (grande tour) au centre, les parcelles autour étant vides
(réservées). Édite `[building]` (`max_floors = 6`) ou `[[landmarks]]` dans `city.toml` + **Shift+F12**.

**Limites P5 (honnêtes)** : grammaire = empilement simple (footprint→étages→toit), pas de variation
de footprint/façade (L-system = futur) ; landmarks placés par worldgen (PAS encore synchronisés aux
anchors `forgia-stage` — crate contendue + invariant découplage ; adaptateur = futur). Grammaire
appliquée à la ville F9 ; le streaming P4 garde des modules simples (extension possible).

**Suivis P6** : mesh de routes (au lieu de gizmos) + bake/cache (sérialiser une ville résolue) +
LOD distant ; couplage chunk↔terrain réel ; adaptateur landmarks→forgia-stage POI.

---

## ✅ P6 — Bake/cache + LOD distant (LIVRÉ 2026-06-07) — STORY COMPLÈTE

**Livrables :**
- `cache.rs` — `bake(points, path)` sérialise la **ville résolue** (points) en RON (via DTO plat
  `[f32…]`, évite la dép glam-serde) ; `load(path)` la recharge **sans régénérer**. Round-trip testé.
- `lib.rs` — **F9** bake la ville après génération (`worldgen_baked_city.ron`) ; **F11** recharge la
  ville bakée instantanément (0 régénération — parité bake Unreal/Houdini).
- `sys_worldgen_lod` — **LOD distant** : cull (`Visibility::Hidden`) des modules au-delà de
  `LOD_FAR_M=200 m` de la caméra, throttlé 0.25 s, écriture seulement sur transition → budget frame
  tenu sur grande ville streamée/bakée. (Pas de `VisibilityRange` dans le workspace → LOD custom.)
- **31 tests** (+1 bake round-trip). clippy 0.

**Demo** : **F9** (génère + bake) → **F11** (recharge instantané, log « loaded N baked modules (no
regen) »). Éloigne-toi d'une grande ville → les modules lointains se cullent (LOD). Perf : streaming
1 chunk/frame + spawn 8/frame + LOD cull.

**Limites P6 (honnêtes)** : pas de mesh de route (toujours gizmos — meshing = futur) ; LOD =
cull binaire (pas d'imposteur/billboard distant) ; bake = points résolus (pas les colliders/asset
handles, reconstruits au spawn). Couplage chunk↔terrain réel + adaptateur forgia-stage = futur.

---

## 🏁 Story-578 — bilan final

Pipeline procédural **complet de bout en bout**, en 7 incréments à valeur prouvée :
P0 registre → P1 spawn grounded → P2 recette hameau → P3 routes/parcelles → P4 streaming
reproductible → P5 grammaire + landmarks → P6 bake + LOD.

**Touches (Roguelite)** : F7 hameau · F9 ville (+bake) · F11 reload baké · F10 streaming ·
F8 debug viz · Shift+F12 hot-reload.

**Qualité** : 31 tests, clippy 0, **crate-feuille 0 dép gameplay/terrain** (découplé : `GroundSampler`
+ chunk grid + landmarks internes), tout gaté Roguelite. Invariants §10 tenus. Suivis ouverts =
mesh routes, couplage terrain réel, adaptateur forgia-stage, L-system façades (P7+ si besoin).

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
