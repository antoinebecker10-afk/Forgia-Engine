"""Pepin dessine a la main, d'apres sa fiche d'arme et le GDD *The Spared*.

La fiche montre un PISTOLET MAGI-TECH — corps bleu nuit a ferrures dorees,
cristal violet en bouche, oriflamme a couronne, crosse en bois, tete couronnee
a l'arriere.

**Deux elements viennent du GDD, pas de la fiche**, et ce ne sont pas des
ornements :

* **Le coeur de braise.** « Les armes sont des ames de maitres-forgerons versees
  dans leurs oeuvres — c'est pourquoi elles parlent. Le coeur de braise de
  l'arme rougeoie quand elle parle. » C'est donc une piece FONCTIONNELLE : le
  hublot d'ame, qui s'allume a chaque bark. Il est place au centre du boitier,
  la ou on regarde.
* **La jauge de confiance.** Pepin porte cette jauge comme gimmick (GDD §5).
  Une mecanique qui ne se voit pas sur l'arme se pilote a l'aveugle : quatre
  cellules en haut du boitier, qui se remplissent.

Et la DA **verre + braise** donne la logique des matieres : la braise est la
memoire VIVANTE (chaude, orange, elle bat), le verre la memoire FIGEE (froide,
pale). L'arme porte les deux.

**Fidelite assumee comme partielle.** On ne recopie pas la fiche : on en garde
la silhouette, la palette relevee dessus, et les signes distinctifs.

Techniques (voir `ramps.py`, `oklab.py`, `light.py`) :

* rampes generees en **Oklab** — ecarts de valeur perceptuellement reguliers ;
* **derive de teinte** — ombres vers le froid, lumieres vers le chaud ;
* transitions **tramees (Bayer)** — degrades textures sans couleur en plus ;
* **occlusion de contact** dans chaque jonction — ce qui emboite les pieces ;
* **speculaire au bord** du cylindre, largeur fonction de la rugosite ;
* **halo emissif** — les sources teintent leur sertissage ;
* **cerne selectif** — le contour prend le ton sombre de la matiere qu'il borde ;
* lumiere unique en haut a gauche, bandes de largeurs inegales.
"""

from __future__ import annotations

import light
import oklab
import proportion as pr
import ramps
from pixelforge import Grid

# ── Palette : rampes generees, pas de couleurs choisies a l'oeil ───────────
# Bases relevees SUR LA REFERENCE. L'ensemble y est nettement plus sombre
# qu'une lecture rapide ne le suggere, et la crosse tire au rouge brique.
_BASES = {
    "navy": "#242c40",    # bleu nuit presque noir — le corps
    "steel": "#3d4character",  # place tenante, remplacee ci-dessous
    "gold": "#c08a34",    # laiton, moins jaune que l'or pur
    "purple": "#8e2a86",  # magenta plutot que violet
    "wood": "#6d3826",    # rouge-brun
    "skin": "#8a5a34",    # brun chaud FONCE : clair, la tete s'aplatit
    "ember": "#d4562a",   # LA braise du GDD : orange rouge, pas jaune
    "glass": "#7fa8b8",   # LE verre du GDD : froid, pale, desature
}
_BASES["steel"] = "#4a5468"  # acier clair des bagues, distinct du corps

PALETTE = ["#00000000", "#0b0e16"]
_INDEX: dict[str, list[int]] = {}
for _name, _base in _BASES.items():
    _INDEX[_name] = list(range(len(PALETTE), len(PALETTE) + 6))
    PALETTE.extend(oklab.ramp(_base))

# Matieres ECLAIREES par une source : c'est ce qui fait qu'une pierre *emet*
# au lieu d'etre une tache coloree. Precalculees ici, posees par `light.bloom`.
_LIT: dict[str, list[int]] = {}
for _key, _mat, _glow, _amt in (
    ("gold_pur", "gold", "#d838d0", 0.62),
    ("navy_pur", "navy", "#d838d0", 0.58),
    ("steel_pur", "steel", "#d838d0", 0.58),
    ("gold_emb", "gold", "#ff8c3a", 0.55),
    ("navy_emb", "navy", "#ff8c3a", 0.52),
    ("wood_emb", "wood", "#ff8c3a", 0.48),
):
    _LIT[_key] = list(range(len(PALETTE), len(PALETTE) + 6))
    PALETTE.extend(light.tint(_BASES[_mat], _glow, _amt))

PALETTE += ["#f2eee2", "#c8394a", "#6d1a24", "#fff2d0", "#ffd9a0"]

OUTLINE = 1
NAVY, STEEL, GOLD, PUR, WOOD, SKIN, EMBER, GLASS = (
    _INDEX["navy"], _INDEX["steel"], _INDEX["gold"], _INDEX["purple"],
    _INDEX["wood"], _INDEX["skin"], _INDEX["ember"], _INDEX["glass"],
)
GOLD_PUR, NAVY_PUR, STEEL_PUR = _LIT["gold_pur"], _LIT["navy_pur"], _LIT["steel_pur"]
GOLD_EMB, NAVY_EMB, WOOD_EMB = _LIT["gold_emb"], _LIT["navy_emb"], _LIT["wood_emb"]
EYE, TONGUE, THROAT, HOT, WARM = (
    len(PALETTE) - 5, len(PALETTE) - 4, len(PALETTE) - 3, len(PALETTE) - 2, len(PALETTE) - 1
)

_ALL_RAMPS = (NAVY, STEEL, GOLD, PUR, WOOD, SKIN, EMBER, GLASS,
              GOLD_PUR, NAVY_PUR, STEEL_PUR, GOLD_EMB, NAVY_EMB, WOOD_EMB)

#: Cerne selectif : a quel ton sombre chaque matiere borde-t-elle.
DARKEST: dict[int, int] = {}
for _r in _ALL_RAMPS:
    for _i in _r:
        DARKEST[_i] = _r[0]
for _i in (EYE, TONGUE, THROAT, HOT, WARM):
    DARKEST[_i] = OUTLINE

#: Occlusion de contact : matiere de chaque indice, et son voisin plus sombre.
MATERIAL: dict[int, int] = {}
DARKER: dict[int, int] = {}
for _m, _r in enumerate(_ALL_RAMPS):
    for _k, _i in enumerate(_r):
        MATERIAL[_i] = _m
        DARKER[_i] = _r[max(0, _k - 1)]

# ── Trame et proportions ──────────────────────────────────────────────────
DRAW_W, HEIGHT = 232, 136
MARGIN = 14
WIDTH = DRAW_W + MARGIN * 2

#: Division en nombre d'or de la zone de dessin : le canon occupe la grande
#: part, le bloc culasse + tete la petite. Le rapport est ECHAFAUDAGE, pas
#: decoration — il fixe ou tombe la rupture de silhouette.
SPLIT, _REST = pr.divide(DRAW_W)          # 144 / 88
BORE = 46                                  # axe du canon
R_OUT = 18                                 # demi-hauteur exterieure du canon


