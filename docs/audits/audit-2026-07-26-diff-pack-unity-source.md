# Diff avec le pack Unity source — ce qui manque, nommément (2026-07-26)

Pack : `tidal_flask_fantastic_highlands_castle_all.unitypackage` (3,85 Go,
poupées russes → 4 paquets internes ; analyse sur la variante **URP**, celle de la
capture du créateur). Scène : `demoscene_highlands_castle_level.unity` (23,2 Mo).

---

## 0. La bonne nouvelle : l'import géométrique est FIDÈLE

Comptage par prefab dans sa scène, comparé à notre export :

| Pièce | Lui | Nous |
|---|---|---|
| `floor_castle` | 818 | 818 |
| `wall_trim_castle` | 556 | 556 |
| `wall_big_castle` | 334 | 334 |
| `wall_small_castle` | 319 | 319 |
| `wall_deco_pillar_castle` | 305 | 305 |
| … 166 autres types | … | … |

**Zéro écart de comptage sur les 171 types communs.** Sur 8 102 instances de
prefab, la reconstruction en a rendu 7 911 à l'instance près.

---

## 1. Ce qui manque : 10 types, 191 instances

| Instances | Prefab | Nature |
|---|---|---|
| **67** | `P_PROP_candle_castle_05_lit` | bougie **allumée** (flamme incluse) |
| **42** | `P_PROP_flag_castle_02_static` | **bannière murale** |
| 36 | `P_FX_fog_castle` | brume volumétrique |
| 16 | `P_FX_particles_castle` | **poussières en suspension** |
| **11** | `P_PROP_candle_castle_04_lit` | bougie allumée |
| **8** | `P_PROP_flag_castle_03_static` | bannière murale |
| 6 | `P_FX_falling_leaves_castle` | feuilles qui tombent |
| 2 | `P_FX_wind_trail_comp_castle` | traînées de vent |
| 2 | `P_MOD_door_huge_comp_castle` | **grande porte** |
| 1 | `P_FX_particle_godray_castle` | rais de lumière |

### La cause racine est nette

**Les 10 types absents portent tous un suffixe** : `_lit`, `_static`, `_comp`. Ce
sont des prefabs **composites ou variantes** — ils n'enveloppent pas un unique FBX
comme les 171 autres. La reconstruction ne traitait que les prefabs à
correspondance 1:1 avec un fichier FBX ; tout le reste est tombé en silence.

### Deux conséquences que je dois corriger dans mes conclusions précédentes

**Les bannières manquent bel et bien.** Je n'avais pas pu conclure de ta capture
(cadrage). C'est confirmé : **50 bannières murales** (`flag_02_static` ×42,
`flag_03_static` ×8) absentes. Ce sont les tentures rouge sombre à galon doré de
sa photo.

**Mes flammes sont posées sur les mauvaises bougies.** Sa salle contient 51 bougies
type 05 **éteintes** *et* 67 type 05 **allumées** — deux prefabs distincts. Nous
n'avons que les éteintes ; les 78 qui brillent chez lui **n'existent pas du tout
dans notre scène, position comprise**. J'ai donc allumé les bougies qu'il laisse
éteintes, et les siennes sont absentes. Ça marche visuellement, mais ce n'est pas
son placement.

---

## 2. L'éclairage : 57 lumières placées à la main

| Type | Nombre | Chez nous |
|---|---|---|
| **Point** | 43 | 0 (avant), 24 auto (après correctif B) |
| **Spot** | 13 | 0 |
| **Directional** | 1, intensité **0,7**, avec **cookie** | 2, à **20 000** et **7 000** lux |

Ses lumières ponctuelles vont de 0,4 à 10 d'intensité, portée 2,8 à 19,5 m. Et
surtout, **elles ne sont pas toutes chaudes** : à côté des oranges (1,0 · 0,85 ·
0,29) il place des **cyans** (0,56 · 1,0 · 1,0) en lumière d'appoint. C'est ce
couple chaud/froid qui donne le rendu de sa photo.

Ses spots montent à 45-59 d'intensité sur 31-35 m de portée : ce sont les puits de
lumière par les fenêtres.

**Ce n'est pas dérivé, c'est composé.** 43 lumières pour ~300 bougies : il éclaire
des *zones*, pas des objets. Mon correctif B (une lumière sur les 24 bougies les
plus proches) est une approximation automatique raisonnable, mais ce n'est pas la
même démarche. **Bonne nouvelle : ces 57 lumières sont extractibles** — leurs
valeurs sont dans le YAML de la scène, leurs positions aussi.

