# story-693 — Pépin en viewmodel pixel art (et le rechargement enfin animable)

**Statut** : 🚧 IN_PROGRESS — compile, 0 clippy, 25 tests verts ; **non validé en jeu**
**Créée** : 2026-08-09
**Niveau BMAD** : Standard (9 fichiers code/data + 18 frames)

---

## Point d'avancement — 2026-08-10

**Fait** : les **quatre faces** de Pépin sont dessinées (`tools/art/pepin_art.py`
→ `views()` : côté / avant / arrière / dessus), sur un **modèle de lumière**
neuf (`tools/art/light.py` : occlusion de contact, spéculaire au bord, halo
émissif). Le dessin porte désormais les deux pièces que le
[GDD *The Spared*](../design/gdd-forgia-the-spared.md) §5 rend obligatoires :
le **cœur de braise** (paramètre `heat`) et la **jauge de confiance**.

**Pas fait, et c'est ce qui bloque la sortie de IN_PROGRESS** :

- Aucune des 4 vues n'est **exportée en frames** ni référencée par
  `assets/genomes/viewmodel_arena.toml`. Les 48 frames installées dans
  `assets/textures/weapons/pixel/pepin/` viennent toujours du **pipeline GLB**.
- Rien n'est **validé manette en main**.
- ⚠ **Contrat de câblage à trancher avant de coder** : la jauge est dessinée à
  **4 cellules**, alors que le système réel `forgia_combat::confidence` est
  gradué **0-10** (+1 hit / −1 miss, HUD 10 cœurs). Quantifier 10→4, ou
  redessiner à 10 ? Et `heat` doit se brancher sur le moteur de barks existant
  (`forgia-ui-lib/src/hud/barks.rs`), pas sur un minuteur propre — l'arme
  rougeoie *quand elle parle*, c'est la formulation du GDD.

---

## Le constat qui décide de tout

`assets/models/weapons/forgia/pepin.glb` — 7,7 Mo — contient **1 node, 1 mesh
fusionné, 0 animation, 0 squelette**. Il n'y a ni chargeur, ni glissière, ni
culasse séparés.

Conséquence : **aucune animation de rechargement n'est possible sur cet asset**,
quel que soit le code écrit. On ne peut déplacer que le bloc entier. Sans Blender
pour découper et rigger, la voie 3D est fermée exactement là où le jeu doit aller.

Le rechargement n'existait jusqu'ici que comme **minuteur** (`reload_time_secs`,
`reload_kind`) — rien à l'écran pendant 1,2 s.

## La réponse

Un sprite se compose de **pièces indépendantes** à la génération. Le rechargement
devient une suite de frames où le chargeur, la glissière et la main bougent
séparément. C'est précisément ce qu'un maillage fusionné interdit.

## Ce qui a été livré

| # | Item | Fichiers |
|---|---|---|
| 1 | Bibliothèque de dessin pixel art déterministe | `tools/art/pixelforge.py` |
| 2 | Pépin : géométrie, palette, 18 frames | `tools/art/pepin.py` |
| 3 | Frames exportées (idle 1 · fire 3 · reload 14) | `assets/textures/weapons/pixel/pepin/` |
| 4 | Couche sprite du viewmodel + capteur | `crates/forgia-viewmodel/src/sprite.rs` |
| 5 | Branchement, masquage des bras 3D, champs génome | `attach.rs`, `arms.rs`, `genome.rs`, `calibration.rs`, `lib.rs` |
| 6 | Bascule data-driven de Pépin | `assets/genomes/viewmodel_arena.toml` |

## Décisions qui portent des invariants

**Les durées ne sont écrites nulle part dans l'animation.** Le clip de
rechargement lit `ReloadState::progress(reload_time_secs)` — la valeur du
gameplay — et le clip de tir se cale sur `fire_rate`. Changer la balance rythme
l'animation automatiquement. Une durée d'anim écrite à côté aurait été *la même
grandeur écrite deux fois*, la classe de défaut canonique du projet
(cf. `feedback_une_grandeur_ecrite_deux_fois`), et les deux auraient divergé au
premier passage de balance : animation finie avant que l'arme soit rechargée.

