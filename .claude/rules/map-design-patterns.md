# Patterns de création de carte (Forgia) — À APPLIQUER, PAS À CONSULTER

> **Ce fichier traite le COMMENT (géométrie).** Le QUOI — spec de combat,
> composition d'une salle, rythme d'une run, porte de sortie — est dans
> [`map-design-intention.md`](map-design-intention.md), et il vient **avant**.
> Déclencheur : la commande `/map`.

> **Pourquoi ce fichier existe.** Le banc Arena Test a accumulé 156 critères de
> recherche et une grille de 39 points dans une story de 700 lignes — donc jamais
> appliqués à 100 %, ré-appris partiellement à chaque passe, et 35 défauts corrigés
> un par un. Ce qu'on ne peut pas garder en tête n'est pas une méthode.
>
> **14 patterns, 4 familles.** Chaque chiffre est dérivé d'une métrique mesurée
> dans le moteur ou d'une source nommée — aucun n'est choisi. Les numéros `[n]`
> renvoient au [registre des défauts](../../docs/audits/arena-test-registre-defauts.md) :
> un pattern ne vaut que s'il aurait empêché une ligne réelle.

**Métriques de référence** (mesurées, non négociables) : capsule 2,0 m × 0,6 m —
donc **disque** en plan · œil **1,70 m** · **pas d'accroupissement** · saut
**1,174 m** · portée de saut 4,69 m (marche) / 7,04 m (sprint) · pas de dégâts de
chute, pas de mantle · pente montable **50°** · dégâts pleins jusqu'à **30 m**
puis −40 % · module de trame **2 m**.

---

## I. MESURER — la forme avant le nombre

> Quatre défauts sur dix venaient d'une mesure juste appliquée à la mauvaise
> géométrie. Le nombre était bon ; la forme était fausse.

**1. Le joueur est un disque.** Toute mesure de passage est une **distance**
segment↔obstacle, jamais une AABB gonflée du rayon. Le coin d'une boîte gonflée
est à `r·√2` — **+41 % dans les diagonales**. Forme canonique déjà dans le
codebase : `forgia_stage::layout::is_in_player_boss_corridor`. `[1, 15, 17]`

**2. La trame se juge aux arêtes, jamais au centre.** Un pavé de côté `M` posé sur
une trame de pas `M` a forcément un centre à un multiple **impair** de `M/2`.
Corollaire dur : **ne jamais re-snapper un centre déjà exact** — l'arrondi le
déplace de `M/2` une fois sur deux (et `round(1.5) == 2` en Python rend même le
sens incohérent). `[19, 35]`

**3. Dilater une trame coûte une demi-diagonale, pas un demi-côté.** Pour qu'aucune
cellule ne morde dans une zone à préserver : `M·√2/2`. **Mais seulement là où il
n'y a pas de marge de design** — l'appliquer à une salle qui a déjà 1 m de marge
voulue creuse d'autant ses bords de plateforme. `[13, 20]`

**4. Une marche est un écart entre deux appuis, pas une hauteur depuis le sol.**
Une caisse de 2 m atteinte depuis une caisse de 1 m est une marche de **1 m**.
Franchissable si `≤ 1,174 m`. `[2, 4]`

---

## II. CONSTRUIRE — dériver, jamais déclarer deux fois

> **Un littéral ne peut pas porter un invariant.** Huit défauts viennent d'une même
> grandeur déclarée deux fois — emprise nominale d'un côté, forme réellement bâtie
> de l'autre — qui finissent toujours par divorcer.

**5. Le sol se déduit de la forme creusée, jamais de l'emprise nominale.** Si le
creusement ouvre `emprise + m` et que la plateforme couvre `emprise`, il reste un
anneau de `m` sans sol — 82 m² de bords de chute sur 4 salles. Élargir le
creusement **aggrave** ; c'est la plateforme qui doit suivre l'ouverture.
`[7, 10, 28]`

**6. Une emprise, un niveau de sol.** Deux salles adjacentes à hauteurs différentes
ne peuvent pas se chevaucher en plan : deux sols à deux altitudes sur le même XZ
est une contradiction, pas un détail. C'est la **transition** qui occupe
l'entre-deux. `[25, 26]`

**7. Une transition déclarée est une transition dégagée.** Une rampe doit être
**creusée dans ce qu'elle traverse**. Une rampe posée dans un socle plein n'est
pas une rampe, c'est une marche — et un contrôle qui vérifie qu'elle est
*déclarée* ne le voit jamais. `[26, 27]`

