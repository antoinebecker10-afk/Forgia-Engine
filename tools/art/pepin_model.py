"""Pepin simplifie : le revolver rebati en primitives, une piece = un objet.

Pourquoi cette version existe : le GLB du jeu est une coque SOUDEE de 30 000
triangles, couverte de fourrure. Trois tentatives de decoupage par seuils
geometriques ont donne des pieces approximatives (un barillet tronque, un pontet
qui avalait la crosse). Ici la question ne se pose plus — le barillet est un
cylindre parce qu'on l'a fabrique comme tel, et son axe de basculement est connu
exactement au lieu d'etre estime.

**Toutes les cotes viennent du vrai modele**, mesurees sur ses sommets, pour que
la version simplifiee tombe dans le meme cadrage et a la meme echelle :

    longueur      z de -0.95 (talon) a +0.96 (bouche)
    hauteur       y de -0.68 a +0.68
    epaisseur     x de -0.21 a +0.21
    ame           y = 0.195
    barillet      r = 0.285, z de -0.33 a 0.00   (profil radial)
    canon         r < 0.22,  z de 0.00 a 0.62
    gueule        z > 0.62, charniere a hauteur d'ame
    yeux          y > 0.50, z de 0.13 a 0.36
    crosse        z < -0.52
    pontet        z de -0.60 a -0.15, y de -0.34 a -0.02
    chien         z de -0.59 a -0.33, y > 0.30
"""

from __future__ import annotations

import math

import numpy as np

import primitives as prim
import render3d

BORE = 0.195
CYL_R = 0.285
CYL_Z = (-0.33, 0.00)
MOUTH_Z = 0.62

# Palette de Pepin, relevee sur sa texture : metal vert d'eau, fourrure orange,
# yeux creme, dents blanches, gueule rose.
COLOURS = {
    "metal": "#8fb5ac",
    "metal_sombre": "#5d7d78",
    "fourrure": "#c1601f",
    "fourrure_claire": "#e08a3c",
    "oeil": "#f0ece0",
    "pupille": "#241c24",
    "dent": "#f5f2e6",
    "gueule": "#a83a58",
}

# Couleurs de PLANCHE (une teinte franche par piece) — pour lire le decoupage.
MAP_COLOURS = {
    "barillet": "#c46be0",
    "canon": "#8de08a",
    "carcasse": "#7d8fa6",
    "crosse": "#b07a4a",
    "pontet": "#e07ab0",
    "detente": "#ff5fa2",
    "chien": "#e0d44a",
    "oeil_gauche": "#4aa3d8",
    "oeil_droit": "#7ec8f0",
    "machoire_haute": "#d94f4f",
    "machoire_basse": "#f5a03c",
}


def _hex(h: str):
    h = h.lstrip("#")
    return tuple(int(h[i : i + 2], 16) / 255.0 for i in (0, 2, 4))


def rot_x(deg: float) -> np.ndarray:
    a = math.radians(deg)
    c, s = math.cos(a), math.sin(a)
    return np.array([[1, 0, 0], [0, c, -s], [0, s, c]], np.float32)


def rot_z(deg: float) -> np.ndarray:
    a = math.radians(deg)
    c, s = math.cos(a), math.sin(a)
    return np.array([[c, -s, 0], [s, c, 0], [0, 0, 1]], np.float32)


def about(pivot, matrix: np.ndarray):
    """Translation qui fait tourner AUTOUR de `pivot` au lieu de l'origine."""
    p = np.asarray(pivot, np.float32)
    return tuple((p - matrix @ p).astype(float))


# ── Les pieces ─────────────────────────────────────────────────────────────


