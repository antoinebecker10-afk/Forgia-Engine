# Story-470 — V7 M1 Roguelite Fondations (scaffold MVP)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_roguelite_state.json`, fichier `lib.rs`, symbole `StartRunEvent`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED
> **État d'origine (périmé, cf bandeau)** : PLAN — en attente validation.
**BMAD scale** : Enterprise (cross-crate : forgia-core + forgia-ui + forgia-mode-roguelite + forgia-observability).
**Vague** : V7 — M1 Fondations.
**Parent story** : `docs/stories/story-468-mode-roguelite-mvp.md` §M1.
**Effort estimé** : 3-4 h (scaffold MVP), pas le 2-3 jours du story-468 (qui inclut combat).

---

## 0. TL;DR

Squelette `forgia-mode-roguelite` jouable end-to-end mais inerte : depuis le menu, le joueur entre dans `GameMode::Roguelite` → `RunState::Lobby` → `StartRunEvent` fire → `RunState::InRun{stage: 0}` → cleanup OnExit. Aucun combat/loot/biome dans cette M1 (réservé M2+).

5 acceptance criteria de story-468 §M1 livrés :
- ✅ Bouton "🎲 Roguelite Run" au menu
- ✅ `RunState` SubStates (Lobby/InRun/Boss/Defeat/Victory)
- ✅ `StartRunEvent` / `EndRunEvent` (BufferedEvent — règle audit 0.1 §A2)
- ✅ `RunSeed` Resource déterministe (u64 + dérive xoshiro)
- ✅ Sensor `forgia2_roguelite_state.json` 1Hz
- ✅ Cleanup `OnExit(RunState::InRun)` via DespawnOnExit marker

---

## 1. Concept-first 5 étapes

### Étape 0 — Data ou code ?

**Code framework** : RunState + Events + Resources. Aucune valeur genome TOML pour M1 (procgen/loot venant en M2+).

### Étape 1 — Hypothèses concurrentes

- **H1 (retenu)** : Ajouter `GameMode::Roguelite` variant + `RunState` SubStates (source = `GameMode::Roguelite`). Pattern miroir Fps/Rpg flat existant. Aligne sur le codebase.
- **H2 (rejeté)** : Refactor AppMode → enum payload `Play(GameMode)` comme suggéré story-468. Trop disruptif (touche 2 commits live de logique Fps+Rpg). Le SubStates idiom 0.18 fait le même travail sans refactor.
- **H3 (rejeté)** : Tout dans `forgia-mode-roguelite` sans toucher `forgia-core`. Impossible : `GameMode` doit avoir la variant `Roguelite` pour le menu route.

### Étape 2 — Cartographier

- **forgia-core** `lib.rs:33` : ajouter variant `Roguelite` à `GameMode`
- **forgia-ui** `lib.rs:118` : ajouter 3e bouton menu (miroir RPG)
- **forgia-mode-roguelite** : populate scaffold 16 LOC → ~250 LOC
- **forgia-observability** : pas touché (sensor écrit par mode-roguelite directement)
- **forgia-game** `lib.rs` : ajouter `app.add_plugins(ForgiaModeRoguelitePlugin)` au boot
- Pas touchés : forgia-fps, forgia-rpg (déjà gated par GameMode, ignorent Roguelite)

### Étape 3 — Verbalisation

| Élément | Producteur (timing) | Consommateurs | Net | Script |
|---|---|---|---|---|
| `GameMode::Roguelite` | menu (transition Menu→InGame) | run_if(in_state(...)) plugin gating | L | int |
| `RunState` SubStates | RunStateTransitions system on `StartRunEvent` | cleanup OnExit, sensor writer | L | int |
| `StartRunEvent` (BufferedEvent) | menu button OR debug shortcut | `start_run` system | L | int |
| `EndRunEvent` (BufferedEvent) | `end_run` system (defeat/victory triggers) | menu return, sensor flush | L | int |
| `RunSeed` Resource | inserted on StartRunEvent (rand_xoshiro seed_from_entropy) | dérive `stage_seed(stage_id)`, `encounter_seed(stage_id, idx)` | L | int |
| Sensor `forgia2_roguelite_state.json` | `sys_write_roguelite_state` 1Hz Update | xtask gate (futur cible 14/14) | L | int |

