"""Extrait du château ce dont Le Vallon a besoin : ses textures et ses ponts.

    & "C:\\Program Files\\Blender Foundation\\Blender 4.5\\blender.exe" ^
      --background --factory-startup ^
      --python tools/blender/expedition/12_kit_chateau.py

À lancer en instance SÉPARÉE : ce script ouvre une bibliothèque de 368 Mo, il
n'a rien à faire dans la scène du Vallon.

DEUX SORTIES

1. `assets/textures/castle_1k/` — les textures d'environnement du château,
   **ramenées en 1024**. Les originales pèsent 7 à 17 Mo pièce (la normale de
   l'herbe seule : 16,7 Mo). Prendre le jeu complet en pleine définition
   ajouterait ~60 Mo à une carte dont le visuel entier fait 78 Mo, et qu'on
   vient justement de découper en cellules pour l'alléger. 1024 suffit
   largement pour un sol vu depuis 1,70 m — c'est ~1 texel par cm à 3 m.
   (Parade déjà documentée en V1 : `pattern_glb_texture_resize_offline`.)

2. `assets/models/environment/castle_kit/` — les modules de pont de pierre,
   avec leurs colliders fournis. On assemble ensuite base + N extensions selon
   la largeur RÉELLE du chenal, au lieu de fabriquer un tablier à la main.

Piège relevé à l'inventaire : `T_ENV_cliff_castle_01` n'existe qu'en NORMALE,
sans albédo. Sa couleur doit venir d'ailleurs (`T_ENV_stone_castle_01_BC`).
"""

import json
import os

import bpy
from mathutils import Vector

FAB = r"D:\ressources externes\FAB\fbx_and_textures_fantastic_highlands_castle"
BLEND = os.path.join(FAB, "fantastic_highlands_castle_blend.blend")
IMAGES = os.path.join(FAB, "2d")
RACINE = r"C:\Users\Antoi\Desktop\Forgia Rewrite"
SORTIE_TEX = os.path.join(RACINE, "assets", "textures", "castle_1k")
SORTIE_KIT = os.path.join(RACINE, "assets", "models", "environment", "castle_kit")

COTE = 1024

# (fichier source, nom court). L'albédo de la falaise manque à la source : on
# prendra celui de la pierre, d'où l'absence de `cliff_BC` ici.
TEXTURES = [
    ("T_ENV_grass_castle_BC.png", "grass_bc"),
    ("T_ENV_grass_castle_N.png", "grass_n"),
    ("T_ENV_ground_castle_BC.png", "ground_bc"),
    ("T_ENV_ground_castle_N.png", "ground_n"),
    ("T_ENV_pavement_castle_BC.png", "pavement_bc"),
    ("T_ENV_pavement_castle_N.png", "pavement_n"),
    ("T_ENV_stone_castle_01_BC.png", "stone_bc"),
    ("T_ENV_stone_castle_01_N.png", "stone_n"),
    ("T_ENV_cliff_castle_01_N.png", "cliff_n"),
]

# LOD0 seulement : les LOD1/2 sont pour un système de LOD qu'on n'a pas encore.
MODULES = [
    "SM_MOD_bridge_base_castle_LOD0",
    "SM_MOD_bridge_base_ext_castle_LOD0",
    # Noms RELEVES dans la bibliotheque, pas devines : « railing_01_castle »
    # n'existe pas, la lisse droite s'appelle « railing_01_straight_castle ».
    "SM_MOD_railing_01_straight_castle_LOD0",
    "SM_MOD_railing_01_straight_half_castle_LOD0",
    "SM_MOD_railing_01_diagonal_castle_LOD0",
    # Rochers du chateau — la meme matiere que le Hall. Ils serviront a fermer
    # les creux de la ceinture du Vallon.
    "SM_ENV_cliff_castle_01_LOD0",
    "SM_ENV_cliff_castle_02_LOD0",
]


def bornes(obj):
    pts = [obj.matrix_world @ Vector(c) for c in obj.bound_box]
    return [round(max(p[i] for p in pts) - min(p[i] for p in pts), 3) for i in range(3)]


