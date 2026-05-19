# Vague 5 Phase 5a — Plan fusion sensors 29 → 12

> **Date** : 2026-05-19
> **Cible** : `C:/Users/Antoi/Desktop/Forgia Rewrite/`
> **Méthode** : agent Explore cartographie READ-ONLY 31 producteurs `fs::write("forgia_*.json")`
> **Scope** : Phase 5a (plan seul). Phases 5b/5c reportées à session dédiée.

---

## 0. Résumé exécutif

L'agent Explore a cartographié **31 producteurs sensors** (29 demandés + 2 health variants découverts : `forgia_anim_layer_health.json`, `forgia_pack_registry_health.json`). Verdict honnête révisé :

- **2 sensors déjà canoniques** (`forgia_health.json`, `forgia_rpg_health.json`) — format `{id, severity, next_step}` respecté
- **7 sensors gameplay** fusionnables en 3 aggregators (`forgia2_arena.json`, `forgia2_combat.json`, `forgia2_chunks.json`)
- **17 sensors debug utility** à **garder séparés** (animation, asset, prefab, terrain LOD, etc.) — ROI fusion négatif
- **5 nouveaux aggregators à créer** (`perf`, `lifecycle`, `entities`, `memory`, `watchdog`, `audio`, `input`)

**Effort réaliste révisé** : Phase 5b = **8-12h** (vs 6-8h estimés initialement). Risque MOYEN sur file-based aggregation, ÉLEVÉ sur memory introspection (API Bevy 0.18 à valider).

**Décision pragmatique** : Phase 5b ne doit PAS attaquer les 12 cibles d'un coup. Séquence recommandée crate-par-crate avec smoke test entre chaque.

---

## 1. Inventaire 31 producteurs

### 1.1 Producteurs déjà conformes (3)

| Sensor | Crate | File:line | Format |
|---|---|---|---|
| `forgia_combat.json` | forgia-combat | sensor.rs:107 | `{id, severity, next_step, timestamp_secs, player_hp, max_hp, active_weapon, shots, hits}` ✅ |
| `forgia_health.json` | forgia-observability | health_sensor.rs:58 | `{id, severity, next_step, timestamp_secs, overall_severity_source, checks_count}` ✅ |
| `forgia_rpg_health.json` | forgia-observability | exporter.rs:13 | Aggregator 6 CHK + `last_severity`/`last_message`/`last_next_step` ✅ |

### 1.2 Producteurs gameplay fusionnables Tier 1 (7)

| Sensor | Crate | File:line | Fréq | Resource source |
|---|---|---|---|---|
| `forgia_arena_feedback.json` | forgia-effects | arena_feedback.rs:85 | 1Hz | SoundEffectsStats |
| `forgia_arena_waves.json` | forgia-mode-fps-arena | wave.rs:463 | 1Hz | WaveState |
| `forgia_hitscan.json` | forgia-fps | hitscan_sensor.rs:196 | 1Hz | HitscanSensorState (recent[] array) |
| `forgia_hud_ammo.json` | forgia-ui-hud-ammo | sensor.rs:65 | 1Hz | EquippedWeapons, AmmoState |
| `forgia_killfeed.json` | forgia-killfeed | lib.rs:407 | 1Hz | KillfeedStreak, StreakDisplay |
| `forgia_screen_flash.json` | forgia-juice-screen-flash | lib.rs:294 | 1Hz | ScreenFlashState |
| `forgia_damage_dir.json` | forgia-ui-damage-direction | lib.rs:280 | 1Hz | DamageArcsState |

### 1.3 Producteurs streaming/terrain Tier 1bis (3)

| Sensor | Crate | File:line | Fréq |
|---|---|---|---|
| `forgia_chunk_stream.json` + `_health.json` | forgia-streaming | lib.rs:473 / 481 | 1Hz |
| `forgia_chunks_snapshot.json` | forgia-rpg | lib.rs:951 | 1Hz |
| `forgia_vegetation.json` | forgia-foliage | lib.rs:455 | 1Hz |

### 1.4 Debug utilities — **gardés séparés** (17)

Aucune valeur "gameplay liveness probe" → fusion ROI négatif :

