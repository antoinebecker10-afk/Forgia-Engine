# Concept-First - Table des concepts Forgia

> Extraite du protocole universel `concept-first` (sa section 6). Specifique Forgia : producteur / consommateurs / timing / sensor par concept.
> Se complete a chaque demande resolue ; une ligne dont le path n'existe plus = a re-pointer (anti-derive).

## 6. Tableau des concepts Forgia

**Légende colonnes** :
- **Layer** : `fw`=framework Rust, `def`=definition TOML, `lua`=behaviour Luau, `story`=exception scénarisée
- **Timing** : `boot`=Startup une fois, `on_enter(State)`=OnEnter, `frame`=every frame, `hr`=hot-reload genome
- **Hot** : `*` si chemin perf critique (déclenche §3 étape 4)
- **Net** : `R`=replicated (lightyear), `L`=local
- **Script** : `Lua`=exposed Luau, `int`=internal Rust

| Concept | Mots à grep | Layer | **Producteur** (vérité, timing) | **Consommateurs** (rôle, timing) | Sensor | Hot | Net | Script |
|---|---|---|---|---|---|---|---|---|
| **water** | water, swim, sea_level, underwater, breath | fw+def | V2 : `forgia-water::SeaLevel(pub f32)` Resource (boot, fix story-17b5743 2026-05-28) ; `WaterSettings.height` (sync via Changed) | V2 : `forgia-rpg/lib.rs:104` insert SeaLevel(RPG_SEA_LEVEL=4.0) ; `forgia2_rpg_player_sensor` is_swimming/depth (1Hz) ; bevy_water render (frame) | V2 : `forgia_water.json` (story-552, 2026-05-28 ✅) + `forgia2_rpg_player.json` (swim/depth) | * | L | int |
| **gravity** | gravity, fall, velocity_y, terminal_velocity | fw+def | `RapierConfiguration.gravity` (boot, Component query Rapier 0.33) ; `FpsTuning.gravity` (hr) | `player::movement::vertical_velocity` (frame) ; AI movement | V2 : `forgia2_physics.json` (story-549, 2026-05-28 ✅) | * | R | int |
| **combat** | combat, damage, hp, attack, weapon, hitscan, melee | fw+def+lua | Genome arme/ennemi `config/genomes/*.toml` (hr) ; `Health` component (frame) | `combat/weapons.rs:314` (frame) ; `combat/melee.rs:65` (frame) ; `combat/viewmodel.rs:582` (frame) ; `ai/combat_ai.rs` | V2 : `forgia2_combat.json` (5 sub-sources : damage_dir, hitscan, hud_ammo, killfeed, screen_flash) | * | R | Lua |
| **inventory** | inventory, slot, pickup, equip, loot | fw+def | `Inventory` **Component sur Player** (LOCK-INV-1 80 slots, V2 = `forgia-rpg-data::inventory`) | `inventory/`, `equipment.rs`, HUD slots (frame) | V2 : `forgia2_inventory.json` (story-555, 2026-05-28 ✅ — audit LOCK-INV-1 + top 5 items) | - | R | Lua |
| **quests** | quest, Quest, QuestLog, objective, completion | fw+def | V2 : `QuestCatalogue` Resource + `QuestLog` Component sur Player (`forgia-rpg-data::quests`) | `advance_quests` system (frame, on `QuestProgress` events) ; HUD UI | V2 : `forgia2_quests.json` (story-554, 2026-05-28 ✅ — counts par status + top 10 active) | - | R | Lua |
| **npc-dialogue** | Npc, dialogue, DialogueSession, DialogueTree | fw+def | V2 : `Npc` Component (forgia-rpg) + `DialogueRegistry` Resource + `DialogueSession` Component sur Player quand actif (forgia-rpg-data) | `start_sessions`/`advance_sessions`/`end_sessions` ; HUD dialogue UI | V2 : `forgia2_npcs.json` (story-556, 2026-05-28 ✅ — npc_count + near_player + dialogue active state) | - | R | Lua |
| **biome** | biome, BiomeMap, biome_at, biome_current, voronoi | fw+def | `BiomeMap` Resource (V2, Voronoi 10 biomes, boot via `BiomeMap::generate`) | `forgia-audio::biome_at(player_pos)` ; `forgia-foliage::biome_at(sample)` ; `forgia-rpg::lib.rs:986` | V2 : `forgia_chunks_snapshot.json::biome_distribution` (global) + `forgia2_rpg_player.json::biome_current` (player lookup) | - | L | int |
| **camera** | camera, fps, third_person, viewmodel, is_third_person | fw | `CameraMode` Resource (boot, on_enter Arena force 1P) | `combat/viewmodel.rs:582` ; `combat/weapons.rs:314` ; `combat/melee.rs:65` ; `ui/hud/crosshair.rs:54` (frame) | `forgia_camera.json` | * | L | int |
| **terrain** | terrain, biome, chunk, heightmap, surface_net | fw+def | `forgia-terrain::ChunkManager` ; `BiomeMap` ; `MapGenConfig` (boot+streaming) | vegetation, roads, water, castle, ai, audio biome (frame+streaming) | `forgia_chunks_snapshot.json`, `forgia_terrain.json` | * | L | int |
| **PBR/material** | pbr, metallic, roughness, albedo, material | fw+def | `StandardMaterial` per asset (boot) ; genome material override (hr) | `material_autofix.rs` (every Material asset Changed) | `forgia_textures.json` | - | L | int |
| **state machine** | GameMode, AppMode, WorldMode, OnEnter, OnExit, DespawnOnExit | fw | `app_state::GameMode/AppMode` States (boot) | `session_cleanup.rs` (OnExit) ; `arena_placeholder.rs` (OnEnter Arena) ; `map_switch.rs` ; `health_monitor` | `forgia_last_state.json` | - | L | int |
| **spawn** | spawn, respawn, teleport, RespawnPoint, ArenaSpawn | fw | `RespawnPoint` Resource ; `ArenaSpawn` (OnEnter Arena) | `arena_force_player_spawn` (OnEnter) ; `NeedsSpawnSnap` ; `SpawnGuardian` ; `deferred_world_setup` | `forgia_entities_snapshot.json` | - | R | int |
| **audio** | audio, sound, biome_audio, MusicState, footstep | fw+def | `MusicState` Resource ; genome `biome_audio.toml` (hr) | `bevy_kira_audio` ; `audio_registry.rs` (boot) ; `audio_footsteps.rs` (frame) | `forgia_audio.json` | - | L | int |
| **input** | input, keybind, PlayerAction, AZERTY | fw+def | `leafwing-input-manager` ActionState (frame) ; `KeybindRegistry` (boot+hr) | All systems `.in_set(GameSet::Input)` (frame) | `forgia_input_log.json` | * | L | int |

