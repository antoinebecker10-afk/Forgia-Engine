"""Rendu Cycles de contrôle d'un layout assemblé de kit PCG.

Charge le .blend source d'un kit (conventions PCG_*), duplique les meshes de
RENDU de chaque pièce aux transforms d'un layout JSON (coordonnées Forgia Y-up,
produites par le solveur et figées en golden test), puis rend une vue 3/4 et une
vue du dessus en Cycles (EEVEE headless crashe sans GPU — piège documenté).

Usage :
  blender --background --factory-startup assets/source/castle_stone_greybox.blend \
    --python tools/blender/render_pcg_kit_preview.py -- \
    --layout assets/pcg/kits/castle_stone/1.0.0/previews/slice_layout.json \
    --out assets/pcg/kits/castle_stone/1.0.0/previews

Conversion Forgia→Blender : forgia (x, y, z) → blender (x, -z, y) ; un yaw
Forgia autour de +Y = même angle autour de +Z Blender.
"""

import argparse
import json
import math
import sys
from pathlib import Path

import bpy
from mathutils import Matrix, Vector


PIECE_PREFIX = "PCG_PIECE__"
ROOT_PREFIX = "PCG_ROOT__"
SOCKET_PREFIX = "PCG_SOCKET__"
COLLISION_PREFIX = "PCG_COLLISION__"
LOD_PREFIX = "PCG_LOD"


def cli_args():
    if "--" not in sys.argv:
        raise SystemExit("Arguments attendus après `--`.")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--layout", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--samples", type=int, default=32)
    parser.add_argument("--width", type=int, default=1600)
    parser.add_argument("--height", type=int, default=900)
    return parser.parse_args(sys.argv[sys.argv.index("--") + 1 :])


def fail(message):
    raise SystemExit(f"PCG_KIT_PREVIEW_ERROR: {message}")


def objects_recursive(collection):
    objects = list(collection.objects)
    for child in collection.children:
        objects.extend(objects_recursive(child))
    return objects


def render_meshes_of(piece_id):
    collection = bpy.data.collections.get(PIECE_PREFIX + piece_id)
    if collection is None:
        fail(f"collection `{PIECE_PREFIX}{piece_id}` introuvable dans le .blend")
    objects = objects_recursive(collection)
    roots = [obj for obj in objects if obj.name == ROOT_PREFIX + piece_id]
    if len(roots) != 1:
        fail(f"{piece_id}: exige exactement un `{ROOT_PREFIX}{piece_id}`")
    meshes = [
        obj
        for obj in objects
        if obj.type == "MESH"
        and not obj.name.startswith(COLLISION_PREFIX)
        and not obj.name.startswith(LOD_PREFIX)
    ]
    if not meshes:
        fail(f"{piece_id}: aucun mesh de rendu")
    return roots[0], meshes


def forgia_to_blender_matrix(translation_m, yaw_deg):
    """Transform monde Blender d'une instance Forgia (Y-up, yaw autour de +Y)."""
    x, y, z = translation_m
    location = Matrix.Translation(Vector((x, -z, y)))
    return location @ Matrix.Rotation(math.radians(yaw_deg), 4, "Z")


def place_layout(layout, stage):
    placed = []
    for index, instance in enumerate(layout["instances"]):
        root, meshes = render_meshes_of(instance["piece"])
        world = forgia_to_blender_matrix(
            instance["translation_m"], instance.get("yaw_deg", 0.0)
        )
        root_inverse = root.matrix_world.inverted()
        for obj in meshes:
            duplicate = obj.copy()  # data partagée : lecture seule
            duplicate.name = f"PREVIEW_{index}_{obj.name}"
            duplicate.matrix_world = world @ (root_inverse @ obj.matrix_world)
            stage.objects.link(duplicate)
            placed.append(duplicate)
    return placed


def scene_bounds(objects):
    lo = Vector((math.inf,) * 3)
    hi = Vector((-math.inf,) * 3)
    for obj in objects:
        for corner in obj.bound_box:
            world = obj.matrix_world @ Vector(corner)
            lo = Vector(map(min, lo, world))
            hi = Vector(map(max, hi, world))
    return lo, hi


def look_at_matrix(position, target):
    forward = (target - position).normalized()
    z_axis = -forward  # la caméra Blender regarde le long de -Z
    up = Vector((0.0, 0.0, 1.0))
    if abs(z_axis.dot(up)) > 0.99:
        up = Vector((0.0, 1.0, 0.0))
    x_axis = up.cross(z_axis).normalized()
    y_axis = z_axis.cross(x_axis)
    return Matrix(
        (
            (x_axis.x, y_axis.x, z_axis.x, position.x),
            (x_axis.y, y_axis.y, z_axis.y, position.y),
            (x_axis.z, y_axis.z, z_axis.z, position.z),
            (0.0, 0.0, 0.0, 1.0),
        )
    )


