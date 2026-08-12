# Peupler une map : ce que Bevy et Godot font, ce que nous faisons

> **Audit du 2026-08-11.** Question posée : comment optimiser les performances tout
> en gardant une expérience immersive avec **beaucoup de contenu dans un espace
> restreint** (Hall, arène peuplée, forêt) ?
>
> **Réponse mesurée, contre-intuitive : nous ne sommes pas GPU-bound.** Sur l'arène
> à **558 props**, 1 605 meshes se dessinent en **0,119 ms** et le GPU occupe
> **8,7 %** du frame. Le levier n'est pas le culling, c'est **l'étalement des
> transitions d'entités** — à l'arrivée *et* au départ.
>
> *Mis à jour le 2026-08-12 avec le run de l'arène peuplée, qui manquait le 08-11
> (`decor_total: 0`). Il confirme le verdict et en durcit une conclusion : ajouter
> 558 props a fait **baisser** la part GPU.*

---

## 1. Les trois mécanismes qui décident quoi dessiner

| Mécanisme | **Godot 4** | **Bevy 0.18.1** |
|---|---|---|
| **Occlusion** | `OccluderInstance3D` + **bake** manuel. Rastérisation **CPU** (Embree) dans un buffer basse résolution, BVH précalculé. Interdit de déplacer un occluder en jeu (rebuild du BVH). Cull mask pour exclure les dynamiques. Occlusion **totale** exigée pour culler. | `OcclusionCulling` sur la vue — **two-phase GPU** avec pyramide de profondeur (HZB), **zéro bake**, entièrement dynamique (un mesh skinné peut occulter). Exige un `DepthPrepass`, incompatible `DeferredPrepass`. **Expérimental** : peut culler à tort. Désactivé par défaut. |
| **LOD / HLOD** | `visibility_range_begin/end` + `_margin`, `visibility_range_fade_mode` (`Disabled` = hystérésis sèche, `Self`/`Dependencies` = alpha). Et surtout **`visibility_parent`** : les enfants se cachent seuls quand le parent LOD apparaît — HLOD déclaratif. | `VisibilityRange` (`start_margin`/`end_margin`), crossfade par dithering. **Non propagé aux enfants** : chaque entité du groupe LOD doit porter le composant. |
| **LOD de mesh auto** | **Oui**, généré à l'import (meshoptimizer), piloté par `lod_bias` / `mesh_lod_threshold`. | **Non.** Niveaux à créer soi-même. Alternative : `bevy_meshlet` (feature présente dans 0.18, opt-in). |
| **Instancing** | **Explicite** : `MultiMeshInstance3D` — N instances, 1 draw call, mais on renonce aux entités individuelles. | **Automatique** : batching par `(Handle<Mesh>, Handle<Material>)` partagés via GPU preprocessing + draw indirect. Gratuit — **et détruit en silence si on clone le matériau par instance.** |
| **Arrivée du contenu** | `ResourceLoader.load_threaded_request` + instanciation étalée **à la main**. | `AssetServer` async pour le **chargement** ; `SceneSpawner` instancie **sur le thread principal** quand l'asset est prêt. **Rien n'est fourni pour étaler.** |

**La différence structurelle.** Godot fait *déclarer* la hiérarchie de détail et
*cuire* l'occlusion : rigidité échangée contre un runtime pas cher. Bevy offre
l'instancing gratuitement mais laisse construire le LOD à la main.

Conséquence pour nous : le levier gratuit de Godot (mesh LOD auto, MultiMesh) **n'a
pas d'équivalent Bevy**, et le levier gratuit de Bevy (batching) **est un piège** si
les handles ne sont pas partagés.

---

## 2. Notre module

Le module comparable : `crates/forgia-mode-roguelite/src/decor.rs` (2 307 l., arène)
et `crates/forgia-foliage/src/lib.rs` (770 l., forêt).

### Ce qui est déjà bon

- **LOD par densité, pas par mesh** — `lib.rs:319-326` : Lod0 = 1.0, Lod1 = ×0.2 avec
  espacement Poisson recalculé `spacing / √density_factor`, Lod2 = aucun arbre.
  C'est le bon axe quand on est CPU-bound : ça réduit le **nombre d'objets**, pas
  leur coût unitaire.
- **Plafond global dur** — `lib.rs:337-341`, leçon de l'OOM à 42k arbres.
- **Handles partagés** — `trunk_meshes`/`canopy_meshes` par variante,
  `trunk_mats`/`canopy_mats` **par biome**, pas par arbre. C'est précisément la
  condition pour que le batching automatique de Bevy morde.
- **Despawn** au passage LOD2 et à l'unload de chunk, `try_despawn` idempotent.

### Ce qui manque

| | Godot | Bevy | Nous |
|---|---|---|---|
| Chargement asynchrone | `load_threaded_request` | `AssetServer` | ✅ |
| **Instanciation étalée** | à la main, obligatoire | **rien de fourni** | ❌ **absent** |
| `VisibilityRange` / HLOD | `visibility_parent` | `VisibilityRange` | ❌ **0 usage** |
| Occlusion culling | bake | expérimental | ❌ (justifié, cf §4) |

