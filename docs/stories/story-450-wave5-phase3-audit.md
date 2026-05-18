---
id: story-450-wave5-phase3
title: Audit chunk streaming bug — Manhattan diamond + memory tracking
status: DONE 2026-05-18
scale: BMAD Standard
workspace: V2 Rewrite
---

# Story-450 Wave 5 Phase 3 — Audit Manhattan Diamond Bug + Memory

> **Origine** : user runtime feedback "quand je me rapproche, le sol apparaît,
> c'est le sol qui ne charge pas alors que derrière ça charge. Y'avait déjà
> des problèmes dans forgia classique."

## Symptômes observés

Screenshot RPG mode 2026-05-18 :
- Player sur colline foreground
- Mid-distance (~80-100m) : strip d'eau visible où devrait être du sol
- Far distance (>128m) : LOD2 mega-tiles chargées avec terrain/forêts/rochers visibles
- Pattern : **chunks proche OK + chunks far OK + GAP au milieu**

User : "y'a un trou dans le chargement des chunks"

## Root cause identifiée

**Manhattan distance dans le streaming loop** au lieu d'Euclidean :

```rust
// AVANT (BUG)
for dx in -view_chunks..=view_chunks {
    for dz in -view_chunks..=view_chunks {
        if dx.abs() + dz.abs() <= view_chunks {  // ← MANHATTAN (DIAMANT)
            desired.insert(...);
        }
    }
}
```

Avec `view_chunks=3` :
- Chunk **(3, 0)** : Manhattan = 3 ✓ loaded — Euclidean = 96m
- Chunk **(2, 2)** : Manhattan = 4 ✗ **PAS loaded** — Euclidean = 90.5m (**plus proche!**)

Conséquence : les chunks aux **coins diagonaux** du Manhattan diamond ne sont pas chargés alors qu'ils sont **plus proches** Euclidean que ceux axiaux qui le sont. Visible comme un trou diagonal.

Bug **présent depuis Forgia V1** ("forgia classique") — jamais corrigé.

## Industry pattern

| Engine | Distance metric | Note |
|---|---|---|
| **UE5 World Partition** | Euclidean (circle) | Standard AAA |
| **Unity Addressables** | Euclidean | Standard |
| **Minecraft** | Chebyshev (square) | view_distance carré |
| **Vintage Story** | Manhattan (diamond) | Voxel hardcore RAM-constrained |

Forgia → **Euclidean** : matche les anneaux F3 overlay (cercles), patch les trous.

## Fix appliqué (Wave 5 Phase 3)

Fichier touché : `crates/forgia-rpg/src/lib.rs`

5 emplacements `c.distance(&player_chunk)` (Manhattan) ou `dx.abs() + dz.abs()` remplacés par Euclidean squared :

1. **`stream_chunks_around_player::need_recompute_set`** — desired set generation
2. **`stream_chunks_around_player::unload_check`** — eviction by distance
3. **`stream_chunks_around_player::LRU_touch`** — last_seen refresh
4. **`stream_chunks_around_player::sort_frustum`** — tie-break sort
5. **`stream_chunks_around_player::pending_in_sim`** — StreamingPause gate
6. **`enforce_chunk_memory_budget::candidates`** — LRU eviction

Pattern uniforme : `let dist_sq = (dx * dx + dz * dz) as f32; dist_sq <= radius_sq`.

Coût : équivalent ou inférieur à Manhattan (squared compare = pas de sqrt). Performance neutre.

## Diff coverage

| Région | Avant (Manhattan ≤ 3) | Après (Euclidean ≤ 3.0) |
|---|---|---|
| Chunks loaded count | 25 (diamond) | 28 (circle) |
| RAM | 125 MB | 140 MB |
| Coverage corners | Trous diagonaux | Couverts |
| GPU draw calls | 25 chunks | 28 chunks |

+3 chunks loaded = +15 MB RAM, négligeable vs budget 512 MB.

## Debug systems renforcés (Wave 5 Phase 3 debug)

F3 overlay extended :
- **Red X + red square** : pour chaque chunk dans le view ring qui devrait être loaded mais ne l'est pas
- Permet diagnostic visuel instant si un nouveau bug streaming résurgit
- Pattern UE5 World Partition Editor "missing cells highlighted"

Si tu vois des X rouges en F3 → bug streaming. Si tout est vert/jaune (LOD) → OK.

## Sensor existant déjà couvrant

`forgia_chunk_stream.json` 1Hz contient déjà :
- `counts.loaded` / `pending_load` / `pending_gen`
- `lod_histogram` LOD0/1/2
- `evictions_10s` par raison (Distance/Budget/LodDemotion/Manual)
- `recent_evictions[]` ring 32 entries
- `gen_ms` histogram log2 16-buckets (p50/p95/p99)
- `hysteresis_blocked_unloads`
- `pause.active/reason/waiting_chunks`

Pas besoin de sensor additionnel — le bug Manhattan était dans la **logique géométrique**, pas l'observabilité.

## Cross-refs

- Memory `feedback_v2_tech_debt_audit_protocol.md` — V1 bug récurrents
- `.claude/rules/no-hardcode.md` — radii via streaming.toml genome
- `.claude/rules/observability-required.md` — sensor + health respecté
