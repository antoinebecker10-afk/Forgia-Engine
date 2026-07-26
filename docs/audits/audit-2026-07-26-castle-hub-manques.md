# Audit — ce qui manque dans le Hall de Forgia (2026-07-26)

**Demande** : « pourquoi les flammes sur les bougies ne sont pas là, pourquoi les
éléments à l'intérieur du château ne sont pas tous là, certaines textures notamment
au plafond ne sont pas présentes ».

**Méthode** : audit du **contenu exporté** (les `.gltf` sont du JSON, donc
directement analysables) croisé avec le **capteur runtime**. Aucune conclusion
tirée du rendu à l'œil.

---

## 0. Résumé

| # | Constat | Gravité | Cause |
|---|---|---|---|
| 1 | **Aucune flamme n'existe dans l'export** | 🔴 | Reconstruction limitée aux meshes : particules et lumières Unity perdues |
| 2 | **Zéro lumière locale dans le Hall** | 🔴 | 2 directionnelles + ambiante 900, aucune ponctuelle |
| 3 | **100 % des meshes à normal map n'ont pas de tangentes** (1138/1138) | 🔴 | Export des cellules sans calcul Mikktspace |
| 4 | 5 matériaux sans carte métal/rugosité, dont le plafond | 🟠 | Jeux de textures incomplets à la source |

**Rien n'est perdu au runtime** : le capteur mesure 46/46 cellules chargées et
8 560 meshes. Tout ce qui a été exporté est bien à l'écran. Les manques sont
**en amont**, à l'export.

---

## 1. Ce qui EST là (mesuré)

`castle_stream_cells_grass/` — 46 cellules, **7 911 nœuds**, 171 types distincts,
1 321 primitives, 28 matériaux, 61 textures (toutes présentes sur disque, aucune
URI cassée).

Répartition : `MOD` (architecture) 6 753 · `PROP` (mobilier) 700 · `ENV`
(végétation/falaises) 458. Niveau de détail : LOD0 partout (7 895 nœuds ; 16
nœuds `SM_PROP_coins_castle_01/02` sans suffixe LOD).

Intérieur présent, contrairement à l'impression : **~283 pièces de plafond**
(`ceiling_plaster_curved/flat/round/sloped`, `deco_ceiling_cap`), **98 bougies**
et **203 bougeoirs**, 65 chaises, 68 tables, 33 rideaux, 22 tableaux, 84 tapis,
13 statues, le trône, la cloche, 27 livres, 16 pièces d'or, 44 vases.

**Capteur runtime** (`forgia2_castle_hub.json`) : `streamed_cells: 46`,
`stream_plan_cells: 46`, `meshes: 8560`, `descendants: 16517`. Le streaming
charge donc l'intégralité du plan.

---

## 2. 🔴 Les flammes n'ont jamais été exportées

**Mesure** : recherche des noms de nœuds contenant `candle|flame|fire|torch|light|
lamp|particle|fx|smoke|glow` sur les 46 cellules → **les bougies et bougeoirs
ressortent, aucune flamme, aucune lumière, aucun système de particules**.

**Confirmation croisée** : sur les 28 matériaux, **aucun n'a de carte émissive ni
de `emissiveFactor` non nul**. Même si une géométrie de flamme existait, rien ne
la ferait briller.

**Cause** : dans un pack Unity, une flamme de bougie est un `ParticleSystem`
(ou un quad additif + `Light`), pas un `MeshRenderer`. La reconstruction de la
scène n'a conservé que les objets porteurs de mesh — tout le reste (particules,
lumières, sons, volumes) a été écarté silencieusement.

**Conséquence** : les bougeoirs sont posés partout, mèche nue. C'est cohérent avec
le constat visuel.

---

## 3. 🔴 Aucune lumière locale dans le Hall

`castle_hub.rs` installe **2 `DirectionalLight`** (20 000 et 7 000 lux) et une
**`AmbientLight` à 900**. Aucune `PointLight`, aucune `SpotLight`.

**Conséquence directe sur le symptôme « textures manquantes au plafond »** : un
plafond en retrait ne reçoit pas le soleil directionnel ; il est donc éclairé
uniquement par une ambiante uniforme. Une ambiante n'a **pas de direction** : elle
donne exactement la même valeur à chaque point de la surface. Résultat, une
surface plate en plâtre clair devient un aplat beige sans relief ni variation —
ce qui se lit très exactement comme « la texture n'est pas là ».

L'historique du fichier montre que le réglage a déjà oscillé (700 → 1600 → 900)
en essayant de compenser au fill ambiant l'absence de sources locales.

---

## 4. 🔴 Aucune tangente : les normal maps sont inertes

**Mesure** : sur 1 321 primitives, **1 138 ont un matériau avec `normalTexture`**,
et **1 138 sur 1 138 n'ont pas d'attribut `TANGENT`** — soit **100 %**.

En revanche `TEXCOORD_0` et `NORMAL` sont présents partout (0 manquant) : les UV
et les normales de sommet vont bien. C'est **uniquement** la tangente qui manque.

**Conséquence** : Bevy ne peut pas construire le repère tangent nécessaire au
placage de normales. Tout le micro-relief disparaît — joints de pierre, grain du
plâtre, veines du bois, moulures. Sur les petites pièces c'est discret ; sur les
**grandes surfaces planes — plafonds et murs — c'est exactement l'aspect « texture
absente »**, parce qu'il ne reste que la couleur de base aplatie.

