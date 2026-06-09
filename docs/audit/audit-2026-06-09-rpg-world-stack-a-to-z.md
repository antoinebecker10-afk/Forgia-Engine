# Audit A→Z — Stack complète de génération du monde RPG (Forgia V2 Rewrite)

> **Date** : 2026-06-09 · **Type** : audit read-only, toutes les couches après le terrain · **Méthode** : 4 explorations parallèles (végétation, eau/ciel/lumière, structures/routes, matériaux/VRAM/streaming/observabilité) + l'audit terrain du même jour ([audit-2026-06-09-mapgen-v1-vs-v2-rpg.md](audit-2026-06-09-mapgen-v1-vs-v2-rpg.md)).
> **Contexte priorité** : SHIP = Roguelite. Le monde RPG = track FORGE (pas ship-critique). Reco pondérées par « ça sert le ship / c'est un bug / c'est un orphelin cheap à brancher ».

---

## 🎯 Le constat méta (le plus important)

**Le monde RPG V2 n'a pas un problème de FEATURES manquantes — il a un problème de CÂBLAGE.** Couche après couche, on retrouve le même motif : **la richesse de V1 est portée dans le code V2 mais ORPHELINE/débranchée**, et le RPG tourne sur une tranche fonctionnelle « large mais peu profonde » + quelques **vrais bugs**.

**Catalogue des orphelins (code/data présents, non câblés)** :
| Orphelin | Où | État |
|---|---|---|
| Gen terrain riche (érosion, redistribution, micro-roughness) | `heightmap_at_gen_ext` | codé, le RPG appelle `heightmap_at` (pauvre) |
| Génomes végétation (densité, wind, L-system, slope, cull, ratios) | `vegetation_default.toml`, `grass_default.toml`, `biome_*.toml` | 308 L, **0 lecteur** en V2 |
| Génome atmosphère (40+ genes : fog, bloom, soleil, exposure, Rayleigh/Mie, god rays, stars, **cycle jour/nuit**) | `atmosphere_default.toml` | **0 consumer** |
| Nuages/vent volumétriques | `clouds_default.toml`, `wind_default.toml` | **0 consumer** |
| 45 effets post-process (bloom, fog, god rays, toon, outline) | `forgia-postprocess` | **pas ajouté** dans forgia-game |
| Toolbox worldgen IA (build_city, grammaire, parcelles, bake/cache) | `forgia-worldgen` | accessible **uniquement en démo Roguelite (F7/F9/F11)**, jamais en RPG |
| Profilage per-chunk 19 couches | `pipeline_diag.rs` | **STUB no-op** |

> **Implication stratégique** (vision « moteur IA-natif ») : ces orphelins ne sont pas du déchet — ce sont des **actifs latents**. Pour un moteur où « l'IA construit le jeu », avoir des systèmes riches prêts-mais-débranchés est exactement ce qu'on veut : l'IA les câble **à la demande**. Le vrai livrable de cet audit = **le catalogue de ce qui est prêt à activer** + **la liste des vrais bugs à corriger**.

---

## Stack A→Z, couche par couche