def add_ground(stage, centre, radius):
    mesh = bpy.data.meshes.new("PREVIEW_ground")
    half = radius * 4.0
    mesh.from_pydata(
        [
            (centre.x - half, centre.y - half, 0.0),
            (centre.x + half, centre.y - half, 0.0),
            (centre.x + half, centre.y + half, 0.0),
            (centre.x - half, centre.y + half, 0.0),
        ],
        [],
        [(0, 1, 2, 3)],
    )
    mesh.update()
    ground = bpy.data.objects.new("PREVIEW_ground", mesh)
    stage.objects.link(ground)


def add_suns(stage):
    # Clé + fill opposé : aucune face totalement noire, même si l'API World de
    # Blender 5 ignore le réglage d'ambiance.
    for name, energy, yaw_deg in (("PREVIEW_sun", 3.0, 30.0), ("PREVIEW_fill", 1.0, 210.0)):
        light = bpy.data.lights.new(name, type="SUN")
        light.energy = energy
        sun = bpy.data.objects.new(name, light)
        sun.rotation_euler = (math.radians(50.0), 0.0, math.radians(yaw_deg))
        stage.objects.link(sun)


def hide_originals_from_render():
    """Les pièces sources sont toutes authored à l'origine : sans ça elles
    rendent empilées au centre du layout (bug attrapé par la vue top-down)."""
    for obj in bpy.data.objects:
        if not obj.name.startswith("PREVIEW_"):
            obj.hide_render = True


def render(camera, filepath, scene):
    scene.camera = camera
    scene.render.filepath = str(filepath)
    bpy.ops.render.render(write_still=True)
    print(f"PCG_KIT_PREVIEW_OK {filepath}")


def main():
    args = cli_args()
    # Chemins absolus obligatoires : un `render.filepath` relatif part dans le
    # vide en headless (perte silencieuse constatée — Blender affiche pourtant OK).
    args.layout = args.layout.resolve()
    args.out = args.out.resolve()
    layout = json.loads(args.layout.read_text(encoding="utf-8"))
    scene = bpy.context.scene

    stage = bpy.data.collections.new("__PCG_PREVIEW__")
    scene.collection.children.link(stage)
    placed = place_layout(layout, stage)
    hide_originals_from_render()
    lo, hi = scene_bounds(placed)
    centre = (lo + hi) * 0.5
    radius = max((hi - lo).length * 0.5, 1.0)
    add_ground(stage, centre, radius)
    add_suns(stage)

    # Fond neutre clair pour lire les volumes grey-box.
    world = scene.world or bpy.data.worlds.new("PREVIEW_world")
    scene.world = world
    world.use_nodes = True
    background = world.node_tree.nodes.get("Background")
    if background is not None:
        background.inputs[0].default_value = (0.85, 0.87, 0.90, 1.0)
        background.inputs[1].default_value = 1.0

    scene.render.engine = "CYCLES"
    scene.cycles.samples = args.samples
    scene.cycles.device = "CPU"
    scene.render.resolution_x = args.width
    scene.render.resolution_y = args.height
    scene.render.image_settings.file_format = "PNG"

    camera_data = bpy.data.cameras.new("PREVIEW_cam")
    camera_data.lens = 35.0  # plus large que le 50mm défaut : tout l'assemblage
    camera = bpy.data.objects.new("PREVIEW_cam", camera_data)
    stage.objects.link(camera)
    args.out.mkdir(parents=True, exist_ok=True)

    # Vue 3/4 : azimut ~35°, élévation ~28°, distance cadrant tout l'assemblage.
    distance = radius * 3.2
    azimuth, elevation = math.radians(35.0), math.radians(28.0)
    offset = Vector(
        (
            math.cos(elevation) * math.cos(azimuth),
            -math.cos(elevation) * math.sin(azimuth),
            math.sin(elevation),
        )
    )
    camera.matrix_world = look_at_matrix(centre + offset * distance, centre)
    render(camera, args.out / "slice_34.png", scene)

    # Vue du dessus.
    camera.matrix_world = look_at_matrix(
        centre + Vector((0.0, 0.0, distance)), centre
    )
    render(camera, args.out / "slice_top.png", scene)

    print(f"PCG_KIT_PREVIEW_SUMMARY instances={len(layout['instances'])} out={args.out}")


if __name__ == "__main__":
    main()
