"""Rend sa tête à l'avatar, et fait bouger ses CHEVEUX quand il court.

    python tools/blender/bmcp.py code tools/blender/personnage/40_cheveux_avatar.py

Repart du corps d'origine — celui à tête humaine — et n'y ajoute qu'une chose :
deux os dans la masse de cheveux, animés dans les clips existants.

POURQUOI DEUX OS, ET PAS UN NI SIX

Un seul os fait pivoter toute la chevelure d'un bloc : ça se lit comme un
chapeau qui glisse. Six donneraient une simulation qu'aucun de ces clips ne
justifie. Deux suffisent à produire le seul effet qui compte — la POINTE arrive
en retard sur la RACINE. C'est ce décalage, et lui seul, qui fait qu'une matière
a l'air souple.

CE QU'ON NE FAIT PAS

Pas de physique, pas de ressort à l'exécution, pas de nouveau clip. Le mouvement
est cuit DANS les neuf actions existantes, comme pour les oreilles du chien :
le moteur joue « run » et les cheveux suivent, sans qu'une ligne de Rust ait à
le savoir.

LA MASSE DE CHEVEUX SE MESURE, ELLE NE SE DÉSIGNE PAS

`SM_Head` est un seul maillage, une seule matière : rien n'y distingue le visage
de la chevelure. On la trouve par la HAUTEUR — au-dessus de la ligne de sourcils,
c'est du cheveu — et le poids monte en fondu jusqu'aux pointes. Un poids franc
ferait plier le crâne avec la mèche.
"""

import json
import math
import os

import bpy
from mathutils import Vector

RACINE = r"C:\Users\Antoi\Desktop\Forgia Rewrite"
SOURCE = os.path.join(RACINE, "assets", "models", "characters", "stylized",
                      "stylized_male_complet.glb")
SORTIE = os.path.join(RACINE, "assets", "models", "characters", "stylized",
                      "stylized_male_cheveux.glb")

SPEC = {
    # Où commence le cheveu, en fraction de la hauteur de tête depuis le bas.
    # 0,62 tombe juste au-dessus de la ligne de sourcils, mesurée sur la boîte
    # de `SM_Head` (1,479 → 1,802 m).
    "depart_cheveux": 0.62,
    "part_premier": 0.55,        # partage racine/pointe entre les deux os

    # Amplitudes en degrés : (amplitude, harmonique, phase). L'harmonique
    # compte les cycles sur la durée du clip.
    #
    # La course est le cas qui compte — c'est là que des cheveux figés se
    # remarquent. L'idle garde un souffle, la marche un balancement discret.
    "etats": {
        "idle": {"tangage": (2.2, 1, 0.0), "roulis": (1.4, 1, 0.3)},
        "walk": {"tangage": (5.0, 2, 0.0), "roulis": (2.6, 1, 0.25)},
        "run": {"tangage": (11.0, 2, 0.0), "roulis": (5.0, 2, 0.35)},
        "jog_backward": {"tangage": (-6.5, 2, 0.0), "roulis": (3.0, 2, 0.3)},
        "swim": {"tangage": (3.0, 4, 0.0), "roulis": (2.0, 3, 0.2)},
        # Les ponctuels : des courbes, pas des cycles.
        "jump": {"tangage_courbe": [(0.0, 0.0), (0.18, -13.0), (0.5, 6.0),
                                    (0.85, -8.0), (1.0, 0.0)]},
        "hit_react": {"tangage_courbe": [(0.0, 0.0), (0.1, 15.0), (0.45, -6.0),
                                         (1.0, 0.0)]},
        "hammer_strike": {"tangage_courbe": [(0.0, 0.0), (0.32, -9.0),
                                             (0.55, 12.0), (1.0, 0.0)]},
        "death": {"tangage_courbe": [(0.0, 0.0), (0.3, 10.0), (0.7, 14.0),
                                     (1.0, 14.0)]},
    },
    "retard_pointe": 0.18,       # la pointe suit la racine, en tours
    "attenuation_pointe": 1.35,  # …et elle va PLUS loin qu'elle
    "echantillon": 2,            # une clé toutes les N frames
}


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
                 bpy.data.cameras, bpy.data.lights, bpy.data.metaballs):
        for bloc in list(coll):
            try:
                coll.remove(bloc)
            except (RuntimeError, ReferenceError):
                pass


def entre_courbe(points, t):
    if t <= points[0][0]:
        return points[0][1]
    for (t0, v0), (t1, v1) in zip(points, points[1:]):
        if t <= t1:
            k = (t - t0) / max(1e-9, t1 - t0)
            k = k * k * (3.0 - 2.0 * k)
            return v0 + (v1 - v0) * k
    return points[-1][1]


def valeur(spec, cle, t, decalage=0.0):
    if f"{cle}_courbe" in spec:
        return entre_courbe(spec[f"{cle}_courbe"], min(1.0, t + decalage))
    if cle in spec:
        a, h, p = spec[cle]
        return a * math.sin(2.0 * math.pi * (h * t + p + decalage))
    return None


