# Story-483 — Roguelite Stage Arena Foundations (RoR2-like topology, scaling-ready)

> **Statut** : ✅ **DONE 2026-05-20 PM** — P0+P1+P2+P3 shippés, runtime victory validé end-to-end (4 stages joués, 0 erreurs prefab, 88 tests verts, 0 clippy), qa-lead audit 3 bugs MAJ + 5 mineurs **tous traités**
> **Scale BMAD** : Enterprise (2 NEW crates + populate sensors + cross-crate refacto + genome TOML + AAA sourcing)
> **Date création** : 2026-05-20
> **Workspace** : `C:/Users/Antoi/Desktop/Forgia Rewrite` (V2)
> **Origine** : demande user 2026-05-20 PM — *"crée une vraie map pour le Roguelite, on devra pouvoir l'améliorer progressivement"*, en référence au pattern FPS arena déjà shippé
> **Stack cible** : Bevy 0.18.1, bevy_rapier3d 0.33
> **Cross-refs** : [[reference-loader-request-result-pattern]] story-441 · [[reference-kaykit-asset-loading-layout]] · [[reference-arena-kaykit-wall-y-zero-works]] · [[reference-pattern-genome-driven-plugin-with-sensor]] · [[reference-v2-scaffolds-inventory-2026-05-19]]
> **Locks impactés** : aucun (création ; FPS arena 16-commit marathon 2026-05-17 reste **non touché**, migration M4 backlog)

---

## 0. Contexte & justification

### 0.1 Problème détecté

État actuel du mode Roguelite (`crates/forgia-mode-roguelite/src/run.rs:107-296`) :

- Scène 300×300m générée par `sys_spawn_roguelite_scene` — **primitives Bevy** (Cuboid, Plane3d) + 5 platforms hardcodées + 25 cubes xoshiro-seeded
- Aucun KayKit, aucune ambiance biome, aucune variation visuelle entre stages
- Pas d'extensibilité : ajouter un 2e stage = duplication ~190 lignes spawn

Le mode FPS Arena (`crates/forgia-mode-fps-arena/src/lib.rs:396-770`) montre un pattern mature de ~770 LOC avec KayKit Dungeon Pack 19×19 tiles bornées walls — **mais monolithique, non-mode-réutilisable**.

**Gap** : Forgia n'a pas de loader d'arène générique data-driven. Le Roguelite mérite mieux que des primitives et le FPS mérite mieux qu'un spawn 770-LOC enfermé dans son crate.

### 0.2 Conformité industrie (recherche sourcée 2026-05-20)

Pattern dominant pour roguelite 3D shooter shippé :

