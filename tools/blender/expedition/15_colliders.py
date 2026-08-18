"""Calcule les ENVELOPPES CONVEXES de collision, et les rend VISIBLES.

À lancer APRÈS `vallon.py`. N'exporte rien : il construit une collection
`collisions` qu'on regarde, et il mesure de combien le cylindre mentait.

  python tools/blender/bmcp.py code tools/blender/expedition/vallon.py
  python tools/blender/bmcp.py code tools/blender/expedition/15_colliders.py
  python tools/blender/bmcp.py code tools/blender/expedition/16_vue_colliders.py

# Pourquoi le cylindre ne suffit pas

Un cylindre est le plus mauvais volume englobant possible pour une forme
irrégulière : il prend le rayon du point le plus éloigné de l'axe, et l'applique
sur toute la hauteur. Mesuré sur la carte cuite :

| famille | rayon cylindre | ce que c'est |
|---|---|---|
| `bouchon` | **4,33 à 7,29 m** sur 25,87 m de haut | un rocher de falaise |
| `eboulis` | jusqu'à 3,37 m | un caillou de 4,9 m |
| `abri`    | 1,03 à 3,67 m | la couverture d'une salle de combat |

Sur un abri, cette marge est du gameplay volé : le joueur se cogne à un mètre du
rocher, et le tir qu'il croit rasant part dans du vide solide. Sur les bouchons
de ceinture, ce sont des murs invisibles qui avancent de plusieurs mètres dans
la carte jouable.

# Ce qui reste un cylindre, et pourquoi ce n'est pas une exception

Rien. **Un tronc aussi devient une enveloppe** — de ses sommets en matière
`wood*` seulement, donc du tronc et pas du houppier. Une enveloppe de tronc est
un prisme, c'est-à-dire plus juste qu'un cylindre pour le même prix, et surtout
ça laisse **UNE SEULE forme de collider** à consommer côté moteur. Deux formes
auraient voulu dire deux chemins de code, donc deux occasions de diverger.

# Le partage, qui rend la précision gratuite

Les 1 616 solides partagent une soixantaine de maillages. On calcule donc **une
enveloppe par (maillage, famille)**, pas par instance : le coût est celui de 60
enveloppes, et chaque pose n'emporte qu'une clé, une position, un lacet et une
échelle. C'est exactement le montage qui fait déjà tenir 1 150 arbres en 20
maillages.
"""

import json
import math
import os

import bmesh
import bpy
from mathutils import Vector

RACINE = r"C:\Users\Antoi\Desktop\Forgia Rewrite"

# Nombre maximal de sommets gardés par enveloppe.
#
# Rapier accepte de grosses enveloppes, mais chaque sommet est du travail à
# chaque contact. 24 : au-delà, sur des pièces facettées à 200 faces, on ne
# gagne plus de forme — on gagne du coût. La réduction se fait en fondant les
# arêtes les moins marquées, donc elle enlève du détail, jamais un coin.
MAX_SOMMETS = 24

# Hauteur, en mètres MONDE, sur laquelle se mesure la largeur d'un tronc.
#
# 2,0 m : la capsule du joueur. C'est la seule tranche où sa largeur compte —
# au-dessus, on ne se cogne plus, on passe dessous. Prendre tout le bois
# donnait 1,5 m de rayon sur un bouleau, dont la ramure est en `woodBirch`.
BANDE_TRONC_M = 2.0

# Couleurs de contrôle, par famille. Choisies pour se distinguer sur un décor
# vert et pierre — ce sont des couleurs d'INSTRUMENT, pas de décor.
COULEURS = {
    "arbre":   (0.20, 0.85, 0.35, 1.0),   # vert vif
    "abri":    (1.00, 0.25, 0.20, 1.0),   # rouge — ce sont les meubles du combat
    "rocher":  (0.30, 0.55, 1.00, 1.0),   # bleu
    "eboulis": (0.55, 0.75, 1.00, 1.0),   # bleu pâle
    "bouchon": (1.00, 0.60, 0.10, 1.0),   # orange — la ceinture
    "brasero": (1.00, 0.90, 0.20, 1.0),   # jaune
    "camp":    (0.90, 0.35, 0.85, 1.0),   # magenta
    "repere":  (0.20, 0.90, 0.90, 1.0),   # cyan
}
COLL_NOM = "collisions"


