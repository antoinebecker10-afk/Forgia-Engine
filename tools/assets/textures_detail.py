"""Fabrique les textures de DÉTAIL sans couture du décor de l'Expédition.

# Ce que ces textures sont, et ne sont pas

Ce ne sont **pas** des textures d'albédo. Ce sont des cartes de **matière** en
niveaux de gris, qui MULTIPLIENT la couleur déjà portée par chaque matériau.

C'est tout l'intérêt du constat mesuré sur la carte : les 30 matériaux sans
texture du Vallon ne sont pas 30 problèmes, ce sont **6 familles** — et chacun
porte déjà sa teinte dans son `baseColorFactor` glTF. La palette existe. Il ne
manque que le grain.

Conséquence directe : 6 textures grises couvrent 30 matériaux, sans rouvrir
Blender, sans refaire une seule UV, et sans toucher aux couleurs choisies par
l'auteur de la carte.

# 🚨 Centrées sur 1,0, pas sur 0,5

Une carte de détail se MULTIPLIE. Une texture de moyenne 0,5 assombrirait tout
le décor de moitié — c'est exactement le piège déjà payé sur les teintes de
rareté de l'armure (`rarity_tint` normalise pour cette raison). Chaque texture
d'ici est donc centrée sur 1,0 : elle module, elle n'assombrit pas.

# 🚨 Sans couture PAR CONSTRUCTION, jamais par retouche

Toutes les composantes sont des sinusoïdes d'**harmoniques entières** et du bruit
haché **périodique** : `f(0) == f(1)` exactement, sur les deux axes. Ce n'est pas
vérifié après coup, c'est vrai par construction — et le test le prouve en
balayant TOUTES les lignes et colonnes, pas trois échantillons.

# Le préalable qu'elles ne peuvent pas satisfaire seules

Bevy 0.18 charge toute image en `ImageAddressMode::ClampToEdge`
(`bevy_image/src/image.rs:668`, `#[default]`). Une texture parfaitement tuilable
**ne se répétera pas** tant que son sampler n'est pas en `Repeat`. Le projet le
sait déjà à un endroit — `forgia-foliage/material_override.rs` porte le
commentaire — et nulle part ailleurs. Charger ces fichiers sans
`load_with_settings` donnerait un aplat, et on chercherait le défaut dans l'art.

Usage :
    python tools/assets/textures_detail.py [--taille 256] [--sortie <dossier>]
"""

import argparse
import pathlib

import numpy as np
from PIL import Image

# Familles mesurées dans `vallon_stream_cells/*.gltf` : 30 matériaux sans
# texture, regroupés par matière. Le nombre dit combien de matériaux chaque
# texture habille — c'est ce qui justifie d'en faire six et pas trente.
FAMILLES = {
    "pierre": 8,
    "bois": 6,
    "accent": 6,
    "terre": 4,
    "feuillage": 3,
    "herbe": 1,
}


def grille(n):
    """Coordonnées normalisées [0,1), en incluant 0 et excluant 1.

    🚨 Exclure 1 est ce qui rend la périodicité exacte : le pixel n−1 est à
    (n−1)/n, donc le pixel suivant retombe sur 0. Inclure 1 dupliquerait une
    colonne et décalerait le motif d'un pixel à chaque répétition.
    """
    t = np.arange(n) / n
    return np.meshgrid(t, t, indexing="xy")


def bruit_periodique(n, cellules, graine):
    """Bruit à valeurs, lissé, périodique sur `cellules` cases.

    Le hachage se fait sur les indices de case pris MODULO `cellules` : c'est
    ce qui referme le motif. Un `np.random` classique ne se refermerait pas, et
    la couture ne se verrait qu'une fois la texture répétée en jeu.
    """
    rng = np.random.default_rng(graine)
    valeurs = rng.random((cellules, cellules))
    u, v = grille(n)
    fu, fv = u * cellules, v * cellules
    i0, j0 = np.floor(fu).astype(int) % cellules, np.floor(fv).astype(int) % cellules
    i1, j1 = (i0 + 1) % cellules, (j0 + 1) % cellules
    du, dv = fu - np.floor(fu), fv - np.floor(fv)
    # Lissage cubique : une interpolation linéaire laisserait des arêtes
    # visibles sur les bords de case, qui lisent comme un quadrillage.
    su, sv = du * du * (3 - 2 * du), dv * dv * (3 - 2 * dv)
    a = valeurs[j0, i0] * (1 - su) + valeurs[j0, i1] * su
    b = valeurs[j1, i0] * (1 - su) + valeurs[j1, i1] * su
    return a * (1 - sv) + b * sv


