# Contrôle de la sortie — RÈGLE BLOQUANTE

> **Un contrôle qui passe déclare ce qu'il a mesuré, et il porte sur l'artefact
> LIVRÉ — jamais sur ce qui l'a produit.**

Origine : nuit du 2026-08-17 au 18. Sept défauts, une seule forme.

| Le contrôle mesurait | Le défaut était |
|---|---|
| la longueur des os ✓ | la dérive de la racine |
| la dérive de la racine ✓ | dans un axe mal nommé |
| les collisions de noms de **nœuds** ✓ | celles des noms de **clips** |
| 2 plugins montés ✓ | dans les 4 autres |
| la `severity` ✓ | la fraîcheur |
| la fraîcheur des capteurs **verts** ✓ | un `critical` de la veille |
| la taille **absolue** d'un os ✓ | le **ratio** au corps cible |

Sept fois, le vert était sincère et la **portée** trop étroite. Aucun contrôle
n'a jamais dit *« j'ai vérifié ceci, pas cela »*. Ce n'est pas l'absence de
contrôle qui coûte, c'est le contrôle qui rassure sur une tranche qu'il ne
couvre pas.

---

## 1. Les deux obligations

**Déclarer sa portée.** Tout contrôle imprime la taille de son échantillon et
ce qu'il a laissé dehors — succès compris. Sans ça, « rien à signaler » et
« rien n'a été regardé » se lisent pareil. Zéro mesuré n'est pas vert, c'est
**aveugle** : rendre 1, pas 0.

**Porter sur la sortie.** Compter le fichier produit, pas ce que la boucle a
ajouté. Mesurer le corps fusionné, pas les fichiers d'entrée. Relire l'artefact
écrit, pas la variable qui a servi à l'écrire. *Chaque fois qu'on a mesuré
l'entrée cette nuit-là, on a raté ; chaque fois qu'on a mesuré la sortie, on a
trouvé.*

## 2. Deux corollaires payés cher

**Une exception se NOMME.** `ExpeditionVentPlugin` est hors du harnais parce que
son `MaterialPlugin` exige l'app de rendu. Écrit dans `PLUGIN_GATE_SANS_HARNAIS`,
c'est une décision ; implicite, c'était un trou.

**Un contrôle ne doit pas pouvoir tuer ce qu'il observe.** Un capteur qui prend
en `Res<T>` une ressource d'une autre crate fait paniquer le système quand elle
manque. `Option<Res<T>>`, et **publier l'absence** — un capteur rapporte, il ne
subit pas.

## 3. Un nouveau contrôle naît avec sa ligne de base

Un contrôle qui échoue sur tout l'existant se désactive dans la semaine, et le
projet se retrouve sans contrôle du tout — pire que la dette. Il interdit donc
les régressions **nouvelles**, consigne les anciennes dans un fichier qui **ne
peut que rétrécir**, et signale les entrées réparées pour qu'on les retire.

`docs/audit/plugin-gate-baseline.txt` (41) · `deps-mortes-baseline.txt` (21) ·
`corps-anim-baseline.txt` (1) · `strates-baseline.txt` (98).

## 3 bis. Une liste écrite à la main est un contrôle qui ne peut pas échouer

Ajouté le 2026-08-18, après **deux occurrences de la même ligne** :

| Date | Zone ajoutée | Ce qui était muet | Durée |
|---|---|---|---|
| 2026-07-20 | Roguelite | flash de dégâts + vignette bas-PV, dans le mode **shippé** | ~4 mois |
| 2026-08-18 | Expédition | flash, arc de direction, killfeed | 4 jours |

`matches!(mode, Fps | Roguelite)`, recopié dans douze fichiers. Il **compile**,
il **passe les tests**, il ne déclenche **aucun cliquet**. Il ne produit pas une
erreur : il produit une **absence**. C'est la sœur du §1 — un contrôle dont la
portée est trop étroite — mais en pire, parce qu'ici la liste *est* le contrôle.

**La règle** : à la **deuxième** occurrence d'une énumération sur le même axe,
extraire une **propriété** et la déclarer en `match` **exhaustif sans joker**
dans le socle. Le compilateur refuse alors la variante non déclarée : l'oubli
devient une erreur au lieu d'un silence.

Implémenté : `forgia_core::capacites` (combat · retour de combat · HUD générique
· vagues), avec un test d'emboîtement pour qu'une zone incohérente casse au test.
Le signal qui doit alerter : **le même `matches!` sur `GameMode` dans deux
fichiers.**

## 4. Les cliquets qui l'appliquent

```bash
cargo run -p xtask -- plugin-gate     # un plugin a un garde : monté dans un
                                      # App::new() qui tourne, et un capteur
cargo run -p xtask -- deps-mortes     # dépendance interne déclarée, jamais utilisée
python tools/assets/verifier_corps.py # le corps d'animation LIVRÉ
```

Les trois tournent en CI (`ratchets`). Régénérer une base : `--ecrire-baseline`.

## 5. Ce que ça ne couvre pas

La **qualité** de ce qui est mesuré. `plugin-gate` juge qu'un garde existe, pas
qu'il soit bon. `verifier_corps` attrape des défauts de structure, pas une
animation laide. `deps-mortes` ne regarde que les crates `forgia-*` — les
dépendances tierces s'utilisent souvent via un prélude, et un contrôle étroit
et juste vaut mieux qu'un large et bruyant.

## 6. Cross-refs

`observability-required.md` (tout nombre publié porte son seuil) ·
`no-speculative-fix.md` (citer la source de l'évidence) ·
`on-demand/map-design-patterns.md` §13-14, d'où cette règle est généralisée ·
`multi-terminal-coordination.md` §5 (artefact = preuve, pas la source).
