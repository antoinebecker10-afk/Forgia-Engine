"""Rendu de contrôle : charge N GLB (terrain + château + végétation déjà exportés)
et rend une vue 3/4 basse. Sert à valider le look final sans re-placer.

Usage : blender -b --factory-startup --python tools/blender/render_glbs.py -- \
  --glb a.glb b.glb --out <png> [--focus x y z] [--dist D] [--el DEG]
"""
import argparse, math, sys
from pathlib import Path
import bpy
from mathutils import Vector, Matrix


def cli():
    p = argparse.ArgumentParser()
    p.add_argument("--glb", nargs="+", required=True)
    p.add_argument("--out", required=True, type=Path)
    p.add_argument("--focus", nargs=3, type=float, default=[-13.0, 30.0, 38.0])
    p.add_argument("--dist", type=float, default=120.0)
    p.add_argument("--el", type=float, default=8.0)
    p.add_argument("--az", type=float, default=35.0)
    p.add_argument("--samples", type=int, default=16)
    p.add_argument("--top", action="store_true", help="vue de dessus orthographique (cadrage garanti)")
    p.add_argument("--ortho", type=float, default=340.0)
    return p.parse_args(sys.argv[sys.argv.index("--") + 1:])


def look_at(loc, tgt):
    f = (tgt - loc).normalized(); z = -f
    up = Vector((0, 0, 1))
    if abs(z.dot(up)) > 0.99:
        up = Vector((0, 1, 0))
    x = up.cross(z).normalized(); y = z.cross(x)
    return Matrix(((x.x, y.x, z.x, loc.x), (x.y, y.y, z.y, loc.y), (x.z, y.z, z.z, loc.z), (0, 0, 0, 1)))


def main():
    a = cli()
    for o in list(bpy.data.objects):
        bpy.data.objects.remove(o, do_unlink=True)
    for g in a.glb:
        bpy.ops.import_scene.gltf(filepath=str(Path(g).resolve()))
    sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN"))
    sun.data.energy = 3.5
    sun.rotation_euler = (math.radians(50), math.radians(8), math.radians(35))
    bpy.context.scene.collection.objects.link(sun)
    sc = bpy.context.scene
    w = bpy.data.worlds.new("w"); sc.world = w; w.use_nodes = True
    for n in w.node_tree.nodes:
        if n.type == "BACKGROUND":
            n.inputs[0].default_value = (0.5, 0.7, 0.92, 1.0)
    sc.render.engine = "CYCLES"; sc.cycles.samples = a.samples; sc.cycles.device = "CPU"
    sc.render.resolution_x = 1600; sc.render.resolution_y = 900
    focus = Vector(a.focus)
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    sc.collection.objects.link(cam)
    if a.top:
        cam.data.type = "ORTHO"
        cam.data.ortho_scale = a.ortho
        cam.matrix_world = look_at(Vector((focus.x, focus.y, focus.z + 300.0)), focus)
    else:
        cam.data.lens = 40
        az, el = math.radians(a.az), math.radians(a.el)
        off = Vector((math.cos(el) * math.cos(az), -math.cos(el) * math.sin(az), math.sin(el)))
        cam.matrix_world = look_at(focus + off * a.dist, focus)
    sc.camera = cam
    a.out.parent.mkdir(parents=True, exist_ok=True)
    sc.render.filepath = str(a.out.resolve())
    bpy.ops.render.render(write_still=True)
    print(f"CHECK_OK -> {sc.render.filepath}")


if __name__ == "__main__":
    main()
