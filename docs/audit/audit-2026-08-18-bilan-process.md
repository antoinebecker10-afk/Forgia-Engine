# Bilan de la journée du 2026-08-18 — ce qu'il faut changer dans le process

> Compagnon de [`audit-2026-08-18-strates-the-spared.md`](audit-2026-08-18-strates-the-spared.md),
> qui traite l'**architecture**. Celui-ci traite la **méthode** : ce que la
> journée a appris sur la façon dont on travaille, et ce qui doit changer pour
> que les mêmes heures ne se repaient pas.
>
> Tout ce qui suit est **mesuré**. Aucun constat n'est une impression.

---

## 0. Le fait de la journée, avant tout le reste

**Deux terminaux ont audité la même chose, le même jour, sans le savoir.**

L'un a livré `cargo xtask strates` (contrat de couches + dette par crate) et le
type `Constat`. L'autre — moi — a livré `tools/ai/strates.py` (plugins
orphelins, génomes morts, fuites de zone) et `forgia_core::capacites`. L'en-tête
de `xtask/strates.toml` dit : *« Le 2026-08-18, on a audité les liaisons entre
strates à la main, en une demi-heure… »*.

**C'est exactement le défaut que la session cherchait à prévenir, et il s'est
produit pendant la session.** Les deux outils sont complémentaires — mais
personne ne l'a décidé : ça s'est trouvé comme ça.

**Ce qui a manqué** : `multi-terminal-coordination.md` §3 règle 1 dit de lire
`git status` au standup. Je l'ai fait. Ça ne suffisait pas : `git status`
montre ce qui a **changé**, pas ce que l'autre est en train de **créer**. Les
deux outils étaient des fichiers neufs, invisibles à la règle.

> **Règle à ajouter** : au standup, lire aussi les **mtimes des 10 fichiers les
> plus récents** et les **fichiers non suivis** (`git status --porcelain` inclut
> les `??`). Un fichier neuf de moins d'une heure dans un domaine proche = poser
> la question avant d'écrire.

---

## 1. Les défauts de classe, et ce qui les attrape désormais

### 1.1 La liste tenue à la main — **corrigé**

| Date | Zone ajoutée | Ce qui était muet | Combien de temps |
|---|---|---|---|
| 2026-07-20 | Roguelite | flash de dégâts + vignette bas-PV | ~4 mois |
| 2026-08-18 | Expédition | flash, arc de direction, killfeed | 4 jours |

**Deux fois la même ligne.** Un `matches!(mode, Fps | Roguelite)` recopié dans
douze endroits ne casse jamais quand on l'oublie : il rend une capacité
**invisible**. Mesuré avant correction : **6 prédicats multi-mode sur 12**
ignoraient l'Expédition.

**Remède livré** : `forgia_core::capacites` — `match` exhaustif **sans joker**.
Une nouvelle zone ne compile plus tant que ses capacités ne sont pas déclarées.
L'oubli devient une erreur, pas un silence.

### 1.2 La règle qui cite une donnée morte — **corrigé**

`map-design-intention.md` §2 dimensionnait des salles entières sur
`assets/genomes/enemies/` : grunt 30 pv / 9,0 m/s, archer, elite qui charge.
**Ces quatre fichiers n'ont aucun consommateur Rust.** Les vrais archétypes sont
tank 120/2,8 · runner 35/7,0 · sniper 45/3,2 · boss 800/3,5.

Ampleur : **61 génomes morts sur 141 (43 %)**. La couche *definition* — celle
que `concept-first.md` étape 0 impose de consulter en premier — ment à 43 %.

**Le coût propre à ce défaut** : il ne se signale jamais. Le tableau est
cohérent, les formules sont justes, la dérivation est rigoureuse. Le résultat
*a l'air sourcé*, donc il se propage.

**Remède livré** : `strates.py` C2 + le geste à faire avant de citer un TOML :

```bash
grep -rl "$(basename FICHIER .toml)" --include=*.rs crates/ src/
```

### 1.3 Le contrôle à la portée trop étroite — **partiellement corrigé**

`xtask plugin-gate` déclare honnêtement sa portée :

```
37 plugin(s) contrôlé(s) sur 3 crate(s)
portée : montage cherché dans les blocs #[cfg(test)] de la crate qui déclare le plugin
```

