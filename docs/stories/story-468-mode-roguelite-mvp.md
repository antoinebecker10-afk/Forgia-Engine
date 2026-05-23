# Story-468 — `forgia-mode-roguelite` MVP (3e jeu Forgia)

> **Statut** : 🟡 PLAN AJUSTÉ — audit deep 5 agents (2026-05-19) identifié 3 BLOQUANTS + 8 AJUSTER, plan corrigé ci-dessous
> **Scale BMAD** : Enterprise (>10 crates touchées, networking, gameplay loop complet)
> **Date création** : 2026-05-19
> **Cible révisée** : démo Next Fest **vertical slice SOLO-ONLY** (coop reporté post-démo)
> **Workspace** : `C:/Users/Antoi/Desktop/Forgia Rewrite` (V2)
> **Doc audit consolidé** : [docs/audit/story-468-deep-audit-2026-05-19.md](../audit/story-468-deep-audit-2026-05-19.md)

## 0. Corrections critiques post-audit (2026-05-19)

### 0.1 BLOQUANTS — résolus dans le plan

- **A2 — DamageEvent** : `BufferedEvent` (pas EntityEvent multi-observer — ordre non garanti Bevy 0.18). 3 systems `.chain()` dans `GameSet::Effects`. `EntityEvent` réservé Hit zones boss avec propagation `ChildOf`.
- **E1 — Timeline** : 12 sem = **démo vertical slice solo-only** seulement. Aucun roguelite FPS solo en <18 mois (Katanaut = 3 ans solo). Coop reporté post-démo. Go/no-go semaine 6 obligatoire.
- **B4 — i18n EN** : refacto strings FR inline → IDs `bevy_fluent` + `.ftl` par locale. Bloquant ship Steam EN. Effort ~1-2 jours, à faire avant ship démo.

### 0.2 AJUSTER — modifications plan

- **Frame budget** : `p50 < 12 ms, p99 < 16.6 ms` @ 1080p RTX 3060 (pas "avg < 6 ms" non sourcé)
- **`bevy_rapier3d 0.34`** (pas 0.33 comme CLAUDE.md actuel)
- **`lightyear_steam` officiel existe** : 0 ligne transport custom à écrire (correction estimation 3-5j R&D)
- **RNG** : `rand_xoshiro::Xoshiro256StarStar` host-authoritative, pas de float det rapier
- **Cleanup** : démarrer avec `DespawnOnExit<S>` natif, `StageScoped` custom seulement si conditionnel
- **`set_if_neq()`** partout (breaking 0.17→0.18 : `set()` re-trigger OnEnter)
- **CI** : `cargo-nextest` obligatoire, `lightyear_crossbeam` pour test coop sans Steam
- **Saves** : RON/TOML versionné + serde_flow (PAS bevy_persistent, maintenance flou)
- **Leaderboards friends-only** par défaut (StS 2 a reculé global → friends en 2026)
- **Linux/Steam Deck Verified +30j** post-Windows (+2x ventes documenté)
- **Marketing Next Fest** = +30-50h plan (Steam page sem 3-4, devlogs sem 5/8, TikTok 2/sem)
- **Voice acting humain $500-1500** semi-pro recommandé (TTS pur = risque pitch ratée)

### 0.3 Décisions strategiques préalables (avant kickoff M1)

- **V1/V2 freeze publique** : décision binaire (support critique only OU pivot V3)
- **V3 cohérence vision "YouTube du gaming"** : justifier ou pivoter pitch (roguelite premium ≠ funnel core)

### 0.4 Risk register top 5

1. Burn-out semaine 6-8 (CRITIQUE, 60-70% prob sans buffer 30%)
2. Coop netcode debug black hole (HAUT, 50% — déjà mitigé par drop coop MVP)
3. Scope creep (HAUT, 80% — feature freeze hard sem 8)
4. Marketing absent du plan (HAUT, 70% — médiane +322 wishlists sans marketing)
5. Voice acting raté tue pitch (HAUT, 40% — budget voice humain recommandé)

---

## 1. Pitch

3e jeu Forgia V2 : **roguelite FPS coop 1-3 joueurs**, hook = armes loufoques qui parlent avec gimmicks mécaniques uniques. Esthétique cartoon stylisée. Références : Gunfire Reborn (gunplay roguelite), Risk of Rain 2 (director scaling coop), High on Life (weapons-as-characters), Hadès (dialogue contextuel réactif).