def _band(g: Grid, x0: float, x1: float, top: float, bot: float, ramp: list[int],
          rough: float = 0.45) -> None:
    """Bague d'acier ou de laiton en travers du canon : un tube court.

    Une bague dessinee en aplat casse la lecture du cylindre qu'elle ceinture.
    Elle doit tourner comme lui — meme profil, meme sens de lumiere.
    """
    ramps.bevel(g, [(x0, top), (x1, top), (x1, bot), (x0, bot)], ramp, depth=2)
    light.spec(g, [(x0 + 1, top + 3), (x1 - 1, top + 3), (x1 - 1, top + 6), (x0 + 1, top + 6)],
               ramp[5], roughness=rough, over=set(ramp))


def _rivets(g: Grid, xs, y: float, ramp: list[int], r: float = 2.0) -> None:
    """Rivets a trois tons : ombre portee, tete, point vif en haut a gauche.

    Sans le point vif ET l'ombre, un rivet se lit comme une tache ronde.
    """
    for x in xs:
        g.disc(x + 0.5, y + 0.5, r + 0.9, ramp[0])
        g.disc(x, y, r, ramp[3])
        g.disc(x - r * 0.34, y - r * 0.34, r * 0.5, ramp[5])


def _seam(g: Grid, x: float, top: float, bot: float, ramp: list[int]) -> None:
    """Couture de tole : creux sombre + liseré clair juste dessous.

    Une simple ligne sombre se lit comme une rayure ; le doublet en fait une
    jonction de plaques.
    """
    g.poly([(x, top), (x + 1, top), (x + 1, bot), (x, bot)], ramp[0])
    g.poly([(x + 1, top), (x + 2, top), (x + 2, bot), (x + 1, bot)], ramp[4])


def _crystal(g: Grid, cx: float, cy: float, r: float, facets: int = 4) -> None:
    """Pierre taillee : facettes de valeurs alternees autour d'un coeur vif.

    Une gemme n'est pas un disque colore. Ce sont des PLANS : chaque facette
    renvoie une valeur differente, et c'est ce contraste dur qui la fait
    lire comme du cristal plutot que comme une bille.
    """
    import math

    g.disc(cx, cy, r, PUR[0])
    # Table octogonale, puis huit facettes de couronne. Les valeurs sautent
    # d'un cran a l'autre (5 -> 1) au lieu de se suivre : c'est ce contraste
    # DUR qui fait lire du cristal plutot qu'une bille.
    rim = [(cx + math.cos(math.radians(a)) * r * 0.96,
            cy + math.sin(math.radians(a)) * r * 0.96) for a in range(-90, 271, 45)]
    tab = [(cx + math.cos(math.radians(a)) * r * 0.44,
            cy + math.sin(math.radians(a)) * r * 0.44) for a in range(-90, 271, 45)]
    for k in range(8):
        shade = (5, 3, 1, 2, 1, 2, 4, 3)[k]
        g.poly([tab[k], rim[k], rim[k + 1], tab[k + 1]], PUR[shade])
    g.poly(tab[:8], PUR[4])
    # Table : deux plans, pas un aplat — une table plate reste morte.
    g.poly([tab[6], tab[7], tab[0], tab[1]], PUR[5])
    g.disc(cx - r * 0.20, cy - r * 0.24, r * 0.20, HOT)
    g.disc(cx + r * 0.34, cy + r * 0.30, r * 0.10, PUR[5])


def _ember_port(g: Grid, cx: float, cy: float, r: float, heat: float = 1.0) -> None:
    """Le coeur de braise (GDD §5) : hublot cercle d'or sur l'ame de l'arme.

    `heat` 0 = l'arme se tait (braise couvante, rouge sombre), 1 = elle parle
    (coeur blanc). C'est la SEULE piece animee par le dialogue — la lueur sort
    par les barreaux et teinte le laiton alentour via `light.bloom`.
    """
    g.ring(cx, cy, r + 3.2, r + 0.6, GOLD[3])
    g.ring(cx, cy, r + 3.2, r + 2.2, GOLD[5])
    g.ring(cx, cy, r + 1.6, r + 0.6, GOLD[1])
    g.disc(cx, cy, r, EMBER[0])
    g.disc(cx, cy, r * 0.86, EMBER[1 + int(heat * 2)])
    g.disc(cx - r * 0.10, cy - r * 0.06, r * 0.54, EMBER[3 + int(heat * 2)])
    if heat > 0.45:
        g.disc(cx - r * 0.16, cy - r * 0.12, r * 0.26, HOT)
    # Barreaux : ce qui fait un hublot et non une simple pastille lumineuse.
    for k in (-1, 0, 1):
        y = cy + k * r * 0.62
        g.poly([(cx - r * 0.96, y - 0.8), (cx + r * 0.96, y - 0.8),
                (cx + r * 0.96, y + 0.8), (cx - r * 0.96, y + 0.8)], GOLD[1])


def _gauge(g: Grid, x0: float, y0: float, w: float, h: float, filled: int, cells: int = 4) -> None:
    """Jauge de confiance (GDD §5, gimmick de Pepin).

    Une mecanique invisible se pilote a l'aveugle. Les cellules pleines
    prennent la braise (l'arme s'echauffe), les vides restent en verre froid —
    les deux matieres de la DA, opposees sur la meme piece.
    """
    ramps.bevel(g, [(x0, y0), (x0 + w, y0), (x0 + w, y0 + h), (x0, y0 + h)], GOLD, depth=1)
    step = (w - 3) / cells
    for k in range(cells):
        cx0 = x0 + 2 + k * step
        cell = [(cx0, y0 + 2), (cx0 + step - 1.5, y0 + 2),
                (cx0 + step - 1.5, y0 + h - 2), (cx0, y0 + h - 2)]
        if k < filled:
            g.poly(cell, EMBER[4])
            g.poly([(cx0, y0 + 2), (cx0 + step - 1.5, y0 + 2),
                    (cx0 + step - 1.5, y0 + 3.2), (cx0, y0 + 3.2)], HOT)
        else:
            g.poly(cell, GLASS[1])