`forgia_anim_layer.json` (+health) · `forgia_auto_rig.json` (+health) · `forgia_bone_trace.json` (+health) · `forgia_bot_ai.json` · `forgia_asset_registry.json` · `forgia_pack_registry.json` (+health) · `forgia_mesh_fader.json` · `forgia_pause_menu.json` · `forgia_prefab.json` · `forgia_terrain_lod.json` · `forgia_viewmodel_calibration.json` · `forgia_village.json` · `forgia_village_debug.json` · `forgia_walk_pose.json` · `forgia_enemy_nameplate.json` (nouveau story-456 WIP)

---

## 2. Mapping 12 cibles canoniques

### Tier 0 — Déjà canoniques (3 = aucune migration)

- `forgia2_health.json` ← `forgia_health.json` (rename only)
- `forgia2_rpg_health.json` ← `forgia_rpg_health.json` (rename only) — détail RPG aggregator
- `forgia2_sensor_health.json` ← nouveau meta-sensor (CHK-5 stale detection canonisé)

### Tier 1 — Fusion gameplay (3 aggregators)

| Cible | Sources fusionnées | Crate dest | Pattern |
|---|---|---|---|
| `forgia2_arena.json` | `arena_feedback` + `arena_waves` | forgia-observability | File-based read 2 fichiers → merge JSON `{id, severity, next_step, sources: {arena_feedback: {...}, arena_waves: {...}}}` |
| `forgia2_combat.json` | `hitscan` + `hud_ammo` + `screen_flash` + `damage_dir` + `killfeed` | forgia-observability | Idem, 5 sources |
| `forgia2_chunks.json` | `chunk_stream` + `chunks_snapshot` + `vegetation` | forgia-observability | Idem, 3 sources |

### Tier 2 — Nouveaux aggregators (6 à créer)

| Cible | Source data | Crate dest | Pré-requis |
|---|---|---|---|
| `forgia2_perf.json` | `bevy::diagnostics::FrameTimeDiagnosticsPlugin` | forgia-observability | Valider Bevy 0.18 Diagnostics API |
| `forgia2_lifecycle.json` | Event listener `OnAdd<Player>`, `OnRemove<TargetCube>`, etc. | forgia-observability | Bevy 0.18 Observer hooks |
| `forgia2_entities.json` | Query count + archetypes | forgia-observability | Perf test sur N=10k entities |
| `forgia2_memory.json` | `MemoryBreakdown` Resource (à valider présence) | forgia-observability | Bevy alloc hooks ? |
| `forgia2_watchdog.json` | Tick heartbeat + lag detection | forgia-observability | Resource `GameTickCounter` à créer |
| `forgia2_audio.json` + `forgia2_input.json` | Biome music + footsteps events / keyboard input + bindings | forgia-observability | Event readers wiring |

---

## 3. Plan migration ordonné par risque (Phase 5b future)

| Ordre | Cible | Sources | Risque | Effort | Bloque |
|---|---|---|---|---|---|
| 1 | `forgia2_arena.json` | 2 | 🟢 BAS | 1h | — |
| 2 | `forgia2_health.json` (rename) | 1 | 🟢 BAS | 0.5h | — |
| 3 | `forgia2_rpg_health.json` (rename) | 1 | 🟢 BAS | 0.5h | — |
| 4 | `forgia2_chunks.json` | 3 | 🟠 MOYEN | 1.5h | streaming stable |
| 5 | `forgia2_combat.json` | 5 | 🟠 MOYEN | 2.5h | — |
| 6 | `forgia2_entities.json` | nouveau Query | 🟠 MOYEN | 1.5h | — |
| 7 | `forgia2_memory.json` | MemoryBreakdown | 🟠 MOYEN | 1h | Resource exist ? |
| 8 | `forgia2_perf.json` | Bevy Diagnostics | 🔴 ÉLEVÉ | 2h | API recherche |
| 9 | `forgia2_lifecycle.json` | Event listeners | 🔴 ÉLEVÉ | 2.5h | entité hooks |
| 10 | `forgia2_watchdog.json` | Heartbeat tick | 🔴 ÉLEVÉ | 3h | timing-sensitive |
| 11 | `forgia2_audio.json` | Audio events | 🟠 MOYEN | 1.5h | — |
| 12 | `forgia2_input.json` | Input events | 🟠 MOYEN | 1h | — |

**Total estimé** : 18-22h effort réaliste (vs 6-8h optimiste initial). À découper en 2-3 sessions Enterprise.

