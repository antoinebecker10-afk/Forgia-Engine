"""Donne des cycles d'animation aux 7 animaux, puis les exporte en GLB.

    & "C:\\Program Files\\Blender Foundation\\Blender 4.5\\blender.exe" ^
      --background --factory-startup ^
      --python tools/blender/expedition/10_animaux.py

À lancer en instance SÉPARÉE : la scène du Vallon vit dans le Blender ouvert,
et ce script importe 7 personnages — il n'a rien à y faire.

POURQUOI CE SCRIPT EXISTE. `animals_free.blend` contient 7 animaux **riggés**
(33 os en moyenne) mais **zéro action**. Or le moteur joue des clips NOMMÉS
depuis le glTF (`reference_forgia_character_animation_model`). Sans clip, une
bête qui se déplace GLISSE — ce qui trahit immédiatement le décor.

Le nommage des os est régulier et rigify-like sur les 7 squelettes, ce qui
rend les cycles scriptables :

    quadrupèdes  thigh/shin/foot/toe .R/.L  +  front_* .R/.L
    poule        Wing[.00N].R/.L  +  thigh/shin
    manchot      fin[.00N].R/.L   +  thigh/shin/foot

HONNÊTETÉ SUR LE RÉSULTAT : ces cycles sont *corrects*, pas *beaux*. Une
démarche scriptée se lit bien à distance — le cas d'usage d'un décor animalier
— mais un cerf vu de près trahira l'absence d'animateur.
"""

import json
import math
import os
import sys

import bpy

SOURCE = r"D:\ressources externes\FAB\animals_free.blend"
SORTIE = r"C:\Users\Antoi\Desktop\Forgia Rewrite\assets\models\characters\animals"

# 24 images = 1 s à 24 i/s. Une foulée par seconde : c'est la cadence d'un
# animal qui flâne, et c'est ce que fera la déambulation côté moteur.
IMAGES = 24

# Amplitudes en degrés. Elles restent modestes : une démarche trop ample sur un
# rig qu'on n'a pas conçu part vite en pantin désarticulé.
QUADRUPEDE = {
    "cuisse": 22.0, "tibia": 16.0, "pied": 10.0,
    "tangage": 0.035,      # bob vertical, en unités du rig
    "echine": 3.0,
}
BIPEDE = {"cuisse": 26.0, "tibia": 18.0, "aile": 18.0, "tangage": 0.030}

# La phase de chaque membre. Un quadrupède marche en DIAGONALE : l'antérieur
# gauche part avec le postérieur droit. Mettre les quatre en phase donne un
# saut de lapin, l'erreur classique.
PHASES_QUAD = {
    "thigh.L": 0.0, "front_thigh.R": 0.0,
    "thigh.R": 0.5, "front_thigh.L": 0.5,
}


def journal(msg):
    print(f"[animaux] {msg}", file=sys.stderr)


def vider():
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)


def courbe(action, chemin_donnee, indice, cles):
    """Crée une f-courbe et y pose ses clés. On écrit l'action directement au
    lieu de passer par le mode pose : pas d'opérateur, donc pas de dépendance
    au contexte — indispensable en `--background`."""
    fc = action.fcurves.new(data_path=chemin_donnee, index=indice)
    fc.keyframe_points.add(count=len(cles))
    for i, (img, val) in enumerate(cles):
        pt = fc.keyframe_points[i]
        pt.co = (img, val)
        pt.interpolation = "BEZIER"
    fc.update()
    return fc


def balancier(amplitude_deg, phase, images=IMAGES):
    """Va-et-vient sinusoïdal sur un cycle, échantillonné en 5 clés.

    Cinq clés suffisent : extrême avant, milieu, extrême arrière, milieu, et le
    retour au point de départ — qui DOIT valoir la première, sinon le cycle
    saute à chaque bouclage."""
    a = math.radians(amplitude_deg)
    cles = []
    for k in range(5):
        t = k / 4.0
        img = 1 + t * images
        cles.append((img, a * math.sin(math.tau * (t + phase))))
    return cles


def os_present(arm_obj, nom):
    return nom in arm_obj.pose.bones


