---
paths:
  - "**/*.rs"
---

# No Hardcode Rule (Forgia) — STRICT

> **Regle bloquante** : tout nouveau code avec un literal numerique gameplay/combat/movement/viewmodel/enemy
> est REFUSE. Pas de "on migrera plus tard" — c'est maintenant ou jamais.

## Sources autorisees (par ordre de preference)

1. **GenomeRegistry** (TOML data-driven, hot-reload Shift+F12) — pour tout ce qui a un genome pack
2. **FpsTuning** (runtime, modifiable via Grimoire touche G) — pour les params gameplay sans genome
3. **config/*.json** ou **config/*.toml** — pour les registres (enemy_types, vegetation, etc.)
4. **`const` nommee** avec commentaire — UNIQUEMENT pour les invariants physiques/maths (PI, EPSILON, GRAVITY)

## Interdit

- `0.025` ou `0.5` ou `1.25` en literal dans le code gameplay → DOIT venir d'un genome gene ou FpsTuning
- `Timer::from_seconds(2.0, ...)` avec un literal → le `2.0` DOIT venir de FpsTuning ou genome
- `Color::rgb(0.5, 0.3, 0.1)` en dur → definir dans config TOML ou FpsTuning
- `"models/xxx.glb"` en dur dans la logique → centraliser dans `GameAssets`, `asset_paths.rs`, ou `scene_cache`
- Viewmodel scale/offset/rotation en dur → DOIT venir du genome pack arme correspondant
- Enemy stats (hp, damage, speed, vision) en dur → DOIT venir du genome pack ennemi
- Boss thresholds/multipliers en dur → DOIT venir du genome pack boss

## Auto-calibration obligatoire

- **Taille modele** : ne JAMAIS hardcoder un scale pour un GLB importe. Utiliser le systeme `NeedsViewmodelCalibrate` (AABB-based) ou equivalent
- **Nouveau asset** : toujours creer un genome TOML dans `config/genomes/` AVANT de coder le systeme qui le consomme
- **Nouveau parametre** : ajouter le gene dans le TOML + fallback pack dans `catalogue.rs` + sync dans `genome_sync.rs`

## Workflow nouveau parametre

```
1. Ajouter gene dans config/genomes/xxx.toml
2. Ajouter fallback dans forgia-engine/genome/catalogue.rs
3. Si FpsTuning field existe: ajouter mapping dans genome_sync.rs
4. Dans le code: lire via gene() helper avec fallback = ancienne valeur hardcodee
5. Supprimer le hardcode
```

## Exception unique

- Valeurs de layout UI (margins, padding, font sizes) autorisees en dur si purement cosmetiques et non exposees au createur