### Étape 4 — Hot path check

Aucun système hot. 1Hz sensor write + RunState transitions sont rares. Cleanup OnExit utilise DespawnOnExit (Bevy 0.18 natif).

### Étape 5 — Scale-up BMAD

Cross-crate (forgia-core + forgia-ui + forgia-mode-roguelite + forgia-game) = Enterprise. Story OBLIGATOIRE. Checklist post-impl OBLIGATOIRE.

---

## 2. Détail implémentation

### 2.1 `crates/forgia-core/src/lib.rs` (ajout variant)

```rust
#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameMode {
    #[default]
    None,
    Fps,
    Rpg,
    Roguelite,
}
```

### 2.2 `crates/forgia-mode-roguelite/src/lib.rs` (populate scaffold)

```rust
//! forgia-mode-roguelite — V7 M1 squelette (story-470).

use bevy::prelude::*;
use forgia_core::prelude::*;
use rand_xoshiro::Xoshiro256StarStar;
use rand_xoshiro::rand_core::{RngCore, SeedableRng};

pub mod run;
pub mod sensor;

pub use run::{RunState, RunSeed, StartRunEvent, EndRunEvent, RunResult, RogueliteRunMarker};

pub mod prelude {
    pub use crate::ForgiaModeRoguelitePlugin;
}

pub struct ForgiaModeRoguelitePlugin;

impl Plugin for ForgiaModeRoguelitePlugin {
    fn build(&self, app: &mut App) {
        app.add_sub_state::<RunState>()
            .add_message::<StartRunEvent>()
            .add_message::<EndRunEvent>()
            .add_systems(
                Update,
                (run::sys_start_run, run::sys_end_run)
                    .in_set(GameSet::Movement)
                    .run_if(in_state(GameMode::Roguelite)),
            )
            .add_systems(
                Update,
                sensor::sys_write_roguelite_state
                    .in_set(GameSet::Sensors)
                    .run_if(in_state(GameMode::Roguelite)),
            )
            .add_systems(
                OnExit(GameMode::Roguelite),
                run::sys_cleanup_run_markers,
            );
    }
}
```

### 2.3 `src/run.rs` (~100 LOC)

```rust
use bevy::prelude::*;
use forgia_core::prelude::*;
use rand_xoshiro::Xoshiro256StarStar;
use rand_xoshiro::rand_core::SeedableRng;

#[derive(SubStates, Default, Debug, Clone, PartialEq, Eq, Hash)]
#[source(GameMode = GameMode::Roguelite)]
pub enum RunState {
    #[default]
    Lobby,
    InRun { stage: u8 },
    Boss { stage: u8 },
    Defeat,
    Victory,
}

#[derive(Message, Debug, Clone)]
pub struct StartRunEvent {
    pub seed: Option<u64>,  // None = random
}

#[derive(Message, Debug, Clone)]
pub struct EndRunEvent {
    pub result: RunResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunResult { Victory, Defeat, Abort }

#[derive(Resource, Debug, Clone)]
pub struct RunSeed {
    pub seed: u64,
    pub stage_count: u8,  // current stage advancement
}

impl RunSeed {
    pub fn stage_seed(&self, stage: u8) -> u64 {
        // Mixed via xoshiro one-shot (déterministe)
        let mut rng = Xoshiro256StarStar::seed_from_u64(self.seed ^ (stage as u64).wrapping_mul(0x9E3779B97F4A7C15));
        rand_xoshiro::rand_core::RngCore::next_u64(&mut rng)
    }
}

/// Marker pour toutes les entités spawnées pendant InRun — cleanup OnExit GameMode.
#[derive(Component, Default)]
pub struct RogueliteRunMarker;

pub fn sys_start_run(
    mut events: MessageReader<StartRunEvent>,
    mut next: ResMut<NextState<RunState>>,
    mut commands: Commands,
) {
    for ev in events.read() {
        let seed = ev.seed.unwrap_or_else(|| {
            let mut rng = Xoshiro256StarStar::from_seed_u64(rand::random::<u64>()).unwrap_or_else(|_| Xoshiro256StarStar::seed_from_u64(0xCAFEBABE));
            // Note: dépendance rand pas voulue → utiliser timestamp:
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0xC0FFEE)
        });
        commands.insert_resource(RunSeed { seed, stage_count: 0 });
        next.set(RunState::InRun { stage: 0 });
        info!("[roguelite] Run started — seed={seed}");
    }
}

pub fn sys_end_run(
    mut events: MessageReader<EndRunEvent>,
    mut next: ResMut<NextState<RunState>>,
) {
    for ev in events.read() {
        let state = match ev.result {
            RunResult::Victory => RunState::Victory,
            RunResult::Defeat => RunState::Defeat,
            RunResult::Abort => RunState::Lobby,
        };
        next.set(state);
        info!("[roguelite] Run ended — {:?}", ev.result);
    }
}

pub fn sys_cleanup_run_markers(
    mut commands: Commands,
    q_markers: Query<Entity, With<RogueliteRunMarker>>,
) {
    let count = q_markers.iter().count();
    for e in &q_markers {
        commands.entity(e).despawn();
    }
    info!("[roguelite] cleanup OnExit GameMode::Roguelite — despawned {count} entities");
}
```

