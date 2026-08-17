"""Anime oreilles, yeux et mâchoire DANS les 9 clips existants.

    python tools/blender/bmcp.py code tools/blender/personnage/38_anim_secondaire.py

À lancer APRÈS `36_rig_chien.py`, avant l'export.

POURQUOI DANS LES CLIPS, ET PAS À CÔTÉ

Chaque clip est déjà une action Blender posée sur sa propre piste NLA homonyme,
et le moteur les joue par leur nom. Greffer les courbes des nouveaux os DANS ces
actions veut dire qu'« idle » fait bouger les oreilles sans que rien, côté Rust,
ait à le savoir. L'alternative — des clips additifs mélangés à l'exécution —
demanderait un graphe d'animation, donc du code, dans une crate qu'un autre
terminal est en train d'éditer.

LES AXES NE SONT PAS DEVINÉS

L'axe local qui fait balancer une oreille d'avant en arrière n'est pas
connaissable sur le papier : il dépend de l'orientation que l'os a prise à sa
création, elle-même dérivée de la géométrie sculptée. On tourne donc chaque os
de 10° autour de chacun de ses trois axes, on MESURE où part sa pointe, et on
garde l'axe qui la déplace le plus vers l'avant (balancement) et celui qui
l'écarte le plus (battement).

CE QUE CHAQUE ÉTAT RACONTE

    idle          balancement lent, un clignement, le regard qui dérive
    walk          les oreilles battent au pas
    run           elles volent, et la gueule HALÈTE — c'est le détail qui
                  vend la course chez un chien
    jump          elles montent au décollage, traînent en l'air, encaissent
                  à la réception ; clignement à l'impact
    jog_backward  elles partent vers l'avant : on recule, elles restent
    hit_react     elles se plaquent, la gueule crie, l'œil se ferme
    death         elles tombent et ne se relèvent pas
    hammer_strike la gueule s'ouvre sur l'effort
    swim          plaquées en arrière, sans battement

Il n'y a PAS de clip « tourner » : le pivot du personnage est produit par la
manette, pas par une animation. Une réaction d'oreille au virage demande donc
un ressort côté moteur — c'est le seul des états demandés qu'un clip ne peut
pas porter.
"""

import json
import math

import bpy
from mathutils import Vector

SQUELETTE = "perso_squelette"
PEAU = "chien_peau"
ECHANTILLON = 2          # une clé toutes les N frames

