# Story-545 — Bot raycast self-hit : player invincible Roguelite

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_combat.json`, fichier `enemies.rs`, symbole `DamageEvent`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

**Status** : DRAFT
**Priorité** : 🔴 P0 — gameplay Roguelite cassé (player invincible, run sans risque)
**Scale BMAD** : Quick (≤3 fichiers)
**Origine** : 2026-05-27 audit wiring session — user signale "HP bar valeur figée". Sensor `forgia2_combat.json` révèle :
- `damage_dir.events_received: 0` (41s gameplay)
- `screen_flash.damage_flashes_session: 0`
- 176 shots tirés par player, 17 kills, mais 0 dégâts reçus

## Symptôme

Player en mode Roguelite ne prend **jamais** de dégâts. Les bots tirent (visible via tracers ?) mais leurs raycasts ne génèrent jamais de `DamageEvent` ciblant le player.

## Cause probable

`forgia-ai-arena-bot/src/lib.rs:330` :
```rust
let predicate = |e: Entity| e != bot_entity;
let filter = QueryFilter::default().predicate(&predicate);
let hit = ctx.cast_ray(origin, shot_dir, config.range, true, filter);
```

Pour Arena (V7), les ArenaBots = primitive Capsule3d, 1 seul Entity, predicate suffisant.

**Pour Roguelite (story-471 V7 M2)**, les enemies = SceneRoot KayKit Skeleton + child collider (réf `reference_roguelite_enemy_skeleton_mapping.md` : "Pattern parent + 2 children (visual SceneRoot + collider invisible)"). Le `bot_entity` du shoot system = root parent, mais le `Collider` est sur child → predicate `e != bot_entity` **n'exclut pas le child** → ray bloqué par bot's own collider → `hit_entity != target_entity` → DamageEvent jamais émis.

Pattern miroir déjà résolu côté player hitscan : [`reference_hitscan_exclude_sensors_walk_name.md`](../../../../d--Forgia/memory/reference_hitscan_exclude_sensors_walk_name.md) + [`reference_los_exclude_rigid_body_split_parent_child.md`](../../../../d--Forgia/memory/reference_los_exclude_rigid_body_split_parent_child.md).

## Fix proposé

`forgia-ai-arena-bot/src/lib.rs:330` — remplacer predicate par `exclude_rigid_body` :

```rust
let filter = QueryFilter::default().exclude_rigid_body(bot_entity);
```

Rapier 0.33 `exclude_rigid_body` traverse la hiérarchie collider→rigidbody et exclut **toute la chaîne** attachée au RigidBody root. Marche pour les 2 archetypes (Arena root-only et Roguelite skeleton-child).

Vérifier aussi `walk_named_ancestor` côté hit resolution : si `hit_entity` est un child collider du **player**, comparer ancestor de `hit_entity` à `target_entity` (Player) au lieu d'égalité stricte ligne 337.

## Critères d'acceptation

- [ ] AC1 — `forgia-ai-arena-bot/src/lib.rs:331` migre vers `QueryFilter::default().exclude_rigid_body(bot_entity)`
- [ ] AC2 — Si AC1 insuffisant : walk ChildOf ancestors sur `hit_entity` pour résoudre `Player`/`BotTarget` (pattern story-484 hitscan player)
- [ ] AC3 — `cargo check -p forgia-ai-arena-bot` + `cargo clippy -p forgia-ai-arena-bot --no-deps` 0 warning
- [ ] AC4 — Sensor `forgia2_combat.json` après 30s de Roguelite gameplay montre `damage_dir.events_received > 0` ET `screen_flash.damage_flashes_session > 0`
- [ ] AC5 — Player Health visible décroitre dans HUD HP bar pendant exposure
- [ ] AC6 — Régression Arena : tests existants + smoke test Arena mode → bots tirent toujours player Arena, damage applied

## Test in-game recap

1. **Action** : `cargo run -p forgia-game --profile release-fast` → menu → Roguelite, attendre wave 1, **rester immobile** près des bots 10-15s sans cover
2. **Redémarrage requis** — modif `.rs`
3. **Effet attendu** :
   - HP bar **diminue progressivement** (visible bandes cel-shaded grâce à toon)
   - Vignette rouge `low_hp_active` activée si HP < 30%
   - Sensor `forgia2_combat.json` : `damage_dir.events_received > 0` après 5-10s
4. **Sensor** :
   - `forgia2_combat.json::damage_dir.events_received` (cible > 0)
   - `forgia2_combat.json::screen_flash.damage_flashes_session` (cible > 0)
   - `forgia2_player_hp_diag.json::last_hp_current` (cible < 100)
5. **Variantes si KO** :
   - `events_received` toujours 0 → fix AC1 insuffisant, appliquer AC2 (walk ancestor)
   - Damage trop fort/mort instantanée → tuner `BotShootConfig.damage` par archetype (`forgia-mode-roguelite/src/enemies.rs:bot_shoot_for`)
   - Damage Arena cassé en régression → revert + isoler le fix Roguelite-only via marker `EnemyArchetype` query

## Cross-refs

- [`reference_hitscan_exclude_sensors_walk_name.md`](../../../../d--Forgia/memory/reference_hitscan_exclude_sensors_walk_name.md) — pattern player hitscan
- [`reference_los_exclude_rigid_body_split_parent_child.md`](../../../../d--Forgia/memory/reference_los_exclude_rigid_body_split_parent_child.md) — pattern LOS Rapier
- [`reference_roguelite_enemy_skeleton_mapping.md`](../../../../d--Forgia/memory/reference_roguelite_enemy_skeleton_mapping.md) — pattern skeleton parent+child collider
- Story-484 (référence historique fix hitscan player)