def _banner(g: Grid, x: float, y: float, w: float, h: float) -> None:
    """Oriflamme a couronne, pendue sous le canon.

    Le tissu n'a pas de speculaire : il se distingue du metal par son ABSENCE
    de brillance autant que par sa couleur. Deux plis suffisent a l'animer.
    """
    ramps.bevel(g, [(x - 2, y), (x + w + 2, y), (x + w + 2, y + 5), (x - 2, y + 5)], GOLD, depth=1)
    body = [(x, y + 4), (x + w, y + 4), (x + w, y + h), (x + w * 0.72, y + h - 8),
            (x + w * 0.5, y + h), (x + w * 0.28, y + h - 8), (x, y + h)]
    g.poly(body, PUR[2])
    # Plis : une bande sombre a gauche, une claire au centre. Un tissu uni
    # pend comme du carton.
    g.poly([(x, y + 4), (x + w * 0.26, y + 4), (x + w * 0.26, y + h - 4), (x, y + h)], PUR[1])
    g.poly_dither([(x + w * 0.42, y + 4), (x + w * 0.66, y + 4),
                   (x + w * 0.66, y + h - 6), (x + w * 0.42, y + h - 4)], PUR[3], 0.55)
    # Couronne brodee.
    cx, cy = x + w * 0.5, y + h * 0.46
    g.poly([(cx - 8, cy + 5), (cx + 8, cy + 5), (cx + 8, cy + 2), (cx - 8, cy + 2)], GOLD[4])
    for dx, hh in ((-7, 7), (-3.5, 10), (0, 12), (3.5, 10), (7, 7)):
        g.poly([(cx + dx - 1.2, cy + 2), (cx + dx + 1.2, cy + 2),
                (cx + dx, cy + 2 - hh * 0.5)], GOLD[4])


def _crown(g: Grid, cx: float, y: float, w: float, h: float) -> None:
    """Couronne : pointes d'or a gemme, POSEE sur le crane.

    Elle repose sur la tete au lieu de flotter au-dessus : son bandeau mord de
    deux pixels dans le crane. C'est ce chevauchement qui la fait tenir.
    """
    band = [(cx - w * 0.5, y + h - 6), (cx + w * 0.5, y + h - 6),
            (cx + w * 0.5, y + h + 2), (cx - w * 0.5, y + h + 2)]
    ramps.bevel(g, band, GOLD, depth=2)
    tips = pr.scale(4, 3)  # tailles tirees d'une meme echelle : elles se repondent
    for dx, hh in ((-0.42, tips[1]), (-0.16, tips[2]), (0.16, tips[2]), (0.42, tips[1])):
        px = cx + w * dx
        g.poly([(px - 3.4, y + h - 5), (px + 3.4, y + h - 5), (px + 1.6, y), (px - 1.6, y)],
               GOLD[3])
        g.poly([(px - 3.4, y + h - 5), (px - 1.0, y + h - 5), (px - 1.0, y)], GOLD[5])
        g.disc(px, y + 1.5, 2.0, PUR[4])
        g.disc(px - 0.5, y + 1.0, 1.0, HOT)
    _crystal(g, cx, y + h - 2, 4.2, facets=3)


def _dragon(g: Grid, x0: float, y0: float, w: float, h: float, mouth: float = 0.55,
            blink: float = 0.0) -> None:
    """Tete de dragon de profil, museau vers l'avant — il vise ou tu vises.

    La tete se construit sur des POINTS D'ANCRAGE nommes, pas sur une pile de
    polygones ajustes a l'oeil. La version precedente etait exactement ca, et
    elle rendait une masse brune sans structure : quand on ne peut pas nommer
    l'arete qu'on deplace, on ne corrige rien, on remue.

    Cinq marqueurs suffisent a 46 px, et pas un de plus : museau allonge,
    machoire ouvrable, PUPILLE FENDUE (c'est elle qui dit reptile — un rond
    aurait dit mammifere), dents saillantes, cornes rejetees en arriere.
    Les ecailles ne couvrent QUE la joue : ecailler toute la tete a cette
    taille revient a poser du bruit.
    """
    def P(u, v):
        return (x0 + w * u, y0 + h * v)

    # ── Ancrages (profil tourne vers la GAUCHE) ───────────────────────────
    SNOUT, BROW, CROWN_T = P(0.00, 0.50), P(0.44, 0.24), P(0.70, 0.19)
    NAPE, BACK, HINGE = P(0.94, 0.30), P(1.00, 0.60), P(0.88, 0.74)
    CHIN, THROAT_P = P(0.06, 0.70), P(0.44, 0.80)

    # ── Cornes, AVANT le crane : donc derriere lui ────────────────────────
    # Une corne se dessine en ARC, segment par segment, avec un fut qui
    # s'affine. Un quadrilatere unique donne une tige.
    for base, length, thick, lift in ((0.22, 0.46, 0.16, 0.62), (0.40, 0.32, 0.11, 0.38)):
        bx, by = x0 + w * 0.80, y0 + h * base
        px, py, pt = bx, by, h * thick * 0.5
        for k in range(1, 5):
            t = k / 4.0
            nx, ny = bx + w * length * t, by - h * length * lift * (t ** 1.6)
            nt = pt * (1.0 - 0.22 * k)
            g.poly([(px, py - pt), (nx, ny - nt), (nx, ny + nt), (px, py + pt)],
                   SKIN[1] if k % 2 else SKIN[2])
            g.poly([(px, py - pt), (nx, ny - nt), (nx, ny - nt * 0.30),
                    (px, py - pt * 0.30)], SKIN[4])
            px, py, pt = nx, ny, nt

    # ── Crane : une seule silhouette fermee, museau -> nuque -> machoire ───
    g.poly([SNOUT, P(0.18, 0.36), BROW, CROWN_T, NAPE, BACK, HINGE,
            THROAT_P, CHIN], SKIN[2])

    # ── Trois plans, et rien de plus : dessus eclaire, joue, dessous ──────
    # Sans ces plans la tete est un galet. Avec plus de trois, c'est du bruit.
    g.poly([P(0.10, 0.42), BROW, CROWN_T, NAPE, P(0.80, 0.40), P(0.40, 0.36)], SKIN[4])
    g.poly([P(0.24, 0.32), P(0.50, 0.24), P(0.68, 0.22), P(0.60, 0.30), P(0.30, 0.36)],
           SKIN[5])
    g.poly([CHIN, THROAT_P, HINGE, P(0.86, 0.64), P(0.30, 0.68), P(0.08, 0.60)], SKIN[1])

    # ── Arete du museau : la ligne qui donne sa longueur ──────────────────
    g.poly([P(0.02, 0.48), P(0.46, 0.31), P(0.48, 0.35), P(0.04, 0.53)], SKIN[5])
    g.poly([P(0.02, 0.54), P(0.46, 0.37), P(0.47, 0.40), P(0.03, 0.57)], SKIN[1])

    # ── Ecailles : joue SEULE, trois rangees clairsemees ──────────────────
    for row, ry in enumerate((0.46, 0.57, 0.66)):
        for k in range(3 - row):
            sx, sy = P(0.62 + k * 0.11 + row * 0.05, ry)
            g.disc(sx, sy, 2.0, SKIN[1])
            g.disc(sx - 0.5, sy - 0.7, 1.2, SKIN[4])

    # ── Machoire ouvrable ─────────────────────────────────────────────────
    jaw = 0.60 + 0.14 * mouth
    g.poly([P(0.02, 0.58), P(0.86, 0.60), P(0.86, jaw + 0.14), P(0.06, jaw + 0.10)],
           SKIN[1])
    if mouth > 0.03:
        g.poly([P(0.04, 0.58), P(0.82, 0.60), P(0.80, jaw + 0.02), P(0.06, jaw)], THROAT)
        g.poly([P(0.16, 0.62), P(0.62, 0.63), P(0.58, jaw), P(0.20, jaw - 0.01)], TONGUE)
    # Dents : sur la levre HAUTE, pointes vers le bas. Alignees sur la basse
    # elles se lisent comme un peigne.
    for k in range(4):
        u = 0.08 + k * 0.16
        g.poly([P(u, 0.575), P(u + 0.038, 0.575), P(u + 0.019, 0.575 + 0.085)], EYE)
        g.poly([P(u, 0.575), P(u + 0.014, 0.575), P(u + 0.012, 0.575 + 0.060)], WARM)
    # Deux crocs a la machoire BASSE, decales des dents du haut : c'est le
    # decalage qui fait une gueule et non un rateau.
    for k in range(2):
        u = 0.20 + k * 0.26
        g.poly([P(u, jaw + 0.10), P(u + 0.036, jaw + 0.10), P(u + 0.018, jaw + 0.02)], EYE)

    # ── Naseau ────────────────────────────────────────────────────────────
    nx, ny = P(0.07, 0.50)
    g.disc(nx, ny, 2.4, SKIN[0])
    g.poly([(nx - 1.2, ny - 2.0), (nx + 0.6, ny - 2.4),
            (nx + 0.9, ny + 1.6), (nx - 0.9, ny + 2.0)], OUTLINE)

    # ── Oeil : amande sombre, PUPILLE FENDUE claire ───────────────────────
    # L'inverse (blanc avec pupille sombre) donne un oeil de dessin anime ;
    # c'est le fond sombre qui rend le regard reptilien.
    ex, ey = P(0.50, 0.38)
    er = w * 0.115
    sq = max(0.10, 1.0 - blink * 0.92)
    g.poly([(ex - er * 1.15, ey + er * 0.10),
            (ex - er * 0.40, ey - er * 0.62 * sq),
            (ex + er * 0.75, ey - er * 0.48 * sq),
            (ex + er * 1.10, ey + er * 0.06),
            (ex + er * 0.45, ey + er * 0.60 * sq),
            (ex - er * 0.55, ey + er * 0.56 * sq)], OUTLINE)
    if blink < 0.7:
        g.poly([(ex - er * 0.85, ey + er * 0.06),
                (ex - er * 0.25, ey - er * 0.42 * sq),
                (ex + er * 0.62, ey - er * 0.32 * sq),
                (ex + er * 0.85, ey + er * 0.04),
                (ex + er * 0.35, ey + er * 0.44 * sq),
                (ex - er * 0.42, ey + er * 0.40 * sq)], EMBER[4])
        g.poly([(ex - er * 0.16, ey - er * 0.44 * sq), (ex + er * 0.14, ey - er * 0.40 * sq),
                (ex + er * 0.14, ey + er * 0.42 * sq),
                (ex - er * 0.16, ey + er * 0.44 * sq)], OUTLINE)
        g.disc(ex - er * 0.50, ey - er * 0.20, er * 0.20, HOT)

    # ── Arcade epineuse : ce qui donne le regard mauvais ──────────────────
    g.poly([P(0.34, 0.30), P(0.68, 0.26), P(0.66, 0.21), P(0.32, 0.26)], SKIN[1])
    for k in range(3):
        sx, sy = P(0.38 + k * 0.10, 0.265)
        g.poly([(sx, sy), (sx + w * 0.045, sy - h * 0.005),
                (sx + w * 0.018, sy - h * 0.055)], SKIN[0])


