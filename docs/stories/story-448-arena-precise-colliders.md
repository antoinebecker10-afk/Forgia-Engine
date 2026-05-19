# Story-448 — V2 Arena Map Precise Colliders

**Status** : DONE (à valider runtime user)

## Post-impl QA (2026-05-18)

- `cargo check -p forgia-mode-fps-arena` : 0 erreur
- `cargo clippy -p forgia-mode-fps-arena --no-deps` : 0 warning
- qa-lead : 0 Bloquant, 1 Majeur (BUG-448-03 quirk rapier3d à valider runtime), 4 Mineurs
- Fixes appliqués : BUG-448-01 (Wall name unique), BUG-448-07 (commentaire LOCK ArenaGround)
- Backlog : BUG-448-03 valider runtime, BUG-448-05 sensor collider count (story future obs), BUG-448-06 arena_layout.toml data-driven (story future)
- Torches/TorchWall : intentionnellement sans collider — décor lumineux pur, couverts par le mur derrière
**Scale** : BMAD Standard
**Date** : 2026-05-18

## Contexte

User report 2026-05-18 : "les dégâts me touchent même derrière un mur" + "améliore la précision des colliders des assets de la map".

Diagnostic préalable (mémoire `reference_v2_arena_wall_collider_too_short.md`) :
- Wall collider top Y=1.5m, bot raycast ascending 1.4→2.0 → passe par-dessus
- Pillars/Columns approximés par `Cylinder(2.0, 0.5)` génériques
- Cover props (crates/barrels/rubble/table) tous en `Cuboid(half_w, half_h, half_w)` carré — barrel rond, table rectangulaire = bad fit
- ChestGold / Chest_SW : aucun collider

## Approche

Pattern `AsyncSceneCollider + ComputedColliderShape::TriMesh` (déjà éprouvé dans `forgia-village-loader`). bevy_rapier3d 0.33 walks scene → génère colliders exacts depuis chaque mesh GLB asynchroniquement après load.

**Trade-off** : TriMesh est exact (raycast bot fiable), coût boot ~5-20ms par scene, runtime gratuit (bodies fixes, pas d'intégration physique). Match pattern village pour cohérence V2.

## Scope

Fichier impacté : `crates/forgia-mode-fps-arena/src/lib.rs`

1. `spawn_wall()` L579 — remplacer `Collider::cuboid(_, 1.5, 0.15)` par AsyncSceneCollider TriMesh
2. CenterPillar (×4) L332-342 — remplacer Cylinder(2.0, 0.5)
3. OuterPillar (×4) L344-355 — remplacer Cylinder(2.0, 0.5)
4. MidColumn (×4) L357-367 — remplacer Cylinder(2.0, 0.4)
5. Cover props (×14) L369-395 — remplacer Cuboid carrés
6. ChestGold + Chest_SW L397-410 — ajouter AsyncSceneCollider (manquant)
7. Torch + TorchWall L429-438 — ajouter AsyncSceneCollider (lecture pour confirmer)

**Hors scope** : Banners (cloth visuel pur, pas de collider), ArenaGroundCollider (déjà OK).

## Acceptance criteria

- [ ] `cargo check -p forgia-mode-fps-arena` 0 erreur
- [ ] `cargo clippy -p forgia-mode-fps-arena --no-deps` 0 warning
- [ ] Bot raycast bloqué par walls (test runtime user)
- [ ] Player ne peut plus traverser barrels/crates (test runtime)
- [ ] Pattern AsyncSceneCollider consistent avec `forgia-village-loader`

## Notes implémentation

- Restructurer parent (Collider primitive + children![SceneRoot]) → parent (SceneRoot + AsyncSceneCollider) flat
- Préserver `ArenaMarker`, `RigidBody::Fixed`, `Name`, `Transform` sur parent
- `with_rotation` / `with_scale` du parent affecte mesh ET collider (rapier 0.33 OK car scale=1)
