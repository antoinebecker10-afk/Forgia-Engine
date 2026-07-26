"""Rendu de contrôle : terrain gazon rehaussé + château, pour voir si l'herbe
remonte bien sous les dalles (plus de vide). Applique l'align runtime Y-2,70 au
terrain (hauteur = Blender Z). Cycles (EEVEE headless crashe).

Usage :
  blender --background --factory-startup --python tools/blender/check_terrain_vs_castle.py -- --out <dir>
"""

import argparse
import math
import sys
from pathlib import Path

import bpy
from mathutils import Matrix, Vector

BASE = "assets/models/environment/castle"
TERRAIN = f"{BASE}/castle_terrain.glb"
CASTLE = f"{BASE}/castle_highlands.glb"
ALIGN_Y = -2.70  # align runtime jeu (Y) = Blender Z


def cli():
    p = argparse.ArgumentParser()
    p.add_argument("--out", required=True, type=Path)
    p.add_argument("--samples", type=int, default=24)
    p.add_argument("--skip-terrain", action="store_true")
    p.add_argument("--name", default="terrain_castle_low")
    return p.parse_args(sys.argv[sys.argv.index("--") + 1 :])


def look_at(loc, target):
    fwd = (target - loc).normalized()
    z = -fwd
    up = Vector((0, 0, 1))
    if abs(z.dot(up)) > 0.99:
        up = Vector((0, 1, 0))
    x = up.cross(z).normalized()
    y = z.cross(x)
    return Matrix(
        ((x.x, y.x, z.x, loc.x), (x.y, y.y, z.y, loc.y), (x.z, y.z, z.z, loc.z), (0, 0, 0, 1))
    )


def bounds(objs):
    lo = Vector((math.inf,) * 3)
    hi = Vector((-math.inf,) * 3)
    for o in objs:
        for c in o.bound_box:
            w = o.matrix_world @ Vector(c)
            lo = Vector(map(min, lo, w))
            hi = Vector(map(max, hi, w))
    return lo, hi


def main():
    args = cli()
    for o in list(bpy.data.objects):
        bpy.data.objects.remove(o, do_unlink=True)

    terrain = []
    if not args.skip_terrain:
        bpy.ops.import_scene.gltf(filepath=str(Path(TERRAIN).resolve()))
        terrain = [o for o in bpy.data.objects if o.type == "MESH"]
        for o in terrain:
            o.location.z += ALIGN_Y  # align runtime

    bpy.ops.import_scene.gltf(filepath=str(Path(CASTLE).resolve()))
    castle = [o for o in bpy.data.objects if o.type == "MESH" and o not in terrain]

    lo, hi = bounds(terrain + castle)
    centre = (lo + hi) * 0.5
    radius = max((hi - lo).length * 0.5, 1.0)

    # Soleil crépusculaire + ambiance.
    light = bpy.data.lights.new("sun", "SUN")
    light.energy = 3.5
    sun = bpy.data.objects.new("sun", light)
    sun.rotation_euler = (math.radians(62), 0, math.radians(35))
    bpy.context.scene.collection.objects.link(sun)

    scene = bpy.context.scene
    world = scene.world or bpy.data.worlds.new("w")
    scene.world = world
    world.use_nodes = True
    bg = world.node_tree.nodes.get("Background")
    if bg:
        bg.inputs[0].default_value = (0.85, 0.70, 0.62, 1.0)
        bg.inputs[1].default_value = 1.0

    scene.render.engine = "CYCLES"
    scene.cycles.samples = args.samples
    scene.cycles.device = "CPU"
    scene.render.resolution_x = 1600
    scene.render.resolution_y = 900
    scene.render.image_settings.file_format = "PNG"

    cam_data = bpy.data.cameras.new("cam")
    cam_data.lens = 32.0
    cam = bpy.data.objects.new("cam", cam_data)
    bpy.context.scene.collection.objects.link(cam)
    args.out.mkdir(parents=True, exist_ok=True)

    # Vue 3/4 basse (rasante) pour voir l'herbe rencontrer la base du château.
    dist = radius * 2.4
    az, el = math.radians(40), math.radians(12)
    off = Vector((math.cos(el) * math.cos(az), -math.cos(el) * math.sin(az), math.sin(el)))
    cam.matrix_world = look_at(centre + off * dist, centre)
    scene.camera = cam
    scene.render.filepath = str((args.out / f"{args.name}.png").resolve())
    bpy.ops.render.render(write_still=True)
    print(f"CHECK_OK -> {scene.render.filepath}")


if __name__ == "__main__":
    main()