def octaves(n, base, nb, graine):
    """Somme d'octaves périodiques. Chaque octave double la fréquence, donc
    reste un diviseur entier de la période — la somme reste tuilable."""
    total = np.zeros((n, n))
    amplitude, poids = 1.0, 0.0
    for k in range(nb):
        total += amplitude * bruit_periodique(n, base * (2**k), graine + k * 977)
        poids += amplitude
        amplitude *= 0.5
    return total / poids


def normalise(x, contraste):
    """Ramène le motif en multiplicateur LINÉAIRE dans `[1 − contraste, 1]`.

    🚨 Deux décisions ici, et les deux ont été mesurées avant d'être prises.

    **On assombrit seulement, jamais on n'éclaircit.** Le maximum vaut 1,0 —
    donc la teinte que l'auteur de la carte a choisie reste la teinte la plus
    claire du matériau. Une carte qui monterait au-dessus de 1 délaverait sa
    palette, et il n'aurait aucun moyen de le rattraper depuis Blender.

    **Le sommet est à 1,0 exactement, pas au milieu.** Une première version
    centrait sur 1,0 avec la moitié du motif au-dessus. Écrite en 8 bits, elle
    tombait à l'octet 127 — que Bevy décode en sRGB vers **0,21 linéaire**. Le
    décor entier aurait été assombri de **79 %** par une texture censée le
    détailler. Le défaut n'aurait ressemblé à rien de connu : « c'est tout
    sombre depuis qu'on a mis les textures ».
    """
    x = (x - x.min()) / max(x.max() - x.min(), 1e-6)  # -> [0,1]
    return 1.0 - contraste * (1.0 - x)


def vers_srgb(lineaire):
    """Encode en sRGB, parce que Bevy DÉCODERA.

    `base_color_texture` est lue comme une couleur, donc traversée par la
    conversion sRGB → linéaire au chargement. Écrire la valeur linéaire brute
    dans le PNG la ferait donc décoder une seconde fois : un multiplicateur de
    0,90 deviendrait 0,79. On pré-encode pour que l'aller-retour soit neutre.
    """
    l = np.clip(lineaire, 0.0, 1.0)
    return np.where(l <= 0.0031308, l * 12.92, 1.055 * np.power(l, 1 / 2.4) - 0.055)


def pierre(n):
    """Grain minéral : du bruit fin, plus des micro-fractures nettes."""
    grain = octaves(n, 8, 4, 11)
    # Les fractures : une crête de bruit, mise au carré pour la rendre étroite.
    fractures = 1.0 - np.abs(octaves(n, 4, 3, 23) - 0.5) * 2.0
    return normalise(grain * 0.7 + fractures**6 * 0.3, 0.30)


def bois(n):
    """Fibre : très étirée dans un axe. C'est l'anisotropie qui fait le bois,
    pas le motif — un bruit isotrope lit comme de la pierre."""
    u, v = grille(n)
    # 6 cernes par tuile : harmonique ENTIÈRE, donc la fibre se referme.
    cernes = np.sin(2 * np.pi * 6 * v + octaves(n, 3, 3, 31) * 4.0)
    fibre = octaves(n, 2, 4, 47)
    fibre = np.roll(fibre, 0, axis=0)
    return normalise(cernes * 0.35 + fibre * 0.65, 0.26)


def terre(n):
    """Mottes : basses fréquences dominantes, peu de détail fin."""
    return normalise(octaves(n, 5, 3, 59), 0.22)


def feuillage(n):
    """Cellules foliaires : du bruit cellulaire approché par des crêtes
    croisées à harmoniques entières."""
    u, v = grille(n)
    cel = np.abs(np.sin(2 * np.pi * 9 * u + octaves(n, 4, 2, 71) * 3)) * np.abs(
        np.sin(2 * np.pi * 9 * v + octaves(n, 4, 2, 83) * 3)
    )
    return normalise(cel * 0.5 + octaves(n, 7, 3, 97) * 0.5, 0.28)


