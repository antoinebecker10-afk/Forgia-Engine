# Story-586 — RPG : débrancher le village legacy (dédup), garder le hex

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (fichier `lib.rs`, symbole `VillageLoadResult`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **État d'origine (périmé, cf bandeau)** : code-complete (2026-06-09), NON COMMITÉ, runtime à valider.
> **Scope BMAD** : Enterprise (cross-crate, cartographié + vérifié adversarialement via workflow 7 agents).
> **Origine** : audit A→Z monde RPG ([audit-2026-06-09-rpg-world-stack-a-to-z.md](../audit/audit-2026-06-09-rpg-world-stack-a-to-z.md)), bug B1 = **deux systèmes de village rendent au même centre**.

## Problème

Le mode RPG lançait DEUX systèmes de village au même centre (16,16) :
- **A (gardé)** : village hexagonal KayKit (`worldgen_village.rs`) — tuiles, murs/tours/portes, bâtiments multicolores, routes hex, PNJ.
- **B (débranché)** : `village-loader` legacy (genome procgen) — **6 bâtiments rouges** imbriqués + **routes ribbon Bézier**, colliders chevauchants, doublons (2 puits, 2 tavernes…).

## Cartographie + vérif adversariale (workflow 7 agents)

`VillageLoadResult` (produit par B) avait 2 consumers critiques que B **seul** alimentait :
- **R1 (CRITIQUE)** : `teleport_player_to_terrain` lisait `village.spawn_position` → sans B, le joueur restait à `(0,2,0)` (hardcodé forgia-player), potentiellement **sous le terrain flatten**.
- **R7 (MAJEUR)** : `FoliageExclusionDisc` posé UNIQUEMENT par B → sans lui, **flicker + arbres dans le village** (le clear réactif de A ne fait que despawn après coup).
- **R2** : `forgia-genome-village` n'est PAS orpheline (le flatten genome A l'utilise, `make_village_flatten_zones`) → **gardée**.
- **Pas de panic possible** (tout `Option<Res<>>`, 0 `.unwrap()` sur Resource village). PNJ/anchor safe (`RpgVillageAnchor` produit par `spawn_world`, indépendant de B).

## Implémentation (incrémentale, sûre — crates NON déposées)

1. **Stop trigger B** : `forgia-rpg/src/lib.rs` — retiré l'insert `LoadVillageGenomeRequest` → `process_village_genome_request` no-op → 0 bâtiment B, 0 `VillageLoadResult`, 0 route ribbon.
2. **R1 — spawn joueur** : `teleport_player_to_terrain` re-câblé sur `RpgVillageAnchor` (Y = `well_y` flatten) au lieu de `VillageLoadResult`. Spawn = `anchor.center + (3, 2, 3)`.
3. **R7 — exclusion foliage** : `sys_spawn_worldgen_village` (hex) pose `FoliageExclusionDisc { center, radius: FOLIAGE_CLEAR_RADIUS }` à la source. Le clear réactif reste un filet de sécurité.

`cargo check -p forgia` OK, clippy 0.

## Critères d'acceptation

- AC1 — Plus de bâtiments rouges doublons ni de routes ribbon dans le village RPG (B inerte). ⏳ runtime
- AC2 — Joueur spawn au centre du village hex (pas à l'origine ni sous le terrain). ⏳ runtime
- AC3 — Pas d'arbres ni flicker dans l'enceinte (exclusion à la source). ⏳ runtime
- AC4 — Aucun crash, PNJ intacts. ✅ (Option-gated, anchor préservé) + ⏳ runtime
- AC5 — `cargo check -p forgia` + clippy 0. ✅

## Suite (follow-up séparé, après validation runtime)

- **Dépose des 3 crates orphelines** : `forgia-village-loader`, `forgia-village-generator`, `forgia-village-kit` (plus aucun consumer une fois B retiré ; **GARDER `forgia-genome-village`** = R2). Inclut : retirer `add_plugins(ForgiaVillageLoaderPlugin)` (forgia-game:121), les systèmes inertes (`spawn_village_paths_when_loaded`), les deps Cargo.toml, l'allowlist L1 `asset-load-allowlist.toml:15`.
- Aujourd'hui ces systèmes B restent **compilables mais inertes** (early-return sans `VillageLoadResult`) — pas de dette runtime, juste du code mort à nettoyer.
