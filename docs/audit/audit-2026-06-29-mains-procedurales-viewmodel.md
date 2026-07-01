# Audit — Maximiser le réalisme STYLISÉ des mains procédurales du viewmodel FPS

> **Date** : 2026-06-29
> **Demande** : « Améliorer au maximum la génération procédurale des mains, rendu plus réaliste
> mais toujours cartoon, SANS importer d'assets. Grosse audite internet. »
> **Scope** : mains du **viewmodel FPS** (les seules mains procédurales du code — [arms.rs](../../crates/forgia-viewmodel/src/arms.rs)).
> **Méthode** : recherche multi-sources vérifiée adversarialement (deep-research, 103 agents,
> 21 sources, 88 claims, 8 confirmés / 17 tués) + **cross-check des claims API Bevy 0.18 contre le code réel du repo**
> (la vérif web a été rate-limitée sur les points Bevy → re-validés ici par le code existant).

---

## 0. État actuel (point de départ)

[arms.rs](../../crates/forgia-viewmodel/src/arms.rs) assemble des **primitives Bevy disjointes** :
`Cuboid` (phalanges), `Capsule3d` (avant-bras), `Cylinder` (manche), `Sphere` (poignet, dos de main,
jointures). 4 doigts à 2 phalanges + pouce. Matériau `StandardMaterial` PBR (3 styles Peau/Gantelet/Cyber)
+ texture peau procédurale (value-noise + grain). L'auteur a déjà documenté le plafond assumé
([arms.rs:14](../../crates/forgia-viewmodel/src/arms.rs#L14)) : *« procédural = stylisé, réalisme = mesh riggé (asset). »*
**Cet audit montre comment repousser ce plafond sans asset.**

Deux causes de l'aspect « plastique/blocky » :
1. **Géométrie** = coquilles séparées qui se chevauchent → coutures, facettes, pas de continuité de peau.
2. **Shading** = PBR neutre → la peau ne « lit » pas comme de la chair (pas de translucidité, pas de rim).

---

## 1. Leviers hiérarchisés par ratio gain visuel / effort

### 🥇 LEVIER A — Shading peau stylisé (faux-SSS + rim) — *meilleur 1er pas*

**Technique (source primaire Riot/VALORANT, vote 3-0 sur les deux mécanismes) :**
- **Faux subsurface scattering** : biaiser le *diffuse falloff* vers le **rouge dans les zones sombres**
  (« more red in darker areas ») + un terme **« underglow » à réponse specular** qui **s'intensifie dans
  les régions en pénombre, loin de la lumière directe**. C'est ce qui fait que la peau lit comme de la chair
  et plus comme du plastique. VALORANT masque par **vertex color** pour éviter une texture → compatible « sans asset ».
- **Rim / Fresnel coloré** sur les angles rasants (priorité aux angles montants/régions hautes) → **détache la main
  du fond et donne du volume** (« silhouette effect »), modulé pour ne pas être trop brillant.

**Pourquoi en 1er** : s'applique sur la **géométrie ACTUELLE** (juste un swap de matériau), zéro refonte mesh
→ gros gain de lisibilité immédiat, dé-risque le « look » avant de toucher la géométrie.

**Faisabilité Bevy 0.18 (validée sur le code repo)** : pattern Material/`AsBindGroup` custom **déjà éprouvé**
dans le repo ([toon.rs](../../crates/forgia-postprocess/src/toon.rs), [outline.rs](../../crates/forgia-postprocess/src/outline.rs),
[mesh_fader.rs](../../crates/forgia-effects/src/mesh_fader.rs), nameplate). Le viewmodel est **déjà isolé**
(layer 1 + lumière dédiée 9000 lux, [vm_camera.rs](../../crates/forgia-viewmodel/src/vm_camera.rs)) → un matériau
peau custom s'applique proprement **sans toucher le monde**. Voie probable : `ExtendedMaterial<StandardMaterial, SkinExt>`
(API exacte à confirmer sur docs.rs Bevy 0.18, mais le pattern Material custom est prouvé en interne).

**Coût perf** : shader par-pixel sur 2 mains = négligeable.

**Pièges sourcés** : la formule de faux-SSS d'Alan Zucconi (`V·-⟨L+N·δ⟩`) a été **RÉFUTÉE** (vote 1-2) et la
« Gradient Lambert ramp » de VALORANT n'a eu que 1-0 (non confirmée) → **s'ancrer uniquement sur les 2 mécanismes
VALORANT confirmés ci-dessus**, pas sur ces deux-là.

---

### 🥈 LEVIER B — Géométrie organique : un seul mesh (SDF smooth-min → Surface Nets) — *plus gros gain visuel absolu, plus gros effort*

**Technique (source primaire canonique Inigo Quilez, vote 3-0) :**
- Construire la main comme un **SDF de capsules + sphères** (phalanges = capsules, paume = rounded box,
  poignet/jointures = sphères) **fusionnées au smooth-minimum** (`opSmoothUnion`). Le smooth-union blende les
  primitives en **formes organiques SANS les coutures** des booléens durs.
- Le paramètre **`k` = largeur de la zone de blend en unités de distance** (un seul réglage intuitif).
  **PIÈGE** : ne pas omettre `k *= 4.0` dans `opSmoothUnion` sinon `k` ne correspond plus à la largeur (décalage ×4).
- Capsule, sphère, rounded box, cylindre sont des **SDF exacts** → fiables.
- Mailler le SDF avec **Surface Nets** (préférable à Marching Cubes pour l'organique : lisse les coins itérativement,
  mesh plus lisse, vote 2-0 + papier académique PMC).

**Pourquoi** : tue d'un coup l'aspect « primitives collées ». Une vraie peau continue sur tout le poing.

**Faisabilité Bevy 0.18 (validée sur le code repo)** :
- `fast-surface-nets 0.2` est **déjà une dép** (utilisée pour le terrain). Sa `SurfaceNetsBuffer` produit
  des **normales lisses dérivées du gradient SDF** → casse le facetté gratuitement *(à confirmer empiriquement
  sur un SDF de main ; sinon `compute_smooth_normals` manuel)*.
- Construction de Mesh custom **prouvée en interne** : `Mesh::new(PrimitiveTopology::TriangleList, …)` +
  `insert_attribute(POSITION/NORMAL/UV_0/COLOR)` + `insert_indices` ([road_mesh.rs:83](../../crates/forgia-worldgen/src/road_mesh.rs#L83), [house_mesh.rs:103](../../crates/forgia-worldgen/src/house_mesh.rs#L103)).

**Coût perf** : meshing **one-time au spawn** (comme un chunk terrain), pas un hot path. Grille ~32³–48³ pour une main = bon marché. Plus de tris qu'en primitives, mais viewmodel = 1 instance × 2 mains → non significatif.

**Alternative plus légère (vote 3-0)** : box-modeling (extrusion de faces) + subdivision **Catmull-Clark**.
Mais nécessite d'**autorer une cage quad connectée** → moins naturel ici que le SDF. (NB : subdiviser des primitives
*disjointes* ne soude rien, ça ne fait qu'arrondir chaque coquille — sans intérêt.)

**Piège sourcé** : la « surface de révolution » (spline 2D tournée) pour des doigts effilés a été **RÉFUTÉE** (vote 0-3)
→ ne pas partir là-dessus.

---

### 🥉 LEVIER C — Détail procédural & AO de creux — *polish, faible rendement seul*

**Technique** :
- **AO en vertex color** baké à la génération (assombrir les creux : entre les doigts, plis de jointures, base de paume).
  Pattern `ATTRIBUTE_COLOR` **déjà utilisé** ([house_mesh.rs:106](../../crates/forgia-worldgen/src/house_mesh.rs#L106), terrain).
- **Cavity darkening piloté par la courbure du SDF** : la concavité du champ = AO, **gratuit** (tu as déjà le champ après le Levier B).
- **Normal map procédurale** (bump bruité runtime) pour micro-relief peau (pores/tendons) — *optionnel, dernier*.

**Pourquoi en dernier** : l'AO dans les creux vend le volume à bas coût, mais **après** que la géométrie + le shading
soient en place (sinon on polit du plastique). N'a de sens qu'avec un mesh custom (le Levier B) — les primitives Bevy ne
portent pas de vertex color custom.

**Coût perf** : génération, gratuit au runtime.

**⚠️ Limite de l'audit** : aucune source confirmée ne couvre les **proportions anatomiques précises** d'une main, les
**ongles procéduraux**, ni le **detail triplanaire / curvature-AO** spécifiques. Ces points relèvent du jugement
d'ingénierie (ci-dessous), pas de l'audit web.

---

## 2. Proportions anatomiques (hors audit — jugement d'ingénierie, à valider visuellement)

Le `FINGER_PROFILE = [0.88, 1.0, 0.95, 0.78]` actuel est déjà raisonnable (majeur le plus long). Repères pour le mesh :
- Longueur **doigt ≈ longueur paume** ; paume légèrement plus large que haute.
- Phalange proximale ≈ 2× la distale (ratio déjà ~26/22 dans le code).
- Pouce opposé, base reculée vers le poignet, ~45–55° d'écart du plan des doigts.
- Poing fermé : doigts enroulés ~90° à la proximale, dos de main bombé, jointures saillantes.

---

## 3. Recommandation — ordre d'implémentation

| Étape | Levier | Gain | Effort | Risque | Touche |
|---|---|---|---|---|---|
| **1** | A (shader faux-SSS + rim) sur géométrie actuelle | 🔥 Élevé | Moyen | Faible | nouveau material + arms.rs (swap mat) |
| **2** | B (SDF smooth-min → Surface Nets, 1 mesh) | 🔥🔥 Max | Élevé | Moyen | refonte `spawn_hand`/`spawn_finger` + module SDF |
| **3** | C (AO vertex color courbure + normal procédural) | Moyen | Faible | Faible | sur le mesh de l'étape 2 |

> **NB ordre vs audit** : l'audit recommandait géométrie-d'abord (gain visuel max). Je **réordonne** : shader-d'abord,
> car il s'applique sur la géométrie existante → 1er incrément à faible risque qui dé-risque le « look » avant la grosse
> refonte mesh. Le shader de l'étape 1 est réutilisé tel quel en étape 2.

**Garde-fous projet à respecter à l'implémentation** :
- **No-hardcode** : tous les réglages (`k` smooth-min, intensité du rouge SSS, couleur/puissance rim, force AO) →
  `FpsTuning`/genome **hot-reload**, pas de literal. Cf [no-hardcode.md](../../.claude/rules/no-hardcode.md).
- **Observability** : capteur `forgia2_viewmodel_hands.json` (mode mesh actif, tris, k, flags shader). Cf rule observability.
- **BMAD** : multi-fichiers + ≥2 implémentations (mesh + shader) → **Standard** (story requise), pas Quick.
- **Crate** : le SDF/mesh de main appartient à `forgia-viewmodel` (1 seul consommateur) → **module local**, pas une crate
  ([fine-grained-crates.md](../../.claude/rules/fine-grained-crates.md)).

---

## 4. Incertitudes & points à re-vérifier avant code

1. **API exacte `ExtendedMaterial<StandardMaterial>` en Bevy 0.18** : pattern Material custom prouvé en interne, mais la
   forme précise (slots de binding, WGSL d'extension PBR) est à confirmer sur docs.rs/bevy 0.18 + exemples du repo.
2. **Normales de `fast-surface-nets`** : confirmer empiriquement que le mesh sort déjà lisse, sinon recalcul manuel.
3. **Cohérence visuelle viewmodel vs monde** : le monde est toon-shadé, le viewmodel **non** (exclu volontairement).
   Décider : PBR+SSS stylisé (recommandé) **ou** cel-ramp pour matcher le monde. `bevy_toon_shader` ne supporte que
   ≤ 0.12 → à réimplémenter si on veut du cel (non confirmé par l'audit).

---

## 5. Sources confirmées (vote adversarial ≥ 2-0)

- **Géométrie SDF** — Inigo Quilez, [3D SDF / distfunctions](https://iquilezles.org/articles/distfunctions/) +
  [smin](https://iquilezles.org/articles/smin/) *(primaire, canonique)*.
- **Surface Nets > Marching Cubes** — [cerbion.net](https://cerbion.net/blog/understanding-surface-nets/) +
  papier [PMC9623606](https://pmc.ncbi.nlm.nih.gov/articles/PMC9623606).
- **SDF humanoïde meshé (marching cubes)** — [ctrl-alt-test 64kB intro](https://www.ctrl-alt-test.fr/2023/procedural-3d-mesh-generation-in-a-64kb-intro/) *(vote 2-1)*.
- **Box-modeling + Catmull-Clark** — ctrl-alt-test + [Blender docs](https://download.blender.org/documentation/htmlI/ch11s05.html).
- **Faux-SSS stylisé + rim/Fresnel** — [Riot/VALORANT shaders & clarity](https://www.riotgames.com/en/news/valorant-shaders-and-gameplay-clarity) *(primaire)* + [3D Game Shaders for Beginners](https://lettier.github.io/3d-game-shaders-for-beginners/).

**Réfutés / non confirmés (à NE PAS citer comme primaires)** : Zucconi fast-SSS `V·-L` (1-2), surface de révolution
pour doigts (0-3), VALORANT « Gradient Lambert » ramp (1-0), GDC 2011 Barre-Brisebois & GPU Gems skin (0-0, abstentions rate-limit).