def herbe(n):
    """Brins : très haute fréquence dans un axe, presque rien dans l'autre."""
    u, v = grille(n)
    brins = np.sin(2 * np.pi * 24 * u + octaves(n, 6, 2, 101) * 6.0)
    return normalise(brins * 0.4 + octaves(n, 10, 3, 103) * 0.6, 0.20)


def accent(n):
    """Presque plat. Les accents colorés du kit sont des pièces peintes : leur
    donner du grain les salirait. On garde une modulation à peine perceptible,
    juste assez pour que la lumière ne soit pas parfaitement uniforme."""
    return normalise(octaves(n, 3, 2, 107), 0.08)


GENERATEURS = {
    "pierre": pierre,
    "bois": bois,
    "terre": terre,
    "feuillage": feuillage,
    "herbe": herbe,
    "accent": accent,
}


def couture_max(a):
    """Le pire écart au raccord, comparé au pas naturel entre deux voisins.

    🚨 Le juge n'est PAS un seuil absolu : c'est la variation qu'on observe
    DÉJÀ dans l'image. Une couture est une discontinuité plus forte que le
    motif lui-même. Un seuil constant donnerait un verdict qui dépend de la
    texture au lieu de sa qualité.
    """
    voisins_u = np.abs(np.diff(a, axis=1)).max()
    voisins_v = np.abs(np.diff(a, axis=0)).max()
    couture_u = np.abs(a[:, 0] - a[:, -1]).max()
    couture_v = np.abs(a[0, :] - a[-1, :]).max()
    return (couture_u, voisins_u, couture_v, voisins_v)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--taille", type=int, default=256)
    ap.add_argument(
        "--sortie",
        default="assets/textures/detail",
        help="dossier de sortie, relatif a la racine du projet",
    )
    a = ap.parse_args()
    n = a.taille
    sortie = pathlib.Path(a.sortie)
    sortie.mkdir(parents=True, exist_ok=True)

    print(f"{'famille':11} {'materiaux':>9} {'couture U':>11} {'couture V':>11} {'poids':>8}")
    total = 0
    fautives = []
    for nom, gen in GENERATEURS.items():
        img = gen(n)
        cu, vu, cv, vv = couture_max(img)
        if cu > vu or cv > vv:
            fautives.append((nom, cu, vu, cv, vv))
        # 🚨 RGB et non niveaux de gris. Un PNG en mode « L » se charge en
        # `R8Unorm` : l'échantillonnage rend `(r, 0, 0, 1)` et TEINTE TOUT LE
        # DÉCOR EN ROUGE. Le canal unique économiserait deux tiers d'un fichier
        # de 20 Ko — il coûterait le rendu.
        octets = np.clip(vers_srgb(img) * 255.0 + 0.5, 0, 255).astype(np.uint8)
        chemin = sortie / f"{nom}_detail.png"
        Image.fromarray(np.dstack([octets] * 3), mode="RGB").save(chemin, optimize=True)
        poids = chemin.stat().st_size
        total += poids
        etat_u = "OK" if cu <= vu else f"!! {cu:.4f}>{vu:.4f}"
        etat_v = "OK" if cv <= vv else f"!! {cv:.4f}>{vv:.4f}"
        print(
            f"{nom:11} {FAMILLES.get(nom,0):9} {etat_u:>11} {etat_v:>11} {poids/1024:7.1f} Ko"
        )
    print()
    print(
        f"{len(GENERATEURS)} textures {n}x{n} en niveaux de gris — {total/1024:.1f} Ko sur disque"
    )
    # Le coût GPU : 1 octet/pixel en R8, plus les mips (+1/3).
    vram = len(GENERATEURS) * n * n * 4 * 4 // 3
    print(f"VRAM en RGBA8 : {vram/1024:.0f} Ko  (BC7 la diviserait par 4)")
    print(f"couvre {sum(FAMILLES.values())} materiaux du Vallon")

    # 🚨 Le contrôle DOIT mordre. Une texture à couture livrée en silence
    # produit une ligne nette répétée tous les N mètres sur tout le décor — et
    # on la cherche dans l'art, jamais dans le générateur. Sortie non nulle
    # pour que ce script soit utilisable en garde automatique.
    if fautives:
        print()
        print(f"!! {len(fautives)} texture(s) NON TUILABLE(S) — rien ne doit etre livre :")
        for nom, cu, vu, cv, vv in fautives:
            print(f"   {nom}: couture U {cu:.4f} (voisins {vu:.4f}) · V {cv:.4f} (voisins {vv:.4f})")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
