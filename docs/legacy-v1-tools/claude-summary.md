# Forgia — Resume Projet (Context Agents)

> Extrait condense de CLAUDE.md pour injection dans les prompts agents.
> Sections 2, 3, 6 + patterns Bevy critiques.

## Projet

**Forgia** — Plateforme creation de jeux IA-native. Showcase: "Game Creator 3D" (Rust + Bevy 0.17.3).
- 92 fichiers .rs, 13 modules, ~19 500 lignes
- Source: `src/` | Configs JSON: `config/` | Assets: `assets/` | Stories: `docs/stories/`
- Status: compile clean (0 errors, 16 warnings)

## Modules

Player, Combat, AI, Inventory, World, UI, Effects, Persistence, Sky, Triggers, GameMode, Debug, Terrain

## Architecture

- **GameSet**: Input → Movement → Physics → Camera → Combat → Effects → UI
- **2 mondes**: Dungeon (BSP) + Terrain (SDF voxel 1km²)
- **Data-driven**: 7 JSON configs (enemies, spells, tuning, player_model, collision, input, prefabs)
- **BMAD**: GPS protocol (1 action → validation → next), stories dans docs/stories/

## Patterns Bevy 0.17.3 CRITIQUES

- Events: `bevy::ecs::message::{MessageReader, MessageWriter}` (PAS EventReader/EventWriter)
- Volume: `Volume::Linear(f32)` / `Volume::Decibels(f32)` — PAS `Volume::Relative`
- Emissive: `StandardMaterial { emissive: LinearRgba::new(r, g, b, a), unlit: true }`
- ChildOf: `ChildOf(entity)` tuple struct, parent via `.0`
- Children: `Children::iter()` yields `Entity` by value (PAS `&Entity`)
- Timer: `timer.is_finished()` PAS `.finished()`
- Max 16 system params — utiliser SystemParam struct si besoin
- Ground colliders: TOUJOURS ajouter `TerrainMeshEntity`
- Cursor unlock: TOUS les panels egui → ajouter a `cursor_lock_system` dans player/camera.rs
- Aabb: `bevy::camera::primitives::Aabb` (PAS `bevy::render::primitives`)
- VFX Hanabi: `SpawnerSettings::rate(N.into())` continu, `::burst(N.into(), 99999.0.into())` one-shot

## Stability Locks (L1-L8)

- L1: GameAssets — tous les loads via GameAssets (zero asset_server.load en gameplay)
- L2: PerfMode F4
- L3: Camera collision 30Hz (CameraCollisionCache)
- L4: EditorRaycast centralise
- L5: Nameplate LOD 10Hz + frustum
- L6: toggle_editor_effects run_if resource_changed
- L7: SystemSets hierarchy (GameSet)
- L8: Minimap LOD cache
- LOCK-INV-1: Inventory 20 slots max

## Regles IA

- Lire avant de modifier, compiler apres
- 0 warnings clippy obligatoire
- Pas de unwrap() sur donnees externes
- Pas de magic numbers sans constante
- Systemes ECS dans le bon GameSet
- Query params ≤ 16

## Conventions

- Axes: +X droite, +Y haut, -Z avant
- GLB: modeles Blender forward inverse 180deg
- Controleur: RigidBody::KinematicPositionBased + KinematicCharacterController
- Colliders editeur: PrefabType.collider_config() + terrain_embed_offset()
- Enemy: AnimationGraph + AnimationNodeIndex, BFS pour AnimationPlayer

## Terrain (M1)

- SDF voxel, Surface Nets meshing, 32x128x32 chunks (1m), 1024m²
- 6 biomes Voronoi, 6 presets, sculpting brushes
- Deps: fast-surface-nets 0.2, ndshape 0.3, hexx 0.19, voronoice 0.2
