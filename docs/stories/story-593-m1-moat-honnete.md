# Story-593 — M1 « Moat honnête » (docs vraies, sensors véridiques, gates actifs)

> **Source** : [audit complet 2026-06-10](../audit/audit-2026-06-10-full-codebase.md) thème A
> (« le déclaré ment au câblé ») + [roadmap post-audit](../ROADMAP_POST_AUDIT_2026-06-10.md) jalon M1.
> **Scale BMAD** : Standard. **Date** : 2026-06-10. **Statut** : DONE (2 reports documentés).

## Contexte

Défaut systémique n°1 de l'audit : l'écart déclaré/câblé (docs fausses ×4, sensors
menteurs, gates décoratifs). Pour un moteur dont le moat est « une IA lit et agit
juste », chaque mensonge du codebase attaque le produit. M1 = rétablir la vérité et
la garder mécaniquement.

## Critères d'acceptance

| # | AC | Statut | Preuve |
|---|---|---|---|
| AC1 | ARCHITECTURE.md liste exactement les 62 crates réelles (rôle, LOC, wired) | ✅ | réécrit + `cargo xtask arch-drift` → OK 62/62 |
| AC2 | Gate `arch-drift` bloque toute dérive future doc↔Cargo.toml | ✅ | xtask, testé (a même attrapé ses faux positifs §6) |
| AC3 | README/CONTRIBUTING : 100 % des commandes documentées fonctionnent | ✅ | commandes testées avant écriture (`cargo run`, clippy, test -p) |
| AC4 | fine-grained-crates.md ne contredit plus la doctrine post-cleanup | ✅ | réécrite (crate à la demande, ratchet no-scaffold, réf ADR-0002) |
| AC5 | ADR-0002 (cleanup 266→62) + ADR-0003 (pivot vision) écrits | ✅ | docs/adr/ |
| AC6 | Décision QA documentée | ✅ ADR-0004 **PROPOSED** | recommandation : option A minimale + descoper replay — décision Antoine |
| AC7 | sensor-audit vert : 0 orphelin, 0 missing (même --strict) | ✅ | scanner étendu (consts SENSOR_PATH, 45→82 produits détectés) + registre +16/-4 |
| AC8 | asset-load vert | ✅ | rebaseline 84 (3 fichiers légitimes : menu_video, village hex, worldgen kit). Cible 30 inchangée |
| AC9 | Gate `story-ids` : nouveaux doublons d'ID bloqués | ✅ | 9 collisions historiques grandfathered ; « prochain ID libre » affiché |
| AC10 | Hook pre-push exécute les 5 gates | ✅ | scripts/git-hooks/pre-push + installé .git/hooks/ |
| AC11 | forgia2_toon.json dit la vérité (outline_attached réel + severity info) | ✅ | const OUTLINE_ATTACHED + 5 tests severity ; 110 tests crate verts |
| AC12 | Catalogue postprocess honnête (2 réels / 43 stubs) | ✅ | doc header forgia-postprocess |
| AC13 | run_debug.ps1 lance forgia.exe ; artefact stale supprimé | ✅ | + check existence avec hint build |

## Reports documentés (pas d'entre-deux silencieux)

1. **Dépose du pipeline village mort + sensor forgia2_village.json du village hex** :
   bloqué par le claim multi-terminal sur `forgia-game/src/lib.rs` (retrait du plugin)
   et couplé à story-586 §Suite (gated validation runtime). Le registre marque
   forgia_village_debug.json « DÉBRANCHÉ, dépose prévue ». À exécuter dès claim levé.
2. **Exécution ADR-0004 (QA)** : décision Antoine requise ; le câblage touche
   forgia-game (claimé).

## Notes techniques

- Scanner sensor-audit : la fenêtre 3-lignes ratait les sensors déclarés via
  `const SENSOR_PATH` (29 faux « missing »). Passe 2 : suit l'ident du const jusqu'à
  un contexte d'écriture dans le même fichier. Résultat 82 produits = 82 déclarés.
- Les 4 « missing » restants étaient des producteurs RETIRÉS (voicelines/music_state =
  refactor bark abandonné ; textures = string de test ; player_hp_diag = WIP jamais
  atterri) → section « Producteurs retirés » du registre, hors backticks (hors parsing).
- Instabilité `cargo test` locale re-confirmée pendant la story : `-p forgia-mode-roguelite`
  101 erreurs E0463 puis **110/110 verts au retry** sans modification — contention avec
  le build de l'autre terminal. Renforce le choix CI per-crate (story-592 AC6).

## Auto-QA

Sub-agents indisponibles (limite dépense API). Substitution manuelle : chaque crate
touchée check+clippy+test verts (fps-mode-roguelite 110, xtask clippy 0, postprocess
check 0) ; les 5 gates exécutés et verts ; claim multi-terminal respecté (zéro fichier
du diff de l'autre terminal modifié — toon_config.rs et memory_sensor.rs vérifiés hors diff).

## Fichiers touchés

run_debug.ps1 · .claude/rules/fine-grained-crates.md · README.md · CONTRIBUTING.md ·
docs/adr/ADR-{0002,0003,0004}*.md · ARCHITECTURE.md · xtask/src/main.rs ·
xtask/asset-load-allowlist.toml · docs/observability/SENSOR_REGISTRY.md ·
scripts/git-hooks/pre-push · crates/forgia-mode-roguelite/src/toon_config.rs ·
crates/forgia-postprocess/src/lib.rs
