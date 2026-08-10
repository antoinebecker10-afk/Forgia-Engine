"""Liaison du VRAI maillage au squelette simplifie.

Le probleme resolu : segmenter le GLB par des seuils geometriques (« z entre a et
b, rayon < c ») a echoue trois fois de suite — un barillet tronque, un pontet qui
avalait la crosse, un canon sans sa fourrure. Un seuil est une frontiere plate
appliquee a une forme qui ne l'est pas.

La liaison par PROXIMITE supprime la question. Chaque triangle du vrai maillage
va a la piece simplifiee dont il est le plus proche. Consequences :

* **partition complete** — chaque triangle a exactement une piece, aucun reste,
  aucun recouvrement, sans avoir a l'ecrire ;
* **frontieres epousant les formes** — la limite entre barillet et carcasse est
  la mediatrice entre un cylindre et une boite, pas un plan choisi ;
* **rien a re-regler** — bouger une cote du squelette deplace la frontiere.

C'est le principe des cages de deformation : une geometrie grossiere pilote une
geometrie fine.
"""

from __future__ import annotations

import numpy as np

from glb import Mesh


def _surface_points(mesh: Mesh, density: float) -> np.ndarray:
    """Nuage dense sur la surface d'une primitive, a densite constante.

    Indispensable : une boite n'a que 24 sommets, et « le plus proche sommet »
    n'approxime pas « la plus proche surface » — la classification se retrouve
    pilotee par l'endroit ou tombent les coins. Une premiere version liait sur
    les sommets bruts, et le pontet reclamait toute la crosse.
    """
    a = mesh.positions[mesh.indices[:, 0]]
    b = mesh.positions[mesh.indices[:, 1]]
    c = mesh.positions[mesh.indices[:, 2]]
    area = 0.5 * np.linalg.norm(np.cross(b - a, c - a), axis=1)
    counts = np.maximum(1, (area * density).astype(int))

    rng = np.random.default_rng(0)  # deterministe : meme decoupe a chaque appel
    total = int(counts.sum())
    tri = np.repeat(np.arange(len(counts)), counts)
    u = rng.random(total)
    v = rng.random(total)
    flip = u + v > 1.0
    u[flip], v[flip] = 1.0 - u[flip], 1.0 - v[flip]
    pts = a[tri] + (b[tri] - a[tri]) * u[:, None] + (c[tri] - a[tri]) * v[:, None]
    return np.concatenate([pts, mesh.positions]).astype(np.float32)


def bind(mesh: Mesh, proxy: dict[str, tuple], to_gun: np.ndarray) -> dict[str, np.ndarray]:
    """Attribue chaque triangle du maillage a une piece du squelette.

    `proxy` = {nom: (maillage_simplifie, couleur)} ; `to_gun` amene le vrai
    maillage dans le repere du squelette. Renvoie {nom: masque de triangles}.
    """
    names = [n for n in proxy if not n.endswith("_pupille")]
    samples, owner = [], []
    for slot, name in enumerate(names):
        pts = _surface_points(proxy[name][0], density=180.0)
        samples.append(pts)
        owner.append(np.full(len(pts), slot, np.int32))
    samples = np.concatenate(samples).astype(np.float32)
    owner = np.concatenate(owner)

    centroids = (mesh.positions @ to_gun.T)[mesh.indices].mean(axis=1).astype(np.float32)

    # Par blocs : la matrice complete ferait 30 000 x N flottants.
    best = np.empty(len(centroids), np.int32)
    step = 2048
    for i in range(0, len(centroids), step):
        chunk = centroids[i : i + step]
        d = ((chunk[:, None, :] - samples[None, :, :]) ** 2).sum(-1)
        best[i : i + step] = owner[d.argmin(1)]

    return {name: (best == slot) for slot, name in enumerate(names)}


def report(masks: dict[str, np.ndarray], mesh: Mesh, to_gun: np.ndarray) -> str:
    c = (mesh.positions @ to_gun.T)[mesh.indices].mean(axis=1)
    lines = []
    for name, mask in masks.items():
        if not mask.any():
            lines.append(f"{name:20s} VIDE")
            continue
        q = c[mask]
        lines.append(
            "%-20s n=%5d  z[%+.2f,%+.2f] y[%+.2f,%+.2f]"
            % (name, mask.sum(), q[:, 2].min(), q[:, 2].max(), q[:, 1].min(), q[:, 1].max())
        )
    lines.append(f"total {sum(int(m.sum()) for m in masks.values())} / {len(mesh.indices)}")
    return "\n".join(lines)