**Note** : ne pas utiliser la crate `rand` pour `from_seed_u64` ; xoshiro `SeedableRng::seed_from_u64` existe directement. Simplification possible.

### 2.4 `src/sensor.rs` (~70 LOC)

```rust
use bevy::prelude::*;
use forgia_core::prelude::*;
use crate::run::{RunState, RunSeed};

pub fn sys_write_roguelite_state(
    time: Res<Time>,
    mut accum: Local<f32>,
    run_state: Option<Res<State<RunState>>>,
    run_seed: Option<Res<RunSeed>>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 { return; }
    *accum = 0.0;

    let (state_str, stage) = match run_state.as_ref().map(|s| s.get().clone()) {
        Some(RunState::Lobby) => ("lobby", 0u8),
        Some(RunState::InRun { stage }) => ("in_run", stage),
        Some(RunState::Boss { stage }) => ("boss", stage),
        Some(RunState::Defeat) => ("defeat", 0),
        Some(RunState::Victory) => ("victory", 0),
        None => ("none", 0),
    };
    let seed = run_seed.map(|s| s.seed).unwrap_or(0);

    let json = format!(
        r#"{{"id":"roguelite_state","severity":"ok","next_step":"","timestamp_secs":{:.1},"run_state":"{state_str}","stage":{stage},"seed":{seed}}}"#,
        time.elapsed_secs()
    );
    let _ = std::fs::write("forgia2_roguelite_state.json", &json);
}
```

### 2.5 `crates/forgia-ui/src/lib.rs` ajout bouton

Après bouton RPG (ligne 121), insérer :

```rust
ui.add_space(20.0);
if ui.add(egui::Button::new(egui::RichText::new("🎲 Roguelite").size(28.0)).min_size(egui::vec2(280.0, 60.0))).clicked() {
    next_game.set(GameMode::Roguelite);
    next_app.set(AppMode::InGame);
    start_run_writer.write(StartRunEvent { seed: None });
}
```

Avec `start_run_writer: MessageWriter<StartRunEvent>` ajouté aux params.

### 2.6 `crates/forgia-game/src/lib.rs`

Ajouter `app.add_plugins(ForgiaModeRoguelitePlugin)` dans le plugin chain.

### 2.7 Cargo.toml additions

```toml
# forgia-mode-roguelite/Cargo.toml
[dependencies]
bevy = { workspace = true }
forgia-core = { workspace = true }
rand_xoshiro = "0.7"

# forgia-ui/Cargo.toml — ajouter
forgia-mode-roguelite = { workspace = true }

# forgia-game/Cargo.toml — ajouter
forgia-mode-roguelite = { workspace = true }
```

### 2.8 xtask CANONICAL_SENSORS

13e sensor optionnel `forgia2_roguelite_state.json` ajouté à la liste (cible 13/13 atteinte).

---

## 3. Tests headless requis