def cycle_marche(arm_obj, nom_action, gabarit, phases, ailes=None):
    action = bpy.data.actions.new(nom_action)
    action.use_fake_user = True          # sinon Blender la purge avant l'export
    pose = 0

    for os_nom, phase in phases.items():
        if not os_present(arm_obj, os_nom):
            continue
        arm_obj.pose.bones[os_nom].rotation_mode = "XYZ"
        courbe(action, f'pose.bones["{os_nom}"].rotation_euler', 0,
               balancier(gabarit["cuisse"], phase))
        pose += 1
        # Le segment suivant contre-balance, décalé d'un quart de cycle : c'est
        # ce retard qui donne le pli du genou au lieu d'une patte raide.
        for suffixe, cle in (("shin", "tibia"), ("foot", "pied")):
            enfant = os_nom.replace("thigh", suffixe)
            if os_present(arm_obj, enfant) and cle in gabarit:
                arm_obj.pose.bones[enfant].rotation_mode = "XYZ"
                courbe(action, f'pose.bones["{enfant}"].rotation_euler', 0,
                       balancier(-gabarit[cle], phase + 0.25))
                pose += 1

    for os_nom in (ailes or []):
        if os_present(arm_obj, os_nom):
            arm_obj.pose.bones[os_nom].rotation_mode = "XYZ"
            courbe(action, f'pose.bones["{os_nom}"].rotation_euler', 2,
                   balancier(gabarit.get("aile", 14.0), 0.0))
            pose += 1

    # Tangage du corps : deux oscillations par foulée (une par appui).
    if os_present(arm_obj, "Root"):
        amp = gabarit["tangage"]
        cles = [(1 + (k / 4.0) * IMAGES, amp * abs(math.sin(math.pi * (k / 4.0) * 2)))
                for k in range(5)]
        courbe(action, 'pose.bones["Root"].location', 2, cles)
        pose += 1

    return action, pose


def cycle_repos(arm_obj, nom_action):
    """Respiration : une seule oscillation lente de l'échine. Sans elle, un
    animal a l'arrêt est une statue, ce qui se remarque plus qu'on ne croit."""
    action = bpy.data.actions.new(nom_action)
    action.use_fake_user = True
    pose = 0
    echine = [b.name for b in arm_obj.pose.bones if b.name.startswith("spine")]
    for os_nom in echine[:3]:
        arm_obj.pose.bones[os_nom].rotation_mode = "XYZ"
        courbe(action, f'pose.bones["{os_nom}"].rotation_euler', 0,
               balancier(1.4, 0.0, images=IMAGES * 3))
        pose += 1
    return action, pose


def cycle_mort(arm_obj, nom_action):
    """Bascule sur le flanc en une demi-seconde. Le moteur y ajoutera son
    impulsion ; ici on ne fait que la pose finale, non bouclée."""
    action = bpy.data.actions.new(nom_action)
    action.use_fake_user = True
    pose = 0
    if os_present(arm_obj, "Root"):
        arm_obj.pose.bones["Root"].rotation_mode = "XYZ"
        courbe(action, 'pose.bones["Root"].rotation_euler', 1,
               [(1, 0.0), (12, math.radians(95.0))])
        courbe(action, 'pose.bones["Root"].location', 2,
               [(1, 0.0), (12, -0.12)])
        pose += 2
    return action, pose


