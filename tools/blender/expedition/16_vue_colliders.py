"""Rend les vues de CONTRÔLE des colliders. À lancer après `15_colliders.py`.

Les enveloppes sont émissives et à 38 % d'opacité : on doit voir la pièce ET son
collider en même temps, sinon on ne juge pas l'écart entre les deux — qui est
tout le sujet.

Le jeu de vues n'est pas un tour touristique. Chaque image répond à UNE question
qu'on ne peut pas trancher sur un tableau :

  - la futaie à hauteur d'œil : **est-ce qu'on peut encore y marcher ?** C'est la
    seule question ouverte laissée par le passage du rayon de tronc de 0,22 à
    0,51 m, et aucun chiffre ne la tranche.
  - un campement de près : les six abris couvrent-ils vraiment, ou laissent-ils
    la salle nue ?
  - la ceinture : les bouchons ferment-ils, sans mordre dans le jouable ?
  - le chemin : le couloir de marche est-il libre de bout en bout ?

  python tools/blender/bmcp.py code tools/blender/expedition/16_vue_colliders.py
"""

import json
import math
import os

import bpy
from mathutils import Vector

SORTIE = r"C:\Users\Antoi\Desktop\Forgia Rewrite\tools\blender\expedition\vues\colliders"
os.makedirs(SORTIE, exist_ok=True)

OEIL = 1.70

# Vues au sol : (xy caméra, xy cible, hauteur caméra, hauteur cible)
# C'est à hauteur d'homme qu'on voit qu'un décor est impraticable ; vu du ciel,
# tout paraît aéré.
VUES_SOL = {
    "01_futaie_oeil":   ((-95.0, 22.0), (-70.0, 30.0), OEIL, OEIL),
    "02_futaie_dense":  ((10.0, -70.0), (34.0, -62.0), OEIL, OEIL),
    "03_camp1_seuil":   ((-76.0, -47.0), (-62.5, -42.9), OEIL, 1.5),
    "04_camp3_seuil":   ((36.0, 16.0), (49.0, 8.0), OEIL, 1.5),
    "05_chemin_couloir": ((-42.0, -10.0), (-44.0, 8.0), OEIL, 1.0),
}

# Vues aériennes : (position, cible)
VUES_AIR = {
    "00_carte_entiere":  ((0.0, -300.0, 250.0), (0.0, 0.0, 0.0)),
    "06_camp1_dessus":   ((-62.5, -42.9, 34.0), (-62.5, -42.8, 0.0)),
    "07_camp2_dessus":   ((-1.9, 45.6, 34.0), (-1.9, 45.7, 0.0)),
    "08_camp3_dessus":   ((49.0, 8.0, 34.0), (49.0, 8.1, 0.0)),
    "09_ceinture_ouest": ((-90.0, 0.0, 40.0), (-135.0, 0.0, 10.0)),
    "10_futaie_dessus":  ((-80.0, 26.0, 46.0), (-80.0, 26.1, 0.0)),
}


def sol_en(x, y):
    depsgraph = bpy.context.evaluated_depsgraph_get()
    touche, position, _, _, _, _ = bpy.context.scene.ray_cast(
        depsgraph, Vector((x, y, 400.0)), Vector((0.0, 0.0, -1.0))
    )
    return position.z if touche else 0.0


def viser(cam, cible):
    d = (cible[0] - cam.location.x, cible[1] - cam.location.y, cible[2] - cam.location.z)
    plat = math.hypot(d[0], d[1])
    # La caméra Blender regarde son -Z local ; le lacet se RETRANCHE.
    cam.rotation_euler = (math.atan2(plat, -d[2]), 0.0, math.atan2(d[1], d[0]) - math.pi / 2)


def main():
    if bpy.data.collections.get("collisions") is None:
        print("RESULT: " + json.dumps({
            "erreur": "collection « collisions » absente",
            "remede": "lancer 15_colliders.py d'abord",
        }, ensure_ascii=False))
        return

    scene = bpy.context.scene
    cam_data = bpy.data.cameras.get("VueCam") or bpy.data.cameras.new("VueCam")
    cam_data.lens = 28.0
    cam = bpy.data.objects.get("VueCam")
    if cam is None:
        cam = bpy.data.objects.new("VueCam", cam_data)
        scene.collection.objects.link(cam)
    cam.data = cam_data
    scene.camera = cam

    if "SoleilVue" not in bpy.data.objects:
        sd = bpy.data.lights.new("SoleilVue", type="SUN")
        sd.energy = 3.2
        sd.angle = math.radians(2.5)
        sun = bpy.data.objects.new("SoleilVue", sd)
        sun.rotation_euler = (math.radians(56.0), 0.0, math.radians(28.0))
        scene.collection.objects.link(sun)

    monde = scene.world or bpy.data.worlds.new("Monde")
    scene.world = monde
    monde.use_nodes = True
    fond = monde.node_tree.nodes.get("Background")
    if fond:
        fond.inputs[0].default_value = (0.42, 0.60, 0.85, 1.0)
        fond.inputs[1].default_value = 1.1

    scene.render.engine = "BLENDER_EEVEE_NEXT"
    scene.render.resolution_x = 1600
    scene.render.resolution_y = 900

    # LE DÉCOR SE CACHE, PUIS SE MONTRE. Une enveloppe translucide posée sur la
    # pièce qu'elle englobe est illisible : on ne sait plus ce qui est collider
    # et ce qui est feuillage. La première passe de rendu l'a prouvé — une image
    # de futaie à hauteur d'œil où RIEN n'était identifiable.
    # On rend donc chaque vue DEUX fois, et c'est la paire qui informe : la
    # forme seule, puis ce qu'elle recouvre.
    autres = [c for c in bpy.data.collections
              if c.name not in ("collisions",) and c.name not in ("_proto", "_src")]

    def montrer_decor(visible):
        for c in autres:
            c.hide_render = not visible

    rendus = []

    def rendre(nom, pos, cible):
        cam.location = pos
        viser(cam, cible)
        for suffixe, decor in (("_seul", False), ("_pose", True)):
            montrer_decor(decor)
            scene.render.filepath = os.path.join(SORTIE, f"{nom}{suffixe}.png")
            bpy.ops.render.render(write_still=True)
            rendus.append(nom + suffixe)
        montrer_decor(True)

    for nom, (pos, cible) in VUES_AIR.items():
        rendre(nom, pos, cible)

    releves = {}
    for nom, (cxy, txy, dh, th) in VUES_SOL.items():
        # Le sol se TROUVE par lancer de rayon. Le supposer poserait la caméra
        # sous le terrain sur une carte qui monte de -5,8 à +15 m.
        # Il se mesure DÉCOR VISIBLE : le rayon doit frapper le terrain, pas
        # traverser une scène dont on vient de cacher le sol.
        montrer_decor(True)
        zc = sol_en(*cxy) + dh
        zt = sol_en(*txy) + th
        releves[nom] = [round(zc - dh, 2), round(zt - th, 2)]
        rendre(nom, (cxy[0], cxy[1], zc), (txy[0], txy[1], zt))

    print("RESULT: " + json.dumps(
        {"rendus": rendus, "dossier": SORTIE, "sol_camera_cible": releves},
        ensure_ascii=False))


main()
