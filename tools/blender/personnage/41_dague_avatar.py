"""Repose la dague sur la hanche et lui donne une matière.

    python tools/blender/bmcp.py code tools/blender/personnage/41_dague_avatar.py

À lancer APRÈS `40_cheveux_avatar.py`, sur la scène qu'il laisse. Ré-exporte le
même fichier.

CE QUI N'ALLAIT PAS, MESURÉ

    parent        `RightHand` — mais le centre tombe à (0,03 · 0,14 · 1,37),
                  c'est-à-dire dans le HAUT DU DOS, derrière les épaules.
                  Les os les plus proches sont Spine2 et les clavicules.
    matière       `Other`, couleur de base BLANCHE, aucune texture. Elle a des
                  UV, mais rien à quoi les appliquer.

Le défaut d'accroche était connu : `25_atelier_personnage.py` rattachait
`SM_Dagger` à un os nommé `Armor`, qui n'existe pas dans ce squelette. Une
accroche qui rate ne déplace pas l'objet — elle le laisse là où il était.

OÙ ELLE VA, ET POURQUOI CE N'EST PAS TAPÉ

Sur la hanche GAUCHE : une dague se dégaine en croisant, main droite sur hanche
gauche. Sa hauteur vient de l'os `Hips`, son écartement de la LARGEUR RÉELLE du
corps à cette hauteur — mesurée sur le maillage, pas devinée. Changer de
personnage ou de ceinture déplace donc la dague toute seule.

LA MATIÈRE : DEUX ZONES, PAS UNE TEXTURE

Ses UV ont été dépliées pour un atlas qui n'accompagne pas le fichier ; les
plaquer sur l'atlas d'armure du corps donnerait un morceau de texture pris au
hasard. On peint donc par la GÉOMÉTRIE — lame métallique, garde et poignée de
cuir, réparties le long de l'axe long — avec les couleurs déjà portées par le
personnage. C'est net, ça lit à distance, et ça ne dépend d'aucun fichier
absent.
"""

import json
import os

import bpy
from mathutils import Matrix, Vector

RACINE = r"C:\Users\Antoi\Desktop\Forgia Rewrite"
SORTIE = os.path.join(RACINE, "assets", "models", "characters", "stylized",
                      "stylized_male_cheveux.glb")

SPEC = {
    # Position, en fractions DÉRIVÉES : rien n'est en mètres absolus.
    "jeu_lateral": 0.030,      # écart entre la hanche et le plat de la lame
    "descente": 0.045,         # sous l'os `Hips`, vers le milieu de cuisse
    "avance": 0.020,           # légèrement vers l'avant : elle pend en biais
    "inclinaison": 0.30,       # part de recul de la pointe (0 = pendante)

    # Le partage le long de l'axe long, depuis la POINTE.
    "part_lame": 0.62,
    "part_garde": 0.12,

    "couleurs": {
        # Reprises de la palette du personnage : l'acier des liserés d'armure,
        # le cuir des sangles, le laiton des boucles.
        "lame": "#9FB0C4",
        "garde": "#B08A4A",
        "poignee": "#6B4630",
    },
    "rugosites": {"lame": 0.28, "garde": 0.42, "poignee": 0.72},
    "metal": {"lame": 1.0, "garde": 0.85, "poignee": 0.0},
}


def srgb_lineaire(c):
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def hex_lineaire(code):
    code = code.lstrip("#")
    return tuple(srgb_lineaire(int(code[i:i + 2], 16) / 255.0)
                 for i in (0, 2, 4)) + (1.0,)


def matiere(nom, couleur, rugosite, metal):
    mat = bpy.data.materials.get(nom) or bpy.data.materials.new(nom)
    mat.use_nodes = True
    bsdf = next((n for n in mat.node_tree.nodes if n.type == "BSDF_PRINCIPLED"),
                None)
    if bsdf is not None:
        bsdf.inputs["Base Color"].default_value = hex_lineaire(couleur)
        bsdf.inputs["Roughness"].default_value = rugosite
        bsdf.inputs["Metallic"].default_value = metal
    return mat


