# Story-485 — Arena Spatial Identity (Roguelite Cover & Lanes Foundations)

**Status:** CODE-COMPLETE (Phases 1-5 livrées + tests purs 89/89 verts) — RUNTIME VALIDATION DEFERRED
**Scale:** BMAD Standard (≤10 fichiers cibles, story requise, checklist post-impl obligatoire)
**Created:** 2026-05-21
**Phases 1-5 done:** 2026-05-21 (commits `89d3c80` / `10dd870` / `496551b` / `d2216e2`)
**Depends on:** story-483 (stage-arena foundations DONE), forgia-anchor (existant), forgia-level-presets (vide, scaffold)
**Blocks:** Story-486 (Cover-aware AI BT), Story-487 (Stage Director L4D pacing)

## Statut livraison (2026-05-21 fin de session)

| Phase | Status | Commit | Tests | Notes |
|---|---|---|---|---|
| 1 — AnchorKind 6→11 | ✅ DONE | `89d3c80` | 17/17 | Indices [0..6) stables (test invariant) |
| 2 — Level-presets data layer | ✅ DONE | `10dd870` | 17/17 | 4 modules TOML, parse_anchor_kind cross-crate |
| 3 — Stage palette wiring | ✅ DONE | `496551b` | inclus | 2 stages annotés, ForgiaLevelPresetsPlugin idempotent |
| 4 — Sight-line solver pur | ✅ DONE | `496551b` | 15 layout | 6 invariants enforced, déterminisme seed |
| 5 — Runtime spawn + sensor | ✅ DONE | `d2216e2` | 10 sensor | LayoutResult + LayoutParams bundle, sensor 1Hz |
| 6 — Hardening + post-impl QA | ⏳ EN COURS | — | — | Sub-agents verifier+qa-lead lancés 2026-05-21 |
| 7 — Validation runtime | ⏸️ **BLOCKED** | — | — | Workspace `cargo run` cassé depuis commit antérieur `9e149ca` (story-483 refactor incomplet par autre terminal — APIs `forgia_audio_voicelines::*`, `forgia_loot_tables::*`, `forgia_audio_music_state::*`, `crate::waves::current_stage_node` etc. référencées mais non-implémentées). Reprendre dès que workspace re-compile. |

**Substitutions assets** (story §7 risque "Élevée" matérialisé) — 5/6 GLB du plan original absents du pack KayKit Dungeon, proxies appliqués :

| Slot story | Original (manquant) | Proxy KayKit Dungeon |
|---|---|---|
| CoverCluster | crate_small.glb | **crates.glb** |
| CoverWall | pillar_round.glb | **pillar.glb** |
| CoverWall | wall_section.glb | **wall.glb** |
| SniperPerch | tower_section.glb | **pillar_deco.glb** (proxy dégradé, TODO story-485b) |
| MeleePit | arena_floor_pit.glb | **rubble.glb** (proxy ground-level, vraie pit = story-488) |

---

## 1. Contexte

Audit 2026-05-21 (cf [project_roguelite_map_audit_2026_05_21](../../memory/project_roguelite_map_audit_2026_05_21.md)) :
le pipeline stage-arena est shippé (commit `9e149ca`) mais l'arène hex 80-90m de rayon
est **vide à l'intérieur des ramparts** — 0 cover, 0 prop intra-arena, 0 sight-line
designed, terrain plat. Les 3 archetypes ennemis (Tank 4m / Runner 7m / Sniper 24m)
ne peuvent pas exprimer leur identité de combat sans structure spatiale.

### Patterns industry à appliquer (sourcés)

| Source | Pattern repris |
|---|---|
| Risk of Rain 2 (Hopoo) | Hand-crafted modules, randomisation ordering+content overlay |
| Returnal (Housemarque) | Pre-authored sub-rooms recomposées par run, seed déterministe |
| Halo (Bungie GDC 2002) | Sandbox triangle environment × weapons × enemies, 30s combat loop |
| Doom Eternal (id Software) | Arena verticality + vantage points asymétriques |
| Uncharted 4 (Naughty Dog) | Cover heights standardisés 1.0/1.25/1.75 m |
| COD WW2 / TF2 (Valve) | Engagement distance ≤ 40 m breakup forcé |
| Resistance (Insomniac) | Vertical increments 3 m / 6 m |
| Level Design Book | "Less cover is better" — sight-line clarté > densité |