**8. Un couloir se définit par son canal utile, pas par son axe.** La largeur
déclarée est un **contrat** : rien n'entre dedans, **tangence comprise**. Un
trajet qui occupe toute la section d'une salle n'est pas un couloir. `[11, 12, 21,
22, 23, 24]`

---

## III. COMPOSER — le combat avant la forme

**9. Chaque voie a une portée, et on la MESURE.** Bandes d'engagement (TF2 via
Level Design Book) : proche ≤ **6,5 m** · moyen ≤ **26 m** · long ≤ **52 m**.
Doctrine Treyarch : *« every lane needs a purpose »*, extérieures longues,
centrale courte, positions de force **par paires en miroir**.
→ Une carte n'a pas « une taille », elle a un **profil de portées** qui doit
correspondre à l'arsenal. Notre shrink 220×144 → 120×60 a supprimé **toute** ligne
au-delà de 30 m alors que l'arsenal contient un sniper 300 m. `[33]`

**10. L'inégalité de crête — un point haut ne se conteste pas avec des caisses.**
Pour qu'un obstacle à distance `x` coupe la vue d'un tireur perché à `h` sur une
cible à portée `span` :

```
c ≥ h·(1 − x/span) + œil        →  à mi-portée : c ≥ h/2 + 1,70
réciproque : h ≤ 2·(c − œil)    →  ce qu'une couverture de hauteur c peut contester
```

Dimensionner le point haut **depuis** la couverture disponible, pas l'inverse.
Implémenté : `cover_height_to_break_sight` / `max_contestable_height`.

**11. Sans accroupissement, la couverture est binaire.** L'œil est à 1,70 m et il
n'y a pas de crouch — la taxonomie haute/basse/nulle ne transpose pas :

| Hauteur | Fonction réelle |
|---|---|
| ≤ 1,17 m | franchissable au saut → **traversée**, pas couverture |
| 1,2 – 1,7 m | masque le corps, **pas la vue** → ne sert à rien |
| ≥ 1,8 m | **casse la ligne de vue**, au même niveau seulement |

Espacement 3–10 m (Watch Dogs, Gears), 10 m maximum.

**12. 3 à 4 étranglements, et aucun point ne les couvre tous.** Level Design Book.
Densité **inverse à la portée** de la voie : une voie de tireur d'élite encombrée
n'est plus une voie longue. Échec type nommé : Berlin (CoD Vanguard) — trop de
lignes ouvertes, le sniper domine tout, la partie devient boucherie ou statu quo.

---

## IV. VÉRIFIER — un contrôle doit dire combien il a mesuré

> La classe la plus coûteuse : un contrôle qui passe à vide ne coûte pas seulement
> son propre défaut, il **cache** tous ceux qu'il devait attraper, et fait
> consigner de faux « verts » comme des preuves.

**13. Zéro mesuré n'est pas vert, c'est aveugle.** Tout contrôle expose la taille
de son échantillon. Un seuil qui n'a rien à mesurer renvoie `info` + « aveugle »,
jamais `ok`. Et **ne jamais regagner le vert en abaissant le seuil**. `[14, 18,
26, 34, 35]`

**14. Aucune source auto-référente, aucun dossier périmé.** Un générateur ne lit
**jamais** sa propre sortie (3 corruptions en cascade). Un dossier d'audit se
**régénère juste avant** la passe, et l'outil qui le produit ne porte **aucun
chiffre codé en dur** — les 5 nôtres étaient tous faux, et 78 constats ont été
produits sur une carte qui n'existait plus. `[6, 29, 32]`

---

## État réel — écrire un pattern ne l'applique pas

> **Le tableau qui compte.** Un pattern noté « tenu » sans que le code le fasse est
> exactement le capteur menteur que la famille IV interdit. Trois états seulement :
> **TENU** = le code le garantit ou le mesure aujourd'hui · **CONTRÔLÉ** = un
> contrôle l'attrape après coup, il n'est pas impossible à violer · **ÉCRIT** =
> aucun code derrière, c'est une intention.

