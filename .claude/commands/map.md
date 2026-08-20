---
description: Concevoir ou auditer une carte — charge les deux règles de conception, qui ne sont PAS chargées par défaut
---

# /map — concevoir une carte

Les deux règles de conception de carte pèsent **23,7 Ko**. Chargées à chaque
session, elles coûtaient ~6 000 tokens par session pour un sujet qu'on aborde
quelques fois par mois. Elles vivent donc dans `docs/design/`, HORS de
`.claude/rules/` — mesuré le 2026-08-20 : le harnais charge `.claude/rules/`
**récursivement**, sous-dossiers compris, donc un dossier `on-demand/` dedans
était chargé quand même. Seule la sortie du dossier rend le « à la demande »
réel ; cette commande est ce qui les fait entrer.

> **Une règle se paie à CHAQUE session ; un fichier chargé à la demande ne se
> paie que quand on s'en sert.** Le budget de contexte du projet a un plafond
> (`cargo run -p xtask -- context-budget`) et il était dépassé.

## Ce que tu dois faire

**Lis les deux fichiers, dans cet ordre — l'intention avant la géométrie :**

1. `docs/design/map-design-intention.md`
   Le **QUOI** : spec de combat, archétypes d'ennemis, composition d'une salle,
   rythme d'une run, porte de sortie. **Bloquant** : sans spec de combat, la
   géométrie n'a pas de juge.

2. `docs/design/map-design-patterns.md`
   Le **COMMENT** : les 14 patterns de construction, leur tableau d'état
   (TENU / PARTIEL / CONTRÔLÉ / ÉCRIT), et ce qu'ils ne couvrent pas.

Puis applique la procédure de la §0 du premier fichier — elle est ordonnée, et
sa première étape est bloquante.

## Ce que cette commande ne dispense pas de faire

Les règles chargées par défaut restent en vigueur : `concept-first`,
`no-hardcode`, `observability-required`, `spawn-clearance` (qui référence les
deux fichiers ci-dessus et reste, lui, toujours chargé parce qu'il porte un
invariant de gameplay, pas une méthode de conception).
