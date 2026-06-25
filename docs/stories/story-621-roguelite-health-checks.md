# story-621 — forgia2_health.json actif en Roguelite (RGL-1/RGL-2)

**Statut** : ✅ READY — implémenté + testé, **à commiter**.
**Épopée** : Plan RPG + QA intégré ([rpg-qa-integrated-plan-2026-06-24](../plan/rpg-qa-integrated-plan-2026-06-24.md)) — Phase 0.6 A.
**Niveau BMAD** : Standard (3 fichiers + tests). **Date** : 2026-06-24.

## Problème
`forgia2_health.json` (capteur santé agrégé) était **aveugle en Roguelite** : ses 6 checks (`CHK-*`,
`checks.rs`) sont gatés `GameMode::Rpg` et surveillent terrain/biome/LOD — concepts inexistants en
Roguelite. Donc en Roguelite il écrivait toujours `severity: ok, checks_count: 0`. Le capteur santé
était **muet sur le jeu qu'on ship**.

## Livré
Nouveau module [roguelite_health.rs](../../crates/forgia-observability/src/roguelite_health.rs) +
[config](../../crates/forgia-observability/src/config.rs) `RogueliteHealthConfig` (seuils config-driven,
hot-reloadable via rpg_monitor.toml) + système gaté `GameMode::Roguelite` dans
[lib.rs](../../crates/forgia-observability/src/lib.rs) :
- **RGL-1 écran vide** : `Mesh3d` présents mais 0 visible (au-dessus de `blank_mesh_floor`), OU aucune
  `Camera3d` active → **critical**. Mêmes seuils que `render_sensor`. **Aurait attrapé le bug « map
  invisible » du 2026-06-24.**
- **RGL-2 vague figée** : run actif (`in_run`/`boss`), hors break, 0 bot vivant depuis > `stuck_wave_secs`
  → **warn**. Lit `forgia2_roguelite_state.json` avec **garde de staleness** (`timestamp_secs` vs
  `elapsed`, même horloge) pour éviter les faux positifs sur capteur figé.
- Les deux peuplent `RpgHealthState` (lu cross-mode par `health_sensor`). **0 conflit** : les checks RPG
  sont gatés `Rpg`, les miens `Roguelite` (mutuellement exclusifs).

**Contrainte d'archi respectée** : `forgia-observability` **ne dépend PAS** de `forgia-mode-roguelite`
(mauvaise direction + zone de churn) — RGL-2 relit le sensor JSON déjà produit.

## Vérification (preuve)
- `cargo check -p forgia-observability` → exit 0.
- `cargo test -p forgia-observability roguelite_health` → **10/10 passent** (blank screen, no-camera,
  sous-plancher, sain, vague figée, break, lobby, bots vivants, sous-seuil, boss).
- Clippy : **0 warning sur mon code** (`roguelite_health`/`config`). NB : `cargo clippy --workspace` est
  bloqué par un warning **pré-existant hors scope** `forgia-core/src/lib.rs:58` (doc_lazy_continuation,
  dérive toolchain clippy 1.94→1.96 — migration trackée séparément, non touché).

## Acceptance criteria
- [x] RGL-1 critical sur écran vide (meshes présents, 0 visible) et sur 0 Camera3d active.
- [x] RGL-1 ok sous le plancher (menu/transition) et en rendu sain.
- [x] RGL-2 warn sur vague figée ; ok en break/lobby/victoire/bots vivants/sous-seuil.
- [x] Seuils config-driven (`RogueliteHealthConfig`), pas de hardcode gameplay.
- [x] Gaté `GameMode::Roguelite`, 0 conflit avec les checks RPG.
- [x] 10 tests unitaires verts ; crate clippy-clean.

## Test runtime
1. **Action** : rebuild `cargo build -p forgia -j 4`, lancer, entrer en Roguelite, démarrer une run.
2. **Effet** : `forgia2_health.json` montre `overall_severity_source: "rpg_health"` + `checks_count: 2`
   (RGL-1/RGL-2). Si écran vide → `severity: critical` + next_step RGL-1.
3. **Où** : `forgia2_health.json` à la racine workspace.

## Différé (reste 0.6)
- 🟢 RGL-3 (état figé) + RGL-4 (HP stale) — étendre quand utile.
- 🟡 Étendre `verify-sensors-format` (13 → couverture large) — story-546 phase 2.
