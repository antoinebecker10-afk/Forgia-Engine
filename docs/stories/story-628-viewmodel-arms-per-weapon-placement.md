# Story-628 — Placement des mains AUTO par-arme + géométrie bras améliorée

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : aucune trace.** Ni fichier, ni capteur, ni symbole
> parmi ceux qu'elle cite n'existe dans le dépôt. Le travail n'a pas été fait.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED
> **État d'origine (périmé, cf bandeau)** : CODE COMPLETE — en attente validation runtime
**Niveau BMAD** : Quick→Standard (forgia-viewmodel arms + forgia-fps sync + genome)
**Créée** : 2026-06-24
**Origine** : suite 617/618. Le placement camera-local fixe marchait pour Pépin mais
cassait sur les autres armes (chaque arme a un `offset_z` + taille différents → mains à
la mauvaise profondeur). User : « ça doit s'adapter à chaque arme + bras manquent de réalisme ».

## Solution

- **Placement dérivé par-arme** : `position_hands` lit la position + taille RÉELLES de
  l'arme équipée via les helpers `viewmodel_transform` + `viewmodel_target_size` (mêmes
  que l'attach). Main droite = crosse (arrière, `grip_back × longueur`), main gauche =
  sous le canon (avant, `barrel_fwd × longueur`). Marche sur les 4 armes sans réglage.
- **Tuning agnostique** `[viewmodel_arms]` = fractions/décalages (grip_back, barrel_fwd,
  drops…), hot-reload. Plus de positions absolues par-arme à maintenir.
- **Géométrie + réaliste** : avant-bras peau + **manche relevée** (cuff sombre) + dos de
  main arrondi (sphère aplatie) + 4 doigts repliés + pouce.

## Acceptance Criteria

- [ ] AC1 — Les mains tiennent l'arme (crosse + sous le canon) sur Pépin ET en changeant d'arme (1/2/3/4).
- [ ] AC2 — Aucun réglage manuel requis au changement d'arme (auto-dérivé).
- [ ] AC3 — Tuning `[viewmodel_arms]` (fractions) ajuste la pose pour toutes les armes à chaud.
- [ ] AC4 — `cargo check`/`clippy` vert, 0 warning.

## Caveats

- En ADS, les mains restent à la pose hipfire (pas de lerp vers l'ADS) → léger décalage en visée. Suivi possible.
- Réalisme : plafond procédural atteint. Saut de qualité réel = mesh de mains riggé (asset CC0).