---

## 4. Pièges identifiés

### Piège 1 — Cascade API breakage (StreamingStats, etc.)

`forgia2_chunks.json` aggregator doit lire `StreamingStats` (interne `forgia-streaming`). Si non `pub`, doit soit (a) être exposé (breaking change), soit (b) aggregator lit le fichier JSON déjà écrit. **Recommandé : pattern file-based** (lecture JSON) plutôt que sharing Resource via crate dependency — évite cascade visibility changes.

### Piège 2 — `default_expected_sensors` config desync

`crates/forgia-observability/src/config.rs:53` liste hardcodée :

```rust
fn default_expected_sensors() -> Vec<String> {
    vec![
        "forgia_terrain_lod.json",
        "forgia_chunks_snapshot.json",
        "forgia_anim_layer.json",
        "forgia_combat.json",
        "forgia_health.json",
    ]
}
```

Si on rename vers `forgia2_*.json` sans updater cette liste → CHK-5 flood retour. **Mitigation** : updater cette liste **dans le même commit** que chaque migration sensor.

### Piège 3 — Dual-write `_health.json` variants

`forgia-streaming` + `forgia-anim-debug` + `forgia-auto-rig` + `forgia-asset-registry` écrivent un `*_health.json` **conditionnel** (seulement si severity != ok). Si l'aggregator lit ces files, race possible : producer delete → aggregator read stale.

**Mitigation** : aggregators ignorent les `*_health.json` variants. Le sensor principal `*.json` suffit pour status global, le variant est side-file debug-only.

### Piège 4 — Timing race aggregator vs producer 1Hz

Si aggregator tick à T=0.0s et producer à T=0.5s, aggregator lit donnée 500ms vieille. **Mitigation** : tolérance `stale_threshold >= 10s` (au-dessus 1Hz throttle). Pas de vrai bug — juste accepter 0-1s latency.

### Piège 5 — Entity count Query perf

`forgia2_entities.json` itère toutes entities + archetypes 1Hz. Sur RPG world 10k+ entities = O(n) par tick.

**Mitigation** : cache via Resource updaté 1Hz, pas par-frame. Bench sur stress-test 500 bots arena.

### Piège 6 — `MemoryBreakdown` Resource existence

L'agent n'a pas pu confirmer que `MemoryBreakdown` Resource existe. Vérifier avant Phase 5b.7 : grep `pub struct MemoryBreakdown` workspace. Si absent → créer ou skip `forgia2_memory.json` initial.

### Piège 7 — Tests régression absents

Aucun test headless n'existe sur la chaîne sensors actuelle. Migration sans filet. **Mitigation** : créer `xtask verify-sensors-format` (Phase 5c) avant Phase 5b pour CI gate. Sinon régression silencieuse.

---

## 5. Design `xtask verify-sensors-format` (Phase 5c)

**But** : CI gate validation 12 forgia2_*.json présents + conformes format à chaque build.

**Implémentation** (`xtask/src/verify_sensors.rs`) :

```rust
const CANONICAL_SENSORS: &[&str] = &[
    "forgia2_health.json", "forgia2_rpg_health.json", "forgia2_sensor_health.json",
    "forgia2_arena.json", "forgia2_combat.json", "forgia2_chunks.json",
    "forgia2_perf.json", "forgia2_lifecycle.json", "forgia2_entities.json",
    "forgia2_memory.json", "forgia2_watchdog.json", "forgia2_audio.json",
    // forgia2_input.json — total = 13, à confirmer
];

pub fn verify_sensors_format(workspace_root: &Path) -> Result<(), String> {
    for sensor in CANONICAL_SENSORS {
        let content = fs::read_to_string(workspace_root.join(sensor))?;
        let val: serde_json::Value = serde_json::from_str(&content)?;
        let obj = val.as_object().ok_or("not a JSON object")?;
        for required in ["id", "severity", "next_step"] {
            if !obj.contains_key(required) {
                return Err(format!("{}: missing '{}'", sensor, required));
            }
        }
        if let Some(sev) = obj.get("severity").and_then(|v| v.as_str()) {
            if !["ok", "warn", "critical", "info"].contains(&sev) {
                return Err(format!("{}: invalid severity '{}'", sensor, sev));
            }
        }
    }
    Ok(())
}
```

