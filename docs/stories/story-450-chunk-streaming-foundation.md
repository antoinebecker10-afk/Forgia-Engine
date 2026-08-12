---
> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia_chunk_stream.json`, fichier `chunk.rs`, symbole `StreamingPauseMode`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

id: story-450
title: Chunk Streaming Foundation — observability + memory budget + dual radii
status: IN_PROGRESS
scale: BMAD Enterprise
created: 2026-05-18
workspace: V2 Rewrite
---

# Story-450 — Chunk Streaming Foundation

> Avant tout enrichissement de contenu (story-451 decorator, 452 villagers, 453
> multi-kit), perfectionner le système chunk + streaming + mémoire de Forgia
> selon les patterns industrie AAA (UE5 World Partition, Minecraft chunks +
> tickets, Roblox StreamingEnabled, Star Citizen OCS, Unity Addressables).

## Audit code actuel (2026-05-18)

| Composant | Lignes | État | Gap industrie |
|---|---|---|---|
| `forgia-terrain::chunk` | 380 | 🟢 Solide — ChunkManager + LRU cache 128 + zstd compress + 16 tests | Pas de splitting sim/view radius |
| `forgia-terrain::lod` | 327 | 🟢 LOD 3-tier hystérèse 16m | Sensor minimal, pas de gen_ms histogram |
| `forgia-terrain::pipeline_diag` | 41 | 🔴 **STUB** (`fn record_event() {}`) | Manque tout |
| `forgia-terrain::meshing_heightmap` | 280 | 🟡 Mesh gen — pas d'instrumentation timing async | Pas de p50/p99 timing |
| Sensor `forgia_chunks_snapshot.json` | — | 🟡 Basique : count + render_dist + biome distribution | Manque eviction, pending, gen_ms, async queue |
| Sensor `forgia_terrain_lod.json` | — | 🟡 Counts par LOD + transitions/frame | Manque histogram, hystérèse rejected |
| Memory budget | — | 🔴 Pas de cap MB (`CHUNK_CACHE_SIZE = 128` hardcoded) | Pas d'enforcement, pas de LRU sur MB |
| Async pipeline | — | ❓ Inconnu (à vérifier `meshing_heightmap`) | Pas de sensor |
| StreamingPause | — | 🔴 Absent | Spawn dans chunk non-loaded possible (Arena sous l'eau pattern) |

## Hardcode violations détectées

- `CHUNK_X = 32`, `CHUNK_Y = 128`, `CHUNK_Z = 32` (chunk.rs:12-14)
- `CHUNK_CACHE_SIZE = 128` (chunk.rs:134)
- `LOD0_MAX_M = 96.0`, `LOD1_MAX_M = 320.0`, `LOD2_MAX_M = 1500.0` (lod.rs:23-27)
- `LOD_HYSTERESIS_M = 16.0` (lod.rs:28)
- `CLUSTER_CHUNKS = 4`, `LOD2_Y_OFFSET = 8.0` (lod.rs:30, 37)
- `streaming_radius = 12` (chunk.rs:194), `streaming_radius = 4` (forgia-rpg)
- `chunks_per_frame = 2` (chunk.rs:195)

Pattern industrie : ces valeurs doivent venir d'un genome `streaming.toml`
hot-reloadable (Shift+F12). Les constantes Rust restent pour `default()`
fallback uniquement (pattern graceful ArenaBotsGenome).

## Industry research (verified sources)

| Source | Pattern | URL |
|---|---|---|
| UE5 World Partition | Grid cells + loading range + HLOD | docs.unrealengine.com/.../world-partition |
| UE5 Level Streaming Volumes | **Unload hysteresis 2.0s default**, no hysteresis on load | docs.unrealengine.com/.../level-streaming-volumes |
| Minecraft chunks | **Decoupled view_radius / simulation_radius** + ticket levels 22-44 | minecraft.wiki/w/Chunk |
| Roblox StreamingEnabled | `StreamingMinRadius` (correctness) + `StreamingTargetRadius` (quality, default 1024 studs) + `StreamingPauseMode` | create.roblox.com/docs/workspace/streaming |
| Star Citizen OCS | Container-based hierarchical streaming | starcitizen.tools/Object_Container_Streaming |
| Witcher 3 / Umbra 3 | Visibility queries → streaming priority (frustum + occlusion) | gdcvault.com/play/1020231 |
| Cyberpunk 2077 | Prefab `.streamingsector` + vertical Y-axis culling | media.gdcvault.com/gdc2023/Slides/Buildingnightcity_Tremblay_Charles.pdf |
| Unity Addressables | `memoryBudgetKB` LRU + refcount eviction | docs.unity3d.com/Packages/com.unity.addressables@2.1 |
| Bevy async | `AsyncComputeTaskPool::get().spawn` + `poll_once` pattern | bevy-cheatbook.github.io/fundamentals/async-compute.html |
| bevy_voxel_world | `ChunkWillChangeLod` event + delegate meshing | github.com/splashdust/bevy_voxel_world |

## Concept-First Protocol

- **Étape 0 (data vs code)** : config = `streaming.toml` (data layer, hot-reload).
  Code = enforcement layer (radii compute, budget eviction, sensor emit).
- **Étape 1 — hypothèses** :
  - (a) **Single-radius simple** (current) → thrash boundaries, no memory cap, no pause
  - (b) **Dual-radius Minecraft** : sim ≠ view → meilleur, mais pas de budget enforcement
  - (c) **Triple-radius industry** : `simulation_m` (correctness) < `view_m` (quality) < `unload_m` (hysteresis) + memory budget LRU + StreamingPause Roblox-style
- **Choix** : (c) — pattern composite Minecraft + UE5 + Roblox + Unity
- **Étape 2 — cartographier** : forgia-terrain::chunk (ChunkManager), forgia-terrain::lod (LOD pipeline), forgia-terrain::pipeline_diag (STUB), forgia-rpg::lib (call site streaming_radius)
- **Étape 3 — verbaliser** : Producteur = `StreamingConfig` Resource genome-loaded au Startup. Consumers = ChunkManager (load/unload decisions), LOD pipeline (radii), foliage (exclusion). Sensor = `forgia_chunk_stream.json` 1Hz. Hot path = oui (chunk update toutes les frames).
- **Étape 4 — hot path check** : sensor 1Hz no-alloc (Local<String> buffer), eviction loop O(N log N) sorted by distance (acceptable N≈256), pas de HashMap::new dans hot path.
- **Étape 5 — scale-up BMAD** : 5+ fichiers touchés workspace-wide + nouvelle crate + genome → Enterprise confirmé.

## Architecture proposée (4 vagues)

```
┌──────────────────────────────────────────────────────────────────┐
│ config/genomes/streaming.toml          ← genome-driven hot-reload│
└──────────────────────────────────────────────────────────────────┘
                          │
                          ▼
        ┌──────────────────────────────────────┐
        │ forgia-streaming (NEW crate, Tier 2) │
        │ ─ StreamingConfig Resource           │
        │ ─ StreamingRadii (sim/view/unload)   │
        │ ─ MemoryBudget (max_mb, max_chunks)  │
        │ ─ StreamingStats Resource            │
        │ ─ StreamingPause Resource            │
        │ ─ Sensor forgia_chunk_stream.json    │
        │ ─ Health side-file                   │
        └──────────────────────────────────────┘
                          │
        ┌─────────────────┴─────────────────┐
        ▼                                   ▼
