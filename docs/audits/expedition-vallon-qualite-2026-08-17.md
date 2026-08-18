# Audit qualité — « Le Vallon », carte d'expédition — 2026-08-17

> **Méthode.** Tout chiffre ci-dessous est mesuré sur les fichiers réels
> (manifeste, 48 cellules glTF, GLB de collision, `vallon.py`, `91_export.py`,
> `plugin.rs`) ou compté par script. Aucun n'est repris d'un bloc `mesures`
> auto-déclaré — la règle du projet est qu'une grandeur écrite deux fois finit
> toujours par diverger, et cet audit s'y soumet lui-même.
>
> **Portée.** Géométrie, contrat d'export, contenu, coût de rendu, observabilité.
> Ne couvre pas : le son, la lisibilité en mouvement, le playtest manette en main.

---

## 0. Verdict en une page

La carte est **bien autorée et mal contractée**. Le relief, le tracé, la rivière
et la révélation du village sont solides — la géométrie fait ce qu'elle promet.
Ce qui manque n'est presque jamais de la forme : c'est **ce que la carte
n'exporte pas**, et **ce qu'elle échoue à produire sans le dire**.

| Famille | État | Le fait qui tranche |
|---|---|---|
| Géométrie / composition | ✅ solide | chemin 358,7 m, révélation par le col, gorge, ceinture close |
| **Collision** | 🔴 **cassé** | les 3 salles de combat ont **0 abri collidable** ; la porte du village se traverse |
| **Contrat déclaré ≠ produit** | 🔴 **cassé** | 8 zones de faune sur 11, 15 abris sur 18 — **en silence** |
| Spec de combat | 🟠 partielle | 4 m de tir gratuit sur les 3 camps, mesurés et non corrigés |
| Coût de rendu | 🟠 à reprendre | **78 %** de la carte résidente en moyenne, aucun LOD |
| Rendu / matière | 🟠 plafonné | 30 matériaux sur 37 sans texture ; le triplanaire dort |
| **Observabilité** | 🔴 **absente** | 131 capteurs, **aucun** sur la carte |

**La conclusion opérationnelle** : ne pas retoucher la forme. Les quatre
chantiers qui suivent portent sur le contrat d'export, et se font tous sous
Blender sans déplacer un seul sommet.

---

## 1. Collision — la classe la plus grave

### 1.1 Ce que le GLB de collision contient réellement

`91_export.py:130-153` ne joint que **deux** objets :

```python
sol   = bpy.data.objects.get("vallon_sol")
batis = bpy.data.objects.get("vallon_village")
```

Or l'export fusionne **neuf** collections (`91_export.py:107-111`) :
`sol`, `falaises`, `foret`, `village`, `campements`, `reperes`, `lampes`,
`eau`, `portes`.

**Sept collections sur neuf n'ont donc aucune collision de maillage.** Mesuré :
`expedition_vallon_collision.glb` = 1 nœud, 1 maillage, **74 474 triangles**.

Le docstring du fichier annonce pourtant `physique → terrain + bâti + pont`
(ligne 10). Le pont n'y est pas — il vit dans sa propre collection.

### 1.2 Ce que les 943 cylindres couvrent

`troncs.append(...)` n'apparaît **qu'une fois** dans `vallon.py`, ligne 1724 —
à l'intérieur de la boucle des futaies. La seconde boucle (ligne 1728, les
arbres isolés) place ses arbres **sans jamais alimenter la liste**.

