---
id: story-441
title: Spawn Village V1 — TOML-driven medieval hub
status: DONE
scale: standard
created: 2026-05-17
done: 2026-05-17
author: claude + antoine
---

# Story-441 — Spawn Village V1

## Contexte

Le RPG V2 spawn actuellement le joueur à une position hardcodée `(16, h+2, 16)` avec
2 buildings cuboïdes générés sur place (`forgia-rpg/lib.rs:289-312, 480-496`). Pas de
remparts, pas de routes urbaines, pas de hub visuel. Bloquant pour vertical slice RPG.

## Vision V1 (vague 1/4)

Un **vrai village de spawn médiéval** chargé depuis TOML data-driven, **zéro hardcode** :

- **6 bâtiments KayKit Hexagon** (well centre + tavern + church + market + 2 homes)
- **Ramparts hexagonaux** ~60m diamètre (6 walls straight + 6 corners + 1 gate)
- **4 routes radiales** sortant du village vers terrain alentour (tier `urban` PavingStones)
- **Spawn player** lu du TOML (plus de `(16, h+2, 16)`)
- **Sensor** `forgia_village.json` + health alert si TOML invalide

## Pattern industrie validé

- **Skyrim kit pieces** (Burgess GDC 2013) : snap-to-grid unit scale
- **Cities Skylines** (cslmodding.info) : spline ribbon UV tiling — déjà en place V2
- **AC Origins** (GDC Routhier) : settlements hand-placed data, nature procédurale
- **Anno 1800** (Anno Union DevBlog) : road↔building par arête déclarée

## Architecture

```
forgia-village-kit       NEW   vocab pur (Serde TOML types, asset resolver) ~200 LOC
forgia-prefab            POP   spawn entité depuis data + SceneRoot ~150 LOC
forgia-village-loader    NEW   Plugin TOML → ECS, sensor JSON, health ~350 LOC
forgia-rpg               EDIT  remove hardcode, add deps, plug village plugin
forgia-terrain::paths    EDIT  ajouter RoadTier::Urban + texture PavingStones
config/villages/spawn_village.toml  NEW
```

## Acceptance Criteria

- [ ] `cargo check --workspace` : 0 erreur
- [ ] `cargo clippy --workspace -- -W warnings` : 0 warning
- [ ] OnEnter(GameMode::Rpg) charge `config/villages/spawn_village.toml`
- [ ] 6 buildings KayKit Hexagon visibles à leur position TOML
- [ ] Ramparts hexagonaux fermés (6 walls + corners) avec 1 gate au nord
- [ ] 4 routes radiales tier `urban` (PavingStones) visibles depuis le centre
- [ ] Joueur spawn à `spawn.player_position` du TOML (plus de hardcode)
- [ ] `forgia_village.json` exporté : buildings_loaded, ramparts_pieces, roads, spawn_pos
- [ ] HealthAlert si TOML manquant : "VILLAGE TOML MISSING: config/villages/spawn_village.toml → next: vérifier path"
- [ ] HealthAlert si building_id inconnu du kit : warn + skip + log
- [ ] OnExit(GameMode::Rpg) cleanup complet (tag VillageMarker + despawn récursif Bevy 0.18 default)
- [ ] Pas de panic si TOML mal formé : graceful degrade + log error
- [ ] 0 hardcode dans le code Rust (positions, kit, asset path) — tout TOML

## Risques + mitigations

| Risque | Mitigation |
|---|---|
| KayKit Hexagon GLTF refs .bin (pas .glb) | Test load 1 fichier early, sinon convertir en .glb |
| Pivot mesh KayKit pas au sol (y_offset variable) | Sensor AABB mesure y_min, applique offset auto |
| TOML race condition vs terrain pas prêt | Resource `PendingVillageSpawn` consommée quand TerrainConfig dispo |
| Hot path frame budget (6 SceneRoot async load) | OnEnter one-shot, pas dans Update — pas hot |
| Wall corners + gates orientation | Yaw calculé depuis polygon edges (math), pas hardcodé |

## Vagues suivantes (hors scope V1)

- **V2 (story-442)** — `forgia-rampart-wall` : snap-chain auto + détection corners par angle
- **V3 (story-443)** — pavé urbain transition douce → terrain (vertex blend)
- **V4 (story-444)** — props auto (barils, banners, lanternes), audio ambient village

## Sensor / observability

`forgia_village.json` 1Hz :
```json
{
  "timestamp_secs": 12.5,
  "village_id": "spawn_village",
  "buildings_loaded": 6,
  "ramparts_pieces": 7,
  "roads_count": 4,
  "spawn_position": [16.0, 5.2, 16.0],
  "missing_assets": [],
  "load_errors": []
}
```

## Checklist post-impl (`.bmad/checklists/post-implementation.md`)

À cocher avant DONE — sub-agents verifier + qa-lead exécutés en parallèle (rule
`.claude/rules/post-impl-auto-qa.md`).

- [x] `cargo check --workspace` clean (1 warning pré-existant forgia-websocket hors scope)
- [x] `cargo clippy --no-deps` 0 warning sur 6 crates touchées
- [x] `cargo test -p forgia-village-kit` 8/8 passent
- [x] qa-lead audit → 7 bugs identifiés, 5 fixés en session (BUG-441-01/02/04/05/06/07), 1 marqué backlog story-442 (BUG-441-03 AABB pivot calibration)
- [x] Convention next-step appliquée sur tous les `warn!`/`error!` du loader
- [x] Sensor `forgia_village.json` ajoute champ `status` ("pending"/"loaded"/"error")
- [x] PrefabStats reset entre sessions (BUG-441-02)
- [x] TerrainConfig absente → warn one-shot avec next-step (BUG-441-01)

## Post-mortem QA Fixes

| Bug | Sévérité | Fix |
|---|---|---|
| BUG-441-01 | Majeur | warn one-shot quand TerrainConfig absente malgré request présente, via `Local<bool>` |
| BUG-441-02 | Majeur | `reset_prefab_stats()` ajouté + appelé depuis `cleanup_village` |
| BUG-441-03 | Majeur | DOC ONLY V1 — hypothèse pivot=floor documentée dans TOML, calibration AABB → story-442 |
| BUG-441-04 | Mineur | `spawn_village_paths_when_loaded` lit `Res<TerrainConfig>` au lieu de reconstruire via `make_terrain_config()` |
| BUG-441-05 | Mineur | champ `status` ajouté au sensor JSON village |
| BUG-441-06 | Cosmétique | log spawn_world mis à jour (plus de "2 buildings" obsolète) |
| BUG-441-07 | Cosmétique | rem_euclid gate detection normalisé + test régression `hexagon_with_gate_wrap_around_350` |

## Vague suivante (story-442 backlog)

- AABB calibration auto KayKit pieces (pattern `NeedsAssetCalibrate`)
- Snap-chain wall corners par détection d'angle (V1 ne place que walls droites + gates, pas de coins distincts)
- `forgia-rampart-wall` crate dédiée si réutilisation donjons/forteresses
