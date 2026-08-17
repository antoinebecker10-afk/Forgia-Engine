"""Pèse le chien sur le squelette et lui donne ses points d'animation.

    python tools/blender/bmcp.py code tools/blender/personnage/36_rig_chien.py

À lancer APRÈS `35_textures_poil.py`.

CE QU'IL AJOUTE, ET POURQUOI EXACTEMENT ÇA

Les 9 clips du corps sont des clips Mixamo HUMAINS : ils ne connaissent ni
oreille ni paupière, et ajouter des os ne les fera pas bouger. Ce script ne
livre donc pas de l'animation — il livre les POIGNÉES par lesquelles le moteur
pourra animer, et rien d'autre :

    4 os d'oreille   `oreille_<g|d>_01` et `_02`, enfants de `Head`.
                     Deux segments par oreille : un seul os la rendrait rigide,
                     trois seraient trois fois le coût pour un pavillon qui
                     retombe pareil.
    1 clé de forme   `cligner` — les paupières basculent SUR l'œil. Une clé de
                     forme et non des os : une paupière ne pivote pas autour
                     d'une articulation, elle glisse sur une bille.

TOUT SE DÉRIVE DE LA GÉOMÉTRIE. La base et la pointe de chaque oreille sont les
sommets les plus proche et les plus lointain de l'articulation ; le centre de
rotation des paupières est le centre de la bille correspondante ; le sens de
fermeture est celui qui RAPPROCHE la paupière du devant de l'œil — testé, pas
supposé. Rien n'est tapé en coordonnées.
"""

import json
import math

import bpy
from mathutils import Matrix, Vector

PEAU, YEUX, MUSEAU = "chien_peau", "chien_yeux", "chien_museau"
SQUELETTE = "perso_squelette"

SPEC = {
    "os_oreille": {
        "part_premier": 0.5,      # partage base→pointe entre les deux segments
        "epaisseur": 0.012,       # rayon d'affichage, sans effet sur la peau
        # Le tout premier centimètre de l'oreille reste tenu par le crâne :
        # sinon l'attache se décolle dès que l'os tourne.
        "ancrage_crane": 0.18,
    },
    # La rotation n'est pas déclarée : elle vaut l'angle entre l'axe de la
    # calotte et le regard, plus ce dépassement.
    #
    # 🚨 16° laissait un liseré de blanc EN HAUT : chaque degré de dépassement
    # avance le bord bas de la calotte et fait reculer le bord haut d'autant.
    # Le capteur `couverture_marge_deg` mesure ce qui reste — il doit rester
    # largement positif.
    "clignement": {"depassement_deg": 6.0},

    # Un os par œil, pour que le regard puisse SUIVRE quelque chose. Chez un
    # personnage cartoon, des yeux qui bougent valent la moitié de
    # l'expressivité — un regard figé droit devant se lit comme une peluche.
    # L'os part du centre de la bille vers l'avant : le moteur n'a plus qu'à le
    # faire tourner.
    "os_oeil": {"longueur": 0.045},

    # La mâchoire. Sa CHARNIÈRE ne s'invente pas : la ligne de gueule sculptée
    # donne déjà les deux commissures, et une mâchoire pivote sur l'axe qui les
    # relie, reculé vers l'articulation. Les sommets sous cette ligne
    # descendent avec elle, en fondu pour qu'aucun pli ne se voie.
    "machoire": {
        "recul": 0.055,          # décalage de la charnière vers l'arrière
        "fondu": 0.030,          # hauteur du dégradé sous la ligne de gueule
    },
}


def bone_monde(arm, nom):
    b = arm.pose.bones.get(nom)
    return None if b is None else (arm.matrix_world @ b.matrix).translation.copy()


