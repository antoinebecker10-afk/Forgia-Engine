"""Mesure la cape et la dague avant de les attacher.

Deux accessoires, deux natures très différentes — et c'est la mesure qui le dit,
pas le nom du fichier :

  - la **cape** est un maillage SKINNÉ sur sa propre chaîne de 6 os génériques
    (`Bone`, `Bone.001`…). Elle ne partage donc rien avec le squelette du
    personnage : on ne peut pas simplement lui appliquer ses animations.
  - la **dague** n'a AUCUN os. C'est un accessoire statique, à socketer sur un
    os de main ou de ceinture.

À noter : le pack ne livre pas de dague masculine — seule
`SM_StylizedFemale_Dagger_UE.fbx` existe. C'est une arme, pas un vêtement : elle
se socketera pareil sur l'un ou l'autre.
"""

import json
import os

import bpy
from mathutils import Vector

BASE = r"D:\ressources externes\FAB\fbx_stylizedfantasycharacters (1)"
CIBLES = [
    ("cape_mixamo", os.path.join(BASE, "Mixamo", "SM_FantasyMale_Cloak.fbx")),
    ("cape_ue", os.path.join(BASE, "UE", "SM_StylizedMale_Cloak_UE.fbx")),
    ("dague", os.path.join(BASE, "UE", "SM_StylizedFemale_Dagger_UE.fbx")),
    ("sac_ue", os.path.join(BASE, "UE", "SM_StylizedMale_Bag_UE.fbx")),
    ("gourde_ue", os.path.join(BASE, "UE", "SM_StylizedMale_Bottle_UE.fbx")),
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


def mesurer(nom, chemin):
    if not os.path.exists(chemin):
        return {"piece": nom, "erreur": "absent"}
    vider()
    bpy.ops.import_scene.fbx(filepath=chemin)
    objets = list(bpy.context.scene.objects)
    arms = [o for o in objets if o.type == "ARMATURE"]
    meshes = [o for o in objets if o.type == "MESH"]

    lo = [1e9] * 3
    hi = [-1e9] * 3
    tris = 0
    modif = set()
    for o in meshes:
        o.data.calc_loop_triangles()
        tris += len(o.data.loop_triangles)
        modif |= {m.type for m in o.modifiers}
        for c in o.bound_box:
            w = o.matrix_world @ Vector(c)
            for a in range(3):
                lo[a] = min(lo[a], w[a])
                hi[a] = max(hi[a], w[a])

    os_noms = [b.name for a in arms for b in a.data.bones]
    return {
        "piece": nom,
        "meshes": [o.name for o in meshes],
        "tris": tris,
        "os": len(os_noms),
        "os_noms": sorted(os_noms)[:8],
        # Un maillage SKINNÉ porte un modificateur Armature ; un accessoire
        # statique n'en a pas. C'est ce qui décide de la façon de l'attacher.
        "skinne": "ARMATURE" in modif,
        "emprise_m": [round(hi[i] - lo[i], 3) for i in range(3)],
        "z": [round(lo[2], 3), round(hi[2], 3)],
        "materiaux": sorted({m.name for o in meshes for m in o.data.materials if m}),
    }


print("RESULT: " + json.dumps([mesurer(n, c) for n, c in CIBLES], ensure_ascii=False))
