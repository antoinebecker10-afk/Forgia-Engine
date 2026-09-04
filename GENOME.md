# Le système de génome

> **Le code définit les mécanismes, le génome définit les valeurs, et chaque valeur porte
> son domaine de validité.**

Ce document spécifie la couche de données de Forgia : ce qu'elle fait, comment elle est
faite, ce qu'elle ne fait pas, et comment la reprendre dans un autre projet Bevy sans
prendre le reste du moteur. Tout ce qui y figure a été mesuré sur le dépôt le
4 septembre 2026, y compris les défauts.

Le noyau tient en un fichier : [`crates/forgia-genome-core/src/lib.rs`](crates/forgia-genome-core/src/lib.rs).
Il dépend de `bevy`, `serde`, `toml` et `thiserror`, et de rien d'autre.

---

## 1. L'idée, et la seule décision qui compte

Tout moteur finit avec une couche de réglage : fichiers de configuration, objets
scriptables, tables de données. Ici elle s'appelle le **génome**, et la métaphore
biologique porte le sens plutôt que la marque. Un génome décrit un concept de jeu (une
arme, un biome, un ennemi). Il contient des **chromosomes** (groupes cohérents) faits de
**gènes** (un paramètre chacun).

Une configuration classique écrit `damage = 14`. Un gène écrit `damage = 14, valide dans
[5, 35]` :

```toml
[[genes]]
id = "ak47_damage"
label = "Damage"
chromosome = "Ballistics"
min = 5.0
max = 35.0
default = 14.0
```

Cette borne transforme la couche de données : elle cesse d'être « des valeurs que
quelqu'un a tapées » pour devenir un **espace de mutation déclaré**.

- **Un outil peut régler le jeu sans le casser.** Ici l'outil est une IA (« rends ce
  fusil plus percutant » revient à déplacer trois gènes dans leurs bornes), mais la même
  propriété sert des curseurs d'éditeur, des balayages d'équilibrage, des tests A/B, des
  variantes procédurales. `damage = 9999` est irreprésentable par construction.
- **Le ressenti devient un espace de recherche.** Équilibrer, c'est explorer une région
  bornée, plus modifier des nombres magiques éparpillés dans le code.

Si vous ne reprenez qu'une chose de ce document : **reprenez les bornes**. Une donnée
rechargeable à chaud est un acquis banal ; c'est le `min`/`max` déclaré à côté du
`default`, paramètre par paramètre, qui rend la couche de données sûre à confier à un
outil, à un game designer ou à une IA, c'est-à-dire à tout ce qui modifie des valeurs
plus vite qu'un humain ne les relit.

---

## 2. Le noyau, en trois types

```rust
/// L'asset. T est la struct serde du consommateur.
pub struct Genome<T> { pub data: T }

/// Le chargeur : tout fichier .toml devient un Genome<T>.
pub struct GenomeLoader<T> { /* ... */ }

/// Le parse pur, testable sans Bevy.
pub fn parse_genome<T: DeserializeOwned>(src: &str) -> Result<T, toml::de::Error>;

/// L'enregistrement, une ligne par type.
app.register_genome::<WeaponTuning>();
```

Chaque crate consommatrice possède son schéma : une struct serde ordinaire, enregistrée
en une ligne. Les génomes vivent sous `assets/genomes/` et empruntent le pipeline d'assets
standard de Bevy, ce qui fait que **le rechargement à chaud vient gratuitement** : on
enregistre le TOML, le jeu en cours le reprend en une seconde, sans recompilation, sans
machine virtuelle de script, sans interface binaire.

### Les trois contrats, chacun couvert par un test

`cargo test -p forgia-genome-core` : 6 tests, verts au 2026-09-04.

| Contrat | Ce qu'il garantit |
| --- | --- |
| TOML invalide donne `Err`, jamais de panique | Une faute de frappe dans une donnée ne peut pas tuer le jeu. Le consommateur garde son `Default`. |
| Champ absent donne le défaut serde | Un vieux fichier reste compatible avec un schéma plus récent. |
| Champ obligatoire absent, ou type faux, donne `Err` | Pas de valeur fantôme silencieuse. |

Le troisième contrat mérite une précision : il ne vaut **que** si la struct n'est pas
annotée `#[serde(default)]`. Avec cette annotation, tout champ manquant prend son défaut,
ce qui est le comportement recherché pour la compatibilité ascendante mais fait perdre la
détection de faute de frappe sur un nom de clé.

