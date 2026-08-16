# story-707 — Les Âmes déjà dépensées : la migration que rien ne couvre

**Statut** : DRAFT (2026-08-13)
**Épic** : E6 · **Scale** : Standard — **risque élevé (perte de progression joueur)**
**Décision source** : [GDD §7 Économie](../design/gdd-forgia-the-spared.md) · story-706

## Pourquoi elle existe séparément

Retirer l'Enclume (story-706) invalide `meta_shop_save.toml` : les Âmes qu'un joueur a dépensées en
`max_hp`, `damage`, `armor`, `gold` pointent vers un système qui n'existe plus.

**Et c'est précisément le cas où le défaut déjà consigné mord.** `SAVE_VERSION` est écrit dans les
quatre saves et **jamais lu** — les seules occurrences hors écriture sont des `assert_eq!` de test.
La compatibilité repose entièrement sur `#[serde(default)]`, qui couvre l'**ajout** d'un champ, pas
un changement de sens. Ici le sens change entièrement.

`meta_shop.rs:605` documente déjà un précédent : *« les sauvegardes antérieures repartent à 0 sans
migration »*. Le refaire une seconde fois, en connaissance de cause, serait un choix — pas un accident.

## Ce qu'il faut décider (et c'est court)

Trois sorties, par ordre de coût croissant :

1. **Rembourser** — les Âmes dépensées reviennent au solde, le joueur re-dépense dans l'arbre.
   Simple, honnête, et le joueur y gagne du choix.
2. **Convertir** — chaque rang d'Enclume donne N nœuds. Demande une table de correspondance qui n'a
   pas de sens naturel (un `+5 max_hp` ne « vaut » aucune technique).
3. **Repartir de zéro**, annoncé. Acceptable seulement si la base joueur est nulle — ce qui est
   probablement le cas aujourd'hui, mais ne le sera plus après le premier playtest externe.

**Recommandation : (1) rembourser.** C'est la seule qui ne demande aucun arbitrage de valeur.

## Critères d'acceptation (falsifiables)

- [ ] **`SAVE_VERSION` est LU** — un code de branchement existe, et une save de version antérieure
      emprunte un chemin de migration explicite. C'est la vraie livraison de cette story
- [ ] **Test** : une `meta_shop_save.toml` de l'ancienne forme, chargée, produit un solde d'Âmes
      égal à `souls_total + Σ(coût des rangs achetés)` — vérifié, pas observé
- [ ] Une save corrompue reste préservée en `.corrupt-<timestamp>` (comportement existant de
      `persist.rs`, à ne pas casser)
- [ ] Une save de version **future** (supérieure) ne détruit rien : elle refuse et le dit
- [ ] Le joueur est **informé** au premier lancement post-migration — pas de progression qui bouge en silence
- [ ] 0 warning clippy · tests verts

## Fichiers attendus

- `crates/forgia-mode-roguelite/src/persist.rs` — lecture et branchement sur `version`
- `crates/forgia-mode-roguelite/src/meta_shop.rs` — chemin de migration
- test dédié avec une save de l'ancienne forme en fixture

## Portée

Cette story règle la migration **pour les quatre saves**, pas seulement `meta_shop` : le mécanisme
manquant est le même partout (`save`, `identity`, `equipment`, `ftue`). Le livrer une fois suffit.

## Dépendances

Sort **avec** story-706, jamais après.
