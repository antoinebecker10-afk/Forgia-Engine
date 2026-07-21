# Roadmap — Rendering Pipeline V2 (mid-distance & horizon)

> ⚠️ **SUPERSEDED — voir [`docs/ROADMAP.md`](./ROADMAP.md) (source de vérité unique).** Conservé pour le détail technique du pipeline de rendu.

**Créé** : 2026-05-19
**Auteur** : session Claude après audit V1+V2 + recherche industrie
**Contexte session** : après fix `sample_offset` (terrain spawn flat à sea_level → relief réel) Antoine demande pourquoi le mid-distance / horizon V2 n'a pas le rendu AAA d'un Witcher 3 / Skyrim / RDR2. Audit révèle 6 trous concrets sur le pipeline de profondeur.

---

## 1. Diagnostic — Ce qui manque dans Forgia V2 vs AAA

Pipeline actuel V2 (post-fix sample_offset 2026-05-19) :

```
[CAMERA] near=0.05m ─────────────────────────────────► far=2000m
   ├─ 0 ────► 96m   │ LOD0 : chunks heightmap full + path ribbons + vegetation
   ├─ 96 ───► 128m  │ LOD1 : chunks mesh seul (no veg)
   ├─ 128 ──► 1500m │ LOD2 : 428 mega-tiles 128×128m
   ├─ >1500m        │ Skybox direct
   └─ Y ≤ 4m partout│ bevy_water tiles
```

État runtime mesuré (sensors live 2026-05-19) :
- `gen_ms.mean = 0.49ms` (max 1ms) sur 119 samples → terrain async OK
- 35 chunks loaded (30 LOD0 + 5 LOD1 + 0 LOD2 chunks) + 428 LOD2 mega-tiles
- Memory budget : 175 MB / 512 MB cap → OK
- `GpuPreprocessingSupport: fully supported` (log boot) → batching auto activable

### Les 6 trous

| # | Symptôme | Cause root | État V1 |
|---|---|---|---|
| 1 | Horizon "trop net" à 1500m, pop visible LOD0/LOD1 | **Aucun `DistanceFog` Bevy attaché caméra** | ✅ V1 a fog volumétrique + 10 profils biome |
| 2 | Trees disparaissent brutalement à 96m | **Pas d'impostors octahedral** (vegetation = LOD0 only) | ⚠️ V1 a meshopt LOD3-tier mais pas impostors |
| 3 | 400+ LOD2 tiles = potentiellement 400 draw calls | **À VÉRIFIER** : Bevy 0.18 doit batcher auto via `BinnedRenderPhase` + `MeshAllocator` | ⚠️ V1 batching manuel (`vegetation_gpu.rs::bake_species_batch`) — pourrait être obsolète |
| 4 | Pop terrain visible à 96m et 128m | **Pas de geomorphing** entre tiers LOD | ❌ V1 absent aussi |
| 5 | Murs village n'occultent pas le terrain derrière | **Pas d'occlusion culling** (Bevy frustum-only par défaut) | ❌ V1 absent (workaround `NoFrustumCulling`) |
| 6 | Pipeline tout CPU-driven theoretically | **`gpu_preprocessing` actif mais batching à vérifier** | ⚠️ Partial |

---

## 2. État de l'art industrie (sourcé)

| Studio / Moteur | Pattern profondeur principal | Source |
|---|---|---|
| **Witcher 3 (REDengine 3)** | DistanceFog + atmospheric scattering + impostors trees (SpeedTree billboards) + Hierarchical Z-Buffer software | GDC 2014 talks REDengine |
| **Skyrim (Creation Engine)** | Distant LOD baked offline (xLODGen tool) + fog dense + impostors | Beth Plugin docs |
| **RDR2 (RAGE)** | Volumetric fog + atmospheric scattering + LOD morphing + occlusion HZB GPU-driven | RDR2 Tech Talks GDC 2019 |
| **Horizon ZD (Decima)** | GPU-driven mesh shaders + virtual texturing + impostors octahedral | Guerrilla Decima papers SIGGRAPH |
| **UE5 (Nanite/Lumen)** | Software rasterizer + virtualized geo + screen-space culling | Epic Nanite SIGGRAPH 2021 |
| **Cities Skylines 2 (Unity HDRP)** | Burst gen + DOTS instancing + fog volumetrique | CO devblog 2023 |

