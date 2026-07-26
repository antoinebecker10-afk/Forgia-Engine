# Schéma `content-spec` — le plan indépendant des assets

> Statut : contrat de données proposé pour le MVP PCG. Les exemples sont du TOML
> valide et montrent la forme cible ; le parseur Rust devra refuser les clés,
> références et unités inconnues. Les valeurs de budget restent des paramètres de
> produit par cible, non des promesses universelles.

## 1. But et invariants

`content-spec` exprime l'intention d'un créateur. Il ne contient aucun chemin GLB,
aucune transform Blender et aucune décision de solveur. Sa compilation donne :

```text
content-spec + registry.lock + GenerationContext
  -> LogicalPlan (graphe sémantique)
  -> SpatialPlan (instances/sockets/transforms/cellules)
  -> bundles offline + activation runtime
```

Invariants : mètres, repère Forgia/Bevy Y-up, IDs stables ASCII (`[a-z0-9._-]`),
`schema_version` obligatoire, références versionnées, hasard exclusivement dérivé
de `generation.seed`. Une spec est portable entre kits si elle demande des
**capabilities** plutôt que des noms de meshes.

## 2. Forme canonique

```toml
schema_version = "forgia.content-spec/v1"
id = "forgia.world.example@0.1.0"
kind = "structure" # structure | dungeon | vehicle | terrain | creature
title = "Nom humain"

[generation]
seed = 0xD00DFEED # u64 ; 0 interdit dans le MVP pour éviter une seed implicite
solver = "auto"  # auto | constructive | csp_wfc | search
registry_lock = "pcg-registry/registry.lock.toml"

[intent]
tags = ["biome.highlands", "faction.forgia"]
requires = ["space.hub", "portal.door"] # capacités, jamais assets/
forbids = ["theme.sci_fi"]

[bounds]
max_extent_m = [128.0, 64.0, 128.0] # [x,y,z], hard
anchor = "world_origin"               # world_origin | entrance | custom anchor ID

[budgets.target_desktop]
max_stream_cells = 16
max_visible_meshes = 1200
max_triangles = 2500000
max_materials = 96
max_texture_vram_mb = 768
max_collision_proxies = 128
max_load_p99_ms = 250
max_frame_p99_ms = 16.67

[[zones]]
id = "entrance"
kind = "space.entrance"
requires = ["portal.door", "nav.walkable"]
size_m = { min = [6.0, 3.0, 6.0], preferred = [10.0, 5.0, 10.0] }

[[constraints.hard]]
id = "all_objectives_reachable"
kind = "reachability"
from = "entrance"
to = ["objective"]

[[objectives.soft]]
id = "variety"
metric = "kit_instance_entropy"
weight = 0.3
target = { min = 0.65 }
```

Les sections possibles sont : `zones`, `graph`, `constraints.hard`,
`objectives.soft`, `style`, `streaming`, `variants`, `author_overrides`. Une clé
spécifique à un domaine va sous `[domain.<kind>]`, jamais au sommet.

## 3. Exemple travaillé — Château / Hall de Forgia

```toml
schema_version = "forgia.content-spec/v1"
id = "forgia.hall.highlands@0.1.0"
kind = "structure"
title = "Hall de Forgia — Highlands"

[generation]
seed = 0xF0471A22
solver = "constructive"
registry_lock = "pcg-registry/registry.lock.toml"

[intent]
tags = ["biome.highlands", "theme.medieval_stone", "mood.warm_hub"]
requires = ["space.hall", "portal.door", "nav.walkable", "social.safe"]
forbids = ["combat.spawn", "theme.sci_fi"]

[bounds]
max_extent_m = [96.0, 32.0, 84.0]
anchor = "great_hall_spawn"

[style]
palette = ["stone.warm", "wood.dark", "banner.forgia"]
allowed_kits = ["forgia.castle.stone@1", "forgia.castle.wood@1"]
symmetry = "axial_x"
variation = 0.20

[[zones]]
id = "great_hall"
kind = "space.hall"
requires = ["nav.walkable", "social.safe", "light.interior"]
size_m = { min = [40.0, 12.0, 32.0], preferred = [64.0, 18.0, 48.0] }
stream_cell = "hall.main"

[[zones]]
id = "throne"
kind = "poi.throne"
parent = "great_hall"
requires = ["landmark.high", "view.from_entrance"]

[[zones]]
id = "west_entry"
kind = "space.entrance"
parent = "great_hall"
requires = ["portal.door", "spawn.player"]

[[graph.edges]]
from = "west_entry"
to = "great_hall"
portal = "door.large"

[[graph.edges]]
from = "great_hall"
to = "throne"
portal = "path.open"

[[constraints.hard]]
id = "hall_accessible"
kind = "reachability"
from = "west_entry"
to = ["throne"]

[[constraints.hard]]
id = "physical_proxy_only"
kind = "budget"
metric = "collision_proxies"
op = "<="
value = 32

[[objectives.soft]]
id = "throne_reveal"
metric = "landmark_visibility"
from = "west_entry"
weight = 0.8
target = { min = 0.85 }

[streaming]
cell_size_m = [32.0, 24.0, 32.0]
preload_neighbors = 1
activate_order = ["collision_proxy", "navmesh", "render"]
deactivate_order = ["render", "collision_proxy"]
```

