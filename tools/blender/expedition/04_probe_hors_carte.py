"""Qui traîne hors de l'emprise de la carte ?

L'emprise est 240 × 160 m, donc tout sommet au-delà de |x|>125 ou |y|>85 est
une anomalie. On veut le NOM et la COLLECTION des fautifs : « il y a des trucs
qui traînent » ne se corrige pas, « 312 objets de la collection foret à 300 m »
se corrige.
"""

import json

import bpy

LIM_X, LIM_Y = 125.0, 85.0

hors = []
for obj in bpy.data.objects:
    if obj.type != "MESH" or not obj.data.vertices:
        continue
    xs, ys, zs = [], [], []
    for v in obj.data.vertices:
        w = obj.matrix_world @ v.co
        xs.append(w.x)
        ys.append(w.y)
        zs.append(w.z)
    if max(map(abs, xs)) > LIM_X or max(map(abs, ys)) > LIM_Y:
        hors.append({
            "objet": obj.name,
            "collections": [c.name for c in obj.users_collection],
            "verts": len(obj.data.vertices),
            "x": [round(min(xs), 1), round(max(xs), 1)],
            "y": [round(min(ys), 1), round(max(ys), 1)],
            "z": [round(min(zs), 1), round(max(zs), 1)],
        })

par_collection = {}
for h in hors:
    cle = ",".join(h["collections"]) or "(aucune)"
    par_collection[cle] = par_collection.get(cle, 0) + 1

print("RESULT: " + json.dumps({
    "objets_scene": len(bpy.data.objects),
    "hors_emprise": len(hors),
    "par_collection": par_collection,
    "collections": [c.name for c in bpy.data.collections],
    "echantillon": hors[:12],
}, ensure_ascii=False))