**Patterns AAA universels qui s'appliquent à Forgia** :
1. **DistanceFog en première ligne** — masque pop, compense absence d'impostors
2. **Impostors octahedral** — réduit 100k+ trees à ~50 draws via texture array
3. **Geomorphing** (Hoppe 1998) — élimine pop LOD entre tiers
4. **HZB occlusion** — économise 20-40% draw calls en environment dense
5. **Batching auto via slabs** (Bevy `MeshAllocator` = port offset_allocator Aaltonen)

---

## 3. Plan d'implémentation phasé

### Ordre de priorité (ratio impact/effort décroissant)

| Vague | Sujet | Effort | Impact visuel | Risque | Crates touchées |
|---|---|---|---|---|---|
| **W1** | DistanceFog port V1→V2 + AtmosphereProfile | S | ⭐⭐⭐⭐⭐ | bas | NEW `forgia-atmosphere` |
| **W2** | OcclusionCulling experimental Bevy 0.18 | S | ⭐⭐ | bas (experimental mais merged) | mod camera_orbit + camera_fps |
| **W3** | Audit batching auto + diag sensor | S | ⭐ (mesure pure) | nul (read-only) | NEW `forgia-render-diag` |
| **W4** | Vegetation LOD port V1 (meshopt 3-tier) | M | ⭐⭐⭐ | moyen | NEW `forgia-vegetation-lod` |
| **W5** | Geomorphing LOD0↔LOD1↔LOD2 | M | ⭐⭐⭐⭐ | moyen | NEW `forgia-terrain-morph` |
| **W6** | Impostor trees octahedral (SpeedTree pattern) | L | ⭐⭐⭐ | haut (R&D) | NEW `forgia-impostors` |

---

### Vague 1 — DistanceFog + AtmosphereProfile biome-driven

**Objectif** : masquer pop LOD2, donner profondeur atmosphérique cinéma.

**Source V1 à porter** :
- `D:/Forgia/.../volumetric_fog.rs:13,160,200` — 228 lignes
- `D:/Forgia/.../game_setup.rs:221` — hook caméra
- `D:/Forgia/.../apply_graphics_settings.rs:52,81` — toggle quality
- 10 profils `AtmosphereProfile` data-driven (Swamp `density=0.008`, Volcanic `0.024`, etc) avec exponential height decay `e^(-falloff*h)`

**API Bevy 0.18 cible** :
```rust
use bevy::pbr::{DistanceFog, FogFalloff};

commands.entity(camera).insert(DistanceFog {
    color: Color::srgb(0.55, 0.62, 0.70),
    falloff: FogFalloff::Linear { start: 600.0, end: 1500.0 },
    ..default()
});
```

**Variantes recommandées** :
- `FogFalloff::ExponentialSquared { density }` pour atmosphère réaliste
- `FogFalloff::Atmospheric { extinction, inscattering }` pour Rayleigh scattering AAA

**Architecture** :
```
crates/forgia-atmosphere/
  src/
    lib.rs           # ForgiaAtmospherePlugin
    profile.rs       # AtmosphereProfile struct + 10 defaults (port V1)
    biome_blend.rs   # Sample biome under camera → interpolate AtmosphereProfile
    sensor.rs        # forgia_atmosphere.json (current biome, density, color)
  Cargo.toml
```

**Data-driven** : profils dans `config/genomes/atmosphere/*.toml`. Hot-reloadable Shift+F12.

**Critères acceptance** :
- [ ] LOD2 tiles à 1500m fade visuellement vers `fog.color`
- [ ] Caméra dans Forest biome → density ~0.005, color verdâtre ; dans Swamp → 0.008 plus dense ; etc.
- [ ] Transition smooth quand player traverse frontière biome (interpolation 2s)
- [ ] Sensor `forgia_atmosphere.json` écrit (current biome + density + color RGB)
- [ ] 0 clippy warning, cargo check propre

