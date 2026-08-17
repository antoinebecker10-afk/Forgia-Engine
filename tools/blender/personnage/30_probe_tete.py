"""Mesure la TÊTE de l'avatar d'Expédition, avant de sculpter quoi que ce soit.

    python tools/blender/bmcp.py code tools/blender/personnage/30_probe_tete.py

« Dans le même style graphique » n'est pas une intention, c'est une liste de
grandeurs : la taille de la tête par rapport au corps, sa densité de triangles,
sa matière, son lissage, et la position exacte de l'os `Head`. Sans ces six
nombres, une tête sculptée « au ressenti » arrive trop grosse, trop lisse ou
mal placée — et on l'ajuste ensuite à l'œil, trois fois de suite.

Ce script ne modifie RIEN dans le dépôt : il importe, mesure, imprime.
"""

import json
import os

import bpy

RACINE = r"C:\Users\Antoi\Desktop\Forgia Rewrite"
CORPS = os.path.join(RACINE, "assets", "models", "characters", "stylized",
                     "stylized_male_complet.glb")


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
                 bpy.data.materials, bpy.data.images, bpy.data.collections):
        for bloc in list(coll):
            try:
                coll.remove(bloc)
            except (RuntimeError, ReferenceError):
                pass


def boite(obj):
    """Emprise monde, en mètres."""
    pts = [obj.matrix_world @ v.co for v in obj.data.vertices]
    if not pts:
        return None
    return {
        "min": [round(min(p[i] for p in pts), 4) for i in range(3)],
        "max": [round(max(p[i] for p in pts), 4) for i in range(3)],
        "dim": [round(max(p[i] for p in pts) - min(p[i] for p in pts), 4)
                for i in range(3)],
    }


def materiau(mat):
    if mat is None:
        return None
    bsdf = next((n for n in mat.node_tree.nodes if n.type == "BSDF_PRINCIPLED"),
                None) if mat.use_nodes else None
    fiche = {"nom": mat.name}
    if bsdf is None:
        return fiche
    for entree in ("Base Color", "Roughness", "Metallic", "Specular IOR Level"):
        if entree not in bsdf.inputs:
            continue
        sock = bsdf.inputs[entree]
        if sock.is_linked:
            amont = sock.links[0].from_node
            fiche[entree] = f"<- {amont.type}" + (
                f" {os.path.basename(amont.image.filepath)}"
                if getattr(amont, "image", None) else "")
        else:
            val = sock.default_value
            fiche[entree] = ([round(v, 4) for v in val] if hasattr(val, "__len__")
                             else round(val, 4))
    return fiche


def main():
    vider()
    bpy.ops.import_scene.gltf(filepath=CORPS)

    maillages = [o for o in bpy.data.objects if o.type == "MESH"]
    armatures = [o for o in bpy.data.objects if o.type == "ARMATURE"]

    # Emprise du corps entier : c'est l'échelle de référence.
    tous = [p for o in maillages for p in
            (o.matrix_world @ v.co for v in o.data.vertices)]
    haut_corps = (max(p.z for p in tous) - min(p.z for p in tous)) if tous else 0.0

    tete = bpy.data.objects.get("SM_Head")
    fiche_tete = None
    if tete is not None:
        mesh = tete.data
        mesh.calc_loop_triangles()
        b = boite(tete)
        fiche_tete = {
            "sommets": len(mesh.vertices),
            "triangles": len(mesh.loop_triangles),
            "boite": b,
            # Un maillage lissé et un maillage à facettes ne se remplacent pas
            # par la même sculpture : le style tient beaucoup à ce booléen.
            "faces_lissees": sum(1 for p in mesh.polygons if p.use_smooth),
            "faces_total": len(mesh.polygons),
            "uv": [c.name for c in mesh.uv_layers],
            "couleurs_sommet": [c.name for c in mesh.color_attributes],
            "materiaux": [materiau(m) for m in mesh.materials],
            "groupes_os": len(tete.vertex_groups),
            # Une tête pesée sur 1 seul os se remplace sans repeser ; sur 4, non.
            "groupes_noms": sorted(g.name for g in tete.vertex_groups)[:12],
        }

    # L'os `Head` : le point d'ancrage de toute tête de remplacement.
    os_tete = {}
    for arm in armatures:
        for nom in ("Head", "HeadTop_End", "Neck", "Neck1"):
            bone = arm.pose.bones.get(nom)
            if bone is None:
                continue
            m = arm.matrix_world @ bone.matrix
            os_tete[f"{arm.name}/{nom}"] = {
                "tete_monde": [round(v, 4) for v in m.translation],
                "longueur": round(bone.bone.length, 4),
            }

    print("RESULT: " + json.dumps({
        "corps_hauteur_m": round(haut_corps, 4),
        "maillages": sorted(o.name for o in maillages),
        "tete": fiche_tete,
        "os": os_tete,
        # Rapport tête/corps : le marqueur de style le plus lisible. Un humain
        # réaliste est à ~1/7,5 ; un cartoon monte vers 1/5 ou 1/4.
        "rapport_tete_corps": (round(fiche_tete["boite"]["dim"][2] / haut_corps, 4)
                               if fiche_tete and haut_corps else None),
    }, ensure_ascii=False))


main()