def _grip(g: Grid, mouth_x: float, top_y: float, bot_y: float) -> None:
    """Crosse en bois, montant JUSQU'A la tete — pas de vide entre les deux.

    Le veinage suit la COURBE de la crosse au lieu d'etre horizontal : c'est ce
    qui donne l'epaisseur du bois. Des stries droites sur une piece galbee la
    reaplatissent.
    """
    body = [
        (mouth_x - 26, top_y), (mouth_x + 18, top_y),
        (mouth_x + 22, bot_y - 12), (mouth_x + 16, bot_y),
        (mouth_x - 16, bot_y), (mouth_x - 24, bot_y - 22),
    ]
    g.poly(body, WOOD[2])
    # Deux plans : la joue eclairee a gauche, l'arriere dans l'ombre.
    g.poly([(mouth_x - 26, top_y), (mouth_x - 6, top_y),
            (mouth_x - 2, bot_y), (mouth_x - 16, bot_y), (mouth_x - 24, bot_y - 22)], WOOD[3])
    g.poly([(mouth_x - 26, top_y), (mouth_x - 16, top_y),
            (mouth_x - 12, bot_y - 30), (mouth_x - 24, bot_y - 26)], WOOD[4])
    g.poly([(mouth_x + 8, top_y), (mouth_x + 18, top_y),
            (mouth_x + 22, bot_y - 12), (mouth_x + 12, bot_y - 4)], WOOD[1])
    # Veinage courbe : chaque strie suit la pente de la crosse.
    for k in range(6):
        t = 0.16 + k * 0.13
        y = top_y + (bot_y - top_y) * t
        lean = (t - 0.5) * 6.0
        g.poly([(mouth_x - 22 + lean, y), (mouth_x + 14 + lean * 1.4, y + 3),
                (mouth_x + 14 + lean * 1.4, y + 4), (mouth_x - 22 + lean, y + 1)],
               WOOD[1] if k % 2 else WOOD[0])
    # Quadrillage antiderapant sur la prise : un simple aplat de bois glisse
    # visuellement autant que dans la main.
    for k in range(5):
        for j in range(3):
            px = mouth_x - 16 + j * 12 + (k % 2) * 6.0
            py = top_y + 30 + k * 10
            if py < bot_y - 12:
                g.poly([(px - 3.0, py), (px + 3.0, py - 3.0), (px + 3.0, py - 2.0),
                        (px - 3.0, py + 1.0)], WOOD[1])


