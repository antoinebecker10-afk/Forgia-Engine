#!/usr/bin/env python3
"""Generate shrunk Cryptes verticales genome (story-667 shrink pass).

Same graph: entree → (terrasse|tunnel|ruines) → cour → pont → chapelle.
Target: room diagonals ≤ ~34 m, world ~112×60 m (was 220×144).
"""
from __future__ import annotations

import math
from pathlib import Path

OUT = Path("assets/genomes/arena_test_crypte_vertical.toml")
MODULE = 2.0
# Demi-diagonale d'une cellule de trame. Une cellule est écartée de la roche dès
# que l'emprise la TOUCHE, pas seulement si son centre est dedans — sinon la
# grille rogne les couloirs dans les diagonales.
CELL_HALF_DIAG = MODULE * 0.70711
ROCK_H = 12.0
MAX_SLOPE_DEG = 25.0

# De combien la roche s'ouvre AU-DELÀ de l'emprise nominale d'une salle.
#
# SOURCE UNIQUE, et c'est l'essentiel : le creusement et le sol lisent la MÊME
# valeur. Tant qu'elle était écrite deux fois — 1.0 en défaut de
# `point_in_room_margin`, l'emprise nue côté plateforme — les deux ont divergé et
# ont produit un anneau de roche creusée sans plancher sur chaque salle en
# hauteur. Une grandeur déclarée deux fois finit toujours par divorcer.
ROOM_CARVE_MARGIN = 1.0

# Métriques joueur et bande de couverture — source unique.
#
# Elles décident du RÔLE d'un bloc autant que du contenu du génome, donc elles
# vivent ici et sont interpolées dans le header émis. L'œil est à 1,70 m et il n'y
# a PAS d'accroupissement : la taxonomie haute/basse/nulle des jeux à couverture
# ne transpose pas. Ce qui compte est la bande où un volume casse la vue sans
# fermer l'espace.
JUMP_HEIGHT_M = 1.174  # au-dessous : on monte dessus, c'est une traversée
EYE_HEIGHT_M = 1.7  # PAS d'accroupissement : un abri sous cette hauteur ne cache rien
COVER_LOW_M = 1.8  # au-dessus de l'œil : commence à casser la ligne de vue
COVER_HIGH_M = 2.8  # au-delà : occultation totale, c'est un mur
# Espacement visé entre deux abris : le compte se DÉRIVE de `aire / espacement²`.
# Bande sourcée 3–10 m, 10 m au maximum (Watch Dogs, Gears of War).
COVER_SPACING_M = 6.0
# Emprise au sol d'un abri : sur le module, assez large pour couvrir un joueur
# (0,6 m) et son pas de côté, assez étroite pour se contourner.
COVER_FOOTPRINT_M = 2.0


def fmt_vec(v) -> str:
    return "[" + ", ".join(f"{x:g}" for x in v) + "]"


def diag(sx: float, sz: float) -> float:
    return math.hypot(sx, sz)


# ── Rooms (center xyz, size xyz, ceiling_m) ─────────────────────────────
# Y in center = floor level. size[1] unused for membership (legacy).
ROOMS = {
    "entree": {
        "role": "spawn",
        "center": (-46.0, 0.0, 0.0),
        "size": (20.0, 12.0, 24.0),
        "ceiling_m": 6.0,
    },
    "terrasse": {
        "role": "combat",
        "center": (-14.0, 4.0, 18.0),
        "size": (26.0, 4.0, 14.0),
        "ceiling_m": 0.0,
    },
    "tunnel": {
        "role": "flank",
        "center": (-16.0, 0.0, 0.0),
        "size": (30.0, 6.0, 10.0),
        "ceiling_m": 4.0,
    },
    "ruines": {
        "role": "reward",
        "center": (-14.0, 0.0, -18.0),
        "size": (18.0, 10.0, 14.0),
        "ceiling_m": 0.0,
    },
    "cour": {
        "role": "combat",
        "center": (14.0, 2.0, 0.0),
        "size": (22.0, 2.0, 22.0),
        "ceiling_m": 0.0,
    },
    "pont": {
        "role": "bridge",
        "center": (32.0, 3.0, 0.0),
        "size": (18.0, 1.0, 12.0),
        "ceiling_m": 0.0,
    },
    "chapelle": {
        "role": "boss",
        "center": (48.0, 4.0, 0.0),
        "size": (22.0, 4.0, 22.0),
        "ceiling_m": 7.0,
    },
}

# ── Chaîne est : cour → pont → chapelle ─────────────────────────────────────
#
# Les emprises sont posées BOUT À BOUT et la rampe occupe l'intervalle. Deux
# salles adjacentes à des hauteurs différentes ne peuvent pas se chevaucher en
# plan : deux sols à deux altitudes sur le même XZ est une contradiction, pas un
# détail. C'est ce chevauchement qui ENTERRAIT les rampes (cour x[3,25] contre
# pont x[23,41], pont contre chapelle x[37,59]) et transformait les transitions
# en marches de 1 m — soit 2,2× le `MaxStepHeight` de 45 cm d'Unreal, et 2,5× la
# borne haute d'Unity (0,1-0,4 m pour un gabarit de 2 m). Sans mantle, ce n'est
# pas une marche : c'est un mur qu'on saute.
#
# La course de rampe est DÉRIVÉE de la dénivelée et de la pente maximale DÉJÀ
# déclarée (`MAX_SLOPE_DEG`), jamais choisie — puis arrondie au module SUPÉRIEUR.
# L'arrondi vers le haut fait deux choses à la fois : il adoucit la pente (jamais
# l'inverse) et il garde les bords d'emprise sur la trame, puisque les largeurs de
# salle sont paires. Une course de 2 m donnerait 26,57°, au-delà de la borne — et
# relever la borne pour faire passer sa géométrie est précisément l'antipattern
# « regagner le vert en abaissant le seuil ».
EAST_CHAIN = ("cour", "pont", "chapelle")
EAST_START_X = 3.0  # bord ouest de la cour — les 3 rampes de l'ouest y atterrissent


def _lay_out_east_chain() -> dict[tuple[str, str], tuple[float, float]]:
    """Pose la chaîne bout à bout. Renvoie {(a, b): (x_pied, x_tete)} par rampe."""
    tan_max = math.tan(math.radians(MAX_SLOPE_DEG))
    gaps: dict[tuple[str, str], tuple[float, float]] = {}
    x = EAST_START_X
    prev = None
    for rid in EAST_CHAIN:
        r = ROOMS[rid]
        if prev is not None:
            rise = abs(r["center"][1] - ROOMS[prev]["center"][1])
            run = math.ceil(rise / tan_max / MODULE) * MODULE
            gaps[(prev, rid)] = (x, x + run)
            x += run
        cx, cy, cz = r["center"]
        half = r["size"][0] * 0.5
        r["center"] = (x + half, cy, cz)  # ← centre DÉRIVÉ, plus déclaré
        x += 2.0 * half
        prev = rid
    return gaps