def build_parts() -> dict[str, tuple[prim.Mesh, str]]:
    """Une entree par piece : (maillage, couleur d'habillage)."""
    parts: dict[str, tuple[prim.Mesh, str]] = {}

    # Barillet : le tambour, ses cannelures, et six chambres creusees en bout.
    drum = [prim.cylinder(CYL_R, CYL_Z[0], CYL_Z[1], (0, BORE), segments=24)]
    for i in range(6):
        a = 2 * math.pi * i / 6
        cx, cy = math.cos(a) * 0.165, BORE + math.sin(a) * 0.165
        # Chambre : un petit cylindre en retrait, qui creuse visuellement le bout.
        drum.append(
            prim.cylinder(0.062, CYL_Z[0] - 0.004, CYL_Z[0] + 0.09, (cx, cy), segments=10)
        )
        # Cannelure entre deux chambres.
        b = a + math.pi / 6
        drum.append(
            prim.box(
                (0.05, 0.05, CYL_Z[1] - CYL_Z[0] - 0.04),
                (math.cos(b) * 0.27, BORE + math.sin(b) * 0.27, sum(CYL_Z) / 2),
            )
        )
    parts["barillet"] = (prim.merge(drum), COLOURS["metal"])

    # Canon : tube + bande de visee, et la tige d'ejecteur sous le canon.
    barrel = [
        # Rayon genereux : le tube reel est plus fin, mais il est habille de
        # fourrure et d'ecailles qui doivent le suivre.
        prim.cylinder(0.20, CYL_Z[1], MOUTH_Z + 0.06, (0, BORE), segments=20),
        prim.box((0.10, 0.06, MOUTH_Z - 0.02), (0, BORE + 0.15, (CYL_Z[1] + MOUTH_Z) / 2)),
        prim.cylinder(0.045, CYL_Z[1] - 0.02, MOUTH_Z - 0.04, (0, BORE - 0.17), segments=10),
    ]
    parts["canon"] = (prim.merge(barrel), COLOURS["metal"])

    # Carcasse : un U autour du barillet — pont superieur, arriere, et avant.
    # Elle ne se referme pas sur les cotes, sinon le barillet ne pourrait pas
    # sortir : la fenetre est un vrai vide, comme sur un revolver.
    # Le pont doit MONTER jusqu'a la crete de fourrure du dessus : sinon, en
    # liaison par proximite, cette crete tombe dans le barillet et bascule avec
    # lui au rechargement.
    frame = [
        prim.box((0.26, 0.22, 0.62), (0, BORE + CYL_R + 0.13, -0.10)),  # pont + crete
        prim.box((0.30, 0.52, 0.18), (0, BORE - 0.06, -0.44)),  # bouclier arriere
        prim.box((0.24, 0.30, 0.10), (0, BORE - 0.02, 0.06)),  # nez avant
        prim.box((0.16, 0.16, 0.24), (0, BORE - 0.22, -0.16)),  # embase sous barillet
    ]
    parts["carcasse"] = (prim.merge(frame), COLOURS["metal_sombre"])

    # Crosse : penchee vers l'arriere, en fourrure. Elle tourne autour de son
    # POINT D'ATTACHE sous la carcasse — la faire tourner autour de l'origine
    # l'envoyait en l'air, a hauteur du canon.
    # Volumineuse : la vraie crosse est empatee de fourrure et va jusqu'au talon
    # (z = -0.95). Trop maigre, le pontet lui prenait tout son devant.
    grip = prim.box((0.40, 0.80, 0.42), (0, -0.36, -0.70))
    anchor = np.array([0.0, -0.08, -0.56], np.float32)
    r = rot_x(22.0)
    grip.positions[:] = (r @ (grip.positions - anchor).T).T + anchor
    grip.normals[:] = (r @ grip.normals.T).T
    parts["crosse"] = (grip, COLOURS["fourrure"])

    # Pontet : trois barres qui ferment une boucle sous la carcasse.
    guard = [
        prim.box((0.09, 0.22, 0.05), (0, -0.13, -0.20)),
        prim.box((0.09, 0.05, 0.34), (0, -0.26, -0.36)),
        prim.box((0.09, 0.18, 0.05), (0, -0.14, -0.52)),
    ]
    parts["pontet"] = (prim.merge(guard), COLOURS["metal_sombre"])
    parts["detente"] = (prim.box((0.06, 0.20, 0.05), (0, -0.14, -0.30)), COLOURS["metal"])

    # Chien : le marteau, en haut a l'arriere.
    parts["chien"] = (prim.box((0.09, 0.20, 0.13), (0, 0.47, -0.47)), COLOURS["metal"])

    # Yeux : globe + pupille, poses sur le dessus du canon.
    for side, name in ((-1, "oeil_gauche"), (1, "oeil_droit")):
        cx = side * 0.135
        eye = [
            prim.sphere(0.115, (cx, 0.575, 0.245)),
            prim.sphere(0.052, (cx + side * 0.028, 0.565, 0.335)),
        ]
        parts[name] = (prim.merge(eye), COLOURS["oeil"])
        parts[name + "_pupille"] = (
            prim.sphere(0.050, (cx + side * 0.030, 0.565, 0.340)),
            COLOURS["pupille"],
        )

    # Gueule : deux machoires articulees a hauteur d'ame, avec leurs dents.
    for name, y0, y1, sign in (
        ("machoire_haute", BORE + 0.02, BORE + 0.26, 1),
        ("machoire_basse", BORE - 0.26, BORE - 0.02, -1),
    ):
        jaw = [prim.box((0.30, y1 - y0, 0.30), (0, (y0 + y1) / 2, MOUTH_Z + 0.20))]
        for i in range(4):
            tx = -0.105 + i * 0.07
            jaw.append(
                prim.box((0.05, 0.09, 0.05), (tx, y0 if sign > 0 else y1, MOUTH_Z + 0.33))
            )
        parts[name] = (prim.merge(jaw), COLOURS["dent"] if sign < 0 else COLOURS["metal"])

    return parts


