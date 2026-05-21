# Story-490 — Roguelite Damage Routing Bridge (forgia_combat Health + DeathEvent trigger)

**Status:** IN PROGRESS
**Scale:** BMAD Standard (3 fichiers, story requise, checklist post-impl)
**Created:** 2026-05-21
**Blocks:** AC6/AC7 runtime validation story-485 (gameplay-bloquant)
**Related:** memory `[[reference-dual-health-type-trap]]`, `[[reference-bevy-rapier-child-collider-pattern-2026-05-20]]`, `[[reference-v7-damage-pipeline-dual-path]]`

---

## 1. Contexte

Runtime story-485 validation a révélé sensor `forgia2_combat.json` :
```
"hits_with_damage": 0,
"hits_blocked_by_world": 15,
```

Tous les hits sur ennemis Roguelite (`RogueliteEnemy_W1_tank_0_collider`) sont
classés `blocker` au lieu de `damage`. Root cause identifiée par sub-agent
Explore (audit complet en conversation) :

- **Roguelite parent** (`waves.rs:127`) porte `forgia_damage::Health`
- **FPS hitscan** (`forgia-fps/lib.rs:235-245`) query `forgia_combat::Health` avec `With<TargetCube>`
- Type mismatch silencieux dans `find_health_ancestor` → walk renvoie `None` →
  classification fallback `BlockerNonZone`

C'est exactement le `[[reference-dual-health-type-trap]]` capitalisé en
memory mais jamais résolu côté Roguelite.

## 2. Goals

1. Débloquer damage routing Roguelite ennemis (gameplay critique)
2. Préserver le path DeathEvent pour loot pickup / defeat detection
3. Minimal surface change — pas de refactor V7 pipeline (= story-491 future)
4. Aligner Roguelite sur le pattern FPS Arena qui marche

## 3. Non-Goals

- Migration V7 complète vers `forgia_damage::Health` uniforme → **Story-491** future
- HUD ammo / player_hp rendering manquant → story dédiée (UI scope distinct)
- Modifier la signature hitscan ou ajouter SystemParam union dual-Health

## 4. Acceptance Criteria

- [ ] AC1 — `forgia-mode-roguelite::waves::spawn_wave_enemies` spawn parent
      avec `forgia_combat::Health` (au lieu de `forgia_damage::Health`)
- [ ] AC2 — `forgia-fps::despawn_dead_cubes` trigger `DeathEvent` AVANT despawn
- [ ] AC3 — Runtime sensor `forgia2_combat.json::hits_with_damage > 0` après
      avoir tiré sur un ennemi en stage `crypts_of_anvil`
- [ ] AC4 — Runtime sensor `forgia2_combat.json::killfeed::total_kills_session > 0`
      après avoir tué un ennemi
- [ ] AC5 — Runtime : pickup Souls apparaît visuellement à la position de
      l'ennemi mort (Observer DeathEvent → spawn pickup déclenché)
- [ ] AC6 — Compteur `Souls` HUD incrémente au walk-over pickup
- [ ] AC7 — `cargo check -p forgia-mode-roguelite -p forgia-fps -p forgia-damage` clean
- [ ] AC8 — `cargo clippy --no-deps --tests -- -D warnings` 0 warning
- [ ] AC9 — Tests existants restent verts (89 stage-arena + tests fps)

## 5. Architecture & Patterns

### 5.1 Roguelite waves.rs — swap import Health

```diff
- use forgia_damage::{Health, Mortal};
+ use forgia_combat::Health;
+ use forgia_damage::Mortal;
```

Le marker `Mortal` reste sur les ennemis (consommé par futures stories
respawn). API `Health::new(stats.hp)` identique entre les 2 crates (champs
`current`, `max`, ctor `new(max)`).

### 5.2 forgia-fps despawn_dead_cubes — bridge DeathEvent

