# Forgia V2 — Architecture

> Document vivant, **gardé mécaniquement** par `cargo xtask arch-drift` (la liste de
> crates ci-dessous doit correspondre exactement aux members de Cargo.toml, sinon le
> gate échoue). Réécrit intégralement le 2026-06-10 (story-593) après l'audit complet —
> la version précédente décrivait 258 crates pour 62 réelles.
>
> **Dernière révision** : 2026-06-10.

## 1. Vue d'ensemble

Workspace Rust de **62 crates + xtask**, binaire canonique = package racine `forgia`
(`src/main.rs`, pattern root-binary Renzora) qui appelle `forgia_game::run_game()`.
`forgia-game` est une **lib** d'assemblage (~135 LOC) : elle câble tous les plugins.

```
src/main.rs (bin forgia, + panic hook → forgia2_crash.json)
   └─ forgia_game::run_game()
        ├─ DefaultPlugins (window 1920×1080) → ForgiaCorePlugin → Rapier → Hanabi
        ├─ Gameplay : assets, input, player, effects, combat, damage, ui, ui-lib(×7),
        │             killfeed, juice-screen-flash, enemy-nameplate, observability,
        │             qa-core+qa-replay (no-op sans feature qa-runtime)
        ├─ Data RPG : rpg-data (inventory/quests/xp/dialogue)
        ├─ Modes    : fps, rpg, mode-roguelite (run_if interne par GameMode)
        ├─ Monde    : asset-registry, streaming, terrain, foliage, water, audio, worldgen
        ├─ Anim/cam : anim-debug, camera-orbit, secondary-motion
        └─ Dev      : debug (F2), prefab (guard idempotent), village-loader (zombie, cf §6)
```

États (forgia-core) : `AppMode` (Menu/InGame), `GameMode` (Fps/Rpg/Roguelite),
`WorldMode`. Boot → `AppMode::Menu`.

## 2. GameSet — chaîne d'ordering canonique (Lock L7)

```
Network → Input → Movement → Physics → Camera → Combat → Effects → Sensors → UI
```

Définie dans `crates/forgia-core/src/lib.rs` (module `system_set` inline — il n'y a
PAS de fichier system_set.rs). Dérive connue (audit 2026-06-10) : la chaîne player de
`forgia-player` est hors GameSet — fix prévu M2 (roadmap B4).

## 3. Les 62 crates réelles (gardé par arch-drift)

> LOC mesurées le 2026-06-10. Wired : ✅ = add_plugins direct dans forgia-game,
> ✦ = consommée transitivement (lib), 🔧 = outil dev/CLI (pas dans le binaire jeu),
> ⚠️ = orpheline (aucun consommateur — décision à prendre).

### Socle
| Crate | LOC | Rôle | Wired |
|---|---|---|---|
| forgia-core | 123 | États, GameSet, 0 dep workspace (DAG-libre) | ✅ |
| forgia-rng | 261 | RNG déterministe seedé (xoshiro256++), pure data | ✦ |
| forgia-input | 110 | Leafwing PlayerAction AZERTY + InputBlockers | ✅ |
| forgia-assets | 39 | GameAssets Resource (Lock L1, whitelist xtask) | ✅ |
| forgia-genome-core | 203 | `Genome<T>` asset TOML + hot-reload (socle data-driven) | ✦ |
| forgia-game | 123 | Lib d'assemblage — wire tous les plugins | ✅ (lib du bin racine) |

### Joueur, caméra, input
| Crate | LOC | Rôle | Wired |
|---|---|---|---|
| forgia-player | 906 | KCC Rapier + caméra 1P/3P + spawn/respawn + dash | ✅ |
| forgia-camera-orbit | 314 | Caméra orbit 3P RPG (WoW-style) | ✅ |

### Combat & feel FPS
| Crate | LOC | Rôle | Wired |
|---|---|---|---|
| forgia-combat | 1500 | Gunfeel V5-F (weapons, hit-stop, recoil, hitmarker) — Health ENNEMIS | ✅ |
| forgia-damage | 280 | Health JOUEUR + DamageEvent + HitZone (⚠ dual-Health connu, M4) | ✅ |
| forgia-fps | 1801 | Orchestrator firing path + ammo + aim assist + tuning | ✅ |
| forgia-viewmodel | 1529 | Bras/arme 1P (CBaseViewModel-like), ADS | ✅ (calibration) ✦ |
| forgia-crosshair | 343 | Crosshair dynamique (spread, hit confirm) | ✦ (via fps) |
| forgia-juice-lib | 513 | recoil + hit_stop + fov_punch + camera_shake | ✦ (via fps/combat) |
| forgia-juice-screen-flash | 291 | Flash écran damage/heal/kill (egui overlay) | ✅ |
| forgia-killfeed | 551 | Kill feed + multi-kill banner | ✅ |
| forgia-enemy-nameplate | 419 | HP bar 3D billboard au-dessus des ennemis | ✅ |
| forgia-ai-arena-bot | 1017 | Bot FSM (Idle/Chase/Attack) + LOS throttlé 8 Hz | ✦ (via fps/roguelite) |

