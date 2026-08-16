"""Mesure les candidats lampadaires avant d'en semer 40 le long du chemin.

Un lampadaire de chemin a deux exigences que seule la mesure tranche :

1. **Sa flamme doit être au-dessus du regard** (œil du joueur : 1,70 m), sinon
   elle éblouit au lieu d'éclairer.
2. **Son emprise au sol doit rester hors du couloir de marche** — le Vallon
   garde 3,6 m dégagés de part et d'autre de l'axe (`degagement_chemin`).

On relève aussi la hauteur de la FLAMME dans chaque pièce : c'est là que le
moteur devra poser sa lumière ponctuelle, et une lumière posée à l'origine de
l'objet éclairerait le sol au lieu du chemin.
"""

import json
import os

import bpy
from mathutils import Vector

RACINE = r"C:\Users\Antoi\Desktop\Forgia Rewrite"
CIBLES = [
    ("torche", "kaykit/dungeon_remastered/torch.gltf.glb"),
    ("torche_allumee", "kaykit/dungeon_remastered/torch_lit.gltf.glb"),
    ("torche_murale", "kaykit/dungeon_remastered/torch_mounted.gltf.glb"),
    ("brasero_2", "environment/inferno/Brazier_002.glb"),
    ("brasero_4", "environment/inferno/Brazier_004.glb"),
]


def vider():
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)
    for coll in (bpy.data.meshes, bpy.data.materials, bpy.data.images):
        for b in list(coll):
            try:
                coll.remove(b)
            except (RuntimeError, ReferenceError):
                pass


def mesurer(nom, rel):
    chemin = os.path.join(RACINE, "assets", "models", rel.replace("/", os.sep))
    if not os.path.exists(chemin):
        return {"piece": nom, "erreur": "absent"}
    vider()
    bpy.ops.import_scene.gltf(filepath=chemin)
    meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    lo = [1e9] * 3
    hi = [-1e9] * 3
    tris = 0
    mats = set()
    for o in meshes:
        o.data.calc_loop_triangles()
        tris += len(o.data.loop_triangles)
        for m in o.data.materials:
            if m:
                mats.add(m.name.split(".")[0])
        for c in o.bound_box:
            w = o.matrix_world @ Vector(c)
            for a in range(3):
                lo[a] = min(lo[a], w[a])
                hi[a] = max(hi[a], w[a])
    return {
        "piece": nom,
        "meshes": len(meshes),
        "tris": tris,
        "emprise_m": [round(hi[i] - lo[i], 3) for i in range(3)],
        "z": [round(lo[2], 3), round(hi[2], 3)],
        "materiaux": sorted(mats),
    }


print("RESULT: " + json.dumps([mesurer(n, r) for n, r in CIBLES], ensure_ascii=False))
