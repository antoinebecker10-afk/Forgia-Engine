# Story-496 — Anim pipeline per-character + bone axis validation

**Status** : WIP — Incrément 1 (axe de flexion par-os) DONE 2026-06-02
**Scale** : Standard (5-10 files, story requise)
**Created** : 2026-05-21 PM
**Depends on** : commit `e7fb48a` (Phase A sensors V2 + Rex baseline)

## Journal d'avancement

### 2026-06-02 — Incrément 1 : axe de swing par-os (le fix coeur)
Cause racine reformulée (vs draft AC1-AC6) après design bevy-specialist : pas
besoin de templates per-character NI de `GlobalTransform`. L'axe de flexion se
dérive **en espace local pur** depuis `tip_local` (direction head→tip déjà
capturée). Livré dans `crates/forgia-anim-locomotion/src/locomotion.rs` :
- `BonePose.flex_axis: Vec3` (défaut `Vec3::X` = comportement historique préservé)
- `derive_flex_axis(tip_local)` : os ~Y-local (jambes) → X ; os ~X-local (bras) →
  `cross(dir, Y)` normalisé. Remplace le `from_rotation_x` hardcodé qui *roulait*
  les bras (T-pose).
- `compose_swing` / `compose_stance_swing` → `from_axis_angle(flex_axis, swing)`
  (+ correction `stance.inverse()` pour la stance ±90° des bras).
- Sensor `forgia2_rex_bones.json` enrichi : `flex_axis` + `bone_dir` + `flex_axis_dir`.
- 4 tests unitaires `flex_axis_tests` (vert), clippy 0 warning.

Backward-compatible : jambes inchangées, aucun `LocomotionTarget` actif par défaut
→ zéro changement visible tant qu'Incrément 2 n'est pas fait.

**Reste** : Incrément 2 = réactiver les 4 lignes sur Rex.glb ([character.rs:155-158])
+ valider runtime (bras non-±90°, pas de pied déformé). Incrément 3 = foot IK
chaîne 3-segments (problème Pinocchio 1-os/jambe, différé).

### 2026-06-02 (suite) — Incrément 2 réactivé + Incrément 1 CORRIGÉ (défaut d'axe)
Vérification adversariale (workflow 5 agents bevy-specialist + qa-lead) AVANT test
runtime : a détecté que `derive_flex_axis` (cross(dir,Y)) donnait `±Z` aux bras →
balancement **frontal/latéral** au lieu de sagittal. Confirmé par witness run
(`forgia2_rex_bones.json` : left_arm flex_axis_dir=`-Z`).

**Données witness runtime** (release-fast 21:51, binaire frais) :
- AABB `[-0.874..0.871] → delta +0.000` → hauteur stable, pas de saut.
- `shin L/R=true/true, foot L/R=true/true` (20 bones Pinocchio) → genoux/chevilles OK.
- **Tous bind ≈ identité** (`bind_was_identity` partout) → squelette repos aligné axes.

**Correction** (locomotion.rs) : la flexion du walk est TOUJOURS un pivot sagittal
autour de l'axe latéral (`LATERAL_AXIS = Vec3::X`). `derive_flex_axis(tip_local)`
remplacé par `swing_axis_local(bind) = bind⁻¹·latéral`. Deux corrections de stance :
- `compose_stance_swing` (bras) : annule le stance propre Rz(±90°) (déjà en place).
- `compose_swing_inherit` (NOUVEAU, avant-bras) : annule le stance HÉRITÉ du bras
  parent (sinon le coude fléchit latéralement). Tibias OK sans correction (parent
  cuisse tourne autour de X, axe invariant).
4 tests d'invariant : `world_swing_axis == latéral` pour les 3 compose-paths. Vert,
clippy 0w. **À valider runtime** (récap ci-dessous).

## Contexte

Session 2026-05-21 PM a tenté de réactiver le pipeline anim sur Rex (`LocomotionTarget` + `LocomotionBoneCache` + `ProcBodyAnim` + `LocomotionTemplate`). Résultat : T-pose horizontale + pied gauche déformé en marche. Re-commenté en baseline propre.

**Root cause identifiée** :
1. Pinocchio embed les bones avec axes locaux **dépendants de la morphologie du mesh**. Rex.glb (clone Kael, MD5 `195ca37c…`) a `left_arm` bone horizontal (`tip_local = [-0.296, 0, -0.017]` dans `forgia2_rex_bones.json`).
2. `proc_walk::compose_swing` rotate autour de X local du bone — mais X local varie selon le mesh → rotation hors plan anatomique → vertex deformation visible (pied gauche).
3. Le TOML `skeleton_humanoid.toml` unique avec `arm_Z=±90°` assume bind T-pose Vitruvian. Le mesh Rex actuel n'est PAS Vitruvian → stance composition produit T-pose erronée.
4. Le lineup (Dorin/Mira/Apprenti/MaitreForgeron) utilise le MÊME template Humanoid mais SANS components anim → rend propre (preuve que le mesh source est OK, le pipeline anim est le coupable).

## Acceptance Criteria

