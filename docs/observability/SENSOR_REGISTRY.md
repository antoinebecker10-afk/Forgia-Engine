# Sensor Registry — Forgia V2

> **Source de vérité unique** pour les ~70 sensors JSON émis à la racine workspace.
> Toute nouvelle entrée code écrivant `forgia*.json` DOIT apparaître ici (gate `cargo xtask sensor-audit`).
>
> Origine : story-546 (2026-05-28). Motivé par story-545 (player invincible) — diagnostic ralenti par dispersion sensors.

## Conventions

- **Tier T0 (unified)** : `forgia2_*.json` agrégés via `forgia-observability::forgia2_aggregator` ou émis avec schéma normalisé `{id, severity, next_step, timestamp_secs, sources?}`. Canonique. **À privilégier pour nouvelles features.**
- **Tier T1 (legacy)** : `forgia_*.json` écrits per-crate, schéma libre. Migration vers T0 en cours (xtask `verify-sensors-format` couvre 13 canoniques).
- **Tier T2 (satellite)** : `*_health.json` companion files — sentinelle santé d'un sensor primaire (auto-rig, anim-layer, bone-trace, chunk-stream, pack-registry).
- **Tier T3 (snapshot)** : `*_entry_N_bind/live.json` — snapshots indexés (rex_bones).

**Sévérité standard (T0)** : `ok | warn | critical | info` (cf `xtask::VALID_SEVERITIES`).

## Registry

