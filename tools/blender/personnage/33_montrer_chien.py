"""Met la tête de chien SOUS LES YEUX, dans le viewport Blender.

    python tools/blender/bmcp.py code tools/blender/personnage/33_montrer_chien.py

À lancer APRÈS `32_tete_chien.py`, sur la scène qu'il laisse.

CE QU'IL CORRIGE. Le script de sculpture masque la tête humaine `hide_render`
seulement : au rendu elle disparaît, dans le viewport elle reste plantée dans le
chien. Un rendu propre et une vue inutilisable, c'est le même défaut que le
« binaire à jour, observable périmé » — deux vérités qui divergent parce que
personne n'a dit laquelle fait foi.

Il range aussi les 22 pièces dans une collection `chien`, pour qu'on puisse les
éteindre d'un clic et comparer avec la tête d'origine.
"""

import json
import math

import bpy
from mathutils import Vector

CIBLE = "chien_"          # préfixe des pièces sculptées
CACHE = "SM_Head"         # la tête d'origine
AZIMUT_DEG = 38.0         # trois quarts — la vue qui montre le plus de forme
ELEVATION_DEG = 6.0
RECUL = 0.95              # mètres depuis le centre de la tête


def collection(nom):
    coll = bpy.data.collections.get(nom)
    if coll is None:
        coll = bpy.data.collections.new(nom)
        bpy.context.scene.collection.children.link(coll)
    return coll


def ranger(objets, nom):
    coll = collection(nom)
    for obj in objets:
        for autre in list(obj.users_collection):
            autre.objects.unlink(obj)
        coll.objects.link(obj)
    return coll


def cadrer(centre, avant):
    """Pointe TOUTES les vues 3D sur la tête, en matériau.

    `view_rotation` tourne un repère dont le -Z regarde : on lui donne donc la
    rotation qui amène +Z sur (œil − cible), pas l'inverse. Se tromper de sens
    ici cadre l'arrière du crâne, ce qui a l'air d'un bug de placement."""
    droite = Vector((0.0, 0.0, 1.0)).cross(avant).normalized()
    a, e = math.radians(AZIMUT_DEG), math.radians(ELEVATION_DEG)
    # 🚨 `+ avant`, pas `- avant`. La formule de rendu de `32_tete_chien.py`
    # s'écrit avec un `-cos(a)` sur l'axe Y du MONDE, parce que le personnage
    # regarde vers -Y : ce moins EST la conversion vers « devant ». Recopié ici
    # sur un vecteur déjà orienté devant, il le retourne — la vue s'est cadrée
    # trois fois de suite sur la nuque. Une formule se relit dans son repère,
    # elle ne se transcrit pas.
    oeil = centre + (droite * math.sin(a) * math.cos(e)
                     + avant * math.cos(a) * math.cos(e)
                     + Vector((0.0, 0.0, 1.0)) * math.sin(e)) * RECUL

    vues = 0
    for fenetre in bpy.context.window_manager.windows:
        for zone in fenetre.screen.areas:
            if zone.type != "VIEW_3D":
                continue
            espace = zone.spaces.active
            espace.shading.type = "MATERIAL"
            espace.shading.use_scene_world = False
            espace.shading.studiolight_intensity = 1.0
            if espace.region_3d.view_perspective == "CAMERA":
                espace.region_3d.view_perspective = "PERSP"
            espace.region_3d.view_location = centre
            espace.region_3d.view_distance = RECUL
            espace.region_3d.view_rotation = (oeil - centre).to_track_quat("Z", "Y")
            espace.clip_start = 0.01
            zone.tag_redraw()
            vues += 1
    return vues


