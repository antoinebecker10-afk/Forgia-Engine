# Audit — Qualité de rendu : Arène vs Parcours + leviers internet

> **Date** : 2026-06-26
> **Déclencheur** : constat user — *« le parcours est de meilleure qualité que l'arène »*.
> **Mission** : comprendre **structurellement pourquoi**, croiser avec la bible art-direction
> et les best-practices industrie/Bevy, et produire un plan d'amélioration **sans rien casser**.
> **Sources de vérité** : (1) le code lu, (2) `docs/lore/locations/crypts_of_anvil.md`, (3) recherche web (sources §7).

---

## 0. Verdict en une phrase

**Le parcours est *authored* (un artiste a placé 1216 pièces avec intention) ; l'arène est un *scatter procédural* (le code lance ~18 props en fléchettes sur un disque).** Le rendu post-process de l'arène est en fait *correct* — le déficit est de **composition, densité et cohérence d'art**, pas d'effets d'écran.

---

## 1. Les deux modèles, côte à côte

| Fichier | Nature | Preuve |
|---|---|---|
| `assets/models/environment/platformer/one_file_assets.glb` | Banque de **107 pièces atomiques** (1 node = 1 mesh) | dump GLB |
| `platformer_underworld.glb` (parcours) | **Niveau assemblé** : 1216 instances de 92 meshes | dump GLB |
| `forgia-stage::spawn_stage_arena_on_request` | **Génération runtime** : sol tuilé + ramparts hex + scatter | `crates/forgia-stage/src/lib.rs:747` |

Le parcours **est** un assemblage authored des pièces de `one_file_assets.glb`. L'arène ne fait aucun assemblage authored — elle compose à la volée.

---

## 2. Pourquoi le parcours gagne — 5 axes sourcés (code + data)

| Axe | Parcours (authored) | Arène (procédural) | Preuve |
|---|---|---|---|
| **Composition** | Layout intentionnel : chemins, gating, flow, séquence de zones | Fléchettes sur un cercle, **aucune section** | `poi_anchor_positions` lib.rs:593 + `place_modules` |
| **Densité** | 1216 instances, déborde de détail | ~0.0014 props/m² — **~36× moins dense que Hadès** (aveu dans la data) | `roguelite_stages.toml:39` |
| **Cohérence d'art** | 1 seul kit homogène | KayKit dungeon (sol/murs) **+** Inferno (props) **mélangés** | stage/lib.rs:829 vs `level_modules.toml:38` |
| **Verticalité** | Plateformes, escaliers, hooks, multi-niveaux | Disque plat + **1** tour (sniper_perch) | floor tiles plats + 1 `TowerBig` |
| **Texture narrative** | C'est un *lieu* | C'est *un sol avec des trucs dessus* | — |

---

## 3. Écart bible ↔ implémentation (le cœur)

La bible `crypts_of_anvil.md` décrit une arène **authored à 6 sections** avec props signature. **Rien de tout ça n'existe dans le code.**

| Section bible | Props clés demandés | État code |
|---|---|---|
| L'entrée | grande porte rouillée, cages vides, banderoles | ❌ inexistant |
| Cour des forges éteintes | 4-6 enclumes en demi-cercle, marteau planté | ❌ inexistant |
| Perchoir du contremaître | échelle, chaise renversée, écriteau | 🟡 `sniper_perch` = 1 `TowerBig` générique |
| Fosse à mêlée | sol rond, 4 cages renversées, gradins | 🟡 `melee_pit` = `CirclePlatform` + brasero |
| Couloir des graffiti | murs d'écriteaux gravés (lore drop) | ❌ inexistant |
| Halle du boss | enclume géante, trône en boucliers fondus | 🟡 BossPad = prefab générique |