**Le tableau se complète à chaque demande résolue.** Stale dès qu'un path est cassé — marquer ou re-pointer.

---

## Discipline grepai — référence mesurée du 2026-08-08

`concept-first.md` §3 étape 2 exige `grepai_search` sur le mot-concept, et pose
la métrique : « **stat à 0 = règle ignorée** ». Voici l'état constaté, pour que
la dérive se mesure au lieu de se supposer.

```
Total queries      11          ← plat depuis le 2026-07-31
By command:        search 11
                   trace-callers  0   ← jamais utilisé
                   trace-callees  0   ← jamais utilisé
                   trace-graph    0   ← jamais utilisé
Économie quand il sert : 93,2 % (78 907 tokens sur 11 requêtes)
```

Pendant ces 8 jours de stat plate ont été livrés : le hub story-678 complet, le
Marketplace, le système de cosmétiques, la refonte de la fiche personnage et un
audit de 63 constats — c'est-à-dire précisément le travail transverse pour
lequel la règle existe.

### Le cercle vicieux à connaître

L'index était figé au **30 juillet**, la dernière requête date du **31**. Il a
cessé d'être utilisé le lendemain du jour où il a cessé d'être frais. Un index
périmé répond à côté → on l'évite → personne ne le réindexe. **Réindexer est
donc la première marche, pas la dernière** : sans ça, exiger son usage revient
à exiger des réponses fausses.

### Comment l'user vérifie, sans dépendre de l'agent

```bash
grepai stats      # le compteur est tenu par grepai, l'agent ne peut pas le forger
```

1. **Le total monte** — plancher minimal.
2. **`By command:` se diversifie** — tant qu'on ne voit que `search`, l'agent
   fait avec grepai ce qu'il faisait avec grep. `trace-callers` qui apparaît
   = il utilise enfin ce qu'un grep ne sait pas faire.
3. **Demander « tu l'as trouvé comment ? »** et attendre un `fichier:ligne`
   issu de l'appel. Un compteur peut être gonflé par des requêtes creuses ;
   une citation, non.
