# story-622 — Réveil du bus QA : pont santé → BugReport + sensor forgia2_qa

**Statut** : ✅ READY — implémenté + testé, **à commiter**.
**Épopée** : Plan RPG + QA intégré ([rpg-qa-integrated-plan-2026-06-24](../plan/rpg-qa-integrated-plan-2026-06-24.md)) — Phase 0.2 (rendre le bus QA utile).
**Niveau BMAD** : Standard (4 fichiers code + story + registry). **Date** : 2026-06-25.

## Problème
Le bus QA (`forgia-qa-core::BugBus`, `ForgiaQaCorePlugin`) était **branché mais muet** :
- **Zéro** site d'émission `BugReport` dans tout le code → `total_ingested` à 0 en permanence.
- Le drain `drain_bug_messages_to_bus` est `#[cfg(feature = "qa-runtime")]` et **aucune crate
  n'activait la feature** → drain compilé en no-op.

Conclusion du finding concept-first (session 2026-06-24) : « 0.2 activer qa-core = NO-OP ». Le rendre
utile = **ajouter des producteurs** + **activer le drain**, pas flipper un toggle.

## Livré
Premier producteur réel du bus + activation du drain + observabilité.

1. **Activation du drain** — [forgia-game/Cargo.toml](../../crates/forgia-game/Cargo.toml) : feature
   `qa = ["forgia-qa-core/qa-runtime"]`, `default = ["qa"]` (ON pré-ship, réversible via
   `--no-default-features` pour une story release). Vérifié résolu dans l'arbre de `forgia` :
   `forgia-qa-core [default,qa-runtime]`.

2. **Producteur — pont santé → bus QA** : nouveau module
   [qa_bridge.rs](../../crates/forgia-observability/src/qa_bridge.rs). Chaque check `RpgHealthState`
   (RGL-1/2 en Roguelite, CHK-1..6 en RPG) qui **passe** en Warn/Critical émet UN `BugReport`,
   source `DetectionSource::HealthMonitor { check_name }` (variant prévu pour ce pont). Discipline :
   - **Edge-trigger** : émission sur le *front montant* seulement (Ok→Warn/Critical, Warn→Critical).
     Pas de réémission par frame.
   - **Catégorie à champs stables** : `BugCategory::LogicInvariantViolation { tr_id: check_name,
     detail: next_step }` — `next_step` est statique → `BugSignature` (hash du `Debug` catégorie)
     stable → une alerte récurrente dédupe en `occurrences++` sur la fenêtre 5 min au lieu de flooder.
   - **Direction de dépendance saine** : observability → qa-core (types seulement, 0 cycle).

3. **Observabilité** — sensor [forgia2_qa.json](../../crates/forgia-observability/src/qa_bridge.rs)
   (T0, 1Hz, cross-mode) : `emitted_total`, `bus_ingested`, `dedup_hits` + sévérité/next_step de la
   dernière émission. Enregistré dans [SENSOR_REGISTRY.md](../observability/SENSOR_REGISTRY.md).
   Toggle + fenêtre config-driven (`QaBridgeConfig` dans
   [config.rs](../../crates/forgia-observability/src/config.rs), hot-reload Shift+F12).

## Vérification (preuve)
- `cargo check -p forgia-observability` → exit 0.
- `cargo check -p forgia-game` (feature `qa` ON) → exit 0 (drain compile).
- `cargo check -p forgia` + `cargo tree` → `forgia-qa-core [default,qa-runtime]` résolu dans le binaire.
- `cargo test -p forgia-observability qa_bridge` → **7/7 passent** (fronts montants, escalade, steady,
  descente, ok-never-emits, mapping sévérité, repr courte JSON-safe).
- `cargo clippy -p forgia-observability` → **0 warning sur mon code** (warning pré-existant hors scope
  `forgia-core/src/lib.rs:58` doc_lazy_continuation, non touché).

## Acceptance criteria
- [x] Le drain `qa-runtime` est activé dans le binaire `forgia` (feature résolue, vérifiée `cargo tree`).
- [x] Un check santé qui passe Warn/Critical émet un `BugReport` (edge-trigger).
- [x] Pas de flood : signature stable (detail=next_step statique) + edge-trigger.
- [x] `DetectionSource::HealthMonitor { check_name }` utilisé (pont legacy→bus typé).
- [x] Sensor `forgia2_qa.json` expose l'activité du bus (emitted/ingested/dedup) + enregistré.
- [x] Toggle + fenêtre config-driven (`QaBridgeConfig`), pas de hardcode.
- [x] Direction de dép saine (observability→qa-core, 0 cycle) ; 7 tests verts ; crate clippy-clean.

## Test runtime
1. **Action** : rebuild `cargo build -p forgia -j 4`, lancer, entrer en Roguelite, provoquer un check
   critical (ex : rester en run avec 0 bot > 8s → RGL-2 warn).
2. **Effet** : `forgia2_qa.json` montre `emitted_total ≥ 1`, `bus_ingested ≥ 1`, `severity: warn/critical`
   + `next_step` de la dernière alerte.
3. **Où** : `forgia2_qa.json` à la racine workspace.
4. **Variantes si KO** :
   - `emitted_total > 0` mais `bus_ingested = 0` → feature `qa-runtime` non active (drain no-op) — vérifier `cargo tree`.
   - `emitted_total = 0` alors qu'un check est critical → le check n'a jamais transité (déjà critical au boot ?) ou `qa_bridge.enabled=false`.

## Post-impl auto-QA (verifier + qa-lead)
Passe BMAD Standard. qa-lead a trouvé 4 défauts (0 bloquant) :
- **#001 Majeur — CORRIGÉ** : le sensor émettait `severity: "major"` (échelle QA) hors schéma T0
  (`ok|warn|critical|info`). Fix : `obs_short(cur)` → `warn`/`critical` pour le champ sensor (le
  BugReport garde l'échelle QA). Test anti-régression `sensor_severity_is_t0_compliant`.
- **#003 Mineur — CORRIGÉ** : `bridge.seen` gardait des fronts fantômes après changement de mode
  (RGL-* persistaient → faux négatif à la ré-entrée). Fix : `prune_ghost_seen` (retain). Test
  `ghost_entries_pruned_when_check_disappears`.
- **#002 Mineur (process) — DIFFÉRÉ** : `forgia2_qa.json` pas dans `CANONICAL_SENSORS` (xtask) → pas
  encore couvert par `verify-sensors-format`. Renvoyé à l'incrément « étendre verify-sensors-format »
  (xtask hors scope/churn de cette story ; le sensor émet désormais des valeurs T0-valides, donc
  l'ajout futur sera vert).
- **#004 Mineur — ACCEPTÉ/DOCUMENTÉ** : `MessageWriter<BugReport>` exige `Messages<BugReport>`
  (ForgiaQaCorePlugin). 0 impact actuel (forgia-game ajoute toujours les 2 plugins). Le « fix »
  (double `add_message`) risque de dédoubler le system de clear → régression pire que le bug ; coupling
  documenté dans lib.rs + qa_bridge.rs à la place.

## Suite (incréments Phase 0.2)
- 🟡 **2e producteur — panic hook** : bridger `forgia2_crash.json` (story-592, déjà existant, racine
  Cargo.toml:207) → `BugReport::Panic`. Capte les crashes (classe de bug la plus précieuse).
- 🟢 **3e producteur — anomalie télémétrie** : seuils sur `forgia2_perf`/`memory` → `BugReport`
  (`TelemetryAnomaly`/`FrameTimeSpike`).
- 🟢 **Sink fichier** : activer `qa-record` (FileSink RON) pour persister les BugReport sur disque.
