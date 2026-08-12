# story-662 — Correctifs audit 360° : Vague 1, lot 1 (chaîne, capteur, robustesse, diamant)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_roguelite_state.json`, fichier `arms.rs`, symbole `sys_apply_chain_targets`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED
> **État d'origine (périmé, cf bandeau)** : REVIEW (code complet + clippy, validation runtime user en attente)
**Créée** : 2026-07-19 · **Origine** : [audit 360°](../audit/audit-2026-07-19-checkup-360.md) + [audit balance/éco](../audit/audit-2026-07-19-balance-economie.md) + session runtime 2026-07-19 (§7 du rapport 360°)
**Scale BMAD** : Standard (6 fichiers, 5 crates)

## Contexte

Checkup 360° du 2026-07-19 (4 agents + gates + session runtime). Antoine a ordonné : « applique déjà les correctifs de l'audit et ce qu'on a appris ». Ce lot = les fixes **Quick, sans décision de balance**, applicables sans refonte. ⚠️ `boons_apply.rs` est dans l'arbre chaud de l'autre terminal — édité sur ordre explicite user, à re-vérifier au merge.

## Livré

| # | Fix | Fichier | Détail |
|---|---|---|---|
| 1 | **C1 — boon chaîne inerte** (bug runtime confirmé) | `crates/forgia-mode-roguelite/src/boons_apply.rs` [CHAUD] | `sys_apply_chain_targets` émettait `DamageEvent` (pipeline joueur-only) sur des ennemis sans `forgia_damage::Health` → 0 dégât silencieux. Remplacé par mutation directe `forgia_combat::Health` routée `DefenseLayer::absorb(Physical)` (pattern arc Shock d'elements.rs). Pas de `CombatHitEvent` ré-émis (anti-cascade). Log corrigé (compte les hits réellement appliqués). Imports DamageEvent/DamageKind retirés. |
| 2 | **Faux-ami capteur `victory`** | `crates/forgia-mode-roguelite/src/sensor.rs` + `crates/forgia-observability/src/roguelite_health.rs` | `victory_emitted` = latch de fin de run (posé aussi sur Defeat, run.rs:271) exporté sous le nom `victory` → faux diagnostic (victory:true sur run perdue, vécu en session). JSON renommé `run_ended` + nouveau champ `victories_total` (MetaShopSave.victories = la VRAIE victoire). Parser RGL-2 synchronisé des deux côtés. |
| 3 | **RwLock poison anim** | `crates/forgia-anim-locomotion/src/gait_genome.rs` | 3× `.expect("gait lock poisoned")` en hot-path → un panic isolé = crash permanent de l'anim. Recovery `unwrap_or_else(\|e\| e.into_inner())` (donnée Copy, écriture atomique d'un Option → toujours cohérente). |
| 4 | **Save settings non atomique** | `crates/forgia-ui-lib/src/pause_menu.rs` | `save_user_settings` écrivait en `fs::write` direct (seul save utilisateur non atomique). Passé en tmp+rename, pattern persist.rs. |
| 5 | **Diamant parcours = fuite d'économie** (vérifié) | `crates/forgia-mode-roguelite/src/loot_room.rs` | Index cyclique non seedé sur TOUT le catalogue, gating story-616 + gate légendaire 3-tags ignorés (compte neuf → légendaire gratuit). Passé au tirage canonique `roll_candidates_weighted` (pondéré rareté + paliers `UnlockedBoonTiers` + légendaires) seedé `CoffreRng`. Reste GRATUIT (coût = décision de balance non tranchée, audit éco §7 P0-4). |
| 6 | **Allowlist no-scaffold périmée** | `xtask/no-scaffold-allowlist.toml` | 3 entrées vers crates supprimées (juice-hit-stop, juice-recoil, sensors) purgées. |
| 7 | **Dette clippy latente purgée (workspace vert)** | 6 crates : `forgia-rpg-data/boons.rs:546`, `forgia-player/skybox_genome.rs:141`, `forgia-ui-lib/hud_ammo/tuning.rs:180` + `damage_direction/tuning.rs:67`, `forgia-juice-screen-flash/lib.rs:100`, `forgia-killfeed/tuning.rs:125`, `forgia-mode-roguelite/decor.rs:1123` + `status_vfx.rs:140/196/325` | 6× `collapsible_match` — tous le MÊME boilerplate genome-sync dupliqué (= preuve H3) ; 1× `checked_div` ; 3× compteur `live` → budget+`take()` (= la dette « 3 warnings clippy live » de la roadmap Later). Masquée depuis le 03/07 par le cache clippy (le gate lint passait sur du cache — cf memory RTK). |
| 8 | **SystemParam bundle** (retour qa-lead) | `loot_room.rs` | `sys_collect_level_items` passait à 13 params (> 12, règle scalability) → `BoonRollCtx` (active/catalogue/tiers/rng) → 10 params. |