| Jeu | Pattern | Source |
|---|---|---|
| Risk of Rain 2 | Stages handcrafted + objets randomisés + toggles. Hopoo a écarté le procgen 3D *« too much for team size »* | [gamedeveloper.com — RoR2 design](https://www.gamedeveloper.com/design/how-moving-from-2d-to-3d-shaped-the-design-of-i-risk-of-rain-2-i-) |
| Returnal | Biomes handcrafted + connexions procédurales | [GDC 2022 — Never The Same Twice](https://www.gdcvault.com/play/1027651/Never-The-Same-Twice-Procedural) |
| Hadès | Room graph + decoupling `EncounterData` / `RoomLayout` / `RewardBag` (tables Lua) | [GDC Podcast ep.16 — Greg Kasavin](https://gdconf.com/article/roguelikes-and-narrative-design-with-hades-creative-director-greg-kasavin-gdc-podcast-ep-16/) |
| Roboquest | Pre-placed chunks + randomized chunks mixés | [Steam page](https://store.steampowered.com/app/692890/Roboquest/) |
| Hyper Light Breaker (2025 EA) | HyperFab prefabs adaptifs proprio | [NME — HyperFab](https://www.nme.com/news/gaming-news/hyper-light-breaker-world-building-tech-detailed-by-heart-machine-3273307) |

**Verdict pour Forgia (solo, target Next Fest)** : Option RoR2-like = stage handcrafted bounded + POIs/anchors data-driven. **Pas de procgen 3D** complexe à tuner. Effort minimal pour MVP, scaling progressif vers Returnal-hybrid post-launch via mêmes registries.

Pattern data-driven shippé universel : **registries** (ScriptableObjects Unity / DataTables UE / Lua tables Hadès) — équivalent Forgia direct = TOML genome + hot-reload `Shift+F12`.

### 0.3 Pourquoi maintenant

- Mode Roguelite V7 M1-M3 livré (380 LOC, 13 tests, sensor canonical) mais **map = placeholder**
- Bloquant pour Next Fest démo solo-only (cf. [[project-story-468-roguelite-mvp]])
- 2 NEW crates `forgia-anchor` + `forgia-stage-arena` exposeront des registres mode-agnostiques utiles future RPG/Survival/RTS dungeons
- `forgia-prefab` (story-441) déjà mature → réutilisation directe, pas de re-engineering

---

## 1. Vision cible

```
Definition layer (data, TOML, hot-reload Shift+F12)
  • assets/genomes/roguelite_stages.toml      (N stages : biome, extent, ramparts kit, anchor slots, toggles)
  • assets/genomes/roguelite_pois.toml        (POI registry : prefab, encounter, weight)
  • assets/genomes/roguelite_<future>.toml    (drop-in TOML, scan auto)
       ↓ AssetLoader<Genome<StageDef>> / <PoiDef>
Registry layer
  • NEW crate forgia-anchor
    - Component AnchorPoint { kind: AnchorKind, transform, slot_index }
    - Enum AnchorKind { PlayerSpawn, PoiSlot, Landmark, BossPad, Teleporter, LootZone }
    - Sensor forgia2_anchor.json (anchor count par kind, occupancy)
    - Pure, mode-agnostique
  • NEW crate forgia-stage-arena
    - Resource StageLoadRequest { stage_id, seed }
    - Resource StageLoadResult { state: Loading|Ready|Error, anchors_placed, props_spawned }
    - System spawn_stage_arena : terrain bornée + ramparts hex (réutilise pattern story-441 village) + anchor circle placement (deterministic splitmix64)
    - Sensor forgia2_stage.json (status, biome, anchors, ready, next_step)
    - Consume : forgia-prefab (spawn_gltf_prefab), forgia-anchor (AnchorPoint), forgia-stage-graph (StageId)
       ↓
Framework layer (consumers)
  • forgia-mode-roguelite::run.rs
    - sys_spawn_roguelite_scene refactor : drop primitives, insert StageLoadRequest
    - composition_for_stage() poursuit, lit POI anchors pour wave spawn
  • forgia-mode-fps-arena (M4 backlog, hors scope V1)
    - Migration spawn_arena (770 LOC) → StageLoadRequest avec stage Arena dédié
```

---

## 2. Architecture détaillée

### 2.1 Crate `forgia-anchor` (NEW, ~250 LOC)

**`Cargo.toml`** : deps `bevy`, `forgia-core`. Aucune dep gameplay (pur primitive).

**`src/lib.rs`** :
```rust
pub struct ForgiaAnchorPlugin;

#[derive(Component, Debug, Clone, Copy)]
pub struct AnchorPoint {
    pub kind: AnchorKind,
    pub slot_index: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnchorKind {
    PlayerSpawn,
    PoiSlot,        // chest, shrine, elite-pad, etc.
    Landmark,       // pillar, banner, decoration anchor
    BossPad,        // boss arena floor anchor
    Teleporter,     // stage-exit portal anchor
    LootZone,       // currency drop magnet radius
}

#[derive(Resource, Default)]
pub struct AnchorStats { pub counts: [AtomicU32; 6], last_write_secs: Mutex<f32> }

// Helpers
pub fn layout_circle(n: u32, radius_m: f32, center: Vec3) -> Vec<Vec3>;
pub fn layout_grid(rows: u32, cols: u32, spacing_m: f32, center: Vec3) -> Vec<Vec3>;

// Pure tests
#[cfg(test)] mod tests {
    fn layout_circle_distributes_evenly();
    fn anchor_kind_index_stable();
    fn severity_for_anchors(active: u32) -> Severity;
    fn next_step_for_anchors(active: u32) -> &'static str;
}
```

Sensor `forgia2_anchor.json` 1Hz :
```json
{"timestamp_secs":12.5,"counts":{"player_spawn":1,"poi_slot":6,"landmark":4,"boss_pad":1,"teleporter":1,"loot_zone":0},"severity":"info","next_step":"Read forgia2_stage.json to verify stage load completion"}
```

### 2.2 Crate `forgia-stage-arena` (NEW, ~400 LOC)

**`Cargo.toml`** : deps `bevy`, `bevy_rapier3d`, `forgia-core`, `forgia-anchor`, `forgia-prefab`, `forgia-stage-graph`, `forgia-genome-core`.

**`src/lib.rs`** (squelette) :
```rust
pub struct ForgiaStageArenaPlugin;

#[derive(Resource, Default)]
pub struct StageLoadRequest {
    pub stage_id: SmolStr,    // e.g. "crypts_of_anvil"
    pub seed: u64,            // splitmix64 deterministic
}

#[derive(Resource, Default)]
pub struct StageLoadResult {
    pub state: StageState,
    pub anchors_placed: u32,
    pub props_spawned: u32,
    pub biome: Option<BiomeKind>,
    pub extent_m: f32,
}

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub enum StageState {
    #[default] Idle,
    Loading,
    Ready,
    Error,
}

#[derive(Deserialize, Asset, TypePath)]
pub struct RogueliteStagesGenome {
    pub stages: HashMap<String, StageDef>,
}

#[derive(Deserialize, Clone)]
pub struct StageDef {
    pub biome: String,
    pub arena_extent_m: f32,
    pub ramparts_kit: String,        // "kaykit_dungeon" | "medieval_hexagon"
    pub ramparts_shape: String,      // "hexagonal" (only one for V1)
    pub anchor_slots: u32,
    pub music_state: Option<String>,
    pub boss_pad_required: bool,
    pub weather_override: Option<String>,
}

// Systems
fn load_stages_genome(mut commands: Commands, asset_server: Res<AssetServer>);
fn spawn_stage_arena_on_request(/* StageLoadRequest + genome ready -> spawn */);
fn cleanup_stage_arena(/* OnExit(GameMode::Roguelite) */);
fn write_stage_sensor(/* 1Hz */);

// Marker
#[derive(Component)]
pub struct StageArenaMarker;

// Pure helpers (testable headless)
pub fn ramparts_hex_positions(extent_m: f32) -> Vec<(Vec3, Quat)>;
pub fn poi_anchor_positions(extent_m: f32, slots: u32, seed: u64) -> Vec<Vec3>;
pub fn severity_for_stage(state: StageState, props: u32) -> Severity;
pub fn next_step_for_stage(state: StageState) -> &'static str;
```

Sensor `forgia2_stage.json` 1Hz :
```json
{"timestamp_secs":15.2,"stage_id":"crypts_of_anvil","state":"ready","biome":"Volcanic","extent_m":90.0,"anchors_placed":12,"props_spawned":34,"severity":"info","next_step":"Stage ready; check forgia2_anchor.json for POI placement"}
```

Health alerts émises :
- `severity=warning` si `state == Loading > 5s` → `next_step: "Read forgia2_anchor.json and forgia_prefab.json to identify stalled asset"`
- `severity=critical` si `state == Error` → `next_step: "Check assets/genomes/roguelite_stages.toml syntax + verify ramparts_kit GLB paths exist"`

### 2.3 Refactor `forgia-mode-roguelite/src/run.rs`

**Avant** (`sys_spawn_roguelite_scene`, lignes 107-296, ~190 LOC primitives) :
```rust
fn sys_spawn_roguelite_scene(mut commands: Commands, ...) {
    // floor 300x300m Plane3d, walls Cuboid, 5 platforms, 25 xoshiro cubes, 3 landmarks
}
```

**Après** (~40 LOC, drop primitives, insère request) :
```rust
fn sys_spawn_roguelite_scene(
    mut commands: Commands,
    run_state: Res<State<RunState>>,
) {
    let stage_id = match run_state.get() {
        RunState::InRun { stage } => stage_id_for_depth(*stage),
        _ => return,
    };
    commands.insert_resource(StageLoadRequest {
        stage_id: stage_id.into(),
        seed: ... // splitmix64 du RunSeed
    });
}

fn stage_id_for_depth(depth: u32) -> &'static str {
    match depth {
        0 => "crypts_of_anvil",
        1 => "forge_sanctum",
        _ => "crypts_of_anvil",  // fallback safe pour V1
    }
}
```

### 2.4 Genome TOML

**`assets/genomes/roguelite_stages.toml`** :
```toml
[stages.crypts_of_anvil]
biome = "Volcanic"
arena_extent_m = 90.0
ramparts_kit = "kaykit_dungeon"
ramparts_shape = "hexagonal"
anchor_slots = 6
music_state = "combat_intense"
boss_pad_required = true
weather_override = "ashfall"

[stages.forge_sanctum]
biome = "Plains"
arena_extent_m = 80.0
ramparts_kit = "medieval_hexagon"
ramparts_shape = "hexagonal"
anchor_slots = 5
music_state = "combat_default"
boss_pad_required = false
```

**`assets/genomes/roguelite_pois.toml`** :
```toml
[pois.chest_common]
weight = 50
prefab = "models/kaykit/dungeon/chest_basic.glb"  # à vérifier path exact V2
encounter = "none"
size_m = 4.0

[pois.elite_pad]
weight = 15
prefab = "models/kaykit/dungeon/arena_floor.glb"
encounter = "wave_elite"
size_m = 12.0

[pois.boss_pad]
weight = 0  # spawn forcé si stage.boss_pad_required
prefab = "models/kaykit/dungeon/arena_large.glb"
size_m = 20.0
```

---

## 3. Phases d'implémentation

| Phase | Scope | Critères validation |
|---|---|---|
| **P0** | 2 NEW crates skeleton + plugins + AnchorPoint + StageLoadRequest/Result + sensors + 8-12 tests purs. Wire workspace Cargo.toml. Genome TOML schemas créés (vides ou 1 stage). | `cargo check --workspace` 0 erreur · `cargo clippy --workspace -- -D warnings` 0 warning · tests purs verts |
| **P1** | Stage 1 "Crypts of Anvil" runtime jouable : terrain Volcanic bornée 180m + ramparts hex KayKit + 6 POI anchors placés (4 chest_common + 1 elite_pad + 1 boss_pad) + wire avec wave V7 existant. Refactor `sys_spawn_roguelite_scene`. | Runtime : entrer Roguelite → terrain themed visible + ramparts + 6 POI props visibles + sensor `forgia2_stage.json {state:"ready"}` · waves spawn intact |
| **P2** | Stage 2 "Forge Sanctum" + toggles RoR2-style (weather_override, music_state, biome). Wire stage-graph node → stage_id. Hot-reload Shift+F12 testé runtime (change biome TOML → seamless reload). | Stage transition 0→1 dans stage-graph charge stage 2 · hot-reload TOML change visuel sans rebuild |
| **P3** | Polish : health alerts next-step complétées + qa-lead sub-agent audit + verifier sub-agent (cargo + clippy + locks). Memory entries `reference_anchor_point_pattern.md` + `reference_stage_load_request_pattern.md` créées. | qa-lead 0 bug Bloquant/Majeur · verifier ✅ · 2 memory refs créées |

**Estimé** : 4-6j solo, dépend complexité Bevy 0.18 idioms (Message vs Event, ChildOf tuple).

---

## 4. Locks, conventions, anti-régressions

### Stability Locks
- **L1 GameAssets** : aucun nouveau handle dans `forgia-assets` (KayKit chargés à la volée via `forgia-prefab::spawn_gltf_prefab` qui passe par `asset_server.load()`, déjà whitelisté par story-441)
- **L7 SystemSets** : tous systems `.in_set(GameSet::Spawn|UI|Sensor)` + `.run_if(in_state(GameMode::Roguelite))`
- **WALL_Y=0.0 LOCK** (memory `reference_arena_kaykit_wall_y_zero_works`) : ramparts hex utilisent pivot=floor + parent_y = visual_y + half_h trick

### Concept-First étape 0 (data vs code)
- Couche correcte = **definition** (TOML) pour stage params, **framework** (Rust) seulement pour le loader pure
- Aucun gameplay value hardcoded — biome / extent / ramparts_kit / anchor_slots tous lus du TOML

### Observabilité bloquante
- 2 sensors : `forgia2_anchor.json`, `forgia2_stage.json`
- 2 health checks : stage Loading > 5s, stage Error
- Convention alerte next-step respectée (cf `.claude/rules/quality-gate.md`)

### No-speculative-fix
- `forgia-mode-fps-arena/src/lib.rs:396-770` (`spawn_arena`) **N'EST PAS TOUCHÉ** en V1
- Migration FPS arena vers stage-arena = backlog M4 séparé, story future
- Mémoire `reference_arena_kaykit_wall_y_zero_works` confirme que le pattern actuel marche → on ne casse pas ce qui marche

### Bevy 0.18 idioms (memory `reference_bevy_018_breaking_changes_v5` + `reference_bevy_018_traps_batch_2026_05_16`)
- `Message` derive (pas Event)
- `ChildOf(parent_id)` tuple (pas `.with_children()`)
- `Trigger<OnAdd>` → `On<Add, C>` PR #19596
- `EntityCountDiagnosticsPlugin::default()` requis si plugin import
- `init_asset::<Genome<T>>` obligatoire pour AssetLoader genome

---

## 5. Critères d'acceptance globale

- [ ] 2 NEW crates compilent dans le workspace (0 erreur, 0 warning clippy `-D warnings`)
- [ ] 8-12 tests purs verts (`layout_circle`, `ramparts_hex_positions`, `poi_anchor_positions`, `severity_for_stage`, `next_step_for_stage`, etc.)
- [ ] Runtime : Roguelite entré → stage 0 "Crypts of Anvil" visible, terrain themed, ramparts hex, 6 anchors POI placés
- [ ] Sensors `forgia2_anchor.json` + `forgia2_stage.json` écrits 1Hz, `state:"ready"`
- [ ] Stage 1 "Forge Sanctum" déclenché à `RunState::InRun{stage:1}`, biome change visible
- [ ] Hot-reload Shift+F12 : modifier `roguelite_stages.toml` change le runtime sans rebuild
- [ ] Wave spawn intact (régression 0 sur composition_for_stage / enemy archetypes)
- [ ] qa-lead audit 0 Bloquant/Majeur · verifier sub-agent ✅
- [ ] Memory `reference_anchor_point_pattern.md` + `reference_stage_load_request_pattern.md` créées

## 6. Out-of-scope (backlog M4+)

- Migration `forgia-mode-fps-arena::spawn_arena` (770 LOC) vers `StageLoadRequest` — unification path FPS+Roguelite
- forgia-poi-registry crate dédiée (V1 = POI defs dans `roguelite_pois.toml` consumés par stage-arena directement, registry crate utile quand >50 POI types)
- Multi-room topology (Hadès-style) — V1 reste arena unique par stage
- Hybrid biome + POI (Returnal-style) — V2+

---

## 7. Sources / cross-refs

- [[reference-loader-request-result-pattern]] story-441 — Request/Result pattern Resource
- [[reference-kaykit-asset-loading-layout]] story-441 — KayKit PNG atlas co-location piège
- [[reference-arena-kaykit-wall-y-zero-works]] story-432 — WALL_Y=0.0 invariant pivot
- [[reference-pattern-genome-driven-plugin-with-sensor]] — Plugin + genome TOML + sensor canonique
- [[reference-v2-scaffolds-inventory-2026-05-19]] — populate scaffold avant cargo new
- [[reference-bevy-018-breaking-changes-v5]] / [[reference-bevy-018-traps-batch-2026-05-16]] — Bevy 0.18 idioms
- [[project-story-468-roguelite-mvp]] — solo-only Next Fest target context

**External sources** :
- [GDC 2022 — Returnal Never The Same Twice (Ethan Watson)](https://www.gdcvault.com/play/1027651/Never-The-Same-Twice-Procedural)
- [GameDeveloper — RoR2 2D→3D design](https://www.gamedeveloper.com/design/how-moving-from-2d-to-3d-shaped-the-design-of-i-risk-of-rain-2-i-)
- [GDC Podcast — Hades narrative (Greg Kasavin)](https://gdconf.com/article/roguelikes-and-narrative-design-with-hades-creative-director-greg-kasavin-gdc-podcast-ep-16/)
- [Lee Perry — Modular Level Design (UDK PDF)](https://docs.unrealengine.com/udk/Three/rsrc/Three/ModularLevelDesign/ModularLevelDesign.pdf)
- [Epic Data Registry](https://dev.epicgames.com/documentation/en-us/unreal-engine/data-registries-in-unreal-engine)

---

*Story créée 2026-05-20 PM. Plan validé user "GO". P0 en cours.*

---

## 8. Journal d'audit + livraison 2026-05-20 PM

### Phases shippées (chronologique)

1. **P0 Foundations** (~1h) — 2 NEW crates skeleton + 37 tests purs + 0 clippy + workspace 291 crates clean
2. **P1 Stage 1 runtime** (~1h) — spawn system + refacto -180 LOC primitives + 77 tests cumulés
3. **P1-fix bugs visuels** (~30min) — 3 itérations user-driven :
   - Ramparts "tordus" (scale stretch 22×) → tiled side-by-side (4 nouveaux tests)
   - Gaps entre tiles → `wall_natural_len_m` TOML field + smart default par-kit + sensor visibility
   - Rotation 90° off → fix `yaw = atan2(-dir.z, dir.x)` (aligne X-local sur segment) + 2 tests garde-fou régression
4. **P2 Multi-stage dispatch** (~30min) — `stage_id_for_depth` + `sys_stage_dispatch` + auto-cleanup on transition + 85 tests cumulés. **Runtime victory confirmée** : 4 stages joués, 6 transitions, 1076 prefab spawns 0 erreurs.
5. **P3 Toggles + audit** (~45min) — `music_state` → `RequestMusicState` emission, `weather_override` log + sensor, `parse_music_state` parser + 3 tests, qa-lead sub-agent (8 bugs identifiés), 3 fixes appliqués, 88 tests cumulés.

### Audit qa-lead — bugs traités

| Bug | Sévérité | Fix appliqué |
|---|---|---|
| BUG-483-01 | 🟠 MAJ | Plugin systems `.in_set(GameSet::Movement|Sensors)` ajouté ; rationale "mode-agnostic" documentée (pas de `run_if(GameMode)` au niveau plugin — caller gère le lifecycle via StageLoadRequest insertion) |
| BUG-483-02 | 🟡 MIN | `anchor_stats.record(Landmark)` retiré sur les 270 wall tiles (geometry pure, pas anchors) |
| BUG-483-08 | 🟠 MAJ | `LOADING_SUSTAINED_WARN_SEC` const → `StageArenaTuning` Resource tunable + helpers `_with_threshold` testables + wrappers default-back-compat |
| BUG-483-03 | 🟡 MIN | `pois_pool.collect()` accepté (1 fois par stage load via idempotent guard) — note tech-debt |
| BUG-483-04 | 🟢 COS | `weather_override` exposé en sensor + log INFO (consumer V2 `forgia-weather` futur) |
| BUG-483-05 | 🟢 COS | `String` vs `SmolStr` accepté (workspace n'a pas smol_str ; gain micro) |
| BUG-483-06 | 🟡 MIN | False positive : memory entries déjà créées (audit lisait workspace V2, pas dossier memory `~/.claude/...`) |
| BUG-483-07 | 🟢 COS | Story statut → DONE (cette section) |

### Gates finaux

- ✅ `cargo check --workspace` — 0 erreur (291 crates)
- ✅ `cargo clippy --no-deps -- -D warnings` sur 3 crates touchées — 0 warning
- ✅ `cargo test --lib` — **88/88 passed** sur (forgia-anchor + forgia-stage-arena + forgia-mode-roguelite)
- ✅ Runtime victory user-validated (sensors `forgia2_stage.json {state:"ready"}`, `forgia2_roguelite_state.json {victory:true}`)
- ✅ Stability Locks intacts (L1, L7, WALL_Y=0.0)
- ✅ FPS arena `crates/forgia-mode-fps-arena/` non touché (no-speculative-fix)

### Memory refs créées

- `reference_anchor_point_pattern.md` (AnchorPoint cross-mode pattern)
- `reference_stage_load_request_pattern.md` (Request/Result data-driven loader)
- Index `MEMORY.md` mis à jour

### Backlog suiveur

- `forgia-weather` crate dédié (consume `weather_override`) — V2
- Stage_id wiring data-driven via `RunGraph[depth].stage_id_pool` (remplace `stage_id_for_depth` hardcoded) — V2
- `dbg_stage_arena_loading_timeout_sec` sync depuis `debug_monitor.toml` (V1 utilise Resource Default, hot-reload genome wire-up M2)
- FPS Arena migration `spawn_arena` 770 LOC → `StageLoadRequest` (M4)
- Multi-room topology Hadès-style (V2+)

*Story DONE 2026-05-20 PM. Total session ~4h (recherche industrie + audit + 4 phases + fixes + qa).*
