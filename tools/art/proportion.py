"""Systeme de proportions : nombre d'or ramene a une trame entiere.

Le nombre d'or (phi ~ 1.618, limite du rapport de Fibonacci) sert d'ECHAFAUDAGE
pour repartir les grandes masses. Ce qui fait l'harmonie n'est pas la valeur
magique — la preuve qu'elle serait objectivement plus belle est faible — mais la
COHERENCE : un systeme de rapports tenu bat un dessin ou chaque cote est choisie
separement.

Piege propre au pixel art : phi est irrationnel, les pixels sont entiers. Un
rapport 1.618 arrondi derive d'un demi-pixel a chaque application, et au bout de
trois divisions le systeme ne tient plus. Toute cote passe donc par `snap`, qui
la ramene sur une trame de module entier. C'est la trame, pas phi, qui garantit
que deux elements censes s'aligner s'alignent VRAIMENT.
"""

from __future__ import annotations

PHI = 1.6180339887
#: Module de la trame. 4 px : assez fin pour du detail, assez gros pour que
#: l'alignement se voie. Toute cote structurelle en est un multiple.
MODULE = 4


def snap(value: float, module: int = MODULE) -> int:
    """Ramene une cote sur la trame. Sans ça, phi laisse des demi-pixels partout."""
    return int(round(value / module) * module)


def divide(total: float, module: int = MODULE) -> tuple[int, int]:
    """Division en nombre d'or, arrondie a la trame. Renvoie (grand, petit)."""
    major = snap(total / PHI, module)
    return major, int(total) - major


def scale(base: float, steps: int, ratio: float = PHI, module: int = MODULE) -> list[int]:
    """Echelle modulaire : des tailles en progression geometrique, snappees.

    Sert aux elements repetes (rivets, plaques, dents de couronne) : des tailles
    tirees d'une meme echelle se repondent, des tailles choisies au coup par coup
    se contredisent.
    """
    out = []
    v = base
    for _ in range(steps):
        out.append(max(module, snap(v, module)))
        v *= ratio
    return out


def symmetric(centre: float, half: float) -> tuple[float, float]:
    """Paire symetrique exacte autour d'un centre.

    A ecrire plutot que deux fractions a la main : `0.31` et `0.68` se lisent
    comme symetriques et ne le sont pas — l'oeil attrape ce decalage d'un pixel
    sur un visage, meme sans savoir le nommer.
    """
    return centre - half, centre + half


def audit(pairs: list[tuple[str, float, float, float]]) -> list[str]:
    """Verifie qu'une liste de paires est bien symetrique autour de son centre.

    Un controle mecanique vaut mieux qu'une relecture : on ne voit pas ses
    propres asymetries d'un pixel, on les mesure.
    """
    problems = []
    for name, centre, left, right in pairs:
        dl, dr = centre - left, right - centre
        if abs(dl - dr) > 1e-6:
            problems.append(f"{name}: ecart {dl:.3f} a gauche contre {dr:.3f} a droite")
    return problems
