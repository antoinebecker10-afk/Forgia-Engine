# Forgia V2 — Architecture

> Document vivant. Mis à jour à chaque ajout de crate ou changement structurel majeur.
>
> **Dernière révision** : 2026-05-19 (Vague 2 audit forensic — drift 13→258 crates documenté).

## 1. Vue d'ensemble

Workspace Rust 258 crates organisées en **DAG strict**, `forgia-core` au centre sans dépendance workspace. Le ratio actuel est **56 crates wired** (21.6 %) depuis `forgia-game` et **203 crates orphelines** (78.4 %) intentionnellement réservées pour les phases ultérieures.

```
                            forgia-game (bin)
                                  │
          ┌──────┬───────┬────────┼────────┬───────┬──────────┐
          │      │       │        │        │       │          │
   forgia-fps  forgia-rpg  forgia-ui  forgia-observability  (mode-spec plugins)
       │         │            │
       ├─────────┴───────┐    │
       │                 │    │
   forgia-combat   forgia-terrain
       │                 │
       ├──────┬──────┐   │
       │      │      │   │
   forgia-effects forgia-player (uses forgia-input)
       │              │
       └──────┬───────┘
              │
        forgia-assets
              │
        forgia-core ← ne dépend de rien (DAG-libre)
```

## 2. Drift 13 → 258 crates — **intentionnel, pas une dérive**

Le bootstrap 2026-05-14 partait sur 13 crates "fondations". Les sessions marathon 2026-05-15 à 2026-05-18 ont **délibérément** ajouté des scaffolds (≤50 LOC stubs) pour réserver les namespaces des futures phases (Renzora editor, post-process effects, networking, scripting Luau, etc.) — règle "fine-grained crates" capitalisée dans la mémoire `reference_rule_fine_grained_crates.md`.

État audit forensic 2026-05-19 :

| Classification | Count | % | Définition |
|---|---|---|---|
| **PARTIAL** (≥50 LOC, travail réel) | **41** | 15.9 % | Phase 0-3 livrée |
| **SCAFFOLD** (<50 LOC, stub) | **220** | 85.3 % | Réservation Phase 2-6 |
| **DEAD** (vide / TODO uniquement) | **0** | 0 % | Discipline minimale tenue |

**Risque géré** : les 220 scaffolds restent surveillés. À 6 mois, tout scaffold encore <50 LOC doit être documenté (Phase prévue + ETA) ou supprimé. `xtask check-orphans` Phase 5 doit flagguer la dérive.

## 3. Phases par préfixe — **réservations délibérées**

