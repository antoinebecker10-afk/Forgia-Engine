# Story-577 — Trou de couverture LOD2 (gap annulaire) + sonde de couverture

**Statut** : Code DONE (validé check+clippy+127 tests). Runtime à confirmer (rebuild requis).
**Scale** : Quick→Standard (forgia-terrain lod.rs + lib.rs ; hot path LOD).
**Date** : 2026-06-06
**Lignée** : User signale « bande orange F3 au loin = map mal chargée ; quand j'avance et regarde
le spawn, les textures du sol ne s'affichent pas ». Diagnostic sensors+code.

## Symptôme

- F3 → anneau orange (= `unload_m` gizmo, 160m, `forgia-rpg/lib.rs:948`).
- À ~160m du spawn, sol manquant ; en regardant le spawn depuis là, terrain sans surface.

## Root cause (géométrie prouvable)

LOD2 mega-tiles exclus par distance du **centre** de cluster (`inner_m = LOD1_MAX_M = 128m`,
`lod.rs` build_lod2_tiles_system), mais un cluster fait **128m de large** (CHUNK_X=32 ×4),
demi-diagonale **+90.5m**. Le cluster diagonal au spawn a son centre à 90m (< 128 → exclu)
mais son coin lointain atteint **181m**. Les chunks ne couvrent que ~144m (view_m 128 + bord).
→ Bande **~144–181m sans chunk NI LOD2** dans les 4 coins diagonaux = skybox à travers le sol.
L'anneau orange (160m) passe pile dedans.

Pré-existant : le fix view_m 96→128 (story-576) a réduit le trou (avant 112→181m) mais la cause
réelle est l'exclusion LOD2 par centre, indépendante de view_m.

## Fix (extent-aware)

`build_lod2_tiles_system` : inclure un cluster dès que son COIN dépasse `inner_m`
(`center_dist + CLUSTER_HALF_DIAG_M >= inner_m`) au lieu du seul centre. Despawn symétrique
(`center_dist + half_diag < inner_despawn`). Recouvrement chunk/LOD2 bénin (même Y heightmap →
depth buffer). `CLUSTER_HALF_DIAG_M = CLUSTER_SIZE_M * FRAC_1_SQRT_2 ≈ 90.5m`.

## Observabilité (sonde de couverture)

Nouveau `sys_update_lod_coverage` (1Hz) : pour 7 rayons [96,120,140,160,180,220,300] × 16 angles
autour du player, compte `chunk` / `lod2` / `gap` (ni l'un ni l'autre). Exporté dans
`forgia_terrain_lod.json` :
- `coverage`: `[{r, samples, chunk, lod2, gap}, …]`
- `worst_gap_r`, `max_gap_frac` (résumé : 0.0 = couverture parfaite).
Garde-fou non-régression : `max_gap_frac` doit rester 0 après le fix.

## Volet 2 — « la montagne a englouti le spawn » (même session)

User signale, sur le binaire rebuild (LOD fix OK, `max_gap_frac=0.000`), que le terrain
dramatique (`max_height=80` × amplitude biome) monte à **60–87m** dans un rayon de 64–181m
autour du spawn (~38m) → mur de montagne qui cerne le village. Cause = tuning relief (choix
user « relief dramatique »), pas un bug.

Fix (data + plomberie hot-reload) :
- `starter_hamlet.toml::terrain_leveling.falloff_m 25→80` → rampe ~14° du plateau (38m) vers
  le relief (~58m@107m), montagnes repoussées en backdrop. radius_m=27 inchangé.
- `forgia-rpg` : `apply_terrain_reload` (Shift+F12) reconstruit désormais la `FlattenZones`
  depuis le genome (helper `make_village_flatten_zones`, partagé avec `spawn_world`) →
  `radius_m`/`falloff_m` hot-reloadables comme `max_height`. target_y re-échantillonné.