**Source URLs** :
- [bevy.org/examples/3d-rendering/fog](https://bevy.org/examples/3d-rendering/fog/)
- [github bevy/examples/3d/fog.rs](https://github.com/bevyengine/bevy/blob/main/examples/3d/fog.rs)
- [DistanceFog docs.rs](https://docs.rs/bevy/latest/bevy/pbr/struct.DistanceFog.html)

**Estimation** : ~250 LOC port + ~80 LOC sensor = ~330 LOC. 1 session focus.

---

### Vague 2 — OcclusionCulling (HZB experimental)

**Objectif** : économiser draw calls dans village + caves (occluders denses).

**Bevy 0.18 livré** via PR #17413 (pcwalton merged) — `OcclusionCulling` Component requires `DepthPrepass`.

**API exacte** :
```rust
use bevy::core_pipeline::prepass::DepthPrepass;
use bevy::render::experimental::occlusion_culling::OcclusionCulling;

commands.entity(camera).insert((DepthPrepass, OcclusionCulling));
```

**Pipeline interne** : 2-phase HZB
1. Rendu visible objects round 1
2. Génère HZB depth pyramid mip-mapped
3. Test occlusion sur "potentially visible" pour round 2

**Impact attendu** :
- Open world flat steppe : ~0% (peu d'occluders)
- Village dense KayKit : 20-30% draw calls économisés
- Caves (futur) : 50%+ économisés

**Critères acceptance** :
- [ ] Component ajouté caméra orbit RPG et caméra FPS Arena
- [ ] FrameTime stable même avec OcclusionCulling actif
- [ ] Sensor `forgia_render_diag.json` (Vague 3) capture culling stats

**Risque** : experimental Bevy = peut changer API en 0.19. Documenter clairement dans le Cargo.toml.

**Source URLs** :
- [PR #17413](https://github.com/bevyengine/bevy/pull/17413)
- [OcclusionCulling docs.rs](https://docs.rs/bevy/latest/bevy/render/experimental/occlusion_culling/struct.OcclusionCulling.html)
- [Medium two-pass HZB tutorial](https://medium.com/@Lucmomber/two-pass-hierarchical-z-buffer-occlusion-culling-93171c5a9808)

**Estimation** : ~30 LOC (2 components à ajouter dans crates camera + 1 test). 0.5 session.

---

### Vague 3 — Audit batching auto + diag sensor

**Objectif** : mesurer si les 400 LOC2 tiles sont effectivement batchés par Bevy 0.18 ou pas.

**Hypothèse à valider** : `MeshAllocator` (offset_allocator port Aaltonen) pack vertex buffers en slabs → `BinnedRenderPhase` génère multi-draw indirect même pour meshes différents partageant material → 400 tiles → ~1-5 draws GPU.

**Implementation** :
```rust
// forgia-render-diag/src/lib.rs
fn export_render_diag_sensor(
    mesh_allocator: Option<Res<MeshAllocator>>,
    render_device: Res<RenderDevice>,
    mut last_export: Local<f32>,
    time: Res<Time>,
) {
    // 1 Hz export
    if time.elapsed_secs() - *last_export < 1.0 { return; }
    *last_export = time.elapsed_secs();

    // Stats : nb slabs, vertices per slab, total draws estimated
    let json = serde_json::json!({
        "timestamp_secs": time.elapsed_secs(),
        "mesh_allocator_slabs": mesh_allocator.as_ref().map(|m| m.slab_count()),
        "gpu_preprocessing_active": true,  // confirmé par log boot
        // ...
    });
    std::fs::write("forgia_render_diag.json", json.to_string())?;
}
```

**Note** : `MeshAllocator::slab_count()` n'est pas une API publique Bevy 0.18, il faudra possiblement passer par les diagnostics existants (`DiagnosticsStore` "render/draw_calls" si exposé) ou ajouter notre propre instrumentation.

**Critères acceptance** :
- [ ] Sensor `forgia_render_diag.json` écrit 1Hz avec draw_calls estimés
- [ ] Si draw_calls > 100 pour 400 LOC2 tiles partageant 1 material → batching auto NOT working → escalate
- [ ] Sinon → batching OK, on peut virer le batching manuel V1 (vegetation_gpu.rs::bake_species_batch) lors d'un futur cleanup

**Source URLs** :
- [docs.rs bevy::render::batching::gpu_preprocessing](https://docs.rs/bevy/latest/bevy/render/batching/gpu_preprocessing/index.html)
- [MeshAllocator docs.rs](https://docs.rs/bevy/latest/bevy/render/mesh/allocator/struct.MeshAllocator.html)
- [pcwalton GPU-driven release gist](https://gist.github.com/pcwalton/7562c1a9b98bb5ae33ba2e52e41a89e5)

**Estimation** : ~80 LOC. 0.5 session.

---

### Vague 4 — Vegetation LOD port V1 (meshopt 3-tier)

**Objectif** : étendre la vision foliage de 96m → 300m via mesh simplification progressive.

**Source V1 à porter** :
- `D:/Forgia/.../vegetation_lod.rs:1-140` — 140 lignes
- Pattern : `meshopt::simplify` → 3 tiers (100% LOD0, 30% LOD1, 8% LOD2) aux distances 60m / 150m / 300m

**Crate Rust** : [`meshopt`](https://crates.io/crates/meshopt) (bindings meshoptimizer Arseny Kapoulkine). Vérifier compat Bevy 0.18.

**Architecture** :
```
crates/forgia-vegetation-lod/
  src/
    lib.rs                   # ForgiaVegetationLodPlugin
    simplify.rs              # Wrapper meshopt::simplify
    swap_system.rs           # Distance-based mesh swap (3 versions per tree)
    bake_pipeline.rs         # Bake LOD0→LOD1→LOD2 au Startup (cache filesystem)
    sensor.rs                # forgia_vegetation_lod.json
  Cargo.toml
```

**Critères acceptance** :
- [ ] Trees visibles jusqu'à 300m (vs 96m actuel)
- [ ] FrameTime stable (mesh simplification = gain perf, pas coût)
- [ ] Bake une fois au boot (~200ms), cached entre sessions
- [ ] Sensor reporte LOD switches/sec
- [ ] 0 hardcode : tiers + distances dans `config/genomes/vegetation_lod.toml`

**Source URLs** :
- [meshopt crate](https://crates.io/crates/meshopt)
- [meshoptimizer GitHub](https://github.com/zeux/meshoptimizer)
- V1 implem `D:/Forgia/.../vegetation_lod.rs`

**Estimation** : ~140 LOC port + ~60 LOC bake cache + ~40 sensor = ~240 LOC. 1 session.

---

### Vague 5 — Geomorphing LOD terrain (Hoppe 1998)

**Objectif** : éliminer pop visible entre LOD0/LOD1/LOD2 par interpolation linéaire vertex shader.

**Pas dans V1** — nouveau dev pure.

**Pattern Hoppe Smooth view-dependent level-of-detail rendering** (paper [hhoppe.com/geomclipmap.pdf](https://hhoppe.com/geomclipmap.pdf)) :

1. **Bake-time** : pour chaque vertex LODn, calculer la position correspondante au LODn+1 (parent vertex après simplification). Store dans vertex attribute custom `morph_target: Vec3`.

2. **Runtime WGSL** :
```wgsl
@vertex
fn vertex(in: VertexInput) -> VertexOutput {
    let dist = length(view.world_position - in.position);
    let lod_band_start = uniforms.lod_threshold_m;
    let lod_band_size = uniforms.lod_band_m;
    let t = clamp((dist - lod_band_start) / lod_band_size, 0.0, 1.0);
    let morphed_pos = mix(in.position, in.morph_target, t);
    // ... reste pipeline standard
}
```

3. **Bevy 0.18 setup** :
```rust
use bevy::mesh::MeshVertexAttribute;
use bevy::render::render_resource::VertexFormat;

pub const ATTRIBUTE_MORPH_TARGET: MeshVertexAttribute =
    MeshVertexAttribute::new("MorphTarget", 988540917, VertexFormat::Float32x3);
```

**Architecture** :
```
crates/forgia-terrain-morph/
  src/
    lib.rs                   # Plugin + custom Material
    material.rs              # ForgiaTerrainMorphMaterial : Material trait
    bake.rs                  # Compute morph_target Vec3 par vertex au mesh build
    shader.wgsl              # Custom vertex shader avec interpolation
  Cargo.toml
```

**Critères acceptance** :
- [ ] Transition LOD0↔LOD1 smooth (pas de pop visible à 96m)
- [ ] Transition LOD1↔LOD2 smooth (pas de pop visible à 128m)
- [ ] Performance equal ou meilleure (1 vertex attribute supplémentaire, négligeable)
- [ ] Material partagé avec `TerrainSharedMaterial` actuel (ou bien le remplace cleanly)

**Risque** :
- Custom Material peut interférer avec `terrain_material::TerrainSharedMaterial` actuel
- Bake morph_target nécessite connaître le parent LOD au build time → modifier `build_chunk_mesh`

**Source URLs** :
- Hoppe paper [hhoppe.com/geomclipmap.pdf](https://hhoppe.com/geomclipmap.pdf)
- [GameDev.net Terrain Geomorphing tutorial](https://www.gamedev.net/tutorials/_/technical/graphics-programming-and-theory/terrain-geomorphing-in-the-vertex-shader-r1936/)
- TUW paper [tuw-138077.pdf](https://www.ims.tuwien.ac.at/publications/tuw-138077.pdf)

**Estimation** : ~150 LOC Rust + 80 LOC WGSL + ~70 LOC bake = ~300 LOC. 1 session focus.

---

### Vague 6 — Impostor trees octahedral (SpeedTree-like)

**Objectif** : trees lisibles à 500m-1500m via texture array + quad face-camera.

**Pas de crate Bevy 0.18 mature** — chantier custom.

**Pipeline** :

#### Phase A — Bake offline (outil dev)
1. Charger 1 tree mesh GLB
2. Caméra orbitale 8×8 = 64 vues hémisphère octaédrique
3. Pour chaque vue : render → capture albedo + normal + depth
4. Pack en Texture2DArray (3 layers × 64 slices)
5. Sauvegarder `tree_<species>.imp` (custom format)

#### Phase B — Runtime
1. Quad billboard 1 par tree (vertex shader oriente vers caméra)
2. Fragment shader : view direction → UV octaédrique → sample Texture2DArray
3. Swap LOD2 → impostor au-delà de 500m

**Architecture** :
```
crates/forgia-impostors/
  src/
    lib.rs                   # ForgiaImpostorsPlugin
    bake/                    # Bake tool offline
      mod.rs                 # CLI : forgia-impostor-bake --mesh tree.glb --output tree.imp
      capture.rs             # 64 vues octahedral capture
      pack.rs                # Texture2DArray packing
    runtime/
      mod.rs                 # Spawn impostors aux distances > IMPOSTOR_START_M
      material.rs            # ImpostorMaterial : Material trait
      shader.wgsl            # Vertex billboard + fragment sample array
    swap_system.rs           # Distance-based swap tree mesh → impostor quad
  Cargo.toml
```

**Critères acceptance** :
- [ ] 1000+ trees visibles à 1500m sans drop FPS
- [ ] Pop entre vegetation LOD2 mesh et impostor masqué par fog (Vague 1)
- [ ] Bake tool fonctionne pour tout GLB tree
- [ ] Texture2DArray cached (re-bake pas systématique)

**Risque haut** :
- Premier projet AAA-style dans Bevy → R&D
- WGSL custom complexe (octahedral UV math)
- Bake tool = mini-project (2-3 jours estimés)

**Source URLs** :
- [UE5 Octahedral Impostors forum](https://forums.unrealengine.com/t/ue5-foliage-and-octahedral-imposters/247858)
- [Crytek Sparse Voxel Octree Global Illumination paper SIGGRAPH](https://www.crytek.com/cryengine/presentations)
- [SpeedTree LOD whitepaper](https://store.speedtree.com/whitepaper/)
- Octahedron mapping math : [JCGT 2014 vol 3 #2](https://jcgt.org/published/0003/02/01/)

**Estimation** : ~600 LOC + outil bake CLI + WGSL custom. **1 semaine focus**, pas une session.

---

## 4. Dépendances entre vagues

```
W1 (Fog) ──┐
           ├──► gain visuel max (peut shipper seul)
W2 (Occl) ─┘
              W3 (diag) ──► informe W4-W6 sur batching
W4 (Veg LOD) ──► indépendant
W5 (Morph) ──► touche terrain_material → faire APRÈS W4 stable
W6 (Impost) ──► dépend W4 (LOD vegetation tiers définis)
```

**Ordre recommandé final** : W1 → W2 → W3 → W4 → W5 → W6.

---

## 5. Stability Locks impactés

- **L1 (GameAssets)** : W4 ajoute meshes simplifiés (cache filesystem, pas dans assets.rs) → pas d'impact
- **L7 (SystemSets)** : tous les nouveaux systems doivent `.in_set(GameSet::X)` correct (probablement `Effects` ou `UI` pour fog, `Movement` pour LOD swap)
- **Locks terrain** (pas dans la liste actuelle) : W5 modifie `terrain_material` → vérifier qu'aucun Lock n'a été ajouté entre 2026-05-19 et la reprise

---

## 6. Reprise — checklist session future

À l'ouverture de la session pour démarrer ce chantier :

1. [ ] Lire ce fichier en entier
2. [ ] Vérifier `forgia_chunk_stream.json` + `forgia_terrain_lod.json` toujours OK runtime
3. [ ] Confirmer Bevy 0.18.x toujours dans Cargo.toml (sinon réauditer API)
4. [ ] Choisir vague à attaquer (W1 recommandé)
5. [ ] Créer story `docs/stories/story-XXX-render-pipeline-wN.md` BMAD Standard
6. [ ] Implémenter selon spec de la vague
7. [ ] Cocher critères acceptance
8. [ ] Mettre à jour ce fichier : status vague → DONE + date
9. [ ] Commit

---

## 7. Sources de référence — gardées pour la reprise

### Bevy 0.18
- [docs.rs bevy 0.18](https://docs.rs/bevy/0.18.0/bevy/)
- [bevy-cheatbook](https://bevy-cheatbook.github.io/)
- [examples/3d](https://github.com/bevyengine/bevy/tree/main/examples/3d)
- [pcwalton GPU-driven gist](https://gist.github.com/pcwalton/7562c1a9b98bb5ae33ba2e52e41a89e5)
- [PR #17413 OcclusionCulling](https://github.com/bevyengine/bevy/pull/17413)

### Industrie
- Hoppe geomorphing : [hhoppe.com/geomclipmap.pdf](https://hhoppe.com/geomclipmap.pdf)
- Witcher 3 REDengine : GDC 2014 talks
- RDR2 RAGE atmospheric : GDC 2019
- Decima virtual texturing : SIGGRAPH 2018-2020
- UE5 Nanite : SIGGRAPH 2021
- meshoptimizer : [github.com/zeux/meshoptimizer](https://github.com/zeux/meshoptimizer)
- Octahedral impostors : [JCGT 2014 vol 3 #2](https://jcgt.org/published/0003/02/01/)

### V1 Forgia (à porter)
- `D:/Forgia/.../volumetric_fog.rs` + `apply_graphics_settings.rs`
- `D:/Forgia/.../vegetation_lod.rs`
- `D:/Forgia/.../vegetation_gpu.rs` (batching manuel, à comparer vs Bevy 0.18 auto)

---

*Roadmap stable. Mise à jour autorisée seulement pour marquer vagues DONE ou re-prioriser après mesure W3.*
