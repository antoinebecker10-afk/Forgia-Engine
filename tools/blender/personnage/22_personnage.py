"""Prépare le personnage : matières câblées, GLB propre, prêt à animer.

    & "C:\\Program Files\\Blender Foundation\\Blender 4.5\\blender.exe" ^
      --background --factory-startup ^
      --python tools/blender/personnage/22_personnage.py

Ce que ce script fait, et ce qu'il ne fait PAS.

IL FAIT : importer le corps rigué Mixamo (62 os, 1,80 m, nommage `Hips`/
`LeftArm` sans préfixe), lui câbler ses trois matières depuis les textures 1K,
et l'exporter en GLB avec son squelette et ses poids.

IL NE FAIT PAS d'animation. Le pack n'en livre aucune (0 action mesurée), et
il n'existe aucun clip Mixamo sur le disque. Les clips viendront de Mixamo —
c'est une étape manuelle (compte Adobe). Ce GLB est la base qu'ils animeront.

CHOIX ASSUMÉS
- **Normales OpenGL**, pas DirectX : c'est la convention glTF. Le pack livre
  les deux, elles ne diffèrent que par le signe du canal vert.
- **AO écarté** : glTF attend l'occlusion empaquetée dans le canal rouge d'une
  texture ORM. La câbler seule ne l'exporterait pas — autant ne pas mentir sur
  ce qui part.
- **Métallique écarté quand il est noir** : c'est déjà la valeur par défaut,
  l'emporter coûte une texture pour rien.
"""

import json
import os

import bpy
from mathutils import Vector

BASE = r"D:\ressources externes\FAB\fbx_stylizedfantasycharacters (1)"
SOURCE = os.path.join(BASE, "Mixamo", "SM_FantasyMale.fbx")
TEX = r"C:\Users\Antoi\Desktop\Forgia Rewrite\assets\textures\personnage_1k"
SORTIE = r"C:\Users\Antoi\Desktop\Forgia Rewrite\assets\models\characters\stylized"

MATIERES = {"Armor": "armor", "Cloth": "cloth", "Organik": "organik"}


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


def carte_utile(chemin, seuil=0.02):
    """Une carte quasi uniforme ne merite pas d'etre embarquee."""
    if not os.path.exists(chemin):
        return False
    return os.path.getsize(chemin) > seuil * 1e6


def cabler(mat, court):
    """Base color + normale + rugosité. Rien d'autre ne franchit le glTF ici."""
    mat.use_nodes = True
    nodes, links = mat.node_tree.nodes, mat.node_tree.links
    bsdf = next((n for n in nodes if n.type == "BSDF_PRINCIPLED"), None)
    if bsdf is None:
        bsdf = nodes.new("ShaderNodeBsdfPrincipled")
        sortie = next(n for n in nodes if n.type == "OUTPUT_MATERIAL")
        links.new(bsdf.outputs["BSDF"], sortie.inputs["Surface"])
    pose = []

    bc = os.path.join(TEX, f"{court}_bc.png")
    if os.path.exists(bc):
        t = nodes.new("ShaderNodeTexImage")
        t.image = bpy.data.images.load(bc, check_existing=True)
        links.new(t.outputs["Color"], bsdf.inputs["Base Color"])
        pose.append("base_color")

    nrm = os.path.join(TEX, f"{court}_n.png")
    if os.path.exists(nrm):
        t = nodes.new("ShaderNodeTexImage")
        t.image = bpy.data.images.load(nrm, check_existing=True)
        t.image.colorspace_settings.name = "Non-Color"
        nm = nodes.new("ShaderNodeNormalMap")
        links.new(t.outputs["Color"], nm.inputs["Color"])
        links.new(nm.outputs["Normal"], bsdf.inputs["Normal"])
        pose.append("normal")

    rgh = os.path.join(TEX, f"{court}_r.png")
    if carte_utile(rgh):
        t = nodes.new("ShaderNodeTexImage")
        t.image = bpy.data.images.load(rgh, check_existing=True)
        t.image.colorspace_settings.name = "Non-Color"
        links.new(t.outputs["Color"], bsdf.inputs["Roughness"])
        pose.append("roughness")
    else:
        bsdf.inputs["Roughness"].default_value = 0.75

    met = os.path.join(TEX, f"{court}_m.png")
    if carte_utile(met, seuil=0.05):
        t = nodes.new("ShaderNodeTexImage")
        t.image = bpy.data.images.load(met, check_existing=True)
        t.image.colorspace_settings.name = "Non-Color"
        links.new(t.outputs["Color"], bsdf.inputs["Metallic"])
        pose.append("metallic")
    else:
        bsdf.inputs["Metallic"].default_value = 0.0
    return pose


def main():
    if not os.path.exists(SOURCE):
        print("RESULT: " + json.dumps({"erreur": f"absent : {SOURCE}"}))
        return
    os.makedirs(SORTIE, exist_ok=True)
    vider()
    bpy.ops.import_scene.fbx(filepath=SOURCE)

    objets = list(bpy.context.scene.objects)
    arm = next((o for o in objets if o.type == "ARMATURE"), None)
    meshes = [o for o in objets if o.type == "MESH"]

    cablage = {}
    for mat in bpy.data.materials:
        court = MATIERES.get(mat.name.split(".")[0])
        if court:
            cablage[mat.name] = cabler(mat, court)

    lo = [1e9] * 3
    hi = [-1e9] * 3
    tris = 0
    for o in meshes:
        o.data.calc_loop_triangles()
        tris += len(o.data.loop_triangles)
        for c in o.bound_box:
            w = o.matrix_world @ Vector(c)
            for a in range(3):
                lo[a] = min(lo[a], w[a])
                hi[a] = max(hi[a], w[a])

    chemin = os.path.join(SORTIE, "stylized_male.glb")
    bpy.ops.object.select_all(action="SELECT")
    bpy.context.view_layer.objects.active = arm or (meshes[0] if meshes else None)
    bpy.ops.export_scene.gltf(
        filepath=chemin, export_format="GLB", use_selection=True,
        export_yup=True, export_skins=True, export_materials="EXPORT",
        export_image_format="AUTO", export_animations=True,
        export_animation_mode="NLA_TRACKS",
    )

    print("RESULT: " + json.dumps({
        "source": os.path.basename(SOURCE),
        "hauteur_m": round(hi[2] - lo[2], 3),
        "tris": tris,
        "os": len(arm.data.bones) if arm else 0,
        "racine_os": arm.data.bones[0].name if arm and arm.data.bones else None,
        "cablage": cablage,
        "actions": sorted(a.name for a in bpy.data.actions),
        "glb_octets": os.path.getsize(chemin) if os.path.exists(chemin) else 0,
        "glb": chemin,
    }, ensure_ascii=False))


main()
