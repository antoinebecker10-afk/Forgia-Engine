# Story-454 — Anim Debug System V2 (Niveau A — Diagnostic bone-trace)

**Status** : IN PROGRESS
**Created** : 2026-05-18
**Scale** : BMAD Standard (concept transversal anim, scale-up §3.5 concept-first)
**Builds on** : story-451 (Rex skinning + walk cycle Phase 2)
**Blocks** : story-455 (Anim Inspector UI Niveau B), story-456 (skinning weight heatmap Niveau C)

## Contexte / Bug à diagnostiquer

Story-451 Phase 1 a réactivé `inject_skinning_for_rigged_meshes` dans `forgia-auto-rig::lib.rs:1125`. Sensor confirme :
- `forgia_auto_rig.json` → `skinning.total_meshes_skinned: 6`
- `forgia_walk_pose.json` (en mouvement) → `thigh_l_deg=33.9°, thigh_r_deg=-32.2°, knee_l=14.4°` (bones tournent ✓)

MAIS visuellement Rex reste en T-pose statique (jambes immobiles, bras horizontaux). 3 hypothèses concurrentes :
- **(a)** Bindpose Cause A pas complètement fixée pour GLB Meshy (Mesh3d enfant d'Armature)
- **(b)** Ordre `joints[]` mismatch avec `ATTRIBUTE_JOINT_INDEX`
- **(c)** Transform du `Mesh3d` non appliqué (cf Bevy `custom_skinned_mesh.rs` : *"its transform doesn't affect the position of the mesh"*)

Objectif Niveau A : produire les données pour trancher entre (a), (b), (c).

## Acceptance Criteria

- [ ] NEW module `forgia-anim-debug::bone_trace` avec rolling buffer `Local<VecDeque<Snapshot>>` capped à 120 samples (60s à 2Hz)
- [ ] Sensor `forgia_bone_trace.json` écrit à `dbg_bone_trace_hz` (default 2Hz)
- [ ] Snapshot contient : entity_name, mesh_root_world.translation, mesh3d_world.translation, bone hierarchy (entity_name, local_rotation_euler_deg, world_translation), max 24 bones
- [ ] `bind_pose_snapshot` capturé one-shot au moment du skinning inject (référence statique)
- [ ] `mesh_aabb_world` propagé via GlobalTransform pour comparer mesh-follow-bones
- [ ] Genome `config/genomes/debug_anim.toml` data-driven : `dbg_bone_trace_enabled`, `dbg_bone_trace_hz`, `dbg_bone_trace_max_bones`
- [ ] Health alert `forgia_bone_trace_health.json` si désync détectée (mesh AABB statique malgré bone movement)
- [ ] Multi-character : array `characters[]` indexé par entity_name (Rex + lineup 5 humanoïdes)
- [ ] 0 errors, 0 warnings clippy strict sur 3 crates impactés
- [ ] Cargo test pass

## Hors scope (Niveau B/C)

- UI interactif egui panel (Niveau B story-455)
- Manipulation runtime bones via sliders (Niveau B)
- Skinning weight heatmap shader (Niveau C story-456)
- AnimGraph viz schématique (Niveau C)
- Bone Renderer Unity-style (Niveau B — étendre `debug_gizmos.rs` cube extrudé)

## Plan d'attaque

| # | Fichier | Type | Changement |
|---|---|---|---|
| 1 | `config/genomes/debug_anim.toml` | NEW | `[debug_anim] dbg_bone_trace_enabled=1, dbg_bone_trace_hz=2.0, dbg_bone_trace_max_bones=24` |
| 2 | `crates/forgia-anim-debug/src/bone_trace.rs` | NEW | Rolling buffer + Snapshot struct + sensor writer 2Hz + health alert |
| 3 | `crates/forgia-anim-debug/src/lib.rs` | EDIT | `pub mod bone_trace;` + register system in Plugin |
| 4 | `crates/forgia-auto-rig/src/skinning.rs` | EDIT (hook) | Push `BindPoseSnapshot` event à l'inject (3 LOC) |
| 5 | `crates/forgia-anim-debug/Cargo.toml` | EDIT si besoin | Dep forgia-auto-rig pour Event type (sinon define localement) |

## Risques & mitigations

- **Hot path** : sensor write 2Hz I/O bloquant. Mitigation : `Local<String>` réutilisable, `std::fs::write` async pas nécessaire à 2Hz.
- **Multi-character scaling** : 6 chars × 24 bones = 144 entries/snapshot. JSON ~30KB par snapshot, négligeable.
- **Race condition** : bind snapshot doit fire AVANT que les bones bougent. Hook directement dans `skinning::inject_skinning_for_rigged_meshes` qui run en `PostUpdate` une fois par mesh.
- **Genome reload** : hot-reload Shift+F12 doit replanifier le timer sensor. Pattern `DiagnosticConfig` existant à étendre.

## Architecture sensor JSON

```json
{
  "timestamp_secs": 12.3,
  "frame": 738,
  "characters": [
    {
      "entity_name": "Rex",
      "template": "BipedLizard",
      "mesh_root_world": [0.0, 1.05, 0.0],
      "mesh3d_world": [0.0, -0.85, 0.0],
      "mesh_aabb_world": { "center": [0.0, 0.0, 0.0], "half_extents": [0.95, 0.87, 0.30] },
      "bind_pose": {
        "captured_at_secs": 1.2,
        "mesh3d_world": [0.0, -0.85, 0.0],
        "bones_count": 20
      },
      "bones": [
        {
          "name": "hip",
          "depth": 0,
          "world_translation": [0.0, 0.55, 0.0],
          "local_rotation_euler_deg": [0.0, 0.0, 0.0]
        },
        {
          "name": "left_thigh",
          "depth": 1,
          "world_translation": [-0.15, 0.40, 0.0],
          "local_rotation_euler_deg": [33.9, 0.0, 0.0]
        }
      ]
    }
  ]
}
```

## Comment exploiter le sensor

1. Lance le jeu, entre en RPG, avance ~3s puis arrête.
2. `cat forgia_bone_trace.json` → vérifier que `mesh_aabb_world.center` change entre samples quand le perso bouge (= mesh suit terrain).
3. Comparer `bones[1].local_rotation_euler_deg` aux N-3 samples vs N : doit varier si gait_phase change.
4. Comparer `mesh_aabb_world.half_extents` au bind vs current : si half_extents reste exact comme bind = **bug skinning confirmé** (mesh figé), car des bones rotated devraient changer la bounding box.

## Source de vérité patterns AAA

- **UE5 Animation Insights** trace channels — append-only buffer time-series sourcé
- **UE Observe Bone** node — debug ciblé sur 1 bone
- **Houdini Skinning Converter** error visualisation — diff bind vs current

Sources URLs dans le research subagent rapport (section 1, story-454 deliverable A).

## Test plan

1. `cargo check -p forgia-anim-debug -p forgia-auto-rig` → 0 erreur
2. `cargo clippy --workspace -- -D warnings` → 0 warning (au moins sur crates impactés)
3. `cargo test -p forgia-anim-debug` → 5/5 pass (snapshot serde, rolling buffer cap, multi-char index)
4. Launch binaire, mode RPG, marcher 3s, ESC
5. Read `forgia_bone_trace.json` → verdict bug
