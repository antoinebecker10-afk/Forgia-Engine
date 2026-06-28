# MEGA PROMPT — Forgia AAA Quality Push

> **Copier ce prompt dans une nouvelle session Claude Code ouverte sur `d:\Forgia`.**
> Objectif : rendre Forgia visuellement comparable a GTA 5, Expedition 33, World of Warcraft.
> Contrainte : 200+ FPS sur RTX 4070 Ti (12GB VRAM), zero regression, zero bug non-capte.

---

## CONTEXTE PROJET

Forgia = jeu 3D open-world procedural en Rust/Bevy 0.18.1. ~96k lignes, 3 crates (forgia-game, forgia-terrain, forgia-engine). Map 4096m, 10 biomes, voxel SDF terrain, vegetation streaming, villages, chateau, NPCs, combat FPS.

**GPU cible** : RTX 4070 Ti 12GB. Budget frame : 5ms (200 FPS) a 16ms (60 FPS).

**Lire en premier** : `CLAUDE.md` (racine + sous-repo), `docs/ROADMAP.md`, `.claude/rules/` (toutes les regles).

**Monitoring** : 21+ fichiers `forgia_*.json` ecrits par le jeu toutes les 10s. TOUJOURS les lire avant de coder (`forgia_diagnostics.json`, `forgia_health.json`, `forgia_render_quality.json`, `forgia_colorimetry.json`, `forgia_entity_breakdown.json`, `forgia_gpu_stats.json`, `forgia_lag_events.json`, `forgia_ai_health.json`, `forgia_bug_correlations.json`).

---

## ETAT ACTUEL (snapshot 2026-04-16)

### Performance
- FPS : 50-65 (Ultra preset, RTX 4070 Ti) — **CIBLE : 200+**
- Entities : 25k (budget 15k) — SceneRoot buildings = x10 children
- Triangles : 13M visibles, 19M draw calls estimes — **CIBLE : <5M visible, <5k draw calls**
- SSGI : 0.4 intensity, 6 samples, 48px — cout ~2ms
- Volumetric fog : 48 steps — cout ~2ms
- SSAO : 4 slices x 2 samples — cout ~1ms

### Bugs actifs detectes par capteurs
- **AI stuck 80%** : `forgia_ai_health.json` stuck_ratio=0.80 — pathfinding terrain casse
- **593 floating objects** : `forgia_render_quality.json` floating_objects=593 — vegetation/buildings flottent
- **103 T-pose enemies** : tpose_enemies=103 — animations pas chargees/appliquees
- **Fog color quasi-noir la nuit** : `forgia_colorimetry.json` fog_linear=[0.03,0.05,0.08] — trop sombre
- **Cave network DISABLED** : `forgia_cave_network.json` — code pret mais pas encore actif sur le binaire
- **settings.json override** : le fichier `target/release-fast/settings.json` ecrase les presets code. TOUJOURS verifier ce fichier apres modification de GraphicsSettings.
- **5 terrain maps manquantes** : flow_map, moisture_map, aspect_map, sunlight_map, soil_depth_map — terrain moins realiste

### Stack rendering actif
TAA, SSGI, SSAO, SSR, Bloom, VolumetricFog (god rays), Atmosphere (Hillaire 2020), MotionBlur, AutoExposure, ColorGrading dynamique jour/nuit, CAS Sharpening, ChromaticAberration. DOF desactive (causait du flou).

---

## MISSION : 7 AXES AAA

### AXE 1 — PERFORMANCE 200+ FPS

**Probleme** : 50 FPS au lieu de 200+. Budget 5ms/frame, on est a 18ms.

**Actions** :
1. **GPU Instancing vegetation** : les 8000 arbres sont 8000 draw calls individuels. Bevy auto-batch par handle identique MAIS chaque arbre a un Transform unique = pas de vrai instancing. Implementer un `MeshInstanceBundle` qui groupe les meshes identiques en 1 draw call. Gain attendu : 8000 → ~30 draw calls.
2. **Billboard LOD3** a 150m+ : remplacer les mesh vegetation par des quads textures (imposters). 1 quad = 2 tris au lieu de 2000. Ajouter un LOD3 dans `vegetation_lod.rs`.
3. **Frustum-aware spawn** : ne spawner vegetation/enemies que dans le cone camera 120deg + 30m behind. `VisibilityRange` existe deja, mais les entities sont quand meme creees. Skip le spawn si hors frustum.
4. **Half-res post-process** : SSGI + SSAO + VolumetricFog a 50% resolution + bilateral upscale. Gain ~60% sur ces passes.
5. **Reduce SceneRoot buildings** : 1237 SceneRoot = ~10k child entities. Migrer vers Mesh3d direct (meme pattern que vegetation).
6. **Profiler par system** : utiliser `forgia_systems_perf.json` pour identifier le system le plus couteux et l'optimiser.