| Couche | État | Ce que fait V2 RPG | Trou principal | Industrie |
|---|---|---|---|---|
| **0. Terrain (base)** | ✅ fonctionnel, pragmatique | heightmap-grid 33×33, `Collider::heightfield` | gen riche débranchée (`heightmap_at` ≠ `_gen_ext`) | heightmap + érosion **bakée offline** (World Machine/Gaea/Houdini) |
| **1. Biomes + couleur** | ✅ | Voronoi 10 biomes, `amplitude_mult` data-driven (story-576), blend couleur 60m / forme 200m | `biome_at` **O(N seeds)** sans index spatial | splatmaps + règles altitude/pente |
| **2. Eau** | ⚠️ cosmétique | plan `bevy_water` à sea_level 4.0, sensor `forgia_water.json` ✅ | terrain **jamais sous sea_level** → eau **non-jouable**, **0 swim/underwater**, pas de rivières/lacs mesh | SSR + flow maps, rivières splines, volumes de nage |
| **3. Végétation** | ⚠️ partiel/figé | arbres seuls, Poisson, densité ÷20 **hardcodée**, imposters LOD2 (24 arbres+8 rochers/cluster) | **arbres UNIQUEMENT** (0 herbe/buisson/rocher/props procéduraux), génomes orphelins, **anti-stutter reverté → stutter de retour** | GPU instancing + density maps + billboards (Horizon PCG, UE5 PCG/Foliage) |
| **4. Routes** | 🔴 **doublon** | tuiles route hex KayKit (3 spokes) **+** ruban Bézier terrain (graphe legacy) **superposés** | deux représentations de route au même endroit | réseau de splines unique + decals |
| **5. Structures/villages** | 🔴 **DEUX systèmes actifs** | (A) village hex KayKit récent **+** (B) loader legacy spawne **6 bâtiments rouges** au **même centre (16,16)** | bâtiments imbriqués/doublés (2 tavernes, 2 puits…), colliders qui se chevauchent | WFC/grammaire/Houdini PCG, instancié |
| **6. Props/déco** | ⚠️ village-only | barils/caisses/rochers **uniquement dans le village** hand-crafted | **0 prop procédural** sur terrain ouvert | scatter par règles |
| **7. Ciel/skybox** | ✅ **le meilleur élément** | cubemap cartoon procédural **per-biome, data-driven, hot-reload** (`skybox_genome.rs`) | — | skybox stylisé OU sky physique (Hosek-Wilkie) |
| **8. Atmosphère/fog** | 🔴 **absent en RPG** | rien (fog n'existe qu'en Roguelite, hardcodé) | horizon dur, 0 profondeur, génome `atmosphere` orphelin | fog volumétrique + aerial perspective |
| **9. Lumière** | ⚠️ statique | 1 `DirectionalLight` figé, **0 ambient/tonemapping/bloom configuré** (defaults Bevy), **0 cycle jour/nuit** | tout hardcodé, 0 sensor, 0 gene consommé | CSM tuné + IBL ambient + TOD + tonemap/bloom |
| **10. Matériaux/textures** | 🔴 VRAM | 1 material terrain partagé (grass 1024², ~16MB) ✅ ; mais **toutes textures non compressées** | feature `ktx2` **ON mais 0 fichier .ktx2** → ~886MB VRAM ; **bark 2048×4096 ×3 = 128MB préchargé inconditionnellement au Startup** | virtual texturing + BCn/KTX2 + texture streaming |
| **11. LOD global** | ⚠️ bug | 3-tier cohérent (0-96 / 96-128 / 128-1500m), hystérèse, coverage sensor ✅ | **LOD2 SANS collider** → chute à travers le sol >128m ; heightfield **sans CollisionGroups** | Nanite/HLOD + proxies de collision |
| **12. Streaming/mémoire** | ⚠️ sync | dual-radii + LRU + hystérèse temporelle (pattern industrie) ✅ | **génération SYNCHRONE** (main thread → stutter burst), `async_queue_depth`=0 | World Partition **async** |
| **13. Observabilité** | ⚠️ trous | sensors streaming/LOD/water/VRAM/memory + F2 monitor ✅ | `pipeline_diag` **stub**, végé hors liveness, **pas de HealthAlert agrégée VRAM/RAM/terrain-zero** | télémétrie + budgets gardés |

Légende : ✅ solide · ⚠️ partiel/figé/dette · 🔴 bug ou redondance active.

---

## 🐞 Vrais bugs (pas juste « manquant »), classés

| # | Bug | Impact | Effort | Fichier |
|---|---|---|---|---|
| B1 | **Deux systèmes de village rendent au même centre** (hex KayKit + loader legacy 6 bâtiments rouges) | bâtiments imbriqués/doublés, colliders chevauchants | Faible (retirer `LoadVillageGenomeRequest` du RPG, `lib.rs:617`) | forgia-rpg/lib.rs |
| B2 | **LOD2 mega-tiles sans collider** | chute à travers le sol >128m (téléport/knockback/véhicule) | Faible | forgia-terrain/lod.rs:592 |
| B3 | **Anti-stutter foliage reverté** (story-583) → tous les chunks peuplés en 1 frame | stutter de chargement | Moyen (re-faire le budget CORRECTEMENT + valider) | forgia-foliage/lib.rs:223 |
| B4 | **Textures 100% non compressées** (ktx2 ON, 0 .ktx2) | ~886MB VRAM ; bark 128MB préchargé inconditionnel | Moyen (pipeline KTX2/Basis, ÷4-6) | assets + forgia-foliage/material_override.rs:142 |
| B5 | **Routes doublées** (tuiles hex + ruban Bézier superposés) | géométrie/coût redondants | Faible (en suivant B1) | forgia-rpg/lib.rs |
| B6 | heightfield terrain **sans CollisionGroups** (groupe Rapier par défaut) | incohérent vs G1-G5 | Faible | forgia-terrain/meshing_heightmap.rs:237 |
| B7 | `enforce_chunk_memory_budget` magic `remaining_mb -= 5.0` stale | éviction budget jamais déclenchée (inoffensif car cap chunks mord avant) | Trivial | forgia-rpg/lib.rs:1187 |

---

## 🎛️ Orphelins « cheap à brancher » (gros ROI visuel, code déjà là)

