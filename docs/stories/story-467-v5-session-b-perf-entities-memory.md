# Story-467 — V5 Session B : sensors perf + entities + memory

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_entities.json`, fichier `config.rs`, symbole `FrameTimeDiagnosticsPlugin`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED
> **État d'origine (périmé, cf bandeau)** : PLAN — en attente validation.
**BMAD scale** : Enterprise (3 sensors + tests + xtask gate, ~3-4 h).
**Vague** : V5 — Phase 5b Session B.
**Prérequis** : Session A DONE (commits `380aa2f10` + `67c20855f`), 4/13 sensors canoniques validés.

---

## 0. TL;DR

3 nouveaux producteurs sensors directs (pas aggregator file-based) écrivant à 1 Hz :
- `forgia2_perf.json` ← `FrameTimeDiagnosticsPlugin` (avg/min/max ms + FPS smoothed)
- `forgia2_entities.json` ← `EntityCountDiagnosticsPlugin` total + Query counts par marker
- `forgia2_memory.json` ← `sysinfo` crate (RAM process), VRAM = stub honnête "N/A"

Étend `xtask verify-sensors-format` : 4 → 7 canonical sensors validés.

Effort réel estimé après research : **3-4 h** (vs 6 h plan initial — research a écarté pièges).

---

## 1. Concept-first 5 étapes

### Étape 0 — Data ou code ?

**Code** (framework Rust). Pas de genome TOML — ces sensors lisent l'état Bevy runtime (Diagnostics, World), aucune valeur à exposer côté définition.

### Étape 1 — Hypothèses concurrentes

- **H1 (retenu)** : 3 systems indépendants dans `forgia-observability`, chacun écrit son JSON 1Hz. Pattern miroir de `health_sensor.rs:14` (system + `Local<f32>` accumulator + `std::fs::write`).
- **H2 (rejeté)** : 1 mega-system qui écrit les 3. Couplage inutile, tests fragiles.
- **H3 (rejeté)** : crate dédié `forgia-sensors-perf`. Le scaffold `forgia-sensors` existe (lib.rs 21 LOC) mais reste inactif — créer 3 sub-crates pour 3 systems = over-engineering. Reste dans `forgia-observability`.

### Étape 2 — Cartographier

- **Producteur unique** par sensor : `crates/forgia-observability/src/{perf_sensor,entities_sensor,memory_sensor}.rs`
- **Plugin** : étendre `ForgiaObservabilityPlugin::build()` (`lib.rs:49`) avec 3 nouveaux systems
- **xtask** : étendre liste `CANONICAL_SENSORS` (`xtask/src/verify_sensors.rs`)
- Sensor liveness (CHK-5) : ajouter 3 entrées à `default_expected_sensors` (`config.rs:53`)

### Étape 3 — Verbalisation producteur / consommateur

| Sensor | Producteur (timing) | Consommateurs | Hot | Net | Script |
|---|---|---|---|---|---|
| `forgia2_perf.json` | `sys_write_perf_sensor` (Update, gate 1Hz Local) | xtask gate + Antoine debug | non | L | int |
| `forgia2_entities.json` | `sys_write_entities_sensor` (Update, gate 1Hz Local) | xtask gate + Antoine debug | non | L | int |
| `forgia2_memory.json` | `sys_write_memory_sensor` (Update, gate 1Hz Local + sysinfo refresh 5s) | xtask gate + Antoine debug | non | L | int |

### Étape 4 — Hot path check

Aucun système tagué hot. Tous à 1 Hz avec `Local<f32>` throttle. Coût marginal :
- Perf : `DiagnosticsStore::get()` + fold ~120 samples (O(N) sur 120 = <10 µs).
- Entities : `Query<Entity>::iter().count()` × 5 markers ~50-200 µs sur 10k entities. ACCEPTABLE 1Hz.
- Memory : `sysinfo` refresh ~2ms cooldowné 5s. Pas par-frame.

### Étape 5 — Scale-up BMAD

Implémentations multiples (3 sensors + plugin wiring + xtask + config + tests) = Enterprise. Story OBLIGATOIRE. Checklist post-impl OBLIGATOIRE.

---

## 2. Détail implémentation

### 2.1 `perf_sensor.rs` (~70 LOC)

```rust
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};

