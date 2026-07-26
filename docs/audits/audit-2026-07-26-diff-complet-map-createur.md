# Audit complet — notre Hall vs la map du créateur (2026-07-26)

Comparaison **instance par instance** de `demoscene_highlands_castle_level.unity`
(8 102 instances) avec nos 46 cellules glTF (7 911 nœuds).

## Méthode et niveau de confiance

Positions monde résolues des deux côtés, puis converties par la transformation
**déduite et vérifiée au millimètre** :

```
bevy = ( −unity.x − 32,041 ,  unity.y − 172,671 ,  unity.z + 35,372 )
```

Sur 8 102 instances de sa scène, **7 857 (97 %) ont une position résolue de façon
fiable**. Les 245 restantes sont imbriquées dans d'autres prefabs (transforms
« stripped » sans position dans le fichier) : elles sont **écartées de l'analyse**
plutôt que comptées comme des défauts.

Appariement par plus proche voisin **sans retrait, dans les deux sens** — sinon un
appariement glouton confond deux tours identiques et invente des écarts de 250 m.

---

## 1. Bilan chiffré

| Verdict | Instances |
|---|---|
| ✅ Placées **exactement** (< 5 cm) | **7 439** |
| ⚠️ **Mal placées** (position différente, confirmé dans les deux sens) | **228** |
| ❌ **Absentes** | **191** |
| ⚪ Non résolues (limite de l'analyse, pas un défaut) | 245 |

---

## 2. ⚠️ Les assets mal placés : un décalage purement VERTICAL

C'est la découverte la plus nette. Sur les pièces concernées, les écarts sont
**purement en Y — X et Z sont exactement à 0,00 m**.

Décalages les plus fréquents (nous − lui) sur `P_MOD_tower_part_arch_castle` :

| Décalage | Occurrences |
|---|---|
| `(0, −19,50, 0)` | 16 |
| `(0, −21,96, 0)` | 8 |
| `(0, +12,24, 0)` | 8 |
| `(0, −4,51, 0)` | 8 |
| `(−14,0, −2,76, −12,0)` | 8 |

Et ça touche massivement les **tours** :

| Pièce | Total | Mal placées | Écart médian |
|---|---|---|---|
| `tower_part_arch` | 88 | **48** | 19,5 m |
| `tower_segment_wall` | 42 | **30** | 2,9 m |
| `tower_segment_window` | 25 | **18** | 7,5 m |
| `tower_roof` | 12 | 8 | 21,3 m |
| `tower_segment_top_deco` | 12 | 8 | 21,5 m |
| `tower_segment_ceiling` | 10 | 6 | 21,9 m |
| `tower_roof_deco` | 10 | 5 | 29,3 m |
| `tower_segment_wall_deco` | 10 | 5 | 25,6 m |
| `floor_diagonal_quarter` | 26 | **20** | 3,6 m |
| murs / colonnes / rambardes divers | — | ~49 | ~3,0–3,7 m |

**Lecture** : les segments de tour sont empilés verticalement. Même colonne (X/Z
identiques), **mauvaise hauteur dans la pile**. Le nombre de pièces est pourtant
correct des deux côtés — ce ne sont pas des pièces perdues, ce sont des pièces
posées au mauvais étage.

Le test symétrique le confirme : 48 des siennes n'ont aucun équivalent chez nous,
et 48 des nôtres n'en ont aucun chez lui. Les hypothèses de miroir, de rotation
90/180° et de décalage global ont toutes été testées et **écartées** (0/48).

---

## 3. 🕯️ Les bougies : le tableau exact

| Prefab | Lui | Nous |
|---|---|---|
| `candle_01` / `02` / `03` | 14 / 3 / 5 | 14 / 3 / 5 ✅ |
| `candle_04` | 25 | 25 ✅ |
| **`candle_04_lit`** | **11** | **0** ❌ |
| `candle_05` | 51 | 51 ✅ |
| **`candle_05_lit`** | **67** | **0** ❌ |
| `candleholder_01` … `08` | 203 | 203 ✅ |
| **TOTAL** | **379** | **301** |

### Ce que ça veut dire, précisément

Le pack distingue deux prefabs par bougie : la version **éteinte** et la version
**`_lit`**. Sa salle contient **78 bougies allumées** et 301 éteintes.

Nous n'avons que les 301 éteintes. Et **j'ai posé une flamme sur les 301**.

**C'est exactement l'inverse de sa scène** : les 301 qu'il laisse éteintes sont
allumées chez nous, et les 78 qui brillent chez lui n'existent pas du tout — leur
position comprise.

### Comment il place ses flammes

J'ai ouvert les 5 prefabs `_lit`. Chacun contient :

- **3 MeshRenderers** — donc la flamme est un **mesh** (`SM_FX_plane_fire_castle`
  + shader `S_fire_URP`), **pas des particules**. Elle aurait donc été importable
  par une reconstruction limitée aux meshes : c'est bien la variante `_lit` entière
  qui a été écartée, pas la flamme en tant que telle.
- **1 lumière ponctuelle enfant**, à une hauteur **propre à chaque type de bougie** :

| Prefab | Hauteur de la flamme | Lumière |
|---|---|---|
| `candle_01_lit` | 0,337 m | intensité 0,7 · portée **2,45 m** |
| `candle_02_lit` | 0,232 m | idem |
| `candle_03_lit` | 0,138 m | idem |
| `candle_04_lit` | 0,395 m | idem |
| `candle_05_lit` | 0,360 m | idem |

Couleur identique partout : `(1,0 · 0,846 · 0,288)` — un orange franc.

**C'est authoré par type, pas calculé.** Ma flamme, elle, est posée au sommet de
la boîte englobante du nœud : ça donne la bonne hauteur sur une bougie simple,
mais sur un **chandelier à 5 branches** (`candleholder_03`, 95 exemplaires) ça
pose **une seule flamme au centre du sommet**, là où il en faudrait cinq, une par
bougeoir.

### Et l'intensité

| | Lui | Moi |
|---|---|---|
| Bougies allumées | 78, **toutes** allumées en permanence | 24 les plus proches |
| Portée | **2,45 m** | 7 m |
| Intensité | 0,7 (unités Unity) | 2 200 lm |

Ses bougies éclairent **à peine au-delà d'elles-mêmes**. Les miennes portent trois
fois plus loin et bien plus fort — d'où un intérieur qui « bave » au lieu d'avoir
des points lumineux nets.

---

## 4. 💡 Son éclairage complet, maintenant qu'on a tout

| Source | Nombre |
|---|---|
| Lumières placées à la main | **57** (43 point, 13 spot, 1 directionnelle) |
| Lumières de bougies allumées | **78** (0,7 · 2,45 m) |
| **Total** | **135** |

Chez nous après les correctifs : 56 portées + 24 automatiques = 80, avec des
paramètres différents et sur les mauvaises bougies.

---

## 5. ❌ Les 191 absentes, par nature

| Instances | Prefab | Nature |
|---|---|---|
| 67 + 11 | `candle_05_lit`, `candle_04_lit` | **bougies allumées** |
| 42 + 8 | `flag_02_static`, `flag_03_static` | **bannières murales** |
| 36 | `fog_castle` | brume |
| 16 | `particles_castle` | poussières |
| 6 / 2 / 1 | feuilles, vent, rai de lumière | FX |
| 2 | `door_huge_comp_castle` | grande porte |

Toutes portent un suffixe `_lit`, `_static` ou `_comp` : ce sont des **prefabs
composites ou variantes**, que la reconstruction — limitée aux prefabs à
correspondance 1:1 avec un FBX — a écartés en silence.

---

## 6. Plan corrigé, par gain réel

| # | Action | Pourquoi maintenant | Effort |
|---|---|---|---|
| ~~**L**~~ | ~~Caler mes flammes sur les siennes~~ | ✅ **FAIT** (§7) | — |
| **I** | **Réimporter les 191 instances absentes** (78 bougies allumées, 50 bannières, 61 FX, 2 portes) — leurs positions sont extraites et vérifiées | Rétablit le vrai placement des flammes et les bannières | ~4 h |
| **M** | **Corriger les 228 pièces mal placées** — décalage vertical des tours | Défaut de géométrie réel, chiffré | ~3 h |
| **K** | 31 sondes de réflexion | Enlève l'aspect « pierre mouillée » | ~3 h |
| **F** | 11 lightmaps | Le rebond coloré ; écart structurel restant | 1-2 j |

**L d'abord** : c'est le seul qui corrige un défaut que tu vois à l'écran *tout de
suite*, sans dépendre du re-bake des prefabs composites.


---

## 7. Correctif L — appliqué (2026-07-26)

### La cause était plus simple que prévu

Je cherchais comment répartir plusieurs flammes sur un chandelier à cinq branches.
L'audit a montré qu'il n'y a rien à répartir : **dans ce pack, les bougies sont des
objets séparés des bougeoirs**. Sa scène compte 176 bougies posées sur 203
bougeoirs en métal — un chandelier à cinq branches porte cinq instances de bougie.

Mon fragment de nom `_candle` attrapait **les deux**. Une flamme se posait donc au
sommet de la boîte englobante du bougeoir, c'est-à-dire au sommet du métal, à côté
des mèches — et une seule pour cinq branches.

Le fragment devient `_candle_castle_`, qui exclut `candleholder_castle_`. On passe
de **301 flammes** (dont 203 sur du métal) à **98**, toutes sur une vraie bougie.
Un test verrouille ce point précis.

### Hauteurs authorées par type

Ses prefabs `_lit` placent la lumière à une hauteur choisie **par type de bougie** :

| Type | Hauteur |
|---|---|
| `candle_castle_01` | 0,337 m |
| `candle_castle_02` | 0,232 m |
| `candle_castle_03` | 0,138 m |
| `candle_castle_04` | 0,395 m |
| `candle_castle_05` | 0,360 m |

Ces valeurs vivent dans `[flames.heights]` et la flamme est posée par la
**transformation du nœud** (`transform_point(Y × hauteur)`), donc correctement même
sur une bougie inclinée dans une applique murale. Un type absent de la table
retombe sur le sommet de la boîte englobante — correct pour une bougie isolée.

### Portée et intensité alignées

| | Avant | Après (ses valeurs) |
|---|---|---|
| Portée | 7 m | **2,45 m** |
| Intensité | 2 200 lm | **630 lm** |
| Couleur | (1,0 · 0,63 · 0,28) | **(1,0 · 0,846 · 0,288)** |
| Bougies allumées | 24 les plus proches | **96** (soit toutes) |

Le plafond passe de 24 à 96 sans coûter plus cher : une lumière de 2,45 m de
portée touche bien moins de groupes de rendu qu'une de 7 m. Ses 78 bougies allumées
le sont toutes en permanence — c'est ce qui donne des **points nets** plutôt qu'un
halo diffus.

### Reste

Nos 98 flammes sont sur les bougies **qu'il laisse éteintes** : ses 78 allumées
n'existent toujours pas dans notre scène. Tant que le correctif **I** (réimport des
191 instances absentes) n'est pas fait, le placement reste approximatif — mais il
est désormais sur les bons objets, à la bonne hauteur et à la bonne portée.