| # | Brancher… | Gain | Effort |
|---|---|---|---|
| O1 | Gen terrain riche sur `heightmap_at` (redistribution + micro-roughness + slope clamp) | relief RPG beaucoup plus expressif | Faible |
| O2 | Génomes végétation (`vegetation_default.toml` : densité/ratios/espèces/wind) + spawn des catégories Bush/Ground/Rock | monde « arbres ou rien » → vraie flore + herbe | Moyen |
| O3 | Génome atmosphère : fog RPG + (option) cycle jour/nuit | profondeur atmosphérique, ambiance variée | Faible (fog) / Moyen (TOD) |
| O4 | `forgia-postprocess` (bloom au minimum) + tonemapping/exposure RPG | rendu nettement plus « fini » | Faible |
| O5 | Porter `pipeline_diag` (au moins `forgia_terrain.json` minimal) | fermer l'angle mort observabilité (règle) | Moyen |

---

## ✅ Ce qui est solide (à NE PAS toucher)

1. **Skybox cartoon per-biome data-driven + hot-reload** (`skybox_genome.rs`) — modèle de propreté, le meilleur élément de la stack.
2. **Eau bien gated par mode** + anti-trap « scène noyée » + sensor T1 conforme (`forgia-water`, story-552).
3. **Streaming dual-radii + LRU + hystérèse temporelle/spatiale** + histogram gen_ms + health next_step — pattern industrie correct (sauf le sync).
4. **LOD 3-niveaux auto-protégé** (CHK-1/CHK-3 garde-fous régression, coverage rings `max_gap_frac`, skirt anti-z-fight).
5. **Material terrain unique partagé** (frugal, ~16MB) + VRAM sensor réel (`forgia2_vram.json` top offenders) + F2 monitor enfin branché.
6. **Tests denses** sur terrain/biomes/flatten/village hex (autotiler complétude, enceinte continue…).

---

## 🧭 Recommandations (pondérées SHIP = Roguelite, RPG = track FORGE)

**Principe** : le monde RPG **n'est pas ship-critique**. Ne pas viser le AAA. Mais (a) corriger les vrais bugs, (b) brancher les orphelins cheap quand le RPG sert d'outil/démo, (c) acter les systèmes morts.

- **P0 — vrais bugs, cheap, à faire quel que soit le track** :
  - **B1 trancher le doublon village** (décision produit : garder le hex récent → retirer le loader legacy du RPG + sa chaîne `forgia-village-generator`/`-kit`/`-genome-village`, ou l'inverse). **Aujourd'hui les deux tournent.**
  - **B2 collider LOD2** (anti-chute).
  - **B3 anti-stutter foliage** refait proprement + validé.
- **P1 — orphelins cheap, gros ROI visuel, si le RPG/monde sert** : O1 (gen riche), O3-fog, O4-bloom, B4 (KTX2 ÷4-6 VRAM).
- **P2 — différer sauf si le RPG devient prioritaire** : streaming async (B/anti-stutter profond), index spatial biome, variété procédurale villages (brancher la toolbox `forgia-worldgen` au lieu du hex hand-codé), eau interactive (swim), cycle jour/nuit complet.
- **NE PAS faire** : SDF/grottes en RPG (cf audit terrain), AAA lighting/atmosphère.

> **Bottom line** : le monde RPG V2 est une tranche fonctionnelle correcte assise sur un **gisement de richesse V1 débranché**. Ses vrais problèmes ne sont pas « il manque X » mais **(1) deux systèmes de village qui s'empilent, (2) 3 bugs de collision/VRAM/stutter, (3) des génomes orphelins**. Pour un moteur IA-natif, c'est même une bonne posture : la richesse est prête, l'IA la câble quand le besoin arrive. La priorité immédiate = **acter le doublon village + les 3 bugs**, le reste est de l'activation à la demande.

---

## Annexe — fichiers clés par couche

- **Terrain** : `forgia-terrain/src/generation/heightmap.rs` (`heightmap_at` vs `_gen_ext`), `meshing_heightmap.rs`, `lod.rs`, `pipeline_diag.rs` (STUB)
- **Végétation** : `forgia-foliage/src/lib.rs:197-535` (populate, sensor), `material_override.rs:142` (bark preload), `lod.rs:476+` (imposters)
- **Eau/ciel/lumière** : `forgia-water/src/lib.rs`, `forgia-player/src/skybox_genome.rs` (✅), `forgia-rpg/src/lib.rs:628-654` (soleil), `:1951-2080` (nuages), `forgia-postprocess` (orphelin)
- **Structures** : `forgia-rpg/src/worldgen_village.rs` (hex, système A), `forgia-rpg/src/lib.rs:617` (`LoadVillageGenomeRequest`, système B legacy), `forgia-village-loader`, `forgia-worldgen` (toolbox dormante)
- **Infra** : `forgia-streaming/src/lib.rs`, `forgia-rpg/src/lib.rs:684` (stream sync), `forgia-observability/src/{checks.rs,vram_sensor.rs}`, `forgia-debug` (F2)
- **Génomes orphelins** : `assets/genomes/biomes/{vegetation,grass}_default.toml`, `assets/genomes/atmosphere_default.toml`, `assets/genomes/sky/{clouds,wind}_default.toml`