def main():
    # Filtré sur le TYPE, pas seulement sur le nom : une caméra de rendu porte
    # le même préfixe et n'a pas de sommets.
    pieces = [o for o in bpy.context.scene.objects
              if o.name.startswith(CIBLE) and o.type == "MESH"]
    if not pieces:
        print("RESULT: " + json.dumps(
            {"erreur": "aucune pièce de chien — lancer 32_tete_chien.py d'abord"}))
        return

    ranger(pieces, "chien")

    # La tête d'origine : masquée POUR DE BON cette fois (viewport + rendu).
    # Alt+H la fait revenir si on veut comparer.
    humaine = bpy.data.objects.get(CACHE)
    if humaine is not None:
        humaine.hide_set(True)
        humaine.hide_render = True

    # Les icosphères de widget d'os ont fuité dans l'export du corps ; elles
    # flottent autour du personnage et polluent la vue. Les caméras de rendu
    # aussi : elles ne servent plus une fois les PNG écrits.
    parasites = 0
    for obj in list(bpy.context.scene.objects):
        if obj.name.startswith("Icosph"):
            obj.hide_set(True)
            obj.hide_render = True
            parasites += 1
        elif obj.type == "CAMERA" and obj.name.startswith(("vue_", CIBLE)):
            bpy.data.objects.remove(obj, do_unlink=True)
            parasites += 1

    # Le cap, relu sur le corps — et AVANT toute modification de visibilité :
    # voir le bloc suivant.
    #
    # 🚨 Les ÉPAULES donnent l'axe, les ORTEILS le sens. Mesuré : le cap des
    # orteils s'écarte de 14° de celui de la tête (pieds écartés au repos),
    # celui des épaules de 2,2° seulement. Cadrer sur les pieds montrait un
    # trois-quarts en croyant montrer une face.
    arm = next((o for o in bpy.context.scene.objects if o.type == "ARMATURE"), None)
    grossier = Vector((0.0, 0.0, 0.0))
    for cheville, orteil in (("LeftFoot", "LeftToeBase"), ("RightFoot", "RightToeBase")):
        a, b = arm.pose.bones.get(cheville), arm.pose.bones.get(orteil)
        if a and b:
            d = ((arm.matrix_world @ b.matrix).translation
                 - (arm.matrix_world @ a.matrix).translation)
            d.z = 0.0
            if d.length > 1e-4:
                grossier += d.normalized()
    lateral = ((arm.matrix_world @ arm.pose.bones["RightArm"].matrix).translation
               - (arm.matrix_world @ arm.pose.bones["LeftArm"].matrix).translation)
    lateral.z = 0.0
    avant = lateral.normalized().cross(Vector((0.0, 0.0, 1.0))).normalized()
    if grossier.length > 1e-6 and avant.dot(grossier.normalized()) < 0.0:
        avant = -avant

    # 🚨 Les icosphères grises autour du personnage ne sont PAS des objets : ce
    # sont les FORMES PERSONNALISÉES des os. Masquer l'objet icosphère ne les
    # enlève pas — l'armature les dessine quand même. Il faut couper côté
    # armature.
    #
    # 🚨 Et cela vient APRÈS la lecture des orteils. Une armature masquée sort
    # du depsgraph : ses matrices de pose deviennent périmées, le repère lu
    # dessus part à l'envers, et la vue s'était cadrée sur la NUQUE. On ne
    # mesure jamais sur ce qu'on vient de modifier.
    for autre in bpy.context.scene.objects:
        if autre.type != "ARMATURE":
            continue
        autre.hide_set(True)
        autre.show_in_front = False

    sommets = [o.matrix_world @ v.co for o in pieces for v in o.data.vertices]
    centre = Vector((
        sum(p.x for p in sommets) / len(sommets),
        sum(p.y for p in sommets) / len(sommets),
        (min(p.z for p in sommets) + max(p.z for p in sommets)) / 2.0))

    vues = cadrer(centre, avant)

    bpy.ops.object.select_all(action="DESELECT")
    bpy.context.view_layer.objects.active = None

    print("RESULT: " + json.dumps({
        "pieces_rangees": len(pieces),
        "collection": "chien",
        "tete_humaine_masquee": humaine is not None,
        "widgets_masques": parasites,
        "centre_tete": [round(v, 4) for v in centre],
        "vues_3d_cadrees": vues,
    }, ensure_ascii=False))


main()
