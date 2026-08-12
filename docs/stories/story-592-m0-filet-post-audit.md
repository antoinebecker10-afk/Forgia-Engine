# Story-592 — M0 « Filet » post-audit (P0 + crash + stutter + CI)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_lag_events.json`, fichier `lib.rs`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **Source** : [audit complet 2026-06-10](../audit/audit-2026-06-10-full-codebase.md) +
> [roadmap post-audit](../ROADMAP_POST_AUDIT_2026-06-10.md) jalon M0.
> **Scale BMAD** : Standard (6 fichiers, 4 crates orthogonales au WIP de l'autre terminal).
> **Date** : 2026-06-10. **Statut** : EN COURS.

## Contexte

L'audit a identifié 3 risques immédiats : zéro filet (CI morte 44/44, 54 commits non
poussés), un P0 gameplay visible à chaque fin de run (tir à travers les écrans UI),
zéro trace de crash. M0 = les corriger AVANT toute feature.

## Critères d'acceptance

| # | AC | Statut | Preuve |
|---|---|---|---|
| AC1 | origin/main == HEAD local (push des 54 commits) | ✅ | `rtk git push` → ok ✓ main (2026-06-10) |
| AC2 | `block_fire=true` → firing path ne tourne pas | ✅ | `fire_allowed` run-condition (forgia-fps) + test `fire_blocked_when_block_fire_set` ; 22 tests verts, clippy 0 |
| AC3 | Champ mort `block_movement` (0 écrivain/0 lecteur) supprimé + contrat documenté | ✅ | forgia-input/lib.rs — doc lecteur/écrivain par champ (vérifié par grep) |
| AC4 | Un panic écrit `forgia2_crash.json` {id, severity, next_step, message, location, thread, backtrace, ts} | ✅ | `install_crash_sensor` + `build_crash_payload` pur (src/main.rs) ; 2 tests JSON ; clippy 0 |
| AC5 | memory_sensor ne rafraîchit QUE notre pid (stutter métronome 5 s) | ✅ code / ⏳ runtime | `ProcessesToUpdate::Some(&[pid])` ; 85 tests verts. **Validation runtime requise** : forgia2_lag_events.json sans spine 5,01 s |
| AC6 | Job CI test sans `continue-on-error`, qui passe | ✅ code / ⏳ run | ci.yml : boucle per-crate ubuntu + timeout-minutes sur tous les jobs lourds. Premier run vert à observer après AC7 |
| AC7 | Billing GitHub réparé → 1 run CI vert | ⏳ ANTOINE | Action compte GitHub, hors de portée IA |

## Notes techniques

- **AC2** : `fire_weapon_minimal` a déjà 16 params (limite Bevy SystemParam) → gate via
  `.run_if(fire_allowed)` plutôt qu'un param supplémentaire. Le test headless partage la
  même run-condition que le système réel.
- **AC4** : payload via serde_json (échappement backslashes Windows + newlines backtrace).
  Le hook délègue au hook par défaut (console préservée). Croisement post-mortem :
  forgia2_roguelite_state.json porte run/wave/seed à 1 Hz.
- **AC5** : fichier `memory_sensor.rs` hors du diff de l'autre terminal (claim vérifié),
  baseline `cargo check -p forgia-observability` verte avant édition.
- **AC6** : le TODO historique (« feature unification bevy_water + bevy_gauge ») est
  **réfuté** : bevy_gauge n'existe pas dans le workspace, et `cargo test -p forgia`
  (racine : tire bevy_hanabi ET bevy_water dans la même sélection) PASSE. 3 probes
  `--workspace` locaux = 3 échecs DIFFÉRENTS (std introuvable / bevy_hanabi AlphaMode /
  prelude bevy manquant dans forgia-terrain, 283 erreurs) → signature d'artefacts
  incrémentaux corrompus ou de courses avec le build de l'autre terminal, PAS un bug de
  manifest. Stratégie CI : boucle per-crate (chaque crate isolée — 9 crates validées
  vertes localement : forgia, fps, input, observability, worldgen, genome-core,
  village-generator, rpg-data, qa-core) + `timeout-minutes` partout (le job windows de
  4 h facturé ×2 est ce qui a épuisé le billing GitHub). Sur machine CI propre,
  `--workspace` mérite un re-essai ultérieur (story de suivi).

## Auto-QA

⚠️ Sub-agents verifier/qa-lead **indisponibles** (limite mensuelle de dépense API atteinte
pendant l'audit). Vérification manuelle substituée : cargo check + cargo test + clippy
par crate touchée (verts, voir AC), greps de non-régression (block_look contrat vérifié),
claim multi-terminal respecté (aucun fichier du diff de l'autre terminal modifié).
Re-passer l'auto-QA agents à la prochaine session si quota restauré.

## Fichiers touchés

- `crates/forgia-fps/src/lib.rs` — import InputBlockers, `fire_allowed`, run_if, test
- `crates/forgia-input/src/lib.rs` — suppression block_movement, doc contrat
- `src/main.rs` — panic hook + build_crash_payload + 2 tests
- `Cargo.toml` (racine) — dep serde_json
- `crates/forgia-observability/src/memory_sensor.rs` — refresh pid-only
- `.github/workflows/ci.yml` — job test (AC6)
