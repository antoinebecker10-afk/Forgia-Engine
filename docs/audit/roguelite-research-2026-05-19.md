# Audit & Recherche industrie — `forgia-mode-roguelite` MVP

> Date : 2026-05-19
> Workspace : V2 (`C:/Users/Antoi/Desktop/Forgia Rewrite`)
> Story rattachée : [story-468-mode-roguelite-mvp.md](../stories/story-468-mode-roguelite-mvp.md)
> Toutes les claims industrielles sont sourcées URL vérifiable. Quand une source canonique n'a pas été trouvée publiquement, c'est dit explicitement.

---

## 1. Audit réutilisation workspace V2

### Topologie

- **615 lignes** dans `Cargo.toml` racine, **258 crates** `forgia-*` (DAG strict)
- **109 crates peuplés** (≥50 LOC, ~42 %)
- **150 crates scaffolds** (≤30 LOC, ~58 %) — convention `forgia-<concept>` réservée
- 2 jeux : `forgia-fps` (1 655 LOC, Arena V1 actif) + `forgia-rpg` (1 832 LOC, squelette Phase 0)

### Tableau de classification (synthèse)

Détail complet ~80 lignes dans le rapport agent, résumé ici par classification :

#### `[FORGIA-CORE]` — réutilisable tel quel (~55 sous-systèmes)

| Domaine | Crates clés (LOC) |
|---|---|
| **Combat** | `forgia-damage` (282), `forgia-weapon-hitscan` (148, post-V6), `forgia-effects` (116), `forgia-hitmarker` (98), `forgia-killfeed` (444) |
| **UI HUD** | `forgia-crosshair` (352), `forgia-ui-hud-ammo` (426), `forgia-ui-pause-menu` (373), `forgia-enemy-nameplate` (358), `forgia-damage-numbers` (74), `forgia-ui` (273) |
| **Juice** | `forgia-juice-camera-shake` (237), `forgia-juice-fov-punch` (187), `forgia-juice-screen-flash` (321), `forgia-juice-recoil` (59), `forgia-juice-hit-stop` (66) |
| **VFX** | `forgia-vfx-tracers` (99) |
| **AI** | `forgia-ai-arena-bot` (478, FSM 4 états) |
| **Inventaire** | `forgia-inventory` (169, LOCK-INV-1 80 slots) |
| **Terrain** | `forgia-terrain` (107), `forgia-streaming` (876), `forgia-foliage` (483) |
| **Assets** | `forgia-asset-registry` (599), `forgia-assets-bundle` (570), `forgia-asset-cdn` (511) |
| **Genome** | `forgia-genome-core` (94, TOML loader) |
| **Observability** | `forgia-observability` (85, sensor JSON 1Hz) |
| **Audio** | `forgia-audio-biome` (119) |
| **Animation** | `forgia-auto-rig` (1 632), `forgia-rig-topology` (509), `forgia-ik` (325), `forgia-camera-orbit` (332) |

#### `[À-EXTRAIRE]` — à factoriser (~10, dont 2 en cours V6)

| ID | Source | Cible | Statut |
|---|---|---|---|
| E1 | `forgia-fps::lib.rs` (LeftMouseState, fire dispatch, BurstState) | `forgia-weapon-hitscan` | 🟡 V6 in progress |
| E2 | `forgia-fps::ads.rs` + viewmodel logic | `forgia-weapon-viewmodel` | 🟡 V6 pending E1 |
| E3 | `forgia-fps::ammo_systems.rs` | `forgia-weapon-ammo` (nouveau) | ⏸️ Backlog |
| E4 | `forgia-combat::melee` | `forgia-weapon-melee` (scaffold présent) | ⏸️ Backlog |
| E5 | Viewmodel FOV/sway/bob générique | `forgia-viewmodel` (peuple scaffold) | ⏸️ Backlog |
| E6 | `forgia-killfeed` `run_if(GameMode::Fps)` → Resource toggle | inchangé (refactor interne) | ⏸️ Backlog |
| E7 | `forgia-damage-numbers` vérifier généricité | inchangé | ✅ déjà OK |
| E8 | `forgia-ai-arena-bot` FSM générique | `forgia-ai-state-machine` (scaffold présent) | ⏸️ Backlog |