Tagline : *"Joue, tire, meurs, recommence — tes armes ont leur mot à dire."*

---

## 2. Contraintes structurelles

- **Ne touche pas** `forgia-fps` ni `forgia-rpg` au-delà des extractions V6 (E1/E2) déjà en cours dans un terminal parallèle.
- **Maximise la réutilisation** : ~55 crates `[FORGIA-CORE]` consommables tels quels (cf. audit).
- **Zéro hardcode** : tout numérique en `assets/genomes/roguelite/*.toml`, hot-reload Shift+F12.
- **Patterns V2 obligatoires** : DAG-libre, GameSet chain L7, sensor JSON 1Hz, `NeedsAssetCalibrate`, `EntityEvent` pour Damage/Death/Hit (cf. PR Bevy #19647).
- **Scaling explicite** : pensé 1-3J, par_iter pour N=1000 enemies/stage, `Local<T>` buffers réutilisés, alloc 0 hot path.

---

## 3. Acceptance Criteria

### M1 — Fondations
- [ ] `AppMode::Play(Roguelite)` choix au menu principal
- [ ] `RunState` (Lobby / InRun{stage} / Boss / Defeat / Victory) — SubState de `AppMode::Play(Roguelite)`
- [ ] `StartRunEvent` / `EndRunEvent` (Message buffered)
- [ ] `RunSeed` Resource déterministe (seed déclencheur unique tout RNG dérivé)
- [ ] Sensor `forgia2_roguelite_state.json` 1Hz (run_state, stage_id, players_alive, difficulty_mult, next_step)
- [ ] Cleanup `OnExit(RunState::InRun)` complet (0 entité résiduelle, vérifié par `forgia2_entities.json`)

### M2 — Combat solo viable
- [ ] 1 biome roguelite (réutilise `forgia-terrain` + preset `RoguelitePool`)
- [ ] 3 ennemis (réutilise pattern `forgia-ai-arena-bot` + 3 variantes TOML)
- [ ] 4 armes parlantes MVP (Pépin, Bourrasque, Madame Lenoir, Boucherie) chargées via genome TOML
- [ ] `forgia-loot-tables` peuplé : drop pools weighted rarity (Common / Uncommon / Rare / Legendary)
- [ ] `forgia-equipment` peuplé : 2 slots (arme primaire + accessoire)
- [ ] Pickup pickups physiques au sol (cooldown anti-double-pick)
- [ ] Run solo linéaire 3 vagues jouable end-to-end

### M3 — Run complète + Boss
- [ ] `StageGraph` : 4 stages + 1 boss arena, choix de portail à chaque stage (2-3 sorties, modèle Hadès)
- [ ] 1 boss avec gimmicks scriptés (phase 1 + enrage phase 2)
- [ ] `DifficultyScale` per stage (modèle RoR2 director : credits accumulés × coef stage)
- [ ] `PortalEvent` + cleanup transitions (zéro leak entre stages, vérifié sensor)
- [ ] `DeathState` solo (back to Lobby + run summary)
- [ ] Run end-to-end (4 stages + boss) gagnable et perdable

### M4 — Armes parlantes + DA
- [ ] `WeaponPersonality` Component : (mood, voice_id, line_pool_id)
- [ ] `forgia-mode-roguelite::weapons::dialogue.rs` : event-driven barks (Kill / LowHp / Idle / Reload / Pickup)
- [ ] `BarkSelector` Resource avec cooldown par ligne + priority override (modèle Hadès cf. Kasavin GDC 2021)
- [ ] `forgia-audio-voicelines` peuplé (TTS placeholder ou enregistrement test)
- [ ] 4 armes × 6 lignes contextuelles = 24 barks MVP
- [ ] Crosshair custom par arme (réutilise `forgia-crosshair` genome)
- [ ] Hit-stop config par gimmick d'arme

### M5 — Coop 2J

> ⚠️ **Note story-512 (2026-05-23)** : les crates `forgia-net-lightyear`,
> `forgia-net-lobby`, `forgia-net-replication-genome` (et 4 autres `forgia-net-*`)
> ont été supprimées comme stubs vides (commit `cceb5e8`). Recréation
> intentionnelle au démarrage M5 — la workspace dep `lightyear = ...`
> reste disponible. Pas de blocage, juste un cold start des crates.

- [ ] `forgia-net-lightyear` peuplé : `LightyearPlugin` + transport custom Steam P2P
- [ ] `forgia-steam` peuplé : `SteamPlugin` (Steamworks init + lobby create/join/list)
- [ ] `forgia-net-lobby` peuplé : lobby state + UI invite Steam
- [ ] `forgia-net-replication-genome` peuplé : marqueurs `Replicated` sur Player, Enemy, PickupItem, RunState (genome-driven policy : interval Hz, interest distance)
- [ ] `RunSeed` broadcast lobby → tous clients (RNG identique côté tous)
- [ ] `DownedState` + `ReviveEvent` (1-3J revivent un coop downed en 5s)
- [ ] Listen-server : host fait tourner client+server même process
- [ ] Run coop 2J en LAN ou Steam P2P terminée end-to-end

### M6 — Polish Next Fest
- [ ] Pause menu spécifique roguelite (Time<Real>, pas Virtual)
- [ ] Run summary screen (kills, time, deaths, loot collected)
- [ ] Audio mix pass (ducking voicelines vs combat SFX)
- [ ] Tous sensors health verts en run end-to-end (`forgia2_*.json` aucun severity > ok)
- [ ] `cargo run -p xtask -- verify-sensors-format` → OK 14/14 (13 actuels + `forgia2_roguelite_state`)
- [ ] Démo jouable 15 min sans crash, sans warn ECS dans `forgia2_run.log`

---

## 4. Architecture cible — `forgia-mode-roguelite`

```
crates/forgia-mode-roguelite/
├── Cargo.toml
└── src/
    ├── lib.rs                        # ModeRogueliteCorePlugin (top-level)
    ├── run/
    │   ├── mod.rs
    │   ├── state.rs                  # RunState (SubStates Bevy 0.18)
    │   ├── lifecycle.rs              # start_run, end_run systems
    │   └── seed.rs                   # RunSeed Resource + derive(seed, stage, encounter)
    ├── level/
    │   ├── mod.rs
    │   ├── stage_graph.rs            # Stage[N] + branching choices
    │   ├── procgen.rs                # consomme forgia-procgen-graph
    │   └── portal.rs                 # PortalEvent + transition cleanup
    ├── biomes/
    │   └── biome_pool.rs             # bind 1 biome MVP via forgia-terrain
    ├── meta/
    │   ├── unlocks.rs                # MetaProgression Resource persisted
    │   └── currency.rs               # Soul/Forge currency
    ├── difficulty/
    │   └── scaling.rs                # DifficultyScale per stage × N players
    ├── coop/
    │   ├── lobby.rs                  # spawn/host/join Steam
    │   └── revive.rs                 # DownedState + ReviveEvent
    ├── weapons/
    │   ├── personality.rs            # WeaponPersonality Component
    │   ├── gimmick.rs                # WeaponGimmick trait + dispatch
    │   ├── dialogue.rs               # BarkSelector + LinePool
    │   └── genome_ext.rs             # extension WeaponGenome
    ├── sensors.rs                    # forgia2_roguelite_state.json writer 1Hz
    └── plugin.rs
```

### API publique

```rust
pub use run::{RunState, RunSeed, StartRunEvent, EndRunEvent, RunResult};
pub use level::{StageId, StageGraph, PortalEvent, StageCleared};
pub use meta::{MetaProgression, UnlockId, Currency};
pub use difficulty::DifficultyScale;
pub use coop::{RoguelitePlayer, DownedState, ReviveEvent};
pub use weapons::{WeaponPersonality, WeaponGimmick, GimmickId, BarkSelector, DialogueLine};
pub use plugin::ModeRogueliteCorePlugin;
```

### Dépendances workspace (Cargo.toml prévu)

```toml
[dependencies]
bevy = { workspace = true }
forgia-core = { workspace = true }
forgia-app-state = { workspace = true }
forgia-genome-core = { workspace = true }
forgia-damage = { workspace = true }
forgia-inventory = { workspace = true }
forgia-loot-tables = { workspace = true }
forgia-equipment = { workspace = true }
forgia-status-effects = { workspace = true }
forgia-weapon-hitscan = { workspace = true }      # post-V6 E1
forgia-weapon-projectile = { workspace = true }
forgia-viewmodel = { workspace = true }           # post-V6 E2 (renamed)
forgia-crosshair = { workspace = true }
forgia-hitmarker = { workspace = true }
forgia-juice-recoil = { workspace = true }
forgia-juice-hit-stop = { workspace = true }
forgia-juice-camera-shake = { workspace = true }
forgia-juice-fov-punch = { workspace = true }
forgia-juice-screen-flash = { workspace = true }
forgia-killfeed = { workspace = true }
forgia-enemy-nameplate = { workspace = true }
forgia-damage-numbers = { workspace = true }
forgia-ai-arena-bot = { workspace = true }
forgia-terrain = { workspace = true }
forgia-streaming = { workspace = true }
forgia-foliage = { workspace = true }
forgia-asset-registry = { workspace = true }
forgia-audio-biome = { workspace = true }
forgia-audio-voicelines = { workspace = true }
forgia-observability = { workspace = true }
forgia-net-lightyear = { workspace = true }       # M5
forgia-net-lobby = { workspace = true }           # M5
forgia-net-replication-genome = { workspace = true }  # M5
forgia-steam = { workspace = true }               # M5
serde = { workspace = true }
```

---

## 5. Crates manquants à peupler (workflow ordonné)

Tous les scaffolds existent déjà (16 LOC chacun) et sont au workspace `Cargo.toml`. Il faut juste les implémenter.

| Ordre | Crate | M | Effort estimé | Priorité |
|---|---|---|---|---|
| 1 | `forgia-loot-tables` | M2 | 1-2 jours | ⭐⭐⭐ |
| 2 | `forgia-equipment` | M2 | 1 jour | ⭐⭐⭐ |
| 3 | `forgia-mode-roguelite` (squelette M1) | M1 | 2-3 jours | ⭐⭐⭐ |
| 4 | `forgia-weapon-projectile` | M2 | 1-2 jours | ⭐⭐ |
| 5 | `forgia-status-effects` | M3 | 1-2 jours | ⭐⭐ |
| 6 | `forgia-mode-roguelite::weapons` (dialogue) | M4 | 2-3 jours | ⭐⭐⭐ |
| 7 | `forgia-audio-voicelines` | M4 | 1 jour | ⭐⭐ |
| 8 | `forgia-vfx-impact-library` | M2-M4 | 1-2 jours | ⭐⭐ |
| 9 | `forgia-vfx-decals` | polish | 1 jour | ⭐ |
| 10 | `forgia-vfx-hanabi` (wrapper anti-freeze) | M2 | 1 jour | ⭐⭐ |
| 11 | `forgia-scene` (stage transitions) | M3 | 2 jours | ⭐⭐⭐ |
| 12 | `forgia-steam` | M5 | 2-3 jours | ⭐⭐⭐ |
| 13 | `forgia-net-lightyear` (transport Steam P2P custom) | M5 | 3-5 jours (R&D) | ⭐⭐⭐ |
| 14 | `forgia-net-lobby` | M5 | 1-2 jours | ⭐⭐⭐ |
| 15 | `forgia-net-replication-genome` | M5 | 2-3 jours | ⭐⭐⭐ |
| 16 | `forgia-skill-tree` (méta-prog) | POST | 3-5 jours | ⭐ |

**Total effort impl pure** : ~30-45 jours-dev. Compatible cible 8-12 semaines solo + IA.

---

## 6. Netcode — décision argumentée (révisée post-audit Q1 2026-05-19)

### Choix : `lightyear 0.26.4` + `lightyear_steam` officiel, listen-server

**Correction audit Q1** : `lightyear_steam` existe en officiel (shipped dans le repo lightyear), avec wrapper `steamworks::networking_sockets` modern API. **Zéro ligne de transport custom à écrire.** L'angle mort de l'estimation initiale (3-5j R&D Steam transport) disparaît.

### Justification (réécrite)

| Critère | lightyear | bevy_replicon | Steam Networking raw |
|---|---|---|---|
| Maturité Bevy 0.18 | ✅ 0.26 active 2026 | ✅ 0.40.1 (2026-05-17) | ✅ stable |
| Snapshot interp + reconcile + prediction | ✅ natif | ⚠ basique | ❌ à faire main |
| **Steam transport officiel** | ✅ `lightyear_steam` shipped | ❌ aucun `bevy_replicon_steam` | ✅ natif Steam SDK direct |
| Scaffold déjà présent V2 | ✅ `forgia-net-lightyear` | ❌ rien | ✅ `forgia-steam` |
| Coop 1-3 listen-server | ✅ excellent | ✅ ok | ✅ ok |
| Argument réel pour lightyear | **Écosystème Steam mature** | Plus simple mais Steam à coder | Bas niveau, perte de features |

### Pattern proposé (révisé)

1. `forgia-steam` : lobby Steam (CreateLobby, JoinLobby) + invite via Steam overlay. Dépend `bevy-steamworks 0.16` (bundles SDK v158a).
2. `forgia-net-lightyear` : wrap `lightyear_steam` officiel + définir replication channels (reliable/ordered, unreliable/unordered) + Resources Lightyear standard.
3. Listen-server : un joueur (host) fait tourner client+server même process (pattern lightyear documenté).
4. `forgia-net-replication-genome` : marqueurs `Replicate` Component sur Player/Enemy/PickupItem/RunState. Policy genome-driven (interval Hz, interest distance, priority).
5. `RunSeed` broadcast lobby → tous clients (RNG identique côté tous via `rand_xoshiro::Xoshiro256StarStar`).

### Architecture host-authoritative + RNG seedé (révisé post-audit Q3)

- **RNG canonique** : `rand_xoshiro::Xoshiro256StarStar` (state 256-bit serializable, `jump()` pour streams indépendants par `(stage_id, encounter_idx)`).
- **Host = vérité absolue** : physics, AI, drops, procgen. Clients consomment via replication.
- **Float déterminisme rapier NON requis** : host run la physics, clients reçoivent positions interpolées. Évite la feature `enhanced-determinism` de rapier (qui désactive SIMD = perf hit).
- **Pattern Slay the Spire confirmé** : `(seed, stage_id)` derive sub-RNG par encounter. Source : [oohbleh losing-seed](https://oohbleh.github.io/losing-seed/).

### Risques + fallbacks (révisé)

- ~~**R1** : transport Steam P2P custom~~ → **résolu, lightyear_steam officiel existe.**
- **R2** : Steam host migration absente (problème DRG/RoR2 documenté). Pattern standard industrie (DRG, RoR2, Remnant 2, Roboquest tous sans host migration).
  - Mitigation : MVP accepte ce trade-off ; UI in-game "si host quitte, run termine".
- **R3** : Loot sync coop = host-authoritative drops, replicate via `LootDrop` Component.
  - Sensor `forgia_coop_drops.json` pour observabilité divergences.

### Sources canoniques (révisées post-audit)

- [docs.rs/lightyear_steam](https://docs.rs/lightyear_steam/latest/lightyear_steam/) — transport officiel SteamNetworkingSockets
- [docs.rs/lightyear](https://docs.rs/lightyear/latest/lightyear/) — 0.26.4 confirmé
- [partner.steamgames.com — ISteamNetworkingSockets](https://partner.steamgames.com/doc/api/ISteamnetworkingSockets) — modern API officiellement recommandée Valve
- [GitHub bevy_replicon](https://github.com/projectharmonia/bevy_replicon) — 0.40.1 Bevy 0.18
- [docs.rs/rand_xoshiro](https://docs.rs/rand_xoshiro/latest/rand_xoshiro/) — RNG canonique gamedev Rust
- [oohbleh.github.io losing-seed](https://oohbleh.github.io/losing-seed/) — pattern Slay the Spire `(seed, floor)` derive
- [DRG Multiplayer wiki](https://deeprockgalactic.fandom.com/wiki/Multiplayer) — pattern host-authoritative drops confirmé industrie

---

## 7. Patterns industrie intégrés

### Loot rarity (M2)
- **Diablo 3 Loot 2.0** (Mosqueira GDC 2015) : *fewer but better* + Smart Loot (affixes biaisés par classe).
- **Path of Exile** : 2 phases (rarity → tier within rarity).
- **Hearthstone pity timer** : `pity_counter` augmente P(rare) à chaque drop raté.
- Forgia : `roguelite_loot.toml` schéma `pool_id → entries[(item_id, weight, rarity, pity_factor)]`, RNG seedé `(run_seed, stage_id, encounter_idx)`.

### Reactive dialogue (M4) — pattern Kasavin Hadès GDC 2021
- Triggers : `BarkEvent { kind: Kill|LowHp|Idle|Reload|Pickup|StagerCleared, ctx }`.
- `LinePool { entries: [(text, weight, priority, cooldown_sec, conditions)] }`.
- `BarkSelector { last_played_at: HashMap<line_id, t>, current_speaker_lock }` anti-spam.
- Priority overrides cooldown si critique (Death > Idle).
- Mantra : *"What would these characters notice?"* (Kasavin).
- Sources : [Kasavin GDC 2021](https://www.gdcvault.com/play/1026975/Breathing-Life-into-Greek-Myth), [GameRant High on Life talking guns](https://gamerant.com/high-on-life-talking-guns-comedy-game-design-tutorials/).

### Director scaling (M3) — pattern RoR2
- Credits accumulés linéairement × difficulty coef.
- Director pioche enemy par cost, dépense credits par groupe (jusqu'à 4).
- Skip enemies "too cheap" pour son budget (fix bug bosses gratuits).
- Coop : `credits_per_sec *= 1 + 0.3 * (players - 1)`.
- Source : [RoR2 Wiki Directors](https://riskofrain2.fandom.com/wiki/Directors).

### Procgen structure (M3) — pattern Dead Cells/Hadès hybride
- Graph statique `Stage[N]` linéaire avec choix 2-3 sorties par nœud.
- Chaque stage = pool de room templates filtrés par biome + difficulty_budget.
- Sources : [Deepnight Dead Cells level design](https://deepnight.net/tutorial/the-level-design-of-dead-cells-a-hybrid-approach/), [Kotaku Hades less random](https://kotaku.com/hades-level-design-is-less-random-than-it-seems-1845254545).

### Bevy 0.18 idioms
- **SubStates** : `RunState` enfant de `AppMode::Play(Roguelite)` — auto-removed si parent quitte (Bevy `examples/state/sub_states.rs`).
- **EntityEvent** (PR #19647) : Damage/Death/Hit → Observer immédiat (juice hit-stop synchrone).
- **Message** : PickupEvent, StageCleared → buffered multi-system.
- **par_iter** : seuil 32 entités (Cheat Book), justifié pour AI tick / damage tick à N=1000.
- **Local<T> + .clear()** : buffers réutilisés, alloc 0 hot path.
- **DespawnOnExit<S>** ou `StageScoped<StageId>` marker custom : cleanup auto par stage.

---

## 8. Risques et mitigations

| Risque | Impact | Probabilité | Mitigation |
|---|---|---|---|
| V6 E1+E2 extraction casse forgia-fps Arena | 🔴 Bloquant | Moyen | Branche dédiée + `cargo check -p forgia-fps -p forgia-mode-fps-arena` après chaque step |
| Steam P2P transport custom lightyear absent doc | 🟡 Moyen | Élevé | R&D 3-5j M5 + fallback UDP+SDR |
| Coop 3J replication explosion bandwidth | 🟡 Moyen | Moyen | Interest mgmt lightyear Rooms + genome-driven interval Hz |
| Talking weapons gameplay frustrant si trop bavard | 🟡 Moyen | Moyen | Cooldown anti-spam strict + priority override + skip key |
| Solo dev burn-out 12 sem | 🟢 Faible-Moyen | Moyen | Milestones de 2 sem livrables indépendamment, MVP démolissable si M5 dérape |
| Hardcode / dette tech cumulée | 🟢 Faible | Moyen | Checklist post-impl obligatoire (CLAUDE.md §3 règle fondatrice) + audit qa-lead après chaque M |

---

## 9. Validation runtime (par milestone)

### M1
- [ ] Lancer Forgia → menu principal → "Roguelite" → `RunState::Lobby`
- [ ] `cat forgia2_roguelite_state.json` → `{"id":"roguelite_state","severity":"ok","run_state":"Lobby","stage":0,"players":1,"next_step":null}`
- [ ] Quitter Roguelite → `RunState` despawn auto + 0 entité `RoguelitePlayer` résiduelle

### M2
- [ ] Démarrer run → spawn dans biome → 3 ennemis présents
- [ ] Tirer Pépin → kill enemy → drop pickup → ramasser → `forgia2_inventory` mise à jour
- [ ] Mourir → back to Lobby + run summary affiché

### M3
- [ ] Run 4 stages → choix de portail à chaque transition → cleanup propre (vérifié sensor entities)
- [ ] Stage 5 → boss spawn → phase 2 enrage → kill boss → victory screen
- [ ] `forgia2_perf.json` : avg < 6ms en stage normal, < 12ms en boss arena

### M4
- [ ] Tirer Bourrasque vide → reload → bark "à court de munitions !" entendu max 1× per 8s
- [ ] Tomber à 20% HP → bark "ouille ouille" entendu max 1× per 15s
- [ ] Kill enemy → bark `Kill` 30% probabilité, cooldown 4s par arme

### M5
- [ ] Lobby Steam → invite ami → 2J connectés (`forgia2_lightyear` montre 2 clients)
- [ ] Joueur 2 voit joueur 1 bouger sans rubber-band visible (snapshot interp OK)
- [ ] Joueur 2 downed → joueur 1 revive en 5s → joueur 2 reprend contrôle
- [ ] Désync RunSeed : non, tous voient mêmes drops aux mêmes positions

### M6
- [ ] Démo 15 min sans crash, sans warn ECS, sans severity > ok dans sensors
- [ ] Pause menu OK pendant gameplay (Time<Real> contourne hit-stop)
- [ ] `xtask verify-sensors-format` → OK 14/14

---

## 10. Liens canoniques

- ROADMAP V7 : [docs/ROADMAP_CURRENT.md](../ROADMAP_CURRENT.md#v7--3e-jeu--roguelite-fps-coop-1-3j--plan-validé--prep-en-cours)
- Recherche industrie sourcée : [docs/audit/roguelite-research-2026-05-19.md](../audit/roguelite-research-2026-05-19.md)
- Genome stubs : [assets/genomes/roguelite/](../../assets/genomes/roguelite/)
- Audit réutilisation V2 : §1 du fichier recherche
- Patterns workspace : [CLAUDE.md](../../../d:/Forgia/CLAUDE.md) §7-9, [.claude/rules/concept-first.md](../../.claude/rules/concept-first.md), [.claude/rules/no-hardcode.md](../../.claude/rules/no-hardcode.md)
- DA cast 6 protagonistes : memory `project_forgia_protagonists_roster.md`
- Armes parlantes Forgés : memory `project_forges_weapons_roster.md`

---

## 11. Validation gate avant impl M1

- [x] Plan présenté à Antoine et validé (2026-05-19)
- [x] Audit réutilisation V2 livré (~80 sous-systèmes)
- [x] Recherche industrie sourcée livrée (5 questions, URLs)
- [x] Deep audit 5 agents livré (3 BLOQUANTS résolus, 8 AJUSTER intégrés)
- [x] ROADMAP V7 ajoutée + re-cadrée vertical slice solo-only
- [x] Genome TOML stubs préparés (run/weapons/enemies/loot/dialogue)
- [x] V6 E1+E2 mergée : `forgia-weapon-hitscan` (`Hitscan` Component + `TryFire`/`HitscanFired` events, design BufferedEvent conforme correction A2) + `forgia-viewmodel` (attach/calibration/fade/genome/pose, 6 fichiers) — commits `6a45c6322` + `138dcc056`
- [x] API publique confirmée stable et consommable directement par `forgia-mode-roguelite`
- [ ] **Décision Antoine V1/V2 freeze publique** (binary : support critique only OU pivot V3)
- [ ] **Décision Antoine V3 cohérence vision "YouTube du gaming"** (justifier ou pivoter pitch)
- [ ] Correction `CLAUDE.md` : `bevy_rapier3d 0.33 → 0.34`
- [ ] Fork interne préventif `bevy-steamworks` + `bevy_hanabi` (sem 1)
- [ ] GO Antoine pour démarrer M1

*Aucune ligne de code touchée tant que ce gate n'est pas complet — uniquement 2 décisions strategiques + 1 correction CLAUDE.md restent côté Antoine.*
