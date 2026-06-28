# Story-634 — Keystone : simulation déterministe (FixedUpdate + RunRng)

> **Statut** : 🔵 IN_PROGRESS — 0.1a-1 fait, reste 0.1a-2 + 0.1b.
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

**Le piège prouvé par la cartographie** : `player_movement` (lib.rs:607) et `dash_input_system`
(dash.rs:95) lisent `action.just_pressed(Jump)`. `just_pressed` reste vrai toute la frame `Update` ;
or `FixedUpdate` tourne 0/1/N fois par frame → déplacer tel quel = saut/dash **doublé** (frame lente)
ou **perdu**. Un déplacement naïf `.in_set(Update)→FixedUpdate` casse l'input. Il faut une **couche de
buffering**.

**Bonne nouvelle (cartographie)** : le **tir est déjà découplé** de `ButtonInput` brut
(`forgia-fps/lib.rs:61` `LeftClickState` via `MessageReader<MouseButtonInput>`, car egui consomme
l'input) et `fire_mode` est une **fn pure** (`lib.rs:84`). Les cooldowns/trauma/flash sont des
**timers purs** (aucun input). La moitié du travail est déjà décentralisée.

#### Couche `FixedInput` (prérequis, slice 1)
```
Update (timing input) :
  PendingInput (Resource) ← latch les edges (just_pressed Jump → jump_latched=true ;
    double-tap dash détecté ici ; move_axis snapshoté ; fire via LeftClickState déjà OK)
FixedUpdate, GameSet::Input (1er système du step) :
  drain PendingInput → FixedInput (input résolu DE CE step) ; CLEAR les latches
FixedUpdate sim systems : lisent FixedInput (jamais just_pressed)
```
EdgeLatch = bool set en Update, consommé 1× par le drain FixedUpdate → **press jamais perdu**
(survit jusqu'au prochain step) **ni doublé** (consommé une fois). Slice 1 est **additive** (rien ne
consomme `FixedInput` encore) → 0 changement de comportement, comme 0.1a-1.

#### Classification des systèmes (règle de migration)
| Cat | Règle | Systèmes (connus) | Action |
|---|---|---|---|
| **A. Timer pur** | `Res<Time>` delta seul, 0 input | `weapon_cooldown_tick`, `melee_cooldown_tick`, `tick_ammo_reload`, `dash_recharge`, `dash_motion`, `sys_tick_run_timer`, `sys_tick_element_status` (DoT), `sys_tick_shockwave_cooldown`, `sys_wave_orchestrator` | move direct → FixedUpdate(set adéquat) |
| **B. Hit-stop** | lit `Time<Virtual>` / relative_speed | `trauma_decay`, `hit_flash_tick`, FpsParams.virtual_time | voir insight hit-stop ↓ |
| **C. Input-edge sim** | lit `just_pressed` pour une action sim | `player_movement` (Jump), `dash_input` (double-tap), `fire`/`burst` (déjà LeftClickState) | rewire → lit `FixedInput` |
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

#### Ordre des slices (chacun : `cargo check` + feel-test runtime par l'user)
1. **Slice 1 — `FixedInput`** (additif, 0 comportement). forgia-core ou forgia-input.
2. **Slice 2 — timers purs (cat A)** → FixedUpdate. Risque Low. Feel : cooldowns/DoT identiques.
3. **Slice 3 — mouvement + dash (C/D)** : rewire sur `FixedInput`. Risque **HIGH**. Feel : saut,
   dash, double-tap, latence input.
4. **Slice 4 — tir + ammo** (fire via LeftClickState bufferisé) → FixedUpdate. Feel : tir, burst, semi/pump, hit-stop.
5. **Slice 5 — damage + cooldowns combat + waves roguelite** → FixedUpdate.
6. **Test déterminisme** (transition vers 0.1b) : préparer le harness `run()==run()`.

`mouse_look` reste Update → écrit la rotation caméra dans une Resource (déjà le cas via CameraFov
pipeline ? à vérifier) lisible en FixedUpdate si besoin.

### 🔲 0.1b — Déterminisme (RunRng, le vrai morceau)
`forgia-rng` n'est PAS utilisé par combat/roguelite (seeds horloge). Sites P0 :
`run.rs:845 default_seed_from_clock`, `fps/lib.rs:742/985` (juice/crit seeds, `toi.to_bits()`),
`run.rs:315` loot seed, `elements.rs:772` ordre AOE. → un seul `RunRng` seedé au `StartRunEvent`,
consommé séquentiellement + ordre d'itération stabilisé. **Preuve** : `run()==run()` (même seed → même état) en test headless.

## Acceptance criteria
- [x] 0.1a-1 : chaîne GameSet déclarée en FixedUpdate, forgia-core compile, 0 effet runtime.
- [ ] 0.1a-2 : ~35 systèmes en FixedUpdate, feel validé (latence + hit-stop OK).
- [ ] 0.1b : RunRng remplace tous les seeds-horloge ; test `run()==run()` vert.

## Cross-refs
- Spike de-risking (surface + ruptures déterminisme).
- Lock L7 (GameSet chain) — **étendu** (FixedUpdate ajouté), pas modifié (Update intact).
