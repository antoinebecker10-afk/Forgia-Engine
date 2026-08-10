"""Genere les frames du viewmodel de Pepin, articulees.

Chaine complete :

    GLB reel  ──┐
                ├─ liaison par proximite ─> pieces du vrai maillage
    squelette ──┘        (bind.py)
    simplifie
    (pepin_model)

    puis : pose -> rendu 3D -> reduction pixel art

On garde donc l'ASPECT du vrai modele — fourrure, yeux peints, dents — avec des
articulations EXACTES, puisque les axes sont ceux du squelette et non des
estimations sur un nuage de points.

Historique utile : trois decoupages par seuils geometriques ont echoue avant ça
(barillet tronque, pontet avalant la crosse). Le squelette a supprime la question
au lieu de l'affiner.
"""

from __future__ import annotations

import math
import os

import numpy as np
from PIL import Image

import aura
import bind
import framing
import glb
import pepin_model as pm
import pixelize
import primitives as prim
import render3d

GUN_PATH = "../../assets/models/weapons/forgia/pepin.glb"
ARM_PATH = "../../assets/models/arms/fps_arm_R.glb"
ARM_L_PATH = "../../assets/models/arms/fps_arm_L.glb"
OUT_DIR = "../../assets/textures/weapons/pixel/pepin"

CANVAS = (128, 144)
COLOURS = 20
BIG = (360, 400)
SCENE_OFFSET = (0.30, -0.34, 0.0)
GUN_AT = (68, 50)  # ou le CENTRE de l'arme tombe dans le cadre final
GUN_HEIGHT_PX = 88

TO_GUN = np.array([[0, 0, 1], [0, 1, 0], [-1, 0, 0]], np.float32)

# La crosse fait 0.42 de large : un poing a 1.55 y paraissait celui d'un enfant.
ARM_SCALE = 1.95
ARM_PITCH = 27.6
# x = -0.25 : la main doit etre SUR le flanc visible de la crosse. A +0.03 le
# poing etait entierement enfoui dedans et on ne voyait qu'un avant-bras nu —
# meme piege que pour la main gauche, verifie par rendu.
# Position trouvee par rendu croise sous DEUX angles — l'angle du viewmodel seul
# ment : une main peut y sembler tenir l'arme tout en flottant a cote d'elle en
# 3D. Le controle a 3/4 l'a montre.
#   x = -0.17 : sur le flanc VISIBLE de la crosse, ni enfoui ni detache
#   y = -0.12 : a la jonction carcasse/crosse — la commissure sous le talon,
#               la ou une main se pose vraiment sur un revolver
ARM_AT = (-0.17, -0.12, -0.63)
ARM_ROLL = -22.0  # paume tournee vers la crosse
FOREARM_STRETCH = 3.2  # sinon l'avant-bras s'arrete DANS le cadre : un moignon

# Main gauche : elle SOUTIENT l'arme sous la main droite (prise a deux mains), et
# c'est elle qui injecte l'energie au rechargement.
ARM_L_SCALE = 1.80
# x = -0.28 : DEHORS. A -0.11 le poing etait enfoui dans la crosse
# (qui s'etend jusqu'a x = -0.21) et on ne voyait qu'un avant-bras nu.
# La gauche vient enserrer les doigts de la droite, juste EN DESSOUS et un peu
# en avant — la prise a deux mains, pas deux poings empiles.
# Choregraphie du rechargement, en repere arme. L'arme est a une main : au repos
# rien n'est visible. La gauche ENTRE dans le cadre pour la recharger.
#   0 hors champ, en bas   ->  1 paume vers le ciel, boule formee
#   1                      ->  2 paume plaquee contre le flanc de l'arme
ARM_L_OUT = (0.42, -1.30, 0.06)
ARM_L_PALM = (0.40, -0.12, 0.06)
ARM_L_PUSH = (0.30, 0.04, -0.06)
#: Roulis de la paume a chaque temps : neutre en entrant, VERS LE CIEL pour
#: former la boule, puis retournee FACE AU FLANC de l'arme pour l'y pousser.
ARM_L_ROLL_OUT, ARM_L_ROLL_PALM, ARM_L_ROLL_PUSH = 60.0, 90.0, 190.0
ARM_L_PITCH_OUT, ARM_L_PITCH_PALM, ARM_L_PITCH_PUSH = 26.0, 22.0, 34.0
#: Ou la boule finit sa course : le coeur de l'arme.
BALL_TARGET = (0.0, 0.10, -0.10)
BALL_RADIUS = 0.13
ARM_L_PITCH = 30.0
ARM_L_ROLL = 26.0  # la gauche vient par-dessous, paume vers le haut  # l'avant-bras gauche remonte plus a plat vers l'arme
# Position « paume ouverte » face au barillet, atteinte pendant le rechargement.
ARM_L_CAST = ARM_L_PUSH
ARM_L_CAST_PITCH = 18.0

