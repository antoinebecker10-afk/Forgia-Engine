"""Pepin modelise en 3D — une seule source, toutes les vues.

Pourquoi passer du dessin au modele : un profil et une vue de dos DESSINES sont
deux images independantes. Rien ne garantit qu'elles decrivent la meme arme, et
un desaccord (diametre de canon, taille de tete, place de la crosse) ne se voit
jamais — on le decouvre en jeu. C'est la meme classe de defaut qu'une grandeur
ecrite deux fois : les deux copies finissent par diverger.

Avec un modele, la geometrie est ecrite UNE fois. Chaque vue en decoule, donc
elles sont coherentes par construction, et n'importe quel angle devient
disponible — dont le trois-quarts facon Valorant que les fiches n'ont pas.

Repere : +x a droite, +y en haut, **-z vers la bouche** (elle s'eloigne du
joueur ; le glTF est droitier, +z pointe vers le spectateur).

Le rendu passe ensuite par la reduction pixel art : c'est elle qui donne les
aplats et le cerne, pas le modele.
"""

from __future__ import annotations

import math

import numpy as np

import detail
import primitives as prim
import render3d

# ── Palette du design, en RVB normalise pour le rendu a plat ───────────────
NAVY = (0.17, 0.22, 0.33)
NAVY_DARK = (0.10, 0.13, 0.20)
GOLD = (0.79, 0.60, 0.23)
GOLD_DARK = (0.48, 0.33, 0.10)
PURPLE = (0.51, 0.16, 0.63)
CRYSTAL = (0.87, 0.42, 0.94)
WOOD = (0.40, 0.24, 0.13)
SKIN = (0.79, 0.63, 0.45)
EYE = (0.95, 0.93, 0.88)
DARK = (0.06, 0.05, 0.09)
TONGUE = (0.72, 0.20, 0.25)

BORE_Y = 0.14  # hauteur de l'axe du canon


def _upright(mesh):
    """Redresse un cylindre construit le long de Z pour qu'il pointe vers +Y."""
    m = np.array([[1, 0, 0], [0, 0, 1], [0, -1, 0]], np.float32)
    return prim.Mesh(mesh.positions @ m.T, mesh.normals @ m.T, mesh.uvs, mesh.indices, None)


def _tilt(mesh, degrees: float, pivot=(0.0, 0.0, 0.0)):
    a = math.radians(degrees)
    c, s = math.cos(a), math.sin(a)
    r = np.array([[1, 0, 0], [0, c, -s], [0, s, c]], np.float32)
    p = np.array(pivot, np.float32)
    return prim.Mesh(
        (mesh.positions - p) @ r.T + p, mesh.normals @ r.T, mesh.uvs, mesh.indices, None
    )


def build() -> list[render3d.Instance]:
    """Assemble l'arme. Une piece = une instance, avec sa couleur a plat."""
    out: list[render3d.Instance] = []

    # Motifs de detail par matiere : la geometrie reste grossiere, la texture
    # porte les rivets, les coutures, les gravures et le veinage.
    plate = detail.combine(detail.panels(0.20, 0.014, 0.40), detail.rivets(0.30, 0.020, 0.45))
    trim = detail.filigree(0.085, 0.30)
    timber = detail.grain(0.028, 0.18)
    tex_of = {NAVY: plate, NAVY_DARK: detail.panels(0.14, 0.012, 0.30),
              GOLD: trim, WOOD: timber}

    def add(mesh, colour):
        out.append(render3d.Instance(mesh, colour=colour, detail=tex_of.get(colour)))

    # ── Canon : cylindre le long de Z, bouche en -z.
    add(prim.cylinder(0.19, -1.05, -0.05, (0, BORE_Y), segments=22), NAVY)
    # Bande de visee sur le dessus : casse le cylindre nu.
    add(prim.box((0.10, 0.07, 0.86), (0, BORE_Y + 0.19, -0.55)), NAVY_DARK)

    # Ferrures : bagues dorees.
    for z in (-0.92, -0.22):
        add(prim.cylinder(0.235, z - 0.055, z + 0.055, (0, BORE_Y), segments=22), GOLD)

    # ── Bouche : anneau epais et cristal.
    add(prim.cylinder(0.30, -1.28, -1.05, (0, BORE_Y), segments=24), GOLD)
    add(prim.cylinder(0.20, -1.29, -1.06, (0, BORE_Y), segments=20), DARK)
    # Cristal : une sphere a 4 segments EST un octaedre — la forme de la fiche.
    add(prim.sphere(0.155, (0, BORE_Y, -1.16), segments=4, rings=4), PURPLE)
    add(prim.sphere(0.085, (0, BORE_Y, -1.16), segments=4, rings=4), CRYSTAL)

    # ── Carcasse.
    add(prim.box((0.30, 0.56, 0.62), (0, BORE_Y - 0.02, 0.22)), NAVY)
    add(prim.box((0.32, 0.09, 0.60), (0, BORE_Y + 0.24, 0.22)), NAVY_DARK)
    # Fenetre d'energie, sur les deux flancs — l'arme doit tenir de dos aussi.
    for side in (-1, 1):
        add(prim.box((0.03, 0.14, 0.28), (side * 0.155, BORE_Y + 0.02, 0.10)), GOLD)
        add(prim.box((0.02, 0.08, 0.20), (side * 0.168, BORE_Y + 0.02, 0.10)), PURPLE)

    # Hausse.
    add(prim.box((0.09, 0.09, 0.14), (0, BORE_Y + 0.31, 0.30)), GOLD)

    # ── Pontet et detente.
    add(prim.box((0.10, 0.06, 0.34), (0, BORE_Y - 0.44, 0.28)), GOLD)
    add(prim.box((0.10, 0.20, 0.06), (0, BORE_Y - 0.36, 0.12)), GOLD)
    add(prim.box((0.06, 0.16, 0.05), (0, BORE_Y - 0.30, 0.24)), NAVY_DARK)

    # ── Crosse : boite penchee vers l'arriere, collier dore a la jonction.
    add(prim.box((0.26, 0.14, 0.24), (0, BORE_Y - 0.28, 0.50)), GOLD)
    grip = prim.box((0.24, 0.62, 0.30), (0, BORE_Y - 0.62, 0.62))
    add(_tilt(grip, -16.0, (0, BORE_Y - 0.30, 0.52)), WOOD)
    butt = prim.box((0.27, 0.09, 0.33), (0, BORE_Y - 0.92, 0.70))
    add(_tilt(butt, -16.0, (0, BORE_Y - 0.30, 0.52)), GOLD)

    # ── Oriflammes, une par flanc : la vue de dos doit rester symetrique.
    for side in (-1, 1):
        add(prim.box((0.02, 0.40, 0.22), (side * 0.17, BORE_Y - 0.42, 0.02)), PURPLE)
        add(prim.box((0.025, 0.09, 0.14), (side * 0.175, BORE_Y - 0.30, 0.02)), GOLD)

    # ── Tete. Le visage est PEINT (voir `scene`), pas sculpte : des yeux
    # modelises de 2 cm sur une tete de 30 cm disparaissent a la taille d'un
    # sprite. La sphere est donc nue ici.
    return out