---

## 3. Les deux formes de fichier, et pourquoi il y en a deux

Le dépôt contient **167 fichiers de génome** : 148 sous `assets/genomes/` (répartis en
17 sous-dossiers) et 19 sous `config/`. Ils se répartissent en deux formes qui ne se
chargent pas de la même façon.

### Forme A : la struct typée (94 fichiers)

Le TOML est le miroir direct d'une struct Rust. C'est la forme la plus simple et la plus
sûre : serde fait tout le travail, le typage est vérifié à la compilation.

```toml
# assets/genomes/player_movement.toml
speed = 6.5
sprint_multiplier = 1.5
jump_velocity = 6.5
gravity = 18.0
```

```rust
#[derive(Deserialize, TypePath, Reflect)]
#[serde(default)]
pub struct PlayerMovementTuning {
    pub speed: f32,
    pub sprint_multiplier: f32,
    pub jump_velocity: f32,
    pub gravity: f32,
}
```

La convention appliquée dans tout le dépôt : **le `Default` Rust est le miroir exact du
TOML**. Si le fichier disparaît ou devient invalide, le ressenti du jeu ne change pas.
C'est ce qui rend l'absence de données non fatale.

### Forme B : le registre de gènes bornés (73 fichiers, 1 883 gènes)

Le TOML est un tableau de gènes, chacun avec son identifiant, son domaine et sa valeur.
C'est la forme qui porte les bornes, donc celle qui intéresse un outil de réglage.

```toml
id = "weapon_ak47"
name = "AK-47"
domain = "Weapon"

[[genes]]
id = "ak47_damage"
label = "Damage"
chromosome = "Ballistics"
target = "Value"
min = 5.0
max = 35.0
default = 14.0
layer = "ak47"
```

Sur les 1 883 gènes, **1 799 portent à la fois `min` et `max`** ; les 84 restants sont
des gènes non numériques ou des entrées incomplètes que le gate laisse passer faute de
bornes déclarées.

Le champ lu au runtime est **`default`**, qui joue donc le rôle de valeur courante
(80 sites de lecture `gene.default` dans le code). Les champs `min`, `max`, `label`,
`chromosome`, `target` et `layer` sont des métadonnées destinées à un outil de réglage,
et le gate les vérifie.

Répartition mesurée :

| Dossier | Forme B (gènes) | Forme A (typée) |
| --- | --- | --- |
| `assets/genomes/` (racine) | 12 | 42 |
| `assets/genomes/biomes/` | 18 | 1 |
| `assets/genomes/roguelite/` | 10 | 23 |
| `assets/genomes/weapons/` | 8 | 0 |
| `assets/genomes/map_gen/` | 6 | 0 |
| `assets/genomes/ai/`, `enemies/` | 8 | 0 |
| autres sous-dossiers | 11 | 9 |
| `config/**` | 0 | 19 |
| **Total** | **73** | **94** |

---

## 4. Les trois chemins de chargement

C'est le point où la description honnête diverge de la description idéale. Le dépôt
charge ses génomes de trois façons, qui n'ont pas les mêmes propriétés.

| Chemin | Mécanisme | Rechargement à chaud | Chemins distincts mesurés |
| --- | --- | --- | --- |
| **1. Asset Bevy** | `asset_server.load("genomes/x.toml")` vers `Genome<T>` | **Oui**, par le `file_watcher` de Bevy (activé hors wasm dans `forgia-game`) | **14** |
| **2. Lecture directe** | `std::fs::read_to_string("assets/genomes/x.toml")` au démarrage | Non par défaut. Deux modules (`weapon_vfx`, `death_ascension`) ajoutent leur propre veille sur la date de modification | **56** |
| **3. Dossier `config/`** | `std::fs` au démarrage | Non. `config/genomes/streaming.toml` le dit explicitement dans son en-tête : relancer le jeu | **19** |

**Le chemin 1 est le bon, et c'est le moins emprunté.** Quatorze chemins passent par le
socle contre cinquante-six qui lisent le disque à la main : la majorité des génomes de ce
dépôt **ne se rechargent donc pas à chaud**, contrairement à ce que la présence du socle
laisse croire. Les chemins 2 et 3 sont de la dette, née de consommateurs qui sont des
fonctions pures appelées hors du monde ECS, ou qui ont été écrits avant le socle. Un
projet qui reprend le système ne devrait implémenter que le chemin 1.