# Lacet NEGATIF : on voit le flanc GAUCHE de l'arme, bouche vers le haut-gauche,
# crosse en bas a droite — la tenue d'un droitier, convention CoD. En positif la
# bouche partait a droite et la main arrivait par la gauche : un viewmodel de
# gaucher, ce que personne n'avait remarque.
IDLE_YAW, IDLE_PITCH = 35.0, 14.0
# Visee : l'arme PLEIN DOS, dans l'axe du canon.
ADS_YAW, ADS_PITCH = 0.0, 2.0


def _stretch_forearm(mesh: glb.Mesh, factor: float) -> glb.Mesh:
    """Allonge l'avant-bras SOUS le poignet, pour qu'il sorte du cadre.

    Sans ça il s'arrete net au milieu de l'image et se lit comme un moignon greffe
    sur l'arme — signale en jeu des la premiere version.
    """
    p = mesh.positions.copy()
    low = p[:, 1] < -0.10
    p[low, 1] = -0.10 + (p[low, 1] + 0.10) * factor
    return glb.Mesh(p, mesh.normals, mesh.uvs, mesh.indices, mesh.base_color)


def _arm_matrix(scale: float, pitch_deg: float, roll_deg: float = 0.0) -> np.ndarray:
    """Matrice d'un bras.

    `roll` oriente la PAUME en tournant autour de l'axe de l'avant-bras (Y), qui
    est aussi la direction du bras. C'est le seul axe qui laisse le bras pointer
    ou il pointe. Une premiere version tournait autour de Z : a 95 deg, la paume
    regardait bien le ciel mais l'AVANT-BRAS partait a l'horizontale — le roulis
    emportait tout le membre.

    `pitch` vient ensuite : il oriente le bras lui-meme.
    """
    r = math.radians(roll_deg)
    cr, sr = math.cos(r), math.sin(r)
    twist = np.array([[cr, 0, sr], [0, 1, 0], [-sr, 0, cr]], np.float32)
    a = math.radians(pitch_deg)
    ca, sa = math.cos(a), math.sin(a)
    pitch = np.array([[1, 0, 0], [0, ca, -sa], [0, sa, ca]], np.float32)
    return (pitch @ twist) * scale


def load_scene():
    gun = glb.load(GUN_PATH)
    arm = _stretch_forearm(glb.load(ARM_PATH), FOREARM_STRETCH)
    arm_left = _stretch_forearm(glb.load(ARM_L_PATH), FOREARM_STRETCH)

    skeleton = pm.build_parts()
    masks = bind.bind(gun, skeleton, TO_GUN)
    pieces = {
        name: glb.Mesh(gun.positions, gun.normals, gun.uvs, gun.indices[mask], gun.base_color)
        for name, mask in masks.items()
        if mask.any()
    }

    return (
        gun,
        pieces,
        arm,
        _arm_matrix(ARM_SCALE, ARM_PITCH, ARM_ROLL),
        arm_left,
        _arm_matrix(ARM_L_SCALE, ARM_L_PITCH, ARM_L_ROLL),
    )


