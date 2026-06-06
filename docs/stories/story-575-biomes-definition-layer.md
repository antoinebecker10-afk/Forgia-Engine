# Story-575 — Biome definition layer (config/biomes/*.toml)

**Statut** : DONE (impl + test zero-régression verts ; runtime à confirmer au prochain run)
**Scale** : Standard (10 TOML + 1 helper + 1 test)
**Date** : 2026-06-05
**Lignée** : Audit procgen → finding **B1** ("Bloquant" : `config/biomes/` inexistant → registry charge 0 spec → tout sur fallbacks Rust). Décision user **(b) miroir des fallbacks**.

## Nuance (comme B2)

B1 était classé "Bloquant" mais les fallbacks **fonctionnent** (biomes diffèrent déjà via noise.rs + registry match). B1 n'est donc PAS un bug visuel — c'est **débloquer le tuning data-driven / hot-reload**. Approche (b) = comportement identique, mais désormais éditable.

## Change set

- **10 fichiers** `config/biomes/{plains,forest,desert,mountain,swamp,tundra,savanna,jungle,volcanic,canyon}.toml` — miroir EXACT des fallbacks Rust.
- Helper `BiomeRegistry::from_specs(Vec<BiomeSpec>)` (refactor `load()` + réutilisé par le test).
- Test `config_biomes_toml_mirror_rust_fallbacks_exactly` : charge les vrais TOML, prouve que chaque accessor == registry vide (fallbacks).

## Garde-fous zero-régression (concept-first consumers)

- `enemy_modifiers()` + `road_config()` n'ont **pas** de garde Option → dès qu'un spec existe, le registry lit `spec.enemy_modifiers`/`spec.road` (defaultent neutre si absents) au lieu du fallback biome → **populés exactement** dans chaque TOML.
- `color`/`roughness`/`preview_rgb`/`display_name_fr` → populés exact (Option-gardés, redondance sûre).
- `lacunarity`/`persistence`/`slope_max`/`thermal_passes`/`height_mult` → **omis** (fallback None/1.0 préservé ; les mettre changerait le terrain).
- `grass` → omis (aucun consommateur actif).

## Acceptance criteria

- [x] AC1 — 10 TOML créés, parsent (`load_all_biome_specs` → 10)
- [x] AC2 — `cargo test -p forgia-terrain` : 123 passed (incl. test miroir)
- [x] AC3 — `cargo clippy -p forgia-terrain` : 0 warning
- [x] AC4 — Zéro régression PROUVÉE (chaque accessor == fallback)
- [ ] AC5 — Runtime : log `BiomeRegistry: loaded 10 biome specs from TOML` (était 0) au prochain run

## Limites connues / honnêteté runtime

- Plusieurs accessors n'ont pas (encore) de consommateur visuel actif en W1 :
  - `color()` terrain : le vertex color du mesh utilise `biome.linear_rgba()` **hardcodé** (meshing_heightmap.rs), PAS `registry.color()` → éditer `color` dans un TOML + Shift+F12 ne changera PAS la teinte du terrain tant que ce path n'est pas wiré (audit M-cosmétique).
  - `setup_biome_materials` (registry.color/roughness) = Resource morte en W1.
- Donc P1 **active la couche** et **débloque le tuning** de enemy_modifiers / road / spawn_weight ; le payoff visuel complet (couleur terrain data-driven) = follow-up de wiring séparé.
