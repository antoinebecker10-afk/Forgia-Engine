"""Petite bibliotheque de dessin pixel art deterministe.

On travaille sur une grille d'INDICES de palette (0 = transparent), jamais sur des
couleurs RGBA directement. Consequence : la palette entiere se change apres coup
sans redessiner, et l'anticrenelage ne peut pas s'inviter par accident -- un pixel
appartient a un index, point.

Aucune rotation de PIXELS n'est jamais appliquee : faire tourner une image pixel
art la detruit (bavures, epaisseurs incoherentes). Les formes sont definies par des
polygones dont on tourne les SOMMETS, puis rasterises une seule fois. C'est la
difference entre un sprite incline et un sprite abime.
"""

from __future__ import annotations

import math
from dataclasses import dataclass, field

from PIL import Image

TRANSPARENT = 0


@dataclass
class Grid:
    """Grille d'indices de palette, origine en haut a gauche."""

    width: int
    height: int
    cells: list[int] = field(default_factory=list)

    def __post_init__(self) -> None:
        if not self.cells:
            self.cells = [TRANSPARENT] * (self.width * self.height)

    def get(self, x: int, y: int) -> int:
        if 0 <= x < self.width and 0 <= y < self.height:
            return self.cells[y * self.width + x]
        return TRANSPARENT

    def put(self, x: int, y: int, index: int) -> None:
        if 0 <= x < self.width and 0 <= y < self.height:
            self.cells[y * self.width + x] = index

    def copy(self) -> "Grid":
        return Grid(self.width, self.height, list(self.cells))

    # -- primitives -------------------------------------------------------

    def rect(self, x0: float, y0: float, x1: float, y1: float, index: int) -> None:
        for y in range(int(round(y0)), int(round(y1))):
            for x in range(int(round(x0)), int(round(x1))):
                self.put(x, y, index)

    def poly(self, points: list[tuple[float, float]], index: int) -> None:
        """Remplit un polygone par balayage de lignes (regle pair-impair).

        On echantillonne au CENTRE du pixel (y + 0.5) : sans ca, deux polygones
        qui partagent une arete se marchent dessus ou laissent une couture d'un
        pixel -- les deux se voient immediatement sur un sprite de 128 px.
        """
        if len(points) < 3:
            return
        ys = [p[1] for p in points]
        y_min = max(0, int(math.floor(min(ys))))
        y_max = min(self.height - 1, int(math.ceil(max(ys))))
        for y in range(y_min, y_max + 1):
            sample = y + 0.5
            crossings: list[float] = []
            for i in range(len(points)):
                ax, ay = points[i]
                bx, by = points[(i + 1) % len(points)]
                if ay == by:
                    continue
                if (ay <= sample < by) or (by <= sample < ay):
                    t = (sample - ay) / (by - ay)
                    crossings.append(ax + t * (bx - ax))
            crossings.sort()
            for i in range(0, len(crossings) - 1, 2):
                x_start = int(math.floor(crossings[i] + 0.5))
                x_end = int(math.floor(crossings[i + 1] + 0.5))
                for x in range(x_start, x_end):
                    self.put(x, y, index)

    def poly_dither(self, points, index: int, ratio: float, phase: int = 0) -> None:
        """Remplit un polygone en TRAMAGE ordonne : seuls les pixels retenus par
        la matrice de Bayer sont poses.

        Sert aux transitions entre deux crans d'une rampe. Une frontiere nette
        entre deux tons se lit comme un palier ; entrelacer les deux sur une
        bande etroite donne une transition texturee, sans depenser une couleur de
        plus. C'est la technique des degrades des fiches d'armes.

        Le motif est FIXE (pas aleatoire) : un tirage au sort ferait gresiller
        l'image d'une frame a l'autre.
        """
        from oklab import BAYER4

        if ratio <= 0.0:
            return
        if ratio >= 1.0:
            self.poly(points, index)
            return
        ys = [p[1] for p in points]
        y_min = max(0, int(math.floor(min(ys))))
        y_max = min(self.height - 1, int(math.ceil(max(ys))))
        for y in range(y_min, y_max + 1):
            sample = y + 0.5
            crossings: list[float] = []
            for i in range(len(points)):
                ax, ay = points[i]
                bx, by = points[(i + 1) % len(points)]
                if ay == by:
                    continue
                if (ay <= sample < by) or (by <= sample < ay):
                    t = (sample - ay) / (by - ay)
                    crossings.append(ax + t * (bx - ax))
            crossings.sort()
            for i in range(0, len(crossings) - 1, 2):
                for x in range(
                    int(math.floor(crossings[i] + 0.5)),
                    int(math.floor(crossings[i + 1] + 0.5)),
                ):
                    if BAYER4[(y + phase) % 4][(x + phase) % 4] < ratio:
                        self.put(x, y, index)

    def disc(self, cx: float, cy: float, radius: float, index: int) -> None:
        r2 = radius * radius
        for y in range(int(cy - radius) - 1, int(cy + radius) + 2):
            for x in range(int(cx - radius) - 1, int(cx + radius) + 2):
                dx = x + 0.5 - cx
                dy = y + 0.5 - cy
                if dx * dx + dy * dy <= r2:
                    self.put(x, y, index)

    def ring(self, cx: float, cy: float, outer: float, inner: float, index: int) -> None:
        o2, i2 = outer * outer, inner * inner
        for y in range(int(cy - outer) - 1, int(cy + outer) + 2):
            for x in range(int(cx - outer) - 1, int(cx + outer) + 2):
                dx = x + 0.5 - cx
                dy = y + 0.5 - cy
                d2 = dx * dx + dy * dy
                if i2 <= d2 <= o2:
                    self.put(x, y, index)

    # -- passes globales --------------------------------------------------

    def outline(self, index: int, only_over: set[int] | None = None) -> None:
        """Cerne d'un pixel autour de toute zone opaque (4-voisinage).

        Le cerne s'ecrit dans une copie puis se reporte : dessiner en place ferait
        grossir le trait sur lui-meme et donnerait un contour de 2-3 px irregulier.
        """
        out = self.copy()
        for y in range(self.height):
            for x in range(self.width):
                if self.get(x, y) != TRANSPARENT:
                    continue
                neighbours = (
                    self.get(x - 1, y),
                    self.get(x + 1, y),
                    self.get(x, y - 1),
                    self.get(x, y + 1),
                )
                for n in neighbours:
                    if n == TRANSPARENT:
                        continue
                    if only_over is not None and n not in only_over:
                        continue
                    out.put(x, y, index)
                    break
        self.cells = out.cells

    def outline_selective(self, fallback: int, darkest: dict[int, int]) -> None:
        """Cerne dont la COULEUR depend de la matiere qu'il borde.

        Un cerne noir uniforme aplatit tout et donne l'aspect autocollant : le
        contour se lit comme un trait pose PAR-DESSUS le dessin. En prenant le
        cran le plus sombre de la rampe voisine, le contour appartient a la
        matiere — c'est le *selective outlining* des references.

        `darkest` associe un index de palette a son cran le plus sombre.
        """
        out = self.copy()
        for y in range(self.height):
            for x in range(self.width):
                if self.get(x, y) != TRANSPARENT:
                    continue
                best = None
                for n in (self.get(x - 1, y), self.get(x + 1, y),
                          self.get(x, y - 1), self.get(x, y + 1)):
                    if n == TRANSPARENT:
                        continue
                    tone = darkest.get(n, fallback)
                    # Le ton le plus sombre l'emporte : sur une arete entre deux
                    # matieres, le contour doit rester lisible.
                    if best is None or tone == fallback:
                        best = tone
                if best is not None:
                    out.put(x, y, best)
        self.cells = out.cells

    def speckle(self, points, index: int, seed: int = 0) -> None:
        """Semis DETERMINISTE de pixels — rivets, grain, usure.

        Le motif derive de la position, jamais d'un tirage : deux rendus doivent
        donner la meme image, sinon un sprite anime scintille.
        """
        ys = [p[1] for p in points]
        for y in range(max(0, int(min(ys))), min(self.height, int(max(ys)) + 1)):
            for x in range(self.width):
                if self.get(x, y) == TRANSPARENT:
                    continue
                if ((x * 7 + y * 13 + seed * 31) % 17) == 0:
                    self.put(x, y, index)

    def shade_top_edge(self, target: int, light: int, depth: int = 1) -> None:
        """Eclaire les `depth` premieres lignes de chaque colonne de `target`.

        Lumiere venant du haut : c'est ce qui donne le volume sans dessiner la
        moindre ombre a la main.
        """
        out = self.copy()
        for x in range(self.width):
            run = 0
            for y in range(self.height):
                if self.get(x, y) == target:
                    if run < depth:
                        out.put(x, y, light)
                    run += 1
                else:
                    run = 0
        self.cells = out.cells

    def shade_bottom_edge(self, target: int, dark: int, depth: int = 1) -> None:
        """Assombrit les `depth` dernieres lignes de chaque colonne de `target`."""
        out = self.copy()
        for x in range(self.width):
            column = [y for y in range(self.height) if self.get(x, y) == target]
            if not column:
                continue
            runs: list[list[int]] = []
            current = [column[0]]
            for y in column[1:]:
                if y == current[-1] + 1:
                    current.append(y)
                else:
                    runs.append(current)
                    current = [y]
            runs.append(current)
            for run in runs:
                for y in run[-depth:]:
                    out.put(x, y, dark)
        self.cells = out.cells

    def translated(self, dx: int, dy: int) -> "Grid":
        out = Grid(self.width, self.height)
        for y in range(self.height):
            for x in range(self.width):
                value = self.get(x, y)
                if value != TRANSPARENT:
                    out.put(x + dx, y + dy, value)
        return out

    def over(self, other: "Grid") -> None:
        """Compose `other` par-dessus soi (les pixels transparents ne masquent pas).

        Les deux grilles doivent avoir la MEME largeur : la composition se fait
        par index plat. Pour poser une grille d'une autre taille, voir `blit`.
        """
        if other.width != self.width or other.height != self.height:
            raise ValueError(
                f"over() exige des grilles identiques ({self.width}x{self.height} "
                f"contre {other.width}x{other.height}) — utiliser blit()"
            )
        for i, value in enumerate(other.cells):
            if value != TRANSPARENT:
                self.cells[i] = value

    def blit(self, other: "Grid", dx: int, dy: int) -> None:
        """Pose `other` a l'offset donne, quelles que soient les tailles.

        A utiliser des que les largeurs different : `over` compose par index
        PLAT, donc deux largeurs differentes decalent chaque ligne un peu plus
        que la precedente et l'image part en rubans.
        """
        for y in range(other.height):
            for x in range(other.width):
                value = other.get(x, y)
                if value != TRANSPARENT:
                    self.put(x + dx, y + dy, value)

    # -- export -----------------------------------------------------------

    def to_image(self, palette: list[str], scale: int = 1) -> Image.Image:
        img = Image.new("RGBA", (self.width, self.height), (0, 0, 0, 0))
        px = img.load()
        rgba = [_hex_to_rgba(c) for c in palette]
        for y in range(self.height):
            for x in range(self.width):
                index = self.get(x, y)
                if index != TRANSPARENT:
                    px[x, y] = rgba[index]
        if scale > 1:
            img = img.resize(
                (self.width * scale, self.height * scale), Image.Resampling.NEAREST
            )
        return img


