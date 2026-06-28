---
description: "Genome system rules — TOML configs, catalogue fallbacks, genome_sync, hot-reload"
paths: ["**/genome/**", "**/genomes/**", "**/genome_sync*", "**/catalogue*"]
---

# Genome Code Rules (Forgia)

## Ajouter un nouveau gene

1. **TOML** : ajouter dans `config/genomes/xxx.toml` avec valeur default et commentaire
2. **Catalogue fallback** : ajouter dans `forgia-engine/src/genome/catalogue.rs` (meme valeur que TOML)
3. **genome_sync.rs** : ajouter mapping via macro `genome_map!` dans `forgia-game/src/terrain/genome_sync.rs`
4. **FpsTuning** : ajouter champ dans `resources.rs` + Default impl (meme valeur)
5. **Utilisation** : lire via `tuning.field` ou `genome_read()`, JAMAIS hardcoder

## Regles absolues

- **Source de verite** : FpsTuning = gameplay runtime, Genome TOML = assets & generation procedurale
- **Hot-reload** : tout gene DOIT fonctionner avec Shift+F12 (pas de cache qui survit au reload)
- **BiomeGenomeOverrides** : pour passer des genes dans forgia-terrain (qui n'a PAS acces a GenomeRegistry)
- **Pas de dep forgia-engine dans forgia-terrain** : utiliser les structs bridge dans `forgia-game/terrain/mod.rs`
- **Validation** : chaque gene a une valeur min/max raisonnable dans le catalogue
- **Nommage** : snake_case, prefixe par systeme (`cave_`, `cloud_`, `water_`, `mat_`, `veg_`, etc.)
- **Doublons interdits** : un gene existe dans UN seul TOML, pas de copies entre fichiers

## Pattern BiomeGenomeOverrides

```rust
// Dans genome_sync.rs : construire BiomeGenomeOverrides depuis GenomeRegistry
let overrides = BiomeGenomeOverrides {
    field: genome_read!(registry, "biome_xxx", "field", default),
    // ...
};
// Passer dans TerrainPipeline via ChunkRequest
```

## Erreurs frequentes

- Oublier le fallback catalogue.rs → crash si TOML manquant
- Oublier genome_sync.rs → gene mort (TOML lu mais jamais utilise)
- Mettre un gene dans biome_xxx.toml ET dans xxx_default.toml → doublon, confusion
- Modifier un gene sans tester hot-reload → regression silencieuse
