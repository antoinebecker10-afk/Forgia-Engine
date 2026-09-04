# The Genome System — Forgia's Data Layer, Explained for Engine Devs

> ⚠️ **Short English introduction, written 2026-06-10. The authoritative reference is
> [`GENOME.md`](../GENOME.md)** (French, measured 2026-09-04): it carries the current
> figures, the two file forms, the three loading paths, and the known design flaws this
> page glosses over. Where the two disagree, `GENOME.md` is right.
>
> One idea: **code defines mechanisms, genomes define values — and every value
> carries its own valid range.** ~120 lines of core code, used by ~100 data files.

---

## The pitch

Every engine ends up with a tuning layer: config files, scriptable objects, data
tables. Ours is called the **genome system**, and the biology metaphor is load-bearing,
not branding. A *genome* describes one game concept (a weapon, a biome, an enemy).
It contains *chromosomes* (coherent groups: Ballistics, Handling) made of *genes*
(one parameter each). Real file from the repo:

```toml
# assets/genomes/weapons/weapon_ak47.toml
id = "weapon_ak47"
name = "AK-47"
domain = "Weapon"

[[genes]]
id = "ak47_damage"
label = "Damage"
chromosome = "Ballistics"
min = 5.0
max = 35.0
default = 14.0
```

## The one design decision that matters: bounds on every gene

A classic config says `damage = 14`. A gene says `damage = 14, valid in [5, 35]`.
That turns the data layer from "values someone typed" into a **declared mutation
space**:

- **Tooling can tune without breaking the game.** In our case the tuner is an AI
  ("make this shotgun punchier" → move three genes inside their ranges), but the same
  property serves sliders in an editor UI, balance sweeps, A/B tests, procedural
  variants. `9999 damage` is unrepresentable by construction.
- **Game feel becomes a search space.** Balancing is exploring a bounded region, not
  editing magic numbers scattered through code.

The metaphor completes itself: the code is the organism, the genome is its DNA, and
mutating DNA within bounds yields a viable variant of the same species.

## Implementation (Bevy, but the pattern is portable)

The core crate (`forgia-genome-core`) is deliberately tiny — a generic typed asset
plus a TOML loader:

```rust
pub struct Genome<T> { pub data: T }          // T = consumer's serde struct
pub struct GenomeLoader<T> { ... }            // AssetLoader for .toml → Genome<T>

app.register_genome::<WeaponGenome>();        // one line per consumer crate
```

Each consumer crate owns its schema: a plain serde struct, registered in one line.
Genomes live under `assets/genomes/<category>/<name>.toml` and ride the engine's
standard asset pipeline — which means **hot-reload comes for free**: edit the TOML,
the running game picks it up in seconds, no recompile, no scripting VM, no FFI.

> In this repository, only 14 genome paths actually go through that pipeline; 56 more are
> read straight from disk at startup and do **not** hot-reload. See
> [`GENOME.md`](../GENOME.md) §4.

Three contracts, all unit-tested:

1. **Broken TOML → clean `Err`, never a panic.** A typo in a data file must not be
   able to kill the game.
2. **Missing fields → serde defaults.** Old data files stay forward-compatible with
   newer schemas.
3. **Wrong types / missing required fields → explicit rejection.** No silent ghost
   values.

## Why this split pays off

The discipline is a single routing question asked before any change: **is this data
or code?** A new *mechanic* → code (a crate, compiled, type-checked). A new *value*
of an existing mechanic → a gene (hot-reloaded, bounded, validated).

Consequences we observe in practice:

- **Iteration speed splits cleanly.** Feel/balance iterates in seconds (data); only
  structural changes pay the compile cost (code).
- **No scripting layer needed for tuning.** A lot of engines embed Lua mainly so
  designers can tweak numbers at runtime. Bounded data + hot-reload covers that use
  case with zero VM, zero binding maintenance, zero ABI surface.
- **Content identity is diffable.** A weapon, a biome, a boss *is* a small text file —
  reviewable in a PR, versionable in git, generatable by tools.

~100 genomes currently drive weapons, biomes, enemies, roguelite boons, dialogues,
visual mood, even our health-monitoring thresholds.

## If you steal one thing

Steal the bounds. Hot-reloadable data is table stakes; **per-parameter `min`/`max`
declared next to `default`** is what makes the data layer safe to hand to tools,
designers, or an AI — anything that mutates values faster than a human reviews them.

---

*Forgia, 2026-06-10. Core: `crates/forgia-genome-core/src/lib.rs`. Companion piece:
`docs/explainer-extensibility-without-dynamic-plugins.md` (why we have no dynamic
plugin ABI at all).*
