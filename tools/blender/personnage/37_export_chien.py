"""Exporte le personnage à tête de chien, prêt à remplacer l'avatar.

    python tools/blender/bmcp.py code tools/blender/personnage/37_export_chien.py

À lancer APRÈS `36_rig_chien.py`.

CE QUI PART, ET CE QUI RESTE

Le génome `[expedition_body].model` désigne UN fichier qui porte le personnage
entier. On exporte donc le corps ET la tête de chien — pas la tête seule.

    part      SM_Body · SM_Legs · SM_Bag · SM_Flask · SM_Dagger · Cloak_low
              chien_peau · chien_yeux · chien_museau
              perso_squelette (66 os) · root.001 (6 os de cape)
    reste     SM_Head — remplacée, c'est tout l'objet de l'opération
              Icosphère* — des widgets d'affichage d'os qui ont fuité dans
              l'export d'origine et qui n'ont rien à faire dans un GLB

LA POSE EST REMISE À ZÉRO AVANT DE PARTIR. Les essais de rig laissent les
oreilles tournées et la paupière fermée ; exporter là-dessus figerait la pose
d'essai dans le fichier livré. Un état d'atelier ne doit jamais franchir la
porte — c'est la même règle que « l'artefact est une preuve, pas la source ».
"""

import json
import os

import bpy

RACINE = r"C:\Users\Antoi\Desktop\Forgia Rewrite"
SORTIE = os.path.join(RACINE, "assets", "models", "characters", "stylized",
                      "chien_expedition.glb")

EXCLUS = ("SM_Head", "Icosph")
CHIEN = ("chien_peau", "chien_yeux", "chien_museau")
#: Le contrat que le fichier remplacé fournissait au moteur. Vérifié à chaque
#: export : leur absence est une erreur, pas un détail.
SOCKETS = ("socket_main_droite", "socket_dos",
           "socket_pied_gauche", "socket_pied_droit")


#: Les seuls os que cette chaîne pose. Tout le reste appartient à l'import.
NOTRE_RIG = ("oreille_", "oeil_", "machoire")


def remettre_au_repos():
    """Annule ce que NOS essais ont laissé — et rien d'autre.

    🚨 Première version : elle remettait TOUS les os de TOUTES les armatures à
    l'identité, échelle comprise. Or l'import glTF avait laissé une échelle de
    0,01 dans la pose des six os de cape ; les forcer à 1,0 a multiplié la cape
    par CENT — mesuré, elle s'étalait de z 43 à z 142, et elle est partie comme
    ça dans le GLB livré.

    La leçon dépasse ce script : **remettre une pose à l'identité n'est pas la
    remettre au repos**. L'identité n'est le repos que si personne n'a rien
    cuit dans la pose — hypothèse fausse dès qu'un fichier vient d'ailleurs. On
    ne touche donc qu'aux os dont on est l'auteur."""
    remis = {"os": 0, "cles": 0, "ignores": 0}
    for obj in bpy.context.scene.objects:
        if obj.type == "ARMATURE":
            for b in obj.pose.bones:
                if not b.name.startswith(NOTRE_RIG):
                    remis["ignores"] += 1
                    continue
                b.location = (0.0, 0.0, 0.0)
                b.rotation_quaternion = (1.0, 0.0, 0.0, 0.0)
                b.rotation_euler = (0.0, 0.0, 0.0)
                b.scale = (1.0, 1.0, 1.0)
                remis["os"] += 1
        elif obj.type == "MESH" and obj.data.shape_keys:
            for k in obj.data.shape_keys.key_blocks:
                if k.name != "Basis" and k.value != 0.0:
                    k.value = 0.0
                    remis["cles"] += 1
    bpy.context.view_layer.update()
    return remis


def main():
    remis = remettre_au_repos()

    # 🚨 Les EMPTY font partie du contrat. Les quatre sockets nommés
    # (`socket_main_droite`, `socket_dos`, `socket_pied_gauche/droit`) sont des
    # objets vides parentés à des os — ni maillages, ni armatures. Un filtre
    # `type in {MESH, ARMATURE}` les jette EN SILENCE, et le GLB livré n'en avait
    # aucun : l'arme en main (`arme_main.rs`) et les feux aux pieds
    # (`avatar_vfx.rs`) n'auraient plus rien où s'accrocher. Un remplacement doit
    # rendre TOUT ce que le fichier remplacé fournissait, pas seulement ce qui
    # saute aux yeux.
    a_exporter = []
    for obj in bpy.context.scene.objects:
        if obj.name.startswith(EXCLUS):
            continue
        if obj.type in {"MESH", "ARMATURE", "EMPTY"}:
            a_exporter.append(obj)

    manquants = [n for n in CHIEN if n not in {o.name for o in a_exporter}]
    manquants += [n for n in SOCKETS if n not in {o.name for o in a_exporter}]
    if manquants:
        print("RESULT: " + json.dumps(
            {"erreur": "pièces absentes", "manquants": manquants}))
        return

    # Désélection SANS opérateur : `bpy.ops.object.select_all` a un `poll` qui
    # dépend du contexte et échoue selon ce que le script précédent a laissé
    # ouvert (« context is incorrect »). Une boucle n'a pas de poll.
    for obj in bpy.context.view_layer.objects:
        obj.select_set(False)
    for obj in a_exporter:
        # 🚨 Un objet MASQUÉ ne se sélectionne pas, et sortirait donc du fichier
        # sans un mot. `33_montrer_chien.py` masque justement les armatures.
        obj.hide_set(False)
        obj.select_set(True)
    bpy.context.view_layer.objects.active = a_exporter[0]

    # 🚨 Le contrôle d'emprise passe AVANT l'écriture, et il REFUSE. Posé après,
    # il regardait partir ce qu'il venait de voir : la cape à cent fois sa
    # taille est sortie une première fois dans le fichier livré pendant que le
    # rapport la signalait. Un capteur qui constate un défaut sortant n'est pas
    # un capteur, c'est un procès-verbal.
    emprises = {}
    for obj in a_exporter:
        if obj.type != "MESH" or not obj.data.vertices:
            continue
        zs = [(obj.matrix_world @ v.co).z for v in obj.data.vertices]
        emprises[obj.name] = [round(min(zs), 2), round(max(zs), 2)]
    hors_boite = {k: v for k, v in emprises.items() if v[1] > 3.0 or v[0] < -0.5}
    if hors_boite:
        print("RESULT: " + json.dumps({
            "erreur": "maillage hors de la boîte du personnage — export refusé",
                "remede": "rejouer la chaîne depuis 32_tete_chien.py : "
                      "une pose a été écrasée en cours de route",
        }, ensure_ascii=False))
        return

    bpy.ops.export_scene.gltf(
        filepath=SORTIE, export_format="GLB", use_selection=True,
        export_yup=True, export_skins=True, export_materials="EXPORT",
        export_morph=True, export_animations=True,
        export_animation_mode="NLA_TRACKS", export_image_format="AUTO",
    )

    print("RESULT: " + json.dumps({
        "remis_au_repos": remis,
        "emprises_z": emprises,
        "exportes": sorted(o.name for o in a_exporter),
        "sockets": sorted(o.name for o in a_exporter if o.type == "EMPTY"),
        "exclus": sorted(o.name for o in bpy.context.scene.objects
                         if o.name.startswith(EXCLUS)),
        "fichier": SORTIE,
        "octets": os.path.getsize(SORTIE) if os.path.exists(SORTIE) else 0,
    }, ensure_ascii=False))


main()
