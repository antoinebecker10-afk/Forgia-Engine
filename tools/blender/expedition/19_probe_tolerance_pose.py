"""D'OÙ VIENNENT LES 19 CENTIMÈTRES ? L'expérience qui tranche.

Deux causes ont été avancées pour les 4 272 props qui flottent, et elles ne
peuvent pas être vraies toutes les deux :

  HYPOTHÈSE A — « `offset_base = 0.05` est un décalage parasite ; ×4 il lève de
    0,20 m ». C'est ce que j'ai affirmé.
  HYPOTHÈSE B — « `offset_base` est la COMPENSATION EXACTE du `z_min = −0,05`
    que porte 91 % du kit ; il pose donc le sommet le plus bas pile au sol, et
    l'écart vient d'ailleurs — de l'écart entre le CHAMP analytique (sur lequel
    on pose) et le MAILLAGE triangulé au pas de 1,6 m (qu'on mesure) ».

# L'expérience qui les sépare, et pourquoi elle est décisive

Les deux hypothèses prédisent le même écart MOYEN. Elles prédisent des choses
opposées sur sa DISTRIBUTION :

  - Sous A, l'écart vaut `0,05 × échelle` : il ne dépend QUE de l'échelle de la
    pièce, et pas du tout de l'endroit où elle est posée.
  - Sous B, l'écart est la flèche de l'interpolation du maillage : il vaut ZÉRO
    sur un nœud de la grille (là, maillage = champ, exactement) et il est MAXIMAL
    au centre d'une cellule. Il dépend donc de la POSITION dans la cellule, et
    pas de l'échelle.

On mesure donc l'écart en fonction des deux, séparément. Une seule des deux
corrélations peut survivre.

C'est la troisième fois aujourd'hui qu'on mesure avant de corriger, et les deux
premières ont chacune tué un correctif faux (les rochers de ceinture, la
rivière). On ne touche à `Kit.poser` qu'après ce verdict.
"""

import json
import math

import bpy
from mathutils import Vector

# Le pas de la grille du terrain, et son origine. Ils ne se devinent pas : ils
# se relisent dans la SPEC qui a bâti le maillage (`pas_terrain`, `demi_x`).
PAS = 1.6
DEMI_X = 140.0
DEMI_Y = 100.0

FAMILLES = ("arbre", "rocher", "abri", "camp", "repere", "brasero", "eboulis")


