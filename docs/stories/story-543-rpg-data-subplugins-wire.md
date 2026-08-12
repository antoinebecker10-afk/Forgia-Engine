# Story-543 — Re-câbler sub-plugins forgia-rpg-data

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_entities.json`, fichier `inventory.rs`, symbole `ForgiaRpgPlugin`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

**Status** : DRAFT
**Priorité** : 🟠 P1 — features silencieusement inactives
**Scale BMAD** : Standard (story + checklist)
**Origine** : audit wiring 2026-05-27 (`docs/audit/wiring-2026-05-27.md` §2)

## Problème

`ForgiaRpgPlugin` (wiré dans `forgia-game/src/lib.rs:79`) est censé activer les sous-systèmes RPG. Mais 3 sub-plugins définis dans `forgia-rpg-data` ne sont jamais `add_plugins(...)` :

- `ForgiaInventoryPlugin` — `crates/forgia-rpg-data/src/inventory.rs:156`
- `ForgiaQuestsPlugin` — `crates/forgia-rpg-data/src/quests.rs:116`
- `ForgiaXpCurvesPlugin` — `crates/forgia-rpg-data/src/xp_curves.rs:86`

Conséquence : les Resources/systems Inventory/Quests/XpCurves sont absents du World. Les call-sites RPG qui les attendent silencieusement (via `Option<Res<>>`) → features mortes, sans erreur visible.

NB : `ForgiaDialoguePlugin` (même crate) **est** wiré (`forgia-game:71`) → l'oubli des 3 autres est très probablement une régression de la fusion `forgia-rpg-data` (sub-plugins individuels avant, meta-plugin attendu mais non créé).

## Critères d'acceptation

- [ ] AC1 — Décider du pattern :
  - **Option A** (recommandée) : créer `ForgiaRpgDataPlugin` meta dans `forgia-rpg-data/src/lib.rs` qui add_plugins les 4 (Inventory + Quests + XpCurves + Dialogue), puis wirer celui-là dans `forgia-game/src/lib.rs` à la place du Dialogue isolé
  - **Option B** : wirer les 3 sub-plugins individuellement dans `forgia-game/src/lib.rs` (cohérent avec pattern actuel Dialogue)
- [ ] AC2 — `cargo check -p forgia-game` clean
- [ ] AC3 — `cargo clippy -p forgia-rpg-data -p forgia-game --no-deps` 0 warning
- [ ] AC4 — Test runtime RPG mode : sensors `forgia2_inventory.json`, `forgia2_quests.json`, `forgia2_xp.json` (s'ils existent) montrent Resources présentes ; sinon vérifier via `forgia2_entities.json`
- [ ] AC5 — Audit memory : noter dans MEMORY.md pattern "sub-plugins via meta vs wirage individuel" pour future fusion

## LOCK-INV-1

L'invariant Inventory 80 slots max (LOCK-INV-1) reste applicable. Cette story réactive simplement le Plugin qui matérialise l'inventaire, pas modifier sa capacité.

## Test in-game recap

1. **Action** : `cargo run -p forgia-game --profile release-fast` → entrer RPG depuis menu
2. **Redémarrage requis** — modif `.rs` lib.rs
3. **Effet attendu** : ouvrir inventaire (touche I ou TAB selon keybind) → UI inventory affichée ; quest log accessible
4. **Sensor** : `forgia2_entities.json` doit lister `InventoryRoot` entity (ou équivalent selon impl)
5. **Variantes si KO** :
   - Si UI ne s'ouvre pas → vérifier que `ForgiaUiHudPlugin` consomme bien `InventoryState` Resource
   - Si panic "Resource not found" → un sub-system attendait déjà ces Resources, créer le Plugin n'est qu'un demi-fix
