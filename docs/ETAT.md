# État du moteur

> Mesuré le **4 septembre 2026** sur ce dépôt, avec rustc 1.96.1. Chaque chiffre a été
> obtenu par une commande, citée quand elle n'est pas évidente. Lisez ce document avant de
> bâtir sur Forgia : il donne l'état réel de chaque partie du moteur.

Forgia a été écrit en construisant un jeu avec lui. Ce jeu n'est pas publié ici, mais il
explique la forme du moteur. Les parties qui ont servi tous les jours sont testées et
solides. Celles qui n'ont jamais servi sont restées à l'état d'échafaudage. Le tri
ci-dessous montre les deux.

---

## 1. En une page

| | |
| --- | --- |
| **Ce qui compile** | `cargo check --workspace --all-targets` : **0 erreur, 0 warning**, 5 min 04 s |
| **Ce qui est testé** | **2 280** fonctions `#[test]`. Six crates de socle relancées et vertes (voir §3) |
| **Ce qui est gardé** | 4 gates verts sur 8 (voir §4) |
| **Ce qui est rouge en CI** | le job `fmt` (**227** écarts) et le job `ratchets` (**4** gates) |
| **Sûreté mémoire** | **0** bloc `unsafe` dans tout le workspace |
| **Chaîne d'approvisionnement** | `cargo deny check licenses advisories sources` : **ok** sur les trois volets, 782 dépendances |
| **Dette déclarée** | 29 `allow(dead_code)`, 84 marqueurs `TODO`/`FIXME` |
| **Plateforme** | Windows 10+ visé ; CI sur Ubuntu ; portage wasm/WebGPU validé en local seulement |
| **Réseau** | **aucun**. Pas une ligne, pas une dépendance |
| **Script runtime** | **aucun**. Ni Lua, ni Luau, ni WASM de gameplay |

**Le dépôt n'est pas un jeu qu'on lance.** Il ne contient **aucun asset binaire**
(0 fichier `.glb`, `.png`, `.ogg`, `.ktx2` suivi par git). Le binaire `forgia` compile,
mais il faut fournir vos propres assets pour obtenir une scène jouable. Le jeu n'a pas été
lancé pour rédiger ce document : tout ce qui suit est mesuré à la compilation, aux tests
et aux gates.

---

## 2. Ce sur quoi on peut bâtir

Ces crates ont servi tous les jours pendant le développement du jeu client. Elles sont
testées, câblées, et leurs contrats sont documentés dans le code.

| Domaine | Crates | Pourquoi c'est solide |
| --- | --- | --- |
| **Couche de données** | `forgia-genome-core` | 167 fichiers de génome, 1 883 gènes bornés, gate de validation vert. Spécification complète dans [GENOME.md](../GENOME.md) |
| **Observabilité** | `forgia-observability` | 140 tests verts. 141 capteurs produits, registre gardé mécaniquement |
| **Socle ECS** | `forgia-core` | 64 tests. États, chaîne `GameSet`, zéro dépendance interne |
| **Aléatoire déterministe** | `forgia-rng` | 10 tests. xoshiro256++ seedé, rejouabilité garantie |
| **Contrats PCG** | `forgia-pcg-core` | 23 tests, purs et sans Bevy. Spécifications de contenu, manifestes de kit, solveur constructif |
| **Dégâts** | `forgia-damage` | 26 tests. Santé, zones de coup, couche défensive |
| **Terrain** | `forgia-terrain` | 133 tests. SDF, surface nets, biomes Voronoï, streaming par chunks |
| **Gates** | `xtask` | Le vrai différenciateur : les 8 contrôles de §4, réutilisables tels quels |

---

## 3. Ce qui a été relancé pour ce document

```bash
rustup run stable cargo check --workspace --all-targets       # 0 erreur, 0 warning
rustup run stable cargo test -p <crate>                       # par crate, voir tableau
rustup run stable cargo deny check licenses advisories sources # ok sur les 3 volets
```

Deux vulnérabilités connues ont été corrigées avant publication, par mise à jour du
verrou : `rtrb` 0.3.5 et `webbrowser` 1.2.4. Le détail est dans
[docs/licences/README.md](licences/README.md) §2.

| Crate | Tests passés | Verdict |
| --- | --- | --- |
| `forgia-stage` | 165 | vert |
| `forgia-observability` | 140 | vert |
| `forgia-terrain` | 127 | vert |
| `forgia-core` | 64 (2 ignorés) | vert |
| `forgia-damage` | 26 | vert |
| `forgia-pcg-core` | 23 | vert |
| `forgia-rng` | 10 | vert |
| `forgia-genome-core` | 6 | vert |
| **Total relancé** | **561** | **vert** |

