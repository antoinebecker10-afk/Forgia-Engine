# Story-644 — P1 : HUD défensif segmenté (Vie / Bouclier / Armure)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (fichier `player_hp.rs`, symbole `DefenseLayer`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **Source** : masterplan `docs/audit/forgia-gunfire-masterplan-2026-07-01.md` §5 (HUD P1,
> « quick-win débloqué après P0-2 »). Rend VISIBLE tout le combat P0-2/3/4 (boucliers,
> armure, réactions, éléments) qui est aujourd'hui invisible pour le joueur.
> **Scale BMAD** : Standard (UI multi-crate : forgia-ui-lib + forgia-enemy-nameplate).
> **État d'origine (périmé, cf bandeau)** : IN_PROGRESS — Inc.1 en cours.

## Objectif
Afficher les couches défensives (Bouclier bleu / Armure jaune) au-dessus/autour de la Vie,
joueur ET ennemis, + icônes de statut, pour rendre lisible le système Gunfire-lite (choix
d'arme piloté par la couleur de défense de l'ennemi).

## Incréments (chacun compile + tests verts)
- **Inc.1 — Barre défensive JOUEUR** — ✅ **FAIT** (non commité) : `player_hp.rs` (egui) dessine
  au-dessus de la barre HP les segments Bouclier (bleu) puis Armure (jaune), empilés, affichés
  seulement si `*_max > 0`. Lit `Option<&DefenseLayer>` sur le joueur (attaché par P0-2 ; absent
  hors Roguelite → pas de barre). Helper `draw_defense_segment` (track+fill+outline). Couleurs UI
  cosmétiques. clippy 0-warn touché, binaire compile. Le bouclier 50 du joueur est enfin visible.
- **Inc.2 — Barres défensives ENNEMI** — ✅ **FAIT** (non commité) : `forgia-enemy-nameplate`
  spawn des mini-barres Bouclier (bleu) / Armure (jaune) empilées AU-DESSUS de la HP, **seulement
  si la couche existe** (Tank=armure, Runner=bouclier → 0 quad superflu). `update_defense_bars`
  met à jour `scale.x` depuis `DefenseLayer` du bot (miroir `update_hp_fill`). Couleurs genome
  (`shield_color`/`armor_color`, `#[serde(default)]` backward-compat). clippy 0-warn touché,
  binaire compile. Drive le choix d'arme (élément vs couleur de couche).
- **Inc.3 — Icônes de statut** : burn/poison/shock/miasma près du nameplate ennemi (et/ou
  joueur), lues depuis les composants StatusBurn/StatusPoison/Vulnerability/StatusMiasma.

## Hors scope
- Barre de boss dédiée (P1 séparé). Refonte visuelle complète du HUD.
- Couleurs pré-rupture (orange) — polish P1 ultérieur.