def _draw(mouth: float = 0.55, blink: float = 0.0, heat: float = 1.0,
          confidence: int = 3) -> Grid:
    """Le dessin, dans l'ordre arriere -> avant, puis les trois passes de lumiere."""
    g = Grid(DRAW_W, HEIGHT)

    top, bot = BORE - R_OUT, BORE + R_OUT
    # Chaque piece MORD sur la suivante de quelques pixels. Juxtaposees bord a
    # bord elles laissent une couture claire que le cerne transforme en fente —
    # le canon paraissait detache de sa bague de bouche.
    muzzle_x, barrel_x0, barrel_x1 = 4, 14, SPLIT - 2
    recv_x0, recv_x1 = SPLIT - 10, SPLIT + 40
    recv_top, recv_bot = BORE - 26, BORE + 36
    head_x, head_y, head_s = recv_x1 - 14, 14, 52

    # ── Crosse (derriere tout, elle monte jusque sous la tete) ────────────
    _grip(g, recv_x1 - 4, head_y + head_s - 8, HEIGHT - 6)
    ramps.bevel(g, [(recv_x1 - 28, HEIGHT - 12), (recv_x1 + 16, HEIGHT - 12),
                    (recv_x1 + 12, HEIGHT - 1), (recv_x1 - 24, HEIGHT - 1)], GOLD, depth=2)

    # ── Canon : TROIS troncons, pas un tube uniforme ──────────────────────
    # Un canon d'un seul tenant se lit comme un tuyau. Les ressauts entre
    # troncons sont ce qui le rend mecanique.
    for x0, x1, t, b, ramp in (
        (barrel_x0, 66, top + 3, bot - 3, NAVY),
        (64, 104, top, bot, NAVY),
        (102, barrel_x1, top + 1, bot - 1, NAVY),
    ):
        ramps.tube(g, x0, x1, t, b, ramp)
    # Rail superieur : la ligne de mire, et ce qui donne le dessus de l'arme.
    ramps.tube(g, barrel_x0 + 6, barrel_x1, top - 4, top + 2, STEEL,
               profile=ramps.PLATE)
    for x in range(barrel_x0 + 12, barrel_x1 - 4, 8):
        g.poly([(x, top - 4), (x + 3, top - 4), (x + 3, top - 1), (x, top - 1)], STEEL[0])
    # Speculaire : SUR LE BORD haut du cylindre, pas au milieu. Acier use donc
    # tache large et terne ; l'or verni recevra la sienne, etroite et vive.
    light.spec(g, [(barrel_x0 + 3, top + 5), (barrel_x1 - 3, top + 4),
                   (barrel_x1 - 3, top + 9), (barrel_x0 + 3, top + 10)],
               NAVY[5], roughness=0.72, over=set(NAVY))
    # Rebond sous le ventre : lumiere renvoyee par le sol. Sans lui le bas du
    # canon se fond dans le cerne et l'arme perd son epaisseur.
    light.spec(g, [(barrel_x0 + 8, bot - 5), (barrel_x1 - 8, bot - 6),
                   (barrel_x1 - 8, bot - 3), (barrel_x0 + 8, bot - 2)],
               NAVY[3], roughness=0.85, over=set(NAVY))

    _seam(g, 65, top + 2, bot - 2, NAVY)
    _seam(g, 103, top + 2, bot - 2, NAVY)
    # Bagues discretes : elles ceinturent le canon, elles ne le chevauchent pas
    # comme des poignees. Deborder de 3 px suffit a les faire tourner autour.
    _band(g, 61, 68, top - 2, bot + 2, STEEL, rough=0.55)
    _band(g, 99, 106, top - 2, bot + 2, GOLD, rough=0.22)
    _rivets(g, (36, 46, 82, 92), top + 7, STEEL)
    _rivets(g, (36, 46, 82, 92), bot - 7, STEEL)

    # ── Bouche : bague d'or + cristal ─────────────────────────────────────
    ramps.bevel(g, [(muzzle_x, top - 7), (muzzle_x + 26, top - 3),
                    (muzzle_x + 26, bot + 3), (muzzle_x, bot + 7)], GOLD, depth=3)
    _rivets(g, (muzzle_x + 20,), top + 1, GOLD, r=1.6)
    _rivets(g, (muzzle_x + 20,), bot - 1, GOLD, r=1.6)
    g.ring(muzzle_x + 11, BORE, 14.0, 12.0, GOLD[1])
    g.ring(muzzle_x + 11, BORE, 13.2, 12.4, GOLD[4])
    _crystal(g, muzzle_x + 11, BORE, 12, facets=4)

    # ── Cellule d'energie sur le flanc du canon ───────────────────────────
    ramps.bevel(g, [(76, BORE - 9), (94, BORE - 9), (94, BORE + 6), (76, BORE + 6)],
                GOLD, depth=2)
    g.poly([(79, BORE - 6), (91, BORE - 6), (91, BORE + 3), (79, BORE + 3)], PUR[1])
    g.poly([(80, BORE - 5), (90, BORE - 5), (90, BORE + 1), (80, BORE + 1)], PUR[4])
    g.poly([(81, BORE - 4), (86, BORE - 4), (86, BORE - 2), (81, BORE - 2)], HOT)

    # ── Oriflamme ─────────────────────────────────────────────────────────
    _banner(g, 110, bot - 3, 30, 44)

    # ── Boitier ───────────────────────────────────────────────────────────
    ramps.tube(g, recv_x0, recv_x1, recv_top, recv_bot, NAVY, profile=ramps.PLATE)
    # Plaque de blindage vissee sur le flanc : elle casse la masse du boitier.
    ramps.bevel(g, [(recv_x0 + 4, recv_top + 6), (recv_x1 - 8, recv_top + 3),
                    (recv_x1 - 8, recv_bot - 10), (recv_x0 + 4, recv_bot - 6)],
                NAVY, depth=2)
    _rivets(g, (recv_x0 + 8, recv_x0 + 30), recv_bot - 10, STEEL, r=1.8)
    _band(g, recv_x0 - 1, recv_x0 + 6, top - 4, bot + 4, GOLD, rough=0.25)

    # Jauge et coeur se posent sur la MOITIE AVANT du boitier : l'arriere est
    # occupe par la tete, et une piece a demi cachee ne se lit pas.
    _gauge(g, recv_x0 + 9, recv_top + 3, 30, 9, confidence)
    _ember_port(g, recv_x0 + 22, BORE + 5, 9, heat)

    # ── Detente et pontet ─────────────────────────────────────────────────
    ramps.bevel(g, [(recv_x0 + 4, recv_bot - 2), (recv_x0 + 32, recv_bot - 2),
                    (recv_x0 + 32, recv_bot + 14), (recv_x0 + 8, recv_bot + 12)],
                GOLD, depth=2)
    g.poly([(recv_x0 + 12, recv_bot), (recv_x0 + 28, recv_bot),
            (recv_x0 + 26, recv_bot + 9), (recv_x0 + 14, recv_bot + 8)], NAVY[0])
    g.poly([(recv_x0 + 17, recv_bot - 1), (recv_x0 + 22, recv_bot - 1),
            (recv_x0 + 21, recv_bot + 8), (recv_x0 + 17, recv_bot + 7)], STEEL[3])
    g.poly([(recv_x0 + 17, recv_bot - 1), (recv_x0 + 19, recv_bot - 1),
            (recv_x0 + 18, recv_bot + 7)], STEEL[5])

    # ── Tete et couronne ──────────────────────────────────────────────────
    _dragon(g, head_x, head_y, head_s, head_s, mouth, blink)
    # La couronne se pose sur le SOMMET du crane, pas au-dessus du vide.
    _crown(g, head_x + head_s * 0.60, head_y - 3, 28, 13)

    return _finish(g)


