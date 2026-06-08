# Story-539 — Village hexagonal KayKit dans le RPG

**Statut** : EN COURS (code-complete, build OK, validation runtime user en attente)
**Niveau BMAD** : Standard
**Date** : 2026-06-08
**Origine** : feedback user sur le village RPG procédural (boîtes posées sur les collines) — *"regarde les transitions entre les dalles, elles ne sont pas propres et les maisons, tu dois prendre des assets dans les kaykits"*. Direction choisie (AskUserQuestion) : **village hexagonal à plat** (KayKit comme prévu).

## Contexte

Le village RPG précédent (`worldgen_village.rs` v3) plaçait des maisons procédurales (boîte + toit pyramidal) sur le relief, avec des routes maillées qui suivaient le terrain. Reproche user : transitions de dalles pas propres + qualité des maisons.

Le pack **KayKit Medieval Hexagon** (téléchargé, `_downloads/kaykit-hexagon`) est un constructeur de village sur **grille hexagonale plate** : tuiles `hex_grass`/`hex_road_*` qui s'emboîtent bord à bord (transitions nettes par construction), bâtiments (home_A/B, church, tavern, blacksmith, market, well, windmill, watermill, tower, lumbermill…) en 4 couleurs, déco (arbres, rochers, props). 1 texture atlas partagée.

## Décisions

1. **Village hex à plat** : on creuse un disque plat dans le terrain (FlattenZones) et on pose la grille hex dessus. Les tuiles ne tessellent que sur un plan → flatten obligatoire.
2. **Centré sur l'ancre village existante (16,16)** = là où les 4 PNJ on-brand spawnent en arc (3.5 m) et près du spawn joueur (origine monde, ~22.6 m). Joueur arrive **au bord, sur l'herbe plate**, voit le village devant.
3. **Centre + ring 1 = plaza dégagée** (joueur + PNJ), well sur la tuile nord du ring 1, bâtiments ring ≥ 2 (densité 62 %), reste = déco/herbe.
4. **Toolbox** : la primitive hex pure (`hex_spiral`, `Hex::to_world` pointy-top) va dans `forgia-worldgen/src/hex.rs` (réutilisable cités/donjons hex futurs). Le RPG compose + spawn Bevy.

## Implémentation

| Fichier | Rôle |
|---|---|
| `assets/models/kaykit/hexagon/{tiles,buildings,decoration}` | 260 gltf KayKit copiés (atlas embarqué par dossier) |
| `crates/forgia-worldgen/src/hex.rs` (NOUVEAU) | primitive hex pure : `Hex{q,r}`, `to_world(size)`, `hex_ring`, `hex_spiral`. 6 tests |
| `crates/forgia-worldgen/src/lib.rs` | `pub mod hex;` |
| `crates/forgia-rpg/src/worldgen_village.rs` (RÉÉCRIT) | spawn grille hex KayKit (tuiles + bâtiments + déco), colliders async, `village_flatten_zone`, foliage clear. 4 tests |
| `crates/forgia-rpg/src/lib.rs` | `make_village_flatten_zones` pousse la zone plate hex **inconditionnellement** avant le meshing (hoist `target_y`) |

Params (const, v1) : `HEX_SCALE=3.0`, `HEX_RADIUS=3` (37 tuiles), flatten inner 26 m / falloff 16 m, `BUILD_DENSITY=0.62`.

## QA

- `cargo check -p forgia-worldgen -p forgia-rpg` : OK
- `cargo clippy --no-deps` : 0 warning
- Tests : worldgen 42 (dont 6 hex) + forgia-rpg village 4 = OK
- `cargo build -p forgia --profile release-fast` : OK, binaire frais 16:01:59
- Crates **non-contendues** (autre terminal = debug/foliage/game/roguelite/observability)

## Reste (incréments)

- [ ] **Validation runtime user** (look du village)
- [ ] Rues : tuiles `hex_road_A..M` (connexions) — v1 = grass only, transitions déjà nettes
- [ ] Recette TOML data-driven (v1 = const) — sortir HEX_SCALE/RADIUS/density dans `rpg_village.toml`
- [ ] Sensor `forgia2_rpg_village.json` (count tuiles/bâtiments + centre)
- [ ] Commit scopé (forgia-worldgen + forgia-rpg + assets) une fois validé
