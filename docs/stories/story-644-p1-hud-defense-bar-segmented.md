# Story-644 — P1 : HUD défensif segmenté (Vie / Bouclier / Armure)

> **Source** : masterplan `docs/audit/forgia-gunfire-masterplan-2026-07-01.md` §5 (HUD P1,
> « quick-win débloqué après P0-2 »). Rend VISIBLE tout le combat P0-2/3/4 (boucliers,
> armure, réactions, éléments) qui est aujourd'hui invisible pour le joueur.
> **Scale BMAD** : Standard (UI multi-crate : forgia-ui-lib + forgia-enemy-nameplate).
> **Statut** : IN_PROGRESS — Inc.1 en cours.

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
- **Inc.2 — Barres défensives ENNEMI** (`forgia-enemy-nameplate`) : sous le nameplate HP,
  mini-barres Bouclier/Armure (billboard 3D quads, réutilise le pattern bg+fill), lues depuis
  `DefenseLayer` du bot. Couleurs genome. Toggle visibilité si la couche manque.
- **Inc.3 — Icônes de statut** : burn/poison/shock/miasma près du nameplate ennemi (et/ou
  joueur), lues depuis les composants StatusBurn/StatusPoison/Vulnerability/StatusMiasma.

## Hors scope
- Barre de boss dédiée (P1 séparé). Refonte visuelle complète du HUD.
- Couleurs pré-rupture (orange) — polish P1 ultérieur.