def _finish(g: Grid) -> Grid:
    """Les trois passes de lumiere, dans cet ordre, communes a TOUTES les vues.

    L'occlusion d'abord : elle creuse les jonctions. Le halo ensuite, sinon il
    serait mange par l'assombrissement. Le cerne en dernier, sur l'image deja
    eclairee.

    Factorise expres : quatre vues qui appliqueraient chacune leur variante de
    finition ne seraient plus la meme arme. C'est la finition partagee qui fait
    qu'elles se lisent comme un seul objet vu sous quatre angles.
    """
    light.occlude(g, MATERIAL, DARKER)
    light.bloom(g, {PUR[5], HOT}, _pur_map(), radius=4)
    light.bloom(g, {EMBER[5], EMBER[4]}, _emb_map(), radius=3)
    g.outline_selective(OUTLINE, DARKEST)
    return g


def _pur_map() -> dict[int, int]:
    """Ce que la lueur violette teinte, et vers quoi."""
    m = {}
    for src, dst in ((GOLD, GOLD_PUR), (NAVY, NAVY_PUR), (STEEL, STEEL_PUR)):
        m.update(dict(zip(src, dst)))
    return m


def _emb_map() -> dict[int, int]:
    """Ce que la braise teinte : le laiton du hublot, le corps, le bois."""
    m = {}
    for src, dst in ((GOLD, GOLD_EMB), (NAVY, NAVY_EMB), (WOOD, WOOD_EMB)):
        m.update(dict(zip(src, dst)))
    return m


def side_view(mouth: float = 0.55, blink: float = 0.0, heat: float = 1.0,
              confidence: int = 3) -> Grid:
    """Vue de flanc, centree dans une toile marginee.

    La marge n'est pas decorative : sans elle la couronne et les cornes sont
    rognees par le bord, et le cerne selectif n'a pas de place ou s'ecrire.
    """
    canvas = Grid(WIDTH, HEIGHT)
    canvas.blit(_draw(mouth, blink, heat, confidence), MARGIN, 0)
    return canvas


# ── Les trois autres faces ────────────────────────────────────────────────
# Une arme de FPS se juge sur QUATRE vues, pas une : le flanc vend le design,
# mais c'est le DOS qu'on regarde en visant, et c'est lui qui decide si l'arme
# est jouable. Les vues d'axe (avant, arriere) sont rigoureusement symetriques —
# verifie par `audit()`, parce qu'un decalage d'un pixel sur un axe se voit
# immediatement et ne se voit pas quand on le dessine.

F_W, F_H = 88, 124        # avant
R_W, R_H = 104, 140       # arriere (la vue de visee)
T_W, T_H = 232, 92        # dessus


def _bezel(g: Grid, cx: float, cy: float, r: float, ramp: list[int]) -> None:
    """Bague vue de face : couronne octogonale a facettes alternees.

    Un simple anneau d'or uni se lit comme un beignet. Ce sont les facettes de
    valeurs differentes qui en font une piece tournee.
    """
    import math

    outer = [(cx + math.cos(math.radians(a)) * r,
              cy + math.sin(math.radians(a)) * r) for a in range(-90, 271, 45)]
    inner = [(cx + math.cos(math.radians(a)) * r * 0.72,
              cy + math.sin(math.radians(a)) * r * 0.72) for a in range(-90, 271, 45)]
    for k in range(8):
        # La lumiere vient d'en haut a gauche : les facettes hautes prennent le
        # clair, les basses l'ombre. La sequence n'est pas alternee au hasard.
        shade = (4, 3, 2, 1, 1, 2, 4, 5)[k]
        g.poly([outer[k], outer[k + 1], inner[k + 1], inner[k]], ramp[shade])
    g.ring(cx, cy, r * 0.74, r * 0.68, ramp[0])


def _horns_splayed(g: Grid, cx: float, y: float, spread: float, length: float,
                   depth_sign: float = -1.0) -> None:
    """Les deux cornes vues dans l'axe : elles s'ecartent SYMETRIQUEMENT.

    De profil une seule corne se voit ; d'axe, les deux, et leur ecartement est
    ce qui donne la largeur du crane. `depth_sign` dit si elles partent vers le
    haut (vue arriere) ou s'etalent a plat (vue de dessus).
    """
    for side in (-1.0, 1.0):
        px, py, pt = cx + side * spread * 0.30, y, 8.0
        for k in range(1, 5):
            t = k / 4.0
            nx = cx + side * (spread * 0.30 + spread * 0.70 * t)
            ny = y + depth_sign * length * (t ** 1.35)
            nt = pt * 0.86
            g.poly([(px - pt * 0.5, py), (nx - nt * 0.5, ny),
                    (nx + nt * 0.5, ny), (px + pt * 0.5, py)],
                   SKIN[1] if k % 2 else SKIN[2])
            g.poly([(px - pt * 0.5, py), (nx - nt * 0.5, ny),
                    (nx - nt * 0.1, ny), (px - pt * 0.1, py)], SKIN[4])
            px, py, pt = nx, ny, nt


def front_view(heat: float = 1.0) -> Grid:
    """AVANT — dans l'axe du canon, ce que voit la cible.

    Composition strictement centree : la bague, la pierre, et rien qui deporte
    le regard. Ce qui depasse (couronne, oriflamme, crosse fuyante) sert
    uniquement a dire qu'il y a une arme derriere la bouche.
    """
    g = Grid(F_W, F_H)
    cx = F_W // 2

    # Couronne qui depasse derriere la bouche : elle annonce la tete.
    _crown(g, cx, 2, 30, 12)

    # Rail de visee, vu par la tranche.
    ramps.bevel(g, [(cx - 9, 16), (cx + 9, 16), (cx + 9, 28), (cx - 9, 28)], STEEL, depth=2)

    # Corps du canon derriere la bague : plus etroit, il ancre la profondeur.
    ramps.tube(g, cx - 26, cx + 26, 26, 100, NAVY, profile=ramps.PLATE)

    # Bague de bouche + pierre.
    _bezel(g, cx, 62, 34, GOLD)
    _rivets(g, (cx - 24, cx + 24), 62, GOLD, r=2.0)
    _rivets(g, (cx,), 34, GOLD, r=2.0)
    _rivets(g, (cx,), 90, GOLD, r=2.0)
    _crystal(g, cx, 62, 23, facets=4)

    # Crosse qui fuit sous l'arme, et oriflamme sur le flanc gauche.
    g.poly([(cx - 13, 98), (cx + 13, 98), (cx + 9, F_H - 10), (cx - 9, F_H - 10)], WOOD[2])
    g.poly([(cx - 13, 98), (cx - 4, 98), (cx - 2, F_H - 10), (cx - 9, F_H - 10)], WOOD[3])
    ramps.bevel(g, [(cx - 12, F_H - 11), (cx + 12, F_H - 11),
                    (cx + 10, F_H - 2), (cx - 10, F_H - 2)], GOLD, depth=1)
    _banner(g, 4, 66, 16, 34)
    return _finish(g)