Refs détaillées dans le rapport `2026-05-21 audit map roguelite` (conversation).

## 2. Goals

1. Transformer l'arène hex vide en **espace de combat lisible** avec sight-lines brisées
2. Étendre primitives mode-agnostic existantes (`AnchorKind`, `level-presets`) **sans** dupliquer ni casser story-483
3. Composition data-driven : aucun hardcode, modules palette TOML hot-reloadable
4. Sight-line solver déterministe (seed run) reproductible debug
5. Observability complète : sensor `forgia2_stage_layout.json` + health alerts next-step

## 3. Non-Goals (reportés)

- Cover-aware AI behavior (SeekCover/Suppress/Flank) → **Story-486**
- Stage Director L4D intensity pacing → **Story-487**
- Verticality terrain procédural (height variation intra-arena) → **Story-488** (besoin forgia-terrain extension)
- Destructible cover (Hunt Showdown) → backlog post-V7
- Modular sub-room composition Returnal-style (au-delà de props placement) → backlog

## 4. Acceptance Criteria

- [x] AC1 — `AnchorKind` enum étendu 6 → 11 variants sans renumérotation indices [0..6) ✅ test `anchor_kind_index_stability_legacy_indices`
- [x] AC2 — `forgia-level-presets` peuplé : 4 modules TOML hot-reloadables ✅ `assets/genomes/level_modules.toml`
- [x] AC3 — `forgia-stage-arena::spawn_stage_arena_on_request` place modules en respectant invariants spatiaux ✅ section 5.5 dans `spawn_stage_arena_on_request`, tests purs `place_modules_*` couvrent les 6 invariants
- [x] AC4 — Sensor `forgia2_stage_layout.json` 1Hz expose tous les counts + métriques + severity + next_step ✅ `layout_sensor.rs`
- [x] AC5 — Health alert severity tiers + next_step explicite (info/ok/warn/error) ✅ `severity_for_layout` + `next_step_for_layout`
- [ ] AC6 — Runtime stage `crypts_of_anvil` ≥ 1 SniperPerch + ≥ 6 CoverLow + ≥ 1 MeleePit central ⏸️ **DEFERRED** (workspace `cargo run` bloqué — refactor `forgia-mode-roguelite` incomplet en cours). Test pur `place_modules_sniper_perch_at_edge` + `_melee_pit_central` validés en isolation.
- [ ] AC7 — Runtime stage `forge_sanctum` layout distinct ⏸️ **DEFERRED** (même blocker — palette TOML distincte committée + test pur déterminisme vert)
- [x] AC8 — Re-run même seed → layout identique ✅ test `place_modules_deterministic_same_seed`
- [x] AC9 — `cargo check -p forgia-stage-arena -p forgia-anchor -p forgia-level-presets` clean ✅
- [x] AC10 — `cargo clippy --no-deps --tests -- -D warnings` 0 warning sur les 3 crates ✅
- [x] AC11 — Tests purs ≥ 8 nouveaux ✅ **25 nouveaux** (15 layout + 10 sensor) sur 89 total
- [ ] AC12 — Sub-agents `verifier` + `qa-lead` validés ⏳ EN COURS (lancés parallèle 2026-05-21 fin de session)
- [ ] AC13 — Checklist `.bmad/checklists/post-implementation.md` complétée ⏳ pending

## 5. Architecture & Patterns

### 5.1 Primitives étendues (rétro-compatibles)

**`forgia-anchor::AnchorKind`** (file `crates/forgia-anchor/src/lib.rs`) :

```rust
pub enum AnchorKind {
    PlayerSpawn,    // index 0 — INCHANGÉ
    PoiSlot,        // index 1 — INCHANGÉ
    Landmark,       // index 2 — INCHANGÉ
    BossPad,        // index 3 — INCHANGÉ
    Teleporter,     // index 4 — INCHANGÉ
    LootZone,       // index 5 — INCHANGÉ
    CoverLow,       // index 6 — NEW (1.0–1.25 m, Uncharted 4)
    CoverHigh,      // index 7 — NEW (≥ 1.75 m)
    SniperPerch,    // index 8 — NEW (overlook 6 m + arc visuel)
    MeleePit,       // index 9 — NEW (close fight zone)
    FlankRoute,     // index 10 — NEW (path waypoint)
}
```

