# Story-539 — Multi-Mode Plugin Gating (RPG-only plugins tournent en Roguelite)

**Status:** DRAFT
**Scale:** BMAD Standard (8 crates touchées, story requise, checklist post-impl)
**Created:** 2026-05-27
**Blocks:** Perf Roguelite sustainée (lag_events CRITICAL en wave 2)
**Related:** memory `[[reference-streaming-already-mature-dont-recreate]]`, `[[feedback-sensor-first-then-assume]]`, `ARCHITECTURE.md` §4 (forgia-terrain désactivé en FPS, activé RPG)

---

## 1. Contexte

Audit 2026-05-27 sur diagnostic "ça finit par planter en Roguelite wave 1-2" :

- `forgia2_lag_events.json` : **severity CRITICAL**, 66 events sustained, 42 sur 30 s, spikes répétés à 100 ms (t=19.30/19.68/32.71/35.48/40.87)
- `forgia2_perf.json` : FPS smoothed **640 → 153** entre l'idle stage-ready et wave 2 (frame_time_avg 1.6 → 5.4 ms, max 42 ms)
- Réduction `wall_natural_len_m: 1.0 → 4.0` divise `walls_per_segment` par 3.9 (90 → 23) et `props_spawned` par 3.4 (566 → 164) **mais n'améliore pas la perf** → bottleneck ailleurs.
- `forgia_chunk_stream.json` en Roguelite : `counts.loaded=0, pending_load=0, gen_ms.sample_count=0` (rien à streamer mais plugin actif)
- `forgia2_lag_events.next_step` pointe explicitement vers `forgia_chunk_stream.json`

Root cause découverte par grep audit `crates/forgia-game/src/lib.rs:74-101` :

**7 plugins "RPG-only" ou cross-mode sont ajoutés inconditionnellement sans gate `GameMode`**, malgré le commentaire ligne 73 *"(run_if interne par GameMode)"* :

| Plugin | Ligne wire | Gating interne ? | Raison d'être |
|---|---|---|---|
| **`forgia-streaming`** (1113 LOC) | 76 | **❌ aucun** | Chunk streaming OpenWorld RPG (story-450) |
| `forgia-asset-registry` | 75 | **❌ aucun** | Asset preload RPG |
| `forgia-viewmodel::calibration_sensor` | 78 | **❌ aucun** | Calibration FPS |
| `forgia-anim-debug` | 89 | **❌ aucun** | Anim debug Rex/RPG |
| `forgia-camera-orbit` | 90 | **❌ aucun** | Caméra 3P RPG |
| `forgia-secondary-motion` | 91 | **❌ aucun** | Anim secondary RPG |
| `forgia-village-loader` | 101 | **❌ aucun** | Village RPG (story-441) |

