"""Reconstruit le sol comme dans Unity : bâtit le mesh terrain depuis le heightmap
Unity réel (513², 300×300 m, relief 117 m) et l'aligne SOUS le château via la
relation de scène mesurée (terrain centré sur le château, plateau ~200 m = sol du
Hall). Rend un contrôle terrain+château, et peut exporter le GLB.

Le heightmap vient de `terrain_height.bin` (dumpé par UnityPy) :
  header <i f f f f> = res, spacing, cornerX, cornerY, cornerZ ; puis res*res float32 (mètres Unity).

Usage :
  blender --background --factory-startup --python tools/blender/terrain_from_unity.py -- \
    --bin <terrain_height.bin> --out <dir> [--plateau-z 36.5] [--vscale 1.0]
    [--flip-v] [--flip-u] [--step 2] [--save-glb <path>] [--render]
"""

import argparse
import math
import struct
import sys
from pathlib import Path

import bpy
from mathutils import Matrix, Vector

CASTLE = "assets/models/environment/castle/castle_highlands.glb"
UNITY_PLATEAU_M = 200.0  # hauteur terrain au centre (sous le château) mesurée dans le fichier


def cli():
    p = argparse.ArgumentParser()
    p.add_argument("--bin", required=True)
    p.add_argument("--out", type=Path, default=None)
    p.add_argument("--plateau-z", type=float, default=36.5, help="Z Blender où poser le plateau (sol du Hall)")
    p.add_argument("--vscale", type=float, default=1.0)
    p.add_argument("--flip-u", action="store_true")
    p.add_argument("--flip-v", action="store_true")
    p.add_argument("--dx", type=float, default=0.0, help="décalage manuel X du centre terrain")
    p.add_argument("--dy", type=float, default=0.0, help="décalage manuel Y du centre terrain")
    p.add_argument("--step", type=int, default=2)
    p.add_argument("--save-glb", default="")
    p.add_argument("--render", action="store_true")
    p.add_argument("--samples", type=int, default=20)
    p.add_argument("--splat", default="", help="alphamap R=herbe G=terre B=pavé")
    p.add_argument("--grass", default="")
    p.add_argument("--ground", default="")
    p.add_argument("--pavement", default="")
    p.add_argument("--layer-tile-m", type=float, default=12.0, help="taille de tuile des textures layer (m)")
    p.add_argument("--uv-flip-v", action="store_true")
    p.add_argument("--uv-flip-u", action="store_true")
    p.add_argument("--bake-png", default="", help="cuire l'albedo splat vers ce PNG")
    p.add_argument("--bake-size", type=int, default=2048)
    p.add_argument("--flatten-radius", type=float, default=0.0, help="aplanir le terrain sous le château (rayon m, 0=off)")
    p.add_argument("--flatten-blend", type=float, default=40.0, help="largeur de transition vers le relief naturel (m)")
    p.add_argument("--flatten-z", type=float, default=36.0, help="hauteur Blender du plateau aplani (juste sous le sol du Hall)")
    p.add_argument("--anchors", default="", help="JSON [[x,y,z]] ancres château (Blender) : le terrain épouse leurs hauteurs")
    p.add_argument("--anchor-drop", type=float, default=0.4, help="décalage sous les ancres (m) pour ne pas transpercer les assets")
    p.add_argument("--anchor-blur", type=int, default=9, help="fondu du champ d'ancres (itérations de flou)")
    p.add_argument("--anchor-strength", type=float, default=12.0, help="force de conformage (satur. du poids)")
    p.add_argument("--anchors-clamp", default="", help="JSON ancres bâtiment : le terrain est forcé SOUS elles (jamais à l'intérieur)")
    p.add_argument("--clamp-margin", type=float, default=1.5, help="marge sous le sol intérieur (terrain ≤ sol − marge)")
    p.add_argument("--clamp-dilate", type=int, default=8, help="remplissage de l'emprise sols (dilatation, comble les trous)")
    p.add_argument("--clamp-blur", type=int, default=3, help="adoucissement du bord de l'emprise")
    p.add_argument("--save-collision", default="", help="exporter un GLB collision décimé")
    p.add_argument("--collision-tris", type=int, default=8000)
    return p.parse_args(sys.argv[sys.argv.index("--") + 1 :])


