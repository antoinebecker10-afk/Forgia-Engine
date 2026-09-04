# Forgia: Extensibility Without Dynamic Plugins

> How an AI-native engine sidesteps the Rust plugin-ABI problem by design.

---

## The problem everyone else has

Game engines traditionally want **runtime plugins**: drop a `.dll`/`.so` in a folder,
the engine loads it, new behavior appears. That requires a stable **ABI** (Application
Binary Interface) — a binary contract between the host and the plugin: calling
conventions, memory layout of types, vtables.

Rust deliberately has **no stable ABI**. Type layout and calling conventions may change
between compiler versions, dependency versions, even build flags. Passing a rich Rust
type (like Bevy's `&mut App`) across a dynamic-library boundary is only sound if both
sides were compiled by the *exact same* toolchain with the *exact same* dependency
graph. Bevy tried (`DynamicPlugin`), and removed it in 0.14 because it was
fundamentally unsound: mismatched `TypeId`s, silent memory corruption, random crashes.

So any Rust engine that wants third-party runtime plugins is pushed toward WASM
sandboxes or scripting layers — both of which pay a marshalling tax at the boundary
and lose direct ECS access.

## Forgia's answer: the AI is the linker

Forgia is an **AI-native engine**: the creator describes the game, imports assets, and
the AI writes the Rust code. That inverts the constraint that makes dynamic plugins
necessary in the first place.

Classic engines need dynamic loading because *someone else, somewhere else, at some
other time* compiles the extension. In Forgia, the extension author is the AI working
**inside the engine's own workspace**. When a creator asks for a double-jump, the AI
doesn't inject a binary into a shipped executable — it writes a crate, wires it into
the orchestrator, and recompiles the whole workspace.

**One workspace, one compiler, one build. The ABI problem cannot exist, because there
is no binary boundary to cross.**

This gives us, for free, everything dynamic plugins struggle to provide:

- **Soundness** — no FFI boundary, no `repr(C)` translation layer, no version-skew UB.
- **Full ECS access** — extensions are first-class Bevy plugins, not sandboxed guests.
- **Whole-program optimization** — the compiler inlines across "plugin" boundaries.
- **Compile-time verification** — an incompatible extension is a build error, not a
  runtime crash in a player's hands.

## The two extension axes

What people actually want from runtime plugins splits into two needs. Forgia serves
each with a dedicated mechanism.

### 1. Code extensibility → fine-grained static crates

The workspace is ~61 atomic crates (`forgia-crosshair`, `forgia-viewmodel`,
`forgia-genome-core`, …), each owning one concept, each a
standard Bevy `Plugin`. Thin **orchestrator crates** wire them together:

```rust
// crates/forgia-fps/src/lib.rs (orchestrator — wiring only, no gameplay logic)
impl Plugin for ForgiaFpsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            forgia_crosshair::CrosshairPlugin,
            forgia_viewmodel::ForgiaViewmodelPlugin,
            // ...
        ));
    }
}
```

This *is* a plugin architecture — modular, swappable, independently testable — just
resolved at compile time instead of load time. Governance is mechanical: a crate must
have real consumers (`xtask no-scaffold` rejects empty shells), and orchestrators stay
thin by rule.

### 2. Runtime mutability → genome data hot-reload

The other thing dynamic plugins get used for is **changing behavior without
recompiling**. In Forgia that is a *data* problem, not a code problem. Balance, weapon
stats, movement tuning, biome parameters live in TOML **genomes**, not Rust constants:

```text
assets/genomes/<category>/<name>.toml
```

`forgia-genome-core` provides a typed loader (`Genome<T>` + `GenomeLoader<T>`) plugged
into Bevy's asset hot-reload: edit the TOML, the running game picks it up. No ABI, no
FFI, no recompile — you are reloading a text file through a serde-validated schema.
Player movement, damage tuning, terrain shape and more already run through this path.

### The decision rule

Every change request goes through one question first (our "Concept-First, step 0"):

> **Is this data or code?**

- Behavior that must iterate at runtime speed → **genome TOML** (hot-reload, seconds).
- Structural behavior → **a crate** (recompile, minutes — and the AI does the waiting).

This split covers what dynamic plugins are usually for, with none of their failure modes.

## What about true third-party extensions?

If the ecosystem phase (Phase 3) ever requires *untrusted third parties* to distribute
extensions without going through the AI + recompile loop, the answer is **WASM**
(sandboxed, stable format, safe by construction) — never Rust dylibs. Nothing in the
current architecture closes that door: WASM would slot in as one more host crate.
But it is deliberately deferred; the AI-compiles-everything model serves Phases 0–2
entirely.

## Summary

| Need | Classic engine answer | Forgia answer |
|---|---|---|
| Add new behavior | Dynamic plugin (`.dll`) — ABI hazard in Rust | AI writes a static crate, workspace recompiles |
| Tweak behavior live | Plugin reload / scripting | Genome TOML hot-reload (`Genome<T>`) |
| Third-party mods | Plugin SDK + ABI versioning hell | Deferred → WASM sandbox when needed |

Forgia doesn't *solve* the Rust plugin-ABI problem — it makes the problem
unconstructible. The AI sits inside the build, so every extension is compiled together,
and everything that must change at runtime is data, not code.

---

*Written 2026-06-10. Cross-refs: `.claude/rules/fine-grained-crates.md`,
`crates/forgia-genome-core/src/lib.rs`, ADR-0002 (crate cleanup 266 → 62).*
