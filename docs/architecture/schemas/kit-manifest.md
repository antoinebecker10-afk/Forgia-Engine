# Schéma `kit-manifest` — vocabulaire, sockets et interfaces formelles

> Statut : contrat proposé pour le MVP PCG. Le manifest rend un kit mesurable,
> substituable et validable. Il est produit/complété par le catalogueur Blender,
> puis validé avant publication dans le registre.

## 1. Principe

Un kit n'est pas une collection de GLB. C'est un ensemble de **pièces nommées**,
de capacités et de sockets dont le contrat spatial et sémantique est explicite.
Le solveur ne teste jamais une compatibilité par nom de fichier ou par proximité
approximative : il résout une relation entre deux sockets.

```toml
schema_version = "forgia.kit-manifest/v1"
id = "forgia.castle.stone@1.0.0"
license = "internal" # SPDX ou politique projet obligatoire
extends = []
source = { tool = "Blender 5.0", coordinate_system = "forgia_y_up_meters" }

[assets]
root = "assets/pcg/kits/castle_stone/1.0.0"

[capabilities]
provides = ["theme.medieval_stone", "portal.door", "wall.load_bearing"]
```

## 2. Modèle de socket

Un socket `S` est un repère local complet et un contrat :

```text
S = (id, family, role, gender, frame, aperture, clearance, channels,
     tags, accepts, rotation_policy, seal_policy)
```

- `frame` : origine en mètres, orientation (quaternion ou base orthonormale) et
  normale **sortante**. La convention est Y-up Forgia ; l'export Blender applique
  l'adaptateur de repère déjà prouvé, jamais le solveur.
- `family` : famille de connexion, par exemple `arch.wall`, `portal.door`,
  `mount.engine`, `power.48v`.
- `role` / `gender` : `structural|portal|utility|decor` et
  `male|female|neutral`. La neutralité est autorisée seulement si le manifest
  l'annonce ; elle évite des modèles de données artificiellement binaires.
- `aperture` : forme/largeur/hauteur/profondeur à faire coïncider. Pour une
  porte c'est le vide praticable ; pour un mur c'est le profil de joint.
- `clearance` : volume libre requis autour de l'interface, distinct du mesh ;
  il protège capsule joueur, porte animée, roue, câble, caméra.
- `channels` : réseaux à raccorder (`power.48v`, `data.can`, `nav.walkable`,
  `water.flow`). Chaque canal a direction, capacité et compatibilité.
- `accepts` : familles/tags/capacités que le socket peut recevoir, avec règles
  de tolérance. Cela évite une matrice globale ingérable.

### Prédicat de compatibilité

Deux sockets `a` et `b` peuvent être reliés si et seulement si :

```text
family_compatible(a,b)
AND gender_compatible(a.gender,b.gender)
AND tags_satisfy(a.accepts,b) AND tags_satisfy(b.accepts,a)
AND aperture_compatible(a.aperture,b.aperture,tolerance)
AND channels_compatible(a.channels,b.channels)
AND transform_match(a.frame,b.frame, rotation_policy, tolerance)
AND clearance_non_intersecting(a,b)
AND no_hard_budget_is_broken
```

`transform_match` aligne les origines, rend les normales opposées (dot ≤ -0.999
par défaut), puis autorise seulement les rotations déclarées (par exemple 0/90/
180/270° pour une tuile carrée). Les tolérances sont exécutées **après** la pose
mathématique et servent au contrôle d'asset, pas à masquer un montage imprécis.

La liaison produit un `SocketBinding { a, b, transform, channels }` dans le
`SpatialPlan`. Une face sans binding doit avoir `seal_policy = "cap"` et recevoir
une pièce de fermeture ; aucune ouverture implicite.

## 3. Exemple complet — mur de pierre + porte

