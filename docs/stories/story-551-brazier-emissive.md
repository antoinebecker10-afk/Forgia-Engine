# Story-551 — Brazier emissive (payoff HDR pipeline)

> **Status** : IN PROGRESS
> **Scale BMAD** : Quick (≤3 fichiers)
> **Effort estimé** : ~half-day
> **Roadmap ref** : [ROADMAP_ROGUELITE Tier 2 #4](../ROADMAP_ROGUELITE.md) (payoff direct du HDR story-549)
> **Prérequis** : story-549 (HDR + Bloom)

## Pourquoi

Sans materials emissive boostés, le HDR+Bloom de story-549 ne sert à rien — il n'y a rien qui dépasse 1.0 en luminance. Les braziers wirés en Crypts d'Anvil (commit `998098f`, `Brazier_002.glb` + `Brazier_004.glb`, 2 prefabs) sont la cible #1 : flammes orange émissives, signature volcanique cartoon canon bible.

## Acceptance Criteria

- [ ] AC1 — `BrazierEmissive` Component marker dans `forgia-effects::brazier_emissive`
- [ ] AC2 — Système `apply_brazier_emissive` détecte `Added<BrazierEmissive>` + poll descendants jusqu'à scène loaded → mutate `StandardMaterial.emissive = LinearRgba(2.5, 1.0, 0.2) × 4.0` (orange chaud HDR)
- [ ] AC3 — Tag `MaterialFaderCloned`-style pour idempotence (`BrazierEmissiveApplied` marker)
- [ ] AC4 — `forgia-stage` : si `prop.prefab.contains("Brazier")` → `commands.entity(e).insert(BrazierEmissive)`
- [ ] AC5 — `cargo check -p forgia-effects -p forgia-stage` 0 erreur
- [ ] AC6 — `cargo clippy --no-deps` 0 warning sur fichiers touchés

## Files

- `crates/forgia-effects/src/brazier_emissive.rs` NEW
- `crates/forgia-effects/src/lib.rs` (export + wire system)
- `crates/forgia-stage/src/lib.rs` (~line 1001 : tag Brazier on spawn)

## Anti-pattern

Pattern Bevy 0.18 piège (cf [`reference_bevy_018_meshmaterial_insert_remove_quirk.md`](../../../memory/reference_bevy_018_meshmaterial_insert_remove_quirk.md)) : `insert(MeshMaterial3d<T>)` ne replace pas. Ici on **mute via `Assets<StandardMaterial>::get_mut`**, donc safe — pas de pattern remove+insert nécessaire.

## Test in-game

1. **Action** : `cargo run --profile release-fast -p forgia-game` puis spawn Roguelite arena Crypts of Anvil
2. **Redémarrage requis** (changement composant + assets)
3. **Effet attendu** : braziers (coupes de feu) émettent halo orange visible (bloom flare), couleur saturée pas grisée
4. **Sensor** : visuel uniquement (compare screenshot avant/après)
5. **Variantes si KO** :
   - Halo invisible → augmenter multiplier 4.0 → 8.0
   - Couleur fade gris → vérifier `Bloom::NATURAL` actif (story-549) + `Hdr` Component auto-required
   - Pas de brazier visible → vérifier palette `melee_pit` instanciée (count > 0)

## Cross-refs

- Story-549 (HDR foundation)
- Story-538 AC2 (extension future avec hanabi flame VFX particles)
- Story-552 follow-up : champignons cyan emissive cluster
