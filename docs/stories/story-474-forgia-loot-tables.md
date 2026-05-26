# Story-474 — `forgia-loot-tables` (P0 V7)

> **Statut** : ⏸️ SKIP / coordination requise — l'autre terminal a livré 130 LOC (Souls + Pickup walk-over M2 step 3) avec note "Drop pools rarity reportés step 4+". Mon design weighted-rarity + pity timer reste valide mais doit être mergé en additif après leur step 4. Re-traiter quand leur terminal libère la crate.
> **Scale BMAD** : Standard
> **Date** : 2026-05-20
> **Origine** : Audit maturité crates 2026-05-19 — P0 #3 (bloque M2 gameplay loop)

## Pitch

Drop tables weighted rarity Diablo 3 Loot 2.0 + PoE 2-phase RNG + Hearthstone pity timer. Lit `assets/genomes/roguelite/roguelite_loot.toml` (180 LOC déjà existant). RNG seedé `(run_seed, pool_id, encounter_idx)` via splitmix64.

## AC

- [x] `Rarity` enum (Common / Uncommon / Rare / Legendary)
- [x] `LootEntry` + `LootPool` + `LootTablesConfig` Resource
- [x] `PityState` Resource (dry counters par rarity)
- [x] `roll_drop(config, pool_id, pity, seed)` fonction pure déterministe
- [x] `should_drop(config, kill_type, seed)` pour gate probabilité drop par enemy type
- [x] Sensor `forgia_loot.json` 1Hz
- [x] Tests purs : determinism, weighted distribution, pity increments, rarity gates
- [x] cargo check + clippy strict + 0 hardcode

## Out of scope

- UI pickup visual (forgia-prefab consumer)
- Equipment integration (story-475 forgia-equipment consume)
- Smart loot Diablo 3 bias classe — pas de classe roguelite Forgia MVP
