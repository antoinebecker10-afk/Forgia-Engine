# Story-549 — Physics sensor (Rapier blind spot)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_physics.json`, fichier `bindings.rs`, symbole `sys_write_physics_sensor`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

**Status** : CODE-COMPLETE (2026-05-28)
**Priorité** : 🟡 P1 — comble blind spot diagnostic Rapier
**Scale BMAD** : Standard
**Origine** : 2026-05-28, suite story-548. Session "il faut que tu t'améliores" — combler les angles morts sensors qui restent. Rapier était le plus impactant (bugs class story-540 player stuck KCC, story-545 raycast self-hit).

## Symptôme

Avant cette story : **0 sensor** exposait l'état Rapier 0.33. Bugs concernés :
- `story-540` — player stuck KCC : impossible de voir count KCC, gravity active, fixed bodies environnants
- `story-545` — raycast self-hit : impossible de voir count colliders sensor (which sont déjà excludés)
- Bug class "collision désynchronisée" — impossible de voir si RigidBody/Collider markers cohérents

## Fix livré

### Producteur — `crates/forgia-observability/src/physics_sensor.rs` (NEW, 113 LOC)

Système `sys_write_physics_sensor` (1Hz, cross-mode) lit Rapier via queries Bevy :
- `Query<&RigidBody>` → count par variant (Dynamic / KinematicPos / KinematicVel / Fixed)
- `Query<&Collider>` → total
- `Query<With<Collider>, With<Sensor>>` → colliders en mode "sensor" (no collision)
- `Query<With<KinematicCharacterController>>` → KCC count
- `Query<With<ImpulseJoint | MultibodyJoint>>` → joints
- `Query<&RapierConfiguration>` → gravity (Component, pas Resource en 0.33)

Output `forgia2_physics.json` schéma normalisé `{id, severity, next_step, timestamp_secs, ...}`.

### Consommateur — `crates/forgia-debug/src/categories/physics.rs` (NEW, 42 LOC)

Catégorie 7 de l'overlay forgia-debug. Touche `Digit7` toggle. Affiche tous les champs gravity/RB/colliders/KCC/joints.

### Plumbing (4 fichiers modifiés)

- `forgia-observability/src/lib.rs` — `pub mod physics_sensor` + `init_resource` + `add_systems`. Tuple Bevy limit forcé un add_systems séparé.
- `forgia-debug/src/categories/mod.rs` — `CategoryId::Physics` + `ALL` + `name` + `numpad_digit(7)` + registry insert
- `forgia-debug/src/snapshot.rs` — `PhysicsSlice` + `read_physics()` fn
- `forgia-debug/src/bindings.rs` — `Digit7 → ToggleCategory(Physics)`
- `docs/observability/SENSOR_REGISTRY.md` — entry T0 row

## Critères d'acceptation

- [x] AC1 — `forgia2_physics.json` créé 1Hz cross-mode (pas gate par GameMode)
- [x] AC2 — Schéma normalisé `{id, severity, next_step, timestamp_secs, ...}` conforme `verify-sensors-format`
- [x] AC3 — Severity `warn` si Rapier world vide (RB+colliders=0) — détecte plugin physique non initialisé
- [x] AC4 — Sensor lu par forgia-debug `PhysicsSlice` + Catégorie Physics (Digit7) — overlay couvre 7/8 catégories
- [x] AC5 — `cargo check -p forgia-observability -p forgia-debug` clean
- [x] AC6 — `cargo clippy --no-deps` 0 warning
- [x] AC7 — `cargo xtask sensor-audit` PASS (0 orphans, registry 65/65)
- [x] AC8 — Story-547 forgia-debug couvre maintenant **7 catégories** (System+Combat+Player+Terrain+Anim+Audio+Physics)

## Test in-game recap (post-wiring forgia-game)

1. **Action** : `cargo run -p forgia-game --profile release-fast` → menu → Roguelite, presser F3 puis 7 (Digit7)
2. **Redémarrage requis** — modif .rs
3. **Effet attendu** : panel Physics dans overlay affiche `gravity.y: -9.81, rigid_bodies: ~50-200, KCC: 1, joints: 0`
4. **Sensor** : `cat forgia2_physics.json` → `severity:"ok"` ou `"warn":"Physics world empty"` selon état
5. **Variantes si KO** :
   - `rigid_bodies_total: 0` partout → RapierPhysicsPlugin non ajouté, audit forgia-game build
   - `gravity_y: null` → query `&RapierConfiguration` retourne vide, entity world Rapier pas spawnée
   - `kcc_count: 0` en RPG → player ne porte pas KinematicCharacterController, vérifier forgia-rpg::character

## Hors scope

- Contact pairs live (nécessite `ReadRapierContext::narrow_phase()` — API complexe, story follow-up)
- Recent collisions ring buffer — story-551 invariants ring fera ça mieux
- VRAM physique (Rapier internal allocations) — story-552 potentielle

## Cross-refs

- Story-547 — forgia-debug 3-layer architecture
- Story-540 — player stuck KCC (bug class ciblé)
- Story-545 — raycast self-hit (bug class ciblé)
- `feedback_mvp_underestimates_coverage_default.md` — scope ≥70% d'emblée
