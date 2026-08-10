"""Rampes de valeurs et remplissages volumetriques pour le dessin pixel art.

Trois regles, tirees des references du metier (Lospec, Pedro Medeiros) et qui
corrigent chacune un defaut visible sur une premiere passe en aplats :

1. **Decalage de teinte.** Une ombre n'est pas la couleur de base assombrie :
   elle derive vers le FROID (bleu/violet), et la lumiere vers le CHAUD
   (jaune/orange). C'est ce qui separe une rampe vivante d'un fondu au gris.

2. **Bandes inegales.** Des bandes de meme largeur produisent du *banding* — on
   lit les paliers au lieu de la courbe. Un cylindre reel a un liseré clair
   etroit, un corps large, une ombre franche, et un mince **rebond** sous
   l'arete basse (lumiere renvoyee par l'environnement).

3. **Une seule direction de lumiere**, ici en haut a gauche. Eclairer chaque
   forme « par le contour » donne du *pillow shading* : la silhouette gonfle et
   plus rien n'a de volume.
"""

from __future__ import annotations

from pixelforge import Grid

#: Profil d'un cylindre eclaire d'en haut. Fractions de la hauteur, du haut vers
#: le bas, associees a un cran de rampe (0 = le plus sombre).
#: Les largeurs sont VOLONTAIREMENT inegales — c'est ce qui evite le banding.
TUBE = (
    (0.00, 0.10, 3),  # arete haute, dans l'ombre rasante
    (0.10, 0.26, 5),  # liseré clair, etroit
    (0.26, 0.40, 4),
    (0.40, 0.68, 3),  # corps, large
    (0.68, 0.88, 1),  # ombre franche
    (0.88, 1.00, 2),  # rebond sous l'arete basse
)

#: Profil d'une plaque plate vue de biais : peu de variation, la lumiere ne
#: tourne pas. Une surface plane doit rester unie — la nuancer comme un cylindre
#: est le defaut inverse du banding.
PLATE = (
    (0.00, 0.16, 4),
    (0.16, 0.78, 3),
    (0.78, 1.00, 2),
)


def tube(
    g: Grid,
    x0: float,
    x1: float,
    y_top,
    y_bot,
    ramp: list[int],
    profile=TUBE,
    dither: bool = True,
) -> None:
    """Remplit un cylindre horizontal entre deux profils de hauteur.

    `y_top` et `y_bot` acceptent un nombre (arete droite) ou un couple
    (gauche, droite) pour une arete qui file — un canon n'est jamais tout a fait
    parallele.
    """
    ty0, ty1 = (y_top, y_top) if isinstance(y_top, (int, float)) else y_top
    by0, by1 = (y_bot, y_bot) if isinstance(y_bot, (int, float)) else y_bot

    def quad(a: float, b: float):
        return [
            (x0, ty0 + (by0 - ty0) * a),
            (x1, ty1 + (by1 - ty1) * a),
            (x1, ty1 + (by1 - ty1) * b),
            (x0, ty0 + (by0 - ty0) * b),
        ]

    for a, b, step in profile:
        g.poly(quad(a, b), ramp[step])

    # Transitions TRAMEES entre crans : sans elles on lit des paliers, pas une
    # courbe. La bande est etroite et le taux decroit du ton clair vers le
    # sombre, ce qui adoucit le passage sans brouiller la forme.
    if dither:
        for (a0, b0_, s0), (a1, b1_, s1) in zip(profile, profile[1:]):
            if abs(ramp[s1] - ramp[s0]) != 1:
                continue  # crans non voisins : un tramage y ferait du bruit
            mid = b0_
            width = min(b0_ - a0, b1_ - a1) * 0.42
            if width < 0.02:
                continue
            lighter = s0 if ramp[s0] > ramp[s1] else s1
            g.poly_dither(quad(mid - width, mid), ramp[lighter], 0.30)
            g.poly_dither(quad(mid, mid + width), ramp[lighter], 0.62)


def sphere(g: Grid, cx: float, cy: float, r: float, ramp: list[int]) -> None:
    """Boule eclairee en haut a gauche : disques concentriques DECENTRES.

    Des disques concentriques centres donneraient du pillow shading — la forme
    aurait l'air d'un coussin, pas d'une sphere.
    """
    g.disc(cx, cy, r, ramp[1])
    g.disc(cx - r * 0.10, cy - r * 0.10, r * 0.86, ramp[2])
    g.disc(cx - r * 0.20, cy - r * 0.20, r * 0.62, ramp[3])
    g.disc(cx - r * 0.28, cy - r * 0.28, r * 0.36, ramp[4])
    g.disc(cx - r * 0.34, cy - r * 0.34, r * 0.16, ramp[5])


def bevel(g: Grid, pts: list[tuple[float, float]], ramp: list[int], depth: int = 2) -> None:
    """Plaque avec un chanfrein clair en haut et sombre en bas.

    Sert aux ferrures et aux plaques de blindage : elles doivent se detacher du
    corps sans etre traitees comme des cylindres.
    """
    g.poly(pts, ramp[3])
    top = min(p[1] for p in pts)
    bottom = max(p[1] for p in pts)
    left = min(p[0] for p in pts)
    right = max(p[0] for p in pts)
    g.poly([(left, top), (right, top), (right, top + depth), (left, top + depth)], ramp[4])
    g.poly(
        [(left, bottom - depth), (right, bottom - depth), (right, bottom), (left, bottom)],
        ramp[1],
    )
