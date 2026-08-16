# story-706 — L'arbre de talents remplace l'Enclume

**Statut** : DRAFT (2026-08-13) — ⚠️ **bloquée par D6** (cf. plus bas)
**Épic** : E6 (reprofilé) · **Scale** : Enterprise (10+ fichiers, méta-progression + save)
**Décision source** : [GDD §7 « L'arbre de talents »](../design/gdd-forgia-the-spared.md)

## Pourquoi, et c'est mesuré

L'Enclume des Âmes vend exactement quatre choses :

```
max_hp · damage · armor · gold
```

Quatre stats pures, permanentes, achetées avec une monnaie farmée. Le capteur `power` est **en
alerte** : *« la puissance réelle dépasse largement le modèle du mur — la difficulté annoncée n'est
pas celle qui est jouée »*. C'est le motif que le consensus 2026 désigne comme ce qui tue le genre :
la réussite devient dépendante du farm, pas de l'habileté.

L'arbre **remplace** l'Enclume, il ne s'y ajoute pas — sinon Forgia porte quatre systèmes de
progression (boons, Enclume, arbre, équipement) dont deux font le même travail.

## La règle dure

**Un nœud ne vend jamais un pourcentage.** Chaque nœud est une technique ou un comportement
(« la foudre chaîne sur 3 cibles », « le rechargement actif accélère le tir suivant »). Un arbre
chiffré reproduirait le défaut mesuré en le renommant.

## Le socle existe déjà à moitié

`crates/forgia-mode-roguelite/src/meta_shop.rs` porte déjà :

- un **niveau de maîtrise par arme** (clé génome → niveau, `+1` par run) — l.588
- `level_up_weapon(&mut self, key, max_level)` — l.922
- un **plafond par arme** (`1` = pas de progression) — l.170
- le **clamp de « niveau effectif »** qui évite d'afficher « Niveau 13/6 » sur une vieille save — l.234

L'arbre **convertit** cet acquis. Il ne repart pas de zéro.

## ⚠️ Bloquée par la décision D6

> Les avantages **joueur** et les techniques **d'arme** partagent-ils la même monnaie ?

Si oui, le joueur achètera toujours les avantages joueur d'abord — ils s'appliquent partout, tout le
temps — et les branches d'arme ne seront prises qu'en dernier. **Ne pas écrire les nœuds avant que D6
soit tranchée** (GDD §14).

## Critères d'acceptation (falsifiables)

- [ ] **Grep des tables de nœuds : zéro champ de stat brute** (`damage`, `max_hp`, `armor`, `+%`).
      C'est le test de P4 appliqué à l'arbre
- [ ] `MenuPage::Enclume` retirée de la nav ; la page `Talents` cesse d'afficher « Ton niveau est la
      somme de ce que tu as débloqué à l'Enclume » — cette phrase devient fausse
- [ ] Le **niveau joueur** a une nouvelle source explicite (il gate le 5v5 en E10 — il doit survivre
      à la disparition de l'Enclume)
- [ ] Chaque nœud se lit dans un génome (`roguelite_talents.toml`), **zéro nœud en dur**
- [ ] Le capteur `forgia2_power.json` **repasse au vert** après recalibrage — c'est la mesure qui dit
      si l'opération a servi à quelque chose
- [ ] 0 warning clippy · tests verts · `xtask validate-genomes` vert

## Fichiers attendus

- `crates/forgia-mode-roguelite/src/meta_shop.rs` → conversion (ou nouveau module `talents.rs`)
- `crates/forgia-menu-hub/src/registry.rs` — page `Talents` réécrite, `Enclume` retirée
- `assets/genomes/roguelite/roguelite_talents.toml` — **neuf**
- `assets/genomes/roguelite/roguelite_meta_shop.toml` — déprécié

## Dépendances

- **D6 tranchée** (bloquant)
- story-705 (les ressources doivent exister avant de pouvoir les dépenser)
- story-707 (migration) doit sortir **en même temps**, pas après