- Sûr : foliage/mesh/village/routes appliquent TOUS `FlattenZones` (pas d'arbre flottant —
  `forgia-foliage/lib.rs:276` fix lévitation story-447).

Leviers live (Shift+F12, 2 fichiers) : `terrain_shape.toml::max_height` (relief global) +
`starter_hamlet.toml::terrain_leveling.falloff_m` (taille du bowl de spawn).

## Volet 3 — RÉGRESSION du fix extent-aware : LOD2 recouvre le spawn (même session)

User (capture) : « la montagne a recouvert le spawn, le gizmo de la ville est recouvert par le
sol de la montagne ». Cause = régression directe du Volet 1 : avant, les mega-tiles LOD2 ne
spawnaient qu'à ≥128m (centre cluster) → jamais sur la ville. Le fix extent-aware les fait
spawner dès `center+90 ≥ 128` ⇒ centre ≥ ~38m ⇒ le cluster (0,0) (centre ~64m du player) est
maintenant dessiné EN LOD2 sur la zone du spawn. Or `build_lod2_terrain_mesh` calculait
`raw_y = heightmap_at()` SANS `FlattenZones` → le mesh LOD2 brut (montagne 50-87m) se rendait
PAR-DESSUS la ville aplanie (37.6m), enterrant les gizmos de footprint.

Fix : `build_lod2_terrain_mesh` applique désormais `FlattenZones` (coords MONDE sans offset,
identique à `build_chunk_mesh:84`). `build_lod2_tiles_system` reçoit `Option<Res<FlattenZones>>`
et le passe au mesh + au scatter trees/rocks. LOD2 et chunks s'accordent partout (overlap bénin,
même Y) → la ville aplanie n'est plus recouverte, le gap reste fermé.

Invariant désormais complet : **TOUT** ce qui dessine du sol applique `FlattenZones` —
chunks LOD0/LOD1 (`meshing_heightmap`), **LOD2 (`lod.rs`, story-577 v3)**, foliage, village, routes.

Watch : les tiles LOD2 chevauchant le spawn peuvent y scatter des arbres-imposteurs (à
surveiller ; suppression du scatter dans la flatten-zone = follow-up si gênant).

## Volet 4 — z-fighting de l'overlap LOD2/chunks (même session)

User : « les textures du sol changent en fonction de l'angle de la caméra » (spawn OK depuis v3).
Cause : le fix extent-aware fait coexister tile LOD2 + chunks (~38–160m) au MÊME Y → z-fighting
(le depth-test alterne le gagnant selon l'angle → la texture/teinte "change"). Le commentaire
lod.rs supposait "pas de flicker car même Y" — faux : meshes de résolutions/UV différentes.

Fix : biais de profondeur (skirt) `LOD2_DEPTH_BIAS_M = 2.0` appliqué via la Transform Y du tile
(`-2.0`) → descend mesh + arbres/rochers ENSEMBLE sous les chunks. Dans le recouvrement le chunk
gagne toujours le depth-test (fin du z-fight) ; dans le trou comblé + lointain le LOD2 est 2m sous
la hauteur vraie (imperceptible à 144m+ / angle rasant). Trou toujours fermé (coverage = présence
d'entité, pas de hauteur → `max_gap_frac` reste 0). Watch : léger ressaut ~2m possible à la
frontière trou (~144m, lointain) — réduire le biais à ~1m si visible.

## Validation

forgia-terrain : clippy clean + 127 tests ✓. forgia-rpg : clippy clean + terrain tests 2/2 ✓.
`cargo check -p forgia` (binaire complet) ✓.

## AC

- [x] Cause identifiée + prouvée (géométrie + sensors)
- [x] Fix extent-aware spawn + despawn
- [x] Sonde de couverture exportée
- [x] check + clippy + 127 tests verts
- [ ] Runtime : rebuild → F3 au spawn → plus de trou diagonal ; `forgia_terrain_lod.json::max_gap_frac == 0.0`
