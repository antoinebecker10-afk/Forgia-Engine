"""Pourquoi la ceinture s'affiche en damier ? Et le bruit de relief est-il mort ?

Deux questions, une passe. On mesure au lieu de supposer : un damier peut être
une texture manquante, un atlas balayé par des UV trop larges, ou la texture
réelle du kit — et ces trois causes n'ont pas le même remède.
"""

import json

import bpy
import mathutils
from mathutils import Vector

RAPPORT = {}

# --- 1. De quoi est faite une pièce de falaise ? --------------------------
bpy.ops.object.select_all(action="SELECT")
bpy.ops.object.delete(use_global=False)
for coll in (bpy.data.meshes, bpy.data.materials, bpy.data.images):
    for block in list(coll):
        try:
            coll.remove(block)
        except (RuntimeError, ReferenceError):
            pass

bpy.ops.import_scene.gltf(
    filepath=r"C:\Users\Antoi\Desktop\Forgia Rewrite\assets\models\kenney\nature\cliff_block_stone.glb"
)
pieces = []
for obj in bpy.data.objects:
    if obj.type != "MESH":
        continue
    mesh = obj.data
    uv = mesh.uv_layers.active
    us = [uv.data[i].uv[0] for i in range(len(uv.data))] if uv else []
    vs = [uv.data[i].uv[1] for i in range(len(uv.data))] if uv else []
    infos = {
        "objet": obj.name,
        "uv_u": [round(min(us), 4), round(max(us), 4)] if us else None,
        "uv_v": [round(min(vs), 4), round(max(vs), 4)] if vs else None,
        "materiaux": [],
    }
    for slot in obj.material_slots:
        mat = slot.material
        if not mat:
            continue
        entree = {"nom": mat.name, "noeuds": bool(mat.use_nodes), "images": []}
        if mat.use_nodes:
            for node in mat.node_tree.nodes:
                if node.type == "TEX_IMAGE":
                    img = node.image
                    entree["images"].append({
                        "nom": img.name if img else None,
                        "taille": list(img.size) if img else None,
                        "pixels_charges": bool(img.has_data) if img else False,
                        "packe": (img.packed_file is not None) if img else False,
                        "source": img.source if img else None,
                    })
                if node.type == "BSDF_PRINCIPLED":
                    couleur = node.inputs["Base Color"]
                    entree["base_color"] = [round(c, 3) for c in couleur.default_value]
                    entree["base_color_liee"] = bool(couleur.is_linked)
        infos["materiaux"].append(entree)
    pieces.append(infos)
RAPPORT["falaise"] = pieces

# --- 2. Le bruit est-il vivant à la graine utilisée ? ---------------------
# La graine 20260813 était injectée telle quelle comme coordonnée Z du bruit :
# 20 260 en entrée d'un Perlin, c'est très loin de son domaine utile.
echantillons = {}
for etiquette, z in (("z_20260 (actuel)", 20260.813), ("z_36 (propose)", 36.4)):
    vals = []
    for i in range(12):
        x = (-104.0 + i * 16.0) * 0.017
        vals.append(round(mathutils.noise.noise(Vector((x, 0.3, z))), 4))
    echantillons[etiquette] = {
        "valeurs": vals,
        "amplitude": round(max(vals) - min(vals), 5),
    }
RAPPORT["bruit"] = echantillons

print("RESULT: " + json.dumps(RAPPORT, ensure_ascii=False))
