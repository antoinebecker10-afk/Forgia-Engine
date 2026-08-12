---
> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia_chunk_stream.json`, fichier `lib.rs`, symbole `FoliageCoverageReport`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

id: story-502-A
title: Foliage coverage sensor (chunks_loaded sans veg)
status: IN PROGRESS
parent: story-502
scope: BMAD Standard (~50 LOC + 8 tests)
crate: forgia-streaming
created: 2026-05-22
owner: Claude (Terminal B)
---

# Story-502-A — Foliage coverage sensor

> Sous-phase isolée de [story-502](story-502-chunks-foliage-budget-and-crosscheck.md) (DRAFT bloquée multi-terminal soir 2026-05-21).
> Avance la partie sensor/infra **sans toucher `forgia-foliage` ni `forgia-rpg`** (verrouillés par autre terminal).

## Contexte (rappel court)

Audit V2 streaming 2026-05-21 a identifié G2+G3 : *"veg charge mal sur certains chunks"*. Aucune télémétrie cross-check chunk↔foliage n'existait. Plan story-502 a été rédigé hier mais bloqué par autre terminal qui édite `forgia-foliage/material_override.rs` + `forgia-rpg/lib.rs`.

Cette sous-story livre **uniquement la couche sensor + Resource publique** côté `forgia-streaming` (libre). Le producteur (foliage qui rapporte ses chunks couverts) est déféré à `story-502-B` quand l'autre terminal aura commit.

## Critères d'acceptation

- [x] Resource publique `FoliageCoverageReport { chunks_loaded: u32, chunks_with_veg: u32 }` exposée via `prelude` (default `0,0` — innocuous si producteur absent)
- [x] Sensor `forgia_chunk_stream.json` étendu avec bloc `"foliage_coverage": { loaded, with_veg, without_veg, threshold, sustained_s }`
- [x] Severity escalation : `without_veg > threshold` ET sustained > `sustained_s` → `warning` avec next_step explicite
- [x] Genome `streaming.toml` : 2 genes nouveaux `dbg_foliage_coverage_warn_threshold` (u32, default 4) + `dbg_foliage_coverage_sustained_s` (f32, default 3.0)
- [x] Backward compat : ancien `streaming.toml` (sans ces fields) doit toujours parser via `#[serde(default)]`
- [x] Tests purs (8+) : Resource default, severity escalation, JSON contient nouveaux champs, gene parse fallback, sustained timing
- [x] `cargo check -p forgia-streaming` 0 erreur, `cargo clippy -p forgia-streaming --no-deps` 0 warning

## Hors scope (= story-502-B)

- ~~Wiring producteur foliage qui populate `FoliageCoverageReport`~~ → **livré dans la même session** (autre terminal a fini Roguelite). `forgia-foliage/src/lib.rs::write_foliage_coverage_report` + dep `forgia-streaming` ajoutée dans `Cargo.toml`. eligible = LOD0+LOD1 chunks (LOD2 exclu car mega-tile plate sans veg).
- Retry pending foliage (G2 plan original) → future story-502-C
- Budget per-frame foliage (G1 plan original) → future story-502-D

## Test in-game

1. **Action** : lancer RPG (`cargo run -p forgia-game --profile release-fast`) + se déplacer 10s
2. **Pas de rebuild requis** après TOML : sensor 1Hz, hot via redémarrage seulement (Resource Bevy load au Startup)
3. **Effet attendu côté sensor** : `forgia_chunk_stream.json` contient un bloc `"foliage_coverage"` avec `loaded > 0`, `with_veg = 0` (producteur pas wiré), `without_veg = loaded`
4. **Sensor à observer** : `cat forgia_chunk_stream.json | jq .foliage_coverage`
5. **Variantes si KO** :
   - JSON ne contient pas le bloc → vérifier que `cargo run` a bien rebuild (mtime binaire > source)
   - Severity reste "ok" malgré without_veg > 4 → vérifier sustained_s écoulé (3s) + RPG bien démarré depuis >3s

## Cross-refs

- [project_story_502_chunks_foliage_plan_2026_05_21.md](../../../memory/project_story_502_chunks_foliage_plan_2026_05_21.md) — plan parent
- [reference_v2_streaming_architecture_audit_2026_05_21.md](../../../memory/reference_v2_streaming_architecture_audit_2026_05_21.md) — audit source G2+G3
- [.claude/rules/multi-terminal-coordination.md](../../.claude/rules/multi-terminal-coordination.md) — raison du split A/B
- [.claude/rules/observability-required.md](../../.claude/rules/observability-required.md) — checklist sensor
