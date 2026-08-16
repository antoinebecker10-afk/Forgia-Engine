"""Mesure le kit Kenney Nature : pas de tuile, emprises, position de l'origine.

Rien ne se place tant que ceci n'a pas parlé. Un kit modulaire assemblé sur un
pas SUPPOSÉ produit des joints ouverts sur toute la carte, et le défaut ne se
voit qu'une fois 400 tuiles posées.

On mesure aussi OÙ est l'origine dans l'emprise : une tuile dont l'origine est
au coin ne se pose pas comme une tuile centrée, et l'erreur est d'un demi-pas.
"""

import json
import os

import bpy

KIT = r"C:\Users\Antoi\Desktop\Forgia Rewrite\assets\models\kenney\nature"

# Échantillon choisi pour couvrir chaque grammaire du kit, pas pour être exhaustif :
# le sol (pas de grille), le chemin et la rivière (mêmes tuiles ?), le relief,
# la végétation (hauteurs réelles), et le mobilier de village.
SAMPLE = [
    "ground_grass.glb",
    "ground_pathStraight.glb",
    "ground_pathBend.glb",
    "ground_pathCross.glb",
    "ground_riverStraight.glb",
    "ground_riverBend.glb",
    "cliff_block_rock.glb",
    "cliff_rock.glb",
    "cliff_blockSlope_rock.glb",
    "cliff_steps_rock.glb",
    "cliff_waterfall_rock.glb",
    "bridge_wood.glb",
    "bridge_stone.glb",
    "tree_default.glb",
    "tree_oak.glb",
    "tree_detailed.glb",
    "tree_pineDefaultA.glb",
    "tree_palm.glb",
    "stump_round.glb",
    "rock_smallA.glb",
    "flower_redA.glb",
    "mushroom_red.glb",
    "grass_large.glb",
    "campfire_stones.glb",
    "fence_simple.glb",
    "crops_wheatStageB.glb",
    "statue_head.glb",
    "path_stone.glb",
]


def wipe():
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)
    for block in list(bpy.data.meshes):
        bpy.data.meshes.remove(block)


def measure(filename):
    """Importe un GLB seul et rend son emprise monde + la place de son origine."""
    path = os.path.join(KIT, filename)
    if not os.path.exists(path):
        return {"file": filename, "erreur": "absent"}

    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=path)
    fresh = [o for o in bpy.data.objects if o not in before]
    meshes = [o for o in fresh if o.type == "MESH"]
    if not meshes:
        return {"file": filename, "erreur": "aucun mesh"}

    lo = [float("inf")] * 3
    hi = [float("-inf")] * 3
    tris = 0
    for obj in meshes:
        tris += len(obj.data.loop_triangles) or len(obj.data.polygons)
        for corner in obj.bound_box:
            world = obj.matrix_world @ __import__("mathutils").Vector(corner)
            for axis in range(3):
                lo[axis] = min(lo[axis], world[axis])
                hi[axis] = max(hi[axis], world[axis])

    return {
        "file": filename,
        "meshes": len(meshes),
        "tris": tris,
        # Blender est Z-up, le glTF Y-up : l'import convertit, donc Z = hauteur ici.
        "dim_xyz": [round(hi[i] - lo[i], 4) for i in range(3)],
        "min_xyz": [round(lo[i], 4) for i in range(3)],
        "max_xyz": [round(hi[i], 4) for i in range(3)],
    }


wipe()
rapport = []
for name in SAMPLE:
    rapport.append(measure(name))
    wipe()

print("RESULT: " + json.dumps(rapport, ensure_ascii=False))
