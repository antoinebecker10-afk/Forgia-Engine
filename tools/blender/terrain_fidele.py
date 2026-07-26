"""Terrain FIDELE (sans rustines) : heightmap lisse pleine resolution, centre sur
le chateau, plateau mesure, vscale=1, ZERO conform/clamp/flatten. But immediat :
rendre les 4 orientations candidates en VUE DE DESSUS avec le chateau pour choisir
l'orientation correcte (la conversion de repere du chateau inclut une reflexion Fx).

Usage :
  blender --background --factory-startup --python terrain_fidele.py -- \
    --bin <terrain_height.bin> --splat <splat.png> --grass <g> --ground <d> --pavement <p> \
    --out <dir> [--orient none|u|v|uv] [--render-candidates] [--save-glb <p>] [--bake-png <p>]
"""
import argparse, math, struct, array, sys
from pathlib import Path
import bpy
from mathutils import Matrix, Vector

CASTLE = "assets/models/environment/castle/castle_highlands.glb"


def cli():
    p = argparse.ArgumentParser()
    p.add_argument("--bin", required=True)
    p.add_argument("--out", type=Path, required=True)
    p.add_argument("--orient", default="uv", choices=["none", "u", "v", "uv"])
    p.add_argument("--render-candidates", action="store_true", help="rend les 4 orientations top-ortho")
    p.add_argument("--vscale", type=float, default=1.0)
    p.add_argument("--step", type=int, default=2)
    p.add_argument("--splat", default="")
    p.add_argument("--grass", default="")
    p.add_argument("--ground", default="")
    p.add_argument("--pavement", default="")
    p.add_argument("--layer-tile-m", type=float, default=12.0)
    p.add_argument("--save-glb", default="")
    p.add_argument("--bake-png", default="")
    p.add_argument("--bake-size", type=int, default=2048)
    p.add_argument("--save-collision", default="")
    p.add_argument("--collision-tris", type=int, default=8000)
    p.add_argument("--samples", type=int, default=16)
    p.add_argument("--highlight", action="store_true", help="chemins pave en magenta vif (aide au calage)")
    p.add_argument("--bake-align", default="", help="ax,ay,az (jeu) a CUIRE dans le mesh (align runtime -> 0)")
    p.add_argument("--cut-under-floors", type=float, default=0.0,
                   help="trou : retire les faces terrain sous un sol chateau a moins de N metres (0=off)")
    return p.parse_args(sys.argv[sys.argv.index("--") + 1:])


def read_bin(path):
    data = open(path, "rb").read()
    res, spacing, cx, cy, cz = struct.unpack_from("<iffff", data, 0)
    off = struct.calcsize("<iffff")
    h = array.array("f"); h.frombytes(data[off:off + res * res * 4])
    return res, spacing, (cx, cy, cz), h


def castle_bounds():
    lo = Vector((math.inf,) * 3); hi = -lo
    for o in bpy.data.objects:
        if o.type != "MESH":
            continue
        for c in o.bound_box:
            w = o.matrix_world @ Vector(c)
            lo = Vector(map(min, lo, w)); hi = Vector(map(max, hi, w))
    return lo, hi


def make_splat_material(args, span):
    mat = bpy.data.materials.new("M_TerrainSplat"); mat.use_nodes = True
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
        node = nt.nodes.new("ShaderNodeTexImage"); node.image = img
        m = nt.nodes.new("ShaderNodeMapping")
        s = span / max(1.0, args.layer_tile_m)
        m.inputs["Scale"].default_value = (s, s, s)
        nt.links.new(tc.outputs["UV"], m.inputs["Vector"])
        nt.links.new(m.outputs["Vector"], node.inputs["Vector"])
        return node

    def layer_color(rgb):
        node = nt.nodes.new("ShaderNodeRGB")
        node.outputs[0].default_value = (*rgb, 1.0)
        return node

    if getattr(args, "highlight", False):
        # aide au calage : herbe/terre eteintes, CHEMINS pave en magenta vif.
        grass = layer_color((0.12, 0.20, 0.06))
        ground = layer_color((0.30, 0.21, 0.10))
        pave = layer_color((1.0, 0.0, 1.0))
    else:
        grass = layer_tex(args.grass); ground = layer_tex(args.ground); pave = layer_tex(args.pavement)
    splat_img = bpy.data.images.load(str(Path(args.splat).resolve()))
    splat_img.colorspace_settings.name = "Non-Color"
    splat = nt.nodes.new("ShaderNodeTexImage"); splat.image = splat_img
    nt.links.new(tc.outputs["UV"], splat.inputs["Vector"])
    sep = nt.nodes.new("ShaderNodeSeparateColor")
    nt.links.new(splat.outputs["Color"], sep.inputs["Color"])
    mix_g = nt.nodes.new("ShaderNodeMix"); mix_g.data_type = "RGBA"
    nt.links.new(sep.outputs["Green"], mix_g.inputs["Factor"])
    nt.links.new(grass.outputs["Color"], mix_g.inputs["A"])
    nt.links.new(ground.outputs["Color"], mix_g.inputs["B"])
    mix_b = nt.nodes.new("ShaderNodeMix"); mix_b.data_type = "RGBA"
    nt.links.new(sep.outputs["Blue"], mix_b.inputs["Factor"])
    nt.links.new(mix_g.outputs["Result"], mix_b.inputs["A"])
    nt.links.new(pave.outputs["Color"], mix_b.inputs["B"])
    nt.links.new(mix_b.outputs["Result"], bsdf.inputs["Base Color"])
    return mat