def courbe(action, chemin, index, cles, groupe):
    for fc in list(action.fcurves):
        if fc.data_path == chemin and fc.array_index == index:
            action.fcurves.remove(fc)
    fc = action.fcurves.new(data_path=chemin, index=index, action_group=groupe)
    fc.keyframe_points.add(count=len(cles))
    for point, (frame, val) in zip(fc.keyframe_points, cles):
        point.co = (frame, val)
        point.interpolation = "BEZIER"
    fc.update()


def axes_de(arm, nom):
    """Quel axe local fait TANGUER la mèche, lequel la fait ROULER ?

    Mesuré en tournant l'os de 10° autour de chacun de ses axes et en regardant
    où part sa pointe : l'axe qui la déplace le plus horizontalement est le
    tangage (avant/arrière), celui qui l'écarte le plus latéralement le roulis.
    L'orientation d'un os créé depuis la géométrie n'est pas connaissable sur le
    papier."""
    bone = arm.pose.bones[nom]
    bone.rotation_mode = "XYZ"
    repos = (arm.matrix_world @ bone.matrix) @ Vector((0.0, bone.length, 0.0))
    scores = {}
    for i in range(3):
        angles = [0.0, 0.0, 0.0]
        angles[i] = math.radians(10.0)
        bone.rotation_euler = angles
        bpy.context.view_layer.update()
        b = arm.pose.bones[nom]
        pointe = (arm.matrix_world @ b.matrix) @ Vector((0.0, b.length, 0.0))
        scores[i] = pointe - repos
    bone.rotation_euler = (0.0, 0.0, 0.0)
    bpy.context.view_layer.update()
    return scores


