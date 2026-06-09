# Rapport — Végétation RPG invisible (régression 2026-06-09)

> Investigation multi-agents (6 angles + vérification adversariale, 13 agents) + instrumentation sensor-first.
> Symptôme : en mode RPG, aucune végétation (arbres) ni asset de biome visible, même à 60m du village.

## ✅ RÉSOLU — root cause + fix (2026-06-09)

**Cause racine** : `sys_clear_village_foliage` (`forgia-rpg/src/worldgen_village.rs`, introduit par commit 28df37d, **jamais run avant le rebuild B4** = binaire stale) lisait le **`GlobalTransform`** des arbres pour décider du clear dans le disque village (50m). Mais les arbres sont spawnés par `populate_new_chunks` dans le **même Update** que ce système, et la **propagation des `GlobalTransform` tourne en PostUpdate** → la frame du spawn, le `GlobalTransform` vaut l'**identité `(0,0,0)`**. Distance de `(0,0)` au centre village `(16,16)` = **22.6m < 50.2m** → le clear pensait que **CHAQUE arbre était dans le village** et les **despawnait tous, chaque frame, avant le rendu**.

**Preuve décisive (instrumentation sensor)** :
- `spawn_rings_0_50_100_150 = [0, 563, 711, 49]` → les arbres spawnent bien, répartis 50-150m (via `Transform`).
- `village_clear : peak_min_d == peak_max_d == 22.6m` → le clear les voyait **tous au même point** = l'origine (GlobalTransform non propagé).
- `cleared_total ≈ total spawné` → le clear despawnait quasiment tout.

**Fix** : `sys_clear_village_foliage` utilise `Transform` (local) au lieu de `GlobalTransform`. Les arbres sont des entités racine → `Transform.translation` = position monde, correcte **immédiatement** (cohérent avec `spawn_village_paths_when_loaded` qui utilisait déjà `Transform`).

**Validation runtime** : `live_diag.live_entities` 0 → **943**, `instantiated: 943` (rendus), répartis 50-138m, `cleared_total` 1358 → **35** (vrais bords). Végétation visible. ✅

**Observabilité ajoutée (permanente)** : `forgia_vegetation.json → live_diag {live_entities, instantiated, min/max_dist_excl}` = vrai compte d'entités vivantes (le `total_trees` historique est un compteur de spawn cumulatif **menteur** — il a induit le diagnostic en erreur pendant des heures). Le reste de l'instrumentation (spawn_rings, peak, sensor village_clear, compteurs despawn) était du scaffolding de debug, **retiré** après résolution.

**Leçon** : [[feedback_unvalidated_wip_detonates_on_rebuild]] — encore un cas. Et : un sensor de **compteur cumulatif** ≠ un sensor de **query live** ; ne jamais déduire la présence d'entités d'un compteur incrément-only.

---

## Verdict (investigation, conservé pour historique)

**La cause n'est dans AUCUN des 6 angles testés** — tous réfutés avec preuves. Le problème est **en aval du spawn ECS**.

**Découverte clé** : `forgia_vegetation.json → total_trees: 978` est un **compteur de spawn incrément-only** (`lib.rs:404` `veg.total_trees += count`, jamais décrémenté), **PAS** une query d'entités vivantes. Preuve : `per_biome Forest:2062 > total:1180` (cumul vs net). Donc « 978 arbres » ne prouve **pas** 978 arbres rendables. Mon diagnostic précédent reposait sur ce compteur menteur.

**2 candidats restants**, à départager par le run instrumenté (§Diagnostic) :

| # | Candidat | Confiance | Mécanisme |
|---|----------|-----------|-----------|
| 1 | **Non-persistance** : les entités `VegetationTree` sont despawnées / ne s'instancient pas en mailles rendables | ~0.5 | query override `q_targets` vide (`0/0/0` sur 388s) + `World cleaned: 0 trees` (query live au exit) |
| 2 | **Géométrie d'exclusion / streaming** : arbres spawnés uniquement au-delà du disque ~50m centré (16,16), joueur dedans (~22m), rien ne persiste près de lui | ~0.35 | `FOLIAGE_CLEAR_RADIUS ≈ 50.2m`, spawn skippé dans le disque (`lib.rs:310-315`) |

