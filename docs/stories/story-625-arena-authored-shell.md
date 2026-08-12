# Story-625 — Arène : coquille authored data-driven (Tier 1, modèle Returnal)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_stage_layout.json`, fichier `authored.rs`, symbole `Ready`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED
> **État d'origine (périmé, cf bandeau)** : 📝 DRAFT
**Créée** : 2026-06-26
**Niveau BMAD** : Standard (~5 fichiers, cross-module léger forgia-stage)
**Audit source** : `docs/audit/audit-2026-06-26-arena-render-quality-vs-parcours.md`
**Plan** : `docs/thoughts/plan-arena-authored-shell-tier1.md`

## Contexte

Constat user : le **parcours** (GLB authored, 1216 pièces composées) rend mieux que l'**arène** (scatter procédural uniforme de ~18 props sur un disque). L'arène ne réalise pas sa propre bible (`docs/lore/locations/crypts_of_anvil.md`, 6 sections authored). Direction validée : **modèle Returnal hybride** — composition authored + procédural en overlay/fallback.

## Objectif

Introduire une **coquille authored data-driven** : un genome `arena_layouts.toml` que l'IA écrit, instancié depuis les GLB atomiques Inferno/KayKit **existants** via le `forgia-prefab` existant, sans casser le procédural. Preuve : fosse à mêlée + perchoir de `crypts_of_anvil` recréés 100 % depuis la data.

## Acceptance Criteria

- [ ] AC1 : `arena_layouts.toml` décrit fosse + perchoir crypts ; 0 coordonnée arène en dur ajoutée en Rust
- [ ] AC2 : l'arène crypts montre une composition authored au lieu du scatter
- [ ] AC3 : pièce `melee_pit` → `AnchorKind::MeleePit` + nom `Module_melee_pit_authored` → porte du boss non cassée
- [ ] AC4 : perchoir walkable (collider TriMesh)
- [ ] AC5 : `suppress_procedural_modules` coupe le scatter sur crypts ; `forge_sanctum` (sans layout) = procédural intact (non-régression)
- [ ] AC6 : `forgia2_stage_layout.json` expose `layout_source="authored"` + `authored_pieces` > 0
- [ ] AC7 : hot-reload Shift+F12 d'`arena_layouts.toml` re-pose les pièces
- [ ] AC8 : 0 warning clippy `-p forgia-stage`, tests purs verts, story-gate vert

## Scope (fichiers)

- `crates/forgia-stage/src/authored.rs` (NEW)
- `crates/forgia-stage/src/lib.rs` (register genome + spawn authored + suppress procédural)
- `crates/forgia-stage/src/layout_sensor.rs` (observabilité authored)
- `assets/genomes/arena_layouts.toml` (NEW data)
- `docs/stories/_index.md` (entrée)

## Hors scope → stories suiveuses

- story-626 : Tier 2 (props signature bible, palette rose pastel, 6 sections complètes)
- story séparée : Tier 4 rendu (outline ré-activé, SSAO, ScatteringMedium 0.18)

## QA auto (2026-06-26)

Passe `verifier` + `qa-lead` (post-impl-auto-qa). Gates mécaniques verts : `cargo check -p forgia-stage`, `clippy -D warnings` EXIT 0, `test` 104 passés (dont 5 nouveaux `authored`), `check -p forgia-mode-roguelite` + `-p forgia`.

**qa-lead = WARN** (0 Bloquant, 0 Majeur, 5 Mineur, 1 Cosmétique). Corrigés dans cette story :
- **BUG-625-01** (Mineur, race) — le stage pouvait passer `Ready` en procédural avant que `arena_layouts.toml` charge → coquille jamais appliquée. Fix : garde d'attente du genme authored (miroir du wait stages/pois).
- **BUG-625-04/05** (Mineur, observabilité) — sensor trompeur (`info`/"0 module") pour un stage authored. Fix : `severity`/`next_step` authored-aware dans `write_layout_sensor`.

Reportés → **story-626** (Tier 2) avec renvoi explicite :
- BUG-625-02 (Mineur) : `sys_collide_authored_pieces` sans cap de retry si un GLB est introuvable (non déclenché — chemins GLB vérifiés existants).
- BUG-625-03 (Mineur, latent) : doublon `PlayerSpawn` possible si un futur TOML ajoute `role=player_spawn` (aucun TOML actuel ne le fait ; commentaire de garde à ajouter).
- BUG-625-05 (compteurs) : `cover/sniper/melee_count` du sensor à 0 pour les pièces authored (placements synthétiques `ModulePlacement` = refacto Tier 2).
- BUG-625-06 (Cosmétique) : position du test de parsing ≠ TOML prod (sans impact).

## Notes

- Règles : no-hardcode, hot-reload, observability-required, fine-grained-crates (module pas crate : 1 seul consommateur), post-impl-auto-qa, story-done-gate.
- Synergie : la pièce `melee_pit` authored pilote la position de la porte du boss (`boss_portal::sys_reconcile_boss_gate`).