def main():
    vider()
    bpy.ops.import_scene.gltf(filepath=SOURCE)

    arm = next((o for o in bpy.data.objects
                if o.type == "ARMATURE" and "Head" in o.pose.bones), None)
    tete = bpy.data.objects.get("SM_Head")
    if arm is None or tete is None:
        print("RESULT: " + json.dumps({"erreur": "corps d'origine incomplet"}))
        return

    # 🚨 Au REPOS. Poser des os et peser une peau dans la POSE revient à
    # déclarer que les coordonnées posées sont celles du bind : le modificateur
    # applique alors une seconde fois l'écart pose↔repos. Leçon payée sur la
    # tête de chien — 116 mm d'écart sur l'os `Head`, la tête partait dans
    # l'épaule.
    arm.data.pose_position = "REST"
    bpy.context.view_layer.update()

    pts = [tete.matrix_world @ v.co for v in tete.data.vertices]
    z_bas, z_haut = min(p.z for p in pts), max(p.z for p in pts)
    seuil = z_bas + (z_haut - z_bas) * SPEC["depart_cheveux"]

    cheveux = [(v.index, tete.matrix_world @ v.co)
               for v in tete.data.vertices
               if (tete.matrix_world @ v.co).z > seuil]
    if not cheveux:
        print("RESULT: " + json.dumps({"erreur": "aucun sommet au-dessus du seuil"}))
        return

    # La masse : son centre, et son point le plus haut-arrière = la pointe.
    centre = sum((p for _, p in cheveux), Vector((0, 0, 0))) / len(cheveux)
    articulation = (arm.matrix_world @ arm.pose.bones["Head"].matrix).translation
    pointe = max((p for _, p in cheveux), key=lambda p: (p - articulation).length)

    inv = arm.matrix_world.inverted()
    racine = Vector((centre.x, centre.y, seuil))
    milieu = racine.lerp(pointe, SPEC["part_premier"])

    for obj in bpy.context.view_layer.objects:
        obj.select_set(False)
    arm.select_set(True)
    bpy.context.view_layer.objects.active = arm
    with bpy.context.temp_override(active_object=arm, object=arm,
                                   selected_editable_objects=[arm]):
        bpy.ops.object.mode_set(mode="EDIT")
        # 🚨 Surtout pas `os` comme nom de variable : il masque le MODULE `os`,
        # et `os.path.getsize` en fin de script tombe alors sur un `EditBone`.
        # Erreur muette jusqu'à la toute dernière ligne, après trois minutes de
        # calcul.
        for ancien in [b for b in arm.data.edit_bones
                       if b.name.startswith("cheveux_")]:
            arm.data.edit_bones.remove(ancien)
        precedent = arm.data.edit_bones["Head"]
        for i, (t, q) in enumerate(((racine, milieu), (milieu, pointe)), 1):
            bone = arm.data.edit_bones.new(f"cheveux_{i:02d}")
            bone.head, bone.tail = inv @ t, inv @ q
            bone.parent = precedent
            bone.use_connect = i == 2
            precedent = bone
        bpy.ops.object.mode_set(mode="OBJECT")

    # ── Les poids : un fondu depuis la ligne de sourcils ─────────────────────
    for nom in ("cheveux_01", "cheveux_02"):
        if tete.vertex_groups.get(nom) is None:
            tete.vertex_groups.new(name=nom)
    groupe_tete = tete.vertex_groups.get("Head")
    hauteur_utile = max(1e-6, z_haut - seuil)
    poses = 0
    for idx, p in cheveux:
        # Fondu doux : 0 à la racine, 1 aux pointes. Un poids franc ferait plier
        # le crâne avec la mèche.
        t = min(1.0, (p.z - seuil) / hauteur_utile)
        part = t * t * (3.0 - 2.0 * t)
        second = t * t
        tete.vertex_groups["cheveux_01"].add([idx], part * (1.0 - second), "REPLACE")
        tete.vertex_groups["cheveux_02"].add([idx], part * second, "REPLACE")
        if groupe_tete is not None:
            groupe_tete.add([idx], 1.0 - part, "REPLACE")
        poses += 1

    # ── Le mouvement, cuit dans les actions existantes ───────────────────────
    scores = {nom: axes_de(arm, nom) for nom in ("cheveux_01", "cheveux_02")}
    axes = {}
    for nom, s in scores.items():
        horizontal = max(s, key=lambda i: Vector((s[i].x, s[i].y, 0.0)).length)
        lateral = max((i for i in s if i != horizontal),
                      key=lambda i: abs(s[i].x))
        axes[nom] = {"tangage": horizontal, "roulis": lateral}

    rapport = {}
    for nom_action, spec in SPEC["etats"].items():
        action = bpy.data.actions.get(nom_action)
        if action is None:
            rapport[nom_action] = {"absent": True}
            continue
        debut, fin = action.frame_range
        pistes = 0
        for nom_os in ("cheveux_01", "cheveux_02"):
            pointe_os = nom_os.endswith("02")
            decalage = SPEC["retard_pointe"] if pointe_os else 0.0
            gain = SPEC["attenuation_pointe"] if pointe_os else 1.0
            for canal in ("tangage", "roulis"):
                if valeur(spec, canal, 0.0, decalage) is None:
                    continue
                cles, f = [], debut
                while f < fin:
                    t = (f - debut) / max(1e-6, fin - debut)
                    cles.append((f, math.radians(
                        valeur(spec, canal, t, decalage) * gain)))
                    f += SPEC["echantillon"]
                cles.append((fin, math.radians(
                    valeur(spec, canal, 1.0, decalage) * gain)))
                courbe(action, f'pose.bones["{nom_os}"].rotation_euler',
                       axes[nom_os][canal], cles, nom_os)
                pistes += 1
        rapport[nom_action] = {"pistes": pistes,
                               "plage": [round(debut, 1), round(fin, 1)]}

    # ── Export ───────────────────────────────────────────────────────────────
    a_exporter = [o for o in bpy.context.scene.objects
                  if o.type in {"MESH", "ARMATURE", "EMPTY"}
                  and not o.name.startswith("Icosph")]
    for obj in bpy.context.view_layer.objects:
        obj.select_set(False)
    for obj in a_exporter:
        obj.hide_set(False)
        obj.select_set(True)
    bpy.context.view_layer.objects.active = a_exporter[0]

    # Contrôle d'emprise AVANT d'écrire : c'est ainsi qu'une cape multipliée par
    # cent est sortie une fois dans un fichier livré.
    hors = {}
    for obj in a_exporter:
        if obj.type != "MESH" or not obj.data.vertices:
            continue
        zs = [(obj.matrix_world @ v.co).z for v in obj.data.vertices]
        if max(zs) > 3.0 or min(zs) < -0.5:
            hors[obj.name] = [round(min(zs), 2), round(max(zs), 2)]
    if hors:
        print("RESULT: " + json.dumps({"erreur": "maillage hors boîte", "hors": hors}))
        return

    arm.data.pose_position = "POSE"
    bpy.context.view_layer.update()
    bpy.ops.export_scene.gltf(
        filepath=SORTIE, export_format="GLB", use_selection=True,
        export_yup=True, export_skins=True, export_materials="EXPORT",
        export_animations=True, export_animation_mode="NLA_TRACKS",
        export_image_format="AUTO",
    )

    print("RESULT: " + json.dumps({
        "seuil_cheveux_z": round(seuil, 4),
        "tete_z": [round(z_bas, 4), round(z_haut, 4)],
        "sommets_cheveux": len(cheveux),
        "sommets_tete": len(tete.data.vertices),
        "part_cheveux": round(len(cheveux) / len(tete.data.vertices), 3),
        "racine": [round(c, 4) for c in racine],
        "pointe": [round(c, 4) for c in pointe],
        "longueur_mm": round((pointe - racine).length * 1000, 1),
        "axes": {k: v for k, v in axes.items()},
        "clips": rapport,
        "sockets": sorted(o.name for o in a_exporter if o.type == "EMPTY"),
        "fichier": SORTIE,
        "octets": os.path.getsize(SORTIE) if os.path.exists(SORTIE) else 0,
    }, ensure_ascii=False))


main()