```toml
schema_version = "forgia.kit-manifest/v1"
id = "forgia.castle.stone@1.0.0"
license = "internal"
source = { tool = "Blender 5.0", coordinate_system = "forgia_y_up_meters" }

[assets]
root = "assets/pcg/kits/castle_stone/1.0.0"

[compatibility]
default_position_tolerance_m = 0.001
default_normal_dot_max = -0.999
default_rotation_tolerance_deg = 0.1

[[pieces]]
id = "wall.straight.4m"
asset = "render/wall_straight_4m.glb#Scene0"
provides = ["wall.load_bearing", "theme.medieval_stone"]
bounds_m = { min = [-2.0, 0.0, -0.25], max = [2.0, 4.0, 0.25] }
mass_class = "static"
lods = [
  { asset = "render/wall_straight_4m_lod0.glb#Scene0", max_distance_m = 30.0 },
  { asset = "render/wall_straight_4m_lod1.glb#Scene0", max_distance_m = 90.0 },
]
collision = { kind = "box", asset = "collision/wall_straight_4m_proxy.glb#Scene0" }

[[pieces.sockets]]
id = "west"
family = "arch.wall"
role = "structural"
gender = "neutral"
frame = { origin_m = [-2.0, 2.0, 0.0], forward = [-1.0, 0.0, 0.0], up = [0.0, 1.0, 0.0] }
aperture = { shape = "rect", width_m = 0.50, height_m = 4.00, depth_m = 0.50 }
clearance = { shape = "box", half_extents_m = [0.30, 2.10, 0.30] }
rotation_policy = { allowed_yaw_deg = [0, 90, 180, 270] }
seal_policy = "cap"
accepts = [{ family = "arch.wall", tags_all = ["theme.medieval_stone"] }]

[[pieces.sockets]]
id = "east"
family = "arch.wall"
role = "structural"
gender = "neutral"
frame = { origin_m = [2.0, 2.0, 0.0], forward = [1.0, 0.0, 0.0], up = [0.0, 1.0, 0.0] }
aperture = { shape = "rect", width_m = 0.50, height_m = 4.00, depth_m = 0.50 }
clearance = { shape = "box", half_extents_m = [0.30, 2.10, 0.30] }
rotation_policy = { allowed_yaw_deg = [0, 90, 180, 270] }
seal_policy = "cap"
accepts = [{ family = "arch.wall", tags_all = ["theme.medieval_stone"] }]

[[pieces.sockets]]
id = "door_front"
family = "portal.door"
role = "portal"
gender = "female"
frame = { origin_m = [0.0, 1.5, -0.25], forward = [0.0, 0.0, -1.0], up = [0.0, 1.0, 0.0] }
aperture = { shape = "rect", width_m = 1.60, height_m = 3.00, depth_m = 0.50 }
clearance = { shape = "box", half_extents_m = [0.90, 1.60, 1.20] }
channels = [{ id = "nav.walkable", direction = "bidirectional", capacity = 1 }]
rotation_policy = { allowed_yaw_deg = [0, 180] }
seal_policy = "must_bind"
accepts = [{ family = "portal.door", gender = "male", requires = ["nav.walkable"] }]

[[pieces]]
id = "door.oak.large"
asset = "render/door_oak_large.glb#Scene0"
provides = ["portal.door", "theme.medieval_stone"]
bounds_m = { min = [-0.8, 0.0, -0.25], max = [0.8, 3.0, 0.25] }
collision = { kind = "box", asset = "collision/door_oak_large_proxy.glb#Scene0" }

[[pieces.sockets]]
id = "frame_back"
family = "portal.door"
role = "portal"
gender = "male"
frame = { origin_m = [0.0, 1.5, 0.25], forward = [0.0, 0.0, 1.0], up = [0.0, 1.0, 0.0] }
aperture = { shape = "rect", width_m = 1.60, height_m = 3.00, depth_m = 0.50 }
clearance = { shape = "box", half_extents_m = [0.90, 1.60, 1.20] }
channels = [{ id = "nav.walkable", direction = "bidirectional", capacity = 1 }]
rotation_policy = { allowed_yaw_deg = [0, 180] }
seal_policy = "must_bind"
accepts = [{ family = "portal.door", gender = "female", requires = ["nav.walkable"] }]
```