pub fn sys_write_perf_sensor(
    time: Res<Time>,
    diagnostics: Res<DiagnosticsStore>,
    mut accum: Local<f32>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 { return; }
    *accum = 0.0;

    let (avg_ms, fps_smoothed, min_ms, max_ms, samples) = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .map(|ft| {
            let avg = ft.average().unwrap_or(0.0);
            let (mn, mx, n) = ft.values().fold(
                (f64::MAX, 0.0_f64, 0usize),
                |(mn, mx, n), v| (mn.min(*v), mx.max(*v), n + 1),
            );
            let fps = diagnostics
                .get(&FrameTimeDiagnosticsPlugin::FPS)
                .and_then(|d| d.smoothed())
                .unwrap_or(0.0);
            (avg, fps, if n > 0 { mn } else { 0.0 }, mx, n)
        })
        .unwrap_or((0.0, 0.0, 0.0, 0.0, 0));

    // severity heuristic : >25ms avg = warn, >50ms = critical (40fps / 20fps thresholds)
    let (severity, next_step) = if avg_ms > 50.0 {
        ("critical", "frame_time avg > 50ms — investigate hot systems (Tracy, forgia2_entities)")
    } else if avg_ms > 25.0 {
        ("warn", "frame_time avg > 25ms — perf budget approaching")
    } else {
        ("ok", "")
    };

    let json = format!(
        r#"{{"id":"perf","severity":"{severity}","next_step":"{next_step}",
"timestamp_secs":{:.1},"frame_time_avg_ms":{:.3},"frame_time_min_ms":{:.3},
"frame_time_max_ms":{:.3},"fps_smoothed":{:.1},"samples":{}}}"#,
        time.elapsed_secs(), avg_ms, min_ms, max_ms, fps_smoothed, samples
    );
    let _ = std::fs::write("forgia2_perf.json", json);
}
```

**Wiring plugin** : `app.add_plugins(FrameTimeDiagnosticsPlugin::default())` (à vérifier déjà présent dans `forgia-game` boot). Si absent, l'ajouter dans `ForgiaObservabilityPlugin::build()` (idempotent).

### 2.2 `entities_sensor.rs` (~80 LOC)

Choix : utiliser `EntityCountDiagnosticsPlugin` pour le total + `Query<Entity, With<Marker>>` pour les markers KNOWN. Markers via deps existantes :

- `Player` ← `forgia_player::Player` (dep nouvelle pour forgia-observability)
- `ArenaBot` ← `forgia_mode_fps_arena::ArenaBot` (dep nouvelle)
- `NameplateRoot` ← `forgia_enemy_nameplate::NameplateRoot` (dep nouvelle)
- ChunkMesh ← skip (utiliser `forgia_terrain::ChunkLoaded` si dispo, déjà dep)

**Verdict deps** : 3 nouvelles deps à ajouter à `forgia-observability/Cargo.toml`. Risque cycle ? Check : aucune de ces 3 crates ne dépend de `forgia-observability` (vérifier `grep -l "forgia-observability"` dans leurs Cargo.toml). Si cycle détecté → fallback file-based read (les markers exposent leur count via leur propre sensor existant).

```rust
pub fn sys_write_entities_sensor(
    time: Res<Time>,
    diagnostics: Res<DiagnosticsStore>,
    mut accum: Local<f32>,
    q_player: Query<Entity, With<Player>>,
    q_bots: Query<Entity, With<ArenaBot>>,
    q_nameplates: Query<Entity, With<NameplateRoot>>,
    q_chunks: Query<Entity, With<ChunkLoaded>>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 { return; }
    *accum = 0.0;

    let total = diagnostics.get(&EntityCountDiagnosticsPlugin::ENTITY_COUNT)
        .and_then(|d| d.value()).unwrap_or(0.0) as u64;
    let players = q_player.iter().count() as u64;
    let bots = q_bots.iter().count() as u64;
    let nameplates = q_nameplates.iter().count() as u64;
    let chunks = q_chunks.iter().count() as u64;

    // severity : > 50k entities = warn (proxy budget), > 100k = critical
    let (severity, next_step) = if total > 100_000 {
        ("critical", "entity total > 100k — check leaks (despawn missing, see forgia_health)")
    } else if total > 50_000 {
        ("warn", "entity total > 50k — approach budget, audit recent spawns")
    } else {
        ("ok", "")
    };

    let json = format!(
        r#"{{"id":"entities","severity":"{severity}","next_step":"{next_step}",
"timestamp_secs":{:.1},"total":{total},"players":{players},"arena_bots":{bots},
"nameplates":{nameplates},"chunks_loaded":{chunks}}}"#,
        time.elapsed_secs()
    );
    let _ = std::fs::write("forgia2_entities.json", json);
}
```

### 2.3 `memory_sensor.rs` (~80 LOC)

```rust
use sysinfo::{System, Pid};

#[derive(Default)]
struct MemSensorState {
    system: Option<System>,
    last_refresh: f32,
    cached_ram_bytes: u64,
}

