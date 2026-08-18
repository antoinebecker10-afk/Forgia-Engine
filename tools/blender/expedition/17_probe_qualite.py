"""Mesure les défauts de POSE et de COMPOSITION. Ne corrige rien.

À lancer sur la scène que `vallon.py` vient de bâtir.

Sept questions, sept réponses chiffrées — chacune correspond à un défaut nommé
et à rien d'autre :

  1. quels props FLOTTENT au-dessus du sol, et de combien ?
  2. lesquels sont ENTERRÉS ?
  3. lesquels sont posés sur une pente où rien ne tiendrait ?
  4. lesquels s'INTERPÉNÈTRENT ?
  5. la route : ses bords sont-ils nets, et que trouve-t-on à sa lisière ?
  6. la rivière : quelle est la longueur de rivage, et l'eau touche-t-elle
     vraiment ses berges ?
  7. le village : ses bâtiments se regardent-ils, ou sont-ils semés ?

Le sol se mesure SUR LE MAILLAGE TERRAIN, jamais par un lancer de rayon global :
celui-ci frappe le premier objet venu — une porte, un arbre — et rend sa hauteur
pour celle du sol. C'est ainsi qu'une sonde de ce projet a annoncé « le sol est
6,9 m au-dessus du seuil » alors qu'elle mesurait un linteau.
"""

import json
import math

import bpy
from mathutils import Vector
from mathutils.kdtree import KDTree

# Collections qui ne sont pas du décor posé : instruments et pièces sources.
IGNORE_COLL = {"collisions", "faune_controle", "faune_apercu", "_proto", "_src"}
# Objets qui ne se posent pas sur le sol : ils SONT le sol, ou l'eau.
IGNORE_NOM = {"terrain", "chemin", "riviere", "pont", "VueCam", "SoleilVue"}

# Seuils. Ce ne sont pas des goûts :
#   - 0,12 m de vide sous une pièce se VOIT à hauteur d'œil (ombre décollée) ;
#   - au-delà de 34° on cesse de planter (c'est déjà la borne du semis) ;
#   - deux props dont les emprises se recouvrent de plus de moitié forment une
#     masse illisible, pas deux objets.
FLOTTE_M = 0.12
ENTERRE_M = -0.60
PENTE_MAX_DEG = 34.0
RECOUVREMENT = 0.5


def terrain_obj():
    return bpy.data.objects.get("terrain")