> **`cargo test --workspace` est instable en local** (builds concurrents, artefacts
> incrémentaux). La CI et le développement testent **par crate**. Les 2 280 fonctions
> `#[test]` du dépôt n'ont pas toutes été relancées pour ce document : huit crates l'ont
> été, pour 561 tests passés.

---

## 4. Les gates, et lesquels sont rouges

`cargo xtask` sans argument liste tout. Verdict au 2026-09-04 :

| Gate | Verdict | Ce qu'il dit |
| --- | --- | --- |
| `arch-drift` | **vert** | `ARCHITECTURE.md` liste exactement les 68 crates du workspace |
| `validate-genomes` | **vert** | 148 fichiers parsés, 1 883 gènes validés (bornes, unicité) |
| `no-scaffold` | **vert** | aucune crate vide |
| `story-ids` | **vert** | 0 story (le dossier repart vide, voir `docs/stories/README.md`) |
| `sensor-audit` | **rouge** | 1 capteur orphelin (`forgia2_castle_sol.json`, produit sans être déclaré), 2 déclarés jamais produits, 2 écrivains multiples |
| `asset-load` | **rouge** | 132 sites de chargement d'asset pour une baseline de 131 : `castle_sol_detail.rs` est entré sans passer par la liste blanche. Cible du ratchet : 30 |
| `deps-mortes` | **rouge** | `forgia-mode-roguelite` déclare `forgia-mode-fps-arena` sans jamais le référencer. 22 dépendances mortes au total, dont 21 tolérées par la baseline |
| `plugin-gate` | **rouge** | `ExpeditionCampementsPlugin` n'est monté par aucun test, **et le gate lui-même panique** : `end byte index is not a char boundary` à `xtask/src/main.rs:786`, un découpage d'octets au milieu d'un caractère UTF-8 |

**Les quatre rouges étaient déjà rouges avant la publication** : ils rendent le même
verdict sur le dépôt d'origine. L'extraction n'a rien cassé ; ce sont des dettes connues
et non payées.

**Le format n'est pas conforme non plus** : `cargo fmt --all -- --check` rend **227
écarts**. Le job `fmt` de la CI sera rouge au premier push, comme le job `ratchets`. Ce
sont les deux premières choses à corriger si vous reprenez le dépôt, et les deux plus
faciles : `cargo fmt --all` règle la première d'un coup.

---

## 5. Ce qui est de l'échafaudage, pas du moteur

Nommé pour que personne ne perde une journée à comprendre pourquoi « ça ne fait rien ».

| Ce que la structure laisse croire | Ce que la mesure dit |
| --- | --- |
| 45 shaders de post-traitement | **44 sur 45 sont des stubs passthrough.** Seul `outline.wgsl` fait quelque chose. Les shaders réels sont ailleurs : terrain (triplanar, biplanar), toon, SSGI, herbe, vent, contour, grain du sol |
| 20 fichiers `.material` | **zéro consommateur dans le code.** Format hérité, en-têtes « stub » |
| 12 fichiers `.particle` | **zéro consommateur.** Les VFX réels passent par `bevy_hanabi` en Rust |
| 12 scripts `.luau` | **zéro consommateur, et aucun runtime Lua dans le workspace.** Ce sont des fichiers de 3 lignes annonçant un portage jamais fait |
| Système de manifeste de capacité | 20 crates sur 68 en portent un, **17 au statut `stub`**, et `ForgiaManifestPlugin` n'est câblé nulle part. Voir [GENOME.md](../GENOME.md) §8 |
| Pipeline de villages (4 crates) | **débranché.** `forgia-village-loader` reste monté mais ne produit rien ; `village-generator`, `village-kit` et `procgen-graph` ne sont consommés que par lui |

### Crates sans aucun consommateur

Mesuré sur le graphe de dépendances du workspace :

| Crate | Statut réel |
| --- | --- |
| `forgia-asset-cdn` | outil en ligne de commande (`cdn`), pas une bibliothèque du jeu. Normal qu'elle soit orpheline |
| `forgia-qa-autopilot` | cadre de test (`SmokeBot`, `SoakBot`), utilisé par `cargo test`, pas par le binaire |
| `forgia-pcg-runtime` | **véritablement orpheline** : l'adaptateur Bevy du PCG n'est branché dans aucune application |

Sept crates n'ont **aucun test** : `forgia-crosshair`, `forgia-juice-screen-flash`,
`forgia-killfeed`, `forgia-postprocess`, `forgia-prefab`, `forgia-village-loader`,
`forgia-water`.

---

## 6. Les dettes d'architecture connues

