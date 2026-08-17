"""Rend la tête de l'avatar en 3 vues, et relève son ORIENTATION.

    python tools/blender/bmcp.py code tools/blender/personnage/31_vue_tete.py

Deux choses qu'aucun nombre ne donne :

1. **Le style se voit.** Densité du lissage, dureté des arêtes, saturation de la
   peau, taille des yeux : on ne les déduit pas d'un compte de triangles.
2. **Le sens du regard.** Une tête de chien a un museau ; posé du mauvais côté,
   il sort par la nuque. Le sens se relève sur les ORTEILS (`LeftToeBase` est
   devant `LeftFoot`), jamais supposé — c'est la même discipline que « les
   positions se lisent » de `spawn-clearance.md`.

Rien n'est écrit dans le dépôt : les images vont dans le bac de session.
"""

import json
import math
import os

import bpy
from mathutils import Vector

RACINE = r"C:\Users\Antoi\Desktop\Forgia Rewrite"
CORPS = os.path.join(RACINE, "assets", "models", "characters", "stylized",
                     "stylized_male_complet.glb")
BAC = (r"C:\Users\Antoi\AppData\Local\Temp\claude"
       r"\c--Users-Antoi-Desktop-Forgia-Rewrite"
       r"\2269ac95-4478-478f-b077-660d6c666db7\scratchpad")


def vider():
    """Vide la scène pour de bon.

    🚨 PAS `bpy.ops.object.select_all` : il ne sélectionne QUE ce qui est
    sélectionnable et visible. Tout objet masqué survit à la suppression — et
    ressort à l'import suivant sous un nom suffixé, pendant que le vrai objet
    attendu manque. Constaté deux fois : les icosphères de widget qui traînaient
    depuis le matin, puis l'armature de cape disparue de la scène alors que sa
    donnée subsistait, laissant `Cloak_low` sans parent et donc affichée à sa
    taille brute — cent fois trop grande.
    On supprime donc par la DONNÉE, où rien ne se cache."""
    for obj in list(bpy.data.objects):
        bpy.data.objects.remove(obj, do_unlink=True)
    for coll in (bpy.data.meshes, bpy.data.armatures, bpy.data.actions,
                 bpy.data.materials, bpy.data.images, bpy.data.collections,
                 bpy.data.cameras, bpy.data.lights):
        for bloc in list(coll):
            try:
                coll.remove(bloc)
            except (RuntimeError, ReferenceError):
                pass


def sens_du_regard(arm):
    """Vecteur horizontal « devant », relevé sur les orteils. Les orteils sont
    DEVANT la cheville : c'est un fait d'anatomie, pas une convention d'export."""
    paires = [("LeftFoot", "LeftToeBase"), ("RightFoot", "RightToeBase")]
    avants = []
    for cheville, orteil in paires:
        a, b = arm.pose.bones.get(cheville), arm.pose.bones.get(orteil)
        if a is None or b is None:
            continue
        d = (arm.matrix_world @ b.matrix).translation - (arm.matrix_world @ a.matrix).translation
        d.z = 0.0
        if d.length > 1e-4:
            avants.append(d.normalized())
    if not avants:
        return None
    moy = sum(avants, Vector((0, 0, 0))) / len(avants)
    return moy.normalized()


def rendre(nom, cible, distance, azimut_deg, elevation_deg, taille=640, lens=45.0):
    """Une vue orbitale autour de `cible`. Azimut 0 = pile dans le dos du regard,
    donc face au visage.

    ⚠️ Le cadrage se DÉRIVE : à `lens` mm sur un capteur 36 mm, la largeur vue à
    la distance `d` vaut `36·d/lens`. Un premier essai à 70 mm / 0,55 m ne
    couvrait que 0,28 m pour une tête de 0,32 m — on n'a vu que des cheveux."""
    cam_data = bpy.data.cameras.new(nom)
    cam_data.lens = lens
    cam = bpy.data.objects.new(nom, cam_data)
    bpy.context.scene.collection.objects.link(cam)

    a, e = math.radians(azimut_deg), math.radians(elevation_deg)
    offset = Vector((math.sin(a) * math.cos(e), -math.cos(a) * math.cos(e),
                     math.sin(e))) * distance
    cam.location = cible + offset
    direction = (cible - cam.location).normalized()
    cam.rotation_euler = direction.to_track_quat("-Z", "Y").to_euler()

    bpy.context.scene.camera = cam
    bpy.context.scene.render.resolution_x = taille
    bpy.context.scene.render.resolution_y = taille
    bpy.context.scene.render.film_transparent = False
    chemin = os.path.join(BAC, f"{nom}.png")
    bpy.context.scene.render.filepath = chemin
    bpy.ops.render.render(write_still=True)
    return chemin