| Préfixe | Count | Phase prévue | Status |
|---|---|---|---|
| `forgia-core` | 1 | Phase 0 | ✅ FULL |
| `forgia-terrain` | 1 | Phase 1 | ✅ FULL (7376 LOC, 23 tests, référence d'excellence) |
| `forgia-combat`, `forgia-fps`, `forgia-rpg` | 3 | Phase 2-3 | ✅ PARTIAL (vertical slice livré) |
| `forgia-juice-*` | 5 | Phase 2 gunfeel | ✅ 4/5 wired (camera-shake, fov-punch, hit-stop, recoil) |
| `forgia-ai-*` | 10 | Phase 3 IA | ⚠️ 1/10 wired (`forgia-ai-arena-bot` only) |
| `forgia-village-*` | 4 | Phase 3 procgen | ✅ 3/4 wired (story-441 + story-442 IP) |
| `forgia-auto-rig`, `forgia-mesh-voxelizer`, `forgia-medial-axis`, `forgia-skeleton-embedder` | 4 | Phase 3 character pipeline | ✅ Wired (story-440 Phase 1A/B DONE) |
| `forgia-camera-*` | 2 | Phase 3 | ✅ Wired (camera-fps, camera-orbit) |
| `forgia-mode-*` | 8 | Phase 3+ | ⚠️ 2/8 wired (fps-arena, rpg-openworld). 6 autres (platformer/puzzle/race/roguelite/rts/survival) **réservés Phase Build/Edit** |
| `forgia-pp-*` (post-process) | 45 | Phase 2-3 rendering | ❌ 0/45 wired — génération batch délibérée, à activer par presets graphics |
| `forgia-render-*` (advanced) | 12 | Phase 2-3 | ❌ 0/12 wired — SSAO/SSR/DOF/clouds/atmosphere |
| `forgia-editor-*` (Renzora bridge) | 16 | Phase 4-5 | ❌ 0/16 wired — Edit mode = M5+ |
| `forgia-net-*` (multiplayer) | 7 | Phase 4 | ❌ 0/7 wired — networking lightyear |
| `forgia-script-*` + `forgia-scripting-luau` | 5 + 1 | Phase Build (freemium) | ❌ 0/6 wired — `bevy_mod_scripting` stack présent |
| `forgia-qa-*` | 8 | Phase 5 observability | ❌ 0/8 wired — autopilot/harness/replay/vlm |
| `forgia-vfx-*` (decals/tracers/impacts) | 4 | Phase 2-3 | ⚠️ Délégué à `forgia-effects` agg actuellement |
| `forgia-weapon-*` (extraction) | 5 | Phase 3 (Tier 2A/B) | ❌ Bloqué par recovery WIP — extraction depuis `forgia-combat` reportée |
| `forgia-input-*` | 6 | Phase 2-4 | ⚠️ 1/6 wired (`forgia-input` core) |
| `forgia-audio-*` | 8 | Phase 3 | ⚠️ 1/8 wired (`forgia-audio-biome`) |
| `forgia-ui-*` | 17 | Phase 3-4 | ⚠️ 5/17 wired (hud, hud-ammo, damage-direction, pause-menu, dialogue) |
| `forgia-asset-*` | 6 | Phase 3+ | ⚠️ 3/6 wired (`forgia-assets`, `forgia-asset-registry`, `forgia-asset-cdn`) |
| `forgia-genome-*` | 9 | Phase 0+ | ⚠️ 1/9 wired (`forgia-genome-core`) |
| Autres feature crates | ~30 | varia | varia |

## 4. Crates (couches centrales)

### forgia-core (lib, 0 dep workspace)
- **Rôle** : Types core hérités à tout le workspace
- **Contient** : `AppMode` / `GameMode` / `WorldMode` States, `GameSet` enum, Resources globales
- **Règle** : modification = audit complet, c'est la fondation immutable

### forgia-assets (lib)
- **Rôle** : `GameAssets` Resource + preload Startup
- **Lock L1 reborn** : whitelist `asset_load_whitelist.txt` enforcée par `xtask check-orphans`
- **Cible** : ≤ 50 handles (vs 136 V1), ≤ 30 call-sites `asset_server.load()` (vs 120 V1)

### forgia-input (lib)
- **Rôle** : Leafwing PlayerAction AZERTY + KeybindRegistry + InputBlockers
- **Anti-trap V1** : 1 KeyCode = 1 handler unique

### forgia-player (lib)
- **Rôle** : KinematicCharacterController rapier3d + caméra 1P/3P + spawn/respawn
- **Pattern V1 conservé** : `is_third_person==false` gate viewmodel/crosshair

### forgia-combat (lib) — **PARTAGÉ FPS/RPG**
- **Rôle** : Gunfeel V5-F (12 fichiers V1 portés VERBATIM Phase 2, exigence game-maker)
- **Piliers** : weapons + viewmodel + hit-stop + camera recoil + hitmarker + HitFlash + damage numbers + tracer cache + damage falloff
- **Decision GO/NO-GO V2** : si gunfeel V2 ≠ V1 après Phase 2 → retour V1 Strangler
- **Sensor** : `forgia_combat.json` (1Hz, story-457 Vague 1)

### forgia-effects (lib)
- **Rôle** : Hanabi VFX + audio combat
- **Pattern obligatoire Phase 0** : pre-spawn dummy `Visibility::Hidden` au Startup (anti-freeze 25s, story-436 V1)

### forgia-terrain (lib) — **PORT VERBATIM V1**
- **Rôle** : Procédural OpenWorld (DAG-libre, 0 dep workspace sauf forgia-core)
- **Statut** : porté Phase 1, désactivé en mode FPS, activé en mode RPG via `run_if(in_state(GameMode::Rpg))`
- **Patterns à reproduire dans toutes crates V2** (cf §"Patterns terrain")
- **Sensors** : `forgia_terrain_lod.json` + `forgia_chunks_snapshot.json` + `forgia_chunk_stream.json` + `forgia_vegetation.json`

### forgia-fps (lib)
- **Rôle** : Mode FPS Arena
- **Phase 3** : modules KayKit assemblés (pattern Hades), bots IA, multi-rooms
- **Lock** : KayKit `WALL_Y = 0.0` (jamais modifier, pivot mesh au sol)

### forgia-rpg (lib) — **SQUELETTE V1, dev V2.M2**
- **Rôle** : Mode RPG OpenWorld
- **Phase 0** : juste Plugin squelette pour que le menu propose "RPG"
- **M2** : quêtes, NPCs, dialog, inventaire (LOCK-INV-1 80 slots)

### forgia-ui (lib)
- **Rôle** : Menu (Start + choix FPS/RPG + Pause + Settings) + HUD partagé
- **Anti-traps V1 dès Phase 0** :
  - `MenuCamera2d` isolé, OnEnter(Menu)/OnExit(Menu)
  - 1 seul handler ESC avec gardes par AppMode
  - `Time<Real>` pour tout sensor UI

### forgia-observability (lib)
- **Rôle** : RPG Health Monitor — 6 checks cross-sectoriels (story-452 DONE)
- **Sensors produits** : `forgia_rpg_health.json` (1Hz aggregator), `forgia_health.json` (cross-mode minimaliste, story-457 Vague 1)
- **Hotkey** : Shift+F12 reload `config/genomes/rpg_monitor.toml`

### forgia-sensors (lib)
- **Rôle** : Stack observability future — fusion 27 producteurs `forgia_*.json` legacy vers 12 sensors `forgia2_*.json` canoniques (Phase 5)
- **Statut Vague 2** : abandon partiel cible "12 max" — décision pragmatique vu drift assumé
- **Convention** : 1 producteur unique par sensor, format `id`/`severity`/`next_step`/`or_exclusion`

### forgia-game (bin)
- **Rôle** : Wire tous les plugins, gate FPS vs RPG par `GameMode`
- **Cible** : ~400 LOC (vs 41 386 octets de `main.rs` V1)

### xtask (bin)
- **Rôle** : Automation
- **Tasks** : `check-orphans`, `schedule-dump`, `baseline-e1-e2`, `verify-sensors-format` (Phase 5)

## 5. GameSet — chaîne ordering canonique

```rust
GameSet::Network    // lightyear receive (V2 ajout)
GameSet::Input      // leafwing ActionState
GameSet::Movement   // player + AI
GameSet::Physics    // rapier
GameSet::Camera     // lerp + collision
GameSet::Combat     // weapons + hitscan + damage
GameSet::Effects    // VFX + screen shake
GameSet::Sensors    // JSON export + health monitor (V2 ajout)
GameSet::UI         // egui + HUD
```

Configuration : `crates/forgia-core/src/system_set.rs`. **Lock L7**.

## 6. Patterns terrain à reproduire (héritage V1 forgia-terrain)

`forgia-terrain` est l'exemple d'excellence d'architecture V1. Reproduire ces patterns dans **toutes** les crates V2 :

### 6.1 DAG-libre
Une crate ne doit dépendre que du strict nécessaire. `forgia-terrain` ne dépend QUE de `forgia-core`. Aucune dépendance vers `forgia-game` ou `forgia-engine`. Bénéfice : compile parallèle, isolation tests, étudiant peut bosser sans recompiler tout.

### 6.2 BiomeGenomeOverrides pattern (struct bridge data-driven)
Quand une crate lib a besoin de paramètres genome mais ne veut pas dépendre de `GenomeRegistry` (pour rester DAG-libre), elle expose une struct plain data remplie par le consommateur :

```rust
// Dans forgia-terrain
pub struct BiomeGenomeOverrides {
    pub noise_scale: f32,
    pub erosion_strength: f32,
    // ... plain data, pas de Res<>
}

// forgia-game remplit la struct depuis GenomeRegistry et la passe
```

À reproduire pour `forgia-combat` (CombatGenomeOverrides), `forgia-fps` (ArenaGenomeOverrides), etc.

### 6.3 Pipeline async via Bevy Tasks
Le meshing terrain V1 est async (poll non-bloquant). Pattern `meshing.rs::poll_one_mesh` avec `swap_remove` pour FIFO non-ordonné. Reproductible pour audio loading, asset streaming, AI computations lourdes.

### 6.4 LRU cache pattern
`ChunkManager` cache 64 chunks max. Reproduire pour : props instances, audio sources spatial, particles ParticleEffect dummy.

### 6.5 Tests headless ≥ 16 par crate
forgia-terrain V1 a 23 tests couvrant ChunkCoord/ChunkData/ChunkManager/TerrainConfig (story-349 E2). Standard à atteindre dans toutes les crates V2.

### 6.6 0 TODO/FIXME orphelin
forgia-terrain V1 = 0 TODO/FIXME dans 13 750 LOC. Standard V2 (audit forensic confirme 192 TODO mais 80 % dans scaffolds Phase 0 intentionnels).

## 7. Anti-patterns V1 enforced dès Phase 0

| Anti-pattern V1 | Garde V2 |
|---|---|
| `#[allow(dead_code)]` au niveau crate/module | CI bloque |
| Plugin défini, jamais wired | `xtask check-orphans` Phase 5 |
| Hardcode gameplay literal | grep CI sur `combat/`, `weapons/`, `viewmodel/` |
| 2 handlers même KeyCode | Convention `forgia-input` strict |
| Sensor sans producteur | `forgia_rpg_health.json` CHK-5 flag stale 1Hz |
| `process_memory_mb=0.0` muet | flag `_available: false` explicite |
| `TimePlugin` + `advance_by` dans tests headless | Helper `app_with_manual_time()` (story-457 Vague 4) |

## 8. Stability Locks V2

Activés au fil des Phases (cf CLAUDE.md §5).

| Lock | Feature | Statut |
|---|---|---|
| L1 | GameAssets whitelist | Reborn — cible ≤ 50 handles / 30 call-sites |
| L2 | PerfMode F4 | Pending Phase 5 |
| L3 | Camera collision (1 raycast/frame) | Wired forgia-camera-fps |
| L4 | EditorRaycast | N/A Phase 0-3 (Edit mode = M5+) |
| L5 | Nameplate LOD | Wired via story-456 forgia-enemy-nameplate WIP |
| L7 | SystemSets GameSet chaîne 9 étapes | Wired forgia-core/src/system_set.rs |
| L8 | Minimap cache | Pending |
| LOCK-INV-1 | Inventory 80 slots max | Pending forgia-inventory wire |

## 9. Sensors V2 — état réel 2026-05-19

| Sensor | Producteur | Format CLAUDE.md `{id, severity, next_step}` |
|---|---|---|
| `forgia_rpg_health.json` | `forgia-observability` (aggregator 6 checks) | ✅ FULL |
| `forgia_health.json` | `forgia-observability::health_sensor` (cross-mode) | ✅ FULL |
| `forgia_combat.json` | `forgia-combat::sensor` (1Hz player_hp/weapon/shots) | ✅ FULL |
| `forgia_auto_rig.json` | `forgia-auto-rig` (pattern next_step) | ✅ FULL |
| 23 autres (anim, arena, chunk_stream, hitscan, hud_ammo, killfeed, mesh_fader, prefab, terrain_lod, vegetation, viewmodel_*, village_*, walk_pose, etc.) | crates dispersées | ⚠️ Format legacy V1 — fusion Phase 5 prévue |

**Cible Phase 5** : 27 sensors → 12 `forgia2_*.json` canoniques. Statut : **non démarrée, scope risqué**. Re-évaluer après Phase 4.

## 10. Ship target V1 V2

| Phase | Durée | Cumul | Statut |
|---|---|---|---|
| 0 Bootstrap | 1 sem | 1 sem | ✅ DONE |
| 1 Hello World | 1.5 sem | 2.5 sem | ✅ DONE |
| 2 Gunfeel V5-F (GO/NO-GO) | 4 sem | 6.5 sem | ✅ DONE |
| 3 Arena modulaire | 5 sem | 11.5 sem | ⚠️ IN PROGRESS (story-441/442/448/449/453/455/456) |
| 4 UI/menu propre | 4 sem | 15.5 sem | À démarrer |
| 5 Sensors + observability | 3 sem | 18.5 sem | Partiellement faite (CHK-1..6 livrés, fusion `forgia2_*` reportée) |
| 6 Polish + Steam ship | 5 sem | 23.5 sem | Pending |

**Ship cible** : Q1-Q2 2027 (60 % confiance) avec fallback Bots Brawl Q4 2026.

---

*Mise à jour : 2026-05-19 — Vague 2 audit forensic. Précédente : 2026-05-14 V2 bootstrap.*
