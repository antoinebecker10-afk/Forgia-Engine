"""Fusionne les 9 clips Mixamo sur le personnage et exporte un GLB animé.

    & "C:\\Program Files\\Blender Foundation\\Blender 4.5\\blender.exe" ^
      --background --factory-startup ^
      --python tools/blender/personnage/23_animer_personnage.py

PAS DE RETARGET. Mesuré sur le JSON brut des GLB : les clips portent le MÊME
squelette que le personnage (`Hips`, `LeftArm`, `LeftHandIndex1`…, nommage
Mixamo sans préfixe). Les courbes visent `pose.bones["<nom>"]` et s'appliquent
donc telles quelles. `retarget_mixamo.py` ne sert que quand les conventions
diffèrent — ce qui était le cas du Trooper (Mixamo → Unreal), pas ici.

TROIS PIÈGES, tous relevés à la mesure et pas devinés :

1. **Trois clips s'appellent `Armature|mixamo.com|Layer0`** — le nom d'export
   par défaut de Mixamo. Importés tels quels, le moteur verrait trois
   animations homonymes. On les nomme d'après leur FICHIER.
2. **L'importateur glTF fabrique une `Icosphere`** comme forme d'affichage des
   os. Elle n'est pas dans le fichier (vérifié sur le JSON brut) mais elle
   arrive à chaque import — trois fois qu'elle fausse une mesure ou pollue une
   sortie. On la supprime.
3. **Une action n'est exportée que si elle est sur une piste NLA.** En mode
   `ACTIONS`, seule l'action active sort : sur les animaux, `idle` et `death`
   manquaient à l'appel sans que rien ne le signale.
"""

import json
import os
import sys

import bpy

RACINE = r"C:\Users\Antoi\Desktop\Forgia Rewrite"
PERSONNAGE = os.path.join(RACINE, "assets", "models", "characters", "stylized",
                          "stylized_male.glb")
CLIPS = os.path.join(RACINE, "assets", "models", "characters", "stylized", "anims")
SORTIE = os.path.join(RACINE, "assets", "models", "characters", "stylized",
                      "stylized_male_anim.glb")


def journal(m):
    print(f"[perso] {m}", file=sys.stderr)


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
                 bpy.data.materials, bpy.data.images):
        for b in list(coll):
            try:
                coll.remove(b)
            except (RuntimeError, ReferenceError):
                pass


def purger_widgets(objets):
    """Retire les icosphères d'affichage d'os créées par l'importateur."""
    n = 0
    for o in list(objets):
        if o.type == "MESH" and o.name.lower().startswith(("icosphere", "icosphère")):
            bpy.data.objects.remove(o, do_unlink=True)
            n += 1
    return n


def main():
    if not os.path.exists(PERSONNAGE):
        print("RESULT: " + json.dumps({"erreur": f"absent : {PERSONNAGE}"}))
        return
    vider()

    # --- le personnage ---------------------------------------------------
    bpy.ops.import_scene.gltf(filepath=PERSONNAGE)
    perso = list(bpy.context.scene.objects)
    widgets = purger_widgets(perso)
    arm = next((o for o in bpy.context.scene.objects if o.type == "ARMATURE"), None)
    if arm is None:
        print("RESULT: " + json.dumps({"erreur": "aucune armature dans le personnage"}))
        return
    os_perso = {b.name for b in arm.pose.bones}

    # --- les clips -------------------------------------------------------
    rapport = []
    fichiers = sorted(f for f in os.listdir(CLIPS) if f.lower().endswith(".glb"))
    for fichier in fichiers:
        nom = os.path.splitext(fichier)[0]
        actions_avant = set(bpy.data.actions)
        objets_avant = set(bpy.context.scene.objects)
        bpy.ops.import_scene.gltf(filepath=os.path.join(CLIPS, fichier))
        neufs = [o for o in bpy.context.scene.objects if o not in objets_avant]
        actions_neuves = [a for a in bpy.data.actions if a not in actions_avant]

        if not actions_neuves:
            rapport.append({"clip": nom, "erreur": "aucune action"})
            for o in neufs:
                bpy.data.objects.remove(o, do_unlink=True)
            continue

        action = actions_neuves[0]
        # Le nom du FICHIER fait foi : trois clips portent le nom d'export par
        # defaut de Mixamo et seraient indiscernables en jeu.
        ancien = action.name
        action.name = nom
        action.use_fake_user = True

        # Combien d'os de cette action existent reellement sur le personnage ?
        vises = set()
        for fc in action.fcurves:
            dp = fc.data_path
            if dp.startswith('pose.bones["'):
                vises.add(dp.split('"')[1])
        connus = vises & os_perso

        rapport.append({
            "clip": nom, "nom_dans_le_fichier": ancien,
            "renomme": ancien != nom,
            "courbes": len(action.fcurves),
            "os_vises": len(vises), "os_reconnus": len(connus),
            "os_inconnus": sorted(vises - os_perso)[:6],
            "images": [round(action.frame_range[0], 1), round(action.frame_range[1], 1)],
        })
        journal(f"{nom} : {len(action.fcurves)} courbes, {len(connus)}/{len(vises)} os reconnus")

        # Le squelette et les widgets du clip ne servent plus : seule l'action
        # compte, et elle survit grace a `use_fake_user`.
        for o in neufs:
            bpy.data.objects.remove(o, do_unlink=True)

    # --- une piste NLA par clip -----------------------------------------
    arm.animation_data_create()
    arm.animation_data.action = None
    for piste in list(arm.animation_data.nla_tracks):
        arm.animation_data.nla_tracks.remove(piste)
    for r in rapport:
        if "erreur" in r:
            continue
        act = bpy.data.actions.get(r["clip"])
        if act is None:
            continue
        piste = arm.animation_data.nla_tracks.new()
        piste.name = act.name
        piste.strips.new(act.name, 1, act)

    purger_widgets(list(bpy.context.scene.objects))

    # UNIFIER LES MATIERES. L'importateur glTF cree un datablock par import :
    # on se retrouve avec `Armor` ET `Armor.001`, donc leurs textures en
    # double dans le GLB. Meme geste que sur la carte (`unifier_materiaux`),
    # meme raison : sans lui, le poids et les draw calls doublent en silence.
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
    for mat in list(bpy.data.materials):
        if mat.users == 0:
            bpy.data.materials.remove(mat)

    bpy.ops.object.select_all(action="SELECT")
    bpy.context.view_layer.objects.active = arm
    bpy.ops.export_scene.gltf(
        filepath=SORTIE, export_format="GLB", use_selection=True,
        export_yup=True, export_skins=True, export_materials="EXPORT",
        export_animations=True, export_animation_mode="NLA_TRACKS",
    )

    print("RESULT: " + json.dumps({
        "personnage": os.path.basename(PERSONNAGE),
        "os_personnage": len(os_perso),
        "widgets_purges": widgets,
        "materiaux_fusionnes": fusions,
        "materiaux_finaux": sorted(m.name for m in bpy.data.materials),
        "clips": rapport,
        "pistes": [t.name for t in arm.animation_data.nla_tracks],
        "glb_octets": os.path.getsize(SORTIE) if os.path.exists(SORTIE) else 0,
    }, ensure_ascii=False))


main()