def main():
    vider()
    bpy.ops.import_scene.gltf(filepath=CORPS)

    arm = next((o for o in bpy.data.objects if o.type == "ARMATURE"), None)
    avant = sens_du_regard(arm) if arm else None

    tete = bpy.data.objects.get("SM_Head")
    pts = [tete.matrix_world @ v.co for v in tete.data.vertices]
    centre = sum(pts, Vector((0, 0, 0))) / len(pts)

    # Point le plus avancé dans le sens du regard = le nez. Il donne la
    # profondeur de visage disponible pour un museau.
    saillie = None
    if avant is not None:
        proj = [(p - centre).dot(avant) for p in pts]
        saillie = {
            "nez_avancee_m": round(max(proj), 4),
            "arriere_crane_m": round(min(proj), 4),
        }

    # Emprise du corps SANS les icosphères de widget d'os, qui ont fuité dans
    # l'export et fausseraient la hauteur (2,80 m relevés au lieu de ~1,80).
    corps = [o for o in bpy.data.objects
             if o.type == "MESH" and not o.name.startswith("Icosph")]
    tous = [o.matrix_world @ v.co for o in corps for v in o.data.vertices]
    z_bas, z_haut = min(p.z for p in tous), max(p.z for p in tous)
    centre_corps = Vector((sum(p.x for p in tous) / len(tous),
                           sum(p.y for p in tous) / len(tous),
                           (z_bas + z_haut) / 2))

    # Éclairage neutre : un ciel gris uniforme montre la FORME. Une lumière
    # dirigée montrerait surtout l'ombre qu'elle projette.
    monde = bpy.data.worlds.new("neutre")
    monde.use_nodes = True
    # Le nœud se cherche par TYPE : son nom change d'une version de Blender à
    # l'autre, et un KeyError sur un décor annulerait tout le rendu.
    fond = next((n for n in monde.node_tree.nodes if n.type == "BACKGROUND"), None)
    if fond is not None:
        fond.inputs[0].default_value = (0.35, 0.36, 0.4, 1)
        fond.inputs[1].default_value = 1.4
    bpy.context.scene.world = monde
    key = bpy.data.lights.new("key", type="AREA")
    key.energy, key.size = 120.0, 1.2
    obj_key = bpy.data.objects.new("key", key)
    obj_key.location = centre + Vector((0.7, -0.9, 0.8))
    obj_key.rotation_euler = (centre - obj_key.location).normalized().to_track_quat("-Z", "Y").to_euler()
    bpy.context.scene.collection.objects.link(obj_key)

    moteurs = [e.identifier for e in
               bpy.types.RenderSettings.bl_rna.properties["engine"].enum_items]
    bpy.context.scene.render.engine = ("BLENDER_EEVEE_NEXT" if "BLENDER_EEVEE_NEXT"
                                       in moteurs else "BLENDER_EEVEE")

    vues = {"corps_entier": rendre("corps_entier", centre_corps,
                                   (z_haut - z_bas) * 1.35, 25, 4, lens=45.0)}

    # On isole la tête : le reste du corps masquerait le visage de trois quarts.
    for o in bpy.data.objects:
        if o.type == "MESH" and o.name != "SM_Head":
            o.hide_render = True

    for nom, azimut, elevation in (("tete_face", 0, 3),
                                   ("tete_trois_quarts", 40, 8),
                                   ("tete_profil", 90, 3)):
        vues[nom] = rendre(nom, centre, 0.62, azimut, elevation, lens=42.0)

    print("RESULT: " + json.dumps({
        "avant_monde": [round(v, 4) for v in avant] if avant else None,
        "centre_tete": [round(v, 4) for v in centre],
        "corps_hauteur_reelle_m": round(z_haut - z_bas, 4),
        "saillie": saillie,
        "moteur": bpy.context.scene.render.engine,
        "vues": vues,
    }, ensure_ascii=False))


main()
