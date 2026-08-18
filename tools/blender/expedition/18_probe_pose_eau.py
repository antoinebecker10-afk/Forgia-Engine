"""Lève les deux doutes de la sonde 17, et relève ce qu'il faut pour décider.

La sonde 17 a rendu deux nombres que je REFUSE d'exploiter tels quels :

  - `bouchon` enfoui de -10,3 m en moyenne. Mesuré au CENTRE DE LA BOÎTE
    ENGLOBANTE, sur une paroi à 70° : sur une telle pente, le centre de la boîte
    est à plusieurs mètres horizontalement de l'origine de la pièce, donc le
    terrain qu'on y mesure n'est pas celui sur lequel elle repose. C'est
    exactement le défaut d'instrument déjà payé sur cette carte (une sonde qui
    visait un linteau et annonçait un sol à 6,9 m).

  - Rivière : 56 sommets sur 65 « sous le terrain », alors que le diagnostic de
    `vallon.py` annonce 0 station enterrée sur 97. Les deux ne mesurent pas la
    même chose : lui compare la nappe au champ ANALYTIQUE, moi au MAILLAGE
    triangulé au pas de 1,6 m. Et j'échantillonnais aussi les bords du ruban,
    qui débordent sur la berge — un bord de nappe sous la berge est normal.

On ne corrige rien tant qu'on ne sait pas lequel des deux instruments ment.
"""

import json
import math

import bpy
from mathutils import Vector

SEUIL_BORD = 0.92     # fraction de la demi-largeur au-delà de laquelle on est « au bord »


