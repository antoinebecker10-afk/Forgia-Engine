"""Vérifie que le cycle de marche RESSEMBLE à une marche.

Le risque du cycle scripté : l'axe de rotation. Si les os des pattes ont une
orientation de repos différente de celle supposée, la jambe s'ÉCARTE sur le
côté au lieu de balancer vers l'avant — le rig fait le grand écart, et rien
dans le rapport de génération ne le dirait. Quatre images de profil suffisent
à trancher.

    & "...\\blender.exe" --background --factory-startup ^
      --python tools/blender/expedition/11_controle_animaux.py
"""

import json
import math
import os

import bpy
from mathutils import Vector

ANIMAUX = os.path.join(r"C:\Users\Antoi\Desktop\Forgia Rewrite",
                       "assets", "models", "characters", "animals")
SORTIE = os.path.join(r"C:\Users\Antoi\Desktop\Forgia Rewrite",
                      "tools", "blender", "expedition", "vues")
# Les extrêmes du cycle : appui avant, passage, appui arrière, passage.
IMAGES = [1, 7, 13, 19]
CIBLES = ["deer", "chicken"]


def vider():
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)
    for c in (bpy.data.meshes, bpy.data.armatures, bpy.data.actions):
        for b in list(c):
            try:
                c.remove(b)
            except (RuntimeError, ReferenceError):
                pass


def bornes(objets):
    lo = [1e9] * 3
    hi = [-1e9] * 3
    for o in objets:
        if o.type != "MESH":
            continue
        for c in o.bound_box:
            w = o.matrix_world @ Vector(c)
            for a in range(3):
                lo[a] = min(lo[a], w[a])
                hi[a] = max(hi[a], w[a])
    return lo, hi


def main():
    os.makedirs(SORTIE, exist_ok=True)
    scene = bpy.context.scene
    scene.render.engine = "BLENDER_EEVEE_NEXT"
    scene.render.resolution_x = 640
    scene.render.resolution_y = 480
    rapport = []

    for espece in CIBLES:
        chemin = os.path.join(ANIMAUX, f"{espece}.glb")
        if not os.path.exists(chemin):
            rapport.append({"animal": espece, "erreur": "GLB absent"})
            continue
        vider()
        bpy.ops.import_scene.gltf(filepath=chemin)
        objets = list(bpy.context.scene.objects)
        arm = next((o for o in objets if o.type == "ARMATURE"), None)
        clips = sorted(a.name for a in bpy.data.actions)

        # On force le clip de marche : l'import peut activer n'importe lequel.
        marche = next((a for a in bpy.data.actions if a.name.endswith("walk")), None)
        if arm and marche:
            arm.animation_data_create()
            arm.animation_data.action = marche

        lo, hi = bornes(objets)
        centre = [(lo[i] + hi[i]) / 2 for i in range(3)]
        taille = max(hi[i] - lo[i] for i in range(3)) or 1.0

        cam_data = bpy.data.cameras.new("C")
        cam = bpy.data.objects.new("C", cam_data)
        scene.collection.objects.link(cam)
        scene.camera = cam
        # DE PROFIL, et le profil se MESURE : le grand axe horizontal de
        # l'emprise est la longueur du corps, donc la caméra se place
        # perpendiculairement à lui. Supposer l'orientation avait donné une vue
        # de derrière, d'où l'on ne distingue rien d'un balancement de patte.
        long_x, long_y = hi[0] - lo[0], hi[1] - lo[1]
        recul = taille * 2.6
        if long_x >= long_y:
            cam.location = (centre[0], centre[1] - recul, centre[2] + taille * 0.30)
            cam.rotation_euler = (math.radians(85.0), 0.0, 0.0)
        else:
            cam.location = (centre[0] + recul, centre[1], centre[2] + taille * 0.30)
            cam.rotation_euler = (math.radians(85.0), 0.0, math.radians(90.0))

        sun = bpy.data.objects.new("S", bpy.data.lights.new("S", type="SUN"))
        sun.data.energy = 4.0
        sun.rotation_euler = (math.radians(55.0), 0.0, math.radians(40.0))
        scene.collection.objects.link(sun)

        rendus = []
        for img in IMAGES:
            scene.frame_set(img)
            scene.render.filepath = os.path.join(SORTIE, f"anim_{espece}_{img:02d}.png")
            bpy.ops.render.render(write_still=True)
            rendus.append(os.path.basename(scene.render.filepath))

        rapport.append({
            "animal": espece, "clips": clips,
            "clip_joue": marche.name if marche else None,
            "taille_m": round(taille, 2), "rendus": rendus,
        })

    print("RESULT: " + json.dumps(rapport, ensure_ascii=False))


main()
