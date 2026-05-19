# Vague 3 — Audit Bevy 0.18 idioms

> **Date** : 2026-05-19
> **Cible** : `C:/Users/Antoi/Desktop/Forgia Rewrite/`
> **Méthode** : agent `bevy-specialist` (audit cartographique READ-ONLY 4 axes parallèles)
> **Verdict global** : ROI moyen, 3 stories de migration suggérées. **Correction audit forensic** : `FreeCamera`/`PanCamera` first-party gameplay n'existent **PAS** en Bevy 0.18.1.

---

## 0. Résumé exécutif

L'audit forensic 2026-05-19 (`audit-2026-05-19.md`) recommandait 4 chantiers Bevy 0.18 idioms. L'audit ciblé Vague 3 confirme 3 axes pertinents et **invalide** un axe basé sur fausse hypothèse :

| Axe | ROI | Verdict |
|---|---|---|
| **1. Required Components** (`#[require]`) | 🟢 Moyen | 3 candidats viables (Player, TargetCube, NameplateRoot) — story-461 Standard ~2h |
| **2. Observers** (`Trigger<E>`) | 🟢 Moyen-haut sur UI events | 2 candidats UI (nameplate, killfeed) — story-462 Standard ~2h. SKIP `apply_damage` (complexité > ROI) |
| **3. ECS Relationships natives** (`ChildOf`) | 🟡 Faible-moyen | 1 candidat ROI clair (wave bots) — story-463 Quick ~1h |
| **4. FreeCamera/PanCamera vs RpgOrbitCamera** | ❌ N/A | **`FreeCamera`/`PanCamera` n'existent PAS first-party gameplay en Bevy 0.18.1**. `RpgOrbitCamera` custom (330 LOC, WoW dual-mode + auto-recenter ease-out) reste la bonne solution |

**Découverte importante** : l'axe 4 de l'audit forensic était basé sur une hypothèse incorrecte sur l'API Bevy 0.18. Les caméras `FreeCamera`/`PanCamera` mentionnées sont uniquement dans `bevy_dev_tools` (debug fly cam), pas gameplay. À corriger dans `audit-2026-05-19.md` § 6.4.

**Constat global** : 0 usage de `#[require]` et 0 usage d'Observers dans les 258 crates. Le codebase est en pattern Bevy 0.13-0.15 idiomatique (Bundles + EventReader → MessageReader). Migration progressive recommandée par stories isolées.

---

## 1. Axe 1 — Required Components candidates

### 1.1 Constat

Grep `#[require(` sur le workspace = **0 résultat**. Pourtant plusieurs spawn-sites insèrent systématiquement les mêmes tuples (Transform + Visibility + Name + composant principal).

### 1.2 Tableau candidats

| Candidat | File:line | Pattern actuel | Migration cible | Risque | Effort |
|---|---|---|---|---|---|
| `Player` | `forgia-player/src/lib.rs:114` | 9 components co-spawn | `#[require(Transform, Visibility)]` ; laisser `RigidBody`/`Collider`/`ActionState` explicites | 🟠 M | 1h |
| `TargetCube` (bot) | `forgia-mode-fps-arena/src/wave.rs:341` | `ArenaMarker + Transform + Visibility + Health + ArenaBot + BotShootConfig + RigidBody` | `#[require(Transform, Visibility)]` sur `TargetCube` | 🟢 L | 0.5h |
| `NameplateRoot` | `forgia-enemy-nameplate/src/lib.rs:117` | Toujours avec `Transform + Visibility + Name` | `#[require(Transform, Visibility)]` | 🟢 L | 0.5h |
| `WeaponViewmodel` | `forgia-fps/src/lib.rs` | `Transform + Visibility + SceneRoot` | **NON viable** — `SceneRoot::default()` = handle nul invalide | 🔴 H | SKIP |
| `RexCharacter` | `forgia-rpg/src/character.rs:195` | Enfant avec `Transform + Visibility + SceneRoot` | Idem — SceneRoot non-Default | 🔴 H | SKIP |

### 1.3 Syntaxe exacte Bevy 0.18

