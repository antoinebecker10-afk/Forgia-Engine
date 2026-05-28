# Story-547 — forgia-debug crate (3-layer architecture)

**Status** : CODE-COMPLETE (2026-05-28) — wiring forgia-game deferred (WIP autre terminal)
**Priorité** : 🟡 P1 — outillage dev loop (suite story-546)
**Scale BMAD** : Standard
**Origine** : 2026-05-28 — discussion architecture debug/QA. User questionne fusion `forgia-qa-*` dans `forgia-debug`. Décision : architecture **3 couches** distinctes.

## Contexte

Story-546 a documenté 64 sensors JSON via `SENSOR_REGISTRY.md` + `cargo xtask sensor-audit`. Mais le diagnostic dev loop reste lent : lire JSON manuellement, pas d'overlay live, F-keys éparpillés (3 sites Shift+F12 ad-hoc, F3 isolé `forgia-rpg`), pas de console runtime, pas d'inspector.

Audit côté QA : 4 crates QA déjà solides (4779 LOC : qa-core + qa-harness + qa-replay + qa-autopilot). Distinctes par lifecycle (CI/headless vs dev/windowed). **NE PAS fusionner.**

## Architecture cible

```
                forgia-game (binary)
                       ▲
       ┌───────────────┼───────────────┐
       │               │               │
  forgia-debug    forgia-qa-*    runtime crates
  (live dev UX)   (automation)
       │               │
       └─── consomme ──┴── forgia-observability
                           (sensors + checks)
```

- `forgia-observability` = foundation partagée (existant)
- `forgia-debug` = NEW, UX dev live (overlay/console/inspector/F-keys)
- `forgia-qa-*` = existant, automation (BugReport/replay/bots)

## Scope MVP (cette story)

**INCLUS** :
1. Scaffold crate `forgia-debug` (Cargo.toml + lib.rs + plugin)
2. `DebugBindings` Resource — catalog F-keys (F3 toggle overlay, future-proof pour F4-F9)
3. `DebugOverlay` egui window F3 — affiche temps réel : FPS, app_mode, sensor count, last alerts. Lecture via `forgia-observability` state ou JSON re-read 1Hz
4. Plugin `ForgiaDebugPlugin` wirable dans `forgia-game`
5. Wire workspace `Cargo.toml` + ajout dep dans `forgia-game/Cargo.toml`

**HORS SCOPE (story-548+ follow-up)** :
- Console runtime (`:set`, `:spawn`, `:teleport`) — complexe, deferred
- Entity inspector (bevy-inspector-egui integration) — deferred
- Migration des 3 Shift+F12 ad-hoc dans forgia-debug — deferred
- F4 PerfMode (Lock L2 du CLAUDE.md, jamais ressorti en V2)
- BugReport bridge depuis overlay vers `forgia-qa-core`

## Critères d'acceptation

- [ ] AC1 — crate `crates/forgia-debug/` créée avec Cargo.toml + src/lib.rs
- [ ] AC2 — `ForgiaDebugPlugin` exporté via `prelude`, déclaré idempotent (`is_plugin_added` guard)
- [ ] AC3 — `DebugBindings` Resource avec API `register(action, KeyCode)` + lookup. F3 wired par défaut sur `DebugAction::ToggleOverlay`
- [ ] AC4 — `DebugOverlay` egui window affiche : FPS, last frame_time_ms, GameMode actuel, count sensors stale (lecture `forgia2_sensor_health.json`), last 3 health alerts (lecture `forgia2_health.json`)
- [ ] AC5 — workspace Cargo.toml + forgia-game/Cargo.toml mis à jour. `cargo check -p forgia-debug` + `cargo check -p forgia-game` clean
- [ ] AC6 — `cargo clippy -p forgia-debug --no-deps` 0 warning
- [ ] AC7 — xtask `no-scaffold` allowlist mise à jour si LOC < 50 effective (sinon réelle production)

## Test in-game recap

1. **Action** : `cargo run -p forgia-game --profile release-fast`, presser **F3** une fois en jeu (n'importe quel mode)
2. **Redémarrage requis** — nouvelle crate
3. **Effet visuel attendu** :
   - Fenêtre egui top-left "Forgia Debug" apparaît
   - Lignes : `FPS: 60.0`, `frame_ms: 16.7`, `mode: InGame::Roguelite`, `sensors_stale: 0`, `alerts: <last 3>`
   - Re-press F3 : fenêtre disparaît
4. **Sensor** :
   - Aucun sensor dédié pour cette MVP (overlay = consommateur pur)
   - Validation indirecte : pas de regression `forgia2_health.json`
5. **Variantes si KO** :
   - F3 conflit avec `forgia-rpg/src/lib.rs:706` → renommer cet ancien usage ou migrer dans `DebugBindings`
   - egui window n'apparaît pas → vérifier `EguiContexts` SystemParam + `EguiPlugin` déjà ajouté par `forgia-ui-lib`
   - JSON lecture trop lente → cache 1Hz côté plugin (Local<f32> throttle)

## Cross-refs

- Story-546 — sensor registry + audit (foundation lectures)
- `forgia-observability` — source des données affichées
- `forgia-qa-*` — relation sœur, pas fusion
- CLAUDE.md Lock L2 — F4 PerfMode (deferred follow-up)
- `.claude/rules/observability-required.md` — règle parente