---

## 3. Les trois piliers de rendu qu'on n'a pas du tout

### 11 lightmaps + 11 shadowmasks
`Lightmap-0..10_comp_light.exr` (4096 max) + `LightingData.asset`. C'est
l'illumination indirecte cuite : le rebond du tapis rouge sur les voûtes, les
ombres de contact sous chaque marche.

### 31 sondes de réflexion
`ReflectionProbe-0..31.exr`. Elles donnent à la pierre son environnement spéculaire.
Sans elles, une surface PBR n'a **rien à réfléchir** : elle paraît mate et plastique.
Je n'avais pas identifié ce point — il compte autant que le reste.

### Le profil de post-traitement — valeurs exactes

`Global Volume Profile World Castle.asset` :

| Effet | Valeurs |
|---|---|
| **Bloom** | seuil 0,5 · intensité **2** · clamp 2,5 · teinte chaude (1 · 0,96 · 0,67) |
| **ColorAdjustments** | postExposure **+1,8** · contraste **+10** · saturation **+7,4** |
| **ShadowsMidtonesHighlights** | ombres (0,767 · 0,808 · **1,0**) → **poussées vers le bleu** |
| **Vignette** | couleur bleu nuit (0,037 · 0,039 · 0,160) · intensité 0,306 |
| **WhiteBalance** | température **+14,9** (chaud) |
| **LiftGammaGain** | gain (0,994 · 1,0 · 0,949) — retire du bleu dans les hautes lumières |

Le mécanisme est explicite : **ombres bleues + hautes lumières chaudes**, forte
saturation, et un bloom généreux qui fait « baver » les flammes. J'avais deviné la
direction pour le correctif C' ; voici les vraies valeurs, nettement plus poussées
que les miennes (saturation +7,4 contre +0,18, contraste +10 contre +0,16 — les
échelles diffèrent entre URP et Bevy, mais l'intention est bien plus marquée).

### Brouillard et ambiante de scène

`RenderSettings` : brouillard **linéaire cyan clair** (0,354 · 0,870 · 1,0) de 20 m
à 8 000 m. Ambiante en **dégradé** ciel bleu-gris (0,414 · 0,487 · 0,632) → sol
quasi noir (0,047 · 0,043 · 0,035), intensité 0,85. Un dégradé sol/ciel, pas une
constante : le bas des volumes est naturellement plus sombre. Bevy n'a qu'une
ambiante uniforme — d'où une partie de notre aplatissement.

---

## 4. Les assets de flamme existent, tout faits

Le pack livre une flamme authorée que je n'ai pas utilisée (je l'ignorais) :

- `SM_FX_plane_fire_castle.fbx` — le plan de flamme
- `M_FX_fire_castle.mat` + `S_fire_URP.shader` — shader de feu animé
- `T_FX_fire_mask_castle.png`, `T_FX_fire_noise_castle.png` — masque + bruit
- `T_FX_glow_castle.png`, `T_FX_light_cookie_castle.png` — halo et **cookie de
  lumière** (le motif projeté par sa directionnelle)

Ma flamme est une sphère émissive procédurale. La sienne est un plan texturé animé
par un shader dédié. Le sien sera meilleur, et il est déjà là.

---

## 5. Plan, chiffré

| # | Action | Gain | Effort |
|---|---|---|---|
| ~~**G**~~ | ~~Porter ses 57 lumières~~ | ✅ **FAIT** (§6) | — |
| ~~**H**~~ | ~~Valeurs exactes de post-traitement~~ | ✅ **FAIT** (§6) | — |
| **I** | **Récupérer les 191 instances manquantes** (50 bannières, 78 bougies allumées, 61 FX, 2 portes) depuis les transformations de la scène | Contenu réellement absent, dont les bannières | ~4 h (dépend du re-bake des meshes composites) |
| **J** | **Flamme authorée** (plan + shader de feu du pack) à la place de ma sphère | Flamme crédible et animée | ~2 h |
| **K** | **31 sondes de réflexion** → environment map Bevy | Rend le spéculaire de la pierre | ~3 h |
| **F** | **11 lightmaps** → composant `Lightmap` Bevy (UV2 déjà présent) | Le rebond coloré, le vrai rendu | 1-2 j |

