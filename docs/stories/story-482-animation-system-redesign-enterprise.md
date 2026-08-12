# Story-482 — Animation System Redesign (Enterprise)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia_foot_ik.json`, fichier `character.rs`, symbole `TestCharacterMode`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> Status : **WIP P1**
> Scale : Enterprise (~2500 LOC net, 6 phases, 4-6 sessions)
> Origin : Audit 360° du 2026-05-20 PM. character.rs monolithique 1585 LOC + 3 hardcodes T-pose + 0 tests intégration + aucun IK.

## Contexte

Le système d'animation actuel (forgia-rpg/character.rs) souffre de :

1. Monolithique — 1585 LOC mélangeant spawn, calibration, walk cycle, sensors, helpers, gizmos
2. T-pose hardcoded (`ARM_STANCE_DROP_RAD = π/2` + 2 autres sites) qui casse dès qu'un rig non-T-pose arrive
3. Aucun IK — pieds flottent/glissent, pas d'adaptation terrain
4. Aucune state machine explicite — enum `TestCharacterMode` ad-hoc
5. 0 tests d'intégration Bevy (12 unit tests `proc_walk` only)
6. Duplications : AABB calibration × 2, bob × 2 (root + pelvis)

Audit complet : voir agents Explore + bevy-specialist + general-purpose lancés le 2026-05-20 PM.

## Vision cible

Architecture multi-crates pose-agnostic + data-driven (TOML stance offsets per rig template), inspirée des consensus AAA :
- Unreal IK Rig (chain mapping, déjà aligné via `forgia-rig-topology`)
- Unity Animation Rigging (2-bone IK + AvatarMask)
- David Rosen Overgrowth (~11 keyframes proc-first, viable solo)
- ozz-animation foot IK (raycast + 2-bone + tilt + lerp 8-15 frames)

Sources URLs sourcées : voir rapport audit en mémoire `reference_animation_system_audit_2026_05_20`.

## Architecture proposée

| Crate | Status | LOC cible | Responsabilité |
|---|---|---|---|
| `forgia-anim-core` | NEW | ~400 | AnimationSet enum + BoneBindCapture + stance offsets TOML |
| `forgia-anim-locomotion` | NEW (P1) | ~600 | ProcWalkCycle + ProcBodyAnim + sensors |
| `forgia-anim-ik` ou `forgia-ik` | scaffold existe | ~200 | wraps `bevy_mod_inverse_kinematics` 0.11 |
| `forgia-anim-state` | NEW | ~150 | FSM enum Rust + transitions TOML |
| `forgia-skeleton-template` | étendre | +50 | stance_offsets_per_class field |
| `forgia-rig-topology` | inchangé | — | chain classification (AAA-aligned) |
| `forgia-skeleton-embedder` | inchangé | — | Pinocchio |
| `forgia-auto-rig` | inchangé | — | pipeline |
| `forgia-rpg/character.rs` | shrink | 1585 → ~400 | reste : spawn + lineup |

## Crates externes adoptées

| Crate | Version | Bevy | Rôle | Justification |
|---|---|---|---|---|
| `bevy_mod_inverse_kinematics` | 0.11.0 | 0.18 ✅ | 2-bone IK + pole target | Standard AAA, seul IK Bevy 0.18 actif maintenu |
| Bevy native `AnimationGraph` | 0.18 | — | Clip/Blend/Add + bone masking | Natif, stable |
| `bevy_animation_graph` | 0.10.0 | 0.18 ✅ | State machine + blend spaces (P5+) | À adopter prudemment (1 mainteneur, 188★), wrap behind interface |

## Crates rejetées

- `bevy_fabrik` (Bevy 0.15 stale)
- `bevy_ik` gschup (Bevy 0.9 abandonné)
- Motion matching custom (over-engineering solo — Ubisoft 100+ ingénieurs)
- Muscle space Unity-like (calibration overhead massif)

## Plan d'implémentation phasé

