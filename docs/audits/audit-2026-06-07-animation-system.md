# Audit — Système d'animation Forgia V2 (2026-06-07)

> Audit multi-angle (4 sous-agents + recherche web best-practices) du système
> d'animation procédurale. Read-only, working tree non-commit (rig WIP). Demande user :
> « propre ? pas de hardcode ? réutilisable pour d'autres personnages ? ce qui manque vs
> l'industrie ? »

## Verdict

Couche **données du squelette** = qualité AAA (TOML genome, registry, hot-reload,
validation, test régression). **Runtime propre** (0 alloc hot-path, scheduling correct,
crates acycliques). **MAIS la couche MOUVEMENT est 100 % hardcodée pour Rex**, et
l'architecture est **structurellement biped-only**. Animer un autre perso (même un 2e
biped) = éditer + recompiler du Rust. Le jeu prioritaire (Roguelite FPS) a **0 perso animé**.

| Question | Réponse |
|---|---|
| Pas de hardcode ? | ❌ Squelette/pose data-driven ✅, mais ~60 const mouvement Rust sans read-path genome |
| Réutilisable autres persos ? | ⚠️ Autre biped : quasi data-only. Quadrupède/ailé : chantier code 3 crates |
| Best practices / manques ? | Plusieurs systèmes standards absents (foot IK appliqué, blend tree, additive layers, anim ennemis, hooks attaque/mort) |

## ✅ Points forts (à garder)

- Template squelette data-driven : `forgia-skeleton-template` (TOML + registry + validate +
  test régression TOML↔builder lib.rs:1358-1383), hot-reload Shift+F12.
- `StanceOffsets` per-character en TOML, hot-reload AssetEvent (locomotion.rs `apply_stance_offsets_from_template`).
- Hot path : 0 alloc régime établi ; BFS name-lookup construit **1×** (gate `!cache.ready`,
  locomotion.rs:335) ; sensors throttlés ; filtres `Without<>` sains.
- Scheduling correct : `Update/GameSet::Movement` pose les os AVANT skinning `PostUpdate`
  (forgia-rpg/lib.rs:197-221).
- Crates acycliques : topology→template→embedder→auto-rig→locomotion ; ik/secondary feuilles.
- Backend Pinocchio morphology-agnostic + skinning nearest-bone générique.
- Principe « compose-not-overwrite » → hybride AnimationGraph futur possible sans gros refactor.

## ❌ 1. Hardcode — couche mouvement non data-driven

~60 const mouvement en Rust, zéro routée via genome. `forgia-anim-locomotion` dépend de
`genome-core` SEULEMENT pour le squelette ; aucun param mouvement routé.

- Gait (24 const) : `STRIDE_PER_M`, `AMP_THIGH/ARM`, `KNEE_FLEX_PEAK` (92° digitigrade),
  `PELVIC_*`, seuils vitesse — proc_walk.rs:31-101.
- Idle/personnalité (15 const inline) : `IDLE_FORWARD_HUNCH`, `IDLE_ARM_ABDUCT_DEG`,
  `IDLE_BREATH_*`, `TAIL_IDLE_*`, `HEAD_LOOK_AMP` — locomotion.rs:713-842.
- Root motion : `LEAN_FORWARD_AMP`, `ROLL_WADDLE_AMP`, squash + divisors `speed/3.0` —
  character.rs:731-840.
- `GaitTunables::for_speed()` a la forme d'un conteneur tunables mais hardcode chaque champ
  (proc_walk.rs:120-134). `FootIkConfig` = Default-literals, migration TOML jamais faite
  (foot_ik.rs:25).
- Magie Rex dans le code engine : `FLIP_FORWARD=-1.0` (axe GLB Rex), `LATERAL_AXIS=X`
  (« confirmé sur Rex »), `TAIL_IDLE_*` (assume une queue), genou 92° digitigrade.

→ Violation `no-hardcode.md` (per-character numeric → couche definition).

## ⚠️ 2. Réutilisabilité — biped-only structurellement

- `ArticulatedBones` = struct biped figée (2 bras/2 jambes/1 queue/1 nuque/1 tête), pas de
  4 pattes ni ailes — locomotion.rs:211-235.
- `RigTopology` = 1 `Option` par membre + greedy → quadrupède perd 2 jambes —
  rig-topology/src/lib.rs:33-46, 131-159.