def sommets_du_groupe(obj, nom_groupe):
    """Indices + positions MONDE des sommets portant ce groupe."""
    groupe = obj.vertex_groups.get(nom_groupe)
    if groupe is None:
        return []
    idx = groupe.index
    sortie = []
    for v in obj.data.vertices:
        if any(g.group == idx and g.weight > 0.5 for g in v.groups):
            sortie.append((v.index, obj.matrix_world @ v.co))
    return sortie


def extremites(pts, reference):
    """Attache et pointe d'une oreille.

    🚨 La référence est le CENTRE DU CRÂNE, pas l'articulation. Mesuré : l'os
    `Head` est en bas du cou (z = 1,539) alors que l'oreille s'attache en haut
    (z ≈ 1,76) et pend jusqu'en bas (z ≈ 1,53). Trier par distance à
    l'articulation désignait donc le bout qui pend comme « la base » — les deux
    os d'oreille poussaient à l'envers, du menton vers le sommet."""
    return (min(pts, key=lambda p: (p - reference).length),
            max(pts, key=lambda p: (p - reference).length))


def centre_du_crane(peau):
    pts = [p for _, p in sommets_du_groupe(peau, "chien_tete")]
    return sum(pts, Vector((0, 0, 0))) / len(pts) if pts else None


def poser_os_oreilles(arm, peau, reference):
    """Deux os par oreille, posés sur l'axe réel de l'oreille sculptée."""
    inv = arm.matrix_world.inverted()
    poses = {}

    bpy.ops.object.select_all(action="DESELECT")
    arm.select_set(True)
    bpy.context.view_layer.objects.active = arm
    with bpy.context.temp_override(active_object=arm, object=arm,
                                   selected_editable_objects=[arm]):
        bpy.ops.object.mode_set(mode="EDIT")
        # Rejouable : on repart des os d'oreille, on ne les empile pas.
        for os in [b for b in arm.data.edit_bones if b.name.startswith("oreille_")]:
            arm.data.edit_bones.remove(os)
        for cote in ("g", "d"):
            pts = [p for _, p in sommets_du_groupe(peau, f"chien_oreille_{cote}")]
            if not pts:
                continue
            base, pointe = extremites(pts, reference)
            milieu = base.lerp(pointe, SPEC["os_oreille"]["part_premier"])

            precedent = arm.data.edit_bones["Head"]
            for i, (tete, queue) in enumerate(((base, milieu), (milieu, pointe)), 1):
                os = arm.data.edit_bones.new(f"oreille_{cote}_{i:02d}")
                os.head = inv @ tete
                os.tail = inv @ queue
                os.parent = precedent
                os.use_connect = i == 2
                os.envelope_distance = SPEC["os_oreille"]["epaisseur"]
                precedent = os
            poses[f"oreille_{cote}"] = {
                "base": [round(c, 4) for c in base],
                "pointe": [round(c, 4) for c in pointe],
                "longueur_mm": round((pointe - base).length * 1000, 1),
            }
        bpy.ops.object.mode_set(mode="OBJECT")
    return poses


