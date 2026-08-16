# story-708 — Deux monnaies étanches, et le cap qui éteint l'Abîme

**Statut** : DRAFT (2026-08-13)
**Épic** : E6 / E3 · **Scale** : Standard
**Décision source** : [GDD §7 « Économie — deux monnaies étanches »](../design/gdd-forgia-the-spared.md)

## Le problème que ça règle

Si les mobs d'arène lâchent des ressources qui alimentent la même progression que les Expéditions,
**les deux modes nourrissent la même économie**. Pourquoi partir explorer si le couloir donne la même
chose, en plus court et plus sûr ? C'est le risque nommé au GDD §13 — *« les modes se cannibalisent »* —
et c'est le pilier **P2** qui existe pour l'empêcher.

## Ce qu'on livre

| Monnaie | Tombe où | Achète quoi |
| --- | --- | --- |
| **Ressources d'arène** — `abyss_resource_drop_<universe>` | mobs de l'Abîme | les nœuds de l'arbre, **et rien d'autre** |
| **Matériaux d'expédition** — `forge_material_drop_<universe>` | Expéditions | les gates d'univers et l'équipement |

Étanches **par construction**, pas par convention.

Plus le frein : **`weapon_level_cap_universe_<n>`** (gène déjà nommé au GDD). Au cap, l'Abîme cesse
de lâcher la ressource concernée. Débloquer l'univers suivant relève le cap → l'Abîme s'éteint et se
**rallume** à chaque étage. C'est ce qui l'empêche de devenir la ferme optimale.

*Le socle existe* : `meta_shop.rs:170` porte déjà un plafond par arme et `l.234` le clamp de niveau
effectif. Il reste à le lier à l'univers, pas à l'inventer.

## Critères d'acceptation (falsifiables)

- [ ] **Grep des tables** : aucune ressource d'arène n'apparaît dans une recette de gate ou
      d'équipement ; aucun matériau d'expédition n'apparaît dans un coût de nœud. **C'est le test de
      P2**, et il doit être un test automatisé, pas une relecture
- [ ] **Test** : au cap de l'univers courant, N mobs tués ⟹ **0 ressource** de la catégorie plafonnée
      (et le joueur le comprend : le HUD ou l'inventaire le dit)
- [ ] Débloquer l'univers suivant ⟹ le drop reprend, sans relancer le jeu
- [ ] Le capteur `forgia2_power.json` expose le cap courant et la distance au cap — sinon « pourquoi
      je ne gagne plus rien » devient une question de support
- [ ] `xtask validate-genomes` vert (références croisées mortes = échec)
- [ ] 0 warning clippy

## Zone d'ombre restante

**Combien d'univers existent aujourd'hui ?** Le GDD en annonce 6 à terme. Le cap par univers n'a de
sens que s'il y en a au moins deux — sinon l'Abîme s'éteint définitivement au premier cap et le jeu
n'a plus de mode B. **À vérifier avant de livrer** : si un seul univers est jouable en v1, le cap
doit être un plafond souple (rendement décroissant) plutôt qu'un mur sec.

## Fichiers attendus

- `assets/genomes/roguelite/roguelite_loot.toml` — table des ressources d'arène, par univers
- `assets/genomes/roguelite/roguelite_progression.toml` — `weapon_level_cap_universe_<n>`
- `crates/forgia-mode-roguelite/src/` — application du cap au drop
- `crates/forgia-observability/src/` — le capteur `power` expose cap + distance
- un test d'étanchéité qui lit **les deux** familles de tables

## Dépendances

story-705 (les ressources doivent tomber) · story-706 (l'arbre doit exister pour les dépenser)
