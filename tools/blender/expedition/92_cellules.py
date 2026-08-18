"""Découpe Le Vallon en cellules streamées, au format que le moteur lit déjà.

À lancer APRÈS `vallon.py`, sur la scène qu'il vient de bâtir. Ce script la
CONSOMME (il fusionne par cellule) : relancer `vallon.py` pour repartir.

Trois leçons héritées du château, déjà payées là-bas :

1. **Jamais de GLB par cellule.** Un GLB ré-embarque chaque texture qu'il
   référence : le château de 48 Mo est devenu 580 Mo une fois tranché. On sort
   donc en `GLTF_SEPARATE` avec une bibliothèque `textures/` commune.
2. **`export_keep_originals=False`.** Les images viennent de fichiers .jpg
   partagés ; `True` les omet silencieusement et rend la roche blanche.
3. **Un cuiseur n'efface jamais en silence une carte déjà cuite.** Il refuse et
   demande qu'on écarte l'ancienne à la main.

Et une leçon de cette carte-ci : **rendre les meshes mono-utilisateurs avant de
joindre**. Les instances partagent leur datablock ; `join()` transforme la
donnée partagée sur place, donc chaque doublon part à 2× son écart réel — la
forêt s'était étalée de x=-274 à +196 sur une carte large de 280.

  python tools/blender/bmcp.py code tools/blender/expedition/vallon.py
  python tools/blender/bmcp.py code tools/blender/expedition/92_cellules.py
"""

import json
import os
import sys
from collections import defaultdict

import bpy
from mathutils import Vector

RACINE = r"C:\Users\Antoi\Desktop\Forgia Rewrite"
SORTIE = os.path.join(RACINE, "assets", "models", "environment", "expedition",
                      "vallon_stream_cells")
# 40 m : l'emprise fait 280 x 200, soit 7 x 5 = 35 cellules. Assez fin pour que
# le rayon de rendu n'en charge qu'une poignée, assez gros pour ne pas produire
# des centaines de fichiers.
CELLULE_M = 40.0
# Ces collections ne se découpent PAS : le moteur doit les garder entières pour
# les animer (défilement de l'eau, battants de la porte).
HORS_DECOUPE = {"eau", "portes"}
# Les disques de controle de la faune servent au RENDU de reperage,
# jamais au jeu : ils ne doivent entrer dans aucune cellule.
JAMAIS_EXPORTE = {"faune_controle", "faune_apercu"}
CACHES = {"_proto", "_src"}


def bornes_monde(obj):
    pts = [obj.matrix_world @ Vector(c) for c in obj.bound_box]
    return (min(p.x for p in pts), max(p.x for p in pts),
            min(p.y for p in pts), max(p.y for p in pts),
            min(p.z for p in pts), max(p.z for p in pts))