Plugins **correctement gatés** (référence d'excellence) :

- `forgia-water` : `OnEnter/OnExit(GameMode::Rpg)` + run-time guard `*state.get() == GameMode::Rpg`
- `forgia-terrain` : `run_if(in_state(GameMode::Rpg))`
- `forgia-foliage` : `run_if(in_state(GameMode::Rpg))`
- `forgia-rpg` : multi-gates GameMode::Rpg
- `forgia-audio::biome` : gated
- `forgia-mode-roguelite` / `forgia-mode-fps-arena` : ont leur propre gating sur leur mode

## 2. Goals

1. Identifier le contributeur perf principal des stutters sustained 100ms en Roguelite
2. Gater tous les plugins RPG-only par `run_if(in_state(GameMode::Rpg))` (pattern terrain/foliage)
3. Préserver le boot order et les Resource init (les Startup systems peuvent rester globaux, seul Update doit gate)
4. Aligner sur l'invariant `ARCHITECTURE.md` §4 *"forgia-terrain désactivé en FPS, activé en RPG"* → étendre à tous

## 3. Non-Goals

- Refactor centralisé "PluginRegistry" qui auto-gate par mode → over-engineering, story future si récurrent
- Migration vers GameSet conditional → existant suffit
- Gating fin par sub-system dans chaque plugin → 1 seul gate au niveau Plugin::build suffit

## 4. Acceptance Criteria

- [ ] AC1 — `forgia-streaming` : tous les `add_systems(Update, ...)` chained `.run_if(in_state(GameMode::Rpg))`
- [ ] AC2 — `forgia-asset-registry` : idem
- [ ] AC3 — `forgia-viewmodel::calibration_sensor` : idem (ou `run_if(in_state(GameMode::Fps).or(in_state(GameMode::Roguelite)))` si shared FPS/Roguelite)
- [ ] AC4 — `forgia-anim-debug`, `forgia-camera-orbit`, `forgia-secondary-motion` : gated `GameMode::Rpg`
- [ ] AC5 — `forgia-village-loader` : gated `GameMode::Rpg`
- [ ] AC6 — Runtime Roguelite : `forgia2_lag_events.severity` passe de `critical` à `ok` OU `warn`, `events_last_30s < 15` (vs 42 baseline)
- [ ] AC7 — Runtime Roguelite : `forgia2_perf.fps_smoothed > 400` au combat wave 2 (vs 153 baseline)
- [ ] AC8 — Runtime RPG : `forgia_chunk_stream.gen_ms.sample_count > 0` confirme que streaming tourne toujours en RPG (preuve qu'on n'a pas tout cassé)
- [ ] AC9 — `cargo check --workspace` + `cargo clippy --workspace --no-deps -- -D warnings` 0 warning
- [ ] AC10 — Tests purs existants restent verts (28 forgia-stage + 26 forgia-level-presets + autres)

## 5. Architecture & Patterns

### 5.1 Pattern canonique (de `forgia-terrain/lib.rs:95`)

```rust
impl Plugin for ForgiaTerrainPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerrainConfig>()        // Startup-safe, pas de gate
            .add_event::<ChunkLoadedEvent>()         // idem
            .add_systems(Startup, load_terrain_genome)  // Startup, pas de gate
            .add_systems(
                Update,
                (
                    stream_chunks_around_player,
                    poll_chunk_meshing,
                    write_terrain_lod_sensor,
                )
                    .chain()
                    .run_if(in_state(GameMode::Rpg)),  // ← LE gate à propager
            );
    }
}
```

### 5.2 Variantes selon le plugin

- **RPG-only** (streaming, village-loader, camera-orbit, secondary-motion, anim-debug, asset-registry) : `run_if(in_state(GameMode::Rpg))`
- **FPS+Roguelite shared** (viewmodel-calibration) : `run_if(in_state(GameMode::Fps).or_else(in_state(GameMode::Roguelite)))` ou helper `combat_modes()` dans `forgia-core`

### 5.3 Helper centralisé proposé (optionnel, si pattern récurrent)

Dans `forgia-core/src/system_set.rs` (ou nouveau `mode_conditions.rs`) :

```rust
pub fn in_rpg() -> impl Condition<()> { in_state(GameMode::Rpg) }
pub fn in_fps() -> impl Condition<()> { in_state(GameMode::Fps) }
pub fn in_combat_modes() -> impl Condition<()> {
    // FPS + Roguelite (= modes avec hitscan/weapon viewmodel)
    in_state(GameMode::Fps).or_else(in_state(GameMode::Roguelite))
}
```

À évaluer phase 1 — si > 3 plugins utilisent le même combo, créer le helper. Sinon inline.

## 6. Plan d'implémentation

### Phase 1 — Profile-driven : gate UN plugin, mesurer (M, 30 min)

Objectif : prouver que streaming est le contributeur #1 avant de gater les 6 autres.

- Add `run_if(in_state(GameMode::Rpg))` sur `forgia-streaming` Update systems (1 fichier)
- `cargo build --profile release-fast`
- Run Roguelite 60 s, lire sensors :
  - `forgia2_lag_events.events_last_30s` (baseline 42)
  - `forgia2_perf.fps_smoothed` (baseline 153 à wave 2)
- **Si delta significatif** (> 30 % FPS gain OU events / 2) → confirmer hypothèse, passer phase 2
- **Sinon** → investiguer autres (lire `forgia_*.json` un par un pour identifier producteur Update lourd)

### Phase 2 — Extension batch aux 6 autres (M, 1 h)

- 6 fichiers `crates/forgia-*/src/lib.rs` : ajouter le gate sur Update systems
- Préserver Startup / `init_resource` / `add_event` sans gate
- `cargo check --workspace` après chaque crate
- 1 commit par crate (atomic) ou 1 commit batch (selon préférence)

### Phase 3 — Verification (M, 30 min)

- `cargo clippy --workspace --no-deps -- -D warnings` 0 warning
- `cargo test --workspace --no-run` (compile tests)
- `cargo test -p forgia-terrain -p forgia-stage -p forgia-level-presets` (référence non-régression)

### Phase 4 — Runtime gates check (M, 30 min)

- Run RPG mode : vérifier que streaming/foliage/terrain tournent bien (gen_ms.sample_count > 0)
- Run Roguelite mode : vérifier que streaming/foliage/terrain sont idle (gen_ms.sample_count == 0)
- Run Menu : tout idle
- Run FPS Arena : viewmodel-calibration tourne (si conservé en combat_modes), streaming idle
- Mesurer AC6/AC7 finaux

### Phase 5 — Capitalisation (S, 15 min)

- Memory `[[reference-multi-mode-plugin-gating-pattern]]` avec :
  - Pattern canonique (5.1)
  - Liste des plugins gatés
  - Helper `in_combat_modes()` si retenu
  - Anti-pattern : ajouter un plugin sans gate dans `forgia-game/lib.rs` bloc 7
- Update `ARCHITECTURE.md` §4 : généraliser "désactivé en FPS, activé en RPG" → tableau gating par plugin

## 7. Risques

| Risque | Mitigation |
|---|---|
| Plugin gaté mais Resource consommée par autre plugin non-gaté en non-RPG | `init_resource` reste hors `run_if`. Seuls les Update systems sont gatés. Resources accessibles en read partout. |
| Système Update qui DOIT tourner en cross-mode (ex : asset preload init) | Audit cas par cas. `forgia-asset-registry` startup load = global. Stream-watch update peut être RPG-only. |
| Bevy Observer triggered en non-RPG → fire system gaté | Observers ne respectent pas `run_if` plugin-level. Vérifier `[[reference-observer-cross-mode-gate-via-state-read]]` memory : gate par early-return `Option<Res<State<GameMode>>>` dans body. |
| Test runtime gating cassé (`forgia_chunk_stream` idle en RPG = regression) | AC8 contre-check : en RPG, sample_count > 0 obligatoire. |
| Régression `state.is_changed()` patterns | Plugins qui watch State transitions doivent garder leurs systèmes hors gate. À identifier au cas par cas. |
| `OnEnter/OnExit(GameMode::X)` ne sont pas équivalents à `run_if(in_state)` | Préserver OnEnter/OnExit existants (water pattern). Gating Update uniquement. |

## 8. Definition of Done

- Tous AC §4 verts
- Runtime sensor preuve perf delta mesuré (avant/après dans la story finale)
- 7 crates avec gate Update systems + 0 régression sur les autres
- Commit propre par phase
- Memory capitalisée + ARCHITECTURE.md mis à jour

## 9. Follow-ups (stories candidates)

- **Story-540** Helper centralisé `in_combat_modes()` dans `forgia-core` si > 3 plugins l'utilisent
- **Story-541** xtask `check-plugin-gates` : CI grep `add_systems(Update,...)` sans `run_if` dans plugins ajoutés au bloc 7 de forgia-game
- **Story-542** Profile-driven optim Phase 2+ : si après gating les stutters restent > 15 events/30s, identifier le contributeur résiduel via `bevy_diagnostic_overlay` ou `tracy_client`

## 10. Notes capitalisées de l'audit 2026-05-27

- **NE PAS recréer `forgia-stage`** : `[[reference-streaming-already-mature-dont-recreate]]` confirmée (forgia-stage = ~120 K, 28 tests, data-driven). Le prompt initial "map_crypte_v1" voulait spawner hardcode 40 m carrée, refusé.
- **Concept-First étape 0 succès** : la map "Crypts of Anvil" existe déjà comme `stage_id` dans `roguelite_stages.toml`, wirée par `forgia-mode-roguelite/run.rs:154`. Couche = `definition`, pas `framework`.
- **TOML side-fix appliqué 2026-05-27** : `wall_natural_len_m: 1.0 → 4.0` (cohérence sur crypts_of_anvil + forge_sanctum). Garde le bénéfice draw-call /3.4 même si pas le coupable principal. Revertable si visuel "gaps entre murs".
