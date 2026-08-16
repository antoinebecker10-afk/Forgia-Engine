# story-705 — Le Sac accueille les ressources d'arène

> ⚠️ **Corrigé le 2026-08-13.** La première rédaction disait « créer un onglet Inventaire ». **Faux** :
> `MenuPage::Sac` existe déjà, et story-678 avait tranché sa raison d'être — *« le Sac et le Marketplace
> dans la barre : ce sont les deux écrans où l'on PREND quelque chose »*. Cette story **complète** le Sac,
> elle ne crée pas de page. Portée réduite d'autant.

**Statut** : DRAFT (2026-08-13)
**Épic** : E4 (socle loot) · **Scale** : Standard
**Décision source** : [GDD §6 accueil](../design/gdd-forgia-the-spared.md) · [GDD §7 Économie](../design/gdd-forgia-the-spared.md)

## Pourquoi

Les mobs doivent lâcher des **ressources d'arène**, et le joueur doit les voir. Sans surface
d'affichage, l'arbre de talents (story-706) n'a pas de carburant visible — et une monnaie qu'on ne
voit pas monter ne motive personne.

## Le socle existe déjà — on branche, on ne construit pas

| Pièce | État |
| --- | --- |
| `ForgiaLootTablesPlugin` | ✅ câblé dans `forgia-mode-roguelite/src/lib.rs:127` |
| `roguelite_loot.toml` | ✅ tables pondérées, pity timer, RNG seedé `(run_seed, stage_id, encounter_idx)` |
| `forgia-rpg-data::inventory` | ✅ 80 slots (LOCK-INV-1) |
| `forgia2_inventory.json` | ✅ capteur existant |
| `MenuPage::Sac` (page + `OwnSystem`) | ✅ existe (story-678) |
| **Catégorie « ressource »** | ❌ à créer |
| **Affichage groupé par catégorie** | ❌ à créer |

## Critères d'acceptation (falsifiables)

- [ ] Le **Sac** liste ce qui est détenu, **groupé par catégorie** (ressource / équipement / relique) —
      aucune page nouvelle n'est créée
- [ ] **Test** : tuer N mobs avec une graine fixée ⟹ le compte de ressources est **déterministe** et
      reproductible (le RNG est déjà seedé, le test doit le prouver)
- [ ] Le capteur `forgia2_inventory.json` expose le compte par catégorie — et **distingue « 0 ressource »
      de « système inerte »** (règle story-699 : un compteur à zéro ne dit pas « ok »)
- [ ] L'inventaire plein (80 slots) a un comportement **écrit** : soit refus lisible, soit auto-conversion.
      Un inventaire qui déborde en silence est un bug de rétention
- [ ] Aucune ressource d'arène n'apparaît dans une table d'Expédition, et réciproquement (cf. story-708)
- [ ] 0 warning clippy · tests verts

## Zone d'ombre à trancher pendant l'implémentation

**Quel est le puits ?** Un inventaire qui se remplit sans se vider devient une corvée. Les ressources
se dépensent à l'arbre (story-706) — mais les **drops d'équipement**, eux, n'ont pas encore de
destination en v1 (les gates et le stuff viennent d'Expédition, qui n'existe pas). Options : les
convertir en ressources, ou ne pas les faire tomber tant que E2 n'est pas là. **Recommandation :
ne pas les faire tomber** — pas de contenu mort dans un menu.

## Fichiers attendus

- `crates/forgia-menu-hub/src/` — le système de dessin du `Sac` (page existante)
- `crates/forgia-rpg-data/src/inventory.rs` — catégorie ressource
- `crates/forgia-mode-roguelite/src/` — drop des ressources sur mort de mob
- `assets/genomes/roguelite/roguelite_loot.toml` — table des ressources d'arène (**data**)

## Dépendances

Aucune bloquante. Se livre en parallèle de story-704.