**Aucun pixel n'est jamais tourné.** Les formes sont des polygones dont on tourne
les *sommets*, rasterisés une seule fois. Faire tourner une image pixel art la
détruit.

**L'arme fuit vers le fond — passe d'orientation du 2026-08-09.** La 1ʳᵉ version
était un profil orthographique pur : rapporté en jeu, *« il est trop de côté »*,
elle se lisait comme un autocollant. Corrigé par `project()` dans `pepin.py`,
qui applique un **raccourci** le long de l'axe du canon (`FORESHORTEN = 0.72`) et
une **fuite** en travers (`TAPER = 0.30`), plus une **face supérieure** de la
glissière. Deux essais ratés à ne pas refaire : `FORESHORTEN = 0.58` rend le
pistolet trapu, et une face supérieure de 7 px contre 19 au flanc laisse le
sprite plat — à cet angle c'est **le dessus qui domine**, pas le côté. La crosse
est en deçà du point d'ancrage, donc son facteur passe au-dessus de 1 et elle
grossit : c'est juste, elle est plus près du joueur.

**Les proportions vivent dans le mesh, pas dans `Transform.scale`.** La pose ADS
écrit un scale uniforme et les écraserait.

**Les rotations du génome passent à 0 pour Pépin.** Le `rotation_y_deg = -90`
orientait le GLB ; appliqué à un quad il le met de chant, donc invisible. Valeur
GLB conservée en commentaire pour le retour arrière.

**Le pipeline existant n'est pas refait.** Le quad porte `WeaponViewmodel`, donc
`propagate_viewmodel_layer` lui applique le layer 1, et `pose.rs` lui applique
sway / bob / ADS / recul sans une ligne de changement.

## Observabilité

`forgia2_viewmodel_sprite.json` (1 Hz) — `declared`, `spawned`, `clip`, `frame`,
`awaiting_mesh`, `probe_state`. `critical` si les frames manquent du dist
(mode d'échec réel : viewmodel invisible sans diagnostic), `warn` si déclaré mais
non spawné, `info` en attente de la première image. Les six branches sont
couvertes par test — un capteur qui ne peut pas rougir ne mesure rien.

## Critères d'acceptation

- [x] `cargo check -p forgia` vert
- [x] 0 warning clippy sur la crate (clippy lancé en direct, pas via RTK)
- [x] 25 tests verts, dont 6 sur la couche sprite
- [x] Aucune durée d'animation dupliquée depuis le génome
- [x] Capteur avec branche `critical` atteignable et testée
- [ ] **Validé en jeu** : Pépin s'affiche en pixel art, net (pas flou)
- [ ] **Validé en jeu** : R déclenche l'animation, le chargeur tombe et remonte
- [ ] **Validé en jeu** : une seule main (pas de bras 3D en doublon)
- [ ] Cadrage / taille jugés manette en main, offsets re-tunés si besoin
- [ ] Décision : on étend aux 3 autres armes, ou on revient au GLB

## Reste à faire

| Tâche | Effort | Risque |
|---|---|---|
| Validation en jeu + tuning `target_size`/`offset_*` (hot-reload) | 15 min | Low |
| Selon verdict DA : étendre à Bourrasque / Lenoir / Boucherie | ~2 h/arme | Low |
| Palette externalisée en TOML si l'art est retenu | 30 min | Low |

## Ce que cette story ne couvre PAS

Les 3 autres armes restent en GLB — mélange assumé pendant le pilote. L'ADS n'a
pas de frames dédiées (le quad se rapproche et rétrécit via la pose existante).
Aucune passe de pixelisation globale : le monde reste en toon 3D, et c'est
exactement la question de direction artistique que le pilote doit trancher.

## Cross-refs

- `.claude/rules/no-hardcode.md` — bascule et comptes de frames en couche definition
- `.claude/rules/observability-required.md` — capteur + next-step
- `feedback_une_grandeur_ecrite_deux_fois` — pourquoi les durées ne sont pas dupliquées
- `feedback_valider_chaque_feature_en_jeu` — compilé ≠ fait
