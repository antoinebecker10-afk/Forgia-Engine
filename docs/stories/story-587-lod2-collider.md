# Story-587 — Collider sur les mega-tiles LOD2 (anti chute-à-travers le sol)

> **Statut** : code-complete (2026-06-09), NON COMMITÉ, runtime à valider (cas limite).
> **Scope BMAD** : Quick→Standard (1 fichier, forgia-terrain). Bug B2 de l'audit A→Z.

## Problème (audit B2)

Les chunks LOD0/LOD1 portent un `Collider::heightfield` (`meshing_heightmap.rs:244`), mais les **mega-tiles LOD2** (128–1500 m, `lod.rs:592`) étaient spawnées **sans collider** (Mesh3d + Transform seulement). → dès qu'une entité sort du ring de chunks (LOD0/1, ~128 m), le sol visible (LOD2) est **non-collisionnable** → chute à travers le terrain (téléport, knockback, projectile rapide, déplacement plus vite que le streaming).

## Fix

`forgia-terrain/src/lod.rs` `build_lod2_tiles_system` : ajoute un `Collider::from_bevy_mesh(&cluster_mesh, TriMesh)` + `RigidBody::Fixed` au spawn de chaque mega-tile LOD2. Le trimesh épouse exactement le mesh visuel (Y per-vertex + flatten + skirt).

- Trimesh (pas heightfield) car `build_lod2_terrain_mesh` retourne un `Mesh` (pas une grille de hauteurs exposée) → from_bevy_mesh = correct + simple.
- **Pas de `CollisionGroups`** : cohérent avec le `Collider::heightfield` LOD0 qui n'en a pas. (B6 — harmoniser les groupes G1-G5 sur tout le terrain = cleanup séparé ; les poser sur LOD2 seul serait incohérent.)
- Coût : ~137 tiles max × ~550 tris, build incrémental throttlé (30 frames), despawn avec la tile. Narrowphase ~inactive (joueur surtout en LOD0).

`cargo check -p forgia` OK, clippy 0 (forgia-terrain).

## Critères d'acceptation

- AC1 — Les tiles LOD2 spawnent avec `RigidBody::Fixed` + `Collider`. ✅ (code)
- AC2 — Plus de chute à travers le sol au-delà de 128 m. ⏳ runtime (cas limite : outrun streaming / téléport / knockback)
- AC3 — Pas de stutter notable au spawn des tiles LOD2. ⏳ runtime (si KO → passer en heightfield)
- AC4 — `cargo check -p forgia` + clippy 0. ✅

## Suite / vigilance

- Si le build trimesh par tile cause un stutter : extraire la grille de hauteurs de `build_lod2_terrain_mesh` et passer en `Collider::heightfield` (moins cher).
- B6 (harmoniser `CollisionGroups` LOD0+LOD2 via `world_collision_groups()`) = cleanup séparé.
