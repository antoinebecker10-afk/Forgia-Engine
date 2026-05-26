# Audit Maturité 360° — Crates Workspace V2 Forgia

> **Date** : 2026-05-19
> **Méthode** : 5 agents general-purpose parallèles, scan LOC/tests/sensors/genome par crate, benchmark industrie AAA 2026
> **Périmètre** : ~115 crates audités sur 258 total (les 143 autres = scaffolds 16 LOC réservés non-prioritaires)
> **Sources** : ~80 URLs vérifiables (GDC vault, dev blogs, paper academics)

---

## Synthèse exécutive

| Domaine | Crates prod | Crates partiels | Scaffolds bloquants | Industry gap top |
|---|---:|---:|---:|---|
| Combat / Weapons / VFX / Juice | 14/24 | 5 | 5 (armor, projectile, melee, vfx-decals, vfx-impact-library) | Sub-tick lag-comp (Valorant), damage type matrix (Hadès) |
| AI / Animation / Rig | 8/21 | 1 | 3 critiques (navmesh, animation-blend, perception) | Navmesh runtime (Recast/Detour), blend tree (UE5/Unity standard) |
| Networking / Steam / Persistence | 1/13 | 0 | 3 critiques M5 (scene, analytics, steam) | Sentry crash dump (P0 ship), lightyear_steam wrapper |
| Audio / UI / Accessibility | 4/15+ | 3 | **7 audio scaffolds bloquants story-468** | Voicelines bark system (Hadès/HoL), accessibility EAA juin 2025 |
| Terrain / Procgen / Assets / Genome | 18/36 | 9 | 11 (loot, equipment, status, skill-tree, crafting, stage-graph) | Stage-graph DAG roguelite (Slay the Spire) |

**Verdict global** : pipeline rig + asset CDN + terrain en niveau **AAA-équivalent**. Tout le reste pour V7 roguelite = **scaffolds 16 LOC à peupler** (cohérent règle fine-grained-crates).

**Découverte clé** : `forgia-mode-roguelite` est en réalité à **380 LOC + 13 tests + sensor `forgia2_roguelite_state.json` canonique V5** — l'autre terminal a livré M1 (story-470) avant cet audit. Plan original Phase M1 = ✅ DONE.

---

## 1. Bucket Combat / Weapons / VFX / Juice (24 crates)

### Top 5 scaffolds bloquants V7

| Crate | LOC | Effort | Bench industrie |
|---|---:|---|---|
| `forgia-armor` | 16 | 1-2j | Apex Evo Shield + OW2 armor mitigation |
| `forgia-weapon-projectile` | 16 | 3-5j | Halo projectile lead, Quake rocket arc |
| `forgia-vfx-impact-library` | 16 | 2-3j | Valorant surface tags (GDC 2021 Riot visual identity) |
| `forgia-weapon-melee` | 16 | 2-3j | Doom Eternal swing arc, For Honor capsule cast |
| `forgia-vfx-decals` | 16 | 2j | CS Source decal fade, Bevy decal RFC #3624 |

### Crates matures déjà OK pour V7

- `forgia-combat` (1502 LOC, 31 tests, sensor + genome) — dette : déduper avec `forgia-weapon-hitscan` (148 LOC parallèle)
- `forgia-damage` (282 LOC, 3 tests, genome `hit_feedback_tuning`) — manque damage type resist matrix
- `forgia-viewmodel` (1237 LOC, 11 tests) — manque sensor `forgia_viewmodel.json` ⚠
- `forgia-effects` (1322 LOC, 9 tests, sensor) — solide, dette : pre-warm dummy Hanabi à migrer vers `forgia-vfx-hanabi`
- `forgia-killfeed` (583 LOC, sensor + genome) — 0 tests sur logique multi-kill window
- `forgia-crosshair` (352 LOC) — 0 tests, 24 fields tuning fragile
- `forgia-enemy-nameplate` (435 LOC, sensor) — manque LOD frustum-cull 1000+ entités