- **AC1** : Au moins 1 template per GLB pour les 5 characters humanoid actifs (Rex, Dorin, Mira, Apprenti, MaitreForgeron). Capture des bind layouts effectifs (ARM_Z, LEG_X, FOOT_X) via sensor à `cache.ready`.
- **AC2** : `procedural_locomotion` lit les axes anatomiques (`anatomical_x_axis`, etc.) depuis le template plutôt que d'assumer X local = X anatomique. Convention bone axis documentée dans `docs/architecture/anim-pipeline.md`.
- **AC3** : Foot IK calibré par mesh — sensor `forgia2_foot_ik.json` doit montrer `bones_missing=false` et `active_ratio>0.9` pour Rex et au moins 1 lineup character après réactivation.
- **AC4** : Rex spawn AVEC anim pipeline activé (décommenter les 4 lignes `LocomotionTarget`+…) → rend visuellement comme un humanoid debout (pas T-pose, pas foot deformation). Comparer avec lineup côte à côte.
- **AC5** : Sensors V2 (`forgia2_rex_bones_live.json` etc.) montrent `state=ok` + valeurs cohérentes (arms non-±90°, legs non-stretched).
- **AC6** : Test pure (≥ 5 seeds proc_walk) : assert ankle/knee rotation autour de l'axe anatomique correct (X dans bone-local-canonical) — pas dépendant du mesh source.

## Plan d'attaque suggéré (5 phases)

### Phase 1 — Witness sensors (Quick BMAD)
Avant tout fix : enrichir `forgia2_rex_bones.json` avec **axes locaux** per bone (left/up/forward dans bone local space) à `cache.ready` time. Permet de voir empiriquement la convention Pinocchio per mesh.

### Phase 2 — Per-character template variant (Standard)
- Créer `assets/genomes/skeleton_rex_humanoid.toml`, `skeleton_dorin_humanoid.toml`, etc.
- Étendre `SkeletonTemplateId` enum avec variants per character
- `spawn_rex_character` passe `LocomotionTemplate(SkeletonTemplateId::RexHumanoid)`
- Lineup utilise variant approprié

### Phase 3 — Bone axis convention (Standard)
- Ajouter `anatomical_x_local: Vec3, anatomical_y_local: Vec3` dans `TemplateBone`
- `procedural_locomotion` lit ces axes pour composer la rotation au lieu d'assumer X local
- Anti-pattern à éviter : `Quat::from_rotation_x()` (assume X = lateral) → remplacer par `Quat::from_axis_angle(bone.anatomical_x_local, angle)`

### Phase 4 — Foot IK calibration (Standard)
- Sensor liveness `forgia2_foot_ik.json` doit écrire en TOUT temps (vu absent ce run, à diagnostiquer en Phase 1)
- Calibrer `raycast_down_dist`, `foot_height`, `lerp_factor` par character

### Phase 5 — Réactivation Rex (Quick)
- Décommenter les 4 lignes dans `spawn_rex_character`
- Run + valider AC4/AC5
- Commit

## Stability Locks impactés
- Aucun direct. L7 (GameSet ordering) inchangé.

## Sensors à observer
- `forgia2_rex_bones_live.json` — bind capture + rotations live
- `forgia2_rex_bones.json` — bind layout à cache.ready (à enrichir avec axes locaux Phase 1)
- `forgia2_walk_pose.json` — gait pose snapshot théorique
- `forgia2_foot_ik.json` — IK liveness (doit exister !)
- `forgia2_auto_rig.json` — Pinocchio backend state

## Cross-refs
- Memory : [reference_arm_rest_z_rad_t_pose_assumption.md](.claude/projects/d--Forgia/memory/reference_arm_rest_z_rad_t_pose_assumption.md)
- Memory : [reference_pinocchio_flat_bone_hierarchy.md](.claude/projects/d--Forgia/memory/reference_pinocchio_flat_bone_hierarchy.md)
- Memory : [reference_rex_glb_is_kael_clone_2026_05_20.md](.claude/projects/d--Forgia/memory/reference_rex_glb_is_kael_clone_2026_05_20.md)
- Memory : [reference_skeleton_template_single_source.md](.claude/projects/d--Forgia/memory/reference_skeleton_template_single_source.md)
- Memory : [reference_stance_offsets_via_quat_compose.md](.claude/projects/d--Forgia/memory/reference_stance_offsets_via_quat_compose.md)
- Memory : [feedback_sensor_first_then_assume.md](.claude/projects/d--Forgia/memory/feedback_sensor_first_then_assume.md)
- Story précédente : 482 (anim system redesign)
- Code à modifier :
  - `crates/forgia-anim-locomotion/src/locomotion.rs` (compose_*, procedural_locomotion)
  - `crates/forgia-anim-locomotion/src/proc_walk.rs` (leg_pose / arm_pose)
  - `crates/forgia-anim-locomotion/src/foot_ik.rs` (diag missing sensor)
  - `crates/forgia-skeleton-template/src/lib.rs` (axes locaux)
  - `crates/forgia-rpg/src/character.rs` (réactivation finale)
  - `assets/genomes/skeleton_*.toml` (variants per character)