- `run_seed_stage_seed_deterministic` : 2 RunSeed même seed → stage_seed(N) identique
- `run_seed_different_seeds_diverge` : seeds différents → stage_seed(0) différent
- `runstate_default_is_lobby` : RunState::default() == Lobby
- `run_result_variants` : enum complet
- `roguelite_state_sensor_format` : serde_json roundtrip de la struct (à extraire pure)

~5 tests purs.

---

## 4. Pièges anticipés

1. **SubStates Bevy 0.18 syntax** : `#[source(GameMode = GameMode::Roguelite)]` — vérifier à la compile (différent de Bevy 0.13). Si fail → utiliser `States` simple + manual gating.
2. **Message vs BufferedEvent vs Event** : story-468 §0.1 dit BufferedEvent. En Bevy 0.18 c'est `Message` derive + `MessageWriter`/`MessageReader`. Cohérent avec Session C.
3. **`rand_xoshiro::SeedableRng::seed_from_u64`** : disponible depuis 0.6, OK pour 0.7.
4. **OnExit(GameMode::Roguelite)** : DespawnOnExit natif Bevy 0.18 — alternative simple à RogueliteRunMarker pour M1.
5. **Menu button order** : ne pas casser les 2 boutons FPS/RPG existants. Insérer entre RPG et "Quitter".
6. **`start_run_writer` MessageWriter dans main_menu_ui** : nécessite ajouter param. Vérifier que `Message<StartRunEvent>` est enregistré AVANT que le system tourne (sinon panic). Add_message dans Plugin = OK.

---

## 5. Acceptance (story-468 §M1)

- [ ] Bouton "🎲 Roguelite" présent au menu, cliquable
- [ ] `GameMode::Roguelite` variant existe et est set par le bouton
- [ ] `RunState` SubStates (Lobby/InRun{stage}/Boss{stage}/Defeat/Victory) défini
- [ ] `StartRunEvent` + `EndRunEvent` (Bevy 0.18 Message) wired
- [ ] `RunSeed` Resource inséré sur StartRunEvent
- [ ] `forgia2_roguelite_state.json` écrit 1Hz (run_state, stage, seed, severity ok)
- [ ] Cleanup `OnExit(GameMode::Roguelite)` despawn entités avec marker
- [ ] `cargo check --workspace` ✅, clippy `-D warnings` ✅ 0
- [ ] Tests headless ~5 verts
- [ ] Smoke test runtime : Menu → cliquer Roguelite → boot OK + sensor écrit + return menu OK
- [ ] xtask `verify-sensors-format` → 13/13 (incluant nouveau)
- [ ] ROADMAP V7 M1 mark DONE

---

## 6. Décisions à valider AVANT code

| # | Question | Recommandation |
|---|---|---|
| 1 | `GameMode::Roguelite` (flat) ou `AppMode::Play(Roguelite)` enum payload ? | **Flat** — aligne sur codebase existant, refactor enum payload risque casser Fps/Rpg paths live |
| 2 | `RunState` SubStates (source GameMode::Roguelite) ou State indépendant + gating manuel ? | **SubStates** — Bevy 0.18 idiom moderne |
| 3 | Cleanup OnExit : `DespawnOnExit<S>` natif ou `RogueliteRunMarker` Component ? | **Marker** — explicite, contrôle fin par crate, plus tard switch DespawnOnExit si simplification |
| 4 | `forgia2_roguelite_state.json` ajouté à CANONICAL_SENSORS xtask ? | **Oui** — 13/13 cible atteinte |
| 5 | M1 = scaffold MVP (3-4h) ou full M1 story-468 (2-3 jours combat solo) ? | **MVP scaffold** — M2 combat suit naturellement, validation incrémentale |

---

## 7. Liens

- Parent : `docs/stories/story-468-mode-roguelite-mvp.md` §M1 (acceptance)
- Audit deep : `docs/audit/story-468-deep-audit-2026-05-19.md`
- Bevy 0.18 findings : memory `reference_bevy_018_breaking_changes_v5.md`
- Pattern menu : `crates/forgia-ui/src/lib.rs:113-121`
- Pattern States : `crates/forgia-core/src/lib.rs:32-38`