### Industry gaps universels

1. **Sub-tick lag compensation** absent — Valorant patent USPTO 11,712,627 (2023)
2. **Surface-typed impact library** — Halo 5 audio surface tags GDC 2018, total miss Forgia
3. **Damage type / resist matrix** — Hadès Kasavin GDC 2021, Borderlands element vs flesh/armor
4. **Critical multiplier data-driven** — actuellement hardcoded path damage
5. **Pattern memorization spray** — CS2 fixed spray, différenciateur skill ceiling

### Sources clés

- [Valorant Peeker's Advantage patent](https://patents.google.com/patent/US11712627B2)
- [Riot tech blog 128-tick](https://technology.riotgames.com/news/valorants-128-tick-servers)
- [Hadès narrative Kasavin GDC 2021](https://www.gdcvault.com/play/1027332/)
- [Apex Evo Shield GameSpot](https://www.gamespot.com/articles/apex-legends-evo-shield-explained/1100-6500036/)

---

## 2. Bucket AI / Animation / Rig (21 crates)

### Maturité split AAA inversé

- **Pipeline rig 5/5 AAA+** : auto-rig 3425 LOC + 46 tests, rig-topology 509+6, skeleton-embedder 1222+11, mesh-voxelizer 446+10, medial-axis 531+7. **Forgia dépasse l'industrie open-source sur auto-rig data-driven** (Pinocchio Baran 2007 + locks/templates TOML).
- **AI runtime 1/10** : seul `forgia-ai-arena-bot` 845 LOC (sensor live). 9 scaffolds.
- **Animation 1/3** : `anim-debug` 643 LOC. `animation-blend` et `animation-mixamo` scaffolds.

### Bloquants V7 (Sem 1-3)

1. **`forgia-ai-navmesh` scaffold** → wrapper `oxidized_navigation` URGENT (bots aveugles aux walls dungeons)
2. **`forgia-animation-blend` scaffold** → wrapper Bevy 0.18 `AnimationGraph` natif (~400 LOC)
3. **`forgia-ai-perception` scaffold** → boss telegraphs et alertes

### Décisions architecturales recommandées V7

| Pattern | Choix | Justification |
|---|---|---|
| AI archi | **Utility AI + FSM simple** | GOAP/BT runtime = 2-3 sem solo, hors budget. 3 enemies + 1 boss = FSM 5 states/ennemi suffit |
| Animation | **BlendTree state machine** | Motion matching = 1000+ clips + cost function = Naughty Dog 8 anim/3 ans, hors budget |
| IK foot placement | **OBLIGATOIRE** roguelite terrain non-plat (~200 LOC + sensor, 1 sem) | 80% FPS modernes l'utilisent |
| Cloth cape boss | **SKIP MVP, secondary-motion Verlet 4-6 bones** | PBD cloth = 2-4 sem, bevy_silk alpha |

### Sources clés

- [F.E.A.R. GOAP Orkin GDC 2006](https://alumni.media.mit.edu/~jorkin/gdc2006_orkin_jeff_fear.pdf)
- [L4D AI Director Booth GDC 2009](https://www.gdcvault.com/play/1011902/)
- [Helldivers 2 difficulty postmortem](https://www.gamedeveloper.com/design/-i-helldivers-2-i---a-postmortem-on-difficulty-design)
- [Bevy big-brain Utility AI](https://github.com/zkat/big-brain)
- [oxidized_navigation crate](https://github.com/TrialDragon/oxidized_navigation)
- [Bevy 0.18 AnimationGraph](https://docs.rs/bevy/0.18/bevy/animation/graph/struct.AnimationGraph.html)

---

## 3. Bucket Networking / Steam / Persistence (13 crates)

### État brutal

**12/13 scaffolds 16 LOC**. Seul `forgia-app-state` (thin alias) compile useful code. Bucket **15% productive**.

### Pour démo Next Fest solo-only (M5 redéfini post-deep-audit)

| Crate | Verdict | Priorité |
|---|---|---|
| `forgia-scene` (saves) | **CRITIQUE M5** | P0 — RON+serde_flow versionned, MetaProgression + RunState |
| `forgia-analytics` (Sentry) | **P0** | Crash dump opt-in, panic_hook, 1j max |
| `forgia-steam` (achievements) | **PARTIEL M5** | Permettre publication Next Fest |
| `forgia-net-lightyear` | REPORT | Démo solo-only, coop reporté |
| `forgia-net-lobby` | REPORT | Idem |
| 8 autres (chat/voice/match/rollback/AC/etc.) | SKIP MVP | Catalog réservé OK |

### Sources clés

- [Sentry Rust SDK](https://docs.sentry.io/platforms/rust/)
- [lightyear_steam officiel](https://github.com/cBournhonesque/lightyear/tree/main/lightyear_transport_steam)
- [Steam Cloud partner doc](https://partner.steamgames.com/doc/features/cloud)
- [bevy_save](https://crates.io/crates/bevy_save) vs roll-your-own RON+serde_flow

---

## 4. Bucket Audio / UI / Accessibility (15+ crates)

### Single point of failure ship V7 : AUDIO

**1/8 audio crates productive** (`audio-biome` 126 LOC mais hardcode `BiomeType→ogg` violation `no-hardcode.md`). **7 scaffolds bloquants story-468**.

### Critiques M2-M4 (à peupler obligatoirement)

1. **`forgia-audio-voicelines`** — bark system 4 armes × 24 lignes. Sans ça, "armes parlantes" = mensonge marketing. Pattern Hadès Kasavin GDC 2021 + High on Life Kibler.
2. **`forgia-audio-music-state`** — adaptive Explore→Combat→Boss. 5 OGG playlist V1 déjà dispos `assets/audio-v1/music/`.
3. **`forgia-audio-ducking`** — voiceline trigger music -6dB sinon barks inaudibles.
4. **`forgia-audio-footsteps`** — minimum 2 surfaces (forge_floor / arena_sand) pour FPS feel.

### UI maturité OK

- `forgia-ui-hud-ammo` (686 LOC, 3 fichiers, sensor + genome) — **MATURE**, modèle à dupliquer
- `forgia-ui-pause-menu` (373 LOC, persistance user_settings.toml) — manque **run summary screen** (RoR2 stats / Hadès death recap)
- `forgia-ui-minimap` scaffold — OPTIONNEL roguelite arena solo

### Accessibility 0% — risque EAA juin 2025

**Aucun crate `forgia-a11y-*`**. Recommandation 2 crates à créer :

1. `forgia-a11y-captions` (~150 LOC) — subtitles ≥46px Xbox A11y guideline 104, speaker name color-coded
2. `forgia-a11y-input-remap` — extension UI Settings panel keybinds (KeybindRegistry existe)

### Sources clés

- [Hadès dialogue Kasavin GDC 2021](https://www.gdcvault.com/play/1027141/)
- [High on Life narrative director Kibler](https://gamerant.com/high-on-life-talking-guns-comedy-game-design-tutorials/)
- [Game Accessibility Guidelines](https://gameaccessibilityguidelines.com/)
- [Xbox A11y 104 captions](https://learn.microsoft.com/en-us/gaming/accessibility/guidelines/104)
- [EAA Bird & Bird gaming impact](https://www.twobirds.com/en/insights/2026/the-impact-of-the-european-accessibility-act-on-online-gaming-and-gaming-devices)

---

## 5. Bucket Terrain / Procgen / Assets / Genome (36 crates)

### Maturité 18 prod / 9 partiel / 11 scaffold

### Niveau AAA-équivalent atteint

- **`forgia-terrain` 7376 LOC, 27 fichiers, 151 tests** — heightmap + SDF caves + biome Voronoi + meshing surface-nets. AAA-grade.
- **Asset pipeline complet** : `assets-bundle` (570) + `asset-cdn` (741+9 tests) + `asset-registry` (969+13 tests). SHA256 pinning, NeedsAssetCalibrate auto-scale. Niveau Unity Addressables / UE5 Asset Manager.
- **`forgia-rpg` 3606 LOC + 21 tests** et `forgia-fps` 1592+17 tests = jeux V1/V2 stables.
- **`forgia-mode-roguelite` 380 LOC + 13 tests + sensor canonique 1Hz** ← M1 V7 déjà livré (story-470)

### Bloquants gameplay V7 M2+

| Crate scaffold | Bloque | Référence industrie |
|---|---|---|
| `forgia-loot-tables` | Drop pools roguelite | Diablo 3 Loot 2.0 |
| `forgia-equipment` | 2 slots arme primaire/accessoire | Standard ARPG |
| `forgia-status-effects` | Burn/slow/stun/poison | Hadès elemental |
| `forgia-skill-tree` | Méta-progression | Hadès Mirror |
| `forgia-stage-graph` (À CRÉER) | StageGraph DAG roguelite | Slay the Spire 2-phase RNG |
| `forgia-crafting` | Optionnel POST-MVP | — |

### Dette identifiée

1. **`forgia-water` hardcode `SEA_LEVEL=4.0`** — exactement bug "Arena sous l'eau" 2026-05-12. Devrait être genome ou consommer `MapGenConfig.sea_level`.
2. **`forgia-genome-{registry,validator}` scaffolds** = dette orchestration cross-crate. 5+ consommateurs réinventent `init_asset::<Genome<T>>`. **À peupler AVANT migration V1→V2 wholesale** (sinon dette explose).
3. **`forgia-inventory` 162 LOC data-only sans Plugin** — régression d'intégration vs V1 LOCK-INV-1 80-slot.
4. **`forgia-camera-fps` vs `forgia-player::FpsCamera`** — duplication morte. Extraire pour symétrie avec `camera-orbit` 3P.
5. **`forgia-procgen-graph` domain mismatch** — 290 LOC dédiés villages 2D (RoadSegment2D), PAS stage-graph roguelite. Créer `forgia-stage-graph` séparé.
6. **Sensor trou noir** : foliage (483 LOC), streaming (876), water (58), inventory (162) = 0 sensor JSON malgré règle `observability-required.md`.
7. **Tests 0 sur 7 crates** terrain/village (streaming, foliage, water, village-kit, village-loader, village-generator, genome-village = ~3700 LOC sans validation).
8. **Genome migration V1→V2 ~10%** : 51 TOML V1, seulement 3+ V2 (debug_anim, rpg_monitor, streaming).

### Réponses ciblées story-468

- **Crates terrain over-engineered pour V7 arena compact** : skip streaming/foliage/water/village-* pour M1. Réutiliser `forgia-terrain` heightmap_at + Collider::heightfield direct.
- **`forgia-procgen-graph` PAS utilisable** stage-graph → créer `forgia-stage-graph`
- **`forgia-mode-roguelite`** = 380 LOC + 13 tests + sensor canonique ✅ M1 LIVRÉ
- **Genome-registry + validator** = bloquant si migration TOML massive

### Sources clés

- [UE5 World Partition](https://dev.epicgames.com/documentation/en-us/unreal-engine/world-partition-in-unreal-engine)
- [Surface Nets Bracegirdle 2017](https://transvoxel.org/Lengyel-VoxelTerrain.pdf)
- [Slay the Spire 2-phase RNG path](https://forgottenarbiter.github.io/Correlated-Randomness/)
- [Spelunky procgen Yu GDC 2009](https://www.gdcvault.com/play/1015908)
- [No Man's Sky Murray GDC 2017](https://gdcvault.com/play/1023976)
- [Anno DevBlog city gen](https://www.anno-union.com)

---

## Top 15 actions priorisées (impact × effort × ship V7)

### P0 — Bloquants démo Next Fest V7 (Sem 1-3)

1. **`forgia-audio-voicelines`** scaffold→prod — sans ça "armes parlantes" = mensonge (3-4j)
2. **`forgia-audio-music-state` + `forgia-audio-ducking`** ensemble — adaptive Explore/Combat/Boss + sidechain (2-3j)
3. **`forgia-loot-tables`** scaffold→prod — bloque M2 gameplay loop (1-2j, Diablo 3 Loot 2.0)
4. **`forgia-equipment`** scaffold→prod — 2 slots primaire/accessoire (1j)
5. **`forgia-status-effects`** scaffold→prod — burn/slow/stun (1-2j)
6. **`forgia-stage-graph`** NEW crate — DAG progression roguelite Slay-the-Spire-like (2-3j)
7. **`forgia-scene`** scaffold→prod — saves RON+serde_flow versionned (2-3j)
8. **`forgia-analytics`** scaffold→prod minimal — Sentry crash dump P0 ship (1j)
9. **`forgia-ai-navmesh`** scaffold→prod via `oxidized_navigation` wrapper (3-5j)
10. **`forgia-animation-blend`** scaffold→prod — Bevy 0.18 AnimationGraph wrapper (~400 LOC, 3-4j)

### P1 — Quality ship 2026 (Sem 4-8)

11. **`forgia-a11y-captions`** NEW — subtitles obligatoires EAA juin 2025 (2j)
12. **`forgia-audio-footsteps`** scaffold→prod minimal 2 surfaces (1j)
13. **`forgia-armor`** scaffold→prod — Apex layered shield+armor+HP (1-2j)
14. **`forgia-weapon-projectile`** scaffold→prod — débloquer rockets/grenades (3-5j)
15. **Run summary screen** — extension `forgia-ui-pause-menu` (1j, RoR2 stats / Hadès recap)

### P2 — Quality + dette (Post Next Fest)

- Peupler `forgia-genome-registry` + `forgia-genome-validator` avant migration V1→V2
- Gene-iser `forgia-water::SEA_LEVEL`
- Activer `forgia-inventory` Plugin (régression vs V1)
- Ajouter sensors `forgia_foliage.json`, `forgia_streaming.json`, `forgia_water.json`
- Tests `forgia-village-generator` (820 LOC sans test)
- Damage type resist matrix dans `forgia-damage`
- Migrer pre-warm Hanabi `forgia-effects` → `forgia-vfx-hanabi`
- `forgia-vfx-impact-library` + `forgia-vfx-decals` surface-typed
- Pattern memorization spray dans `forgia-juice-recoil`
- Sensor `forgia_viewmodel.json` (ADS state visible)

---

## Conclusions

1. **L'autre terminal a déjà livré M1** (`forgia-mode-roguelite` 380 LOC + sensor + tests). Plan V7 démarrage = OK.
2. **Bottleneck V7 = AUDIO + LOOT/EQUIPMENT/STATUS scaffolds** (~10 jours dev cumulés, 8 crates).
3. **Pipeline rig + asset CDN + terrain dépassent niveau AAA** — surdimensionné pour V7 arena compact mais utilisable tel quel.
4. **Networking + Steam quasi-vide** mais OK car M5 reporté post Next Fest (démo solo-only).
5. **Accessibility 0%** = risque EAA juin 2025 légal + Steam tag manquant. À combler avant ship.
6. **Genome migration V1→V2 à 10%** = dette transverse à clarifier (peupler registry/validator AVANT migration massive).
7. **3 duplications mortes à résoudre** : combat::weapons vs weapon-hitscan, camera-fps scaffold vs player::FpsCamera, inventory vs équipement V1.

Plan V7 reste exécutable dans budget 12 sem **uniquement si scope discipliné** : 10 P0 actions = ~25 jours dev = ~5 sem si AI assist gain réel +15% (Octoverse 2025). Buffer 30% = 6.5 sem occupées sur 12 disponibles.
