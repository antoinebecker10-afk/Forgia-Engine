# Forgia V2 Rewrite — ROADMAP_CURRENT

> ⚠️ **SUPERSEDED — le pilotage vivant est passé dans [`docs/ROADMAP.md`](./ROADMAP.md) (Now/Next/Later).** Ce fichier reste comme **historique** des vagues V1→V7, plus comme source d'autorité.
>
> Historique de l'état des vagues V2 et de la priorisation BMAD.
> Mise à jour à chaque livraison story ou à la commande "Memorise" (CLAUDE.md §11).
>
> **Dernière révision** : 2026-06-04 — 🎯 **PIVOT VISION** : Forgia = moteur IA-natif (créateur importe assets, l'IA construit), priorité = **SHIP le Roguelite** (FPS roguelite type Gunfire Reborn). RPG = track FORGE. Vision : [`vision/FORGIA_VISION_2026-06-04.md`](./vision/FORGIA_VISION_2026-06-04.md). Ship-audit : [`audit/roguelite-ship-readiness-2026-06-04.md`](./audit/roguelite-ship-readiness-2026-06-04.md) → **~40% MVG**. Chemin critique : 559(B)→566→564→565→569 + onboarding + honnêteté UI (~25-30j). story-566 bloquée sur décision archi read-path éco (cf SESSION_STATE).
> **Révision préc.** : 2026-05-28 — Observability overhaul (10 commits pushés) + Roguelite pivot bible v1 + Workspace cleanup 266→167 crates.
> **HEAD courant** : `c1e759d` (WIP auto-save). Session 2026-06-04 : RIEN commité. Working tree sale (autre terminal anim/rig + forgia-mode-roguelite).

---

## 📌 SYNTHÈSE 2026-05-28

| Axe | État |
|---|---|
| **V2 Rewrite vagues V1-V6** | ✅ DONE (cf. tables ci-dessous) |
| **V7 Roguelite M1 fondations** | ✅ DONE 2026-05-19 (story-470) |
| **V7 Roguelite M2 playable e2e** | ✅ DONE 2026-05-22 (3 waves + victory sensor confirmé) |
| **Workspace cleanup** | ✅ 266→167 crates -37% (2026-05-23 PR #1), puis 69→67 (audio+genome fusions 2026-05-26 non commit) |
| **Roguelite bible v1 + roadmap dédiée** | ✅ 2026-05-26 — voir [`ROADMAP_ROGUELITE.md`](./ROADMAP_ROGUELITE.md) (source de vérité Roguelite) |
| **Observability overhaul** | ✅ 2026-05-28 — 10 commits, sensor registry 64 sensors, xtask gate, forgia-debug crate, console, sensors physics/water/rpg_player/quests/inventory/npcs (couverture RPG 60→90%) |
| **HDR/Bloom pipeline** | ⚠️ revert (stories 549-551) — skybox LDR incompat → écran noir. Story-553 skybox HDR PolyHaven foundation re-add en cours (`831dc20`). |
| **Plugin wiring P0/P1** | 🟡 stories 542-545 DRAFT (double-add guard, rpg-data subplugins, orphan crates, bot raycast self-hit) |

### Prochaine cible majeure

**Roguelite Tier 1 (11 stories décomposées 528-538)** — voir [`ROADMAP_ROGUELITE.md`](./ROADMAP_ROGUELITE.md). Ordre dépendances : 528 (FPS feel) → 535 (ennemis) → 529 (boons arch) → 530 (catalogue) → 531-534 (movesets 4 armes) → 538 (polish) → 536 (boss) → 537 (méta).

Effort total estimé : **~50-54 jours solo** ≈ 10-11 semaines.

### Story-528 — FPS Feel Accessible ✅ DONE 2026-05-28 (6/7 AC, commits `8f508ff`+`2595b30`+`12cf6de`)

| AC | Statut | Notes |
|---|---|---|
| AC1 aim assist | ✅ | `forgia-fps::aim_assist` + genome hot-reload TOML strength/max_angle/distance |
| AC2 hitbox 1.2× | 🟡 | HS detection déjà via HitZoneTag(Head). Capsule physique SKIP (forgia-ai-arena-bot M autre terminal) |
| AC3 Dash "Pas de l'Apprenti" | ✅ | `forgia-player::dash` double-tap Espace 4m/0.25s, 2 charges, voiceline "Hop!" log |
| AC4 fatigue Énergie + fade | ✅ | `forgia-ui-lib/hud/energy.rs` overlay non-destructif Roguelite (player_hp.rs M préservé) |
| AC5 stars + flash | ✅ | `forgia-effects::hitmarker` extend cartoon ✨ jaunes + flash blanc 80ms |
| AC6 HS POW! + DING | ✅ | hitmarker extend pop scale 1.5→1.0 outlined noir + DING log |
| AC7 sensor `forgia2_fps_feel.json` | ✅ | `forgia-observability::fps_feel_sensor` 1Hz, 14e canonical |
| **Bonus cursor fix** | ✅ | InputBlockers OnEnter(RunState::Defeat\|Victory) → cursor libre + mouse_look bloqué |

Patterns canoniques shipés : [`reference_foundation_resource_anti_cycle`](../../../../Users/Antoi/.claude/projects/d--Forgia/memory/reference_foundation_resource_anti_cycle.md), [`reference_input_blockers_anti_cycle_pattern`](../../../../Users/Antoi/.claude/projects/d--Forgia/memory/reference_input_blockers_anti_cycle_pattern.md), [`reference_hud_overlay_non_destructive_cover_label`](../../../../Users/Antoi/.claude/projects/d--Forgia/memory/reference_hud_overlay_non_destructive_cover_label.md).

### Backlog wiring (parallèle, indépendant Roguelite)

- story-542 plugin double-add guard (P0)
- story-543 rpg-data subplugins wire (P1)
- story-544 orphan crates cleanup (P2)
- story-545 bot raycast self-hit player invincible (P1)
- story-553 skybox HDR PolyHaven (P1) — débloque re-add Bloom V2
- PR #1 cleanup vagues 1+4 — vérifier merge status

### ⚠️ Multi-terminal actif 2026-05-28

9 fichiers M dans `forgia-mode-roguelite`, `forgia-ai-arena-bot`, `forgia-asset-registry`, `forgia-game`, `forgia-village-loader`, `forgia-ui-lib`. Toucher ces crates sans coord = conflit. Cf. `.claude/rules/multi-terminal-coordination.md` §3.

---

## 🌊 Vagues — état canonique

Plan original : `docs/audit/audit-2026-05-19.md` §7. Cette table est le statut **vivant**.

### V1 — Débloquer (P0) ✅ DONE

| Item | Statut | Livré par |
|---|---|---|
| Fix `LocomotionBoneCache` fields | ✅ | session 2026-05-18 (résolu avant ma session) |
| `forgia_combat.json` producer | ✅ | session 2026-05-19 (story-457 commit `50444ba41`) |
| `forgia_health.json` producer | ✅ | session 2026-05-19 (story-457 commit `50444ba41`) |

### V2 — Discipline & traçabilité (P1) ✅ DONE

| Item | Effort | Statut | Livré par |
|---|---|---|---|
| ARCHITECTURE.md actualisé | 1h | ✅ | session 2026-05-19 (commit `1b3301b37`) |
| Sensor fusion Tier 1 (`forgia2_combat` + `forgia2_arena`) | 2h | ✅ | story-465 (commit `aae934198`) — file-based aggregator 5+2 sources |
| Code mort `WeaponData` supprimé | 30 min | ✅ | commit `1b3301b37` (Vague 2 hardcode → confirmé code mort) |
| Story-458 concept-mapping doc | 30 min | ✅ | cleanup 2026-05-19 — `docs/stories/story-458-locomotion-bone-cache-concept-mapping.md` |

**Note** : "Migration weapon balance → genome TOML" du plan original a été RÉSOLU par suppression du code mort `WeaponData` (audit 0 call-site externe), pas par migration. Cohérent avec `.claude/rules/no-speculative-fix.md`.

### V3 — Modernisation Bevy 0.18 (P1) ✅ DONE (avec SKIPs documentés)

| Item | Statut | Livré par |
|---|---|---|
| Required Components Player/TargetCube/NameplateRoot | ✅ | story-461 (commit `9b74b08bb`) |
| Wave bots ChildOf relationships | ✅ | story-463 (commit `fb26eeb89`) |
| Observers death/pickup/damage | ✅ partial | story-466 (DeathEvent only — DamageEvent + CombatHitEvent SKIP justifié 8 consumers cascade) |
| RpgOrbitCamera vs PanCamera first-party | ✅ FALSIFIÉ | audit Vague 3 — FreeCamera/PanCamera gameplay n'existent pas en 0.18.1 |

Audit doc : `docs/audit/vague-3-bevy-018-idioms-2026-05-19.md` (commit `6d1836308` + correction `ca5c3b99a`).

### V4 — Tech debt P1-P2 ✅ DONE

| Item | Statut | Livré par |
|---|---|---|
| Fix tests melee TimePlugin advance_by trap | ✅ | commit `50444ba41` (helper `app_with_manual_time()`) |
| Fix test weapons cycle off-by-one | ✅ | commit `50444ba41` (`cycles_full` + `ARENA_V1_WEAPONS.len()`) |
| `tech-debt-plan-2026-05-18.md` obsolète à 80 % | ✅ | cleanup 2026-05-19 — déplacé vers `docs/archive/` avec note ARCHIVÉ |

### V5 — Phase 5 sensors complet (P2) ⚠️ Session A DONE, B+C pending

Plan Phase 5a livré : `docs/audit/vague-5-sensors-fusion-plan-2026-05-19.md` (commit `9c486bd86`).

**Cible révisée : 13 sensors `forgia2_*.json` canoniques** (vs 12 initial — séparation `health` cross-mode + `rpg_health` détail RPG valuable).

#### Session A (Tier 0 renames + Tier 1 fusion + xtask gate) ✅ DONE

| Sensor | Statut | Commit |
|---|---|---|
| `forgia2_health.json` | ✅ 149B, conforme | `380aa2f10` rename |
| `forgia2_rpg_health.json` | ✅ 1.3K, format fix | `380aa2f10` rename + `67c20855f` add `id` + rename `overall_severity` → `severity` |
| `forgia2_arena.json` | ✅ 410B, fusion 2 sources | `aae934198` story-465 (avant ma session) |
| `forgia2_combat.json` | ✅ 7.2K, fusion 5 sources | `aae934198` story-465 (avant ma session) |
| `xtask verify-sensors-format` | ✅ validates 4/4 | `380aa2f10` + `67c20855f` |

`cargo run -p xtask -- verify-sensors-format` → **OK (4/4 canonical sensors validated)**.

#### Session B (Tier 2 producers perf/entities/memory) ✅ DONE 2026-05-19

Story-467. Effort réel ~3 h (vs 6 h estimé — research a écarté 2 pièges).

| Sensor | Source | Status runtime |
|---|---|---|
| `forgia2_perf.json` | `FrameTimeDiagnosticsPlugin` avg/min/max + FPS smoothed | ✅ avg=3.4ms FPS=414 samples=120 |
| `forgia2_entities.json` | `EntityCountDiagnosticsPlugin` + 4 Query markers | ✅ total=2646 (player=1, bots=3, nameplates=4) |
| `forgia2_memory.json` | `sysinfo` RAM (cooldown 5s), VRAM stub `"N/A"` | ✅ 1414 MB RAM, severity=ok |

- 12 tests purs `severity_for_*` verts
- 3 nouveaux fichiers sources `crates/forgia-observability/src/{perf,entities,memory}_sensor.rs`
- 3 Cargo deps ajoutées (`forgia-player`, `forgia-ai-arena-bot`, `forgia-enemy-nameplate`) + `sysinfo = "0.32"`
- xtask `verify-sensors-format` → **OK (7/7 canonical sensors validated)**
- `default_expected_sensors` étendu (CHK-5 ne flood pas)
- VRAM = stub honnête `"N/A — wgpu adapter telemetry custom needed"` (assumé)

#### Session C (lifecycle/watchdog/audio/input + sensor_health) ✅ DONE 2026-05-19 (story-469)

Effort réel ~3 h (vs 6 h estimé — research bevy-specialist a écarté pièges Bevy 0.18 `On<Add, C>` syntax + `EventReader` → `MessageReader` rename).

| Sensor | Source | Status |
|---|---|---|
| `forgia2_lifecycle.json` | 7 Observers `On<Add\|Remove\|Insert, C>` (Player, TargetCube, NameplateRoot, ArenaBot) | ✅ compile |
| `forgia2_watchdog.json` | `GameTickCounter` Resource + First schedule + lag >50ms detection | ✅ compile |
| `forgia2_audio.json` | `Assets<AudioInstance>::iter()` filter Playing + `BiomeAmbientState.current_biome()` | ✅ compile |
| `forgia2_input.json` | `MessageReader<KeyboardInput>` + `ActionState<PlayerAction>::get_just_pressed()` | ✅ compile |
| `forgia2_sensor_health.json` | Meta — lit timestamps des 12 forgia2_*.json, CHK-5 canonisé | ✅ compile |

- **19 nouveaux tests purs** verts (severity_for_* + lifecycle/tick counter defaults), **66 total forgia-observability**
- 5 nouveaux fichiers `crates/forgia-observability/src/{lifecycle,watchdog,audio,input,sensor_health}_sensor.rs`
- 5 Cargo deps ajoutées (`forgia-mode-fps-arena`, `forgia-audio-biome`, `forgia-input`, `bevy_kira_audio`, `leafwing-input-manager`)
- 1-line pub accessor `BiomeAmbientState::current_biome()` ajouté à `forgia-audio-biome`
- xtask `CANONICAL_SENSORS` étendu 7 → 12, `default_expected_sensors` +5 (CHK-5 ne flood pas)
- **Bevy 0.18 critical findings** :
  - `EventReader` → `MessageReader` (KeyboardInput now `#[derive(Message)]`)
  - `Trigger<OnAdd, C>` → `On<Add, C>` (PR #19596)
  - `EntityCountDiagnosticsPlugin::default()` requis (struct avec field)

**✅ Smoke test runtime DÉBLOQUÉ** par fix commit `138dcc056` (10 lignes `init_asset` + `register_asset_loader` dans `ForgiaViewmodelPlugin::build()`, pattern miroir forgia-damage:202). 12/12 canonical confirmés runtime.

### V6 — Crates extraction (P2) ✅ DONE-partiel 2026-05-19

| Étape | Cible | Statut | Commit |
|---|---|---|---|
| E1 | `forgia-weapon-hitscan` (scaffold design alternatif Component `Hitscan` + `TryFire` event, 148 LOC) | ✅ scaffold présent | (terminal // user) |
| E2 | `forgia-viewmodel` (extraction Tier 2B) — `WeaponViewmodel`, `WeaponModelAssets`, `ads`, `scope_glass`, `viewmodel_debug`, `ViewmodelGenome*`, `attach/switch/auto-scale` | ✅ DONE | `6a45c6322` extraction + `138dcc056` fix init_asset |

**Note honnête** : Le scaffold `forgia-weapon-hitscan` (148 LOC) adopte un design *différent* du plan original ROADMAP (`LeftMouseState` + `dispatch_fire_trigger` + `fire_weapon_minimal` restent dans `forgia-fps`). Le scaffold expose plutôt `Hitscan` Component + `TryFire`/`HitscanFired` events. Migration complète des helpers `LeftMouseState` etc. → reportée à V7 ou plus tard si nécessité concrète (pas bloquant pour V7 roguelite qui peut consommer `Hitscan` Component directement).

Zéro breaking change FPS Arena. V7 roguelite peut consommer `forgia-weapon-hitscan` (firing déclaratif) + `forgia-viewmodel` (1P render) directement.

### V7 — 3e jeu : Roguelite FPS Coop 🟢 M1 FONDATIONS DONE 2026-05-19 (story-470)

**M1 livré** (commit pending) — scaffold MVP 3-4 h :
- `GameMode::Roguelite` variant ajouté
- `RunState` SubStates `#[source(GameMode = GameMode::Roguelite)]` (Lobby/InRun{stage}/Boss{stage}/Defeat/Victory)
- `StartRunEvent` + `EndRunEvent` (`#[derive(Message)]` Bevy 0.18)
- `RunSeed` Resource déterministe `Xoshiro256StarStar` (`stage_seed(stage)` + `encounter_seed(stage, idx)`)
- `forgia2_roguelite_state.json` sensor 1Hz cross-mode
- Bouton menu 🎲 Roguelite Run → `StartRunEvent { seed: None }`
- `RogueliteRunMarker` Component stub (cleanup OnExit géré par terminal // dédié)

**Validation** : `cargo check --workspace` ✅ • clippy `-D warnings` ✅ 0 • 6 tests verts (run_seed déterministe + variants distincts + default Lobby) • smoke test runtime forgia2_roguelite_state.json écrit • **`xtask verify-sensors-format` → OK 13/13** (cible V5+V7 finale atteinte).

Combat solo viable (M2) = scope future session : 1 biome roguelite + 3 ennemis + 4 armes parlantes + loot/equipment + 3 vagues end-to-end.

---

#### Plan original V7 (legacy avant M1)

**Audit deep 5 agents** : [docs/audit/Story-469-deep-audit-2026-05-19.md](audit/Story-469-deep-audit-2026-05-19.md) — 3 BLOQUANTS + 8 AJUSTER identifiés, corrigés ci-dessous.

**Cible révisée Next Fest oct 2026** : **démo vertical slice SOLO-ONLY** (coop 2-3J reporté post-démo). Aucun roguelite FPS solo n'a shipped 1.0 en <18 mois (Katanaut solo = 3 ans, Roboquest 4 devs = 5 ans). 12 sem = vertical slice marketing pour wishlists + Steam page, pas ship 1.0.

**Décisions préalables avant kickoff M1** :

- V1/V2 freeze publique (support critique only) OU pivot V3
- V3 cohérence vision "YouTube du gaming" : justifier ou pivoter pitch
- CLAUDE.md correction : `bevy_rapier3d 0.33 → 0.34`
- Fork interne `bevy-steamworks` + `bevy_hanabi` semaine 1 (dep solo mainteneur)

Hook : armes loufoques qui parlent avec gimmicks mécaniques uniques. Refs : Gunfire Reborn × Risk of Rain 2 × High on Life × Hadès (dialogue reactif).

**Cadrage** :
- Crate cible : `forgia-mode-roguelite` (scaffold 16 LOC déjà présent, à peupler)
- Bevy 0.18.1, AppMode étendu `Play(Roguelite)`
- Coop 1-3J listen-server (un host = client+server même process)
- Tout genome-driven (`assets/genomes/roguelite/*.toml`) — zéro hardcode
- Patterns workspace respectés : DAG-libre, sensor JSON 1Hz, GameSet L7, NeedsAssetCalibrate, EntityEvent pour Damage/Death/Hit

**Audit réutilisation V2** (cf. `docs/audit/roguelite-research-2026-05-19.md` §1) :
- ~55 sous-systèmes **[FORGIA-CORE]** consommables tels quels (damage, inventory, crosshair, juice×5, killfeed, hitmarker, ai-arena-bot, terrain, streaming, foliage, asset-registry, genome-core, observability)
- ~10 **[À-EXTRAIRE]** dont E1/E2 (V6) déjà en cours + E3-E8 backlog
- ~15 **[MANQUANT]** dont 100% des scaffolds 16 LOC sont déjà créés et au workspace : `forgia-loot-tables`, `forgia-equipment`, `forgia-status-effects`, `forgia-skill-tree`, `forgia-weapon-projectile`, `forgia-mode-roguelite`, `forgia-net-lightyear`, `forgia-net-replication-genome`, `forgia-net-lobby`, `forgia-vfx-impact-library`, `forgia-vfx-decals`, `forgia-vfx-hanabi`, `forgia-scene`, `forgia-steam`

**Roadmap MVP — 6 milestones × ~2 sem (cible Next Fest, 8-12 semaines)** :

| M | Nom | Statut | Pré-requis |
|---|---|---|---|
| M1 | Fondations (RunState + AppMode::Roguelite + sensor) | ⏸️ Pending | V6 E1+E2 merge |
| M2 | Combat solo viable (1 biome + 3 ennemis + 4 armes + loot-tables peuplé) | ⏸️ Pending | M1 |
| M3 | Run complète + Boss (StageGraph + 1 boss + DifficultyScale) | ⏸️ Pending | M2 |
| M4 | Armes parlantes + DA (`WeaponPersonality` + dialogue.rs + voicelines) | ⏸️ Pending | M2 |
| M5 | Coop 2J (lightyear + Steam P2P transport + listen-server) | ⏸️ Pending | M3+M4 |
| M6 | Polish Next Fest (pause menu, summary, audio mix) | ⏸️ Pending | M5 |

**Story dédiée** : [Story-469-mode-roguelite-mvp.md](stories/Story-469-mode-roguelite-mvp.md) (enterprise scale).

**Recherche industrie sourcée** : [docs/audit/roguelite-research-2026-05-19.md](audit/roguelite-research-2026-05-19.md) — 5 questions (netcode coop, loot rarity, weapons-as-characters, procgen runs, Bevy 0.18 patterns), URLs vérifiables.

**Décision netcode** : `lightyear 0.26.4` + transport custom Steam P2P (via `bevy-steamworks 0.16`), modèle listen-server. Fallback UDP direct + Steam Datagram Relay si Steam P2P custom transport bloque. Cf. Story-469 §Netcode.

**Genome TOML stubs préparés** (data-driven, hot-reload Shift+F12) :
- `assets/genomes/roguelite/roguelite_run.toml` — RunSeed, stages, difficulty scaling
- `assets/genomes/roguelite/roguelite_weapons.toml` — 4 armes parlantes MVP (Pépin, Bourrasque, Madame Lenoir, Boucherie)
- `assets/genomes/roguelite/roguelite_enemies.toml` — 3 ennemis + 1 boss
- `assets/genomes/roguelite/roguelite_loot.toml` — drop pools rarity (Diablo 3 Loot 2.0 + Hearthstone pity timer)
- `assets/genomes/roguelite/roguelite_dialogue.toml` — barks contextuels (Hadès pattern : event triggers + pool + cooldown anti-spam)

---

## 🚀 Hors plan vagues — historique commits session 2026-05-19

### Session précédente (avant ma session)

| Story | Type | Commit |
|---|---|---|
| story-464 LOS state gating (bot AI) | feat(ai) | `20fefe9d7` |
| Nameplate permanent + face-cam + cartoon | feat(ui) | `1a7ce3eff` |
| 3 fixes audit qa-lead (BUG-464-01/02/03) | fix(audit) | `9d2baeaae` |
| story-465 sensor fusion Tier 1 | feat(observability) | `aae934198` |
| story-466 DeathEvent → Observer | refactor(ecs) | `f3bd4fdf3` |
| SESSION_STATE.md snapshot | docs | `51c084925` |

### Ma session 2026-05-19 (15 commits)

| Commit | Description |
|---|---|
| `50444ba41` | feat(audit+sensors): Vague 1+4 — sensors combat+health + tests fixes |
| `eb3c732b0` | feat(arena): story-448+449+453 — colliders + auto-calibrate + reset |
| `17634a5d4` | feat(terrain+rig+rpg): wave 5 LOD2 + auto-rig + Rex 3P |
| `d16ead641` | docs(stories): story-452 + 453-rpg-monitor docs orphelins |
| `dc740e133` | wip(hit-feedback): story-456 scaffold — forgia-enemy-nameplate crate |
| `c881e1982` | docs(roadmap): rendering pipeline 2026-05-19 |
| `bf1144842` | assets(packs): add Kenney + Quaternius CC0 (~181 MB) |
| `1b3301b37` | docs(audit)+refactor(combat): Vague 2 — ARCHITECTURE.md + code mort weapons |
| `6d1836308` | docs(audit): Vague 3 — Bevy 0.18 idioms audit + correction FreeCamera |
| `fb26eeb89` | refactor(arena): story-463 — wave bots .with_children → ChildOf |
| `9b74b08bb` | refactor(ecs): story-461 — Required Components Player + TargetCube + NameplateRoot |
| `ca5c3b99a` | docs(audit): Vague 3 — story-462 SKIP justifié (CombatHitEvent 8 consumers) |
| `9c486bd86` | docs(audit): Vague 5 Phase 5a — plan fusion sensors 29→13 |
| `380aa2f10` | refactor(sensors): Vague 5 Session A Étape 1 — renames forgia2_* + xtask verify |
| `67c20855f` | fix(sensors): Vague 5 Session A — format forgia2_rpg_health conforme + xtask étend canonical |

### Audits livrés (3 documents)

- `docs/audit/audit-2026-05-19.md` (~430 lignes) — forensic V2 général, 258 crates, 6 vagues
- `docs/audit/vague-3-bevy-018-idioms-2026-05-19.md` (~250 lignes) — audit Bevy 0.18 + corrections honnêtes
- `docs/audit/vague-5-sensors-fusion-plan-2026-05-19.md` (~264 lignes) — plan Phase 5b 18-22h en 3 sessions

---

## 🔥 Prochaine session — priorités par ROI

### Option A — V5 Session B ✅ DONE 2026-05-19 (story-467)

3 sensors perf/entities/memory livrés, 7/13 canonical atteints. Effort réel 3h.

### Option B — V5 Session C ✅ DONE 2026-05-19 (story-469)

Voir §V5 Session C ci-dessus. **V5 complète à 12/13**. Runtime validation pending V6 stable.

### Option C — Vague 1 story-456 hit feedback (Enterprise 10h+) **AAA gameplay impact**

Layered shield/armor (Apex tiers) + headshot/bodyshot routing + audio cue distinct. Fix au passage :
- Bug nameplate HP fill anchor center (commentaire code dit lui-même qu'il faut anchor left)
- Race ChildOf orphelin (~1 warn par kill, check `target.exists()`)

### Option D — Git LFS migration (Standard 2h)

2.9 GB packs binaires tracked → `git lfs migrate import --include="*.glb"`. 0 risque code, hygiène repo. Indépendant.

### Option E — Cleanup ROADMAP + archives tech-debt-plan (Quick 30min) ✅ DONE 2026-05-19

Tech-debt-plan archivé dans `docs/archive/` avec note ARCHIVÉ. Story-458 livrée. ROADMAP V2 → 100 %.

---

## 🚨 Backlog identifié (à ne pas oublier)

- **BUG-464-04 cosmétique** : `ArenaBot::default()` hardcode `los_lost_grace_left: 2.0` au lieu de lire TacticalTuning. Diverge si genome change.
- **Race ChildOf orphelin** : ~1 warn par kill (spawn nameplate ~4ms après despawn bot). Bevy auto-corrige. Fix futur = check `target.exists()` avant spawn dans `forgia-enemy-nameplate::spawn_or_refresh_on_hit`.
- **Nameplate HP fill anchor center** : `forgia-enemy-nameplate/src/lib.rs:175` — commentaire code dit anchor left mais code fait scale.x = frac sans translation décalage. Visible quand HP descend.
- **WIP story-456** layered hit feedback : option C ci-dessus.
- **6 hardcodes weapons.rs:110-141** : SUPPRIMÉS comme code mort (commit `1b3301b37`), pas migrés. À retraiter quand Tier 2A `forgia-weapon-hitscan` extraction reprise (V6).

---

## 📋 Validation runtime requise (avant Session B/C)

Validations Session A passées **2026-05-19 fin session** :

1. ✅ `forgia2_health.json` + `forgia2_rpg_health.json` écrits 1Hz format conforme
2. ✅ `forgia2_arena.json` (410B) + `forgia2_combat.json` (7.2K) aggregators fonctionnels
3. ✅ Anciens `forgia_health.json` + `forgia_rpg_health.json` supprimés, plus écrits
4. ✅ `cargo run -p xtask -- verify-sensors-format` → OK (4/4 canonical sensors validated)
5. ✅ `forgia2_run.log` : 0 ERROR / 0 panic / 0 CHK-5 flood (62K logs propres)

---

## 📎 Liens canoniques

- Plan original vagues : [docs/audit/audit-2026-05-19.md](audit/audit-2026-05-19.md) §7
- Audit Bevy 0.18 : [docs/audit/vague-3-bevy-018-idioms-2026-05-19.md](audit/vague-3-bevy-018-idioms-2026-05-19.md)
- Plan Vague 5 Phase 5b : [docs/audit/vague-5-sensors-fusion-plan-2026-05-19.md](audit/vague-5-sensors-fusion-plan-2026-05-19.md)
- Architecture : [ARCHITECTURE.md](../ARCHITECTURE.md)
- Stories actives : [stories/](stories/)
- Concept-first règle : [.claude/rules/concept-first.md](../.claude/rules/concept-first.md)
- No-speculative-fix règle : [.claude/rules/no-speculative-fix.md](../.claude/rules/no-speculative-fix.md)

*Source de vérité unique. Si conflit avec SESSION_STATE.md, ce fichier prime.*
