"""Cadre la scène et rend des images. Le seul juge honnête d'une carte.

Les vues au sol sont posées à 1,70 m — l'œil du joueur mesuré dans le projet —
et le sol est trouvé par lancer de rayon, jamais supposé. Une carte jugée
depuis le ciel se croit belle ; c'est à hauteur d'homme qu'on voit qu'elle est
plate, ou qu'un massif bouche le chemin.
"""

import json
import math
import os

import bpy
from mathutils import Vector

SORTIE = r"C:\Users\Antoi\Desktop\Forgia Rewrite\tools\blender\expedition\vues"
os.makedirs(SORTIE, exist_ok=True)

OEIL = 1.70

# nom: (xy caméra, xy cible, hauteur caméra, hauteur cible)
VUES_SOL = {
    "01_depart":   ((-124.0, -10.0), (-100.0, -40.0), OEIL, 1.0),
    "02_camp1":    ((-76.0, -47.0), (-58.0, -40.0), OEIL, 1.5),
    "03_riviere":  ((-14.0, 20.0), (-30.0, 46.0), OEIL + 6.0, 0.0),
    "04_pont":     ((-34.0, 26.0), (-22.0, 44.0), OEIL, 1.0),
    "05_camp3":    ((36.0, 20.0), (52.0, 6.0), OEIL, 1.5),
    "06_porte":    ((90.0, -52.0), (90.0, -22.0), OEIL, 4.0),
    "07_village":  ((90.0, -22.0), (90.0, 4.0), OEIL, 4.0),
}
VUES_AIR = {
    "00_plongee":    ((0.0, -300.0, 250.0), (0.0, 0.0, 0.0)),
    "08_survol_est": ((-30.0, -150.0, 105.0), (80.0, 0.0, 0.0)),
    "09_pont_dessus": ((-26.0, 36.0, 62.0), (-26.0, 36.5, 0.0)),
    "15_pont_pierre": ((-52.0, 20.0, 14.0), (-24.0, 39.0, 3.0)),
    "10_faune": ((0.0, -1.0, 340.0), (0.0, 0.0, 0.0)),
    "18_lampes": ((-96.0, -30.0, 8.0), (-60.0, -42.0, 2.0)),
    "16_gorge_nord": ((-34.0, 52.0, 18.0), (-35.0, 90.0, 2.0)),
    "17_gorge_sud": ((-40.0, -52.0, 16.0), (-42.0, -90.0, 2.0)),
    "11_cerfs": ((-18.0, -18.0, 9.0), (6.0, -6.0, 1.0)),
    "12_village_poules": ((78.0, -62.0, 8.0), (96.5, -46.9, 1.0)),
}


def sol_en(x, y):
    """Altitude du sol par lancer de rayon vers le bas."""
    depsgraph = bpy.context.evaluated_depsgraph_get()
    touche, position, _, _, _, _ = bpy.context.scene.ray_cast(
        depsgraph, Vector((x, y, 400.0)), Vector((0.0, 0.0, -1.0))
    )
    return position.z if touche else 0.0


def viser(cam, cible):
    d = (cible[0] - cam.location.x, cible[1] - cam.location.y, cible[2] - cam.location.z)
    plat = math.hypot(d[0], d[1])
    # La caméra Blender regarde son -Z local ; le lacet se RETRANCHE (avec +π/2
    # elle vise l'exact opposé et on rend un fond vide).
    cam.rotation_euler = (math.atan2(plat, -d[2]), 0.0, math.atan2(d[1], d[0]) - math.pi / 2)


def main():
    scene = bpy.context.scene
    cam_data = bpy.data.cameras.get("VueCam") or bpy.data.cameras.new("VueCam")
    cam_data.lens = 28.0                      # ~65° horizontal, proche d'un FOV FPS
    cam = bpy.data.objects.get("VueCam")
    if cam is None:
        cam = bpy.data.objects.new("VueCam", cam_data)
        scene.collection.objects.link(cam)
    cam.data = cam_data
    scene.camera = cam

    if "SoleilVue" not in bpy.data.objects:
        sd = bpy.data.lights.new("SoleilVue", type="SUN")
        sd.energy = 3.2
        sd.angle = math.radians(2.5)          # ombres légèrement douces
        sun = bpy.data.objects.new("SoleilVue", sd)
        sun.rotation_euler = (math.radians(56.0), 0.0, math.radians(28.0))
        scene.collection.objects.link(sun)

    # Ciel : sans lumière d'ambiance, les ombres sont des trous noirs et la
    # scène paraît sale. Un dégradé bleu suffit à asseoir la lecture.
    monde = scene.world or bpy.data.worlds.new("Monde")
    scene.world = monde
    monde.use_nodes = True
    fond = monde.node_tree.nodes.get("Background")
    if fond:
        fond.inputs[0].default_value = (0.42, 0.60, 0.85, 1.0)
        fond.inputs[1].default_value = 1.1

    scene.render.engine = "BLENDER_EEVEE_NEXT"
    scene.render.resolution_x = 1280
    scene.render.resolution_y = 720

    rendus = []
    for nom, (pos, cible) in VUES_AIR.items():
        cam.location = pos
        viser(cam, cible)
        scene.render.filepath = os.path.join(SORTIE, f"{nom}.png")
        bpy.ops.render.render(write_still=True)
        rendus.append(nom)

    releves = {}
    for nom, (cxy, txy, dh, th) in VUES_SOL.items():
        zc = sol_en(*cxy) + dh
        zt = sol_en(*txy) + th
        releves[nom] = [round(zc - dh, 2), round(zt - th, 2)]
        cam.location = (cxy[0], cxy[1], zc)
        viser(cam, (txy[0], txy[1], zt))
        scene.render.filepath = os.path.join(SORTIE, f"{nom}.png")
        bpy.ops.render.render(write_still=True)
        rendus.append(nom)

    print("RESULT: " + json.dumps({"rendus": rendus, "sol_camera_cible": releves},
                                  ensure_ascii=False))


main()
