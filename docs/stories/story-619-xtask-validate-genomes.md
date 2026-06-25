# story-619 — `xtask validate-genomes` (gate QA couche 1)

**Statut** : ✅ READY — implémenté + vérifié (planted-error), **à commiter** (G1 git-tracked passera au commit).
**Épopée** : Plan RPG + QA intégré ([rpg-qa-integrated-plan-2026-06-24](../plan/rpg-qa-integrated-plan-2026-06-24.md)) — Phase 0.5.
**Niveau BMAD** : Standard. **Date** : 2026-06-24.
**Origine** : décorticage World of ClaudeCraft — sa seule faiblesse data notée = « no ID cross-reference
validation » ; le contenu genome de Forgia (108 TOML) n'avait **aucune** validation (erreurs = runtime-only).

## Problème
Le contenu data-driven (`assets/genomes/**/*.toml`) n'était vérifié nulle part avant le runtime : une
erreur de syntaxe, un `default` hors bornes, un id de gène dupliqué ou une cross-ref morte ne sortait
qu'en jeu. C'est la couche 1 manquante du QA intégré.

## Livré
- Nouveau gate `cargo xtask validate-genomes` ([xtask/src/main.rs](../../xtask/src/main.rs)) — dep `toml` ajoutée :
  - **L1** : tous les `assets/genomes/**/*.toml` parsent en TOML valide (attrape syntaxe / NBSP).
  - **L1b** : pour les fichiers `[[genes]]` — chaque gène a un `id` unique dans le fichier ; si
    `min`/`max`/`default` sont numériques alors `min ≤ default ≤ max`.
  - **L2** : cross-réfs DÉCLARÉES (jamais inventées, vérifiées green sur le contenu réel) — v1 :
    `roguelite_elements.toml` → chaque élément de `[mapping]` doit avoir sa table `[matchup.<element>]`.
    Extensible via les `check_*` documentés dans le code.
- Branché en **pre-push** ([scripts/git-hooks/pre-push](../../scripts/git-hooks/pre-push)) + **CI** (job `ratchets` de [ci.yml](../../.github/workflows/ci.yml)).
- Fix data trouvé PAR le gate : `roguelite_seed_xor_constant` (`roguelite_run.toml:100`) avait
  `default=2654435769` hors bornes `[1, 999999]` → `max` corrigé à `4294967295` (u32::MAX). Décision
  user : corriger la borne, différer le wiring.

## Vérification (preuve, pas auto-report)
- `cargo xtask validate-genomes` → **OK, 108 fichiers, 1875 gènes** validés, exit 0.
- **Test à erreur plantée** (pattern `malware_scan.test.ts` de WoC) : 2 fautes temporaires
  (`default` hors bornes + TOML invalide) → le gate les **nomme toutes les deux** et exit 1 ; après
  suppression → retour OK exit 0. Le gate n'est pas un no-op.
- `cargo clippy -p xtask --all-targets -- -D warnings` → **0 warning**.

## Acceptance criteria
- [x] `validate-genomes` parse les 108 genomes, exit 0 sur l'arbre courant.
- [x] Attrape `default` hors bornes (prouvé).
- [x] Attrape un TOML invalide (prouvé).
- [x] Attrape un id de gène dupliqué (code + logique testée sur planted).
- [x] Cross-ref elements green sur le contenu réel.
- [x] Branché pre-push + CI (validate-genomes seul).
- [x] 0 warning clippy xtask.

## Auto-QA (post-impl, règle post-impl-auto-qa)
- **verifier** : clippy 0, gate exit 0, `default` gameplay inchangé, dep `toml` résout, gène mort sans autre consommateur (grep workspace-wide).
- **qa-lead** : **0 Bloquant, 0 Majeur** ; 6 Mineurs + 2 Cosmétiques (faux négatifs sur invariants non encodés). **Corrigés dans cette story** : #1 message UTF-8/droits distinct, #2 `min>max` vérifié indépendamment de `default` (+ check valeurs **non finies** inf/nan), #4 commentaire L2, #5 WARN si `[mapping]` absent, #6 message « dossier introuvable » distinct, #7/#8 chemins normalisés `/` (cliquables + tri stable Windows/Linux). Re-prouvés par tests plantés.
- **Différé** : #3 (doublon id cross-fichiers same-layer) → ci-dessous.

## Différé (hors scope, noté honnêtement)
- 🟠 **Phase 0.1** : `roguelite_seed_xor_constant` est un **gène mort** ([run.rs:76](../../crates/forgia-mode-roguelite/src/run.rs#L76) hardcode `0x9E3779B97F4A7C15`, 64-bit, et ne lit jamais ce gène). À wirer OU convertir en const nommée + retirer le hardcode (territoire discipline-seed de la Phase 0.1).
- 🟡 Ajouter à la CI les 4 autres gates pre-push (`arch-drift`, `no-scaffold`, `sensor-audit`, `story-ids`) : **différé** car `sensor-audit`, `story-ids`, `asset-load` sont **rouges localement** sur du travail en cours (doublons stories 597/598/617, drift sensors). À faire quand l'arbre sera propre.
- 🟢 Étendre L2 : cross-réfs loot→item, quête→mob, meta-shop→arme (chacune après vérif du schéma réel).
- 🟢 Détection de doublons de gène **cross-fichiers** dans un même `layer`.