`grep` sur tout le workspace : **aucun budget d'instanciation par frame.**
Story-583 l'avait tenté ; le commentaire `lib.rs:312-314` dit pourquoi ça a été
annulé — *« régression végétation disparue, jamais validé runtime »*. Le défaut se
lit sans relire le code : **un budget qui abandonne le travail au lieu de le
reporter supprime du contenu.**

---

## 3. Les mesures — deux runs

| | Hall (08-11, 11:36) | **Arène peuplée (08-12, 11:20)** |
|---|---|---|
| meshes | 1 437 | **1 605** |
| entités | 3 753 → 4 479 | **4 408** |
| props décor | **0** | **558** |
| frame moyen | 4,38 ms (244 fps) | **7,61 ms (139,9 fps)** |
| amplitude | max 35,6 ms | **6,41 → 9,92 ms** |
| **GPU / frame** | 15 % | **8,7 %** |
| `main_opaque_pass_3d` | 0,380 ms | **0,119 ms** |
| 4 cascades d'ombre | 0,210 ms | **0,409 ms** |
| `render_cpu_sum` | 0,103 ms | 0,195 ms (2,6 %) |
| `bound_hint` | `headroom` | **`headroom`** |
| spikes 15/30/45/60 | — | **0 / 0 / 0 / 0** |

Sources : `forgia2_perf.json`, `forgia2_perf_diag.json`, `forgia2_load_timing.json`,
`forgia2_stage_decor.json` (558 props : 4 landmarks, 26 braseros, 52 murs, 65 gros
props, 34 gravats, 327 scatter).

Les passes `early_mesh_preprocessing` et `main_indirect_parameters_building`
apparaissent dans les deux profils → **le chemin draw-indirect de Bevy est actif**,
le batching automatique fonctionne.

### Ce que le second run change

**La réserve du 08-11 est levée** : le cas dense est mesuré. Et le résultat est plus
net que l'inférence depuis le Hall — en ajoutant **558 props**, la part GPU a
**baissé** (15 % → 8,7 %) et la passe opaque est devenue **trois fois moins chère**.

Note utile : les **ombres coûtent désormais 3,4× la passe opaque** (0,409 vs
0,119 ms). Si un jour on cherche du GPU, il est là — pas dans la géométrie.

### Les deux freezes n'ont pas la même cause

| t | durée | entités | cause |
|---|---|---|---|
| 22,4 s | **56 ms** | **+91** | `scene_spawn_gltf` — **arrivée** |
| 678,5 s | **43,6 ms** | 4 133 → **2 185** (−1 948) | nettoyage — **départ** |

L'arrivée : 56 ms / 91 entités = **0,62 ms l'unité**, contre 0,51 ms au Hall. Deux
runs, deux scènes, même signature — la dérivation de `DEFAULT_COST_PER_FRAME` tient.

Le départ : **non couvert par `SpawnQueue`**. Perdre 1 948 entités en une frame est
le problème miroir. Même forme de solution (différer, budgéter, ne rien perdre),
mais **c'est un second consommateur à écrire**, pas un effet de bord du premier.

### Le chiffre qui décidera du ship

7,61 ms de frame, dont 0,66 ms de GPU et 0,195 ms de rendu CPU → il reste
**~6,7 ms de logique de jeu et de physique, soit 88 % du frame**. Stable, donc
invisible à 139 fps sur la machine de dev. **À 60 fps sur une machine de joueur,
c'est le budget entier.** Hors périmètre de cet audit, mais c'est là que se joue le
ship — pas dans les 0,119 ms de la passe opaque.

### Réserves restantes

- `chunks_loaded: 0` → **le cas forêt n'est toujours pas mesuré.**
- Machine de dev. Aucune mesure sur une configuration cible faible.
- `forgia2_crash.json` n'existe pas : l'alerte `[critical] crash` du digest vient de
  `forgia2_crash.previous.json`, artefact d'un run antérieur.

---

## 4. Verdict et priorités

**1 605 meshes en 0,119 ms, GPU à 8,7 %, zéro spike en régime permanent.**
L'occlusion culling et le LOD de mesh optimiseraient une passe déjà quasi gratuite.
Ce qui casse l'immersion, ce sont **deux freezes de 56 et 44 ms** — 3 frames
sautées, visibles manette en main.

