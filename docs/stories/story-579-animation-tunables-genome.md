# Story-579 — Animation tunables genome (mouvement data-driven, multi-perso)

**Statut** : Incr.1 DONE (crate-validé : check + 3 tests gait_genome + clippy ✓ ;
runtime « Rex identique » à confirmer). Incr.1b/2/3/4 = PLANNED.
**Implémenté incr.1 (2026-06-07)** : `gait_genome.rs` (NEW) — `GaitGenome` (7 paires
WALK/RUN, serde defaults = miroir EXACT des const proc_walk) + global lazy-load
(`gait()` / `reload_gait()`, lecture hot-path sans alloc/fs par frame, pattern story-576)
+ 3 tests dont `default_mirrors_proc_walk_consts` (zéro régression). `proc_walk::for_speed`
lit le genome (signature inchangée → 0 churn des 3 call-sites). `config/genomes/anim/
gait_biped_lizard.toml` + `gait_humanoid.toml` (miroir). Cargo.toml += serde, toml.
Choix : global lazy (1 perso animé = Rex) au lieu d'un registry per-perso, pour
**zéro churn forgia-rpg** (contendu autre terminal). Per-perso = incr.1b ; Shift+F12
hot-reload (via `reload_gait()` déjà fourni) = incr.2.
**Scale** : Standard→Enterprise (forgia-anim-locomotion + forgia-rpg + config genomes ;
potentiellement forgia-skeleton-template pour le binding per-character).
**Date** : 2026-06-07
**Lignée** : `docs/audits/audit-2026-06-07-animation-system.md` → finding #1 (hardcode) +
#2 (réutilisabilité). Le squelette/pose sont déjà data-driven ; le MOUVEMENT ne l'est pas.

## Problème

~60 constantes de mouvement (gait, idle, root-motion, foot IK) sont des `const` Rust
tunées pour Rex, sans aucun read-path genome. `GaitTunables::for_speed()` (proc_walk.rs:120)
et `FootIkConfig` (foot_ik.rs:27) **ressemblent** à des conteneurs data mais sont sourcés de
literals. Animer un 2e personnage aux proportions/style différents = éditer Rust + recompiler
→ violation `no-hardcode.md`. Le plumbing data (`Genome<T>`, registry, hot-reload, sensor)
existe déjà et est prouvé (pattern `skeleton_*.toml` / story-576 terrain_shape).

## Objectif

Router les tunables de mouvement via genome TOML, référencés per-character à côté de
`LocomotionTemplate`, hot-reloadables Shift+F12 — sans changer le comportement de Rex
(les TOML par défaut = miroir exact des const actuelles → zéro régression).

## Increments proposés

### Incr.1 — `GaitTunables` data-driven
- `assets/genomes/anim/gait_biped_lizard.toml` (+ `gait_humanoid.toml`) : tous les champs
  de `GaitTunables` (stride_per_m walk/run, amp_thigh/arm, knee/ankle flex, stance_frac,
  pelvic yaw/roll/bob, speed thresholds, clavicle_protract, spine_counter_rot, elbow_rest_flex).
- `GaitGenome` + `load_or_default()` + tests (pattern `TerrainShapeGenome`).
- Read-path : `GaitTunables::for_speed` lit le genome au lieu des const module ; const
  deviennent les `default_*()` (source des fallbacks).
- Binding per-character : `LocomotionTemplate` ou un nouveau `GaitTemplateId` réfère le genome.

### Incr.2 — `IdleTunables` data-driven (personnalité)
- `idle_<character>.toml` : hunch, arm_abduct, breath freq/amp, arm_sway, elbow_breathe,
  tail_idle (freq/amp/phase), head_look (amp + freqs + neck split).
- Sort la personnalité Rex (locomotion.rs:713-842) du code engine.
- ⚠️ `TAIL_IDLE_*` ne s'applique que si `tail_chain` non vide (déjà le cas) → un perso sans
  queue ignore proprement.

### Incr.3 — `FootIkConfig` data-driven
- `foot_ik.toml` : raycast dists, foot_height, lerp, max_tilt (foot-type-specific :
  digitigrade vs plantigrade vs sabot).
- Ferme la dette du commentaire foot_ik.rs:25.

### Incr.4 — Enforcement convention de noms (anti-footgun)
- `validate()` (skeleton-template) vérifie la présence des os requis par `BoneClass`
  (thigh/shin/foot/arm/forearm/hand/clavicle/neck/head) → warning explicite si un os
  attendu manque, au lieu d'un `BonePose{entity:None}` silencieux.

## Invariants à protéger (Locks)

- **Zéro régression Rex** : les TOML par défaut = valeurs actuelles exactes (test miroir,
  comme story-575/576). Rex doit bouger pareil après migration.
- Le hot path reste 0-alloc : snapshot du genome 1× (pas de lock/lecture per-frame/per-os) —
  pattern `WORLD_BIOME_AMPLITUDES` (story-576 incr.6).
- Hot-reload via AssetEvent (comme StanceOffsets), pas de re-rig (largeur d'os = re-rig, mais
  les tunables mouvement = runtime).

## AC

- [ ] `gait_*.toml` chargé au boot (log), `GaitTunables` lit le genome, tests miroir verts
- [ ] `idle_*.toml` : personnalité Rex sortie du code engine, miroir exact
- [ ] `foot_ik.toml` : FootIkConfig data-driven
- [ ] `validate()` warn sur os manquant par classe
- [ ] Runtime : éditer un TOML + Shift+F12 → mouvement change sans rebuild ; Rex identique aux défauts
- [ ] Animer un 2e personnage = créer ses TOML (gait+idle+foot_ik) SANS toucher Rust

## Hors scope (→ P1/P2 audit, stories suiveuses)

- Généricité body-plan (quadrupède/ailé : `ArticulatedBones`/`RigTopology` N-membres).
- Foot IK appliqué (P3b), locomotion directionnelle, anim NPCs/ennemis, additive layers,
  hybride AnimationGraph.