## 4. Exemple de socket non architectural — moteur véhicule

```toml
[[pieces.sockets]]
id = "power_out"
family = "power.48v"
role = "utility"
gender = "male"
frame = { origin_m = [0.0, 0.5, -1.2], forward = [0.0, 0.0, -1.0], up = [0.0, 1.0, 0.0] }
aperture = { shape = "cylinder", radius_m = 0.04, length_m = 0.08 }
clearance = { shape = "cylinder", radius_m = 0.12, length_m = 0.30 }
channels = [{ id = "power.48v", direction = "out", capacity = 150 }]
rotation_policy = { allowed_roll_deg = [0, 90, 180, 270] }
seal_policy = "must_bind"
accepts = [{ family = "power.48v", gender = "female", direction = "in", min_capacity = 120 }]
```

Une connexion mécanique peut être valide et une connexion électrique invalide :
les deux `SocketBinding` sont donc distincts et la contrainte de réseau du
`content-spec` exige un graphe électrique connecté.

## 5. Catalogueur et gates de publication

Le catalogueur Blender lit les empties nommés `PCG_SOCKET__<id>`, les meshes
`PCG_COLLISION__*`, `PCG_LOD<n>__*` et les custom properties. Il produit un
manifest brouillon, puis ces gates bloquent la publication :

La première implémentation est disponible :

```powershell
& "C:\Program Files\Blender Foundation\Blender 5.0\blender.exe" `
  --background assets/source/castle_stone.blend `
  --python tools/blender/catalog_pcg_kit.py -- `
  --kit-id forgia.castle.stone@1.0.0 `
  --asset-root assets/pcg/kits/castle_stone/1.0.0 `
  --output assets/pcg/kits/castle_stone/1.0.0/kit.toml
```

Elle exige une collection `PCG_PIECE__<id>`, un Empty
`PCG_ROOT__<id>` (repère local stable), et les empties `PCG_SOCKET__<id>`.
Les propriétés `pcg_family`, `pcg_role`, `pcg_gender`,
`pcg_aperture_shape`, dimensions et `pcg_accepts` sont obligatoires : le
catalogueur échoue explicitement si elles sont ambiguës. Les LOD/collision sont
exportés par le cooker compagnon :

```powershell
& "C:\Program Files\Blender Foundation\Blender 5.0\blender.exe" `
  --background assets/source/castle_stone.blend `
  --python tools/blender/export_pcg_kit.py -- `
  --asset-root assets/pcg/kits/castle_stone/1.0.0
```

Il produit `render/<piece>.glb`, `render/<piece>_lod<n>.glb` et, uniquement si
un mesh `PCG_COLLISION__*` existe, `collision/<piece>_proxy.glb`. L'export est
non destructif : une collection temporaire retire le transform du
`PCG_ROOT__<id>` sans modifier le `.blend`. La V1 est intentionnellement limitée
aux modules statiques : un rig, une animation ou une contrainte de scène doit
être cuit par un pipeline dédié et ne peut pas être publié silencieusement comme
une pièce statique.

1. frame orthonormale, unité mètre, bornes et pivots valides ;
2. socket dans/au bord de la pièce et clearance sans chevauchement interdit ;
3. asset de rendu, LOD et proxy existent ; proxy sans TriMesh per-mesh en runtime ;
4. export GLB valide, tangentes présentes si matériau normal-mapped ;
5. tests de montage avec tous les `accepts` déclarés ;
6. rendu Cycles headless et rapport de taille/budgets.

Les pièges prouvés restent contraignants : conversion Unity→Blender par réflexion,
rotations Unity ZXY en cas de quaternion incomplet, offset ancré bâtiment,
instanciation exportée sans application destructive, et Cycles — non EEVEE — pour
le rendu headless.
