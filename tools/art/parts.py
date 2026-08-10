"""Decoupage de Pepin en pieces nommees, depuis un maillage SOUDE.

Le GLB est une seule coque connexe : aucune decoupe topologique n'est possible.
Chaque piece se definit donc par un critere geometrique, et parfois par la
couleur de sa texture (les dents sont blanches, la fourrure orange).

**Le controle qui vaut** : ne pas regarder la piece extraite — a peu pres
n'importe quelle tranche d'un revolver ressemble a un morceau de mecanique
convaincant. Regarder ce qui RESTE, et verifier que le trou tombe ou il doit.
Une premiere version du barillet passait le coup d'oeil et prenait pourtant la
piece situee devant lui.

Les criteres sont ordonnes : un triangle va a la PREMIERE piece qui le reclame.
"""

from __future__ import annotations

import numpy as np

import glb

TO_GUN = np.array([[0, 0, 1], [0, 1, 0], [-1, 0, 0]], np.float32)
BORE_Y = 0.195  # hauteur de l'ame, mesuree sur les sommets du canon
CYL_R = 0.285  # rayon du tambour, MESURE sur le profil radial (pas choisi)

# Ordre = priorite. La couleur sert a la planche de controle.
PART_ORDER = [
    ("oeil_gauche", "#4aa3d8"),
    ("oeil_droit", "#7ec8f0"),
    ("machoire_haute", "#d94f4f"),
    ("machoire_basse", "#f5a03c"),
    ("canon", "#8de08a"),
    ("barillet", "#c46be0"),
    ("chien", "#e0d44a"),
    ("detente", "#e07ab0"),
    ("crosse", "#b07a4a"),
    ("carcasse", "#7d8fa6"),
]


def segment(mesh: glb.Mesh) -> dict[str, np.ndarray]:
    """Renvoie un masque de triangles par piece. Les masques sont disjoints."""
    p = mesh.positions @ TO_GUN.T
    c = p[mesh.indices].mean(axis=1)
    x, y, z = c[:, 0], c[:, 1], c[:, 2]
    radius = np.hypot(x, y - BORE_Y)

    remaining = np.ones(len(mesh.indices), bool)
    parts: dict[str, np.ndarray] = {}

    def claim(name: str, condition: np.ndarray) -> None:
        mask = condition & remaining
        parts[name] = mask
        remaining[mask] = False

    # Les yeux depassent nettement du corps : au-dessus de y = 0.50, et groupes
    # sur z = 0.03 a 0.48 (histogramme : 1 800 des 1 981 triangles hauts y sont).
    # Sans la borne en z, le critere ramassait aussi la fourrure du dessus, du
    # museau jusqu'a la crosse.
    eyes = (y > 0.50) & (z > 0.03) & (z < 0.48)
    claim("oeil_gauche", eyes & (x < 0))
    claim("oeil_droit", eyes & (x >= 0))

    # La gueule occupe la bouche du canon. La charniere est a hauteur de l'ame :
    # au-dessus la machoire haute (solidaire du canon), en dessous la BASSE, qui
    # est la piece qui s'ouvre quand l'arme parle.
    mouth = z > 0.62
    claim("machoire_haute", mouth & (y >= 0.235))
    claim("machoire_basse", mouth & (y < 0.235))

    # Barillet : un tambour plein autour de l'ame. Son rayon et sa longueur se
    # MESURENT sur le profil radial (`python -c` dans l'historique : de z=-0.30 a
    # z=0 l'enveloppe tient a r≈0.28, puis le pic tombe a 0.14 = le canon, plus
    # fin). Choisi au jugé a 0.235, le critere TRANCHAIT DANS le tambour : il n'en
    # gardait qu'un noyau, plus tout ce qui traînait dedans. Une condition peut
    # avoir la bonne forme et les mauvaises cotes.
    claim("barillet", (z >= -0.33) & (z < 0.005) & (radius < CYL_R))

    # Canon : le tube, plus fin. Au-dela de ce rayon on est sur la fourrure et
    # les ecailles qui l'habillent, pas sur la piece.
    claim("canon", (z >= 0.005) & (z <= 0.62) & (radius < 0.22))

    # Chien : en haut a l'arriere, derriere le barillet.
    claim("chien", (z < -0.33) & (z > -0.62) & (y > 0.30))
    # Detente et pontet : la boucle SOUS la carcasse. Bornee en bas, sinon elle
    # avale le devant de la crosse (elle ramassait jusqu'a y=-0.67).
    claim("detente", (z < -0.15) & (z > -0.60) & (y < -0.02) & (y > -0.34))
    # Crosse : tout l'arriere bas.
    claim("crosse", z < -0.52)

    parts["carcasse"] = remaining.copy()
    return parts


def part_mesh(mesh: glb.Mesh, mask: np.ndarray) -> glb.Mesh:
    return glb.Mesh(mesh.positions, mesh.normals, mesh.uvs, mesh.indices[mask], mesh.base_color)


def flat_colour_mesh(mesh: glb.Mesh, parts: dict[str, np.ndarray]) -> list[tuple[glb.Mesh, str]]:
    """Une entree par piece, avec sa couleur de planche."""
    out = []
    for name, colour in PART_ORDER:
        mask = parts.get(name)
        if mask is None or not mask.any():
            continue
        out.append((part_mesh(mesh, mask), colour))
    return out


def report(parts: dict[str, np.ndarray], mesh: glb.Mesh) -> str:
    p = mesh.positions @ TO_GUN.T
    c = p[mesh.indices].mean(axis=1)
    lines = []
    for name, _ in PART_ORDER:
        mask = parts.get(name)
        if mask is None or not mask.any():
            lines.append(f"{name:16s} VIDE")
            continue
        q = c[mask]
        lines.append(
            "%-16s n=%5d  x[%+.2f,%+.2f] y[%+.2f,%+.2f] z[%+.2f,%+.2f]"
            % (
                name,
                mask.sum(),
                q[:, 0].min(),
                q[:, 0].max(),
                q[:, 1].min(),
                q[:, 1].max(),
                q[:, 2].min(),
                q[:, 2].max(),
            )
        )
    total = sum(int(m.sum()) for m in parts.values())
    lines.append(f"total {total} / {len(mesh.indices)} triangles")
    return "\n".join(lines)
