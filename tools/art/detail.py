"""Motifs de detail evalues par pixel, depuis la position dans l'espace.

L'idee : garder une geometrie GROSSIERE et laisser une texture porter le detail.
C'est ce que font les jeux — un fusil n'a pas ses rivets modelises, il les a
peints. Ici le detail est procedural et lu a la position monde, donc :

* aucune primitive n'a besoin d'etre depliee en UV ;
* le motif reste CONTINU d'une piece a l'autre — deux boites accolees partagent
  la meme trame, alors que deux depliages independants auraient une couture ;
* changer une cote du modele ne casse aucun mappage.

C'est le principe du mappage triplanaire : on projette selon l'axe dominant de
la normale, ce qui evite l'etirement sur les faces obliques.
"""

from __future__ import annotations

import numpy as np


def _plane(positions: np.ndarray, normal: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    """Deux coordonnees a plat, choisies selon l'axe dominant de la normale."""
    axis = int(np.argmax(np.abs(normal)))
    if axis == 0:
        return positions[:, 2], positions[:, 1]
    if axis == 1:
        return positions[:, 0], positions[:, 2]
    return positions[:, 0], positions[:, 1]


def panels(spacing: float = 0.16, width: float = 0.016, strength: float = 0.42):
    """Coutures de toles : lignes sombres a intervalle regulier, doublees d'un
    liseré clair juste dessous. Une simple ligne sombre se lit comme une rayure ;
    le doublet en fait une jonction."""

    def fn(pos, normal, albedo):
        u, v = _plane(pos, normal)
        du = np.abs((u % spacing) - spacing * 0.5)
        dv = np.abs((v % spacing) - spacing * 0.5)
        edge = (du < width) | (dv < width)
        lip = (
            ((du >= width) & (du < width * 2.2))
            | ((dv >= width) & (dv < width * 2.2))
        )
        out = albedo.copy()
        out[edge] *= 1.0 - strength
        out[lip] *= 1.0 + strength * 0.32
        return out

    return fn


def rivets(spacing: float = 0.20, radius: float = 0.022, strength: float = 0.55):
    """Rivets : un disque clair, assombri sur son bord bas-droit.

    Sans le bord sombre, un rivet se lit comme une tache. C'est le contraste
    entre les deux qui le rend bombe.
    """

    def fn(pos, normal, albedo):
        u, v = _plane(pos, normal)
        du = (u % spacing) - spacing * 0.5
        dv = (v % spacing) - spacing * 0.5
        d = np.hypot(du, dv)
        out = albedo.copy()
        rim = (d < radius * 1.5) & (d >= radius)
        head = d < radius
        out[rim] *= 0.55
        out[head] *= 1.0 + strength
        gloss = head & (du < 0) & (dv < 0)
        out[gloss] *= 1.18
        return out

    return fn


def grain(spacing: float = 0.030, strength: float = 0.16):
    """Veinage du bois : stries fines, irregulieres, le long d'un seul axe."""

    def fn(pos, normal, albedo):
        u, v = _plane(pos, normal)
        wave = np.sin(v / spacing * 6.283 + np.sin(u * 7.0) * 0.8)
        return albedo * (1.0 + strength * wave)[:, None]

    return fn


def filigree(spacing: float = 0.075, strength: float = 0.30):
    """Gravure ornementale : un entrelacs sinusoidal, pour l'or.

    L'or nu paraît plastique a cette resolution ; c'est la gravure qui lui donne
    sa qualite de metal travaille.
    """

    def fn(pos, normal, albedo):
        u, v = _plane(pos, normal)
        a = np.sin(u / spacing * 6.283) * np.cos(v / spacing * 6.283)
        engraved = a > 0.55
        polish = a < -0.72
        out = albedo.copy()
        out[engraved] *= 1.0 - strength
        out[polish] *= 1.0 + strength * 0.8
        return out

    return fn


def combine(*fns):
    """Applique plusieurs motifs a la suite."""

    def fn(pos, normal, albedo):
        for f in fns:
            albedo = f(pos, normal, albedo)
        return albedo

    return fn


def face(
    centre,
    eye_dx: float = 0.115,
    eye_y: float = 0.075,
    eye_r: float = 0.082,
    pupil_r: float = 0.036,
    brow: float = 0.0,
    blink: float = 0.0,
    mouth: float = 1.0,
    palette=None,
):
    """Visage PEINT sur une sphere, au lieu d'etre sculpte.

    Sculpter des yeux de 2 cm sur une tete de 30 cm ne se lit pas a la taille
    d'un sprite : ils disparaissent. Peints, ils occupent la surface qu'il faut
    et restent nets apres la reduction en pixel art.

    Et tout devient parametrable : `blink` ecrase les yeux, `mouth` ouvre la
    bouche, `brow` fronce les sourcils. Les expressions et la parole ne coutent
    plus un modele de plus — juste d'autres valeurs.

    Ne peint que l'hemisphere tourne vers le joueur : `centre` est le centre de
    la tete, et la face regarde vers +z.
    """
    eye_c, pupil_c, dark_c, tongue_c, brow_c = palette

    def fn(pos, normal, albedo):
        lx = pos[:, 0] - centre[0]
        ly = pos[:, 1] - centre[1]
        lz = pos[:, 2] - centre[2]
        front = lz > 0.01
        out = albedo.copy()

        # Yeux : l'ecrasement du clignement se fait sur la coordonnee VERTICALE,
        # ce qui donne une paupiere qui se ferme, pas un oeil qui retrecit.
        squash = max(0.08, 1.0 - 0.94 * blink)
        for side in (-1.0, 1.0):
            dx = lx - side * eye_dx
            dy = (ly - eye_y) / squash
            d = np.hypot(dx, dy)
            out[front & (d < eye_r * 1.12)] = dark_c
            out[front & (d < eye_r)] = eye_c
            pd = np.hypot(dx - side * 0.012, dy - 0.006)
            out[front & (pd < pupil_r)] = pupil_c
            gl = np.hypot(dx + side * 0.028, dy + 0.028)
            out[front & (gl < pupil_r * 0.42)] = eye_c

        # Sourcils : inclines vers l'interieur quand `brow` monte -> l'arme
        # fronce. Un seul parametre suffit a passer de hilare a furieux.
        for side in (-1.0, 1.0):
            bx = lx - side * eye_dx
            by = ly - eye_y - eye_r * 1.45 + side * bx * brow * 0.55
            out[front & (np.abs(bx) < eye_r * 1.15) & (np.abs(by) < 0.016)] = brow_c

        if mouth > 0.02:
            mh = 0.075 * mouth
            my = ly + 0.115
            wide = np.abs(lx) < 0.135 - np.abs(my) * 0.5
            inside = wide & (np.abs(my) < mh)
            out[front & inside] = dark_c
            out[front & inside & (my > mh * 0.10)] = tongue_c
            out[front & wide & (np.abs(my - -mh * 0.92) < 0.014)] = eye_c

        return out

    return fn