def build_terrain(res, spacing, heights, center_xy, base_z, plateau_h, args, flip_u, flip_v):
    """Mesh terrain PROPRE : Unity world -> Blender, centre sur le chateau, plateau
    (hauteur Unity au centre) -> base_z mesuree. Zero conform/clamp."""
    step = max(1, args.step)
    span = spacing * (res - 1); half = span * 0.5
    cx, cy = center_xy
    verts, uvs, idx, k = [], [], {}, 0
    for ii, i in enumerate(range(0, res, step)):
        for jj, j in enumerate(range(0, res, step)):
            u = -half + j * spacing
            v = -half + i * spacing
            if flip_u:
                u = -u
            if flip_v:
                v = -v
            hm = heights[i * res + j]
            z = base_z + (hm - plateau_h) * args.vscale
            verts.append((cx + u, cy + v, z))
            uvs.append((j / (res - 1), i / (res - 1)))
            idx[(ii, jj)] = k; k += 1
    faces = []
    rows = len(range(0, res, step)); cols = rows
    for a in range(rows - 1):
        for b in range(cols - 1):
            faces.append((idx[(a, b)], idx[(a + 1, b)], idx[(a + 1, b + 1)], idx[(a, b + 1)]))
    mesh = bpy.data.meshes.new("UnityTerrain")
    mesh.from_pydata(verts, [], faces); mesh.update()
    uvl = mesh.uv_layers.new(name="UVMap")
    for loop in mesh.loops:
        uvl.data[loop.index].uv = uvs[loop.vertex_index]
    import bmesh
    bm = bmesh.new(); bm.from_mesh(mesh)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bm.to_mesh(mesh); bm.free()
    obj = bpy.data.objects.new("UnityTerrain", mesh)
    if args.splat and args.grass and args.ground and args.pavement:
        obj.data.materials.append(make_splat_material(args, span))
    else:
        m = bpy.data.materials.new("M_TerrainGrass"); m.use_nodes = True
        b = m.node_tree.nodes.get("Principled BSDF")
        if b:
            b.inputs["Base Color"].default_value = (0.28, 0.45, 0.12, 1.0)
        obj.data.materials.append(m)
    bpy.context.scene.collection.objects.link(obj)
    return obj


def setup_world_sun():
    sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN"))
    sun.data.energy = 3.5
    sun.rotation_euler = (math.radians(55), 0, math.radians(35))
    bpy.context.scene.collection.objects.link(sun)
    sc = bpy.context.scene
    w = sc.world or bpy.data.worlds.new("w"); sc.world = w; w.use_nodes = True
    bg = w.node_tree.nodes.get("Background")
    if bg:
        bg.inputs[0].default_value = (0.7, 0.8, 0.95, 1.0)
    sc.render.engine = "CYCLES"; sc.cycles.device = "CPU"; sc.cycles.samples = 16
    sc.render.resolution_x = 1200; sc.render.resolution_y = 1200


def render_3q_west(out_png, cx, cy, base_z, radius):
    """Vue 3/4 depuis l'ouest (cote entree du chateau), angle bas rasant."""
    sc = bpy.context.scene
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    cam.data.lens = 34
    focus = Vector((cx, cy, base_z + radius * 0.25))
    az, el = math.radians(200), math.radians(16)  # ~ouest, rasant
    off = Vector((math.cos(el) * math.cos(az), math.cos(el) * math.sin(az), math.sin(el)))
    loc = focus + off * (radius * 2.4)
    fwd = (focus - loc).normalized(); z = -fwd
    up = Vector((0, 0, 1))
    x = up.cross(z).normalized(); y = z.cross(x)
    cam.matrix_world = Matrix(((x.x, y.x, z.x, loc.x), (x.y, y.y, z.y, loc.y),
                              (x.z, y.z, z.z, loc.z), (0, 0, 0, 1)))
    bpy.context.scene.collection.objects.link(cam)
    sc.camera = cam
    Path(out_png).parent.mkdir(parents=True, exist_ok=True)
    sc.render.filepath = str(Path(out_png).resolve())
    bpy.ops.render.render(write_still=True)
    bpy.data.objects.remove(cam, do_unlink=True)
    print(f"CANDIDATE3Q_OK -> {out_png}")


