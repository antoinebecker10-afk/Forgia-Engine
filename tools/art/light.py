"""Modele de lumiere : occlusion de contact, speculaire, halo emissif.

Trois passes qui s'appliquent APRES le dessin des formes, sur la grille
d'indices. Elles sont ce qui separe un assemblage d'aplats d'un objet eclaire —
et ce sont exactement les trois choses que la fiche de reference fait et qu'un
dessin en bandes de valeurs ne fait pas.

1. **Occlusion de contact** (`occlude`). Partout ou deux matieres se touchent,
   la lumiere ambiante n'entre pas : la jonction s'assombrit. Sans elle, les
   pieces se juxtaposent au lieu de s'emboiter — le defaut se lit comme un
   collage. C'est la passe la plus rentable des trois.

2. **Speculaire** (`spec`). Sur un cylindre le point brillant se pose SUR LE
   BORD, jamais au milieu (Lospec / GameDev Academy). Et sa largeur dit la
   matiere : un metal poli renvoie une tache etroite et vive, un metal use une
   plage large et terne. Une seule fonction, un seul parametre `roughness`.

3. **Halo emissif** (`bloom`). Un cristal qui brille ECLAIRE ce qui l'entoure.
   En palette indexee ca ne peut pas se faire en additionnant de la lumiere :
   on precalcule, pour chaque matiere, sa version *teintee par la lueur*, et le
   halo se contente de remplacer l'indice a proximite d'une source. C'est ce qui
   fait qu'un serti d'or vire au violet pres de la pierre — la difference entre
   une pierre qui brille et une tache violette.
"""

from __future__ import annotations

import oklab
from pixelforge import Grid

#: Direction de la lumiere : en haut a gauche. Une seule pour tout le dessin.
LIGHT = (-1, -1)


def occlude(
    g: Grid,
    material_of: dict[int, int],
    darker: dict[int, int],
    depth: int = 1,
) -> None:
    """Assombrit chaque pixel qui touche une AUTRE matiere.

    `material_of` : indice de palette -> identifiant de matiere.
    `darker` : indice de palette -> l'indice d'un cran plus sombre.

    Ne s'applique qu'aux jonctions INTERNES (les deux pixels sont opaques) : le
    pourtour exterieur est deja traite par le cerne selectif, et l'assombrir une
    seconde fois epaissirait le contour au lieu de creuser la jonction.
    """
    src = list(g.cells)
    w, h = g.width, g.height

    for _ in range(depth):
        out = list(src)
        for y in range(h):
            row = y * w
            for x in range(w):
                i = src[row + x]
                mine = material_of.get(i)
                if mine is None:
                    continue
                for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                    nx, ny = x + dx, y + dy
                    if not (0 <= nx < w and 0 <= ny < h):
                        continue
                    j = src[ny * w + nx]
                    if j == 0:
                        continue  # bord exterieur : pas notre affaire
                    other = material_of.get(j)
                    if other is not None and other != mine:
                        out[row + x] = darker.get(i, i)
                        break
        src = out

    g.cells[:] = src


def spec(
    g: Grid,
    points: list[tuple[float, float]],
    index: int,
    roughness: float = 0.5,
    over: set[int] | None = None,
) -> None:
    """Pose une tache speculaire, plus ou moins etalee selon la rugosite.

    `roughness` 0 = miroir (tache franche), 1 = mat (tache tramee et diffuse).
    `over` restreint la pose a certains indices : un speculaire ne doit jamais
    deborder sur la matiere voisine, sinon il la traverse comme un autocollant.
    """
    if roughness <= 0.34:
        g.poly(points, index) if over is None else _poly_over(g, points, index, over)
        return
    ratio = 0.85 - roughness * 0.62
    before = list(g.cells)
    g.poly_dither(points, index, ratio)
    if over is not None:
        _restore(g, before, over, index)


def _poly_over(g: Grid, points, index: int, over: set[int]) -> None:
    before = list(g.cells)
    g.poly(points, index)
    _restore(g, before, over, index)


def _restore(g: Grid, before: list[int], over: set[int], index: int) -> None:
    """Annule la pose partout ou le fond n'etait pas dans `over`."""
    for k, was in enumerate(before):
        if g.cells[k] == index and was not in over:
            g.cells[k] = was


def bloom(g: Grid, sources: set[int], mapping: dict[int, int], radius: int = 3) -> None:
    """Teinte le voisinage des pixels emissifs vers leur couleur de lueur.

    `sources` : les indices qui emettent. `mapping` : indice normal -> indice
    teinte. Le remplacement se fait par distance, du plus proche au plus loin :
    on n'applique la teinte forte qu'au contact, faible ensuite — un halo de
    force uniforme se lit comme une aureole decoupee.
    """
    w, h = g.width, g.height
    lit: list[int] = [-1] * (w * h)
    front = [k for k, v in enumerate(g.cells) if v in sources]
    for k in front:
        lit[k] = 0

    for step in range(1, radius + 1):
        nxt = []
        for k in front:
            x, y = k % w, k // w
            for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, 1), (1, -1), (-1, -1)):
                nx, ny = x + dx, y + dy
                if not (0 <= nx < w and 0 <= ny < h):
                    continue
                n = ny * w + nx
                if lit[n] >= 0 or g.cells[n] == 0:
                    continue
                lit[n] = step
                nxt.append(n)
        front = nxt

    for k, dist in enumerate(lit):
        if dist <= 0:
            continue
        tinted = mapping.get(g.cells[k])
        if tinted is None:
            continue
        # Au contact : plein. Au-dela : trame decroissante, pour que le halo
        # s'eteigne au lieu de s'arreter net.
        if dist == 1:
            g.cells[k] = tinted
        else:
            x, y = k % w, k // w
            fade = 1.0 - (dist - 1) / float(radius)
            if oklab.BAYER4[y % 4][x % 4] / 16.0 < fade * 0.8:
                g.cells[k] = tinted


def tint(base: str, glow: str, amount: float, steps: int = 6) -> list[str]:
    """Rampe d'une matiere ECLAIREE par une lueur coloree.

    Melange en Oklab (pas en RVB : un melange RVB de deux teintes saturees passe
    par un gris sale) et remonte legerement la clarte — une surface eclairee est
    plus claire, pas seulement plus coloree.
    """
    import numpy as np

    gl = oklab.to_oklab(_rgb(glow))
    out = []
    for hexa in oklab.ramp(base, steps=steps):
        c = oklab.to_oklab(_rgb(hexa))
        mixed = np.array(
            [
                min(0.97, c[0] * (1 - amount * 0.55) + gl[0] * amount * 0.55 + amount * 0.11),
                c[1] * (1 - amount) + gl[1] * amount,
                c[2] * (1 - amount) + gl[2] * amount,
            ]
        )
        rgb = oklab.from_oklab(mixed)
        out.append("#%02x%02x%02x" % tuple(int(round(float(v))) for v in rgb))
    return out


def _rgb(hexa: str):
    import numpy as np

    h = hexa.lstrip("#")
    return np.array([int(h[i : i + 2], 16) for i in (0, 2, 4)], np.float32)