┌──────────────────┐              ┌──────────────────┐
│ forgia-terrain    │              │ forgia-rpg       │
│ chunk.rs          │              │ lib.rs           │
│ ↳ consume radii   │              │ ↳ consume pause  │
│ ↳ enforce budget  │              │ ↳ player gate    │
│ pipeline_diag.rs  │              │                  │
│ ↳ REAL metrics    │              │                  │
└──────────────────┘              └──────────────────┘
```

## Acceptance criteria

### Wave 1 — Observabilité + foundation crate (cette session)
- [ ] NEW crate `forgia-streaming` (Tier 2, lib only)
- [ ] `StreamingConfig` Resource + TOML genome `streaming.toml`
- [ ] `StreamingStats` Resource (counts, gen_ms histogram, eviction log)
- [ ] Sensor `forgia_chunk_stream.json` 1Hz écrit depuis NEW crate
- [ ] Health side-file `forgia_chunk_stream_health.json` avec next_step
- [ ] `pipeline_diag.rs` réécrit (was STUB → real impl)
- [ ] Plugin enregistré dans forgia-game
- [ ] 8+ tests headless (config parse, stats compute, severity logic)
- [ ] 0 clippy warning

### Wave 2 — Dual radii integration (next session)
- [ ] forgia-terrain consume `StreamingRadii` (replace single `streaming_radius`)
- [ ] Hysteresis on unload (`min_residence_secs >= 2.0`)
- [ ] Sensor reports `hysteresis_blocked_unloads`

### Wave 3 — Memory budget + async metrics
- [ ] `MemoryBudget { max_mb: 512, max_chunks: 256 }` LRU enforcement
- [ ] Async chunk gen pipeline timing (gen_ms_p50, p99)
- [ ] `eviction_reason` histogram (distance/budget/lod_demotion)

### Wave 4 — StreamingPause + frustum priority + debug overlay
- [ ] `StreamingPause` Roblox-style (block spawn until min-radius ready)
- [ ] Pending load queue sorted by `(in_frustum_first, distance_asc)`
- [ ] Debug overlay F3 niveau 3 : chunk grid + LOD colors + load state gizmos

## Out of scope (Phase 2 backlog)

- HLOD remesh builder (UE5 pattern) — offline tool
- Container grouping (Star Citizen OCS) — Bevy ECS direct enough Phase 1
- Occlusion-based priority (Umbra) — frustum suffisant
- Per-player streaming (lightyear multi-joueur) — V2 ship single-player

## Stability Locks impactés

- L1 (GameAssets) : N/A
- L7 (GameSet) : sensor system dans `Sensors` set
- Aucun Lock modifié

## Risks

- 🟡 Refacto chunk.rs en Wave 2 risque casser ChunkManager LRU. Mitigation : tests headless existants + auto-QA verifier
- 🟢 Genome hot-reload : pattern déjà éprouvé (ArenaBotsGenome, fps_tuning)
- 🟢 Sensor overhead : 1Hz no-alloc, négligeable

## Cross-refs

- `.claude/rules/no-hardcode.md` — toutes les constantes terrain → genome
- `.claude/rules/observability-required.md` — sensor + health side-file obligatoires
- `.claude/rules/fine-grained-crates.md` — NEW crate justifié (Tier 2, ≥2 callers)
- Memory `reference_v2_heightmap_grid_industry_rpg_pattern.md` — Skyrim/Witcher 3 RPG terrain
- Memory `reference_v2_lod_gta5_3tier_port.md` — V1 LOD GTA5 port (lod.rs origin)
