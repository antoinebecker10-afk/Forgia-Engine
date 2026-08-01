# Ciels et sols d'arène — inventaire MESURÉ + ambiances proposées

**Date** : 2026-08-01
**Méthode** : registre `assets/genomes/asset_registry.toml` (986 assets mesurés, story-673),
relu par mesure — pas par nom de fichier.
**Cross-refs** : `.claude/rules/map-design-intention.md`, `no-hardcode.md`, story-674

---

## 0. Ce que j'ai trouvé en ouvrant le capot

Trois constats, tous vérifiés dans le code, avant même de trier :

**Le ciel a 12 palettes, les arènes en utilisent 2.** `assets/genomes/biome_sky.toml`
déclare 12 ambiances (volcanic, plains, forest, desert, mountain, swamp, tundra,
savanna, jungle, canyon, crypts…). Les 4 stages de `roguelite_stages.toml` déclarent
`Volcanic` (crypts_of_anvil) et `Plains` **pour les trois autres**. Trois arènes sur
quatre ont donc exactement le même ciel.

**Le brouillard est volcanique partout, y compris aux Hauts Pâturages.**
`atmosphere.rs` applique `volcanic_fog()` + `volcanic_ambient()` sur
`run_if(in_state(GameMode::Roguelite))` — **aucun filtre par biome**. Les couleurs sont
des constantes Rust (`FOG_COLOR`, `AMBIENT_COLOR`) ; seules la densité et la
luminosité viennent du génome. Un pâturage vert sous une brume rouge-orangée.

**Le sol est une constante Rust, identique aux 4 arènes.**
`forgia-stage/src/lib.rs:43` — `MERGED_FLOOR_GLB: [&str; 3]` = `floor.glb`,
`floor_dirt.glb`, `floor_rocks.glb` du kit KayKit dungeon. Ce n'est même pas en couche
definition : on ne peut pas le changer sans recompiler.

> Le déséquilibre structurant : **12 ambiances de ciel, 1 sol.** Faire varier le ciel
> sans faire varier le sol donnera 4 arènes qui se ressemblent avec des couleurs
> différentes.

---

## 1. Les sols — triés à la MESURE

Le registre porte déjà un champ `nature = "floor"` sur 119 assets. **Il est exact à
26 %** : 88 des 119 ne sont pas des sols (des fondations de 2 m de haut, des pics, un
projectile de catapulte). Encore de la classification par nom de fichier.

Le bon critère n'est pas la platitude — les tuiles hexagonales sont des **prismes**
avec une jupe d'1 m sous la surface, et elles sont parfaitement pavables. Le bon
critère est le **pas de trame** : un sol, c'est ce qui se répète sur une grille.

### 1.1 Les quatre familles pavables

| Famille | Pas de trame | Tuiles | Surfaces disponibles | Coût vs actuel |
|---|---|---|---|---|
| **kaykit dungeon** *(en service)* | 4 m | 4 | pierre, terre, gravats | ×1 |
| **kaykit dungeon_remastered** | 2 / 4 / **8 m** | 24 | pierre, terre, **bois** (clair + foncé), herbes folles, dalles **cassées**, coins | ×1 en 4 m, **÷4 en 8 m** |
| **kaykit hexagon tiles** | hex 2,00 × 2,31 m | 60 | herbe, eau, côte ×5, **rivières ×15**, **routes ×15**, + variantes sèches | ×3,6 |
| **kenney nature** | 1 m | 29 | herbe, **chemins ×13**, **rivières ×13** (jeu de connexions complet) | ×16 |

**Le coût est le point dur.** L'arène pave ~1 600 cellules à `FLOOR_TILE_SIZE = 4.0`.
Seules les familles **4 m et 8 m sont substituables telles quelles** ; l'hexagone
multiplie la fusion par 3,6 et le kenney 1 m par 16. Le sol est fusionné
(`floor_merge.rs`), donc ce n'est pas 16× d'entités au final — mais c'est 16× de
meshes à fusionner au chargement, ce qui n'est pas mesuré aujourd'hui.

### 1.2 Ce qui n'est PAS une tuile, et qu'on a pris pour telle

Dix « floor_* » du kit remastered ne pavent rien — ce sont des **modules de jeu** :