def cellule_de(b):
    """Indice de cellule. Bevy (x, z) = Blender (x, -y) — convention château."""
    cx = (b[0] + b[1]) * 0.5
    cz = -((b[2] + b[3]) * 0.5)
    return (int(cx // CELLULE_M), int(cz // CELLULE_M))


def fusionner(objets):
    bpy.ops.object.select_all(action="DESELECT")
    for o in objets:
        o.hide_set(False)
        o.select_set(True)
    bpy.context.view_layer.objects.active = objets[0]
    if len(objets) > 1:
        # BLOQUANT : voir l'en-tête. Sans ça la géométrie part au loin.
        bpy.ops.object.make_single_user(object=True, obdata=True)
        bpy.ops.object.join()
    return bpy.context.view_layer.objects.active


def exporter(objets, chemin, separe=True):
    bpy.ops.object.select_all(action="DESELECT")
    for o in objets:
        o.hide_set(False)
        o.select_set(True)
    bpy.context.view_layer.objects.active = objets[0]
    options = dict(
        filepath=chemin,
        export_format="GLTF_SEPARATE" if separe else "GLB",
        use_selection=True,
        export_apply=True,
        export_yup=True,
        export_texcoords=True,
        export_normals=True,
        export_materials="EXPORT",
    )
    if separe:
        options["export_texture_dir"] = "textures"
        # `True` omettrait les images sans fichier source : roche blanche.
        options["export_keep_originals"] = False
    try:
        bpy.ops.export_scene.gltf(export_vertex_color="ACTIVE", **options)
    except TypeError:
        bpy.ops.export_scene.gltf(**options)


def main():
    # LE GARDE PORTE SUR LA CARTE, PAS SUR LA BIBLIOTHEQUE DE TEXTURES.
    #
    # Ecrit « tout fichier present = refus », il se contredisait avec
    # l'exportateur : celui-ci n'A PAS CREE le sous-dossier `textures/` et
    # echouait sur `OSError [Errno 22]` des que la sortie etait vraiment vide.
    # Impossible de satisfaire les deux — donc impossible de recuire sans
    # contourner le garde, ce qui est la pire facon de garder une protection.
    #
    # Ce qu'on protege reellement, ce sont les CELLULES : elles portent le
    # travail. La bibliotheque de textures est partagee et se reecrit a
    # l'identique. Le garde ne regarde donc plus qu'elles, et on cree le
    # sous-dossier nous-memes au lieu d'esperer que l'exportateur le fasse.
    cuits = [f for f in os.listdir(SORTIE)] if os.path.isdir(SORTIE) else []
    cellules = [f for f in cuits if f.endswith((".gltf", ".bin", ".toml"))]
    if cellules:
        raise RuntimeError(
            f"sortie déjà peuplée : {SORTIE} — {len(cellules)} fichier(s) de carte, "
            "l'écarter à la main avant de recuire")
    os.makedirs(os.path.join(SORTIE, "textures"), exist_ok=True)

    for nom in CACHES:
        coll = bpy.data.collections.get(nom)
        if coll:
            for obj in list(coll.objects):
                bpy.data.objects.remove(obj, do_unlink=True)
            bpy.data.collections.remove(coll)
    for obj in bpy.data.objects:
        obj.hide_set(False)
        obj.hide_viewport = obj.hide_render = False

    rapport = {"cellule_m": CELLULE_M, "cellules": [], "entiers": []}

    # --- 1. ce qui reste entier (animé) ----------------------------------
    for nom in sorted(HORS_DECOUPE):
        coll = bpy.data.collections.get(nom)
        objets = [o for o in coll.objects if o.type == "MESH"] if coll else []
        if not objets:
            continue
        fusion = fusionner(objets)
        fusion.name = f"vallon_{nom}"
        chemin = os.path.join(SORTIE, f"vallon_{nom}.gltf")
        exporter([fusion], chemin)
        fusion.data.calc_loop_triangles()
        rapport["entiers"].append({
            "nom": fusion.name, "tris": len(fusion.data.loop_triangles),
            "fichier": os.path.basename(chemin)})
        bpy.data.objects.remove(fusion, do_unlink=True)

    # --- 2. le reste, par cellule ----------------------------------------
    par_cellule = defaultdict(list)
    bornes = {}
    for obj in list(bpy.context.scene.objects):
        if obj.type != "MESH":
            continue
        if any(c.name in HORS_DECOUPE for c in obj.users_collection):
            continue
        if any(c.name in JAMAIS_EXPORTE for c in obj.users_collection):
            continue
        b = bornes_monde(obj)
        cle = cellule_de(b)
        par_cellule[cle].append(obj)
        anc = bornes.get(cle)
        bornes[cle] = b if anc is None else (
            min(anc[0], b[0]), max(anc[1], b[1]), min(anc[2], b[2]),
            max(anc[3], b[3]), min(anc[4], b[4]), max(anc[5], b[5]))

    for cle in sorted(par_cellule):
        objets = par_cellule[cle]
        source = len(objets)
        fusion = fusionner(objets)
        fusion.name = f"cell_x{cle[0]}_z{cle[1]}"
        chemin = os.path.join(SORTIE, f"{fusion.name}_render.gltf")
        exporter([fusion], chemin)
        fusion.data.calc_loop_triangles()
        b = bornes[cle]
        rapport["cellules"].append({
            "id": fusion.name,
            "render": f"models/environment/expedition/vallon_stream_cells/{fusion.name}_render.gltf#Scene0",
            # Bornes converties en repère Bevy : Y de Blender devient -Z.
            "bounds_min_m": [round(b[0], 3), round(b[4], 3), round(-b[3], 3)],
            "bounds_max_m": [round(b[1], 3), round(b[5], 3), round(-b[2], 3)],
            "source_meshes": source,
            "tris": len(fusion.data.loop_triangles),
        })
        bpy.data.objects.remove(fusion, do_unlink=True)

    # --- 3. manifeste, au format que castle_hub.rs sait déjà lire ---------
    lignes = [
        "# Généré par tools/blender/expedition/92_cellules.py — ne pas éditer à la main.",
        "schema_version = 1",
        f"cell_size_m = {CELLULE_M}",
        'coordinate_system = "bevy_y_up_meters"',
        'asset_format = "gltf_separate_shared_texture_library"',
        "",
    ]
    for c in rapport["cellules"]:
        lignes += [
            "[[cells]]",
            f'id = "{c["id"]}"',
            f'render = "{c["render"]}"',
            f"bounds_min_m = [{', '.join(str(v) for v in c['bounds_min_m'])}]",
            f"bounds_max_m = [{', '.join(str(v) for v in c['bounds_max_m'])}]",
            f"source_meshes = {c['source_meshes']}",
            "",
        ]
    with open(os.path.join(SORTIE, "vallon_stream_cells.toml"), "w", encoding="utf-8") as fh:
        fh.write("\n".join(lignes))

    octets = sum(os.path.getsize(os.path.join(SORTIE, f))
                 for f in os.listdir(SORTIE)
                 if os.path.isfile(os.path.join(SORTIE, f)))
    tex = os.path.join(SORTIE, "textures")
    octets_tex = (sum(os.path.getsize(os.path.join(tex, f)) for f in os.listdir(tex))
                  if os.path.isdir(tex) else 0)
    # Le decoupage a VIDE la scene : les objets ont ete fusionnes par cellule
    # puis supprimes. Ne restent que les collections non exportees (l'apercu de
    # la faune), ce qui donne l'impression que la carte a disparu. On le dit
    # ici, sur la derniere ligne lue, plutot que dans un en-tete que personne
    # ne relit au moment ou la question se pose.
    print("[cellules] SCENE CONSOMMEE — relancer vallon.py pour retrouver la carte",
          file=sys.stderr)
    print("RESULT: " + json.dumps({
        "cellules": len(rapport["cellules"]),
        "entiers": rapport["entiers"],
        "tris_total": sum(c["tris"] for c in rapport["cellules"]),
        "octets_cellules": octets,
        "octets_textures": octets_tex,
        "total_octets": octets + octets_tex,
        "plus_grosse": max(rapport["cellules"], key=lambda c: c["tris"])["id"],
        "tris_max_par_cellule": max(c["tris"] for c in rapport["cellules"]),
    }, ensure_ascii=False))


main()
