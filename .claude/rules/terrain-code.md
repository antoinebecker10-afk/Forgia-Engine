---
paths:
  - "**/terrain/**"
---

# Terrain Code Rules (Forgia)

- Streaming pattern OBLIGATOIRE: queue-based (VecDeque<ChunkCoord>), N chunks/frame
- Manager struct avec HashMap chunk_entities + scene_cache + total_count
- Spawn/despawn uniquement via chunks, jamais d'entites terrain isolees
- Vegetation, villages, ennemis terrain: chargement GLB via scene_cache (exception L1 autorisee)
- Heightmap generation order: buffer 2D → domain warp → redistribution → erosion → path flattening → village flattening → micro-roughness → SDF → mesh → vertex colors
- Biomes data-driven: 10 BiomeType, fichiers TOML dans config/biomes/, BiomeRegistry avec fallback
- ZERO allocation dans les hot paths de streaming (pre-allouer les buffers)
- TerrainMat = StandardMaterial (ExtendedMaterial desactive, bug Bevy 0.18 bindless)