def poser_os_visage(arm, peau, yeux, museau, devant):
    """Un os par œil, et la mâchoire. Tous enfants de `Head`.

    La charnière de mâchoire se DÉDUIT de la ligne de gueule : ses deux
    extrémités sont les commissures, et une mâchoire pivote sur l'axe qui les
    relie. Reculée de quelques centimètres, parce qu'une charnière posée sur les
    commissures elles-mêmes ferait pivoter le menton autour de la bouche."""
    inv = arm.matrix_world.inverted()
    infos = {}

    centres = {}
    for cote in ("g", "d"):
        pts = [p for _, p in sommets_du_groupe(yeux, f"chien_oeil_{cote}")]
        if pts:
            centres[cote] = sum(pts, Vector((0, 0, 0))) / len(pts)

    gueule = [p for _, p in sommets_du_groupe(museau, "chien_gueule")]

    bpy.ops.object.select_all(action="DESELECT")
    arm.select_set(True)
    bpy.context.view_layer.objects.active = arm
    with bpy.context.temp_override(active_object=arm, object=arm,
                                   selected_editable_objects=[arm]):
        bpy.ops.object.mode_set(mode="EDIT")
        for os in [b for b in arm.data.edit_bones
                   if b.name.startswith(("oeil_", "machoire"))]:
            arm.data.edit_bones.remove(os)
        tete = arm.data.edit_bones["Head"]

        for cote, centre in centres.items():
            os = arm.data.edit_bones.new(f"oeil_{cote}")
            os.head = inv @ centre
            os.tail = inv @ (centre + devant * SPEC["os_oeil"]["longueur"])
            os.parent = tete
            infos[f"oeil_{cote}"] = [round(c, 4) for c in centre]

        if gueule:
            # Les commissures : les deux points les plus écartés de la ligne.
            milieu = sum(gueule, Vector((0, 0, 0))) / len(gueule)
            lateral = max(gueule, key=lambda p: (p - milieu).length) - milieu
            lateral = lateral.normalized()
            gauche = max(gueule, key=lambda p: (p - milieu).dot(lateral))
            droite = min(gueule, key=lambda p: (p - milieu).dot(lateral))
            charniere = (gauche + droite) / 2.0 - devant * SPEC["machoire"]["recul"]
            avant_gueule = max(gueule, key=lambda p: (p - milieu).dot(devant))

            os = arm.data.edit_bones.new("machoire")
            os.head = inv @ charniere
            os.tail = inv @ avant_gueule
            os.parent = tete
            hinge = charniere.copy()
            infos["machoire"] = {
                "charniere": [round(c, 4) for c in charniere],
                "longueur_mm": round((avant_gueule - charniere).length * 1000, 1),
                "ligne_gueule_z": round(milieu.z, 4),
            }
        bpy.ops.object.mode_set(mode="OBJECT")
    return infos, (milieu.z if gueule else None), (hinge if gueule else None)