```rust
#[derive(Component)]
#[require(Transform, Visibility)]
pub struct Player { /* ... */ }
```

Les types `require` doivent implémenter `Default` ou être fournis explicitement : `#[require(Visibility = Visibility::Hidden)]`.

### 1.4 Piège connu

`#[require(T)]` force l'insertion de `T` **même via `commands.insert(A)` après coup** — pas uniquement lors de `spawn(A)`. Si un system insère `Player` sur entité existante, `Transform::default()` écrase la position. À auditer call-sites `.insert(Player ...)`.

### 1.5 Story-461 suggérée

**Scope BMAD Standard (~2h)** :
- 3 components migrés (Player + TargetCube + NameplateRoot)
- Suppression lignes `Transform::default(), Visibility::default()` dans les 3 spawn-sites
- `cargo check -p forgia-player -p forgia-mode-fps-arena -p forgia-enemy-nameplate` 0 erreur
- Vérifier qu'aucun system query ne filtre `Without<Transform>` ou `Without<Visibility>` (contradiction silencieuse)

---

## 2. Axe 2 — Observers candidates

### 2.1 Constat

Le workspace utilise `MessageReader<T>` / `MessageWriter<T>` (Bevy 0.18 a renommé `Event` → `Message`). Polling Update partout. **0 Observer**.

### 2.2 Tableau candidats

| Candidat | File:line | Pattern actuel | Migration cible | Risque | Effort |
|---|---|---|---|---|---|
| `spawn_or_refresh_on_hit` (nameplate) | `forgia-enemy-nameplate/src/lib.rs:83` | `MessageReader<CombatHitEvent>` Update polling | `app.observe(\|t: Trigger<CombatHitEvent>\| ...)` | 🟢 L | 1h |
| `ingest_kill_events` (killfeed) | `forgia-killfeed/src/lib.rs:111` | `MessageReader<CombatHitEvent>` filtré is_kill | Observer `CombatHitEvent` | 🟢 L | 0.5h |
| Ammo HUD render | `forgia-ui-hud-ammo/src/lib.rs` | Polling Update egui | `Changed<EquippedWeapons>` query filter (pas Observer) | 🟠 M | 0.5h |
| `apply_damage` core | `forgia-damage/src/lib.rs:217` | `MessageReader<DamageEvent>` Update | Observer **NON recommandé** — signature `Query<&mut Health>` via `trigger.world_mut()` ou `Commands` deferred complexifie | 🔴 H | SKIP |
| `wave_orchestrator` | `forgia-mode-fps-arena/src/wave.rs:179` | Timer polling + count bots vivants | **Pas viable** — besoin du timer dt + query continue | N/A | SKIP |
| `tick_lifetime_and_despawn` (nameplate) | `forgia-enemy-nameplate/src/lib.rs:200` | Timer Update fade alpha + despawn | **Pas viable** — besoin du timer dt | N/A | SKIP |

### 2.3 Nuance Observers Bevy 0.18

```rust
app.observe(|trigger: Trigger<CombatHitEvent>, mut commands: Commands| {
    let event = trigger.event();
    // commands deferred, pas de Query<&mut T> directe dans la signature
});
```

Hooks lifecycle disponibles : `OnAdd<C>`, `OnRemove<C>`, `OnInsert<C>` (utile pour patterns "spawn nameplate on enemy add").

### 2.4 Story-462 suggérée

**Scope BMAD Standard (~2h)** :
- Migration nameplate + killfeed vers Observer `CombatHitEvent`
- Garder `apply_damage` en MessageReader Update (clarté > économie polling)
- Critère : sensors `forgia_killfeed.json` + nameplate billboard restent fonctionnels, pas de regression hitbox
- Bonus : ajouter `Changed<EquippedWeapons>` filter sur ammo HUD render

---

## 3. Axe 3 — ECS Relationships natives

### 3.1 Constat — mix incohérent

Le workspace utilise **3 styles coexistants** :

