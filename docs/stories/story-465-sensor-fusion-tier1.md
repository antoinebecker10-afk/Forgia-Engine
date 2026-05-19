# Story-465 — Sensor Fusion Tier 1 (forgia2_combat + forgia2_arena)

**Status** : IN PROGRESS
**Scale** : Standard (2 fichiers Rust + 1 doc)
**Date** : 2026-05-19
**Parent** : Vague 2 reliquat audit-2026-05-19.md §7

## Symptôme

Phase 5 ARCHITECTURE.md cible "12 sensors `forgia2_*.json` max", réalité
2026-05-19 = 27 sensors `forgia_*.json` legacy + 0/12 forgia2 conformes.
Audit identifie 2 fusions Tier 1 ship-blockers :

- `forgia_hitscan` + `forgia_hud_ammo` + `forgia_screen_flash`
  + `forgia_damage_dir` + `forgia_killfeed` → `forgia2_combat.json`
- `forgia_arena_feedback` + `forgia_arena_waves` → `forgia2_arena.json`

## Approche

**Aggregator file-based** dans `forgia-observability`. Producteurs legacy
restent INCHANGÉS (0 risque régression). Nouveau système `sys_write_forgia2_aggregates`
qui :

1. Lit les 7 sensors legacy via `std::fs::read_to_string` (1Hz throttle).
2. Compose 2 outputs au format CLAUDE.md `{id, severity, next_step, ...payload}`.
3. Calcule severity globale (ok/warn/critical) basée sur fraicheur + sources manquantes.
4. Écrit `forgia2_combat.json` et `forgia2_arena.json` à la racine workspace.

Pattern : copié du `sensor_reader.rs` existant. Reuse `serde_json::Value`.

## Acceptance Criteria

- [ ] `forgia2_combat.json` écrit 1Hz quand `GameMode::Fps` actif.
- [ ] `forgia2_arena.json` écrit 1Hz quand `GameMode::Fps` actif.
- [ ] Format conforme CLAUDE.md `{id, severity, next_step, timestamp_secs, sources, sources_missing}`.
- [ ] Severity logique : `ok` (tout présent + fresh), `warn` (1-2 stale/missing), `critical` (tout missing).
- [ ] `next_step` actionnable quand non-ok (ex: "verify forgia-killfeed plugin wired").
- [ ] Producteurs legacy strictement inchangés.
- [ ] `cargo check --workspace` clean, `clippy -D warnings` clean.

## Non-objectifs

- Cleanup des 7 legacy producers — Vague 5 (P2).
- Migration vers Resource publish-subscribe pattern — refactor structurel hors scope.
- Aggregation des Tier 2 (`forgia2_chunks`, `forgia2_anim`) — Vague 5.

## Fichiers touchés

1. `crates/forgia-observability/src/forgia2_aggregator.rs` (nouveau, ~150 LOC)
2. `crates/forgia-observability/src/lib.rs` (register module + system)
3. `docs/stories/story-465-sensor-fusion-tier1.md` (ce fichier)

## Risques

- Faible : 0 modification des producteurs existants. Si bug dans aggregator,
  on retire le système et tout fonctionne comme avant.
- Stale detection basée sur `std::fs::metadata().modified()` — peut diverger
  sur Windows si timestamps fs lag. Tolerance 10s par défaut.
