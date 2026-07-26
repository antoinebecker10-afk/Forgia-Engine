"""Encode le look « top projection » du créateur (roche TAN + herbe sur le dessus)
dans les VERTEX COLORS des falaises, puis re-tranche les cellules de streaming.

Les textures T_ENV_ground / T_ENV_grass étant quasi-uniformes, une couleur sommet
(mix tan/herbe piloté par la normale monde Z) reproduit fidèlement le shader Unity
perdu à l'export — et survit intégralement au slicing (aucune texture ajoutée).
Bevy multiplie base_color × couleur sommet nativement.

Modes :
  --render  : applique le mat vertex-color aux falaises et rend un contrôle.
  --reslice : applique + re-tranche vers castle_stream_cells_grass/.

Couleurs LINÉAIRES (Bevy lit COLOR_0 en linéaire) — mesurées sur les PNG créateur.
"""

import argparse
import math
import sys
from collections import defaultdict
from pathlib import Path

import bpy
from mathutils import Matrix, Vector

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "assets/models/environment/castle/castle_highlands.glb"
CELL_SIZE_M = 32.0

# Moyennes linéaires mesurées (tools : PIL sur T_ENV_*_castle_BC.png).
TAN_LIN = (0.2628, 0.1664, 0.0692)
GRASS_LIN = (0.1788, 0.4293, 0.0231)


def cli():
    p = argparse.ArgumentParser()
    p.add_argument("--mode", choices=["render", "reslice"], required=True)
    p.add_argument("--normal", default="", help="normal map falaise partagée (option)")
    p.add_argument("--out", type=Path, default=None)
    p.add_argument("--outdir", default="castle_stream_cells_grass")
    p.add_argument("--samples", type=int, default=24)
    p.add_argument("--from-min", type=float, default=0.62)
    p.add_argument("--from-max", type=float, default=0.9)
    return p.parse_args(sys.argv[sys.argv.index("--") + 1 :])


def smoothstep(t):
    t = max(0.0, min(1.0, t))
    return t * t * (3.0 - 2.0 * t)


def bake_vertex_colors(obj, fmin, fmax):
    mesh = obj.data
    nmat = obj.matrix_world.to_3x3().inverted_safe().transposed()
    name = "Col"
    ca = mesh.color_attributes
    if name in ca:
        ca.remove(ca[name])
    col = ca.new(name=name, type="FLOAT_COLOR", domain="POINT")
    span = max(1e-4, fmax - fmin)
    for i, v in enumerate(mesh.vertices):
        wn = (nmat @ v.normal)
        wn.normalize()
        t = smoothstep((wn.z - fmin) / span)
        c = tuple(TAN_LIN[k] * (1 - t) + GRASS_LIN[k] * t for k in range(3))
        col.data[i].color = (c[0], c[1], c[2], 1.0)
    try:
        ca.active_color = col
        mesh.attributes.active_color_index = list(mesh.attributes).index(col)
    except Exception:
        pass


def make_vc_material(normal_path):
    mat = bpy.data.materials.new("M_CliffGrassVC")
    mat.use_nodes = True
    nt = mat.node_tree
    for n in list(nt.nodes):
        nt.nodes.remove(n)
    out = nt.nodes.new("ShaderNodeOutputMaterial")
    bsdf = nt.nodes.new("ShaderNodeBsdfPrincipled")
    bsdf.inputs["Roughness"].default_value = 0.92
    nt.links.new(bsdf.outputs["BSDF"], out.inputs["Surface"])
    attr = nt.nodes.new("ShaderNodeAttribute")
    attr.attribute_name = "Col"
    attr.attribute_type = "GEOMETRY"
    nt.links.new(attr.outputs["Color"], bsdf.inputs["Base Color"])
    if normal_path:
        img = bpy.data.images.load(str(Path(normal_path).resolve()))
        img.colorspace_settings.name = "Non-Color"
        tex = nt.nodes.new("ShaderNodeTexImage")
        tex.image = img
        m = nt.nodes.new("ShaderNodeMapping")
        m.inputs["Scale"].default_value = (7, 7, 7)
        tc = nt.nodes.new("ShaderNodeTexCoord")
        nt.links.new(tc.outputs["Generated"], m.inputs["Vector"])
        nt.links.new(m.outputs["Vector"], tex.inputs["Vector"])
        nmap = nt.nodes.new("ShaderNodeNormalMap")
        nmap.inputs["Strength"].default_value = 0.6
        nt.links.new(tex.outputs["Color"], nmap.inputs["Color"])
        nt.links.new(nmap.outputs["Normal"], bsdf.inputs["Normal"])
    return mat


def apply_to_cliffs(mat, fmin, fmax):
    cliffs = [o for o in bpy.data.objects if o.type == "MESH" and "cliff" in o.name.lower()]
    for o in cliffs:
        bake_vertex_colors(o, fmin, fmax)
        o.data.materials.clear()
        o.data.materials.append(mat)
    print(f"CLIFF_VC_APPLIED cliffs={len(cliffs)}")
    return cliffs


