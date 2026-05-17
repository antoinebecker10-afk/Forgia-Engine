---
id: story-442
title: Procgen Village V1 — Hamlet + Village generator (data-driven, seeded, reproducible)
status: IN_PROGRESS
scale: enterprise
created: 2026-05-17
author: claude + antoine
parent_audit: docs/audit-procgen-village-2026-05-17.md
---

# Story-442 — Procgen Village V1

## Contexte (depuis audit 2026-05-17)

La story-441 livre un village TOML hardcodé positions à la main. Limites identifiées :
1 fichier = 1 village, aucune variation, pas de seed, pas de validation overlap, pas
de connexion routes↔buildings effective. Audit AAA (Parish-Müller, AC Origins,
Townscaper) montre que la couche correcte est **génération hiérarchique** : street
network → district partition → lot subdivision → building assembly.

## Scope V1 validé (audit §8, recommandation §9 acceptée)

- **Layout** : Hamlet (3-7 buildings) + Village (10-30 buildings) — fusion 2 algos
- **Kit** : KayKit Medieval Hexagon uniquement (cohérence visuelle)
- **Tier** : visuel only via couleur kit (red/blue/yellow propagés depuis genome)
- **Crates externes** : `fast_poisson` en V1 (`hexx`/`ghx_proc_gen`/`landmass` plus tard)

Reportés à story-443/444/445 :
- Capital generator (L-system streets, WFC blocks)
- LOD auto distance
- NPC spawner intra-building
- Economy tier coupling

## Architecture livrable

### Crates à peupler (scaffolds existants)

- `forgia-rng` — xoshiro256++ seeded RNG (reproducibility foundation)
- `forgia-spline` — Bezier utilities (extract depuis `forgia-terrain::paths`)

### Crates à créer (new)

- `forgia-genome-village` — typed `VillageGenome` (Serde TOML)
- `forgia-procgen-graph` — `VillageGraph` data structures (nodes/edges/districts)
- `forgia-village-generator` — orchestrateur procgen (dispatch hamlet/village)

### Crates à adapter

- `forgia-village-loader` — accept `VillageDef` from generator OR file TOML
- `forgia-rpg` — wire generator path via genome request
- `Cargo.toml` workspace — add 3 new crates + `fast_poisson` dep

### Data (new)

- `config/genomes/villages/starter_hamlet.toml` — first genome
- `config/genomes/villages/standard_village.toml` — village preset

## Algorithmes V1

### Hamlet (3-7 buildings) — `hamlet_layout`

1. `fast_poisson` 2D dans bounding circle radius `genome.bounding_radius`
2. Filter samples with `min_separation = max(building_footprints)`
3. Place buildings : required first (well center), optional fill by Poisson order
4. Routes radiales : N segments depuis center via `forgia-spline::build_path_segment`
5. Output : `VillageDef` compatible village-loader (positions + scales fixes du genome)

### Village (10-30 buildings) — `village_layout`

1. Voronoi N cells (3-5 selon target_count) via `voronoice` (déjà workspace)
2. Lloyd relaxation 3 iters pour cells équilibrées
3. Per-cell role assignment (market/residential/workshop) — seeded
4. Per-cell Poisson placement intra-cell (density from genome)
5. A* candidate roads sur graph Delaunay entre cell centers
6. Output : `VillageDef`

### Common

- Seeded entirely via `genome.seed` — même seed = même village exactement
- Genome dispatch via `genome.layout_type` enum

## Acceptance Criteria

### Functional

- [ ] `cargo check --workspace` 0 erreur
- [ ] `cargo clippy --no-deps` 0 warning sur les 5 crates touchées
- [ ] `cargo test -p forgia-rng -p forgia-spline -p forgia-genome-village -p forgia-procgen-graph -p forgia-village-generator` tous tests passent
- [ ] Hamlet seed 42 reproductible bit-à-bit (run 2x = même VillageDef)
- [ ] Hamlet seed 100 ≠ hamlet seed 42 (diversité)
- [ ] Village 15 buildings : 0 overlap (validation post-placement)
- [ ] OnEnter(Rpg) charge `starter_hamlet.toml` via generator, plus de TOML hardcode positions
- [ ] Player teleport vers spawn issu du generator (pas hardcode)

### Observability

- [ ] Sensor `forgia_village_gen.json` 1Hz : seed, layout_type, build_count, time_ms, success/fail
- [ ] Sensor `forgia_village_graph.json` 1Hz : nodes, edges, districts count
- [ ] Health alert si `genome.seed = 0` (suspicieux — default uninit)
- [ ] Health alert si build_count_actual < build_count_target * 0.5 (Poisson échoue)

### Quality

- [ ] 0 hardcode Rust dans positions/sizes/scales (tout via genome TOML)
- [ ] Tests régression : Hamlet basique 5 buildings, Village basique 15 buildings
- [ ] Post-impl auto-QA (verifier + qa-lead agents) — bugs Bloquant/Majeur fixés en session

## Risques + mitigations

| Risque | Mitigation |
|---|---|
| Poisson échoue à placer N buildings dans bounding circle | Retry avec radius +20%, fallback warn + log seed |
| Layout type non implémenté | Match exhaustif + `LayoutType::Capital` => `unimplemented!()` clean error |
| Genome TOML invalide | `forgia-genome-validator` (scaffold à peupler V2) — V1 : Serde fail-fast |
| Race condition VillageDef vs TerrainConfig | Pattern story-441 (Request → Result Resource) appliqué |
| Hex algo coupling avant `hexx` adoption | Math pure dans `forgia-village-generator`, swap vers `hexx` en V2 |

## Hors scope V1 (V2+ explicitement)

- L-system street networks (Capital)
- WFC building assembly
- LOD auto generation
- NPC spawner intra-building
- Economy tier coupling (rich/poor pricing)
- Audio ambient village
- Foliage exclusion radius around village

## Checklist post-impl

À cocher avant DONE — sub-agents verifier + qa-lead exécutés en parallèle.

- [ ] cargo check workspace clean
- [ ] cargo clippy 0 warning sur crates touchées
- [ ] cargo test crates nouvelles passent
- [ ] auto-QA exécutée et bugs Bloquant/Majeur fixés
- [ ] `docs/stories/_index.md` mis à jour
- [ ] story-441 marquée superseded-by 442 (positions hardcode remplacé par generator)
- [ ] Memory update post-session (patterns nouveaux découverts)

## Cross-refs

- Audit parent : `docs/audit-procgen-village-2026-05-17.md`
- Story prédécesseur : story-441 (village TOML hardcode V1)
- Rules : `concept-first.md` §3 étape 0 (data vs code couche), `no-hardcode.md`, `observability-required.md`, `post-impl-auto-qa.md`
