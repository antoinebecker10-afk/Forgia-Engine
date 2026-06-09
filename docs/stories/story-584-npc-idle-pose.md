# Story-584 — Pose idle « bras le long » des PNJ RPG

> **Statut** : code-complete (2026-06-08), NON COMMITÉ, à valider runtime.
> **Scope BMAD** : Standard (1 nouveau fichier + 2 edits lib.rs, 0 Cargo change).
> **Track** : FORGE (outils anim/rig qui refluent) — généralisation rig Rex → PNJ.

## Demande user

« Dans le RPG, faire le même travail anim/bones/rings aux autres PNJ qu'à Rex. »
Scope choisi (AskUserQuestion) : **idle + posture naturelle** (sortir les PNJ de
la T-pose), puis approche **C — pose statique bras-le-long** (mini, zéro risque Rex).

## Blocage d'architecture découvert (clé)

Le cœur `procedural_locomotion` est **mono-personnage** :
`q_cache.single()` + `q_driver.single_mut()` (locomotion.rs:661/678). Ajouter
`LocomotionTarget` aux PNJ → >1 target → `.single()` échoue → **l'anim de Rex
meurt**. Animer N persos = refonte multi-perso (incr.1b, risque Rex) OU système
séparé. → On a choisi le séparé statique.

## Solution livrée

- **Nouveau** `crates/forgia-rpg/src/npc_pose.rs` : `sys_pose_npcs_idle`
  - gate `With<crate::Npc>, Without<NpcPosed>` → **Rex (RexCharacter, pas Npc) jamais touché**,
  - classifie les os via `forgia_rig_topology::analyze_rig_topology` (géométrique, robuste aux noms),
  - compose `StanceOffsets::humanoid_tpose()` (= miroir exact `skeleton_humanoid.toml` : arm_l Z+90°, arm_r Z-90°) PAR-DESSUS la rest pose des os arm/leg/spine,
  - 1-shot idempotent (`NpcPosed`), retry tant que l'auto-rig async n'a pas spawné les os.
- **lib.rs** : `pub mod npc_pose;` + enregistrement `Update` / `GameSet::Movement` / `run_if(Rpg)` (système SÉPARÉ du chain locomotion Rex).
- **Zéro** `LocomotionTarget`, zéro édition du cœur locomotion, zéro édition de `character.rs`/`worldgen_village.rs` (claimés par l'autre terminal).

## Critères d'acceptation

- AC1 — Les 4 PNJ ne sont plus en T-pose (bras horizontaux) → bras le long du corps. ✅ (à valider runtime)
- AC2 — Rex inchangé (anim + idle intacts). ✅ (gate `Npc` exclut Rex ; `cargo check -p forgia` OK)
- AC3 — `cargo clippy -p forgia-rpg` 0 warning. ✅
- AC4 — Idempotent (pose 1×, pas de re-compose chaque frame). ✅ (`NpcPosed`)

## Limites / suite

- Statique (pas de respiration idle) — c'était le scope « mini » choisi.
- Pas data-driven du template (utilise le helper `humanoid_tpose()` qui mirror le
  TOML actuel). Upgrade = lire le template via `SkeletonTemplateRegistry` (cf
  `apply_stance_offsets_from_template`) si on tune la stance PNJ dans le TOML.
- Animation/marche + respiration des PNJ = option B (système séparé) ou A (refonte
  multi-perso, incr.1b) si demandé plus tard.
