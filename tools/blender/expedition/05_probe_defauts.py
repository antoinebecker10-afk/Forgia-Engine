"""Mesure les défauts signalés, au lieu de les corriger à vue.

La rivière a déjà résisté à une correction faite d'après une capture : elle
avait été « réparée » sans qu'on ait jamais mesuré la hauteur de sa nappe
contre celle de son lit. On mesure donc, cette fois, et on cite des nombres.

Cinq questions, cinq réponses chiffrées :
  1. la nappe d'eau est-elle AU-DESSUS du terrain, et de combien ?
  2. le sol obstrue-t-il la porte du rempart, et de combien ?
  3. combien d'objets empiètent sur le corridor du chemin ?
  4. les UV du terrain sont-ils étirés (roche projetée) ?
  5. quelle est la densité réelle de props, par zone ?
"""

import json
import math

import bpy
from mathutils import Vector

RAPPORT = {}
LIM = 0.001


def hauteur_sol(x, y):
    """Altitude DU TERRAIN, mesuree sur le mesh terrain lui-meme.

    Un lancer de rayon sur la scene entiere frappe le premier objet venu — la
    porte, un arbre — et rend sa hauteur pour celle du sol. C'est ainsi que
    cette sonde a annonce « le sol est 6,9 m au-dessus du seuil » alors qu'elle
    mesurait le linteau de la porte. Un instrument qui vise a cote produit un
    faux defaut, et on corrige alors ce qui n'etait pas casse.
    """
    t = bpy.data.objects.get("terrain")
    if t is None:
        return None, None
    inv = t.matrix_world.inverted()
    origine = inv @ Vector((x, y, 400.0))
    direction = (inv.to_3x3() @ Vector((0.0, 0.0, -1.0))).normalized()
    ok, pos, _, _ = t.ray_cast(origine, direction)
    return ((t.matrix_world @ pos).z, "terrain") if ok else (None, None)


