# Animations Mixamo → Trooper — quoi télécharger, et pourquoi

> Mixamo est derrière un compte Adobe : le téléchargement est **manuel**, aucune
> automatisation possible. Ce document dit exactement quoi prendre et avec quels
> réglages, pour que `tools/blender/retarget_mixamo.py` les consomme sans retouche.

## 1. Réglages d'export — les trois qui comptent

Sur chaque animation, bouton **DOWNLOAD** :

| Réglage | Valeur | Pourquoi |
|---|---|---|
| Format | **FBX Binary (.fbx)** | Blender lit le binaire ; l'ASCII n'est pas supporté |
| Skin | **Without Skin** | On ne veut que le squelette animé — le maillage est déjà le nôtre |
| Frames per Second | **30** | Suffisant, et divise proprement |
| Keyframe Reduction | **none** | La réduction écrase les micro-mouvements qui font le réalisme |

🚨 **« In Place » est OBLIGATOIRE** sur toute locomotion (marche, course,
strafe). Sans lui, l'animation déplace le personnage — or c'est le contrôleur
qui déplace la capsule. Les deux se cumuleraient et le personnage patinerait ou
partirait en avant. La case est sur la page de l'animation, à côté des curseurs.

## 2. Les clips à prendre

Le jeu a des verbes précis, mesurés : sprint **9,75 m/s**, saut **1,174 m**, un
dash, **pas d'accroupissement**. La liste suit ces verbes, pas un catalogue
générique.

### Indispensables (le personnage cesse d'être une statue)

| Clip Mixamo | Nom de fichier attendu | Usage |
|---|---|---|
| *Breathing Idle* | `idle.fbx` | à l'arrêt |
| *Walking* (In Place) | `walk_f.fbx` | avance |
| *Running* (In Place) | `run_f.fbx` | sprint |
| *Jumping Up* | `jump_start.fbx` | départ du saut |
| *Falling Idle* | `jump_air.fbx` | en l'air (bouclé) |
| *Hard Landing* ou *Landing* | `jump_land.fbx` | réception |

### Ce qui fait vraiment la différence en 3ᵉ personne

La caméra orbite : on voit le personnage de côté et de dos, donc les
déplacements latéraux et arrière se remarquent immédiatement. Sans eux, il
marche en crabe en regardant devant.

| Clip Mixamo | Fichier | Usage |
|---|---|---|
| *Walking Backwards* (In Place) | `walk_b.fbx` | recul |
| *Left Strafe Walking* | `walk_l.fbx` | pas chassé gauche |
| *Right Strafe Walking* | `walk_r.fbx` | pas chassé droit |
| *Left Strafe* (course) | `run_l.fbx` | strafe rapide |
| *Right Strafe* | `run_r.fbx` | strafe rapide |
| *Running Backward* | `run_b.fbx` | recul rapide |

### Confort

| Clip Mixamo | Fichier | Usage |
|---|---|---|
| *Left Turn* / *Right Turn* | `turn_l.fbx` / `turn_r.fbx` | rotation sur place quand la caméra tourne |
| *Standing Dodge Forward* ou *Roll* | `dash.fbx` | le dash existe déjà côté contrôleur |

Déposer le tout dans `assets/source/mixamo/` (dossier non versionné, comme les
autres sources d'import).

## 3. Ce que le script fait ensuite

`tools/blender/retarget_mixamo.py` :

1. importe le FBX Mixamo (squelette `mixamorig:*`) ;
2. mappe chaque os sur son équivalent Unreal du trooper — table ci-dessous ;
3. transfère la pose **os par os, frame par frame**, en repère armature ;
4. écrit le clip dans le rig du trooper, sous le nom de fichier.

### Table de correspondance

Confirmée sur la doc de `mixamo_converter` (cf. sources en fin de fichier) et
vérifiée contre les 68 os réels du trooper.

| Mixamo | Trooper |
|---|---|
| `Hips` | `pelvis` |
| `Spine` / `Spine1` / `Spine2` | `spine_01` / `spine_02` / `spine_03` |
| `Neck` / `Head` | `neck_01` / `head` |
| `LeftShoulder` / `RightShoulder` | `clavicle_l` / `clavicle_r` |
| `LeftArm` / `RightArm` | `upperarm_l` / `upperarm_r` |
| `LeftForeArm` / `RightForeArm` | `lowerarm_l` / `lowerarm_r` |
| `LeftHand` / `RightHand` | `hand_l` / `hand_r` |
| `LeftHandThumb1..3` | `thumb_01..03_l` |
| `LeftHandIndex1..3` | `index_01..03_l` |
| `LeftHandMiddle1..3` | `middle_01..03_l` |
| `LeftHandRing1..3` | `ring_01..03_l` |
| `LeftHandPinky1..3` | `pinky_01..03_l` |
| `LeftUpLeg` / `RightUpLeg` | `thigh_l` / `thigh_r` |
| `LeftLeg` / `RightLeg` | `calf_l` / `calf_r` |
| `LeftFoot` / `RightFoot` | `foot_l` / `foot_r` |
| `LeftToeBase` / `RightToeBase` | `ball_l` / `ball_r` |

🚨 **Os du trooper SANS équivalent Mixamo** : `*_twist_01_*` (avant-bras,
bras, cuisse, mollet) et les `*_Muscle`. Ce sont des os d'assistance ; ils
restent au repos, ce qui est correct — les animer depuis un rig qui ne les a pas
produirait du bruit.

🚨 **Ne PAS utiliser l'auto-mapping** des outils de retarget sur un squelette
Mixamo : il associe les os aux mauvais nœuds. La table explicite ci-dessus est
la seule source.

## 4. Rappel d'architecture

Les clips n'ont besoin d'exister **que dans `body.gltf`** : les pièces d'armure
partagent le squelette du corps au runtime (rebranchement des joints par nom).
Une pièce qui embarquerait sa propre armature animée se désynchroniserait —
c'est le défaut « armure détachée des bras » observé le 2026-08-01.

## Sources

- [enziop/mixamo_converter — renommage d'os](https://deepwiki.com/enziop/mixamo_converter/3.3-bone-renaming-options)
- [mixamo_converter README](https://github.com/enziop/mixamo_converter/blob/master/README.md)
- [UE5 Mixamo Animation Retargeting (UNAmedia)](https://www.unamedia.com/ue5-mixamo/docs/tutorial/)
- [Skeleton Hierarchy: Why Your Retargets Keep Breaking (MoCap Online)](https://mocaponline.com/blogs/mocap-news/skeleton-hierarchy-animation-guide)
