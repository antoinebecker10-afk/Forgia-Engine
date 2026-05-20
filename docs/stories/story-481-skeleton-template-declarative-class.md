# Story-481 — Skeleton Template Declarative Bone Class (suite story-480)

> **Statut** : 🟡 EN COURS — audit hardcode 2026-05-20 a identifié 4 BLOQUANTS pattern "substring classification" dupliqué cross-crate
> **Scale BMAD** : Standard (~6 fichiers : 2 TOML + 2 crates Rust + tests + story)
> **Date création** : 2026-05-20
> **Workspace** : `C:/Users/Antoi/Desktop/Forgia Rewrite` (V2)
> **Origine** : audit hardcode reprise anim Rex 2026-05-20 PM (post-story-480)
> **Cross-refs** : story-480 (single source of truth TOML), [[reference_skeleton_template_single_source]]

## 0. Contexte & justification

### 0.1 Symptôme runtime constaté

Rex.glb au runtime : arm bones placés au centre du torse, **PAS dans la géométrie réelle des bras**. Rotation `arm_L` au runtime ne déforme pas le mesh visible. Visuellement : Rex en pose semi-raptor avec bras inactifs.

### 0.2 Root cause (audit hardcode 2026-05-20)

Pattern récurrent **classification anatomique par substring matching** hardcodé dans **3 crates** :

| Crate | Localisation | Conséquence |
|---|---|---|
| `forgia-skeleton-template` | `classify_bone()` (`lib.rs:216-250`) — `n.starts_with("thigh")`, `n.contains("arm")` | `rescaled_for_landmarks` applique mauvaise formule Y si Meshy nomme `leg_l` au lieu de `thigh_L` |
| `forgia-skeleton-template` | `template_arm_tip_x_abs` calc (`lib.rs:331-339`) — `filter contains("arm"\|"hand")` | `arm_scale` fallback à 0.10 si arm bones ne matchent pas → bras pas déployés latéralement |
| `forgia-skeleton-embedder` | `is_spine_xz`/`is_leg_y_lock` (`lib.rs:498-509`) — `starts_with("thigh"\|"shin")` | Locks YXZ par classe ratent → free path walking au lieu de template lock |
| `forgia-rig-topology` | `name_boost(&["thigh", "upleg", "leg"])` (`lib.rs:151-200`) | Penalty/boost L/R via substring — rig EXTERNE Meshy mal classifié |

### 0.3 Pourquoi ça casse pour Rex spécifiquement

Le mesh Rex.glb Meshy est **exporté avec un naming convention** dont on dépend implicitement. Si Meshy change la convention ou si un nouveau character a un naming différent, le pipeline silencieusement reclassifie en `BoneClass::Other` → fallback Vitruvian (0.50 hip, 0.95 head, X arm lateral) → bones placés mal.

**Pattern AAA** (Unreal Skeleton, Unity Avatar, Godot SkeletonProfile) : la classe d'un bone est **déclarée dans l'asset**, pas inférée par substring. C'est ce qu'on doit faire ici.

### 0.4 Conformité règle Forgia

Violation `.claude/rules/no-hardcode.md` : 4 fichiers contiennent des constantes de naming convention qui devraient être en **definition layer** (TOML).

## 1. Vision cible

```
Avant (anti-pattern) :
  TOML : [[bones]] name = "thigh_L"; parent = 0; pos = [-0.10, 0.40, 0.0]
                          ↓
  Rust : if name.starts_with("thigh") { BoneClass::Leg }  ← hardcode substring 4×

Après (story-481) :
  TOML : [[bones]] name = "thigh_L"; parent = 0; pos = [-0.10, 0.40, 0.0]; class = "leg"
                          ↓
  Rust : bone.class                                       ← single source TOML
```

Effets :
- Plus de drift entre crates (1 source declarative)
- Naming Meshy/Mixamo/AccuRig conventions arbitraires fonctionnent (la classe est dans le TOML, pas dans le name)
- Quadruped futur : nouveau TOML avec `class = "leg"` x4 + `class = "tail"` + adapt classes — pas de modif Rust
- Tests régression simplifiés (compare class field directement)

