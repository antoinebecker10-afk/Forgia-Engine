"""Build the walkable collision proxy for the imported Highlands Castle.

Usage (from the repository root):
  & "C:\\Program Files\\Blender Foundation\\Blender 5.0\\blender.exe" --background \
    --factory-startup --python tools/blender/build_castle_collision.py

The visual castle has thousands of instances. Runtime `AsyncSceneCollider` on it
would create a collider per instance and freeze the game. This offline step keeps
only floors and stairs, which must never be removed by the structural-collider
decimation. It is loaded alongside the coarse walls/cliffs proxy at runtime.
"""

from pathlib import Path

import bpy
from mathutils import Vector


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "assets/models/environment/castle/castle_highlands.glb"
OUTPUT = ROOT / "assets/models/environment/castle/castle_highlands_walkable_runtime.glb"

# Intentionally exclude trees, bushes, furniture, candles, carpets, roofs and
# visual trims. They should not snag the player controller. The retained names
# come from the Unity → Blender import and describe load-bearing walkable parts.
STRUCTURAL_PREFIXES = (
    "SM_MOD_floor",
    "SM_MOD_stairs",
    "SM_MOD_wall_big",
    "SM_MOD_wall_small",
    "SM_MOD_wall_sloped",
    "SM_MOD_citywall",
    "SM_MOD_tower_part_wall",
    "SM_MOD_tower_segment_wall",
    "SM_MOD_pillar_castle",
    "SM_ENV_cliff",
)

# Une décimation globale à 0.006 a supprimé le triangle de plancher sous le
# spawn du Hall : le joueur tombait alors sur le terrain à Y≈-10.  Les surfaces
# jouables sont donc conservées, les escaliers restent lisibles, tandis que les
# masses verticales (murs/falaises) portent l'essentiel de la réduction.
DECIMATE_RATIOS = {
    "floors": 1.0,
    "stairs": 0.08,
}
WALKABLE_GROUPS = ("floors", "stairs")

# Coordonnées Blender de GREAT_HALL_SPAWN. Le GLB est Z-up dans Blender et Y-up
# dans Bevy, d'où (x, -z, y). Cette sentinelle protège l'invariant le plus
# important : un spawn ne peut pas être publié sans support physique.
GREAT_HALL_SPAWN_BLENDER = Vector((10.321, -35.625, 1000.0))
GREAT_HALL_FLOOR_Z_RANGE = (36.0, 37.0)


def clear_scene() -> None:
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)


def classify_structural_meshes() -> dict[str, list[bpy.types.Object]]:
    groups: dict[str, list[bpy.types.Object]] = {"floors": [], "stairs": []}
    for obj in bpy.context.scene.objects:
        if obj.type != "MESH":
            continue
        name = obj.name
        if name.startswith("SM_MOD_floor"):
            groups["floors"].append(obj)
        elif name.startswith("SM_MOD_stairs"):
            groups["stairs"].append(obj)
    return groups


def join_group(name: str, objects: list[bpy.types.Object]) -> bpy.types.Object | None:
    if not objects:
        return None
    bpy.ops.object.select_all(action="DESELECT")
    for obj in objects:
        obj.select_set(True)
    bpy.context.view_layer.objects.active = objects[0]
    bpy.ops.object.join()
    proxy = bpy.context.active_object
    proxy.name = f"CastleCollision_{name}"
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)

    ratio = DECIMATE_RATIOS[name]
    if ratio < 1.0:
        decimate = proxy.modifiers.new(name=f"CollisionDecimate_{name}", type="DECIMATE")
        decimate.decimate_type = "COLLAPSE"
        decimate.ratio = ratio
        bpy.ops.object.modifier_apply(modifier=decimate.name)
    return proxy


def verify_great_hall_support(proxy: bpy.types.Object) -> float:
    inverse = proxy.matrix_world.inverted()
    origin = inverse @ GREAT_HALL_SPAWN_BLENDER
    direction = inverse.to_3x3() @ Vector((0.0, 0.0, -1.0))
    hit, location, _normal, _face = proxy.ray_cast(origin, direction)
    if not hit:
        raise RuntimeError("collision proxy has no support under GREAT_HALL_SPAWN")
    world_location = proxy.matrix_world @ location
    if not GREAT_HALL_FLOOR_Z_RANGE[0] <= world_location.z <= GREAT_HALL_FLOOR_Z_RANGE[1]:
        raise RuntimeError(
            f"collision floor at spawn is z={world_location.z:.3f}, expected "
            f"{GREAT_HALL_FLOOR_Z_RANGE}"
        )
    return world_location.z


def main() -> None:
    if not SOURCE.is_file():
        raise RuntimeError(f"missing source castle: {SOURCE}")

    clear_scene()
    bpy.ops.import_scene.gltf(filepath=str(SOURCE))
    groups = classify_structural_meshes()
    walkable_count = sum(len(group) for group in groups.values())
    if not walkable_count:
        raise RuntimeError("no walkable castle meshes matched the collision filter")

    source_faces = sum(
        len(obj.data.polygons) for group in groups.values() for obj in group
    )
    proxies = [
        proxy for name, group in groups.items()
        if (proxy := join_group(name, group)) is not None
    ]
    if not proxies:
        raise RuntimeError("collision groups unexpectedly empty after join")

    bpy.ops.object.select_all(action="DESELECT")
    for proxy in proxies:
        proxy.select_set(True)
    bpy.context.view_layer.objects.active = proxies[0]
    bpy.ops.object.join()
    proxy = bpy.context.active_object
    proxy.name = "CastleHighlandsCollision"
    floor_z = verify_great_hall_support(proxy)

    # Collision-only asset: no materials or textures need to be decoded by Bevy.
    proxy.data.materials.clear()
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    bpy.ops.export_scene.gltf(
        filepath=str(OUTPUT),
        export_format="GLB",
        use_selection=True,
        export_materials="NONE",
        export_normals=False,
        export_texcoords=False,
        export_yup=True,
    )
    print(
        "CASTLE_COLLISION_BUILT "
        f"source_faces={source_faces} proxy_faces={len(proxy.data.polygons)} "
        f"great_hall_floor_z={floor_z:.3f} ratios={DECIMATE_RATIOS} output={OUTPUT}"
    )


if __name__ == "__main__":
    main()
