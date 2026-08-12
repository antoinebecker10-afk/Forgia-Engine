# Story-616 — Roguelite : déblocage des paliers d'atouts (boons) « évolutif »

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_coffre.json`, fichier `meta_shop.rs`, symbole `MetaShopCatalogue`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **État d'origine (périmé, cf bandeau)** : CODE-COMPLETE (2026-06-24) — validation runtime à faire
> **Niveau BMAD** : Standard (cross-crate `forgia-rpg-data` + `forgia-mode-roguelite` + genome)
> **Demande user** : « limiter les armes, atouts etc. pour rendre le jeu évolutif » →
> increment 2 (les **atouts**, après les armes en [story-613](story-613-roguelite-weapon-unlocks-evolutif.md)).
> Décisions (AskUserQuestion) : Âmes-shop à l'Enclume + départ minimal (Common seul).

## Ce qui change

Avant : les ~18 boons (Common→Légendaire) sont tous dans le pool dès le départ.
Après : **pool Common offert d'office** ; les paliers **Uncommon → Rare → Légendaire**
se débloquent en **permanence** à l'Enclume (en Âmes). Le gate légendaire intra-run
(3 tags identiques, `ActiveBoons.unlocked_legendary`) **reste** et s'applique EN PLUS.

| Palier | key | Coût (Âmes) |
|---|---|---|
| Common | — | gratuit (toujours offert) |
| Uncommon | `uncommon` | 80 |
| Rare | `rare` | 200 |
| Légendaire | `legendary` | 400 |

## Architecture (concept-first) — cross-crate sans cycle

- **Producteur (vérité)** : `MetaShopSave.unlocked_boon_tiers: Vec<String>`
  (forgia-mode-roguelite, persisté `config/meta_shop_save.toml`, défaut vide = Common only) ;
  coûts = `[[boon_tier_unlocks]]` du genome `roguelite_meta_shop.toml` → `MetaShopCatalogue`.
- **Pont** : `forgia_rpg_data::boons::UnlockedBoonTiers` Resource (définie BAS, lue par le roll).
  `meta_shop::sys_sync_unlocked_boon_tiers` (forgia-mode-roguelite) la pousse depuis le save
  (écrit seulement si différent). Sens de dépendance respecté : forgia-mode-roguelite → forgia-rpg-data.
- **Consommateur** : `forgia_rpg_data::boons::roll_candidates(catalogue, active, tiers, count, rng)`
  filtre `tiers.allows(b.rarity)` AVANT le filtre légendaire intra-run. **Deux call-sites**
  branchés : `sys_handle_open_coffre` (coffre wave-clear) + `loot_room::sys_roll_zone_reward`
  (portail de fin de zone).
- **UI** : l'Enclume (`draw_meta_shop_lobby`) liste les 3 paliers sous les 4 upgrades ;
  `sys_meta_shop_input` gère **Digit5/6/7** = achat de palier (déduit Âmes + persiste).

## Critères d'acceptation

- [ ] Nouveau save : au coffre/portail, seuls des boons **Common** sont proposés.
- [ ] Enclume : section « ATOUTS » avec les 3 paliers + coûts ; `5/6/7` débloquent (Âmes déduites, persisté).
- [ ] Après déblocage `uncommon`, des boons Uncommon apparaissent dans les rolls suivants.
- [ ] Légendaires : nécessitent palier `legendary` débloqué **ET** 3-tag-stack intra-run (inchangé).
- [ ] Coûts lus du genome (fallback miroir par champ).
- [x] `cargo check` + clippy 0 warning + **183 tests** (142 roguelite + 41 rpg-data, dont gating) + binaire `-j 4` OK.

## Test runtime

1. **Action** : `cargo run -p forgia -j 4`. Au Lobby, regarde l'Enclume (droite) : section ATOUTS, paliers verrouillés.
2. **Effet attendu** : lance une run, ouvre un coffre (fin de vague) → seuls des boons Common.
   Reviens au Lobby, `5` pour débloquer Uncommon (80 Âmes) → relance → des Uncommon apparaissent.
3. **Où observer** : Enclume (paliers DÉBLOQUÉ/coût) ; `forgia2_coffre.json` (candidats) ; console
   `[meta-shop] palier d'atouts débloqué`.
4. **Variantes si KO** :
   - Toujours tous les boons → `sys_sync_unlocked_boon_tiers` ne tourne pas, ou `UnlockedBoonTiers` pas init.
   - `5/6/7` sans effet → pas assez d'Âmes / déjà débloqué (log) ; conflit Digit5-7 ? (peu probable).

## Suivi

- Numéro **story-615** pris par un autre chantier (aim assist) → ce gating boons = 616 ;
  le gating de loadout in-run (armes verrouillées injouables) a été re-référencé **story-613**.
- Increment 3 = déblocages par **accomplissement** (meilleur pour le save à 1666 Âmes).
- Hotspot `meta_shop.rs` (>15 edits) + `weapon_select.rs` → à splitter.
