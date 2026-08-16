"""Ramène les textures du personnage stylisé en 1K. Python pur, sans Blender.

    python tools/blender/personnage/21_textures_personnage.py

DEUX GAINS, pas un seul.

1. **La définition.** Les normales font 6 à 7 Mo pièce. Le pipeline a déjà fait
   ses preuves sur le château : 69,2 → 5,3 Mo (−92 %).

2. **Le doublon DirectX / OpenGL.** Le pack livre CHAQUE normale en deux
   conventions — elles ne diffèrent que par le signe du canal vert. glTF (donc
   Bevy) attend la convention **OpenGL**. Emporter les deux, c'est doubler le
   poste le plus lourd pour un fichier qui ne servira jamais.

On ignore aussi `Height` : le glTF de base ne transporte pas de parallaxe, et
Bevy ne la consomme pas ici. L'emporter serait du poids mort.
"""

import os
import sys

from PIL import Image

BASE = r"D:\ressources externes\FAB\fbx_stylizedfantasycharacters (1)"
SORTIE = r"C:\Users\Antoi\Desktop\Forgia Rewrite\assets\textures\personnage_1k"
COTE = 1024

# Les trois matières du personnage, telles que nommées dans le FBX : Armor,
# Cloth, Organik. Pour chacune : couleur, normale (OpenGL), rugosité, AO.
MATIERES = ["Armor", "Cloth", "Organik"]
CARTES = [
    ("BaseColor", "bc"),
    ("Normal_OpenGL", "n"),     # DirectX ignoré : glTF attend OpenGL
    ("Rough", "r"),
    ("_AO", "ao"),
    ("Metalic", "m"),
]


def main():
    src_dir = os.path.join(BASE, "T_Male")
    if not os.path.isdir(src_dir):
        sys.exit(f"source absente : {src_dir}")
    os.makedirs(SORTIE, exist_ok=True)

    avant = apres = 0
    ignore = 0
    print(f"{'fichier':30s} {'source':>10s}  ->  {'1024':>8s}  {'gain':>5s}")
    for matiere in MATIERES:
        for suffixe, court in CARTES:
            nom = f"T_{matiere}_{suffixe}.png" if not suffixe.startswith("_") \
                else f"T_{matiere}_{suffixe}.png"
            src = os.path.join(src_dir, nom)
            if not os.path.exists(src):
                print(f"{nom:30s}   ABSENT")
                continue
            a = os.path.getsize(src)
            # Une carte quasi vide (métallique tout noir) ne merite pas 1 Mo :
            # on la garde, mais son poids tombe de lui-meme a la reduction.
            dest = os.path.join(SORTIE, f"{matiere.lower()}_{court}.png")
            with Image.open(src) as im:
                mode = "RGB" if im.mode in ("RGB", "L", "P") else "RGBA"
                im.convert(mode).resize((COTE, COTE), Image.LANCZOS) \
                  .save(dest, optimize=True)
            b = os.path.getsize(dest)
            avant += a
            apres += b
            print(f"{nom:30s} {a/1e6:7.2f} Mo  ->  {b/1e6:5.2f} Mo  {100*(a-b)//a:4d} %")

    # Ce qu'on a volontairement laisse de cote.
    for matiere in MATIERES:
        for saute in (f"T_{matiere}_Normal_DirectX.png", f"T_{matiere}_Height.png"):
            p = os.path.join(src_dir, saute)
            if os.path.exists(p):
                ignore += os.path.getsize(p)

    print(f"\nRETENU  {avant/1e6:.1f} Mo -> {apres/1e6:.1f} Mo "
          f"({100*(avant-apres)//max(1,avant)} % en moins)")
    print(f"IGNORE  {ignore/1e6:.1f} Mo (normales DirectX + cartes de hauteur)")
    print(f"Ecrit dans {SORTIE}")


main()
