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

## Audit runtime (2026-06-08) — bug "plus de végétation à côté du village"

User report après le 1er commit. **2 fausses pistes avant la bonne** (leçon [[feedback_dont_deflect_to_other_terminal_own_the_code]]) :
1. ❌ « budget foliage story-583 autre terminal » — réfuté en rebuildant SANS leur WIP (végétation toujours absente près du village).
2. ❌ « flatten + skip `h < sea_level+0.3` » — réfuté par sensor : `player_pos Y=38.7`, biome Forest, ≫ 4.3m.
3. ✅ **`sys_clear_village_foliage` rasait un disque de 32.4m** (= flatten inner+falloff) alors que l'emprise tuiles ≈ 21.5m → 11m de forêt rasée en trop, chaque frame.

**Fix** : `FOLIAGE_CLEAR_RADIUS` dérivé de `VILLAGE_TILE_EXTENT` (≈ 21.5m) + 1m → la forêt pousse jusqu'au bord du village. Validé runtime user ("c'est revenu").

**"Rien sous la map"** vérifié : placement == rendu (tout posé à `anchor.y == flatten target_y`, terrain rendu aplani au même Y), joueur `depth_below_surface=0`, seul le socle des tuiles KayKit est enterré (design, invisible).

## Incréments

- [x] **Validation runtime** look + végétation + sous-map + rues — OK (« routes parfait »)
- [x] Rue principale puis **réseau autotilé complet** (3 rues radiales, autotiler A-M connection-aware)
- [x] Fix clear foliage (dérivé de l'emprise)
- [x] **Commité** `2371fbc` (village) + `0ffa760` (rue + fix clear)

### Itération 5-retours (CODE-COMPLETE, clippy 0, 11 tests — NON COMMITÉ, runtime à valider)
Fichiers : `worldgen_village.rs` + `character.rs` + `hex.rs` (tous MIENS ; foliage = autre terminal, exclu).
- [x] **Scale ×1.8** (HEX_SCALE 4.5 + BUILDING_SCALE_MUL 1.2) — bâtiments ~5m vs Rex ~2m
- [x] **Ville R=4 moins dense** (BUILD_DENSITY 0.42) — bâtiments espacés
- [x] **Fortifications** : enceinte continue (mur sur chaque hex non-porte + tours par-dessus) + 3 portes
- [x] **PNJ aux stations** (devant forge/marché/taverne/puits) + fix flottement (calib ancrée Y village)
- [x] Sécurité spawn joueur (`Hex::from_world` → hex de spawn dégagé)

### Reste
- [ ] **Valider runtime** R=4 fortifié (enceinte fermée ? espacement ? PNJ au sol ?) → commit scopé
- [ ] Recette TOML data-driven (sortir HEX_SCALE/RADIUS/density/scale)
- [ ] Sensor `forgia2_rpg_village.json`
