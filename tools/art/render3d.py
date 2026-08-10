"""Rasteriseur logiciel minimal : un maillage GLB -> une image, sous l'angle voulu.

Pipeline « modele 3D photographie puis reduit », celui de Doom / Duke Nukem 3D.
On rend en suréchantillonnage puis on reduit : c'est ce qui donne des aretes
propres. Rendre directement a 128 px ferait scintiller un maillage de 30 000
triangles dont la plupart sont sous-pixel.

Repere de sortie : +x a droite, +y en haut, +z vers l'avant (loin du joueur).
La camera est en -z et regarde vers +z.
"""

from __future__ import annotations

import math
from dataclasses import dataclass

import numpy as np
from PIL import Image

from glb import Mesh


@dataclass
class View:
    """Camera + placement de l'arme.

    `offset` place l'arme DANS le repere camera. La descendre sous l'axe est ce
    qui fait converger son bout lointain vers le centre de l'ecran — c'est de la
    que vient la fuite d'un viewmodel, pas d'une inclinaison de camera.
    """

    yaw: float = 25.0
    pitch: float = 8.0
    roll: float = 0.0
    offset: tuple[float, float, float] = (0.0, 0.0, 0.0)
    distance: float = 3.0
    focal: float = 300.0
    light: tuple[float, float, float] = (-0.4, 0.8, -0.45)
    ambient: float = 0.45


def _rotation(view: View) -> np.ndarray:
    ya, pa, ra = (math.radians(v) for v in (view.yaw, view.pitch, view.roll))
    cy, sy = math.cos(ya), math.sin(ya)
    cp, sp = math.cos(pa), math.sin(pa)
    cr, sr = math.cos(ra), math.sin(ra)
    ry = np.array([[cy, 0, sy], [0, 1, 0], [-sy, 0, cy]], np.float32)
    rx = np.array([[1, 0, 0], [0, cp, -sp], [0, sp, cp]], np.float32)
    rz = np.array([[cr, -sr, 0], [sr, cr, 0], [0, 0, 1]], np.float32)
    return rz @ rx @ ry


@dataclass
class Instance:
    """Une piece dans la scene : maillage + son placement propre.

    Indispensable des qu'on rend l'arme ET la main ensemble. Les composer apres
    coup, en superposant deux images, forcerait l'une a toujours masquer l'autre ;
    ici elles partagent un vrai tampon de profondeur, donc les doigts passent
    devant la crosse et le pouce derriere le barillet.
    """

    mesh: Mesh
    matrix: np.ndarray | None = None
    translation: tuple[float, float, float] = (0.0, 0.0, 0.0)
    #: Couleur a plat (0-1) qui remplace la texture. Sert aux planches de
    #: controle : une piece se juge a sa forme, pas a son habillage.
    colour: tuple[float, float, float] | None = None
    #: Fonction de DETAIL, evaluee par pixel : (positions monde, normales,
    #: albedo) -> albedo. C'est elle qui porte rivets, coutures et gravures.
    #:
    #: Elle lit la position dans l'espace, pas des UV : aucune primitive n'a
    #: besoin d'etre depliee, et le motif reste continu d'une piece a l'autre.
    #: C'est le principe du mappage triplanaire.
    detail: object | None = None