def articulate(name, swing, jaw, blink_l, blink_r, hammer, trigger, spin=0.0):
    """Transformation d'une piece. Les axes viennent du squelette, pas d'une
    estimation : le yoke et la charniere sont des cotes de construction.

    `spin` fait tourner le barillet SUR SON AXE ; `swing` le fait sortir de la
    carcasse. Les deux ne se valent pas : le maillage est une coque SANS
    INTERIEUR, donc toute piece qui quitte son logement decouvre du vide et
    dechire la silhouette. Un barillet qui tourne reste dans son logement — il ne
    peut pas dechirer quoi que ce soit, et c'est le mouvement qui caracterise un
    revolver.
    """
    rot = None
    pivot = None
    if name == "barillet" and spin:
        rot, pivot = pm.rot_z(spin), (0.0, pm.BORE, 0.0)
    elif name == "barillet" and swing:
        rot, pivot = pm.rot_z(swing), pm.YOKE
    elif name == "machoire_basse" and jaw:
        rot, pivot = pm.rot_x(-jaw), pm.JAW_HINGE
    elif name == "chien" and hammer:
        rot, pivot = pm.rot_x(hammer), (0.0, 0.37, -0.47)
    elif name == "detente" and trigger:
        rot, pivot = pm.rot_x(trigger), (0.0, -0.04, -0.30)
    elif name.startswith("oeil"):
        amount = blink_l if "gauche" in name else blink_r
        if amount:
            # Clignement = ecrasement vertical du globe. Il n'y a pas de paupiere
            # a animer, et a la taille d'un sprite l'ecrasement se lit exactement
            # comme un clin d'oeil.
            rot = np.diag([1.0, 1.0 - 0.90 * amount, 1.0]).astype(np.float32)
            pivot = (0.0, 0.575, 0.0)
    if rot is None:
        return TO_GUN, (0.0, 0.0, 0.0)
    return rot @ TO_GUN, pm.about(pivot, rot)


def _lerp(a, b, k):
    return a + (b - a) * k


def _lerp3(a, b, k):
    return tuple(x + (y - x) * k for x, y in zip(a, b))


# Pivot du poignet : l'arme se releve AUTOUR de lui, avec la main. Faire tourner
# la CAMERA a la place ferait pivoter l'avant-bras avec le decor, et le bras
# quitterait le coin de l'ecran ou il doit rester ancre.
WRIST = (0.0, -0.45, -0.72)


# ── Poses ──────────────────────────────────────────────────────────────────
Pose = dict


def _p(**kw) -> Pose:
    base = dict(
        yaw=IDLE_YAW, pitch=IDLE_PITCH, dip=0.0, tilt=0.0, spin=0.0, swing=0.0,
        jaw=0.0, blink_l=0.0, blink_r=0.0, hammer=0.0, trigger=0.0,
        charge=0.0, flow=0.0, halo=0.0, reach=0.0, ball=0.0, ball_travel=0.0, at=None,
    )
    base.update(kw)
    return base


# Repos : Pepin est VIVANT, il cligne. Le battement se joue sur la fin de la
# boucle pour que l'oeil reste ouvert l'essentiel du temps.
IDLE = [_p() for _ in range(9)] + [
    _p(blink_l=0.5, blink_r=0.45),
    _p(blink_l=1.0, blink_r=1.0),
    _p(blink_l=0.45, blink_r=0.55),
]

# Tir : le chien tombe, la detente recule.
FIRE = [
    _p(dip=-3.0, hammer=-34.0, trigger=-16.0),
    _p(dip=-1.0, hammer=-12.0, trigger=-8.0),
    _p(),
]

# Rechargement PAR L'ENERGIE : la main injecte une aura, l'arme se remplit.
#
# Pourquoi pas un barillet qui bascule : le maillage est une coque SANS
# INTERIEUR. Toute piece qui quitte son logement decouvre du vide, pas un
# mecanisme — un basculement a -50° ouvre deja un trou beant, et la fourrure qui
# fait pont entre les pieces se dechire meme sur une simple rotation. Mesure
# faite, pas supposee.
#
# L'energie ne deplace RIEN, donc elle ne peut pas dechirer. Et elle colle au
# personnage : Pepin est une creature vivante, pas un outil. Ses yeux s'allument
# quand on le nourrit.
#
# Quatre temps : la main se charge, le flux monte, l'arme s'embrase, tout retombe.
RELOAD = [
    _p(),                                                     # 00 repos, rien
    _p(reach=0.18, halo=0.08),                                # 01 la main entre
    _p(reach=0.38, ball=0.35, halo=0.18),                     # 02 elle monte
    _p(reach=0.50, ball=0.85, halo=0.28),                     # 03 paume au ciel
    _p(reach=0.52, ball=1.00, halo=0.34),                     # 04 boule formee
    _p(reach=0.66, ball=1.00, ball_travel=0.10, halo=0.44),   # 05 elle se retourne
    _p(reach=0.84, ball=0.95, ball_travel=0.45, halo=0.62, charge=0.25),
    _p(reach=1.00, ball=0.70, ball_travel=0.85, halo=0.86, charge=0.65),
    _p(reach=1.00, ball=0.00, halo=1.00, charge=1.00),        # 08 embrasement
    _p(reach=0.72, halo=0.62, charge=0.72),                   # 09 la main se retire
    _p(reach=0.34, halo=0.28, charge=0.36),
    _p(),                                                     # 11 repos
]

