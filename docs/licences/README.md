# Licences et provenance

> Audit du 4 septembre 2026. Objet : établir que tout ce que ce dépôt distribue peut
> l'être sous licence MIT, et dire précisément ce qu'il ne distribue pas.

## 1. Le moteur

Forgia est sous **licence MIT** ([LICENSE](../../LICENSE)), titulaire Antoine Becker.
Chaque manifeste de crate déclare `license.workspace = true`, et le `Cargo.toml` racine
porte `license = "MIT"` : les 68 crates et `xtask` sont donc couverts par une source
unique.

Sauf mention contraire explicite, toute contribution soumise pour inclusion dans Forgia
est réputée l'être sous cette même licence, sans condition supplémentaire.

## 2. Les dépendances : 782 paquets, toutes permissives

`cargo deny check licenses advisories sources` rend **ok** sur les trois volets. La liste
blanche vit dans [`deny.toml`](../../deny.toml) : une dépendance nouvelle sous une licence
absente de cette liste fait échouer le contrôle, et le job CI `licences` le rejoue à
chaque poussée. Ce n'est donc pas un audit ponctuel, c'est une barrière.

Inventaire complet, un paquet par ligne : [`dependances.csv`](dependances.csv)
(nom, version, licence, dépôt). Régénération : voir §7.

| Licence déclarée | Paquets |
| --- | --- |
| MIT ou Apache-2.0 (au choix, toutes formulations) | 531 |
| MIT seul | 121 |
| Apache-2.0 seul | 30 |
| Unicode-3.0 (données ICU) | 18 |
| Apache-2.0 WITH LLVM-exception, au choix | 17 |
| Zlib, seul ou au choix | 19 |
| MPL-2.0 | 9 |
| ISC | 7 |
| BSD-2 et BSD-3-Clause, seules ou au choix | 12 |
| Unlicense ou MIT | 8 |
| autres combinaisons permissives | 10 |

### Les cinq cas qui méritent une phrase

| Paquet | Licence déclarée | Pourquoi c'est sans risque |
| --- | --- | --- |
| `self_cell` | `Apache-2.0 OR GPL-2.0-only` | Licence **au choix**. Forgia retient Apache-2.0 ; l'option GPL n'est pas exercée et ne contamine rien |
| `r-efi` | `MIT OR Apache-2.0 OR LGPL-2.1-or-later` | Même raisonnement : MIT retenu. Paquet lié à l'amorçage UEFI, jamais utilisé sur nos cibles |
| 9 paquets MPL-2.0 (`symphonia*`, `dyn-eq`, `triple_buffer`) | `MPL-2.0` | Copyleft **au fichier**, pas au projet. Lier une bibliothèque MPL depuis du code MIT est explicitement permis ; l'obligation ne porte que sur les fichiers MPL modifiés, et Forgia n'en modifie aucun |
| `epaint_default_fonts` | `(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0` | Polices embarquées dans egui. Les deux licences de police autorisent la redistribution ; elles sont dans la liste blanche |
| `webpki-roots` | `CDLA-Permissive-2.0` | Présent dans le `Cargo.lock` mais **hors du graphe construit** pour nos cibles : `cargo deny` ne le rencontre pas. Licence permissive de toute façon |

Aucune dépendance sous GPL, AGPL, SSPL, BUSL ou licence non commerciale n'est
effectivement liée.

### Vulnérabilités : deux corrigées avant publication

L'audit `advisories` du 2026-09-04 a trouvé deux failles connues dans le `Cargo.lock`,
toutes deux dans des dépendances transitives. Les deux ont été corrigées par mise à jour
du verrou, sans changement d'API :

