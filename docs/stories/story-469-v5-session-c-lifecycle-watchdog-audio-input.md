# Story-468 — V5 Session C : sensors lifecycle + watchdog + audio + input + sensor_health

**Statut** : PLAN — en attente validation.
**BMAD scale** : Enterprise (5 sensors + Observer hooks + xtask + tests, ~5-6 h).
**Vague** : V5 — Phase 5b Session C (final, cible 12/13 atteint, +1 reservé future).

---

## 0. TL;DR

5 nouveaux producteurs sensors cross-mode 1Hz, complète V5 :

| Sensor | Approche | LOC |
|---|---|---|
| `forgia2_lifecycle.json` | Observers `On<Add, C>` + Resource counter (Player, TargetCube, NameplateRoot) | ~110 |
| `forgia2_watchdog.json` | `GameTickCounter` Resource (First schedule) + lag frames (>50ms) | ~90 |
| `forgia2_audio.json` | `BiomeAmbientState.current` + `Assets<AudioInstance>::iter()` filter Playing | ~70 |
| `forgia2_input.json` | `EventReader<KeyboardInput>` accum + `ActionState<PlayerAction>` transitions | ~90 |
| `forgia2_sensor_health.json` | Lit 13 forgia2_* timestamps, expose CHK-5 stale_secs canonisé | ~80 |

Étend xtask `verify-sensors-format` : **7 → 12** canonical (sensor_health = meta exposé via JSON propre).

