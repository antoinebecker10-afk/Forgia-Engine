"""CAPTURE ce qu'Antoine a déplacé à la main, pour que la cuisson le garde.

  1. `vallon.py`        bâtit la carte et écrit l'instantané des poses
  2. tu déplaces librement dans Blender (G / R / S ; N pour les valeurs exactes)
  3. `20_retouches.py`  ← ce script : il compare et n'écrit QUE les différences
  4. `vallon.py`        les réapplique à la cuisson suivante

  python tools/blender/bmcp.py code tools/blender/expedition/20_retouches.py

# Pourquoi ce détour plutôt que de sauver le .blend

Parce que la carte est BÂTIE PAR DES RÈGLES, et que c'est ce qui lui permet de
suivre quand le tracé, le relief ou le kit changent. Sauver la scène gèlerait
tout : la première modification de règle ferait diverger le .blend du code, et
on retomberait sur le défaut n°1 du projet — une grandeur écrite deux fois.

On garde donc les règles ET les exceptions, séparées. Le déplacement à la main
devient une DONNÉE, versionnable, relisible, et qui se périme bruyamment quand
la règle qu'elle corrigeait a changé.

# Ce que le fichier de sortie contient

Pour chaque pièce déplacée : sa pose AVANT (celle que la règle avait produite)
et sa pose APRÈS (la tienne). L'`avant` n'est pas décoratif — c'est lui qui
permet à la cuisson de vérifier qu'elle réapplique la retouche à la BONNE
pièce. Sans lui, un changement de semis suffirait à déplacer un arbre au
hasard, en silence.
"""

import json
import math
import os

import bpy

RACINE = r"C:\Users\Antoi\Desktop\Forgia Rewrite"
SORTIE = os.path.join(RACINE, "assets", "models", "environment", "expedition")
REFERENCE = os.path.join(SORTIE, "vallon_pose_reference.json")
RETOUCHES = os.path.join(SORTIE, "vallon_retouches.json")

# En deçà, ce n'est pas un déplacement voulu — c'est du bruit de manipulation
# (un clic qui a glissé, une valeur retapée à l'identique). 1 cm en position,
# et l'équivalent en rotation sur une pièce de 1 m.
SEUIL_POS_M = 0.01
SEUIL_ROT_RAD = 0.01
SEUIL_ECHELLE = 0.005

ECARTEES = {"_proto", "_src", "faune_apercu", "faune_controle", "collisions"}


def objets_autores():
    for obj in bpy.data.objects:
        if obj.type != "MESH":
            continue
        if any(c.name in ECARTEES for c in obj.users_collection):
            continue
        yield obj


def pose_de(obj):
    return [round(obj.location.x, 3), round(obj.location.y, 3),
            round(obj.location.z, 3), round(obj.rotation_euler.z, 4),
            round(obj.scale.x, 4)]


def main():
    if not os.path.exists(REFERENCE):
        print("RESULT: " + json.dumps({
            "erreur": "aucun instantané de référence",
            "remede": "relancer vallon.py — c'est lui qui l'écrit en fin de cuisson",
        }, ensure_ascii=False))
        return
    with open(REFERENCE, encoding="utf-8") as fh:
        reference = json.load(fh)

    # On REPART des retouches existantes : capturer deux fois de suite ne doit
    # pas effacer la session précédente. Une pièce remise exactement à sa place
    # d'origine, elle, voit sa retouche RETIRÉE — c'est ainsi qu'on annule.
    anciennes = {}
    if os.path.exists(RETOUCHES):
        with open(RETOUCHES, encoding="utf-8") as fh:
            anciennes = json.load(fh)

    nouvelles = dict(anciennes)
    bougees, annulees, hors_reference = [], [], []
    for obj in objets_autores():
        avant = reference.get(obj.name)
        if avant is None:
            hors_reference.append(obj.name)
            continue
        apres = pose_de(obj)
        d_pos = math.dist(apres[:3], avant[:3])
        d_rot = abs(apres[3] - avant[3])
        d_ech = abs(apres[4] - avant[4])
        change = (d_pos > SEUIL_POS_M or d_rot > SEUIL_ROT_RAD
                  or d_ech > SEUIL_ECHELLE)
        if change:
            # `avant` reste celui de la RÉFÉRENCE, pas celui de la retouche
            # précédente : c'est la pose que la règle produit aujourd'hui, donc
            # la seule contre laquelle la reconnaissance a un sens.
            nouvelles[obj.name] = {"avant": avant, "apres": apres,
                                   "ecart_m": round(d_pos, 3)}
            bougees.append({"objet": obj.name, "ecart_m": round(d_pos, 2),
                            "rot_deg": round(math.degrees(d_rot), 1),
                            "echelle": round(d_ech, 3)})
        elif obj.name in nouvelles:
            # Remise à sa place d'origine = la retouche est annulée. C'est le
            # geste naturel pour revenir en arrière, il doit marcher.
            del nouvelles[obj.name]
            annulees.append(obj.name)

    with open(RETOUCHES, "w", encoding="utf-8") as fh:
        json.dump(nouvelles, fh, ensure_ascii=False, indent=1, sort_keys=True)

    bougees.sort(key=lambda r: -r["ecart_m"])
    print("RESULT: " + json.dumps({
        "objets_examines": len(reference),
        "retouches_totales": len(nouvelles),
        "nouvelles_ou_modifiees": len(bougees),
        "annulees": len(annulees),
        "hors_reference": len(hors_reference),
        "fichier": os.path.basename(RETOUCHES),
        "detail": bougees[:20],
    }, ensure_ascii=False))


main()