#### `[GAME-SPECIFIC]` — ne pas toucher (~6)

- `forgia-fps` (Arena V1 actif, ne pas perturber pendant V6/V7)
- `forgia-rpg` (squelette, mais conserve sa propre boucle)
- `forgia-mode-fps-arena` (832 LOC, Arena scene logic)
- `forgia-village-loader` (850 LOC, spécifique RPG)
- DA cast Apprenti / Maître Forgeur (lore RPG, pas roguelite)

#### `[MANQUANT]` — scaffolds 16 LOC déjà créés au workspace (~15)

Tous présents dans `crates/`, déclarés au `Cargo.toml` racine, prêts à peupler :

| Crate | Description (Cargo.toml) | M cible |
|---|---|---|
| `forgia-loot-tables` | Drop tables genome-driven (rarity) | M2 |
| `forgia-equipment` | Equip slots (weapon, armor, accessory) | M2 |
| `forgia-status-effects` | Burn/slow/stun/poison status with stacking | M3 |
| `forgia-skill-tree` | Skill tree (nodes, prereqs, allocation) | POST |
| `forgia-weapon-projectile` | Projectile weapon (rockets, grenades) | M2 |
| `forgia-mode-roguelite` | Roguelite mode plugin (procedural) | M1-M6 |
| `forgia-net-lightyear` | lightyear wrapper + replication channels | M5 |
| `forgia-net-replication-genome` | Genome-aware replication policies | M5 |
| `forgia-net-lobby` | Lobby system (create/join/list) | M5 |
| `forgia-vfx-impact-library` | Impact preset library per surface type | M2-M4 |
| `forgia-vfx-decals` | Bullet impact decals | polish |
| `forgia-vfx-hanabi` | Hanabi VFX wrapper + pre-spawn dummy anti-freeze | M2 |
| `forgia-scene` | Scene loader + map_switch + DespawnOnExit | M3 |
| `forgia-steam` | bevy-steamworks wrapper (story-424) | M5 |

**Conclusion audit** : la dette d'extraction est faible (V6 traite l'essentiel), le scaffolding est anticipé (convention fine-grained), l'impl pure est entièrement contenue dans des crates dédiées sans toucher Fps/Rpg.

---

## 2. Q1 — Netcode coop PvE 1-3J listen-server

### Précédents AAA/AA

