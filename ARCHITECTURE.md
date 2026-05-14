# Forgia V2 — Architecture

> Document vivant. Mis à jour à chaque ajout de crate ou changement structurel majeur.

## Vue d'ensemble

13 crates organisées en **graphe acyclique** (DAG strict). `forgia-core` au centre, ne dépend de RIEN.

```
                            forgia-game (bin)
                                  │
          ┌──────┬───────┬────────┼────────┬───────┬──────────┐
          │      │       │        │        │       │          │
   forgia-fps  forgia-rpg  forgia-ui  forgia-sensors    (mode-spec plugins)
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

## Crates

### forgia-core (lib, 0 dep workspace)
- **Rôle** : Types core hérités à tout le workspace
- **Contient** : `AppMode` / `GameMode` / `WorldMode` States, `GameSet` enum, Resources globales
- **Règle** : modification = audit complet, c'est le fondation immutable

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

### forgia-effects (lib)
- **Rôle** : Hanabi VFX + audio combat
- **Pattern obligatoire Phase 0** : pre-spawn dummy `Visibility::Hidden` au Startup (anti-freeze 25s, story-436 V1)

### forgia-terrain (lib) — **PORT VERBATIM V1**
- **Rôle** : Procédural OpenWorld (DAG-libre, 0 dep workspace sauf forgia-core)
- **Statut** : porté Phase 1, désactivé en mode FPS, activé en mode RPG via `run_if(in_state(GameMode::Rpg))`
- **Patterns à reproduire dans toutes crates V2** (cf §"Patterns terrain")

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

### forgia-sensors (lib)
- **Rôle** : Stack observability — 12 sensors max (vs 95 V1)
- **Phase 5** : `forgia2_health.json` + 11 sensors fusionnés depuis ~30 V1 doublons
- **Convention** : 1 producteur unique par sensor, format `id`/`severity`/`next_step`/`or_exclusion`

### forgia-game (bin)
- **Rôle** : Wire tous les plugins, gate FPS vs RPG par `GameMode`
- **Cible** : ~400 LOC (vs 41 386 octets de `main.rs` V1)

### xtask (bin)
- **Rôle** : Automation
- **Tasks** : `check-orphans`, `schedule-dump`, `baseline-e1-e2`

## GameSet — chaîne ordering canonique

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

Configuration : `crates/forgia-core/src/system_set.rs`. Lock L7.

## Patterns terrain à reproduire (héritage V1 forgia-terrain)

`forgia-terrain` est l'exemple d'excellence d'architecture V1. Reproduire ces patterns dans **toutes** les crates V2 :

### 1. DAG-libre
Une crate ne doit dépendre que du strict nécessaire. `forgia-terrain` ne dépend QUE de `forgia-core`. Aucune dépendance vers `forgia-game` ou `forgia-engine`. Bénéfice : compile parallèle, isolation tests, étudiant peut bosser sans recompiler tout.

### 2. BiomeGenomeOverrides pattern (struct bridge data-driven)
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

### 3. Pipeline async via Bevy Tasks
Le meshing terrain V1 est async (poll non-bloquant). Pattern `meshing.rs::poll_one_mesh` avec `swap_remove` pour FIFO non-ordonné. Reproductible pour audio loading, asset streaming, AI computations lourdes.

### 4. LRU cache pattern
`ChunkManager` cache 64 chunks max. Reproduire pour : props instances, audio sources spatial, particles ParticleEffect dummy.

### 5. Tests headless ≥ 16 par crate
forgia-terrain V1 a 16 tests couvrant ChunkCoord/ChunkData/ChunkManager/TerrainConfig (story-349 E2). Standard à atteindre dans toutes les crates V2.

### 6. 0 TODO/FIXME orphelin
forgia-terrain V1 = 0 TODO/FIXME dans 13 750 LOC. Standard V2.

## Anti-patterns V1 enforced dès Phase 0

| Anti-pattern V1 | Garde V2 |
|---|---|
| `#[allow(dead_code)]` au niveau crate/module | CI bloque |
| Plugin défini, jamais wired | `xtask check-orphans` Phase 5 |
| Hardcode gameplay literal | grep CI sur `combat/`, `weapons/`, `viewmodel/` |
| 2 handlers même KeyCode | Convention `forgia-input` strict |
| Sensor sans producteur | `forgia2_sensor_health.json` flag stale |
| `process_memory_mb=0.0` muet | flag `_available: false` explicite |

## Stability Locks V2

Activés au fil des Phases (cf CLAUDE.md §5).

## Ship target V1 V2

| Phase | Durée | Cumul |
|---|---|---|
| 0 Bootstrap | 1 sem | 1 sem |
| 1 Hello World | 1.5 sem | 2.5 sem |
| 2 Gunfeel V5-F (GO/NO-GO) | 4 sem | 6.5 sem |
| 3 Arena modulaire | 5 sem | 11.5 sem |
| 4 UI/menu propre | 4 sem | 15.5 sem |
| 5 Sensors + observability | 3 sem | 18.5 sem |
| 6 Polish + Steam ship | 5 sem | 23.5 sem |

**Ship cible** : Q1-Q2 2027 (60 % confiance) avec fallback Bots Brawl Q4 2026.

---

*Mise à jour : 2026-05-14 — V2 bootstrap*