HEAD = (0.0, BORE_Y + 0.26, 0.72)
HEAD_R = 0.32
FACE_PALETTE = (EYE, DARK, (0.30, 0.20, 0.12), TONGUE, (0.42, 0.28, 0.16))


def scene(blink: float = 0.0, mouth: float = 1.0, brow: float = 0.0) -> list[render3d.Instance]:
    """Instances pretes a rendre. La couronne est posee ici plutot que dans
    `build` : elle depend de la position de la tete, et la dupliquer serait
    exactement la grandeur ecrite deux fois qu'on cherche a eviter."""
    parts = build()
    hy, hz = HEAD[1], HEAD[2]

    # La tete, avec son visage peint. Il regarde vers +z, c'est-a-dire vers le
    # JOUEUR — une premiere version l'avait tourne vers la bouche du canon, donc
    # invisible de dos, la ou il doit precisement nous regarder.
    parts.append(
        render3d.Instance(
            prim.sphere(HEAD_R, HEAD, segments=22, rings=14),
            colour=SKIN,
            detail=detail.face(HEAD, blink=blink, mouth=mouth, brow=brow,
                               palette=FACE_PALETTE),
        )
    )
    # Machoire : elle avance sous le crane et donne un menton.
    parts.append(
        render3d.Instance(
            prim.box((0.44, 0.20, 0.34), (0, hy - 0.26, hz - 0.02)), colour=SKIN
        )
    )

    # Bandeau : un anneau redresse, pose sur le crane.
    band = _upright(prim.cylinder(0.245, -0.055, 0.055, (0, 0), segments=20))
    band = prim.Mesh(
        band.positions + np.array([0.0, hy + 0.26, hz], np.float32),
        band.normals, band.uvs, band.indices, None,
    )
    parts.append(render3d.Instance(band, colour=GOLD))

    # Trois pointes, la centrale plus haute.
    for dx, dz, h in ((-0.15, -0.05, 0.17), (0.0, -0.09, 0.24), (0.15, -0.05, 0.17)):
        spike = prim.box((0.085, h, 0.085), (dx, hy + 0.30 + h * 0.5, hz + dz))
        parts.append(render3d.Instance(spike, colour=GOLD))

    # Gemme frontale : c'est la HAUSSE en visee, alignee sur le cristal de bouche.
    parts.append(
        render3d.Instance(
            prim.sphere(0.058, (0.0, hy + 0.30, hz - 0.20), segments=6, rings=5),
            colour=PURPLE,
        )
    )
    return parts


# ── Vues ───────────────────────────────────────────────────────────────────
# Toutes derivees du MEME modele : elles ne peuvent pas se contredire.

# La camera est en -z et regarde vers +z : elle voit donc d'abord ce qui a le z
# le plus PETIT. La bouche etant en -z, yaw = 0 la met face a nous — c'est la vue
# AVANT, pas la vue de dos. Inverse au premier essai, et c'est le genre d'erreur
# qu'un modele 3D revele immediatement : sur deux dessins separes, elle serait
# passee inapercue.
VIEWS = {
    "profil": (90.0, 6.0),
    "trois_quarts": (128.0, 14.0),  # le cadrage Valorant, absent des fiches
    "dos": (180.0, 4.0),  # la visee
    "avant": (0.0, 4.0),
    "dessus": (90.0, 70.0),
}
