"""Que contient `animals_free.blend` ? Sans l'ouvrir.

Ouvrir le fichier remplacerait la scene du Vallon en cours. `libraries.load`
permet de LISTER le contenu d'une bibliotheque sans rien importer.

La question qui decide de tout : y a-t-il des ARMATURES et des ACTIONS ? Le
registre `packs.toml` prevenait qu'un pack d'animaux envisage etait « .obj
static only, no rig, no anims ». Un animal sans squelette ne se balade pas — il
glisse. Autant le savoir avant de promettre de la faune vivante.
"""

import json
import os

import bpy

CHEMIN = r"D:\ressources externes\FAB\animals_free.blend"

if not os.path.exists(CHEMIN):
    print("RESULT: " + json.dumps({"erreur": f"absent : {CHEMIN}"}, ensure_ascii=False))
else:
    with bpy.data.libraries.load(CHEMIN, link=False) as (source, _cible):
        contenu = {
            "objets": list(source.objects),
            "actions": list(source.actions),
            "armatures": list(source.armatures),
            "meshes": list(source.meshes),
            "materiaux": list(source.materials),
            "collections": list(source.collections),
            "images": list(source.images),
        }
    print("RESULT: " + json.dumps({
        "fichier": CHEMIN,
        "octets": os.path.getsize(CHEMIN),
        "nb": {k: len(v) for k, v in contenu.items()},
        # Le verdict : sans armature ni action, pas de deambulation credible.
        "rigge": bool(contenu["armatures"]),
        "anime": bool(contenu["actions"]),
        "objets": sorted(contenu["objets"])[:60],
        "actions": sorted(contenu["actions"])[:40],
        "armatures": sorted(contenu["armatures"])[:20],
        "collections": sorted(contenu["collections"])[:20],
    }, ensure_ascii=False))
