"""Cuit la carte pour le moteur : visuel, collision, surface jouable.

Le contrat vient du château (`castle_hub.rs:64-93`), qui a déjà payé les
erreurs : un GLB monolithique de milliers de pièces ne s'instancie pas, la
collision se cuit HORS LIGNE en un TriMesh à part, la surface jouable se cuit
séparément (elle ne subit pas la décimation des murs), et un point critique
repose sur un proxy déterministe, jamais sur le maillage décimé.

  visuel   → expedition_vallon.glb            (fusionné par collection)
  physique → expedition_vallon_collision.glb  (terrain + bâti + pont)
  navmesh  → expedition_vallon_walkable.glb   (terrain + chemin seuls)

Les fusions ne sont pas cosmétiques : sans elles, 4 400 objets deviennent
4 400 nœuds glTF, donc autant d'entités ECS à l'entrée dans la carte.
"""

import json
import os

import bpy

SORTIE = r"C:\Users\Antoi\Desktop\Forgia Rewrite\assets\models\environment\expedition"
CACHES = {"_proto", "_src"}


def objets_visibles(nom_collection):
    coll = bpy.data.collections.get(nom_collection)
    if coll is None:
        return []
    return [o for o in coll.objects if o.type == "MESH"]


def fusionner(nom_collection, nom_final):
    """Joint tous les meshes d'une collection en un objet."""
    objets = objets_visibles(nom_collection)
    if not objets:
        return None
    bpy.ops.object.select_all(action="DESELECT")
    for obj in objets:
        obj.hide_set(False)
        obj.select_set(True)
    bpy.context.view_layer.objects.active = objets[0]
    if len(objets) > 1:
        # BLOQUANT — rendre chaque mesh mono-utilisateur AVANT de joindre.
        # Les instances partagent leur datablock (c'est tout l'intérêt : 1 500
        # arbres pour 20 meshes). Mais `join()` transforme la donnée partagée
        # SUR PLACE : chaque doublon se fait donc transformer une deuxième
        # fois, et sa géométrie part à ~2× son écart réel. Mesuré : la forêt
        # s'étalait de x=-274 à +196 sur une carte qui s'arrête à ±120.
        # Le défaut ne lève aucune erreur — il produit un GLB plausible et faux.
        bpy.ops.object.make_single_user(object=True, obdata=True)
        bpy.ops.object.join()
    fusion = bpy.context.view_layer.objects.active
    fusion.name = nom_final
    return fusion


def exporter(objets, chemin, materiaux=True):
    bpy.ops.object.select_all(action="DESELECT")
    for obj in objets:
        if obj is None:
            continue
        obj.hide_set(False)
        obj.select_set(True)
    bpy.context.view_layer.objects.active = next((o for o in objets if o), None)
    options = dict(
        filepath=chemin,
        export_format="GLB",
        use_selection=True,
        export_apply=True,          # applique les échelles d'instance
        export_yup=True,            # Blender Z-up → glTF/Bevy Y-up
        export_texcoords=True,
        export_normals=True,
        export_materials="EXPORT" if materiaux else "NONE",
        export_image_format="AUTO",
    )
    # Le nom de l'option « couleurs de sommet » a bougé entre versions de
    # Blender. On tente, et on retombe sans elle plutôt que de perdre l'export
    # entier pour un mot-clé — le terrain garde alors sa texture, il perd juste
    # sa variation de teinte.
    try:
        bpy.ops.export_scene.gltf(export_vertex_color="ACTIVE", **options)
    except TypeError:
        bpy.ops.export_scene.gltf(**options)
    return os.path.getsize(chemin) if os.path.exists(chemin) else 0


def main():
    # Les prototypes cachés ne doivent JAMAIS partir à l'export : ce sont les
    # pièces sources, pas des instances placées.
    for nom in CACHES:
        coll = bpy.data.collections.get(nom)
        if coll:
            for obj in list(coll.objects):
                bpy.data.objects.remove(obj, do_unlink=True)
            bpy.data.collections.remove(coll)

    for obj in bpy.data.objects:
        obj.hide_set(False)
        obj.hide_viewport = False
        obj.hide_render = False

    rapport = {}

    # --- 1. visuel -------------------------------------------------------
    parties = []
    for coll, nom in (("sol", "vallon_sol"), ("falaises", "vallon_eboulis"),
                      ("foret", "vallon_foret"), ("village", "vallon_village"),
                      ("campements", "vallon_campements"), ("reperes", "vallon_reperes"),
                      ("lampes", "vallon_lampes"),
                      ("eau", "vallon_eau"), ("portes", "vallon_portes")):
        fusion = fusionner(coll, nom)
        if fusion:
            parties.append(fusion)
            rapport[nom] = {
                "tris": len(fusion.data.loop_triangles) or len(fusion.data.polygons),
                "materiaux": len(fusion.data.materials),
            }

    for obj in parties:
        obj.data.calc_loop_triangles()
        rapport[obj.name]["tris"] = len(obj.data.loop_triangles)

    rapport["visuel_octets"] = exporter(parties, os.path.join(SORTIE, "expedition_vallon.glb"))

    # --- 2. collision ----------------------------------------------------
    # Terrain + bâti + pont. Les arbres en sont EXCLUS : 1 500 TriMesh d'arbre
    # coûteraient ~300 000 triangles de collision là où un cylindre par tronc
    # suffit — ces cylindres sont listés dans le manifeste JSON.
    sol = bpy.data.objects.get("vallon_sol")
    batis = bpy.data.objects.get("vallon_village")
    copies = []
    for source in (sol, batis):
        if source is None:
            continue
        copie = source.copy()
        copie.data = source.data.copy()
        bpy.context.scene.collection.objects.link(copie)
        copies.append(copie)
    if copies:
        bpy.ops.object.select_all(action="DESELECT")
        for c in copies:
            c.select_set(True)
        bpy.context.view_layer.objects.active = copies[0]
        if len(copies) > 1:
            bpy.ops.object.join()
        collision = bpy.context.view_layer.objects.active
        collision.name = "vallon_collision"
        collision.data.calc_loop_triangles()
        rapport["collision_tris"] = len(collision.data.loop_triangles)
        rapport["collision_octets"] = exporter(
            [collision], os.path.join(SORTIE, "expedition_vallon_collision.glb"),
            materiaux=False)
        bpy.data.objects.remove(collision, do_unlink=True)

    # --- 3. surface jouable ---------------------------------------------
    # Le terrain seul : c'est lui que le navmesh doit voir. Y verser le bâti
    # ferait passer les toits pour du sol.
    if sol is not None:
        copie = sol.copy()
        copie.data = sol.data.copy()
        bpy.context.scene.collection.objects.link(copie)
        copie.name = "vallon_walkable"
        copie.data.calc_loop_triangles()
        rapport["walkable_tris"] = len(copie.data.loop_triangles)
        rapport["walkable_octets"] = exporter(
            [copie], os.path.join(SORTIE, "expedition_vallon_walkable.glb"),
            materiaux=False)
        bpy.data.objects.remove(copie, do_unlink=True)

    rapport["fichiers"] = sorted(f for f in os.listdir(SORTIE))
    print("RESULT: " + json.dumps(rapport, ensure_ascii=False))


main()
