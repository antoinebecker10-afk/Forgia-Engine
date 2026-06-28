# Story-634 — Keystone : simulation déterministe (FixedUpdate + RunRng)

> **Statut** : 🔵 IN_PROGRESS — 0.1a-1 fait ; 0.1a-2 slice 2 fait ; **0.1b-1 fait** (RNG combat hors
> horloge : crit/recoil/loot/spread via CombatRng + fix boons inertes, runtime validé). Reste : 0.1a-2
> slices 3-5 (mouvement/tir en FixedUpdate, **différé** = besoin Rapier en FixedUpdate) + 0.1b-2 ordre d'itération.
>
> **🔎 MISE À JOUR 2026-06-28 (exécution + finding majeur)** : le piège `just_pressed` en
> FixedUpdate **est déjà corrigé en amont par leafwing 0.20** (preuve dure : `leafwing-input-manager`
> `RELEASES.md` L269 décrit mot-pour-mot le « doublé si N steps / perdu si 0 step » et l'a résolu en
> tickant `ActionState` en `FixedPreUpdate` + états fixed/update séparés, cf. `set_fixed_update_state_from_state`
> L44). → **La couche `FixedInput` (ex-slice 1) est SUPPRIMÉE** (la construire = sur-ingénierie, CLAUDE.md §3).
> Les systèmes input-edge (saut/dash) liront `just_pressed`/`pressed` **directement** en FixedUpdate.
> Décision user 2026-06-28. La numérotation des slices est mise à jour ci-dessous (plus de slice 1).
> **Niveau BMAD** : Enterprise (multi-crate, change le scheduling cœur). **Date début** : 2026-06-25.
> **Épopée** : Plan RPG+QA intégré ([rpg-qa-integrated-plan](../plan/rpg-qa-integrated-plan-2026-06-24.md)) — keystone Phase 0.1.
> **Spike** : [spike-fixedupdate-determinism-2026-06-24](../audit/spike-fixedupdate-determinism-2026-06-24.md).

## Pourquoi (la keystone)
La sim gameplay est soudée à `Update` (timestep variable) → **pas de déterminisme** → pas de tests
reproductibles, pas de replay, pas de serveur autoritatif. WoC (décortiqué) doit tout ça à un cœur sim
**déterministe**. C'est le débloqueur du vrai palier QA.

Découverte du spike : **Rapier tourne déjà en `FixedUpdate` 64 Hz** pendant que le gameplay est en
`Update` → il existe une désync latente (KCC écrit hors du step physique). Migrer le gameplay en
FixedUpdate **aligne** sur Rapier et **corrige** ce bug — le risque #1 est en fait une opportunité.

