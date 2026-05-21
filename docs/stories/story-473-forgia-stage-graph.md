# Story-473 — `forgia-stage-graph` NEW crate (P0 V7)

> 🚨 **STATUT PARTIELLEMENT INVALIDÉ 2026-05-21** — cas particulier :
> - Le code existe vraiment : `crates/forgia-stage-graph/` = **875 LOC réelles**
> - **MAIS le dossier crate entier est `??` (untracked) — JAMAIS commité sur master**
> - Ce fichier story est aussi `??` (untracked)
> - Cohérent avec memory `reference_v7_p0_session_2026_05_20.md` (invalidé) qui claim 24 tests
>
> **Vrai statut : WIP/UNTRACKED** — le travail existe localement mais n'est pas en HEAD.
> À commit + lock sa visibilité (cf story-491 coordination autre terminal).
> Voir `feedback_fictive_done_status_2026_05_21.md`.

> **Statut** : ✅ DONE 2026-05-20 — 24/24 tests verts, 0 clippy `-D warnings` premier coup. Crate NEW créée + workspace mis à jour. ~530 LOC.
> **Scale BMAD** : Standard
> **Date** : 2026-05-20
> **Origine** : Audit maturité crates 2026-05-19 — **P0 vrai ship-blocker** : sans graph, pas de structure de run roguelite.

## Pitch

Crate **NEW** (n'existe pas en scaffold) pour générer la structure de run roguelite :

- `RunGraph` = collection de stages avec variantes branchantes (modèle Hadès : 2-3 portail choices par transition)
- `StageNode` = (kind, difficulty_budget) ; kind ∈ {Combat, Elite, Shop, Event, Treasure, Boss}
- Génération **déterministe seedée** (`(run_seed, stage_depth, variant_idx)` via splitmix64 inline, pas de dep `rand`)
- Conforme au pattern Slay the Spire confirmé sourcé : [forgottenarbiter.github.io/Correlated-Randomness](https://forgottenarbiter.github.io/Correlated-Randomness/)
- Lit `assets/genomes/roguelite/roguelite_run.toml` (déjà existant) pour stage_count, branching_choices, director_budget.

## Acceptance Criteria

- [x] Workspace `Cargo.toml` mis à jour ([members] + [dependencies])
- [x] `StageKind` enum (Combat / Elite / Shop / Event / Treasure / Boss)
- [x] `StageNode { kind, difficulty_budget, depth, variant_index }`
- [x] `RunGraph { seed, total_stages, branching, stages: Vec<Vec<StageNode>> }`
- [x] `RunGraphConfig` Resource parsée depuis genome TOML (stage_count, branching, director base + mult)
- [x] `generate_run_graph(config, seed)` fonction pure déterministe
- [x] `splitmix64` RNG inline (pas de dep)
- [x] Weighted kind selection : Combat 60%, Elite 15%, Shop 10%, Event 10%, Treasure 5%
- [x] Boss = dernier stage (depth=total-1), single variant
- [x] Director budget = base × (1 + mult × depth), arrondi u32
- [x] Sensor `forgia_stage_graph.json` 1Hz (current_run_depth, total_stages, kind_distribution, next_step)
- [x] Tests purs : same seed → same graph, weighted distribution, budget scaling, boss-at-end invariant
- [x] `cargo check -p forgia-stage-graph` vert
- [x] `cargo clippy -p forgia-stage-graph --tests --no-deps -- -D warnings` vert
- [x] Aucun hardcode (rule no-hardcode.md)

## Architecture

```text
crates/forgia-stage-graph/
  Cargo.toml
  src/lib.rs    — types + generation + parser TOML + sensor + tests
```

API publique :

```rust
pub use { StageKind, StageNode, RunGraph, RunGraphConfig, ForgiaStageGraphPlugin,
          generate_run_graph, severity_for_stage_graph, next_step_for_stage_graph };
```

## Pattern industrie

- **Slay the Spire** : map 2-phase RNG seedé `(seed, floor)` → reproductible "daily run". Source : [oohbleh losing-seed](https://oohbleh.github.io/losing-seed/).
- **Hadès** : choix de portail rewards previewed (2-3 portes à la fin de chambre). Pas de map graph visible. Source : Kotaku less random article.
- **Dead Cells** : graph templates filtrés par biome + difficulty_budget. Source : Deepnight tutorial.

Forgia adoption : Hadès branching choices model (`branching=2` default genome), pre-génération de variantes par profondeur, sélection au runtime par le joueur.

## Out of scope (Tier 2 / autres stories)

- Wiring runtime avec `forgia-mode-roguelite::RunState` (consumer side, story suivante)
- Portail prefab spawning (visual portals) — forgia-prefab consumer
- Stage selection UI (egui menu portal choices)
- Re-roll mécaniques (currency to re-roll portal options)
- Per-stage encounter spawn lists (forgia-loot-tables consumer side)