L'arithmétique le confirme exactement : `part_en_futaie 0,82 × arbres_total
1150 = 943`, soit précisément le nombre de colliders exportés.

### 1.3 Le bilan — ce qui se traverse en jeu

| Élément | Nombre | Collision |
|---|---:|---|
| Arbres en futaie | 943 | ✅ cylindre |
| **Arbres isolés** | **207** | ❌ aucune |
| **Rochers** | **110** | ❌ aucune |
| **Éboulis de falaise** | **260** | ❌ aucune |
| **Rochers de bouchage (ceinture)** | **22** | ❌ aucune |
| Murs | 17 | ✅ TriMesh — *voir correction ci-dessous* |
| **Abris de campement** | **15** | ❌ aucune |
| **Braseros** | **16** | ❌ aucune |
| **Porte du village (2 battants)** | **2** | ❌ aucune |
| Bâtiments du village | 10 | ✅ TriMesh |
| Terrain | — | ✅ TriMesh |
| Sous-bois, herbe | 3 820 | ❌ *(voulu)* |

### 1.4 Pourquoi c'est le constat n°1 et pas une finition

Trois conséquences, par ordre de gravité :

1. **Les trois salles de combat n'ont aucune couverture fonctionnelle.** Les
   abris sont déclarés `blocs >= 1,8 m : ils cassent la vue`
   (`vallon.py:285`). Sans collider, ils n'arrêtent ni le joueur ni un rayon
   de tir : ce sont des décors. `map-design-patterns.md` §11 note le pattern
   « couverture binaire » comme *ÉCRIT* — sur cette carte il est **contredit**.
2. **La porte du village se franchit fermée.** Le manifeste porte
   `porte_village` avec rayon de déclenchement et durée d'ouverture ; le
   `plugin.rs:46-52` la charge en statique et documente que l'ouverture reste à
   faire. Sans collision, l'ouverture n'a même pas d'objet.
3. **La ceinture est perméable par ses bouchons.** Les 22 rochers de château
   posés pour combler les 58 creux du rempart (`ceinture_creux: 58`) sont
   précisément ce qui ferme la carte — et ils se traversent.

### 1.4 bis — CORRECTION apportée à cet audit

La première rédaction comptait les **17 murs du village** parmi les éléments sans
collision. **C'est faux** : ils sont posés dans `c_village` (`vallon.py:2117`),
donc fusionnés dans `vallon_village`, donc présents dans le TriMesh de collision.
Idem pour les clôtures et les champs.

Le constat est corrigé dans le tableau ci-dessus. Il est laissé visible plutôt
que réécrit en silence : un audit qui se corrige sans le dire vaut exactement ce
que valait le commentaire faux qu'il dénonce.

### 1.5 La commentaire Rust à corriger côté moteur

`crates/forgia-mode-expedition/src/manifest.rs:151` :

```rust
/// `[x, y, z, rayon]` en repère Blender. Les troncs, rochers et murs.
pub colliders_cylindre_xyzr: Vec<[f32; 4]>,
```

**Faux** : uniquement des troncs, et seulement 82 % d'entre eux. Un commentaire
qui décrit une donnée qu'il n'a pas est exactement ce qui fait chercher le
défaut ailleurs.

---

## 2. Déclaré ≠ produit — l'échec silencieux

Deux systèmes de placement ratent une partie de leur cible **sans qu'aucune
sortie ne le dise**. Le manifeste rapporte ce qu'il a fabriqué, jamais ce qu'il
a manqué.

### 2.1 Faune : 8 zones sur 11

| Espèce | Attendu | Produit | Manque |
|---|---:|---:|---:|
| deer | 2 | 2 | 0 |
| horse | 1 | 1 | 0 |
| kitty | 2 | 2 | 0 |
| **pinguin** | 2 | 1 | **1** |
| dog | 1 | 1 | 0 |
| **chicken** | 2 | **0** | **2** |
| tiger | 1 | 1 | 0 |
| **TOTAL** | **11** | **8** | **3 (27 %)** |

Les poules sont déclarées dans `SPEC["faune"]["especes"]` et **totalement
absentes** de la carte. Cause : les critères du milieu `abords`
(village 36–52 m, chemin ≥ 8 m, pente ≤ 14°, écart inter-zones 26 m) ne
trouvent pas deux créneaux. Rien ne le signale.

Défaut connexe : la zone `pinguin` survivante est en `berge`, à **z = 11,63 m**
pour un plan d'eau à ≈ −2 m à cette latitude. Les manchots sont **13 m
au-dessus de leur rive**. Le critère `riviere: [11, 24]` mesure la **distance à
l'axe en plan** et jamais la hauteur au-dessus de la nappe — c'est un critère
2D appliqué à un milieu qui est défini par sa relation verticale à l'eau.

### 2.2 Abris de campement : 15 sur 18

| Camp | Demandés | Posés |
|---|---:|---:|
| camp_1 | 6 | **4** |
| camp_2 | 6 | **5** |
| camp_3 | 6 | 6 |

Cause identifiée : `poser_camp` (`vallon.py:2075-2081`) refuse tout prop dont
la latitude est sous `couloir_libre` (3,6 m) — et **rend `0` sans compter le
rejet**. Le filtre est correct ; c'est son silence qui ne l'est pas.

### 2.3 La règle que ces deux cas enfreignent

`map-design-patterns.md` §13 — « **Zéro mesuré n'est pas vert, c'est aveugle.**
Tout contrôle expose la taille de son échantillon. » Ici les compteurs existent
(`rapport["zones_faune"]`, `nabris`) mais rapportent **l'obtenu sans la
demande**, ce qui se lit comme un succès.

---

## 3. Spec de combat — le tir gratuit, mesuré et non corrigé

Les trois campements annoncent `ligne_max_m: 24,0` pour
`grunt_vision_m: 20,0` — **4 m de tir gratuit chacun**.

Le moteur est déjà honnête sur ce point : `CampementDef::vision_couvre_la_ligne`
le détecte, `plugin.rs:288-300` l'écrit en `warn!` à chaque chargement, et le
test `le_vallon_donne_du_tir_gratuit_et_on_le_mesure` **épingle l'état mesuré**
pour qu'il tombe le jour où la carte change. C'est exemplaire — et ça ne corrige
rien.

**La cause est dans la SPEC, et elle est auto-contradictoire.** Le commentaire
`vallon.py:271-279` cite `map-design-intention.md` §2.2 (« ligne max ≤ vision du
grunt ») puis déclare `"rayon": 12.0` — soit un diamètre de 24 m. Le rayon est
**écrit**, alors qu'il devrait se **dériver** :

```
rayon_max = min(vision des archétypes présents) / 2  =  20 / 2  =  10 m
```

Un rayon de 10 m rend l'invariant vrai par construction. Le coût : les
apparitions à 6–10 m deviennent 5–8 m, ce qui **renforce** §2.1 (l'essaim arrive
encore plus sûrement).

L'alternative — casser la ligne de vue par de vrais abris — n'est pas
disponible tant que le §1 n'est pas fait : on ne casse pas une ligne de vue avec
une couverture qui n'a pas de collider.

### 3.1 Ce que la spec de combat ne dit toujours pas

`map-design-intention.md` §1 exige, par salle : ennemis **et leurs archétypes**,
arsenal joueur attendu, durée d'engagement visée, condition de sortie. Le
manifeste porte les apparitions (7 par camp) et les deux nombres de vision —
déjà plus qu'aucune arène du projet n'a jamais eu — mais **pas quel archétype
apparaît**, ni la durée visée, ni la condition de sortie. Sans ça, `verrou_xyz`
est une position sans règle.

---

## 4. Coût de rendu et streaming

### 4.1 Le décor, mesuré

| Grandeur | Valeur |
|---|---:|
| Cellules | 48 (40 m de côté) |
| Primitives (≈ appels de rendu) | **619** |
| Triangles | **967 046** |
| Matériaux distincts | 37 |
| Cellule médiane | 14 prim / 22 099 tri |
| Pire cellule (`cell_x0_z0`) | 20 prim / **76 594 tri** |

### 4.2 Le streaming ne streame presque rien

Rayon de chargement 140 m (`plugin.rs:61`), cellules de 40 m. Simulé le long
des 91 points du chemin, avec la distance horizontale point↔AABB qu'utilise
`cells::horizontal_distance` :

```
cellules résidentes : min 28 / médiane 40 / max 44  sur 48
→ 78 % de la carte en mémoire en moyenne
```

Le test `le_rayon_de_chargement_couvre_ce_qu_on_voit_sans_charger_toute_la_carte`
vérifie `140 < demi-diagonale (172)` et passe — mais la demi-diagonale n'est pas
le bon juge sur une carte **rectangulaire de 200 m de large** : depuis n'importe
quel point du chemin, un rayon de 140 m couvre toute la largeur. Le contrôle est
vert et aveugle (§13 encore).

### 4.2 bis — CORRECTION : ce n'est pas un défaut, et B7 était une erreur

Mesuré après coup, sur la carte recuite (47 cellules), en comptant non plus les
cellules mais **ce qu'elles coûtent** :

| | min | médiane | max | total |
|---|---:|---:|---:|---:|
| cellules résidentes | 28 | 39 | 42 | 47 |
| primitives résidentes | 392 | 576 | 603 | 633 |
| triangles résidents | 619 454 | **927 785** | 960 634 | 972 102 |

**95 % des triangles sont résidents**, pas 78 % : les grosses cellules centrales
ne quittent jamais la mémoire. Le streaming gagne 5 %.

Mais la cause n'est pas un mauvais réglage — **c'est que la carte est plus petite
que sa portée de vue**. `cycle.rs:71` porte le brouillard à **420 m** au départ,
pour une demi-diagonale de 172 m. Tout est dans le champ, donc tout doit être
chargé. Une résidence de 95 % est la conséquence *correcte* de ces deux nombres.

**B7 (« cellules de 20 m ») est donc retiré**, et il aurait aggravé les choses :
chaque cellule reporte ses propres primitives par matériau, donc quadrupler leur
nombre ferait passer le total de 633 à ~1 800 pour n'en rendre résidentes que la
moitié — **plus d'appels de rendu qu'aujourd'hui, pour moins de mémoire**. On
aurait payé une régression au prix d'une optimisation.

Si le coût d'image devient un problème, le levier est le **LOD** (B8) : réduire
les triangles de ce qu'on voit, pas décharger ce qu'on voit.

⚠️ **Tension mesurée, à trancher manette en main** : brouillard 420 m contre
rayon de chargement 140 m. Depuis le départ, le fond de carte est à ~280 m — donc
visible et non chargé. La ceinture de 26 m l'occulte peut-être entièrement ; ça
ne se sait qu'en regardant. Le capteur `forgia2_expedition_carte.json` donne
désormais `part_residente_pct` et `rayon_chargement_m` pour juger sur pièce.
Rien n'a été changé : aucun symptôme n'a été rapporté sur ce point
(`no-speculative-fix.md`).

### 4.3 Aucun LOD, nulle part

- Rien dans le pipeline ne produit de niveau de détail.
- Le kit château **fournit** LOD1/LOD2 : `12_kit_chateau.py:57` les écarte avec
  la mention « pour un système de LOD qu'on n'a pas encore ».
- Conséquence : un pin à 130 m coûte autant qu'à 3 m.

### 4.4 Deux exports morts sur le disque

| Fichier | Poids | Référencé par |
|---|---:|---|
| `expedition_vallon.glb` | **75,6 Mo** | **rien** |
| `expedition_vallon_walkable.glb` | 1,3 Mo | **rien** |

Le premier est le GLB monolithique, remplacé par les 48 cellules (67 Mo) mais
jamais retiré. Le second a été cuit pour un navmesh — `91_export.py:11`
l'annonce comme tel — et **aucun navmesh n'existe** : les bots marchent en
ligne droite. Sur une carte à ceinture de 26 m, gorge et col, c'est un
problème à part entière (`map-design-intention.md` §2.5).

---

## 5. Rendu et matière — le plafond visuel

- **30 matériaux sur 37 n'ont aucune texture** — héritage direct du kit Kenney,
  qui est nativement en aplats. Chaque matériau porte sa teinte dans son
  `baseColorFactor` glTF.
- `crates/forgia-mode-expedition/src/matiere.rs` (nouveau, non commité) couvre
  28 de ces matériaux avec **six cartes de gris de 256², 71 Ko**, multipliées
  dans `base_color_texture`. C'est une très bonne réponse moteur — et elle
  traite le **grain**, pas l'**ancrage**.
- `assets/shaders/v1-port/terrain_triplanar.wgsl`, **702 lignes** (anti-tiling
  Quilez, poids de biome par couleurs de sommet, roche par la pente) —
  **aucune référence Rust**. C'est un asset, pas une capacité.
- **Pas de cel-shading en Expédition** : `toon_config.rs` vit dans
  `forgia-mode-roguelite`. Le capteur le confirme —
  `toon → toon strength>0 mais 0 Camera3d attached`. Le mode rend en PBR nu.
- 🚨 **Les couleurs de sommet n'ont JAMAIS été cuites.** Mesuré après coup, et
  c'est le constat le plus coûteux de cet audit : **zéro primitive des 48
  cellules ne portait `COLOR_0`**. Le terrain a pourtant son attribut `Col`
  (22 176 sommets), rempli par `vallon.py` avec le brouillage de teinte aux
  frontières de matière — un travail soigné qui n'atteignait pas le jeu.

  La cause n'est pas l'option d'export : `export_vertex_color="ACTIVE"` est bien
  valide en Blender 4.5.10 (vérifié en interrogeant l'opérateur). C'est le
  `join()` des cellules : sur 117 maillages, **3 seulement** portaient un
  attribut de couleur, et quand la fusion prend pour objet actif un prop qui
  n'en a pas, l'attribut des autres est perdu. Le défaut ne lève rien — il
  produit un glTF plausible et incomplet.
- **Aucune occlusion ambiante cuite**, aucun contact au sol, aucun vent.

C'est ici que se trouve le plus gros gain visuel encore disponible sous
Blender — voir chantier **B4**.

---

## 6. Observabilité — absente

**131 capteurs lus, aucun sur la carte.** Les deux seuls fichiers d'expédition
concernent l'arme tenue en main :

```
forgia2_expedition_arme.json     (story-717)
forgia2_expedition_visee.json
```

Rien ne publie : cellules résidentes, triangles à l'écran, colliders posés,
campements franchis, faune vivante, contenu manquant. `observability-required.md`
est explicite : « Quand l'user dit *regarde*, l'IA doit pouvoir diagnostiquer la
feature en lisant sa sortie. Si l'IA ne voit rien → la feature est incomplète. »

Le `info!` de chargement (`plugin.rs:303`) est la seule trace, et elle ne
survit pas à la frame.

---

## 7. Chantiers Blender — ordonnés par rendement

> Aucun ne déplace un sommet. Tous portent sur le **contrat d'export** et sur
> **ce que le pipeline avoue**.

### B1 — Publier les emprises solides de tout ce qui est solide 🔴 P0

**Le fait** : sept collections sur neuf n'ont aucune collision (§1).

**La forme correcte** : ne pas verser `falaises`, `campements` et `reperes`
dans le TriMesh de collision — un rocher du kit en TriMesh coûte cher pour une
forme qu'un cylindre décrit très bien. Étendre plutôt la **liste de proxys**,
qui existe déjà et que le moteur sait consommer.

**La dérivation, et le piège à éviter** : le rayon actuel est
`0,055 × echelle` (`vallon.py:1724`) — un coefficient, pas une mesure.
`spawn-clearance.md` §4 nomme exactement cette faute : *« une valeur de tuning
n'est pas une mesure »*, et c'est elle qui a fait naître des mobs dans des
bâtiments. Le rayon doit venir de l'**AABB de la pièce placée**, après
application de l'échelle d'instance :

```python
r = 0.5 * max(aabb.x, aabb.y)      # emprise au sol, généreuse
h = aabb.z                          # et publier la hauteur, cf. B2
```

**Sortie** : garder `colliders_cylindre_xyzr` pour les troncs (le moteur le lit
déjà), et ajouter `colliders_prop_xyzhr` — `[x, y, z, hauteur, rayon]` — pour
rochers, éboulis, bouchons, murs et braseros. La hauteur cesse d'être devinée
côté moteur (`HAUTEUR_COLLIDER_M = 6.0` en dur dans `plugin.rs:69`).

**Inclure les 207 arbres isolés** : déplacer l'`append` hors des deux boucles,
dans `nature.poser`, pour qu'aucune boucle future ne puisse l'oublier.

**Effort** ~1 h · **Risque** bas · **Débloque** la porte, la ceinture, et B2.

---

### B2 — Sortir les abris du décor : ce sont des meubles de combat 🔴 P0

**Le fait** : les 15 abris sont dans `campements`, donc sans collision, donc
sans fonction (§1.4).

**Ce qui les distingue d'un rocher** : un abri a un **contrat** — casser la
ligne de vue à hauteur d'œil (1,70 m). Il doit donc publier sa **hauteur
mesurée**, pas seulement son emprise, pour que le moteur — et le prochain audit
— puissent vérifier qu'il fait ce que son nom dit
(`map-design-intention.md` §5.1 : « le nom est un contrat »).

**Sortie** : dans chaque campement du manifeste, remplacer `abris_xy`
(`[[x, y]]`, muet) par :

```json
"abris": [{"xyz": [...], "rayon_m": 1.9, "hauteur_m": 2.4, "casse_la_vue": true}]
```

`casse_la_vue` se **dérive** (`hauteur ≥ 1,8 m`), ne se déclare pas. Un abri
posé à 1,2 m sort avec `false` et devient visible dans le capteur au lieu de
passer pour une couverture.

**Effort** ~45 min · **Risque** bas · **Débloque** la correction du §3 par la
géométrie plutôt que par le rayon.

---

### B3 — Faire échouer bruyamment : un bloc `defauts` dans le manifeste 🔴 P0

**Le fait** : 3 zones de faune et 3 abris manquent sans aucun signal (§2).

**La règle** : tout compteur à cible publie **la demande, l'obtenu, et les
rejets par cause**. Un manque > 0 est une alerte, jamais un silence.

```json
"defauts": [
  {"quoi": "faune.chicken", "demande": 2, "obtenu": 0, "gravite": "alerte",
   "cause": "aucun creneau : milieu abords, village 36-52 m, ecart 26 m",
   "rejets": {"pente": 412, "distance_village": 1877, "ecart_zones": 203}},
  {"quoi": "campements.camp_1.abris", "demande": 6, "obtenu": 4,
   "gravite": "alerte", "cause": "couloir_libre 3,6 m", "rejets": {"couloir": 2}}
]
```

Faire porter le compte par `poser_camp` et par le tirage de faune — les deux
rendent déjà `0` ou bouclent, il ne manque qu'un compteur par cause de rejet.

**Corollaire** : `"defauts": []` devient une **preuve**, alors qu'aujourd'hui
l'absence de la clé ne prouve rien.

**Effort** ~1 h 30 · **Risque** bas · **Rend l'audit rejouable sans script.**

---

### B4 — Cuire l'occlusion ambiante dans les couleurs de sommet 🟠 P1

**C'est le plus gros gain visuel encore disponible, et il ne coûte aucun shader.**

**Le fait** : les props sont en aplats sans texture (§5), donc sans aucune
variation de valeur. C'est ce qui donne l'impression d'autocollants posés sur
un sol — pas la palette, qui est bonne.

**Pourquoi ça passe le glTF** : `COLOR_0` est l'un des rares canaux que le
format transporte, et Bevy le multiplie par `base_color`. L'export les envoie
déjà (`91_export.py:82`) — il n'y a **rien à câbler côté moteur**, et ça se
compose avec le grain de `matiere.rs` au lieu de le concurrencer.

Trois cuissons, par rendement décroissant :

1. **AO sur les props** — assombrit les dessous de feuillage, les creux de
   rocher, l'intérieur des massifs. C'est ce qui **pose** un objet au sol.
2. **Ancrage au sol sur le terrain** — assombrir les sommets du terrain dans un
   rayon de 1 à 2 m sous chaque tronc et chaque rocher. Le contact fait plus
   pour l'intégration qu'une ombre portée, et il est gratuit à l'exécution.
   Les positions sont déjà toutes connues au moment du plan.
3. **Dégradé de hauteur sur le feuillage** — sombre à la base du houppier,
   clair au sommet. Un arbre en aplat n'a aucun volume ; ce gradient lui en
   donne un, sans polygone de plus.

**Vérification obligatoire** : la teinte finale est `COLOR_0 × baseColorFactor`.
Une cuisson centrée sur 0,5 assombrirait tout le décor de moitié. Les cartes
doivent **plafonner à 1,0** et ne faire que descendre — même piège que celui
déjà payé et documenté dans `matiere.rs` §2 (l'octet 127 valant 0,21 en
linéaire).

**Effort** ~3 h · **Risque** moyen (à valider en jeu) · **Le levier n°1 du look.**

---

### B5 — Dériver le rayon des campements au lieu de l'écrire 🟠 P1

**Le fait** : `"rayon": 12.0` produit 24 m de ligne pour 20 m de vision (§3).

```python
"rayon": min(vision de chaque archétype présent) / 2.0
```

Le nombre disparaît de la SPEC ; il devient une conséquence des génomes
d'ennemis, qui sont sa vraie source. Le jour où l'archer (vision 35 m) est seul
dans un camp, le camp s'agrandit tout seul.

**Ce que ça casse volontairement** : le test
`le_vallon_donne_du_tir_gratuit_et_on_le_mesure` tombera — c'est exactement ce
pour quoi il a été écrit (« si la carte a change, mettre ce test a jour au lieu
de le contourner »).

**Effort** ~20 min + recuisson · **Risque** bas.

---

### B6 — Corriger le critère « berge » : la relation à l'eau est verticale 🟠 P1

**Le fait** : les manchots sont à 13 m au-dessus de leur rive (§2.1).

Le milieu `berge` contraint la distance **en plan** à l'axe (`riviere: [11, 24]`).
Or une berge est définie par sa hauteur au-dessus de la nappe, pas par son
éloignement — le profil de l'eau est déjà calculé station par station, il suffit
de le lire :

```python
"berge": {..., "hauteur_sur_nappe_m": [0.0, 2.5]}
```

Même correction à envisager pour tout milieu dont le nom implique une relation
verticale.

**Effort** ~30 min · **Risque** bas.

---

### B7 — Rendre le streaming utile : cellules de 20 m 🟡 P2

**Le fait** : 78 % de la carte résidente (§4.2).

Passer `92_cellules.py` de 40 à 20 m fait ~192 cellules ; à rayon égal la part
résidente tombe vers 45–55 %. Le nombre d'appels de rendu ne monte pas
proportionnellement — les cellules périphériques sont quasi vides (`cell_x3_z-3`
= 1 primitive, 36 triangles).

À décider avec toi : la granularité est un compromis entre mémoire résidente et
nombre d'entités ECS. Ça se mesure en jeu, une fois le capteur B9 en place —
**dans cet ordre**, sinon on règle à l'aveugle.

**Effort** ~30 min · **Risque** moyen (à mesurer avant/après).

---

### B8 — Produire un LOD par cellule 🟡 P2

Le kit château a déjà ses LOD1/LOD2 (`12_kit_chateau.py:57-69`). Pour le kit
nature, un `Decimate` à 35 % sur la fusion de cellule donne un
`cell_*_render_lod1.gltf`, et le manifeste gagne un champ `render_lod1`.

Dépend d'un système de LOD côté moteur — **à cadrer avec toi avant de cuire**,
sinon c'est 48 fichiers morts de plus, comme le `walkable`.

**Effort** ~2 h · **Risque** moyen · **Dépendance moteur.**

---

### B9 — Supprimer les deux exports morts 🟢 P3

`expedition_vallon.glb` (75,6 Mo) et `expedition_vallon_walkable.glb` (1,3 Mo)
ne sont référencés nulle part (§4.4).

Le second **ne doit pas être supprimé mais assumé** : soit le navmesh se fait et
il sert, soit on retire son export de `91_export.py` avec la raison écrite. Un
fichier cuit pour un consommateur qui n'existe pas est la définition de la dette
silencieuse — et l'audit du 13/08 avait déjà noté cette classe
(« une description n'est pas une preuve : un artefact ne se prouve que par son
consommateur »).

**Effort** ~10 min · **Risque** nul · **Gain** 77 Mo.

---

## 8. Côté moteur — pour ton travail parallèle

Listé sans plan d'exécution : c'est ta moitié.

| # | Sujet | Le fait |
|---|---|---|
| M1 | **Capteur `forgia2_expedition_carte.json`** | 131 capteurs, aucun sur la carte (§6). Sans lui, B7 se règle à l'aveugle et B4 ne se valide pas. |
| M2 | Consommer `colliders_prop_xyzhr` | La hauteur cesse d'être `HAUTEUR_COLLIDER_M = 6.0` en dur (`plugin.rs:69`). |
| M3 | Corriger le commentaire `manifest.rs:151` | Il annonce « troncs, rochers et murs » pour 82 % de troncs (§1.5). |
| M4 | Ouverture de la porte | `porte_village` est lu, la porte est statique (`plugin.rs:46-52`). Dépend de B1. |
| M5 | Navmesh, ou assumer son absence | `walkable` cuit, aucun consommateur ; les bots vont en ligne droite sur une carte à 26 m de falaise. |
| M6 | Cel-shading en Expédition | `toon_config.rs` est enfermé dans `forgia-mode-roguelite` ; le capteur `toon` alerte déjà. |
| M7 | 943 colliders posés d'un coup | `setup_expedition` les spawne tous à l'entrée ; à mesurer après B1, qui va augmenter le compte. |
| M8 | Le rayon de chargement | Le test le valide contre la demi-diagonale, mauvais juge sur une carte rectangulaire (§4.2). |

**Ordre conseillé pour que nos deux moitiés se rejoignent** : M1 d'abord (il
mesure tout le reste), puis B1+B2 sous Blender pendant que tu fais M2, et B4
en dernier — c'est le seul chantier dont le résultat se juge à l'œil.

---

## 9. Porte de sortie — `map-design-intention.md` §5.2

État de la checklist, honnêtement :

- [x] Chaque salle de combat a sa spec — **partielle** : ni archétype, ni durée, ni condition de sortie (§3.1)
- [ ] **Les archétypes peuvent arriver** — non vérifié, aucun archétype n'est assigné
- [ ] **Les archétypes peuvent voir** — ❌ 4 m de tir gratuit × 3 camps
- [x] Les arrivées ennemies sont déclarées et distinctes — 7 par camp, à 6–10 m
- [ ] Aucun endroit accessible au joueur et pas à l'IA — **invérifiable sans navmesh**
- [ ] Couvertures conformes et **bidirectionnelles** — ❌ 15 sur 18, et **aucune n'est solide**
- [x] Chaque salle de combat a ≥ 2 entrées — les camps sont sur le chemin, traversants
- [ ] Profil de portées mesuré vs arsenal — non mesuré sur cette carte
- [ ] **Aucun rôle déclaré sans instance** — ❌ `chicken` déclaré, 0 instance
- [ ] Budget de durée de run — seul le transit est connu (55,2 s de marche)
- [ ] Ce qui varie entre deux runs est écrit — **rien** : la carte est fixe, et aucun document ne dit si c'est voulu

**7 cases sur 11 ouvertes.** Conformément à la règle : on dit **où on en est**,
pas « fini ».

---

## 9 bis. Ce qui a été livré le jour même (2026-08-17)

> État à la clôture de la session. Ce qui est marqué ✅ est **mesuré**, pas
> supposé — chaque nombre vient de la sortie du cuiseur ou du glTF produit.

| # | Chantier | État | Preuve |
|---|---|---|---|
| B1 | Emprises solides de tous les props | ✅ | **1 616 solides contre 943**, en 8 familles ; rayon dérivé de l'AABB placée |
| B2 | Abris = meubles de combat | ✅ | 18/18 posés (contre 15), **tous ≥ 1,95 m**, `casse_la_vue` dérivé |
| B3 | Bloc `defauts` | ✅ | `"defauts": []` — et il a servi : il a révélé 2 faux abris |
| B4 | Couleurs de sommet | ✅ | **632 primitives portent `COLOR_0`, 99,8 %**, variation réelle 0,554→0,961 |
| B5 | Rayon de campement dérivé | ✅ | rayon 10 m lu depuis `enemy_grunt.toml` → **tir gratuit 0,0 m sur les 3 camps** |
| B6 | Critère « berge » vertical | ✅ | `hauteur_sur_nappe_m` ; **11 zones de faune sur 11** (les poules existent) |
| B9 | Exports morts | ✅ | `expedition_vallon.glb` (75,6 Mo) supprimé, zéro consommateur vérifié |
| M2/M3 | Lecteur moteur au nouveau schéma | ✅ | **101 tests verts**, clippy propre, `cargo check -p forgia` passe |
| M1 | Capteur `forgia2_expedition_carte.json` | ✅ | 7 contrôles de santé, verdict pur et testé |
| B7 | Cellules 20 m | ⛔ **retiré, à raison** | la mesure le condamne — cf. §4.2 bis |
| B8 | LOD par cellule | ❌ non fait | dépend d'un système de LOD moteur |

### Trois défauts trouvés *pendant* le chantier, absents de l'audit initial

1. **`COLOR_0` jamais exporté** (§5) — le plus coûteux, et invisible sans mesure.
2. **Le rayon d'arbre naïf rendait la futaie impraticable.** Mesurer l'emprise
   sur la tranche basse attrapait le bas du houppier : rayon **médian 0,72 m**,
   27 % au-delà de 1 m, là où le tronc fait 0,2 m. Corrigé en restreignant la
   mesure aux matières `wood*`, que le kit distingue déjà → médian **0,51 m**,
   max 1,50. **À juger manette en main** : c'est la densité ressentie de la
   forêt qui tranche, pas le tableau (§5.3 de `map-design-intention.md`).
3. **Les ancrages au sol se multipliaient.** Six arbres couvrant le même sommet
   de terrain donnaient 0,68⁶ ≈ 0,10 — un sol de sous-bois à 8 % de luminosité.
   Corrigé en retenant la contribution la plus sombre au lieu de les composer,
   avec plancher. Le contrôle de cohérence du script l'a attrapé au premier jet.

### Un quatrième défaut, trouvé en voulant appliquer B7

**Ma propre recommandation B7 était fausse**, et c'est la mesure qui l'a montrée
— pas une relecture. Voir §4.2 bis : le streaming n'est pas mal réglé, il est
sans objet sur une carte de 172 m de demi-diagonale vue à travers 420 m de
brouillard. Affiner les cellules aurait ajouté des appels de rendu en croyant en
retirer.

C'est la raison d'être de l'ordre « M1 d'abord » : sans capteur, B7 se serait
appliqué, aurait coûté deux recuissons, et aurait dégradé la carte sous couvert
d'optimisation.

### L'épisode multi-terminal, pour mémoire

Le manifeste a changé de schéma (`colliders_cylindre_xyzr` →
`colliders_prop_xyzhr`, `abris_xy` → `abris`), et le Rust n'a pas pu être
compilé pendant une heure : `crates/forgia-anim-debug/src/anim_sensor.rs` était
rouge depuis le 16/08 22:40 (deux appels d'API Bevy 0.18 périmés), et c'est une
dépendance directe de `forgia-mode-expedition`.

`multi-terminal-coordination.md` §3 règle 2 interdit de patcher l'erreur
d'autrui. La coordination a été demandée, l'autre terminal a corrigé à 13:21, et
le chantier a repris sans conflit. **La règle a fonctionné exactement comme
prévu** : le coût a été une heure d'attente, contre un conflit de fusion certain.

---

## 10. Ce que cet audit ne couvre pas

Le **son** · l'**occlusion et le budget GPU réels** (mesurés en triangles et
primitives, jamais en ms) · le **pathfinding** · la **lisibilité en mouvement** ·
le passage à l'**art pass** · la **rejouabilité** (une seule disposition).

Et surtout : **rien ici ne remplace la manette**. `map-design-intention.md` §5.3
réserve au playtest la taille ressentie des salles, la lisibilité des repères, le
plaisir du chemin et le rythme perçu. Les onze chantiers ci-dessus corrigent des
**contrats**, pas des sensations.

---

## Cross-refs

- `.claude/rules/map-design-intention.md` — §1 spec de combat, §2.2 vision, §5.2 porte de sortie
- `.claude/rules/map-design-patterns.md` — §11 couverture binaire, §13 « zéro mesuré n'est pas vert »
- `.claude/rules/spawn-clearance.md` — §4 « l'emprise, c'est l'emprise »
- `.claude/rules/observability-required.md` — §6 de cet audit
- `[[reference_vallon_carte_expedition]]` — pipeline, dérivations, contrat d'export
- `tools/blender/expedition/vallon.py` · `91_export.py` · `92_cellules.py`
- `crates/forgia-mode-expedition/src/manifest.rs` · `plugin.rs` · `matiere.rs`