### Modes de jeu
| Crate | LOC | Rôle | Wired |
|---|---|---|---|
| forgia-mode-roguelite | 11379 | **LE jeu à shipper** : run/vagues/boss, boons, éléments, méta-shop persisté, parcours, HUD | ✅ |
| forgia-mode-fps-arena | 1335 | Arena KayKit (plus un produit ; dépendance structurelle du Roguelite : TargetCube/spawn) | ✦ |
| forgia-rpg | 3896 | Track FORGE : open world, village hex, PNJ, interactions | ✅ |
| forgia-rpg-data | 2125 | Data layer : inventory (LOCK-INV-1 80 slots), quests, loot, XP, dialogue | ✅ |

### Monde & génération
| Crate | LOC | Rôle | Wired |
|---|---|---|---|
| forgia-terrain | 8598 | Terrain procédural heightmap + LOD2 + biomes Voronoi (~30 % port V1 dormant, cf audit) | ✅ |
| forgia-streaming | 1020 | Config streaming dual-radii + budget + sensor (gen async = placeholder assumé) | ✅ |
| forgia-foliage | 982 | Végétation genome-driven (budget/frame : reverté, story-583 à refaire) | ✅ |
| forgia-water | 145 | Wrapper bevy_water + WaterTiles | ✅ |
| forgia-worldgen | 2862 | Toolbox authoring IA P0-P6 : registre, points, recettes, routes/parcelles, grammaire, bake RON | ✅ |
| forgia-stage | 3760 | Arena loader + run graph (DAG Slay-the-Spire) | ✦ (via roguelite) |
| forgia-level-presets | 465 | Palette de modules d'arène (TOML) | ✦ (via stage) |
| forgia-anchor | 430 | Anchor-points génériques (spawn/POI/boss/teleporter) | ✦ (via stage) |
| forgia-spline | 203 | Splines/paths pour IA/cinématiques | ✦ |
| forgia-prefab | 147 | Spawn helpers GLTF data-driven + sensor | ✅ (guard) |

### Villages (procgen)
| Crate | LOC | Rôle | Wired |
|---|---|---|---|
| forgia-genome-village | 430 | VillageGenome TOML typé (sert au flatten du village hex) | ✦ |
| forgia-village-loader | 837 | Pipeline genome→spawn — **DÉBRANCHÉ** (story-586), plugin encore câblé = zombie, dépose planifiée | ✅ (zombie) |
| forgia-village-generator | 866 | Procgen hamlet/village (R-tree) — consommé par le pipeline débranché | ✦ (zombie) |
| forgia-village-kit | 457 | Vocabulaire kit TOML | ✦ (zombie) |
| forgia-procgen-graph | 266 | Graphe village (nodes/edges) pure data | ✦ (zombie) |
| forgia-pcg-core | 1450 | Contrats PCG purs headless : content-spec / kit-manifest / registry-lock, solveur constructif, validateurs hard, cellules & ladder de streaming | ✦ (xtask + runtime) |
| forgia-pcg-runtime | 330 | Adapter Bevy : SpatialPlan→cellules + ordonnancement d'activation collision/nav→rendu (pas encore câblé dans l'app live) | ✦ (isolée) |

### Animation & auto-rig (différenciateur FORGE)
| Crate | LOC | Rôle | Wired |
|---|---|---|---|
| forgia-auto-rig | 2681 | Synthèse de squelette runtime (pipeline Pinocchio) | ✦ (via rpg) |
| forgia-mesh-voxelizer | 403 | Voxelisation solide (étape 1 auto-rig) | ✦ (via auto-rig) |
| forgia-medial-axis | 491 | Distance field → graphe d'axe médian (étape 2) | ✦ (via auto-rig) |
| forgia-skeleton-embedder | 929 | Embedding du template sur l'axe médian (étape 3) | ✦ (via auto-rig) |
| forgia-skeleton-template | 1285 | Templates anatomiques TOML (Humanoid, BipedLizard) | ✦ |
| forgia-rig-topology | 488 | Classification d'os par heuristiques 3D (rig-agnostic) | ✦ |
| forgia-anim-locomotion | 2695 | Locomotion procédurale (gait genome, foot IK calculé) | ✦ (via rpg) |
| forgia-ik | 297 | Two-bone IK (consommé par foot_ik) | ✦ |
| forgia-secondary-motion | 321 | Spring bones (queue/oreilles) — désactivé par défaut | ✅ |
| forgia-anim-debug | 582 | Sensors anim + health alert | ✅ |

