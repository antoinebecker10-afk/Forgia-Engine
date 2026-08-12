# Forgia

> **Un moteur de jeu 3D conçu pour être piloté par une IA — et le jeu qui le prouve.**

Rust · [Bevy 0.18.1](https://bevyengine.org) · 66 crates · 157 000 lignes · 1 924 tests

---

## À propos

**Forgia est un moteur de jeu IA-natif.** Le pari : le créateur apporte son idée et ses
assets, et une IA construit le jeu 3D. Ce n'est pas du no-code — c'est de l'**IA-code** :
du langage naturel et des assets, en entrée d'un codebase spécifiquement conçu pour qu'un
agent y travaille sans se tromper.

Ce dépôt contient deux choses indissociables :

1. **Le moteur** — la couche technique et, surtout, les *garde-fous* qui rendent un
   codebase de 157 000 lignes pilotable par un agent : observabilité systématique,
   données externalisées, et des ratchets mécaniques qui refusent la dérive.
2. **[Forgia: The Spared](docs/design/gdd-forgia-the-spared.md)** — le premier jeu
   construit dessus. Il n'est pas une démo : c'est le client interne qui prouve que le
   moteur tient. Chaque manque du moteur se découvre en essayant de shipper ce jeu.

**Pourquoi les deux ensemble ?** Un moteur « pilotable par IA » qui n'a jamais servi à
finir un vrai jeu est une hypothèse, pas un produit. Le jeu est la falsification du moteur.

---

## Le moteur — ce qui le rend pilotable par une IA

Un agent qui travaille sur un gros codebase échoue toujours de la même façon : il ne voit
pas l'état réel du programme, il devine des valeurs, et il déclare « fait » ce qui ne
l'est pas. Forgia répond aux trois par de la mécanique, jamais par de la discipline.

### 1. Observabilité — l'agent lit l'état du jeu sans le lancer

**97 capteurs** écrivent chacun un `forgia2_<feature>.json` à 1 Hz, au format
`{id, severity, next_step, …}`. Diagnostiquer une régression, c'est lire des fichiers.

```json
{ "id": "memory", "severity": "ok", "ram_mb": 2712.3, "next_step": "" }
```

Le `next_step` n'est pas décoratif : un capteur qui alerte doit dire **quoi faire**.
Et un capteur dont tous les compteurs sont à zéro n'a pas le droit de rapporter `ok` — un
système inerte ne lève aucune erreur, c'est précisément ce qui le rend invisible.

### 2. Données externalisées — l'IA règle le jeu sans toucher au code

**159 fichiers de génome TOML** (136 dans `assets/genomes/`, 23 dans `config/`), soit
**1 883 gènes** bornés et hot-reloadables en jeu. Aucune valeur de gameplay n'est écrite
en dur : dégâts, vitesses, seuils, courbes, tables de loot vivent en couche *definition*.

Corollaire pour l'agent : la plupart des demandes d'équilibrage ne sont **pas** des
modifications de code.

### 3. Ratchets — la dérive est refusée, pas surveillée

**16 gates** exécutables (`cargo xtask <gate>`), dont certains en pre-push :

| Gate | Ce qu'il refuse |
| --- | --- |
| `arch-drift` | Un `ARCHITECTURE.md` qui ne liste pas exactement les crates réelles |
| `validate-genomes` | Un gène hors bornes, un id dupliqué, une référence croisée morte |
| `story-gate` | Un « DONE » auto-déclaré dont le code n'existe pas |
| `no-scaffold` | Le retour des crates vides |
| `sensor-audit` · `verify-sensors-format` | Un capteur non enregistré ou mal formé |
| `check-orphans` | Un plugin que personne ne branche |
| `wip-check` · `context-budget` | Le travail en cours qui déborde, le contexte qui explose |

### 4. Traçabilité — chaque chantier a ses critères falsifiables

Tout travail non trivial a une story dans [`docs/stories/`](docs/stories/) avec des
critères qu'on peut **réfuter**, pas seulement cocher. `story-gate` vérifie
mécaniquement qu'un « DONE » correspond à du code réellement livré.

### Stack

| Domaine | Techno |
| --- | --- |
| Moteur / ECS | Bevy 0.18.1 (WebGPU) |
| Physique | bevy_rapier3d 0.33 |
| UI | bevy_egui 0.39 |
| VFX | bevy_hanabi 0.18 (GPU) |
| Audio | bevy_kira_audio 0.25 |
| Entrées | leafwing-input-manager 0.20 (AZERTY natif) |
| Terrain | noise · fast-surface-nets (SDF) · voronoice (biomes) |
| Navigation | vleue_navigator 0.15 (navmesh polyanya) |
| Cible | Windows 10+ · portage wasm/WebGPU validé en local |

---

## Le jeu — *Forgia: The Spared*

> *Deux amis explorent des mondes pour nourrir leur forge, et plongent dans l'Abîme pour
> mériter le monde suivant.*

Un **looter-FPS d'exploration en duo**, sur un cœur roguelite. Vous jouez avec un
**compagnon** — d'abord un bot, plus tard un ami. Public visé : les joueurs de coop FPS
roguelite (*Gunfire Reborn*, *Roboquest*) croisés avec les farmers de long terme
(*Warframe*, *Destiny*).

**Le titre nomme votre camp.** Les Épargnés, c'est tout ce qui a échappé à l'Oubli : vous,
vos armes, le Hall, et les créatures rares que vous recueillez.

### Deux modes qui se nourrissent

**Les Expéditions** — explorer des mondes procéduraux, remplir des objectifs qui ne sont
pas des massacres (atteindre, trouver, purger, rapporter), affronter les gardiens de
palier. Et à chaque palier, **la décision** : extraire maintenant, ou pousser plus loin
pendant que l'Oubli avance ?

**L'Abîme** — le creuset primordial sous la Forge, où les premiers mondes ont été fondus.
Une descente infinie où l'on ne gagne rien : **on s'y trempe**. Seule récompense, l'XP
d'arme — et elle survit à la mort.

Entre les deux, **la Forge** : elle transforme les matériaux rapportés en armes plus
puissantes. Chaque mode a son économie propre, et aucun n'est esquivable.

### L'Oubli, et pourquoi le monde est votre interface

L'antagoniste n'est pas une armée, c'est une **corrosion de la mémoire**. Il se propage
comme la rouille et progresse en quatre stades lisibles à l'œil nu :

| Stade | Ce que vous voyez |
| --- | --- |
| **Terni** | Les couleurs se voilent, les sons s'assourdissent |
| **Pâli** | Les matières deviennent translucides, les contours tremblent — les Oubliés rôdent |
| **Vitrifié** | Êtres et arbres figés en verre |
| **Effacé** | Le vide |

Le gradient d'Oubli **est** la carte du danger : aucune interface n'est nécessaire pour
lire où il fait mauvais. Et purger un foyer fait **remonter la couleur le long des
veines** — la récompense visuelle du jeu.

Les ennemis, les **Oubliés**, sont pâles et à moitié effacés. Les abattre est une
délivrance.

### Le coop se joue *ensemble*, pas côte à côte

C'est la promesse qui distingue Forgia du genre. Chaque acteur ne porte **qu'un** élément
à la fois, et les grosses réactions en exigent **deux sur la même cible** : *l'un applique,
l'autre détone*. En solo, c'est votre compagnon qui pose le second élément.

Ce n'est pas une intention de design : c'est mesuré. Un ennemi standard meurt en 0,18 s —
trop vite pour poser deux éléments seul. **L'interdépendance n'est pas un bonus, c'est la
condition d'accès à la mécanique.**

### Quatre armes, et elles parlent

Les armes sont des âmes de maîtres-forgerons versées dans leurs œuvres. Leur cœur de
braise rougeoie quand elles prennent la parole.

| Arme | Identité |
| --- | --- |
| **Pépin** | Fusil — la confiance, avec sa jauge |
| **Bourrasque** | SMG — le chaos joyeux |
| **Madame Lenoir** | Précision et patience |
| **Boucherie** | Le chaos physique |

### Trois chasses, trois horizons

- **L'équipement** — la puissance. Elle vient d'*où* la pièce est tombée, la rareté n'est
  qu'un bonus borné : un légendaire d'un palier vaut moins qu'un commun deux paliers plus bas.
- **Les reliques** — la beauté. **Zéro statistique**, jamais. Elles s'exposent au Hall et
  racontent vos voyages.
- **Les ultra-rares** — l'obsession. Des Épargnés qui se **chassent** : traces, cris,
  silhouettes semées par la génération. Ce sont des variantes, jamais de la puissance brute.
  Un Épargné recueilli ne « drope » pas : il **se sauve**, et rejoint le Hall.

### Et plus tard — l'Arène 5v5

> ⚠️ **Décidé, pas commencé.** Zéro ligne de code en v1. Ce qui suit est une direction
> assumée, pas une promesse de contenu.

Un **MOBA en vue FPS** — trois lignes, jungle, sbires, tours, boutique en cours de partie
— débloqué à un niveau joueur donné, et bâti sur le contenu déjà payé par les deux modes
principaux : mêmes armes, mêmes éléments, mêmes ultimes, même économie.

**Son principe fondateur : dix *slots*, pas dix humains.** Tout slot vide est tenu par un
compagnon, via exactement l'interface décrite plus haut. Un humain contre neuf bots, trois
contre trois plus quatre bots, ou dix joueurs : toutes les combinaisons sont jouables. Sans
ça, un mode 5v5 meurt en file d'attente avant même d'être jugé sur son design.

Trois adaptations non triviales du modèle MOBA à la vue FPS :

- **Le last-hit** est impossible au réticule. L'or **jaillit** du sbire mourant, à récupérer
  — ou à refuser à l'adversaire — en tirant dessus dans une fenêtre courte.
- **Le brouillard de guerre** est redondant quand on ne voit déjà que devant soi. Ce qui
  survit : une minimap limitée à la vision de l'équipe, et des balises déployables.
- **Les lignes gagnent la verticalité** que le genre vu de dessus ne peut pas exploiter.

Et l'atout qui n'appartient qu'à nous : **les réactions élémentaires deviennent une méta de
draft**. « L'un applique, l'autre détone » à deux, c'est de l'entraide ; à cinq, c'est une
composition d'équipe. Aucun MOBA-FPS n'a cet axe — et il est déjà à moitié construit.

**Le GDD complet** : [`docs/design/gdd-forgia-the-spared.md`](docs/design/gdd-forgia-the-spared.md)

---

## État du projet

Le cœur combat FPS est jouable : vagues, boss, boons, éléments, méta-progression
persistée, hub de menu. Le terrain procédural (biomes Voronoï, streaming, SDF) existe et
tourne, mais n'est pas encore branché comme mode d'expédition.

La structure décrite ci-dessus a été gravée en août 2026 ; le chemin pour y aller — phases,
dépendances et **jalons falsifiables** — vit dans
[`docs/REFONTE_GDD.md`](docs/REFONTE_GDD.md). Le pilotage courant est dans
[`docs/ROADMAP.md`](docs/ROADMAP.md), source unique en Now/Next/Later.

> **Sur l'honnêteté des chiffres.** Ce README ne porte que des valeurs mesurées le jour où
> elles sont écrites. Un tableau de jalons datés a été retiré en août 2026 : ses échéances
> étaient passées sans que rien n'indique si elles avaient été tenues, ce qui est pire que
> pas de jalons du tout. Il reviendra quand ses dates seront tenables.

**Pas encore de licence** — le dépôt est privé et aucun fichier `LICENSE` n'a été choisi.
À trancher avant toute publication.

---

## Quickstart

```bash
# Setup (Rust stable, Windows 10+ cible production)
rustup default stable

# Vérifier
cargo check --workspace
cargo clippy --workspace --no-deps
cargo test -p forgia-mode-roguelite        # tests par crate (voir note)

# Lancer le jeu (binaire canonique = `forgia`, package racine)
cargo forgia-dev                            # debug + Tracy (commande de dev canonique)
cargo forgia-fast                           # release-fast + Tracy
cargo forgia-memory                         # capture d'allocations (pas une mesure FPS)
.\run_debug.ps1                             # release-fast + Tracy + log forgia2_run.log

# Build joueur / distribution, sans instrumentation :
cargo run --profile release-fast
```

> Les builds lourds sont volontairement limités à deux jobs dans `.cargo/config.toml` :
> Bevy + wgpu + Tracy peuvent sinon épuiser la mémoire lors d'une compilation complète.
>
> **`cargo test --workspace`** casse en local (builds concurrents / artefacts
> incrémentaux — story-592). La CI et le dev testent **par crate** : `cargo test -p <crate>`.
> Chaque crate passe isolément.

---

## Documentation

| Document | Rôle |
| --- | --- |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Les 66 crates réelles, assemblage, GameSet, capteurs |
| [docs/design/gdd-forgia-the-spared.md](docs/design/gdd-forgia-the-spared.md) | **Le GDD maître** — le *quoi* |
| [docs/REFONTE_GDD.md](docs/REFONTE_GDD.md) | Le *chemin* — phases et jalons falsifiables |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Le *quand* — Now/Next/Later, source unique |
| [CLAUDE.md](CLAUDE.md) | Le contrat qui régit le travail des agents IA sur ce dépôt |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Setup, conventions, workflow |
| [docs/adr/](docs/adr/) | Décisions structurantes |
| [docs/vision/](docs/vision/) | La vision produit long terme |

*Les fichiers `docs/ROADMAP_*.md` sont **archivés** et ne font plus autorité.*

---

## Référence V1

Le code V1 (`D:/Forgia/`) est en mode bug-fix only. Il sert de carrière de patterns
(streaming async, breakdown VRAM, procgen de villages) re-portés à la demande — avec une
règle : **porter, c'est corriger**. On ne copie jamais, on réécrit aux normes V2 après
avoir listé les défauts connus du système d'origine.