- Gait math biped figé (`leg_L@gait`, `leg_R@gait+0.5`, bras opposés) — proc_walk.rs:178-184, 253.
- `AutoRigTemplate` = 2 variantes ; `SkeletonTemplateId::Quadruped` existe mais PAS de
  `skeleton_quadruped.toml` (pinocchio_pipeline.rs:291).
- Résolution d'os = noms en dur sans fallback (forearm/hand/clavicle/shin/foot/neck) →
  os renommé = jamais animé, sans erreur (footgun) — locomotion.rs:483-498 ; `validate()`
  ne vérifie pas les noms.
- NPCs rigés + skinnés mais JAMAIS animés (pas de `LocomotionTarget`) — character.rs:295 vs :135.

→ Autre biped = quasi data-only. Quadrupède/ailé = projet code (rig-topology + anim-locomotion + auto-rig/anatomy_detect).

## 🕳️ 3. Features manquantes vs jeu qui ship

| Feature | État | Importance |
|---|---|---|
| Anim ennemis/NPC | ❌ MANQUANT | 🔴 critique (Roguelite FPS = 0 perso animé) |
| Hook anim d'attaque | ❌ MANQUANT | 🔴 critique |
| Mort / ragdoll | ❌ MANQUANT | 🔴 critique |
| Hit-reaction / flinch | ❌ MANQUANT | 🟠 haute |
| Foot IK appliqué | ⚠️ calculé puis jeté (foot_ik.rs:236 TODO P3b) | 🟠 haute |
| Locomotion directionnelle (strafe/recul) | ❌ MANQUANT | 🟠 haute |
| Blend idle↔walk | ⚠️ branche dure (pop) | 🟠 haute |
| Additive layers (aim/carry) | ❌ MANQUANT | 🟡 moyenne |
| Turn-in-place | ❌ MANQUANT | 🟡 moyenne |
| `forgia-ik` | ⚠️ STUB (output jeté) | — |
| `forgia-secondary-motion` | ⚠️ réel mais désactivé (`TAIL_USE_SPRINGBONE=false` + bug axe +Y solver.rs:153-160) | — |

## 🌐 4. Best practices industrie (recherche web)

- Blend trees + state machine : standard = state machine (Locomotion/Combat/Swim) + blend
  continu, 8-16 échantillons directionnels + idle. Forgia = slerp manuel.
- Additive layers : base + offsets additifs (aim, hit-react, facial) — absent.
- Foot IK = colonne vertébrale de toute loco procédurale — calculé mais non appliqué.
- Retargeting par convention de noms (Mannequin-compatible) — Forgia a une convention
  interne non centralisée, non enforced.
- LOD anim (hero full / mid 1D / distant single-clip) — pertinent pour les foules ennemies.
- Hybride procédural + `AnimationGraph` : Forgia = 0 `AnimationPlayer`/`AnimationClip` →
  clips artistes (Mixamo/Blender) injouables. Le « compose-not-overwrite » rend l'hybride
  faisable. Sources : MoCap Online, Little Polygon, Magic Media.

## 🧭 Roadmap priorisée

**P0 — débloque réutilisabilité, faible effort, pattern prouvé (plumbing data existe) :**
1. **Router le mouvement via genome** (`gait_<character>.toml` + `idle_*` + `foot_ik.toml`,
   peuplés via registry, réf per-character à côté de `LocomotionTemplate`). → **story-579**.
2. Enforcer la convention de noms (`validate()` vérifie les os requis par classe).
3. Sortir la personnalité Rex (hunch, abduction, head-look, tail) du code engine vers le genome.

**P1 — généricité body-plan + features ship :**
4. Appliquer le foot IK (finir P3b).
5. Animer NPCs/ennemis (idle+walk procédural) — critique Roguelite.
6. Locomotion directionnelle (strafe/backpedal) — critique FPS.
7. Généraliser `ArticulatedBones`/`RigTopology` en N-membres.

**P2 — architecture long terme :**
8. Hybride `AnimationGraph` pour clips non-locomotion (attaques/morts).
9. Système de couches additives (aim offset, hit-react).
10. Activer/fixer `secondary-motion` + brancher `forgia-ik` (look-at, hand IK).
