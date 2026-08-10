"""Banc d'essai des mains, SANS l'arme.

Pourquoi isoler : sur l'arme complete, deux objets qui se superposent a l'ecran
peuvent etre a dix centimetres l'un de l'autre en 3D. Un placement valide « a
l'angle du viewmodel » s'est revele flottant des qu'on le regardait de trois
quarts. Ici la main tient un cylindre nu — il n'y a plus rien derriere quoi se
cacher.

Le cylindre a les COTES MESUREES du manche de Pepin :

    section a mi-hauteur   0.369 (x) x 0.378 (z)  -> quasi circulaire, r = 0.19
    hauteur utile          0.72
    inclinaison reelle     15.7 deg  (le banc le garde VERTICAL : on regle la
                           prise d'abord, l'inclinaison se remettra apres)

Anatomie du poing, relevee par rendu sous cinq angles : les doigts s'empilent
le long de Y et se referment dans le plan XZ. Ce poing agrippe donc un manche
VERTICAL — c'est bien une prise de pistolet, et le canal de prise court le long
de Y.
"""

from __future__ import annotations

import math

import numpy as np
from PIL import Image, ImageDraw

import framing
import glb
import primitives as prim
import render3d

ARM_R = "../../assets/models/arms/fps_arm_R.glb"
ARM_L = "../../assets/models/arms/fps_arm_L.glb"

# Manche : cotes mesurees sur la crosse reelle (voir en-tete).
GRIP_R = 0.19
GRIP_TOP = 0.36
GRIP_BOTTOM = -0.36

GRIP_COLOUR = (0.42, 0.46, 0.52)


def grip_cylinder() -> glb.Mesh:
    """Manche VERTICAL. `primitives.cylinder` construit le long de Z — il faut le
    redresser, sinon on obtient un rouleau couche et la camera regarde dans son
    axe (premier essai : quatre vues montrant un disque)."""
    body = prim.cylinder(GRIP_R, GRIP_BOTTOM, GRIP_TOP, segments=28)
    upright = np.array([[1, 0, 0], [0, 0, 1], [0, -1, 0]], np.float32)
    body = glb.Mesh(
        body.positions @ upright.T, body.normals @ upright.T, body.uvs, body.indices, None
    )
    # Repere de HAUT : sans lui, impossible de dire dans quel sens la main tient
    # le manche sur un rendu fixe.
    cap = prim.box((0.44, 0.05, 0.44), (0, GRIP_TOP + 0.03, 0))
    return prim.merge([body, cap])


def _rotate(pitch: float, yaw: float, roll: float, scale: float) -> np.ndarray:
    p, y, r = (math.radians(v) for v in (pitch, yaw, roll))
    rx = np.array([[1, 0, 0], [0, math.cos(p), -math.sin(p)], [0, math.sin(p), math.cos(p)]], np.float32)
    ry = np.array([[math.cos(y), 0, math.sin(y)], [0, 1, 0], [-math.sin(y), 0, math.cos(y)]], np.float32)
    rz = np.array([[math.cos(r), -math.sin(r), 0], [math.sin(r), math.cos(r), 0], [0, 0, 1]], np.float32)
    return (rx @ ry @ rz) * scale


# Centre du POING dans le repere du maillage (releve sur ses bornes, partie
# main seule : y > -0.10). C'est autour de lui que tout doit tourner.
FIST_CENTRE = np.array([0.011, -0.017, 0.022], np.float32)

# Bande de flexion. LARGE a dessein : elle couvre presque tout l'avant-bras, si
# bien que l'inclinaison s'y repartit au lieu de se concentrer sur le poignet.
#
# C'est ce qui permet d'amener l'avant-bras a l'HORIZONTALE sans que rien ne
# casse. Sur une bande etroite (-0.16), passe 55 deg le poignet se tasse et se
# plisse — un vrai poignet ne va pas au-dela. Reparti sur 0.46, le bras s'incurve
# comme un bras, et le poing reste d'equerre sur le manche.
WRIST_TOP = -0.04
WRIST_BOTTOM = -0.18
#: ZERO par defaut : on ne deforme rien. La flexion existe si on la demande, mais
#: elle n'est PAS le moyen d'obtenir un avant-bras horizontal — s'en servir pour
#: ça revient a tordre les os pour compenser un manche qu'on a decrete vertical.
#: Le bon levier est TILT (voir `scene`), qui incline main et manche ensemble.
WRIST_BEND = 0.0