**37 sur 147 = 25 %.** Et il répond à une autre question que celle qu'on croit :
« ce plugin a-t-il un garde de test ? », pas « ce plugin **tourne-t-il** ? ».
`strates.py` C1 pose la seconde (reachability depuis `forgia-game::run`) et
trouve **11 plugins déclarés que l'assemblage n'atteint jamais**.

C'est l'application directe de `controle-de-la-sortie.md` : *un contrôle déclare
sa portée, et il porte sur l'artefact livré*.

### 1.4 Le doublon sous un autre nom — **NON corrigé, et c'est le trou principal**

Mesuré ce jour :

- **15 types publics déclarés dans ≥ 2 crates.** Dont `Health`
  (combat/damage — piège documenté), `Severity` (observability/qa-core),
  `Rarity`, `KillPopup`, `Knockback`, `WallSeg`, `AssetRegistry`, `SpawnDef`.
- **`parse_toml` dans 8 crates** et **`load_or_default` dans 8 crates** — huit
  réimplémentations de « charger un TOML + relire son mtime ». Ce n'est pas de la
  négligence, c'est **un manque dans le socle** que chacun a comblé chez soi.

Et le cas qu'**aucun outil** ne voit : `mode-expedition/arme_main.rs` (1 503 l.)
et la brique `forgia-viewmodel` font tous deux « l'arme dans la main ». Pas le
même nom, pas le même type, pas le même vocabulaire.

**Les quatre filets, par fiabilité :**

| Filet | Attrape | Limite mesurée |
|---|---|---|
| nom exact (`ls`, grep) | 0 doublon sous un autre nom | — |
| **recherche sémantique** (grepai) | le **comportement** — a trouvé les 2 spawners sans que je les nomme | **l'index n'avait pas mon code de la dernière heure** ; le score ne discrimine pas (0,016 partout) ; interroger en **anglais** |
| détecteur exact de collisions | types de même nom dans 2 crates | ne voit que les noms |
| table des concepts | « où vit ce concept aujourd'hui » | se périme en silence — elle ignore encore `forgia-enemy-archetypes` |

---

## 2. Ce que la journée dit de la méthode

### 2.1 L'outil qu'on n'utilise pas est pire qu'un outil absent

`grepai` a résolu en une requête ce que j'ai cherché à la main pendant vingt
minutes — et je ne l'ai sorti qu'à la **onzième heure**, parce qu'Antoine a posé
la question. Sa statistique était plate depuis des jours, exactement comme le
2026-08-08 où la même dérive avait été consignée.

**La cause n'est pas la paresse, c'est la fraîcheur** : un index périmé répond à
côté → on l'évite → personne ne le réindexe. Aujourd'hui il tourne, et il rate
quand même la dernière heure. **Vérifier la fraîcheur fait partie de la
requête**, pas du diagnostic d'après.

### 2.2 Un artefact de compilation pollué se lit comme un bug de code

Deux échecs ont coûté du temps :
`STATUS_STACK_BUFFER_OVERRUN` (plantage de rustc) et `E0460 wgpu_hal`
(plusieurs versions de la même crate dans `target/`). **Aucun n'était du code** —
`clippy --workspace --all-targets` était vert. Cause : deux terminaux compilant
avec des jeux de features différents.

`xtask/strates.toml` l'avait **prédit** dans un commentaire écrit le matin même.
Le remède est mécanique : `cargo clean -p <crate>` (14,8 Go ici), puis vérifier
crate par crate. Et surtout : **quand un échec frappe une cible `bin test` qui
ne contient aucun test, suspecter l'artefact avant le code.**

### 2.3 `rtk` fausse la lecture de `cargo`

`rtk cargo check` a affiché « ✓ 0 crates compiled » sur une crate qui n'avait
jamais été compilée. Mémoire existante :
`reference_rtk_wraps_cargo_hides_clippy_lints`. **Pour toute vérification qui
sert de preuve, utiliser `"$(rustup which cargo)"`.**

### 2.4 Le récap de test doit dire par où entrer

`FORGIA_BOOT_MODE` accepte `arena_test`, `roguelite`, `castle_hub`, `fps`,
`rpg` — **pas `expedition`**. Une zone livrée sans porte d'entrée directe se
teste au clic, donc mal, donc rarement. **Une zone nouvelle naît avec son entrée
dans `boot_to_menu`**, au même titre qu'avec son capteur.

---

