"""Mesure le personnage stylisé avant tout retarget.

    & "C:\\Program Files\\Blender Foundation\\Blender 4.5\\blender.exe" ^
      --background --factory-startup ^
      --python tools/blender/personnage/20_probe_personnage.py

Trois questions décident de la suite, et aucune ne se devine :

1. **Comment les os sont-ils nommés ?** Mixamo préfixe `mixamorig:` ; Unreal
   utilise `pelvis`/`spine_01`/`upperarm_l`. `retarget_mixamo.py` fait la
   correspondance entre les deux — encore faut-il savoir de quel côté on est.
2. **Quelle taille fait le personnage ?** Le joueur de Forgia mesure 2,00 m.
   Un FBX exporté en centimètres arrive 100 fois trop grand, et l'erreur ne se
   voit qu'une fois le personnage posé dans la carte.
3. **Le pack porte-t-il déjà des animations ?** S'il n'y a qu'une pose de
   repos, tout le mouvement viendra de Mixamo.

Leçon V1 en mémoire : « Mixamo rig non interchangeable — 1 rig par character,
jamais cross-rig ». On ne réutilise donc pas la table du Trooper sans avoir
vérifié que les noms correspondent.
"""

import json
import os

import bpy
from mathutils import Vector

BASE = r"D:\ressources externes\FAB\fbx_stylizedfantasycharacters (1)"
CIBLES = [
    ("mixamo_male", os.path.join(BASE, "Mixamo", "SM_FantasyMale.fbx")),
    ("mixamo_male_cape", os.path.join(BASE, "Mixamo", "SM_FantasyMale_Cloak.fbx")),
    ("ue_male", os.path.join(BASE, "UE", "SM_StylizedMale_UE.fbx")),
    ("ue_male_sac", os.path.join(BASE, "UE", "SM_StylizedMale_Bag_UE.fbx")),
]


def vider():
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)
    for coll in (bpy.data.meshes, bpy.data.armatures, bpy.data.actions,
                 bpy.data.materials, bpy.data.images):
        for b in list(coll):
            try:
                coll.remove(b)
            except (RuntimeError, ReferenceError):
                pass


def convention(noms):
    """De quelle famille de nommage ce squelette relève-t-il ?"""
    if any(n.startswith("mixamorig") for n in noms):
        return "mixamo"
    if any(n in noms for n in ("pelvis", "spine_01", "upperarm_l")):
        return "unreal"
    if any(n in noms for n in ("Hips", "Spine", "LeftArm")):
        return "mixamo_sans_prefixe"
    return "inconnue"


def mesurer(nom, chemin):
    if not os.path.exists(chemin):
        return {"cible": nom, "erreur": "absent"}
    vider()
    bpy.ops.import_scene.fbx(filepath=chemin)

    objets = list(bpy.context.scene.objects)
    arms = [o for o in objets if o.type == "ARMATURE"]
    meshes = [o for o in objets if o.type == "MESH"]

    lo = [1e9] * 3
    hi = [-1e9] * 3
    tris = 0
    for o in meshes:
        o.data.calc_loop_triangles()
        tris += len(o.data.loop_triangles)
        for c in o.bound_box:
            w = o.matrix_world @ Vector(c)
            for a in range(3):
                lo[a] = min(lo[a], w[a])
                hi[a] = max(hi[a], w[a])

    os_noms = [b.name for a in arms for b in a.data.bones]
    return {
        "cible": nom,
        "octets": os.path.getsize(chemin),
        "objets": len(objets),
        "meshes": [o.name for o in meshes],
        "tris": tris,
        "armatures": [a.name for a in arms],
        "os": len(os_noms),
        "convention": convention(set(os_noms)),
        "os_exemples": sorted(os_noms)[:16],
        # La hauteur DÉCIDE de l'échelle d'import : un FBX en centimètres
        # arrive à 178 au lieu de 1,78.
        "hauteur_m": round(hi[2] - lo[2], 3),
        "emprise_m": [round(hi[i] - lo[i], 3) for i in range(3)],
        "z_min": round(lo[2], 3),
        "actions": sorted(a.name for a in bpy.data.actions),
        "materiaux": sorted({m.name for o in meshes for m in o.data.materials if m}),
    }


rapport = [mesurer(n, c) for n, c in CIBLES]
print("RESULT: " + json.dumps({"joueur_m": 2.0, "cibles": rapport}, ensure_ascii=False))
