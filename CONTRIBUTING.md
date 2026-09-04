# Contribuer à Forgia

> Tout ce qu'il faut pour un premier build et une première contribution propre.

## Setup

### Prérequis

- **Rust stable** : `rustup default stable` (rustfmt et clippy sont déclarés dans
  `rust-toolchain.toml`)
- **Git**
- **Disque** : environ 10 Go libres pour le cache de build
- **OS** : Windows 10+ est la cible de production ; macOS et Linux sont supportés pour
  le développement. Sous Linux : `libasound2-dev libudev-dev pkg-config`.

### Premier build

```bash
git clone <url-du-depot> forgia
cd forgia
cargo check --workspace
```

Si tout est vert : `cargo run --profile release-fast` lance le binaire canonique, le
package racine `forgia` (`forgia-game` est une lib d'assemblage). Sans les assets, qui ne
sont pas distribués avec le dépôt, le jeu ne se joue pas : voir le README.

Si le build plante : ouvre une issue avec ton OS, ta version de Rust et le log complet.

## Conventions de code

### Anglais pour le code, français pour la doc

```rust
// FR : on documente en français
// EN: code identifiers in English
fn spawn_player() {
    // ...
}
```

### Zéro warning clippy

La CI bloque `clippy -D warnings`. En local :

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Si un warning ne peut pas être corrigé, `#[allow(...)]` avec un commentaire
`// SAFETY: <raison>` ou `// INVARIANT: <raison>`.

**Interdit** : `#[allow(dead_code)]` au niveau module ou crate. Si du code est mort, on
le supprime.

### API publique explicite via `prelude`

Chaque crate expose ses types publics dans un module `prelude` :

```rust
// crates/forgia-foo/src/lib.rs
pub mod prelude {
    pub use crate::{Foo, ForgiaFooPlugin};
}
```

Les consommateurs font `use forgia_foo::prelude::*;`. Pas de `pub use crate::*;`.

### Un Plugin principal par crate

```rust
pub struct ForgiaFooPlugin;

impl Plugin for ForgiaFooPlugin {
    fn build(&self, app: &mut App) {
        // ...
    }
}
```

Si la crate est complexe : sous-plugins explicites et vérifications défensives
`is_plugin_added::<DependencyPlugin>()`. Tout plugin nouveau doit avoir son garde de
montage et son capteur : c'est ce que vérifie `cargo xtask plugin-gate`.

### Tests headless obligatoires

Chaque module testable sans fenêtre a son `#[cfg(test)] mod tests`. La logique testable
est extraite en fonction pure, et le système Bevy n'est qu'un wrapper autour.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ma_fonction_handles_zero_input() {
        assert_eq!(ma_fonction(0), 0);
    }
}
```

La CI exécute les tests **par crate** (`cargo test -p <crate>`) : `cargo test --workspace`
est instable en local à cause des builds concurrents. En local : `cargo test -p <crate-touchée>`.

### Ordonnancement `GameSet` respecté

Tout système qui touche au gameplay porte `.in_set(GameSet::X)` :

```rust
app.add_systems(
    Update,
    my_combat_system
        .in_set(GameSet::Combat)
        .run_if(in_state(GameMode::Fps)),
);
```

C'est le Stability Lock L7. La chaîne canonique (Network → Input → Movement → Physics →
Camera → Combat → Effects → Sensors → UI) vit dans `crates/forgia-core/src/lib.rs`.

### Zéro valeur de gameplay en dur

Toute valeur numérique de gameplay vient d'un génome TOML (`assets/genomes/`, `config/`)
chargé via `forgia-genome-core` et rechargeable à chaud. Pas de nombre magique.

```rust
// INTERDIT
let damage = 25.0;

// OK : struct de tuning typée, désérialisée d'un TOML rechargeable
#[derive(Deserialize, TypePath)]
pub struct WeaponTuning { pub damage: f32 }
// système : assets: Res<Assets<Genome<WeaponTuning>>> puis tuning.damage
```

Exception : les invariants physiques (`PI`, `EPSILON`) avec un commentaire
`// CONST: <invariant>`. Le gate `cargo xtask validate-genomes` vérifie que chaque gène
reste dans ses bornes et que chaque id est unique.

### Un capteur par feature

Aucun système significatif sans export `forgia2_<feature>.json` portant `severity` et
un `next_step` actionnable. Le nouveau capteur s'enregistre dans
`docs/observability/SENSOR_REGISTRY.md`, gardé par `cargo xtask sensor-audit`.

## Workflow

| Niveau | Quand | Story requise |
| --- | --- | --- |
| **Quick** | 3 fichiers au plus, correction directe | Non |
| **Standard** | 10 fichiers au plus, feature nouvelle | Oui (`docs/stories/story-NNN-slug.md`) |
| **Enterprise** | plus de 10 fichiers, ou architectural | Oui, avec recherche et plan écrits avant le code |

### Quick, la majorité des cas

1. Lire les capteurs pertinents (`forgia2_health.json` d'abord) si le sujet est un
   comportement runtime.
2. Lire le code concerné, vraiment.
3. Modifier.
4. `cargo check`, `cargo clippy`, `cargo test -p <crate>`.
5. PR avec une description claire. Le gabarit est dans `.github/`.

### Standard et Enterprise

1. Créer `docs/stories/story-NNN-slug.md` (convention dans `docs/stories/README.md`,
   prochain id libre affiché par `cargo xtask story-ids`).
2. Écrire des critères d'acceptation falsifiables avant le code.
3. Marquer DONE seulement quand `cargo xtask story-gate --story NNN` passe.

### Avant de pousser

Le hook `scripts/git-hooks/pre-push` lance les gates locaux. Installation :

```bash
cp scripts/git-hooks/pre-push .git/hooks/pre-push
```

## Anti-patterns à bannir

- Modifier sans avoir lu le code.
- « Tant qu'on est dans ce fichier, je nettoie aussi X. »
- Ignorer un warning clippy parce que ça compile.
- Inventer une API Bevy 0.18 qui n'existe pas : vérifier la documentation.
- Sur-ingénier : trois lignes similaires valent mieux qu'une abstraction prématurée.
- Pousser sans `cargo check` et `cargo clippy`.
- Modifier un Stability Lock sans l'avoir demandé explicitement dans la PR.

## Pièges Bevy 0.18 connus

La liste vit dans `CLAUDE.md`, section « Anti-traps ». Les plus coûteux :

- `Query<Entity, Added<T>>` et `Query<&mut T>` séparés (B0001) : fusionner.
- `PrimaryEguiContext` et `DespawnOnExit` : jamais de `MenuCamera2d` sur la caméra FPS.
- Hanabi compile ses shaders paresseusement : pré-spawner un dummy caché au Startup.
- Un `KeyCode` = un handler unique, gardé par `AppMode`.
- UI et menus sur `Time<Real>`, gameplay sur `Time<Virtual>`.
- `add_systems` : dix systèmes au plus par tuple.

## Ressources externes

- [Bevy Cheat Book](https://bevy-cheatbook.github.io/)
- [Bevy releases](https://github.com/bevyengine/bevy/releases), pour vérifier une API
- [Rapier3D](https://rapier.rs/docs/)
- [Leafwing input manager](https://github.com/Leafwing-Studios/leafwing-input-manager)

## Licence des contributions

Sauf mention contraire explicite, toute contribution soumise pour inclusion dans Forgia
est réputée l'être sous la double licence du projet (MIT ou Apache-2.0), sans condition
supplémentaire.