# --- 1. la rivière ---------------------------------------------------------
eau = bpy.data.objects.get("riviere")
terrain = bpy.data.objects.get("terrain")
if eau and terrain:
    echantillons = []
    enfouis = 0
    # Les collections Blender ne se tranchent pas avec un pas (`[::7]` lève une
    # TypeError) : on filtre à l'index.
    for i, v in enumerate(eau.data.vertices):
        if i % 7:
            continue
        w = eau.matrix_world @ v.co
        # Hauteur du terrain SOUS ce sommet d'eau, calculée sur le mesh terrain
        # uniquement (le ray_cast global toucherait un rocher posé au-dessus).
        proche = None
        meilleure = 1e9
        for tv in terrain.data.vertices:
            d = (tv.co.x - w.x) ** 2 + (tv.co.y - w.y) ** 2
            if d < meilleure:
                meilleure, proche = d, tv.co.z
        delta = w.z - proche
        echantillons.append(round(delta, 3))
        if delta < 0.0:
            enfouis += 1
    RAPPORT["riviere"] = {
        "sommets_testes": len(echantillons),
        "sous_le_terrain": enfouis,
        "delta_min": min(echantillons),
        "delta_max": max(echantillons),
        "delta_median": sorted(echantillons)[len(echantillons) // 2],
        "verdict": "ENFOUIE" if enfouis > len(echantillons) * 0.3 else "visible",
    }
else:
    RAPPORT["riviere"] = {"erreur": f"eau={bool(eau)} terrain={bool(terrain)}"}

# --- 2. la porte du rempart ------------------------------------------------
portes = [o for o in bpy.data.objects if "gate" in o.name.lower()]
detail = []
for porte in portes:
    lo = [1e9] * 3
    hi = [-1e9] * 3
    for c in porte.bound_box:
        w = porte.matrix_world @ Vector(c)
        for a in range(3):
            lo[a] = min(lo[a], w[a])
            hi[a] = max(hi[a], w[a])
    cx, cy = (lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0
    sol, quoi = hauteur_sol(cx, cy)
    detail.append({
        "objet": porte.name,
        "centre_xy": [round(cx, 2), round(cy, 2)],
        "seuil_z": round(lo[2], 2),
        "sommet_z": round(hi[2], 2),
        "sol_z": round(sol, 2) if sol is not None else None,
        "touche": quoi,
        # >0 : le sol monte AU-DESSUS du seuil de la porte → il l'obstrue.
        "sol_au_dessus_du_seuil_m": round(sol - lo[2], 2) if sol is not None else None,
    })
RAPPORT["portes"] = detail

# --- 3. objets dans le corridor du chemin ---------------------------------
chemin = bpy.data.objects.get("chemin")
axe = []
if chemin:
    for v in chemin.data.vertices:
        w = chemin.matrix_world @ v.co
        axe.append((w.x, w.y))


def distance_axe(x, y):
    meilleure = 1e9
    for i in range(0, len(axe) - 2, 2):
        ax, ay = axe[i]
        bx, by = axe[i + 2]
        dx, dy = bx - ax, by - ay
        l2 = dx * dx + dy * dy
        if l2 < 1e-9:
            continue
        t = max(0.0, min(1.0, ((x - ax) * dx + (y - ay) * dy) / l2))
        d = math.hypot(x - (ax + t * dx), y - (ay + t * dy))
        meilleure = min(meilleure, d)
    return meilleure


IGNORE = {"terrain", "chemin", "riviere", "pont", "VueCam", "SoleilVue"}
intrus = []
if axe:
    for obj in bpy.data.objects:
        if obj.type != "MESH" or obj.name in IGNORE:
            continue
        if obj.hide_render:
            continue
        d = distance_axe(obj.location.x, obj.location.y)
        if d < 2.6:                      # demi-largeur du chemin = 2,2 m
            intrus.append({"objet": obj.name, "d": round(d, 2),
                           "xy": [round(obj.location.x, 1), round(obj.location.y, 1)]})
RAPPORT["dans_le_chemin"] = {
    "total": len(intrus),
    "par_type": {},
    "echantillon": intrus[:15],
}
for it in intrus:
    base = it["objet"].split(".")[0]
    RAPPORT["dans_le_chemin"]["par_type"][base] = \
        RAPPORT["dans_le_chemin"]["par_type"].get(base, 0) + 1

# --- 4. étirement des UV du terrain ---------------------------------------
if terrain:
    uv = terrain.data.uv_layers.active
    par_materiau = {}
    for poly in terrain.data.polygons:
        idx = poly.material_index
        # Longueur d'arête en mètres vs en UV : leur rapport dit la densité.
        boucles = list(poly.loop_indices)
        meilleur = None
        for k in range(len(boucles)):
            i0, i1 = boucles[k], boucles[(k + 1) % len(boucles)]
            a = terrain.data.vertices[terrain.data.loops[i0].vertex_index].co
            b = terrain.data.vertices[terrain.data.loops[i1].vertex_index].co
            duv = (Vector(uv.data[i1].uv) - Vector(uv.data[i0].uv)).length
            metres = (b - a).length
            if metres > LIM and duv > LIM and (meilleur is None or metres > meilleur[0]):
                meilleur = (metres, metres / duv)
        if meilleur:
            par_materiau.setdefault(idx, []).append(meilleur[1])
    RAPPORT["uv_terrain_metres_par_tuile"] = {
        str(k): {"min": round(min(v), 2), "median": round(sorted(v)[len(v) // 2], 2),
                 "max": round(max(v), 2), "faces": len(v)}
        for k, v in par_materiau.items()
    }
    RAPPORT["materiaux_terrain"] = [m.name if m else None for m in terrain.data.materials]

# --- 5. densité de props ---------------------------------------------------
compte = {}
for coll in ("foret", "campements", "falaises", "village", "reperes"):
    c = bpy.data.collections.get(coll)
    compte[coll] = len(c.objects) if c else 0
# L'emprise a change (280 x 200) : une densite rapportee a l'ancienne
# surface serait fausse de 46 %.
aire = 280.0 * 200.0
compte["props_par_1000m2"] = round(sum(compte.values()) / aire * 1000.0, 1)
RAPPORT["densite"] = compte

print("RESULT: " + json.dumps(RAPPORT, ensure_ascii=False))
