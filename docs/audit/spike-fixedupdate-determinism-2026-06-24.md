# Spike 0.1 — Migration FixedUpdate + déterminisme (READ-ONLY, de-risking)

> Spike de la keystone Phase 0.1 du plan QA intégré (`docs/plan/rpg-qa-integrated-plan-2026-06-24.md`).
> Objectif : mesurer la surface + le risque AVANT d'éditer. **Aucune édition** (zone de churn active).
> Produit le 2026-06-24 via 2 agents read-only (surface migration + ruptures déterminisme).
>
> **Verdict : GO, par étapes — et 3 corrections du plan.**

---

## 0. Découverte qui change le risque : Rapier est DÉJÀ en FixedUpdate

`forgia-game/src/lib.rs:58` ajoute `RapierPhysicsPlugin::<NoUserData>::default()` — sur bevy_rapier
0.33 / Bevy 0.18, **la physique step en `FixedUpdate` à 64 Hz par défaut**. Or **tout le gameplay
(mouvement, combat, roguelite) est en `Update`** (variable). Conséquence :

- Il existe **déjà une désync latente** : `player_movement` écrit le `KinematicCharacterController`
  depuis `Update`, hors du step physique FixedUpdate.
- Migrer le gameplay en FixedUpdate ne **combat pas** Rapier — il **s'aligne** dessus et **corrige** ce
  bug latent. Le risque #1 redouté (physique vs logique désynchronisées) est en réalité **résolu** par la
  migration, pas créé.
- `Time::<Fixed>::from_hz(N)` réglera **à la fois** le FixedUpdate Bevy ET le rate Rapier (couplés au même
  schedule).

→ **Le verrou Rapier est favorable. C'est le feu vert principal du spike.**

---

## 1. Trois corrections du plan

| Plan (0.1) disait | Spike corrige |
|---|---|
| « FixedUpdate **20 Hz** » | **60 Hz** (ou 30 mini). 20 Hz = 50 ms latence input, inacceptable pour un FPS twitch ; et ça ralentit la résolution de collisions Rapier. **Le déterminisme vient du « fixed », pas du « 20 »** — n'importe quel rate fixe suffit. WoC = 20 Hz car MMO top-down, pas FPS. |
| « FixedUpdate **+** discipline seed » (un bloc) | **Deux workstreams distincts** : (0.1a) migration FixedUpdate = timing cohérent + alignement Rapier ; (0.1b) déterminisme = RunRng + tuer les seeds-horloge + ordre d'itération. FixedUpdate **seul ne donne PAS** le déterminisme. |
| « ~1 sem, High » | Surface réelle : **~35-40 systèmes, 5 crates**. 0.1a Medium (Rapier favorable) ; 0.1b est le vrai morceau délicat. |

---

## 2. Surface de migration (0.1a)

**Zéro système en FixedUpdate aujourd'hui ; ~62 systèmes gameplay en `Update`.**

**DOIT bouger** (logique temps-sensible, intègre `delta_secs`) : `player_movement` + `dash_*` +
`player_floor_safety_net` (forgia-player) ; `fire_weapon` + `tick_ammo_reload` + burst (forgia-fps) ;
`weapon_cooldown_tick` + `melee_cooldown_tick` + `trauma_decay` (forgia-combat) ; `apply_damage`
(forgia-damage) ; `sys_wave_orchestrator` + `sys_tick_run_timer` + `sys_tick_element_status` (DoT) +
`sys_tick_shockwave_cooldown` (forgia-mode-roguelite, ~35 systèmes).

**Peut rester en Update** (input/cosmétique/télémétrie) : `mouse_look` (fluidité caméra — pattern :
écrire la rotation dans une `Resource` lue depuis FixedUpdate), VFX/hit-flash, tous les `sys_write_*_sensor`,
les sync genome event-driven, observers/OnEnter/OnExit.

### Pré-requis bloquant R1 — la chaîne GameSet
`forgia-core/src/lib.rs:107` configure la chaîne `GameSet` **uniquement sur `Update`** :
```rust
.configure_sets(Update, (Network, Input, Movement, Physics, Camera, Combat, Effects, Sensors, UI).chain())
```
Déplacer des sets en FixedUpdate **sans** un second `.configure_sets(FixedUpdate, (...).chain())` = ordre
d'exécution non défini. **Premier travail avant tout déplacement.**

