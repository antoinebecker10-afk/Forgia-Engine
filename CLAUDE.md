# Forgia : contrat des agents IA (CLAUDE.md)

> Ce fichier est chargé à chaque session par les agents qui travaillent sur ce dépôt.
> Il porte les invariants, pas le comportemental. Le détail des conventions est dans
> `CONTRIBUTING.md`, l'architecture dans `ARCHITECTURE.md`.

## 1. Vision

**Forgia = moteur de jeu IA-natif** : le créateur apporte son idée et ses assets, l'IA
construit le jeu 3D. Le moteur n'a de valeur que s'il permet de finir un vrai jeu ; tout
ce qui ne sert pas cette preuve est différé.

## 2. Qualité sans compromis

> **Propre. Performant. Optimisé. Scalable. Observable. Peu importe le temps.**

Zéro warning clippy · pas d'allocation en hot path · data-driven (génome TOML, pas de
valeur en dur) · `SystemParam` au-delà de 12 paramètres · `forgia2_<feature>.json` et
alerte de santé avec `next_step` · pas de « on migrera plus tard ». `cargo check` après
chaque modification. Français pour la documentation et les échanges, anglais pour le code.

**Concept-first est bloquant** sur toute demande non triviale : avant le premier `Edit`,
(1) la couche visée est-elle la donnée (génome, config) ou le code ? (2) deux ou trois
hypothèses à falsifier, pas à confirmer ; (3) cartographier le concept (producteur,
consommateurs, capteur, timing) avec `grep` sur le mot-concept, pas sur un nom de type ;
(4) verbaliser ce qu'on touche et ce qu'on ne touche pas ; (5) si le chemin est chaud,
vérifier itération filtrée, dirty-flag, zéro allocation.

## 3. Lexique

`AppMode` (Boot/Menu/InGame/Paused) : gate UI · `GameMode` (None/Fps/Rpg/Roguelite) :
gate des plugins de mode · `WorldMode` (Game/Editor/Test) : gate simulation · `GameSet`
(Network/Input/Movement/Physics/Camera/Combat/Effects/Sensors/UI) : chaîne d'ordonnancement
· `Health` (forgia-combat) : partagé · `Player` marker (forgia-player) · `CameraMode`
(forgia-player) : `is_third_person == false` gate FPS.

## 4. Stability Locks

| Lock | Statut |
| --- | --- |
| **L7 chaîne GameSet** | **actif**, encodé dans forgia-core, gardé par les tests |
| L1 baseline GameAssets | actif via `cargo xtask asset-load` |
| L4 EditorRaycast · L5 Nameplate LOD · L8 cache Minimap · LOCK-INV-1 80 slots | à activer au fil des phases |

Un Lock ne se modifie **jamais** sans demande explicite du mainteneur.

## 5. Anti-traps : ne jamais reproduire

- `Query<Entity, Added<T>>` et `Query<&mut T>` séparés : fusionner (B0001)
- `MenuCamera2d` jamais sur `FpsCamera` (`PrimaryEguiContext` et `DespawnOnExit`)
- 1 `KeyCode` = 1 handler unique, gardé par `AppMode`
- Hanabi : pré-spawner un dummy `Visibility::Hidden` au Startup (compilation paresseuse
  des shaders)
- 1 rig Mixamo par personnage, jamais de rig croisé
- bevy_water : feature `easings` obligatoire si water
- `add_systems` : 10 systèmes au plus par tuple
- UI et menus sur `Time<Real>`, gameplay sur `Time<Virtual>`
- Plugin orphelin : `cargo xtask check-orphans` bloque

## 6. Niveaux de travail

| Niveau | Quand | Exécution |
| --- | --- | --- |
| Quick | 3 fichiers au plus | correction directe puis `cargo check` |
| Standard | 10 fichiers au plus | story dans `docs/stories/` |
| Enterprise | au-delà de 10 | recherche, plan, implémentation, vérification |

Une story marquée DONE doit passer `cargo xtask story-gate`.

## 7. Règles absolues

**INTERDIT** : casser sans sauvegarde · supprimer sans trace · inventer des faits ·
violer un Lock · laisser un warning · sur-ingénier · modifier ce fichier sans demande.
**OBLIGATOIRE** : lire avant de modifier · compiler après · respecter `GameSet` ·
respecter `GameAssets` · mettre à jour la documentation.

---

*« Pas de blabla, pas de flou, pas de régression. Du concret, du stable, du livrable. »*
