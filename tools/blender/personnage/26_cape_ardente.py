"""Cape ardente + sockets de flammes aux pieds, puis export du personnage complet.

    python tools/blender/bmcp.py code tools/blender/personnage/26_cape_ardente.py

À lancer APRÈS `25_atelier_personnage.py`, sur l'atelier qu'il a monté.

CE QUI S'EXPORTE, ET CE QUI NE S'EXPORTE PAS

Un système de particules Blender ne franchit pas le glTF — même mur que le
shader triplanaire du terrain. Les flammes seront donc faites par `bevy_hanabi`
côté moteur. Ce script prépare les TROIS choses que le glTF sait transporter et
sans lesquelles le moteur n'aurait rien à quoi accrocher son feu :

1. **Un ourlet émissif.** glTF transporte `emissive` et
   `KHR_materials_emissive_strength`. La cape reçoit une SECONDE matière sur
   son bas — pas un dégradé de shader, qui ne passerait pas, mais une
   affectation de faces. La frontière est **brouillée par du bruit**, exactement
   comme la limite herbe/roche du Vallon : une coupure nette se lirait comme un
   ruban peint, un bord dentelé se lit comme du feu qui lèche.

2. **Des sockets nommés aux pieds.** Des `Empty` parentés à `LeftFoot` et
   `RightFoot` : ils sortent en nœuds glTF, et le moteur les retrouve PAR NOM
   pour y ancrer sa traînée. Sans eux il devrait deviner une position d'os.

3. **Une couleur de sommet en masque.** `COLOR_0` s'exporte : elle dit au
   moteur OÙ le feu mord le plus fort, au lieu d'embraser uniformément.

DISCRÉTION VOULUE. `part_ardente` 0,28 : seul le quart bas de la cape rougeoie,
et `emissive_force` reste à 1,6 — assez pour se voir de nuit, pas assez pour
transformer le personnage en torche.
"""

import json
import math
import os

import bpy
import mathutils
from mathutils import Vector

RACINE = r"C:\Users\Antoi\Desktop\Forgia Rewrite"
SORTIE = os.path.join(RACINE, "assets", "models", "characters", "stylized",
                      "stylized_male_complet.glb")
TEX = os.path.join(RACINE, "assets", "textures", "personnage_1k")

SPEC = {
    # Fraction BASSE de la cape qui rougeoie. 0,28 = l'ourlet seulement.
    "part_ardente": 0.28,
    # Amplitude du brouillage de la frontière, en fraction de hauteur de cape.
    # Sans lui, la limite est une ligne horizontale nette — un ruban, pas du feu.
    "flou": 0.09,
    "bruit_echelle": 14.0,
    "emissive": "#FF6420",
    "emissive_force": 1.6,
    # Os porteurs des sockets. Le moteur les retrouve par le NOM du socket.
    "sockets": {
        "socket_pied_gauche": "LeftFoot",
        "socket_pied_droit": "RightFoot",
        "socket_main_droite": "RightHand",
        "socket_dos": "Spine2",
    },
}


def srgb_lineaire(c):
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def hex_lineaire(code):
    code = code.lstrip("#")
    return tuple(srgb_lineaire(int(code[i:i + 2], 16) / 255.0) for i in (0, 2, 4)) + (1.0,)


def materiau_ardent(source):
    """Copie du tissu, avec émissif. On COPIE au lieu de modifier : la chemise
    du personnage partage la matière `Cloth`, et la faire rougeoyer aussi
    transformerait le torse en lanterne."""
    mat = source.copy() if source else bpy.data.materials.new("Cloth_ardent")
    mat.name = "Cloth_ardent"
    mat.use_nodes = True
    bsdf = next((n for n in mat.node_tree.nodes if n.type == "BSDF_PRINCIPLED"), None)
    if bsdf is None:
        return mat
    couleur = hex_lineaire(SPEC["emissive"])
    for entree, valeur in (("Emission Color", couleur),
                           ("Emission Strength", SPEC["emissive_force"])):
        if entree in bsdf.inputs:
            bsdf.inputs[entree].default_value = valeur
    return mat