| Module | Emprise | Hauteur | Ce que ça fait |
|---|---|---|---|
| `floor_tile_big_spikes` | 4 m | **2,10 m** | zone de refus — pousse le joueur à bouger |
| `floor_tile_grate` + `_open` | 4 m | 1,03 m | grille ouvrable → fosse |
| `floor_tile_extralarge_grates` + `_open` | **8 m** | 1,13 m | grande fosse |
| `floor_foundation_*` (6 connexions) | 2 m | 2,00 m | rebords / socles bordés |

Et deux **rampes**, qui valent d'être notées : `hex_grass_sloped_low` (+1,50 m) et
`_sloped_high` (+2,00 m), idem en version route. Le saut du joueur est **1,174 m** :
ces deux pentes **ne se franchissent pas au saut**, ce sont donc de vraies liaisons
verticales — franchissables par l'IA, qui ne saute pas. C'est exactement ce que
demande le pattern 7 (`map-design-patterns.md`).

### 1.3 Trois plateformes d'arène entières, jamais utilisées

| Asset | Emprise | Usage possible |
|---|---|---|
| `inferno/PlatformStar_001` | **61,3 m** | une arène en étoile complète, d'un bloc |
| `inferno/PedestalBig_001` | 21,4 m | plateforme de boss surélevée |
| `inferno/CirclePlatformSmall_001` | 19,2 m | îlot / salle de repos |

---

## 2. Les ciels — les quatre leviers, et lesquels sont branchés

| Levier | Où | État |
|---|---|---|
| **Gradient cubemap** (zénith / horizon / sol) | `biome_sky.toml`, 12 palettes | ✅ branché, **2/12 utilisées** |
| **Overlay nuages cartoon** `sky_129` | `overlay_blend` 0-1 par palette | ✅ branché, réglé sur 2 palettes |
| **Brouillard + ambiante** | `atmosphere.rs` | ⚠️ **couleurs en dur**, volcaniques, sur les 4 arènes |
| **IBL / HDRI** | `assets/hdri/env-maps-v1/outdoor/` — 9 fichiers | ❌ **zéro usage roguelite** (câblé seulement sur le Castle Hub) |

Les 9 HDRI en stock : `autumn_field`, `desert_sky`, `dramatic_clouds`, `evening_road`,
`kloofendal_dawn`, `night_sky`, `snowy_forest`, `sunset_forest`, `studio_small`.

### Une remarque qui change la valeur du brouillard

La densité est `0.008` → demi-brume à **~125 m**. La plus longue ligne de vue mesurée
dans nos arènes est **24,2 m**. Le brouillard ne fait donc **rien** au gameplay
aujourd'hui : il ne sert que la teinte. Pour qu'il porte quelque chose, il faut soit
des lignes plus longues, soit une densité calée sur la portée d'arme (pleine puissance
jusqu'à 30 m, −40 % au-delà).

---

## 3. Les ambiances possibles — ce qu'on peut bâtir SANS nouvel asset

Une ambiance = **ciel + sol + brouillard + lumière**, déclarés ensemble. C'est la
bonne unité : aujourd'hui le ciel est indexé sur `biome`, un champ hérité du terrain
ouvert, alors qu'une arène de roguelite n'a pas de biome — elle a une identité.

| # | Ambiance | Ciel | Sol | Manque |
|---|---|---|---|---|
| 1 | **Forge ardente** *(crypts, actuel)* | `volcanic` ✅ | dungeon pierre ✅ | lave émissive |
| 2 | **Donjon suintant** | `forest` / `swamp` ✅ | remastered pierre + herbes folles + grilles ✅ | — |
| 3 | **Halles de bois** | `plains` ✅ | remastered **wood** clair/foncé ✅ | — |
| 4 | **Hauts pâturages** | `plains` ✅ | hexagon herbe + routes ✅ | — |
| 5 | **Ruines noyées** | `swamp` ✅ | hexagon eau + côte ✅ | eau jouable |
| 6 | **Nécropole nocturne** | `mountain` + HDRI `night_sky` ✅ | remastered dalles cassées ✅ | — |
| 7 | Désert | `desert` ✅ | ❌ | tuiles sable |
| 8 | Toundra | `tundra` ✅ | ❌ | tuiles neige/glace |