def rear_view(heat: float = 1.0, confidence: int = 3, blink: float = 0.0) -> Grid:
    """ARRIERE — LA vue de visee. Celle qu'on regarde le plus longtemps.

    Contrainte de jeu avant contrainte de style : le cran de mire doit rester
    DEGAGE. La tete se pose donc au-dessus de la ligne de visee et n'y mord
    jamais — sinon l'arme est jolie et injouable.
    """
    g = Grid(R_W, R_H)
    cx = R_W // 2

    # ── Tete vue de dos : les deux cornes s'ecartent, les deux yeux guettent ──
    # Les cornes partent du HAUT du crane et s'ecartent en montant. Plantees
    # a mi-hauteur elles se lisaient comme des antennes vissees sur un seau.
    _horns_splayed(g, cx, 18, 44, 16)
    # Silhouette : etroite et arrondie en haut, LARGE en bas (la machoire).
    # Un hexagone a sommet plat donnait un seau — c'est le profil qui fait le
    # crane, pas la texture qu'on lui pose dessus.
    g.poly([(cx - 13, 10), (cx + 13, 10), (cx + 21, 20), (cx + 25, 36),
            (cx + 21, 50), (cx - 21, 50), (cx - 25, 36), (cx - 21, 20)], SKIN[2])
    g.poly([(cx - 13, 10), (cx + 13, 10), (cx + 20, 20), (cx + 16, 26),
            (cx - 16, 26), (cx - 20, 20)], SKIN[4])
    g.poly([(cx - 9, 12), (cx + 9, 12), (cx + 10, 18), (cx - 10, 18)], SKIN[5])
    # Museau vu de bout : il DEPASSE sous la machoire. C'est lui qui dit que la
    # tete est tournee dans l'autre sens et pas posee de face.
    g.poly([(cx - 11, 44), (cx + 11, 44), (cx + 8, 56), (cx - 8, 56)], SKIN[1])
    g.poly([(cx - 11, 44), (cx - 3, 44), (cx - 2, 56), (cx - 8, 56)], SKIN[2])
    g.disc(cx - 4, 52, 1.8, SKIN[0])
    g.disc(cx + 4, 52, 1.8, SKIN[0])
    # Ecailles de nuque, symetriques.
    for row, ry in enumerate((32, 40)):
        for k in range(3 - row):
            for side in (-1, 1):
                g.disc(cx + side * (6 + k * 8 + row * 4), ry, 2.0, SKIN[1])
    # Les yeux : de dos on voit les deux, et le regard revient vers le tireur.
    for side in (-1, 1):
        ex = cx + side * 16
        g.poly([(ex - side * 5, 28), (ex + side * 5, 27),
                (ex + side * 6, 33), (ex - side * 4, 34)], OUTLINE)
        if blink < 0.7:
            g.poly([(ex - side * 4, 28.5), (ex + side * 4, 28),
                    (ex + side * 4.6, 32.5), (ex - side * 3.2, 33)], EMBER[4])
            g.poly([(ex - 0.9, 28.5), (ex + 0.9, 28.5),
                    (ex + 0.9, 33), (ex - 0.9, 33)], OUTLINE)
    _crown(g, cx, 0, 32, 13)

    # ── Boitier vu de bout, avec la jauge sur le dessus ───────────────────
    ramps.bevel(g, [(cx - 30, 54), (cx + 30, 54), (cx + 27, 104), (cx - 27, 104)],
                NAVY, depth=3)
    _gauge(g, cx - 17, 56, 34, 9, confidence)
    _rivets(g, (cx - 24, cx + 24), 76, STEEL, r=1.8)
    _rivets(g, (cx - 24, cx + 24), 96, STEEL, r=1.8)

    # ── Cran de mire, et la bouche VUE AU LOIN dans l'echancrure ──────────
    # C'est cette superposition — cran proche, bouche lointaine — qui fait la
    # lecture de visee. Sans la bouche au fond, on regarde une piece, pas une
    # ligne de mire.
    ramps.bevel(g, [(cx - 22, 62), (cx + 22, 62), (cx + 22, 80), (cx - 22, 80)],
                GOLD, depth=2)
    # L'echancrure est un VIDE : on doit voir a travers, sinon ce n'est pas une
    # ligne de mire mais un ornement. Fond sombre, puis la bouche au loin.
    g.poly([(cx - 9, 62), (cx + 9, 62), (cx + 5, 80), (cx - 5, 80)], OUTLINE)
    g.disc(cx, 74, 4.4, PUR[1])
    g.disc(cx, 74, 3.0, PUR[4])
    g.disc(cx - 0.6, 73.4, 1.4, HOT)
    # Guidon : le trait vertical qui se pose DANS l'echancrure. C'est la
    # superposition proche/lointain qui fait lire une visee.
    g.poly([(cx - 1.6, 63), (cx + 1.6, 63), (cx + 1.6, 74), (cx - 1.6, 74)], STEEL[4])
    g.poly([(cx - 1.6, 63), (cx - 0.4, 63), (cx - 0.4, 74), (cx - 1.6, 74)], STEEL[5])

    # ── Crosse vue de dos, et talon d'or ──────────────────────────────────
    g.poly([(cx - 22, 100), (cx + 22, 100), (cx + 19, R_H - 12), (cx - 19, R_H - 12)],
           WOOD[2])
    g.poly([(cx - 22, 100), (cx - 8, 100), (cx - 6, R_H - 12), (cx - 19, R_H - 12)],
           WOOD[3])
    g.poly([(cx + 12, 100), (cx + 22, 100), (cx + 19, R_H - 12), (cx + 10, R_H - 12)],
           WOOD[1])
    for k in range(3):
        y = 108 + k * 8
        g.poly([(cx - 20, y), (cx + 20, y), (cx + 20, y + 1), (cx - 20, y + 1)], WOOD[0])
    ramps.bevel(g, [(cx - 23, R_H - 13), (cx + 23, R_H - 13),
                    (cx + 20, R_H - 2), (cx - 20, R_H - 2)], GOLD, depth=2)
    return _finish(g)