| Filename | Tier | Producer crate | Producer file:line | Frequency | Canonical bugs | Status |
|---|---|---|---|---|---|---|
| `forgia_anim_layer.json` | T1 | forgia-anim-debug | `src/lib.rs:226` | 1Hz | anim layer blending stuck, locomotion freeze | active |
| `forgia_anim_layer_health.json` | T2 | forgia-anim-debug | `src/lib.rs:236` | 1Hz | anim_layer sensor liveness | active |
| `forgia_arena_feedback.json` | T1 | forgia-effects | `src/arena_feedback.rs:84` | event | hit feedback missing | active |
| `forgia_arena_waves.json` | T1 | forgia-mode-fps-arena | `src/wave.rs:471` | 1Hz | wave progression stuck Arena | active |
| `forgia_asset_registry.json` | T1 | forgia-asset-registry | `src/lib.rs:543` | 1Hz | asset registry desync | active |
| `forgia_bone_trace.json` | T1 | forgia-anim-debug | `src/bone_trace.rs:265,345` | event | bone transform NaN, skinning broken | active |
| `forgia_bone_trace_health.json` | T2 | forgia-anim-debug | `src/bone_trace.rs:273,290` | 1Hz | bone_trace sensor liveness | active |
| `forgia_bot_ai.json` | T1 | forgia-ai-arena-bot | `src/tactical.rs:471` | 1Hz | bot LOS, alert, chase, shoot — **story-545 candidat** | active |
| `forgia_chunk_stream.json` | T1 | forgia-streaming | `src/lib.rs:460` | 1Hz | chunk load/unload, streaming budget | active |
| `forgia_chunk_stream_health.json` | T2 | forgia-streaming | `src/lib.rs:461` | 1Hz | chunk_stream sensor liveness | active |
| `forgia_chunks_snapshot.json` | T1 | forgia-rpg | `src/lib.rs:1007` | 1Hz | terrain chunks count, vegetation_total — story-502 | active |
| `forgia_combat.json` | T1 | forgia-combat | `src/sensor.rs:117` | 1Hz | damage events, kill count (per-crate, **agrégé dans forgia2_combat**) | active |
| `forgia_damage_dir.json` | T1 | forgia-ui-lib (damage_direction) | `src/damage_direction/mod.rs:286` | event | direction indicator missing, **story-545** (events_received=0) | active |
| `forgia_enemy_nameplate.json` | T1 | forgia-enemy-nameplate | `src/lib.rs:373` | 1Hz | nameplate LOD culling | active |
| `forgia_foliage_fallback.json` | T1 | forgia-foliage | `src/material_override.rs:342` | event | jolcham bark override, story-486/502 | active |
| `forgia_foot_ik.json` | T1 | forgia-anim-locomotion | `src/foot_ik.rs:246` | 1Hz | foot IK clipping, bones_missing | active |
| `forgia_hitscan.json` | T1 | forgia-fps | `src/hitscan_sensor.rs:196` | event | shots, hits_blocked_by_world, missed_no_hit | active |
| `forgia_hud_ammo.json` | T1 | forgia-ui-lib (hud_ammo) | `src/hud_ammo/sensor.rs:65` | event | mag/reserve, reload progress, low_ammo | active |
| `forgia_killfeed.json` | T1 | forgia-killfeed | `src/lib.rs:424` | event | total_kills_session, streak, banner | active |
| `forgia_mesh_fader.json` | T1 | forgia-effects | `src/mesh_fader.rs:172` | event | fade transitions stuck | active |
| `forgia_pack_registry.json` | T1 | forgia-asset-registry | `src/pack_registry.rs:24` | poll | catalog.hash drift, pack version mismatch | active |
| `forgia_pack_registry_health.json` | T2 | forgia-asset-registry | `src/pack_registry.rs:25` | 1Hz | pack_registry sensor liveness | active |
| `forgia_pause_menu.json` | T1 | forgia-ui-lib (pause_menu) | `src/pause_menu.rs:348` | event | pause toggle, focus_lost | active |
| `forgia_prefab.json` | T1 | forgia-stage **+** forgia-prefab | `forgia-stage/src/lib.rs:1177` **+** `forgia-prefab/src/lib.rs:164` | event | total_spawned (**trompeur**, voir feedback_total_spawned_counter_trompeur) | **duplicate-writer** |
| `forgia_screen_flash.json` | T1 | forgia-juice-screen-flash | `src/lib.rs:296` | event | damage_flashes_session, low_hp_active, **story-545** | active |
| `forgia_skinning_weights.json` | T1 | forgia-auto-rig | `src/skinning.rs:400` | once | bone weights distribution (story-482 audit) | active |
| `forgia_stage_graph.json` | T1 | forgia-stage | `src/graph.rs:23` | event | stage graph traversal, transitions | active |
| `forgia_terrain_lod.json` | T1 | forgia-terrain | `src/lod.rs:690` | 1Hz | LOD0/LOD1 producteurs, story-502 | active |
| `forgia_textures.json` | T1 | forgia-qa-core | `src/source.rs:77,92` | once | PBR material audit (purple shader, missing maps) | active |
| `forgia_vegetation.json` | T1 | forgia-foliage | `src/lib.rs:513` | 1Hz | vegetation placement, density | active |
| `forgia_viewmodel_calibration.json` | T1 | forgia-viewmodel | `src/calibration_sensor.rs:298` | 1Hz | viewmodel offset, FOV calibration | active |
| `forgia_village.json` | T1 | forgia-village-loader | `src/lib.rs:481` | 1Hz | village spawn, prefab loading | active |
| `forgia_village_debug.json` | T1 | forgia-village-loader | `src/lib.rs:846,897` | event | village placement debug, terrain leveling | active |
| `forgia2_anchor.json` | T0 | forgia-anchor | `src/lib.rs:30` | 1Hz | AnchorKind stats, props_spawned counter | active |
| `forgia2_arena.json` | T0 | forgia-observability | `src/forgia2_aggregator.rs:136` | 1Hz | Arena unified (arena_feedback + arena_waves agrégés) | active |
| `forgia2_assets.json` | T0 | forgia-observability | `src/assets_load_sensor.rs:98` | 1Hz | LoadState::Failed silencieux, scene_failed, mesh_failed | active |
| `forgia2_audio.json` | T0 | forgia-observability | `src/audio_sensor.rs:56` | 1Hz | audio channels, music_state, voicelines | active |
| `forgia2_auto_rig.json` | T0 | forgia-auto-rig | `src/lib.rs:187` | event | Pinocchio rig success/fail | active |
| `forgia2_auto_rig_health.json` | T2 | forgia-auto-rig | `src/lib.rs:188` | 1Hz | auto_rig sensor liveness | active |
| `forgia2_combat.json` | T0 | forgia-observability | `src/forgia2_aggregator.rs:133` | 1Hz | **combat unified** (damage_dir + hitscan + hud_ammo + killfeed + screen_flash) — **story-545 sensor canonique** | active |
| `forgia2_entities.json` | T0 | forgia-observability | `src/entities_sensor.rs:73` | 1Hz | entities count, archetype breakdown | active |
| `forgia2_health.json` | T0 | forgia-observability | `src/health_sensor.rs:60` | 1Hz | health checks aggregator (`checks.rs` 747 LOC) | active |
| `forgia2_input.json` | T0 | forgia-observability | `src/input_sensor.rs:72` | 1Hz | leafwing actions, AZERTY mapping | active |
| `forgia2_lag_events.json` | T0 | forgia-observability | `src/lag_events_sensor.rs:114` | event | frame stutter > 30ms, cluster detection | active |
| `forgia2_level_modules.json` | T0 | forgia-level-presets | `src/lib.rs:59` | once | level preset palette, anchor population | active |
| `forgia2_lifecycle.json` | T0 | forgia-observability | `src/lifecycle_sensor.rs:93` | 1Hz | players/bots/target_cubes added/removed/inserted | active |
| `forgia2_memory.json` | T0 | forgia-observability | `src/memory_sensor.rs:69` | 1Hz | RAM RSS, VRAM (when available) | active |
| `forgia2_migration_baseline.json` | T0 | forgia-observability | `src/migration_baseline.rs:33` | once | baseline E1/E2 forgia_*→forgia2_* migration tracking | active |
| `forgia2_perf.json` | T0 | forgia-observability | `src/perf_sensor.rs:73` | 1Hz | FPS, frame_time, smooth | active |
| `forgia2_player_hp_diag.json` | T0 | forgia-ui-lib (hud/player_hp) | `src/hud/player_hp.rs:?` (WIP autre terminal 2026-05-28) | 1Hz | HP bar render skips, frames_skipped reasons | active-wip |
| `forgia2_player_state.json` | T0 | forgia-observability | `src/player_state_sensor.rs:134` | 1Hz | player position, velocity, grounded, swim | active |
| `forgia2_rex_bones.json` | T3 | forgia-anim-locomotion | `src/locomotion.rs:477` | event | Rex skeleton bind dump (story-482) | active |
| `forgia2_rex_bones_live.json` | T3 | forgia-anim-locomotion | `src/locomotion.rs:780` | 1Hz | Rex bones live rotations (clavicle_l/r etc.) | active |
| `forgia2_roguelite_state.json` | T0 | forgia-mode-roguelite | `src/sensor.rs:110` | 1Hz | run_state, wave, bots_alive, victory | active |
| `forgia2_rpg_health.json` | T0 | forgia-observability | `src/exporter.rs:16` | 1Hz | RPG-specific health (terrain ready, player spawn) | active |
| `forgia2_sensor_health.json` | T0 | forgia-observability | `src/sensor_health_sensor.rs:86` | 1Hz | meta-sensor : liveness des 13 forgia2_ canoniques | active |
| `forgia2_skeleton_template_registry.json` | T0 | forgia-skeleton-template | `src/lib.rs:786,1267` | once+hot | SkeletonTemplate registry, TOML hot-reload | active |
| `forgia2_stage.json` | T0 | forgia-anchor **+** forgia-stage **+** forgia-stage::layout_sensor | `forgia-anchor/src/lib.rs:474` **+** `forgia-stage/src/lib.rs:51` **+** `forgia-stage/src/layout_sensor.rs:248` | event | stage load, anchor record (**triple-writer** à investiguer) | **duplicate-writer** |
| `forgia2_stage_layout.json` | T0 | forgia-stage | `src/layout_sensor.rs:16` | event | stage layout post-load (cover, sightline) | active |
| `forgia2_walk_pose.json` | T0 | forgia-anim-locomotion | `src/locomotion.rs:884` | 1Hz | walk pose phase, foot contacts | active |
| `forgia2_watchdog.json` | T0 | forgia-observability | `src/watchdog_sensor.rs:74` | 1Hz | watchdog heartbeat, seconds_in_emergency | active |