### Risque R2 — hit-stop `Time<Virtual>`
forgia-fps fait `virtual_time.set_relative_speed(hs_speed)` (hit-stop). En FixedUpdate, lire
`Time<Fixed>::delta()` pour les timers ignorerait le `relative_speed` → casse le ressenti du hit-stop.
Les timers gameplay devront lire `Time<Virtual>::delta()`, pas `Time<Fixed>`. À valider au spike.

---

## 3. Ruptures de déterminisme (0.1b) — le vrai morceau

`forgia-rng` (xoshiro256++ déterministe) **n'est PAS utilisé** par le combat/roguelite : ils utilisent
`rand_xoshiro` brut OU des seeds ad-hoc basés sur l'horloge. Sites **bloquants** :

| Site | Problème | Fix |
|---|---|---|
| `roguelite/run.rs:845` `default_seed_from_clock()` | seed run = `SystemTime::now().as_nanos()` si pas de seed | **P0** : seed toujours `Some`, généré 1× depuis une `SessionRng` |
| `fps/lib.rs:742` `juice_seed = elapsed_secs*1000` | direction recoil/shake dépend du timing de frame | **P0** : `RunRng` séquentiel |
| `fps/lib.rs:985` `crit_seed ^= toi.to_bits()` | mêle temps écoulé **+ time-of-impact Rapier** (ordre BVH non déterministe) | **P0** : compteur de tirs via `RunRng`, jamais `toi` |
| `roguelite/run.rs:315` loot seed = `elapsed*1000 ^ entity` | drop conditionnel (heart 35% low-HP) non reproductible | **P0** : `RunRng` |
| `roguelite/elements.rs:772,797` AOE combustion | `Query::iter()` ordre archétype → **qui meurt en premier change** | **P0** : collecter, trier par `Entity::to_bits()`, appliquer |
| `fps/lib.rs:867` spread pellets via position f32 | sensible arrondi f32 cross-plateforme | P2 |
| Chaîne dégâts 100% f32, aucun `round()` | divergence ~0.001 HP/50 coups → replay non exact | P3 (option WoC : HP entiers) |

**Insight clé** : un seul `RunRng: Xoshiro256StarStar` (ou `forgia_rng::Rng`) initialisé au `StartRunEvent`,
consommé **séquentiellement** (tir, crit, drop, spawn), remplace TOUS les seeds ad-hoc. Comme WoC : l'ordre
des appels EST le monde → fixer l'ordre des systèmes consommateurs (`decor/poi/run/waves`) via
`before/after`.

---

## 4. Plan staged + GO/NO-GO

**GO**, parce que Rapier est favorable et la surface est cernée. Ordre :

1. **0.1a-1 — GameSet en FixedUpdate** (forgia-core) : ajouter `.configure_sets(FixedUpdate, chain)`.
   `Time::<Fixed>::from_hz(60.0)` dans forgia-game. Risque Low, fondation.
2. **0.1a-2 — migrer les ~35 systèmes** Movement/Combat/Physics-adjacents en FixedUpdate, `mouse_look`
   reste Update (rotation via Resource). Valider feel (hit-stop R2, latence) — **`feel:smoke` headless**.
3. **0.1b-1 — `RunRng`** : resource seedée au StartRunEvent, tuer `default_seed_from_clock` + tous les
   seeds `elapsed_secs`/`toi.to_bits()`. **C'est ici que le déterminisme naît.**
4. **0.1b-2 — ordre d'itération** : stabiliser l'AOE (`elements.rs`) + l'ordre des systèmes RNG.
5. **Test déterminisme** : `run()==run()` (2 runs même seed → même état) en test headless — la preuve.

**NO-GO conditions** : si le feel à 60 Hz est mauvais après 0.1a-2 → revenir à Update pour le
sous-ensemble twitch (aim) et garder FixedUpdate pour la sim lente. Si Rapier diverge entre runs même
seedé → le déterminisme exact est hors de portée sans `enhanced-determinism` Rapier (feature à activer).

---

## 5. Gating (multi-terminal)
- Le spike (read-only) est **fait**. L'**implémentation** touche `forgia-mode-roguelite` (~35 systèmes) +
  forgia-player/fps/combat — **zone de churn active de l'user**. À lancer **sur arbre propre** uniquement.
- 0.1a-1 (GameSet/forgia-core) est isolé et peut démarrer en premier (forgia-core hors churn).
- Supersede la ligne 0.1 du plan (20 Hz → 60 Hz ; un bloc → 0.1a/0.1b).

*Sources : agents read-only (surface migration + ruptures déterminisme), file:line dans le corps.
forgia-rapier déjà FixedUpdate 64 Hz = découverte centrale.*