def read_bin(path):
    data = open(path, "rb").read()
    res, spacing, cx, cy, cz = struct.unpack_from("<iffff", data, 0)
    off = struct.calcsize("<iffff")
    import array
    h = array.array("f")
    h.frombytes(data[off:off + res * res * 4])
    return res, spacing, (cx, cy, cz), h


def castle_center_xy():
    lo = Vector((math.inf, math.inf, math.inf))
    hi = -lo
    for o in bpy.data.objects:
        if o.type != "MESH":
            continue
        for c in o.bound_box:
            w = o.matrix_world @ Vector(c)
            lo = Vector(map(min, lo, w))
            hi = Vector(map(max, hi, w))
    return ((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5), lo, hi


def make_splat_material(args, span):
    """Matériau terrain type Unity : mix herbe(R)/terre(G)/pavé(B) piloté par la splatmap."""
    mat = bpy.data.materials.new("M_TerrainSplat")
    mat.use_nodes = True
    nt = mat.node_tree
    for n in list(nt.nodes):
        nt.nodes.remove(n)
    out = nt.nodes.new("ShaderNodeOutputMaterial")
    bsdf = nt.nodes.new("ShaderNodeBsdfPrincipled")
    bsdf.inputs["Roughness"].default_value = 0.95
    nt.links.new(bsdf.outputs["BSDF"], out.inputs["Surface"])
    tc = nt.nodes.new("ShaderNodeTexCoord")

    def layer_tex(path):
        img = bpy.data.images.load(str(Path(path).resolve()))
        node = nt.nodes.new("ShaderNodeTexImage")
        node.image = img
        m = nt.nodes.new("ShaderNodeMapping")
        s = span / max(1.0, args.layer_tile_m)
        m.inputs["Scale"].default_value = (s, s, s)
        nt.links.new(tc.outputs["UV"], m.inputs["Vector"])
        nt.links.new(m.outputs["Vector"], node.inputs["Vector"])
        return node

    grass = layer_tex(args.grass)
    ground = layer_tex(args.ground)
    pave = layer_tex(args.pavement)

    splat_img = bpy.data.images.load(str(Path(args.splat).resolve()))
    splat_img.colorspace_settings.name = "Non-Color"
    splat = nt.nodes.new("ShaderNodeTexImage")
    splat.image = splat_img
    sm = nt.nodes.new("ShaderNodeMapping")
    if args.uv_flip_u:
        sm.inputs["Scale"].default_value = (-1, 1, 1)
    if args.uv_flip_v:
        sc = sm.inputs["Scale"].default_value
        sm.inputs["Scale"].default_value = (sc[0], -1, 1)
    nt.links.new(tc.outputs["UV"], sm.inputs["Vector"])
    nt.links.new(sm.outputs["Vector"], splat.inputs["Vector"])
    sep = nt.nodes.new("ShaderNodeSeparateColor")
    nt.links.new(splat.outputs["Color"], sep.inputs["Color"])

    # base=herbe → terre où G → pavé où B
    mix_g = nt.nodes.new("ShaderNodeMix")
    mix_g.data_type = "RGBA"
    nt.links.new(sep.outputs["Green"], mix_g.inputs["Factor"])
    nt.links.new(grass.outputs["Color"], mix_g.inputs["A"])
    nt.links.new(ground.outputs["Color"], mix_g.inputs["B"])
    mix_b = nt.nodes.new("ShaderNodeMix")
    mix_b.data_type = "RGBA"
    nt.links.new(sep.outputs["Blue"], mix_b.inputs["Factor"])
    nt.links.new(mix_g.outputs["Result"], mix_b.inputs["A"])
    nt.links.new(pave.outputs["Color"], mix_b.inputs["B"])
    nt.links.new(mix_b.outputs["Result"], bsdf.inputs["Base Color"])
    return mat


def build_anchor_field(anchor_path, res_m, blur_it, drop, strength):
    """Rasterise les ancres château (x,y,hauteur) en un champ de hauteur lissé + un
    poids de couverture, pour que le terrain épouse les niveaux des assets (escaliers,
    sols, pont) avec un fondu doux vers le relief naturel au loin."""
    import json as _json
    import numpy as np
    A = _json.load(open(anchor_path))
    xs = [p[0] for p in A]
    ys = [p[1] for p in A]
    xmin, xmax = min(xs) - 15, max(xs) + 15
    ymin, ymax = min(ys) - 15, max(ys) + 15
    W = int((xmax - xmin) / res_m) + 1
    H = int((ymax - ymin) / res_m) + 1
    hsum = np.zeros((H, W))
    cnt = np.zeros((H, W))
    for x, y, z in A:
        j = int((x - xmin) / res_m)
        i = int((y - ymin) / res_m)
        if 0 <= i < H and 0 <= j < W:
            hsum[i, j] += z - drop  # sous l'asset pour ne pas transpercer
            cnt[i, j] += 1.0
    have = cnt > 0
    havg = np.zeros_like(hsum)
    havg[have] = hsum[have] / cnt[have]
    cov = have.astype(float)

    def blur(a, it):
        for _ in range(it):
            a = (a + np.roll(a, 1, 0) + np.roll(a, -1, 0) + np.roll(a, 1, 1) + np.roll(a, -1, 1)) / 5.0
        return a

    hb = blur(havg * cov, blur_it)
    wb = blur(cov, blur_it)
    field = np.where(wb > 1e-5, hb / wb, 0.0)
    weight = np.clip(wb * strength, 0.0, 1.0)  # 1 près des ancres, 0 au loin
    return {"field": field, "weight": weight, "xmin": xmin, "ymin": ymin, "res": res_m, "H": H, "W": W}


def build_clamp_field(anchor_path, res_m, dilate_it, edge_blur, margin):
    """Emprise PLEINE des sols intérieurs : rasterise les ancres (avec leur
    hauteur), DILATE pour remplir l'intérieur (zéro trou), bord adouci. Renvoie
    un plafond = hauteur du sol local − marge : le terrain sera forcé SOUS ça
    (plus de sol qui perce le Hall), par-niveau (gère les sols multi-hauteurs)."""
    import json as _json
    import numpy as np
    A = _json.load(open(anchor_path))
    xs = [p[0] for p in A]
    ys = [p[1] for p in A]
    xmin, xmax = min(xs) - 10, max(xs) + 10
    ymin, ymax = min(ys) - 10, max(ys) + 10
    W = int((xmax - xmin) / res_m) + 1
    H = int((ymax - ymin) / res_m) + 1
    cov = np.zeros((H, W))
    hsum = np.zeros((H, W))
    cnt = np.zeros((H, W))
    for x, y, z in A:
        j = int((x - xmin) / res_m)
        i = int((y - ymin) / res_m)
        if 0 <= i < H and 0 <= j < W:
            cov[i, j] = 1.0
            hsum[i, j] += z
            cnt[i, j] += 1.0

    def dmax(a, it):
        for _ in range(it):
            a = np.maximum.reduce([a, np.roll(a, 1, 0), np.roll(a, -1, 0), np.roll(a, 1, 1), np.roll(a, -1, 1)])
        return a

    def blur(a, it):
        for _ in range(it):
            a = (a + np.roll(a, 1, 0) + np.roll(a, -1, 0) + np.roll(a, 1, 1) + np.roll(a, -1, 1)) / 5.0
        return a

    # hauteur de sol propagée STABLEMENT : une cellule inconnue se remplit avec la
    # moyenne de ses voisins CONNUS uniquement (borné par les hauteurs réelles, pas
    # de rétroaction qui explose le ceil).
    h = np.where(cnt > 0, hsum / np.maximum(cnt, 1.0), 0.0)
    known = (cnt > 0).astype(float)
    for _ in range(dilate_it + edge_blur):
        hk = h * known
        sum_h = hk + np.roll(hk, 1, 0) + np.roll(hk, -1, 0) + np.roll(hk, 1, 1) + np.roll(hk, -1, 1)
        cnt_k = known + np.roll(known, 1, 0) + np.roll(known, -1, 0) + np.roll(known, 1, 1) + np.roll(known, -1, 1)
        newly = (known < 0.5) & (cnt_k > 0)
        h = np.where(newly, sum_h / np.maximum(cnt_k, 1.0), h)
        known = np.where(cnt_k > 0, 1.0, known)
    hfield = np.where(known > 0.5, h, 36.0) - margin
    # couverture pleine (dilate franc + bord doux, saturé pour clamp net à l'intérieur)
    cov_soft = np.clip(blur(dmax(cov, dilate_it), edge_blur) * 3.0, 0.0, 1.0)
    return {"field": hfield, "weight": cov_soft,
            "xmin": xmin, "ymin": ymin, "res": res_m, "H": H, "W": W}


def sample_anchor(af, wx, wy):
    j = int((wx - af["xmin"]) / af["res"])
    i = int((wy - af["ymin"]) / af["res"])
    if 0 <= i < af["H"] and 0 <= j < af["W"]:
        return float(af["field"][i, j]), float(af["weight"][i, j])
    return 0.0, 0.0


def build_terrain(res, spacing, heights, center_xy, args):
    step = max(1, args.step)
    verts = []
    uvs = []
    span = spacing * (res - 1)  # 300 m
    half = span * 0.5
    cx, cy = center_xy
    af = build_anchor_field(args.anchors, 2.0, args.anchor_blur, args.anchor_drop, args.anchor_strength) if args.anchors else None
    cf = build_clamp_field(args.anchors_clamp, 2.5, args.clamp_dilate, args.clamp_blur, args.clamp_margin) if args.anchors_clamp else None
    if cf is not None:
        import numpy as _np
        print(f"CLAMP_DBG weight max={float(_np.max(cf['weight'])):.2f} mean={float(_np.mean(cf['weight'])):.2f} "
              f"ceil[min,max]=[{float(_np.min(cf['field'])):.1f},{float(_np.max(cf['field'])):.1f}] "
              f"grid xmin={cf['xmin']:.0f} ymin={cf['ymin']:.0f} WxH={cf['W']}x{cf['H']}")
    clamped_n = 0
    idx = {}
    k = 0
    for ii, i in enumerate(range(0, res, step)):
        for jj, j in enumerate(range(0, res, step)):
            u = -half + j * spacing  # X local Unity (-150..150)
            v = -half + i * spacing  # Z local Unity (-150..150)
            if args.flip_u:
                u = -u
            if args.flip_v:
                v = -v
            hm = heights[i * res + j]
            z = args.plateau_z + (hm - UNITY_PLATEAU_M) * args.vscale
            wx = cx + u + args.dx
            wy = cy + v + args.dy
            if af is not None:
                # le terrain épouse les niveaux de l'approche (escaliers/pont/porte)
                ah, aw = sample_anchor(af, wx, wy)
                if aw > 0.0:
                    z = z * (1.0 - aw) + ah * aw
            if cf is not None:
                # sols intérieurs : le terrain est forcé SOUS le sol local (jamais dedans)
                ceil, cw = sample_anchor(cf, wx, wy)
                if cw > 0.0 and z > ceil:
                    z = ceil + (z - ceil) * (1.0 - cw)
                    clamped_n += 1
            if af is None and cf is None and args.flatten_radius > 0.0:
                d = math.hypot(wx - cx, wy - cy)
                if d <= args.flatten_radius:
                    z = args.flatten_z
                elif d < args.flatten_radius + args.flatten_blend:
                    t = (d - args.flatten_radius) / args.flatten_blend
                    t = t * t * (3.0 - 2.0 * t)  # smoothstep naturel<-plat
                    z = args.flatten_z * (1.0 - t) + z * t
            verts.append((wx, wy, z))
            uvs.append((j / (res - 1), i / (res - 1)))  # aligne splatmap sur la grille
            idx[(ii, jj)] = k
            k += 1
    if cf is not None:
        print(f"CLAMP_DBG clamped_verts={clamped_n}")
    faces = []
    rows = len(range(0, res, step))
    cols = len(range(0, res, step))
    for a in range(rows - 1):
        for b in range(cols - 1):
            faces.append((idx[(a, b)], idx[(a + 1, b)], idx[(a + 1, b + 1)], idx[(a, b + 1)]))
    mesh = bpy.data.meshes.new("UnityTerrain")
    mesh.from_pydata(verts, [], faces)
    mesh.update()
    uv_layer = mesh.uv_layers.new(name="UVMap")
    for loop in mesh.loops:
        uv_layer.data[loop.index].uv = uvs[loop.vertex_index]
    import bmesh
    bm = bmesh.new()
    bm.from_mesh(mesh)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bm.to_mesh(mesh)
    bm.free()
    obj = bpy.data.objects.new("UnityTerrain", mesh)
    if args.splat and args.grass and args.ground and args.pavement:
        mat = make_splat_material(args, span)
    else:
        mat = bpy.data.materials.new("M_TerrainGrass")
        mat.use_nodes = True
        bsdf = mat.node_tree.nodes.get("Principled BSDF")
        if bsdf:
            bsdf.inputs["Base Color"].default_value = (0.28, 0.45, 0.12, 1.0)
            bsdf.inputs["Roughness"].default_value = 0.95
    obj.data.materials.append(mat)
    bpy.context.scene.collection.objects.link(obj)
    return obj


def bake_albedo(obj, out_png, size):
    """Cuit le matériau splat (mix nodes) vers une texture albedo unique (glTF-safe)."""
    img = bpy.data.images.new("terrain_albedo", size, size, alpha=False)
    mat = obj.data.materials[0]
    nt = mat.node_tree
    target = nt.nodes.new("ShaderNodeTexImage")
    target.image = img
    for n in nt.nodes:
        n.select = False
    target.select = True
    nt.nodes.active = target
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    sc = bpy.context.scene
    sc.render.engine = "CYCLES"
    sc.cycles.device = "CPU"
    sc.cycles.samples = 4
    sc.render.bake.use_pass_direct = False
    sc.render.bake.use_pass_indirect = False
    sc.render.bake.use_pass_color = True
    sc.render.bake.margin = 8
    bpy.ops.object.bake(type="DIFFUSE")
    img.filepath_raw = str(Path(out_png).resolve())
    img.file_format = "PNG"
    img.save()
    # remplace le matériau par un simple Principled + albedo cuit (exportable glTF)
    for n in list(nt.nodes):
        nt.nodes.remove(n)
    out = nt.nodes.new("ShaderNodeOutputMaterial")
    bsdf = nt.nodes.new("ShaderNodeBsdfPrincipled")
    bsdf.inputs["Roughness"].default_value = 0.95
    baked = nt.nodes.new("ShaderNodeTexImage")
    baked.image = bpy.data.images.load(str(Path(out_png).resolve()))
    nt.links.new(baked.outputs["Color"], bsdf.inputs["Base Color"])
    nt.links.new(bsdf.outputs["BSDF"], out.inputs["Surface"])
    print(f"BAKED_ALBEDO -> {out_png}")


def export_collision(obj, out_glb, target_tris):
    """Duplique + décime le terrain en un mesh collision léger (1 TriMesh)."""
    dup = obj.copy()
    dup.data = obj.data.copy()
    bpy.context.scene.collection.objects.link(dup)
    tris = len(dup.data.polygons) * 2
    ratio = min(1.0, max(0.02, target_tris / max(1, tris)))
    m = dup.modifiers.new("dec", "DECIMATE")
    m.ratio = ratio
    bpy.ops.object.select_all(action="DESELECT")
    dup.select_set(True)
    bpy.context.view_layer.objects.active = dup
    bpy.ops.object.modifier_apply(modifier="dec")
    bpy.ops.export_scene.gltf(filepath=str(Path(out_glb).resolve()), export_format="GLB",
                              use_selection=True, export_apply=True, export_yup=True)
    print(f"SAVED_COLLISION {out_glb} tris~{len(dup.data.polygons)}")
    bpy.data.objects.remove(dup, do_unlink=True)


def look_at(loc, target):
    fwd = (target - loc).normalized()
    z = -fwd
    up = Vector((0, 0, 1))
    if abs(z.dot(up)) > 0.99:
        up = Vector((0, 1, 0))
    x = up.cross(z).normalized()
    y = z.cross(x)
    return Matrix(((x.x, y.x, z.x, loc.x), (x.y, y.y, z.y, loc.y), (x.z, y.z, z.z, loc.z), (0, 0, 0, 1)))


def render(out, samples, focus, radius):
    sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN"))
    sun.data.energy = 3.0
    sun.rotation_euler = (math.radians(55), 0, math.radians(40))
    bpy.context.scene.collection.objects.link(sun)
    sc = bpy.context.scene
    w = sc.world or bpy.data.worlds.new("w")
    sc.world = w
    w.use_nodes = True
    bg = w.node_tree.nodes.get("Background")
    if bg:
        bg.inputs[0].default_value = (0.55, 0.72, 0.92, 1.0)
    sc.render.engine = "CYCLES"
    sc.cycles.samples = samples
    sc.cycles.device = "CPU"
    sc.render.resolution_x = 1600
    sc.render.resolution_y = 900
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    cam.data.lens = 38
    bpy.context.scene.collection.objects.link(cam)
    az, el = math.radians(38), math.radians(14)
    dist = radius * 2.3
    off = Vector((math.cos(el) * math.cos(az), -math.cos(el) * math.sin(az), math.sin(el)))
    cam.matrix_world = look_at(focus + off * dist, focus)
    sc.camera = cam
    out.parent.mkdir(parents=True, exist_ok=True)
    sc.render.filepath = str(out.resolve())
    bpy.ops.render.render(write_still=True)
    print(f"CHECK_OK -> {sc.render.filepath}")


def main():
    args = cli()
    for o in list(bpy.data.objects):
        bpy.data.objects.remove(o, do_unlink=True)
    bpy.ops.import_scene.gltf(filepath=str(Path(CASTLE).resolve()))
    (cx, cy), lo, hi = castle_center_xy()
    print(f"CASTLE_CENTER_XY=({cx:.2f},{cy:.2f}) bounds Z=[{lo.z:.2f},{hi.z:.2f}]")
    res, spacing, corner, heights = read_bin(args.bin)
    terrain = build_terrain(res, spacing, heights, (cx, cy), args)
    print(f"TERRAIN_BUILT verts~{len(terrain.data.vertices)} plateau_z={args.plateau_z} vscale={args.vscale}")

    if args.save_collision:
        export_collision(terrain, args.save_collision, args.collision_tris)

    if args.bake_png:
        bake_albedo(terrain, args.bake_png, args.bake_size)

    if args.save_glb:
        bpy.ops.object.select_all(action="DESELECT")
        terrain.select_set(True)
        bpy.context.view_layer.objects.active = terrain
        bpy.ops.export_scene.gltf(filepath=str(Path(args.save_glb).resolve()), export_format="GLB",
                                  use_selection=True, export_apply=True, export_yup=True)
        print(f"SAVED_GLB {args.save_glb}")

    if args.render and args.out:
        focus = Vector((cx, cy, args.plateau_z))
        radius = (hi - lo).length * 0.6
        render(args.out / "terrain_unity_aligned.png", args.samples, focus, radius)


if __name__ == "__main__":
    main()