**Invariant** : `index()` reste const, indices [0..6) **jamais renumérotés** — downstream sensor consumers et `[[reference-anchor-point-pattern]]` préservés. Atomic counter array `[AtomicU32; 11]` (au lieu de 6) — pas de changement Arc/Mutex.

### 5.2 Module palette (NEW data layer)

**`forgia-level-presets`** (file `crates/forgia-level-presets/src/lib.rs` — actuellement scaffold 1 LOC) :

```rust
#[derive(Asset, TypePath, Debug, Clone, serde::Deserialize)]
pub struct LevelModulesGenome {
    pub modules: HashMap<String, ModuleDef>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ModuleDef {
    pub kind: ModuleKind,                  // CoverCluster | SniperPerch | MeleePit | etc.
    pub prop_palette: Vec<PropEntry>,
    pub density_per_m2: f32,                // 0.05–0.5 typique
    pub min_spacing_m: f32,                 // ≥ 3.0 (cover spacing rule)
    pub footprint_radius_m: f32,            // exclusion zone par module
    pub allowed_biomes: Vec<String>,
    pub anchor_kinds_emitted: Vec<String>,  // e.g. ["cover_low", "cover_high"]
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PropEntry {
    pub prefab: String,        // GLB path relative to assets/
    pub weight: f32,            // RNG selection weight
    pub anchor_kind: String,    // map to AnchorKind via FromStr
    pub height_class: HeightClass,  // Low | High | Tall
}
```

**TOML** `assets/genomes/level_modules.toml` (NEW) :

```toml
[modules.cover_low_cluster]
kind = "CoverCluster"
density_per_m2 = 0.08
min_spacing_m = 3.0
footprint_radius_m = 6.0
allowed_biomes = ["Volcanic", "Plains"]
anchor_kinds_emitted = ["cover_low"]

[[modules.cover_low_cluster.prop_palette]]
prefab = "models/kaykit/dungeon/crate_small.glb"
weight = 0.6
anchor_kind = "cover_low"
height_class = "Low"

[[modules.cover_low_cluster.prop_palette]]
prefab = "models/kaykit/dungeon/barrel.glb"
weight = 0.4
anchor_kind = "cover_low"
height_class = "Low"

[modules.cover_high_wall]
kind = "CoverWall"
density_per_m2 = 0.04
min_spacing_m = 5.0
footprint_radius_m = 8.0
allowed_biomes = ["Volcanic"]
anchor_kinds_emitted = ["cover_high"]
# (...palette pillars KayKit Dungeon)

[modules.sniper_perch]
kind = "SniperPerch"
density_per_m2 = 0.0           # 1 fixed instance
min_spacing_m = 0.0
footprint_radius_m = 4.0
allowed_biomes = ["Volcanic", "Plains"]
anchor_kinds_emitted = ["sniper_perch"]

[[modules.sniper_perch.prop_palette]]
prefab = "models/kaykit/dungeon/tower_section.glb"
weight = 1.0
anchor_kind = "sniper_perch"
height_class = "Tall"          # 6 m, Resistance verticality

[modules.melee_pit]
kind = "MeleePit"
density_per_m2 = 0.0
min_spacing_m = 0.0
footprint_radius_m = 6.0
allowed_biomes = ["Volcanic", "Plains"]
anchor_kinds_emitted = ["melee_pit"]

[[modules.melee_pit.prop_palette]]
prefab = "models/kaykit/dungeon/arena_floor_pit.glb"
weight = 1.0
anchor_kind = "melee_pit"
height_class = "Low"
```

### 5.3 Stage palette per-stage

**Extension `assets/genomes/roguelite_stages.toml`** :

```toml
[stages.crypts_of_anvil]
# ...existing fields preserved...
module_palette = [
    { id = "cover_low_cluster",  count = 3 },
    { id = "cover_high_wall",     count = 2 },
    { id = "sniper_perch",        count = 1 },
    { id = "melee_pit",           count = 1 },
]

[stages.forge_sanctum]
module_palette = [
    { id = "cover_low_cluster",  count = 4 },
    { id = "cover_high_wall",     count = 1 },
    { id = "sniper_perch",        count = 0 },  # plus de plain combat
    { id = "melee_pit",           count = 2 },
]
```

### 5.4 Sight-line solver (placement algorithm)

