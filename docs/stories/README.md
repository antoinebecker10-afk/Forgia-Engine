# Stories

Tout travail non trivial (niveau Standard ou Enterprise dans `CONTRIBUTING.md`) a une
story dans ce dossier. Une story n'est pas un ticket : c'est un contrat de livraison avec
des critères qu'on peut **réfuter**.

## Convention

- **Nom de fichier** : `story-NNN-slug.md`, avec `NNN` un entier unique. Le prochain id
  libre est affiché par `cargo xtask story-ids`, qui échoue sur tout doublon nouveau.
- **En-tête** : un champ `Status:` parmi `TODO`, `IN_PROGRESS`, `DONE`, `BLOCKED`,
  `CANCELLED`.
- **Corps** : le contexte (quel symptôme, quelle mesure), les critères d'acceptation
  falsifiables (une commande, une valeur attendue, un capteur à lire), les fichiers
  touchés, et le récap de test runtime quand l'effet n'est visible qu'en jeu.

## Gates

| Gate | Ce qu'il vérifie |
| --- | --- |
| `cargo xtask story-ids` | aucun id dupliqué |
| `cargo xtask story-gate --story NNN` | un `DONE` correspond à du code réellement livré (fichiers présents, lignes, tests) |
| `cargo xtask story-index --check` | `_index.md` est à jour (`cargo xtask story-index` le régénère) |
| `cargo xtask wip-check` | trois stories au plus en `IN_PROGRESS` |

## Historique

Les stories écrites pendant le développement initial du moteur ne sont pas publiées : elles
décrivent le jeu qui a servi de client interne, et non le moteur. Ce dossier repart
vide ; la numérotation reprend au prochain id libre.