### Phase 1 — Extract `forgia-anim-locomotion` (current)

**Objectif** : 0 régression visuelle, juste move + re-export.

- [ ] Créer crate `forgia-anim-locomotion`
- [ ] Move `forgia-rpg/src/proc_walk.rs` → `forgia-anim-locomotion/src/proc_walk.rs`
- [ ] Move types depuis `character.rs` : `LocomotionBoneCache`, `ArticulatedBones`, `BonePose`, `ProcBodyAnim`, `LocomotionState`
- [ ] Move systems : `procedural_locomotion`, `procedural_whole_body_anim`, `attach_rex_bone_systems`, `write_walk_pose_sensor`, `write_rex_bones_live_sensor`
- [ ] Move helpers : `compose_swing`, `slerp_to_bind`, `compose_stance_swing`, `slerp_to_stance`, `apply_pitch`
- [ ] Move constants : `WALK_FREQ`, `WALK_BOB_AMP`, `LEAN_FORWARD_AMP`, etc.
- [ ] Plugin `AnimLocomotionPlugin` exposant les systems
- [ ] forgia-rpg dépend du nouveau crate + re-exporte ce qui était public
- [ ] `cargo check forgia-rpg` clean
- [ ] Runtime smoke test : RPG mode marche comme avant Phase 1

**Acceptance** : `forgia-rpg/character.rs` < 700 LOC, 0 régression visuelle, sensors continuent d'écrire.

### Phase 2 — `forgia-anim-core` + stance offsets data-driven