def repere(arm):
    """Le cap du personnage : axe par les épaules, sens par les orteils."""
    lateral = ((arm.matrix_world @ arm.pose.bones["RightArm"].matrix).translation
               - (arm.matrix_world @ arm.pose.bones["LeftArm"].matrix).translation)
    lateral.z = 0.0
    lateral = lateral.normalized()
    devant = lateral.cross(Vector((0.0, 0.0, 1.0))).normalized()
    orteil = ((arm.matrix_world @ arm.pose.bones["LeftToeBase"].matrix).translation
              - (arm.matrix_world @ arm.pose.bones["LeftFoot"].matrix).translation)
    orteil.z = 0.0
    if orteil.length > 1e-4 and devant.dot(orteil.normalized()) < 0.0:
        devant = -devant
        lateral = -lateral
    return lateral, devant


def axes_locaux(obj):
    """Axe LONG et axe PLAT de la dague, dans son propre repère.

    Déduits de sa boîte englobante locale : le plus grand côté porte la lame,
    le plus petit la traverse. Les nommer à la main supposerait de connaître
    l'orientation dans laquelle l'objet a été modelé."""
    co = [v.co for v in obj.data.vertices]
    etendue = [max(c[i] for c in co) - min(c[i] for c in co) for i in range(3)]
    long_i = max(range(3), key=lambda i: etendue[i])
    plat_i = min(range(3), key=lambda i: etendue[i])
    return long_i, plat_i, etendue