**`forgia-stage-arena::layout::place_modules`** (NEW fn pure, testable) :

```rust
pub fn place_modules(
    extent_m: f32,
    palette: &[(ModuleDef, u32)],
    player_spawn: Vec3,
    boss_pad: Option<Vec3>,
    rng_seed: u64,
) -> Vec<ModulePlacement> { /* ... */ }
```

**Invariants enforced** :
1. Sight-line PlayerSpawn ↔ BossPad **brisée** par au moins 1 CoverHigh à distance ≤ 40 m (COD WW2 engagement comfort).
2. Cover spacing : tout couple (CoverLow, CoverLow) ≥ 3 m (Level Design Book).
3. ≥ 1 SniperPerch placé en bord (distance > 0.6 * extent_m du centre) si palette inclut.
4. MeleePit central (distance < 0.3 * extent_m du centre) si palette inclut.
5. Aucun module dans cercle 8m autour PlayerSpawn (anti-cheese spawn camp).
6. Tous modules dans cercle inscrit hex `0.866 * extent_m`.

**Algo** : seed `Xoshiro256StarStar` + dart-throw rejection sampling (max 200 essais/module) + sight-line check via segments. **Pure function, 0 alloc dans hot path** (Vec préalloué).

### 5.5 Sensor `forgia2_stage_layout.json`

```json
{
  "timestamp_secs": 1234.56,
  "stage_id": "crypts_of_anvil",
  "modules_placed": 7,
  "cover_low_count": 9,
  "cover_high_count": 4,
  "sniper_perch_count": 1,
  "melee_pit_count": 1,
  "flank_route_count": 0,
  "longest_sightline_m": 32.4,
  "min_cover_spacing_m": 3.2,
  "module_palette_used": ["cover_low_cluster", "cover_high_wall", "sniper_perch", "melee_pit"],
  "rejection_attempts_total": 47,
  "severity": "ok",
  "next_step": null
}
```

Severity rules (cf [[reference-pattern-genome-driven-plugin-with-sensor]]) :
- `ok` : tous invariants respectés
- `warn` : `longest_sightline_m > 35.0` (proche limite) → next_step *"Add 1 CoverHigh between PlayerSpawn and BossPad. Edit `assets/genomes/roguelite_stages.toml`, increase cover_high_wall.count in current stage."*
- `error` : `longest_sightline_m > 40.0` OR `min_cover_spacing_m < 3.0` OR `cover_low_count == 0` → next_step *"Module placement violated invariants. Check `forgia2_stage_layout.json rejection_attempts_total`. If > 150, palette too dense for extent_m — reduce count or increase extent."*

## 6. Plan d'implémentation phasé

### Phase 1 — Anchor extension (XS, 1h)

**Fichiers** :
- `crates/forgia-anchor/src/lib.rs` — add 5 enum variants, extend `index()` const match, extend `all()` const, extend `as_str()` const, change `AtomicU32` array `6 → 11`, sensor JSON adds 5 new count fields with `0` default for backward compat
- `crates/forgia-anchor/src/lib.rs` — extend test `anchor_kind_all_covers_6_variants` → `anchor_kind_all_covers_11_variants` + new test `anchor_kind_index_stability_legacy_indices` asserting indices [0..6) inchangés

**Gates** :
- `cargo check -p forgia-anchor` 0 erreur
- `cargo test -p forgia-anchor` tous tests verts, ≥ 1 nouveau
- Commit : `feat(anchor): extend AnchorKind 6 → 11 variants (CoverLow/CoverHigh/SniperPerch/MeleePit/FlankRoute) — story-485 phase 1`

### Phase 2 — Level-presets data layer (M, 1j)

**Fichiers nouveaux/modifiés** :
- `crates/forgia-level-presets/src/lib.rs` — full rewrite scaffold → plugin + `LevelModulesGenome` Asset + `ModuleDef` + `PropEntry` + `HeightClass` + `ModuleKind` + tests purs déserialization
- `crates/forgia-level-presets/Cargo.toml` — add deps `bevy = { workspace }`, `serde`, `forgia-anchor`, `forgia-core`
- `assets/genomes/level_modules.toml` — NEW, 4 modules définis
- `crates/forgia-level-presets/src/lib.rs` — sensor stub `forgia2_level_modules.json` exposant `modules_count`, `modules_loaded`, palette parse errors
- Test : `parse_level_modules_toml_minimal`, `module_kind_from_str_roundtrip`, `prop_entry_weight_sum_valid`

