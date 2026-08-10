"""Rampes de couleurs generees en Oklab, et tramage ordonne de Bayer.

Deux techniques reprises de l'outillage pixel art du domaine :

* **Oklab** (PixelRefiner) est un espace perceptuellement uniforme : un ecart de
  clarte y correspond a un ecart VU constant. Choisir une rampe a l'oeil en
  hexadecimal donne presque toujours des paliers irreguliers — deux crans qui se
  confondent, deux autres qui sautent.

* **Tramage ordonne de Bayer** (Pyxelate, pixel-mcp) : entre deux crans, on
  entrelace les deux teintes selon une matrice fixe. On obtient une transition
  texturee au lieu d'une frontiere nette, sans depenser une couleur de plus.
  C'est ce qui donne aux fiches d'armes leurs degrades riches.

Aucune dependance ajoutee : tout tient en numpy.
"""

from __future__ import annotations

import numpy as np

# Matrice de Bayer 4x4, normalisee dans [0,1). L'ordre des valeurs est ce qui
# rend le motif regulier et non bruite — un tirage aleatoire ferait grésiller
# l'image d'une frame a l'autre.
BAYER4 = (
    np.array(
        [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]], np.float32
    )
    + 0.5
) / 16.0


def _srgb_to_linear(c: np.ndarray) -> np.ndarray:
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)


def _linear_to_srgb(c: np.ndarray) -> np.ndarray:
    return np.where(c <= 0.0031308, c * 12.92, 1.055 * np.clip(c, 0, None) ** (1 / 2.4) - 0.055)


def to_oklab(rgb: np.ndarray) -> np.ndarray:
    r, g, b = _srgb_to_linear(rgb / 255.0)
    l = np.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b)
    m = np.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b)
    s = np.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b)
    return np.array(
        [
            0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
            1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
            0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
        ]
    )


def from_oklab(lab: np.ndarray) -> np.ndarray:
    L, a, b = lab
    l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3
    m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3
    s = (L - 0.0894841775 * a - 1.2914855480 * b) ** 3
    rgb = np.array(
        [
            4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
            -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
            -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
        ]
    )
    return np.clip(_linear_to_srgb(rgb) * 255.0, 0, 255)


def _hex_to_rgb(h: str) -> np.ndarray:
    h = h.lstrip("#")
    return np.array([int(h[i : i + 2], 16) for i in (0, 2, 4)], np.float32)


def _rgb_to_hex(rgb: np.ndarray) -> str:
    return "#%02x%02x%02x" % tuple(int(round(v)) for v in rgb)


def ramp(
    base: str,
    steps: int = 6,
    span: float = 0.52,
    hue_shift: float = 0.055,
    chroma: float = 0.35,
    base_at: int = 2,
) -> list[str]:
    """Rampe perceptuellement reguliere autour d'une couleur de base.

    `hue_shift` fait DERIVER la teinte le long de la rampe : les crans sombres
    vont vers le froid (bleu/violet), les clairs vers le chaud (jaune). Une rampe
    sans cette derive se lit comme un fondu au gris, quel que soit le soin mis
    aux valeurs.

    `chroma` remonte la saturation des tons moyens : les extremes d'une rampe se
    desaturent naturellement, et sans correction le milieu paraît terne.

    `base_at` dit a quel CRAN se place la couleur donnee. Etaler la rampe
    symetriquement autour d'elle envoie les crans sombres au noir quand la base
    est deja foncee — un bleu nuit y perdait ses deux premiers tons.
    """
    lab = to_oklab(_hex_to_rgb(base))
    L0 = float(lab[0])
    a0, b0 = float(lab[1]), float(lab[2])

    out = []
    for i in range(steps):
        t = i / (steps - 1)  # 0 = le plus sombre
        t0 = base_at / (steps - 1)
        L = np.clip(L0 + (t - t0) * span, 0.06, 0.97)
        # Derive de teinte : rotation dans le plan (a,b) autour de la base.
        angle = (t - t0) * hue_shift * np.pi
        ca, sa = np.cos(angle), np.sin(angle)
        a = a0 * ca - b0 * sa
        b = a0 * sa + b0 * ca
        # Les tons moyens gardent leur chroma, les extremes se desaturent.
        boost = 1.0 + chroma * (1.0 - abs(t - 0.5) * 2.0)
        out.append(_rgb_to_hex(from_oklab(np.array([L, a * boost, b * boost]))))
    return out


def bayer_mask(width: int, height: int, ratio: float) -> np.ndarray:
    """Masque booleen d'un tramage a `ratio` de couverture (0 = vide, 1 = plein)."""
    tile = np.tile(BAYER4, (height // 4 + 1, width // 4 + 1))[:height, :width]
    return tile < ratio
