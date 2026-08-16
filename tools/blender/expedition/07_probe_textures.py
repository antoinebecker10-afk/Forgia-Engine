"""Que valent vraiment les 17 jeux de terrain ? Teinte, clarté, contraste.

Choisir « mossy_rock » parce que le nom sonne bien, c'est ce qui a donné une
falaise gris-noir-vert. On mesure donc chaque jeu : sa couleur moyenne, sa
luminance, et son écart-type — une roche qui manque de contraste paraît sale,
une trop sombre avale la lumière.

Les images sont réduites à 32x32 avant lecture : lire 4 M de pixels par jeu
coûterait des minutes pour une moyenne qu'un vignettage donne aussi bien.
"""

import json
import math
import os

import bpy

TEXTURES = r"C:\Users\Antoi\Desktop\Forgia Rewrite\assets\textures-v1\terrain"
TAILLE = 32


def lineaire_srgb(c):
    return 12.92 * c if c <= 0.0031308 else 1.055 * (c ** (1 / 2.4)) - 0.055


def mesurer(dossier):
    chemin = os.path.join(TEXTURES, dossier, "diff.jpg")
    if not os.path.exists(chemin):
        chemin = os.path.join(TEXTURES, dossier, "diff.png")
    if not os.path.exists(chemin):
        return None
    img = bpy.data.images.load(chemin, check_existing=False)
    try:
        img.scale(TAILLE, TAILLE)
        px = list(img.pixels)
    finally:
        bpy.data.images.remove(img)

    n = TAILLE * TAILLE
    somme = [0.0, 0.0, 0.0]
    lum = []
    for i in range(n):
        r, g, b = px[i * 4], px[i * 4 + 1], px[i * 4 + 2]
        somme[0] += r
        somme[1] += g
        somme[2] += b
        lum.append(0.2126 * r + 0.7152 * g + 0.0722 * b)
    moy = [s / n for s in somme]
    moy_lum = sum(lum) / n
    ecart = math.sqrt(sum((l - moy_lum) ** 2 for l in lum) / n)
    srgb = [lineaire_srgb(max(0.0, min(1.0, c))) for c in moy]
    return {
        "jeu": dossier,
        "hex": "#%02X%02X%02X" % tuple(int(round(c * 255)) for c in srgb),
        "luminance": round(moy_lum, 4),
        "contraste": round(ecart, 4),
        # Saturation grossière : 0 = gris parfait.
        "saturation": round((max(moy) - min(moy)) / (max(moy) + 1e-6), 3),
    }


resultats = []
for d in sorted(os.listdir(TEXTURES)):
    if os.path.isdir(os.path.join(TEXTURES, d)):
        m = mesurer(d)
        if m:
            resultats.append(m)
resultats.sort(key=lambda e: -e["luminance"])
print("RESULT: " + json.dumps(resultats, ensure_ascii=False))
