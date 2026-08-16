"""Ramène les textures d'environnement du château en 1024. Python pur, sans Blender.

    python tools/blender/expedition/13_textures_1k.py

POURQUOI HORS BLENDER. L'API image de Blender charge en différé : `scale()`
fixe la taille sans remplir le tampon, et `save()` échoue ensuite sur « does
not have any image data ». Forcer le chargement n'a pas suffi. PIL fait le
travail en trois lignes et en une fraction du temps — inutile de s'acharner sur
l'outil qui résiste quand un autre est déjà installé.

POURQUOI 1024. Les originales pèsent 7 à 17 Mo pièce (normale de l'herbe :
16,7 Mo). Le jeu complet en pleine définition ajouterait ~60 Mo à une carte
dont le visuel entier fait 78 Mo, et qu'on vient de découper en cellules pour
l'alléger. À 1024 sur une tuile de 4 m, on est à ~2,5 texels/cm — au-delà de ce
que l'œil distingue depuis 1,70 m.
"""

import os
import sys

from PIL import Image

SOURCE = r"D:\ressources externes\FAB\fbx_and_textures_fantastic_highlands_castle\2d"
SORTIE = r"C:\Users\Antoi\Desktop\Forgia Rewrite\assets\textures\castle_1k"
COTE = 1024

# (fichier, nom court). L'albédo de la falaise n'existe pas à la source — sa
# couleur viendra de la pierre.
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


def main():
    if not os.path.isdir(SOURCE):
        sys.exit(f"source absente : {SOURCE}")
    os.makedirs(SORTIE, exist_ok=True)

    avant_total = apres_total = 0
    print(f"{'texture':14s} {'source':>12s}  {'->':^4s} {'1024':>9s}   {'gain':>6s}")
    for fichier, court in TEXTURES:
        src = os.path.join(SOURCE, fichier)
        if not os.path.exists(src):
            print(f"{court:14s}   ABSENTE : {fichier}")
            continue
        dest = os.path.join(SORTIE, f"{court}.png")
        with Image.open(src) as im:
            taille = im.size
            # LANCZOS : le rééchantillonnage de référence pour une réduction.
            # Sur une normale il introduit une dénormalisation infime, sans
            # effet visible à cette échelle de réduction.
            im.convert("RGBA" if "a" in im.mode.lower() else "RGB") \
              .resize((COTE, COTE), Image.LANCZOS) \
              .save(dest, optimize=True)
        a, b = os.path.getsize(src), os.path.getsize(dest)
        avant_total += a
        apres_total += b
        print(f"{court:14s} {a/1e6:9.2f} Mo  ->  {b/1e6:6.2f} Mo   {100*(a-b)//a:4d} %  "
              f"({taille[0]}x{taille[1]})")

    print(f"\nTOTAL {avant_total/1e6:.1f} Mo -> {apres_total/1e6:.1f} Mo "
          f"({100*(avant_total-apres_total)//max(1,avant_total)} % en moins)")
    print(f"Ecrit dans {SORTIE}")


main()
