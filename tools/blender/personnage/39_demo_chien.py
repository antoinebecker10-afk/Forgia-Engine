"""Monte la DÉMO : chaque clip cadré, éclairé, joué et rendu en vidéo.

    python tools/blender/bmcp.py code tools/blender/personnage/39_demo_chien.py

À lancer APRÈS `38_anim_secondaire.py`.

CE QU'ELLE PRODUIT

    une vidéo par clip, dans le bac de session — 9 fichiers courts qu'on ouvre
    d'un clic, sans Blender ;
    la scène laissée sur `idle`, caméra posée, prête à jouer à la barre Espace.

POURQUOI UNE VIDÉO PAR CLIP, ET PAS UNE SEULE

Assembler neuf plages en une timeline demanderait de DÉPLACER les bandes NLA —
or ce sont elles que l'export glTF lit pour découper ses animations. Une démo
qui casse l'export n'est pas une démo, c'est un piège. Chaque clip est donc
rendu dans sa propre plage, sans rien déplacer.

LE CADRAGE EST DÉRIVÉ, PAS POSÉ. La caméra vise le milieu du personnage et
recule de ce qu'il faut pour le tenir en entier, à la focale choisie. Écrire une
distance en dur donnerait un cadrage juste pour CE personnage et faux au premier
changement de taille.
"""

import json
import math
import os

import bpy
from mathutils import Vector

BAC = (r"C:\Users\Antoi\AppData\Local\Temp\claude"
       r"\c--Users-Antoi-Desktop-Forgia-Rewrite"
       r"\2269ac95-4478-478f-b077-660d6c666db7\scratchpad\demo_chien")

SPEC = {
    "taille": 560,
    "focale": 50.0,
    "azimut_deg": 32.0,       # trois quarts : on voit le museau ET une oreille
    "elevation_deg": 6.0,
    "marge": 1.22,            # ce qu'on laisse autour du personnage
    # Le buste seulement : les oreilles, les yeux et la gueule sont l'objet de
    # la démo, et un plan pied les rendrait illisibles.
    "part_haute": 0.42,       # fraction haute du corps que l'on cadre
    "fps": 24,
}


def vider_demo():
    for nom in ("demo_camera", "demo_key", "demo_fill"):
        obj = bpy.data.objects.get(nom)
        if obj is not None:
            bpy.data.objects.remove(obj, do_unlink=True)


def corps_visible():
    return [o for o in bpy.context.scene.objects
            if o.type == "MESH" and not o.name.startswith("Icosph")
            and not o.hide_render]


def cadrer(devant):
    """Caméra et lumières, déduites de l'emprise réelle du personnage."""
    pts = [o.matrix_world @ v.co for o in corps_visible() for v in o.data.vertices]
    z_bas, z_haut = min(p.z for p in pts), max(p.z for p in pts)
    hauteur = z_haut - z_bas
    haut = SPEC["part_haute"] * hauteur
    cible = Vector((sum(p.x for p in pts) / len(pts),
                    sum(p.y for p in pts) / len(pts),
                    z_haut - haut * 0.5))

    # Distance dérivée : à `focale` mm sur un capteur 36 mm, il faut reculer de
    # `36 · h / (2 · focale · tan(fov/2))`… autrement dit `h · focale / 36`.
    distance = haut * SPEC["marge"] * SPEC["focale"] / 36.0

    data = bpy.data.cameras.new("demo_camera")
    data.lens = SPEC["focale"]
    cam = bpy.data.objects.new("demo_camera", data)
    bpy.context.scene.collection.objects.link(cam)

    a, e = math.radians(SPEC["azimut_deg"]), math.radians(SPEC["elevation_deg"])
    lateral = Vector((0.0, 0.0, 1.0)).cross(devant).normalized()
    cam.location = cible + (lateral * math.sin(a) * math.cos(e)
                            + devant * math.cos(a) * math.cos(e)
                            + Vector((0.0, 0.0, 1.0)) * math.sin(e)) * distance
    cam.rotation_euler = (cible - cam.location).normalized().to_track_quat(
        "-Z", "Y").to_euler()
    bpy.context.scene.camera = cam

    monde = bpy.data.worlds.new("demo_monde")
    monde.use_nodes = True
    fond = next((n for n in monde.node_tree.nodes if n.type == "BACKGROUND"), None)
    if fond is not None:
        fond.inputs[0].default_value = (0.33, 0.35, 0.39, 1.0)
        fond.inputs[1].default_value = 1.3
    bpy.context.scene.world = monde

    for nom, decalage, energie in (("demo_key", (0.9, -1.1, 1.0), 130.0),
                                   ("demo_fill", (-1.3, -0.7, 0.3), 45.0)):
        lampe = bpy.data.lights.new(nom, type="AREA")
        lampe.energy, lampe.size = energie, 1.4
        obj = bpy.data.objects.new(nom, lampe)
        obj.location = cible + Vector(decalage)
        obj.rotation_euler = (cible - obj.location).normalized().to_track_quat(
            "-Z", "Y").to_euler()
        bpy.context.scene.collection.objects.link(obj)
    return cible, distance