⚠️ **Nuance importante** (révisée vs workflow) : les 2 preuves de « non-persistance » sont **faibles** :
- `0/0/0` de l'override peut juste signifier « marqueur `NeedsTrunkOverride` non inséré » (cfg), pas « arbres despawnés ». De toute façon l'override n'affecte que le tronc — sans lui, l'arbre garde son material GLB et reste visible.
- `World cleaned: 0 trees` est au **exit** RPG (chunks déjà déchargés) → ne prouve pas l'absence pendant la session.

→ **Aucune donnée live existante ne tranche.** D'où l'instrumentation ajoutée.

## Angles réfutés (avec preuve)

| Angle | Réfuté car | Conf. |
|-------|-----------|-------|
| Render/material global | Le village (91 tuiles + 20 bâtiments) utilise le **même** chemin `SceneRoot + StandardMaterial` que les arbres et **est visible** ; aucune mutation globale de material/visibilité en V2 | 0.90 PAS la cause |
| Commit 79683fb (tronc/feuillage) | +42/-4, aucun code visibilité/despawn ; logique inatteignable (override 0/0/0) ; pire cas = canopée bark, jamais invisible | 0.04 |
| Commit 28df37d (clear village) | `sys_clear_village_foliage` **borné à ~50m** autour de (16,16) (`worldgen_village.rs:567-573`), et populate **skippe** déjà ce disque → pas de recouvrement, ne despawn pas les arbres au-delà. Loader legacy inerte. | 0.10 |
| basis-universal + bark .ktx2 | Les GLB d'arbres n'ont **aucune texture** (vertex-color/factor) ; les .ktx2 ne servent qu'au material bark appliqué à **0** arbre ; binaire non-stale ; 0 erreur ktx2/transcode | 0.95 PAS la cause |
| Y terrain vs Y foliage | Terrain et foliage appellent la **même** `heightmap_at()` + **même** `flatten_zones.sample()`, offset XZ identique (ZERO) ; `forgia_terrain_lod.json` lod0_y==lod2_y (delta 0.0m). Pas d'enterrement. | 0.90 PAS la cause |
| Lifecycle / LOD despawn | Despawns localisés (disque 50m) ou distance-based (mêmes chunks que terrain visible) ; aucun despawn de masse loggé | 0.90 PAS la cause |

## Comment ça a disparu (timeline)

- **6dbd89c** = dernier état foliage **validé runtime** (« parfait », arbres visibles avec materials GLB).
- Après, le **binaire est resté stale** jusqu'à cette session (standup : sources anim plus récentes que `forgia.exe`).
- Mon rebuild (B4 KTX2) a été le **premier run combiné** de 3 commits jamais exécutés : `79683fb` (réfuté), `28df37d` (B1 village + `FoliageExclusionDisc` actif 50m — suspect candidat #2), `0088552` (B2 terrain, orthogonal) + l'uncommit bark (réfuté).
- **Pattern documenté** [[feedback_unvalidated_wip_detonates_on_rebuild]] : du WIP non-validé runtime qui « explose » au prochain build. La règle CLAUDE.md « binaire = preuve » est exactement ce qui a masqué le bug jusqu'ici.

## Fix recommandé

**Ne PAS toucher** les 6 angles réfutés (`material_override.rs`, Y terrain/foliage, `.ktx2`/basis-universal — tous corrects). Respect strict « ne pas casser ce qui marche ».

**Étape 1 — instrumenter (FAIT, story-588)** : champ `live_diag` ajouté à `forgia_vegetation.json` (`forgia-foliage/src/lib.rs` `write_vegetation_sensor`) avec query LIVE :
- `live_entities` = vrai nombre d'entités `VegetationTree` vivantes
- `instantiated` = celles dont le `SceneRoot` a produit des `Children` (mailles rendables)
- `inside_excl` / `min_dist_excl_m` / `max_dist_excl_m` = distribution spatiale vs disque d'exclusion

**Étape 2 — fix selon le diagnostic** :
- `live_entities ≈ 0` → **candidat #1** : tracer le despawn (story Standard, lifecycle ECS).
- `live_entities` haut + `instantiated ≈ 0` → **scènes vides** (SceneRoot n'instancie pas) → render path.
- `live_entities` haut + `instantiated` haut + `min_dist_excl_m` grand → **candidat #2** : réduire `FOLIAGE_CLEAR_RADIUS` (`worldgen_village.rs:63`) à l'emprise des murs, ou décaler le spawn joueur hors du disque.

**Hygiène (story suiveuse, hors cause)** : `sys_clear_village_foliage` despawn sans décrémenter `VegetationManager` → compteur sensor menteur. Router via un helper décrémentant.