def render_top_ortho(out_png, lo, hi):
    sc = bpy.context.scene
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    cam.data.type = "ORTHO"
    cam.data.ortho_scale = max(hi.x - lo.x, hi.y - lo.y) * 1.05
    cx = (lo.x + hi.x) * 0.5; cy = (lo.y + hi.y) * 0.5
    cam.location = (cx, cy, hi.z + 400)
    cam.rotation_euler = (0, 0, 0)  # regarde -Z (vers le bas)
    bpy.context.scene.collection.objects.link(cam)
    sc.camera = cam
    Path(out_png).parent.mkdir(parents=True, exist_ok=True)
    sc.render.filepath = str(Path(out_png).resolve())
    bpy.ops.render.render(write_still=True)
    bpy.data.objects.remove(cam, do_unlink=True)
    print(f"CANDIDATE_OK -> {out_png}")


def cut_terrain_under_floors(terr, castle_objs, threshold):
    """Trou sous le batiment : retire les faces terrain dont un raycast VERS LE HAUT
    touche un sol/escalier/plafond du chateau a < threshold m. Suit l'emprise des
    sols (keep, cour) donc cache par les murs vu de l'exterieur ; ignore l'aqueduc
    (SM_MOD_bridge) et les falaises. Libere l'espace des sous-sols."""
    import bmesh
    from mathutils.bvhtree import BVHTree
    floors = [o for o in castle_objs
              if o.type == "MESH" and any(k in o.name.lower() for k in ("floor", "stairs", "ceiling"))]
    verts, faces = [], []
    dg = bpy.context.evaluated_depsgraph_get()
    for o in floors:
        me = o.evaluated_get(dg).to_mesh()
        base = len(verts)
        mw = o.matrix_world
        verts += [mw @ v.co for v in me.vertices]
        faces += [tuple(base + i for i in p.vertices) for p in me.polygons]
        o.evaluated_get(dg).to_mesh_clear()
    if not faces:
        print("CUT: aucun sol chateau trouve")
        return
    bvh = BVHTree.FromPolygons(verts, faces)
    me = terr.data
    bm = bmesh.new()
    bm.from_mesh(me)
    bm.faces.ensure_lookup_table()
    kill = []
    for f in bm.faces:
        c = f.calc_center_median()  # terr a l'identite => local == monde
        hit = bvh.ray_cast(c + Vector((0.0, 0.0, 0.1)), Vector((0.0, 0.0, 1.0)), threshold)
        if hit and hit[0] is not None:
            kill.append(f)
    bmesh.ops.delete(bm, geom=kill, context="FACES")
    bm.to_mesh(me)
    bm.free()
    print(f"CUT_UNDER_FLOORS threshold={threshold} floors={len(floors)} faces_removed={len(kill)}/{len(me.polygons)+len(kill)}")