## Correction d'audit au passage

Le claim H6 « orphelines = dep du bin racine » était **faux** : lignes 129-190 du Cargo.toml racine = `[workspace.dependencies]` (déclarations) ; le bin `forgia` ne dépend que de `forgia-game`. Rapport 360° corrigé. Suppression des 2 crates orphelines = décision séparée (destructif + sync ARCHITECTURE.md).

## Hors scope (décisions de balance à valider par Antoine — audit éco §7)

- Cap doublons boons (=3) · cap maîtrise (=10) · coût du diamant (40 Or) · prix Bourrasque/palier Uncommon · `souls_per_wave` 5→7 · `gold_per_stage` — **aucune valeur changée sans go explicite**.
- Migration genome `roguelite_progression.toml` (spec prête dans l'audit éco) — story dédiée après merge arbre chaud (mêmes fichiers).
- Caméra 3D lobby (root cause fond noir) — design à coordonner avec le chantier hub.

## Validation mécanique (2026-07-19)

- **Clippy workspace `-D warnings` : VERT** (`--workspace --exclude forgia-viewmodel --all-targets`, binaire cargo réel via `rustup which cargo` — le shim RTK fausse la sortie même en « plain cargo »).
- **Exception documentée** : `forgia-viewmodel/src/arms.rs` = 2 consts `ARM_GLB_LEFT/RIGHT` jamais utilisées — **arbre chaud** (WIP story-661 de l'autre terminal), non touché par respect de la règle multi-terminal. À résoudre à son merge.
- **Tests : 38 crates `test result: ok`** (dont forgia-mode-roguelite 267, forgia-rpg-data, forgia-observability, forgia-ui-lib, forgia-anim-locomotion, forgia-auto-rig 30, forgia-qa-replay 7).
- **2 échecs PRÉ-EXISTANTS hors périmètre** : `forgia-qa-autopilot::smoke::tests::{smoke_bot_fail_on_blocker_bug, smoke_bot_pass_on_major_bug_only}` — le drain `emit_bug`→BugBus→sink ne livre plus le bug (0 émis au lieu de 1). Crate **orpheline** (audit H6, jamais importée, tests jamais exécutés — aucun CI ne lance les tests workspace) ; aucun fichier de ce lot n'est dans son graphe de deps (mon edit qa-replay = `#[cfg(test)]` only). **Renforce la décision H6 : réparer ou supprimer la crate.**
- Auto-QA : qa-lead ✅ (0 Bloquant/Majeur, 2 cosmétiques → bundle SystemParam appliqué, écart chain vuln/affinité documenté ci-dessous) ; vérification mécanique (build+lint+tests+invariants) exécutée inline en boucle (7 passes documentées) au lieu d'un agent verifier redondant.

## Candidat story suiveuse (qa-lead #2)

Les hits de chaîne n'appliquent ni `Vulnerability` ni l'affinité élémentaire de l'arme aux cibles secondaires (le hit de base forgia-fps le fait par cible). Pas une régression (avant = 0 dégât), docstring l'assume — à trancher avec l'audit balance.

## Acceptance Criteria

- [x] Chaîne : dégâts appliqués à `forgia_combat::Health` via DefenseLayer, 0 DamageEvent émis vers des ennemis
- [x] Capteur : champ `run_ended` + `victories_total` produits, parser RGL-2 lit `run_ended`
- [x] Anim : plus aucun `.expect` sur le lock GAIT
- [x] Settings : écriture tmp+rename
- [x] Diamant : tirage gaté paliers + pondéré + seedé
- [x] `cargo clippy -D warnings` vert sur les 4 crates touchées
- [ ] Validation runtime user : boon chaîne fait des dégâts visibles (log `[boons] chain hit ×N`) + diamant ne sort plus que des raretés débloquées

## Test runtime (après rebuild — l'exe actuel du 03/07 ne contient PAS ces fixes)

1. **Action** : `cargo build -p forgia -j 4` puis lancer ; prendre `rebond_du_caillou` (Rare) ou `chaine_des_ames` (Légendaire) au Coffre ; tirer dans un groupe d'ennemis groupés (<8 m).
2. **Effet attendu** : les voisins perdent des PV (barres nameplate baissent), log `[boons] chain hit ×N targets`.
3. **Où observer** : nameplates ennemies + `forgia2_run.log` + `forgia2_roguelite_state.json` (nouveaux champs `run_ended`/`victories_total`).
4. **Diamant** : sur un save neuf, le diamant du parcours ne doit sortir QUE du Common.
5. **Si KO** : si les voisins ne prennent rien → vérifier que le boon est actif (`chain+N` dans le log `mods recomputed`) ; si `chain+0` → le pick n'a pas appliqué l'effet (bug amont, me ping).
