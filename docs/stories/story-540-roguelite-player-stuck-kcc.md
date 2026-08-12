# Story-540 — Player stuck KCC contre modules intérieurs (Roguelite Crypts of Anvil)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_combat.json`, fichier `layout.rs`, symbole `Collider`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

**Status:** DRAFT
**Scale:** BMAD Standard (cross-crate investigation, story requise, checklist post-impl)
**Created:** 2026-05-27
**Blocks:** Run Roguelite jouable jusqu'au boss
**Related:** memory `[[reference-bevy-rapier-child-collider-pattern-2026-05-20]]`, `[[reference-hitscan-exclude-sensors-walk-name]]`, `[[reference-los-exclude-rigid-body-split-parent-child]]`

---

## 1. Contexte

Session test Roguelite 2026-05-27 (stage `crypts_of_anvil`, depth 0, wave 2) :
- Player a marché droit du spawn vers le BossPad
- À position (3.58, 1.12, -53.54) — ~9.5 m du BossPad `(0, 0, -63)` — le player s'est retrouvé bloqué
- `forgia2_player_state.json` SEVERITY CRITICAL :
  ```
  stuck_frames_consecutive: 538 (~9s),
  stuck_events_session: 5,
  velocity_planar_m_s: 0.000,
  grounded: true,
  kcc_collisions: 0
  ```
- Hitscan sensor `forgia2_combat.json` montre 3 `blocker` hits consécutifs sur **`hit_entity_idx: 30064769224`** (même entité, 3 fois), `hit_name: null`, toi décroissant 1.4 → 0.46 → 0.30 m
- Le jeu n'a pas crashé techniquement (498 FPS, 14 457 ticks, 28 bots alive, run_state=in_run, RAM stable) — c'est un **stuck gameplay** que l'user perçoit comme "plantage"

Pattern récurrent (`stuck_events_session: 5`) — pas un cas isolé.

## 2. Diagnostic préliminaire

**3 hypothèses concurrentes** (à départager phase 1) :

| # | Hypothèse | Évidence pro | Évidence contra |
|---|---|---|---|
| **H1** | Module GLB (cover_high_wall / sniper_perch) sans `Collider` Rapier mais avec visual mesh qui clip KCC | `hit_name: null` (Name pas propagé sur child GLB), `kcc_collisions=0`, module palette posée à 7.58 m min spacing | À vérifier : `spawn_gltf_prefab` ajoute-t-il un Collider via AsyncSceneCollider ? |
| **H2** | 2 modules placés en overlap → cage pincée | `min_cover_spacing_m=7.58` (proche de KCC capsule diameter ~0.6m, mais 2 props peuvent être à 3.5m chacun de player) | À vérifier : layout.rs solver respecte-t-il le `footprint_radius_m` lors de la dart-throw ? |
| **H3** | BossPad scale énorme → collider déborde | Player stuck 9.5m du BossPad | Player pas pile sur le boss_pad, plus probable un module intermédiaire |

Position player stuck : `(3.58, 1.12, -53.54)`
- Distance au BossPad (0, 0, -63) : 9.6 m
- Hex apothème (mur le plus proche) : 90 × √3/2 ≈ 77.9 m → ramparts loin
- Zone : **moitié nord de l'arène, sur axe boss-player approximatif**

## 3. Goals

