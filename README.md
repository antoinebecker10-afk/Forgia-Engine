# Forgia

> Un moteur de jeu 3D conçu pour être piloté par une IA.

Rust · [Bevy 0.18.1](https://bevyengine.org) · 68 crates · 183 825 lignes de Rust · 2 280 tests · 167 génomes TOML

---

## À propos

**Forgia est un moteur de jeu IA-natif.** Le pari : le créateur apporte son idée et ses
assets, et une IA construit le jeu 3D. Ce n'est pas du no-code, c'est de l'**IA-code** :
du langage naturel et des assets, en entrée d'un codebase spécifiquement conçu pour qu'un
agent y travaille sans se tromper.

Un agent qui travaille sur un gros codebase échoue toujours de la même façon : il ne voit
pas l'état réel du programme, il devine des valeurs, et il déclare « fait » ce qui ne
l'est pas. Forgia répond aux trois par de la mécanique, jamais par de la discipline.

### 1. Observabilité : l'agent lit l'état du jeu sans le lancer

Chaque système significatif écrit un capteur `forgia2_<feature>.json` à 1 Hz, au format
`{id, severity, next_step, ...}`. Diagnostiquer une régression, c'est lire des fichiers.

```json
{ "id": "memory", "severity": "ok", "ram_mb": 2712.3, "next_step": "" }
```

Le `next_step` n'est pas décoratif : un capteur qui alerte doit dire **quoi faire**.
Et un capteur dont tous les compteurs sont à zéro n'a pas le droit de rapporter `ok` : un
système inerte ne lève aucune erreur, c'est précisément ce qui le rend invisible.

Le registre des capteurs vit dans [`docs/observability/SENSOR_REGISTRY.md`](docs/observability/SENSOR_REGISTRY.md)
et le gate `cargo xtask sensor-audit` refuse tout capteur non enregistré.

### 2. Données externalisées : l'IA règle le jeu sans toucher au code

167 fichiers de **génome** TOML (`assets/genomes/`, `config/`), chaque gène
borné (minimum, défaut, maximum) et rechargeable à chaud en jeu. Aucune valeur de gameplay
n'est écrite en dur : dégâts, vitesses, seuils, courbes et tables de loot vivent en couche
*definition*.

Corollaire pour l'agent : la plupart des demandes d'équilibrage ne sont **pas** des
modifications de code. Le mécanisme est expliqué dans
[`docs/explainer-genome-system.md`](docs/explainer-genome-system.md).

### 3. Ratchets : la dérive est refusée, pas surveillée

Des gates exécutables (`cargo xtask <gate>`), dont une partie tourne en CI et en pre-push :

| Gate | Ce qu'il refuse |
| --- | --- |
| `arch-drift` | Un `ARCHITECTURE.md` qui ne liste pas exactement les crates réelles |
| `validate-genomes` | Un gène hors bornes, un id dupliqué, une référence croisée morte |
| `sensor-audit` · `verify-sensors-format` | Un capteur non enregistré ou mal formé |
| `plugin-gate` | Un plugin nouveau sans garde de montage ni capteur |
| `deps-mortes` | Une dépendance interne déclarée et jamais référencée |
| `asset-load` | Une dérive des sites d'appel `asset_server.load` au-dessus de la baseline |
| `no-scaffold` | Le retour des crates vides |
| `check-orphans` | Un plugin que personne ne branche |
| `story-gate` · `story-ids` · `wip-check` | Un « DONE » sans code, un id de story dupliqué, trop de travail en cours |

`cargo xtask` sans argument liste tous les gates. Le gate `context-budget` mesure le
poids des règles d'agent chargées à chaque session et attend un dossier `.claude/rules/`
qui n'est pas publié ici.

### 4. Traçabilité : chaque chantier a ses critères falsifiables

Tout travail non trivial a une story dans [`docs/stories/`](docs/stories/) avec des
critères qu'on peut **réfuter**, pas seulement cocher. `story-gate` vérifie
mécaniquement qu'un « DONE » correspond à du code réellement livré. La convention est
décrite dans [`docs/stories/README.md`](docs/stories/README.md).

---

## Ce que contient ce dépôt

- **`crates/`** : les 68 crates du moteur. Socle et données (`forgia-core`,
  `forgia-genome-core`, `forgia-rng`), monde et génération (terrain SDF, streaming,
  biomes Voronoï, PCG, villages), animation et auto-rig (voxelisation, axe médian,
  squelette, locomotion procédurale, IK), combat et ressenti FPS, joueur et caméras,
  UI et rendu, observabilité et QA. [`ARCHITECTURE.md`](ARCHITECTURE.md) les liste
  toutes avec leur rôle et leur câblage.
- **`assets/` et `config/`** : la couche de définition en texte. Génomes TOML, shaders
  WGSL, matériaux, presets de particules, scripts Luau, localisation, manifestes de packs.
- **`xtask/`** : les gates mécaniques.
- **`tools/`**, **`scripts/`**, **`docker/`** : outillage glTF et audio, contrôle du
  corps d'animation livré, build distribuable, build web, images de compilation croisée.
- **`docs/`** : décisions d'architecture (ADR), explainers, méthodologie PCG, registre
  des capteurs, préparation de la migration Bevy 0.19.

## Ce qu'il ne contient pas

- **Aucun asset binaire.** Modèles 3D, textures, audio et HDRI ne sont pas redistribués.
  Les packs utilisés en développement sont des packs CC0 ou commerciaux ;
  `assets/packs.toml` décrit le mécanisme d'installation des packs CC0 par manifeste et
  empreinte SHA-256. Seules les polices, sous SIL Open Font License, sont incluses.
  Quatre fichiers de données décrivant la scène d'un lot commercial ont également été
  retirés : le détail est dans [docs/licences/README.md](docs/licences/README.md) §6.
- **Le jeu qui a servi de client interne.** Son design, son contenu et ses stories de
  travail ne font pas partie de cette publication. Les crates de mode
  (`forgia-mode-roguelite`, `forgia-mode-expedition`, `forgia-mode-fps-arena`) restent
  dans le workspace parce que le moteur a été conçu en les faisant vivre : elles
  compilent, mais ne se jouent pas sans assets.
- **L'outillage d'agent privé de l'auteur** : hooks, mémoire, configuration MCP.

Conséquence : le workspace compile et les tests se lancent par crate, mais lancer le
binaire `forgia` sans assets n'ouvre pas un jeu jouable.

---

## État du moteur

Le moteur est publié tel quel, avec ses défauts nommés. Le socle (génomes, capteurs,
gates, PCG, auto-rig, terrain SDF) compile et se teste ; les modes de jeu ont servi de
client interne et ne sont plus développés activement. Ce qui est vérifié, ce qui est
partiel et ce qui est cassé, mesuré le 4 septembre 2026, est dans
[docs/ETAT.md](docs/ETAT.md) : c'est le document à lire avant de bâtir dessus.

Le système de génome, réutilisable seul dans un autre projet Bevy, a son propre
document : [GENOME.md](GENOME.md).

---

## Stack

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

## Démarrer

```bash
# Rust stable (rustfmt et clippy sont déclarés dans rust-toolchain.toml)
rustup default stable

# Linux : dépendances système de Bevy
sudo apt-get install -y libasound2-dev libudev-dev pkg-config

# Vérifier
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p forgia-genome-core        # les tests se lancent par crate (voir note)

# Les gates
cargo xtask                             # liste
cargo xtask arch-drift                  # exemple

# Lancer le binaire (package racine `forgia`)
cargo run --profile release-fast
```

> **Deux jobs de compilation.** `.cargo/config.toml` limite le build à deux jobs :
> Bevy, wgpu et Tracy peuvent sinon épuiser la mémoire lors d'une compilation complète.
>
> **`cargo test --workspace` est instable en local** (builds concurrents, artefacts
> incrémentaux). La CI et le développement testent **par crate** : `cargo test -p <crate>`.
>
> Les commandes de développement (`cargo forgia-dev`, `cargo forgia-jeu`,
> `cargo forgia-lint`) sont des alias définis et commentés dans `.cargo/config.toml`.

---

## Documentation

| Document | Rôle |
| --- | --- |
| [docs/ETAT.md](docs/ETAT.md) | **L'état du moteur** : vérifié, partiel, cassé, mesuré le 2026-09-04 |
| [GENOME.md](GENOME.md) | **Le système de génome**, spécifié pour être réutilisé seul |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Les crates réelles, l'assemblage, la chaîne `GameSet`, les capteurs |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Setup, conventions, workflow |
| [docs/licences/README.md](docs/licences/README.md) | Licences des dépendances et provenance des données embarquées |
| [CLAUDE.md](CLAUDE.md) | Le contrat qui régit le travail des agents IA sur ce dépôt |
| [docs/adr/](docs/adr/) | Décisions structurantes |
| [docs/explainer-genome-system.md](docs/explainer-genome-system.md) | La couche de données, expliquée |
| [docs/explainer-extensibility-without-dynamic-plugins.md](docs/explainer-extensibility-without-dynamic-plugins.md) | Pourquoi pas de plugins dynamiques |
| [docs/architecture/pcg-methodology.md](docs/architecture/pcg-methodology.md) | Générer n'importe quoi depuis un plan |
| [docs/observability/SENSOR_REGISTRY.md](docs/observability/SENSOR_REGISTRY.md) | Le registre des capteurs |
| [docs/AI_RUNTIME_INSPECTION.md](docs/AI_RUNTIME_INSPECTION.md) | Inspecter le jeu vivant par Bevy Remote Protocol |

---

## Contribuer

Les conventions (anglais pour le code, français pour la doc, zéro warning clippy, zéro
valeur de gameplay en dur, un capteur par feature) sont dans
[CONTRIBUTING.md](CONTRIBUTING.md). Les agents IA lisent [CLAUDE.md](CLAUDE.md).

## Licence

Forgia est distribué sous licence MIT ([LICENSE](LICENSE)). Toutes les dépendances du
`Cargo.lock` sont sous licence permissive et `cargo deny check licenses` refuse toute
dérive ; l'inventaire complet, les cas particuliers et la provenance des données
embarquées sont dans [docs/licences/README.md](docs/licences/README.md).

Les polices de `assets/fonts/` sont sous SIL Open Font License 1.1 (fichiers `OFL-*.txt`
à côté d'elles). Sauf mention contraire explicite, toute contribution soumise pour
inclusion dans Forgia est réputée l'être sous licence MIT, sans condition supplémentaire.
