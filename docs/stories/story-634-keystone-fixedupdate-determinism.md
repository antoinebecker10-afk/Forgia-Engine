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

### 🔲 0.1a-2 — Migration des systèmes (~35, HIGH, session dédiée)
Déplacer Movement/Combat/Physics-adjacents en `FixedUpdate` : `player_movement`+`dash_*` (forgia-player),
`fire_weapon`+`tick_ammo_reload`+burst (forgia-fps), `*_cooldown_tick`+`trauma_decay` (forgia-combat),
`apply_damage` (forgia-damage), `sys_wave_orchestrator`+`sys_tick_*` (forgia-mode-roguelite).
- `mouse_look` **reste en `Update`** (fluidité caméra) → écrire la rotation dans une Resource lue en FixedUpdate.
- ⚠️ R2 hit-stop : les timers gameplay lisent `Time<Virtual>::delta()` (pas `Time<Fixed>`) pour garder le `relative_speed`.
- Valider le **feel** (latence, hit-stop) via `feel:smoke` headless avant de continuer.

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
