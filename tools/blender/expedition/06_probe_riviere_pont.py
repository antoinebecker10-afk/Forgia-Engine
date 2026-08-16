"""Pourquoi la rivière ne se voit pas, et où est vraiment le pont ?

Deux corrections « à vue » ont déjà échoué sur cette rivière. Cette fois on
demande au moteur ce qu'on VOIT réellement : depuis le ciel, à l'aplomb de
l'axe de la rivière, quel objet le rayon touche-t-il en premier ? Si la réponse
n'est pas `riviere`, elle est cachée — et par quoi.

On vérifie aussi que le pont enjambe bien la brèche, et que le rempart fait
le tour du village.
"""

import json
import math

import bpy
from mathutils import Vector

RAPPORT = {}
SCENE = bpy.context.scene


def premier_touche(x, y):
    dg = bpy.context.evaluated_depsgraph_get()
    ok, pos, _, _, obj, _ = SCENE.ray_cast(dg, Vector((x, y, 400.0)), Vector((0, 0, -1)))
    return (obj.name if obj else None, round(pos.z, 2)) if ok else (None, None)


# --- 1. la rivière est-elle la première chose vue d'en haut ? --------------
eau = bpy.data.objects.get("riviere")
if eau:
    mat = [m.name if m else None for m in eau.data.materials]
    xs = [(eau.matrix_world @ v.co) for v in eau.data.vertices]
    axe = {}
    for i in range(0, len(xs), 2):
        # milieu du ruban = axe de la rivière
        if i + 1 >= len(xs):
            break
        mx = (xs[i].x + xs[i + 1].x) / 2.0
        my = (xs[i].y + xs[i + 1].y) / 2.0
        nom, z = premier_touche(mx, my)
        axe.setdefault(nom or "(rien)", 0)
        axe[nom or "(rien)"] += 1
    RAPPORT["riviere"] = {
        "materiaux": mat,
        "largeur_m": round((xs[0] - xs[1]).length, 2),
        "z_min": round(min(v.z for v in xs), 2),
        "z_max": round(max(v.z for v in xs), 2),
        "vu_den_haut": axe,
    }
else:
    RAPPORT["riviere"] = {"erreur": "objet riviere absent"}

# --- 2. le pont enjambe-t-il la brèche ? ----------------------------------
pont = bpy.data.objects.get("pont")
if pont and eau:
    lo = [1e9] * 3
    hi = [-1e9] * 3
    for c in pont.bound_box:
        w = pont.matrix_world @ Vector(c)
        for a in range(3):
            lo[a] = min(lo[a], w[a])
            hi[a] = max(hi[a], w[a])
    cx, cy = (lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2
    # Distance du centre du pont à l'axe de l'eau le plus proche
    meilleure = 1e9
    for v in eau.data.vertices:
        w = eau.matrix_world @ v.co
        meilleure = min(meilleure, math.hypot(w.x - cx, w.y - cy))
    RAPPORT["pont"] = {
        "centre_xy": [round(cx, 2), round(cy, 2)],
        "emprise": [round(hi[i] - lo[i], 2) for i in range(3)],
        "z": [round(lo[2], 2), round(hi[2], 2)],
        "distance_a_l_eau_m": round(meilleure, 2),
        "materiaux": [m.name if m else None for m in pont.data.materials],
    }
else:
    RAPPORT["pont"] = {"erreur": f"pont={bool(pont)}"}

# --- 3. le rempart fait-il le tour ? --------------------------------------
murs = [o for o in bpy.data.objects
        if o.type == "MESH" and "wall_" in o.name.lower() and not o.hide_render]
village = bpy.data.objects.get("building_well_red.001")
centre = (village.location.x, village.location.y) if village else (90.0, 0.0)
angles = []
for m in murs:
    d = math.hypot(m.location.x - centre[0], m.location.y - centre[1])
    if d < 5.0:
        continue
    angles.append(round(math.degrees(math.atan2(m.location.y - centre[1],
                                                m.location.x - centre[0])) % 360.0, 1))
angles.sort()
trous = []
if angles:
    for i in range(len(angles)):
        a, b = angles[i], angles[(i + 1) % len(angles)]
        ecart = (b - a) % 360.0
        if ecart > 25.0:
            trous.append({"de": a, "a": b, "ecart_deg": round(ecart, 1)})
RAPPORT["rempart"] = {
    "pieces": len(murs),
    "centre_village": [round(centre[0], 1), round(centre[1], 1)],
    "angles_deg": angles,
    "trous_sup_25deg": trous,
    "couverture_deg": round(360.0 - sum(t["ecart_deg"] for t in trous), 1),
}

# --- 4. objets qui s'interpénètrent dans le village ------------------------
# Deux boîtes englobantes qui se recouvrent de plus de 35 % = un élément
# planté dans un autre. C'est ce qu'on voit sur la capture.
def boite(o):
    lo = [1e9] * 3
    hi = [-1e9] * 3
    for c in o.bound_box:
        w = o.matrix_world @ Vector(c)
        for a in range(3):
            lo[a] = min(lo[a], w[a])
            hi[a] = max(hi[a], w[a])
    return lo, hi


batis = [o for o in bpy.data.objects
         if o.type == "MESH" and not o.hide_render
         and ("building" in o.name.lower() or "wall_" in o.name.lower())
         and math.hypot(o.location.x - centre[0], o.location.y - centre[1]) < 60.0]
chocs = []
for i in range(len(batis)):
    lo1, hi1 = boite(batis[i])
    for j in range(i + 1, len(batis)):
        lo2, hi2 = boite(batis[j])
        rec = 1.0
        for a in range(2):                      # recouvrement en plan
            inter = min(hi1[a], hi2[a]) - max(lo1[a], lo2[a])
            if inter <= 0:
                rec = 0.0
                break
            rec *= inter / max(1e-6, min(hi1[a] - lo1[a], hi2[a] - lo2[a]))
        if rec > 0.35:
            chocs.append({"a": batis[i].name, "b": batis[j].name, "recouvrement": round(rec, 2)})
RAPPORT["interpenetrations_village"] = {"total": len(chocs), "echantillon": chocs[:10]}

print("RESULT: " + json.dumps(RAPPORT, ensure_ascii=False))