1. **`children![(...)]` macro** (Bevy 0.18 idiomatique) — `forgia-player`, `forgia-mode-fps-arena` floor/walls ✅
2. **`.with_children(|p| { p.spawn(...) })` builder** (legacy 0.13-0.15) — `wave.rs` bots, `character.rs` Rex, `terrain/lod.rs` ⚠️
3. **`ChildOf(entity)` direct dans tuple spawn** (Bevy 0.16+ first-class) — `forgia-enemy-nameplate` ✅

### 3.2 Tableau call-sites

| Call-site | File:line | Pattern | Migration | Risque | Effort |
|---|---|---|---|---|---|
| FpsCamera child Player | `forgia-player/src/lib.rs:142` | `children![...]` macro | Déjà idiomatique | - | 0 |
| CharacterMesh enfant bot | `forgia-mode-fps-arena/src/wave.rs:399` | `.with_children` | `ChildOf(parent_id)` tuple | 🟢 L | 0.5h |
| HeadProxy enfant bot | `forgia-mode-fps-arena/src/wave.rs:419` | `.with_children` | `ChildOf(parent_id)` tuple | 🟢 L | 0.5h |
| Rex GLB enfant Player | `forgia-rpg/src/character.rs:195` | `.with_children` | Garder — race condition documentée memory `reference_bevy_on_enter_cross_plugin_race.md` | 🔴 M | SKIP |
| Humanoid blocks 6 enfants | `forgia-rpg/src/character.rs:653` | `.with_children` 6 primitives | 6 `ChildOf` séparés — plus verbeux, faible ROI | 🟡 L | SKIP |
| NameplateRoot enfant bot | `forgia-enemy-nameplate/src/lib.rs:126` | `ChildOf(ev.target)` direct + `.with_children` pour bg/fill | Déjà partiellement migré | - | 0 |
| LOD tiles ShadowView | `forgia-terrain/src/lod.rs:533,569` | `.with_children` mesh child | Migrable mais terrain hot path testing requis | 🟠 M | SKIP |

### 3.3 Note `despawn()` récursif

`forgia-rpg/src/character.rs:729` documente correctement que `despawn()` est récursif par défaut en Bevy 0.18 (memory `reference_bevy_018_despawn_recursive_default.md`). Aucune action.

### 3.4 Story-463 suggérée

**Scope BMAD Quick (~1h)** :
- `wave.rs` CharacterMesh + HeadProxy via `ChildOf(parent_id)` dans tuple
- Vérifier `AsyncSceneCollider` toujours sur enfant
- Critère : wave 1 bot spawn + hit fonctionnel, sensor `forgia_arena_waves.json` inchangé

---

## 4. Axe 4 — FreeCamera/PanCamera ❌ N/A

### 4.1 Découverte invalidante

**`FreeCamera`/`PanCamera` first-party gameplay n'existent PAS en Bevy 0.18.1 stable.** Grep sur les sources Bevy confirme :
- Seul `bevy_dev_tools::picking_debug` a un `FlyCam` utilitaire (debug uniquement)
- Pas de `FreeCamera`/`PanCamera` dans `bevy_core` ni `bevy_camera` (qui contient `Camera3d`, `Camera2d`, `Projection`)
- L'hypothèse de l'audit forensic original (`audit-2026-05-19.md` § 6.4) était basée sur des notes incorrectes ou une feature Bevy 0.19+ non encore livrée

### 4.2 Analyse `RpgOrbitCamera`

`forgia-camera-orbit/src/lib.rs` = **330 LOC, 4 systèmes** :
- `orbit_input` — pitch (mouse Y) + yaw_offset (mouse X LMB) + zoom (wheel)
- `orbit_auto_recenter_on_move` — WoW pattern, ease-out quad vers yaw=0 si player bouge, duration 1.2s
- `orbit_follow` — calcul position derrière target avec distance + pitch
- `orbit_cursor_grab` — `CursorOptions` Bevy 0.16+ (correct)

**Fonctionnalités custom non couvertes par Bevy first-party** :
1. WoW LMB/RMB dual-mode (look-orbit vs mouselook-steer)
2. Auto-recenter ease-out quad configurable
3. Pitch/yaw séparés avec yaw_offset indépendant du player yaw
4. Clamp pitch asymétrique

### 4.3 Verdict