**Recommandation : G puis H.** Ensemble ~4 h, et ce sont eux qui ferment le gros
de l'écart — parce que le sujet, depuis le début, c'est la lumière. **I** ensuite
pour le contenu manquant. **F** reste le chantier de fond, à décider séparément.


---

## 6. G + H — appliqués (2026-07-26)

### La conversion de repère : déduite, pas devinée

C'était le point qui pouvait tout faire rater. Plutôt que de reconstituer le
pipeline d'import (absent du dépôt), la transformation a été **déduite par
appariement** puis vérifiée :

1. Résolution des positions **monde** Unity (remontée de `m_Father` sur 4 niveaux).
2. Appariement avec les mêmes pièces dans nos cellules glTF.
3. Ajustement sur 3 pièces uniques (pont, porte de ville, trône) →
   **écart 0,0000 m sur les trois axes**.

```
bevy = ( −unity.x − 32,041 ,  unity.y − 172,671 ,  unity.z + 35,372 )
```

Le déterminant est **négatif** : le château porte bien un miroir sur X. Cette
mesure le confirme indépendamment de ce qui avait été constaté sur le terrain.

**Validation à grande échelle** : les positions Unity de 7 types de pièces
converties puis comparées aux nôtres — **écart médian 0,000 m sur 1 195 pièces**
(818 sols, 156 escaliers, 95 chandeliers, 65 chaises, 30 tables, 18 vases,
13 statues).

La cloche est le seul écart résiduel (15,7 m en Y, X et Z exacts) : c'est son
décalage interne dans le beffroi, non résolu — écartée de l'ajustement.

### G — 56 lumières portées

`tools/unity/extract_scene_lights.py` → `assets/genomes/castle_hub_creator_lights.toml`
(43 ponctuelles + 13 spots ; hauteurs 18 à 75 m pour un sol du Hall à 36,5 m).

Direction des spots : une lumière Unity pointe vers son **+Z local** ; on applique
au vecteur avant le même miroir que sur les positions, puis `looking_to` côté Bevy
(qui aligne son −Z).

Deux écarts assumés :

- **Ombres portées désactivées.** 56 lumières à ombres coûteraient bien plus
  qu'elles n'apportent — et sa scène s'appuyait de toute façon sur des ombres
  **cuites** (shadowmasks) que nous n'avons pas.
- **Sa directionnelle n'est pas reprise.** Elle est à 0,7 d'intensité avec un
  cookie, là où nous avons 20 000 et 7 000 lux. Mais nos valeurs tiennent
  l'extérieur, qui rend bien : les remplacer était un pari inutile. Un curseur
  `sun_scale` (hot-reload) permet de les atténuer à l'œil.

L'intensité Unity (0,4 à 59) n'est pas en lumens : un facteur unique
`creator_lights.scale` les convertit toutes, réglable à chaud.

### H — post-traitement aux valeurs relevées

Conversion d'échelle URP → Bevy (Unity est en pourcentage −100..100, nous en
multiplicateur autour de 1) :

| Son réglage | Valeur URP | Reprise |
|---|---|---|
| `ColorAdjustments.contrast` | +10 | `contrast = 0.10` |
| `ColorAdjustments.saturation` | +7,4 | `saturation = 0.074` |
| `WhiteBalance.temperature` | +14,9 | `temperature = 0.149` |
| `ColorAdjustments.postExposure` | +1,8 EV | `exposure = 0.60` (part seulement) |

L'exposition n'est reprise qu'en partie : son +1,8 EV compensait une scène
lightmappée que nous n'avons pas, et notre éclairage vient d'être refait.

**Non repris, faute d'équivalent dans notre `ModeGrade`** : ses **ombres bleues**
(`ShadowsMidtonesHighlights`, ombres à 0,767 · 0,808 · 1,0) et sa **vignette bleu
nuit**. Bevy expose bien un `ColorGrading.shadows`, mais notre structure ne rend
que le global — c'est le prochain écart à combler, et c'est un morceau
caractéristique de son look.

### Vérification

`cargo clippy -p forgia-game --all-targets` : 0 warning. Le placement des lumières
repose sur une transformation vérifiée au millimètre sur 1 195 pièces ; il reste à
juger l'intensité à l'œil, ce que le hot-reload permet sans rebuild.