**Props signature manquants** (l'identité « sinistre digestible » Cult of the Lamb) : lampions roses, banderoles rongées, **écriteaux/graffiti gravés**, **champignons lumineux** bleus/jaunes/roses, **cages en métal tordu**. Palette cible bible = *rouge braise + noir suie + **rose pastel** + bleu champignon* — le rose absent du rendu actuel.

**Constat clé** : les docs de la crate stage **citaient déjà** Returnal *« handcrafted + procedural »* et RoR2 *« pré-bâti + objets randomisés »* (`stage/lib.rs:27-29`). **L'intention était hybride** ; l'implémentation est partie 100 % procédurale et a perdu la moitié authored. Le parcours prouve que la moitié authored marche.

---

## 4. État réel du rendu (ce qui est CÂBLÉ — vérifié en code)

Le post-process n'est **pas** le problème principal : l'arsenal est riche et largement actif.

| Levier | État roguelite | Source |
|---|---|---|
| **Brume volcanique** (`DistanceFog` rouge-orangé expo, density 0.008) | ✅ actif sur FpsCamera | `atmosphere.rs:31` |
| **Ambiante chaude** (`AmbientLight` orange sombre, brightness 300) | ✅ actif | `atmosphere.rs:42` |
| **Toon shading** (`ForgiaPpToonPlugin`) | ✅ actif | `lib.rs:166` |
| **Outline** (`ForgiaPpOutlinePlugin`) | ❌ **DÉSACTIVÉ** (conflit render-graph node_edges avec Toon → crash surface texture) | `lib.rs:175-176` |
| **Key + Fill lighting** biome-tuné + **ClearColor** sky | ✅ actif (arène) | `stage/lib.rs:1134` |
| **Glow d'ambiance** (PointLight chaud par POI/boss, clamp 8k) | ✅ actif | `stage/lib.rs:978` |
| **Bloom / HDR** | ✅ utilisé ailleurs (cyber city, viewmodel) | mémoire `reference_cyber_city_render` |
| **SSAO / ambient occlusion contact** | ❌ non câblé | — |
| **Atmosphere 0.18** (procédurale + occlusion) / **ScatteringMedium** | ❌ non utilisé (fog plat global à la place) | Bevy 0.18 §7 |

**Lecture** : l'arène a déjà fog + ambient + toon + key/fill + glow. Les gaps rendu sont ciblés : **(a) outline OFF**, **(b) pas de SSAO** (props scatterés « flottent » sans ombre de contact), **(c) fog plat** au lieu de l'atmosphère volumétrique 0.18.

---

## 5. Recherche internet — synthèse actionnable

### 5.1 Returnal — *Never The Same Twice* (GDC 2022, Housemarque)
- **Tout est handcrafted** ; à chaque run, le jeu **connecte** procéduralement des espaces pré-conçus.
- Chaque biome = série d'**areas pré-designées séparées par des portes** ; après chaque mort, sélection + shuffle de **rooms pré-faites** assemblées différemment.
- Le **contenu** d'une area (ennemis, loot) est spawné procéduralement ; la **structure** est authored.
- → **Modèle cible Forgia** : coquille authored par biome + overlay procédural fin (cover/loot/spawns).

### 5.2 Cult of the Lamb (référence explicite de la bible)
- **La lumière fait le look** : solution de lighting custom, fortement travaillée — *« ça a élevé le jeu massivement »*.
- **Palette riche et profonde** (rouges/violets/terreux) **+** formes simples arrondies + **gros yeux expressifs** = juxtaposition mignon/sinistre.
- Recette = **contraste cute ↔ dark** assumé. (Pile la bible : *« triste de loin, mignon dans les détails »*.)
- → Pour Forgia : **toon + outline** (signature graphique), palette braise+rose pastel, emissive généreux (champignons/feux/lave).

### 5.3 The Level Design Book — Composition
- **Hiérarchie spatiale par contraste local** : *« une chose haute n'est spéciale que entourée de choses basses »*. Jouer hauteur / densité / orientation / forme.
- **Landmarks** = points mémorables qui « tirent » la composition ; mais ils doivent être **pertinents/utiles**, pas du set-dressing aléatoire (← exactement le défaut du scatter actuel).
- **Sightlines** (vistas + approaches) > « leading lines » : ménager des vues vers la zone suivante.
- **Set dressing subordonné au hero asset** ; composer **autour d'un point focal**.
- → L'arène n'a ni point focal, ni hiérarchie, ni section : le scatter uniforme tue la composition.

### 5.4 Bevy 0.18 — leviers de rendu disponibles
- **Atmosphere Occlusion + PBR** : l'atmosphère procédurale affecte la lumière reçue par les objets (cohérence volumétrique, halo soleil horizon) — marche avec volumetric fog.
- **`ScatteringMedium`** (asset) : scattering atmosphérique custom (absorption/scattering/phase) → **brume volcanique + ashfall avec profondeur** au lieu du `DistanceFog` plat.
- **`FullscreenMaterial`** (trait) : post-process custom simplifié + contrôle ordre render-graph → **piste pour résoudre le conflit Toon↔Outline**.
- **Fix PBR specular** (point/area lights) : matériaux moins « plastique ».
- SSAO / bloom / tonemapping / DoF existent depuis des versions antérieures (pas nouveaux en 0.18) — **dispo, juste pas câblés sur l'arène**.

---

## 6. Plan d'amélioration arène — par impact (ne rien casser)

> Principe : **ajouter une couche authored** par-dessus le procédural qui marche (modèle Returnal), pas réécrire le procédural.

### 🟢 Tier 1 — Coquille authored (le vrai saut de qualité)
- **T1.1** Une **coquille GLB authored par biome** (`crypts_of_anvil`, `forge_sanctum`) : la géométrie structurante + les props signature de la bible placés à la main. Assemblée depuis le même kit atomique que le parcours (réutilise `one_file_assets.glb` + Inferno).
- **T1.2** Le procédural devient **overlay fin** : seulement variation run-to-run (positions cover, loot, spawns ennemis). Structure = authored. (Returnal/RoR2.)
- **Effort** : élevé · **Risque** : moyen (nouvelle couche, le procédural reste en fallback) · **Impact** : ★★★★★

### 🟢 Tier 2 — Identité bible (cohérence + narration)
- **T2.1** Modéliser/sourcer les **props signature manquants** : lampions roses, banderoles, **écriteaux graffiti**, **champignons emissive**, **cages tordues**.
- **T2.2** **Palette rose pastel + bleu champignon** dans la color-grading roguelite (le rose = secret de la bible).
- **T2.3** Sections nommées de la bible (entrée / cour forges / couloir graffiti) comme zones de la coquille T1.
- **Effort** : moyen · **Risque** : faible · **Impact** : ★★★★

### 🟡 Tier 3 — Densité + verticalité (composition)
- **T3.1** Monter la densité du scatter vers la cible Hadès (~0.05/m²) **autour de points focaux**, pas uniformément (Level Design Book : contraste local).
- **T3.2** Vraie verticalité : gradins, perchoir walkable, fosse en contrebas (la bible les demande, le disque plat ne les a pas).
- **Effort** : faible-moyen · **Risque** : faible (data TOML) · **Impact** : ★★★

### 🟡 Tier 4 — Leviers rendu Bevy
- **T4.1** **Ré-activer l'outline** (signature Cult of the Lamb) via `FullscreenMaterial` 0.18 pour résoudre le conflit node_edges Toon↔Outline.
- **T4.2** **SSAO** sur la caméra roguelite → ancre les props scatterés (fin du « flottement »).
- **T4.3** Migrer `DistanceFog` plat → **`ScatteringMedium`/Atmosphere 0.18** pour la brume volcanique + ashfall volumétrique.
- **Effort** : moyen (T4.1 a un historique de crash) · **Risque** : moyen · **Impact** : ★★★ (T4.1 fort stylistiquement)

---

## 7. Sources

**Code / data Forgia** : `forgia-stage/src/lib.rs`, `forgia-mode-roguelite/src/{atmosphere.rs,loot_room.rs,lib.rs}`, `forgia-anchor`, `forgia-prefab`, `assets/genomes/{roguelite_stages,level_modules}.toml`, `docs/lore/locations/crypts_of_anvil.md`, dumps GLB `one_file_assets.glb` / `platformer_underworld.glb`.

**Web** :
- [Returnal — *Never The Same Twice* (GDC Vault)](https://www.gdcvault.com/play/1027651/Never-The-Same-Twice-Procedural) · [slides PDF](https://ubm-twvideo01.s3.amazonaws.com/o1/vault/GDC+2022/Speaker+Slides/Never+The+Same_Watson_Ethan.pdf)
- [Returnal — making-of (Game Developer)](https://www.gamedeveloper.com/marketing/a-third-person-action-roguelike-bullet-hell-arcade-thriller-the-making-of-returnal)
- [Cult of the Lamb — recette (Unity Blog)](https://unity.com/blog/games/recipe-behind-smash-hit-cult-of-the-lamb) · [art director (Inverse)](https://www.inverse.com/gaming/cult-of-the-lamb-concept-art-interview-massive-monster/amp) · [cute → dark (Game Developer)](https://www.gamedeveloper.com/design/interview-corralling-the-inherent-cuteness-of-cult-of-the-lamb)
- [The Level Design Book — Composition](https://book.leveldesignbook.com/process/blockout/massing/composition) · [Composition in Level Design (Game Developer)](https://www.gamedeveloper.com/design/composition-in-level-design)
- [Bevy 0.18 release notes](https://bevy.org/news/bevy-0-18/) · [HDR & Tonemapping (Bevy Cheatbook)](https://bevy-cheatbook.github.io/graphics/hdr-tonemap.html)

---

*Audit lecture-seule. Aucune ligne de code modifiée. Prochaine étape proposée : story + plan d'implémentation Tier 1 (coquille authored), modèle Returnal hybride.*