# Amplitudes en degrés. `(amplitude, harmonique, phase)` — harmonique = nombre
# de cycles sur la durée du clip, phase en tours.
ETATS = {
    "idle": {
        "balancement": (5.0, 1, 0.0), "battement": (2.2, 1, 0.25),
        "regard_lacet": (6.0, 1, 0.0), "regard_site": (2.5, 2, 0.15),
        "machoire": (1.2, 2, 0.0),
        "clignements": [0.58],
    },
    "walk": {
        "balancement": (9.0, 2, 0.0), "battement": (4.0, 2, 0.3),
        "regard_lacet": (2.0, 1, 0.0), "machoire": (2.0, 2, 0.1),
        "clignements": [],
    },
    "run": {
        # La gueule ouverte À MI-COURSE et refermée à peine : un halètement ne
        # claque pas, il oscille autour d'une position ouverte.
        "balancement": (17.0, 2, 0.0), "battement": (9.0, 2, 0.35),
        "machoire_base": 9.0, "machoire": (4.5, 4, 0.0),
        "regard_lacet": (1.0, 1, 0.0),
        "clignements": [],
    },
    "jog_backward": {
        "balancement": (-11.0, 2, 0.0), "battement": (5.0, 2, 0.3),
        "regard_lacet": (7.0, 1, 0.5), "machoire": (1.5, 2, 0.0),
        "clignements": [0.35],
    },
    "swim": {
        "balancement_courbe": [(0.0, 0.0), (0.08, -16.0), (1.0, -16.0)],
        "battement": (3.0, 6, 0.0), "machoire": (1.0, 4, 0.0),
        "clignements": [0.2, 0.55, 0.85],
    },
    # ── états ponctuels : des courbes, pas des cycles ────────────────────────
    "jump": {
        "balancement_courbe": [(0.0, 0.0), (0.18, 22.0), (0.45, -14.0),
                               (0.72, -6.0), (0.86, 14.0), (1.0, 0.0)],
        "battement_courbe": [(0.0, 0.0), (0.2, 7.0), (0.6, -4.0), (1.0, 0.0)],
        "machoire_courbe": [(0.0, 0.0), (0.16, 13.0), (0.5, 5.0), (1.0, 0.0)],
        "clignements": [0.84],
    },
    "hit_react": {
        "balancement_courbe": [(0.0, 0.0), (0.1, -24.0), (0.4, -18.0), (1.0, -2.0)],
        "battement_courbe": [(0.0, 0.0), (0.1, -12.0), (0.5, -6.0), (1.0, 0.0)],
        "machoire_courbe": [(0.0, 0.0), (0.08, 16.0), (0.3, 6.0), (1.0, 0.0)],
        "clignements": [0.08],
    },
    "death": {
        "balancement_courbe": [(0.0, 0.0), (0.25, -20.0), (0.6, -26.0), (1.0, -26.0)],
        "battement_courbe": [(0.0, 0.0), (0.3, -8.0), (1.0, -10.0)],
        "machoire_courbe": [(0.0, 0.0), (0.2, 8.0), (0.7, 4.0), (1.0, 3.0)],
        # L'œil se ferme et NE SE ROUVRE PAS : la seule fermeture tenue du lot.
        "paupieres_courbe": [(0.0, 0.0), (0.45, 1.0), (1.0, 1.0)],
    },
    "hammer_strike": {
        "balancement_courbe": [(0.0, 0.0), (0.3, 12.0), (0.52, -18.0),
                               (0.75, 6.0), (1.0, 0.0)],
        "battement": (4.0, 2, 0.0),
        "machoire_courbe": [(0.0, 0.0), (0.35, 11.0), (0.55, 2.0), (1.0, 0.0)],
        "clignements": [0.54],
    },
}

RETARD_POINTE = 0.16     # la pointe suit la base avec ce retard, en tours
DUREE_CLIGNEMENT = 5.0   # frames pour fermer puis rouvrir


def axes_de(arm, nom, devant, lateral):
    """Quel axe LOCAL balance l'os, et lequel l'écarte ? Mesuré, pas déduit."""
    bone = arm.pose.bones[nom]
    bone.rotation_mode = "XYZ"
    repos = (arm.matrix_world @ bone.matrix) @ Vector((0.0, bone.length, 0.0))
    scores = {}
    for i in range(3):
        angles = [0.0, 0.0, 0.0]
        angles[i] = math.radians(10.0)
        bone.rotation_euler = angles
        bpy.context.view_layer.update()
        b2 = arm.pose.bones[nom]
        pointe = (arm.matrix_world @ b2.matrix) @ Vector((0.0, b2.length, 0.0))
        d = pointe - repos
        scores[i] = (d.dot(devant), d.dot(lateral))
    bone.rotation_euler = (0.0, 0.0, 0.0)
    bpy.context.view_layer.update()

    balance = max(scores, key=lambda i: abs(scores[i][0]))
    ecarte = max((i for i in scores if i != balance),
                 key=lambda i: abs(scores[i][1]))
    return {
        "balancement": (balance, 1.0 if scores[balance][0] > 0 else -1.0),
        "battement": (ecarte, 1.0 if scores[ecarte][1] > 0 else -1.0),
        "mesures": {i: [round(v, 5) for v in s] for i, s in scores.items()},
    }


def courbe(action, chemin, index, cles, groupe):
    """Pose une F-curve, en écrasant celle qui existerait déjà."""
    for fc in list(action.fcurves):
        if fc.data_path == chemin and fc.array_index == index:
            action.fcurves.remove(fc)
    fc = action.fcurves.new(data_path=chemin, index=index, action_group=groupe)
    fc.keyframe_points.add(count=len(cles))
    for point, (frame, valeur) in zip(fc.keyframe_points, cles):
        point.co = (frame, valeur)
        point.interpolation = "BEZIER"
    fc.update()
    return fc


