# Contributing to Forgia V2

> Bienvenue. Si tu lis ce doc en venant de l'école Technofutur, c'est que tu vas bosser sur Forgia. Voici tout ce qu'il te faut pour démarrer en moins de 1h.

## Setup day-1

### Prérequis

- **Rust 1.85+** : `rustup default stable`
- **Git** : pour cloner et collaborer
- **Disque** : ~5 GB libres (assets + build cache)
- **OS** : Windows 10+ recommandé (cible production), macOS/Linux supporté pour dev

### Premier build

```bash
git clone <repo-url> forgia-v2
cd forgia-v2
cargo check --workspace
```

Si tout est vert : `cargo run` (le binaire canonique est le package racine `forgia` —
`forgia-game` est une lib). Une fenêtre 1920×1080 doit s'ouvrir.
Pour l'itération quotidienne : `cargo run --profile release-fast`.

Si ça plante : ouvre une issue avec ton OS + version Rust + le log complet.

## Conventions de code

### Anglais pour le code, français pour la doc

```rust
// FR : on documente en français pour aider les francophones
// EN: code identifiers in English, multi-language collaboration
fn spawn_player() {
    // ...
}
```

### 0 warning clippy mergeable

CI bloque `clippy -D warnings`. Pour vérifier en local :

```bash
cargo clippy --workspace -- -D warnings
```

Si un warning ne peut PAS être corrigé (raison documentée), `#[allow(...)]` avec commentaire `// SAFETY: <raison>` ou `// INVARIANT: <raison>`.

**Interdit** : `#[allow(dead_code)]` au niveau module/crate. Si du code est mort, supprime-le.

### Public API explicite via `prelude`

Chaque crate expose ses types publics dans un module `prelude` :

```rust
// crates/forgia-foo/src/lib.rs
pub mod prelude {
    pub use crate::{Foo, ForgiaFooPlugin};
}
```

Les consommateurs font `use forgia_foo::prelude::*;`. Pas de `pub use crate::*;` sauvage.

### Plugin trait obligatoire par crate

Chaque crate lib expose **un seul** Plugin principal :

```rust
pub struct ForgiaFooPlugin;

impl Plugin for ForgiaFooPlugin {
    fn build(&self, app: &mut App) {
        // ...
    }
}
```

Si la crate est complexe → sub-plugins explicites + `is_plugin_added::<DependencyPlugin>()` defensive checks.

### Tests headless obligatoires

Chaque module qui peut être testé en headless DOIT avoir `#[cfg(test)] mod tests`. Pattern :

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ma_fonction_handles_zero_input() {
        let result = ma_fonction(0);
        assert_eq!(result, 0);
    }
}
```

La CI exécute les tests **par crate** (`cargo test -p <crate>`) et bloque le merge en
cas d'échec (story-592 : `--workspace` est instable en local — builds concurrents /
artefacts incrémentaux ; chaque crate passe isolément). En local : `cargo test -p <crate-touchée>`.

### GameSet ordering respecté

Tout système qui touche au gameplay DOIT avoir `.in_set(GameSet::X)` :

```rust
app.add_systems(
    Update,
    my_combat_system
        .in_set(GameSet::Combat)
        .run_if(in_state(GameMode::Fps)),
);
```

Lock L7. La chaîne canonique (9 étapes, Network→…→UI) vit dans
`crates/forgia-core/src/lib.rs` (module `system_set` inline).

### 0 hardcode gameplay

Toute valeur numérique gameplay vient d'un genome TOML (`assets/genomes/*.toml`,
`config/`) chargé via `forgia-genome-core` et hot-reloadable. Pas de magic number.

```rust
// ❌ INTERDIT
let damage = 25.0;

// ✅ OK — struct tuning typée, désérialisée d'un TOML hot-reloadable
// (pattern réel : voir forgia-damage::HitFeedbackTuning, forgia-enemy-nameplate::tuning)
#[derive(Deserialize, TypePath)]
pub struct WeaponTuning { pub damage: f32, /* + Default = fallback documenté */ }
// système : assets: Res<Assets<Genome<WeaponTuning>>> → tuning.damage
```

Exception : invariants physiques (`PI`, `EPSILON`) avec commentaire `// CONST: <invariant>`.

## Workflow BMAD

| Niveau | Quand | Story requise |
|---|---|---|
| **Quick** | ≤ 3 fichiers, fix direct | Non |
| **Standard** | ≤ 10 fichiers, feature nouvelle | Oui (`docs/stories/story-NNN-slug.md`) |
| **Enterprise** | 10+ fichiers ou architectural | Oui + /research → /plan → /implement → /verify |

### Workflow Quick (la majorité)

1. Lire `forgia2_health.json` + sensors pertinents (sensors first)
2. Lire le code concerné (vraie lecture, pas skim)
3. Modifier
4. `cargo check + clippy + test`
5. PR avec description claire

### Workflow Standard

1. Créer `docs/stories/story-NNN-slug.md`
2. Quick steps + checklist post-implémentation `.bmad/checklists/post-implementation.md`

## Anti-patterns à bannir

- ❌ Modifier sans avoir lu le code
- ❌ "Tant qu'on est dans ce fichier, je nettoie aussi X" (no-speculative-fix.md)
- ❌ Ignorer un warning clippy "ça compile"
- ❌ Inventer une API Bevy 0.18 qui n'existe pas (vérifier docs)
- ❌ Sur-ingénier (3 lignes similaires > abstraction prématurée)
- ❌ Pousser sans `cargo check` + `clippy`
- ❌ Modifier un Stability Lock sans autorisation explicite Antoine

## Pièges Bevy 0.18 connus (lecture obligatoire)

Voir CLAUDE.md §6 pour la liste complète :

- B0001 (Added + &mut conflict)
- PrimaryEguiContext + DespawnOnExit trap
- Hanabi shader compile lazy → pre-spawn dummy obligatoire
- 2 handlers ESC dupliqués
- Mixamo rig non interchangeable
- bevy_water `easings` OFF
- add_systems tuple > 20
- Time<Real> vs Time<Virtual>

## Owner par crate

Voir `OWNERS.md` (à créer Phase 4 quand recrutement étudiants démarre).

Phase 0-3 : Antoine owner sur tout. Pose questions sur le canal Discord/Slack du studio.

## Resources externes

- [Bevy Cheat Book](https://bevy-cheatbook.github.io/) — référence patterns Bevy
- [Bevy 0.18 changelog](https://github.com/bevyengine/bevy/releases) — vérifier API
- [Rapier3D docs](https://rapier.rs/docs/) — physique
- [Leafwing input manager](https://github.com/Leafwing-Studios/leafwing-input-manager) — input

---

*Mise à jour : 2026-06-10 — story-593 M1 (commandes corrigées et testées, CI per-crate,
chemin GameSet réel). Précédente : 2026-05-14 bootstrap.*