def main():
    args = cli()
    for o in list(bpy.data.objects):
        bpy.data.objects.remove(o, do_unlink=True)
    bpy.ops.import_scene.gltf(filepath=str(Path(CASTLE).resolve()))
    lo_c, hi_c = castle_bounds()
    cx = (lo_c.x + hi_c.x) * 0.5; cy = (lo_c.y + hi_c.y) * 0.5
    base_z = lo_c.z
    res, spacing, corner, heights = read_bin(args.bin)
    plateau_h = heights[(res // 2) * res + res // 2]  # hauteur Unity au centre (=200)
    print(f"CASTLE center=({cx:.2f},{cy:.2f}) z=[{lo_c.z:.2f},{hi_c.z:.2f}]  plateau_unity={plateau_h:.2f}")
    # plateau -> juste sous le sol du Hall (36.5) : on garde le mapping mesure historiquement bon.
    PLATEAU_Z = 36.5
    if args.bake_align:
        ax, ay, az = (float(x) for x in args.bake_align.split(","))
        cx += ax
        cy -= az
        PLATEAU_Z = ay + args.vscale * PLATEAU_Z  # cuit align.y + scale.y runtime
        print(f"BAKE_ALIGN ax={ax} ay={ay} az={az} -> cx={cx:.2f} cy={cy:.2f} plateau_z={PLATEAU_Z:.2f} vscale={args.vscale}")

    setup_world_sun()
    castle_objs = [o for o in bpy.data.objects if o.type == "MESH"]

    if args.render_candidates:
        # Compare v (flip_v) vs uv (miroir X de v) en VUE DE DESSUS + chemins magenta :
        # celui dont le chemin rejoint le pont sans decalage geant est le bon.
        combos = [("v", False, True), ("uv", True, True)]
        for name, fu, fv in combos:
            terr = build_terrain(res, spacing, heights, (cx, cy), PLATEAU_Z, plateau_h, args, fu, fv)
            lo = Vector((min(cx - 150, lo_c.x), min(cy - 150, lo_c.y), min(base_z, lo_c.z)))
            hi = Vector((max(cx + 150, hi_c.x), max(cy + 150, hi_c.y), max(hi_c.z, base_z + 60)))
            render_top_ortho(args.out / f"cmp_{name}.png", lo, hi)
            bpy.data.objects.remove(terr, do_unlink=True)
        return

    fu = args.orient in ("u", "uv"); fv = args.orient in ("v", "uv")
    terr = build_terrain(res, spacing, heights, (cx, cy), PLATEAU_Z, plateau_h, args, fu, fv)
    print(f"TERRAIN_BUILT orient={args.orient} verts~{len(terr.data.vertices)}")

    if args.cut_under_floors > 0.0:
        cut_terrain_under_floors(terr, castle_objs, args.cut_under_floors)

    if args.save_collision:
        dup = terr.copy(); dup.data = terr.data.copy()
        bpy.context.scene.collection.objects.link(dup)
        tris = len(dup.data.polygons) * 2
        ratio = min(1.0, max(0.02, args.collision_tris / max(1, tris)))
        mo = dup.modifiers.new("dec", "DECIMATE"); mo.ratio = ratio
        bpy.ops.object.select_all(action="DESELECT"); dup.select_set(True)
        bpy.context.view_layer.objects.active = dup
        bpy.ops.object.modifier_apply(modifier="dec")
        bpy.ops.export_scene.gltf(filepath=str(Path(args.save_collision).resolve()),
                                  export_format="GLB", use_selection=True, export_apply=True, export_yup=True)
        print(f"SAVED_COLLISION {args.save_collision} tris~{len(dup.data.polygons)}")
        bpy.data.objects.remove(dup, do_unlink=True)

    if args.bake_png:
        # bake albedo splat -> PNG unique glTF-safe
        img = bpy.data.images.new("terrain_albedo", args.bake_size, args.bake_size, alpha=False)
        nt = terr.data.materials[0].node_tree
        tgt = nt.nodes.new("ShaderNodeTexImage"); tgt.image = img
        for n in nt.nodes:
            n.select = False
        tgt.select = True; nt.nodes.active = tgt
        bpy.ops.object.select_all(action="DESELECT"); terr.select_set(True)
        bpy.context.view_layer.objects.active = terr
        sc = bpy.context.scene
        sc.cycles.samples = 4
        sc.render.bake.use_pass_direct = False; sc.render.bake.use_pass_indirect = False
        sc.render.bake.use_pass_color = True; sc.render.bake.margin = 8
        bpy.ops.object.bake(type="DIFFUSE")
        img.filepath_raw = str(Path(args.bake_png).resolve()); img.file_format = "PNG"; img.save()
        for n in list(nt.nodes):
            nt.nodes.remove(n)
        out = nt.nodes.new("ShaderNodeOutputMaterial")
        bsdf = nt.nodes.new("ShaderNodeBsdfPrincipled"); bsdf.inputs["Roughness"].default_value = 0.95
        baked = nt.nodes.new("ShaderNodeTexImage")
        baked.image = bpy.data.images.load(str(Path(args.bake_png).resolve()))
        nt.links.new(baked.outputs["Color"], bsdf.inputs["Base Color"])
        nt.links.new(bsdf.outputs["BSDF"], out.inputs["Surface"])
        print(f"BAKED_ALBEDO -> {args.bake_png}")

    if args.save_glb:
        bpy.ops.object.select_all(action="DESELECT"); terr.select_set(True)
        bpy.context.view_layer.objects.active = terr
        bpy.ops.export_scene.gltf(filepath=str(Path(args.save_glb).resolve()),
                                  export_format="GLB", use_selection=True, export_apply=True, export_yup=True)
        print(f"SAVED_GLB {args.save_glb}")


if __name__ == "__main__":
    main()