## Incréments
### ✅ 0.1a-1 — Fondation GameSet FixedUpdate (FAIT, ce commit)
`forgia-core/src/lib.rs` : `GameSet` (chaîne Network→…→UI) configuré **aussi** en `FixedUpdate`
(prérequis bloquant R1 du spike : sans ça, l'ordre en FixedUpdate serait indéfini). **Déclaration
pure** — aucun système n'y est encore migré → 0 effet runtime, 0 changement de feel. Hz = défaut
Bevy 64 (= timestep Rapier actuel, pas de 60 forcé).

### 🔲 0.1a-2 — Migration des systèmes (~35, HIGH, par slices) — DESIGN

**Le piège soupçonné — et pourquoi il NE s'applique pas ici** : `player_movement` (lib.rs:607) et
`dash_input_system` (dash.rs:95) lisent `action.just_pressed(Jump)`. En théorie générale, `just_pressed`
en `FixedUpdate` serait doublé (frame lente, N steps) ou perdu (0 step). **MAIS** ces lectures passent
par `ActionState<PlayerAction>` (leafwing 0.20), et leafwing tick l'`ActionState` en `FixedPreUpdate`
avec un état fixed séparé depuis 0.15 (preuve : `RELEASES.md` L269 — le bug exact « doublé/perdu » y est
décrit comme **corrigé**). → en FixedUpdate, `just_pressed` est correct **pour exactement 1 step**. La
couche de buffering manuelle (`FixedInput`) **n'est donc PAS nécessaire** : on déplace les systèmes et on
lit `just_pressed`/`pressed` directement. (Le tir reste à part : `LeftClickState` via `MouseButtonInput`
brut, non-leafwing → soin spécifique slice 4.)

**Bonne nouvelle (cartographie)** : le **tir est déjà découplé** de `ButtonInput` brut
(`forgia-fps/lib.rs:61` `LeftClickState` via `MessageReader<MouseButtonInput>`, car egui consomme
l'input) et `fire_mode` est une **fn pure** (`lib.rs:84`). Les cooldowns/trauma/flash sont des
**timers purs** (aucun input). La moitié du travail est déjà décentralisée.

#### ~~Couche `FixedInput` (ex-slice 1)~~ — ABANDONNÉE (2026-06-28)

Design initial : latch les edges en Update → drain 1× en FixedUpdate(Input) → la sim lit le buffer.
**Supprimé** : leafwing 0.20 fournit déjà la garantie « 1 edge = 1 step » (cf. paragraphe ci-dessus).
Construire le buffer = recoder à la main ce que la lib fait → sur-ingénierie (CLAUDE.md §3). Si le
**replay déterministe (0.1b)** réclame plus tard une frontière input enregistrable, leafwing expose
`ActionState::set_fixed_update_state_from_state` (pensé pour le réseau/replay) — on s'en servira **au
point du besoin**, pas en réserve (doctrine `fine-grained-crates`).

#### Classification des systèmes (règle de migration)
| Cat | Règle | Systèmes (connus) | Action |
|---|---|---|---|
| **A. Timer pur** | `Res<Time>` delta seul, 0 input | `weapon_cooldown_tick`, `melee_cooldown_tick`, `tick_ammo_reload`, `dash_recharge`, `dash_motion`, `sys_tick_run_timer`, `sys_tick_element_status` (DoT), `sys_tick_shockwave_cooldown`, `sys_wave_orchestrator` | move direct → FixedUpdate(set adéquat) |
| **B. Hit-stop** | lit `Time<Virtual>` / relative_speed | `trauma_decay`, `hit_flash_tick`, FpsParams.virtual_time | voir insight hit-stop ↓ |
| **C. Input-edge sim** | lit `just_pressed` pour une action sim | `player_movement` (Jump), `dash_input` (double-tap), `fire`/`burst` (déjà LeftClickState) | move direct → lit `just_pressed` (leafwing 0.20 OK), PAS de buffer |
| **D. Physics-write** | écrit KCC/velocity | `player_movement`, `dash_motion` | le cœur, move avec C |
| **E. Reste Update** | menu/UI/caméra/cosmétique | `mouse_look` (fluidité), `weapon_select` (Digit), `reload_key_input` (→ remplit buffer), `loot_room` choix, `intro_dialogue`, `scoreboard_toggle`, tous les `sys_write_*_sensor`, egui/HUD | NE PAS toucher |

`apply_damage` (forgia-damage/lib.rs:244, bare `Update`, event-driven, 0 input) → move FixedUpdate
GameSet::Combat avec le combat (les `DamageEvent`/`CombatHitEvent` circulent alors dans le step).

#### 💡 Insight hit-stop (R2 peut se résoudre seul)
`Time<Virtual>` pilote l'accumulation de `Time<Fixed>`. Ralentir Virtual (hit-stop
`set_relative_speed`) → **FixedUpdate tourne moins de steps** → la sim ralentit **automatiquement**.
Donc en FixedUpdate, les systèmes lisant `Res<Time>` (= Fixed) avec moins de steps pendant le
hit-stop ralentissent correctement **sans lire Virtual explicitement**. → À **valider runtime** au
slice 3/4 : si le hit-stop marche via le couplage Virtual→Fixed, ne rien changer ; sinon lire
`Time<Virtual>::delta()` explicitement sur les systèmes concernés.

#### Ordre des slices (chacun : `cargo check`/`clippy`/`test` + feel-test runtime par l'user)
*(slice 1 `FixedInput` supprimée — voir finding leafwing ci-dessus ; renumérotation conservée 2→5.)*

- ✅ **Slice 2 — timers purs (cat A non-chaînés)** → FixedUpdate. **FAIT 2026-06-28** (feel-test user
  en attente). Migrés : `weapon_cooldown_tick_system`, `melee_cooldown_tick_system` (forgia-combat) ;
  `sys_tick_element_status` (DoT), `sys_tick_run_timer` (chrono) (forgia-mode-roguelite). Vérif :
  check ✓ / clippy 0 ✓ / test 206 ✓. **Déféré exprès** (chaînés avec input/fire/movement → leur slice) :
  `tick_ammo_reload`→s4, `dash_recharge`/`dash_motion`→s3, `sys_tick_shockwave_cooldown`→s3/4,
  `sys_wave_orchestrator`→s5, `trauma_decay`/`hit_flash`→s3/4 (cat B hit-stop).
- 🔲 **Slice 3 — mouvement + dash (C/D)** : move `player_movement` + chaîne dash en FixedUpdate, lecture
  `just_pressed`/`pressed` **directe** (plus de buffer). Risque **HIGH**. Feel : saut, dash, double-tap,
  **latence input** (vérifier au ressenti que leafwing FixedUpdate ne dégrade pas le twitch).
- 🔲 **Slice 4 — tir + ammo** : `fire_weapon_minimal` (via `LeftClickState`/`MouseButtonInput` brut,
  non-leafwing → vérifier le comportement `MessageReader` en FixedUpdate) + `tick_ammo_reload` → FixedUpdate.
  Feel : tir, burst, semi/pump, hit-stop.
- 🔲 **Slice 5 — damage + cooldowns combat + waves roguelite** : `apply_damage` (forgia-damage),
  `sys_wave_orchestrator`, `sys_tick_shockwave_cooldown` → FixedUpdate.
- 🔲 **Test déterminisme** (transition vers 0.1b) : préparer le harness `run()==run()`.

`mouse_look` reste Update → écrit la rotation caméra dans une Resource (déjà le cas via CameraFov
pipeline ? à vérifier) lisible en FixedUpdate si besoin.

### 🔄 0.1b — Déterminisme (RunRng)
Découverte exécution : `RunSeed` (xoshiro) **existe déjà** + seed capturé/exposé
(`forgia2_roguelite_state.json:"seed"`). Le vrai P0 = la RNG mid-run qui **contournait** RunSeed
(seeds horloge/`toi`/adresse). Résolu par **`CombatRng`** (forgia-combat, DAG-safe, `forgia_rng::Rng`
éphémère par événement via compteur+sel — pas un Rng partagé fragile), reseedé au `StartRunEvent`.

#### ✅ 0.1b-1 — Sources RNG hors horloge (FAIT 2026-06-28, 4 commits)
- ✅ **crit** (`fps/lib.rs`) : `elapsed_secs ^ toi.to_bits()` → `shot_stream(pellet, CRIT_SALT)`.
- ✅ **recoil yaw** (`fps` juice) : `elapsed_secs*1000` → `shot_stream(0, RECOIL_SALT)`. `begin_shot()` avant juice.
- ✅ **loot drop** (`run.rs` observer mort) : `elapsed_secs ^ entity.to_bits()` → `drop_stream(LOOT_SALT)`.
- ✅ **pellet spread** (`fps`) : seed position f32 → `shot_stream(0, SPREAD_SALT)`.
- `default_seed_from_clock` **conservé** (variété run, seed capturé dans RunSeed → rejouable).
- Runtime validé : crit proc à 20% avec boon Œil de Lynx (4× `CRIT! dmg`) ; distribution unit-prouvée uniforme.

#### 🐛 Bug latéral corrigé (commit `6dceb32`)
`sys_recompute_boon_mods` avait une garde `is_changed` qui ratait les picks → **boons inertes**
(crit/damage jamais appliqués). Recompute rendu **inconditionnel** (log au changement). Sans ce fix,
0.1b-1 crit était invalidable (crit_chance figé à 0%).

#### 🔲 0.1b-2 — Ordre d'itération (death/AOE) — DIFFÉRÉ avec la physique
Reste : ordre des `DeathEvent` (despawn_dead_cubes archetype-iter) + AOE combustion (`elements.rs`,
trier `combust.buf` par `Entity::to_bits`). **Couplé au déterminisme de la sim** (mouvement/physique en
FixedUpdate, différé) : sans ordre de systèmes/physique déterministe, `run()==run()` bit-exact reste
hors de portée. À faire avec le workstream physique-FixedUpdate.

## Acceptance criteria
- [x] 0.1a-1 : chaîne GameSet déclarée en FixedUpdate, forgia-core compile, 0 effet runtime.
- [ ] 0.1a-2 : systèmes sim en FixedUpdate, feel validé (latence + hit-stop OK). Slice 2 (4 timers) faite,
  feel-test user en attente ; slices 3-5 à faire (lecture `just_pressed` directe, pas de couche FixedInput).
- [~] 0.1b-1 : sources RNG combat hors horloge (crit/recoil/loot/spread via CombatRng) — FAIT,
  runtime validé (crit proc 20% via boon). + fix boons inertes (`sys_recompute` inconditionnel).
- [ ] 0.1b-2 : ordre d'itération (DeathEvent/AOE) — différé avec le workstream physique-FixedUpdate.
- [ ] 0.1b-3 : test `run()==run()` headless vert (nécessite 0.1b-2 + sim/physique déterministe).

## Cross-refs
- Spike de-risking (surface + ruptures déterminisme).
- Lock L7 (GameSet chain) — **étendu** (FixedUpdate ajouté), pas modifié (Update intact).