def action_de_cles(peau, nom_clip):
    """L'action de clés de forme qui accompagne ce clip, si elle existe."""
    return bpy.data.actions.get(f"{nom_clip}_cligner")


def main():
    arm = bpy.data.objects.get("perso_squelette")
    peau = bpy.data.objects.get("chien_peau")
    if arm is None or peau is None:
        print("RESULT: " + json.dumps({"erreur": "scène incomplète"}))
        return
    if not os.path.isdir(BAC):
        os.makedirs(BAC)

    arm.data.pose_position = "POSE"
    for obj in bpy.context.scene.objects:
        if obj.type == "ARMATURE":
            obj.hide_set(False)
            obj.hide_render = True   # l'armature ne se rend pas, mais déforme
    tete = bpy.data.objects.get("SM_Head")
    if tete is not None:
        tete.hide_render = True
    for obj in bpy.context.scene.objects:
        if obj.name.startswith("Icosph"):
            obj.hide_render = True
    bpy.context.view_layer.update()

    lateral = ((arm.matrix_world @ arm.pose.bones["RightArm"].matrix).translation
               - (arm.matrix_world @ arm.pose.bones["LeftArm"].matrix).translation)
    lateral.z = 0.0
    devant = lateral.normalized().cross(Vector((0.0, 0.0, 1.0))).normalized()
    orteil = ((arm.matrix_world @ arm.pose.bones["LeftToeBase"].matrix).translation
              - (arm.matrix_world @ arm.pose.bones["LeftFoot"].matrix).translation)
    orteil.z = 0.0
    if devant.dot(orteil.normalized()) < 0.0:
        devant = -devant

    vider_demo()
    cible, distance = cadrer(devant)

    scene = bpy.context.scene
    moteurs = [e.identifier for e in
               bpy.types.RenderSettings.bl_rna.properties["engine"].enum_items]
    scene.render.engine = ("BLENDER_EEVEE_NEXT" if "BLENDER_EEVEE_NEXT" in moteurs
                           else "BLENDER_EEVEE")
    scene.render.resolution_x = scene.render.resolution_y = SPEC["taille"]
    scene.render.fps = SPEC["fps"]
    format_avant = scene.render.image_settings.file_format
    scene.render.image_settings.file_format = "FFMPEG"
    scene.render.ffmpeg.format = "MPEG4"
    scene.render.ffmpeg.codec = "H264"
    scene.render.ffmpeg.constant_rate_factor = "MEDIUM"

    # 🚨 Les bandes NLA restent EN PLACE. On joue chaque clip en assignant son
    # action directement ; toucher aux bandes casserait le découpage que
    # l'export glTF lit.
    if arm.animation_data is None:
        arm.animation_data_create()
    for piste in arm.animation_data.nla_tracks:
        piste.mute = True
    cles = peau.data.shape_keys
    if cles.animation_data is not None:
        for piste in cles.animation_data.nla_tracks:
            piste.mute = True

    rendus = {}
    for nom_clip in ("idle", "walk", "run", "jump", "jog_backward",
                     "hit_react", "hammer_strike", "swim", "death"):
        action = bpy.data.actions.get(nom_clip)
        if action is None:
            rendus[nom_clip] = {"absent": True}
            continue
        debut, fin = action.frame_range
        arm.animation_data.action = action
        if cles.animation_data is None:
            cles.animation_data_create()
        cles.animation_data.action = action_de_cles(peau, nom_clip)

        scene.frame_start = int(math.floor(debut))
        scene.frame_end = int(math.ceil(fin))
        chemin = os.path.join(BAC, f"{nom_clip}.mp4")
        scene.render.filepath = os.path.join(BAC, nom_clip)
        bpy.ops.render.render(animation=True)

        # Blender suffixe la vidéo par la plage rendue : on retrouve le fichier
        # au lieu de le supposer.
        produits = [f for f in os.listdir(BAC)
                    if f.startswith(nom_clip) and f.endswith(".mp4")]
        rendus[nom_clip] = {
            "frames": scene.frame_end - scene.frame_start + 1,
            "fichiers": sorted(produits),
            "cligne": action_de_cles(peau, nom_clip) is not None,
        }

    # Le format revient à l'image fixe : laisser MPEG4 en place fait échouer
    # tout script de rendu lancé ensuite — c'est ce qui a cassé `32`.
    scene.render.image_settings.file_format = (
        format_avant if format_avant != "FFMPEG" else "PNG")

    # On laisse la scène jouable sur `idle`, barre Espace.
    idle = bpy.data.actions.get("idle")
    arm.animation_data.action = idle
    cles.animation_data.action = action_de_cles(peau, "idle")
    scene.frame_start = 1
    scene.frame_end = int(math.ceil(idle.frame_range[1])) if idle else 45
    scene.frame_set(1)

    print("RESULT: " + json.dumps({
        "dossier": BAC,
        "cible_camera": [round(c, 3) for c in cible],
        "distance_m": round(distance, 3),
        "rendus": rendus,
    }, ensure_ascii=False))


main()
