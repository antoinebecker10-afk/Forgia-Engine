"""Cadrage MESURE d'un rendu, au lieu de regler l'offset camera au jugé.

Regler la position d'un sprite en bougeant la camera 3D est un mauvais levier :
translation, distance et focale se compensent mutuellement, et chaque essai coute
un rendu complet. On rend donc l'arme centree, on MESURE sa silhouette, puis on
la pose dans le cadre a la taille et a l'ancre voulues. La camera ne sert plus
qu'a choisir l'ANGLE, ce qu'elle seule sait faire.
"""

from __future__ import annotations

import numpy as np
from PIL import Image

from glb import Mesh
from render3d import View, render


def crop_to_content(img: Image.Image) -> tuple[Image.Image, tuple[int, int]]:
    alpha = np.array(img)[..., 3]
    ys, xs = np.nonzero(alpha)
    if len(xs) == 0:
        raise ValueError("rendu vide — l'arme est hors champ ou derriere la camera")
    x0, x1 = int(xs.min()), int(xs.max()) + 1
    y0, y1 = int(ys.min()), int(ys.max()) + 1
    return img.crop((x0, y0, x1, y1)), (x0, y0)


def shoot(
    mesh: Mesh,
    view: View,
    model_matrix: np.ndarray,
    target_height: int,
    pad: int = 24,
    supersample: int = 4,
    fit_to=None,
) -> Image.Image:
    """Rend l'arme centree et la renvoie recadree a `target_height` de haut.

    Un premier rendu sert de mesure, un second corrige la focale. Deux rendus
    valent mieux qu'un redimensionnement : reduire une image deja rendue rajoute
    du flou que la passe pixel art devra ensuite deviner.
    """
    probe_size = (target_height * 3, target_height * 3)
    # On MESURE sur `fit_to` (l'arme seule) et on REND la scene entiere. Sans ce
    # decouplage, un avant-bras qui sort du cadre compte dans la mesure et fait
    # retrecir l'arme d'autant — c'est l'arme qui doit tenir la taille voulue.
    probe_scene = fit_to if fit_to is not None else mesh
    first = render(probe_scene, view, probe_size, supersample=1, model_matrix=model_matrix)
    cropped, _ = crop_to_content(first)
    if cropped.height == 0:
        raise ValueError("silhouette de hauteur nulle")

    corrected = View(**{**view.__dict__, "focal": view.focal * target_height / cropped.height})
    final = render(mesh, corrected, probe_size, supersample=supersample, model_matrix=model_matrix)
    out, _ = crop_to_content(final)
    if pad:
        padded = Image.new("RGBA", (out.width + pad * 2, out.height + pad * 2), (0, 0, 0, 0))
        padded.alpha_composite(out, (pad, pad))
        out = padded
    return out


def place(
    sprite: Image.Image,
    canvas_size: tuple[int, int],
    anchor: tuple[float, float],
    at: tuple[int, int],
) -> Image.Image:
    """Pose `sprite` sur un cadre vide. `anchor` en fractions (0,0)=coin haut-gauche
    du sprite, (1,1)=coin bas-droit ; `at` = ou cette ancre doit tomber."""
    canvas = Image.new("RGBA", canvas_size, (0, 0, 0, 0))
    x = int(round(at[0] - anchor[0] * sprite.width))
    y = int(round(at[1] - anchor[1] * sprite.height))
    canvas.alpha_composite(sprite, (x, y))
    return canvas