---

## 8. Correctif L — deuxième passe (2026-07-26)

La première passe excluait les bougeoirs, au motif que les bougies sont des objets
séparés. **Retour utilisateur : « je n'en vois aucune. »** Mesuré aussitôt :

| | Distance au spawn de la Grande Salle |
|---|---|
| Bougie la plus proche | **38,7 m** — aucune dans un rayon de 20 m |
| Bougeoir le plus proche | **8,1 m** — 11 dans un rayon de 20 m |
| Sa bougie allumée la plus proche | 51,1 m |

Autour du joueur il n'y a **que des bougeoirs**. Or beaucoup d'entre eux ont leurs
bougies **modelées dans le mesh** (une applique à deux bougies, un chandelier à
trois branches) : il n'existe aucun objet séparé où accrocher une flamme. Les
exclure revenait à éteindre tout le Hall.

### La solution : chercher les mèches dans la géométrie

`tools/gltf/extract_candle_mounts.py` analyse chaque mesh de la famille bougie :
il garde la tranche supérieure des sommets, les regroupe en X/Z, et chaque amas
assez fourni devient un point de flamme.

**Validation croisée** : sur `SM_PROP_candle_castle_05`, la méthode trouve la mèche
à Y = 0,326 m ; le créateur place sa flamme à 0,360 m dans son prefab `_lit`.
L'écart de 3,4 cm est la hauteur de la flamme au-dessus de la cire — il devient le
`LIFT_M` du script. La détection géométrique **retrouve donc sa valeur authorée**.