### UI & rendu
| Crate | LOC | Rôle | Wired |
|---|---|---|---|
| forgia-ui | 674 | Menu (fond vidéo webp) + ESC handler unique + Time pause | ✅ |
| forgia-ui-lib | 3550 | style + hud + hud_ammo + pause_menu + damage_direction + dialogue + quest_journal + inventory + shop | ✅ (×7 sub-plugins) |
| forgia-postprocess | 444 | Matériaux fullscreen — **2 shaders réels (toon, outline), 43 stubs passthrough** | ✦ (toon via roguelite) |
| forgia-effects | 1857 | VFX hanabi + tracers + damage numbers (prespawn anti-freeze = TODO, roadmap M2-B5) | ✅ |
| forgia-editor | 3520 | Éditeur de scène in-game (pavé num `.`) — sélection, transform Blender, bibliothèque, aimant, persistance non destructive. Gaté `GameMode::CastleHub` (story-665) | ✅ (Hall only) |

### Audio & assets
| Crate | LOC | Rôle | Wired |
|---|---|---|---|
| forgia-audio | 137 | Fondation audio + ambiance biome | ✅ |
| forgia-asset-registry | 986 | Scan assets/ + tags par convention + Resource queryable | ✅ |
| forgia-assets-bundle | 520 | Manifests de packs content-addressed (pattern Cargo.lock) | ✦ |
| forgia-asset-cdn | 674 | Fetch + SHA256 + extract de packs CC0 (binaire `cdn`) | 🔧 CLI |

### Observabilité, QA & dev
| Crate | LOC | Rôle | Wired |
|---|---|---|---|
| forgia-observability | 4108 | ~25 sensors 1 Hz + health checks + watchdog + lag + VRAM | ✅ |
| forgia-debug | 1541 | Monitor in-game F2 + console (gate dev-tools : à faire, roadmap M2-B2) | ✅ |
| forgia-qa-core | 1464 | Bus BugReport typé + dédup (no-op sans feature qa-runtime) | ✅ (no-op) |
| forgia-qa-replay | 959 | Record/replay sessions (clavier seul — voir ADR-0004) | ✅ (no-op) |
| forgia-qa-harness | 1137 | TestApp builder + golden frames (framework cargo test) | 🔧 dev |
| forgia-qa-autopilot | 728 | SmokeBot/SoakBot (framework cargo test) | 🔧 dev |

## 4. Sensors & genomes

- **Sensors** : source de vérité = [docs/observability/SENSOR_REGISTRY.md](docs/observability/SENSOR_REGISTRY.md)
  (gardé par `cargo xtask sensor-audit`). Convention : `forgia2_<feature>.json`,
  format `{id, severity, next_step, timestamp_secs, ...}`, 1 producteur unique.
- **Genomes** : ~105 TOML (`assets/genomes/**`, `config/**`) chargés via
  `Genome<T>` (forgia-genome-core), hot-reload Shift+F12 ou mtime-watch.

## 5. Gates mécaniques (xtask)

| Gate | Protège | Statut |
|---|---|---|
| `asset-load` | Lock L1 (budget call-sites asset_server.load par fichier) | CI (job ratchets) |
| `no-scaffold` | Le cleanup 266→62 (aucune crate vide) | local |
| `sensor-audit` | Registre sensors ↔ code | local |
| `story-gate` | Anti-« DONE fictif » (claims vs git/LOC/tests) | local |
| `verify-sensors-format` | Format des sensors canoniques (nécessite JSONs runtime) | local |
| `arch-drift` | CE document ↔ Cargo.toml members | local (story-593) |

CI : `.github/workflows/ci.yml` — check (windows), clippy `-D warnings` (windows),
test per-crate (ubuntu), fmt, ratchets. Timeouts sur tous les jobs lourds.

## 6. Dettes architecturales connues (trackées)

| Dette | Source | Plan |
|---|---|---|
| Pipeline village zombie (4 crates + 2 sensors fantômes) | story-586 §Suite | dépose après validation runtime du village hex |
| Dual Health (combat=ennemis vs damage=joueur) | audit 2026-06-10 | unification ou renommage M4 |
| Player controller hors GameSet + movement hardcodé | audit | M2-B4 (player_movement.toml) |
| forgia-terrain ~30 % port V1 dormant sous allow(dead_code) | audit | feature-gate `legacy-v1` ou suppression (post-ship) |
| QA crates no-op | ADR-0004 (PROPOSED) | décision Antoine |
| 43 shaders postprocess stubs | audit | doc honnête faite ; implémenter à la demande |

## 7. Patterns de référence (hérités V1, à reproduire)

- **DAG-libre** : une crate ne dépend que du strict nécessaire (forgia-core en a 0).
- **Bridge data struct** : une lib qui a besoin de params genome sans dépendre du
  registry expose une struct plain-data remplie par le consommateur
  (ex. BiomeGenomeOverrides terrain, GroundSampler worldgen).
- **Fonction pure + system wrapper** : la logique testable headless est extraite
  (dispatch_fire_trigger, resolve_target_hit, autotiler hex…).
- **Sensor par feature** : aucun système significatif sans export JSON + severity +
  next_step actionnable (règle observability-required).
- **Budget par frame** : tout spawn/stream en masse est budgété et trié par distance
  (stream_chunks 2/frame, worldgen spawn 8/frame). Contre-exemples connus : foliage
  (reverté) et LOD2 — voir audit.
