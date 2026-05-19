# Story-464 — Bot LOS State Gating (Arena AI tracking fix)

**Status** : IN PROGRESS
**Scale** : Standard (4 fichiers)
**Date** : 2026-05-19
**Parent** : story-456 (Hit Feedback AAA) — Phase 2 refinement

## Symptôme

Bots arena Chase player à travers les murs : la state machine et le tactical
movement ignorent `bot.has_los` (produit par `tactical::bot_los_check`).
Tant que `distance ≤ detect_range (50m)`, le bot fonce vers la dernière
position connue. Tracking "permanent" perçu par l'user.

Confirmé concept-first 2026-05-19 :
- Producteur `tactical.rs:80 bot_los_check` set `has_los` correctement (8 Hz raycast).
- Consommateur `lib.rs:268 bot_shoot_at_target` gate déjà le tir sur `has_los` ✅.
- Consommateurs `lib.rs:208 bot_state_machine` et `tactical.rs:248 bot_tactical_movement` **ignorent** `has_los` ← BUG.

Hypothèse (a) "reset alerted cassé" falsifiée — `bot_perception_alert:174`
gate `!bot.alerted` empêche re-trigger continu, et reset via `alert_left`
fonctionne.

## Acceptance Criteria

- [ ] Genome `arena_bots.toml` expose `los_lost_grace_secs` (default 2.0).
- [ ] `ArenaBot` étendu avec `los_lost_grace_left: f32`.
- [ ] `bot_los_check` set `los_lost_grace_left = los_lost_grace_secs` lors
      de la transition `has_los: true → false`. Décrémenté chaque frame.
- [ ] `bot_state_machine` ne sélectionne `Chase` que si :
      `has_los || los_lost_grace_left > 0 || alerted`. Sinon → `Idle`.
- [ ] `bot_tactical_movement` applique le même gate (pas de movement vers
      target sans LOS récente ni alerted).
- [ ] Tests headless ajoutés (3 minimum) :
      `lost_los_drops_chase_after_grace`,
      `keeps_chase_if_alerted_without_los`,
      `regains_chase_on_los_reacquire`.
- [ ] Sensor `forgia_bot_ai.json` : `bots_with_los` doit baisser quand le
      player se cache, et `bots_chasing` redescendre à 0 après grace.
- [ ] Runtime smoke-test : bot ne fonce plus à travers mur en arena.

## Non-objectifs (scope creep évité)

- Pas de `BotState::Search` avec navigation vers dernière position connue
  (reporté en story-465 si demandé).
- Pas de `last_known_pos` mémorisé (Idle = freeze sur place suffit V1).
- Pas de retouche tuning numérique `detect_range/alert_radius` (la cause
  était structurelle, pas numérique).

## Fichiers touchés

1. `assets/genomes/arena_bots.toml` (+1 gene)
2. `crates/forgia-ai-arena-bot/src/tactical.rs` (TacticalTuning + bot_los_check)
3. `crates/forgia-ai-arena-bot/src/lib.rs` (ArenaBot + bot_state_machine)
4. `crates/forgia-ai-arena-bot/tests/los_gating.rs` (nouveau)

## Sources

- Halo 2 props poll AI (Damian Isla GDC 2005) — LOS check ~8 Hz, grace post-loss.
- F.E.A.R. SAPI (Jeff Orkin GDC 2006) — "lost sight" timer avant disengage.
- `concept-first.md` étape 3 cartographie — producteur/consommateurs explicités.

## Risques

- Faible : tactical.rs isolé, pas de cascade cross-crate.
- ⚠️ Régression potentielle sur tests existants `forgia-ai-arena-bot` qui
  spawn un bot et le déplacent sans rapier context — vérifier que default
  `los_lost_grace_left = 0` ne casse pas leurs assertions (bot doit pouvoir
  Chase si jamais eu LOS, donc gate doit aussi accepter le warmup initial).

## Notes implém

- Default `los_lost_grace_left = los_lost_grace_secs` au spawn → comportement
  "grace de spawn" : bot a 2s de Chase autorisé même avant 1er raycast LOS.
  Sinon les bots gèlent jusqu'au 1er check.