### Sur le raccourci Shift+F12

Plusieurs commentaires du dépôt annoncent un rechargement global par `Shift+F12`. La
mesure dit autre chose : **un seul gestionnaire de touche existe**, dans
`crates/forgia-asset-registry/src/lib.rs:627`, et il ne recharge que le registre
d'assets. Le vrai rechargement à chaud des génomes est celui du `file_watcher` de Bevy,
qui ne demande aucune touche : enregistrer le fichier suffit.

---

## 5. La validation : `cargo xtask validate-genomes`

Le gate parcourt récursivement `assets/genomes/`, et pour chaque fichier :

1. **lecture** : un encodage non-UTF-8 est signalé nommément (l'espace insécable
   U+00A0 est le fautif habituel) ;
2. **parse TOML** : toute erreur de syntaxe est un échec, avec le message de `toml` ;
3. **pour chaque `[[genes]]`** :
   - `id` présent, sinon échec sur l'index du gène ;
   - `id` unique **dans le fichier** ;
   - `min`, `max` et `default` finis (ni `NaN`, ni infini) ;
   - `min <= max` si les deux sont présents ;
   - `min <= default <= max` si les trois sont présents ;
4. **une référence croisée** : dans `roguelite_elements.toml`, tout élément associé à une
   arme dans `[mapping]` doit avoir sa table `[matchup.<élément>]`, sans quoi le calcul
   retombe en silence sur le défaut du code.

Verdict au 2026-09-04 : **148 fichiers parsés, 1 883 gènes validés, vert**.

Chaque contrôle est indépendant, pour qu'une inversion `min > max` ne soit pas avalée
quand `default` est absent. Le tri des chemins est normalisé en `/` afin que l'ordre du
rapport soit identique sous Windows et sous Linux.

### Ce que le gate ne couvre pas

- **Le dossier `config/` n'est pas parcouru.** Ses 19 fichiers ne sont ni parsés ni
  validés par le gate.
- **L'unicité des identifiants est locale au fichier.** Deux fichiers peuvent déclarer le
  même `id` de gène sans que rien ne le signale.
- **Une seule référence croisée est vérifiée**, et elle est écrite en dur dans le gate.
- **Le gate ne sait pas si un gène est lu par quelqu'un.** Un gène déclaré et jamais
  consommé passe au vert. C'est un mode de panne réel : une valeur qu'on croit régler et
  qui n'a aucun effet.

---

## 6. Le défaut de conception à connaître avant de reprendre le système

**Les bornes sont validées au gate, pas appliquées au runtime.**

Le consommateur lit `gene.default` et l'utilise tel quel. Rien, à l'exécution, ne
garantit que la valeur est restée dans `[min, max]` : c'est le gate, donc une étape de
développement, qui le garantit. Sur un dépôt où toute donnée passe par la CI, cela
suffit ; sur un jeu qui accepterait des génomes fournis par un joueur ou téléchargés,
cela ne suffit pas.

Pire, certains consommateurs compensent en **réécrivant les bornes dans le code Rust** :

```rust
// crates/forgia-juice-lib/src/knockback.rs
"knockback_base_m" => t.base_m = v.clamp(0.0, 2.0),
"knockback_kill_mult" => t.kill_mult = v.clamp(1.0, 8.0),
```

Ces `clamp` sont une **seconde source pour la même grandeur**. Le jour où le TOML élargit
une borne, le code la rogne en silence, et personne ne sait plus laquelle fait foi. Si
vous reprenez ce système, la correction est claire : faire porter le `clamp` par le
chargeur, à partir des `min`/`max` du fichier, et nulle part ailleurs.

**Autre dette de la forme B : il n'existe pas de lecteur générique.** Huit crates
réimplémentent chacune leur `struct GeneToml { id, default }` et leur `match` sur les
identifiants, soit une neuvième copie du même code à chaque nouveau consommateur. Un
lecteur unique rendant une `HashMap<String, f32>` bornée supprimerait la duplication.

---

## 7. Reprendre le système dans un autre projet Bevy

La crate est autonome. Marche à suivre :

1. **Copier** `crates/forgia-genome-core/`. Retirer la dépendance `forgia-core` du
   `Cargo.toml` : elle est déclarée mais aucun code ne l'utilise.
