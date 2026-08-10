"""Decoupe une planche d'arme en sprites individuels.

Les fiches d'armes de Forgia rassemblent sur une seule image tout ce qu'un
viewmodel demande : la vue heros, les vues orthogonales (cote / avant / DERRIERE
/ dessus), la frame de tir, les expressions du visage, les projectiles.

La case **DERRIERE** est exactement la visee : on regarde dans l'axe du canon, a
travers la mire. Les **EXPRESSIONS** sont deja une suite de frames de visage. Il
n'y a donc rien a redessiner — il y a a DECOUPER.

Methode : le fond des planches est un aplat sombre uniforme. On seuille dessus,
on etiquette les ilots connexes, et chaque ilot est un sprite. Aucune coordonnee
codee en dur : une planche au gabarit different se decoupe pareil.
"""

from __future__ import annotations

import os
from dataclasses import dataclass

import numpy as np
from PIL import Image

#: Un ilot plus petit que ça est du texte, une puce de statistique ou un liseré.
MIN_AREA = 2200
#: Distance au fond au-dela de laquelle un pixel compte comme du contenu.
BG_TOLERANCE = 26
#: Marge laissee autour de chaque decoupe.
PAD = 2


@dataclass
class Piece:
    x: int
    y: int
    w: int
    h: int
    pixels: int

    @property
    def area(self) -> int:
        return self.w * self.h


def background_colour(rgb: np.ndarray) -> np.ndarray:
    """Couleur de fond = la plus frequente sur le POURTOUR de l'image.

    Prendre la plus frequente de toute l'image marcherait aussi la plupart du
    temps, mais une planche dont un panneau occupe la moitie de la surface
    piegerait la mesure. Le pourtour, lui, est du fond par construction.
    """
    border = np.concatenate(
        [rgb[:4].reshape(-1, 3), rgb[-4:].reshape(-1, 3),
         rgb[:, :4].reshape(-1, 3), rgb[:, -4:].reshape(-1, 3)]
    )
    colours, counts = np.unique(border, axis=0, return_counts=True)
    return colours[counts.argmax()]


def _label(mask: np.ndarray) -> tuple[np.ndarray, int]:
    """Etiquetage des composantes connexes (8-voisinage), sans scipy.

    Union-find sur les pixels allumes. Un balayage en deux passes suffirait ;
    celui-ci est plus simple a relire et la taille des planches le permet.
    """
    h, w = mask.shape
    labels = np.zeros((h, w), np.int32)
    parent: list[int] = [0]

    def find(a: int) -> int:
        while parent[a] != a:
            parent[a] = parent[parent[a]]
            a = parent[a]
        return a

    def union(a: int, b: int) -> None:
        ra, rb = find(a), find(b)
        if ra != rb:
            parent[max(ra, rb)] = min(ra, rb)

    nxt = 1
    for y in range(h):
        row = mask[y]
        for x in np.nonzero(row)[0]:
            neighbours = []
            if y > 0:
                for dx in (-1, 0, 1):
                    xx = x + dx
                    if 0 <= xx < w and labels[y - 1, xx]:
                        neighbours.append(labels[y - 1, xx])
            if x > 0 and labels[y, x - 1]:
                neighbours.append(labels[y, x - 1])
            if neighbours:
                lab = min(neighbours)
                labels[y, x] = lab
                for n in neighbours:
                    union(lab, n)
            else:
                labels[y, x] = nxt
                parent.append(nxt)
                nxt += 1

    roots = np.array([find(i) for i in range(nxt)], np.int32)
    labels = roots[labels]
    return labels, nxt


def cut(path: str, min_area: int = MIN_AREA) -> tuple[Image.Image, list[Piece]]:
    img = Image.open(path).convert("RGB")
    rgb = np.asarray(img, np.int16)
    bg = background_colour(rgb).astype(np.int16)

    distance = np.abs(rgb - bg).sum(axis=2)
    mask = distance > BG_TOLERANCE

    labels, count = _label(mask)
    pieces: list[Piece] = []
    for lab in range(1, count):
        ys, xs = np.nonzero(labels == lab)
        if len(xs) == 0:
            continue
        x0, x1 = int(xs.min()), int(xs.max()) + 1
        y0, y1 = int(ys.min()), int(ys.max()) + 1
        piece = Piece(x0, y0, x1 - x0, y1 - y0, len(xs))
        if piece.area >= min_area:
            pieces.append(piece)

    pieces.sort(key=lambda p: (p.y // 40, p.x))
    return img, pieces


def export(path: str, out_dir: str, min_area: int = MIN_AREA) -> list[str]:
    """Ecrit chaque ilot en PNG detoure, nomme par sa position.

    Le nommage reste NEUTRE (`r02_c01`) : c'est a la relecture qu'on decide
    quelle case est la visee et laquelle est une expression. Deviner le role a
    partir d'un rang serait exactement le genre de correspondance implicite qui
    casse a la premiere planche au gabarit different.
    """
    img, pieces = cut(path, min_area)
    rgb = np.asarray(img.convert("RGB"), np.int16)
    bg = background_colour(rgb).astype(np.int16)
    os.makedirs(out_dir, exist_ok=True)

    written = []
    row_of: dict[int, int] = {}
    for piece in pieces:
        band = piece.y // 40
        row_of.setdefault(band, len(row_of))

    counters: dict[int, int] = {}
    for piece in pieces:
        band = row_of[piece.y // 40]
        col = counters.get(band, 0)
        counters[band] = col + 1

        x0 = max(0, piece.x - PAD)
        y0 = max(0, piece.y - PAD)
        x1 = min(img.width, piece.x + piece.w + PAD)
        y1 = min(img.height, piece.y + piece.h + PAD)
        crop = np.asarray(img.crop((x0, y0, x1, y1)).convert("RGB"), np.int16)

        rgba = np.zeros((*crop.shape[:2], 4), np.uint8)
        rgba[..., :3] = crop.astype(np.uint8)
        rgba[..., 3] = (np.abs(crop - bg).sum(axis=2) > BG_TOLERANCE) * 255

        name = os.path.join(out_dir, f"r{band:02d}_c{col:02d}_{piece.w}x{piece.h}.png")
        Image.fromarray(rgba, "RGBA").save(name)
        written.append(name)
    return written


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Decoupe une planche d'arme en sprites")
    parser.add_argument("sheet")
    parser.add_argument("--out", required=True)
    parser.add_argument("--min-area", type=int, default=MIN_AREA)
    args = parser.parse_args()

    files = export(args.sheet, args.out, args.min_area)
    print(f"{len(files)} pieces ecrites dans {args.out}")
    for f in files:
        print("  ", os.path.basename(f))
