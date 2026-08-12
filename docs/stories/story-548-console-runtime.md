# Story-548 — Console runtime debug (forgia-debug::console)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (fichier `console.rs`, symbole `Console`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

**Status** : CODE-COMPLETE (2026-05-28) — 9 unit tests parser passent, wiring forgia-game deferred
**Priorité** : 🟢 P2 — gain dev loop ×10
**Scale BMAD** : Standard
**Origine** : 2026-05-28 — suite story-547, user pousse pour vrai gain de productivité au-delà des overlays passifs.

## Contexte

Stories 546+547 livrent observabilité passive. Mais pour itérer rapidement sur les bugs (story-540 stuck, 545 invincible, etc.) il faut **modifier l'état runtime sans recompile**. V1 avait `console.rs` (600 LOC, F6+`~`) avec commandes `:teleport` / `:heal` / `:god` / `:regen_chunk` / `:load_scene`. V2 = absent.

## Architecture

Module `forgia-debug::console` qui :
- Expose une `Console` egui window toggle par touche `~` (Backquote en AZERTY) ou F1
- Parse les commandes texte → émet des `ConsoleEvent` typés
- Les crates gameplay (forgia-player, forgia-mode-roguelite, etc.) **subscribent optionnellement** aux events via `EventReader<ConsoleEvent>` — **forgia-debug ne dépend PAS de gameplay**
- Maintient une history scrollable + input recall up/down (V1 pattern)
- Bloque les inputs gameplay quand focus console (via `ConsoleState.has_focus()` que les systèmes gameplay gate via `run_if`)

## Commandes MVP (couverture ≥70% besoins identifiables)

| Commande | Effet | Bug ciblé |
|---|---|---|
| `:help` | Liste commandes | UX |
| `:teleport <x> <y> <z>` | Move player | story-540 stuck |
| `:heal [amount=full]` | Restore HP | story-545 validation |
| `:god` | Toggle invulnerable | dev loop |
| `:respawn` | Trigger respawn | flow test |
| `:wave <n>` | Roguelite jump wave | progression test |
| `:set <key> <value>` | Modify FpsTuning runtime | balance |
| `:spawn <archetype>` | Spawn entity | combat tests |
| `:dump_sensors` | Force re-poll sensors maintenant | diagnostic |
| `:clear` | Clear console history | UX |

## Critères d'acceptation

- [ ] AC1 — `crates/forgia-debug/src/console.rs` créé, ~250-300 LOC
- [ ] AC2 — `ConsoleEvent` enum exporté via `prelude`, 10 variants MVP
- [ ] AC3 — `ConsoleState` Resource avec `visible`, `has_focus()`, `history`, `input_history` (up/down recall)
- [ ] AC4 — Binding `Backquote` (~) toggle console par défaut, registrable via `DebugBindings`
- [ ] AC5 — Parser commande robuste : tokenize args, gestion erreur "unknown command" / "missing args"
- [ ] AC6 — Events émis correctement (test mental : `:teleport 10 5 20` → `ConsoleEvent::Teleport { x: 10.0, y: 5.0, z: 20.0 }`)
- [ ] AC7 — `cargo check -p forgia-debug` + `cargo clippy -p forgia-debug --no-deps` 0 warning
- [ ] AC8 — Story DONE + memory `reference_console_runtime_pattern.md`

## Test in-game recap (post-wiring)

1. **Action** : `cargo run -p forgia-game --profile release-fast`, presser `~` en jeu
2. **Redémarrage requis** — modif `.rs`
3. **Effet visuel attendu** :
   - Fenêtre egui "Forgia Console" bottom (style retro CLI)
   - Type `:help` + Enter → liste 10 commandes
   - Type `:teleport 0 5 0` → event émis (visible dans log + sensor `forgia_debug_console.json` futur)
   - Up arrow → recall dernière commande
4. **Sensor** :
   - Pas de sensor MVP (events flux mémoire). Phase 2 = `forgia_debug_console.json` audit trail
5. **Variantes si KO** :
   - `~` conflit AZERTY → re-binder via `DebugBindings::register(KeyCode::F1, DebugAction::ToggleConsole)`
   - Input gameplay non bloqué → vérifier que systems player utilisent `run_if(not(console_focused))`
   - Events non consommés → gameplay crates pas encore connectées, normal MVP

## Hors scope (follow-ups)

- Consommateurs gameplay (forgia-player, forgia-mode-roguelite consomment les events) — story-548b plug-in
- Sensor `forgia_debug_console.json` audit trail — story-548c
- Auto-complete (Tab) — nice-to-have
- Multi-line scripts (`.run script.txt`) — over-scope

## Cross-refs

- Story-547 — forgia-debug architecture 3 couches (foundation)
- Pattern V1 : `d:/Forgia/RUST/Forgia/Forgia/forgia-game/src/debug/console.rs` (600 LOC, inspirations)
- `feedback_mvp_underestimates_coverage_default.md` — scope ≥70% d'emblée
