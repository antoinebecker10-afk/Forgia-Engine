# Story-466 — DeathEvent → Observer (Bevy 0.18 EntityEvent migration)

**Status** : DONE
**Scale** : Quick (3 fichiers)
**Date** : 2026-05-19
**Parent** : Vague 3 reliquat audit-2026-05-19.md §7

## Symptôme

Vague 3 audit identifie "Adopter Observers pour death/pickup/damage" (~2h
prévu). Aucun Observer dans le workspace V2 actuellement — pattern Bevy 0.18
non adopté.

## Évaluation des candidats

| Event | Producer | Consumers | Verdict |
|---|---|---|---|
| **DeathEvent** | 1 (forgia-damage) | 1 (forgia-ai-arena-bot) | ✅ **MIGRATE** — per-entity, one-shot, scope minimal |
| **DamageEvent** | N (weapons, melee, bot) | 1 (apply_damage) | ⚠️ SKIP — hot path, cascade Observer-from-Observer fragile |
| **CombatHitEvent** | 1 (fps:1130) | 8 consumers (stats/UI) | ❌ SKIP — Message naturel pour fanout statistique |

Seul DeathEvent migré cette story. Les 2 autres restent Message — décision
explicite documentée pour préserver le pattern le mieux adapté.

## Implémentation

**forgia-damage** (`src/lib.rs`) :
- DeathEvent : `#[derive(Message)]` → `#[derive(EntityEvent)]` + `#[event_target]` sur `target`
- Plugin : suppression `add_message::<DeathEvent>()`
- `apply_damage` signature : `MessageWriter<DeathEvent>` → `Commands`
- Production : `deaths.write(...)` → `commands.trigger(DeathEvent {...})`

**forgia-ai-arena-bot** (`src/lib.rs`) :
- `fn handle_bot_deaths(MessageReader<DeathEvent>, ...)` → `fn on_bot_death(event: On<DeathEvent>, ...)`
- Plugin : retiré du `.add_systems(Update, ...)`, ajouté via `.add_observer(on_bot_death)`

## Acceptance Criteria

- [x] DeathEvent derive `EntityEvent` avec `#[event_target]` sur `target` (nom conservé pour API compat).
- [x] `commands.trigger(DeathEvent {...})` route auto vers les Observer per-entity.
- [x] `on_bot_death(event: On<DeathEvent>, ...)` consomme via `event.target`.
- [x] Plugin enregistre `app.add_observer(on_bot_death)` au lieu du system Update.
- [x] `cargo check --workspace` clean, `clippy -D warnings` clean.
- [x] 12/12 tests verts (3 damage + 9 los_gating story-464).

## Bénéfices

- **1 system Update en moins** (handle_bot_deaths polling à chaque frame, même sans event)
- **Pattern idiomatique Bevy 0.18** pour events per-entity one-shot
- **Pilot exemplaire** pour futures migrations (pickup, level-up, etc.)

## Risques résiduels

- Observers s'exécutent SUR le thread principal au moment du `trigger` (vs MessageReader
  qui peut bufferiser). Si beaucoup de morts simultanées (>50/frame), pourrait spiker.
  Non observé en pratique (5-20 bots arena max).

## Sources

- `bevy_ecs-0.18.1::src/event/mod.rs:88-170` (`Event`, `EntityEvent`, `#[event_target]`)
- `bevy_ecs-0.18.1::src/lib.rs:78,83` (prelude exports `EntityEvent`, `On`)
- Audit `docs/audit/audit-2026-05-19.md` §7 Vague 3

## Hors scope

- Migration DamageEvent/CombatHitEvent — patterns Message restent appropriés (décision documentée ci-dessus).
- Pickup events — non implémentés en V2 actuellement (couche RPG M3+).