EAST_GAPS = _lay_out_east_chain()

# L'enceinte suit le contenu, elle ne le contraint pas : poser les emprises bout
# à bout allonge la chaîne est de 6 m de chevauchement supprimé, et la roche
# s'ajuste. Une marge d'un module au-delà du débord le plus large.
_EAST_EDGE = max(r["center"][0] + r["size"][0] * 0.5 for r in ROOMS.values())
_WEST_EDGE = min(r["center"][0] - r["size"][0] * 0.5 for r in ROOMS.values())
_HALF_X = math.ceil((max(_EAST_EDGE, -_WEST_EDGE) + MODULE) / MODULE) * MODULE
SIZE_X = 2.0 * _HALF_X
SIZE_Z = 60.0
SPAWN = (-46.0, 1.5, 0.0)

# ── SPEC DE COMBAT ──────────────────────────────────────────────────────────
#
# Sans elle, aucune mesure de géométrie n'est un verdict : « 76 % de lignes
# moyennes » ne veut rien dire tant qu'on ignore quel combat s'y joue. Elle est
# donc DONNÉE, versionnée et émise dans le génome, pas un commentaire.
#
# Archétypes réels (`assets/genomes/enemies/`, valeurs `default`) :
#   grunt  30 pv · 9,0 m/s · vision 20 m · mêlée 3,0 m   → essaim
#   archer 45 pv · 5,5 m/s · vision 35 m · tir 15 m      → kite
#   elite 120 pv · charge 12,5 m/s · vision 25 m         → charge
ENEMIES = {
    "grunt": {"hp": 30.0, "speed": 9.0, "vision": 20.0, "reach": 3.0, "melee": True},
    "archer": {"hp": 45.0, "speed": 5.5, "vision": 35.0, "reach": 15.0, "melee": False},
    "elite": {"hp": 120.0, "speed": 12.5, "vision": 25.0, "reach": 3.5, "melee": True},
}
# DPS de l'arme de départ (Pépin, `viewmodel_arena.toml`). C'est le diviseur de
# toutes les durées : le temps de descente d'un ennemi fixe la distance qu'il
# parcourt, donc la taille utile de la salle.
PLAYER_DPS = 168.0

ENCOUNTERS = {
    # entrée : aucun combat. On lit les trois routes avant de s'engager.
    "entree": {},
    # terrasse : ouverte et en hauteur → terrain d'archers, qui exploitent
    # leur vision de 35 m et leur repli. Quelques grunts pour interdire le camp.
    "terrasse": {"archer": 3, "grunt": 4},
    # tunnel : couvert, chicane, contact court. Peu de monde — un essaim dans un
    # goulot unique se fauche en file indienne, ce n'est plus un essaim.
    "tunnel": {"grunt": 4},
    # ruines : la branche de récompense DOIT coûter, sinon ce n'est pas un choix.
    # Un elite en est le prix.
    "ruines": {"elite": 1},
    # cour : l'arène de convergence, le gros morceau de la run.
    "cour": {"grunt": 8, "archer": 3},
    # pont : pas de mêlée — le risque du pont, c'est le vide. Deux archers
    # côté chapelle mettent la traversée sous pression.
    "pont": {"archer": 2},
    # chapelle : le boss, plus des adds qui empêchent de le kiter indéfiniment.
    "chapelle": {"grunt": 6},
}


def melee_reach_budget(room_id: str) -> float:
    """Distance MAX à laquelle un engagement peut démarrer pour que la mêlée arrive.

    Le joueur tue en séquence ; pendant qu'il descend la file, les survivants
    avancent. Le dernier d'un groupe de N a donc parcouru `vitesse × N × pv / dps`
    avant que son tour vienne. Au-delà, l'archétype de mêlée ne touche jamais :
    la salle ne l'a pas supprimé par choix, elle l'a supprimé par sa taille.
    """
    best = float("inf")
    for kind, n in ENCOUNTERS.get(room_id, {}).items():
        e = ENEMIES[kind]
        if not e["melee"]:
            continue
        ttk_total = n * e["hp"] / PLAYER_DPS
        best = min(best, e["speed"] * ttk_total + e["reach"])
    return best


def sight_budget(room_id: str) -> float:
    """Plus longue ligne de vue admissible dans la salle.

    Deux contraintes, on garde la plus dure :
    - la mêlée doit pouvoir arriver (`melee_reach_budget`) ;
    - un ennemi ne doit pas être tiré hors de sa propre vision, sinon c'est du
      tir gratuit et il ne réagit même pas.
    """
    vision = min(
        (ENEMIES[k]["vision"] for k in ENCOUNTERS.get(room_id, {})),
        default=float("inf"),
    )
    return min(vision, melee_reach_budget(room_id))


def _east_route(a: str, b: str, rid: str) -> dict:
    """Route de chaîne : trajet et rampe DÉRIVÉS de l'intervalle entre emprises.

    La rampe occupe exactement le vide laissé entre les deux salles, donc elle ne
    peut pas être enterrée dans un socle — c'est l'invariant, pas un contrôle.
    """
    x0, x1 = EAST_GAPS[(a, b)]
    ya, yb = ROOMS[a]["center"][1], ROOMS[b]["center"][1]
    return {
        "id": rid,
        "from": a,
        "to": b,
        "width_m": 5.0,
        "ramp_section": f"rampe_{b}",
        "path": [
            (ROOMS[a]["center"][0], ya, 0.0),
            (x0, ya, 0.0),
            (x1, yb, 0.0),  # rampe
            (ROOMS[b]["center"][0], yb, 0.0),
        ],
        "ramp": ((x0, ya, 0.0), (x1, yb, 0.0), 5.0),
    }