## 4. Exemple travaillé — Donjon roguelite lock-and-key

```toml
schema_version = "forgia.content-spec/v1"
id = "forgia.roguelite.crypt.azur@0.1.0"
kind = "dungeon"
title = "Crypte du Sceau Azur"

[generation]
seed = 0xA2A20001
solver = "search"
registry_lock = "pcg-registry/registry.lock.toml"

[intent]
tags = ["biome.crypt", "theme.undead", "difficulty.tier_2"]
requires = ["gameplay.combat", "progression.lock_key", "nav.walkable"]

[domain.dungeon]
room_count = { min = 7, max = 11 }
critical_path_rooms = { min = 5, max = 7 }
loop_count = { min = 1, max = 2 }
grammar = "forgia.dungeon.lock_key@1"
entry = "entrance"
goal = "boss"

[[zones]]
id = "entrance"
kind = "space.entrance"
requires = ["spawn.player", "portal.door"]

[[zones]]
id = "azure_key"
kind = "reward.key"
requires = ["item.sigil.azur", "combat.encounter"]

[[zones]]
id = "azure_gate"
kind = "gate.lock"
requires = ["lock.sigil.azur", "portal.door"]

[[zones]]
id = "boss"
kind = "combat.boss"
requires = ["arena.boss", "portal.exit"]

[[graph.edges]]
from = "entrance"
to = "azure_key"
portal = "door.standard"

[[graph.edges]]
from = "azure_key"
to = "azure_gate"
portal = "door.standard"

[[graph.edges]]
from = "azure_gate"
to = "boss"
portal = "door.locked"
requires = ["item.sigil.azur"]

[[constraints.hard]]
id = "key_before_gate"
kind = "dominance"
grant = "item.sigil.azur"
require = "azure_gate"

[[constraints.hard]]
id = "boss_reachable"
kind = "reachability"
from = "entrance"
to = ["boss"]

[[constraints.hard]]
id = "capsule_clearance"
kind = "clearance"
radius_m = 0.30
height_m = 2.00

[[objectives.soft]]
id = "combat_pacing"
metric = "encounter_spacing_m"
weight = 0.5
target = { min = 18.0, max = 45.0 }

[[objectives.soft]]
id = "run_variety"
metric = "room_archetype_entropy"
weight = 0.4
target = { min = 0.65 }
```

## 5. Exemple travaillé — Véhicule modulaire

```toml
schema_version = "forgia.content-spec/v1"
id = "forgia.vehicle.scout.bourrasque@0.1.0"
kind = "vehicle"
title = "Éclaireur Bourrasque"

[generation]
seed = 0xB0A55201
solver = "csp_wfc"
registry_lock = "pcg-registry/registry.lock.toml"

[intent]
tags = ["faction.forgia", "vehicle.scout", "style.cartoon"]
requires = ["vehicle.chassis.light", "vehicle.power_bus", "vehicle.mobility.wheel"]
forbids = ["vehicle.weapon.heavy"]

[domain.vehicle]
wheel_count = 4
mass_kg = { min = 700.0, max = 1100.0 }
power_kw = { min = 90.0, max = 150.0 }
required_buses = ["power.48v", "data.can"]

[[zones]]
id = "chassis"
kind = "vehicle.chassis.light"
requires = ["socket.mount.engine", "socket.mount.axle", "socket.mount.cabin"]

[[zones]]
id = "engine"
kind = "vehicle.power.unit"
requires = ["socket.power.48v", "socket.mount.engine"]

[[zones]]
id = "front_axle"
kind = "vehicle.axle.steer"
requires = ["socket.mount.axle", "socket.data.can"]

[[graph.edges]]
from = "engine"
to = "chassis"
interface = "mount.engine"

[[graph.edges]]
from = "engine"
to = "chassis"
interface = "power.48v"

[[graph.edges]]
from = "front_axle"
to = "chassis"
interface = "mount.axle"

[[constraints.hard]]
id = "all_buses_connected"
kind = "network_connected"
networks = ["power.48v", "data.can"]

[[constraints.hard]]
id = "wheel_ground_clearance"
kind = "clearance"
min_m = 0.18

[[objectives.soft]]
id = "balanced_mass"
metric = "center_of_mass_offset_m"
weight = 0.7
target = { max = 0.15 }
```

## 6. Compilation et erreurs utiles

Le compilateur doit émettre des diagnostics attachés à l'ID de spec, non des
panics : `unknown_capability`, `unresolved_registry_id`, `incompatible_bounds`,
`hard_constraint_unsatisfiable`, `budget_impossible`, `non_reproducible_seed`.
Un `--explain` exporte le sous-graphe ou l'ensemble minimal de contraintes en
contradiction ; c'est essentiel pour les créateurs et pour l'agent IA.