CI hook GitHub Actions : `cargo xtask verify-sensors-format` exit non-zéro si fail.

---

## 6. Recommandations finales

### 6.1 Cible révisée : **13 sensors** canoniques, pas 12

L'audit montre que séparer `forgia2_health.json` (cross-mode aggregator) et `forgia2_rpg_health.json` (détail 6 CHK RPG-only) est valuable. La cible ARCHITECTURE.md "12 sensors max" devrait être **13** ou bien fusionner `health` + `rpg_health` (perte de granularité).

### 6.2 Faisabilité Phase 5b = 2-3 sessions, pas 1

Effort estimé révisé : **18-22h** vs 6-8h initial. À planifier en 2 ou 3 sessions Enterprise dédiées :

- **Session A (~6h)** : Tier 0 renames + Tier 1 fusion (arena, chunks, combat) + xtask verify
- **Session B (~6h)** : Tier 2 aggregators perf/entities/memory + recherche API Bevy 0.18 (Diagnostics, OnAdd hooks)
- **Session C (~6h)** : Tier 2 lifecycle/watchdog/audio/input + tests régression + doc finale

### 6.3 Dépendances critiques à valider AVANT Phase 5b

| Item | Action |
|---|---|
| `MemoryBreakdown` Resource | Grep workspace, fallback si absent |
| `bevy::diagnostics::FrameTimeDiagnosticsPlugin` registered | Vérifier `forgia-game/src/lib.rs` plugin setup |
| Observer `OnAdd<Player>` syntax | Confirmer via context7 + bevy-cheatbook |
| `forgia-streaming::StreamingStats` visibility | File-based aggregator pour éviter cascade |

### 6.4 Garder 17 debug utilities séparés (CONFIRMÉ)

Ne pas fusionner `forgia_anim_layer.json`, `forgia_auto_rig.json`, etc. Ces sensors documentent des opérations internes (animation, asset, terrain procgen) — orthogonal à la liveness check gameplay. Fusion = +5h complexité + risque régression systèmes animation/asset. ROI **négatif**.

Ces 17 restent `forgia_*.json` legacy, sans CI gate format strict. Liberté tooling externe pour les consommer.

---

## 7. Plan d'attaque concret Phase 5b (3 sessions dédiées)

### Session A — Tier 0+1 + xtask gate (~6h, faible-moyen risque)

1. Rename `forgia_health.json` → `forgia2_health.json` + update `default_expected_sensors`
2. Rename `forgia_rpg_health.json` → `forgia2_rpg_health.json`
3. Create file-based aggregator `forgia2_arena.json` (lit 2 fichiers, merge)
4. Create file-based aggregator `forgia2_chunks.json` (lit 3 fichiers)
5. Create file-based aggregator `forgia2_combat.json` (lit 5 fichiers) — **étendre celui existant** créé en Vague 1 plutôt que recréer
6. Create `xtask verify-sensors-format` binary
7. Smoke test runtime + verify 6 forgia2_*.json présents

### Session B — Tier 2 aggregators (~6h, moyen-élevé risque)

8. Research Bevy 0.18 Diagnostics API → create `forgia2_perf.json`
9. Create `forgia2_entities.json` (Query count + cached Resource)
10. Audit `MemoryBreakdown` existence → create or skip `forgia2_memory.json`
11. Smoke test + verify 9 forgia2_*.json présents

### Session C — Lifecycle/Watchdog/Audio/Input + cleanup (~6h, moyen-élevé risque)

12. Create `forgia2_lifecycle.json` (Observer hooks `OnAdd<Player>`, `OnRemove<TargetCube>`)
13. Create `forgia2_watchdog.json` (Resource `GameTickCounter` + lag detection)
14. Create `forgia2_audio.json` (biome events)
15. Create `forgia2_input.json` (keyboard events)
16. Final cleanup `default_expected_sensors` → 13 forgia2_*
17. ARCHITECTURE.md §9 update (cible 13 atteinte, 17 debug séparés documentés)
18. Full smoke test + CI gate passing

---

*Audit produit par Explore agent Phase 5a 2026-05-19. READ-ONLY strict, 0 modification code source. Plan révisé : effort 18-22h réaliste vs 6-8h initial. Cible 13 sensors canoniques (vs 12 initial). 17 debug utilities gardés séparés.*