## 3. Propreté du code — l'état chiffré

| | Avant | Après | Reste |
|---|---:|---:|---:|
| Prédicats multi-mode nommant l'Expédition | 6/12 | **10/12** | 2 (Rpg/CastleHub, sans objet) |
| Crates partagées dépendant d'une zone | 4 | **3** | `ui-lib → mode-fps-arena` (`WaveState`) |
| Modules de zone importés du dehors | 22 | 22 | dont **18 depuis `mode-roguelite`** |
| Plugins jamais montés | 11 | 11 | — |
| Génomes morts | 61/141 | 61/141 | — |
| Types publics en collision | 15 | 15 | — |
| `parse_toml` réimplémenté | 8 crates | 8 crates | — |

**Ce qui a bougé** est ce qui bloquait le jeu. **Ce qui n'a pas bougé** est
maintenant *mesuré et figé* par une ligne de base qui ne peut que rétrécir.

---

## 4. Ce qu'il faut faire, par ordre de rendement

### Immédiat — coûte presque rien, ouvre une porte

1. **`Faction` dans `assemblage::assembler`** — 1 ligne. Fait passer le concept
   de **0 à 1 consommateur**. C'est une des quatre portes du GDD §10, et il
   existe pour la première fois un point de passage unique où tout ennemi est
   monté. Gratuit aujourd'hui, refonte transverse plus tard.
2. **`expedition` dans `FORGIA_BOOT_MODE`** — 1 ligne, rend la zone testable
   sans souris.
3. **C5 dans `strates.py`** : type public déclaré dans ≥ 2 crates, ligne de base
   à 15. Fige la dette, interdit la 16ᵉ.

### Court terme — supprime la prochaine famille de doublons

4. **Sortir le bloc profil** de la zone `mode-roguelite` vers une brique :
   `equipment`, `avatar`, `cosmetics`, `identity`, `progress`, `meta_shop`,
   `chapters`. **18 modules importés du dehors**, dont le menu (18 à lui seul),
   le Hall et l'Expédition. C'est le plus gros gisement de doublons à venir —
   il mordra dès la première demande « change l'équipement en Expédition ».
5. **`load_genome` dans le socle** — solde 8 réimplémentations de
   `parse_toml` + `load_or_default`.
6. **Registre de cartes** (`assets/genomes/expedition_cartes.toml`) — trois
   constantes empêchent une 2ᵉ carte d'Expédition, alors que **tout le contenu
   est déjà data-driven** (prouvé : `campements.rs` ne connaît pas le Vallon).

### Quand le dépôt est commité — mécanique, gros diff

7. **Renommer `roguelite` → `arène`.** « Roguelite » est un genre, pas un lieu ;
   les zones sont Expédition / Arène / 5v5 / Menu / Hall. Coût : **167 fichiers,
   1 365 occurrences**, 36 génomes, 3 capteurs, 42 docs. À faire en une passe
   isolée, revertable.

### De fond

8. **Mettre à jour `concept-first-table-forgia.md`** — elle ignore
   `forgia-enemy-archetypes`, `capacites`, les campements. Une table périmée est
   pire qu'absente : elle envoie chercher au mauvais endroit.
9. **Les 20 capteurs figés et les 32 sans sévérité** — le type `Constat` livré
   par l'autre terminal rend le second cas impossible à écrire ; reste à
   convertir l'existant.

---

## 5. Ce que cet audit ne couvre pas

La **qualité de jeu** : rien ici ne dit si l'Expédition est amusante, si les
campements sont bien dosés, si le rythme tient. Ces jugements se rendent manette
en main et **rien n'a été validé en jeu aujourd'hui**.

Ni la **performance** : 27 ennemis potentiels sur un vallon de 359 m, avec des
bots sans navmesh, n'a jamais tourné.

---

## 6. Cross-refs

`controle-de-la-sortie.md` (déclarer sa portée, mesurer la sortie) ·
`multi-terminal-coordination.md` (§1 à compléter : lire les fichiers **neufs**) ·
`concept-first.md` (étape 0 data/code — sa couche definition ment à 43 %) ·
`fine-grained-crates.md` (2 consommateurs réels avant d'extraire) ·
`outillage.md` (grepai : vérifier la fraîcheur **dans** la requête) ·
[`audit-2026-08-18-strates-the-spared.md`](audit-2026-08-18-strates-the-spared.md).