**Quatre ambiances (2, 3, 4, 6) sont livrables sans produire un seul asset.** Elles ne
demandent que de sortir le sol et le brouillard de leurs constantes.

---

## 4. Ce que je recommande d'apporter — par ordre de valeur

### A. Ce qui débloque le plus, et ne coûte aucun asset

**A1 — Sortir le sol et l'atmosphère du code.** Tant que `MERGED_FLOOR_GLB` est une
`const` et que `volcanic_fog()` a ses couleurs en dur, aucune variation n'est possible
sans recompiler. C'est le préalable à tout le reste, et c'est du pur déplacement vers
la couche definition.

**A2 — Indexer sur l'AMBIANCE, pas sur le biome.** Un `[ambiance.X]` qui porte le
quadruplet ciel/sol/brouillard/lumière, et un champ `ambiance` par stage. Le biome
reste pour le terrain ouvert, où il a du sens.

**A3 — Le ciel comme horloge de run.** C'est le plus fort rapport valeur/effort de la
liste : au lieu d'indexer le ciel sur l'identité de la salle, l'indexer sur la
**profondeur**. Salle 1 aube → salle 2 jour → salle 3 crépuscule → boss nuit. Le
joueur *voit* où il en est dans sa run sans regarder l'interface. Hades le fait par
région, Risk of Rain 2 par étage. Les 4 palettes nécessaires sont **déjà écrites**.

### B. Les types de sol qui manquent vraiment

Par ordre de ce que ça apporte au jeu, pas de facilité :

**B1 — Lave / sol émissif.** Zéro asset mesuré contient `lava` ou `magma`. C'est le
manque criant : le jeu s'appelle Forgia, l'identité est la forge, et on n'a pas une
seule surface chaude. Un plan émissif + masque de fissures suffit, ça ne demande pas
un kit.

**B2 — Sol qui blesse.** On a les pics (`floor_tile_big_spikes`, 4 m) et les grilles
ouvrables, jamais posés. Un sol qui refuse une zone est le levier le moins cher
contre le stand de tir — il force le déplacement sans toucher aux ennemis.

**B3 — Eau jouable.** `hex_water` existe, `bevy_water` est dans la stack, et une crate
`forgia-water` avec une ressource `SeaLevel` existe. Zéro usage roguelite. De l'eau
qui ralentit, c'est un modificateur de rythme gratuit.

**B4 — Sable et neige.** Les deux palettes de ciel existent et n'ont aucun sol
assorti. À acheter ou générer — c'est le seul point de cette liste qui demande
d'acquérir des assets.

### C. Ce que je ne recommande PAS tout de suite

- **Le kenney 1 m en sol d'arène** — ×16 sur la fusion pour un gain de variété que le
  remastered 4 m donne déjà. En revanche son jeu de **chemins** (13 connexions) vaut
  d'être utilisé en *sur-couche* sur un sol 4 m, pas comme sol.
- **Les HDRI en IBL sur les arènes** — 9 fichiers en stock, mais l'échec documenté du
  2026-07-26 (`reference_arena_lighting_scale_ibl_failed_attempt`) dit qu'une IBL à
  0,6 % de 8 000 lux est invisible. Ne pas y retourner sans baisser le soleil d'abord.
- **Le brouillard comme outil de gameplay** — inutile tant que les lignes de vue font
  24 m. À reprendre quand les portées auront changé.

---

## 5. Ce que cet inventaire ne dit pas

- **Le coût de fusion** d'une trame plus fine (hexagone ×3,6, kenney ×16) n'est pas
  mesuré. Il faut le mesurer avant de choisir une famille non-4 m.
- **Aucune ambiance n'a été jugée en jeu.** Les couples ciel/sol ci-dessus sont
  cohérents sur le papier ; la lisibilité en combat se tranche manette en main
  (`map-design-intention.md` §5.3).
- **Le pivot des tuiles** n'est pas vérifié famille par famille : le registre porte un
  champ `pivot`, mais je n'ai pas contrôlé que les quatre familles posent leur surface
  au même Y. C'est un piège classique et il coûte un sol flottant ou enterré.