# Parole : la machoire basse s'ouvre et se ferme. Deux battements par boucle,
# d'amplitudes differentes — une bouche qui parle n'est pas un metronome.
TALK = [
    _p(), _p(jaw=16), _p(jaw=30), _p(jaw=14), _p(jaw=2),
    _p(jaw=22), _p(jaw=34), _p(jaw=12), _p(),
]

# Visee : l'arme PLEIN DOS et CENTREE, convention CoD — on regarde dans l'axe du
# canon. Ce n'est pas la meme image recadree : c'est la meme arme vue d'ailleurs,
# donc un clip a part. Le lacet passe de -30 a -4.
ADS_AT = (88, 54)
ADS = [
    _p(yaw=ADS_YAW, pitch=ADS_PITCH, at=ADS_AT) for _ in range(9)
] + [
    _p(yaw=ADS_YAW, pitch=ADS_PITCH, at=ADS_AT, blink_l=0.5, blink_r=0.45),
    _p(yaw=ADS_YAW, pitch=ADS_PITCH, at=ADS_AT, blink_l=1.0, blink_r=1.0),
    _p(yaw=ADS_YAW, pitch=ADS_PITCH, at=ADS_AT, blink_l=0.45, blink_r=0.55),
]

CLIPS = {"idle": IDLE, "fire": FIRE, "reload": RELOAD, "talk": TALK, "ads": ADS}