def peser(obj, arm, reference, os_cou, machoire):
    """Chaque sommet à son os. Trois régimes, aucun par défaut silencieux."""
    for nom in ("Head", "Neck", "Spine2", "oreille_g_01", "oreille_g_02",
                "oreille_d_01", "oreille_d_02", "oeil_g", "oeil_d", "machoire"):
        if nom in arm.data.bones and obj.vertex_groups.get(nom) is None:
            obj.vertex_groups.new(name=nom)

    # Les yeux suivent leur propre os, en entier : une bille ne se déforme pas,
    # elle tourne. L'éclat part avec elle, sinon il glisse sur la cornée.
    billes = {}
    for cote in ("g", "d"):
        for source in (f"chien_oeil_{cote}", f"chien_eclat_{cote}"):
            billes.update({i: f"oeil_{cote}"
                           for i, _ in sommets_du_groupe(obj, source)})

    oreilles = {}
    for cote in ("g", "d"):
        indices = set()
        for source in (f"chien_oreille_{cote}", f"chien_pavillon_{cote}"):
            indices |= {i for i, _ in sommets_du_groupe(obj, source)}
        if not indices:
            continue
        pts = {i: obj.matrix_world @ obj.data.vertices[i].co for i in indices}
        base, pointe = extremites(list(pts.values()), reference)
        axe = pointe - base
        longueur2 = max(1e-9, axe.length_squared)
        oreilles[cote] = (pts, base, axe, longueur2)

    cou = {i for i, _ in sommets_du_groupe(obj, "chien_cou")}
    compte = {"oreilles": 0, "cou": 0, "yeux": 0, "machoire": 0, "crane": 0}

    for v in obj.data.vertices:
        pose = None

        if v.index in billes:
            pose = {billes[v.index]: 1.0}
            compte["yeux"] += 1
        for cote, (pts, base, axe, longueur2) in oreilles.items():
            if pose is not None or v.index not in pts:
                continue
            t = max(0.0, min(1.0, (pts[v.index] - base).dot(axe) / longueur2))
            # L'attache reste tenue par le crâne, sinon elle se décolle.
            au_crane = max(0.0, 1.0 - t / SPEC["os_oreille"]["ancrage_crane"])
            reste = 1.0 - au_crane
            # Fondu doux entre les deux segments : un partage net produirait un
            # pli visible au milieu de l'oreille.
            second = t * t * (3.0 - 2.0 * t)
            pose = {"Head": au_crane,
                    f"oreille_{cote}_01": reste * (1.0 - second),
                    f"oreille_{cote}_02": reste * second}
            compte["oreilles"] += 1
            break

        if pose is None and v.index in cou:
            p = obj.matrix_world @ v.co
            haut, milieu, bas = os_cou
            t = (p - haut).dot(bas - haut) / max(1e-9, (bas - haut).length_squared)
            t = max(0.0, min(1.0, t))
            if t < 0.5:
                k = t / 0.5
                pose = {"Head": 1.0 - k, "Neck": k}
            else:
                k = (t - 0.5) / 0.5
                pose = {"Neck": 1.0 - k, "Spine2": k}
            compte["cou"] += 1

        # La mâchoire : tout ce qui est SOUS la ligne de gueule descend avec
        # elle, en fondu sur `fondu` mètres pour qu'aucun pli ne se voie. La
        # ligne est mesurée, pas décidée — c'est le ruban de gueule sculpté.
        z_gueule, charniere, devant = machoire
        if pose is None and z_gueule is not None and "machoire" in arm.data.bones:
            p = obj.matrix_world @ v.co
            sous = z_gueule - p.z
            # 🚨 « Sous la ligne de gueule » ne suffit pas : le critère attrapait
            # l'arrière du crâne, et ouvrir la gueule aurait fait pivoter la
            # nuque. Une mâchoire est sous la ligne ET DEVANT sa charnière.
            devant_charniere = (p - charniere).dot(devant)
            if sous > 0.0 and devant_charniere > 0.0:
                part = min(1.0, sous / SPEC["machoire"]["fondu"])
                # Fondu aussi vers l'arrière, sinon une arête franche apparaît
                # au niveau de la charnière.
                part *= min(1.0, devant_charniere / SPEC["machoire"]["fondu"])
                if part > 1e-3:
                    pose = {"machoire": part, "Head": 1.0 - part}
                    compte["machoire"] += 1

        if pose is None:
            pose = {"Head": 1.0}
            compte["crane"] += 1

        for nom, poids in pose.items():
            groupe = obj.vertex_groups.get(nom)
            if groupe is not None and poids > 1e-4:
                groupe.add([v.index], poids, "REPLACE")

    if not any(m.type == "ARMATURE" for m in obj.modifiers):
        mod = obj.modifiers.new("squelette", "ARMATURE")
        mod.object = arm
    # Parenté SANS transformation : les sommets sont déjà en place dans le monde.
    obj.parent = arm
    obj.matrix_parent_inverse = arm.matrix_world.inverted()
    return compte


def regard_de(yeux, cote, centre):
    """La direction du regard, lue sur la PUPILLE.

    🚨 Première version : je la déduisais de la position de la paupière, en
    annulant sa composante verticale. Or la paupière est posée juste AU-DESSUS
    de l'œil — une fois le z annulé il ne reste qu'un résidu de bruit, et la
    bascule partait dans une direction arbitraire. Les paupières s'ouvraient en
    éventail sur les joues au lieu de fermer.

    La pupille, elle, ne peut pas mentir sur l'endroit où l'œil regarde : c'est
    sa définition."""
    slots = [i for i, s in enumerate(yeux.material_slots)
             if s.material and s.material.name.endswith("pupille")]
    if not slots:
        return None
    groupe = yeux.vertex_groups.get(f"chien_oeil_{cote}")
    if groupe is None:
        return None
    dedans = {v.index for v in yeux.data.vertices
              if any(g.group == groupe.index and g.weight > 0.5 for g in v.groups)}
    pts = [yeux.matrix_world @ (sum((yeux.data.vertices[i].co for i in p.vertices),
                                    Vector((0, 0, 0))) / len(p.vertices))
           for p in yeux.data.polygons
           if p.material_index in slots and set(p.vertices) <= dedans]
    if not pts:
        return None
    direction = sum(pts, Vector((0, 0, 0))) / len(pts) - centre
    return direction.normalized() if direction.length > 1e-6 else None