def main():
    t = bpy.data.objects.get("terrain")
    if t is None:
        print("RESULT: " + json.dumps({"erreur": "terrain absent"}))
        return
    inv = t.matrix_world.inverted()
    direction = (inv.to_3x3() @ Vector((0.0, 0.0, -1.0))).normalized()

    def sol(x, y):
        ok, pos, _, _ = t.ray_cast(inv @ Vector((x, y, 500.0)), direction)
        return (t.matrix_world @ pos).z if ok else None

    rapport = {}

    # ---- DOUTE 1 : les rochers de ceinture ------------------------------
    # Trois mesures pour la même pièce, et l'écart entre elles EST le
    # diagnostic :
    #   a) sous l'ORIGINE (là où le script l'a posée) ;
    #   b) sous le CENTRE de la boîte (ce que faisait la sonde 17) ;
    #   c) le plus petit écart sous N'IMPORTE lequel de ses sommets — c'est
    #      celui-là qui dit si la pièce touche vraiment le sol quelque part.
    bouchons = []
    for obj in bpy.data.objects:
        if obj.get("famille") != "bouchon":
            continue
        coins = [obj.matrix_world @ Vector(c) for c in obj.bound_box]
        cx = sum(p.x for p in coins) / 8.0
        cy = sum(p.y for p in coins) / 8.0
        bas = min(p.z for p in coins)
        s_origine = sol(obj.location.x, obj.location.y)
        s_centre = sol(cx, cy)
        # Le contact réel : sur tous les sommets du maillage, le plus petit
        # écart entre le sommet et le terrain juste dessous.
        contact = None
        for i, v in enumerate(obj.data.vertices):
            if i % 4:
                continue
            w = obj.matrix_world @ v.co
            h = sol(w.x, w.y)
            if h is None:
                continue
            e = w.z - h
            contact = e if contact is None else min(contact, e)
        bouchons.append({
            "nom": obj.name,
            "origine_xy": [round(obj.location.x, 1), round(obj.location.y, 1)],
            "decalage_centre_m": round(math.hypot(cx - obj.location.x,
                                                  cy - obj.location.y), 2),
            "sous_origine_m": round(obj.location.z - s_origine, 2) if s_origine else None,
            "bas_vs_centre_m": round(bas - s_centre, 2) if s_centre else None,
            "contact_min_m": round(contact, 2) if contact is not None else None,
        })
    rapport["bouchons"] = {
        "n": len(bouchons),
        "decalage_centre_median_m": round(
            sorted(b["decalage_centre_m"] for b in bouchons)[len(bouchons) // 2], 2)
        if bouchons else None,
        "detail": bouchons[:8],
    }

    # ---- DOUTE 2 : la rivière -------------------------------------------
    # On sépare le CŒUR du ruban de ses BORDS. Un bord de nappe qui passe sous
    # la berge est normal — c'est même ce qui fait le rivage. Un cœur enterré,
    # non : c'est une rivière invisible.
    eau = bpy.data.objects.get("riviere")
    if eau is not None:
        # Le ruban est bâti par bandes : on retrouve l'axe en projetant chaque
        # sommet sur la polyligne des centres. Plus simple et sûr : la distance
        # au sommet le plus proche de l'autre rive donne la largeur locale.
        sommets = [eau.matrix_world @ v.co for v in eau.data.vertices]
        # Regrouper par tranche de V (le long du courant) via l'UV si dispo,
        # sinon par proximité en Y — la rivière court globalement selon Y.
        coeur, bords = [], []
        uv = eau.data.uv_layers.active
        largeur_u = {}
        if uv is not None:
            # U = largeur (0..1) : c'est EXACTEMENT ce qu'il faut pour
            # distinguer le coeur du bord, et c'est déjà dans la géométrie.
            for poly in eau.data.polygons:
                for li in poly.loop_indices:
                    vi = eau.data.loops[li].vertex_index
                    largeur_u[vi] = uv.data[li].uv[0]
        for i, w in enumerate(sommets):
            h = sol(w.x, w.y)
            if h is None:
                continue
            e = w.z - h
            u = largeur_u.get(i)
            au_bord = (u is not None and abs(u - 0.5) * 2.0 > SEUIL_BORD)
            (bords if au_bord else coeur).append(e)

        def stat(v):
            if not v:
                return None
            v = sorted(v)
            return {"n": len(v), "min": round(v[0], 2),
                    "median": round(v[len(v) // 2], 2), "max": round(v[-1], 2),
                    "sous_le_sol": sum(1 for x in v if x < 0.0)}
        rapport["riviere"] = {
            "u_disponible": uv is not None,
            "coeur": stat(coeur),
            "bords": stat(bords),
        }

        # Et la question qui décide vraiment : VOIT-ON l'eau ? On tire un rayon
        # vertical depuis chaque sommet de coeur : s'il ressort au-dessus du
        # terrain, la nappe est visible d'en haut.
        visibles = caches = 0
        for i, w in enumerate(sommets):
            u = largeur_u.get(i)
            if u is not None and abs(u - 0.5) * 2.0 > SEUIL_BORD:
                continue
            h = sol(w.x, w.y)
            if h is None:
                continue
            if w.z >= h - 0.01:
                visibles += 1
            else:
                caches += 1
        rapport["riviere"]["coeur_visible"] = visibles
        rapport["riviere"]["coeur_cache"] = caches

    # ---- CE QU'IL FAUT POUR DÉCIDER LA SUITE ----------------------------
    # Pas de correctif ici : des RELEVÉS, pour que les décisions qui suivent
    # portent sur des nombres et pas sur des impressions.
    releves = {}

    # La route : largeur réelle du ruban, et sa matière.
    ch = bpy.data.objects.get("chemin")
    if ch is not None:
        pts = [ch.matrix_world @ v.co for v in ch.data.vertices]
        releves["chemin"] = {
            "sommets": len(pts),
            "z_min_max": [round(min(p.z for p in pts), 2),
                          round(max(p.z for p in pts), 2)],
            "materiaux": [m.name if m else None for m in ch.data.materials],
            "a_couleurs": bool(ch.data.color_attributes),
        }

    # Le terrain : ses matériaux, et si ses couleurs de sommet existent.
    releves["terrain"] = {
        "sommets": len(t.data.vertices),
        "materiaux": [m.name if m else None for m in t.data.materials],
        "attributs_couleur": [c.name for c in t.data.color_attributes],
    }

    # Le village : position et cap de chaque bâtiment, pour juger la trame.
    vil = []
    for obj in bpy.data.objects:
        n = obj.name.lower()
        if obj.type == "MESH" and ("building_" in n or "wall_" in n):
            vil.append({
                "nom": obj.name.split(".")[0],
                "xy": [round(obj.location.x, 1), round(obj.location.y, 1)],
                "cap_deg": round(math.degrees(obj.rotation_euler.z), 1),
                "z": round(obj.location.z, 2),
            })
    releves["village"] = {"n": len(vil), "pieces": vil[:40]}

    rapport["releves"] = releves
    print("RESULT: " + json.dumps(rapport, ensure_ascii=False))


main()