Résultat (`assets/genomes/castle_hub_candle_mounts.toml`) :

| Type | Mèches |
|---|---|
| `candle_castle_01` … `05` | 1 chacune |
| `candleholder_castle_01`, `02`, `05`, `06` | 1 |
| **`candleholder_castle_03`** | **3** (chandelier à trois branches) |
| **`candleholder_castle_04`** | **5** |
| **`candleholder_castle_08`** | **5** (candélabre de 1,49 m) |

Un premier jet donnait 17 mèches sur le candélabre : le disque du sommet d'une
bougie large (4,6 cm) se découpait en trois amas au même angle. Une passe de
fusion à 9 cm les recolle sans jamais réunir deux bougies voisines (les plus
proches d'un chandelier sont à 13,6 cm).

### Ce que ça donne

**767 flammes**, chacune au-dessus de **sa** mèche : 3 sur un chandelier à trois
branches, 5 sur celui à cinq, 1 sur une applique. Les lumières restent plafonnées
à 96, les plus proches du joueur.

C'est la table — pas une règle sur les noms — qui décide où va une flamme. Un type
absent n'en reçoit aucune. Quatre tests verrouillent le comportement, dont celui
qui a cassé deux fois : **une applique murale doit porter une flamme**, sans quoi la
Grande Salle reste noire.

### Écart assumé avec sa scène

Il n'allume que **78** bougies sur 176 ; nous allumons les 767 mèches détectées.
Rétablir son choix suppose le correctif **I** — importer les 78 positions `_lit`,
qui sont extraites et vérifiées — et n'allumer que celles-là.


---

## 9. Correctif I — les 50 bannières murales (2026-07-26)

### Ce qui manquait n'était pas de la géométrie

`P_PROP_flag_castle_02_static` référence exactement le même
`SM_PROP_flag_castle_02.fbx` que sa version non statique — l'inventaire du pack ne
contient d'ailleurs **aucun** FBX `_static`. Le prefab n'ajoute qu'un matériau
(`M_PROP_flag_static_castle`). Ces 50 instances ne demandaient donc que des
**transformations**, jamais un mesh.

### La conversion des rotations, vérifiée et non devinée

Les positions se reflètent en niant X. Une **rotation** ne se traite pas ainsi :
son axe est un pseudo-vecteur. Sous la réflexion `diag(-1, 1, 1)`, l'axe devient
`det(M) · M·a`, à angle constant, soit sur le quaternion :

    (x, y, z, w)  ->  (x, −y, −z, w)

Nier X comme sur une position aurait retourné les bannières. Le point est vérifié
sur `P_PROP_flag_castle_02`, présent des deux côtés — nos 4 nœuds existants sont
reproduits à :

| | Écart |
|---|---|
| Position | **0,0006 m** |
| Rotation | **≤ 0,104°** (bruit de flottant) |
| Échelle | **identique** (1,0 et 0,8409) |

La chaîne d'extraction reproduit donc exactement ce que la reconstruction
d'origine avait produit — c'est ce qui autorise à lui faire confiance sur les 50
instances qu'elle n'avait pas produites.

### Pourquoi cloner un mesh chargé plutôt qu'ajouter un asset

Deux autres voies, écartées :

- **Injecter les nœuds dans les cellules glTF.** L'éditeur de scène identifie une
  pièce par son *rang de fratrie* : insérer des nœuds décalerait ces rangs et
  invaliderait les retouches enregistrées dans `castle_hub_edits.json`.
- **Extraire un GLB autonome.** Duplique une géométrie déjà en mémoire et impose
  de re-router ses textures.

Le château tient dans 193 m, le streaming charge à 240 m : dans le Hall, une
bannière d'origine est toujours chargée quelque part. On capte les `Handle` de son
mesh et de ses matériaux, on les réutilise. Zéro octet ajouté, matériau exact par
construction. Chaque drapeau ayant **2 primitives**, la capture les relève toutes.

### Reste du correctif I

| Absentes | Nature | État |
|---|---|---|
| 50 bannières | variantes `_static`, même mesh | ✅ **fait** |
| 78 bougies allumées | variantes `_lit`, même mesh | ⏸️ voir ci-dessous |
| 61 FX | brume, poussières, rai de lumière | ❌ systèmes de particules Unity — à réauthorer |
| 2 grandes portes | prefab **composite** multi-mesh | ❌ hors du format à un mesh |

Les 78 `_lit` sont extractibles par le même outil, mais les poser rouvre une
décision : nous allumons aujourd'hui les 767 mèches détectées, lui n'en allume que
78. Ajouter ses bougies allumées sans restreindre l'allumage donnerait 845 flammes.
C'est un choix d'ambiance, pas un défaut technique — à trancher avant de l'appliquer.