# ── Rendu de boites en perspective ─────────────────────────────────────────
# Un profil deforme ne sait pas montrer une arme vue de DOS : il n'a pas de face
# arriere, pas d'occultation entre pieces, et la mire ne peut pas etre un vrai
# trou. On projette donc de vraies boites 3D, faces cachees comprises.
#
# Repere de l'arme : +x a droite, +y en haut, +z vers l'avant (la ou pointe le
# canon, donc en s'eloignant du joueur). La camera est derriere, en -z.

# (normale, 4 coins en signes) -- l'ordre des coins est sans importance pour un
# remplissage pair-impair, seule la normale sert au tri et a l'eclairage.
_FACES = (
    ((0.0, 0.0, -1.0), ((-1, -1, -1), (1, -1, -1), (1, 1, -1), (-1, 1, -1))),
    ((0.0, 0.0, 1.0), ((1, -1, 1), (-1, -1, 1), (-1, 1, 1), (1, 1, 1))),
    ((-1.0, 0.0, 0.0), ((-1, -1, 1), (-1, -1, -1), (-1, 1, -1), (-1, 1, 1))),
    ((1.0, 0.0, 0.0), ((1, -1, -1), (1, -1, 1), (1, 1, 1), (1, 1, -1))),
    ((0.0, 1.0, 0.0), ((-1, 1, -1), (1, 1, -1), (1, 1, 1), (-1, 1, 1))),
    ((0.0, -1.0, 0.0), ((-1, -1, 1), (1, -1, 1), (1, -1, -1), (-1, -1, -1))),
)