#: Inclinaison du bloc main+manche. Le bras reste rigide, dans sa pose de repos,
#: et l'avant-bras se rapproche de l'horizontale. Ce que ça coute : le manche
#: s'incline d'autant. Les deux s'echangent UN POUR UN, parce que sur cette pose
#: l'avant-bras prolonge l'axe du manche — c'est une contrainte du maillage, pas
#: un reglage. L'arme devra donc adopter cette inclinaison au remontage.
BLOCK_TILT = -55.0


def bend_wrist(mesh: glb.Mesh, degrees: float) -> glb.Mesh:
    """Plie l'AVANT-BRAS au poignet, en laissant le poing intact.

    Sans ça, incliner le bras faisait pivoter le POING avec lui : les doigts
    traversaient alors le manche en diagonale au lieu de l'enserrer a
    l'horizontale, ce qui se lit comme un poignet demis. Le maillage etant rigide
    (512 triangles, aucune articulation), on le plie nous-memes.

    La transition est lissee sur la bande du poignet — une coupure nette y
    produirait un pli d'accordeon bien visible.
    """
    if not degrees:
        return mesh
    p = mesh.positions.copy()
    n = mesh.normals.copy()

    span = WRIST_TOP - WRIST_BOTTOM
    w = np.clip((WRIST_TOP - p[:, 1]) / span, 0.0, 1.0)
    w = w * w * (3.0 - 2.0 * w)  # lissage cubique
    angles = math.radians(degrees) * w
    ca, sa = np.cos(angles), np.sin(angles)

    pivot_y = WRIST_TOP
    ry = p[:, 1] - pivot_y
    rz = p[:, 2]
    p[:, 1] = pivot_y + ry * ca - rz * sa
    p[:, 2] = ry * sa + rz * ca

    ny, nz = n[:, 1].copy(), n[:, 2].copy()
    n[:, 1] = ny * ca - nz * sa
    n[:, 2] = ny * sa + nz * ca

    return glb.Mesh(p, n, mesh.uvs, mesh.indices, mesh.base_color)


def scene(
    hand_path: str,
    at,
    pitch=0.0,
    yaw=0.0,
    roll=0.0,
    scale=1.0,
    with_grip=True,
    wrist=0.0,
    tilt=0.0,
):
    """`at` = ou tombe le POING (pas l'origine du maillage, qui est au coude).

    `tilt` incline la MAIN ET LE MANCHE ensemble, d'un seul bloc.

    C'est le bon levier, et le seul honnete. Vouloir un avant-bras horizontal en
    PLIANT le bras revient a tordre les os pour compenser un manche qu'on a
    decrete vertical — le bras finit en banane. Ici on ne deforme rien : la main
    garde sa pose naturelle, le manche s'oriente sur elle, et l'angle qui donne
    un avant-bras horizontal est justement l'inclinaison que l'arme devra
    adopter. La main est la reference, pas l'arme.
    """
    out = []
    block = _rotate(tilt, 0.0, 0.0, 1.0)  # inclinaison commune main + manche
    pivot = np.array(at, np.float32)
    shift = tuple(float(v) for v in (pivot - block @ pivot))

    if with_grip:
        out.append(render3d.Instance(grip_cylinder(), block, shift, colour=GRIP_COLOUR))

    hand = bend_wrist(glb.load(hand_path), wrist)
    matrix = block @ _rotate(pitch, yaw, roll, scale)
    translation = tuple(float(v) for v in (pivot - matrix @ FIST_CENTRE))
    out.append(render3d.Instance(hand, matrix, translation))
    return out


VIEWS = (
    ("face", 0.0, 6.0),
    ("flanc droit", 90.0, 6.0),
    ("dos", 180.0, 6.0),
    ("dessus", 20.0, 62.0),
)


def sheet(cases, path: str, height: int = 210) -> None:
    """Une ligne par cas, une colonne par angle. Un placement ne se juge JAMAIS
    sous un seul angle — c'est ce qui a laisse passer des mains flottantes."""
    rows = []
    for label, kwargs in cases:
        row = []
        for view_name, yaw, pitch in VIEWS:
            v = render3d.View(yaw=yaw, pitch=pitch, distance=1.5, focal=300.0, ambient=0.45)
            im = framing.shoot(
                scene(**kwargs), v, np.eye(3, dtype=np.float32), target_height=height, pad=16
            )
            d = ImageDraw.Draw(im)
            d.text((4, 4), f"{label} · {view_name}", fill=(255, 255, 255, 255))
            row.append(im)
        rows.append(row)
    w = max(c.width for r in rows for c in r)
    h = max(c.height for r in rows for c in r)
    out = Image.new("RGBA", (w * len(VIEWS), h * len(rows)), (18, 26, 25, 255))
    for j, r in enumerate(rows):
        for i, c in enumerate(r):
            out.alpha_composite(c, (i * w, j * h))
    out.save(path)
