# Étude — comment on éclaire un jeu, et comment le créateur du pack l'a fait

> 2026-07-26. Croisement de trois sources : la documentation des deux moteurs, les
> fichiers de réglage du pack « FANTASTIC Highlands Castle », et l'état réel de
> notre Hall.
>
> **Conclusion en une phrase** : notre écart avec sa capture n'est pas un problème
> de réglage, c'est un problème de **modèle d'éclairage**. Il éclaire en *cuit*,
> nous éclairons en *temps réel*, et ces deux modèles ne convergent pas en montant
> des valeurs.

---

## 1. Les trois façons d'éclairer, et ce qu'elles coûtent

| Modèle | Ce qui est calculé | Rebond indirect | Coût par frame |
|---|---|---|---|
| **Temps réel** | tout, à chaque image | ❌ aucun | élevé (ombres surtout) |
| **Cuit** (*baked*) | rien : tout est précalculé dans des textures | ✅ complet | quasi nul |
| **Mixte** | direct en temps réel, indirect lu dans les textures | ✅ complet | moyen |

Le point qui décide de tout, c'est la colonne du milieu.

### Pourquoi l'absence de rebond ne se rattrape pas en montant les valeurs

Dans une pièce fermée en pierre, la lumière qui arrive à l'œil a majoritairement
**rebondi au moins une fois**. Une bougie n'éclaire pas seulement le mur qu'elle
touche : ce mur réémet vers le plafond, qui réémet vers le sol.

Sans rebond, tout ce qui n'est pas frappé **directement** par une source tombe au
plancher de l'ambiante. On obtient des flaques de lumière isolées séparées par du
noir, jamais une pièce qui « baigne ».

Et monter l'intensité des sources n'y change rien : ça agrandit les flaques et ça
crame leur centre, sans jamais remplir l'entre-deux. C'est la limite exacte que
mon dernier réglage a atteinte — j'y reviens au §6.

> Baked GI *« profite du temps de calcul disponible pour produire des ombres douces
> et de la lumière indirecte plus réalistes que ce qu'on obtient normalement en
> temps réel »* — documentation Unity.

---

## 2. Comment le créateur a fait — mesuré, pas supposé

Tout ce qui suit est relevé dans `Lighting Settings Castle.lighting`, dans sa
scène et dans `LightingData.asset`.

### 2.1 Son moteur de cuisson

| Réglage | Valeur | Ce que ça veut dire |
|---|---|---|
| `m_EnableBakedLightmaps` | **1** | lightmaps cuites — c'est la base de son rendu |
| `m_EnableRealtimeLightmaps` | 0 | pas de GI temps réel |
| `m_BakeBackend` | **2** | *GPU Progressive Lightmapper* |
| `m_PVRBounces` | **2** | **deux rebonds** de lumière |
| `m_PVRSampleCount` | 512 | 512 échantillons indirects par texel |
| `m_BakeResolution` | 20 | 20 texels par unité de monde |
| `m_LightmapMaxSize` | 4096 | atlas de 4096² |
| `m_AO` / `m_AOMaxDistance` | **1** / 0,8 | **occlusion ambiante cuite**, rayon 0,8 m |
| `m_CompAOExponentDirect` | 0 | l'AO ne s'applique qu'à l'**indirect** |
| `m_MixedBakeMode` | **2** | **Shadowmask** |
| `m_BounceScale`, `m_AlbedoBoost`, `m_IndirectOutputScale` | 1, 1, 1 | **aucun trucage** : il ne gonfle rien |

Ce dernier point mérite d'être souligné. Ces trois curseurs servent habituellement
à « tricher » quand le rendu manque de lumière. Il les laisse tous à 1 : sa scène
est lumineuse **parce que le rebond est vraiment calculé**, pas parce qu'il a
poussé des multiplicateurs.

### 2.2 Ses lumières — et la surprise

57 lumières dans la scène :

| Type | Mode | Nombre |
|---|---|---|
| directionnelle (le soleil) | **Baked** | **1** |
| ponctuelle | Mixed | 42 |
| spot | Mixed | 8 |
| ponctuelle | Realtime | 1 |
| spot | Realtime | 5 |

Et le chiffre qui change la lecture de tout :

> **49 lumières sur 57 ne projettent AUCUNE ombre.** Seules 8 ont des ombres douces.

Son soleil est **entièrement cuit** : au runtime il n'existe pas. Ses 50 lumières
mixtes ne fournissent que leur composante directe ; leur indirect est dans les
lightmaps, et leurs ombres dans le shadowmask — une texture qui stocke
**jusqu'à 4 lumières par texel**, une par canal RGBA.