@dataclass
class Box:
    """Boite alignee sur les axes, dans le repere de l'arme.

    `shades` = (sombre, moyen, clair) : la face prend son ton de son orientation
    face a la lumiere, pas d'une couleur peinte a la main. C'est ce qui garde la
    coherence quand une piece bouge d'une frame a l'autre.
    """

    center: tuple[float, float, float]
    size: tuple[float, float, float]
    shades: tuple[int, int, int]
    #: Si defini, toutes les faces prennent cet index (pieces lumineuses).
    glow: int | None = None
    #: Ecarte la boite du tri par profondeur pour la forcer devant/derriere.
    bias: float = 0.0
    #: Inclinaison autour de X, en degres. Une crosse est penchee ; l'empiler en
    #: marches d'escalier se verrait comme un escalier.
    pitch: float = 0.0

    def corner(self, signs: tuple[int, int, int]) -> tuple[float, float, float]:
        hx, hy, hz = (s * 0.5 for s in self.size)
        x, y, z = signs[0] * hx, signs[1] * hy, signs[2] * hz
        if self.pitch:
            a = math.radians(self.pitch)
            ca, sa = math.cos(a), math.sin(a)
            y, z = y * ca - z * sa, y * sa + z * ca
        return (self.center[0] + x, self.center[1] + y, self.center[2] + z)

    def normal(self, n: tuple[float, float, float]) -> tuple[float, float, float]:
        if not self.pitch:
            return n
        a = math.radians(self.pitch)
        ca, sa = math.cos(a), math.sin(a)
        return (n[0], n[1] * ca - n[2] * sa, n[1] * sa + n[2] * ca)