def main():
    dague = bpy.data.objects.get("SM_Dagger")
    arm = next((o for o in bpy.data.objects
                if o.type == "ARMATURE" and "Head" in o.pose.bones), None)
    corps = bpy.data.objects.get("SM_Body")
    if dague is None or arm is None or corps is None:
        print("RESULT: " + json.dumps({"erreur": "scène incomplète — lancer 40 d'abord"}))
        return

    arm.data.pose_position = "REST"
    bpy.context.view_layer.update()

    lateral, devant = repere(arm)
    hanche = (arm.matrix_world @ arm.pose.bones["Hips"].matrix).translation.copy()

    # 🚨 L'écartement se MESURE sur le corps, à la hauteur de la hanche. Le
    # taper en mètres marcherait pour CE personnage et raterait au premier
    # changement de gabarit ou de ceinture.
    tranche = [corps.matrix_world @ v.co for v in corps.data.vertices
               if abs((corps.matrix_world @ v.co).z - hanche.z) < 0.05]
    demi_largeur = max((abs((p - hanche).dot(lateral)) for p in tranche),
                       default=0.12)

    def centre_geometrique(obj):
        """Le centre des SOMMETS, pas l'origine de l'objet. Ici l'origine est à
        (0, 0, 0) alors que la géométrie flotte dans le haut du dos — mesurer
        l'origine ne dirait rien de ce qu'on voit."""
        pts = [obj.matrix_world @ v.co for v in obj.data.vertices]
        return sum(pts, Vector((0, 0, 0))) / len(pts)

    avant_monde = (centre_geometrique(dague), dague.parent_bone)

    # 🚨 L'ÉCHELLE vit dans la matrice de l'objet, pas dans ses sommets : les
    # coordonnées locales de cette dague vont jusqu'à 2 139 unités. Reconstruire
    # `matrix_world` sans la reprendre l'a envoyée à onze kilomètres du
    # personnage. Une matrice se recompose en entier — translation, rotation ET
    # échelle — ou pas du tout.
    echelle = dague.matrix_world.to_scale()

    long_i, plat_i, etendue = axes_locaux(dague)
    autre_i = 3 - long_i - plat_i

    # Le repère cible : la lame pend vers le bas, inclinée vers l'arrière ; le
    # PLAT de la lame regarde vers l'extérieur, donc contre la cuisse.
    bas = (Vector((0.0, 0.0, -1.0)) - devant * SPEC["inclinaison"]).normalized()
    plat = -lateral                      # hanche gauche → plat vers la gauche
    troisieme = bas.cross(plat).normalized()

    base = [None, None, None]
    base[long_i] = bas
    base[plat_i] = plat
    base[autre_i] = troisieme
    rot = Matrix((
        (base[0].x, base[1].x, base[2].x, 0.0),
        (base[0].y, base[1].y, base[2].y, 0.0),
        (base[0].z, base[1].z, base[2].z, 0.0),
        (0.0, 0.0, 0.0, 1.0)))
    if rot.to_3x3().determinant() < 0.0:      # jamais de repère miroir
        base[autre_i] = -troisieme
        rot = Matrix((
            (base[0].x, base[1].x, base[2].x, 0.0),
            (base[0].y, base[1].y, base[2].y, 0.0),
            (base[0].z, base[1].z, base[2].z, 0.0),
            (0.0, 0.0, 0.0, 1.0)))

    cible = (hanche
             - lateral * (demi_largeur + SPEC["jeu_lateral"])
             - Vector((0.0, 0.0, SPEC["descente"]))
             + devant * SPEC["avance"])

    # Le centre géométrique local doit tomber sur `cible`.
    co = [v.co for v in dague.data.vertices]
    centre_local = Vector((sum(c[i] for c in co) / len(co) for i in range(3)))
    dague.parent = arm
    dague.parent_type = "BONE"
    dague.parent_bone = "Hips"
    dague.matrix_world = (Matrix.Translation(cible) @ rot
                          @ Matrix.Diagonal(echelle.to_4d())
                          @ Matrix.Translation(-centre_local))
    bpy.context.view_layer.update()

    # ── La matière, par zones le long de l'axe long ──────────────────────────
    mats = {cle: matiere(f"Dague_{cle}", val, SPEC["rugosites"][cle],
                         SPEC["metal"][cle])
            for cle, val in SPEC["couleurs"].items()}
    dague.data.materials.clear()
    ordre = ("lame", "garde", "poignee")
    for cle in ordre:
        dague.data.materials.append(mats[cle])
    slot = {cle: i for i, cle in enumerate(ordre)}

    # La POINTE est l'extrémité la plus éloignée de la main : ici, la plus basse
    # une fois posée. On trie donc sur l'axe long en coordonnées locales, et on
    # oriente par le monde pour savoir de quel bout on part.
    valeurs = [c[long_i] for c in co]
    bas_l, haut_l = min(valeurs), max(valeurs)
    p_bas = dague.matrix_world @ Vector(
        tuple((bas_l if i == long_i else centre_local[i]) for i in range(3)))
    p_haut = dague.matrix_world @ Vector(
        tuple((haut_l if i == long_i else centre_local[i]) for i in range(3)))
    depuis_pointe_en_bas = p_bas.z < p_haut.z

    compte = {cle: 0 for cle in ordre}
    for poly in dague.data.polygons:
        v = poly.center[long_i]
        t = (v - bas_l) / max(1e-9, haut_l - bas_l)
        if not depuis_pointe_en_bas:
            t = 1.0 - t
        if t < SPEC["part_lame"]:
            cle = "lame"
        elif t < SPEC["part_lame"] + SPEC["part_garde"]:
            cle = "garde"
        else:
            cle = "poignee"
        poly.material_index = slot[cle]
        compte[cle] += 1

    # ── Ré-export ────────────────────────────────────────────────────────────
    a_exporter = [o for o in bpy.context.scene.objects
                  if o.type in {"MESH", "ARMATURE", "EMPTY"}
                  and not o.name.startswith("Icosph")]
    for obj in bpy.context.view_layer.objects:
        obj.select_set(False)
    for obj in a_exporter:
        obj.hide_set(False)
        obj.select_set(True)
    bpy.context.view_layer.objects.active = a_exporter[0]

    arm.data.pose_position = "POSE"
    bpy.context.view_layer.update()
    bpy.ops.export_scene.gltf(
        filepath=SORTIE, export_format="GLB", use_selection=True,
        export_yup=True, export_skins=True, export_materials="EXPORT",
        export_animations=True, export_animation_mode="NLA_TRACKS",
        export_image_format="AUTO",
    )

    print("RESULT: " + json.dumps({
        "avant": {"centre": [round(c, 4) for c in avant_monde[0]],
                  "os": avant_monde[1]},
        "apres": {"centre": [round(c, 4) for c in centre_geometrique(dague)],
                  "os": dague.parent_bone},
        "echelle": [round(c, 5) for c in echelle],
        "hanche_z": round(hanche.z, 4),
        "demi_largeur_mesuree": round(demi_largeur, 4),
        "axe_long": long_i, "axe_plat": plat_i,
        "etendue_locale": [round(e, 4) for e in etendue],
        "determinant": round(rot.to_3x3().determinant(), 4),
        "faces_par_zone": compte,
        "octets": os.path.getsize(SORTIE) if os.path.exists(SORTIE) else 0,
    }, ensure_ascii=False))


main()