# Routes: list of foot points. Height changes must match a ramp.
ROUTES = [
    {
        "id": "entree_terrasse",
        "from": "entree",
        "to": "terrasse",
        "width_m": 5.0,
        "ramp_section": "rampe_terrasse_entree",
        "path": [
            (-46.0, 0.0, 0.0),
            (-36.0, 0.0, 12.0),
            (-27.0, 4.0, 18.0),  # ramp
            (-14.0, 4.0, 18.0),
        ],
        "ramp": ((-36.0, 0.0, 12.0), (-27.0, 4.0, 18.0), 5.0),
    },
    {
        "id": "entree_tunnel",
        "from": "entree",
        "to": "tunnel",
        "width_m": 5.0,
        "ramp_section": "",
        "path": [
            (-46.0, 0.0, 0.0),
            (-38.0, 0.0, 4.0),
            (-30.0, 0.0, 4.0),
            (-30.0, 0.0, -4.0),  # connecteur axial
            (-18.0, 0.0, -4.0),
            (-16.0, 0.0, -4.0),
            (-16.0, 0.0, 0.0),
        ],
        "ramp": None,
        "covered": True,
    },
    {
        "id": "entree_ruines",
        "from": "entree",
        "to": "ruines",
        "width_m": 5.0,
        "ramp_section": "",
        "path": [
            (-46.0, 0.0, 0.0),
            (-36.0, 0.0, -10.0),
            (-24.0, 0.0, -18.0),
            (-14.0, 0.0, -18.0),
        ],
        "ramp": None,
    },
    {
        "id": "terrasse_cour",
        "from": "terrasse",
        "to": "cour",
        "width_m": 5.0,
        "ramp_section": "rampe_terrasse_cour",
        "path": [
            (-14.0, 4.0, 18.0),
            (-4.0, 4.0, 18.0),
            (4.0, 2.0, 10.0),  # ramp
            (10.0, 2.0, 4.0),
            (14.0, 2.0, 0.0),
        ],
        "ramp": ((-4.0, 4.0, 18.0), (4.0, 2.0, 10.0), 5.0),
    },
    {
        "id": "tunnel_cour",
        "from": "tunnel",
        "to": "cour",
        "width_m": 5.0,
        "ramp_section": "rampe_tunnel_cour",
        "path": [
            (-16.0, 0.0, 0.0),
            (-8.0, 0.0, 0.0),
            (-2.0, 0.0, 0.0),
            (4.0, 2.0, 0.0),  # ramp
            (14.0, 2.0, 0.0),
        ],
        "ramp": ((-2.0, 0.0, 0.0), (4.0, 2.0, 0.0), 5.0),
        "covered": True,
    },
    {
        "id": "ruines_cour",
        "from": "ruines",
        "to": "cour",
        "width_m": 5.0,
        "ramp_section": "rampe_ruines_cour",
        "path": [
            (-14.0, 0.0, -18.0),
            (-8.0, 0.0, -16.0),
            (4.0, 2.0, -10.0),  # ramp
            (10.0, 2.0, -6.0),
            (14.0, 2.0, -4.0),
        ],
        "ramp": ((-8.0, 0.0, -16.0), (4.0, 2.0, -10.0), 5.0),
    },
    _east_route("cour", "pont", "cour_pont"),
    _east_route("pont", "chapelle", "pont_chapelle"),
]


def room_contains(room_id: str, x: float, z: float, eps: float = 0.05) -> bool:
    r = ROOMS[room_id]
    cx, _, cz = r["center"]
    sx, _, sz = r["size"]
    return abs(x - cx) <= sx * 0.5 + eps and abs(z - cz) <= sz * 0.5 + eps


def slope_deg(a, b) -> float:
    dx, dy, dz = b[0] - a[0], b[1] - a[1], b[2] - a[2]
    run = math.hypot(dx, dz)
    if run < 1e-6:
        return 90.0
    return abs(math.degrees(math.atan2(dy, run)))


def validate() -> None:
    for rid, r in ROOMS.items():
        d = diag(r["size"][0], r["size"][2])
        print(f"  {rid:10s} diag={d:5.1f}m  size={r['size'][0]:g}x{r['size'][2]:g}")
        assert d <= 34.5, f"{rid} diag {d} too large"
    assert room_contains("entree", SPAWN[0], SPAWN[2])
    for route in ROUTES:
        p0, p1 = route["path"][0], route["path"][-1]
        assert room_contains(route["from"], p0[0], p0[2]), route["id"]
        assert room_contains(route["to"], p1[0], p1[2]), route["id"]
        if route["ramp"]:
            a, b, _ = route["ramp"]
            s = slope_deg(a, b)
            assert s <= MAX_SLOPE_DEG + 0.1, f"{route['id']} slope {s}"
            # path must include the ramp endpoints as consecutive height change
            found = False
            for u, v in zip(route["path"], route["path"][1:]):
                if abs(u[1] - v[1]) > 0.25:
                    assert (u, v) == (a, b) or (u, v) == (b, a) or (
                        math.dist(u, a) < 0.35 and math.dist(v, b) < 0.35
                    ) or (math.dist(u, b) < 0.35 and math.dist(v, a) < 0.35)
                    found = True
            assert found, route["id"]
        half_x, half_z = SIZE_X / 2, SIZE_Z / 2
        for p in route["path"]:
            assert abs(p[0]) <= half_x and abs(p[2]) <= half_z, route["id"]


def arrivals() -> list[dict]:
    """Points d'ARRIVÉE des ennemis — dérivés, et c'est eux qui décident du combat.

    Erreur qu'on vient d'éviter : croire que la plus longue ligne de la salle fixe
    la distance d'engagement. Elle ne la fixe pas — un couloir déclaré de 5 m qui
    traverse une salle de 22 m EST une ligne de 24 m, par construction, et aucun
    champ d'abris ne la cassera. Ce qui fixe l'engagement, c'est **où les ennemis
    apparaissent**.

    Règles appliquées (map-design-intention §2.4) :
    - au MILIEU DES CÔTÉS de la salle, ce qui borne la distance au centre à la
      demi-largeur — bien en deçà du budget de mêlée, donc l'essaim arrive ;
    - **jamais dans un couloir de route** : une arrivée sur le chemin du joueur
      déclenche le combat au seuil, avant qu'il ait lu la salle ;
    - la mêlée au plus près, le tir sur les côtés longs pour garder sa distance.
    """
    out = []
    for rid, roster in ENCOUNTERS.items():
        if not roster:
            continue
        r = ROOMS[rid]
        cx, fy, cz = r["center"]
        sx, _, sz = r["size"]
        marge = COVER_FOOTPRINT_M
        cotes = [
            (cx - sx * 0.5 + marge, cz),
            (cx + sx * 0.5 - marge, cz),
            (cx, cz - sz * 0.5 + marge),
            (cx, cz + sz * 0.5 - marge),
        ]
        libres = [(x, z) for x, z in cotes if not near_route(x, z, marge)]
        if not libres:  # salle entièrement traversée : on retombe sur les coins
            libres = [(cx - sx * 0.25, cz - sz * 0.25), (cx + sx * 0.25, cz + sz * 0.25)]
        for i, (kind, n) in enumerate(sorted(roster.items())):
            x, z = libres[i % len(libres)]
            out.append({"room": rid, "kind": kind, "count": n, "pos": (x, fy, z)})
    return out