@dataclass
class Camera:
    """Camera orbitale simple, en degres.

    `yaw` = 0 met la camera pile dans l'axe du canon (arme vue plein dos) ;
    l'augmenter fait pivoter l'arme pour decouvrir un flanc.
    """

    yaw: float
    pitch: float
    distance: float
    focal: float
    screen: tuple[float, float]

    def view(self, p: tuple[float, float, float]) -> tuple[float, float, float]:
        yaw = math.radians(self.yaw)
        pitch = math.radians(self.pitch)
        x, y, z = p
        # Lacet autour de Y.
        cy, sy = math.cos(yaw), math.sin(yaw)
        x, z = x * cy + z * sy, -x * sy + z * cy
        # Tangage autour de X.
        cp, sp = math.cos(pitch), math.sin(pitch)
        y, z = y * cp - z * sp, y * sp + z * cp
        return (x, y, z)

    def project(self, p: tuple[float, float, float]) -> tuple[float, float, float]:
        x, y, z = self.view(p)
        depth = z + self.distance
        if depth < 0.05:
            depth = 0.05
        sx, sy = self.screen
        return (sx + self.focal * x / depth, sy - self.focal * y / depth, depth)


def render_boxes(
    grid: Grid,
    boxes: list[Box],
    camera: Camera,
    light: tuple[float, float, float] = (-0.35, 0.86, -0.36),
) -> None:
    """Peint les boites du plus loin au plus proche (algorithme du peintre).

    Le tri se fait sur la profondeur MOYENNE de chaque face. Sur des pieces
    disjointes c'est exact ; c'est le compromis assume plutot qu'un tampon de
    profondeur, qui serait inutile a cette taille.
    """
    faces: list[tuple[float, list[tuple[float, float]], int]] = []
    for box in boxes:
        for raw_normal, corners in _FACES:
            points = []
            depth_sum = 0.0
            for signs in corners:
                px, py, pd = camera.project(box.corner(signs))
                points.append((px, py))
                depth_sum += pd
            mean_depth = depth_sum / 4.0

            # Faces cachees : on garde la face si sa normale regarde la camera.
            normal = box.normal(raw_normal)
            nv = camera.view(normal)
            face_centre = box.corner((0, 0, 0))
            face_centre = (
                face_centre[0] + normal[0],
                face_centre[1] + normal[1],
                face_centre[2] + normal[2],
            )
            centre = camera.view(face_centre)
            to_cam = (-centre[0], -centre[1], -(centre[2] + camera.distance))
            if nv[0] * to_cam[0] + nv[1] * to_cam[1] + nv[2] * to_cam[2] <= 0.0:
                continue

            if box.glow is not None:
                index = box.glow
            else:
                lambert = nv[0] * light[0] + nv[1] * light[1] + nv[2] * light[2]
                dark, mid, bright = box.shades
                index = bright if lambert > 0.45 else mid if lambert > -0.15 else dark
            faces.append((mean_depth - box.bias, points, index))

    for _, points, index in sorted(faces, key=lambda f: -f[0]):
        grid.poly(points, index)


