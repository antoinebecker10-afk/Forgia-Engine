"""Aura de rechargement : la main injecte de l'energie dans l'arme.

Pourquoi cette voie plutot qu'un barillet qui bascule : le maillage est une coque
SANS INTERIEUR. Toute piece qui quitte son logement ne decouvre pas un mecanisme,
elle decouvre que l'arme est creuse — mesure faite, un basculement a -50° ouvre
deja un trou beant, et la fourrure qui fait pont entre les pieces se dechire meme
sur une simple rotation.

Une recharge par l'energie ne deplace RIEN. Elle ne peut donc pas dechirer, et
elle colle au personnage : Pepin est une creature vivante dans un monde de forge.

Le rendu se fait en espace PIXEL, apres la reduction. Deux raisons : les teintes
tombent pile sur la palette du sprite, et on peut viser une piece precise —
le barillet et les yeux ont leur masque, rendu avec la meme camera.
"""

from __future__ import annotations

import numpy as np
from PIL import Image

# Braise Forgia. Le degrade va du rouge sombre au blanc chaud : une energie qui
# monte en puissance change de TEINTE, pas seulement de luminosite.
RAMP = (
    (0.00, (122, 40, 20)),
    (0.35, (214, 92, 28)),
    (0.65, (255, 160, 60)),
    (1.00, (255, 236, 190)),
)


def _ramp(t: float) -> tuple[int, int, int]:
    t = max(0.0, min(1.0, t))
    for (t0, c0), (t1, c1) in zip(RAMP, RAMP[1:]):
        if t <= t1:
            k = 0.0 if t1 == t0 else (t - t0) / (t1 - t0)
            return tuple(int(a + (b - a) * k) for a, b in zip(c0, c1))
    return RAMP[-1][1]


def _dilate(mask: np.ndarray, steps: int) -> np.ndarray:
    out = mask.copy()
    for _ in range(steps):
        grown = out.copy()
        grown[1:, :] |= out[:-1, :]
        grown[:-1, :] |= out[1:, :]
        grown[:, 1:] |= out[:, :-1]
        grown[:, :-1] |= out[:, 1:]
        out = grown
    return out


def apply(
    sprite: Image.Image,
    core_mask: np.ndarray | None,
    eye_mask: np.ndarray | None,
    charge: float,
    flow: float,
    halo: float,
) -> Image.Image:
    """Pose l'aura sur un sprite deja reduit.

    `charge` = incandescence du barillet [0..1] · `flow` = progression du flux de
    la main vers l'arme [0..1] · `halo` = liseré autour de la silhouette [0..1].
    """
    rgba = np.array(sprite.convert("RGBA"))
    alpha = rgba[..., 3] > 0
    h, w = alpha.shape

    # 1. Lisere exterieur : l'arme irradie. Opaque, jamais semi-transparent — le
    # moteur affiche le sprite en AlphaMode::Mask, un alpha intermediaire y
    # disparaitrait purement et simplement.
    if halo > 0.02:
        thickness = 1 + int(halo * 2.0)
        ring = _dilate(alpha, thickness) & ~alpha
        rgba[ring, :3] = _ramp(0.25 + 0.5 * halo)
        rgba[ring, 3] = 255

    # 2. Le coeur s'allume — ici le barillet, la piece qu'on rechargerait.
    if core_mask is not None and charge > 0.02:
        core = core_mask & alpha
        if core.any():
            # Plafonne dans la braise au lieu de monter jusqu'au blanc : pousse
            # au bout, la rampe donne une image SUREXPOSEE, pas incandescente.
            tint = np.array(_ramp(0.25 + 0.50 * charge), np.float32)
            k = min(1.0, charge) * 0.72
            base = rgba[core, :3].astype(np.float32)
            rgba[core, :3] = (base * (1 - k) + tint * k).astype(np.uint8)

    # 3. Les yeux s'allument aussi : c'est une creature, elle REAGIT a ce qu'on
    # lui injecte. Sans ça l'aura a l'air posee sur un objet inerte.
    if eye_mask is not None and charge > 0.15:
        eyes = eye_mask & alpha
        if eyes.any():
            tint = np.array(_ramp(0.55 + 0.45 * charge), np.float32)
            k = min(1.0, (charge - 0.15) / 0.85) * 0.9
            base = rgba[eyes, :3].astype(np.float32)
            rgba[eyes, :3] = (base * (1 - k) + tint * k).astype(np.uint8)

    # 4. Le flux : des grains d'energie remontent du poignet vers l'arme. Leurs
    # positions sont DERIVEES de `flow`, jamais tirees au hasard — deux appels
    # doivent rendre la meme image, sinon la frame scintille au reroll.
    if 0.02 < flow < 0.99:
        start = np.array([w * 0.72, h * 0.92])  # poignet, en bas a droite
        end = np.array([w * 0.44, h * 0.36])  # coeur de l'arme
        for i in range(7):
            phase = flow * 1.35 - i * 0.11
            if not (0.0 <= phase <= 1.0):
                continue
            # Arc LEGER : trop ouvert, les grains passent a cote du bras et ne se
            # lisent plus comme un flux qui entre dans l'arme. Trop droit, ça fait
            # tuyau.
            p = start + (end - start) * phase
            p[0] += math_sin(phase) * (5.0 - i * 0.55)
            x, y = int(p[0]), int(p[1])
            size = 2 if phase < 0.75 else 1
            colour = _ramp(0.35 + 0.6 * phase)
            for dy in range(size):
                for dx in range(size):
                    xx, yy = x + dx, y + dy
                    if 0 <= xx < w and 0 <= yy < h:
                        rgba[yy, xx, :3] = colour
                        rgba[yy, xx, 3] = 255

    return Image.fromarray(rgba, "RGBA")


def math_sin(t: float) -> float:
    import math

    return math.sin(t * math.pi)


def mask_from(render: Image.Image, threshold: int = 110) -> np.ndarray:
    """Masque binaire depuis un rendu de piece isolee, deja au format du sprite."""
    return np.array(render.convert("RGBA"))[..., 3] >= threshold
