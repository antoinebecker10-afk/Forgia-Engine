# story-711 — Les munitions disent quelle branche tu as choisie

**Statut** : DRAFT (2026-08-13) — ⚠️ **bloquée par story-706 et D6**
**Épic** : E6 (surface visuelle de l'arbre) · **Scale** : Standard
**Décision source** : session design 2026-08-13 (Antoine) · [GDD §7 arbre de talents](../design/gdd-forgia-the-spared.md)

## Le problème qu'elle règle

L'arbre de talents ne vend **que des choix**, jamais des pourcentages — c'est la règle qui empêche la
dérive de puissance mesurée par le capteur `power`. Mais elle a une contrepartie connue :
**un choix qui ne se voit pas ne se ressent pas.**

Un `+12 % dégâts` se lit dans une fiche. Une technique, non — sauf si le jeu la montre.

Les munitions de [story-710](story-710-munitions-dessinees-et-element-visible.md) sont déjà à
l'écran, en permanence, sous les yeux du joueur à chaque tir. Les faire changer d'apparence selon la
branche choisie ferme la boucle : **choix → visible → identité**.

## La règle qui borne le coût : par BRANCHE, jamais par nœud

> **4 armes × 3 branches = 12 variantes.** Pas une par nœud.

Deux raisons, et la seconde compte plus que la première :

1. **Le coût d'art suivrait l'arbre.** Un visuel par nœud, c'est autant d'assets que de nœuds — et
   la taille de l'arbre n'est pas encore chiffrée. On signerait un chèque en blanc.
2. **C'est plus lisible.** On lit une **spécialisation**, pas un détail. Un joueur qui croise un
   coéquipier doit pouvoir dire « il est parti foudre » d'un coup d'œil — pas déchiffrer un nœud.

## ⚠️ Bloquée par D6

Tant que la décision D6 n'est pas prise (les avantages *joueur* partagent-ils la monnaie des
techniques d'arme ?), la **forme des branches** n'est pas fixée. Écrire les variantes visuelles avant
elle, c'est produire de l'art pour une structure qui peut changer.

## Critères d'acceptation (falsifiables)

- [ ] Chaque branche d'arme a **une** variante visuelle de munition, déclarée en génome
      (`roguelite_talents.toml` → clé visuelle), **zéro variante en dur**
- [ ] **Test** : branche choisie ⟹ variante affichée, déterministe et testable headless
- [ ] Sans branche choisie, la munition reste celle de story-710 (l'élément de base) — **jamais de
      trou visuel**
- [ ] Le nombre de variantes produites est **≤ 3 par arme**. Un test de garde le vérifie : au-delà,
      la story a dérivé vers du par-nœud
- [ ] La couleur d'élément reste lisible sous la variante — la branche **module** la silhouette,
      elle n'écrase pas l'élément (le GDD réserve ces couleurs à la lisibilité gameplay)
- [ ] 0 warning clippy · tests verts · `xtask validate-genomes` vert

## Ce qu'on ne fait PAS

- Pas de variante par nœud
- Pas de changement de **taille** de chargeur selon la branche : ce serait une stat déguisée, et
  l'arbre n'en vend pas
- Pas de variante qui rende une arme **méconnaissable** — l'identité de l'arme prime sur celle de la branche

## Dépendances

- **story-706** (l'arbre doit exister) — bloquant
- **D6 tranchée** — bloquant
- **story-710** (les munitions doivent être dessinées avant d'avoir des variantes) — bloquant