| Avis | Paquet | Nature | Correctif appliqué |
| --- | --- | --- | --- |
| [RUSTSEC-2026-0274](https://rustsec.org/advisories/RUSTSEC-2026-0274) | `rtrb` (via `kira`, l'audio) | double libération et usage après libération dans `ReadChunk::commit` si un `Drop` panique | 0.3.4 vers **0.3.5** |
| [RUSTSEC-2026-0257](https://rustsec.org/advisories/RUSTSEC-2026-0257) | `webbrowser` (via `bevy_egui`) | injection d'arguments par la variable `BROWSER` sous Unix | 1.2.1 vers **1.2.4** |

Deux avis restent **ignorés explicitement** dans `deny.toml`, avec leur raison :
RUSTSEC-2026-0194 et RUSTSEC-2026-0195 visent `wayland-scanner`, une macro de compilation
propre à Linux, dont l'amont fige `quick-xml` en version 0.39. La cible de production
étant Windows, l'exposition est nulle ; l'ignorance est datée et justifiée dans le
fichier, pas silencieuse.

## 3. Ce que le dépôt distribue comme données

| Contenu | Volume | Licence |
| --- | --- | --- |
| Génomes TOML, configuration, spécifications PCG | 157 fichiers `.toml` | MIT, écrits pour Forgia |
| Shaders WGSL | 67 fichiers | MIT, écrits pour Forgia (voir §4) |
| Polices | 3 fichiers `.ttf` | **SIL Open Font License 1.1**, textes dans `assets/fonts/OFL-Poppins.txt` et `OFL-LilitaOne.txt` |
| Données de niveau, manifestes, registres | 12 `.json`, 1 `.ron` | MIT, générés par l'outillage du projet |
| Gabarits de matériaux, de particules, scripts Luau | 44 fichiers | MIT, mais ce sont des échafaudages sans consommateur (voir [ETAT.md](../ETAT.md) §5) |

**Aucun asset binaire n'est distribué** : zéro fichier `.glb`, `.gltf`, `.png`, `.jpg`,
`.ogg`, `.wav`, `.ktx2`, `.fbx` ou `.blend` suivi par git. Ni modèle, ni texture, ni son,
ni carte d'environnement.

## 4. Emprunts et attributions dans le code

Recherche systématique de marqueurs d'emprunt (`adapted from`, `ported from`,
`based on`, `copyright`, URL GitHub, gist, Shadertoy) sur l'ensemble des sources Rust,
WGSL, Python, PowerShell et TOML :

- **Zéro URL de code externe** dans les sources.
- **Zéro bloc de code copié** identifié.

Deux familles de références existent, et ce sont des références, pas des copies :

| Référence | Nature |
| --- | --- |
| **Inigo Quilez** : domain warping (`forgia-terrain`), biplanar mapping (`terrain_array.wgsl`, `terrain_triplanar.wgsl`) | **Techniques** décrites dans ses articles publics, réimplémentées en WGSL et en Rust. La citation est une attribution de courtoisie de la méthode, pas la marque d'un code repris |
| **Source SDK** de Valve : `CBaseViewModel`, `weapon_script.txt` (`forgia-fps`, `forgia-viewmodel`) | **Comparaisons conceptuelles** dans les commentaires, pour situer une architecture par rapport à une référence connue. Aucun code, aucun asset, aucune donnée de Valve |

Le seul fichier tiers du dépôt d'origine, un module complémentaire Blender écrit par un
auteur externe, se trouvait dans `tools/blender/` : ce dossier n'est **pas** publié.

## 5. Ce que le dépôt référence sans le contenir

Plusieurs fichiers de données citent des chemins ou des noms d'objets appartenant à des
lots d'assets externes. Ce sont des **références textuelles** : ni géométrie, ni texture,
ni son n'accompagne ces noms.

| Fichier | Ce qu'il cite | Statut du lot cité |
| --- | --- | --- |
| `assets/genomes/asset_registry.toml`, `arena_layouts.toml` | chemins `models/kaykit/...` | KayKit, lots CC0 distribués sur itch.io |
| `assets/models/environment/expedition/*.json` | noms de maillages (`tree_oak`, `stone_tallB`, `tent_smallOpen`…) et poses | lots CC0 de type KayKit et Kenney |
| `assets/models/environment/polyhaven/manifeste.json`, `assets/textures/polyhaven/manifeste.json` | identifiants Poly Haven | Poly Haven, **CC0** |
| `assets/packs.toml` | manifeste d'installation de 9 lots CC0, avec empreinte SHA-256 | mécanisme de téléchargement, aucun contenu |

Deux fichiers de licence subsistent pour des assets **absents** du dépôt :
`assets/models/arms/src/drillimpact/LICENSE.txt` (CC0) et
`assets/textures/vfx/kenney/LICENSE-CC0-Kenney.txt` (CC0). Ils sont conservés parce
qu'ils documentent la provenance et la licence de ce que le pipeline attend à cet
emplacement ; ils ne signifient pas que ce contenu est ici.

## 6. Ce qui a été retiré avant publication

| Retiré | Motif |
| --- | --- |
| Tous les assets binaires (modèles, textures, sons, HDRI, logo) | Contenu tiers ou non destiné à la redistribution |
| 4 fichiers de données du Hall (`castle_hub_lightmaps.json`, `recettes_couleur_hall.json`, `resolutions_hall.json`, `retouches_hall.json`) | **Dérivés de la scène d'un lot commercial Unity** : ils décrivent la disposition, les zones de lightmap et les corrections de ses objets, nom par nom. Aucun n'est requis à la compilation, et leur absence est tolérée à l'exécution (le module de lightmaps journalise un avertissement et se désactive) |
| `tools/blender/`, `tools/art/`, `tools/unity/` | Outillage de production lié aux assets, et un module tiers |
| Documents de conception, stories, lore du jeu client | Hors périmètre du moteur |
| Outillage d'agent privé, hooks, configuration MCP, chemins locaux | Informations personnelles |

## 7. Refaire l'audit

```bash
# licences, vulnérabilités et provenance des dépendances (ce que fait la CI)
rustup run stable cargo deny check licenses advisories sources

# audit complet chaîne d'approvisionnement (vulnérabilités, sources, licences)
cargo forgia-supply-chain

# régénérer l'inventaire CSV
rustup run stable cargo metadata --format-version 1 --locked > metadata.json
# puis extraire nom, version, licence, dépôt des paquets dont "source" n'est pas nul

# vérifier qu'aucun binaire n'est suivi
git ls-files | grep -E '\.(glb|gltf|png|jpg|ogg|wav|mp3|ktx2|dds|exr|hdr|fbx|blend)$'
```

## 8. Limites de cet audit

Il a été conduit par lecture systématique des métadonnées de dépendances et par recherche
de motifs sur l'intégralité des fichiers texte du dépôt. Il **n'est pas un avis
juridique**. Deux points restent hors de sa portée :

- **Les licences déclarées par les paquets sont prises pour argent comptant.** Le champ
  `license` d'un `Cargo.toml` est déclaratif ; l'audit ne relit pas les fichiers de
  licence de 782 paquets.
- **Un emprunt de code non commenté est indétectable** par recherche de motifs. La
  recherche couvre ce qui s'annonce ; elle ne prouve pas l'originalité de chaque ligne.
