---
paths:
  - "RUST/Forgia/Forgia/forgia-game/src/combat/**"
  - "RUST/Forgia/Forgia/forgia-game/src/ai/**"
  - "RUST/Forgia/Forgia/forgia-game/src/inventory/**"
  - "RUST/Forgia/Forgia/forgia-game/src/player/**"
  - "RUST/Forgia/Forgia/forgia-game/src/effects/**"
  - "RUST/Forgia/Forgia/forgia-terrain/src/**"
---

# Data-Driven Paths Rule (Forgia)

> Complement formel a `no-hardcode.md`. Cette rule path-scoped applique le
> principe **TR-system-004** (data-driven) sur les modules gameplay critiques.

## Principe

Tout code gameplay (combat, AI, inventory, player, effects, terrain) lit ses
parametres depuis :
1. `GenomeRegistry` (TOML hot-reload)
2. `FpsTuning` (runtime params)
3. `config/*.toml` ou `config/*.json` (registres)

Aucune valeur gameplay litterale dans ces paths.

## Cross-references

| Rule | Type de valeur | Source autorisee |
|---|---|---|
| TR-system-004 | Tout objet placeable | genome TOML |
| TR-system-003 | Params runtime | FpsTuning |
| TR-lock-001 | Asset path | GameAssets resource |
| TR-invariant-008 | Material mutation | clone before mutate |

## Patterns acceptes (correct)

```rust
let damage = tuning.combat_base_damage * weapon_genome.damage_multiplier;
let speed = tuning.player_walk_speed * delta;
let asset = game_assets.weapon_sword.clone();
```

## Patterns refuses (violation)

```rust
let damage = 25.0;  // VIOLATION TR-system-004 — use genome
let speed = 5.0 * delta;  // VIOLATION TR-system-003 — use FpsTuning
let asset = asset_server.load("weapons/sword.glb");  // VIOLATION TR-lock-001 — use GameAssets
```

## Exceptions autorisees (limitatif)

- Constantes physiques nommees : `const GRAVITY: f32 = -9.81;`
- Constantes mathematiques : `const TAU: f32 = 6.283185;`
- Limites techniques : `const MAX_BIOMES: usize = 10;` (mais valeurs gameplay = TOML)

## Enforcement

- Hook `validate-commit.sh` (scan proactif L1/L7/QA/hardcode)
- Hook `validate-assets.sh` (genome TOML naming + parse)
- Skill `/balance-check` (audit balance combat/economy)
- Code review obligatoire sur PR