Effort réviséé après research : **5-6 h** (vs 6h plan initial). Risque MOYEN sur Observer `OnRemove` bundle (Bevy issue #18720 à valider compile).

---

## 1. Concept-first 5 étapes

### Étape 0 — Data ou code ?

**Code framework**. Aucune valeur à exposer genome — ces sensors lisent runtime state Bevy (Events, Observers, Resources existants `BiomeAmbientState`, `ActionState`).

### Étape 1 — Hypothèses concurrentes

- **H1 (retenu)** : 5 systems indépendants dans `forgia-observability`, chacun écrit son JSON 1Hz. Pattern miroir de Session B (perf/entities/memory). Observer hooks ajoutés au plugin pour lifecycle.
- **H2 (rejeté)** : extraire crate dédié `forgia-sensors-lifecycle`. Stub `forgia-sensors/src/lib.rs` existe mais reste inactif — 5 sensors dans 5 sub-crates = over-engineering.
- **H3 (rejeté)** : 1 mega-system. Couplage inutile, tests fragiles, ordering pénible.

### Étape 2 — Cartographier

**Producteurs sensors existants** (par concept) :
- Audio biome : `crates/forgia-audio-biome/src/lib.rs:19` `BiomeAmbientState` Resource. Field `current: Option<BiomeType>`.
- Input : `crates/forgia-input/src/lib.rs:40` `PlayerAction` enum derive Actionlike.
- TargetCube : `crates/forgia-mode-fps-arena/src/lib.rs:299` `pub struct TargetCube;`
- Player : `forgia-player/src/lib.rs:71`
- NameplateRoot : `forgia-enemy-nameplate/src/lib.rs:47`
- CHK-5 source : `crates/forgia-observability/src/checks.rs:269` `chk_sensor_liveness`

**Pré-requis Bevy 0.18 syntax confirmé via bevy-specialist research** :
- `On<Add, C>` (PAS `Trigger<OnAdd, C>` — breaking change Bevy 0.18 PR #19596)
- Observer ne peut PAS recevoir `Query<>`, seulement `Resource`/`Commands`/`EventWriter`
- `KeyboardInput` purge events après 2 frames non-lus → accum dans `Local<>` chaque frame
- `bevy_kira_audio` 0.25 : `Assets<AudioInstance>::iter()` + filter `PlaybackState != Stopped`

### Étape 3 — Verbalisation

| Sensor | Producteur (timing) | Consommateurs | Hot | Net | Script |
|---|---|---|---|---|---|
| `forgia2_lifecycle.json` | `sys_write_lifecycle_sensor` (Update, 1Hz Local) + 3 observers `On<Add, _>` | xtask gate + debug | non | L | int |
| `forgia2_watchdog.json` | `sys_update_tick_counter` (First, every frame) + `sys_write_watchdog_sensor` (Update, 1Hz) | xtask gate + Antoine freeze detect | non | L | int |
| `forgia2_audio.json` | `sys_write_audio_sensor` (Update, 1Hz) | xtask gate | non | L | int |
| `forgia2_input.json` | `sys_track_input_accum` (Update, every frame, GameSet::Input) + `sys_write_input_sensor` (1Hz) | xtask gate | non | L | int |
| `forgia2_sensor_health.json` | `sys_write_sensor_health` (Update, 1Hz, lit timestamps des 13 forgia2_*.json) | xtask gate + Antoine méta | non | L | int |

### Étape 4 — Hot path check

Aucun système tagué hot. Tous gates 1Hz. Coût mesuré :
- Lifecycle observers ~1-3µs / event × <100 events/s = négligeable
- Watchdog tick counter en `First` = O(1) increment
- Audio : `Assets::iter().filter()` ~10-50 µs sur ~5-20 instances → OK 1Hz
- Input track accum : `EventReader<KeyboardInput>::read()` boucle chaque frame mais simple ++ counter

### Étape 5 — Scale-up BMAD

5 sensors + Observer hooks + extension Plugin + xtask + config + tests = Enterprise. Story OBLIGATOIRE. Checklist post-impl OBLIGATOIRE.

---

## 2. Détail implémentation

### 2.1 `lifecycle_sensor.rs` (~110 LOC)

```rust
#[derive(Resource, Default)]
pub struct LifecycleCounter {
    pub players_added: u32,
    pub players_removed: u32,
    pub target_cubes_added: u32,
    pub target_cubes_removed: u32,
    pub nameplates_inserted: u32,
}

// 5 observers ajoutés dans ForgiaObservabilityPlugin::build()
app.add_observer(|_: On<Add, Player>, mut c: ResMut<LifecycleCounter>| c.players_added += 1);
app.add_observer(|_: On<Remove, Player>, mut c: ResMut<LifecycleCounter>| c.players_removed += 1);
app.add_observer(|_: On<Add, TargetCube>, mut c: ResMut<LifecycleCounter>| c.target_cubes_added += 1);
app.add_observer(|_: On<Remove, TargetCube>, mut c: ResMut<LifecycleCounter>| c.target_cubes_removed += 1);
app.add_observer(|_: On<Insert, NameplateRoot>, mut c: ResMut<LifecycleCounter>| c.nameplates_inserted += 1);

pub fn sys_write_lifecycle_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    mut counter: ResMut<LifecycleCounter>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 { return; }
    *accum = 0.0;

    let (severity, next_step) = severity_for_lifecycle(counter.players_removed);
    let json = format!(
        r#"{{"id":"lifecycle","severity":"{severity}","next_step":"{next_step}",
"timestamp_secs":{:.1},"players_added":{},"players_removed":{},
"target_cubes_added":{},"target_cubes_removed":{},"nameplates_inserted":{}}}"#,
        time.elapsed_secs(),
        counter.players_added, counter.players_removed,
        counter.target_cubes_added, counter.target_cubes_removed,
        counter.nameplates_inserted,
    );
    let _ = std::fs::write("forgia2_lifecycle.json", &json);

    // Reset deltas après écriture (cumul par-seconde)
    *counter = LifecycleCounter::default();
}

pub fn severity_for_lifecycle(players_removed: u32) -> (&'static str, &'static str) {
    if players_removed > 0 {
        ("warn", "player removed during last second — unexpected outside mode switch")
    } else {
        ("ok", "")
    }
}
```

**Mitigation piège OnRemove bundle (#18720)** : si compile error sur `On<Remove, TargetCube>` ou warnings spurious counts, ajouter au commit message une note de fallback + commenter le bundle hook problématique. Pas bloquant pour MVP.

### 2.2 `watchdog_sensor.rs` (~90 LOC)

```rust
#[derive(Resource, Default)]
pub struct GameTickCounter {
    pub ticks: u64,
    pub last_dt_ms: f32,
    pub total_lag_frames: u32,
    pub consecutive_lag_frames: u32,
}

// Run en First schedule (avant tous les GameSets)
pub fn sys_update_tick_counter(time: Res<Time>, mut counter: ResMut<GameTickCounter>) {
    let dt_ms = time.delta_secs() * 1000.0;
    counter.ticks += 1;
    counter.last_dt_ms = dt_ms;
    if dt_ms > 50.0 {
        counter.total_lag_frames += 1;
        counter.consecutive_lag_frames += 1;
    } else {
        counter.consecutive_lag_frames = 0;
    }
}

pub fn sys_write_watchdog_sensor(
    time: Res<Time>,
    counter: Res<GameTickCounter>,
    mut accum: Local<f32>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 { return; }
    *accum = 0.0;

    let (severity, next_step) = severity_for_watchdog(counter.consecutive_lag_frames);
    let json = format!(
        r#"{{"id":"watchdog","severity":"{severity}","next_step":"{next_step}",
"timestamp_secs":{:.1},"ticks":{},"last_dt_ms":{:.2},
"total_lag_frames":{},"consecutive_lag_frames":{}}}"#,
        time.elapsed_secs(), counter.ticks, counter.last_dt_ms,
        counter.total_lag_frames, counter.consecutive_lag_frames,
    );
    let _ = std::fs::write("forgia2_watchdog.json", &json);
}

pub fn severity_for_watchdog(consecutive_lag: u32) -> (&'static str, &'static str) {
    if consecutive_lag > 30 {  // ~1.5s of lag at 20fps
        ("critical", "consecutive_lag_frames > 30 — sustained freeze detected (Tracy, GPU profile)")
    } else if consecutive_lag > 10 {
        ("warn", "consecutive_lag_frames > 10 — stutter cluster")
    } else {
        ("ok", "")
    }
}
```

**Note watchdog "frozen" complet** : nécessite OS thread séparé (research) — HORS scope Session C. Tick timestamp suffit : Antoine lit forgia2_watchdog.json toutes les 5s, si `ticks` stagne → freeze. Decision documentée.

### 2.3 `audio_sensor.rs` (~70 LOC)

```rust
use bevy_kira_audio::{AudioInstance, PlaybackState};
use forgia_audio_biome::BiomeAmbientState;

pub fn sys_write_audio_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    audio_instances: Res<Assets<AudioInstance>>,
    biome_state: Option<Res<BiomeAmbientState>>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 { return; }
    *accum = 0.0;

    let active_count = audio_instances
        .iter()
        .filter(|(_, inst)| !matches!(inst.state(), PlaybackState::Stopped))
        .count();

    let current_biome = biome_state
        .as_ref()
        .and_then(|s| s.current_biome())  // pub accessor à ajouter ou via field pub
        .map(|b| format!("{:?}", b))
        .unwrap_or_else(|| "none".to_string());

    let (severity, next_step) = severity_for_audio(active_count);
    let json = format!(
        r#"{{"id":"audio","severity":"{severity}","next_step":"{next_step}",
"timestamp_secs":{:.1},"active_instances":{},"current_biome":"{}"}}"#,
        time.elapsed_secs(), active_count, current_biome
    );
    let _ = std::fs::write("forgia2_audio.json", &json);
}

pub fn severity_for_audio(active: usize) -> (&'static str, &'static str) {
    if active > 64 {
        ("warn", "active audio instances > 64 — possible leak")
    } else {
        ("ok", "")
    }
}
```

**Pré-requis** : `BiomeAmbientState.current` peut être private. Ajouter pub accessor `pub fn current_biome(&self) -> Option<BiomeType>` au crate `forgia-audio-biome`. Si refus user → fallback `"unknown"`.

### 2.4 `input_sensor.rs` (~90 LOC)

```rust
use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use leafwing_input_manager::prelude::ActionState;
use forgia_input::PlayerAction;

#[derive(Default)]
pub struct InputSensorAccum {
    pub keys_pressed: u32,
    pub keys_released: u32,
    pub actions_just_pressed: u32,
}

// Tourne chaque frame pour ne pas perdre d'events
pub fn sys_track_input_accum(
    mut events: EventReader<KeyboardInput>,
    action_q: Query<&ActionState<PlayerAction>>,
    mut accum: Local<InputSensorAccum>,
    mut writer_state: Local<InputWriterState>,  // pour passer à writer
    time: Res<Time>,
) {
    for ev in events.read() {
        match ev.state {
            ButtonState::Pressed => accum.keys_pressed += 1,
            ButtonState::Released => accum.keys_released += 1,
        }
    }
    for state in &action_q {
        // PlayerAction::variants() via Actionlike — sinon iterate sur InputMap
        // MVP : compter total juste-pressé this frame via state.get_just_pressed().len()
        accum.actions_just_pressed += state.get_just_pressed().len() as u32;
    }

    // Throttle 1Hz pour flush
    writer_state.elapsed += time.delta_secs();
    if writer_state.elapsed < 1.0 { return; }
    writer_state.elapsed = 0.0;

    let json = format!(
        r#"{{"id":"input","severity":"ok","next_step":"",
"timestamp_secs":{:.1},"keys_pressed_per_sec":{},"keys_released_per_sec":{},
"actions_just_pressed_per_sec":{}}}"#,
        time.elapsed_secs(), accum.keys_pressed, accum.keys_released, accum.actions_just_pressed
    );
    let _ = std::fs::write("forgia2_input.json", &json);
    *accum = InputSensorAccum::default();
}

#[derive(Default)]
pub struct InputWriterState { elapsed: f32 }
```

### 2.5 `sensor_health_sensor.rs` (~80 LOC)

```rust
// Meta-sensor : lit timestamps des 13 forgia2_*.json attendus et expose CHK-5 stale.
const EXPECTED_SENSORS: &[&str] = &[
    "forgia2_health.json", "forgia2_rpg_health.json",
    "forgia2_arena.json", "forgia2_combat.json",
    "forgia2_perf.json", "forgia2_entities.json", "forgia2_memory.json",
    "forgia2_lifecycle.json", "forgia2_watchdog.json",
    "forgia2_audio.json", "forgia2_input.json", "forgia2_sensor_health.json",
];
const STALE_THRESHOLD_SECS: u64 = 10;

pub fn sys_write_sensor_health(time: Res<Time>, mut accum: Local<f32>) {
    *accum += time.delta_secs();
    if *accum < 1.0 { return; }
    *accum = 0.0;

    let now = std::time::SystemTime::now();
    let mut stale = Vec::new();
    let mut missing = Vec::new();
    let mut present = 0u32;

    for path in EXPECTED_SENSORS {
        match std::fs::metadata(path).and_then(|m| m.modified()) {
            Ok(mtime) => {
                let age = now.duration_since(mtime).map(|d| d.as_secs()).unwrap_or(0);
                if age > STALE_THRESHOLD_SECS { stale.push(*path); }
                present += 1;
            }
            Err(_) => missing.push(*path),
        }
    }

    let (severity, next_step) = severity_for_sensor_health(missing.len(), stale.len());
    let json = format!(
        r#"{{"id":"sensor_health","severity":"{severity}","next_step":"{next_step}",
"timestamp_secs":{:.1},"present":{},"missing":{},"stale":{},"missing_paths":{:?}}}"#,
        time.elapsed_secs(), present, missing.len(), stale.len(), missing
    );
    let _ = std::fs::write("forgia2_sensor_health.json", &json);
}

pub fn severity_for_sensor_health(missing: usize, stale: usize) -> (&'static str, &'static str) {
    if missing >= 3 {
        ("critical", "≥3 sensors missing — observability degraded (check plugin wiring)")
    } else if missing > 0 || stale > 0 {
        ("warn", "1-2 sensors missing/stale — producer may not be tick'd")
    } else {
        ("ok", "")
    }
}
```

### 2.6 Wiring `lib.rs`

```rust
pub mod lifecycle_sensor;
pub mod watchdog_sensor;
pub mod audio_sensor;
pub mod input_sensor;
pub mod sensor_health_sensor;

// ForgiaObservabilityPlugin::build() :
app.init_resource::<LifecycleCounter>()
   .init_resource::<GameTickCounter>();

app.add_observer(/* 5 observers lifecycle */);

app.add_systems(First, watchdog_sensor::sys_update_tick_counter);

app.add_systems(Update, (
    lifecycle_sensor::sys_write_lifecycle_sensor,
    watchdog_sensor::sys_write_watchdog_sensor,
    audio_sensor::sys_write_audio_sensor,
    input_sensor::sys_track_input_accum,
    sensor_health_sensor::sys_write_sensor_health,
).in_set(GameSet::Sensors));
```

### 2.7 Cargo deps ajoutées

```toml
# forgia-observability/Cargo.toml
bevy_kira_audio = { workspace = true }   # AudioInstance + PlaybackState
forgia-audio-biome = { workspace = true } # BiomeAmbientState
forgia-input = { workspace = true }       # PlayerAction
leafwing-input-manager = { workspace = true }  # ActionState
forgia-mode-fps-arena = { workspace = true }   # TargetCube
```

**Vérification cycles** : pre-check `grep -l forgia-observability` dans Cargo.toml de chaque crate added. Si cycle détecté → fallback File-based.

### 2.8 xtask extension

Étendre `CANONICAL_SENSORS` : 7 → 12 (5 ajoutés). Cible 13 réservée future (`forgia2_chunks.json` reste optionnel — agg streaming pas critique).

### 2.9 `default_expected_sensors` (config.rs:53)

Étendre la liste — 5 nouveaux pour CHK-5 ne flood pas.

---

## 3. Tests headless requis (~15 tests)

Helpers purs extraits + tests :
- `severity_for_lifecycle`: 2 tests (0 vs >0 removed)
- `severity_for_watchdog`: 3 tests (≤10, 10-30, >30)
- `severity_for_audio`: 2 tests (≤64, >64)
- `severity_for_sensor_health`: 3 tests (ok, warn 1-2 missing, critical ≥3)
- `tick_counter_increments`: 1 test (10 ticks 16ms → counter 10, 0 lag)
- `tick_counter_lag_frames`: 1 test (10 ticks alterned 30/60ms → 5 lag frames)
- `input_accum_default`: 1 test
- `lifecycle_counter_default`: 1 test
- `sensor_health_missing_list_critical`: 1 test (3 paths nonexistent → critical)
- `sensor_health_all_present_ok`: 1 test (touch tempfiles → ok)

Total ~15 tests purs.

---

## 4. Pièges anticipés (research AAA)

1. **Bevy 0.18 syntax `On<Add, C>`** — PAS `Trigger<OnAdd, C>` (rename PR #19596). Imports : `bevy::prelude::{On, Add, Remove, Insert}`.
2. **OnRemove bundle issue #18720** : peut compter spurious. Fallback documenté si trigger. Mitigation = ajouter filter check ou commenter le hook problématique.
3. **EventReader purge 2 frames** : `sys_track_input_accum` DOIT tourner chaque frame, pas 1Hz, sinon events perdus.
4. **Observer ne reçoit pas Query** : ne pas tenter d'accéder à des entités depuis l'observer — uniquement Resource/Commands/EventWriter.
5. **`First` schedule pour tick counter** : avant tous GameSets pour capturer vrai `delta_secs`.
6. **`BiomeAmbientState.current` privé** : ajouter pub accessor ou fallback `"unknown"`. Décider avant impl.
7. **Cross-crate deps cycles** : check avant Edit Cargo.toml. `forgia-input` et `forgia-audio-biome` ne dépendent pas de `forgia-observability` (à vérifier).
8. **`PlayerAction::variants()` ou `ActionState::get_just_pressed()`** : utiliser `get_just_pressed()` qui retourne `&[A]` — pas besoin de `Actionlike::variants()`.
9. **Watchdog "frozen" complet** : OUT OF SCOPE Session C. Documenté comme tâche future (OS thread requis).
10. **`sysinfo` Session B** : déjà ajoutée, pas re-ajouter.

---

## 5. Acceptance

- [ ] 5 fichiers `forgia2_{lifecycle,watchdog,audio,input,sensor_health}.json` écrits 1Hz format conforme
- [ ] `cargo run -p xtask -- verify-sensors-format` → OK 12/12
- [ ] `default_expected_sensors` updated (5 nouveaux) — CHK-5 ne flood pas
- [ ] ~15 tests headless purs verts (severity_for_* + tick counter increments + accum defaults)
- [ ] `cargo clippy --workspace --no-deps -- -D warnings` clean
- [ ] Smoke test runtime 30s RPG + Arena : 5 sensors présents + severity ok
- [ ] ROADMAP V5 Session C mark DONE, V5 = 12/13 (final)
- [ ] Commit message : `feat(observability): Vague 5 Session C — sensors lifecycle+watchdog+audio+input+sensor_health (12/13 canonical)`

---

## 6. Décisions à valider AVANT code

| # | Question | Recommandation |
|---|---|---|
| 1 | Watchdog "frozen" OS thread incluse Session C ? | **Non** — out-of-scope, tick timestamp suffit pour MVP (Antoine peut le détecter en relisant le JSON) |
| 2 | `BiomeAmbientState.current` accessor pub ajouté à `forgia-audio-biome` ? | **Oui** — simple `pub fn current_biome(&self)`, 0 risque, 1-shot |
| 3 | OnRemove bundle piège (#18720) : si compile fail ou warn spurious → ? | **Fallback** = commenter l'observer problématique + note dans commit |
| 4 | Cible 12 ou 13 canonical (forgia2_chunks.json) ? | **12** — chunks_snapshot legacy reste valid, file-based aggregator chunks reportable à V5 Session D si besoin |

---

## 7. Liens

- Plan parent : `docs/audit/vague-5-sensors-fusion-plan-2026-05-19.md` §2 Tier 2
- Session B : commit `7715fd3bb` (story-467)
- Pattern producer : `crates/forgia-observability/src/health_sensor.rs`
- Pattern lifecycle Observer : research bevy-specialist 2026-05-19 (`On<Add, C>` syntax)
- CHK-5 existant : `crates/forgia-observability/src/checks.rs:269`
- ARCHITECTURE.md §9 cible 13 sensors
