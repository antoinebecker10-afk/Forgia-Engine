# Forgia

Un moteur de jeu 3D **IA-natif** construit sur [Bevy 0.18](https://bevyengine.org). Le
créateur apporte son idée et ses assets, une IA construit le jeu, et le codebase est
conçu pour qu'un agent y travaille sans se tromper : observabilité systématique, valeurs
externalisées et bornées, garde-fous mécaniques qui refusent la dérive.

Rust · 68 crates · 183 825 lignes · 2 280 tests · 1 883 gènes de réglage · licence MIT

> **Alpha, publié tel quel.** Le moteur compile, se teste et se garde, mais il ne contient
> **aucun asset** : lancer le binaire n'ouvre pas un jeu. Deux gates de la CI sont rouges,
> et plusieurs sous-systèmes sont des échafaudages. Tout est mesuré et nommé dans
> **[docs/ETAT.md](docs/ETAT.md)** : lisez-le avant de bâtir dessus.

---

## Démarrer

```bash
rustup default stable
git clone https://github.com/antoinebecker10-afk/Forgia-Engine.git forgia
cd forgia
cargo check --workspace
```

Sous Linux, Bevy réclame en plus `libasound2-dev libudev-dev pkg-config`. C'est tout le
setup : le reste des outils est déjà déclaré dans `rust-toolchain.toml` et
`.cargo/config.toml`.

## Commandes

| Commande | Ce qu'elle fait |
| --- | --- |
| `cargo run --profile release-fast` | Lance le binaire `forgia` (itération quotidienne) |
| `cargo forgia-jeu` | Lance avec le pont d'inspection ECS, sans profilage |
| `cargo forgia-dev` | Lance avec Tracy **et** le pont d'inspection (viewer Tracy obligatoire) |
| `cargo forgia-lint` | Clippy dans son propre répertoire de cible, pour ne pas invalider le build |
| `cargo forgia-supply-chain` | Licences, vulnérabilités et provenance des dépendances |
| `cargo xtask` | Liste les garde-fous ; `cargo xtask <gate>` en exécute un |
| `cargo test -p <crate>` | Tests d'une crate. `--workspace` est instable en local |

## Ce qui rend ce codebase pilotable par une IA

Un agent qui travaille sur un gros codebase échoue toujours de la même façon : il ne voit
pas l'état réel du programme, il devine des valeurs, et il déclare « fait » ce qui ne
l'est pas. Forgia traite ces trois problèmes par des mécanismes automatiques, plutôt que
par des consignes que l'agent devrait suivre.

**1. Le jeu écrit son état dans des fichiers.** Chaque système important écrit un
`forgia2_<feature>.json` une fois par seconde, au format
`{id, severity, next_step, ...}`. Pour diagnostiquer une régression, on lit ces fichiers
au lieu de lancer le jeu. Le champ `next_step` dit quoi faire quand le capteur alerte. Et
un capteur dont tous les compteurs sont à zéro n'a pas le droit de rapporter `ok` : un
système qui ne tourne plus ne lève aucune erreur, c'est ce qui le rend invisible.

**2. Les valeurs de jeu vivent hors du code, avec leurs limites.** 1 883 gènes répartis
dans 167 fichiers TOML, chacun avec son `min`, son `max` et son `default`. Aucune valeur
de gameplay n'est écrite en dur dans le Rust. Comme la limite est écrite à côté de la
valeur, un outil peut régler le jeu sans pouvoir le casser : un fichier ne peut pas
demander `damage = 9999`. Le détail est dans **[GENOME.md](GENOME.md)**, et le système est
aussi publié seul dans
[antoinebecker10-afk/genome](https://github.com/antoinebecker10-afk/genome).

**3. Huit contrôles bloquent au lieu d'avertir.** Ils s'exécutent en ligne de commande et
font échouer la CI :

| Gate | Ce qu'il refuse |
| --- | --- |
| `arch-drift` | Un `ARCHITECTURE.md` qui ne liste pas exactement les crates réelles |
| `validate-genomes` | Un gène hors bornes, un id dupliqué, une référence croisée morte |
| `sensor-audit` | Un capteur non enregistré, ou déclaré et jamais produit |
| `plugin-gate` | Un plugin nouveau sans garde de montage ni capteur |
| `deps-mortes` | Une dépendance interne déclarée et jamais référencée |
| `asset-load` | Une dérive des sites de chargement d'asset au-dessus de la baseline |
| `no-scaffold` | Le retour des crates vides |
| `story-gate` | Un « DONE » auto-déclaré dont le code n'existe pas |

**4. Chaque chantier porte des critères vérifiables.** Tout travail non trivial a une
story dont les critères se testent, et `story-gate` vérifie qu'une story marquée « DONE »
correspond à du code réellement livré.

## Ce que contient le dépôt

| Dossier | Contenu |
| --- | --- |
| `crates/` | Les 68 crates : socle et données, terrain SDF et streaming, PCG par kits et sockets, auto-rig, combat et ressenti FPS, UI, observabilité, QA |
| `assets/`, `config/` | La couche de définition en texte : génomes TOML, shaders WGSL, localisation, manifestes de packs |
| `xtask/` | Les garde-fous mécaniques |
| `tools/`, `scripts/`, `docker/` | Outillage glTF et audio, contrôle du corps d'animation livré, build distribuable, build web, compilation croisée |
| `docs/` | Décisions d'architecture, explainers, méthodologie PCG, registre des capteurs |

**Ce qu'il ne contient pas** : aucun asset binaire (ni modèle, ni texture, ni son), le jeu
qui a servi de client interne et son design, et l'outillage d'agent privé de l'auteur. Les
crates de mode restent dans le workspace parce que le moteur a été construit en les
faisant tourner. Elles compilent, mais il faut fournir des assets pour les jouer.

## Stack

| Domaine | Techno |
| --- | --- |
| Moteur, ECS | Bevy 0.18.1 (WebGPU) |
| Physique | bevy_rapier3d 0.33 |
| UI | bevy_egui 0.39 |
| VFX | bevy_hanabi 0.18 (GPU) |
| Audio | bevy_kira_audio 0.25 |
| Entrées | leafwing-input-manager 0.20 (AZERTY natif) |
| Terrain | noise · fast-surface-nets (SDF) · voronoice (biomes) |
| Navigation | vleue_navigator 0.15 (navmesh polyanya) |

## Plateformes

| Cible | État |
| --- | --- |
| Windows 10+ | cible de production |
| Linux | développement et CI |
| macOS | supporté pour le développement, non éprouvé |
| Web (wasm, WebGPU) | portage validé en local, jamais publié |

## Documentation

| Document | Rôle |
| --- | --- |
| [docs/ETAT.md](docs/ETAT.md) | **L'état réel du moteur** : ce qui tient, ce qui est partiel, ce qui est cassé |
| [GENOME.md](GENOME.md) | Le système de génome, spécifié pour être repris seul |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Les crates, l'assemblage, la chaîne `GameSet`, les capteurs |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Setup, conventions, workflow |
| [CLAUDE.md](CLAUDE.md) | Le contrat qui régit le travail des agents IA sur ce dépôt |
| [docs/adr/](docs/adr/) | Décisions structurantes |
| [docs/architecture/pcg-methodology.md](docs/architecture/pcg-methodology.md) | Générer n'importe quoi depuis un plan |
| [docs/observability/SENSOR_REGISTRY.md](docs/observability/SENSOR_REGISTRY.md) | Le registre des capteurs |
| [docs/licences/README.md](docs/licences/README.md) | Licences des dépendances et provenance des données |

## Contribuer

Les conventions tiennent en cinq lignes : anglais pour le code et français pour la
documentation, zéro warning clippy, zéro valeur de gameplay en dur, un capteur par
feature, et les gates verts avant de pousser. Le détail est dans
[CONTRIBUTING.md](CONTRIBUTING.md) ; les agents IA lisent [CLAUDE.md](CLAUDE.md).

Les deux premiers chantiers utiles sont nommés dans [docs/ETAT.md](docs/ETAT.md) : rendre
le format conforme (`cargo fmt --all`) et réparer les quatre gates rouges.

## Licence

MIT ([LICENSE](LICENSE)). Toutes les dépendances sont sous licence permissive, et
`cargo deny check licenses advisories sources` bloque en local comme en CI si une
dépendance nouvelle sort de la liste autorisée. Les polices de `assets/fonts/` sont sous
SIL Open Font License 1.1. Sauf mention contraire explicite, toute contribution est
réputée soumise sous licence MIT.
