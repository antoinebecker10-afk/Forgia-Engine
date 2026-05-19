# Story-449 — V2 Arena Bot Hitbox Auto-Calibrate

**Status** : DONE (à valider runtime user)
**Scale** : BMAD Standard
**Date** : 2026-05-18

## Post-impl

- `cargo check -p forgia-mode-fps-arena` : 0 erreur
- `cargo clippy -p forgia-mode-fps-arena --no-deps` : 0 warning
- Pattern aligné `forgia-viewmodel-calibration` (BFS Children walk + Aabb local-space)
- Log `info!` par calibration : `[arena-bot-calibrate] entity=X body_size=(w,h,d) head_radius=r`
- Idempotent : marker `NeedsBotCalibrate` retiré post-resize

## Contexte

User 2026-05-18 : "quand je tire à côté d'un ennemi ça lui met des dégâts alors que ça doit être sur son mesh".

Diagnostic : `arena_bots.toml` définit Body `Cuboid(0.7, 1.3, 0.4)` et Head `Ball(0.22)` à dimensions fixes. Les characters Meshy AI ont des silhouettes plus fines que le Cuboid → hit "à côté" damage quand même.

## Approche

Pattern `NeedsAssetCalibrate` (V1 réf [reference_v2_needs_asset_calibrate.md]) étendu aux bots :

1. Tag chaque character SceneRoot d'un Component `NeedsBotCalibrate { body: Entity, head: Entity }`
2. Système Update mesure le AABB agrégé des descendants `Mesh3d` une fois la scene loaded (via `Aabb` Bevy + GlobalTransform → world-space corners)
3. Split vertical à 80% hauteur : 80% bas = body, 20% haut = head
4. Replace Collider + Transform sur body/head entities sibling
5. Remove marker (idempotent)

## Scope

Fichiers impactés :
- `crates/forgia-mode-fps-arena/src/lib.rs` (ajout Component `NeedsBotCalibrate` + système calibrate)
- `crates/forgia-mode-fps-arena/src/wave.rs` (attache marker au spawn + entity refs)

**Hors scope** : changer le pattern HitZone Head/Body (reste primitive overlay).

## Acceptance criteria

- [ ] `cargo check -p forgia-mode-fps-arena` 0 erreur
- [ ] `cargo clippy -p forgia-mode-fps-arena --no-deps` 0 warning
- [ ] Body collider épouse silhouette mesh ± 5cm (runtime user)
- [ ] Head collider centré sur tête mesh (runtime user)
- [ ] Sensor `forgia_arena_waves.json` continue à reporter bots_alive correct
- [ ] Idempotent : marker retiré après calibration, pas de race au respawn

## Marqueur Diagnostic

- Log `info!` chaque calibration : `"[arena-bot-calibrate] entity={e} body_size=({w},{h},{d}) head_radius={r}"`