**Gates** :
- `cargo check -p forgia-level-presets` 0 erreur
- TOML déserialise via test (ne touche pas asset_server, lecture file direct + `toml::from_str`)
- Commit : `feat(level-presets): NEW module palette + LevelModulesGenome asset — story-485 phase 2`

### Phase 3 — Stage palette wiring (S, 4h)

**Fichiers** :
- `crates/forgia-stage-arena/src/lib.rs` — `StageDef` ajoute champ `module_palette: Vec<ModulePaletteEntry>` (Option avec default empty pour rétro-compat 2 stages existants)
- `assets/genomes/roguelite_stages.toml` — annoter les 2 stages existants avec palette (cf §5.3)
- `crates/forgia-stage-arena/Cargo.toml` — add dep `forgia-level-presets`
- `crates/forgia-stage-arena/src/lib.rs` — `StageArenaHandles` ajoute `Handle<Genome<LevelModulesGenome>>`
- Tests : `stage_def_with_module_palette_parses`, `stage_def_without_palette_defaults_empty`

**Gates** :
- `cargo check -p forgia-stage-arena` 0 erreur
- 2 stages existants chargent sans régression
- Sensor `forgia2_stage.json` champs existants inchangés
- Commit : `feat(stage-arena): wire module palette per stage — story-485 phase 3`

### Phase 4 — Sight-line solver pur (M, 1j)

**Fichiers** :
- `crates/forgia-stage-arena/src/layout.rs` — NEW module : `place_modules()` + `check_sightline()` + `dart_throw_sample()` + `ModulePlacement` struct
- `crates/forgia-stage-arena/src/lib.rs` — `pub mod layout;` + re-export
- Tests dans `layout.rs` ≥ 5 :
  - `place_modules_respects_min_spacing`
  - `place_modules_breaks_sightline_player_boss`
  - `place_modules_sniper_perch_at_edge`
  - `place_modules_melee_pit_central`
  - `place_modules_deterministic_same_seed`
  - `place_modules_no_module_in_spawn_safety`

**Gates** :
- `cargo test -p forgia-stage-arena layout::` 6/6 verts
- `cargo clippy -p forgia-stage-arena -- -D warnings` 0 warning
- Aucun appel Bevy `Commands`/`AssetServer` dans `layout.rs` (test headless)
- Commit : `feat(stage-arena): NEW layout::place_modules — sight-line solver — story-485 phase 4`

### Phase 5 — Runtime spawn integration (M, 1j)

**Fichiers** :
- `crates/forgia-stage-arena/src/lib.rs` — `spawn_stage_arena_on_request` consomme `place_modules` après ramparts spawn, avant POI anchors. Spawn props via `spawn_gltf_prefab` (réutilise pattern existant story-483).
- Pour chaque `ModulePlacement` :
  1. Spawn prefab GLB selon `PropEntry` choisi (RNG weighted seed-derived)
  2. Spawn `AnchorPoint` companion entity au même `Transform` avec `kind` mappé
  3. Add `RogueliteRunMarker` pour cleanup OnExit (réutilise pattern existant)
- Sensor `forgia2_stage_layout.json` writer :
  - `crates/forgia-stage-arena/src/layout_sensor.rs` NEW, 1Hz `IoTaskPool` write (pattern existant cf [[reference-pattern-genome-driven-plugin-with-sensor]])

**Gates** :
- `cargo run -p forgia-game --profile release-fast` lance OK
- Stage `crypts_of_anvil` runtime : sensor montre `cover_low_count >= 6`, `sniper_perch_count == 1`, `melee_pit_count == 1`, `longest_sightline_m < 40`
- Screenshot manuel user-validé
- Re-run même seed → mêmes positions modules
- Commit : `feat(stage-arena): runtime spawn modules + sensor forgia2_stage_layout — story-485 phase 5`

### Phase 6 — Sensor health alerts + next-step (XS, 2h)

**Fichiers** :
- `crates/forgia-stage-arena/src/layout_sensor.rs` — fonctions pures `severity_for_layout(metrics) -> Severity` et `next_step_for_layout(metrics) -> Option<String>` (testables headless)
- Tests : `severity_ok_when_invariants_respected`, `severity_warn_when_sightline_35_to_40`, `severity_error_when_sightline_over_40`, `next_step_specifies_toml_edit`

