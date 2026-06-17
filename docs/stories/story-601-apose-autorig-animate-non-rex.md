# Story-601 — Support A-pose : auto-rig + anim de personnages non-Rex (arms-down)

**Statut** : À FAIRE (rédigée 2026-06-16)
**Niveau BMAD** : Enterprise (4 crates, multi-incréments, capacité moteur)
**Date** : 2026-06-16
**Cible** : FORGE (banc d'outils anim/rig) qui reflue dans le ship. **Cas de test = Cyber** (androïde cyberpunk, mesh A-pose) pour la démo anim. Vision : *« importe ton asset, l'IA l'anime »* — or aujourd'hui le moteur **n'anime que Rex** (BipedLizard T-pose, hardcodé). Cette story exécute le P0/P1 de l'audit anim 2026-06-07.

**Origine** : session 2026-06-16, tentative d'animer `Cyber.glb` en mode CyberCity. Diagnostic sensors complet ci-dessous. Décision user : construire le support A-pose proprement (vs régénérer en T-pose).

---

## Contexte vérifié (sensors + code, 2026-06-16)

L'auto-rig **réussit** sur Cyber (`forgia2_auto_rig.json` : `state: rigged`, **20 os**, template Humanoid, `looks_humanoid: true`, `has_tail: false`, height 1.898). Mais Cyber reste **statique**. 4 blocages identifiés, tous sourcés :

| # | Blocage | Preuve | Cause |
|---|---|---|---|
| B1 | **Bind T-pose ≠ mesh A-pose** | `forgia2_rex_bones` arm head `[-0.11,0,0]`→main `[-0.404,0,0]` (Y constant = horizontal). Cyber largeur X=0,68 (demi 0,34) vs Rex 1,9 = bras baissés | `skeleton_humanoid.toml` arm_L/forearm_L/hand_L à **Y=0,78 constant** (T-pose Vitruvien). Skinning bind sur T-pose ≠ mesh arms-down → bind faux + rings hors mesh |
| B2 | **Skinning skippé** | `forgia2_auto_rig.json` : `total_meshes_skinned: 0`, `failed: 42823`, `verts_skinned: 0` | `skinning.rs:320` cap `max_verts_per_mesh: 200_000` ; Cyber = **281 427 verts** → skip silencieux |
| B3 | **Locomotion jamais testée hors Rex** | `forgia_anim_full.json` : `cache_not_ready, gave_up: true` (120 frames) | Seul Rex (BipedLizard) a été animé. Résolution d'os locomotion = noms hardcodés sans fallback (audit) ; noms template Humanoid (`arm_L`) ≠ noms attendus cache (`left_arm`) ? + timeout 120 frames trop court pour mesh lourd |
| B4 | **Calibration per-perso = Rex** | `locomotion.rs:845` `FLIP_FORWARD=-1`, genou 92° digitigrade, `TAIL_IDLE_*` (queue), hunch idle | Const mouvement hardcodées Rex (audit §1, ~60 const). Humanoid = plantigrade, sans queue |

**Faisabilité confirmée** : `pinocchio_pipeline.rs:125` — le template demandé par le caller est **honoré, jamais écrasé** par `looks_humanoid()`. Et `skeleton_humanoid.toml:6-8` anticipe déjà les meshes arms-down (*« si mesh est déjà en pose game, mettre stance à zéros »*). Donc diriger l'auto-rig vers un template A-pose = supporté.

**Déjà fait cette session** (scaffolding, non commité) : `character.rs:spawn_rex_character` spawn Cyber.glb (template Humanoid actuel, offset -0,95) en `GameMode::CyberCity` ; overlay rig (rings+transparence) activé en CyberCity (`enable_rig_overlay`/`disable_rig_overlay`). Incrément 1 changera le template Humanoid → HumanoidApose.

---

## Incréments (livrables indépendamment, testables par sensor)

### Incr 1 — Template A-pose (fit du bind/rings) — couche DATA

- **Nouveau** `assets/genomes/skeleton_humanoid_apose.toml` : copie de `skeleton_humanoid.toml` avec bras **pointant vers le bas** (clavicle au niveau épaule, puis arm/forearm/hand avec X≈±0,18 et **Y décroissant** ~0,80→0,30 le long du corps) + **`stance_offsets` tous à zéro** (mesh déjà en pose game). Tuner les angles à l'œil sur Cyber (hot-reload Shift+F12).
- **Enum** : `SkeletonTemplateId::HumanoidApose` (+ `asset_path`, `as_str`, preload `forgia-skeleton-template/lib.rs:709`) ; `AutoRigTemplate::HumanoidApose` (`forgia-auto-rig/lib.rs:146`) ; mapping `auto_rig_to_skeleton_template_id` (`pinocchio_pipeline.rs:293`) ; **mettre à jour le test sentinelle** `pinocchio_pipeline.rs:414`.
- **Wire** : `character.rs` branche Cyber sur `AutoRigTemplate::HumanoidApose` + `SkeletonTemplateId::HumanoidApose` (au lieu de Humanoid).
- **Livrable** : avec l'overlay rig ON, les **rings/squelette suivent les bras baissés** de Cyber (plus de bras horizontaux dans le vide). Test : visuel + `forgia2_rex_bones.json` arm tip_local en -Y.

### Incr 2 — Débloquer le skinning (mesh lié au squelette)

- **Décimer Cyber** à ~30-50k verts (gltf-transform, `Cyber_lod.glb`, sans toucher l'original) → sous le cap 200k. (Option vision long terme : étape de décimation auto dans le pipeline d'import — hors scope ici.)
- **Livrable** : `forgia2_auto_rig.json` `total_meshes_skinned > 0`, `verts_skinned > 0` ; le mesh **se déforme** quand on bouge les os. Test : `forgia2_skinning_weights.json` (count_any/primary par os, cf [[reference_skinning_weights_sensor_diagnosis]]).

### Incr 3 — Locomotion sur template Humanoid (cache + proc-walk)

- **Résoudre la cache** : `attach_locomotion_bones` (`locomotion.rs:325`) doit retrouver les os du template Humanoid. Vérifier la table de noms (Humanoid `arm_L`/`thigh_L` vs attendus `left_arm`/`left_leg`) → ajouter mapping/fallback. Augmenter `GIVEUP_FRAMES` (`locomotion.rs:630`) si mesh lourd met >2s à s'instancier.
- **Livrable** : `forgia_anim_full.json` `cache_ready: true`, `gave_up: false`, `topology` ~`1111…`, os pilotés. Test : `gait.is_moving=true` en mouvement.

### Incr 4 — Gait/calibration Humanoid (marche naturelle) — couche DATA (audit P0)

- **Router le mouvement via genome per-perso** (audit P0 / [[reference_gait_genome_data_driven]] story-579) : `gait_humanoid.toml` (plantigrade, genou ~normal pas 92°, **pas de queue**, hunch idle ≈0). `FLIP_FORWARD` per-perso (constante Rex à sortir).
- **Livrable** : Cyber **marche naturellement** (jambes/bras contra-latéraux, pas de pédalage inversé, pas d'idle de queue). Test : `forgia_anim_full.json` gait cohérent + visuel.

---

## QA / Acceptance globale

- [ ] check + clippy 0 sur les 4 crates touchées ; test sentinelle enum (Incr 1) à jour
- [ ] Pas de régression Rex (RPG) ni lineup T-pose (Dorin/Mira) — ils gardent Humanoid/BipedLizard, JAMAIS basculés sur HumanoidApose
- [ ] Runtime final : entrée CyberCity → Cyber **rigué A-pose + skinné + marche** en ZQSD
- [ ] Sensors : `forgia2_auto_rig.json` (skinned>0), `forgia_anim_full.json` (cache_ready, gait), `forgia2_skinning_weights.json`

## Multi-terminal

Crates anim (`forgia-auto-rig`, `forgia-skeleton-template`, `forgia-anim-locomotion`, `forgia-rpg`) **hors diff** de l'autre terminal (lui : debug/roguelite/observability/ui-lib) → collision-safe au 2026-06-16. Re-vérifier `git diff HEAD --name-only` au moment d'implémenter.

## Hors scope (stories suiveuses)

- Décimation auto dans le pipeline d'import (tout asset lourd)
- Détection de pose géométrique (auto T-pose vs A-pose sans template explicite)
- Locomotion directionnelle (strafe/recul), foot IK appliqué, blend idle↔walk, anim attaque/mort (audit P1/P2)
- Généralisation N-membres (quadrupède/ailé)

## Cross-refs

- Audit fondateur : `docs/audits/audit-2026-06-07-animation-system.md` (P0/P1)
- [[reference_v2_locomotion_single_character_core]] (mono-perso .single, animer un 2e perso = système séparé ou refonte)
- [[reference_skinning_weights_sensor_diagnosis]], [[reference_gait_genome_data_driven]]
- `.claude/rules/no-hardcode.md` (B4 = const Rex → couche definition), `concept-first.md` (concept `camera`/anim layer)