def main():
    cape = bpy.data.objects.get("Cloak_low")
    arm = bpy.data.objects.get("perso_squelette")
    if cape is None or arm is None:
        print("RESULT: " + json.dumps(
            {"erreur": "atelier absent — lancer 25_atelier_personnage.py d'abord"}))
        return

    mesh = cape.data
    # --- 1. l'ourlet ardent ---------------------------------------------
    base = mesh.materials[0] if mesh.materials else None
    ardent = materiau_ardent(base)
    if ardent.name not in [m.name for m in mesh.materials]:
        mesh.materials.append(ardent)
    idx_ardent = len(mesh.materials) - 1

    zs = [(cape.matrix_world @ v.co).z for v in mesh.vertices]
    z_bas, z_haut = min(zs), max(zs)
    hauteur = max(1e-6, z_haut - z_bas)
    seuil = z_bas + hauteur * SPEC["part_ardente"]

    faces_ardentes = 0
    for poly in mesh.polygons:
        c = cape.matrix_world @ poly.center
        # Frontière dentelée : la même recette que la limite herbe/roche.
        bruit = mathutils.noise.noise(Vector((c.x * SPEC["bruit_echelle"],
                                              c.z * SPEC["bruit_echelle"], 3.0)))
        if c.z < seuil + bruit * hauteur * SPEC["flou"]:
            poly.material_index = idx_ardent
            faces_ardentes += 1

    # --- 2. le masque par couleurs de sommet -----------------------------
    couche = mesh.color_attributes.get("Col")
    if couche is None:
        couche = mesh.color_attributes.new(name="Col", type="FLOAT_COLOR", domain="POINT")
    for i, v in enumerate(mesh.vertices):
        z = (cape.matrix_world @ v.co).z
        t = max(0.0, min(1.0, (seuil - z) / (hauteur * SPEC["part_ardente"])))
        # Rouge = intensité du feu. Le moteur y lit OÙ ça brûle.
        couche.data[i].color = (t, 1.0 - t * 0.4, 1.0 - t * 0.8, 1.0)

    # --- 3. les sockets --------------------------------------------------
    poses = {}
    for nom, os_nom in SPEC["sockets"].items():
        if os_nom not in arm.pose.bones:
            poses[nom] = None
            continue
        vide = bpy.data.objects.get(nom)
        if vide is None:
            vide = bpy.data.objects.new(nom, None)
            vide.empty_display_type = "PLAIN_AXES"
            vide.empty_display_size = 0.08
            bpy.context.scene.collection.objects.link(vide)
        vide.parent = arm
        vide.parent_type = "BONE"
        vide.parent_bone = os_nom
        # Sans reset, l'empty herite d'une transformation locale arbitraire.
        vide.location = (0.0, 0.0, 0.0)
        vide.rotation_euler = (0.0, 0.0, 0.0)
        poses[nom] = os_nom
    bpy.context.view_layer.update()

    # --- 4. export du personnage COMPLET ---------------------------------
    # Le GLB livre precedemment n'avait ni cape ni dague. Celui-ci porte tout.
    bpy.ops.object.select_all(action="DESELECT")
    exclues = {"atelier"}
    exportes = []
    for coll in bpy.data.collections:
        if coll.name in exclues or coll.name.startswith("glTF"):
            continue
        for o in coll.objects:
            o.select_set(True)
            exportes.append(o.name)
    for nom in SPEC["sockets"]:
        o = bpy.data.objects.get(nom)
        if o:
            o.select_set(True)
            if o.name not in exportes:
                exportes.append(o.name)
    bpy.context.view_layer.objects.active = arm

    bpy.ops.export_scene.gltf(
        filepath=SORTIE, export_format="GLB", use_selection=True,
        export_yup=True, export_skins=True, export_materials="EXPORT",
        export_animations=True, export_animation_mode="NLA_TRACKS",
        export_image_format="AUTO",
    )

    print("RESULT: " + json.dumps({
        "cape_hauteur_m": round(hauteur, 3),
        "faces_ardentes": faces_ardentes,
        "faces_cape": len(mesh.polygons),
        "part_reelle": round(faces_ardentes / max(1, len(mesh.polygons)), 3),
        "sockets": poses,
        "objets_exportes": sorted(exportes),
        "materiaux": sorted(m.name for m in bpy.data.materials),
        "glb_octets": os.path.getsize(SORTIE) if os.path.exists(SORTIE) else 0,
    }, ensure_ascii=False))


main()
