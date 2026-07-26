"""Rendu de contrôle du look COMPLET du Hall : terrain reconstruit + château +
végétation du créateur (20k mottes d'herbe + fleurs + buissons) placée aux
positions/échelles/rotations exactes lues dans le Unity Terrain (m_TreeInstances).

Les instances (px,pz normalisés [0,1] sur le terrain) sont posées sur la SURFACE
du terrain par raycast BVH. Meshes = FBX du pack (SM_ENV_*). Instances liées
(mesh partagé) → 21k copies tenables pour un rendu Cycles.

Usage :
  blender --background --factory-startup --python tools/blender/veg_preview.py -- \
    --instances tree_instances.json --veg-dir <veg fbx dir> --grass-tex <png> --out <dir>
"""

import argparse
import json
import math
import sys
from pathlib import Path

import bpy
from mathutils import Vector, Matrix
from mathutils.bvhtree import BVHTree

TERRAIN = "assets/models/environment/castle/castle_terrain_unity.glb"
CASTLE = "assets/models/environment/castle/castle_highlands.glb"
# proto guid-prefix -> fichier FBX (type)
TYPE_FBX = {
    "tree_01": "SM_ENV_tree_castle_01.fbx", "tree_02": "SM_ENV_tree_castle_02.fbx",
    "grass": "SM_ENV_grass_castle.fbx",
    "flower_01": "SM_ENV_flower_castle_01.fbx", "flower_02": "SM_ENV_flower_castle_02.fbx",
    "bush_01": "SM_ENV_bush_castle_01.fbx", "bush_02": "SM_ENV_bush_castle_02.fbx",
    "bush_03": "SM_ENV_bush_castle_03.fbx",
}
# le dump stocke le guid complet en "type" ; on mappe par préfixe
PREFIX = {
    "636bd2100f81": "tree_01", "23b1e7b25dd0": "tree_02", "5f34b6f78c36": "grass",
    "6713d0ece9c1": "flower_01", "4dd6cd27eed1": "flower_02",
    "e9a4c4547223": "bush_01", "a4f0384ffe6d": "bush_02", "cf05101281438": "bush_03",
    "cf0510128143": "bush_03",
}


def cli():
    p = argparse.ArgumentParser()
    p.add_argument("--instances", required=True)
    p.add_argument("--veg-dir", required=True)
    p.add_argument("--atlas", required=True, help="T_ENV_foliage_castle.png (couleur+alpha)")
    p.add_argument("--out", type=Path, default=None)
    p.add_argument("--samples", type=int, default=16)
    p.add_argument("--max-grass", type=int, default=20000)
    p.add_argument("--save-glb", default="", help="exporter la végétation en GLB (jeu)")
    p.add_argument("--render", action="store_true")
    return p.parse_args(sys.argv[sys.argv.index("--") + 1 :])


def import_template(fbx_path, name):
    before = set(bpy.data.objects)
    bpy.ops.import_scene.fbx(filepath=str(Path(fbx_path).resolve()))
    new = [o for o in bpy.data.objects if o not in before]
    meshes = [o for o in new if o.type == "MESH"]
    # joindre si plusieurs, garder le plus gros
    tmpl = max(meshes, key=lambda o: len(o.data.vertices)) if meshes else None
    for o in new:
        if o is not tmpl:
            bpy.data.objects.remove(o, do_unlink=True)
    if tmpl:
        tmpl.name = f"TMPL_{name}"
        tmpl.hide_render = True
        tmpl.hide_viewport = True
    return tmpl


def make_foliage_material(name, atlas_path):
    """Matériau billboard alpha-cutout : atlas feuillage (couleur + alpha MASK)."""
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    nt = mat.node_tree
    bsdf = nt.nodes.get("Principled BSDF")
    bsdf.inputs["Roughness"].default_value = 0.9
    img = bpy.data.images.load(str(Path(atlas_path).resolve()))
    tex = nt.nodes.new("ShaderNodeTexImage")
    tex.image = img
    nt.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    nt.links.new(tex.outputs["Alpha"], bsdf.inputs["Alpha"])
    mat.blend_method = "CLIP" if hasattr(mat, "blend_method") else mat.blend_method
    try:
        mat.alpha_threshold = 0.5
    except Exception:
        pass
    # glTF : mode MASK
    try:
        mat.use_backface_culling = False
    except Exception:
        pass
    return mat


