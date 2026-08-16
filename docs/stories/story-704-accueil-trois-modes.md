# story-704 — L'accueil : trois modes, deux portes verrouillées

**Statut** : DRAFT (2026-08-13) — décision design tranchée, code non commencé
**Épic** : structure d'accueil (GDD §6) · **Scale** : Standard
**Décision source** : [GDD §6 « L'écran d'accueil — un menu de préparation, pas un lieu »](../design/gdd-forgia-the-spared.md)

## Pourquoi

Le Hall comme **lieu** part en post-v1 avec le 5v5. L'accueil de la v1 assume d'être un menu de
préparation : le joueur voit la forme du jeu au premier écran, se prépare, et part vite.

Aujourd'hui `MenuPage::Root` n'annonce aucun mode — on arrive dans une liste d'onglets
(`Forgeron`, `Armes`, `Talents`, `Enclume`, `Codex`, `Missions`, `Succes`, `Stats`) sans savoir ce
qu'on peut jouer.

## Ce qu'on livre

```
   [ EXPÉDITIONS ]   [ ARÈNES ]   [ 5v5 · prochainement ]
                          [ Château de Forgia · prochainement ]
```

- Trois cartes de mode sur `Root`, dont **une verrouillée lisiblement** (pas grisée sans explication).
- Un bouton **Château de Forgia**, verrouillé, qui dit qu'il existe un ailleurs.
- **Expéditions** est verrouillé lui aussi tant que E2 n'existe pas — même traitement que le 5v5,
  aucune promesse fausse.

## Le bug qu'on corrige au passage

Le menu promet l'arène du chapitre choisi, et le stage est tiré au `run_seed` **sans lire
`SelectedChapter`** — sur 7 lancements du playtest 2026-08-09 : 4× `forge_sanctum`, 2×
`hauts_paturages`. Le mode choisi doit être celui qui se lance, sinon l'écran ment.

## Critères d'acceptation (falsifiables)

- [ ] Depuis `Root`, les 3 modes sont visibles **sans naviguer**, et l'état verrouillé porte un motif
      lisible (« bientôt », pas un cadenas muet)
- [ ] Cliquer un mode verrouillé ne lance rien et ne produit aucune erreur console
- [ ] **Test** : `SelectedChapter = X` ⟹ le stage chargé est celui de `X`, sur 10 graines différentes
      (c'est un test, pas une observation)
- [ ] Le capteur `forgia2_roguelite_state.json` expose le chapitre demandé **et** le stage résolu —
      un écart doit se voir sans relancer
- [ ] Aucune page existante n'est supprimée : la nav reste accessible
- [ ] 0 warning clippy · `cargo check -p forgia-menu-hub` vert

## Fichiers attendus

- `crates/forgia-menu-hub/src/nav.rs` — `MenuPage::Root` gagne les cartes de mode
- `crates/forgia-menu-hub/src/registry.rs` — déclaration des 3 modes + bouton Château
- `crates/forgia-mode-roguelite/src/lib.rs` — résolution du stage depuis `SelectedChapter`
- `assets/genomes/roguelite/roguelite_identity.toml` — libellés et états de verrouillage (**data**, pas de littéral en dur)

## Ce que ça engage

⚠ Afficher du contenu verrouillé sur l'écran d'accueil est une **promesse publique**. Elle se tient,
ou elle se retire — elle ne se laisse pas pourrir.

## Dépendances

Aucune. C'est le premier incrément, et il corrige un bug connu.