def clignement(peau, yeux):
    """Une clé de forme qui fait BASCULER les paupières sur les billes.

    Ni angle ni sens ne sont choisis : on fait tourner la calotte de paupière
    jusqu'à ce que son axe TOMBE SUR le regard, plus un léger dépassement pour
    qu'elle ferme au lieu d'affleurer. L'axe de rotation se déduit des deux
    directions, donc il n'y a plus de signe à deviner."""
    if peau.data.shape_keys is None:
        peau.shape_key_add(name="Basis", from_mix=False)
    for bloc in list(peau.data.shape_keys.key_blocks):
        if bloc.name == "cligner":
            peau.shape_key_remove(bloc)
    cle = peau.shape_key_add(name="cligner", from_mix=False)

    inv = peau.matrix_world.inverted()
    detail = {}
    for cote in ("g", "d"):
        bille = [p for _, p in sommets_du_groupe(yeux, f"chien_oeil_{cote}")]
        paupiere = sommets_du_groupe(peau, f"chien_paupiere_{cote}")
        if not bille or not paupiere:
            continue
        centre = sum(bille, Vector((0, 0, 0))) / len(bille)
        rayon = max((p - centre).length for p in bille)
        regard = regard_de(yeux, cote, centre)
        if regard is None:
            continue

        # L'axe de la calotte de paupière, tel qu'il est au repos.
        moyenne = sum((p for _, p in paupiere), Vector((0, 0, 0))) / len(paupiere)
        axe_paupiere = (moyenne - centre).normalized()

        axe = axe_paupiere.cross(regard)
        if axe.length < 1e-6:      # déjà alignée : rien à fermer
            continue
        axe = axe.normalized()
        angle = axe_paupiere.angle(regard) + math.radians(
            SPEC["clignement"]["depassement_deg"])

        R = Matrix.Rotation(angle, 3, axe)
        deplace = [centre + R @ (p - centre) for _, p in paupiere]
        for (idx, _), cible in zip(paupiere, deplace):
            cle.data[idx].co = inv @ cible

        # Contrôle : une paupière fermée doit couvrir le DEVANT de l'œil.
        devant = centre + regard * rayon
        reste = (sum(deplace, Vector((0, 0, 0))) / len(deplace) - devant).length
        # Capteur de couverture : rayon angulaire de la calotte, moins l'écart
        # que le dépassement creuse entre son pôle fermé et le regard.
        pole_ferme = R @ axe_paupiere
        rayon = max((p - centre).normalized().angle(axe_paupiere)
                    for _, p in paupiere)
        marge = math.degrees(rayon) - math.degrees(pole_ferme.angle(regard))

        detail[cote] = {
            "sommets": len(paupiere),
            "rotation_deg": round(math.degrees(angle), 1),
            "calotte_rayon_deg": round(math.degrees(rayon), 1),
            "couverture_marge_deg": round(marge, 1),
            "regard": [round(c, 3) for c in regard],
            "centre_paupiere_au_devant_mm": round(reste * 1000, 1),
        }
    cle.value = 0.0
    return detail