def bounds(objs):
    lo = Vector((math.inf,) * 3)
    hi = -lo
    for o in objs:
        for c in o.bound_box:
            w = o.matrix_world @ Vector(c)
            lo = Vector(map(min, lo, w))
            hi = Vector(map(max, hi, w))
    return lo, hi


def look_at(loc, target):
    fwd = (target - loc).normalized()
    z = -fwd
    up = Vector((0, 0, 1))
    if abs(z.dot(up)) > 0.99:
        up = Vector((0, 1, 0))
    x = up.cross(z).normalized()
    y = z.cross(x)
    return Matrix(((x.x, y.x, z.x, loc.x), (x.y, y.y, z.y, loc.y), (x.z, y.z, z.z, loc.z), (0, 0, 0, 1)))


def main():
    args = cli()
    for o in list(bpy.data.objects):
        bpy.data.objects.remove(o, do_unlink=True)

    bpy.ops.import_scene.gltf(filepath=str(Path(TERRAIN).resolve()))
    terrain_objs = [o for o in bpy.data.objects if o.type == "MESH"]
    terrain = terrain_objs[0]
    if args.render:
        bpy.ops.import_scene.gltf(filepath=str(Path(CASTLE).resolve()))

    tlo, thi = bounds([terrain])
    span_x = thi.x - tlo.x
    span_y = thi.y - tlo.y
    print(f"TERRAIN_BOUNDS x=[{tlo.x:.1f},{thi.x:.1f}] y=[{tlo.y:.1f},{thi.y:.1f}] z=[{tlo.z:.1f},{thi.z:.1f}]")

    # BVH du terrain pour raycast rapide
    depsgraph = bpy.context.evaluated_depsgraph_get()
    tm = terrain.evaluated_get(depsgraph)
    mesh = tm.to_mesh()
    mw = terrain.matrix_world
    verts = [mw @ v.co for v in mesh.vertices]
    faces = [tuple(p.vertices) for p in mesh.polygons]
    bvh = BVHTree.FromPolygons(verts, faces)
    tm.to_mesh_clear()

    # herbe + fleurs + buissons (les arbres sont déjà dans le GLB château)
    foliage = make_foliage_material("M_ENV_foliage_castle", args.atlas)
    templates = {}
    target_h = {"grass": 0.5, "flower_01": 0.42, "flower_02": 0.42,
                "bush_01": 1.4, "bush_02": 1.4, "bush_03": 1.5}
    for t, fbx in TYPE_FBX.items():
        if t.startswith("tree"):
            continue  # arbres déjà dans le GLB château
        fp = Path(args.veg_dir) / fbx
        if not fp.is_file():
            continue
        tmpl = import_template(fp, t)
        if not tmpl:
            continue
        tlo2, thi2 = bounds([tmpl])
        h = thi2.z - tlo2.z
        scale = target_h.get(t, 1.0) / h if h > 0 else 1.0
        tmpl.data.materials.clear()
        tmpl.data.materials.append(foliage)
        templates[t] = (tmpl, scale)
        print(f"TMPL {t}: hauteur_brute={h:.2f} scale={scale:.3f}")

    insts = json.load(open(args.instances))
    placed = {}
    grass_n = 0
    coll = bpy.data.collections.new("Veg")
    bpy.context.scene.collection.children.link(coll)
    # géométrie template pré-extraite (verts locaux + uv par coin de face)
    tinfo = {}
    for t, (tmpl, base_scale) in templates.items():
        me = tmpl.data
        uvl = me.uv_layers.active
        faces = []
        for p in me.polygons:
            uvs = [uvl.data[li].uv.copy() for li in p.loop_indices] if uvl else None
            faces.append((tuple(p.vertices), uvs))
        # CLÉ : verts en coords MONDE (l'import FBX met l'échelle sur l'objet, pas
        # les verts) puis recentré XY + base à Z=0, pour que base_scale (calculé sur
        # les bornes monde ~0.42 m) donne la bonne taille et la carte pose au sol.
        mw = tmpl.matrix_world
        wv = [mw @ v.co for v in me.vertices]
        mnz = min(v.z for v in wv)
        cx = (min(v.x for v in wv) + max(v.x for v in wv)) * 0.5
        cy = (min(v.y for v in wv) + max(v.y for v in wv)) * 0.5
        tv = [Vector((v.x - cx, v.y - cy, v.z - mnz)) for v in wv]
        tinfo[t] = {"tv": tv, "faces": faces,
                    "base": base_scale, "verts": [], "faces_out": [], "uvs_out": []}
    for ins in insts:
        gp = ins["type"][:13]
        t = PREFIX.get(gp) or PREFIX.get(ins["type"][:12])
        if t not in tinfo:
            continue
        if t == "grass":
            grass_n += 1
            if grass_n > args.max_grass:
                continue
        info = tinfo[t]
        px, _, pz = ins["pos"]
        wx = tlo.x + px * span_x
        wy = tlo.y + pz * span_y
        hit = bvh.ray_cast(Vector((wx, wy, 500.0)), Vector((0, 0, -1)))
        wz = hit[0].z if (hit and hit[0]) else tlo.z
        s = info["base"] * ins.get("w", 1.0)
        sz = info["base"] * ins.get("h", 1.0)
        # world = T · Rz · S ; verts bakés en coordonnées monde (nœud = identité)
        M = (Matrix.Translation((wx, wy, wz))
             @ Matrix.Rotation(ins.get("rot", 0.0), 4, "Z")
             @ Matrix.Diagonal(Vector((s, s, sz, 1.0))))
        base = len(info["verts"])
        for co in info["tv"]:
            info["verts"].append(M @ co)
        for vids, uvs in info["faces"]:
            info["faces_out"].append(tuple(base + i for i in vids))
            if uvs:
                info["uvs_out"].extend(uvs)
        placed[t] = placed.get(t, 0) + 1
    print("PLACED:", placed)

    if args.save_glb:
        merged = []
        for t, info in tinfo.items():
            if not info["verts"]:
                continue
            mesh = bpy.data.meshes.new(f"VEG_{t}")
            mesh.from_pydata([tuple(v) for v in info["verts"]], [], info["faces_out"])
            mesh.update()
            if info["uvs_out"]:
                uv_layer = mesh.uv_layers.new(name="UVMap")
                for i, uv in enumerate(info["uvs_out"]):
                    uv_layer.data[i].uv = uv
            mesh.materials.append(foliage)
            obj = bpy.data.objects.new(f"VEG_{t}", mesh)
            coll.objects.link(obj)
            merged.append(obj)
            print(f"MERGED {t}: {placed.get(t, 0)} inst -> {len(mesh.polygons)} faces (nœud identité)")
        bpy.ops.object.select_all(action="DESELECT")
        for o in merged:
            o.select_set(True)
        bpy.context.view_layer.objects.active = merged[0]
        bpy.ops.export_scene.gltf(
            filepath=str(Path(args.save_glb).resolve()), export_format="GLB",
            use_selection=True, export_apply=True, export_yup=True,
        )
        print(f"SAVED_VEG_GLB {args.save_glb} instances={sum(placed.values())}")

    if not args.render:
        return

    # éclairage + caméra aérienne haute (robuste)
    sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN"))
    sun.data.energy = 4.0
    sun.rotation_euler = (math.radians(45), math.radians(10), math.radians(30))
    bpy.context.scene.collection.objects.link(sun)
    sc = bpy.context.scene
    w = bpy.data.worlds.new("w")
    sc.world = w
    w.use_nodes = True
    for n in w.node_tree.nodes:
        if n.type == "BACKGROUND":
            n.inputs[0].default_value = (0.5, 0.7, 0.92, 1.0)
            n.inputs[1].default_value = 1.2
    sc.render.engine = "CYCLES"
    sc.cycles.samples = args.samples
    sc.cycles.device = "CPU"
    sc.render.resolution_x = 1600
    sc.render.resolution_y = 900
    focus = Vector(((tlo.x + thi.x) * 0.5, (tlo.y + thi.y) * 0.5, 38.0))
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    cam.data.lens = 45
    bpy.context.scene.collection.objects.link(cam)
    az, el = math.radians(30), math.radians(45)
    off = Vector((math.cos(el) * math.cos(az), -math.cos(el) * math.sin(az), math.sin(el)))
    cam.matrix_world = look_at(focus + off * 300.0, focus)
    sc.camera = cam
    print(f"CAM at {tuple(round(c,1) for c in (focus + off*300.0))} -> focus {tuple(round(c,1) for c in focus)}")
    args.out.mkdir(parents=True, exist_ok=True)
    sc.render.filepath = str((args.out / "veg_full_look.png").resolve())
    bpy.ops.render.render(write_still=True)
    print(f"CHECK_OK -> {sc.render.filepath}")


if __name__ == "__main__":
    main()