def main():
    t = terrain_obj()
    if t is None:
        print("RESULT: " + json.dumps({"erreur": "terrain absent"}))
        return

    inv = t.matrix_world.inverted()
    direction = (inv.to_3x3() @ Vector((0.0, 0.0, -1.0))).normalized()

    def sol(x, y):
        """Altitude du terrain sous (x, y), mesurée sur LE TERRAIN."""
        ok, pos, _, _ = t.ray_cast(inv @ Vector((x, y, 500.0)), direction)
        return (t.matrix_world @ pos).z if ok else None

    def pente(x, y, pas=1.2):
        h = sol(x, y)
        if h is None:
            return None
        a, b = sol(x + pas, y), sol(x, y + pas)
        if a is None or b is None:
            return None
        return math.degrees(math.atan(math.hypot(a - h, b - h) / pas))

    # -- recenser le décor posé -------------------------------------------
    props = []
    for obj in bpy.data.objects:
        if obj.type != "MESH" or obj.name.split(".")[0] in IGNORE_NOM:
            continue
        if any(c.name in IGNORE_COLL for c in obj.users_collection):
            continue
        coins = [obj.matrix_world @ Vector(c) for c in obj.bound_box]
        bas = min(p.z for p in coins)
        cx = sum(p.x for p in coins) / 8.0
        cy = sum(p.y for p in coins) / 8.0
        rayon = max(math.hypot(p.x - cx, p.y - cy) for p in coins)
        fam = obj.get("famille") or (
            obj.users_collection[0].name if obj.users_collection else "?")
        props.append({"nom": obj.name, "fam": fam, "x": cx, "y": cy,
                      "bas": bas, "r": rayon})

    # -- 1 à 3 : flottement, enfouissement, pente --------------------------
    flottants, enterres, pentus, hors_sol = [], [], [], 0
    par_famille = {}
    for p in props:
        h = sol(p["x"], p["y"])
        if h is None:
            hors_sol += 1
            continue
        d = p["bas"] - h
        p["ecart"] = d
        stat = par_famille.setdefault(p["fam"], {"n": 0, "somme": 0.0,
                                                 "flottent": 0, "enterres": 0})
        stat["n"] += 1
        stat["somme"] += d
        if d > FLOTTE_M:
            stat["flottent"] += 1
            flottants.append((p["nom"], p["fam"], round(d, 2)))
        elif d < ENTERRE_M:
            stat["enterres"] += 1
            enterres.append((p["nom"], p["fam"], round(d, 2)))
        s = pente(p["x"], p["y"])
        if s is not None and s > PENTE_MAX_DEG:
            pentus.append((p["nom"], p["fam"], round(s, 1)))

    flottants.sort(key=lambda r: -r[2])
    enterres.sort(key=lambda r: r[2])
    pentus.sort(key=lambda r: -r[2])

    # -- 4 : interpénétrations --------------------------------------------
    # Deux props dont les centres sont plus proches que la moitié de la somme
    # de leurs rayons forment une masse, pas deux objets.
    kd = KDTree(len(props))
    for i, p in enumerate(props):
        kd.insert(Vector((p["x"], p["y"], 0.0)), i)
    kd.balance()
    paires = set()
    for i, p in enumerate(props):
        for _co, j, d in kd.find_range(Vector((p["x"], p["y"], 0.0)), p["r"] * 2.0):
            if j <= i:
                continue
            q = props[j]
            seuil = (p["r"] + q["r"]) * RECOUVREMENT
            if d < seuil:
                paires.add((p["fam"], q["fam"]))
    # On compte, on ne liste pas : la liste ferait des milliers de lignes.
    chevauchements = {}
    for i, p in enumerate(props):
        n = sum(1 for _co, j, d in kd.find_range(Vector((p["x"], p["y"], 0.0)),
                                                 p["r"] * 2.0)
                if j != i and d < (p["r"] + props[j]["r"]) * RECOUVREMENT)
        if n:
            chevauchements[p["fam"]] = chevauchements.get(p["fam"], 0) + 1

    # -- 5 : la lisière de la route ---------------------------------------
    chemin = bpy.data.objects.get("chemin")
    lisiere = {}
    if chemin is not None:
        axe = [(chemin.matrix_world @ v.co).xy for v in chemin.data.vertices]
        kd_axe = KDTree(len(axe))
        for i, a in enumerate(axe):
            kd_axe.insert(Vector((a.x, a.y, 0.0)), i)
        kd_axe.balance()

        def d_axe(x, y):
            _co, _i, d = kd_axe.find(Vector((x, y, 0.0)))
            return d

        bandes = {"0-3 m": 0, "3-5 m": 0, "5-8 m": 0, "8-14 m": 0}
        for p in props:
            d = d_axe(p["x"], p["y"])
            if d < 3.0:
                bandes["0-3 m"] += 1
            elif d < 5.0:
                bandes["3-5 m"] += 1
            elif d < 8.0:
                bandes["5-8 m"] += 1
            elif d < 14.0:
                bandes["8-14 m"] += 1
        lisiere = {
            "props_par_bande": bandes,
            "sommets_du_ruban": len(chemin.data.vertices),
            "materiaux": [m.name if m else None for m in chemin.data.materials],
        }

    # -- 6 : la rivière ----------------------------------------------------
    eau = bpy.data.objects.get("riviere")
    riviere = {}
    if eau is not None:
        sommets = [eau.matrix_world @ v.co for v in eau.data.vertices]
        # Tirant d'eau : de combien la nappe domine le terrain, sommet par
        # sommet. Négatif = la nappe est SOUS le sol, donc invisible.
        ecarts = []
        for i, w in enumerate(sommets):
            if i % 3:
                continue
            h = sol(w.x, w.y)
            if h is not None:
                ecarts.append(w.z - h)
        ecarts.sort()
        riviere = {
            "sommets": len(sommets),
            "materiaux": [m.name if m else None for m in eau.data.materials],
            "tirant_min": round(ecarts[0], 2) if ecarts else None,
            "tirant_median": round(ecarts[len(ecarts) // 2], 2) if ecarts else None,
            "tirant_max": round(ecarts[-1], 2) if ecarts else None,
            "sous_le_sol": sum(1 for e in ecarts if e < 0.0),
            "testes": len(ecarts),
        }

    # -- 7 : le village ----------------------------------------------------
    batis = [p for p in props
             if "building" in p["nom"].lower() or "home" in p["nom"].lower()
             or "church" in p["nom"].lower() or "tavern" in p["nom"].lower()
             or "market" in p["nom"].lower() or "well" in p["nom"].lower()]
    village = {"batiments": len(batis)}
    if len(batis) > 1:
        d = []
        for i, a in enumerate(batis):
            for b in batis[i + 1:]:
                d.append(math.hypot(a["x"] - b["x"], a["y"] - b["y"]))
        d.sort()
        village.update({
            "ecart_min_m": round(d[0], 2),
            "ecart_median_m": round(d[len(d) // 2], 2),
            "ecart_max_m": round(d[-1], 2),
            "noms": sorted({b["nom"].split(".")[0] for b in batis}),
        })

    rapport = {
        "props_mesures": len(props),
        "hors_terrain": hors_sol,
        "ecart_au_sol_par_famille": {
            f: {"n": s["n"], "moyen_m": round(s["somme"] / s["n"], 3),
                "flottent": s["flottent"], "enterres": s["enterres"]}
            for f, s in sorted(par_famille.items())
        },
        "flottants": {"total": len(flottants), "pires": flottants[:12]},
        "enterres": {"total": len(enterres), "pires": enterres[:12]},
        "sur_pente_forte": {"total": len(pentus), "pires": pentus[:12]},
        "chevauchements_par_famille": dict(sorted(chevauchements.items())),
        "lisiere_route": lisiere,
        "riviere": riviere,
        "village": village,
    }
    print("RESULT: " + json.dumps(rapport, ensure_ascii=False))


main()
