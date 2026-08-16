"""Mesure le kit village KayKit (medieval_hexagon) : bâtiments et murs.

Deux kits d'origines différentes n'ont AUCUNE raison d'être à la même échelle.
Supposer qu'ils le sont, c'est livrer une église de 2 m ou une porte de 30 m.
On mesure, on dérive le facteur, on l'écrit une seule fois.
"""

import json
import os

import bpy
import mathutils

BASE = r"C:\Users\Antoi\Desktop\Forgia Rewrite\assets\models\kaykit\medieval_hexagon"
CIBLES = [
    ("buildings/red", "building_church_red.gltf"),
    ("buildings/red", "building_home_A_red.gltf"),
    ("buildings/red", "building_home_B_red.gltf"),
    ("buildings/red", "building_market_red.gltf"),
    ("buildings/red", "building_tavern_red.gltf"),
    ("buildings/red", "building_well_red.gltf"),
    ("walls", "wall_straight.gltf"),
    ("walls", "wall_straight_gate.gltf"),
    ("walls", "wall_corner_A_outside.gltf"),
    ("walls", "wall_corner_B_outside.gltf"),
]


def wipe():
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)
    for coll in (bpy.data.meshes, bpy.data.materials, bpy.data.images):
        for block in list(coll):
            coll.remove(block)


def measure(sub, filename):
    path = os.path.join(BASE, sub, filename)
    if not os.path.exists(path):
        return {"file": filename, "erreur": "absent"}
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=path)
    meshes = [o for o in bpy.data.objects if o not in before and o.type == "MESH"]
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
        "tris": tris,
        "dim": [round(hi[i] - lo[i], 3) for i in range(3)],
        "z_min": round(lo[2], 3),
        "materiaux": sorted(mats),
    }


wipe()
rapport = []
for sub, name in CIBLES:
    rapport.append(measure(sub, name))
    wipe()

print("RESULT: " + json.dumps(rapport, ensure_ascii=False))
