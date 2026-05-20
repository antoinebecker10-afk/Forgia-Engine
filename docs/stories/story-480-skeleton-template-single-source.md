# Story-480 — Skeleton Template Single Source of Truth (AAA conformance)

> **Statut** : 🟡 PLAN — research industrie + audit deep terminés, attente validation avant /implement
> **Scale BMAD** : Enterprise (>10 fichiers touchés, suppression cross-crate, story + plan + checklist obligatoires)
> **Date création** : 2026-05-20
> **Workspace** : `C:/Users/Antoi/Desktop/Forgia Rewrite` (V2)
> **Origine** : audit 2026-05-20 (session reprise anim Rex) — détection 3 sources de vérité pour skeleton templates
> **Stack cible** : Bevy 0.18.1, bevy_rapier3d 0.33
> **Cross-refs** : [[reference-auto-rig-template-creation-process]], [[session-pause-2026-05-18-rex-anim-wip]], story-440 (auto-rig Phase 1A-1C), story-451 (skinning), story-454 (anim debug)

## 0. Contexte & justification

### 0.1 Problème détecté

Audit du chemin runtime Rex révèle **3 sources de vérité** pour le skeleton template :

| # | Localisation | Forme | Statut runtime |
|---|---|---|---|
| (1) | `assets/genomes/skeleton_humanoid.toml` (20 bones avec hand_L/R), `skeleton_biped_lizard.toml` (20 bones) | Position absolue normalisée Y∈[0,1] | **ACTIF** (chargé via `Genome<SkeletonTemplate>`) |
| (2) | `crates/forgia-skeleton-embedder/src/lib.rs::humanoid()` (18 bones), `biped_lizard()` (20 bones drift) | Position absolue normalisée | **ACTIF au boot** (fallback si TOML pas ready, race 1-3 frames) |
| (3) | `crates/forgia-auto-rig/src/lib.rs::HUMANOID_BONES`, `BIPED_LIZARD_BONES` (18 bones each, `local_translation` delta) | Position locale delta vs parent | **DEAD CODE** (gated derrière `AutoRigBackend::TemplateFit`, défaut = `PinocchioV1`) |

**Drifts mesurés sources (1) vs (2)** :
- BipedLizard shin : TOML `y=0.20 z=+0.12` (digitigrade Rex) vs fallback `y=0.28 z=+0.05` (drift copy/paste Humanoid)
- Humanoid : TOML 20 bones (avec hand_L/R) vs fallback 18 bones (sans hand)
- BipedLizard hip : TOML `z=+0.02` vs fallback `z=0.0`

**Drift structurel sources (1)/(2) vs (3)** :
- (3) utilise `local_translation` (delta parent) au lieu de position absolue → conventions différentes
- (3) référence parents par `&'static str` au lieu d'index
- (3) BipedLizard manque tail_03 + tail_04 (4-segment vs 4-segment)

### 0.2 Conformité AAA (recherche sourcée 2026-05-20)

Pattern dominant industrie = **single asset source of truth**, code = seed/builder seulement :

| Engine | Pattern | Source |
|---|---|---|
| Unreal | `USkeleton` UAsset, `FReferenceSkeleton`, AnimGraph référence par asset pointer. Pas de bone tree hardcodé en C++ shipping. | dev.epicgames.com docs |
| Unity | Avatar sub-asset (`.ht` Human Template), configuré visuellement, jamais en code. | https://docs.unity3d.com/Manual/ConfiguringtheAvatar.html |
| Godot | `SkeletonProfile` Resource + `BoneMap` Resource. `SkeletonProfileHumanoid` est engine default read-only, **pas un pattern projet**. | https://docs.godotengine.org/en/stable/classes/class_skeletonprofile.html , https://docs.godotengine.org/en/stable/classes/class_bonemap.html |
| Mixamo | Template fixe ~21 joints `mixamorig:` namespace, no public spec. | https://helpx.adobe.com/creative-cloud/help/mixamo-rigging-animation.html |
| Pinocchio (Baran & Popović 2007) | `-skel file` argument canonical, `skeleton.cpp` constants = **demo seeds**, paper dit explicitement "modify skeleton.cpp pour ajouter built-in". | https://www.cs.toronto.edu/~jacobson/seminar/baran-and-popovic-2007.pdf , https://github.com/pmolodo/Pinocchio |
| Epic Data Registry | Centralized read-only store, lookup par `FDataRegistryId`, multi-sources avec override. | https://dev.epicgames.com/documentation/en-us/unreal-engine/data-registries-in-unreal-engine |
| Larian Generic Behaviour | 4 couches : framework / **definition** / behaviour / exception. Skeleton template = Definition layer. | docs.larian.game/Generic_behaviour |