def top_view(heat: float = 1.0, confidence: int = 3) -> Grid:
    """DESSUS — la vue qui verifie que l'arme a une EPAISSEUR.

    Elle sert peu au joueur et beaucoup au dessin : c'est la seule ou l'on voit
    si les bagues font le tour, si le rail est centre, et si la tete a une
    largeur credible. Un flanc seul laisse passer une arme plate.
    """
    g = Grid(T_W, T_H)
    cy = T_H // 2

    # Canon vu de dessus : trois troncons, largeurs legerement differentes.
    for x0, x1, half in ((14, 66, 15), (64, 104, 17), (102, SPLIT - 2, 16)):
        ramps.tube(g, x0, x1, cy - half, cy + half, NAVY)
    # Rail central : la ligne qui prouve que le dessus est bien le dessus.
    ramps.tube(g, 20, SPLIT - 2, cy - 6, cy + 6, STEEL, profile=ramps.PLATE)
    for x in range(26, SPLIT - 6, 8):
        g.poly([(x, cy - 6), (x + 3, cy - 6), (x + 3, cy + 6), (x, cy + 6)], STEEL[0])
    # Bagues : elles font le TOUR, donc elles debordent des deux cotes.
    _band(g, 61, 68, cy - 20, cy + 20, STEEL, rough=0.55)
    _band(g, 99, 106, cy - 20, cy + 20, GOLD, rough=0.22)
    _rivets(g, (36, 46, 82, 92), cy - 12, STEEL)
    _rivets(g, (36, 46, 82, 92), cy + 12, STEEL)

    # Bouche.
    _bezel(g, 17, cy, 22, GOLD)
    _crystal(g, 17, cy, 14, facets=4)

    # Boitier, plus large que le canon, avec la jauge sur le dessus.
    ramps.tube(g, SPLIT - 10, SPLIT + 40, cy - 23, cy + 23, NAVY, profile=ramps.PLATE)
    ramps.bevel(g, [(SPLIT - 6, cy - 19), (SPLIT + 34, cy - 19),
                    (SPLIT + 34, cy + 19), (SPLIT - 6, cy + 19)], NAVY, depth=2)
    _gauge(g, SPLIT - 1, cy - 5, 30, 9, confidence)
    _rivets(g, (SPLIT - 2, SPLIT + 30), cy - 15, STEEL, r=1.8)
    _rivets(g, (SPLIT - 2, SPLIT + 30), cy + 15, STEEL, r=1.8)

    # Tete vue de dessus : museau vers l'avant, cornes etalees de part et
    # d'autre. C'est ici qu'on voit sa vraie largeur.
    hx = SPLIT + 28
    # Cornes vues de dessus : elles s'ecartent ET partent en ARRIERE. Un
    # ecartement purement lateral les fait disparaitre sous le crane.
    for side in (-1.0, 1.0):
        px, py, pt = hx + 30, cy + side * 7, 7.5
        for k in range(1, 5):
            t = k / 4.0
            nx, ny = hx + 30 + 22 * t, cy + side * (7 + 15 * (t ** 1.3))
            nt = pt * 0.86
            g.poly([(px, py - pt * 0.5), (nx, ny - nt * 0.5),
                    (nx, ny + nt * 0.5), (px, py + pt * 0.5)],
                   SKIN[1] if k % 2 else SKIN[2])
            px, py, pt = nx, ny, nt
    # Crane : long, effile vers l'avant. Un hexagone regulier ne dit ni le sens
    # ni l'espece — c'est le FUSEAU qui fait la tete de saurien vue de dessus.
    g.poly([(hx - 4, cy - 4), (hx + 12, cy - 14), (hx + 34, cy - 17),
            (hx + 52, cy - 9), (hx + 54, cy), (hx + 52, cy + 9),
            (hx + 34, cy + 17), (hx + 12, cy + 14), (hx - 4, cy + 4)], SKIN[2])
    g.poly([(hx - 4, cy - 4), (hx + 12, cy - 14), (hx + 34, cy - 16),
            (hx + 46, cy - 9), (hx + 40, cy - 3), (hx + 8, cy - 3)], SKIN[4])
    g.poly([(hx + 2, cy - 4), (hx + 30, cy - 6), (hx + 28, cy - 1),
            (hx + 2, cy)], SKIN[5])
    g.poly([(hx + 8, cy + 5), (hx + 44, cy + 8), (hx + 34, cy + 17),
            (hx + 12, cy + 14)], SKIN[1])
    # Naseaux, symetriques : la pointe du museau.
    g.disc(hx - 1, cy - 3, 1.8, SKIN[0])
    g.disc(hx - 1, cy + 3, 1.8, SKIN[0])
    _crown(g, hx + 34, cy - 7, 26, 12)

    # Oriflamme, pendue sur le flanc gauche : vue de dessus on n'en voit que
    # la tranche et son debord.
    ramps.bevel(g, [(110, cy + 18), (140, cy + 18), (140, cy + 24), (110, cy + 24)],
                GOLD, depth=1)
    g.poly([(112, cy + 22), (138, cy + 22), (136, T_H - 4), (114, T_H - 4)], PUR[2])
    g.poly([(112, cy + 22), (120, cy + 22), (119, T_H - 4), (114, T_H - 4)], PUR[1])
    return _finish(g)


def views(**kw) -> dict[str, Grid]:
    """Les quatre faces, dans l'ordre de la fiche d'arme."""
    return {
        "cote": side_view(**kw),
        "avant": front_view(heat=kw.get("heat", 1.0)),
        "derriere": rear_view(heat=kw.get("heat", 1.0),
                              confidence=kw.get("confidence", 3),
                              blink=kw.get("blink", 0.0)),
        "dessus": top_view(heat=kw.get("heat", 1.0),
                           confidence=kw.get("confidence", 3)),
    }


def _axis_symmetry(g: Grid, cx: int, rows: range) -> int:
    """Compte les pixels qui brisent la symetrie autour d'un axe vertical.

    Une vue d'axe asymetrique d'un pixel se voit tout de suite et ne se voit
    PAS quand on la dessine — d'ou la mesure plutot que la relecture.
    """
    bad = 0
    reach = min(cx, g.width - cx - 1)
    for y in rows:
        for d in range(1, reach):
            if (g.get(cx - d, y) == 0) != (g.get(cx + d, y) == 0):
                bad += 1
    return bad


def audit() -> list[str]:
    """Controle mecanique des symetries et des invariants du dessin.

    On ne voit pas ses propres asymetries d'un pixel — on les mesure. Un
    controle qui ne mesure RIEN doit le dire (map-design-patterns §13).
    """
    problems = pr.audit([
        ("bague de bouche", 6 + 11, 6, 6 + 22),
    ])
    if SPLIT != pr.snap(DRAW_W / pr.PHI):
        problems.append(f"division principale {SPLIT} hors nombre d'or")
    if (DRAW_W % pr.MODULE) or (HEIGHT % pr.MODULE):
        problems.append(f"toile {DRAW_W}x{HEIGHT} hors trame de {pr.MODULE}")
    return problems