## Sensors observés runtime mais sans producteur trouvé via grep

À investiguer (story follow-up) :

| Filename | Hypothèse | Action |
|---|---|---|
| `forgia_voicelines.json` | Producer supprimé lors fusion `forgia-audio-voicelines` → scaffold vide (cf MEMORY) | Vérifier : sensor stale OU producteur résiduel |
| `forgia_water.json` | Système water sans observabilité (cf `observability-required.md` §"NON CONFORMES") | Adresser dans story water-observability |
| `forgia_music_state.json` | Probablement écrit par audio_sensor (chemin indirect) | Tracer via cargo expand |
| `forgia_rex_bones_entry_{1..4}_{bind,live}.json` | Snapshots indexés statiques | Documenter ou archiver |

## Duplicate writers

Audit révèle **2 sensors** écrits par ≥2 producteurs (risque de race + valeurs incohérentes) :

1. **`forgia_prefab.json`** — `forgia-stage/src/lib.rs:1177` + `forgia-prefab/src/lib.rs:164`
   - **Risque** : counter `total_spawned` ambigu (déjà documenté trompeur in MEMORY).
   - **Recommandation** : décider d'un seul writer, l'autre lit.

2. **`forgia2_stage.json`** — 3 writers (`forgia-anchor` + 2× `forgia-stage`)
   - **Risque** : last-write-wins sur dump 1Hz, ordre dépendant scheduling.
   - **Recommandation** : un seul writer authoritative dans `forgia-stage::layout_sensor`, autres deviennent consommateurs.

## Sensors couverts par xtask `verify-sensors-format`

13 canoniques (cf `xtask/src/main.rs::CANONICAL_SENSORS`) :
`forgia2_health, forgia2_rpg_health, forgia2_arena, forgia2_combat, forgia2_perf, forgia2_entities, forgia2_memory, forgia2_lifecycle, forgia2_watchdog, forgia2_audio, forgia2_input, forgia2_sensor_health, forgia2_roguelite_state`.

**Gap** : 47+ sensors non couverts. La story-546 phase 2 (`sensor-audit`) étend la couverture à 100%.

## Cross-refs

- `xtask/src/main.rs::verify_sensors_format` — gate format JSON (13 canoniques)
- `xtask/src/main.rs::sensor_audit` (story-546 phase 2) — gate registry exhaustif
- `.claude/rules/observability-required.md` — règle d'origine pour features nouvelles
- `.claude/rules/concept-first.md` §6 — colonne Sensor du tableau concepts
- Story-545 — bug canonique motivant cette story

---

*Mise à jour : 2026-05-28 (création story-546). Format inspiré Epic Data Registry (typed versioned registries) + Bevy Cheat Book observability patterns.*