### AXE 2 — TERRAIN REALISTE (Expedition 33 / RDR2 level)

**Probleme** : terrain = vertex colors plats, pas de normal maps, pas de detail micro. Ressemble a du low-poly.

**Actions** :
1. **Terrain PBR textures** : triplanar shader WGSL avec 4 textures par biome (albedo, normal, roughness, AO). Les textures existent dans `C:\Users\Antoi\Desktop\Forgia - Fichiers additionnels\Procedural\Textures\`. Extraire le .unitypackage, convertir en KTX2.
2. **Implementer les 5 maps manquantes** :
   - `flow_map` : direction ecoulement eau → placement rivieres + humidite sol
   - `moisture_map` : humidite terrain → vegetation density + type
   - `aspect_map` : orientation pente → ombrage/ensoleillement
   - `sunlight_map` : exposition solaire → neige fond cote sud, mousse cote nord
   - `soil_depth_map` : profondeur sol → rocher affleurant vs terre meuble
3. **Micro-roughness** sur le ring Full : amplitude noise haute frequence pour le detail de surface (already in pipeline but only Full detail).
4. **Elargir le ring Full detail** : actuellement 30% inner du streaming radius. Passer a 50% pour que le joueur ait toujours du terrain Full autour de lui (erosion hydraulique, caves, micro-roughness).
5. **Wetness system** : apres pluie, roughness terrain *= 0.3, albedo *= 0.7 (sol mouille fonce). Lie a WeatherState.

### AXE 3 — ECLAIRAGE & ATMOSPHERE (GTA 5 / God of War)

**Probleme** : eclairage plat, pas de profondeur, nuit trop sombre, god rays faibles.

**Actions** :
1. **6-keyframe color script** : au lieu de 2 etats (jour/nuit), definir 6 presets horaires : Aube (rose-orange), Matin (frais bleu), Midi (chaud dore), Apres-midi (doux), Crepuscule (ambre-violet), Nuit (bleu-argent). Interpoler via spline cubique dans `compute_sky_state`.
2. **God rays puissants** : `VolumetricFog.ambient_intensity` = 0.12 est trop faible. Pousser a 0.25+ quand le soleil est bas (twilight factor). Reduire a 0.05 a midi (pas de god rays en plein jour).
3. **Contact shadows** : petites ombres au pied des objets (herbe, pierres). Bevy 0.18 a `ScreenSpaceAmbientOcclusion` mais pas de contact shadows dediques. Approximer via SSAO avec `constant_object_thickness: 0.08` (plus fin = ombres de contact).
4. **Ambient occlusion vegetation** : bake per-vertex AO sur les modeles vegetation (script Blender batch). Les troncs au sol = sombres, les feuilles hautes = claires.
5. **Light probes** par biome : definir 1 ambient color par biome (foret = vert rebond, desert = jaune chaud, tundra = bleu froid). Appliquer via `GlobalAmbientLight.color` dynamique dans `apply_sky_state`.

### AXE 4 — VEGETATION & MONDE VIVANT (WoW / Elden Ring)

**Probleme** : arbres statiques, pas de vent, certains arbres sans feuilles (1 mesh/tree = parfois le tronc seul).

**Actions** :
1. **Wind sway shader** : vertex shader WGSL qui deplace les vertices hauts (Y > seuil) avec sin(time + world_pos). Genome-driven via `wind_sway_amplitude`, `wind_sway_frequency` per biome.
2. **Fix arbres sans feuilles** : le tri par vertex count dans `vegetation_mesh_extract_system` met parfois le tronc en [0]. Changer le critere : prendre le mesh avec le plus grand bounding box VOLUME (pas vertex count). Le feuillage a toujours un plus grand AABB que le tronc.
3. **Grass wind** : meme shader wind sur l'herbe 3D. Plus rapide = herbe plus legere.
4. **Fireflies/pollen particles** : biome_particles existe deja. Augmenter la densite, ajouter des lucioles la nuit (emissif jaune-vert, random path). Lie au cycle jour/nuit.
5. **Fauna ambient** : sons d'oiseaux de jour, grillons de nuit, vent dans les arbres. Lie a `audio_registry.rs` + biome.

### AXE 5 — BUGS & CAPTEURS (zero bug invisible)

**Probleme** : 593 objets flottants, 103 T-pose, 80% AI stuck — non captures par les alertes.

**Actions** :
1. **Capteur floating objects** : `forgia_render_quality.json` rapporte 593 floating objects mais pas de health alert. Ajouter un check dans `health_monitor.rs` : si floating_objects > 50, severity=warning.
2. **Capteur T-pose** : idem, `tpose_enemies: 103` sans alerte. Ajouter check : si tpose > 10, severity=critical + log entity IDs.
3. **Fix AI pathfinding** : les NPCs n'ont pas de navmesh. Ils patrouillent en ligne droite et se coincent sur le terrain. Options :
   - Simple : raycaster terrain devant le NPC, si pente > 45deg → inverser direction
   - Moyen : grille de walkability par chunk (1m resolution) precalculee a la generation
   - Complet : NavMesh via `oxidized_navigation` crate (Recast/Detour en Rust)
4. **Fix floating vegetation** : les arbres sont spawnes avec un Y estime puis ajustes par `vegetation_height_system`. Si le chunk SDF n'est pas encore pret, le Y reste faux. Ajouter un retry avec timeout + despawn si jamais place.
5. **Correlation engine V2** : ajouter des regles :
   - `FLOATING_OBJECTS > 100` → "Height placement incomplete"
   - `TPOSE > 10` → "Animation loading failed"
   - `AI_STUCK > 0.5` → "Pathfinding broken — needs navmesh"
   - `SSGI_INTENSITY > 0.5` → "SSGI too expensive — settings.json may override"

### AXE 6 — CAVE NETWORK ACTIVATION

**Probleme** : le code mega-cave est implemente (`forgia-terrain/src/cave_network.rs`) mais `forgia_cave_network.json` = DISABLED. Le jeu tourne sur l'ancien binaire ou le cave_network n'est pas passe aux chunks.

**Actions** :
1. Verifier que `CaveNetworkTopologyRes` est bien insere comme resource au world init (`terrain/mod.rs`)
2. Verifier que `streaming.rs` passe `cave_net_arc` a `generate_chunk_lod`
3. Relancer avec le nouveau build
4. Tester : descendre dans une grotte village → suivre un tunnel → atteindre un hub junction → continuer vers un autre village
5. Capteur : `forgia_cave_network.json` doit reporter hubs/routes counts > 0

### AXE 7 — SETTINGS.JSON GUARD

**Probleme critique** : `target/release-fast/settings.json` ecrase toutes les optimisations. Les presets code ne s'appliquent que si le fichier est absent ou si le joueur change de preset in-game.

**Actions** :
1. **Version field** : ajouter `"settings_version": N` dans GraphicsSettings. A chaque modification de preset, incrementer N. Au chargement, si version saved < version code → re-appliquer le preset par defaut.
2. **Capteur settings drift** : ajouter un check dans `forgia_sensor_health.json` qui compare `settings.json` vs preset defaults. Si divergence > 30% sur un champ critique (ssgi, fog, entity_budget) → alert "SETTINGS_DRIFT".
3. **Log au boot** : quand le jeu charge settings.json, logger les valeurs lues vs les defaults pour chaque champ. Ecrire dans `forgia_settings_loaded.json`.

---

## PROTOCOLE D'EXECUTION

1. **Lire** les fichiers `forgia_*.json` AVANT toute modification
2. **1 axe a la fois**, dans l'ordre 5→7→1→3→2→4→6 (bugs d'abord, puis perf, puis visuel)
3. **`cargo check`** apres CHAQUE fichier modifie
4. **Mettre a jour** `target/release-fast/settings.json` si des GraphicsSettings changent
5. **Verifier** les capteurs apres chaque axe (demander a l'utilisateur de lancer le jeu + `regarde`)
6. **Jamais** hardcoder de valeurs — tout en genome TOML ou FpsTuning
7. **Memoire** : sauvegarder les decisions dans `.claude/projects/d--Forgia/memory/`

## CRITERE DE SUCCES

- [ ] 200+ FPS sur RTX 4070 Ti en Ultra
- [ ] 0 floating objects, 0 T-pose, 0 AI stuck >50%
- [ ] Tous les capteurs `forgia_*.json` = status OK
- [ ] Color grading 6 keyframes (aube→nuit)
- [ ] God rays visibles au coucher/lever
- [ ] Terrain PBR avec normal maps
- [ ] Wind sway vegetation
- [ ] Cave network actif (hubs > 0)
- [ ] settings.json versionne

---

*Genere le 2026-04-16 par Claude Code — basé sur l'audit complet des 21 capteurs + git history + codebase 96k lignes.*