def main():
    arm = bpy.data.objects.get(SQUELETTE)
    peau = bpy.data.objects.get(PEAU)
    if arm is None or peau is None:
        print("RESULT: " + json.dumps({"erreur": "scène incomplète — 34 puis 35"}))
        return

    # 🚨 Ce script REMET la pose en fin de course. Il doit donc réimposer le
    # repos en entrant, sinon un second passage lit des matrices posées et
    # reconstruit tout avec 116 mm d'écart — mesuré : 144,7 mm de dérive de bind
    # au deuxième lancement. Un script qui n'est pas rejouable est un script
    # qu'on n'ose plus relancer.
    arm.data.pose_position = "REST"
    bpy.context.view_layer.update()

    articulation = bone_monde(arm, "Head")
    os_cou = (articulation, bone_monde(arm, "Neck"), bone_monde(arm, "Spine2"))
    reference = centre_du_crane(peau)
    if reference is None:
        print("RESULT: " + json.dumps({"erreur": "trace `chien_tete` absente"}))
        return

    poses = poser_os_oreilles(arm, peau, reference)

    # Le cap, relu sur les epaules (axe) et les orteils (sens) — meme discipline
    # que la sculpture : les pieds sont ecartes au repos, ils donnent le sens
    # mais pas l'angle.
    lateral = ((arm.matrix_world @ arm.pose.bones["RightArm"].matrix).translation
               - (arm.matrix_world @ arm.pose.bones["LeftArm"].matrix).translation)
    lateral.z = 0.0
    devant = lateral.normalized().cross(Vector((0.0, 0.0, 1.0))).normalized()
    grossier = Vector((0.0, 0.0, 0.0))
    for cheville, orteil in (("LeftFoot", "LeftToeBase"), ("RightFoot", "RightToeBase")):
        a, b = arm.pose.bones.get(cheville), arm.pose.bones.get(orteil)
        if a and b:
            d = ((arm.matrix_world @ b.matrix).translation
                 - (arm.matrix_world @ a.matrix).translation)
            d.z = 0.0
            if d.length > 1e-4:
                grossier += d.normalized()
    if grossier.length > 1e-6 and devant.dot(grossier.normalized()) < 0.0:
        devant = -devant

    museau = bpy.data.objects.get(MUSEAU)
    yeux0 = bpy.data.objects.get(YEUX)
    visage, z_gueule, charniere = poser_os_visage(
        arm, peau, yeux0, museau, devant)

    comptes = {}
    for nom in (PEAU, YEUX, MUSEAU):
        obj = bpy.data.objects.get(nom)
        if obj is not None:
            comptes[nom] = peser(obj, arm, reference, os_cou,
                                 (z_gueule, charniere, devant))

    yeux = bpy.data.objects.get(YEUX)
    detail_cligne = clignement(peau, yeux) if yeux else {}

    # Contrôle du bind, AVANT de remettre la pose : au repos, un modificateur
    # d'armature correct ne doit rien déplacer du tout. Toute valeur non nulle
    # ici est la double transformation qui a coûté la passe précédente.
    bpy.context.view_layer.update()
    dg = bpy.context.evaluated_depsgraph_get()
    evalue = peau.evaluated_get(dg)
    me = evalue.to_mesh()
    # `bpy_prop_collection` ne se tranche pas comme une liste — on échantillonne
    # par indice.
    derives = [(me.vertices[i].co - peau.data.vertices[i].co).length
               for i in range(0, len(peau.data.vertices), 17)]
    evalue.to_mesh_clear()

    # La pose revient : la liaison est faite, le corps peut se rasseoir dedans.
    arm.data.pose_position = "POSE"
    bpy.context.view_layer.update()

    print("RESULT: " + json.dumps({
        "derive_bind_au_repos_mm": round(max(derives) * 1000, 3),
        "os_ajoutes": sorted(b.name for b in arm.data.bones
                             if b.name.startswith(("oreille_", "oeil_",
                                                   "machoire"))),
        "visage": visage,
        "oreilles": poses,
        "pesee": comptes,
        "clignement": detail_cligne,
        "cles_de_forme": [k.name for k in peau.data.shape_keys.key_blocks],
        "total_os": len(arm.data.bones),
    }, ensure_ascii=False))


main()