Autrement dit : **son éclairage temps réel est minuscule**. Toute la richesse est
précalculée.

### 2.3 Le reste de sa configuration

| | |
|---|---|
| Ambiante | **Skybox** (`m_AmbientMode: 0`), intensité 0,85 — pas une couleur plate |
| Brouillard | exponentiel, densité 0,01, cyan (0,35 · 0,87 · 1,0) |
| Réflexions | 31 sondes, 1 rebond, intensité 1, résolution 256 |
| Lightmaps | **11 atlas 4096²**, mode **non directionnel** |
| Renderers cuits | **20 484** |

---

## 3. Ce que nous faisons, nous

| | Lui | Nous |
|---|---|---|
| Rebond indirect | 2 rebonds cuits | **aucun** |
| Occlusion ambiante | cuite, sur l'indirect | aucune |
| Soleil | cuit, gratuit | **2 directionnelles temps réel**, dont une à 4 cascades sur 420 m |
| Lumières locales | 50 mixtes, 49 sans ombre | jusqu'à **128 ponctuelles temps réel** |
| Ombres | shadowmask cuit, 8 sources temps réel | cascades temps réel |
| Ambiante | skybox 0,85 | **couleur plate** 400 |
| Réflexions | 31 sondes localisées | 1 cubemap globale (livrée aujourd'hui) |

Nous payons donc **plus cher par frame** que lui, pour un résultat **plus pauvre**.
C'est le cœur du problème : nous faisons tourner un modèle temps réel coûteux là
où il fait tourner un modèle cuit quasi gratuit.

---

## 4. Le verdict

**On essaie de reproduire un rendu cuit avec de l'éclairage temps réel.**

Ça explique tout ce qu'on observe depuis le début :

- Les pièces restent sombres → il manque l'indirect, pas du direct.
- Monter les valeurs délave sans révéler le volume → on ajoute du direct là où il
  manque de l'indirect.
- La pierre paraissait « mouillée » → aucun environnement à réfléchir (corrigé
  aujourd'hui, mais avec **une** ambiance globale et non ses 31 sondes).
- Deux murs identiques côte à côte, l'un blanc l'autre noir → c'était le
  remplissage sans ombres qui traversait les murs, un pansement sur le même
  manque.

On ne rattrape pas l'absence de rebond en changeant l'ampoule.

---

## 5. Le déblocage : tout ce qui manquait est lisible

Jusqu'ici deux choses bloquaient le portage, toutes deux enfermées dans
`LightingData.asset`, un binaire Unity :

1. quelle lightmap et quelle zone d'atlas va avec quelle pièce ;
2. quel fichier EXR de sonde va avec quelle sonde.

**`UnityPy` 1.25.2 est déjà installé sur cette machine et lit ce fichier.** Vérifié
aujourd'hui — voici ce qu'il en sort :

| Donnée | Contenu |
|---|---|
| `m_LightmappedRendererData` | **20 484** entrées : `lightmapIndex` + `lightmapST` |
| `m_LightmapsMode` | **0** = non directionnel — le cas le plus simple |
| `m_BakedReflectionProbeCubemaps` | **32** cubemaps, dans l'ordre des `ReflectionProbe-N.exr` |
| `m_Lightmaps` | 11 entrées (lightmap + shadowmask, pas de lightmap directionnelle) |

Et le détail qui rend la suite presque mécanique :

> `lightmapST = {x, y, z, w}` est **exactement** le `uv_rect` du composant
> `Lightmap` de Bevy : rectangle de `(z, w)` à `(z + x, w + y)`.

Côté géométrie, la condition est déjà remplie : **l'UV2 est présent sur 100 % de
nos primitives** (423 sur 423 vérifiées). C'est le canal que Bevy lit pour les
lightmaps (`ATTRIBUTE_UV_1`).

Enfin, l'identité d'un renderer est donnée par `{targetObject, targetPrefab}`,
c'est-à-dire l'identifiant de l'instance de prefab dans la scène — celui-là même
que mon extracteur de props sait déjà lire.

**Il n'y a donc plus de blocage de principe. Il reste du travail, pas une inconnue.**

---

## 6. Ce que je dois corriger dans ce que j'ai livré

Mon dernier réglage a monté la portée des bougies de 2,45 à 5,5 m et l'intensité
de 630 à 1 400 lm, en expliquant que « ses valeurs supposent ses lightmaps ». Le
raisonnement était juste, la réponse est un **pansement** :

- elle coûte cher (le volume d'une lumière croît au cube de sa portée — 128
  lumières à 5,5 m au lieu de 2,45 m, c'est **11 fois** le volume à grouper) ;
- elle ne produit toujours pas de rebond, donc elle délave au lieu de remplir ;
- et elle nous éloigne de ses valeurs mesurées, qu'il faudra rétablir le jour où
  les lightmaps arrivent.

À garder tel quel **en attendant**, mais à annuler dès que le correctif F passe.

---

## 7. Les limites de Bevy qu'il faut connaître

| Limite | Valeur | Conséquence pour nous |
|---|---|---|
| `MAX_VIEW_LIGHT_PROBES` | **8** | ses 31 sondes ne tiennent pas dans une vue : il faudra sélectionner les plus proches |
| `MAX_DIRECTIONAL_LIGHTS` | 10 (desktop ; 1 en WebGL) | nos 2 directionnelles passent |
| `MAX_UNIFORM_BUFFER_CLUSTERABLE_OBJECTS` | 204 | nos 128 lumières passent, mais la marge est plus faible qu'elle n'en a l'air |
| `MAX_CASCADES_PER_LIGHT` | 4 | notre soleil est déjà au maximum |

Deux corrections de rendu arrivées en 0.18 méritent d'être notées : le spéculaire
des lumières ponctuelles était **trop brillant**, et le Fresnel dépendant de la
rugosité des cartes d'environnement a été remplacé. Nos matériaux ne se
comporteront donc pas comme sur les captures d'avant cette version.

---

## 8. Plan, réordonné par ce qu'on vient d'apprendre

| # | Action | Pourquoi | Effort |
|---|---|---|---|
| **F1** | Lire `LightingData.asset` (UnityPy) → table `pièce → lightmap + uv_rect` | Débloque tout le reste ; c'est de la donnée, pas du rendu | ~3 h |
| **F2** | Convertir les 11 lightmaps EXR 4096² en textures Bevy | 250 Mo bruts — à évaluer en KTX2 compressé | ~3 h |
| **F3** | Poser le composant `Lightmap` sur chaque pièce du Hall | **C'est ici que le rebond apparaît** | ~4 h |
| **F4** | Rétablir ses valeurs de lumière mesurées, retirer les compensations | Le pansement du §6 devient inutile et coûteux | ~1 h |
| **K2** | Les 31 sondes localisées, via la table de cubemaps | 8 par vue au plus — sélection par proximité | ~4 h |
| **M** | 228 pièces mal placées (tours) | Indépendant, extérieur | ~3 h |

**F1 d'abord** : sans cette table, ni F3 ni K2 ne sont possibles. Avec elle, les
deux le deviennent.

Ce qui restera hors de portée : son **shadowmask** (Bevy n'a pas d'équivalent) et
son **AO cuite**. On peut approcher la seconde avec le SSAO temps réel de Bevy,
déjà présent dans le projet.

---

## Sources

- [Bevy — composant `Lightmap`](https://docs.rs/bevy/latest/bevy/pbr/struct.Lightmap.html)
- [Bevy 0.18 — notes de version](https://bevy.org/news/bevy-0-18/)
- [Bevy — `bevy_light`](https://docs.rs/bevy_light)
- [Bevy — `PointLight` (lumens)](https://docs.rs/bevy/latest/bevy/prelude/struct.PointLight.html)
- [Bevy — `DirectionalLight` (lux)](https://docs.rs/bevy/latest/bevy/prelude/struct.DirectionalLight.html)
- [Bevy — éclairage et ombres (DeepWiki)](https://deepwiki.com/bevyengine/bevy/5.6-lighting-and-shadows)
- [Bevy — support de l'éclairage mixte, PR #16761](https://github.com/bevyengine/bevy/pull/16761)
- [Unity — modes d'éclairage (Shadowmask)](https://docs.unity3d.com/Manual/lighting-mode.html)
- [Unity — modes de lumière](https://docs.unity3d.com/6000.0/Documentation/Manual/LightModes-introduction.html)
- [Unity — Global Illumination](https://docs.unity3d.com/560/Documentation/Manual/GIIntro.html)
- [Unity HDRP — unités physiques de lumière](https://docs.unity3d.com/Packages/com.unity.render-pipelines.high-definition@14.0/manual/Physical-Light-Units.html)
- [pcwalton — `bevy-baked-gi`, workflow de GI cuite pour Bevy](https://github.com/pcwalton/bevy-baked-gi)
- [Temps réel vs cuit, retour de terrain](https://medium.com/@JasonTuttle/real-time-gi-vs-baked-for-mobile-games-ef173929d8cb)