**Verdict** : Forgia diverge AAA standard. Pattern `Genome<T>` AssetLoader (`forgia-genome-core/src/lib.rs`) est déjà bon, à étendre pour devenir **seul** path runtime. Sources (2) et (3) sont à supprimer/déplacer en fixtures test.

### 0.3 Pourquoi maintenant

- Audit reprise anim Rex 2026-05-20 a confirmé le mess avant tout fix runtime
- Bloquant pour scaling : ajouter Quadruped / Hexapod / Avian sans dupliquer 3× la définition
- Skeleton template reste un concept stable (n'évolue plus quotidiennement) → bon moment pour solidifier
- L'extension `forgia-skeleton-template` comble un gap réel du Bevy ecosystem (pas de crate canonique selon research)

## 1. Vision cible

```
Definition layer (data)
  • assets/genomes/skeleton_humanoid.toml      (20 bones)
  • assets/genomes/skeleton_biped_lizard.toml  (20 bones)
  • assets/genomes/skeleton_quadruped.toml     (à venir, ex: chevaux RPG)
  • assets/genomes/skeleton_<future>.toml      (drop-in, scan auto)
       ↓ AssetLoader<Genome<SkeletonTemplate>>
Registry layer — NEW crate forgia-skeleton-template
  • struct SkeletonTemplate (Deserialize, Asset, validate)
  • struct BoneDef { name, parent_idx, pos: Vec3 }
  • Resource SkeletonTemplateRegistry { handles: HashMap<SkeletonTemplateId, Handle<Genome<SkeletonTemplate>>> }
  • SkeletonTemplateId : enum string-backed (Humanoid, BipedLizard, Quadruped, Custom(String))
  • #[cfg(test)] fn test_humanoid() / test_biped_lizard()  — fixtures headless seulement
  • Pas de Plugin Bevy, pas de system — pure data + load helper
       ↓
Framework layer (consumers)
  • forgia-auto-rig — Pinocchio path UNIQUE
    - Plus de AutoRigBackend enum (PinocchioV1 seul)
    - Plus de auto_rig_pending_meshes / place_template / HUMANOID_BONES / BIPED_LIZARD_BONES
    - load_skeleton_template → Result<SkeletonTemplate, NotReady> (defer, pas fallback)
  • forgia-skeleton-embedder — algo embed pur (BFS + locks YXZ + path walking medial axis)
    - Plus de humanoid() / biped_lizard() runtime methods
    - Méthodes data-fn déplacées en test fixtures
```

## 2. Plan d'implémentation phasé

### Phase 1 — NEW crate `forgia-skeleton-template` (extraction)

**Objectif** : extraire `SkeletonTemplate` + `BoneDef` + validation depuis `forgia-skeleton-embedder` vers crate dédiée pure-data.

- [ ] `cargo new crates/forgia-skeleton-template --lib`
- [ ] `Cargo.toml` deps : `bevy = { workspace = true }`, `serde`, `forgia-genome-core`
- [ ] `src/lib.rs` :
  - struct `SkeletonTemplate { bones: Vec<BoneDef> }` (Deserialize, Asset via Genome<T>)
  - struct `BoneDef { name: String, parent: Option<usize>, pos: [f32; 3] }`
  - `enum SkeletonTemplateId { Humanoid, BipedLizard, Quadruped }` + `as_str()` mapping
  - `fn validate(t: &SkeletonTemplate) -> Result<(), String>` (BFS order check)
  - `fn rescaled_for_landmarks(...)` (déplacée depuis skeleton-embedder)
  - `fn flipped_z(&self) -> Self` (déplacée depuis skeleton-embedder)
  - `#[cfg(test)]` mod fixtures : `pub fn test_humanoid()` / `test_biped_lizard()` — copies des valeurs TOML actuelles, pour tests headless qui n'ont pas accès à AssetServer
- [ ] Tests headless 6 : structure, BFS order, hip root, validate ok, rescale match, flipped_z symmetry
- [ ] `cargo check -p forgia-skeleton-template` + clippy 0 warning

### Phase 2 — Registry pattern

**Objectif** : Resource centralisée loading + lookup, scan auto.

- [ ] Dans `forgia-skeleton-template/src/lib.rs` :
  - Resource `SkeletonTemplateRegistry { handles: HashMap<SkeletonTemplateId, Handle<Genome<SkeletonTemplate>>>, load_state: HashMap<Id, LoadState> }`
  - `Plugin SkeletonTemplatePlugin` — `Startup` system charge les 3 TOMLs (`assets/genomes/skeleton_*.toml`)
  - System `update_skeleton_registry_load_state` — track Assets<Genome<SkeletonTemplate>> events
  - `fn try_get(&self, id: SkeletonTemplateId) -> Option<&SkeletonTemplate>` — Some seulement si asset ready et valide
- [ ] Sensor JSON `forgia_skeleton_template_registry.json` 1Hz :
  ```json
  {
    "timestamp_secs": 12345,
    "templates": {
      "Humanoid":    { "load_state": "ready", "bones_count": 20, "valid": true },
      "BipedLizard": { "load_state": "ready", "bones_count": 20, "valid": true },
      "Quadruped":   { "load_state": "loading", "bones_count": 0, "valid": false }
    },
    "missing_files": []
  }
  ```
- [ ] Health check `forgia_health.json` : warn si template requested mais `load_state == "failed"` après 5s
- [ ] Tests headless 4 : registry insère, try_get None tant que loading, try_get Some quand ready, validate cascade

### Phase 3 — Migration `forgia-auto-rig` Pinocchio path unique

**Objectif** : supprimer TemplateFit + source (3), Pinocchio devient unique chemin, defer au lieu de fallback.

- [ ] `pinocchio_pipeline.rs` :
  - Consomme `Res<SkeletonTemplateRegistry>` au lieu de `Option<Res<SkeletonHumanoidGenomeHandle>>` + `Option<Res<SkeletonBipedLizardGenomeHandle>>`
  - `load_skeleton_template()` retourne `Option<&SkeletonTemplate>` via registry.try_get — si None, **continue boucle** (retry frame suivant), pas de fallback hardcoded
  - Suppression `SkeletonHumanoidGenomeHandle` / `SkeletonBipedLizardGenomeHandle` (remplacés par Registry)
  - Sensor `forgia_auto_rig.json` : champ `template_source` devient `"toml" | "pending"` (plus jamais `"fallback_hardcoded"`)
- [ ] `forgia-auto-rig/src/lib.rs` :
  - Supprimer `enum AutoRigBackend` + Resource (Pinocchio devient seul path)
  - Supprimer `const HUMANOID_BONES` + `BIPED_LIZARD_BONES` + `struct BoneDef` (local_translation version) + `impl AutoRigTemplate::bones()`
  - Supprimer fn `place_template` + `place_template_with_landmarks` + tous tests associés
  - Supprimer system `auto_rig_pending_meshes` + retirer du Plugin add_systems
  - Garder `AutoRigTemplate { Humanoid, BipedLizard }` enum (alias léger vers `SkeletonTemplateId`)
  - Garder `NeedsAutoRig::Template(...)` API publique
- [ ] `forgia-skeleton-embedder/src/lib.rs` :
  - Supprimer `pub fn humanoid()` + `pub fn biped_lizard()` (déplacés en test fixtures `forgia-skeleton-template`)
  - Garder algo `embed_template_skeleton` + BFS path walking + locks YXZ
  - Tests utilisent `forgia_skeleton_template::test_humanoid()` etc.

### Phase 4 — Tests + observabilité + validation runtime

- [ ] Test régression cross-crate `assert_template_toml_matches_test_fixture` :
  - Charge `skeleton_humanoid.toml` depuis fs
  - Compare bone par bone avec `forgia_skeleton_template::test_humanoid()`
  - Empêche tout futur drift entre fixture test et TOML
- [ ] Sensor `forgia_skeleton_template_registry.json` câblé sur health monitor
- [ ] Runtime check Rex RPG : spawn Rex en GameMode::Rpg, attendre 30 frames, assert `forgia_auto_rig.json::template_source == "toml"` (jamais "pending" après ce délai)
- [ ] Hot-reload Shift+F12 sur `skeleton_humanoid.toml` → Rex re-rigged en <1s

### Phase 5 — Cleanup checklist BMAD

- [ ] `cargo check --workspace` 0 erreur
- [ ] `cargo clippy --workspace -- -W warnings` 0 warning
- [ ] Stability Locks L1-L8 + LOCK-INV-1 inchangés (vérif via `check_stability_locks`)
- [ ] `cargo test -p forgia-skeleton-template -p forgia-auto-rig -p forgia-skeleton-embedder` vert
- [ ] Sensor `forgia_skeleton_template_registry.json` apparaît à runtime
- [ ] `MEMORY.md` mis à jour : nouvelle entrée `reference_skeleton_template_single_source.md`
- [ ] `_index.md` story → DONE + critères acceptance cochés
- [ ] `FRICTION_LOG.md` : RESOLVED entrée "3 sources skeleton template" si présente

## 3. Critères d'acceptance

- [ ] Recherche `HUMANOID_BONES|BIPED_LIZARD_BONES` retourne **0 match dans `crates/forgia-auto-rig/src/lib.rs`** (constantes supprimées)
- [ ] Recherche `pub fn humanoid\(\)|pub fn biped_lizard\(\)` retourne **0 match dans `crates/forgia-skeleton-embedder/src/lib.rs`** (fallback runtime supprimé)
- [ ] Resource `SkeletonTemplateRegistry` présent au runtime, contient ≥ 2 handles (Humanoid + BipedLizard)
- [ ] `forgia_auto_rig.json::template_source == "toml"` pour 100% des Rex spawnés (jamais "fallback_hardcoded")
- [ ] Sensor `forgia_skeleton_template_registry.json` écrit toutes les 1s
- [ ] Test régression `assert_template_toml_matches_test_fixture` vert (cross-crate)
- [ ] Hot-reload TOML fonctionnel (modif Y bone → Rex re-rigged sans recompile)
- [ ] 0 warning clippy workspace
- [ ] Story-454 (anim debug) toujours fonctionnelle (bone_trace.json continue à écrire)
- [ ] Aucune régression test : `cargo test --workspace` count avant == après ± nouveaux tests Phase 1/2/4

## 4. Risques & mitigations

| Risque | Probabilité | Impact | Mitigation |
|---|---|---|---|
| Race condition asset loading au boot (Rex spawn avant Registry ready) | Moyenne | Bloquant Rex | `auto_rig_pinocchio_v1` retry frame suivant, idempotent. Health alert si pending > 5s. |
| Suppression `place_template` casse story-454 anim-debug | Faible | Moyen | story-454 utilise `forgia_anim_debug::bone_trace` qui lit `BoneEntity` Components, pas `place_template` fn |
| Tests headless cassent (utilisent humanoid()/biped_lizard()) | Haute | Mineur | Migrer vers `forgia_skeleton_template::test_humanoid()` fixtures Phase 1 |
| Drift TOML test_fixture re-créé | Moyenne | Mineur | Test régression Phase 4 `assert_template_toml_matches_test_fixture` |
| Convention `local_translation` vs `pos absolu` (3) vs (1) — code consumer dépend de l'un ? | Moyenne | Bloquant | Phase 3 audit : grep `local_translation`/`BoneDef` dans tout consumer, refactor avant supprimer |
| Hot-reload Shift+F12 cassé par changement Registry | Faible | Mineur | Registry track AssetEvent::Modified → re-validate + re-emit health |

## 5. Locks à respecter

- **L1 GameAssets** : ajouter `Genome<SkeletonTemplate>` handles dans `resources/assets.rs` si pas déjà (Registry vérifie ratchet E1)
- **L7 SystemSets** : `SkeletonTemplatePlugin::Startup` registry load + `Update` registry maintenance gating sur `GameSet::Input` (avant Movement où auto-rig consume)
- **LOCK-INV-1** : N/A
- **skinning.rs formule bindpose** (lock implicite story-451) : NE PAS TOUCHER. Cette story est sur les **templates**, pas le skinning lui-même.

## 6. Sensors / Observabilité

| Sensor | Fréquence | Contenu | Quand consulter |
|---|---|---|---|
| `forgia_skeleton_template_registry.json` | 1Hz | load_state par Id + bones_count + valid + missing_files | Au boot pour valider tous loaded ; après hot-reload Shift+F12 |
| `forgia_auto_rig.json` (existant, enrichi) | 1Hz | + `template_source: "toml" \| "pending"`, drop `"fallback_hardcoded"` | Diagnostic Rex T-pose / pending |
| `forgia_health.json` (existant) | Continu | warn si registry.load_state == "failed" > 5s | Détecte TOML corrompu / path cassé |

## 7. Ce qui n'est PAS dans cette story

- **Fix runtime T-pose Rex** (3 hypothèses H3a/c/d ouvertes — concerne skinning + bind pose, pas templates) → story séparée post-merge
- **Animation procédurale tuning** (amplitudes proc_walk) → géré story-451 Phase 2
- **Templates additionnels** (Quadruped, Avian) → story future une fois architecture validée
- **Refacto skinning weights nearest-bone** → lock skinning.rs story-451
- **Net replication skeleton** → out of scope solo-only

## 8. Sources industrie (research 2026-05-20)

- **Unreal USkeleton** : dev.epicgames.com docs
- **Unity Avatar/Human Template** : https://docs.unity3d.com/Manual/ConfiguringtheAvatar.html
- **Godot SkeletonProfile + BoneMap** : https://docs.godotengine.org/en/stable/classes/class_skeletonprofile.html , https://docs.godotengine.org/en/stable/classes/class_bonemap.html
- **Mixamo template fixe** : https://helpx.adobe.com/creative-cloud/help/mixamo-rigging-animation.html
- **Pinocchio paper (Baran & Popović 2007)** : https://www.cs.toronto.edu/~jacobson/seminar/baran-and-popovic-2007.pdf , https://github.com/pmolodo/Pinocchio
- **Epic Data Registry** : https://dev.epicgames.com/documentation/en-us/unreal-engine/data-registries-in-unreal-engine
- **Larian Generic Behaviour 4-layer** : docs.larian.game/Generic_behaviour
- **Mike Acton "Data-Oriented Design" CppCon 2014** : https://www.youtube.com/watch?v=rX0ItVEVjHc
- **Bevy ecosystem context** : `bevy_animation`, `bevy_gltf`, `bevy_mod_skinned_aabb` (https://github.com/greeble-dev/bevy_mod_skinned_aabb), PR #21837
- **`bevy_common_assets` TomlAssetPlugin** : https://github.com/NiklasEi/bevy_common_assets , https://docs.rs/bevy_common_assets/

---

*Story rédigée 2026-05-20 par audit reprise anim Rex. Reflète conformité AAA pattern dominant (single asset source of truth), élimine 3 sources de vérité au profit d'une architecture data-driven scalable.*