**`RpgOrbitCamera` reste custom — recommandation invalidée.** Code stable (330 LOC isolés, 2 tests headless passent, `.in_set(GameSet::Camera)`). À documenter comme "Forgia-specialty crate" dans ARCHITECTURE.md.

---

## 5. Plan d'action consolidé

### 5.1 Ordre de priorité recommandé

| Ordre | Story | Scope | Effort | Bénéfice |
|---|---|---|---|---|
| 1 | **story-463** `.with_children` → `ChildOf` wave bots | Quick | ~1h | Zone active (spawn fréquent), bénéfice mesurable |
| 2 | **story-461** Required Components Player + TargetCube + NameplateRoot | Standard | ~2h | Qualité codebase, suppression bundles redondants |
| 3 | **story-462** Observers nameplate + killfeed | Standard | ~2h | Gain perf UI (poll Update → trigger on-event), Bevy 0.18 idiomatic |

**Total Vague 3 implémentation** : ~5h en 3 stories isolées.

### 5.2 Stories à SKIP (avec justification)

| Item | Raison SKIP |
|---|---|
| `WeaponViewmodel` Required | `SceneRoot::default()` = handle nul invalide |
| `RexCharacter` Required | Idem `SceneRoot` non-Default safe |
| `apply_damage` → Observer | Signature complexe (Query mut via Commands deferred), clarté > économie |
| `wave_orchestrator` → Observer | Pattern timer + query continue inadapté |
| Rex `.with_children` → `ChildOf` | Race condition documentée — refacto risqué |
| Humanoid blocks `.with_children` → `ChildOf` | 6 enfants fixes, ROI faible |
| LOD tiles `.with_children` → `ChildOf` | Terrain hot path, testing lourd requis |
| `RpgOrbitCamera` → `FreeCamera` first-party | **`FreeCamera` gameplay n'existe pas en Bevy 0.18.1** |

### 5.3 Correction `audit-2026-05-19.md`

§ 6.4 "Bevy 0.18 idioms à adopter" mentionnait : *"`FreeCamera`/`PanCamera` first-party (peut remplacer `RpgOrbitCamera` custom)"*. **Cette recommandation est invalidée**. À corriger : *"`RpgOrbitCamera` custom reste optimal — pas d'équivalent first-party gameplay en Bevy 0.18.1"*.

---

## 6. Pièges techniques documentés

1. **`#[require(T)]` + `.insert()` post-spawn** = `T::default()` écrase la valeur existante. Auditer call-sites `.insert(Player ...)`.
2. **`ChildOf(e)` dans tuple + `.with_children()` même parent** = OK séparé mais double-parent silencieux si appliqué au même child.
3. **`Trigger<E>` Observer** = pas de `Query<&mut T>` directe dans signature. Passer par `Commands` deferred ou `trigger.world_mut()`.
4. **Hooks `OnAdd<C>`/`OnRemove<C>`/`OnInsert<C>`** disponibles Bevy 0.18 — utile pour spawn nameplate à l'ajout d'un Enemy component.
5. **`Message` = nouveau nom Bevy 0.18 de `Event`** : `#[derive(Message)]`, `MessageReader`, `MessageWriter`, `add_message()`.

---

## 7. Recommandation Vague 3 — Decision

**Cette session** : audit livré uniquement (ce document). **Pas de migration code** — chaque story (461/462/463) doit avoir sa propre passe BMAD Standard avec tests headless validés avant et après.

**Justification** : les 3 migrations touchent des paths actifs (player spawn, bot wave, combat events). Sans tests régression existants sur ces flows, un batch unique = risque cascade. Migration en 3 stories isolées avec smoke test entre chaque = mode prudent qui préserve la stabilité acquise en Vagues 1+4.

**Prochaine action proposée** : story-463 d'abord (Quick, faible risque, bénéfice immédiat zone active). Confirmer fonctionnel runtime, puis story-461. story-462 (Observers) en dernier car nuance API + risque oubli call-site polling.

---

*Audit produit par bevy-specialist agent (sources : Bevy 0.18.1 official + bevy-cheatbook). Read-only strict, 0 modification code source.*
