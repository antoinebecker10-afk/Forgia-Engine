# Story-590 — Obstacles animés du parcours (façon Fall Guys)

**Statut** : EN COURS
**Niveau BMAD** : Standard (4 fichiers, 1 crate `forgia-mode-roguelite`)
**Date** : 2026-06-09
**Cible** : SHIP Roguelite — complexifier le parcours (marteaux qui balancent + balayeurs + blocs coulissants), demande user « comme dans Fall Guys ».

## Taxonomie GLB (platformer_underworld.glb, AABB analysés)
`obstacle_<type>_001.<instance>` :
| Type | n | dim L×H×P | Animation |
|---|---|---|---|
| `obstacle_1` | 10 | 1.6×**4.6**×1.5 (pend sous pivot, ymin −4.6) | 🔨 **Marteau pendule** (swing local Z autour du pivot du haut) |
| `obstacle_3/4/6` | 2/6/7 | barres + croix 7.9×7.9 | 🌀 **Rotation Y** (balayeurs ; 4 = ajouté) |
| `obstacle_2` | 10 | 2.8×1.7×1.6 (bloc) | ↔️ **Coulissant** (translation X sinusoïdale) |
| `obstacle_5/7/8/9/11`, platform, circle | flats / plateformes (le joueur marche dessus) | — | ⛔ laissés statiques (ne pas faire tomber le joueur) |

## Implémentation
| Fichier | Rôle |
|---|---|
| `parcours_obstacles.rs` (nouveau) | composants `SwingingHammer`/`RotatingObstacle`/`SlidingObstacle` ; `classify(name)` ; genome `ObstacleConfig` (mtime hot-reload) ; systèmes swing/rotate/slide ; `ObstacleStats` + sensor `forgia2_obstacles.json` ; `ParcoursObstaclesPlugin` |
| `loot_room.rs` | `sys_mark_demo_meshes` : `classify` + insert composant (capture base rotation/translation + phase désync par position). Retrait de `RotatingObstacle`/`sys_rotate_obstacles` (déplacés) |
| `roguelite_obstacles.toml` (nouveau) | params swing/spin/slide (hot-reload Shift+F12-like) |
| `lib.rs` | `add_plugins(ParcoursObstaclesPlugin)` |

Colliders : les obstacles gardent leur `ConvexHull` (NeedsLevelCollider) ; animer leur `Transform` déplace le collider via propagation `GlobalTransform` → le joueur (KCC) est poussé/bloqué = vraie complexité. Pattern prouvé par les spinners existants (obstacle_3/6).

## QA
- [x] `cargo check` + clippy 0 ; **104 tests verts** (classify, phase, default) — 2026-06-09
- [x] Auto-QA verifier (mécanique) + qa-lead : 0 bloquant ; 2 « Majeurs » réfutés (GLB feuilles kids=0 ; scene write atomique) ; mineurs justifiés ; `forgia.exe` frais 18:49
- [x] **Runtime VALIDÉ** ("parfait") : marteaux balancent, croix/barres tournent, blocs coulissent + spin Y ; `forgia2_obstacles.json` hammers/spinners/sliders > 0
- [x] **Push physique VALIDÉ** : `push.events: 363` (≠ 0), plus de traversée. Fix = obstacles `RigidBody::KinematicPositionBased` (collider statique ne se resync pas dans Rapier) + `AnimatedObstacle` propagé au sous-arbre (collider sur entité-enfant glTF)
- [x] Maillets (obstacle_2) tournent sur Y en plus de coulisser (demande user)
- [ ] Hot-reload : tuner `swing_deg`/`freq`/`speed`/`amplitude`/`push.speed` dans le TOML sans rebuild

## Reste / suite
- [ ] obstacle_5 (panneaux fins) = flippers pendules si besoin de variété
- [ ] Plateformes mobiles qui PORTENT le joueur (obstacle_11 ; nécessite héritage de vélocité KCC)