```diff
fn despawn_dead_cubes(
    mut commands: Commands,
    q: Query<(Entity, &Health), With<TargetCube>>,
) {
    for (entity, hp) in &q {
        if hp.is_dead() {
+           // Story-490 — bridge V7 pipeline : trigger DeathEvent avant
+           // despawn pour que les observers Roguelite (loot pickup, defeat
+           // detection) puissent réagir. Sans ça, dead code.
+           commands.trigger(DeathEvent {
+               target: entity,
+               source: None,
+               final_kind: DamageKind::Physical,
+           });
            if let Ok(mut ec) = commands.get_entity(entity) {
                ec.try_despawn();
            }
            info!("[death] cube {:?} despawned (HP=0)", entity);
        }
    }
}
```

Note : `source: None` car `despawn_dead_cubes` n'a pas l'info attaquant à
ce point (le hitscan a déjà appliqué le dmg + dispatché `CombatHitEvent`).
Pour précision attaquant, story-491 émettra DamageEvent en amont avec
source attaquant.

### 5.3 Imports `forgia-fps`

```diff
use bevy_rapier3d::prelude::*;
+ use forgia_damage::{DamageKind, DeathEvent};
use forgia_combat::prelude::*;
```

(`HitZone` déjà importé via `forgia_damage::HitZone` plus bas — dep OK.)

## 6. Plan d'implémentation

### Phase 1 — Swap Roguelite Health type (S, 5 min)

- `waves.rs:20` split import
- `cargo check -p forgia-mode-roguelite` clean

### Phase 2 — Bridge DeathEvent dans despawn_dead_cubes (S, 5 min)

- `forgia-fps/lib.rs` add imports `DamageKind, DeathEvent`
- `despawn_dead_cubes` trigger DeathEvent avant despawn
- `cargo check -p forgia-fps` clean

### Phase 3 — Verification (S, 10 min)

- `cargo check -p forgia-game` clean
- `cargo clippy --no-deps --tests -- -D warnings` 3 crates touchées
- `cargo test -p forgia-mode-roguelite -p forgia-fps` regression check

### Phase 4 — Runtime test (S, 5 min)

- `cargo build -p forgia-game --profile release-fast`
- Run game, entrer Roguelite stage
- Tirer sur ennemis, vérifier sensors :
  - `forgia2_combat.json::hits_with_damage` > 0
  - `forgia2_combat.json::killfeed::total_kills_session` > 0
- Souls pickup spawn + walk-over → compteur Souls HUD incrémente

## 7. Risques

| Risque | Mitigation |
|---|---|
| `Mortal` marker fait référence à `forgia_damage::Mortal` traits | grep workspace : 0 system query `With<Mortal>`, safe |
| DeathEvent trigger 2× pour même entité (race avec Roguelite damage path) | `despawn_dead_cubes` query `With<TargetCube>` — ennemis FPS+Roguelite mais pas Player. Player keeps forgia_damage::Health, hits via DamageEvent path indépendant. |
| Loot pickup observer attend `source: Some(Player)` pour crediter Souls | Vérifier `run.rs:257` Observer code — si filtre par source, story-490 b nécessitera pass attaquant entity au despawn |
| Player Health switching | Player garde `forgia_damage::Health` (run.rs:263, stations.rs:142) — pas touché |

## 8. Definition of Done

- Tous AC §4 verts
- Sensor runtime preuve damage routing
- Commit propre + record memory `[[reference-roguelite-damage-bridge-pattern]]`
- Update story-485 AC6/AC7 status DONE-runtime

## 9. Follow-ups (stories candidates)

- **Story-491** Migration V7 uniforme : hitscan FPS émet `DamageEvent`, tous
  ennemis (FPS+Roguelite) sur `forgia_damage::Health`. Refactor + tests.
- **Story-492** Loot pickup attribution attaquant : DeathEvent.source =
  attaquant entity → crédit Souls au bon joueur (multi-coop futur).