def reduire_textures():
    os.makedirs(SORTIE_TEX, exist_ok=True)
    faits = []
    for source, court in TEXTURES:
        chemin = os.path.join(IMAGES, source)
        if not os.path.exists(chemin):
            faits.append({"texture": court, "erreur": "absente"})
            continue
        img = bpy.data.images.load(chemin, check_existing=False)
        # Blender charge les images en DIFFERE : `scale()` fixe alors la taille
        # sans tampon, et `save()` echoue sur « does not have any image data ».
        # Lire un seul pixel force le chargement (le transfert vers Python ne
        # porte que sur ce pixel, pas sur les 16 M de la 4K).
        if not img.has_data:
            _ = img.pixels[0]
        avant = list(img.size)
        octets_avant = os.path.getsize(chemin)
        img.scale(COTE, COTE)
        # Les normales et les cartes PBR ne sont PAS de la couleur : les passer
        # en sRGB les fausserait silencieusement.
        img.colorspace_settings.name = "Non-Color" if court.endswith(("_n", "_pbr")) else "sRGB"
        dest = os.path.join(SORTIE_TEX, f"{court}.png")
        img.filepath_raw = dest
        img.file_format = "PNG"
        img.save()
        bpy.data.images.remove(img)
        faits.append({
            "texture": court, "avant_px": avant, "apres_px": [COTE, COTE],
            "avant_mo": round(octets_avant / 1e6, 2),
            "apres_mo": round(os.path.getsize(dest) / 1e6, 2),
        })
    return faits


def extraire_modules():
    os.makedirs(SORTIE_KIT, exist_ok=True)
    # On charge aussi les colliders : le château les fournit, autant ne pas
    # re-fabriquer un proxy qui existe déjà.
    voulus = list(MODULES) + [m.replace("_LOD0", "_Collider") for m in MODULES]
    # `SM_MOD_railing_01_castle_LOD0` n'existe pas sous ce nom : on releve les
    # candidats reels plutot que d'en inventer un.
    with bpy.data.libraries.load(BLEND, link=False) as (src, _d):
        candidats = sorted(o for o in src.objects
                           if "railing" in o.lower() and o.endswith("_LOD0"))[:10]
    # On suit les objets CREES par le chargement, au lieu de les rechercher par
    # nom : Blender suffixe « .001 » en cas de collision, et le filtre par nom
    # laissait alors les colliders de cote sans rien signaler.
    avant = set(bpy.data.objects)
    with bpy.data.libraries.load(BLEND, link=False) as (src, dst):
        dispo = set(src.objects)
        dst.objects = [n for n in voulus if n in dispo]
        manquants = [n for n in voulus if n not in dispo]
    poses = [o for o in bpy.data.objects if o not in avant]
    for obj in poses:
        bpy.context.scene.collection.objects.link(obj)
    # Sans cette mise a jour, `select_set` ne prend pas dans une passe
    # `--background` : l'export sortait un glTF vide de 388 octets.
    bpy.context.view_layer.update()

    rapport = []
    for obj in poses:
        if obj.type != "MESH":
            continue
        obj.data.calc_loop_triangles()
        rapport.append({
            "module": obj.name,
            "emprise_m": bornes(obj),
            "tris": len(obj.data.loop_triangles),
            "materiaux": [m.name if m else None for m in obj.data.materials],
        })

    if poses:
        bpy.ops.object.select_all(action="DESELECT")
        for o in poses:
            o.select_set(True)
        bpy.context.view_layer.objects.active = poses[0]
        chemin = os.path.join(SORTIE_KIT, "castle_bridge_kit.glb")
        bpy.ops.export_scene.gltf(
            filepath=chemin, export_format="GLB", use_selection=True,
            export_apply=True, export_yup=True,
            # SANS MATERIAUX. Avec, le GLB re-embarque les textures 2K/4K du
            # chateau : 37,5 Mo pour 15 522 triangles de pont. La pierre 1K
            # (castle_1k/stone_*) est reappliquee au montage, cote carte.
            export_materials="NONE",
        )
        octets = os.path.getsize(chemin)
    else:
        octets = 0
    return rapport, manquants, octets, candidats


def main():
    if not os.path.exists(BLEND):
        print("RESULT: " + json.dumps({"erreur": f"absent : {BLEND}"}))
        return
    # Les textures sont traitees par `13_textures_1k.py` (PIL, hors Blender) :
    # l'API image de Blender chargeait en differe et refusait de sauver.
    tex = []
    modules, manquants, octets, candidats = extraire_modules()
    print("RESULT: " + json.dumps({
        "textures": tex,
        "textures_mo_total": round(sum(t.get("apres_mo", 0) for t in tex), 2),
        "modules": modules,
        "modules_manquants": manquants,
        "kit_octets": octets,
        "lisses_disponibles": candidats,
    }, ensure_ascii=False))


main()