**L'outil de correction existe déjà** : `tools/blender/reexport_glb_with_tangents.py`
(« l'export glTF de Blender calcule les tangentes Mikktspace… Cela déplace ce coût
hors du runtime Bevy »). Il n'a simplement jamais été passé sur ces 46 cellules —
et il attend du `.glb` alors que les cellules sont en `.gltf` + `.bin`.

---

## 5. 🟠 Matériaux à jeu de cartes incomplet

Sur les 28 matériaux, un seul n'a pas de couleur de base : `M_CliffGrassVC`, et
c'est **volontaire** (falaise peinte en couleurs de sommets).

En revanche 5 matériaux n'ont pas de carte métal/rugosité, dont deux qui comptent
pour l'intérieur :

| Matériau | Cartes | Manque |
|---|---|---|
| `M_MOD_ceiling_plaster_castle` | BC + N | **métal/rugosité** |
| `M_MOD_roof_base_castle` | BC + N | métal/rugosité |
| `M_PROP_carpet_castle` | BC | N + métal/rugosité |
| `M_PROP_flag_castle` | BC | N + métal/rugosité |
| `M_MOD_glass_castle` | BC (masque) | N + métal/rugosité |

Sans carte de rugosité, Bevy applique une valeur unique sur toute la surface : le
plafond réagit à la lumière de façon parfaitement homogène. Cumulé au §3 (pas de
lumière locale) et au §4 (pas de relief), ça fait trois raisons convergentes pour
que le plafond paraisse nu.

Note : 9 fichiers de texture du dossier n'ont pas de couleur de base associée
(`T_MOD_wall_plaster_castle_01`, `T_MOD_wall_blocks_castle_01`, …), mais les
matériaux vont chercher la couleur ailleurs (`_02_BC`, `wall_bricks_BC`). Ce sont
des variantes non utilisées, pas un manque.

---

## 6. Limite de cet audit

Je peux affirmer que **tout ce qui a été exporté est chargé** (46/46 cellules) et
énumérer précisément ce que contient l'export. Je **ne peux pas** dresser la liste
des meshes présents dans le pack Unity d'origine et absents de l'export : le pack
source et le script de reconstruction ne sont pas dans ce dépôt.

Pour fermer ce point : indiquer le chemin du `.unitypackage` (ou du dossier
extrait) et je produis le diff nom par nom.

---

## 7. Correctifs proposés, par rapport gain/effort

> **Correctif A appliqué le 2026-07-26** — voir §8.

| # | Correctif | Effet | Effort | Risque |
|---|---|---|---|---|
| A | ~~**Recalculer les tangentes** des 46 cellules~~ | ✅ **FAIT** (§8) | — | — |
| B | **Flammes procédurales** sur les 203 bougeoirs : petit quad émissif + `PointLight` à portée courte, posé par script sur les nœuds `candleholder` | Rend les bougies vivantes ET éclaire l'intérieur là où il faut | ~2 h | Perf : 203 lumières = trop. À plafonner (les N plus proches du joueur) |
| C | **Baisser l'ambiante** une fois B en place | Rend du contraste et de la lecture de volume à l'intérieur | 15 min | Faible, réglage |
| D | Compléter les cartes métal/rugosité manquantes | Variation de brillance du plafond | ~1 h | Faible |
| E | Diff avec le pack source | Chiffre exact des meshes perdus | 30 min | Nul (lecture) |

Ordre recommandé : **A → E → B → C → D**. A est le seul qui corrige un défaut
*catégorique* (100 % des surfaces concernées) avec un risque quasi nul, et E dit
s'il reste un vrai trou de géométrie avant d'investir dans B.

---

## 8. Correctif A — appliqué (2026-07-26)

**Outil livré** : `tools/gltf/add_tangents.py`. Il **injecte** l'attribut `TANGENT`
dans les `.gltf`/`.bin` existants au lieu de repasser la scène par Blender.

Raison de ce choix : un aller-retour Blender recalculerait bien des tangentes
Mikktspace, mais referait une conversion de repère sur toute la scène — exactement
là où ce pipeline a déjà déraillé (miroirs invisibles, frames décalés). Et les
clés de persistance de l'éditeur de scène (`<scène>#<index de fratrie>:<nœud>`)
dépendent des noms **et de l'ordre** des nœuds : un réexport les invaliderait.

**Résultat mesuré** :

| Contrôle | Avant | Après |
|---|---|---|
| Primitives à normal map sans tangente | 1138 / 1138 (100 %) | **0** |
| Nœuds, noms, ordre de fratrie | — | **identiques** (diff vs `HEAD`) |
| Transformations des nœuds | — | **identiques** |
| Matériaux / textures / images | 28 / 61 | **inchangés** |
| Poids du dossier | 122 Mo | 145 Mo (+20,6 Mo) |

**Validation numérique** (échantillon de 120 sommets sur 40 primitives) : `|T| = 1`,
`w = ±1`, `T·N ≈ 0` (orthogonalité Gram-Schmidt). Cohérence `buffers[0].byteLength`
↔ taille réelle du `.bin` vérifiée sur les 46 fichiers. Zéro erreur.

**Base tangente** : accumulation par triangle + Gram-Schmidt (Lengyel), pas
Mikktspace strict. Les deux ne diffèrent qu'aux coutures d'UV, de façon
imperceptible sur de la pierre et du plâtre.

**Portée** : terrain et végétation n'utilisent pas de normal map, ils n'étaient pas
concernés. `castle_highlands.glb` (GLB source monolithique, plus chargé au runtime
depuis le passage aux cellules) porte encore le défaut : **repasser l'outil dessus
si un nouveau bake de cellules est fait**, sinon la régression revient.

**Aucun rebuild nécessaire** : ce sont des assets, pas du code. Un relancement du
jeu suffit.
