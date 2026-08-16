# Story 717 — Le combat à la 3ᵉ personne : l'arme dans la main, le tir sur la caméra d'épaule

**Statut** : IN_PROGRESS — incrément 1 livré, non validé en jeu
**Niveau BMAD** : Standard (story + checklist)
**Dépend de** : rien (la caméra par-dessus l'épaule est livrée, commit `6b71a00`)
**Complète** : 713-716, qui traitent les **ennemis**. Celle-ci traite le **joueur**.

---

## Le constat

L'Expédition est passée à la 3ᵉ personne le 2026-08-14. Le personnage marche,
court, porte une cape ardente — et **n'a rien dans les mains**. Il ne peut pas
tirer non plus : `fps_combat_mode` listait `Fps | Roguelite | ArenaTest`, et
c'est ce prédicat qui décide si le tir, les munitions et le changement d'arme
tournent.

Trois pièces manquaient, et la deuxième est la moins visible :

1. **L'arme n'est pas rendue** dans le monde. Le seul rendu d'arme du projet est
   le viewmodel FPS, enfant de la caméra subjective.
2. **Le tir lisait sa direction sur la `FpsCamera`** — celle que `castle_avatar`
   **désactive** en entrant en 3ᵉ personne. Ouvrir le combat sans traiter ce
   point aurait donné des balles partant d'une caméra que personne ne regarde,
   avec un écart croissant avec le tangage (les deux caméras n'ont ni la même
   sensibilité ni les mêmes bornes).
3. **Le HUD de combat était gaté hors Expédition** : barre de vie `Fps` seul,
   munitions `Fps | Roguelite`.

---

## Ce que l'incrément 1 livre

| # | Livraison | Où |
|---|---|---|
| 1 | L'arme équipée est accrochée à `socket_main_droite` (enfant de `RightHand`), à l'échelle dérivée de son AABB | `crates/forgia-mode-expedition/src/arme_main.rs` (nouveau) |
| 2 | Son orientation, sa taille et son décalage vivent en couche definition, **rechargés à chaud** | `assets/genomes/expedition_arme_main.toml` (nouveau) |
| 3 | La dague du corps est masquée tant qu'une arme est tenue (même main) | idem |
| 4 | **On tire depuis la caméra qui rend** — marqueur `AimCamera` sur les deux caméras, `is_active` départage | `forgia-player`, `forgia-game/castle_avatar.rs`, `forgia-fps` |
| 5 | `Expedition` entre dans `fps_combat_mode` | `forgia-fps/src/lib.rs` |
| 6 | Lueur de bouche et traceur partent **du canon**, plus de la caméra (3,2 m derrière) | `MuzzleWorld` (`forgia-combat`) + chemin de tir |
| 7 | HUD : vie (bas-gauche), munitions (bas-droite), bandeau d'armes | `forgia-ui-lib` |
| 8 | Capteur `forgia2_expedition_arme.json`, 1 Hz, 5 causes distinguées | `arme_main.rs` |

### La décision structurante : le rayon décide, le canon raconte

Le **rayon** part de la caméra active. C'est ce qui garantit **par construction**
que le réticule au centre de l'écran désigne ce que la balle touche : la caméra
d'épaule fait `look_at(look_target)`, donc son avant EST le centre de l'écran.
Maintenir une seconde orientation en parallèle et espérer qu'elles restent
d'accord ne marche jamais.

Le **visuel** (lueur, traceur) part de la bouche du canon de l'arme tenue. C'est
la répartition standard des jeux de tir à la 3ᵉ personne.

### Une grandeur rassemblée au passage

La table `WeaponType → clé de génome / nom de GLB` existait en **deux copies**,
dont l'une se déclarait « dupliquée pour éviter la dép crate ». Une troisième
s'annonçait ici. Elle vit maintenant sur `WeaponType::genome_key()`
(`forgia-combat`, déjà dépendance commune des trois) ; les deux anciennes
fonctions délèguent, leurs appelants n'ont rien vu.

---

## Ce qui n'est PAS livré — et doit se dire

- **Le personnage ne vise pas du torse.** Il joue ses clips de marche/course ;
  l'arme suit sa main. Regarder vers le haut fait monter le réticule, pas le
  canon. Correctif : faire pivoter `Spine1`/`Spine2` du tangage caméra entre
  l'animation et la propagation (motif `forgia-secondary-motion`). **À faire
  APRÈS** le réglage des orientations d'armes : une colonne qui bouge pendant
  qu'on règle une rotation de poignet rend le réglage illisible.
- **Pas de visée (clic droit).** Le plugin ADS est gaté `Fps | Roguelite`, donc
  neutre ici. En Fortnite la visée rapproche la caméra ; ce serait un réglage sur
  `OrbitCamera`, pas sur le viewmodel.
- **Aucun ennemi dans Le Vallon.** On tire sur le décor. Les campements sont
  spécifiés dans le manifeste et attendent 713-716.
- **Le rayon part de la caméra, 3,2 m derrière le personnage.** Un obstacle entre
  la caméra et lui peut donc arrêter la balle. Le bras à ressort rapproche déjà
  la caméra des murs, ce qui limite le cas ; il n'est pas traité.
- **Les valeurs d'orientation du génome sont des points de départ**, reprises des
  rotations du viewmodel (qui encodent l'orientation native de chaque GLB, et
  diffèrent : −90°, +90°, 180°). Elles sont lues dans un autre repère — le socket
  ajoute +90° autour de X — donc elles seront à corriger à l'œil.

---

## Retour de jeu #1 (2026-08-15) — ce que les capteurs ont tranché

Cinq symptômes rapportés. Les capteurs en ont réglé trois **sans une hypothèse** :

| Symptôme | Ce que la mesure a dit | Suite |
|---|---|---|
| « armes pas dans sa main » | Les 4 GLB sont **centrés sur leur origine** (centre à 0,001 m près, emprise 1,911 m pour les quatre) → la main tenait le **milieu** de l'arme | Gène `prise` (0 = crosse, 0,5 = milieu, 1 = canon), défaut 0,30 |
| « certaines trop grandes » | `taille_mesuree_m: 1.911` → `echelle: 0.3139` → 0,60 m rendu, conforme au génome. La chaîne d'os est à l'échelle **1,0000** : ce n'était donc pas un facteur, mais la valeur déclarée | Tailles ramenées à 0,36 / 0,52 / 0,90 / 0,80 m |
| « pas de VFX » ? | `forgia2_aimassist.json` → **`shots_total: 26`**. Le tir résout bien en Expédition ; `forgia-effects` n'a **aucun gate de mode** | Reste à séparer « rien ne s'affiche » de « ça s'affiche au mauvais endroit » |
| « pas de viseur » | Deux ressources décident, **aucun système de ce mode ne les écrit** : `CrosshairHidden` (écrivains = Lobby Roguelite) et `CrosshairMode` (écrivain = pose viewmodel, gaté `Fps \| Roguelite`) | `reprendre_la_main_sur_le_reticule` en `OnEnter` |
| « que l'anim idle » | `speed_mps: 0.00` **au moment de l'échantillon** — ne prouve rien, il était à l'arrêt | `vitesse_max_vue_mps` ajouté au capteur : sépare « le producteur de vitesse est mort » de « le choix de clip est mort » |

Leçon à garder : *une vitesse instantanée échantillonnée à 1 Hz ne peut pas
réfuter « ça ne bouge jamais »*. Il fallait un **maximum vu**, accumulé à chaque
frame. Le capteur mesurait le bon concept à la mauvaise cadence.

## Critères d'acceptation

- [ ] L'arme équipée est visible dans la main droite, à une taille crédible
- [ ] Elle suit la main pendant la marche et la course
- [ ] Elle change quand on presse 1/2/3/4
- [ ] La dague ne traverse plus l'arme
- [ ] Le clic gauche tire ; l'impact tombe **sous le réticule**
- [ ] La lueur et le traceur sortent du canon, pas de l'épaule
- [ ] Barre de vie et compteur de munitions visibles
- [ ] `forgia2_expedition_arme.json` au vert (`severity: ok`)
- [x] 0 warning clippy, 6 tests nouveaux verts, binaire `forgia` lié

---

## Vérifications mécaniques faites

```
cargo check  -p forgia-mode-expedition -p forgia-fps -p forgia-combat
             -p forgia-game -p forgia-ui-lib -p forgia-viewmodel
             -p forgia-mode-roguelite        → 0 erreur
cargo clippy (mêmes crates + forgia-player)  → 0 warning
cargo test   -p forgia-mode-expedition       → 60 passés (dont 6 nouveaux)
cargo build  -p forgia                        → lié
```

Le `cargo` du shell est enveloppé par RTK, qui a déjà masqué des lints sur ce
projet : ces passes ont été faites avec `$(rustup which cargo)` après `touch`.