1. Identifier la cause exacte du stuck (1 phase d'audit avant tout patch)
2. Garantir qu'aucun prop module ne peut piéger le player (collider hygiene OU spacing minimum corrigé)
3. Préserver le layout sight-line solver (story-485) — pas de revert du module placement
4. Ajouter un test runtime sensor anti-régression

## 4. Non-Goals

- Refactor KCC controller (forgia-player kinematic) — out of scope
- Modification du sight-line solver story-485 (sauf si spacing root cause)
- Module placement RNG seed-based deterministic preview (story future si récurrent)
- Auto-respawn / téléport joueur en cas de stuck (band-aid)

## 5. Acceptance Criteria

- [ ] AC1 — Phase 1 investigation : identification claire de H1/H2/H3 documentée
- [ ] AC2 — `forgia2_player_state.stuck_events_session = 0` sur run Roguelite 60s wave 1+2
- [ ] AC3 — `forgia2_combat.json` : aucun `blocker` consécutif sur même `hit_entity_idx` avec `hit_name: null`
- [ ] AC4 — Module placement layout solver respecte un `safe_corridor_radius` ≥ 1.5 m autour de l'axe player-spawn → boss-pad (corridor de circulation)
- [ ] AC5 — Tous les GLB props spawned par `forgia-prefab::spawn_gltf_prefab` ont un Name component propagé au walk_ancestors (fix `hit_name: null`)
- [ ] AC6 — Sensor `forgia2_player_state.json` ajoute `last_stuck_position` + `last_stuck_nearby_entity_name` pour debug futur
- [ ] AC7 — Test runtime : 3 runs successifs Crypts of Anvil sans stuck event
- [ ] AC8 — `cargo check --workspace` + `cargo clippy --workspace --no-deps -- -D warnings` 0 warning
- [ ] AC9 — Tests purs forgia-stage (28) + forgia-level-presets (26) restent verts
- [ ] AC10 — `forgia2_stage_layout.min_cover_spacing_m` reste ≥ 3.0 m (ne dégrade pas l'identité spatial story-485)

## 6. Architecture & Patterns

### 6.1 Phase 1 — Investigation (read-only, pas de patch)

Trois fichiers à lire dans cet ordre :

1. **`forgia-prefab/src/lib.rs`** : `spawn_gltf_prefab` ajoute-t-il un Collider ? Quelle stratégie (AsyncSceneCollider, ColliderConstructor, manual) ?
2. **`forgia-stage/src/layout.rs`** : `place_modules` solver — quel critère pour respecter le corridor player-boss ? Y a-t-il un `safe_radius_around_axis` ?
3. **`forgia-player/src/*.rs`** : config KCC controller — `slide`, `autostep`, `up_direction`, `offset`, `snap_to_ground` — détecter si penetration_resolution est activée

Mempalace triple à créer : `<module_placement> intersects <player_corridor>` ou `<glb_prefab> missing <collider>`.

### 6.2 Phase 2 — Fix selon hypothèse retenue

**Si H1 (GLB sans collider)** :
```rust
// Dans forgia-stage spawn block module placements (lib.rs:993+)
let spawn = PrefabSpawn::new(&prop.prefab, placement.position)
    .with_name(format!("Module_{}_{idx}", placement.module_id))
    .with_collider_strategy(ColliderStrategy::ConvexHullFromMesh); // ← AJOUT
```
ou si AsyncSceneCollider Rapier 0.33 :
```rust
.with_async_scene_collider(AsyncSceneCollider {
    shape: Some(ComputedColliderShape::ConvexHull),
    ..default()
})
```

**Si H2 (overlap modules)** :
- Patcher `layout.rs::place_modules` : ajouter un `corridor_keepout` rectangle entre `player_pos` et `boss_pos` (largeur 3m) où aucun module ne peut être placé
- Test pur : `corridor_clear_player_to_boss(placements, player, boss, width=3.0)`

**Si H3 (BossPad scale)** :
- Limiter `with_scale((boss.size_m / 4.0).max(1.0).min(2.5))` (cap supérieur)
- Audit genome `boss.size_m` value

### 6.3 Phase 3 — Name propagation (AC5)

Pattern memory `[[reference-hitscan-exclude-sensors-walk-name]]` :
```rust
// Dans forgia-prefab après spawn scene, walk children et propager Name si absent
fn propagate_name_to_glb_children(parent: Entity, world: &mut World) {
    // Pour chaque descendant sans Name<>, insert Name::new(format!("{parent_name}_child"))
}
```

### 6.4 Phase 4 — Sensor enrichi (AC6)

Dans `forgia-player::player_state_sensor` :
```rust
// Quand stuck_frames_consecutive > 60, capture entity nearest player
fn capture_stuck_context(player_pos: Vec3, rapier: &RapierContext, query_named: Query<&Name>) -> Option<String> {
    // Sphere query 1.5m autour player → entity nearest → walk_ancestors_with_name
}
```

## 7. Plan d'implémentation

### Phase 1 — Audit cause exacte (S, 30 min)

- Lire `forgia-prefab/src/lib.rs` (Collider strategy)
- Lire `forgia-stage/src/layout.rs::place_modules` (corridor handling)
- Lire `forgia-player` KCC config
- Décision matrix H1 vs H2 vs H3 + écrire conclusion dans story §10

### Phase 2 — Fix root cause (M, 1-2 h selon H)

- Implémentation selon §6.2
- Test pur si modifiable (layout corridor → test deterministic seed)
- `cargo check -p <crate-touchée>` clean

### Phase 3 — Name propagation prop GLB (S, 30 min)

- forgia-prefab : propagation Name au spawn time
- Tests pré-existants `forgia-prefab` regression

### Phase 4 — Sensor enrichment (S, 30 min)

- forgia-player::player_state_sensor : ajouter last_stuck_position + last_stuck_nearby_entity_name
- Pas de gameplay impact, seulement diag amélioré

### Phase 5 — Verification (M, 30 min)

- `cargo check --workspace` clean
- `cargo clippy --workspace --no-deps -- -D warnings` 0 warning
- `cargo test -p forgia-stage -p forgia-level-presets -p forgia-prefab` verts
- Runtime test 3 runs Crypts of Anvil → 0 stuck event

### Phase 6 — Capitalisation (S, 15 min)

- Memory `[[reference-glb-prefab-collider-propagation]]`
- Memory `[[reference-module-placement-safe-corridor]]` si H2 retenu
- Update story-485 §"Risques" : ajout entry "corridor circulation player-boss"

## 8. Risques

| Risque | Mitigation |
|---|---|
| `spawn_gltf_prefab` génère déjà des colliders mais le walk_ancestors hitscan rate le Name | Phase 3 focus exclusif Name propagation, H1 collider devient non-issue |
| Cause réelle = bug KCC penetration sans rapport avec props | Phase 1 audit KCC config exhaustif, fallback story-541 KCC penetration_resolution |
| Module corridor keepout casse story-485 sight-line solver | AC10 garde-fou `min_cover_spacing_m ≥ 3.0` non-régression |
| Name propagation casse autres systèmes (forgia-stage cleanup query) | grep workspace `Query<&Name>` audit ; cleanup utilise StageArenaMarker pas Name |
| Player aussi stuck contre ramparts hex (pas modules) | Phase 1 vérifier hit_entity_idx mapping → si rampart, pivot vers correction wall_thickness |

## 9. Definition of Done

- Tous AC §5 verts
- 3 runs Crypts of Anvil sans stuck event (preuve sensor)
- 1 fix root cause + Name propagation + sensor enrichi
- Commit propre par phase
- Memory capitalisée
- In-game test recap dans la story finale

## 10. Findings phase 1 (audit 2026-05-27)

### Cause retenue : H2 majeure + H1 contributing + KCC config aggravante

### Evidence chiffrée

**H2 confirmée — Le solver place INTENTIONNELLEMENT un module sur l'axe player→boss**

`forgia-stage/src/layout.rs:109-145` :
```rust
// Pre-flag : doit-on forcer le 1er CoverHigh sur le midpoint player↔boss ?
let mut sightline_break_pending = boss_pad.is_some();
...
ModuleKind::CoverWall if sightline_break_pending && instance_idx == 0 => {
    let mp = midpoint_player_boss(player_xz, boss_xz_opt.unwrap());
    ...
}
```

Pour Crypts of Anvil avec player_spawn=(0,0,0) et boss=(0,0,-63), le 1er `cover_high_wall` est placé à **(0, 0, -31.5)** — exactement sur la trajectoire directe player→boss.

Les 17 autres modules sont placés via `sample_dart_throw` (uniform disk) avec `is_position_valid` qui contraint **seulement** :
- cercle inscrit hex (max_radius)
- spacing entre modules
- footprint vs player_spawn

**Aucune contrainte de corridor de circulation player↔boss.** Player marche droit vers le boss et **traverse les modules** sur sa trajectoire.

Position player stuck (3.58, 1.12, -53.54) :
- 22 m au-delà du midpoint forcé (-31.5)
- 9.5 m du boss_pad
- Zone dart-throw uniform où n'importe quel module peut atterrir

**H1 confirmée — `spawn_gltf_prefab` n'ajoute pas de Collider explicite**

`forgia-prefab/src/lib.rs:105-129` :
```rust
let mut e = commands.spawn((
    tag,
    SceneRoot(scene_handle),
    Transform { ... },
));
// AUCUN Collider attaché
```

Le commentaire ligne 100 dit explicitement *"Returns the [`Entity`] so callers can attach extra Components (Colliders, ...)"*. Le caller `forgia-stage::spawn_stage_arena_on_request:993` spawn juste avec tag `(StageArenaMarker,)` — **PAS de Collider**.

Or le hitscan sensor CONFIRME des `blocker` hits sur l'entité. Donc :
- Bevy GLTF loader OU bevy_rapier3d AsyncSceneCollider auto-génère des colliders depuis les meshes GLB des Inferno props (Vase_001, Box_001, RockMid_001, etc.)
- Ces colliders auto-générés sont **probablement trimesh exact**, dont la geometry peut piéger le KCC capsule
- `hit_name: null` confirme que le Name component du SceneRoot parent n'est pas walk-up correctement par le hitscan walk_ancestors (problème distinct mais cohérent)

**KCC config aggravante** — `forgia-player/src/lib.rs:144-156`

```rust
KinematicCharacterController {
    offset: CharacterLength::Absolute(0.01),  // ← 1cm seulement
    slide: true,
    autostep: Some(CharacterAutostep { ... }),
    snap_to_ground: Some(CharacterLength::Absolute(0.5)),
    ...
}
```

`offset: 0.01` est très petit. Pattern Rapier kinematic : offset trop petit + trimesh detail → penetration creep silencieuse → slide fail → velocity=0 sans report `kcc_collisions`.

**Recommandation industry** : offset ≥ 0.05 (5 cm) pour robustesse contre trimesh GLB.

### Fix path retenu

**Phase 2 — Option A + B combinés (recommandé)** :

1. **`layout.rs::place_modules`** : ajouter un keepout `corridor_player_boss(player_xz, boss_xz, width=2.5m)` dans `is_position_valid` pour TOUS les modules sauf le 1er `CoverHigh` (qui doit rester sur le midpoint par design story-485). Aussi : exclure le 1er CoverHigh d'un cercle 1.5m autour du midpoint exact (laisser le player passer à côté).
2. **`forgia-player`** : bumper `offset: 0.01 → 0.05` (safety net contre tous trimesh hostiles, pas juste modules).
3. **`forgia-prefab`** : (phase 3) propager Name aux children spawned du SceneRoot après load asynchrone (fix `hit_name: null` pour future diag).

**Tests purs à ajouter** :
- `place_modules_clear_corridor_player_to_boss` — aucun module dans un rectangle largeur 2.5m entre player et boss (sauf 1er CoverHigh qui est sur l'axe mais shiftable de 1.5m)
- `place_modules_first_coverhigh_shifted_off_axis` — le 1er CoverHigh est ≥ 1.5m d'écart latéral du midpoint exact

**AC10 garde-fou** : `min_cover_spacing_m ≥ 3.0` non-régression (déjà 7.58 actuellement, marge confortable).

### Fix path NON-retenu

- ❌ Retirer le 1er CoverHigh forcé sur midpoint → casse story-485 sight-line invariant
- ❌ Auto-respawn téléport sur stuck → band-aid, masque la cause
- ❌ Désactiver les module placements → casse identity spatial Crypts

## 11. Notes capitalisées de l'audit 2026-05-27

- **Le "plantage" n'était pas un crash** : 498 FPS, 14 457 ticks, RAM 1234 MB stable, watchdog ok. User perçoit le stuck comme freeze.
- **5 stuck events / session** : pattern récurrent, pas one-shot.
- **Bug distinct de story-539** (plugin gating perf) : 2 problèmes différents en parallèle.
- **Forgia-streaming pas le coupable du stuck** : streaming idle (counts.loaded=0) en Roguelite, n'affecte pas KCC.
- **TOML edit walls 4.0m maintenu** : améliore draw-calls /3.4 mais sans rapport avec le stuck.