- [ ] Créer crate `forgia-anim-core`
- [ ] Définir `AnimationSet` enum (Idle/Walk/Run/Jump/Aim/Dead pour V1)
- [ ] `AnimationLayerState` Resource (poids par couche)
- [ ] Étendre `SkeletonTemplate` avec `stance_offsets_per_class: HashMap<BoneClass, Vec3>` (euler degrees per class)
- [ ] TOML stance_offsets dans `assets/genomes/skeleton_humanoid.toml` (Arm = Z(±90°), autres = 0)
- [ ] Supprimer `ARM_STANCE_DROP_RAD` constant, lire depuis template
- [ ] `BoneBindCapture` plugin centralisé (au lieu d'inline dans attach_rex_bone_systems)

**Acceptance** : `ARM_STANCE_DROP_RAD` retiré, stance offsets hot-reloadables via TOML, Kael-pose et T-pose meshes supportés sans recompiler.

### Phase 3 — Foot IK via `bevy_mod_inverse_kinematics`

- [ ] Populer scaffold `forgia-ik` (ou créer `forgia-anim-ik`)
- [ ] Wrap `bevy_mod_inverse_kinematics::IkConstraint` derrière `FootIkSolver`
- [ ] Algo standard : raycast vertical → target = hit + offset → 2-bone IK hip→knee→ankle → tilt ankle vers normale → lerp 8-15 frames
- [ ] TOML `foot_ik.toml` (raycast distance, tilt max, lerp speed)
- [ ] Sensor `forgia_foot_ik.json`
- [ ] Per-character toggle, désactivé en LOD distant

**Acceptance** : pieds collés au sol sur slope ±35°, pas de slide/float observable.

### Phase 4 — `forgia-anim-state` FSM

- [ ] Créer crate `forgia-anim-state`
- [ ] FSM enum Rust simple (Idle/Walk/Run/Jump/Aim/Dead)
- [ ] Transitions data-driven : TOML `state_transitions.toml`
- [ ] Retire `TestCharacterMode` ad-hoc
- [ ] Sensor `forgia_anim_state.json`

**Acceptance** : transitions état explicites observables, ≤10 états V1, pas de bevy_animation_graph (risque mainteneur).

### Phase 5 — Animation layering Bevy natif

- [ ] Utiliser `add_clip_with_mask` natif Bevy 0.18 pour upper/lower body split
- [ ] Bone groups : lower_body, upper_body, head, hands
- [ ] Aim overlay (additive sur upper body) déclenché par état Aim

**Acceptance** : aim overlay fonctionne sans casser walk cycle lower body.

### Phase 6 (futur) — Retargeting chain-based

- [ ] Utiliser `forgia-rig-topology` chains comme retargeting layer
- [ ] Map chain-to-chain entre rigs (humanoid → biped_lizard partial)
- [ ] Tests cross-rig

## Tests

- Unit tests purs (no Bevy World) déjà 12 dans `proc_walk` — garder
- Phase 1+ : ajouter tests intégration Bevy :
  - `App::new()` + `MinimalPlugins` + `AnimLocomotionPlugin`
  - Spawn entity avec mock `RexCharacter` + `LocomotionBoneCache` ready
  - Tick `procedural_locomotion` 60 frames @ speed=1.5
  - Assert bones rotations within expected range
- Target : ≥10 integration tests d'ici Phase 4

## Sensors maintenus / ajoutés

| Sensor | Phase | Contenu |
|---|---|---|
| `forgia_walk_pose.json` | existe | gait, bobs, leg/arm degrees |
| `forgia_rex_bones.json` | existe | bind capture one-shot |
| `forgia_rex_bones_live.json` | existe | current rotations 10Hz |
| `forgia_foot_ik.json` | P3 | per-foot raycast hit + target Y + tilt deg |
| `forgia_anim_state.json` | P4 | current state + last transition |

## Risques + mitigations

| Risque | Mitigation |
|---|---|
| `bevy_animation_graph` 1 mainteneur | Skip en V1, FSM enum Rust suffit |
| Extraction P1 casse call sites cross-crate | P1 = juste move + re-export, pas de logique changée |
| Foot IK coût per-frame | Per-character toggle, off en LOD distant |
| TOML stance offsets nouveau format | Defaults équivalents au hardcode actuel, validation hot-reload |
| Tests intégration manquants | Ajouter à chaque phase, target 10+ |

## Acceptance globale story

- [x] Audit document complet (ce fichier)
- [ ] Phase 1 livrée + verify clean
- [ ] Phase 2 livrée + ARM_STANCE_DROP_RAD supprimé
- [ ] Phase 3 livrée + foot IK fonctionnel
- [ ] Phase 4 livrée + FSM enum opérationnelle
- [ ] Phase 5 livrée + aim overlay fonctionnel
- [ ] `character.rs` final < 500 LOC
- [ ] ≥10 integration tests Bevy
- [ ] 0 hardcode T-pose dans le codebase
- [ ] 5 sensors anim cohérents

## Sources externes

Voir mémoires Forgia :
- `reference_animation_system_audit_2026_05_20.md` (à créer après story)
- `reference_aaa_animation_industry_patterns_2026_05_20.md` (à créer)
- Bevy 0.18 release notes : https://bevy.org/news/bevy-0-18/
- bevy_animation_graph : https://github.com/mbrea-c/bevy_animation_graph
- bevy_mod_inverse_kinematics : https://github.com/Kurble/bevy_mod_inverse_kinematics
- Naughty Dog GDC : https://www.naughtydog.com/blog/2014_naughty_dog_gdc_talks
- Ubisoft Motion Matching SIGGRAPH 2020 PDF : https://theorangeduck.com/media/uploads/other_stuff/Learned_Motion_Matching.pdf
- Unreal IK Rig : https://dev.epicgames.com/documentation/en-us/unreal-engine/ik-rig-in-unreal-engine
- Unity Humanoid Avatar : https://docs.unity3d.com/Manual/AvatarCreationandSetup.html
- ozz-animation foot IK : https://guillaumeblanc.github.io/ozz-animation/samples/foot_ik/
- David Rosen GDC 2014 : https://www.youtube.com/watch?v=LNidsMesxSE

---

*Adoptée 2026-05-20 PM, début Phase 1.*