def main():
    t = bpy.data.objects.get("terrain")
    if t is None:
        print("RESULT: " + json.dumps({"erreur": "terrain absent"}))
        return
    inv = t.matrix_world.inverted()
    dirn = (inv.to_3x3() @ Vector((0.0, 0.0, -1.0))).normalized()

    def sol(x, y):
        ok, pos, _, _ = t.ray_cast(inv @ Vector((x, y, 500.0)), dirn)
        return (t.matrix_world @ pos).z if ok else None

    # -- 1. VÉRIFIER LE FAIT SUR LEQUEL TOUT REPOSE ------------------------
    # `z_min = −0,05` sur 91 % du kit : c'est l'affirmation qui falsifie
    # l'hypothèse A. On la contrôle sur les maillages RÉELLEMENT posés, pas sur
    # un catalogue — un catalogue décrit ce qui est sur le disque, pas ce que
    # l'importateur a produit.
    zmins = {}
    for obj in bpy.data.objects:
        f = obj.get("famille")
        if not f or obj.type != "MESH":
            continue
        base = obj.data.name.split(".")[0]
        if base in zmins:
            continue
        zs = [v.co.z for v in obj.data.vertices]
        if zs:
            zmins[base] = round(min(zs), 4)
    distrib = {}
    for z in zmins.values():
        distrib[str(z)] = distrib.get(str(z), 0) + 1

    # -- 2. L'EXPÉRIENCE ---------------------------------------------------
    # Pour chaque prop : son écart au sol, son échelle, et sa position DANS la
    # cellule de grille. Puis on croise.
    obs = []
    for obj in bpy.data.objects:
        f = obj.get("famille")
        if f not in FAMILLES or obj.type != "MESH":
            continue
        s = abs(obj.scale.x)
        zs = [v.co.z for v in obj.data.vertices]
        if not zs:
            continue
        # Le point le plus bas de la pièce, en monde. On le calcule sur les
        # sommets et non sur la boîte englobante : la boîte d'une pièce inclinée
        # descend plus bas que sa géométrie, et c'est ce qui a produit le faux
        # « −10 m » des rochers de ceinture ce matin.
        mw = obj.matrix_world
        bas = min((mw @ v.co).z for v in obj.data.vertices)
        h = sol(obj.location.x, obj.location.y)
        if h is None:
            continue
        # Position dans la cellule : 0 = sur un nœud de la grille, 0,5 = au
        # centre d'une arête, ~0,71 = au centre de la cellule (en diagonale).
        u = ((obj.location.x + DEMI_X) / PAS) % 1.0
        v = ((obj.location.y + DEMI_Y) / PAS) % 1.0
        du = min(u, 1.0 - u)
        dv = min(v, 1.0 - v)
        obs.append({
            "fam": f,
            "ecart": bas - h,                       # ce qu'on cherche à expliquer
            "echelle": s,
            "predit_A": 0.05 * s,                   # hypothèse A
            "cellule": math.hypot(du, dv),          # hypothèse B (0 = nœud)
            "zmin_local": min(zs),
        })

    if not obs:
        print("RESULT: " + json.dumps({"erreur": "aucun prop etiquete"}))
        return

    def correlation(xs, ys):
        """Pearson. Une corrélation nulle FALSIFIE l'hypothèse ; une corrélation
        forte ne la prouve pas seule, mais ici les deux prédicteurs sont
        indépendants, donc la comparaison tranche."""
        n = len(xs)
        mx, my = sum(xs) / n, sum(ys) / n
        num = sum((a - mx) * (b - my) for a, b in zip(xs, ys))
        dx = math.sqrt(sum((a - mx) ** 2 for a in xs))
        dy = math.sqrt(sum((b - my) ** 2 for b in ys))
        return num / (dx * dy) if dx > 1e-12 and dy > 1e-12 else 0.0

    ecarts = [o["ecart"] for o in obs]
    r_echelle = correlation([o["echelle"] for o in obs], ecarts)
    r_cellule = correlation([o["cellule"] for o in obs], ecarts)

    # Le test le plus lisible : l'écart moyen par tranche de position dans la
    # cellule. Sous B il doit MONTER du nœud vers le centre ; sous A il doit
    # être plat.
    tranches = {}
    for o in obs:
        k = round(o["cellule"] * 10) / 10.0
        tranches.setdefault(k, []).append(o["ecart"])
    par_cellule = {str(k): {"n": len(v), "moyen": round(sum(v) / len(v), 3)}
                   for k, v in sorted(tranches.items())}

    # Et le même par tranche d'échelle.
    tranches_e = {}
    for o in obs:
        k = round(o["echelle"])
        tranches_e.setdefault(k, []).append(o["ecart"])
    par_echelle = {str(k): {"n": len(v), "moyen": round(sum(v) / len(v), 3)}
                   for k, v in sorted(tranches_e.items())}

    # -- 3. LE RÉSIDU ------------------------------------------------------
    # Ce que l'hypothèse A n'explique pas : ecart − 0,05 × echelle − zmin × echelle.
    # Si A était vraie ET que zmin valait 0, le résidu serait nul.
    residus = [o["ecart"] - (0.05 + o["zmin_local"]) * o["echelle"] for o in obs]
    residus.sort()

    rapport = {
        "props": len(obs),
        "zmin_des_prototypes": dict(sorted(distrib.items(),
                                           key=lambda kv: -kv[1])),
        "ecart_mesure": {
            "moyen": round(sum(ecarts) / len(ecarts), 3),
            "median": round(sorted(ecarts)[len(ecarts) // 2], 3),
            "min": round(min(ecarts), 3), "max": round(max(ecarts), 3),
        },
        "correlation_avec_echelle_hypA": round(r_echelle, 3),
        "correlation_avec_position_cellule_hypB": round(r_cellule, 3),
        "ecart_par_position_dans_cellule": par_cellule,
        "ecart_par_echelle": par_echelle,
        # Ce que la compensation zmin explique déjà, à elle seule.
        "residu_apres_compensation_zmin": {
            "moyen": round(sum(residus) / len(residus), 3),
            "median": round(residus[len(residus) // 2], 3),
            "min": round(residus[0], 3), "max": round(residus[-1], 3),
        },
    }
    print("RESULT: " + json.dumps(rapport, ensure_ascii=False))


main()
