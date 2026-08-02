"""preview_trooper.py — Planche de contrôle des pièces du Trooper.

Rend le personnage assemblé puis chaque slot isolé, EN RELISANT LES .gltf PRODUITS
(pas la scène de build) : la planche valide donc aussi l'export et les textures,
pas seulement la découpe en mémoire.

Usage :
    blender -b --python tools/blender/preview_trooper.py -- <trooper_dir> <out_dir>
"""

import bpy
import json
import os
import sys

import mathutils

SLOT_ORDER = ["helmet", "chest", "gloves", "legs", "boots"]


RES_X, RES_Y = 460, 760


def clear():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    # `use_empty` laisse quand même un maillage fantôme : il fausserait le compte
    # de triangles rapporté par la planche (piège déjà payé sur le nain).
    for m in list(bpy.data.objects):
        if m.type == "MESH":
            bpy.data.objects.remove(m, do_unlink=True)


def setup_scene():
    sc = bpy.context.scene
    sc.render.engine = "BLENDER_EEVEE"
    sc.render.resolution_x, sc.render.resolution_y = RES_X, RES_Y
    sc.render.film_transparent = False
    sc.world = bpy.data.worlds.new("w")
    sc.world.use_nodes = True
    # Blender 5 ne nomme plus le nœud « Background » : on le retrouve par type.
    bg = next(n for n in sc.world.node_tree.nodes if n.type == "BACKGROUND")
    bg.inputs["Color"].default_value = (0.05, 0.055, 0.07, 1)
    bg.inputs["Strength"].default_value = 1.1

    cam_data = bpy.data.cameras.new("cam")
    cam_data.type = "ORTHO"
    cam = bpy.data.objects.new("cam", cam_data)
    sc.collection.objects.link(cam)
    sc.camera = cam

    for loc, energy, color in (
        ((3, -4, 4), 900, (1.0, 0.92, 0.82)),
        ((-4, -2, 2.5), 380, (0.72, 0.82, 1.0)),
        ((-1.5, 4, 3), 520, (1.0, 0.66, 0.42)),
    ):
        lamp = bpy.data.lights.new("l", "AREA")
        lamp.energy, lamp.size, lamp.color = energy, 2.5, color
        ob = bpy.data.objects.new("l", lamp)
        ob.location = loc
        d = mathutils.Vector((0, 0, 1.0)) - ob.location
        ob.rotation_euler = d.to_track_quat("-Z", "Y").to_euler()
        sc.collection.objects.link(ob)
    return sc


def frame_on(cam, meshes):
    """Cadre la caméra sur l'emprise réelle des mesh présents.

    🚨 `ortho_scale` cadre la PLUS GRANDE dimension du rendu — ici la hauteur.
    Le dériver de l'emprise évite à la fois de couper le personnage et de rendre
    une pièce isolée invisible parce qu'elle est loin de l'origine.
    """
    pts = [o.matrix_world @ mathutils.Vector(c) for o in meshes for c in o.bound_box]
    mn = mathutils.Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
    mx = mathutils.Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
    center = (mn + mx) / 2
    size = mx - mn
    span = max(size.z, max(size.x, size.y) * RES_Y / RES_X)
    cam.data.ortho_scale = span * 1.25
    direction = mathutils.Vector((0.58, -0.80, 0.16))
    cam.location = center + direction * 6.0
    cam.rotation_euler = (-direction).to_track_quat("-Z", "Y").to_euler()


def render(files, trooper_dir, out_png):
    clear()
    sc = setup_scene()
    for f in files:
        bpy.ops.import_scene.gltf(filepath=os.path.join(trooper_dir, f))
    # 🚨 L'importeur glTF ajoute une Icosphère de 80 tris par fichier importé —
    # vérifié absente des .gltf (`grep -i icosph` = 0). Sans ce filtre, la planche
    # sur-compte de 80 tris par pièce et le chiffre annoncé est faux.
    for o in [o for o in bpy.data.objects if o.type == "MESH" and not o.name.startswith("trooper_")]:
        bpy.data.objects.remove(o, do_unlink=True)
    meshes = [o for o in bpy.data.objects if o.type == "MESH"]
    frame_on(sc.camera, meshes)
    sc.render.filepath = out_png
    bpy.ops.render.render(write_still=True)
    tris = sum(sum(len(p.vertices) - 2 for p in o.data.polygons) for o in meshes)
    print(f"[preview] {os.path.basename(out_png)}  {len(meshes)} mesh, {tris} tris")


def main():
    argv = sys.argv[sys.argv.index("--") + 1:]
    trooper_dir, out_dir = argv[0], argv[1]
    os.makedirs(out_dir, exist_ok=True)
    with open(os.path.join(trooper_dir, "manifest.json"), encoding="utf-8") as f:
        manifest = json.load(f)

    body = manifest["body"]["file"]
    slots = [s for s in SLOT_ORDER if s in manifest["slots"]]

    # Assemblé, puis corps nu, puis chaque pièce SEULE (sans corps : c'est la
    # découpe qu'on juge, pas l'habillage).
    render([body] + [manifest["slots"][s]["file"] for s in slots],
           trooper_dir, os.path.join(out_dir, "00_equipe.png"))
    render([body], trooper_dir, os.path.join(out_dir, "01_corps.png"))
    for i, s in enumerate(slots, start=2):
        render([manifest["slots"][s]["file"]],
               trooper_dir, os.path.join(out_dir, f"{i:02d}_{s}.png"))


main()
