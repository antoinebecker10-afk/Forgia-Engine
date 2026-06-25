# story-620 — Hygiène sensors : registre complet + gates verts en CI (Phase 0.6)

**Statut** : ✅ READY — implémenté + vérifié, **à commiter**.
**Épopée** : Plan RPG + QA intégré ([rpg-qa-integrated-plan-2026-06-24](../plan/rpg-qa-integrated-plan-2026-06-24.md)) — Phase 0.6.
**Niveau BMAD** : Standard (docs + xtask + CI, 0 code runtime). **Date** : 2026-06-24.

## Problème
`cargo xtask sensor-audit` était **rouge** (exit 1) → le pre-push échouait dessus, et le gate ne pouvait
pas être porté en CI. Cause réelle (révélée par concept-first, ≠ supposition du plan) : **6 sensors
orphelins** (produits dans le code mais absents de `SENSOR_REGISTRY.md`). Le plan supposait aussi un
« dédup writers `forgia2_stage`×3 / `forgia_prefab`×2 » — mais ces duplications sont **déjà marquées
`duplicate-writer`** dans le registre et **acceptées par le gate** : rien à dédupliquer.

## Livré
- **6 orphelins enregistrés** dans [SENSOR_REGISTRY.md](../observability/SENSOR_REGISTRY.md) (fréquence
  vérifiée par lecture du producteur + bug canonique) :
  - `forgia2_aimassist.json` (forgia-fps, 1Hz, story-615)
  - `forgia2_color_grading.json` (forgia-game, 1Hz)
  - `forgia2_ftue.json` (forgia-mode-roguelite, 1Hz, story-597)
  - `forgia2_load_timing.json` (forgia-mode-roguelite, event)
  - `forgia2_merchant.json` (forgia-mode-roguelite, 1Hz, story-591)
  - `forgia2_render.json` (forgia-observability, 1Hz — le capteur écran-vide)
- **`sensor-audit` repassé vert** : 94 déclarés = 94 produits, 0 orphelin, 0 manquant.
- **Gates portés du pre-push vers la CI** (job `ratchets` de [ci.yml](../../.github/workflows/ci.yml)) une
  fois confirmés verts : `arch-drift`, `no-scaffold`, `sensor-audit` (+ `validate-genomes` déjà là en 0.5).
  Ferme la faille « 4/5 gates bypassables via `--no-verify` ».

## Vérification (preuve)
- `cargo xtask sensor-audit` → **exit 0** (avant : exit 1 / 6 orphelins).
- Gates verts confirmés un par un : `arch-drift`, `no-scaffold`, `sensor-audit`, `validate-genomes` → exit 0.

## Acceptance criteria
- [x] Les 6 orphelins sont déclarés dans SENSOR_REGISTRY.md (filename + tier + producer file:line + freq + bug).
- [x] `sensor-audit` exit 0.
- [x] `arch-drift` / `no-scaffold` / `sensor-audit` ajoutés au job CI `ratchets`.
- [x] 0 régression : les gates verts le restent ; les duplicate-writers connus restent acceptés.

## Différé (reste de la Phase 0.6, hors scope de cette story)
- 🟡 **`forgia2_health.json` actif en Roguelite** : les 6 checks de `checks.rs` sont RPG-gated → **aveugles
  sur le jeu qu'on ship**. Story dédiée (touche `forgia-observability/checks.rs`, demande de comprendre le
  gating RPG). **Haute valeur** — c'est le capteur santé du roguelite.
- 🟡 Étendre `verify-sensors-format` (13 → couverture large des ~94 T0) — story-546 phase 2.
- 🟢 `story-ids` rouge (doublons 597/598/617) + `asset-load` rouge (drift local) : **non câblés en CI** tant
  que l'arbre n'est pas propre (travail en cours). À porter quand vert.
