"""Ouvre un ATELIER personnage dans la session Blender courante.

    python tools/blender/bmcp.py code tools/blender/personnage/25_atelier_personnage.py

⚠️ Ce script REMPLACE la scène. Si la carte du Vallon s'y trouve, elle part —
sans conséquence : `vallon.py` la rebâtit d'une commande, à l'identique
(même graine).

CE QU'IL MONTE
- le personnage animé (9 clips, 62 os) ;
- la **cape UE** attachée au haut du dos. On prend la version UE et non la
  Mixamo : mêmes 748 triangles, mais ses os s'appellent `cloak_01`…`cloak_06`
  au lieu de `Bone.003`. Pour y brancher une physique plus tard, des noms
  valent mieux qu'une numérotation anonyme ;
- la **dague** socketée à la main droite.

CHAQUE PIÈCE DANS SA COLLECTION, pour qu'on puisse l'éteindre et la rallumer
d'un clic pendant qu'on personnalise.

L'ATTACHEMENT SANS OPÉRATEUR. On pose `parent` / `parent_type` / `parent_bone`
puis on RÉ-ÉCRIT `matrix_world` avec la valeur relevée avant. Blender en déduit
la matrice locale. `parent_set` exigerait un contexte de vue 3D — fragile à
travers une socket, et inutile ici.
"""

import json
import math
import os

import bpy
from mathutils import Vector

RACINE = r"C:\Users\Antoi\Desktop\Forgia Rewrite"
FAB = r"D:\ressources externes\FAB\fbx_stylizedfantasycharacters (1)"
PERSONNAGE = os.path.join(RACINE, "assets", "models", "characters", "stylized",
                          "stylized_male_anim.glb")
CAPE = os.path.join(FAB, "UE", "SM_StylizedMale_Cloak_UE.fbx")
DAGUE = os.path.join(FAB, "UE", "SM_StylizedFemale_Dagger_UE.fbx")

# Os d'accroche. Mixamo nomme la colonne `Spine`/`Spine1`/`Spine2`.
OS_CAPE = ["Spine2", "Spine1", "Spine", "Hips"]      # par ordre de préférence
OS_DAGUE = ["RightHand", "RightForeArm", "Hips"]


def vider():
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)
    for coll in (bpy.data.meshes, bpy.data.armatures, bpy.data.actions,
                 bpy.data.materials, bpy.data.images, bpy.data.collections):
        for b in list(coll):
            try:
                coll.remove(b)
            except (RuntimeError, ReferenceError):
                pass


def collection(nom):
    c = bpy.data.collections.get(nom)
    if c is None:
        c = bpy.data.collections.new(nom)
        bpy.context.scene.collection.children.link(c)
    return c


def ranger(objets, nom_coll):
    c = collection(nom_coll)
    for o in objets:
        for autre in list(o.users_collection):
            autre.objects.unlink(o)
        c.objects.link(o)
    return c


def purger_widgets(objets):
    """Retire les icospheres d'affichage d'os ET rend les survivants.

    Elle DOIT filtrer la liste elle-meme : garder une reference vers un objet
    supprime puis lire son `.name` leve « StructRNA of type Object has been
    removed ». On ne relit jamais une liste apres une suppression, on la
    reconstruit pendant.
    """
    gardes = []
    for o in objets:
        if o.type == "MESH" and o.name.lower().startswith(("icosphere", "icosphère")):
            bpy.data.objects.remove(o, do_unlink=True)
        else:
            gardes.append(o)
    return gardes


def premier_os(arm, candidats):
    for nom in candidats:
        if nom in arm.pose.bones:
            return nom
    return None


def attacher(obj, arm, os_nom):
    """Parente `obj` à un os SANS opérateur, en conservant sa pose visuelle."""
    garde = obj.matrix_world.copy()
    obj.parent = arm
    obj.parent_type = "BONE"
    obj.parent_bone = os_nom
    bpy.context.view_layer.update()
    obj.matrix_world = garde
    return os_nom


def importer(chemin):
    avant = set(bpy.context.scene.objects)
    if chemin.lower().endswith(".fbx"):
        bpy.ops.import_scene.fbx(filepath=chemin)
    else:
        bpy.ops.import_scene.gltf(filepath=chemin)
    return [o for o in bpy.context.scene.objects if o not in avant]