**Gates** :
- `cargo test -p forgia-stage-arena layout_sensor::` 4/4 verts
- Sensor JSON file contient `next_step` field non-vide quand error
- Commit : `feat(stage-arena): layout sensor severity + next-step — story-485 phase 6`

### Phase 7 — Hardening + post-impl QA (XS, 4h)

**Actions** :
- `cargo clippy --workspace --no-deps -- -D warnings` (full workspace clean — vérifier 0 warning sur fichiers touchés story-485)
- Run sub-agent `verifier` : `cargo check`, clippy 0 warn, Stability Locks intacts (L1 GameAssets : aucun `Handle<` ajouté dans `resources/assets.rs` — modules sont via AssetServer file path, hors whitelist baseline)
- Run sub-agent `qa-lead` : BUG REPORT structuré
- Treat issues found → corrections OR justify with story-486/487 deferral
- Update `docs/stories/_index.md` story-485 status DONE
- Add memory `[[reference-arena-spatial-identity-pattern]]` capturant pattern complet réutilisable autres modes (FPS arena standalone, RPG instances)
- Complete `.bmad/checklists/post-implementation.md`
- Commit : `feat(stage-485): hardening + qa-lead validation — story-485 DONE`

## 7. Risques & mitigations

| Risque | Probabilité | Mitigation |
|---|---|---|
| Asset GLB manquant (arena_floor_pit, tower_section) | Élevée | Phase 2 : fallback to `crate_small.glb` + log warn ; story-485b si pack KayKit additionnel requis |
| Dart-throw rejection blocks > 200 tries | Moyenne | Sensor `rejection_attempts_total` exposé, severity warn si > 150 ; doc TOML guideline density |
| Backward compat AnchorKind sensor consumers | Faible | Tests d'index stability + sensor JSON nouveaux champs avec default 0 |
| forgia-mode-roguelite stage transition spawn 2× modules | Moyenne | StageLoadRequest flow déjà gère cleanup (story-483) ; re-tester transition Lobby→InRun→Boss |
| Performance hot path (spawn ~15 modules × ~5 props chacun) | Faible | Spawn one-shot OnEnter stage, pas every frame ; pré-alloc Vec ; pas un hot path |

## 8. Stability Locks impact

- **L1 GameAssets** : ⚠️ Phase 2 ajoute `Handle<Genome<LevelModulesGenome>>` dans `StageArenaHandles` (déjà existant pour `RogueliteStagesGenome`, pattern identique). Phase 5 spawn props via `asset_server.load(prefab_path)` — vérifier que `asset_load_whitelist.txt` couvre `models/kaykit/dungeon/*` (probable déjà whitelisted via story-483). **À valider phase 7**.
- **L7 SystemSets** : Phase 5 ajoute systems dans `GameSet::Movement` (cohérent avec stage-arena existant).
- Aucun autre Lock touché.

## 9. Definition of Done

Tous AC §4 verts. Sensor visible dans MCP `read_diagnostic_report`. Sub-agents OK. Screenshots user-validés des 2 stages avec layouts distincts. Memory `[[reference-arena-spatial-identity-pattern]]` créée. Story status DONE dans `_index.md`. Commits poussés ou prêts à push selon décision user.

## 10. Follow-ups identifiés (stories candidates)

- **Story-486** Cover-aware AI BT : `forgia-ai-arena-bot` étend `BotState` avec `SeekCover`/`Suppress`/`Flank` consommant `AnchorPoint` queries par kind. Pattern Halo BT + stimulus priority.
- **Story-487** Stage Director L4D : NEW crate `forgia-stage-director` Resource `EncounterIntensity` + states BuildUp/Peak/Relaxed pilote wave spawning + audio ducking.
- **Story-488** Arena verticality terrain : extension `forgia-terrain` ou heightmap override per stage (3m/6m increments Insomniac).
- **Story-489** Destructible cover : Hunt Showdown pattern, ECS state machine prop_intact → prop_damaged → prop_destroyed.

---

*Plan rédigé 2026-05-21 post-audit map roguelite. Sources industry détaillées dans [project_roguelite_map_audit_2026_05_21](../../memory/project_roguelite_map_audit_2026_05_21.md) section Sources.*
