# story-663 — Perf : fusion de la géométrie statique d'arène par cellule × matériau (sol + murs)

**Statut** : REVIEW (code complet + tests, validation runtime user en attente)
**Créée** : 2026-07-19 · **Origine** : audit 360° C3/M13 + question user « fusionner les meshes ? » → go explicite (« faut pas que ça lag » sur petites configs)
**Scale BMAD** : Standard (2 fichiers, 1 crate — forgia-stage, hors arbre chaud)

## Contexte

L'audit perf a établi que le CPU est borné par la **scène statique** (13 k entités / 2 254 meshes visibles), pas par le gameplay. Le sol d'arène spawnait **1 597 entités-tuiles** (+ ~3 200 nœuds de scène GLB), chacune payant transform-propagation + visibilité + extraction par frame. Bevy instancie déjà le GPU (mêmes mesh+matériau → peu de draw calls) : le coût restant est ce par-entité CPU. Cible ship = **60 fps GTX 1060** — nos mesures confortables viennent d'une 4070 Ti.

## Livré (Incrément 1 : le sol)

**Nouveau module [`floor_merge.rs`](../../crates/forgia-stage/src/floor_merge.rs)** :

- `plan_floor_tiles()` — extrait **pur** de l'ex-boucle de spawn (mêmes maths : culling circulaire, centre propre 1/3, mix dirt/rocks, yaw déterministe 0/90/180/270°). Testé headless (5 tests).
- **Sondes cachées** : 1 instance `Visibility::Hidden` par type de tuile — on lit les meshes/matériaux/transforms **réellement produits** par le scene-spawner (zéro hypothèse sur la hiérarchie GLB — même philosophie que `NeedsAssetCalibrate`).
- `sys_build_merged_floor` — retry jusqu'à sondes prêtes (pattern `sys_collide_authored_pieces`), puis fusion `Mesh::transformed_by` + `Mesh::merge` par **(cellule 8×8 tuiles = 32 m) × matériau** → ~30-45 meshes au lieu de 1 597 entités. Vertices bakés en espace monde, AABB par cellule → le frustum culling reste efficace (pas de mesh géant tout-ou-nothing).
- **`MergedFloorCache`** par extent : re-entrer dans une salle de même taille = respawn direct des handles fusionnés, **zéro rebuild, zéro hitch** (les salles s'enchaînent toutes les ~2 min).
- **Fallback garanti** : timeout 5 s (GLB KO) ou `Mesh::merge` en échec → spawn tuile-par-tuile historique. Le sol ne peut JAMAIS manquer.
- **Collider inchangé** (1 cuboid global, comme avant).

**`lib.rs`** : bloc sol remplacé par plan + sondes + `PendingFloorMerge` ; système + cache enregistrés ; observabilité : `forgia2_stage.json` expose `floor_merged_cells` + `floor_merge_pending` (pending soutenu = sondes bloquées, next_step dans le doc du champ).

## Gain attendu

- Entités sol : **1 597 parents + ~3 200 nœuds → ~30-45 meshes** (−98 %) → baisse mesurable de `forgia2_perf_diag.total_entities` (~13 400 → ~8 700) et du coût CPU extraction/culling/transforms.
- À valider sur la cible réelle : le gain fps sur 4070 Ti sera modeste (déjà à 240) — le vrai juge = GTX 1060.

## Inc.2 — Murs fusionnés + module généralisé (2026-07-19, après validation user Inc.1 « c'est mieux »)

Le module est généralisé en **`PendingStaticMerge`** (label "floor"/"walls", sondes taguées, cache par clé `label:extent`, fallback `SceneRoot` uniforme, fix race QA-663 #1 conservé) — la leçon anti-duplication H3 appliquée à nous-mêmes. Les **~130 prefabs mur** (`ramparts_hex_tiled_positions` × 1 GLB par kit) passent par la même fusion → ~6-12 meshes périmètre. Colliders murs inchangés (6 cuboids segment). Sensor : `walls_merged_cells` + `static_merge_pending` (remplace `floor_merge_pending`). Note assumée : `forgia_prefab.json::total_spawned` ne compte plus les murs (hors pipeline prefab).

## Validation runtime Inc.1+2 (session user 2026-07-20 01:49) + Inc.2b spawn étalé

**Mesures réelles** (forgia2_run.log + perf_diag) :
- `Static merge 'floor': 1597 instances → 68 meshes` · `'walls': 138 → 20 meshes` ✅
- **`total_entities` : 13 400 → 4 615 (−65 %)** — au-delà de l'attendu (~8 700)
- Frame max en jeu : **18,4 ms** (vs 109 ms avant) ; avg 4,2 ms
- User : « c'est mieux mais pas parfait » → nouveau profil de freeze : **burst au 1er rendu de run** (t=37-44 : 412/190/190/190 ms) = spécialisation pipeline + upload GPU des 88 meshes fusionnés + 1re vague, tout sur une frame. Résiduels 50-64 ms aux vagues (t=53/57/65).

**Inc.2b (fix du burst)** : les meshes fusionnés passent par une **file de spawn étalée** (`MergedSpawnQueue`, 8 cellules/frame, ~11 frames pendant le fade-in) — build et cache-hit. Même garde anti-péremption stage_id que QA-663 #1. Clippy + 109 tests verts.

## Hors scope (incréments suivants)

- **Inc.3 coquille authored + props statiques morts** (24 pièces + 170 props) — attention : props avec anchors/POI/`melee_pit` restent séparés (rôles gameplay).
- **Prewarm pipelines** : Hanabi/VFX = **déjà prewarmé** (forgia-effects/lib.rs:132, dummies par EffectAsset + textures, story-647). Reste l'hypothèse « pipeline skinned des ennemis au 1er spawn de vague » — NON traitée volontairement (no-speculative-fix : `Visibility::Hidden` ne compile PAS les pipelines mesh — un prewarm fiable exige l'entité en vue ~1 frame ; à cibler après MESURE des freezes post-merge).

## Auto-QA (qa-lead, 2026-07-19)

**0 Bloquant / 0 Majeur.** Preuves fournies : parité `plan_floor_tiles` vérifiée **bit-à-bit** contre `git show HEAD` (pas juste le docstring) ; cleanup des 4 branches terminales ; `Mesh3d #[require(Transform)]` confirmé dans bevy_mesh-0.18.1 ; despawn récursif ; multi-matériaux ; borrow Assets ; sensor aligné aux 3 emplacements ; cache borné (2 extents dans le genome actuel : 90 m crypts / 80 m forge_sanctum).

- 🟡 **Mineur #1 CORRIGÉ dans la foulée** : race transition×cache-hit same-frame (un cache-hit pouvait spawner un sol orphelin hors du snapshot de despawn de la transition). Fix : `PendingFloorMerge.stage_id` — tout plan dont le stage ne correspond plus au `StageLoadRequest` courant est jeté sans spawner.
- 🔵 Cosmétique #2 : commentaire « même sol que le FPS » pré-existant et faux (layouts différents) — reformulé dans le nouveau bloc.
- **Risque résiduel n°1 (non tranchable en lecture)** : si le scene-root des GLB tuiles porte un offset/scale baked, le sol serait décalé — c'est exactement la « variante si KO » n°2 du test runtime ci-dessous. Seul le lancement tranche.
- Dette actée : `MergedFloorCache` sans éviction (inoffensif à 2 extents ; LRU si les tailles d'arène deviennent variées).

## Acceptance Criteria

- [x] `plan_floor_tiles` = mêmes maths que l'ex-boucle (tests : déterminisme, bornes, centre propre, yaw quantifié, grouping cellules)
- [x] Fusion par cellule × matériau, cache par extent, fallback tuile-par-tuile
- [x] Collider sol inchangé ; cleanup via `StageArenaMarker` (pending + sondes inclus)
- [x] Sensor `forgia2_stage.json` : `floor_merged_cells` / `floor_merge_pending`
- [x] `cargo clippy -D warnings` + tests forgia-stage verts
- [ ] Validation runtime : sol visuellement identique (mix propre/dirt/rocks, rotations) + `floor_merged_cells` ~30-45 + `total_entities` en baisse de ~4 500

## Test runtime (après rebuild)

1. **Action** : `cargo build -p forgia -j 4`, lancer, entrer dans une run Roguelite.
2. **Effet attendu** : sol strictement identique visuellement (même mix de tuiles, mêmes rotations). Log : `[stage-arena] Floor: 1597 tuiles planifiées → merge par cellule` puis `Floor merge: 1597 tuiles → N meshes fusionnés`.
3. **Où observer** : `forgia2_stage.json::floor_merged_cells` (~30-45, pending=false) ; `forgia2_perf_diag.json::load.total_entities` (baisse ~4 500 vs ~13 400 avant) ; salle 2 : log `cache hit`.
4. **Variantes si KO** :
   - Sol invisible → lire `floor_merge_pending` : true soutenu = sondes bloquées (GLB) ; false + cells=0 = fallback actif (log warn) — me ping avec le log.
   - Sol décalé/échelle fausse → hiérarchie GLB non plate malgré les sondes — me ping, j'ajoute la composition de nœuds.
   - Trous dans le sol aux bords de cellules → problème de culling AABB — me ping avec un screenshot.