def main():
    if not os.path.exists(SOURCE):
        print("RESULT: " + json.dumps({"erreur": f"absent : {SOURCE}"}))
        return
    os.makedirs(SORTIE, exist_ok=True)

    # Quels animaux contient la bibliotheque ? On lit la liste SANS importer,
    # pour pouvoir ensuite traiter chacun dans une scene vierge.
    with bpy.data.libraries.load(SOURCE, link=False) as (src, _dst):
        noms = [c for c in src.collections if c != "Assets"]

    rapport = []
    for nom_coll in noms:
        # UNE SCENE VIERGE PAR ANIMAL. Les 7 squelettes partagent leurs noms
        # d'os (thigh.R, spine.NNN...), donc l'exporteur glTF juge l'action de
        # la poule « compatible » avec le cerf et la fourre dans son GLB.
        # Mesure : le fichier du cerf contenait `chicken_walk`.
        vider()
        for coll in (bpy.data.actions, bpy.data.collections):
            for b in list(coll):
                try:
                    coll.remove(b)
                except (RuntimeError, ReferenceError):
                    pass

        avant = set(bpy.data.collections)
        with bpy.data.libraries.load(SOURCE, link=False) as (src, dst):
            dst.collections = [nom_coll]
        importees = [c for c in bpy.data.collections if c not in avant]
        if not importees:
            rapport.append({"animal": nom_coll, "erreur": "collection non chargee"})
            continue
        coll = importees[0]
        bpy.context.scene.collection.children.link(coll)

        arm = next((o for o in coll.all_objects if o.type == "ARMATURE"), None)
        if arm is None:
            rapport.append({"animal": nom_coll, "erreur": "aucune armature"})
            continue

        espece = nom_coll.split(".")[0].replace("_001", "")
        # Le gabarit se choisit sur les OS PRESENTS, pas sur une table de noms
        # ecrite a la main : un animal renomme casserait la table en silence.
        a_membres_avant = any(b.name.startswith("front_thigh") for b in arm.pose.bones)
        ailes = [b.name for b in arm.pose.bones
                 if b.name.startswith(("Wing", "fin")) and b.name.count(".") <= 1]
        gabarit = QUADRUPEDE if a_membres_avant else BIPEDE
        phases = dict(PHASES_QUAD) if a_membres_avant else {"thigh.L": 0.0, "thigh.R": 0.5}

        marche, n_m = cycle_marche(arm, f"{espece}_walk", gabarit, phases, ailes)
        repos, n_r = cycle_repos(arm, f"{espece}_idle")
        mort, n_d = cycle_mort(arm, f"{espece}_death")

        # UNE PISTE NLA PAR CLIP. En mode ACTIONS l'exporteur n'avait sorti que
        # l'action active : `idle` et `death` manquaient a l'appel. Les pistes
        # NLA sont la voie explicite — ce qui est sur une piste sort, point.
        arm.animation_data_create()
        arm.animation_data.action = None
        for act in (marche, repos, mort):
            piste = arm.animation_data.nla_tracks.new()
            piste.name = act.name
            piste.strips.new(act.name, 1, act)

        # MENAGE AVANT EXPORT — par la LISTE BLANCHE, pas par exclusion.
        # Le fichier source embarque une icosphere par collection (placeholder
        # d'auteur). Un filtre « ce qui n'a pas de modificateur » l'a laissee
        # passer (parasites_retires = 0) et 30 boules de 2 m se sont retrouvees
        # posees dans la carte. On garde donc UNIQUEMENT ce qui est deforme par
        # l'armature, et on supprime tout le reste : aucune ambiguite possible.
        corps = [o for o in bpy.data.objects
                 if o.type == "MESH" and any(m.type == "ARMATURE" for m in o.modifiers)]
        retires = 0
        # A l'echelle de la SCENE, pas de la collection : l'icosphere n'y etait
        # pas (parasites_retires restait a 0) — elle est enfant de l'armature
        # dans une autre collection, et l'exporteur l'emporte avec la
        # hierarchie. Chaque animal etant traite dans une scene vierge, tout
        # mesh non deforme par l'armature est un intrus, ou qu'il vive.
        for o in list(bpy.data.objects):
            if o.type == "MESH" and o not in corps:
                bpy.data.objects.remove(o, do_unlink=True)
                retires += 1
        for k, o in enumerate(corps):
            o.name = espece if k == 0 else f"{espece}_{k}"
            o.data.name = o.name

        chemin = os.path.join(SORTIE, f"{espece}.glb")
        bpy.ops.object.select_all(action="DESELECT")
        for o in coll.all_objects:
            o.select_set(True)
        bpy.context.view_layer.objects.active = arm
        bpy.ops.export_scene.gltf(
            filepath=chemin, export_format="GLB", use_selection=True,
            export_yup=True, export_animations=True,
            export_animation_mode="NLA_TRACKS",
            export_skins=True, export_materials="EXPORT",
        )
        rapport.append({
            "animal": espece,
            "os": len(arm.pose.bones),
            "gabarit": "quadrupede" if a_membres_avant else "bipede",
            "ailes": ailes,
            "courbes": {"walk": n_m, "idle": n_r, "death": n_d},
            "pistes": [t.name for t in arm.animation_data.nla_tracks],
            "parasites_retires": retires,
            "corps": [o.name for o in corps],
            "octets": os.path.getsize(chemin) if os.path.exists(chemin) else 0,
        })
        journal(f"{espece} : {n_m} courbes de marche, 3 pistes, GLB ecrit")

    print("RESULT: " + json.dumps({"sortie": SORTIE, "animaux": rapport},
                                  ensure_ascii=False))


main()