| Dette | Ce que ça coûte |
| --- | --- |
| **Double `Health`** | `forgia-combat` porte la santé des ennemis, `forgia-damage` celle du joueur. Deux types pour un concept : toute logique transverse doit gérer les deux |
| **Contrôleur joueur hors `GameSet`** | La chaîne de `forgia-player` échappe à l'ordonnancement canonique (Lock L7). L'ordre relatif à la physique n'est donc pas garanti par la structure |
| **Trois chemins de chargement des génomes** | Voir [GENOME.md](../GENOME.md) §4. Un seul recharge à chaud, et c'est le moins emprunté : 14 chemins passent par le socle, 56 lisent le disque à la main. Le dossier `config/` (19 fichiers) n'est même pas validé par le gate |
| **Bornes des gènes non appliquées au runtime** | Le gate les vérifie, l'exécution ne les impose pas, et deux consommateurs les réécrivent en dur dans le Rust. Détail et correction dans [GENOME.md](../GENOME.md) §6 |
| **`register_genome` n'est appelé nulle part** | Le helper du socle exige `FromReflect`, ce dont `init_asset` et `register_asset_loader` n'ont pas besoin. La contrainte force à dériver `Reflect` sur des structs de réglage, et deux crates appellent les deux méthodes à la main pour l'éviter, en le disant en commentaire. Retirer la contrainte rend le helper utilisable. Le dépôt [genome](https://github.com/antoinebecker10-afk/genome) l'a fait |
| **`forgia-terrain` porte un portage V1 dormant** | Environ 30 % de la crate sous `allow(dead_code)` : 11 des 29 tolérances du workspace sont là |
| **Crates QA neutralisées par défaut** | `forgia-qa-core` et `forgia-qa-replay` sont montées mais inertes sans la feature `qa-runtime`. Une décision non tranchée (ADR-0004, statut `PROPOSED`) |

---

## 7. Ce qui manque franchement

Ces fonctionnalités n'existent pas dans le moteur. Les nommer évite de les chercher.

- **Le réseau.** Il n'y a rien : ni transport, ni réplication, ni prédiction. Aucune
  dépendance réseau dans le `Cargo.lock`.
- **Le script de gameplay.** Rien non plus. Le réglage passe par les génomes (ce qui
  couvre l'essentiel du besoin, voir [GENOME.md](../GENOME.md) §9), mais aucune logique
  ne peut être écrite hors de Rust.
- **Une scène de démonstration.** Sans assets, il n'y a rien à regarder. Bâtir une scène
  minimale à partir de primitives Bevy est le premier chantier utile pour qui veut
  éprouver le moteur.
- **La documentation d'API.** `cargo doc` fonctionne et les crates sont commentées en
  français, mais aucun site de documentation n'est publié.
- **Une version.** Toutes les crates sont en `0.1.0` et rien n'est publié sur crates.io.
  Les numéros de version ne veulent donc rien dire pour l'instant.

---

## 8. Par où commencer, selon ce que vous cherchez

| Vous voulez… | Commencez par |
| --- | --- |
| **La couche de données bornée** | [GENOME.md](../GENOME.md), puis `crates/forgia-genome-core/` et la fonction `validate_genomes` de `xtask` |
| **Les gates mécaniques** | `xtask/src/main.rs` : `arch-drift`, `deps-mortes`, `plugin-gate`, `sensor-audit` sont indépendants du reste et se copient |
| **L'observabilité** | `crates/forgia-observability/`, puis `docs/observability/SENSOR_REGISTRY.md` |
| **Le terrain procédural** | `crates/forgia-terrain/`, chunks 32×128×32, SDF plus surface nets, biomes Voronoï |
| **Le PCG par kits et sockets** | `docs/architecture/pcg-methodology.md`, `crates/forgia-pcg-core/`, `assets/pcg/` |
| **L'auto-rig** | `forgia-mesh-voxelizer` puis `forgia-medial-axis` puis `forgia-skeleton-embedder` puis `forgia-auto-rig` : le pipeline Pinocchio en quatre étapes |
| **Comprendre la philosophie** | `docs/explainer-extensibility-without-dynamic-plugins.md`, puis `CLAUDE.md` |

---

## 9. Comment ce document reste vrai

Les chiffres ci-dessus périment. Les commandes qui les recalculent :

```bash
rustup run stable cargo check --workspace --all-targets   # compilation et warnings
rustup run stable cargo fmt --all -- --check              # écarts de format
rustup run stable cargo deny check licenses               # licences des dépendances
cargo xtask arch-drift                                    # crates réelles vs ARCHITECTURE.md
cargo xtask validate-genomes                              # fichiers et gènes validés
cargo xtask sensor-audit                                  # capteurs orphelins et doublons
cargo xtask deps-mortes                                   # dépendances internes inutilisées
```

Si un chiffre de ce document contredit une de ces commandes, croyez la commande.
