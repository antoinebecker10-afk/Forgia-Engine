"""Catalogue les 329 pièces du kit Kenney Nature : dimensions, tris, matériaux.

Pourquoi un catalogue plutôt qu'une liste de noms écrite à la main : le
générateur doit pouvoir dire « une pièce de sous-bois basse » ou « un arbre de
plus de 6 m » sans qu'un humain ait retapé 329 hauteurs — et sans qu'un nom
mal deviné (`tree_pineDefaultA` existe, `tree_pine` non) fasse échouer une
passe entière après 20 minutes de placement.

Sortie : tools/blender/expedition/kit_catalog.json
"""

import json
import os

import bpy
import mathutils

KIT = r"C:\Users\Antoi\Desktop\Forgia Rewrite\assets\models\kenney\nature"
OUT = r"C:\Users\Antoi\Desktop\Forgia Rewrite\tools\blender\expedition\kit_catalog.json"


def wipe():
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)
    for coll in (bpy.data.meshes, bpy.data.materials, bpy.data.images):
        for block in list(coll):
            coll.remove(block)


def measure(filename):
    path = os.path.join(KIT, filename)
    before = set(bpy.data.objects)
    try:
        bpy.ops.import_scene.gltf(filepath=path)
    except Exception as exc:  # noqa: BLE001 — un GLB cassé ne doit pas tuer la passe
        return {"file": filename, "erreur": str(exc)[:120]}

    fresh = [o for o in bpy.data.objects if o not in before]
    meshes = [o for o in fresh if o.type == "MESH"]
    if not meshes:
        return {"file": filename, "erreur": "aucun mesh"}

    lo = [float("inf")] * 3
    hi = [float("-inf")] * 3
    tris = 0
    mats = set()
    for obj in meshes:
        obj.data.calc_loop_triangles()
        tris += len(obj.data.loop_triangles)
        for slot in obj.material_slots:
            if slot.material:
                mats.add(slot.material.name.split(".")[0])
        for corner in obj.bound_box:
            world = obj.matrix_world @ mathutils.Vector(corner)
            for axis in range(3):
                lo[axis] = min(lo[axis], world[axis])
                hi[axis] = max(hi[axis], world[axis])

    return {
        "file": filename,
        "nom": os.path.splitext(filename)[0],
        "famille": os.path.splitext(filename)[0].split("_")[0],
        "meshes": len(meshes),
        "tris": tris,
        "dim": [round(hi[i] - lo[i], 4) for i in range(3)],
        "z_min": round(lo[2], 4),
        "z_max": round(hi[2], 4),
        "materiaux": sorted(mats),
    }


wipe()
fichiers = sorted(f for f in os.listdir(KIT) if f.endswith(".glb"))
catalogue = []
for name in fichiers:
    catalogue.append(measure(name))
    wipe()

with open(OUT, "w", encoding="utf-8") as handle:
    json.dump(catalogue, handle, ensure_ascii=False, indent=1)

familles = {}
materiaux = {}
for entry in catalogue:
    familles[entry.get("famille", "?")] = familles.get(entry.get("famille", "?"), 0) + 1
    for mat in entry.get("materiaux", []):
        materiaux[mat] = materiaux.get(mat, 0) + 1

resume = {
    "pieces": len(catalogue),
    "erreurs": [e for e in catalogue if "erreur" in e],
    "familles": dict(sorted(familles.items(), key=lambda kv: -kv[1])),
    # LE point décisif pour la fusion : si tout le kit partage un seul matériau,
    # la carte entière se fusionne en très peu de meshes.
    "materiaux": dict(sorted(materiaux.items(), key=lambda kv: -kv[1])),
    "tris_total_si_une_de_chaque": sum(e.get("tris", 0) for e in catalogue),
}
print("RESULT: " + json.dumps(resume, ensure_ascii=False))