- **Deep Rock Galactic (Ghost Ship)** : P2P, un joueur = host. **Aucune GDC talk officielle publiée** (recherché, non trouvé). Host migration absente, plainte communautaire récurrente. Sources : [Steam P2P discussion](https://steamcommunity.com/app/548430/discussions/1/1874000952589262415/), [Dev tracker host migration](https://devtrackers.gg/deep-rock-galactic/p/17551c7b-host-migration-should-be-mandatory-for-any-game-with-peer-to-peer-connections).

- **Risk of Rain 2 (Hopoo)** : client-serveur, host = listen-server. Actions client relayées vers host qui re-broadcast. Supporte dedicated server UDP 27015. Cross-platform layer custom Steam-like via Pingle Studio. Sources : [Pingle case study RoR2](https://pinglestudio.com/cases/risk-of-rain-2-multiplayer), [Steam Listen Server thread](https://steamcommunity.com/app/632360/discussions/0/2793874219989407685/).

- **Vermintide 2 (Fatshark)** : P2P confirmé sur forum officiel. Pas de GDC talk publique trouvée. Source : [Fatshark forum P2P](https://forums.fatsharkgames.com/t/p2p-please/67513).

- **Gunfire Reborn (Duoyi)** : migré P2P → serveur centralisé. Détails tick rate / replication **non documentés publiquement**. Source : [Steam discussion ping](https://steamcommunity.com/app/1217060/discussions/0/4513255384648340931/).

### Stack Rust

- **lightyear 0.26+ (Charles Bournhonesque)** :
  - Server-authoritative, `Replicate` bundle replication auto
  - Client prediction + rollback activable en 1 ligne
  - Snapshot interpolation pour entités update infrequentes
  - Interest management via Rooms
  - Input buffering par tick
  - Transports officiels : WebTransport, WebSocket, UDP native (pas Steam P2P out-of-box)
  - Sources : [GitHub lightyear](https://github.com/cBournhonesque/lightyear), [docs.rs lightyear](https://docs.rs/lightyear/latest/lightyear/)

- **bevy_replicon** :
  - High-level API server → clients seulement
  - Abstractions singleplayer/client/dedicated/listen simultané
  - Plus simple, moins de features (pas de prediction/rollback)
  - Source : [GitHub projectharmonia/bevy_replicon](https://github.com/projectharmonia/bevy_replicon)

### Décision Forgia roguelite

**lightyear + transport custom Steam P2P, listen-server**, parce que :
- Justifie scaffold `forgia-net-lightyear` déjà au workspace
- Prediction utile pour FPS (TTK rapide → 100ms compte)
- Steam P2P évite NAT/punch-through (Steam SDK gère)
- 1-3J = bandwidth faible, listen-server suffit

**Effort R&D anticipé** : 3-5 jours pour le transport Steam custom (pas d'exemple lightyear officiel trouvé). Fallback : UDP direct + Steam Datagram Relay (SDR).

---

## 3. Q2 — Drop tables / loot rarity

### Patterns AAA documentés

- **Diablo 3 Loot 2.0** (Josh Mosqueira, GDC 2015) :
  - *Fewer but better items*
  - **Smart Loot** : affixes biaisés vers la classe du joueur (Wizard reçoit +Int)
  - Mesure d'impact : Act III Inferno 1 legendary/run → 6 legendaries/run après Loot 2.0
  - Source : [PureDiablo Mosqueira GDC 2015](https://www.purediablo.com/josh-mosqueira-diablo-3-presentation-gdc-2015)

- **Path of Exile** :
  - 4 rarités (Normal/Magic/Rare/Unique)
  - 2 phases RNG : (1) rarity, (2) tier within rarity (T5 worst → T0 mythic)
  - Weighted random à chaque étape, poids tiers augmente avec item rarity
  - Source : [PoE Wiki Rarity](https://pathofexile.fandom.com/wiki/Rarity), [PoE Wiki tier analysis](https://www.poewiki.net/wiki/Guide:Analysis_of_unique_item_tiers)

- **Borderlands 3** :
  - Bernoulli trial par kill (`rand < drop_rate`)
  - Dedicated drops (boss → arme spécifique pool weighted)
  - "Smart loot" Gearbox revendiqué en interviews mais **non documenté tech publiquement**
  - Source : [Hindle confidence intervals](https://softwareprocess.es/homepage/posts/borderlands-3-and-confidence-intervals/), [LootCalc Bl3 farming](https://lootcalc.com/guides/borderlands3-legendary-farming-guide)

- **Hearthstone pity timer** (officiellement confirmé Blizzard) :
  - Compteur par expansion set
  - P(rare) augmente à chaque pack ouvert sans drop
  - Garantie legendary tous les 40 packs (un dans les 10 premiers)
  - Duplicate protection séparée
  - Pattern transposable roguelite : `pity_counter += 1 → P(rare) *= 1 + k*counter`
  - Sources : [Esports.gg pity](https://esports.gg/news/hearthstone/hearthstone-pity-timer/), [GosuNoob pity](https://www.gosunoob.com/guides/hearthstone-pity-timer-legendary-drop-rate/)

### Schéma data-driven Forgia

```toml
# assets/genomes/roguelite/roguelite_loot.toml
[[pool]]
id = "stage_1_basic"
entries = [
  { item = "weapon_pepin", weight = 100, rarity = "common" },
  { item = "weapon_bourrasque", weight = 30, rarity = "uncommon" },
  { item = "weapon_lenoir", weight = 8, rarity = "rare" },
  { item = "weapon_boucherie", weight = 2, rarity = "legendary" },
]
pity_increment = 0.05   # +5% rare chance per dry drop
pity_reset_on = ["rare", "legendary"]
```

**RNG seedé** : `xorshift32(seed = run_seed * 31 + stage_id * 17 + encounter_idx * 7)` côté host (authoritative).

---

## 4. Q3 — Weapons-as-characters (armes parlantes)

### Sources canoniques

- **High on Life (Squanch)** :
  - Senior gameplay designer **Andy Kibler** a piloté le "narrative director system"
  - Triggers cités : standing still (idle), missed shot (mockery), environnement réactif
  - **>10 000 pages de dialogue** enregistrées, milliers de lignes contextuelles par arme
  - Sources : [DigiPen Kibler showcase](https://www.digipen.edu/showcase/news/digipen-alumni-help-squanch-launch-record-breaking-sci-fi-fps-comedy-high-life), [GameRant talking guns](https://gamerant.com/high-on-life-talking-guns-comedy-game-design-tutorials/)
  - **Pas de papier tech détaillé** sur la structure du narrative director

- **Hadès (Greg Kasavin + Darren Korb, GDC 2021)** "Breathing Life into Greek Myth: The Dialogue of Hades" :
  - **22 000+ lignes** voiced pour <20 employés
  - Système monitore conditions mid-run (HP < threshold, character + state combo)
  - Pool pre-written de "possible events" → sélection contextuelle
  - Mantra : *"What would these characters notice?"*
  - Sources : [GDC Vault Hades](https://www.gdcvault.com/play/1026975/Breathing-Life-into-Greek-Myth), [GameDeveloper Kasavin](https://www.gamedeveloper.com/design/roguelikes-and-narrative-design-with-i-hades-i-creative-director-greg-kasavin)

- **Risk of Rain 2 voice lines** : pas de talk publique. Pattern observé empiriquement = event triggers + pool + cooldown.

### Pattern stack synthétisé (sources Kasavin + Kibler)

```rust
// Triggers
#[derive(Event)]
struct BarkEvent {
    speaker: Entity,
    kind: BarkKind,           // Kill | LowHp | Idle | Reload | Pickup | StageCleared | Death
    context: BarkContext,     // HashMap<String, BarkValue>
}

// Pool
struct LinePool {
    entries: Vec<LineEntry>,
}
struct LineEntry {
    id: String,
    text: String,            // ou clé localisation
    audio_handle: Handle<AudioSource>,
    weight: f32,
    priority: u8,            // 0-255, override cooldown si > threshold
    cooldown_sec: f32,
    conditions: Vec<BarkCondition>,  // ex: hp < 0.3, kill_count > 5
}

// Selector anti-spam
#[derive(Resource)]
struct BarkSelector {
    last_played_at: HashMap<String, f64>,  // line_id → t
    current_speaker_lock: Option<(Entity, f64)>,  // speaker, until_t
}
```

### Application Forgia (4 armes MVP)

- **Pépin** : pistolet apprenti, ton timide, lignes hésitantes
- **Bourrasque** : SMG vent, ton extraverti, lignes énergiques
- **Madame Lenoir** : fusil de précision, ton snob aristocratique, lignes acerbes
- **Boucherie** : shotgun butcher, ton brutal joyeux, lignes sanguinaires

Chaque arme : 6 barks contextuels MVP × 4 = 24 lignes total (placeholder TTS acceptable M4).

Cooldown défaut : 8s par bark, 4s lock par speaker, priority Kill=80 Idle=20 Death=200.

---

## 5. Q4 — Roguelite procgen run structure

### Patterns documentés

- **Hadès** :
  - 4 biomes (Tartarus, Asphodel, Elysium, Styx)
  - Rooms = templates **hand-crafted** assemblés en séquence
  - Tailles graduées (petites tôt, grandes tard) pour onboarding
  - Walled-in → claire lisibilité
  - **Pas de procgen libre**, gates déterministes
  - Source : [Kotaku Hades less random](https://kotaku.com/hades-level-design-is-less-random-than-it-seems-1845254545)

- **Dead Cells (Motion Twin / Deepnight)** :
  - Hybride : graph statique de rooms par biome avec contraintes (longueur, nb specials, ratio labyrinthe)
  - Algo procgen sélectionne aléatoirement parmi templates hand-crafted matching contraintes
  - Map d'île fixe (entry/exit/keys), variants seedés
  - Sources : [Deepnight tutorial Dead Cells](https://deepnight.net/tutorial/the-level-design-of-dead-cells-a-hybrid-approach/), [GameDeveloper hybrid approach](https://www.gamedeveloper.com/design/building-the-level-design-of-a-procedurally-generated-metroidvania-a-hybrid-approach-)

- **Risk of Rain 2 Director system** :
  - Credits accumulés linéairement × difficulty coef
  - Director pioche enemy random par environnement, dépense credits par groupe (≤ 4)
  - Coûts : Lemurian basique cheap, boss max, Elite 6×, Mythic 36×
  - Skip enemies "too cheap" pour son budget (fix bug bosses gratuits late game)
  - **Wiki est la seule source structurée**, pas de talk Hopoo officielle
  - Sources : [RoR2 Wiki Directors](https://riskofrain2.fandom.com/wiki/Directors), [RoR2 Wiki Difficulty](https://riskofrain2.wiki.gg/wiki/Difficulty)

### Structure StageGraph Forgia recommandée

```
Stage 1 ─┬─ Stage 2A (combat) ─┬─ Stage 3A (combat) ─┐
         └─ Stage 2B (elite)   ─┘                    │
                                ┌─ Stage 3B (event) ─┤
                                └─                   ├─ Stage 4 (mini-boss)
                                                     │
                                                     └─ Stage 5 (boss arena)
```

- Linéaire avec 2-3 choix par nœud (modèle Hadès)
- Chaque stage = pool de room templates filtrés par biome + difficulty_budget
- Director-style budget pour scaling 1J vs 3J : `credits_per_sec *= 1 + 0.3 * (players - 1)`

### Déterminisme seed coop

**Pattern industrie sans source canonique publique** (synthèse) :
- Host génère `run_seed` au lobby start
- Broadcast `RunSeed { value }` aux clients en lobby
- Tous les RNG dérivent de `(run_seed, stage_id, encounter_idx)` côté host
- Clients consomment via replication d'events (`EnemySpawned`, `LootDropped`)

---

## 6. Q5 — Patterns Bevy 0.18 ECS pour gameplay roguelite scalable

### SubStates

- `SubStates` enfants d'un parent state, auto-removed du World si parent quitte
- Exemple officiel `examples/state/sub_states.rs`
- Application Forgia : `AppMode::Play(Roguelite) → SubState RunState { Lobby | InRun{stage} | Boss | Defeat | Victory }`
- Sources : [Bevy sub_states.rs](https://github.com/bevyengine/bevy/blob/main/examples/state/sub_states.rs), [bevy_state docs](https://docs.rs/crate/bevy_state/latest), [Cheat Book States](https://bevy-cheatbook.github.io/programming/states.html)

### State-scoped entities

- Markers `DespawnOnEnter<S>` / `DespawnOnExit<S>` (renommés en 0.17 depuis `StateScoped`)
- Cleanup auto par stage : `commands.spawn((Enemy, DespawnOnExit(RunState::InRun{stage:3})))`
- ⚠ **Caveat** : double-despawn bug ouvert (#20866) sur certaines combinaisons
- Pour Forgia : marker custom `StageScoped(StageId)` + system `clear_on_stage_change` plus prévisible que markers par variante
- Sources : [bevy_state state_scoped.rs](https://github.com/bevyengine/bevy/blob/main/crates/bevy_state/src/state_scoped.rs), [Issue #20866](https://github.com/bevyengine/bevy/issues/20866), [Migration 0.16→0.17](https://bevy.org/learn/migration-guides/0-16-to-0-17/)

### EntityEvent vs Message (PR #19647 event rearchitecture)

| Type | Quand | Usage Forgia |
|---|---|---|
| `EntityEvent` | Trigger immédiat, cible entité, support bubbling | `DamageEvent`, `DeathEvent`, `HitEvent` |
| `Message` | Buffered queue, lu via `MessageReader<T>` next frame | `PickupEvent`, `StageCleared`, `RunStarted` |
| `Event` simple | Trigger immédiat global Observer | rare, événements globaux non ciblés |

Sources : [PR #19647 Event split](https://github.com/bevyengine/bevy/pull/19647), [Discussion #21492](https://github.com/bevyengine/bevy/discussions/21492), [PR #20731 Event Rearchitecture](https://github.com/bevyengine/bevy/pull/20731), [EntityEvent doc](https://docs.rs/bevy/latest/bevy/ecs/event/trait.EntityEvent.html), [Observer propagation](https://bevy.org/examples/ecs-entity-component-system/observer-propagation/)

### par_iter — scaling N=1000 enemies/stage

- `BatchingStrategy` Bevy auto-choisit batch size + nb threads
- Fallback single-thread auto si peu d'entités
- Bénéfice perf si travail par entity **significatif et homogène** (sinon overhead > gain)
- Seuil Forgia (32 entités) conservateur ; pour AI tick / damage tick à N=1000 → `par_iter` justifié
- Combiner avec `Changed<Transform>` / `With<Active>` pour ne pas itérer dormants
- Sources : [Cheat Book par_iter](https://bevy-cheatbook.github.io/programming/par-iter.html), [Cheat Book change detection](https://bevy-cheatbook.github.io/programming/change-detection.html)

### Alloc 0 hot path

- `Local<Vec<T>>` cleared per system tick (pas `Vec::new()`)
- Resource buffers globaux pré-alloués (jamais grow runtime > capacity)
- `SmallVec<[T; N]>` pour collections petites stack-allouées

### Cleanup par stage recommandé

```rust
#[derive(Component, Copy, Clone, Eq, PartialEq)]
pub struct StageScoped(pub StageId);

// OnExit InRun{stage}
fn cleanup_stage(
    mut commands: Commands,
    query: Query<Entity, With<StageScoped>>,
    current_stage: Res<CurrentStageId>,
) {
    for entity in &query {
        commands.entity(entity).despawn();  // récursif par défaut en 0.18
    }
}
```

---

## 7. Caveats et limites de la recherche

- **Hopoo (RoR2)**, **Fatshark (V2)**, **Ghost Ship (DRG)**, **Duoyi (Gunfire Reborn)** : aucun GDC talk publique trouvée sur leur netcode. Wikis et forums seulement → indicatif, pas canonique.
- **Borderlands smart loot** : revendiqué en interviews Gearbox, non documenté tech publiquement.
- **Seed sync coop** : pattern industrie sans source canonique publique — bon sens, à valider en prototypage.
- **High on Life narrative director** : interview-level (Kibler/DigiPen), pas de papier tech. Hadès reste la source la plus structurée pour reactive dialogue.
- **Transport Steam P2P pour lightyear** : pas d'exemple officiel trouvé → R&D 3-5j en M5.

---

## 8. Sources canoniques les plus fortes

- [Greg Kasavin GDC 2021 Hades dialogue (vault)](https://www.gdcvault.com/play/1026975/Breathing-Life-into-Greek-Myth)
- [Josh Mosqueira GDC 2015 Diablo 3 Loot 2.0](https://www.purediablo.com/josh-mosqueira-diablo-3-presentation-gdc-2015)
- [Deepnight Dead Cells level design (auteur direct)](https://deepnight.net/tutorial/the-level-design-of-dead-cells-a-hybrid-approach/)
- [Bevy PR #19647 Event split (architecture officielle)](https://github.com/bevyengine/bevy/pull/19647)
- [lightyear GitHub officiel](https://github.com/cBournhonesque/lightyear)
- [bevy_replicon GitHub](https://github.com/projectharmonia/bevy_replicon)
- [Bevy Cheat Book par_iter / change detection / states](https://bevy-cheatbook.github.io/)

---

*Document de référence pour story-468. À retoucher uniquement si nouvelles sources canoniques émergent.*
