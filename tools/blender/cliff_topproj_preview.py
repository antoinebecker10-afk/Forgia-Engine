"""Reproduit le shader « top projection » du créateur sur les falaises du château :
albedo TERRE TAN (T_ENV_ground) sur les flancs + HERBE (T_ENV_grass) sur les faces
dont la normale monde pointe vers le haut. Rend un contrôle Cycles.

Usage :
  blender --background --factory-startup <castle_highlands.blend/glb> --python \
    tools/blender/cliff_topproj_preview.py -- --tan <png> --grass <png> --out <dir>
"""

import argparse
import math
import sys
from pathlib import Path

import bpy
from mathutils import Matrix, Vector

CASTLE = "assets/models/environment/castle/castle_highlands.glb"


def cli():
    p = argparse.ArgumentParser()
    p.add_argument("--tan", required=True)
    p.add_argument("--grass", required=True)
    p.add_argument("--out", required=True, type=Path)
    p.add_argument("--samples", type=int, default=24)
    p.add_argument("--tile", type=float, default=8.0)
    p.add_argument("--from-min", type=float, default=0.35)
    p.add_argument("--from-max", type=float, default=0.72)
    p.add_argument("--apply-only", action="store_true", help="applique le mat + sauve le glb, pas de rendu")
    p.add_argument("--save-glb", default="")
    return p.parse_args(sys.argv[sys.argv.index("--") + 1 :])


def make_topproj_material(name, tan_path, grass_path, tile, fmin, fmax):
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    nt = mat.node_tree
    for n in list(nt.nodes):
        nt.nodes.remove(n)
    out = nt.nodes.new("ShaderNodeOutputMaterial")
    bsdf = nt.nodes.new("ShaderNodeBsdfPrincipled")
    bsdf.inputs["Roughness"].default_value = 0.9
    nt.links.new(bsdf.outputs["BSDF"], out.inputs["Surface"])

    tc = nt.nodes.new("ShaderNodeTexCoord")

    def tex(path):
        img = bpy.data.images.load(str(Path(path).resolve()))
        node = nt.nodes.new("ShaderNodeTexImage")
        node.image = img
        m = nt.nodes.new("ShaderNodeMapping")
        m.inputs["Scale"].default_value = (tile, tile, tile)
        nt.links.new(tc.outputs["Generated"], m.inputs["Vector"])
        nt.links.new(m.outputs["Vector"], node.inputs["Vector"])
        return node

    tan = tex(tan_path)
    grass = tex(grass_path)

    # masque du dessus : normale MONDE .Z remappée en [0,1] doux
    geo = nt.nodes.new("ShaderNodeNewGeometry")
    sep = nt.nodes.new("ShaderNodeSeparateXYZ")
    nt.links.new(geo.outputs["Normal"], sep.inputs["Vector"])
    mr = nt.nodes.new("ShaderNodeMapRange")
    mr.inputs["From Min"].default_value = fmin
    mr.inputs["From Max"].default_value = fmax
    nt.links.new(sep.outputs["Z"], mr.inputs["Value"])

    mix = nt.nodes.new("ShaderNodeMix")
    mix.data_type = "RGBA"
    nt.links.new(mr.outputs["Result"], mix.inputs["Factor"])
    nt.links.new(tan.outputs["Color"], mix.inputs["A"])
    nt.links.new(grass.outputs["Color"], mix.inputs["B"])
    nt.links.new(mix.outputs["Result"], bsdf.inputs["Base Color"])
    return mat


def look_at(loc, target):
    fwd = (target - loc).normalized()
    z = -fwd
    up = Vector((0, 0, 1))
    if abs(z.dot(up)) > 0.99:
        up = Vector((0, 1, 0))
    x = up.cross(z).normalized()
    y = z.cross(x)
    return Matrix(((x.x, y.x, z.x, loc.x), (x.y, y.y, z.y, loc.y), (x.z, y.z, z.z, loc.z), (0, 0, 0, 1)))


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
    bpy.ops.import_scene.gltf(filepath=str(Path(CASTLE).resolve()))

    mat = make_topproj_material("M_CliffTopProj", args.tan, args.grass, args.tile, args.from_min, args.from_max)
    cliffs = [o for o in bpy.data.objects if o.type == "MESH" and "cliff" in o.name.lower()]
    for o in cliffs:
        o.data.materials.clear()
        o.data.materials.append(mat)
    print(f"CLIFF_MAT_APPLIED cliffs={len(cliffs)}")

    if args.save_glb:
        bpy.ops.object.select_all(action="SELECT")
        bpy.ops.export_scene.gltf(filepath=str(Path(args.save_glb).resolve()), export_format="GLB", use_selection=True, export_apply=False)
        print(f"SAVED_GLB {args.save_glb}")
    if args.apply_only:
        return

    all_m = [o for o in bpy.data.objects if o.type == "MESH"]
    lo, hi = bounds(all_m)
    centre = (lo + hi) * 0.5
    radius = max((hi - lo).length * 0.5, 1.0)

    sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN"))
    sun.data.energy = 3.2
    sun.rotation_euler = (math.radians(58), 0, math.radians(35))
    bpy.context.scene.collection.objects.link(sun)
    sc = bpy.context.scene
    w = sc.world or bpy.data.worlds.new("w")
    sc.world = w
    w.use_nodes = True
    bg = w.node_tree.nodes.get("Background")
    if bg:
        bg.inputs[0].default_value = (0.55, 0.75, 0.95, 1.0)
    sc.render.engine = "CYCLES"
    sc.cycles.samples = args.samples
    sc.cycles.device = "CPU"
    sc.render.resolution_x = 1600
    sc.render.resolution_y = 900
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    cam.data.lens = 40
    bpy.context.scene.collection.objects.link(cam)
    dist = radius * 2.2
    az, el = math.radians(35), math.radians(10)
    off = Vector((math.cos(el) * math.cos(az), -math.cos(el) * math.sin(az), math.sin(el)))
    cam.matrix_world = look_at(centre + off * dist, centre)
    sc.camera = cam
    args.out.mkdir(parents=True, exist_ok=True)
    sc.render.filepath = str((args.out / "cliff_topproj.png").resolve())
    bpy.ops.render.render(write_still=True)
    print(f"CHECK_OK -> {sc.render.filepath}")


if __name__ == "__main__":
    main()