2. **Activer le `file_watcher` de Bevy** hors wasm, sinon le rechargement à chaud
   n'existe pas :

   ```toml
   [target.'cfg(not(target_arch = "wasm32"))'.dependencies]
   bevy = { version = "0.18.1", features = ["file_watcher"] }
   ```
3. **Déclarer un schéma** dans la crate qui consomme, avec son `Default` en miroir du
   TOML :

   ```rust
   #[derive(Deserialize, TypePath, Reflect, Clone)]
   #[serde(default)]
   pub struct WeaponTuning { pub damage: f32 }

   impl Default for WeaponTuning {
       fn default() -> Self { Self { damage: 14.0 } }
   }
   ```

4. **Enregistrer, charger, synchroniser.** Le motif complet, tel qu'il est écrit dans
   `crates/forgia-damage/src/lib.rs`, tient en trois systèmes :

   ```rust
   app.register_genome::<WeaponTuning>()
      .init_resource::<Weapon>()            // la Resource lue par le gameplay
      .add_systems(Startup, load_weapon)    // garde le Handle
      .add_systems(Update, sync_weapon);    // recopie l'asset dans la Resource

   fn sync_weapon(
       handle: Option<Res<WeaponHandle>>,
       assets: Res<Assets<Genome<WeaponTuning>>>,
       mut out: ResMut<Weapon>,
   ) {
       let Some(g) = handle.as_deref().and_then(|h| assets.get(&h.0)) else { return };
       out.0 = g.data.clone();
   }
   ```
   La `Resource` existe **toujours** (son `Default` au démarrage, écrasée au
   rechargement) : le gameplay n'a donc jamais à gérer l'absence de données.
5. **Reprendre le gate.** La fonction `validate_genomes` de `xtask/src/main.rs` ne dépend
   que de `toml` et de `std::fs` : elle se copie telle quelle dans n'importe quel
   `xtask`, et c'est elle qui donne au système sa valeur.

### Portabilité hors Bevy

Le seul point d'attache à Bevy est `AssetLoader`, c'est-à-dire environ trente lignes. La
fonction `parse_genome` et le gate sont du Rust ordinaire. Le format de fichier, lui, ne
dépend de rien : c'est du TOML, lisible par n'importe quel langage. La forme B est
d'ailleurs le meilleur candidat au portage, puisque son schéma est fixe et
auto-descriptif.

---

## 8. Le manifeste de capacité, et son statut réel

Le module `crates/forgia-genome-core/src/manifest.rs` implémente une seconde idée :
chaque crate publie un `manifest.toml` déclarant ce qu'elle sait faire (catégorie,
intention, gènes exposés, messages émis et consommés, capteur, shader, dépendances), afin
qu'un agent puisse composer un jeu en choisissant des crates.

**Ce mécanisme n'est pas en service**, et le document le dit plutôt que de le laisser
croire :

- 20 crates sur 68 portent un `manifest.toml` ;
- sur ces 20, **17 sont au statut `stub`**, 1 `wip`, 2 `ready` ;
- `ForgiaManifestPlugin` n'est ajouté par aucune application du dépôt.

Le code de lecture fonctionne et il est propre. Ce qui manque, c'est le remplissage des
manifestes et un consommateur. À reprendre comme une intention documentée, pas comme une
fonctionnalité livrée.

---

## 9. Ce que le système remplace, et ce qu'il ne remplace pas

**Il remplace** une couche de script pour le réglage. Beaucoup de moteurs embarquent Lua
essentiellement pour que les designers puissent modifier des nombres pendant l'exécution.
Une donnée bornée et rechargeable couvre ce besoin avec zéro machine virtuelle, zéro
liaison à maintenir, zéro surface d'interface binaire.

**Il ne remplace pas** un langage de script pour de la logique : un génome décrit des
valeurs, pas des comportements. La question de routage à se poser avant chaque
modification est toujours la même : *est-ce une donnée ou du code ?* Un mécanisme nouveau
est du code, compilé et vérifié par le typage. Une valeur nouvelle d'un mécanisme
existant est un gène, rechargé à chaud, borné et validé.

**Conséquence pratique** : l'itération se sépare proprement en deux vitesses. Le ressenti
et l'équilibrage itèrent en secondes ; seuls les changements de structure paient le coût
de compilation. Et l'identité d'un contenu devient un petit fichier texte, relisible en
revue de code, versionnable, et générable par un outil.

---

*Forgia est sous licence MIT. Le noyau tient dans un fichier, prenez-le.*