def collection(nom):
    coll = bpy.data.collections.get(nom)
    if coll is None:
        coll = bpy.data.collections.new(nom)
        bpy.context.scene.collection.children.link(coll)
    return coll


def materiau(famille):
    nom = f"collider_{famille}"
    mat = bpy.data.materials.get(nom)
    if mat is not None:
        return mat
    mat = bpy.data.materials.new(nom)
    mat.use_nodes = True
    mat.diffuse_color = COULEURS.get(famille, (1.0, 1.0, 1.0, 1.0))
    bsdf = next((n for n in mat.node_tree.nodes if n.type == "BSDF_PRINCIPLED"), None)
    if bsdf is not None:
        c = COULEURS.get(famille, (1.0, 1.0, 1.0, 1.0))
        bsdf.inputs["Base Color"].default_value = c
        # Émissif : un collider doit se LIRE même dans l'ombre d'un massif.
        # C'est un instrument, pas une matière — il n'a pas à être éclairé.
        if "Emission Color" in bsdf.inputs:
            bsdf.inputs["Emission Color"].default_value = c
            bsdf.inputs["Emission Strength"].default_value = 0.8
        bsdf.inputs["Alpha"].default_value = 0.38
    mat.blend_method = "BLEND"
    return mat


def enveloppe(mesh, matieres, bande_locale=None):
    """Enveloppe convexe d'un maillage, éventuellement restreinte à des matières.

    Rend `(sommets, volume)`, en coordonnées LOCALES et à l'échelle 1. C'est ce
    partage qui rend la précision gratuite : une enveloppe pour toutes les
    instances de la pièce.

    # `bande_locale` — le PRISME de tronc, et pourquoi il ne suffit pas de filtrer

    Restreindre aux matières `wood*` donne le bois, mais sur un bouleau le bois
    c'est le fût **et la ramure** : mesuré, `tree_default_fall` a 308 sommets de
    bois sur 384, et son enveloppe sortait à 1,5 m de rayon monde. On aurait
    donné à chaque bouleau un pilier de trois mètres de large.

    Quand `bande_locale` est donné, on ne garde donc que les sommets de bois
    **sous cette hauteur** — la tranche que le joueur traverse réellement — et on
    EXTRUDE le contour obtenu jusqu'au sommet du bois. Le résultat est un prisme
    à la largeur du fût et à la hauteur du tronc : juste là où on marche, et
    assez haut pour qu'on ne le saute pas.

    C'est aussi ce qui laisse une balle passer dans le houppier, ce qu'on veut :
    on tire à travers des branches, on ne tire pas à travers un tronc.
    """
    indices = None
    if matieres:
        noms = [m.name.split(".")[0] if m else "" for m in mesh.materials]
        gardes = {i for i, n in enumerate(noms)
                  if any(n.startswith(p) for p in matieres)}
        if gardes:
            indices = set()
            for poly in mesh.polygons:
                if poly.material_index in gardes:
                    indices.update(poly.vertices)
            # Une pièce sans matière reconnue (un buisson tout en feuillage)
            # retombe sur sa géométrie entière. Rendre vide en ferait un fantôme.
            if not indices:
                indices = None

    retenus = [v.co for i, v in enumerate(mesh.vertices)
               if indices is None or i in indices]
    if len(retenus) < 4:
        return [], 0.0

    if bande_locale is not None:
        # Prisme : contour pris dans la tranche basse, hauteur prise sur tout le
        # bois. `z_haut` vient des sommets RETENUS (le bois), pas du maillage
        # entier — sinon un prisme de tronc monterait jusqu'à la cime du feuillage.
        z_bas = min(c.z for c in retenus)
        z_haut = max(c.z for c in retenus)
        plafond = z_bas + min(bande_locale, z_haut - z_bas)
        dans_bande = [c for c in retenus if c.z <= plafond]
        # Une pièce dont le bois commence au-dessus de la bande (rare, mais un
        # pin dégagé du sol le ferait) retomberait sur zéro sommet : on garde
        # alors tout le bois plutôt que de rendre un fantôme.
        if len(dans_bande) >= 3:
            retenus = [Vector((c.x, c.y, z_bas)) for c in dans_bande] + \
                      [Vector((c.x, c.y, z_haut)) for c in dans_bande]

    bm = bmesh.new()
    for co in retenus:
        bm.verts.new(co)
    bm.verts.ensure_lookup_table()
    if len(bm.verts) < 4:
        bm.free()
        return [], 0.0

    bmesh.ops.convex_hull(bm, input=bm.verts, use_existing_faces=False)
    # `convex_hull` laisse les sommets intérieurs dans le bmesh : les retirer,
    # sinon on exporte des points qui ne sont sur aucune face et qui ne
    # changent rien à la forme tout en coûtant à chaque contact.
    bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context="VERTS")

    if len(bm.verts) > MAX_SOMMETS:
        # Fondre les arêtes les moins marquées : on retire du détail, jamais un
        # coin. Un angle croissant jusqu'à ce que le compte passe sous la borne.
        for angle_deg in (2.0, 5.0, 10.0, 18.0, 28.0, 40.0):
            if len(bm.verts) <= MAX_SOMMETS:
                break
            bmesh.ops.dissolve_limit(
                bm, angle_limit=math.radians(angle_deg),
                verts=bm.verts[:], edges=bm.edges[:])
            bm.verts.ensure_lookup_table()

    bm.verts.ensure_lookup_table()
    pts = [tuple(round(c, 4) for c in v.co) for v in bm.verts]
    vol = bm.calc_volume(signed=False)
    bm.free()
    return pts, vol