# ---- géométrie cellules (repris de slice_castle_stream_cells.py) ----
def world_bounds(obj):
    pts = [obj.matrix_world @ Vector(c) for c in obj.bound_box]
    return (min(p.x for p in pts), max(p.x for p in pts), min(p.y for p in pts),
            max(p.y for p in pts), min(p.z for p in pts), max(p.z for p in pts))


def cell_of(b):
    min_x, max_x, min_y, max_y, _, _ = b
    cx = (min_x + max_x) * 0.5
    cz = -((min_y + max_y) * 0.5)
    return (int(cx // CELL_SIZE_M), int(cz // CELL_SIZE_M))


def reslice(outdir):
    OUT = ROOT / "assets/models/environment/castle" / outdir
    if OUT.exists():
        raise RuntimeError(f"refuse d'écraser {OUT} — déplace-le ou supprime-le d'abord.")
    OUT.mkdir(parents=True)
    cells = defaultdict(list)
    cbounds = {}
    for obj in bpy.context.scene.objects:
        if obj.type != "MESH" or obj.hide_render:
            continue
        b = world_bounds(obj)
        c = cell_of(b)
        cells[c].append(obj)
        prior = cbounds.get(c)
        if prior is None:
            cbounds[c] = list(b)
        else:
            for k in (0, 2, 4):
                prior[k] = min(prior[k], b[k])
            for k in (1, 3, 5):
                prior[k] = max(prior[k], b[k])
    lines = [
        "# Generated by tools/blender/bake_cliff_grass_cells.py — do not hand edit.",
        "schema_version = 1", f"cell_size_m = {CELL_SIZE_M:.1f}",
        'coordinate_system = "bevy_y_up_meters"',
        'asset_format = "gltf_separate_shared_texture_library"', "",
    ]
    total = 0
    for c in sorted(cells):
        objs = cells[c]
        bpy.ops.object.select_all(action="DESELECT")
        for o in objs:
            o.select_set(True)
        bpy.context.view_layer.objects.active = objs[0]
        fp = OUT / f"cell_x{c[0]}_z{c[1]}_render.gltf"
        bpy.ops.export_scene.gltf(
            filepath=str(fp), export_format="GLTF_SEPARATE", export_texture_dir="textures",
            export_keep_originals=False, use_selection=True, export_apply=False,
            export_yup=True, export_vertex_color="ACTIVE",
        )
        b = cbounds[c]
        bevy_min = (b[0], b[4], -b[3])
        bevy_max = (b[1], b[5], -b[2])
        rel = fp.relative_to(ROOT / "assets").as_posix()
        lines += ["[[cells]]", f'id = "cell_x{c[0]}_z{c[1]}"', f'render = "{rel}#Scene0"',
                  "bounds_min_m = [%.3f, %.3f, %.3f]" % bevy_min,
                  "bounds_max_m = [%.3f, %.3f, %.3f]" % bevy_max,
                  f"source_meshes = {len(objs)}", ""]
        total += len(objs)
    (OUT / "castle_stream_cells.toml").write_text("\n".join(lines), encoding="utf-8")
    print(f"CASTLE_GRASS_CELLS_BUILT cells={len(cells)} meshes={total} out={OUT}")


def look_at(loc, target):
    fwd = (target - loc).normalized()
    z = -fwd
    up = Vector((0, 0, 1))
    if abs(z.dot(up)) > 0.99:
        up = Vector((0, 1, 0))
    x = up.cross(z).normalized()
    y = z.cross(x)
    return Matrix(((x.x, y.x, z.x, loc.x), (x.y, y.y, z.y, loc.y), (x.z, y.z, z.z, loc.z), (0, 0, 0, 1)))


def render_check(out, samples):
    ms = [o for o in bpy.data.objects if o.type == "MESH"]
    lo = Vector((math.inf,) * 3)
    hi = Vector((-math.inf,) * 3)
    for o in ms:
        for c in o.bound_box:
            w = o.matrix_world @ Vector(c)
            lo = Vector(map(min, lo, w))
            hi = Vector(map(max, hi, w))
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
    sc.cycles.samples = samples
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
    out.parent.mkdir(parents=True, exist_ok=True)
    sc.render.filepath = str(out.resolve())
    bpy.ops.render.render(write_still=True)
    print(f"CHECK_OK -> {sc.render.filepath}")


def main():
    args = cli()
    for o in list(bpy.data.objects):
        bpy.data.objects.remove(o, do_unlink=True)
    bpy.ops.import_scene.gltf(filepath=str(SOURCE.resolve()))
    bpy.context.view_layer.update()
    mat = make_vc_material(args.normal)
    apply_to_cliffs(mat, args.from_min, args.from_max)
    if args.mode == "render":
        render_check(args.out or (ROOT / "scratch_cliff_vc.png"), args.samples)
    else:
        reslice(args.outdir)


if __name__ == "__main__":
    main()
