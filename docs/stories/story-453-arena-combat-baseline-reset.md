# Story-453 — V2 Arena Combat Baseline Reset

**Status** : DONE (à valider runtime user)
**Scale** : BMAD Standard (~5 fichiers)
**Date** : 2026-05-18

## Post-impl

- `cargo check -p forgia-mode-fps-arena -p forgia-fps` : 0 erreur
- `cargo clippy ... --no-deps` : 0 warning
- `cargo check --workspace` : 0 erreur (1 warning pre-existing forgia-websocket)
- Code supprimé : ~270 lignes (calibration, viz sync, HitboxVizDims, NeedsBotCalibrate, HitZone Body/Head split, wallbang scan, sphere cast forgiveness, HitApplyCtx.zones ChildOf walk)
- Code ajouté : ~100 lignes (capsule capsule_y + 1 mesh debug viz au spawn, BotHitbox marker, direct Health lookup damage path)

## Pourquoi

Combat V2 FPS accumulé 8 couches de fixes en cascade (clamp T-pose, wallbang scan, sphere cast forgiveness, AABB calibration, HitboxVizDims, sync debug viz, floor sink clamp, HP hot-reload). Plus de baseline pour valider. Reset aux fondations.

## Architecture cible — minimale, prédictible, hot-reloadable

**Bot entité** :

- 1 entité parent avec : Transform, Health, TargetCube, ArenaBot, BotShootConfig, `RigidBody::Fixed`, `Collider::capsule_y(half_h, radius)`
- 1 enfant CharacterMesh (SceneRoot) avec character_y_offset pour pivot
- **PAS** de Body/Head split
- **PAS** de HitZone Component (ray hit le parent direct)

**TOML schema simplifié** :

```toml
hp = 30.0
body_radius = 0.40
body_half_h = 0.65
[ai]
shot_range = 35.0
shot_cooldown_secs = 1.5
shot_damage = 12.0
```

**Damage path** :

- Ray hits entity → if `Health` Component present + `TargetCube` → apply damage direct
- 1 hit = damage × falloff (pas de zone multiplier)
- Skill ceiling : tu vises bien = tu tues. Pas de headshot pour l'instant.

**Code SUPPRIMÉ** :

- `calibrate_bot_hitboxes` system + tous appelants
- `NeedsBotCalibrate` Component
- `HitboxVizDims` Component
- `sync_bot_debug_viz` system (remplacé par debug mesh créé au spawn directement)
- Wallbang `intersect_ray` scan-all
- `HitApplyCtx.zones` query (ChildOf walk → direct)
- `HitZone` enum (deprecated, retire spawn)

**Code GARDÉ** :

- Predicate récursif (anti self-hit)
- HitscanSensor (diagnostic)
- `sync_existing_bot_hp` (hot-reload)
- Floor sink Y clamp (devient trivial avec capsule_y)

## Acceptance criteria

- [x] `cargo check -p forgia-mode-fps-arena -p forgia-fps` 0 erreur
- [x] `cargo clippy ... --no-deps` 0 warning
- [ ] Bot capsule visible à l'oeil = capsule hitbox (no calibration drift)
- [ ] Tire sur bot → damage applied → bot meurt en 1-2 hits (HP 30, damage 28+)
- [ ] Tire à côté du bot → miss
- [ ] Player ne sink plus dans le sol
- [ ] Sensor `forgia_hitscan.json` montre `hits_with_damage` qui monte significativement (>50%)

## Hors scope

- Headshot multiplier (re-add couche par couche après baseline OK)
- Aim forgiveness sphere cast (re-add si M+K trop tight après test)
- Wallbang penetration (re-add si bot caché derrière cover frustre)
- AABB auto-calibration (re-add si user veut hitboxes mesh-driven)