def main():
    vider()
    rapport = {}

    # --- le personnage ---------------------------------------------------
    perso = purger_widgets(importer(PERSONNAGE))
    arm = next((o for o in perso if o.type == "ARMATURE"), None)
    if arm is None:
        print("RESULT: " + json.dumps({"erreur": "aucune armature"}))
        return
    arm.name = "perso_squelette"
    # Chaque partie du corps dans sa collection : on personnalise en
    # allumant/eteignant, pas en cherchant dans une liste de 60 objets.
    for o in [x for x in perso if x.type == "MESH"]:
        ranger([o], "perso_" + o.name.replace("SM_", "").lower())
    ranger([arm], "perso_squelette")
    rapport["personnage"] = {
        "os": len(arm.pose.bones),
        "clips": sorted(a.name for a in bpy.data.actions),
        "parties": sorted(o.name for o in perso if o.type == "MESH"),
    }

    # --- POSE DE REPOS. Les 9 pistes NLA s'appliquent des l'ouverture : selon
    # l'image courante, le personnage se retrouve en pleine chute de `death`.
    # Un atelier de personnalisation se regarde debout. On coupe les pistes ;
    # elles se rallument d'un clic dans l'editeur NLA.
    if arm.animation_data:
        for piste in arm.animation_data.nla_tracks:
            piste.mute = True
        arm.animation_data.action = None
    bpy.context.scene.frame_set(1)
    # Couper les pistes NE SUFFIT PAS : la pose est une donnee stockee sur
    # chaque os, elle garde sa derniere valeur evaluee. Sans cette remise a
    # zero, le personnage reste affale dans la pose ou `death` l'avait laisse.
    for pb in arm.pose.bones:
        pb.location = (0.0, 0.0, 0.0)
        pb.scale = (1.0, 1.0, 1.0)
        if pb.rotation_mode == "QUATERNION":
            pb.rotation_quaternion = (1.0, 0.0, 0.0, 0.0)
        else:
            pb.rotation_euler = (0.0, 0.0, 0.0)
    bpy.context.view_layer.update()

    # L'ORDRE COMPTE : la remise au repos precede l'attachement des
    # accessoires. Attacher d'abord, c'est figer la cape sur la position que
    # `Spine2` occupait dans la pose de `death` — elle suivait ensuite l'os
    # jusqu'au repos et se retrouvait par terre, derriere les talons.

    # --- la cape ---------------------------------------------------------
    if os.path.exists(CAPE):
        pieces = importer(CAPE)
        cape_arm = next((o for o in pieces if o.type == "ARMATURE"), None)
        cible = premier_os(arm, OS_CAPE)
        if cape_arm and cible:
            attacher(cape_arm, arm, cible)
        ranger(pieces, "cape")
        rapport["cape"] = {
            "objets": [o.name for o in pieces],
            "os_cape": sorted(b.name for b in cape_arm.pose.bones) if cape_arm else [],
            "accrochee_a": cible,
        }

    # --- la dague --------------------------------------------------------
    if os.path.exists(DAGUE):
        pieces = importer(DAGUE)
        cible = premier_os(arm, OS_DAGUE)
        for o in pieces:
            if o.type == "MESH" and cible:
                attacher(o, arm, cible)
        ranger(pieces, "dague")
        rapport["dague"] = {
            "objets": [o.name for o in pieces],
            "accrochee_a": cible,
        }

    # --- l'atelier : lumière, caméra, fond -------------------------------
    if "AtelierSoleil" not in bpy.data.objects:
        sd = bpy.data.lights.new("AtelierSoleil", type="SUN")
        sd.energy = 3.4
        sd.angle = math.radians(3.0)
        sun = bpy.data.objects.new("AtelierSoleil", sd)
        sun.rotation_euler = (math.radians(56.0), 0.0, math.radians(35.0))
        ranger([sun], "atelier")
    monde = bpy.context.scene.world or bpy.data.worlds.new("Monde")
    bpy.context.scene.world = monde
    monde.use_nodes = True
    fond = monde.node_tree.nodes.get("Background")
    if fond:
        fond.inputs[0].default_value = (0.30, 0.33, 0.40, 1.0)
        fond.inputs[1].default_value = 1.0
    bpy.context.scene.render.engine = "BLENDER_EEVEE_NEXT"

    # --- MATIERES UNIFIEES ET TEXTUREES ---------------------------------
    # Les imports FBX de la cape et de la dague apportent leurs propres
    # datablocks (`Cloth.002`…) SANS texture : d'ou la cape magenta. On les
    # rabat sur les matieres deja cablees du personnage.
    canon = {}
    for mat in sorted(bpy.data.materials, key=lambda m: m.name):
        canon.setdefault(mat.name.split(".")[0], mat)
    fusions = 0
    for mesh in bpy.data.meshes:
        for i, mat in enumerate(mesh.materials):
            if mat is None:
                continue
            propre = canon[mat.name.split(".")[0]]
            if propre is not mat:
                mesh.materials[i] = propre
                fusions += 1
    # La cape est en tissu, la dague en metal : on les rattache explicitement
    # plutot que d'esperer que leurs noms coincident.
    for nom_mesh, nom_mat in (("Cloak_low", "Cloth"), ("SM_Dagger", "Armor")):
        m = bpy.data.meshes.get(nom_mesh)
        cible = canon.get(nom_mat)
        if m and cible:
            m.materials.clear()
            m.materials.append(cible)
            fusions += 1
    for mat in list(bpy.data.materials):
        if mat.users == 0:
            bpy.data.materials.remove(mat)
    rapport["materiaux_fusionnes"] = fusions

    rapport["os_disponibles"] = sorted(b.name for b in arm.pose.bones)
    rapport["materiaux"] = sorted(m.name for m in bpy.data.materials)
    rapport["collections"] = sorted(c.name for c in bpy.data.collections)
    print("RESULT: " + json.dumps(rapport, ensure_ascii=False))


main()
