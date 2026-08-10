"""Placage par projection : coller une illustration 2D sur un modele 3D.

Le procede des studios pour convertir une illustration en asset : on modelise
grossierement, on PROJETTE l'illustration depuis l'angle ou elle a ete faite, et
on complete par du procedural les faces qu'elle ne couvre pas.

Ici c'est gratuit : la fonction de detail du rasteriseur recoit deja la position
MONDE de chaque pixel. Projeter revient a choisir deux de ses coordonnees et a y
lire l'image.

Le point delicat est l'ETIREMENT. Une projection plaque correctement les faces
tournees vers elle, et etire abominablement celles qui lui sont paralleles — le
dessus d'un canon recoit alors une seule ligne de pixels tiree sur toute sa
longueur. On mesure donc l'angle entre la normale et l'axe de projection, et on
bascule progressivement vers un motif procedural la ou la projection ne vaut
plus rien. C'est cette bascule qui rend le procede utilisable, pas la projection
elle-meme.
"""

from __future__ import annotations

import numpy as np
from PIL import Image


def load_plate(path: str) -> np.ndarray:
    """Charge une illustration en tableau RVB normalise."""
    return np.asarray(Image.open(path).convert("RGB"), np.float32) / 255.0


def planar(
    plate: np.ndarray,
    axis: int,
    bounds: tuple[tuple[float, float], tuple[float, float]],
    flip_u: bool = False,
    flip_v: bool = True,
    fallback=None,
    blend_from: float = 0.30,
):
    """Projette `plate` le long de `axis` (0=x, 1=y, 2=z).

    `bounds` = ((u_min, u_max), (v_min, v_max)) : l'emprise MONDE que couvre
    l'image. C'est la seule calibration a faire, et elle se lit sur le modele —
    pas a l'oeil sur le rendu.

    `fallback` prend le relais la ou la projection s'etire. `blend_from` est le
    cosinus en deca duquel on lui passe la main.
    """
    h, w = plate.shape[:2]
    (u0, u1), (v0, v1) = bounds
    others = [i for i in range(3) if i != axis]

    def fn(pos, normal, albedo):
        u = (pos[:, others[0]] - u0) / (u1 - u0)
        v = (pos[:, others[1]] - v0) / (v1 - v0)
        if flip_u:
            u = 1.0 - u
        if flip_v:
            v = 1.0 - v

        inside = (u >= 0) & (u < 1) & (v >= 0) & (v < 1)
        px = np.clip((u * w).astype(np.int32), 0, w - 1)
        py = np.clip((v * h).astype(np.int32), 0, h - 1)
        sampled = plate[py, px]

        out = albedo.copy() if fallback is None else fallback(pos, normal, albedo)
        # Poids de la projection : plein en face d'elle, nul de profil.
        facing = abs(float(normal[axis]))
        if facing <= blend_from:
            return out
        k = min(1.0, (facing - blend_from) / (1.0 - blend_from))
        out[inside] = out[inside] * (1.0 - k) + sampled[inside] * k
        return out

    return fn


def measure_bounds(meshes, axis: int) -> tuple[tuple[float, float], tuple[float, float]]:
    """Emprise du modele dans le plan perpendiculaire a `axis`.

    Se mesure sur la geometrie plutot que de se regler a l'oeil : c'est ce qui
    garantit que l'illustration tombe pile sur l'arme, et non a peu pres.
    """
    others = [i for i in range(3) if i != axis]
    pts = np.concatenate([m.positions for m in meshes])
    return (
        (float(pts[:, others[0]].min()), float(pts[:, others[0]].max())),
        (float(pts[:, others[1]].min()), float(pts[:, others[1]].max())),
    )
