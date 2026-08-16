# Story 713 — Donner un consommateur aux trois archétypes d'ennemis

**Statut** : DRAFT
**Niveau BMAD** : Standard (story + checklist)
**Dépend de** : 712 (sinon on règle à l'aveugle)
**Bloque** : 714, 716

---

## Le constat qui a déclenché cette story

```
enemy_grunt   : 0 fichier(s) Rust
grunt_max_hp  : 0 fichier(s) Rust
enemy_elite   : 0 fichier(s) Rust
elite_charge  : 0 fichier(s) Rust
archer_max_hp : 0 fichier(s) Rust
```

**Les trois archétypes n'existent pas en code.** `assets/genomes/enemies/`
contient `enemy_grunt.toml`, `enemy_archer.toml`, `enemy_elite.toml`,
`boss_default.toml` — et rien ne les lit.

C'est plus grave qu'un fichier mort : `map-design-intention.md` §2 fait reposer
**tout le dimensionnement des cartes** sur eux (« une salle doit laisser
l'archétype ARRIVER », « ligne_max ≤ min(vision des ennemis) »). Le Vallon a été
dimensionné contre des valeurs — grunt 9,0 m/s, vision 20 m, elite charge ×2,5 —
qui ne sont branchées nulle part. Le manifeste du Vallon écrit d'ailleurs
`grunt_vision_m = 20.0` par campement : une donnée dérivée d'un archétype fantôme.

Ce qui pilote réellement les ennemis aujourd'hui est **un seul type**,
`assets/genomes/arena_bots.toml` :

| | valeur réelle | archétype prétendu |
|---|---|---|
| PV | 200 | grunt 30 / archer 45 / elite 120 |
| vitesse | **3,5 m/s** | grunt **9,0** / elite 5,0 (charge 12,5) |
| attaque | tir hitscan 35 m, 12 dmg / 1,5 s | grunt **mêlée 3,0 m** |
| vision | 50 m | grunt 20 / archer 35 / elite 25 |

Autrement dit : **il n'y a qu'un ennemi dans Forgia, et c'est un tireur lent.**

## Ce que fait cette story

Un `EnemyArchetype` lu depuis les génomes, consommé au spawn. Pas de nouveau
comportement — seulement débrancher le hardcode et brancher la donnée qui existe
déjà.

## Critères d'acceptation

- [ ] Les 4 génomes (`grunt`, `archer`, `elite`, `boss`) ont un consommateur Rust
- [ ] `hp`, `vitesse`, `vision`, `portée`, `dégâts` viennent du génome, 0 littéral
- [ ] Le spawn accepte un archétype nommé ; l'arène garde son comportement actuel
      (régression zéro : ses 502 tests passent)
- [ ] **Un test compare le génome au code** : si un gène est renommé, ça casse ici
      et pas en jeu
- [ ] Hot-reload `Shift+F12` prend effet sur un ennemi vivant
- [ ] Le capteur de 712 expose l'archétype de chaque ennemi vivant

## Décision à trancher (game design, pas technique)

Les archétypes déclarent **grunt 9,0 m/s** contre **9,75 m/s** en sprint joueur.
`tactical.rs` documente déjà la conséquence : *« on ne sème pas un grunt à la
course »* — 27 s de sprint pur pour gagner 20 m. Est-ce voulu ?

- **Oui** → la fuite passe par la laisse et le compteur d'evade (déjà en place)
- **Non** → il faut baisser la vitesse des grunts, et la mêlée devient kitable

**Cette décision conditionne 714.** Un mob de mêlée à 3,5 m/s face à un joueur
qui sprinte à 9,75 est décoratif.

## Fichiers

- `crates/forgia-combat/` ou nouvelle `forgia-enemy-archetypes/` (≥ 2 consommateurs
  prévus : arène + expédition → crate fine justifiée, cf. `fine-grained-crates.md`)
- `crates/forgia-mode-fps-arena/src/wave.rs` (spawn)
- `assets/genomes/enemies/*.toml` (compléter les gènes manquants)

## Risque

**Moyen.** Touche le spawn de l'arène, qui marche. Le filet est ses 502 tests.

## Cross-refs

- `.claude/rules/map-design-intention.md` §2 — les archétypes comme contrainte de carte
- `.claude/rules/no-hardcode.md`
- `[[feedback_une_description_n_est_pas_une_preuve]]` — un artefact ne se prouve
  que par son consommateur