def build(out_dir: str = OUT_DIR, supersample: int = 4) -> list[str]:
    gun, pieces, arm, arm_matrix, arm_left, arm_left_matrix = load_scene()
    gun_only = render3d.Instance(gun, TO_GUN)

    probe_view = render3d.View(
        yaw=IDLE_YAW, pitch=IDLE_PITCH, offset=(0, 0, 0), distance=2.6, focal=240.0
    )
    probe = render3d.render(gun_only, probe_view, (400, 400), supersample=1)
    box, _ = framing.crop_to_content(probe)
    focal = probe_view.focal * GUN_HEIGHT_PX / box.height

    aimed = render3d.View(**{**probe_view.__dict__, "focal": focal, "offset": SCENE_OFFSET})
    located = render3d.render(gun_only, aimed, BIG, supersample=1)
    gun_box, (gx, gy) = framing.crop_to_content(located)
    win_x = int(gx + gun_box.width * 0.5 - GUN_AT[0])
    win_y = int(gy + gun_box.height * 0.5 - GUN_AT[1])

    def shoot(pose: Pose) -> Image.Image:
        # `tilt` s'applique a TOUT — pieces et bras — autour du poignet : c'est
        # la main qui releve l'arme, pas la camera qui tourne autour.
        wrist = pm.rot_x(pose["tilt"]) if pose["tilt"] else None
        wrist_shift = pm.about(WRIST, wrist) if wrist is not None else (0.0, 0.0, 0.0)

        def apply(matrix, translation):
            if wrist is None:
                return matrix, translation
            return wrist @ matrix, tuple(
                float(v) for v in (wrist @ np.array(translation, np.float32)) + wrist_shift
            )

        scene = [
            render3d.Instance(
                mesh,
                *apply(
                    *articulate(
                        name, pose["swing"], pose["jaw"], pose["blink_l"],
                        pose["blink_r"], pose["hammer"], pose["trigger"], pose["spin"],
                    )
                ),
            )
            for name, mesh in pieces.items()
        ]

        # ── Main gauche ───────────────────────────────────────────────
        # L'arme est a UNE MAIN : au repos rien n'est visible, la droite est
        # cachee derriere elle (cadrage Valorant). La gauche n'entre dans le
        # cadre que pour recharger, en trois temps :
        #   0 → 0.5  elle monte, paume tournee VERS LE CIEL, la boule s'y forme
        #   0.5 → 1  elle se retourne et plaque la boule contre le flanc
        reach = pose["reach"]
        if reach > 0.001:
            if reach <= 0.5:
                k = reach / 0.5
                at = _lerp3(ARM_L_OUT, ARM_L_PALM, k)
                roll = _lerp(ARM_L_ROLL_OUT, ARM_L_ROLL_PALM, k)
                tilt = _lerp(ARM_L_PITCH_OUT, ARM_L_PITCH_PALM, k)
            else:
                k = (reach - 0.5) / 0.5
                at = _lerp3(ARM_L_PALM, ARM_L_PUSH, k)
                roll = _lerp(ARM_L_ROLL_PALM, ARM_L_ROLL_PUSH, k)
                tilt = _lerp(ARM_L_PITCH_PALM, ARM_L_PITCH_PUSH, k)
            scene.append(
                render3d.Instance(arm_left, *apply(_arm_matrix(ARM_L_SCALE, tilt, roll), at))
            )

            # Boule d'energie : elle naît dans la paume, puis file dans l'arme.
            ball = pose["ball"]
            if ball > 0.01:
                travel = pose["ball_travel"]
                palm = (at[0], at[1] + 0.20, at[2])
                centre = _lerp3(palm, BALL_TARGET, travel)
                sphere = prim.sphere(BALL_RADIUS * ball, centre, segments=14, rings=9)
                scene.append(
                    render3d.Instance(sphere, *apply(np.eye(3, dtype=np.float32), (0, 0, 0)),
                                      colour=(1.0, 0.72, 0.36))
                )

        view = render3d.View(
            yaw=pose["yaw"], pitch=pose["pitch"], offset=SCENE_OFFSET, distance=2.6, focal=focal
        )
        dip = pose["dip"]
        anchor = pose["at"] or GUN_AT
        wx = win_x + (GUN_AT[0] - anchor[0])
        top = win_y + (GUN_AT[1] - anchor[1]) - int(dip)
        window = (wx, top, wx + CANVAS[0], top + CANVAS[1])

        big = render3d.render(scene, view, BIG, supersample=supersample)
        sprite = pixelize.pixelize(big.crop(window), colours=COLOURS)

        if pose["charge"] > 0.02 or pose["flow"] > 0.02 or pose["halo"] > 0.02:
            # Masques des pieces a faire rougeoyer, rendus avec EXACTEMENT la
            # meme camera et la meme fenetre : l'aura tombe donc pile sur le
            # barillet et les yeux, sans avoir a deviner ou ils sont a l'ecran.
            def piece_mask(names: tuple[str, ...]):
                subset = [i for i, n in enumerate(pieces) if n in names]
                if not subset:
                    return None
                shot = render3d.render(
                    [scene[i] for i in subset], view, BIG, supersample=1
                )
                return aura.mask_from(shot.crop(window))

            sprite = aura.apply(
                sprite,
                piece_mask(("barillet",)),
                piece_mask(("oeil_gauche", "oeil_droit")),
                charge=pose["charge"],
                flow=pose["flow"],
                halo=pose["halo"],
            )
        return sprite

    os.makedirs(out_dir, exist_ok=True)
    written = []
    for name, poses in CLIPS.items():
        for i, pose in enumerate(poses):
            path = os.path.join(out_dir, f"pepin_{name}_{i:02d}.png")
            shoot(pose).save(path)
            written.append(path)
    return written


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser()
    parser.add_argument("--out", default=OUT_DIR)
    parser.add_argument("--sheet")
    parser.add_argument("--clip", default="reload")
    args = parser.parse_args()

    files = build(args.out)
    print(f"{len(files)} frames ecrites dans {args.out}")
    for name, poses in CLIPS.items():
        print(f"  {name:8s} {len(poses):2d} frames")
    if args.sheet:
        shots = [Image.open(p) for p in files if f"_{args.clip}_" in p]
        pixelize.contact(shots, zoom=2).save(args.sheet)
        print("planche :", args.sheet)