# ── Pose ───────────────────────────────────────────────────────────────────

# Axe du yoke : parallele au canon, a GAUCHE et sous l'ame. Le barillet sort
# donc sur le cote en descendant sous l'arme. Ici l'axe est CHOISI a la
# construction, pas devine sur un nuage de points.
YOKE = (-0.30, 0.05, 0.0)
JAW_HINGE = (0.0, BORE, MOUTH_Z)


def pose(
    parts: dict,
    swing: float = 0.0,
    jaw: float = 0.0,
    blink_left: float = 0.0,
    blink_right: float = 0.0,
    hammer: float = 0.0,
    trigger: float = 0.0,
    flat: bool = False,
) -> list[render3d.Instance]:
    """Assemble la scene pour un etat donne.

    `swing` = bascule du barillet (deg), `jaw` = ouverture de la machoire basse
    (deg), `blink_*` = 0 ouvert / 1 ferme, `hammer` = armement (deg).
    """
    out = []
    for name, (mesh, colour) in parts.items():
        matrix = np.eye(3, dtype=np.float32)
        translation = (0.0, 0.0, 0.0)

        if name == "barillet" and swing:
            matrix = rot_z(swing)
            translation = about(YOKE, matrix)
        elif name == "machoire_basse" and jaw:
            matrix = rot_x(-jaw)
            translation = about(JAW_HINGE, matrix)
        elif name == "chien" and hammer:
            matrix = rot_x(hammer)
            translation = about((0.0, 0.37, -0.47), matrix)
        elif name == "detente" and trigger:
            matrix = rot_x(trigger)
            translation = about((0.0, -0.04, -0.30), matrix)
        elif name.startswith("oeil"):
            amount = blink_left if "gauche" in name else blink_right
            if amount:
                # Clignement = ECRASEMENT vertical du globe. Il n'y a pas de
                # paupiere a modeliser, et a cette taille l'ecrasement se lit
                # exactement comme un clin d'oeil.
                squash = 1.0 - 0.92 * amount
                matrix = np.diag([1.0, squash, 1.0]).astype(np.float32)
                pivot = (0.0, 0.575, 0.0)
                translation = about(pivot, matrix)

        key = name.replace("_pupille", "")
        out.append(
            render3d.Instance(
                mesh,
                matrix,
                translation,
                colour=_hex(MAP_COLOURS.get(key, "#888888")) if flat else _hex(colour),
            )
        )
    return out