def _neighbour_floors(room_id: str) -> list[float]:
    """Niveaux de sol des salles reliées par une route — celles qui dominent."""
    out = []
    for r in ROUTES:
        other = r["to"] if r["from"] == room_id else r["from"] if r["to"] == room_id else None
        if other:
            out.append(ROOMS[other]["center"][1])
    return out


def derived_covers() -> list[dict]:
    """Champ de couverture DÉRIVÉ : compte, hauteur et écartement descendent des
    métriques et de la spec de combat, jamais du goût.

    - **Compte** : `aire / espacement²`, espacement sourcé 3–10 m (Watch Dogs,
      Gears of War). 13 abris pour la cour, pas « quelques-uns ».
    - **Hauteur** : inégalité de crête `c ≥ h/2 + œil` contre la salle voisine la
      plus haute. Un abri qui ne conteste pas le surplomb ne sert à rien, un abri
      plus haut que la bande est un mur.
    - **Placement** : quinconce, pas grille — une grille laisse les diagonales
      ouvertes, or c'est justement la plus longue ligne qu'il faut casser.
    - **Exclusions** : couloirs de route et emprises de rampe, pour que le contrat
      de largeur tienne PAR CONSTRUCTION et non par correction après coup ; et une
      marge au bord pour ne pas coller un abri contre un mur ou une entrée.
    """
    blocks = []
    demi = COVER_FOOTPRINT_M * 0.5
    for rid, r in ROOMS.items():
        if r["role"] not in ("combat", "boss"):
            continue
        cx, fy, cz = r["center"]
        sx, _, sz = r["size"]
        surplomb = max((h - fy for h in _neighbour_floors(rid)), default=0.0)
        hauteur = min(COVER_HIGH_M, max(COVER_LOW_M, surplomb * 0.5 + EYE_HEIGHT_M))

        # Une emprise d'écart au mur : assez pour qu'on contourne l'abri, pas plus.
        # (Une demi-portée d'espacement laissait 3 m de vide au bord et vidait la
        # salle de ses abris avant même l'exclusion des routes.)
        marge = COVER_FOOTPRINT_M
        nx = max(1, int((sx - 2 * marge) // COVER_SPACING_M) + 1)
        nz = max(1, int((sz - 2 * marge) // COVER_SPACING_M) + 1)
        for k in range(nz):
            z = cz - (nz - 1) * COVER_SPACING_M * 0.5 + k * COVER_SPACING_M
            # Quinconce : une rangée sur deux décalée d'un demi-pas.
            decal = (COVER_SPACING_M * 0.5) if k % 2 else 0.0
            for i in range(nx):
                x = cx - (nx - 1) * COVER_SPACING_M * 0.5 + i * COVER_SPACING_M + decal
                if abs(x - cx) > sx * 0.5 - marge:
                    continue
                # `near_route` ajoute DÉJÀ la demi-largeur déclarée : passer la
                # demi-emprise de l'abri suffit à ne pas mordre dans le couloir.
                # En rajouter dégageait 4,5 m au lieu de 3,5 et vidait les salles.
                if near_route(x, z, demi):
                    continue
                if any(
                    x0 - demi <= x <= x1 + demi and z0 - demi <= z <= z1 + demi
                    for x0, x1, z0, z1 in _ramp_footprints()
                ):
                    continue
                blocks.append(
                    {
                        "pos": (x, fy, z),
                        "size": (COVER_FOOTPRINT_M, hauteur, COVER_FOOTPRINT_M),
                        "role": "cover",
                        "section": f"couverture_{rid}",
                    }
                )
    return blocks


def _ramp_footprints() -> list[Rect]:
    """Emprise au sol de chaque rampe, à sa largeur déclarée."""
    out: list[Rect] = []
    for route in ROUTES:
        if not route["ramp"]:
            continue
        a, b, w = route["ramp"]
        x0, x1 = sorted((a[0], b[0]))
        z0, z1 = sorted((a[2], b[2]))
        # Élargir de la demi-largeur sur l'axe le plus court : une rampe est un
        # ruban, son emprise n'est pas le carré de ses extrémités.
        if (x1 - x0) >= (z1 - z0):
            z0, z1 = z0 - w * 0.5, z1 + w * 0.5
        else:
            x0, x1 = x0 - w * 0.5, x1 + w * 0.5
        out.append((x0, x1, z0, z1))
    return out


def point_in_room_margin(x: float, z: float, margin: float = ROOM_CARVE_MARGIN) -> bool:
    for r in ROOMS.values():
        cx, _, cz = r["center"]
        sx, _, sz = r["size"]
        if abs(x - cx) <= sx * 0.5 + margin and abs(z - cz) <= sz * 0.5 + margin:
            return True
    return False


Rect = tuple[float, float, float, float]  # (x0, x1, z0, z1)


def rect_minus(rect: Rect, holes: list[Rect]) -> list[Rect]:
    """`rect` privé de chaque trou, découpé en rectangles axiaux disjoints."""
    parts = [rect]
    for hole in holes:
        parts = [p for r in parts for p in _rect_split(r, hole)]
    return parts


def _rect_split(r: Rect, h: Rect) -> list[Rect]:
    x0, x1, z0, z1 = r
    hx0, hx1, hz0, hz1 = h
    if hx1 <= x0 or hx0 >= x1 or hz1 <= z0 or hz0 >= z1:
        return [r]  # disjoints
    out = []
    if hx0 > x0:
        out.append((x0, hx0, z0, z1))
    if hx1 < x1:
        out.append((hx1, x1, z0, z1))
    mx0, mx1 = max(x0, hx0), min(x1, hx1)
    if hz0 > z0:
        out.append((mx0, mx1, z0, hz0))
    if hz1 < z1:
        out.append((mx0, mx1, hz1, z1))
    return out


def dist_to_segment(px, pz, ax, az, bx, bz) -> float:
    abx, abz = bx - ax, bz - az
    apx, apz = px - ax, pz - az
    ab2 = abx * abx + abz * abz
    if ab2 < 1e-9:
        return math.hypot(apx, apz)
    t = max(0.0, min(1.0, (apx * abx + apz * abz) / ab2))
    return math.hypot(px - (ax + t * abx), pz - (az + t * abz))


def near_route(x: float, z: float, half_w: float) -> bool:
    for route in ROUTES:
        path = route["path"]
        w = route["width_m"] * 0.5 + half_w
        for a, b in zip(path, path[1:]):
            if dist_to_segment(x, z, a[0], a[2], b[0], b[2]) <= w:
                return True
    return False


def greedy_merge(cells: set[tuple[int, int]]) -> list[tuple[float, float, float, float]]:
    """Merge grid cells into axis-aligned rects. Returns (cx,cz,sx,sz) world."""
    remaining = set(cells)
    rects = []
    while remaining:
        i0, k0 = min(remaining)
        # grow X
        i1 = i0
        while (i1 + 1, k0) in remaining:
            i1 += 1
        # grow Z while full width available
        k1 = k0
        while True:
            nxt = k1 + 1
            if all((i, nxt) in remaining for i in range(i0, i1 + 1)):
                k1 = nxt
            else:
                break
        for i in range(i0, i1 + 1):
            for k in range(k0, k1 + 1):
                remaining.discard((i, k))
        # cell (i,k) covers [i*M, (i+1)*M) — center and size
        x0, x1 = i0 * MODULE, (i1 + 1) * MODULE
        z0, z1 = k0 * MODULE, (k1 + 1) * MODULE
        rects.append(((x0 + x1) * 0.5, (z0 + z1) * 0.5, x1 - x0, z1 - z0))
    return rects


def carve_rock() -> list[dict]:
    half_x, half_z = SIZE_X / 2, SIZE_Z / 2
    i_min = int(math.floor((-half_x) / MODULE))
    i_max = int(math.ceil(half_x / MODULE)) - 1
    k_min = int(math.floor((-half_z) / MODULE))
    k_max = int(math.ceil(half_z / MODULE)) - 1
    rock_cells = set()
    for i in range(i_min, i_max + 1):
        for k in range(k_min, k_max + 1):
            cx = (i + 0.5) * MODULE
            cz = (k + 0.5) * MODULE
            # Côté ROUTE, la dilatation vaut la DEMI-DIAGONALE de cellule, pas
            # le demi-côté : à 1,0 m sur une trame de 2 m, une cellule dont le
            # centre tombe entre 1,0 et 1,414 m du couloir reste en roche, et son
            # coin mord jusqu'à 0,41 m DANS les 5 m déclarés. Le couloir se
            # retrouve pincé dans les diagonales — c'était l'origine des
            # 8 obstructions `roche` du contrôle de dégagement.
            #
            # Côté SALLE, on garde la marge de design (1,0 m par défaut) : elle
            # est voulue, et l'élargir creuserait d'autant les bords de
            # plateforme sans garde-corps des salles en hauteur. Un coin de roche
            # qui dépasse de 0,41 m dans une salle ne gêne aucune route (elles
            # visent les centres) et masque plutôt la tranche de la plateforme.
            if point_in_room_margin(cx, cz) or near_route(cx, cz, CELL_HALF_DIAG):
                continue
            rock_cells.add((i, k))
    blocks = []
    for cx, cz, sx, sz in greedy_merge(rock_cells):
        # PAS de snap() ici : le centre sorti de la fusion est DÉJÀ exact sur la
        # trame, et snap() le déplaçait de 1 m une fois sur deux.
        #
        # Une bande de n cellules à partir de i0 a pour centre 2·i0 + n. Si n est
        # impair, ce centre est impair, et snap() l'arrondit vers l'entier pair
        # voisin (`round(1.5) == 2` en Python) → décalage de 1 m. Ça frappait
        # toute bande de 2, 6, 10, 14 m… soit une large part de la roche : elle
        # se retrouvait 1 m à côté, bouchant un couloir ici et laissant un trou
        # là. Les pavés `size = [2, 12, 2]` de la carte étaient tous à une
        # position paire, alors qu'une cellule seule tombe forcément sur un
        # centre impair — la preuve du décalage.
        blocks.append(
            {
                "pos": (cx, 0.0, cz),
                "size": (sx, ROCK_H, sz),
                "role": "wall",
                "section": "roche",
            }
        )
    return blocks


def corridor_ceilings() -> list[dict]:
    out = []
    for route in ROUTES:
        if not route.get("covered"):
            continue
        path = route["path"]
        w = route["width_m"]
        for a, b in zip(path, path[1:]):
            if abs(a[1] - b[1]) > 0.25:
                continue  # skip ramp span — open or separate
            mx = (a[0] + b[0]) * 0.5
            mz = (a[2] + b[2]) * 0.5
            length = math.hypot(b[0] - a[0], b[2] - a[2])
            if length < 0.5:
                continue
            yaw = math.degrees(math.atan2(-(b[2] - a[2]), b[0] - a[0]))  # align long on X local? 
            # Block long axis is X in local after yaw around Y: Bevy yaw rotates X toward (cos,0,-sin)
            # Simpler: put size along segment using yaw so local X follows segment.
            yaw = math.degrees(math.atan2(a[2] - b[2], b[0] - a[0]))
            top = max(a[1], b[1]) + 4.0
            out.append(
                {
                    "pos": (mx, top, mz),
                    "size": (length + 0.5, 0.5, w),
                    "yaw_deg": yaw,
                    "role": "ceiling",
                    "section": f"plafond_{route['id']}",
                }
            )
    return out


def lights() -> list[dict]:
    out = []
    # Covered rooms: grid of lights under ceiling
    for rid, r in ROOMS.items():
        if r["ceiling_m"] <= 0:
            continue
        cx, fy, cz = r["center"]
        sx, _, sz = r["size"]
        y = fy + min(r["ceiling_m"] - 0.8, 5.2)
        xs = [-0.3, 0.0, 0.3] if sx >= 18 else [0.0]
        zs = [-0.3, 0.0, 0.3] if sz >= 18 else ([0.0] if sz < 12 else [-0.25, 0.25])
        for fx in xs:
            for fz in zs:
                out.append(
                    {
                        "pos": (cx + fx * sx, y, cz + fz * sz),
                        "intensity": 6000,
                        "range_m": 18,
                        "room": rid,
                    }
                )
    # Covered corridors
    for route in ROUTES:
        if not route.get("covered"):
            continue
        path = route["path"]
        for a, b in zip(path, path[1:]):
            if abs(a[1] - b[1]) > 0.25:
                continue
            mx = (a[0] + b[0]) * 0.5
            mz = (a[2] + b[2]) * 0.5
            my = max(a[1], b[1]) + 3.2
            out.append(
                {
                    "pos": (mx, my, mz),
                    "intensity": 4200,
                    "range_m": 12,
                    "room": route["id"],
                }
            )
    return out


def combat_blocks() -> list[dict]:
    """Floors, raised platforms, covers, piles, boss disc — relative to rooms."""
    # Centres de la chaîne est : DÉRIVÉS, jamais recopiés. Tout prop de ces salles
    # se pose en écart à son centre, sinon il reste en arrière quand la chaîne
    # bouge — c'est exactement comme les covers et piliers ont dérivé de leur
    # intention dans le registre.
    cour_x = ROOMS["cour"]["center"][0]
    pont_x = ROOMS["pont"]["center"][0]
    chap_x = ROOMS["chapelle"]["center"][0]
    blocks = []
    # World floor
    blocks.append(
        {
            "pos": (0.0, -0.5, 0.0),
            "size": (SIZE_X + 4.0, 0.5, SIZE_Z + 4.0),
            "role": "floor",
            "section": "sol",
        }
    )

    # Socles des salles en hauteur — DÉRIVÉS de la forme creusée.
    #
    # `plateforme = (emprise ⊕ marge de creusement) − (socles plus hauts) − (rampes)`
    #
    # 1. ⊕ MARGE : le creusement ouvre `emprise + ROOM_CARVE_MARGIN`, et un sol
    #    taillé sur l'emprise NOMINALE laisse donc un anneau de roche creusée sans
    #    plancher — 82 m² de bords de chute mesurés. Un sol s'arrête au mur, pas
    #    un mètre avant. C'est la seule façon de fermer l'anneau : élargir le
    #    creusement l'aggrave.
    # 2. − SOCLES PLUS HAUTS : chaque socle étant plein depuis y=0, un socle bas
    #    n'apporte rien là où un socle haut remplit déjà ; son volume y serait
    #    noyé, invisible et non marchable (144 m³ + 48 m³ retirés).
    # 3. − RAMPES : c'est le piège du registre nº10 — la première tentative
    #    d'élargir les plateformes avait ENGLOUTI les atterrissages de rampe.
    #    On ne retire que la part d'emprise de rampe située HORS de la salle,
    #    donc uniquement dans l'anneau : le plancher réel n'est jamais entamé, et
    #    la rampe débouche à fleur du sol au lieu de finir sous lui.
    raised = [
        (rid, ROOMS[rid])
        for rid in ("terrasse", "cour", "pont", "chapelle")
        if ROOMS[rid]["center"][1] > 0
    ]
    raised.sort(key=lambda kv: -kv[1]["center"][1])
    deja_rempli: list[tuple[float, float, float, float]] = []
    for rid, r in raised:
        cx, fy, cz = r["center"]
        sx, _, sz = r["size"]
        emprise = (cx - sx * 0.5, cx + sx * 0.5, cz - sz * 0.5, cz + sz * 0.5)
        m = ROOM_CARVE_MARGIN
        creuse = (emprise[0] - m, emprise[1] + m, emprise[2] - m, emprise[3] + m)
        trous = list(deja_rempli)
        for corridor in _ramp_footprints():
            trous.extend(rect_minus(corridor, [emprise]))  # ← anneau seulement
        for x0, x1, z0, z1 in rect_minus(creuse, trous):
            blocks.append(
                {
                    "pos": ((x0 + x1) * 0.5, 0.0, (z0 + z1) * 0.5),
                    "size": (x1 - x0, fy, z1 - z0),
                    "role": "platform",
                    "section": f"plateforme_{rid}",
                }
            )
        deja_rempli.append(emprise)

    # Covers — keep off route centerlines
    covers = [
        # vestibule / entree coins (hors trajets vers terrasse/ruines)
        ((-52.0, 0.0, 10.0), (3.0, 6.0, 3.0), "vestibule"),
        ((-52.0, 0.0, -10.0), (3.0, 6.0, 3.0), "vestibule"),
        # terrasse
        ((-22.0, 4.0, 13.0), (3.5, 3.0, 2.5), "terrasse_haute"),
        ((-8.0, 4.0, 23.0), (3.5, 3.0, 2.5), "terrasse_haute"),
        ((-6.0, 4.0, 13.0), (3.5, 3.0, 2.5), "terrasse_haute"),
        # ruines reward (hors route centre)
        # z=-22.5 et non -22 : `entree_ruines` suit la médiane de la salle
        # (z=-18) et cette cover atteignait z=-20, soit 2.0 m pile de l'axe pour
        # une route de 5 m — tangence, donc couloir rogné. 2.5 m maintenant.
        ((-20.0, 0.0, -22.5), (4.0, 6.0, 4.0), "ruines_basses"),
        ((-8.0, 0.0, -22.0), (4.0, 6.0, 4.0), "ruines_basses"),
        ((-20.0, 0.0, -12.0), (3.5, 5.0, 3.5), "ruines_basses"),
        # reward platform basse (marche)
        ((-14.0, 0.0, -23.0), (8.0, 0.4, 5.0), "recompense_basse"),
        # cour — coins EST seulement (les routes entrent par l'ouest)
        # Décalages RELATIFS au centre de salle : la chaîne est se déplace quand
        # les emprises sont reposées, et un prop en coordonnées absolues resterait
        # sur place. C'est la classe de défaut la plus fréquente du registre.
        ((cour_x + 8.0, 2.0, 9.0), (2.5, 5.0, 2.5), "cour_convergence"),
        ((cour_x + 8.0, 2.0, -9.0), (2.5, 5.0, 2.5), "cour_convergence"),
        ((cour_x + 4.0, 2.0, 9.0), (2.5, 5.0, 2.5), "cour_convergence"),
        ((cour_x + 4.0, 2.0, -9.0), (2.5, 5.0, 2.5), "cour_convergence"),
        # chapelle covers — relatifs au centre de la salle
        # z=±6.25 et non ±7 : à ±7 la cover atteignait z=8.75 et pénétrait de
        # 0.75 m les piliers de colonnade (z=8) — 4 × 2.8 m³. À ±6.25 elle
        # TANGENTE le pilier. On ne s'arrête pas à mi-chemin : un écart de 0.25 m
        # serait une fente où le joueur (0.6 m de large) se coince à moitié.
        ((chap_x - 6.0, 4.0, 6.25), (3.5, 5.0, 3.5), "chapelle_finale"),
        ((chap_x - 6.0, 4.0, -6.25), (3.5, 5.0, 3.5), "chapelle_finale"),
        ((chap_x + 6.0, 4.0, 6.25), (3.5, 5.0, 3.5), "chapelle_finale"),
        ((chap_x + 6.0, 4.0, -6.25), (3.5, 5.0, 3.5), "chapelle_finale"),
    ]
    # LE RÔLE SE DÉRIVE DE LA HAUTEUR — un bloc ne peut plus mentir sur ce qu'il est.
    #
    # Le génome déclare une bande de couverture (`cover_low_m` … `cover_high_m`,
    # soit 1,8–2,8 m) : au-dessous de l'œil (1,70 m) un abri ne cache rien,
    # au-dessus de la bande ce n'est plus un abri mais un mur qu'on ne peut ni
    # survoler du regard ni contourner à vue.
    #
    # Les 16 blocs de cette liste étaient tous étiquetés `cover` alors qu'ils font
    # 3, 5 ou 6 m : **zéro** dans la bande déclarée. La carte annonçait 16
    # couvertures et n'en offrait aucune — donc aucune mécanique de peek nulle
    # part. C'est le pattern « le nom est un contrat » pris en flagrant délit.
    for pos, size, section in covers:
        h = size[1]
        if section == "recompense_basse":
            role = "platform"
        elif h <= JUMP_HEIGHT_M:
            role = "traversal"  # on monte dessus, ce n'est pas un abri
        elif h <= COVER_HIGH_M:
            role = "cover"  # vraie couverture : casse la vue sans fermer l'espace
        else:
            role = "wall"  # occultation totale — c'est un mur, qu'on le dise
        blocks.append({"pos": pos, "size": size, "role": role, "section": section})

    # Chicane tunnel — murs qui FORCENT le zigzag (sinon la salle reste un boyau droit).
    # Trajet axial libre : z=+4 jusqu'à x=-30, descente, z=-4 jusqu'à x=-16.
    #
    # La descente à x=-30 passe ENTRE les deux premiers murs : leur écart doit
    # donc valoir au moins la largeur déclarée de `entree_tunnel` (5 m), sinon la
    # route promet un couloir qu'elle n'offre pas. Les deux premiers murs sont
    # raccourcis de 0.5 m côté passage uniquement — faces extérieures inchangées
    # (dégagement du trajet diagonal d'entrée), et tous deux enjambent toujours
    # z=0, donc l'axe du tunnel reste occulté et le zigzag reste forcé.
    chicane = [
        ((-35.25, 0.0, -0.5), (5.5, 4.0, 4.0)),  # bouche le sud du corridor nord
        # Bord est ramené à x=-21.5 pour venir TANGENTER le mur suivant au lieu de
        # le pénétrer (9 m³ de chevauchement). Les parties visibles des deux faces
        # ne sont pas coplanaires, donc pas de z-fighting. L'axe z=0 reste coupé
        # deux fois (x[-37.5,-32.5] puis x[-27.5,-21.5]).
        ((-24.5, 0.0, 0.5), (6.0, 4.0, 4.0)),  # bouche le nord du corridor sud
        ((-20.0, 0.0, 2.5), (3.0, 4.0, 3.0)),  # empêche de couper au centre
        # Plaqué contre le mur nord du tunnel (z=5), pas en îlot : en z=2.5 sa
        # face sud tombait à 1,0 m de l'axe de `tunnel_cour` (qui sort plein est
        # en z=0), ne laissant que 3,5 m utiles sur les 5 déclarés. Adossé, il
        # bouche la bande nord comme voulu, sans la fente de 1 m derrière lui.
        ((-12.0, 0.0, 3.75), (3.0, 4.0, 2.5)),  # bouche le nord à la sortie tunnel
    ]
    for pos, size in chicane:
        blocks.append(
            {
                "pos": pos,
                "size": size,
                "role": "wall",
                "section": "chicane_tunnel",
            }
        )

    # Bridge piles — décalées en Z pour ne pas boucher l'axe de route
    for x, z in ((pont_x - 4.0, 3.5), (pont_x, -3.5), (pont_x + 4.0, 3.5)):
        blocks.append(
            {
                "pos": (x, 0.0, z),
                "size": (2.0, 3.0, 2.0),
                "role": "wall",
                "section": "piles_pont",
            }
        )

    # Minimal pillars in chapelle / entree (2m module)
    # Paire avant en x=-44 et non -42 : en -42 le pilier nord tombait à 1.92 m de
    # l'axe de `entree_terrasse` (5 m de large → 2.0 m exigés), il rognait le
    # couloir de 8 cm. Les DEUX bougent pour garder la symétrie du hall de spawn ;
    # en -44 le dégagement passe à 3.46 m (nord) et 4.24 m (sud).
    for x, z in [(-50.0, 8.0), (-50.0, -8.0), (-44.0, 10.0), (-44.0, -10.0)]:
        blocks.append(
            {
                "pos": (x, 0.0, z),
                "size": (2.0, 6.0, 2.0),
                "role": "wall",
                "section": "colonnade_entree",
            }
        )
    for x, z in [
        (chap_x - 4.0, 9.0),
        (chap_x - 4.0, -9.0),
        (chap_x + 4.0, 9.0),
        (chap_x + 4.0, -9.0),
    ]:
        blocks.append(
            {
                "pos": (x, 4.0, z),
                "size": (2.0, 7.0, 2.0),
                "role": "wall",
                "section": "colonnade_chapelle",
            }
        )

    return blocks


def emit_block(b: dict) -> str:
    lines = ["[[blocks]]", f"pos = {fmt_vec(b['pos'])}", f"size = {fmt_vec(b['size'])}"]
    if b.get("yaw_deg"):
        lines.append(f"yaw_deg = {b['yaw_deg']:g}")
    lines.append(f"role = \"{b['role']}\"")
    lines.append(f"section = \"{b['section']}\"")
    lines.append("")
    return "\n".join(lines)


def main() -> None:
    print("Validate plan…")
    validate()
    print("Carve rock…")
    rock = carve_rock()
    print(f"  rock blocks: {len(rock)}")
    ceilings = corridor_ceilings()
    combat = combat_blocks()
    couvertures = derived_covers()
    combat.extend(couvertures)
    print(f"  couvertures dérivées: {len(couvertures)}")
    spawns = arrivals()
    print(f"  arrivées ennemies: {len(spawns)}")
    for rid, roster in ENCOUNTERS.items():
        if not roster:
            continue
        r = ROOMS[rid]
        budget = sight_budget(rid)
        pires = [
            math.dist((s["pos"][0], s["pos"][2]), (r["center"][0], r["center"][2]))
            for s in spawns
            if s["room"] == rid
        ]
        pire = max(pires) if pires else 0.0
        etat = "OK" if pire <= budget else "HORS BUDGET"
        n_abris = sum(1 for b in couvertures if b["section"] == f"couverture_{rid}")
        print(
            f"    {rid:10s} {n_abris:2d} abris · arrivée la + loin {pire:5.1f} m "
            f"· budget {budget:5.1f} m  {etat}"
        )
        assert pire <= budget, f"{rid} : arrivée à {pire:.1f} m pour un budget de {budget:.1f} m"
    light_list = lights()
    ramps = []
    for route in ROUTES:
        if route["ramp"]:
            a, b, w = route["ramp"]
            ramps.append({"from": a, "to": b, "width_m": w, "section": route["ramp_section"]})

    # Header + metrics copied from prior file spirit
    lines = []
    lines.append(
        """# Arena Test — « Les Cryptes verticales » v2 SHRINK (story-667, 2026-07-28).
#
# ⚠️ ÉTAPE GREYBOX. On juge la FORME, pas l'art. Primitives grises uniquement.
# Passe shrink : même graphe, emprises ramenées dans la bande combat (~30–34 m).
# Monde 116×60 m (était 220×144). Backup : arena_test_crypte_vertical_oversized.bak.toml
#
# Structure :
#   entrée → (terrasse | tunnel | ruines) → cour → pont → chapelle
#
# Convention : `pos` = centre de l'EMPREINTE AU SOL, boîte de pos.y à pos.y+size[1].

[metrics]
player_height_m = 2
player_radius_m = 0.3
walk_speed_ms = 6.5
sprint_speed_ms = 9.75
jump_velocity_ms = 6.5
gravity_ms2 = 18
air_time_s = 0.722
jump_reach_walk_m = 4.69
jump_reach_sprint_m = 7.04
engagement_close_m = 6.5
engagement_core_m = 26
engagement_sniper_m = 52
"""
    )
    # Ces trois-là décident aussi du RÔLE des blocs (voir combat_blocks) : elles
    # sont donc des constantes Python interpolées ici, pas des littéraux recopiés.
    # Une valeur écrite deux fois finit toujours par divorcer.
    lines.append(f"jump_height_m = {JUMP_HEIGHT_M:g}")
    lines.append(f"eye_height_m = {EYE_HEIGHT_M:g}")
    lines.append("")
    lines.append("[grid]")
    lines.append(f"module_m = {MODULE:g}")
    lines.append(f"cover_low_m = {COVER_LOW_M:g}")
    lines.append(f"cover_high_m = {COVER_HIGH_M:g}")
    lines.append(f"cover_spacing_m = {COVER_SPACING_M:g}")
    lines.append(
        """step_max_m = 0.4
platform_step_m = 1.1
wall_min_m = 1.5
gap_max_m = 4.0
show = true
cell_count = 30
ceiling_overhang_m = 1.5

[palette]
floor = [0.52, 0.52, 0.54]
platform = [0.62, 0.60, 0.56]
cover = [0.38, 0.41, 0.45]
wall = [0.34, 0.34, 0.37]
traversal = [0.85, 0.68, 0.22]
perch = [0.35, 0.62, 0.78]
ceiling = [0.22, 0.22, 0.25]

[arena]
name = "Les Cryptes verticales"
shape = "rectangular"
"""
    )
    lines.append(f"size_x = {SIZE_X:g}")
    lines.append(f"size_z = {SIZE_Z:g}")
    lines.append("extent_m = 0")
    lines.append("wall_height_m = 12")
    lines.append("wall_thickness_m = 1")
    lines.append(f"spawn_pos = {fmt_vec(SPAWN)}")
    lines.append("spawn_yaw_deg = -90")
    lines.append("")
    lines.append("# ── SALLES ──")
    for rid, r in ROOMS.items():
        lines.append("[[rooms]]")
        lines.append(f"id = \"{rid}\"")
        lines.append(f"role = \"{r['role']}\"")
        lines.append(f"center = {fmt_vec(r['center'])}")
        lines.append(f"size = {fmt_vec(r['size'])}")
        lines.append(f"ceiling_m = {r['ceiling_m']:g}")
        lines.append("")

    lines.append("# ── ROUTES ──")
    for route in ROUTES:
        lines.append("[[routes]]")
        lines.append(f"id = \"{route['id']}\"")
        lines.append(f"from = \"{route['from']}\"")
        lines.append(f"to = \"{route['to']}\"")
        lines.append(f"width_m = {route['width_m']:g}")
        if route["ramp_section"]:
            lines.append(f"ramp_section = \"{route['ramp_section']}\"")
        lines.append("path = [")
        for p in route["path"]:
            lines.append(f"  {fmt_vec(p)},")
        lines.append("]")
        lines.append("")

    lines.append(
        """[lighting]
sun_illuminance = 9000
sun_color = [0.94, 0.76, 0.58]
fill_illuminance = 2800
fill_color = [0.44, 0.54, 0.74]
ambient_brightness = 500
ambient_color = [0.55, 0.6, 0.72]

# ── LUMIÈRES ──
"""
    )
    for L in light_list:
        lines.append("[[lights]]")
        lines.append(f"pos = {fmt_vec(L['pos'])}")
        lines.append(f"intensity = {L['intensity']}")
        lines.append(f"range_m = {L['range_m']}")
        lines.append(f"room = \"{L['room']}\"")
        lines.append("")

    lines.append("# ── ROCHE (carving espace négatif) ──\n")
    for b in rock:
        lines.append(emit_block(b))

    lines.append("# ── PLAFONDS DE COULOIR COUVERTS ──\n")
    for b in ceilings:
        lines.append(emit_block(b))

    lines.append("# ── SOL / PLATEFORMES / COUVERTURES / PILES ──\n")
    for b in combat:
        lines.append(emit_block(b))

    lines.append("# ── RAMPES ──\n")
    for ramp in ramps:
        lines.append("[[ramps]]")
        lines.append(f"from = {fmt_vec(ramp['from'])}")
        lines.append(f"to = {fmt_vec(ramp['to'])}")
        lines.append(f"width_m = {ramp['width_m']:g}")
        lines.append(f"section = \"{ramp['section']}\"")
        lines.append("")

    # ── SPEC DE COMBAT ──
    # Émise en DONNÉES : c'est elle qui rend la géométrie falsifiable. Sans elle,
    # « 24,6 m de plus longue ligne » est un nombre, pas un verdict.
    lines.append("# ── SPEC DE COMBAT : arrivées ennemies ──")
    lines.append("# `budget_m` = distance max à laquelle l'engagement peut démarrer")
    lines.append("# pour que la mêlée arrive ET que l'ennemi voie le joueur.\n")
    for s in spawns:
        lines.append("[[spawns]]")
        lines.append(f'room = "{s["room"]}"')
        lines.append(f'kind = "{s["kind"]}"')
        lines.append(f"count = {s['count']}")
        lines.append(f"pos = {fmt_vec(s['pos'])}")
        lines.append(f"budget_m = {sight_budget(s['room']):.1f}")
        lines.append("")

    # Boss disc
    lines.append("# ── DISQUES ──\n")
    lines.append("[[discs]]")
    _chap = ROOMS["chapelle"]["center"]
    lines.append(f"pos = [{_chap[0]:g}, {_chap[1]:g}, {_chap[2]:g}]")
    lines.append("radius_m = 3.5")
    lines.append("height_m = 0.4")
    lines.append("role = \"perch\"")
    lines.append("section = \"socle_boss\"")
    lines.append("")

    OUT.write_text("\n".join(lines), encoding="utf-8")
    print(f"Wrote {OUT} ({OUT.stat().st_size} bytes)")
    print(f"lights={len(light_list)} ceilings={len(ceilings)} combat={len(combat)}")


if __name__ == "__main__":
    main()
