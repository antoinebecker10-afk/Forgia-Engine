# Story 714 — L'attaque de mêlée, qui n'existe pas

**Statut** : DRAFT
**Niveau BMAD** : Standard
**Dépend de** : 713 (les archétypes, et la décision de vitesse)

---

## Le constat

**Aucun ennemi de Forgia ne frappe au corps à corps.** Recherche sur
`attack_cooldown`, `melee_range`, `windup`, `telegraph` dans
`forgia-ai-arena-bot/` et `forgia-mode-fps-arena/` : **zéro résultat**. Le seul
mode d'attaque est `shot_damage` — un hitscan à 35 m, avec 0,35 s de mise en
joue et 4° de dispersion.

Or les génomes décrivent grunt et elite comme des **mêlées** (`melee_damage`,
portée 3,0 m et 3,5 m), et `map-design-intention.md` §2.1 dérive la taille des
salles de la capacité d'un essaim de mêlée à *arriver* sur le joueur.

Conséquence directe : un campement peuplé aujourd'hui serait **trois tireurs de
plus**, pas un combat varié.

## Pourquoi c'est ce qui manque le plus pour un combat « à la WoW »

La lisibilité d'un combat WoW tient à ce qu'on **voit venir** le coup : une barre
de cast, un mob qui s'élance, un cercle au sol. Forgia n'a aujourd'hui qu'un
warmup de 0,35 s invisible. La mêlée est l'occasion d'introduire le télégraphe,
parce qu'un coup de mêlée SANS télégraphe est juste un dégât aléatoire au contact.

## La mécanique proposée — trois temps, lisibles

```
   approche          ARMEMENT (télégraphe)        frappe          récupération
  ─────────────►   ████ 0,7 s, visible  ────►   cône 3 m   ────►   0,9 s vulnérable
   navmesh          le mob s'immobilise           dégâts            fenêtre de riposte
```

- **L'armement immobilise** : c'est ce qui rend l'esquive possible et le combat
  lisible. Un mob qui frappe en courant est injouable en FPS.
- **La récupération est la fenêtre du joueur.** C'est là qu'est le rythme.
- **Le télégraphe est visuel ET sonore** — un joueur qui regarde ailleurs doit
  l'entendre.

## Critères d'acceptation

- [ ] Un ennemi d'archétype mêlée s'approche jusqu'à sa portée puis attaque
- [ ] Les trois durées (armement / frappe / récupération) viennent du génome
- [ ] **Le télégraphe est visible ≥ 0,5 s avant les dégâts** — mesuré par un test,
      pas jugé à l'œil
- [ ] Esquiver en reculant hors de portée pendant l'armement annule les dégâts
- [ ] Le mob **ne se téléporte jamais** pour atteindre sa cible (invariant existant)
- [ ] Le capteur de 712 compte les attaques armées / abouties / esquivées
- [ ] Les dégâts passent par `forgia_damage::DamageEvent` côté joueur
      (⚠️ deux types `Health` coexistent — cf. `[[reference_two_health_types_combat_vs_damage]]`)

## Le piège de vitesse — à trancher AVANT d'écrire

| | vitesse |
|---|---|
| joueur, sprint | **9,75 m/s** |
| ennemi actuel (`arena_bots`) | **3,5 m/s** |
| grunt selon son génome | 9,0 m/s |

À 3,5 m/s, **un mob de mêlée ne rattrape jamais un joueur qui recule** : il n'est
pas difficile, il est décoratif. À 9,0 m/s il colle au joueur en permanence et
l'esquive devient impossible sans dash — que le joueur n'a pas
(`map-design-patterns.md` : saut 1,174 m, ni mantle ni dash).

**Piste** : vitesse d'approche modérée (~5 m/s) + une **charge** brève et
télégraphée (le gène `elite_charge` ×2,5 existe déjà dans le génome elite). La
menace vient alors de la charge, pas de la course de fond — et la charge se lit,
donc s'esquive.

## Fichiers

- `crates/forgia-ai-arena-bot/src/tactical.rs` (nouvelle phase d'attaque)
- `crates/forgia-combat/` (application des dégâts)
- `assets/genomes/enemies/enemy_grunt.toml`, `enemy_elite.toml` (gènes de timing)

## Risque

**Moyen-haut.** Touche `tactical.rs` (2 401 lignes) qui pilote toute l'arène.

## Cross-refs

- `.claude/rules/map-design-intention.md` §2.1, §2.3
- `.claude/rules/in-game-test-recap.md` — l'esquive se juge manette en main