def render(
    scene: Mesh | Instance | list[Instance],
    view: View,
    size: tuple[int, int],
    supersample: int = 4,
    model_matrix: np.ndarray | None = None,
) -> Image.Image:
    """Rend la scene en RGBA. Le fond reste transparent."""
    if isinstance(scene, Mesh):
        scene = [Instance(scene, model_matrix)]
    elif isinstance(scene, Instance):
        scene = [scene]

    width, height = size[0] * supersample, size[1] * supersample
    focal = view.focal * supersample
    rot = _rotation(view)
    offset = np.array(view.offset, np.float32)

    all_cam, all_nrm, all_uv, all_tri, all_world = [], [], [], [], []
    texture_of: list[tuple[int, int, np.ndarray | None]] = []
    detail_of: list[object] = []
    base = 0
    for inst in scene:
        positions, normals = inst.mesh.positions, inst.mesh.normals
        if inst.matrix is not None:
            positions = positions @ inst.matrix.T
            normals = normals @ inst.matrix.T
        positions = positions + np.array(inst.translation, np.float32)
        all_world.append(positions)
        all_cam.append(positions @ rot.T + offset)
        all_nrm.append(normals @ rot.T)
        all_uv.append(inst.mesh.uvs)
        all_tri.append(inst.mesh.indices + base)
        tex = (
            np.array(inst.colour, np.float32).reshape(1, 1, 3)
            if inst.colour is not None
            else (
                np.asarray(inst.mesh.base_color, np.float32) / 255.0
                if inst.mesh.base_color is not None
                else None
            )
        )
        texture_of.append((base, base + len(positions), tex))
        detail_of.append(inst.detail)
        base += len(positions)

    cam = np.concatenate(all_cam)
    world = np.concatenate(all_world)
    nrm = np.concatenate(all_nrm)
    uv = np.concatenate(all_uv)
    tri = np.concatenate(all_tri)

    depth = cam[:, 2] + view.distance
    depth = np.maximum(depth, 1e-3)
    # CHIRALITE. Le glTF est un repere DROITIER : +x a droite, +y en haut, +z
    # vers le spectateur. On regarde ici le long de +z, donc +x tombe a GAUCHE de
    # l'ecran. Projeter +x a droite revient a lire tous les maillages en miroir.
    #
    # Une arme quasi symetrique ne trahit pas l'inversion — un revolver miroir
    # reste un revolver. Une MAIN, si : `fps_arm_R.glb` se rendait en main gauche,
    # et c'est le seul symptome qui l'a revele, apres plusieurs passes de reglage
    # de prise sur une main du mauvais cote.
    sx = width * 0.5 - focal * cam[:, 0] / depth
    sy = height * 0.5 - focal * cam[:, 1] / depth
    p0, p1, p2 = (np.stack([sx[tri[:, i]], sy[tri[:, i]]], axis=1) for i in range(3))
    inv_w = 1.0 / depth

    # Faces arriere : aire signee a l'ecran. `doubleSided` dans le materiau, mais
    # garder les deux faces coute le double pour un resultat identique de dos.
    area = (p1[:, 0] - p0[:, 0]) * (p2[:, 1] - p0[:, 1]) - (p2[:, 0] - p0[:, 0]) * (
        p1[:, 1] - p0[:, 1]
    )
    keep = np.abs(area) > 1e-6
    tri, area = tri[keep], area[keep]
    p0, p1, p2 = p0[keep], p1[keep], p2[keep]

    zbuf = np.full((height, width), np.inf, np.float32)
    colour = np.zeros((height, width, 3), np.float32)
    mask = np.zeros((height, width), bool)

    light = np.array(view.light, np.float32)
    light = light / np.linalg.norm(light)

    # Chaque triangle sait quelle texture l'habille : la scene en melange
    # plusieurs (l'arme et le gant n'ont pas le meme atlas).
    textures = [t for _, _, t in texture_of]
    tri_tex = np.zeros(len(tri), np.int32)
    # `tri` est deja filtre des faces arriere a ce stade — le refiltrer ici
    # decalerait la correspondance triangle -> texture d'un cran.
    for slot, (lo, hi, _) in enumerate(texture_of):
        tri_tex[(tri[:, 0] >= lo) & (tri[:, 0] < hi)] = slot

    xmin = np.clip(np.floor(np.minimum(np.minimum(p0[:, 0], p1[:, 0]), p2[:, 0])), 0, width - 1)
    xmax = np.clip(np.ceil(np.maximum(np.maximum(p0[:, 0], p1[:, 0]), p2[:, 0])), 0, width - 1)
    ymin = np.clip(np.floor(np.minimum(np.minimum(p0[:, 1], p1[:, 1]), p2[:, 1])), 0, height - 1)
    ymax = np.clip(np.ceil(np.maximum(np.maximum(p0[:, 1], p1[:, 1]), p2[:, 1])), 0, height - 1)

    for t in range(len(tri)):
        x0, x1 = int(xmin[t]), int(xmax[t])
        y0, y1 = int(ymin[t]), int(ymax[t])
        if x1 < x0 or y1 < y0:
            continue
        gx, gy = np.meshgrid(np.arange(x0, x1 + 1) + 0.5, np.arange(y0, y1 + 1) + 0.5)

        ax, ay = p0[t]
        bx, by = p1[t]
        cx, cy = p2[t]
        inv_area = 1.0 / area[t]
        w0 = ((bx - gx) * (cy - gy) - (cx - gx) * (by - gy)) * inv_area
        w1 = ((cx - gx) * (ay - gy) - (ax - gx) * (cy - gy)) * inv_area
        w2 = 1.0 - w0 - w1
        inside = (w0 >= 0) & (w1 >= 0) & (w2 >= 0)
        if not inside.any():
            continue

        i0, i1, i2 = tri[t]
        # Correction de perspective : interpoler 1/w puis diviser. Sans ca, une
        # texture sur un triangle vu en biais glisse visiblement.
        wsum = w0 * inv_w[i0] + w1 * inv_w[i1] + w2 * inv_w[i2]
        z = 1.0 / np.maximum(wsum, 1e-9)

        ys, xs = np.nonzero(inside)
        py, px = ys + y0, xs + x0
        zz = z[inside]
        closer = zz < zbuf[py, px]
        if not closer.any():
            continue
        py, px, zz = py[closer], px[closer], zz[closer]
        b0 = (w0[inside][closer] * inv_w[i0]) * zz
        b1 = (w1[inside][closer] * inv_w[i1]) * zz
        b2 = (w2[inside][closer] * inv_w[i2]) * zz

        texture = textures[tri_tex[t]]
        if texture is not None:
            u = b0 * uv[i0, 0] + b1 * uv[i1, 0] + b2 * uv[i2, 0]
            v = b0 * uv[i0, 1] + b1 * uv[i1, 1] + b2 * uv[i2, 1]
            tw, th = texture.shape[1], texture.shape[0]
            tx = np.clip((u % 1.0) * tw, 0, tw - 1).astype(np.int32)
            ty = np.clip((v % 1.0) * th, 0, th - 1).astype(np.int32)
            albedo = texture[ty, tx]
        else:
            albedo = np.full((len(py), 3), 0.7, np.float32)

        # Position MONDE du pixel : c'est elle que la fonction de detail lit.
        fn = detail_of[tri_tex[t]]
        if fn is not None:
            wp = (
                b0[:, None] * world[i0]
                + b1[:, None] * world[i1]
                + b2[:, None] * world[i2]
            )
            albedo = fn(wp, nrm[i0], albedo)

        n = b0[:, None] * nrm[i0] + b1[:, None] * nrm[i1] + b2[:, None] * nrm[i2]
        norm = np.linalg.norm(n, axis=1, keepdims=True)
        n = n / np.maximum(norm, 1e-6)
        lam = np.clip(n @ light, 0.0, 1.0)
        shade = view.ambient + (1.0 - view.ambient) * lam

        zbuf[py, px] = zz
        colour[py, px] = albedo * shade[:, None]
        mask[py, px] = True

    rgba = np.zeros((height, width, 4), np.uint8)
    rgba[..., :3] = np.clip(colour * 255.0, 0, 255).astype(np.uint8)
    rgba[..., 3] = mask * 255
    img = Image.fromarray(rgba, "RGBA")
    if supersample > 1:
        img = img.resize(size, Image.Resampling.BOX)
    return img
