# Story-580 — Collider de tronc foliage découplé du scale (anti sky-high / sous-la-map)

> (Renuméroté 578→580 : 578/579 pris par l'autre terminal — worldgen-procgen / animation-tunables.)

**Statut** : Code DONE (clippy clean + binaire compile). Runtime à confirmer (rebuild requis).
**Scale** : Quick (forgia-foliage/src/lib.rs, 1 fichier).
**Date** : 2026-06-07
**Lignée** : User toggle collider debug → cylindres orange montant jusqu'au ciel + colliders sous
la map au spawn. "Il ne doit jamais rien avoir sous la map !"

## Root cause

`forgia-foliage/lib.rs` spawn d'arbre : `Transform.scale = initial_scale = target/measured × user_scale`
(scale visuel) ET `Collider::cylinder(1.5, 0.3)` sur la MÊME entité. Le visuel scale correctement
(`measured × target/measured = target` ≈ 8m), mais le collider a une taille NATIVE fixe (3m) qui se
fait multiplier par le même facteur. Quand `measured` est petit (GLB natif minuscule, ex. 0.24m),
`target/measured` ≈ ×33 → collider ~100m de haut alors que l'arbre visible fait 8m. Et le cylindre
étant CENTRÉ sur l'origine (base de l'arbre) → s'étend autant au-dessus QUE sous le sol. D'où les
deux symptômes simultanés : cylindres jusqu'au ciel + colliders sous la map.

## Attribution (multi-terminal)

- Pas le terrain (le terrain ne contrôle pas les colliders foliage).
- Pas un WIP frais de l'autre terminal : il n'a touché que `material_override.rs` (rendu).
  `lib.rs` (placement/collider) = code committé, NON contendu → éditable sans conflit.
- `calibrate_assets` (forgia-asset-registry:406) ne fait que `tf.scale` — ne touche pas le collider.

## Fix

Collider de tronc dimensionné en taille MONDE absolue, divisé par `initial_scale` pour annuler la
mise à l'échelle de l'entité (`col_half = trunk_world_h*0.5/initial_scale`), + `Collider::compound`
avec offset `+col_half` → base posée AU SOL (jamais centré → jamais sous la map). Taille monde
≈ tronc (`target*0.85` haut, `target*0.05` rayon clampé) peu importe `measured`.

## Limite connue

Cas `measured = None` (1er spawn d'un asset jamais mesuré) : spawn à `user_scale`, puis
`calibrate_assets` re-scale l'entité sans re-dériver le collider → léger transitoire si l'asset
n'était pas mesuré. En steady-state (mesures persistées en TOML) → chemin `Some(m)`, fix complet.
Si résiduel observé : ajouter une re-dérivation post-calibrate côté foliage (système + marqueur).

## Validation

forgia-foliage clippy clean ✓. `cargo check -p forgia` (binaire complet) ✓.

## AC

- [x] Collider découplé du scale + base au sol
- [x] clippy + binaire OK
- [ ] Runtime : rebuild → collider debug → plus de cylindres au ciel, rien sous la map au spawn
