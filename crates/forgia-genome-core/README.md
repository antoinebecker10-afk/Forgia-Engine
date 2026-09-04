# forgia-genome-core

Le socle de la couche de données de Forgia : un asset Bevy générique `Genome<T>`, son
chargeur TOML `GenomeLoader<T>`, la fonction pure `parse_genome`, et le trait
`RegisterGenome` qui enregistre un type de génome en une ligne.

La spécification complète du système (les deux formes de fichier, les trois chemins de
chargement, la validation `cargo xtask validate-genomes`, les limites connues et la
marche à suivre pour le réutiliser dans un autre projet Bevy) est dans
[`GENOME.md`](../../GENOME.md) à la racine du dépôt.

## Usage

```rust
use forgia_genome_core::{Genome, RegisterGenome};

#[derive(serde::Deserialize, bevy::reflect::TypePath, bevy::reflect::Reflect, Default)]
#[serde(default)]
pub struct WeaponTuning { pub damage: f32 }

app.register_genome::<WeaponTuning>();
let handle: Handle<Genome<WeaponTuning>> = asset_server.load("genomes/weapon.toml");
```

## Dépendances

`bevy`, `serde`, `toml`, `thiserror`. La dépendance `forgia-core` est déclarée dans le
manifeste mais n'est pas utilisée par le code : elle peut être retirée pour un usage
hors workspace.

## Contrats testés

Six tests unitaires (`cargo test -p forgia-genome-core`) : un TOML invalide rend `Err`
et ne panique jamais, un champ manquant prend le défaut serde, un champ obligatoire
absent ou un type faux sont refusés, le chargeur n'accepte que l'extension `toml`.

## Module `manifest`

`manifest.rs` lit les `manifest.toml` de capacité des crates (`scan_workspace`,
`ManifestIndex`, `ForgiaManifestPlugin`). Ce module n'est câblé nulle part dans le
binaire et les manifestes présents sont des gabarits : voir `GENOME.md`, section
« Le manifeste de capacité ».
