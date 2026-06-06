# Story-576 — Terrain shape genome (data-driven, hot-reloadable)

**Statut** : Increment 1 + 2 DONE (genome-driven + Shift+F12 live regen). Validés check+clippy+tests ; runtime à confirmer.
**Scale** : Standard (forgia-terrain + forgia-rpg + config)
**Date** : 2026-06-06
**Lignée** : Session reprise terrain → user choisit "Forme & relief" → retune octaves (rolling/traversable) → user veut tuner sans rebuild.

## Increment 1 — genome foundation (DONE)

La FORME du relief (octaves Perlin, edge_falloff, max_height) vient désormais de `config/genomes/terrain_shape.toml` au lieu d'être hardcodée dans `heightmap_at`.

**Changements** :
- `forgia-terrain/src/terrain_shape.rs` (NEW) : `TerrainShapeGenome` + `load_or_default()` + `default_octaves()` (retune rolling 2026-06-06) + 4 tests.
- `forgia-terrain/src/chunk.rs` : `TerrainConfig` += `octaves: Vec<(f64,f32)>` + `edge_falloff_m: f32`.
- `forgia-terrain/src/generation/heightmap.rs` : `heightmap_at` lit `config.octaves` + `config.edge_falloff_m` (plus de hardcode).
- `forgia-terrain/src/lib.rs` : export `TerrainShapeGenome` + `default_octaves`.
- `forgia-rpg/src/lib.rs` : `make_terrain_config` charge le genome ; test bound `cfg.max_height*1.1`.
- `config/genomes/terrain_shape.toml` (NEW) : fichier tunable.

**Validation** : forgia-terrain 127 tests + clippy ✓ ; forgia-rpg check + terrain_height tests + clippy ✓.

**Comportement** : édite le TOML → ressors au menu → re-entre RPG → terrain régénéré (OnEnter re-run make_terrain_config). **Pas de rebuild.** sea_level reste sur RPG_SEA_LEVEL const (couplé à l'eau, hors genome pour éviter la cascade water).

## Retune octaves (pass 1, inclus)

Amplitudes hautes-fréquences `0.5/0.15/0.05 → 0.40/0.08/0.025` → pentes cumulées ~68°→~40° (sous KCC 50° → traversable, fini le blocage) + rolling. Base 0.003=1.0 inchangée (layout + lacs préservés).

## Increment 5 — Biome-specific shape (DONE 2026-06-06)

Recherche web best practices (NoisePost "Fast Biome Blending", RedBlobGames) →
blend des hauteurs pondéré, poids normalisés smoothstep (sum=1), pas de seam dur.
Impl (forgia-terrain SEUL, zéro churn forgia-rpg) : `BiomeType::amplitude_mult()`
(Mountain 1.5, Plains 0.45...) + `BiomeMap::blended_amplitude_mult()` (somme
pondérée via `biome_weights_at` existant) + `heightmap_at_blended` (1 éval bruit +
amplitude blendée). `build_chunk_mesh` + `build_lod2` (mesh + scatter) l'utilisent
(ils ont déjà `biome_map`, pas de changement de signature). 127 tests + clippy ✓.
FIX assets flottants (même jour) : 1re version faisait diverger mesh (biome-aware)
vs placement foliage/village/spawn (`heightmap_at` global) → assets flottaient.
Solution : `heightmap_at` rendu biome-aware via un GLOBAL `WORLD_BIOME_MAP`
(`set/clear_world_biome_map`, publié par spawn_world, lu par heightmap_at) →
TOUS les consommateurs (mesh + foliage verrouillé via heightmap_at:280 + village
+ spawn + décor) reposent sur le MÊME sol biome-aware, sans toucher forgia-foliage.
heightmap_at_blended supprimé (redondant). amplitude_mult hardcodé (→ genome plus tard).

## Increment 6 — amplitude_mult data-driven (DONE 2026-06-06)

Ferme la dette Incr.5 ("amplitude_mult hardcodé → genome plus tard"). Le
multiplicateur d'amplitude relief par biome passe de **framework** (match hardcodé
`biomes.rs::amplitude_mult()`) à **definition** (`config/biomes/<id>.toml::amplitude_mult`),
via le pipeline biome TOML existant (BiomeRegistry, pattern miroir story-575).

Impl (forgia-terrain + config, +3 lignes forgia-rpg) :
- `biome_spec.rs` : champ `amplitude_mult: Option<f32>`.
- `biome_registry.rs` : accessor `amplitude_mult_for()` (TOML ou fallback) + assertion mirror test.
- `biomes.rs` : `amplitude_mult()` lit un global `WORLD_BIOME_AMPLITUDES` (table `[f32;10]`)
  publiée depuis le registry ; `amplitude_mult_default()` = fallback hardcodé (source des défauts) ;
  hot path `blended_amplitude_mult` snapshot la table **1×** (pas 4 locks/vertex).
- `lib.rs` : export `publish_biome_amplitudes_from_config` / `set` / `clear`.
- 10× `config/biomes/*.toml` : `amplitude_mult = X` (miroir exact des fallbacks → zéro régression).
- forgia-rpg : publish au `spawn_world` (à côté de `set_world_biome_map`), clear au
  `cleanup_world`, re-publish dans `apply_terrain_reload` (Shift+F12 retune le relief par biome live).

**Pourquoi un global et pas le registry directement** : `heightmap_at` est une fn pure
(threads de meshing, foliage) sans accès ECS au `BiomeRegistry`. Même pattern que
`WORLD_BIOME_MAP` (Incr.5). `None` → fallback hardcodé (tests, hors RPG).

**Validation** : forgia-terrain clippy + 127 tests ✓ (mirror test étendu) ; forgia-rpg
clippy + terrain tests 2/2 ✓. (4 `spawn_rex_*` KO = WIP rig autre terminal, hors scope.)

## Increment 4 — Ridged mountains (DONE 2026-06-06)

Flag `ridged: bool` par octave (OctaveSpec + TerrainConfig.octaves = `Vec<(f64,f32,bool)>`).
`heightmap_at` applique `(1-|perlin|)*2-1` sur les octaves ridged → crêtes acérées
(montagnes) là où le perlin croise 0. Mixe base smooth + octave ridged = chaînes de
montagnes sur fond rolling. Tunable live (Shift+F12). 127 tests + clippy ✓.

## Increment 3 — Domain warping (DONE 2026-06-06)

Ajout `warp_strength` (40 défaut) + `warp_freq` (0.006) au genome → `heightmap_at`
applique un warp IQ cohérent (déplace les coords d'échantillonnage avec un bruit
basse-fréquence) → terrain naturel/sinueux vs blobs Perlin alignés grille. Câblé
make_terrain_config + apply_terrain_reload (live + re-enter). 127 tests + clippy ✓.

## Increment 2 — Shift+F12 live regen (DONE)

Pour tuner en LIVE (sans re-enter) : un système Shift+F12 qui recharge le genome → update `Res<TerrainConfig>` → despawn chunks + LOD2 + clear ChunkManager + reset `last_player_chunk` → re-snap player (heightmap_at(player.xz)) + re-level village (FlattenZones target_y). Risque modéré (chunk regen sur forgia-rpg contendu) → fait sur demande.

## AC
- [x] Genome chargé au boot (log `[terrain-shape] genome chargé`)
- [x] heightmap_at data-driven, 127+2 tests verts
- [ ] Runtime : éditer max_height/octaves dans le TOML + re-enter → terrain change
- [ ] Increment 2 : Shift+F12 live
