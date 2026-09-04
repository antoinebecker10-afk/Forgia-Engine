"""Fabrique la texture de DÉTAIL du sol du Hall — un grain tuilable par construction.

# Pourquoi une texture de détail plutôt qu'un albédo plus grand

Le sol du Hall est un seul albédo 2048² cuit sur 300 × 300 m, soit **5 texels par
mètre** — 25 fois moins que la médiane du reste de la carte (194). Monter cet albédo
ne sert à rien : à 8192² il ferait encore 20 texels/m pour 268 Mo de VRAM.

Ce qui manque n'est pas de la *couleur*, c'est de la **haute fréquence**. Une texture
de détail tuilée toutes les quelques mètres, multipliée par l'albédo cuit, rend le
grain que la basse fréquence ne peut pas porter — et coûte 512², une fois.

# Pourquoi la FFT

Un bruit tuilable ne se recadre pas, il se **construit** : on tire un bruit blanc, on
pondère son spectre en 1/f^alpha, et la transformée inverse est périodique par
définition. Découper un bruit quelconque laisserait une couture visible tous les
quelques mètres sur 300 m de terrain — soit une centaine de coutures alignées.

# Ce que la sortie garantit

Moyenne exactement 0,5, pour que `albédo × détail × 2` laisse la couleur d'ensemble
inchangée : le détail ajoute du relief perçu, il ne rassombrit ni n'éclaircit la carte.

    python tools/assets/generer_detail_sol.py
"""

import os

import numpy as np
from PIL import Image

TAILLE = 512
ALPHA = 1.35      # pente du spectre : 1,0 = bruit rose, 2,0 = très doux
ECART = 0.16      # écart-type visé en linéaire — le shader peut encore l'atténuer
GRAINE = 20260821
SORTIE = os.path.join("assets", "models", "environment", "castle",
                      "sources_sol", "detail_sol_hall.png")


def bruit_tuilable(n, alpha, rng):
    """Bruit fractal périodique : spectre en 1/f^alpha, donc tuilable sans couture."""
    blanc = rng.normal(0.0, 1.0, (n, n))
    spectre = np.fft.fft2(blanc)

    # Fréquences en cycles/image. `fftfreq` les rend déjà périodiques.
    fx = np.fft.fftfreq(n)[:, None]
    fy = np.fft.fftfreq(n)[None, :]
    f = np.sqrt(fx * fx + fy * fy)
    f[0, 0] = 1.0                       # la composante continue ne se pondère pas

    spectre *= 1.0 / np.power(f, alpha)
    spectre[0, 0] = 0.0                 # moyenne nulle : on la repose ensuite

    champ = np.real(np.fft.ifft2(spectre))
    return champ


def main():
    rng = np.random.default_rng(GRAINE)
    champ = bruit_tuilable(TAILLE, ALPHA, rng)

    # Normaliser : moyenne 0, écart-type 1, puis poser la moyenne et l'écart voulus.
    champ = (champ - champ.mean()) / champ.std()
    champ = 0.5 + champ * ECART
    ecretes = int(np.count_nonzero((champ < 0.0) | (champ > 1.0)))
    champ = np.clip(champ, 0.0, 1.0)

    # CONTRÔLE SUR LA SORTIE, pas sur l'intention : on relit ce qu'on écrit.
    octets = np.round(champ * 255.0).astype(np.uint8)
    os.makedirs(os.path.dirname(SORTIE), exist_ok=True)
    Image.fromarray(octets, mode="L").save(SORTIE)

    relu = np.asarray(Image.open(SORTIE), dtype=np.float32) / 255.0
    couture_x = float(np.abs(relu[:, 0] - relu[:, -1]).mean())
    couture_y = float(np.abs(relu[0, :] - relu[-1, :]).mean())
    interne = float(np.abs(np.diff(relu, axis=1)).mean())

    print(f"ecrit    : {SORTIE} ({os.path.getsize(SORTIE) / 1024:.0f} Ko, {TAILLE}²)")
    print(f"moyenne  : {relu.mean():.4f}   ecart-type : {relu.std():.4f}   "
          f"ecretes : {ecretes}")
    print(f"couture  : bord X {couture_x:.4f} · bord Y {couture_y:.4f} · "
          f"ecart interne moyen {interne:.4f}")
    # Une couture se voit quand l'ecart au bord depasse l'ecart interne. Le bruit
    # etant periodique, les deux doivent etre du meme ordre.
    if max(couture_x, couture_y) > interne * 1.5:
        raise SystemExit("COUTURE VISIBLE : le bruit n'est pas periodique")
    print("verdict  : tuilable (l'ecart au bord ne depasse pas l'ecart interne)")


if __name__ == "__main__":
    main()