## 2. Plan d'implémentation

### Phase A — `BoneClass` enum public dans `forgia-skeleton-template`

- Rendre `BoneClass` enum **public** + `#[derive(Deserialize)]` + serde `rename_all = "snake_case"` (TOML format `class = "leg"`)
- Ajouter `pub class: BoneClass` field dans `TemplateBone` (Deserialize)
- `#[serde(default)]` pour rétro-compat (TOML sans class → `Other`)

### Phase B — Update builders

- `humanoid()` 20 bones : ajouter `class` pour chaque tuple (hip → Spine, thigh_L → Leg, arm_L → Arm, etc.)
- `biped_lizard()` 20 bones : idem + tail_01-04 → Tail

### Phase C — Refactor consumers

**forgia-skeleton-template/src/lib.rs** :
- `rescaled_for_landmarks_with_torso` : `let class = bone.class` au lieu de `classify_bone(&bone.name)`
- `template_arm_tip_x_abs` : `filter |b| matches!(b.class, BoneClass::Arm)` au lieu de `contains("arm")`
- Supprimer `fn classify_bone(name: &str)` (mort)

**forgia-skeleton-embedder/src/lib.rs** :
- `is_spine_xz`, `is_spine_x_only`, `is_leg_y_lock` : prennent `bone.class` au lieu de `bone.name`
- Drop substring matching dans `embed_one_chain`

### Phase D — Update TOML

`assets/genomes/skeleton_humanoid.toml` (20 bones) :
- hip/spine_lower/spine_mid/chest/neck → `spine`
- head → `head`
- clavicle_L/R, arm_L/R, forearm_L/R, hand_L/R → `arm`
- thigh_L/R, shin_L/R, foot_L/R → `leg`

`assets/genomes/skeleton_biped_lizard.toml` (20 bones) :
- Idem + tail_01-04 → `tail`
- Pas de clavicle pour BipedLizard

### Phase E — Tests

- Update `assert_humanoid_toml_matches_builder_fixture` pour comparer `class` field
- Update `validate()` : un `Hip` n'est pas obligé d'avoir class==Spine (root) mais doit avoir class déclarée
- Ajouter test `bone_class_is_declarative_not_inferred` : un TOML avec `name="leg_l" class="leg"` → BoneClass::Leg même si starts_with("thigh") return false

## 3. Critères d'acceptance

- [ ] `BoneClass` enum est `pub` + Deserialize avec `snake_case`
- [ ] `TemplateBone.class` field présent, default = Other
- [ ] 0 match `classify_bone(` dans `forgia-skeleton-template/src/lib.rs`
- [ ] 0 match `is_leg_y_lock`/`is_spine_xz` qui font substring matching dans `forgia-skeleton-embedder/src/lib.rs` (ils prennent `BoneClass` en param)
- [ ] Les 2 TOML ont `class` field sur tous les bones (20+20)
- [ ] Builder `humanoid()` + `biped_lizard()` injectent `class`
- [ ] Test régression cross-source vert (TOML class == builder class)
- [ ] Tests headless verts (cargo test -p forgia-skeleton-template -p forgia-skeleton-embedder)
- [ ] 0 clippy warning -D warnings sur 3 crates touchés
- [ ] Non-régression : `forgia-auto-rig` + `forgia-rpg` compilent inchangés

## 4. Out of scope (story future)

- `forgia-rig-topology::analyze` substring `name_boost` (s'applique sur rig EXTERNE Meshy sans TOML class) — story-482
- Resource `AnatomyDetectConfig` pour thresholds anatomy_detect — story-482
- Plugin preload configurable Quadruped — story-482
- Test runtime Rex visuel — dépend story-481 + story-482 combinées

## 5. Locks à respecter

- skinning.rs formule bindpose (lock implicite story-451) — NE PAS TOUCHER
- API publique `SkeletonTemplate` (consumers pinocchio_pipeline + tests) — keep stable, juste enrichir
- TOML format : ajout de field rétro-compatible via `#[serde(default)]`
