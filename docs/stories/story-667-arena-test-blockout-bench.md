# story-667 — Arena Test : le banc de blockout d'arène

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_arena_test.json`, fichier `arena_test.rs`, symbole `GameMode`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED
> **État d'origine (périmé, cf bandeau)** : IN_PROGRESS (implémentation livrée, validation runtime en attente)
**Niveau BMAD** : Enterprise (nouveau `GameMode` + 6 fichiers cross-crate)
**Date** : 2026-07-27
**Related** : story-625 (coquille arène authored), story-665 (éditeur in-game),
story-483/485 (stage-arena, palette de modules)

---

## Demande

> « Pour ne pas perdre le travail actuel, on va créer un nouvel onglet dans le
> menu d'accueil "Arena Test" et on va suivre ton process. Inspire toi de High on
> Life, Gunfire Reborn et Overwatch pour préparer le terrain. »

Portée arbitrée avec l'utilisateur : **une seule arène, itérée à fond** (et non le
parcours multi-salles). C'est l'ordre industriel : on prototype une salle jusqu'à
l'excellence avant de bâtir le graphe qui en enchaîne quatre.

---

## 1. Concept-First

### Étape 0 — Data ou code ?

**Definition à 90 %.** La géométrie d'une arène est de la donnée, pas de la
logique. Le Rust ne doit contenir aucune coordonnée : il instancie ce que le TOML
décrit. La couche `framework` se limite au variant `GameMode`, au chargement et
au rendu des primitives.

### Étape 1 — Hypothèses concurrentes

| # | Hypothèse | Verdict |
|---|---|---|
| a | Il faut un nouveau pipeline de chargement d'arène | ❌ **Falsifiée** — `forgia-stage` (story-625) charge déjà une arène authored depuis un TOML. Mais il est couplé aux GLB Inferno/KayKit finis, donc inadapté au **greybox** : on ne juge pas une forme à travers de la décoration. |
| b | On peut réutiliser `GameMode::Fps` (arène de cibles existante) | ❌ **Falsifiée** — le mode tire `forgia-mode-fps-arena` (vagues, cubes) et n'est plus dans le menu. Le réveiller mélangerait deux intentions. |
| c | Un mode autonome greybox, data-driven, isolé | ✅ **Retenue** — précédents exacts : `CyberCity` et `CastleHub`. Zéro impact sur ce qui tourne, ce qui est la demande explicite. |

### Étape 2 — Cartographie

Metrics **mesurées**, pas choisies (c'est l'étape que tout projet saute) :

| Mesure | Valeur | Source |
|---|---|---|
| Vitesse marche / sprint | 6.5 / 9.75 m/s | `player_movement.toml:11,14` |
| Hauteur de saut | **1.174 m** | dérivée v²/2g = 6.5²/(2×18) |
| Temps d'air | 0.722 s | 2v/g |
| Portée de saut marche / sprint | 4.69 / 7.04 m | vitesse × temps d'air |
| Gabarit joueur | 2.0 m × 0.6 m | `forgia-player/src/lib.rs:379` `capsule_y(0.7, 0.3)` |
| Dégâts pleins Pépin / Bourrasque | 0 – **30 m** | `viewmodel_arena.toml` `damage_falloff_start` |
| Sans falloff (Lenoir) | 0 – 300 m | idem |

**Constat majeur** : la bande de combat utile est 8–30 m, alors que
`crypts_of_anvil` fait `arena_extent_m = 90` → **156 à 180 m de large**, 18 s de
traversée en sprint. 90 % de sa surface est hors de portée utile de trois armes
sur quatre. Ce n'est pas corrigé ici (`no-speculative-fix` : aucun symptôme n'a
été nommé sur cette arène, et l'isolement est justement la demande) — le banc
naît avec `extent_m = 30` (52 m entre faces, 5 s de traversée).

### Étape 3 — Producteur / consommateurs

| Rôle | Où | Timing |
|---|---|---|
| **Producteur** | `assets/genomes/arena_test.toml` — metrics, grille, palette, arène, blockout | `hot-reload` (poll mtime 1 Hz) |
| **Producteur (arbitre)** | `assets/genomes/player_movement.toml` — locomotion réelle | lu au chargement pour recalculer les dérivées |
| **Consommateur** | `forgia-game/src/arena_test.rs` — construction des primitives + colliders | `OnEnter(GameMode::ArenaTest)` + reconstruction sur révision |
| **Consommateur** | `forgia-ui/src/lib.rs` — onglet + bouton de lancement | `EguiPrimaryContextPass` (menu) |
| **Consommateur** | `forgia-fps/src/lib.rs` — `fps_combat_mode` autorise le tir | `Update` / `FixedUpdate` |
| **Capteur** | `forgia2_arena_test.json` | 1 Hz, `GameSet::Sensors` |

Réseau : `L` (banc local). Script : `int`.

### Étape 4 — Hot path check

Aucun système chaud. La construction est ponctuelle (`OnEnter` + rechargement à
chaud). Points traités quand même :

- **Un matériau par rôle**, pas par pièce — chaque matériau distinct est une
  spécialisation de pipeline PBR à compiler (leçon `reference_pbr_pipeline_warmup_frustum_trap`).
- Le poll disque est à 1 Hz, pas par frame.
- La grille passe par `Gizmos` (≈ 82 lignes) et se coupe par bascule.
- Le capteur est à 1 Hz via `sensor_io::enqueue` (I/O borné, hors frame).

### Étape 5 — Scale-up BMAD

6 fichiers, 4 crates, nouveau variant d'état partagé → **Enterprise**. Story
obligatoire (ce document).

---

## 2. Implémentation

### 2.0 Scénarios conservés et comparables

Le banc ne remplace jamais un blockout validé par le suivant. `F7` alterne entre
les genomes déclarés dans `arena_test.rs` :

- `arena_test.toml` : **Le Creuset**, benchmark symétrique trois voies ;
- `arena_test_crypte_vertical.toml` : **Crypte verticale**, hypothèse PvE
  roguelite asymétrique (rampe sûre, tunnel de flanc, corniche de récompense,
  convergence élite).

Chaque scénario reste data-driven et hot-reloadable. La Crypte est protégée par
un test de contrat : TOML lisible, spawn hors du sol, escalier sous la hauteur de
saut et rampes sous 35°. Elle ne remplace pas encore un stage final ; elle doit
d'abord passer les playtests combat et bots avant d'être promue vers
`roguelite_stages.toml`.

| Fichier | Nature | Contenu |
|---|---|---|
| `crates/forgia-core/src/lib.rs` | +8 l. | variant `GameMode::ArenaTest` |
| `assets/genomes/arena_test.toml` | **nouveau, 361 l.** | metrics, grille, palette, arène « Le Creuset », blockout complet |
| `crates/forgia-game/src/arena_test.rs` | **nouveau, ~880 l.** | chargement + hot-reload, construction des primitives, spawn joueur, grille, encart metrics, capteur, 9 tests purs |
| `crates/forgia-game/src/lib.rs` | +10 l. | déclaration du module + `add_plugins` |
| `crates/forgia-ui/src/lib.rs` | +60 l. | `MenuPage::ArenaTest`, libellés, section + bouton |
| `crates/forgia-fps/src/lib.rs` | +16 l. / −4 | condition `fps_combat_mode` en source unique |
| `docs/observability/SENSOR_REGISTRY.md` | +1 l. | déclaration du capteur (gate `sensor-audit`) |

### 2.1 « Le Creuset » — l'intention

Synthèse des trois références demandées :

- **Overwatch** → le triangle : à tout instant trois lectures — la fosse, l'anneau
  haut, la route de contournement derrière les plateformes.
- **Gunfire Reborn** → cuvette de brawl centrale dominée par un anneau surélevé.
- **High on Life** → la couleur dit la fonction : l'ocre se grimpe, le bleu
  domine, le gris est du décor. Aucune UI pour l'expliquer.

| Niveau | Alt. | Rôle | Engagement |
|---|---|---|---|
| Fosse | 0 m | brawl, Ø 33 m libre | 5–12 m |
| Anneau (6 plateformes) | +3 m | voie principale, intervalles = flancs | 15–25 m |
| Perchoirs (×2 opposés) | +6 m | la seule ligne longue | 40 m |

Deux vocabulaires d'accès pour la même destination — **3 rampes** (route sûre,
≈ 21°) et **3 chaînes de saut** (marches de 1,0 m, écarts de 1,5 m). C'est ce
contraste qui crée de la maîtrise plutôt qu'un simple déplacement.

### 2.2 Le contrat de vérité des metrics

Les metrics déclarées dans le TOML sont **recalculées au chargement** depuis
`player_movement.toml` et comparées à 2 % près. Tout écart part dans
`forgia2_arena_test.json::metrics_drift` en `warn`, s'affiche dans l'encart et
part en `warn!` dans le log.

Motif : une arène dimensionnée sur des metrics périmées est fausse, et la panne
est **silencieuse** — le level design dérive sans que rien ne casse.

### 2.3 Les quatre pièges attrapés avant livraison

Trois trouvés à l'auto-relecture, un quatrième par les tests. Tous invisibles à la
compilation, tous lisibles en jeu comme « un bug de collision » :

1. **Sol taillé sur l'apothème.** Un hexagone de rayon circonscrit 30 touche ses
   murs à 26 (apothème) mais ses **sommets sont à 30**. Un sol de rayon 26 laisse
   six coins ouverts où le joueur tombe hors de la map — exactement le trou de
   collision du Hall relevé par l'audit du 2026-07-24. Sol porté à 30 (il déborde
   derrière les murs, invisible de l'intérieur) **et** contrôle de couverture
   ajouté au capteur.
2. **Spawn encastré.** `spawn_pos.y = 0.5` alors que la capsule joueur a une
   demi-hauteur de 1,0 m : le joueur naissait un demi-mètre dans le sol. Porté à
   1,5, et un test verrouille `spawn_y ≥ player_height/2`.
3. **Double construction à l'entrée.** La révision construite était suivie par un
   `Local<u32>` partant de 0, alors que `OnEnter` avait déjà construit la
   révision 1 : le banc se reconstruisait entièrement à la première frame. La
   révision est désormais portée par la télémétrie, remise à zéro au nettoyage.

4. **Marche confondue avec hauteur de volume** — trouvé par le test, pas par la
   relecture. Le contrôle prenait `size.y` d'une pièce de traversée comme hauteur
   de marche : la caisse de 2 m de la chaîne de saut le faisait crier, alors
   qu'on y monte depuis celle de 1 m. Une **caisse n'est pas une marche** — la
   marche est l'écart entre deux surfaces successives. Extrait en
   `max_traversal_step()` (sommets + sol, triés, plus grand écart consécutif),
   avec sa limite écrite noir sur blanc : le saut final vers la plateforme n'est
   pas couvert, il reste au playtest.

Le capteur vérifie donc quatre pannes muettes : genome illisible / arène vide
(`critical`), sol qui n'atteint pas les sommets (`warn`), marche de traversée
plus haute que le saut (`warn`), metrics périmées (`warn`).

**Leçon de process** : les trois premiers défauts ont été trouvés en relisant, le
quatrième par un test qui, lui, était faux. Un contrôle de santé écrit en même
temps que le code qu'il surveille hérite de ses angles morts — il faut le
confronter aux données réelles, pas seulement le compiler.

### 2.4 Conventions d'auteur

- `pos` d'un pavé = **centre de l'empreinte au sol**, la boîte monte de `pos.y` à
  `pos.y + size[1]`. Un designer pense « une couverture d'1 m ici », pas en
  demi-extents.
- Une rampe se décrit par ses **deux extrémités** (`from`/`to` + largeur). Le code
  en déduit longueur, lacet et pente. Personne ne compose un quaternion à la main.

---

## 3. Critères d'acceptation

- [x] Onglet « 📐 Arena Test » présent dans la sidebar du menu d'accueil
- [x] Le bouton entre dans `GameMode::ArenaTest` + `AppMode::InGame`
- [x] Zéro coordonnée d'arène dans le Rust (tout dans `arena_test.toml`)
- [x] Aucune arène existante modifiée (`crypts_of_anvil`, `forge_sanctum` intacts)
- [x] Metrics recalculées et confrontées à `player_movement.toml`
- [x] Capteur `forgia2_arena_test.json` déclaré au registre, avec `next_step`
- [x] Tests purs (18) : dérivées, dérive, pose de rampe, enceinte selon la forme,
      sélection de voie, marches, santé, inégalité de crête, hauteur contestable,
      domination, **+ trois tests sur le TOML livré — sol débordant l'enceinte,
      marches franchissables, rampes montables, spawn hors du sol, trois voies aux
      portées distinctes, positions de force appariées en miroir**
- [x] `cargo check -p forgia-game` — 0 erreur
- [x] `cargo clippy` (vrai cargo, 4 crates) — 0 warning
- [x] `cargo xtask sensor-audit` — OK, 124 déclarés / 124 produits, 0 orphelin
- [ ] **Validation runtime** — l'arène apparaît, se parcourt, se tire dessus
- [ ] Rechargement à chaud du TOML vérifié en jeu

---

## 3bis. Passe « logiques mathématiques » (2026-07-27, recherche externe)

Recherche menée sur les cadres quantitatifs réels du design d'arène. Ce qui en
ressort d'exploitable, confronté à nos propres mesures.

### Ce que la littérature donne de chiffré

| Source | Métrique | Valeur |
|---|---|---|
| Team Fortress 2 (Valve, via Level Design Book) | bandes d'engagement | proche ≤ 6,5 m · moyen ≤ 26 m · long ≤ 52 m |
| Uncharted 4 / Naughty Dog | hauteurs de couverture | haute ≥ 1,75 m · basse 1,0–1,25 m · non-couverture ≤ 0,5 m |
| Watch Dogs, Gears of War | espacement des couvertures | 3 à 10 m, 10 m maximum |
| Level Design Book | points d'étranglement | **3 à 4 par carte**, un par voie, *impossible à couvrir tous depuis un seul point* |
| Level Design Book | largeur de couloir | ≥ 2× la largeur du joueur ; 4 m pour trois passants |
| Level Design Book | pente d'escalier | 30–35° (arctan 7/11 = 32°) |
| Epic Games | préférence | *low cover is better than tall cover* en multijoueur |
| arXiv 2605.30570 | qualité de carte | matrice de visibilité, maxima locaux et leur écartement, symétrie, boucles ; bonnes cartes = **longues boucles ou grande salle centrale**, mauvaises = bruit et culs-de-sac |

### Ce que ça valide de notre arène

- **La taille est juste.** 52 m entre faces = exactement le plafond « longue
  portée » de TF2, et notre chute de dégâts à 30 m tombe dans leur bande
  « moyenne » (26 m). Deux référentiels indépendants concordent.
- **L'espacement des couvertures** (~6 m) est au centre de la bande 3–10 m.
- **La topologie** — grande salle centrale + boucle périphérique, zéro cul-de-sac
  — correspond exactement à ce que le papier arXiv classe comme bonne carte.
- **Les couloirs** : rampes à 4 m, intervalles de flanc à ~4,8 m. ✓

### Ce que ça invalide — quatre constats

**1. Sans accroupissement, la taxonomie de couverture ne transpose pas.**
Forgia n'a pas de crouch (vérifié : aucune occurrence dans `forgia-player` ni
`forgia-input`). L'œil est à **1,70 m** (capsule centre 1,0 + caméra +0,7,
`forgia-player/src/lib.rs:426`). Une couverture « basse » à 1,0 m ne cache donc
**rien** : elle masque le bas du corps, pas la ligne de vue. Le vocabulaire utile
en Forgia est binaire :

| Hauteur | Fonction réelle |
|---|---|
| ≤ 1,17 m | franchissable au saut → traversée, pas couverture |
| 1,2 – 1,7 m | masque le corps, pas la vue |
| ≥ 1,8 m | **casse la ligne de vue — au même niveau seulement** |

Nos 8 couvertures basses à 1,00 m sont pile à la hauteur de saut : ni marche
franche, ni couverture. Elles ne servent à rien.

**2. L'inégalité de crête condamne les perchoirs.** Le résultat le plus utile
n'est pas une bonne pratique mais une relation. Pour qu'un obstacle placé à la
distance `x` coupe la vue d'un tireur perché à `h` sur une cible à la portée
`span` :

```
c ≥ h·(1 − x/span) + œil
```

À mi-portée : `c ≥ h/2 + 1,7`. Inversement, une couverture de hauteur `c` ne
conteste qu'un point haut de `h ≤ 2·(c − 1,7)`.

Appliqué à « Le Creuset » : perchoirs à **+6 m**, fosse à 20 m, couverture la plus
haute **2,0 m** → contestable jusqu'à **0,6 m**. Il faudrait des obstacles de
**4,7 m** à mi-portée. Nos perchoirs ne sont pas des positions à conquérir, ce
sont des acquis. Un point haut ne se conteste pas avec des caisses : il se
conteste avec de l'architecture, ou en descendant le point haut.

**3. Six points d'étranglement au lieu de 3–4**, et surtout : la règle « aucun
point ne doit tous les couvrir » est violée par les deux perchoirs.

**4. Symétrie d'ordre 6 = aucun repère.** Six plateformes identiques : le joueur
ne sait pas où il regarde. Overwatch et High on Life reposent tous deux sur la
différenciation des zones pour l'orientation.

Bonus : le contrôleur accepte **50°** de pente (`max_slope_climb_angle`,
`forgia-player/src/lib.rs:398`). Nos rampes à 21° sont timides — 32° (norme
escalier) ramènerait leur emprise de 8 m à 4,8 m et rendrait 3 m de fosse par
rampe.

### Ce qui a été implémenté

La relation ci-dessus est devenue du code mesurable, pas une note de doc :

| Fonction | Rôle |
|---|---|
| `cover_height_to_break_sight(h, span, x, œil)` | l'inégalité de crête |
| `max_contestable_height(c, œil)` | sa réciproque — dimensionner le point haut **depuis** la couverture disponible |
| `box_support(size, yaw, n)` | fonction d'appui d'un pavé orienté |
| `pit_free_radius(platforms)` | rayon de fosse **dérivé**, jamais déclaré |
| `overlook_fraction(...)` | part de la fosse dominée par un point haut, échantillonnage polaire |

Exposé dans `forgia2_arena_test.json::worst_overlook_pct`, affiché dans l'encart
(F4), et **alerte `warn` au-delà de 85 %**. Un test mesure la valeur du blockout
livré et échouera le jour où le layout sera corrigé — ce qui est le signal
attendu, pas une régression.

## 3ter. « Le Creuset » v2 — le layout corrigé (2026-07-27)

Demande : *« ajouter des hauteurs, des paliers, des passages, avec longue et
courte portée »*. Traité en même temps que la correction de la domination, parce
que c'est la **même** opération : ce qui casse une ligne de vue, c'est du volume.

### Deux défauts corrigés dans la métrique elle-même

Avant de bâtir dessus, l'analyse de domination avait deux biais :

1. **Occultants incomplets** — seules les pièces marquées « couverture » étaient
   comptées. Une plateforme de 3 m coupe pourtant une visée exactement comme un
   muret. Désormais **toute** géométrie solide occulte, disques compris.
2. **Échantillonnage biaisé** — les anneaux de rayon régulier donnaient le même
   poids à un anneau intérieur minuscule qu'à un anneau extérieur large, donc un
   massif central gonflait artificiellement la protection. Corrigé en √, et les
   points tombant dans un volume solide sont écartés (ce n'est pas du sol).

### Le layout

| Niveau | Alt. | Contenu | Portée |
|---|---|---|---|
| Fosse | 0 | sol, couvertures 1,8 et 2,4 m | moyenne (12–25 m) |
| Passages couverts | 0 | sous 2 plateformes sur piliers, plafond 2,4 m | **courte** (7 m) |
| Paliers | +1,1 / +2,2 | chaînes de sauts (marches de 1,1 m) | — |
| Anneau | +3,0 | 6 plateformes, 4 pleines / 2 sur piliers | moyenne |
| Massif central | +3,5 | praticable, 2 rampes + 1 chaîne | courte autour |
| Bastions | +4,5 à +6,0 | 6 volumes, **non praticables** | découpent tout |
| Couronne | +5,0 | repère central, non praticable | — |
| Crêtes | +5,0 | 2 en vis-à-vis | **longue** (40 m) |

**Six niveaux praticables**, contre trois en v1.

### Ce que les chiffres disent

| Mesure | v1 | v2 |
|---|---|---|
| Domination du point haut le plus fort | **96,9 %** | **38,5 %** |
| Domination de l'autre crête | ~97 % | 33,0 % |
| Niveaux praticables | 3 | 6 |
| Liaisons fosse ↔ anneau extérieur | 6 + 6 = 12 | **4** (règle 3–4 ✓) |
| Structure la plus haute | 2,0 m | 6,0 m |
| Requis à mi-portée pour contester | 4,7 m | 4,2 m |
| Marche max d'une chaîne | 1,0 m | 1,1 m (saut 1,174) |
| Rampe la plus raide | 20,6° | 25,0° (norme 30–35°) |

Les bastions ont été dimensionnés **par l'inégalité**, pas au jugé : à mi-portée
d'une crête à +5, il faut 5/2 + 1,7 = **4,2 m**. D'où 4,5 m au minimum. Leurs
hauteurs varient (4,5 / 5,2 / 4,5 / 6,0 / 4,5 / 5,2) pour donner une ligne
d'horizon asymétrique — réponse au reproche « symétrie d'ordre 6 = aucun repère ».

Le réglage n'a pas été deviné : balayage sur le nombre, le rayon et la largeur des
bastions, cible 30–45 %.

| Config | Domination |
|---|---|
| 6 bastions, r 11, l 5,5 | 22,6 % (trop aveugle) |
| 6 bastions, r 12,5, l 4,5 | 31,5 % |
| 5 bastions, r 12, l 4,5 | 44,6 % |
| **6 bastions, r 12,5, l 4,0** | **38,5 %** ← retenu |

### L'encadrement mécanique

Le test ne vérifie plus une valeur, il vérifie un **intervalle** : un point haut
doit voir entre 15 % et 85 % de la fosse. En dessous c'est un piège où personne
ne monte, au-dessus c'est un acquis. Les deux échecs sont muets en jeu.

S'y ajoute une contre-épreuve : il doit exister dans l'arène une structure au
moins aussi haute que ce que l'inégalité exige. Sans elle, une domination basse
ne serait qu'un accident de placement.

## 3quater. « Le Creuset » v3 — carte trois voies (2026-07-27)

Demande : *« agrandir vers une carte type Call of Duty, attention à la densité,
de vraies lignes pour tous les types de combat, toucher toutes les règles »*.

### La doctrine, sourcée

Treyarch (Matt Scronce, design director) et David Vonderhaar :

- deux voies extérieures + une centrale relient les deux camps ;
- **« Every lane needs a purpose »** — chaque voie a sa portée : extérieures
  longues (tireur d'élite), centrale courte (fusil à pompe, pistolet-mitrailleur) ;
- les positions de force vont **par paires en miroir** — « si cette voie a une
  position de force, il en faut une opposée » ;
- **« au plus trois décisions »** : la carte se lit d'un coup d'œil au spawn ;
- **échec type** — Berlin (CoD Vanguard) : trop de lignes ouvertes, le tireur
  d'élite domine toutes les routes, la partie devient boucherie ou statu quo.

### La carte

**80 × 56 m** (4 480 m², ×1,9 vs v2), symétrie **miroir sur X** — les deux camps
voient la même carte, et les trois voies gardent chacune leur caractère. Une
symétrie 180° les échangerait et forcerait les deux voies extérieures à être
identiques ; c'est le piège que ce choix évite.

| Voie | Z | Rôle | Profil **mesuré** | Densité au sol |
|---|---|---|---|---|
| **Haute Halle** | +18 | longue | 17 % courte · 56 % moy. · **27 % longue** · max 76,5 m | **15,4 %** |
| **Les Fours** | 0 | courte | **41 % courte** · 58 % moy. · **0 % longue** · max 29,2 m · méd. 7,5 m | 21,5 % |
| **L'Atelier** | −18 | moyenne | 38 % courte · **60 % moyenne** · 2 % longue · max 61,5 m | 26,1 % |

La densité croît de la voie longue vers les autres : une voie de tireur d'élite
encombrée n'est plus une voie longue. Densité globale **inférieure** à la v2
(0,019 vs 0,026 pièce/m²) malgré une carte deux fois plus grande.

### Quatre défauts corrigés en cours de route, tous trouvés par la mesure

| Défaut | Constat | Correction |
|---|---|---|
| Rampes à **48,8°** | injouable — on rampe au lieu de courir | course allongée → **23-27°** |
| Voie courte : ligne de **71 m** | un couloir de pompe qui offrait un tir de sniper | chicane de refends alternés → **29 m** |
| Voie moyenne : **74 m** | devenait une 2ᵉ voie longue (échec « Berlin ») | redans en quinconce sur les deux demi-voies → 61 m, 2 % de lignes longues |
| **8 jonctions** entre voies | doctrine : 3-4 | percées ramenées à x = 0 et ±20 → **4** |
| Densité **inversée** | la voie longue était devenue la plus dense | abris resserrés puis espacés → gradient rétabli |

### Toutes les règles, vérifiées

| Règle | Source | Résultat |
|---|---|---|
| Trois voies, portées dédiées | Treyarch | ✅ trois profils distincts mesurés |
| Positions de force en miroir | Treyarch | ✅ 4, appariées (test mécanique) |
| Aucun point ne couvre toutes les jonctions | LD Book | ✅ max 2/4 (positions de force : 0-1/4) |
| 3-4 étranglements | LD Book | ✅ 4 |
| Espacement des abris 3-10 m | Watch Dogs | ✅ min 3,0 · méd. 9,0 · max 9,9 |
| Couverture > hauteur d'œil | Uncharted (adapté) | ✅ 0 couverture inutile sur 40 |
| Pas d'impasse, boucles | arXiv | ✅ **1 seule composante connexe** |
| Aire praticable | arXiv | ✅ 72 % |
| Symétrie | arXiv | ✅ 100 % des pièces ont leur reflet |
| Marche ≤ hauteur de saut | metrics | ✅ 1,10 m pour 1,174 |
| Pente de rampe | LD Book | ✅ 23-27° (norme escalier 30-35° = plafond) |
| Domination sur sa voie | inégalité de crête | ✅ 62 % (voie longue) · 32 % (voie moyenne) |
| Chute sûre | TF2 | ✅ sans objet — pas de dégâts de chute |

### Ce qui a changé dans le moteur

- `[arena] shape` = `"rectangular"` | `"hexagonal"` → enceinte 4 murs ou 6 côtés,
  et emprise commune (`half_extents`) pour l'enceinte, le contrôle de sol et
  l'échantillonnage de l'analyse.
- **`[[lanes]]` en donnée** — l'analyse mesure la domination d'une position
  *sur sa propre voie*. Rapportée à la carte entière, la mesure ne voudrait rien
  dire : c'est ce changement qui rend le chiffre interprétable.
- Contrôle du sol : rayon → **marge par axe** (le sol n'est plus un disque).

## 3quinquies. Passe « donjon » — Les Cryptes verticales (2026-07-28)

Demande : *« améliore la map simplifiée pour qu'elle ressemble à un vrai roguelite
avec des salles, des chemins, des murs et des plafonds »*.

Point de départ : un plan de salles/routes existait déjà (`arena_test_crypte_vertical.toml`,
avec `RoomDef`/`RouteDef` et un contrat de plan validé au chargement). Le graphe
était bon — **entrée → terrasse / tunnel / ruines → cour → pont → chapelle** —
mais la matière manquait.

### Le constat qui a tout orienté

Le sol faisait 224 × 148 m pour **6 murs**. On pouvait marcher partout : le graphe
de salles était **décoratif**, on coupait par le vide en ignorant les routes.
Et le tunnel, seul espace couvert, n'avait aucune lumière — il était **noir**.

### Ce qui a été livré

**Moteur** (`arena_test.rs`) :

| Ajout | Pourquoi |
|---|---|
| `RoomDef.ceiling_m` + dalles générées | une salle couverte doit l'être depuis son emprise, pas à la main |
| `[[lights]]` + `PointLight` (plafond 8 000 lm) | une dalle arrête le soleil directionnel ; sans lumière la salle est noire |
| Palette `ceiling` (plus sombre) | un plafond doit se lire comme une limite, pas comme une paroi |
| Alerte **critical** « salle couverte sans lumière » | panne certaine du passage aux salles fermées, muette en jeu |
| Alerte **warn** « hauteur libre < joueur + saut » | sous 3,18 m on se cogne dès qu'on saute |
| `region_of()` — analyse par **salle** | une position tient sa salle, pas la carte ; rapportée à la carte le signal se noie |

**Carte** — carving en **espace négatif** : tout ce qui n'est ni salle ni couloir
devient de la roche pleine. 43 % de la surface reste ouverte, en **24 blocs**
seulement (fusion gloutonne de ~4 500 cellules). Le graphe devient physique.

Plus : 30 plafonds de couloir, **59 lumières**, 48 piliers et refends, chicane du
tunnel. Les 26 pièces posées à la main (couvertures, plateformes, socle du boss)
sont **conservées telles quelles** — la passe ajoute l'enveloppe, elle ne réécrit
pas l'intention de combat.

**Alternance clos / ouvert** (3 couvertes / 4 à ciel ouvert) : entrée 6 m, tunnel
4 m, chapelle 9 m — trois paliers distincts, écarts ≥ 1,5×, ce que la recherche
donne comme condition pour qu'une salle haute *se lise* comme haute.

### Vérifications

| Contrôle | Résultat |
|---|---|
| Connexité après carving | ✅ **7/7 salles atteignables**, 99 % du praticable connexe |
| Contrat de plan (routes, rampes, BFS spawn→boss) | ✅ inchangé, toujours vert |
| Salles couvertes éclairées | ✅ 3/3 |
| Hauteur libre | ✅ ≥ 4 m partout (min requis 3,18) |
| Chicane du tunnel | ✅ **62,6 → 30,5 m**, médiane 7,3 m, 0 % de ligne longue |
| Tests / clippy / sensor-audit | ✅ 23 tests · 0 warning · OK |

### Le mur sur lequel la passe bute — et il est sourcé

Recherche menée en parallèle (5 angles, 882 k tokens). Elle donne la contrainte
qui manquait, **Level Design Book, *Classic Combat*, verbatim** :

> *« do not make an arena more than ~1024 units wide »* — soit **36,6 m** à notre
> échelle, que nos armes (pleins dégâts jusqu'à 30 m) ramènent à **30 m**.

Or **6 salles sur 7 dépassent ce plafond de 16 à 30 m** :

| salle | emprise | diagonale | lignes longues mesurées |
|---|---|---|---|
| pont | 22 × 14 | **26 m ✅** | **0 %** |
| terrasse | 55 × 22 | 59 m | 29 % |
| cour | 30 × 52 | 60 m | 26 % |
| entrée | 36 × 64 | 73 m | 39 % |
| chapelle | 45 × 54 | 70 m | 20 % |
| ruines | 64 × 46 | 79 m | 36 % |

**La seule salle dans le barème est la seule au profil de combat correct.** La
corrélation est nette et elle valide la contrainte contre nos propres mesures.

Conclusion honnête : on ne rattrape pas une salle deux fois trop grande en la
meublant. Les colonnades et refends ont fait gagner 2 à 4 points — le tunnel, lui,
est passé de 62 à 30 m parce que sa *forme* a changé, pas son mobilier. **Le lot
suivant est un redimensionnement du plan**, pas un ajout de murs.

### Le contrat de plan a servi trois fois

La validation écrite par la session précédente (`route_contract_errors`) a rejeté
trois états successifs de ma génération, et à chaque fois elle avait raison :

1. **La chicane coupait les deux routes du tunnel.** Cause réelle : `entree_tunnel`
   était déclarée **large de 14 m**, soit toute la section du tunnel. Un couloir
   dont le trajet occupe toute la largeur n'est pas un couloir, c'est une salle —
   aucune obstruction n'y tient. Ramenée à 5 m (standard sourcé : couloir de
   combat 4,0–4,6 m).
2. **Le trajet serpentant traversait une couverture** posée à la main.
3. **La route de 5 m ne passait pas en diagonale** dans un jeu de 5 m. Passage
   élargi à 7 m et trajets rendus **axiaux** dans chaque refend.

Les deux couvertures centrales du tunnel ont été retirées : elles dataient du
trajet rectiligne. **Dans un couloir en zigzag, l'angle EST la couverture** — une
caisse au milieu ne garde plus rien et tombe en travers du chemin.

C'est le meilleur argument pour ce type de contrat : il transforme une erreur de
plan en message précis, au lieu de la laisser se découvrir manette en main.

### Un bug muet trouvé au passage

Sur 10 refends demandés, **1 seul a été posé**. Le filtre « ne pas boucher une
route » les rejetait tous en silence, les trois routes partant de l'entrée. Le
générateur annonçait « 48 piliers/refends » sans distinguer ce qui avait été
écarté. C'est exactement la panne muette contre laquelle le reste de ce banc est
construit : **un générateur doit journaliser ce qu'il refuse de poser**, sinon on
lit un chiffre global qui ment.

## 3sexies. Passe FINITIONS (2026-07-28)

Retour : *« la taille c'est bon mais ça pourrait être plus propre sur les
finitions »*. La taille est donc **validée par le ressenti** — la contrainte des
30 m mesurée au tour précédent reste vraie sur le papier, mais c'est le jeu qui
tranche, et le jeu dit que ça passe. Pas de redimensionnement.

Plutôt que de deviner ce qui « fait sale », un scan géométrique a cherché sept
classes de défauts visibles. Il en a trouvé cinq réels.

| # | Défaut mesuré | Correction |
|---|---|---|
| 1 | **Le plafond ne touchait pas les murs** — fente de 1 m sur ~200 m de développé par salle couverte (le creusement ouvre l'emprise + marge, la dalle ne couvrait que l'emprise) | `[grid] ceiling_overhang_m = 1.5` → la dalle **déborde de 0,5 m** dans la roche |
| 2 | **La dalle de la chapelle sortait du massif** — sol +4 + plafond 9 = 13 m contre 12 m de roche : elle flottait, avec une fente horizontale sur tout le périmètre | plafond 9 → **7 m** (reste dans la fourchette « haut » sourcée 7-9 m) |
| 3 | **11 chevauchements de volumes** — piliers plantés dans des couvertures posées à la main, refends dans des piliers | rejet à la génération, **9 pièces écartées et journalisées** ; les refends passent avant les colonnades (une cloison structure, un pilier remplit) |
| 4 | **19 coudes de couloir à nu** — les dalles suivent des segments droits, les angles restaient ouverts | **29 dalles d'angle**, posées 2 cm plus bas pour éviter le scintillement de faces coplanaires → **0 coude à nu** |
| 5 | **Le pont flottait** — il enjambe le vide entre cour et chapelle sans appui | **3 piles** sous le tablier |

Plus : trame de 2 m appliquée à tout ce que le générateur produit (piliers 2,4 →
2,0 · refends et chicane 1,8 → 2,0 · positions arrondies), et un chevauchement de
12 m³ entre deux pièces posées à la main (une ruine mordait dans la plateforme de
récompense) corrigé en décalant la ruine — la plateforme porte la récompense,
c'est elle la contrainte forte.

### Ce qui reste hors trame, et pourquoi c'est assumé

63 positions et 26 tailles restent hors module. Ce sont **les dalles de plafond
de couloir** — elles suivent des trajets à angles quelconques, une dalle qui
épouse un chemin ne peut pas être sur trame — et **des blocs posés à la main**
par la session précédente, qui portent une intention et ne m'appartiennent pas.

### Le bug muet, corrigé cette fois

La passe précédente annonçait « 48 piliers/refends » en en refusant 9 en silence.
Le générateur **journalise désormais chaque pièce écartée avec son motif**
(`ECARTE pilier_dans_bloc: 9`). Un générateur qui ne dit pas ce qu'il refuse fait
mentir ses propres totaux.

### Et un défaut que le scan a créé avant de le corriger

Les premières dalles d'angle étaient **coplanaires** avec les segments : deux
faces au même Z scintillent. Le scan les a comptées comme 22 nouveaux
chevauchements — il avait raison. Décalage de 2 cm : invisible à l'œil,
suffisant pour le tampon de profondeur.

## 3septies. Passe SHRINK — bande combat (2026-07-28)

Demande audit design : *« shrinker le plan salle par salle »*. Même graphe,
emprises ramenées dans le barème ~30–34 m.

| | Avant (oversized) | Après (v2 shrink) |
|---|---|---|
| Monde | 220 × 144 m | **120 × 60 m** |
| Diag. salles | 26–79 m (6/7 hors barème) | **21.6–31.6 m (7/7 OK)** |
| `extent_m` | 30 (mensonge) | **0** (ignoré en rectangulaire) |
| Backup | — | `arena_test_crypte_vertical_oversized.bak.toml` |
| Générateur | — | `tools/shrink_crypte_vertical.py` |

Contrats : `route_contract_errors` vert · test
`crypt_vertical_scenario_is_playable_from_declared_metrics` OK · validateur
`tools/validate_crypte_shrink.py` 0 erreur.

> ⚠️ **Ce « vert » ne valait rien** — corrigé en 3octies. `route_contract_errors`
> ne testait qu'un fil de 0,6 m au milieu de couloirs de 5 m : il passait au vert
> sur presque n'importe quelle géométrie. Une fois réparé, il a remonté 15
> obstructions réelles sur cette même carte. Ne pas citer cette ligne comme preuve.

**Prochaine étape** : playtest manette (validation runtime toujours ouverte) puis
promotion vers `forgia-stage` / `roguelite_stages.toml`.

---

## 3octies. Passe AUDIT — trois capteurs menteurs et un générateur qui décale (2026-07-29)

Demande : *« audite complètement la map et identifie toutes les erreurs de design,
autant de loops que nécessaire, rendu AAA exigé »*.

L'audit a produit 156 critères de recherche, une grille de 39 points et 7 lentilles
d'inspection → 78 constats. **Mais le premier résultat de l'audit a été que les
capteurs qui devaient l'arbitrer étaient faux.** Les réparer d'abord était la seule
manière de ne pas empiler des constats sur une mesure cassée.

### Les trois capteurs réparés

| # | Capteur | Ce qu'il prétendait | Ce qu'il mesurait vraiment |
|---|---|---|---|
| 1 | `route_contract_errors` | Les routes offrent la largeur déclarée | Un **fil de 0,6 m** (`player_radius_m`) au centre de couloirs de 5 m → vert sur presque tout |
| 2 | `tallest_traversal_step_m` | Les marches sont franchissables | `0.0` parce qu'il y a **zéro bloc `traversal`** → vert par absence de sujet |
| 3 | `segment_hits_block_clearance` | Le joueur passe | AABB **gonflée du rayon** : le coin est à `r·√2`, donc **+41 % de faux positifs** dans les diagonales |

Le n° 3 n'a été trouvé qu'*en réparant* le n° 1 : une fois le contrôle porté à
80 % de la largeur déclarée, il a annoncé 15 obstructions — dont 3 étaient ses
propres faux positifs de coin. Un capteur réparé peut mentir dans l'autre sens.

Corrections : dégagement testé sur `width_m × 0.5 × 0.8` avec filtre de tranche
verticale (les piles sous un tablier ne bouchent plus le tablier) ; **distance
exacte segment ↔ pavé** au lieu de l'AABB gonflée, alignée sur la forme déjà
canonique du codebase (`forgia_stage::layout::is_in_player_boss_corridor`) ;
statut `info` / « aveugle » quand il n'y a rien à mesurer, au lieu de vert.

### Les quatre défauts de géométrie, une fois la mesure fiable

15 obstructions réelles → **0**, mesurées, cliquet supprimé.

1. **`snap()` décalait la roche de 1 m une bande sur deux** — cause dominante des
   8 obstructions `roche`. Une bande de `n` cellules a pour centre `2·i₀ + n` ; si
   `n` est impair ce centre est impair, et `snap()` l'arrondissait vers l'entier
   pair voisin (`round(1.5) == 2` en Python). La roche se retrouvait un mètre à
   côté : elle bouchait un couloir ici et laissait un trou là. **Preuve dans la
   donnée** : tous les pavés `size = [2, 12, 2]` étaient à une position paire,
   alors qu'une cellule seule tombe forcément sur un centre impair. La fusion
   produit déjà un centre exact — `snap()` ne corrigeait rien.
2. **Le creusement dilatait de 1,0 m sur une trame de 2,0 m** — demi-côté au lieu
   de demi-diagonale (1,414 m), donc les coins de cellule mordaient jusqu'à 0,41 m
   dans les couloirs. Appliqué **aux routes seulement** : côté salles la marge de
   1 m est voulue, et l'élargir aurait creusé d'autant les bords de plateforme
   sans garde-corps des salles en hauteur — un défaut connu qu'on aurait aggravé.
3. **Le passage de chicane faisait 4,0 m pour 5 déclarés**, et son 4ᵉ mur flottait
   en îlot à 1,0 m de l'axe de `tunnel_cour`. Les deux premiers murs raccourcis de
   0,5 m côté passage (faces extérieures intactes, tous deux enjambent toujours
   `z=0` → occultation du zigzag préservée) ; le 4ᵉ adossé au mur nord du tunnel,
   ce que son intention décrivait déjà.
4. **Une couverture et une paire de piliers tangents.** `entree_ruines` suit la
   médiane de sa salle : la cover atteignait 2,0 m pile de l'axe. Le pilier nord du
   hall était à 1,92 m de `entree_terrasse` — 8 cm de trop. La paire entière a été
   déplacée pour garder la symétrie du hall de spawn.

### Vérifications de la passe audit

- `cargo test -p forgia-game arena_test` → **24 passed, 0 failed** (23 + le test
  qui verrouille le non-sur-report de coin : `√2` disponible, 1,2 m exigé → passe ;
  1,5 m exigé → obstruction)
- `cargo clippy -p forgia-game --all-targets` → **0 warning**
- Le cliquet `OBSTRUCTIONS_CONNUES = 15` est **supprimé** : l'assertion est
  `errs.is_empty()`. L'historique des cinq causes est consigné en commentaire au
  point d'assertion, avec l'interdit explicite de regagner le vert en abaissant le
  seuil du contrôle.

### L'audit lui-même était périmé — et c'est le constat le plus utile

Le workflow a atteint la limite de session (93 agents, 52 aboutis, 41 échoués) et
son `defauts_confirmes: 2` était un artefact de l'échec. Mais en cherchant à le
reprendre, on a trouvé pire : **le dossier qu'il a audité décrivait la carte
d'AVANT le shrink**.

| | Dossier de l'audit | Carte réelle |
|---|---|---|
| Monde | 220 × 144 m = 31 680 m² | 120 × 60 m = 7 200 m² |
| Blocs | 165 (75 murs, 66 plafonds) | 80 (47 murs, 9 plafonds) |
| Lumières | 64 | 30 |
| `ruines` | 64 × 46, diag 78,8 m | 18 × 14, diag 22,8 m |
| `cour` | 30 × 52, diag 60 m | 22 × 22, diag 31,1 m |

Les 78 constats portent donc sur une géométrie 4 fois plus grande qui n'existe
plus. **Reprendre les vérifications aurait validé des constats sur une carte
remplacée.** Le texte complet des constats est de toute façon perdu : l'état
sauvegardé ne garde que des aperçus tronqués à 401 caractères.

Leçon de process : un dossier d'audit doit être **regénéré juste avant** la passe,
et l'outil qui le produit ne doit pas porter de chiffres codés en dur — ceux de
`dossier.py` étaient tous faux (voir ci-dessous).

### Les défauts réels, remesurés sur la carte courante

Chaque chiffre « défaut connu » du dossier était faux :

| Dossier (codé en dur) | Mesuré |
|---|---|
| 49 m² de bords sans garde-corps, 32 sur la cour | **82 m²** — terrasse 16, cour 19, pont 22, chapelle 25 |
| 2 chevauchements mineurs (3 et 6 m³) | **15**, dont **144 m³** et 48 m³ |
| 63 positions / 26 tailles hors trame | 52 / 25 |
| 6 salles sur 7 hors barème | 3 sur 7, de 1 à 2 m |
| « 19 coudes à nu → 29 dalles d'angle » (noté CORRIGÉ) | **3 coudes à nu, 0 dalle d'angle** — le correctif a été perdu à la régénération du shrink |

**Corrigé dans cette passe** : les chevauchements sont passés de **15 à 8**, et
les deux plus gros ont disparu. Cause : les emprises de salles adjacentes se
chevauchent volontairement (pour que les rampes atterrissent dans les deux), et
chaque socle étant plein de `y=0` au niveau du sol, un socle plus bas était
entièrement noyé dans un socle plus haut — 144 m³ `pont×chapelle`, 48 m³
`cour×pont`. Les socles sont désormais émis du plus haut au plus bas avec
soustraction de l'intersection (`rect_minus`), ce qui est **strictement neutre** :
on ne retire que du volume enfermé dans un solide plus haut. `plateforme_cour` est
découpée en 3 pavés, aucun sol n'est perdu. Aussi corrigés : 9 m³ de la chicane
contre elle-même, 4 × 2,8 m³ des covers de la chapelle dans sa colonnade.

**Trouvé, NON corrigé — demande un arbitrage** :

1. **Les deux rampes entre salles en hauteur sont enterrées.** `rampe_pont`
   (x 22→26, 2→3 m) et `rampe_chapelle` (x 38→42, 3→4 m) sont dans le plein des
   socles : la plateforme du pont remplit `x[23,41]` jusqu'à y=3, celle de la
   chapelle `x[37,59]` jusqu'à y=4. Les vraies transitions sont des **marches de
   1 m** (franchissables, le saut fait 1,17 m — mais ce n'est pas ce que le plan
   déclare). Corriger exige de **désaligner les emprises de salles**, or leur
   taille a été jugée en jeu et validée. Décision utilisateur.
   → `route_contract_errors` ne voit rien : il vérifie qu'une rampe est
   *déclarée*, pas qu'elle est *dégagée*. Capteur à compléter.
2. **Le pont n'est pas un pont** : `plateforme_pont` est un massif plein, et les
   3 piles ajoutées pour corriger « pont sans appui » sont enterrées dedans
   (3 × 12 m³). Soit on assume le massif et on retire les piles, soit on évide le
   dessous et les piles portent vraiment.
3. **82 m² de bords sans garde-corps**, cause identifiée : plateforme = emprise de
   la salle, creusement = emprise + 1 m → anneau de 1 m sans sol sur chaque salle
   en hauteur. C'est la **plateforme** qui doit couvrir l'anneau (élargir le
   creusement aggrave, cf. le piège évité plus haut).
4. **Aucune ligne de vue au-delà de 30 m dans aucune salle** (max 29 m). Le
   sniper (300 m sans chute de dégâts) et le lance-roquettes (60 m) n'ont aucun
   terrain. La bande « longue portée » demandée n'existe plus depuis le shrink.
5. 3 coudes de couloir à nu · marches au plafond aux raccords (entrée 2 m,
   chapelle 3 m) · puits de roche morts au-dessus des dalles (entrée 6 m, tunnel
   8 m) · le banc duplique `forgia-pcg-core` / `forgia-stage::layout` /
   `forgia-level-presets`.

> ⚠️ `assets/genomes/arena_test_crypte_vertical.toml` est **généré**. Éditer
> `tools/shrink_crypte_vertical.py` puis régénérer — jamais le TOML à la main.

---

## 4. Ce qui reste (lots suivants)

| # | Lot | Pourquoi | Effort | Risque |
|---|---|---|---|---|
| 1 | Ouvrir `forgia-editor` à `ArenaTest` + **export TOML** | Aujourd'hui gaté `GameMode::CastleHub` et persiste en JSON. Sans boucle édition→export, on itère 3×/h au lieu de 20 | ~2 h | Medium |
| 2 | Règle de mesure in-game (distances, hauteurs au curseur) | La grille donne l'échelle au sol, pas les portées | ~1 h | Low |
| 3 | Mannequins / cibles paramétrables | Vérifier les distances d'engagement, pas seulement la circulation | ~1 h | Low |
| 4 | Télémétrie de playtest (heatmap positions/morts) | Le capteur donne l'état du banc, pas le comportement du joueur | ~2 h | Medium |
| 5 | Rechargement à chaud des pièces authored de `forgia-stage` | Une fois « Le Creuset » validé, le porter en vraie arène de run | ~3 h | High |

---

## 5. Cross-refs

- `.claude/rules/concept-first.md` — protocole suivi ci-dessus
- `.claude/rules/no-hardcode.md` — geometry en couche definition
- `.claude/rules/observability-required.md` — capteur + seuils + next-step
- `docs/observability/SENSOR_REGISTRY.md` — `forgia2_arena_test.json`
- `crates/forgia-stage/src/authored.rs` — le pipeline authored (story-625) qui
  recevra le blockout validé
