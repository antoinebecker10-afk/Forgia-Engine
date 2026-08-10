"""Primitives maillees : boite, cylindre, sphere, cone.

Sert a rebatir une version SIMPLIFIEE d'une arme, ou chaque piece est un objet
des la construction. Segmenter un maillage soude par des seuils geometriques
donne toujours des pieces approximatives — ici la question ne se pose plus : le
barillet est un cylindre parce qu'on l'a fabrique comme tel.

Les maillages sortent au format de `glb.Mesh` pour passer dans le meme
rasteriseur et la meme reduction pixel art que le modele reel.
"""

from __future__ import annotations

import math

import numpy as np

from glb import Mesh


def _mesh(positions, normals, indices) -> Mesh:
    positions = np.asarray(positions, np.float32)
    return Mesh(
        positions,
        np.asarray(normals, np.float32),
        np.zeros((len(positions), 2), np.float32),
        np.asarray(indices, np.int64).reshape(-1, 3),
        None,
    )


def box(size=(1.0, 1.0, 1.0), center=(0.0, 0.0, 0.0)) -> Mesh:
    hx, hy, hz = (s * 0.5 for s in size)
    cx, cy, cz = center
    faces = (
        ((0, 0, -1), ((-1, -1, -1), (1, -1, -1), (1, 1, -1), (-1, 1, -1))),
        ((0, 0, 1), ((1, -1, 1), (-1, -1, 1), (-1, 1, 1), (1, 1, 1))),
        ((-1, 0, 0), ((-1, -1, 1), (-1, -1, -1), (-1, 1, -1), (-1, 1, 1))),
        ((1, 0, 0), ((1, -1, -1), (1, -1, 1), (1, 1, 1), (1, 1, -1))),
        ((0, 1, 0), ((-1, 1, -1), (1, 1, -1), (1, 1, 1), (-1, 1, 1))),
        ((0, -1, 0), ((-1, -1, 1), (1, -1, 1), (1, -1, -1), (-1, -1, -1))),
    )
    pos, nrm, idx = [], [], []
    for normal, corners in faces:
        base = len(pos)
        for sx, sy, sz in corners:
            pos.append((cx + sx * hx, cy + sy * hy, cz + sz * hz))
            nrm.append(normal)
        idx += [base, base + 1, base + 2, base, base + 2, base + 3]
    return _mesh(pos, nrm, idx)


def cylinder(
    radius: float,
    z0: float,
    z1: float,
    center=(0.0, 0.0),
    segments: int = 20,
    caps: bool = True,
    radius_top: float | None = None,
) -> Mesh:
    """Cylindre d'axe z. `radius_top` different -> cone tronque."""
    cx, cy = center
    r1 = radius if radius_top is None else radius_top
    pos, nrm, idx = [], [], []

    for i in range(segments):
        a0 = 2 * math.pi * i / segments
        a1 = 2 * math.pi * (i + 1) / segments
        c0, s0 = math.cos(a0), math.sin(a0)
        c1, s1 = math.cos(a1), math.sin(a1)
        quad = (
            (cx + c0 * radius, cy + s0 * radius, z0, c0, s0),
            (cx + c1 * radius, cy + s1 * radius, z0, c1, s1),
            (cx + c1 * r1, cy + s1 * r1, z1, c1, s1),
            (cx + c0 * r1, cy + s0 * r1, z1, c0, s0),
        )
        base = len(pos)
        for x, y, z, nx, ny in quad:
            pos.append((x, y, z))
            nrm.append((nx, ny, 0.0))
        idx += [base, base + 1, base + 2, base, base + 2, base + 3]

    if caps:
        for z, r, sign in ((z0, radius, -1.0), (z1, r1, 1.0)):
            base = len(pos)
            pos.append((cx, cy, z))
            nrm.append((0.0, 0.0, sign))
            for i in range(segments + 1):
                a = 2 * math.pi * i / segments
                pos.append((cx + math.cos(a) * r, cy + math.sin(a) * r, z))
                nrm.append((0.0, 0.0, sign))
            for i in range(segments):
                if sign < 0:
                    idx += [base, base + i + 2, base + i + 1]
                else:
                    idx += [base, base + i + 1, base + i + 2]
    return _mesh(pos, nrm, idx)


def sphere(radius: float, center=(0.0, 0.0, 0.0), segments: int = 16, rings: int = 10) -> Mesh:
    cx, cy, cz = center
    pos, nrm, idx = [], [], []
    for r in range(rings + 1):
        phi = math.pi * r / rings
        for s in range(segments + 1):
            theta = 2 * math.pi * s / segments
            nx = math.sin(phi) * math.cos(theta)
            ny = math.cos(phi)
            nz = math.sin(phi) * math.sin(theta)
            pos.append((cx + nx * radius, cy + ny * radius, cz + nz * radius))
            nrm.append((nx, ny, nz))
    for r in range(rings):
        for s in range(segments):
            a = r * (segments + 1) + s
            b = a + segments + 1
            idx += [a, b, a + 1, a + 1, b, b + 1]
    return _mesh(pos, nrm, idx)


def merge(meshes: list[Mesh]) -> Mesh:
    """Fusionne plusieurs primitives en UN maillage — pour une piece composee
    (une machoire et ses dents bougent ensemble)."""
    positions, normals, indices = [], [], []
    base = 0
    for m in meshes:
        positions.append(m.positions)
        normals.append(m.normals)
        indices.append(m.indices + base)
        base += len(m.positions)
    return _mesh(
        np.concatenate(positions), np.concatenate(normals), np.concatenate(indices)
    )