def mesh_depuis_points(nom, pts):
    """Un maillage d'affichage à partir des points de l'enveloppe."""
    bm = bmesh.new()
    for p in pts:
        bm.verts.new(Vector(p))
    bm.verts.ensure_lookup_table()
    bmesh.ops.convex_hull(bm, input=bm.verts, use_existing_faces=False)
    bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context="VERTS")
    me = bpy.data.meshes.new(nom)
    bm.to_mesh(me)
    bm.free()
    return me


def main():
    # Repartir propre : relancer ce script ne doit pas empiler deux jeux de
    # colliders l'un dans l'autre, ce qui doublerait tous les comptes.
    ancienne = bpy.data.collections.get(COLL_NOM)
    if ancienne is not None:
        for obj in list(ancienne.objects):
            bpy.data.objects.remove(obj, do_unlink=True)
        bpy.data.collections.remove(ancienne)
    coll = collection(COLL_NOM)

    # -- 1. recenser les solides, par (maillage, famille) ------------------
    solides = []
    for obj in bpy.data.objects:
        f = obj.get("famille")
        if f and obj.type == "MESH":
            solides.append((obj, f))
    if not solides:
        print("RESULT: " + json.dumps({
            "erreur": "aucun objet etiquete « famille »",
            "remede": "relancer vallon.py — c'est lui qui pose les etiquettes",
        }, ensure_ascii=False))
        return

    # -- 1 bis. l'échelle de référence de chaque forme --------------------
    #
    # 🚨 UNE FORME PARTAGÉE NE PEUT PAS DÉPENDRE D'UNE INSTANCE. La bande de
    # tronc est en mètres MONDE, la géométrie est en LOCAL : il faut diviser par
    # une échelle. Prendre celle de la première instance rencontrée faisait
    # dépendre la forme de l'ordre d'itération — mesuré, `tree_detailed` sortait
    # à 0,77 m de rayon et `tree_detailed_dark`, même géométrie, à 1,27 m.
    #
    # On prend donc la MÉDIANE des échelles réellement posées pour cette pièce :
    # déterministe, et elle suit le kit si son échelle change, ce qu'une
    # constante écrite ici ne ferait pas.
    echelles = {}
    for obj, famille in solides:
        cle = f"{obj.data.name.split('.')[0]}__{famille}"
        echelles.setdefault(cle, []).append(abs(obj.scale.x))
    reference = {c: sorted(v)[len(v) // 2] for c, v in echelles.items()}

    formes = {}          # cle -> {"points": [...], "volume": v, "famille": f}
    rapport_familles = {}
    for obj, famille in solides:
        matieres = tuple(
            m for m in (obj.get("matieres_emprise") or "").split(",") if m)
        cle = f"{obj.data.name.split('.')[0]}__{famille}"
        if cle not in formes:
            # La bande vaut ce que le joueur traverse : sa capsule fait 2,0 m.
            ech = max(reference.get(cle, 1.0), 1e-6)
            bande = (BANDE_TRONC_M / ech) if famille == "arbre" else None
            pts, vol = enveloppe(obj.data, matieres, bande)
            if len(pts) < 4:
                continue
            formes[cle] = {"points": pts, "volume": vol, "famille": famille,
                           "echelle_reference": round(ech, 3)}
        rapport_familles.setdefault(famille, 0)
        rapport_familles[famille] += 1

    # -- 2. mesurer de combien le cylindre mentait -------------------------
    # C'est LE nombre qui justifie ce chantier. Sans lui, « plus precis » est
    # une opinion. Volume du cylindre englobant / volume de l'enveloppe :
    # 1,0 = le cylindre etait deja juste, 4,0 = il englobait quatre fois trop.
    gains = {}
    for cle, forme in formes.items():
        pts = forme["points"]
        zs = [p[2] for p in pts]
        haut = max(zs) - min(zs)
        rayon = max(math.hypot(p[0], p[1]) for p in pts)
        v_cyl = math.pi * rayon * rayon * haut
        v_env = forme["volume"]
        if v_env > 1e-9:
            gains.setdefault(forme["famille"], []).append(v_cyl / v_env)

    # -- 3. construire la VUE ----------------------------------------------
    meshes = {}
    poses = 0
    for obj, famille in solides:
        cle = f"{obj.data.name.split('.')[0]}__{famille}"
        forme = formes.get(cle)
        if forme is None:
            continue
        if cle not in meshes:
            me = mesh_depuis_points(f"col_{cle}", forme["points"])
            me.materials.append(materiau(famille))
            meshes[cle] = me
        vue = bpy.data.objects.new(f"col_{obj.name}", meshes[cle])
        # MEME transform que la piece : l'enveloppe est en local, donc elle se
        # pose exactement dessus. Copier les trois champs plutot que la matrice
        # garde la lecture possible dans l'interface.
        vue.location = obj.location
        vue.rotation_euler = obj.rotation_euler
        vue.scale = obj.scale
        # Clé DIFFÉRENTE de `famille` : étiqueter les proxys comme les props les
        # ferait compter deux fois par toute passe qui recense les solides. Vu
        # une fois — une sonde a relevé 52 « arbres » de plus, qui étaient mes
        # propres colliders.
        vue["famille_collider"] = famille
        coll.objects.link(vue)
        poses += 1

    rapport = {
        "solides": len(solides),
        "formes_uniques": len(formes),
        "poses_affichees": poses,
        "par_famille": dict(sorted(rapport_familles.items())),
        "sommets_par_enveloppe": {
            "min": min(len(f["points"]) for f in formes.values()),
            "median": sorted(len(f["points"]) for f in formes.values())[len(formes) // 2],
            "max": max(len(f["points"]) for f in formes.values()),
        },
        # Facteur de sur-approximation du cylindre, par famille.
        "cylindre_englobait_x_fois_trop": {
            f: round(sum(v) / len(v), 2) for f, v in sorted(gains.items())
        },
    }
    rapport["gain_moyen"] = round(
        sum(sum(v) for v in gains.values()) / max(1, sum(len(v) for v in gains.values())), 2)

    # Le brouillon des formes, pour que la passe d'export n'ait pas à les
    # recalculer — et pour qu'on puisse les relire sans Blender.
    sortie = os.path.join(RACINE, "assets", "models", "environment", "expedition",
                          "vallon_colliders_formes.json")
    with open(sortie, "w", encoding="utf-8") as fh:
        json.dump({c: {"famille": f["famille"], "points": f["points"]}
                   for c, f in sorted(formes.items())}, fh, ensure_ascii=False, indent=1)
    rapport["formes_ecrites"] = os.path.basename(sortie)

    print("RESULT: " + json.dumps(rapport, ensure_ascii=False))


main()