| # | Action | Justification | Risque |
|---|---|---|---|
| ~~1~~ | ~~**File d'instanciation budgétée**~~ — **DÉJÀ FAIT depuis juin, cf §4 bis** | — | — |
| **1** | **Budget de DÉPART** (le seul volet non couvert) | 1 948 entités perdues en une frame = 43,6 ms. `DecorSpawnQueue` (story-626) ne traite que l'arrivée. | **Masquer à la mise en file** (`Visibility::Hidden` + collider off) : à 200/frame le pic met 10 frames à disparaître, donc différer sans masquer laisserait des fantômes en scène. |
| **2** | Vérifier le partage des `Handle<Scene>` dans `decor.rs` | Deux props du même GLB doivent partager le handle. Si `material_override` clone un `StandardMaterial` par instance, le batch saute **en silence**. | Faible — c'est une vérification. |
| **3** | `VisibilityRange` sur les props d'arène | Pas pour dessiner moins : pour **collapser plusieurs meshes en un seul** à distance (doc Bevy : *« reducing the number of meshes… useful for reducing drawcall count »*). Piège : non propagé aux enfants → système post-instanciation sur les `SceneRoot`. | Moyen. **Déclassé** par le run du 08-12 : la passe opaque a baissé en ajoutant des props. |
| **4** | **Occlusion culling : pas maintenant.** | Expérimental, peut culler à tort, exige `DepthPrepass`, et on a **91 %** de marge GPU. | — |
| **5** | *(hors périmètre, mais nommé)* Les **6,7 ms de logique/physique** | 88 % du frame. Invisible à 139 fps, fatal à 60 fps sur machine cible. | C'est le vrai sujet de ship. |

**Critère de réouverture du point 4** : un capteur qui rapporte
`bound_hint: "gpu"`. Tant que `forgia2_perf.json` dit `"headroom"`, toute
optimisation de rendu est spéculative au sens de `no-speculative-fix.md`.

---

## 4 bis. Correction — l'étalement de l'arrivée existait déjà

**Découvert le 2026-08-12, après coup.** [story-626](../stories/story-626-roguelite-etalement-spawn-decor.md),
CODE-COMPLETE depuis le **2026-06-25**, avait déjà livré exactement la
recommandation n°1 de la version initiale de cet audit : séparation plan/drain,
`plan_decor_set` (RNG seul, aucune instanciation) → `DecorSpawnQueue` (Resource,
file + curseur) drainée par frame. C'est **vivant dans le code**
(`forgia-mode-roguelite/src/decor.rs:736`).

### Comment l'audit est passé à côté

Le `grep` cherchait `budget`, `per_frame`, `spawn_budget` — le vocabulaire de la
solution que j'allais écrire. Le code existant dit **étalement, plan, drain,
queue**. C'est mot pour mot l'anti-pattern n°1 de `concept-first.md` :
*« grepper un nom de type au lieu du mot-concept »*. La règle existait, elle n'a
pas été appliquée avant d'écrire.

### Ce que ça change aux conclusions

- Le freeze que 626 corrigeait était **65 ms pour +797 entités**. Ce qui restait au
  11-08 — **56 ms pour +91** — est un résidu bien plus petit, et la run du 12-08 à
  12:31 n'a **aucun** freeze `scene_spawn_gltf` : deux freezes seulement, tous deux
  `cause=unattributed_cpu_or_gpu`, à **−3 et +0 entités**.
- Donc « le levier, c'est l'étalement de l'arrivée » était **en retard d'un fix**.
  Le volet réellement ouvert est le **départ** (1 948 entités en une frame), que
  626 ne traite pas.
- `crates/forgia-streaming/src/entity_budget.rs` (écrit le 11-08, 18 tests) est
  **redondant sur sa moitié arrivée**. Il n'a aucun consommateur — le garder tel
  quel violerait `fine-grained-crates.md` (« interdit : créer pour plus tard sans
  consommateur »). Décision : **non commité tant qu'il n'a pas de consommateur** ;
  le câblage du budget de départ sur le nettoyage d'arène est ce qui le justifiera.

### La leçon, plus large que ce fichier

Un audit qui recommande d'écrire quelque chose doit d'abord chercher **le concept**,
pas le nom qu'il donnerait à sa propre solution. Ici, une requête `grepai` en
anglais sur `spread decor spawning across frames` aurait rendu `decor.rs` du
premier coup.

---

## 5. Cross-refs

- `.claude/rules/scalability.md` — « LOD distance-based pour tout ce qui a un mesh
  visible » : **cette règle n'est pas tenue** (0 `VisibilityRange`), mais les
  mesures disent qu'elle n'est pas la priorité. À re-arbitrer, pas à appliquer
  mécaniquement.
- `.claude/rules/no-speculative-fix.md` — pourquoi on ne touche pas au rendu.
- `.claude/rules/observability-required.md` — `load_timing.json` est la référence
  du point 1.
- `reference_vegetation_density_genome_et_plafond` (memory) — la leçon OOM.
- **`forgia-mode-roguelite/src/decor.rs:736`** — `DecorSpawnQueue`, l'étalement de
  l'arrivée, livré par story-626 en juin. **À lire avant toute reprise du sujet.**
- `.claude/rules/concept-first.md` §4 — l'anti-pattern qui a coûté cet aller-retour
  (cf §4 bis).
- `crates/forgia-streaming/src/entity_budget.rs` — **non commité** (cf §4 bis).

---

*Sources moteur lues directement : `bevy_camera-0.18.1/src/visibility/range.rs`,
`bevy_render-0.18.1/src/experimental/occlusion_culling/mod.rs`,
`bevy_render-0.18.1/src/batching/gpu_preprocessing.rs` ; docs Godot 4
`tutorials/3d/occlusion_culling` et `tutorials/3d/visibility_ranges`.*
