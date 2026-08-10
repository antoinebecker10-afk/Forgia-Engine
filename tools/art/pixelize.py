"""Reduction d'un rendu 3D en pixel art : quantification + aplats + cerne.

Un rendu lisse reduit a 128 px n'est pas du pixel art, c'est une petite photo :
degrades continus, bords en demi-teintes, silhouette molle. Trois passes le
rendent lisible a cette taille.

1. **Alpha binaire** — un bord a 50 % d'opacite fait une frange grise autour du
   sprite des qu'il passe sur un fond clair. On tranche net.
2. **Palette reduite** — regroupe les degrades en aplats. C'est ce qui donne la
   lecture « dessinee » plutot que « photo reduite ».
3. **Cerne** — un contour sombre detache le sprite du decor. Sans lui, une arme
   vert d'eau disparaît sur un ciel clair.
"""

from __future__ import annotations

import numpy as np
from PIL import Image


def pixelize(
    img: Image.Image,
    colours: int = 20,
    alpha_cut: int = 110,
    outline: tuple[int, int, int] = (20, 16, 28),
    shadow_boost: float = 1.0,
) -> Image.Image:
    """Rendu lisse -> sprite. `colours` compte SANS le cerne ni la transparence."""
    rgba = np.array(img.convert("RGBA"))
    alpha = rgba[..., 3]
    mask = alpha >= alpha_cut

    if shadow_boost != 1.0:
        # Ecarte les tons avant de quantifier : sur un modele peu contraste, la
        # palette reduite ecraserait tout sur deux teintes.
        lab = rgba[..., :3].astype(np.float32) / 255.0
        mean = lab[mask].mean() if mask.any() else 0.5
        lab = np.clip((lab - mean) * shadow_boost + mean, 0.0, 1.0)
        rgba[..., :3] = (lab * 255).astype(np.uint8)

    # Quantification sur les seuls pixels opaques : inclure le fond transparent
    # gaspillerait une entree de palette pour du vide.
    flat = Image.fromarray(rgba[..., :3], "RGB")
    quantised = np.array(flat.quantize(colors=colours, dither=Image.Dither.NONE).convert("RGB"))

    out = np.zeros(rgba.shape, np.uint8)
    out[..., :3] = quantised
    out[..., 3] = mask * 255

    # Cerne : tout pixel vide touchant un pixel plein (4-voisinage).
    filled = mask
    ring = np.zeros_like(filled)
    ring[1:, :] |= filled[:-1, :]
    ring[:-1, :] |= filled[1:, :]
    ring[:, 1:] |= filled[:, :-1]
    ring[:, :-1] |= filled[:, 1:]
    ring &= ~filled
    out[ring, :3] = outline
    out[ring, 3] = 255

    return Image.fromarray(out, "RGBA")


def contact(images: list[Image.Image], zoom: int = 2, background=(20, 18, 28, 255)) -> Image.Image:
    """Planche de controle, en agrandissement au plus proche."""
    w = max(i.width for i in images)
    h = max(i.height for i in images)
    sheet = Image.new("RGBA", (w * len(images), h), background)
    for i, img in enumerate(images):
        sheet.alpha_composite(img, (i * w, 0))
    return sheet.resize((sheet.width * zoom, sheet.height * zoom), Image.Resampling.NEAREST)
