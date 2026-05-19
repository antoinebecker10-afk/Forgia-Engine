---
id: story-453-rpg-monitor-debt
title: RPG Health Monitor — Debt closure (CHK-3 LOD asymmetry + CHK-4 handles + sensor sample_points)
status: DONE 2026-05-19
scale: BMAD Standard
workspace: V2 Rewrite
created: 2026-05-19
parent_story: story-452 (RPG Health Monitor)
---

# Story-453 — RPG Monitor Debt Closure

## Origine

Story-452 (DONE 2026-05-18) avait identifié 3 items de dette technique + 1 cosmétique :
- CHK-3 LOD asymmetry (skipped — nécessite données LOD0/LOD2 cross-checkables)
- CHK-4 critical assets désactivé par défaut (`asset_server.load()` chaque tick = handles droppés)
- Extension sensor `forgia_terrain_lod.json` avec sample_points
- BUG-452-11 cosmétique : value/threshold incohérent en CHK-2 multi-biome

## Objectif

Fermer les 4 items pour passer le monitor à pleine puissance.

## Plan technique

### Item 1 — Sensor extension `forgia_terrain_lod.json`

Dans `crates/forgia-terrain/src/lod.rs::export_lod_sensor_system` :
- Sample 8-12 points stratifiés autour du joueur (rayon 96-256m couvrant LOD0/LOD1/LOD2)
- Pour chaque point : `(x, z, lod0_y, lod2_y, sea_level)` via `heightmap_at` + `build_lod2_terrain_mesh` clamp simulation
- Sérialiser en `sample_points: [{x, z, lod0_y, lod2_y, sea_level}, ...]` dans le JSON
- 1Hz, alloc minimale (Vec fixed-size avec reset)

### Item 2 — CHK-3 LOD asymmetry

Dans `crates/forgia-observability/src/checks.rs::chk_lod_asymmetry` :
- Lire `snapshots.terrain_lod.get("sample_points")`
- Pour chaque point : `delta = (lod0_y - lod2_y).abs()`
- Si `delta > config.lod_asymmetry.max_delta_m` → Warn
- Cas spécial : si point underwater (raw_y < sea_level) ET LOD2 clamp ≠ LOD0 → critical (phantom water)
- Tests : injection points synthétiques (delta=0.1m=Ok, delta=3m=Warn, phantom water=Critical)

### Item 3 — CHK-4 refacto

Nouveau fichier `crates/forgia-observability/src/asset_handles.rs` :
- `Resource CriticalAssetHandles { pub handles: Vec<(String, UntypedHandle)> }`
- System `sys_preload_critical_assets` à `OnEnter(GameMode::Rpg)` :
  - Lit `config.critical_assets.paths`
  - `asset_server.load_untyped(path)` → stocke dans la Resource
- `chk_critical_assets` lit la Resource, **plus de load() dans le check**

### Item 4 — BUG-452-11 cosmétique

Dans `chk_biome_luminance` : `value = failing.len() as f32, threshold = 0.0` est déjà appliqué (post-fix 452). Vérifier cohérence avec CHK-4/5. Aligner si écart.

## Critères d'acceptation

- [x] `forgia_terrain_lod.json` contient `sample_points: [...]` (12 entries — 3 rings × 4 cardinals)
- [x] CHK-3 implémenté + 6 tests unitaires (no sensor, empty, aligned ok, above-sea warn, underwater critical, disabled skipped)
- [x] CHK-4 utilise Resource `CriticalAssetHandles` préchargée, plus aucun `asset_server.load()` dans `checks.rs`
- [x] `critical_assets.enabled = true` par défaut (réactivé propre)
- [x] 0 warnings clippy strict (`cargo clippy -p forgia-observability -p forgia-terrain --no-deps -- -D warnings`)
- [x] **31 tests passed** (story-452: 26 + story-453: 5 nouveaux CHK-3 − 1 obsolète "skipped")
- [x] Full app `cargo check -p forgia-game` clean

## Résultat (2026-05-19)

### Fichiers livrés (6)
1. `crates/forgia-terrain/src/lod.rs` — ajout `LodSamplePoint` struct, `simulate_lod2_y_at` helper, `sys_update_lod_sample_points` system, extension JSON sensor avec `sample_points` array
2. `crates/forgia-terrain/src/lib.rs` — register `sys_update_lod_sample_points` dans plugin
3. `crates/forgia-observability/src/asset_handles.rs` (NEW) — Resource `CriticalAssetHandles` + preload/release OnEnter/OnExit Rpg
4. `crates/forgia-observability/src/config.rs` — `LodAsymmetryConfig` (max_delta_m, epsilon_m), `default_asset_paths` = grass textures réelles
5. `crates/forgia-observability/src/checks.rs` — CHK-3 implémentation complète (read sample_points, detect phantom water), CHK-4 refacto (lit CriticalAssetHandles, plus de load() per-tick), wildcard fix
6. `crates/forgia-observability/src/lib.rs` — module asset_handles, register dans Plugin
7. `config/genomes/rpg_monitor.toml` — CHK-3 enabled + thresholds, CHK-4 enabled + paths grass textures

### Architecture CHK-3 (regression preventer)

`build_lod2_terrain_mesh` (forgia-terrain) et `simulate_lod2_y_at` (helper exposé) doivent rester en sync. Si quelqu'un re-introduit un clamp sea_level (régression du bug Phase 2d corrigé hier), `simulate_lod2_y_at` doit être mis à jour → CHK-3 alertera instantanément via les sample_points. Si on oublie d'updater le helper, CHK-3 reste silencieux mais le code review révèlerait le drift.

### Architecture CHK-4 (handle preloading)

- `CriticalAssetHandles` Resource (Vec<(path, Handle<Image>)>) populée à `OnEnter(GameMode::Rpg)` via `sys_preload_critical_assets`
- Handles vivants tant que mode Rpg actif → refcount stable, LoadState reflète l'état réel
- `OnExit(GameMode::Rpg)` → release handles (permet unload Bevy si besoin)
- CHK-4 lit les LoadState des handles stockés — **aucun `asset_server.load()` per-tick**

### Validation

- ✅ `cargo check -p forgia-observability`
- ✅ `cargo check -p forgia-terrain`
- ✅ `cargo check -p forgia-game` (135 crates)
- ✅ `cargo clippy ... -- -D warnings` raw : 0 warnings
- ✅ `cargo test -p forgia-observability` : **31/31 tests passed**

### Dette résiduelle (post-story-453)

- BUG-452-13 cosmétique : mot "mort" hardcodé dans message CHK-6 (qa-lead avait jugé acceptable)
- Markdown linting story doc (MD025/MD032/MD060) — cosmétique non-bloquant

### Dette → backlog futur

- Tracy integration (story-454?)
- egui dashboard in-engine
- Sensor extension RPG : forgia_player_state.json + forgia_quest_progress.json