pub fn sys_write_memory_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    mut state: Local<MemSensorState>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 { return; }
    *accum = 0.0;

    let now = time.elapsed_secs();
    if now - state.last_refresh > 5.0 {
        let sys = state.system.get_or_insert_with(System::new);
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        let pid = sysinfo::get_current_pid().ok();
        state.cached_ram_bytes = pid
            .and_then(|p| sys.process(p))
            .map(|p| p.memory()).unwrap_or(0);
        state.last_refresh = now;
    }

    let ram_mb = state.cached_ram_bytes as f64 / 1024.0 / 1024.0;
    // severity : > 4GB warn, > 8GB critical (Windows builds)
    let (severity, next_step) = if ram_mb > 8192.0 {
        ("critical", "RAM > 8GB — investigate leaks (forgia2_entities, sensor_health)")
    } else if ram_mb > 4096.0 {
        ("warn", "RAM > 4GB — approach budget")
    } else {
        ("ok", "")
    };

    let json = format!(
        r#"{{"id":"memory","severity":"{severity}","next_step":"{next_step}",
"timestamp_secs":{:.1},"ram_bytes":{},"ram_mb":{:.1},"vram_status":"N/A (wgpu adapter telemetry custom needed)"}}"#,
        time.elapsed_secs(), state.cached_ram_bytes, ram_mb
    );
    let _ = std::fs::write("forgia2_memory.json", json);
}
```

**Dep Cargo.toml** : `sysinfo = "0.32"` (default-features minimal).

### 2.4 Wiring `lib.rs`

```rust
pub mod perf_sensor;
pub mod entities_sensor;
pub mod memory_sensor;

// dans ForgiaObservabilityPlugin::build() :
app.add_systems(
    Update,
    (
        perf_sensor::sys_write_perf_sensor,
        entities_sensor::sys_write_entities_sensor,
        memory_sensor::sys_write_memory_sensor,
    ).in_set(GameSet::Sensors),
);
```

Aucun `run_if(in_state)` : ces sensors doivent tourner cross-mode (FPS + RPG + Menu).

### 2.5 `xtask verify_sensors.rs` extension

Étendre `CANONICAL_SENSORS` : 4 → 7 (`forgia2_perf.json`, `forgia2_entities.json`, `forgia2_memory.json` ajoutés).

### 2.6 `config.rs:53` `default_expected_sensors`

Ajouter `"forgia2_perf.json"`, `"forgia2_entities.json"`, `"forgia2_memory.json"` à la liste pour CHK-5 sensor liveness.

---

## 3. Tests headless requis

3 tests purs (pas d'App Bevy) suffisent pour valider :
- `perf_severity_thresholds` : avg_ms 10 → ok ; 30 → warn ; 60 → critical
- `entities_severity_thresholds` : 1000 → ok ; 60000 → warn ; 200000 → critical
- `memory_severity_thresholds` : 1GB → ok ; 5GB → warn ; 10GB → critical

Extraction des fonctions `severity_for_perf(avg_ms)`, `severity_for_entities(total)`, `severity_for_memory(ram_mb)` en helpers purs.

---

## 4. Pièges anticipés (recherche AAA)

1. **`FrameTimeDiagnosticsPlugin` plugin ajouté ?** Vérifier `forgia-game/src/lib.rs`. Si absent, ajouter `app.add_plugins(FrameTimeDiagnosticsPlugin::default())` dans `ForgiaObservabilityPlugin::build()` (idempotent — Bevy 0.18 ignore double-add).
2. **`EntityCountDiagnosticsPlugin` non default** : doit être explicitement ajouté.
3. **Cycle deps `forgia-observability` → player/arena/nameplate** : check grep avant Edit Cargo.toml. Si cycle → fallback file-based (lit sensors existants).
4. **`sysinfo` first refresh** : refresh BLOQUANT ~2ms première fois. Acceptable pour Local cooldown 5s.
5. **`PostUpdate` vs `Update`** : `FrameTimeDiagnosticsPlugin` met à jour en `Last`. Lire en `Update` du frame suivant = OK (1Hz tolère la latence frame).
6. **Tests headless `DiagnosticsStore`** : pas dispo sans App. D'où tests sur helpers purs `severity_for_*`.

---

## 5. Acceptance

- [ ] 3 fichiers `forgia2_perf.json` / `forgia2_entities.json` / `forgia2_memory.json` écrits 1Hz format `{id, severity, next_step, ...}`
- [ ] `cargo run -p xtask -- verify-sensors-format` → OK 7/7
- [ ] `default_expected_sensors` updated (3 nouveaux) — CHK-5 ne flood pas
- [ ] 3 tests headless purs verts (severity thresholds)
- [ ] `cargo clippy --workspace --no-deps -- -D warnings` clean
- [ ] Smoke test runtime 2 min RPG + 2 min Arena : 3 sensors présents + severity `ok`
- [ ] ROADMAP V5 Session B mark DONE, ROADMAP_CURRENT next-session-options updated
- [ ] Commit message : `feat(observability): Vague 5 Session B — sensors perf+entities+memory (7/13 canonical)`

---

## 6. Liens

- Plan parent : `docs/audit/vague-5-sensors-fusion-plan-2026-05-19.md` §2 Tier 2
- Session A : commits `380aa2f10` + `67c20855f`
- Pattern producer : `crates/forgia-observability/src/health_sensor.rs`
- Pattern config : `crates/forgia-observability/src/config.rs:53`
- ARCHITECTURE.md §9 cible 13 sensors
