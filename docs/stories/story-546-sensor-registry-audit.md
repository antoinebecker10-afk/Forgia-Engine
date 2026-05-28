# Story-546 — Sensor Registry + Audit gate

**Status** : CODE-COMPLETE (2026-05-28)
**Priorité** : 🟡 P1 — outillage diagnostic (débloque tous bugs runtime futurs)
**Scale BMAD** : Standard (≤10 fichiers, additif pur)
**Origine** : 2026-05-28 — discussion user après story-545 (player invincible). User observe que les ~72 sensors JSON sont éparpillés sur 25+ crates, 2 conventions (`forgia_*` / `forgia2_*`), aucun registry central. Diagnostic story-545 = 3 min de Glob + Read au hasard pour trouver le sensor pertinent.

## Symptôme

Aucune source de vérité unique liant :
- Nom de sensor → crate productrice → fichier source
- Sensor → bugs canoniques qu'il diagnostique
- Sensor → schéma (champs JSON attendus)
- Sensor → fréquence d'écriture
- Convention `forgia_*` vs `forgia2_*` (legacy vs unified)

Conséquences :
- Diagnostic 5-10× plus lent (exploration au lieu de lookup)
- Sensors morts non détectés (11 stale > 11j au standup actuel sans alerte)
- Duplicate writers silencieux (audit révèle : `forgia_prefab.json` écrit par 2 crates, `forgia2_stage.json` par 3)
- Features non-conformes à `observability-required.md` non détectées mécaniquement (water 25 gènes 0 sensor, audio biome 0 sensor)

## Cause

Outil registry + audit jamais formalisé. Le xtask `verify-sensors-format` (existant) liste 13 sensors canoniques `forgia2_*` mais ignore 60+ autres et ne croise pas producteurs.

## Fix proposé

### Phase 1 — Registry markdown (~1h)
- `docs/observability/SENSOR_REGISTRY.md` (NEW) : table 1 ligne / sensor
- Colonnes : `filename | tier | producer_crate | producer_file | frequency | schema_fields | canonical_bugs | status`
- Tier : `T0 unified` (forgia2_ via aggregator) / `T1 legacy` (forgia_) / `T2 satellite` (`_health.json` companions)

### Phase 2 — xtask `sensor-audit` (~2h)
- Nouvelle commande `cargo xtask sensor-audit` (`xtask/src/main.rs`)
- Scan : grep `"forgia*.json"` dans `crates/**/*.rs` → set producteurs
- Lit `docs/observability/SENSOR_REGISTRY.md` → set déclarés
- Compare :
  - **Orphans** (écrit, pas dans registry) → fail exit 1
  - **Missing** (dans registry, jamais écrit) → warn (peut être WIP)
  - **Duplicate writers** (≥2 producteurs) → warn (signale concurrence)
- Mode `--strict` : fail si missing aussi

### Phase 3 — Compléter `concept-first.md` §6 (~30min)
- Tableau §6 a colonne "Sensor" déjà — la croiser avec registry
- Ajouter sensors absents (audio biome, water, etc.) ou flag `none`

### Phase 4 — Skill `/sensor <keyword>` (optionnel, hors scope MVP)
- Reporté à story follow-up si registry adoption confirmée

## Critères d'acceptation

- [ ] AC1 — `docs/observability/SENSOR_REGISTRY.md` créé, ≥ 55 sensors listés (audit code grep)
- [ ] AC2 — `cargo xtask sensor-audit` compile + exécute sans erreur baseline (0 orphan tolérable initial)
- [ ] AC3 — `cargo xtask sensor-audit --strict` exit 1 si nouveau sensor non-déclaré ajouté (testable via mock)
- [ ] AC4 — Duplicate writers identifiés et documentés dans registry (status `duplicate-writer`)
- [ ] AC5 — `concept-first.md` §6 sensors mis à jour (au minimum 4 lignes : water, combat, terrain, state machine)
- [ ] AC6 — `cargo check -p xtask` + `cargo clippy -p xtask --no-deps` 0 warning
- [ ] AC7 — Story DONE + checklist post-impl complétée + memory `reference_sensor_registry_pattern.md`

## Test in-game recap

N/A — pure tooling, no runtime effect. Validation :

1. **Action** : `cargo run -p xtask -- sensor-audit` depuis workspace root
2. **Effet attendu** : output JSON-friendly listant `orphans: 0`, `missing: 0` (ou liste explicite), `duplicates: 2` (prefab + stage), exit 0
3. **Sensor** : N/A (le xtask EST le sensor du sensor system)
4. **Variantes si KO** :
   - Beaucoup d'orphans → registry initial incomplet, compléter
   - Faux positifs (path constants dans tests, commentaires) → affiner regex grep
   - Duplicates inattendus → investiguer dans story follow-up

## Cross-refs

- `.claude/rules/observability-required.md` — règle existante, désormais vérifiable
- `.claude/rules/concept-first.md` §6 — colonne Sensor à enrichir
- `xtask/src/main.rs::verify_sensors_format` — couvre seulement 13 canoniques, complémentaire
- Story-545 — bug canonique qui a motivé cette story (3 min de tâtonnement registry-less)