| # | Pattern | État | Preuve / ce qui manque |
|---|---|---|---|
| 1 | Le joueur est un disque | **TENU** | `segment_aabb_distance_2d` + test du coin diagonal (√2 dispo, 1,2 passe / 1,5 bloque) |
| 2 | La trame se juge aux arêtes | **PARTIEL** | `snap()` supprimé côté générateur ✅ · mais le contrôle « hors trame » juge encore le **centre** → 52 faux positifs `[35]` |
| 3 | Dilater = demi-diagonale | **TENU** | `CELL_HALF_DIAG` côté routes, marge des salles préservée |
| 4 | Une marche est un écart | **TENU** | `max_traversal_step` — mais capteur **aveugle** (0 bloc `traversal` dans la carte) |
| 5 | Le sol se déduit du creusé | **TENU** | `plateforme = (emprise ⊕ ROOM_CARVE_MARGIN) − socles plus hauts − rampes hors salle` · marge = **source unique** lue aussi par le creusement · bords **99 → 61 m²** |
| 6 | Une emprise, un niveau | **TENU** | chaîne est **dérivée** (`_lay_out_east_chain`) : cour x[3,25] · rampe · pont x[29,47] · rampe · chapelle x[51,73]. Les centres ne sont plus écrits |
| 7 | Transition dégagée | **TENU** | la rampe occupe **l'intervalle entre deux emprises** — elle ne peut plus être dans un socle. Course dérivée de `MAX_SLOPE_DEG`, arrondie au module supérieur → 14° |
| 8 | Le canal utile | **CONTRÔLÉ** | 0 obstruction mesurée ✅ — mais obtenue en **corrigeant des littéraux à la main**, pas par construction |
| 9 | Chaque voie a une portée | **CONTRÔLÉ** | profil mesuré par salle · **aucun seuil** : 0 % de lignes > 30 m n'a déclenché aucune alerte |
| 10 | Inégalité de crête | **TENU** | `cover_height_to_break_sight` / `max_contestable_height` / `worst_overlook_pct`, alerte > 85 % |
| 11 | Couverture binaire | **TENU** (étiquetage) · **ÉCRIT** (densité) | le **rôle se dérive de la hauteur** dans le générateur, test `cover_role_matches_the_declared_band` · mesure : **16 blocs `cover` étaient des murs de 3-6 m, zéro dans la bande 1,8-2,8** → la carte a en réalité **0 couverture** et aucune mécanique de peek · `covers_expected(aire, espacement)` dérive le compte, mais rien ne l'impose encore |
| 12 | 3-4 étranglements | **ÉCRIT** | jamais compté |
| 13 | Zéro mesuré = aveugle | **TENU** | appliqué aux 2 capteurs concernés (traversal, routes) |
| 14 | Pas de source auto-référente | **PARTIEL** | le générateur ne lit plus sa sortie ✅ · le dossier se régénère **à la main**, rien ne l'impose |

**Bilan : 8 TENU, 2 PARTIEL, 2 CONTRÔLÉ, 2 ÉCRIT.**

La famille II est passée d'intention à construction en une passe, en suivant les
normes du marché : **emprises disjointes** (deux sols à deux altitudes sur le même
XZ est une contradiction), **transition = son propre espace**, et un pas de plus de
**45 cm** (`MaxStepHeight` d'Unreal) exige une rampe, pas un saut. Coût assumé :
le monde passe de 120 à **152 m** en X — supprimer 6 m de chevauchement allonge la
chaîne, et l'enceinte suit le contenu. **Aucune taille de salle n'a bougé.**

Effet de bord instructif : rendre les emprises disjointes a d'abord fait **monter**
les bords de chute de 82 à 99 m² — le chevauchement les *masquait*. C'est le sol
dérivé (pattern 5) qui les a ramenés à **61 m²**. Les deux patterns ne se
livrent pas séparément.

## Ce que ces patterns ne couvrent PAS

Honnêteté de portée — aucun pattern ci-dessus ne traite : le **son**, l'**occlusion
et le budget de rendu**, le **pathfinding des ennemis**, la **coop à 3**, la
**lisibilité en mouvement**, le passage à l'**art pass**, le **streaming**, la
**rejouabilité** d'un roguelite (une seule disposition). Ils couvrent la
construction géométrique d'un blockout, rien de plus.

---

## Cross-refs

- [Registre des 35 défauts](../../docs/audits/arena-test-registre-defauts.md) — la matière première
- [story-667](../../docs/stories/story-667-arena-test-blockout-bench.md) — le détail sourcé et l'historique des passes
- `no-hardcode.md` — les chiffres vivent en couche definition
- `concept-first.md` §3 étape 0 — data ou code, avant tout Edit