def rotate_points(
    points: list[tuple[float, float]],
    degrees: float,
    pivot: tuple[float, float],
) -> list[tuple[float, float]]:
    """Tourne des SOMMETS (jamais des pixels). Ecran : y vers le bas."""
    rad = math.radians(degrees)
    cos_a, sin_a = math.cos(rad), math.sin(rad)
    cx, cy = pivot
    out = []
    for x, y in points:
        dx, dy = x - cx, y - cy
        out.append((cx + dx * cos_a - dy * sin_a, cy + dx * sin_a + dy * cos_a))
    return out


def _hex_to_rgba(value: str) -> tuple[int, int, int, int]:
    value = value.lstrip("#")
    if len(value) == 6:
        value += "ff"
    return tuple(int(value[i : i + 2], 16) for i in (0, 2, 4, 6))  # type: ignore[return-value]


def contact_sheet(
    images: list[Image.Image], columns: int, background: str = "#1b1b24"
) -> Image.Image:
    """Planche de controle : voir les frames cote a cote est le seul moyen
    d'attraper une incoherence d'animation (une piece qui saute d'un pixel)."""
    if not images:
        raise ValueError("planche vide")
    cell_w = max(i.width for i in images)
    cell_h = max(i.height for i in images)
    rows = (len(images) + columns - 1) // columns
    sheet = Image.new(
        "RGBA", (cell_w * columns, cell_h * rows), _hex_to_rgba(background)
    )
    for i, img in enumerate(images):
        x = (i % columns) * cell_w
        y = (i // columns) * cell_h
        sheet.alpha_composite(img, (x, y))
    return sheet