def echantillonne(debut, fin, fonction):
    """Une clé toutes les `ECHANTILLON` frames, bornes comprises."""
    cles, f = [], debut
    while f < fin:
        cles.append((f, fonction((f - debut) / max(1e-6, fin - debut))))
        f += ECHANTILLON
    cles.append((fin, fonction(1.0)))
    return cles


def entre_courbe(points, t):
    """Interpolation linéaire dans une liste [(t, valeur)] triée."""
    if t <= points[0][0]:
        return points[0][1]
    for (t0, v0), (t1, v1) in zip(points, points[1:]):
        if t <= t1:
            k = (t - t0) / max(1e-9, t1 - t0)
            # Lissage en S : une interpolation linéaire se voit sur une oreille.
            k = k * k * (3.0 - 2.0 * k)
            return v0 + (v1 - v0) * k
    return points[-1][1]


def valeur_de(spec, cle, t, decalage=0.0):
    """Un canal vaut soit un cycle, soit une courbe, soit rien."""
    if f"{cle}_courbe" in spec:
        return entre_courbe(spec[f"{cle}_courbe"], min(1.0, t + decalage))
    if cle in spec:
        amplitude, harmonique, phase = spec[cle]
        return amplitude * math.sin(2.0 * math.pi
                                    * (harmonique * t + phase + decalage))
    return None


def poser_clignements(spec, debut, fin, fps):
    """Le clignement : fermé en 2 frames, rouvert en 3. Jamais symétrique —
    une paupière tombe plus vite qu'elle ne remonte."""
    if "paupieres_courbe" in spec:
        return [(debut + (fin - debut) * t, v) for t, v in spec["paupieres_courbe"]]
    cles = [(debut, 0.0)]
    for part in spec.get("clignements", []):
        centre = debut + (fin - debut) * part
        for delta, valeur in ((-DUREE_CLIGNEMENT * 0.5, 0.0),
                              (-DUREE_CLIGNEMENT * 0.1, 1.0),
                              (DUREE_CLIGNEMENT * 0.5, 0.0)):
            f = min(fin, max(debut, centre + delta))
            cles.append((f, valeur))
    cles.append((fin, 0.0))
    cles.sort(key=lambda c: c[0])
    return cles


def main():
    arm = bpy.data.objects.get(SQUELETTE)
    peau = bpy.data.objects.get(PEAU)
    if arm is None or peau is None or peau.data.shape_keys is None:
        print("RESULT: " + json.dumps({"erreur": "scène incomplète — 36 d'abord"}))
        return
    if "oreille_g_01" not in arm.data.bones:
        print("RESULT: " + json.dumps({"erreur": "os d'oreille absents — 36 d'abord"}))
        return

    arm.data.pose_position = "POSE"
    bpy.context.view_layer.update()

    lateral = ((arm.matrix_world @ arm.pose.bones["RightArm"].matrix).translation
               - (arm.matrix_world @ arm.pose.bones["LeftArm"].matrix).translation)
    lateral.z = 0.0
    lateral = lateral.normalized()
    devant = lateral.cross(Vector((0.0, 0.0, 1.0))).normalized()
    orteil = ((arm.matrix_world @ arm.pose.bones["LeftToeBase"].matrix).translation
              - (arm.matrix_world @ arm.pose.bones["LeftFoot"].matrix).translation)
    orteil.z = 0.0
    if devant.dot(orteil.normalized()) < 0.0:
        devant = -devant

    os_oreille = [f"oreille_{c}_{i:02d}" for c in ("g", "d") for i in (1, 2)]
    axes = {nom: axes_de(arm, nom, devant, lateral) for nom in os_oreille}
    for nom in ("oeil_g", "oeil_d", "machoire"):
        if nom in arm.pose.bones:
            arm.pose.bones[nom].rotation_mode = "XYZ"

    # La mâchoire : le sens qui OUVRE, mesuré (cf. `36_rig_chien.py`).
    m = arm.pose.bones["machoire"]
    hauteurs = {}
    for signe in (1.0, -1.0):
        m.rotation_euler = (math.radians(20.0) * signe, 0.0, 0.0)
        bpy.context.view_layer.update()
        hauteurs[signe] = (arm.matrix_world @ arm.pose.bones["machoire"].tail).z
    sens_gueule = min(hauteurs, key=lambda s: hauteurs[s])
    m.rotation_euler = (0.0, 0.0, 0.0)
    bpy.context.view_layer.update()

    # Les clés de forme ont leur propre porteur d'animation. Pour que l'export
    # glTF réunisse os et morph dans UN clip, les deux actions doivent vivre sur
    # des pistes NLA de MÊME NOM.
    cles = peau.data.shape_keys
    if cles.animation_data is None:
        cles.animation_data_create()
    for piste in list(cles.animation_data.nla_tracks):
        cles.animation_data.nla_tracks.remove(piste)

    rapport = {}
    for nom_action, spec in ETATS.items():
        action = bpy.data.actions.get(nom_action)
        if action is None:
            rapport[nom_action] = {"absent": True}
            continue
        debut, fin = action.frame_range
        pistes = 0

        for nom_os in os_oreille:
            pointe = nom_os.endswith("02")
            decalage = RETARD_POINTE if pointe else 0.0
            attenuation = 0.75 if pointe else 1.0
            for canal in ("balancement", "battement"):
                idx, signe = axes[nom_os][canal]
                if valeur_de(spec, canal, 0.0, decalage) is None:
                    continue

                def f(t, canal=canal, decalage=decalage, signe=signe,
                      attenuation=attenuation):
                    v = valeur_de(spec, canal, t, decalage)
                    return math.radians(v * signe * attenuation)

                courbe(action, f'pose.bones["{nom_os}"].rotation_euler', idx,
                       echantillonne(debut, fin, f), nom_os)
                pistes += 1

        for nom_oeil, sens in (("oeil_g", 1.0), ("oeil_d", 1.0)):
            if nom_oeil not in arm.pose.bones:
                continue
            for canal, idx in (("regard_site", 0), ("regard_lacet", 2)):
                if valeur_de(spec, canal, 0.0) is None:
                    continue

                def f(t, canal=canal, sens=sens):
                    return math.radians(valeur_de(spec, canal, t) * sens)

                courbe(action, f'pose.bones["{nom_oeil}"].rotation_euler', idx,
                       echantillonne(debut, fin, f), nom_oeil)
                pistes += 1

        if "machoire" in arm.pose.bones:
            base = spec.get("machoire_base", 0.0)

            def f(t, base=base):
                v = valeur_de(spec, "machoire", t)
                return math.radians((base + (v or 0.0)) * sens_gueule)

            if base or valeur_de(spec, "machoire", 0.0) is not None:
                courbe(action, 'pose.bones["machoire"].rotation_euler', 0,
                       echantillonne(debut, fin, f), "machoire")
                pistes += 1

        # Le clignement, dans une action de clés de forme au MÊME nom de piste.
        cles_paupiere = poser_clignements(spec, debut, fin, 24)
        if len(cles_paupiere) > 2:
            nom_cle = f"{nom_action}_cligner"
            ancienne = bpy.data.actions.get(nom_cle)
            if ancienne:
                bpy.data.actions.remove(ancienne)
            act_cle = bpy.data.actions.new(nom_cle)
            fc = act_cle.fcurves.new(data_path='key_blocks["cligner"].value',
                                     index=0)
            fc.keyframe_points.add(count=len(cles_paupiere))
            for point, (frame, valeur) in zip(fc.keyframe_points, cles_paupiere):
                point.co = (frame, valeur)
                point.interpolation = "BEZIER"
            fc.update()
            piste = cles.animation_data.nla_tracks.new()
            piste.name = nom_action
            bande = piste.strips.new(nom_action, int(debut), act_cle)
            bande.frame_end = fin
            pistes += 1

        rapport[nom_action] = {"plage": [round(debut, 1), round(fin, 1)],
                               "pistes_ajoutees": pistes,
                               "courbes_totales": len(action.fcurves)}

    print("RESULT: " + json.dumps({
        "sens_gueule": sens_gueule,
        "axes_oreille": {k: {"balancement": v["balancement"],
                             "battement": v["battement"]}
                         for k, v in axes.items()},
        "etats": rapport,
    }, ensure_ascii=False))


main()
