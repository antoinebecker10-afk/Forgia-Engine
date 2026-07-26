"""Génère de zéro le nain de la forge + son set d'armure modulaire (WoW-like).

Tout est procédural : aucune source externe, aucune dépendance de licence. La
géométrie est bâtie à la main (tubes loftés / ellipsoïdes / boîtes chanfreinées),
le squelette est écrit os par os, et le skinning est calculé par enveloppes
(distance point→segment d'os) — pas de `parent_set(ARMATURE_AUTO)`, dont le
heat-weighting échoue sur des primitives qui s'interpénètrent.

Sortie (assets/models/characters/dwarf/) :
  body.glb      corps de base, 7 sous-mesh NOMMÉS : head hair beard torso hands
                pelvis feet — le runtime en cache certains selon l'équipement
  helmet.glb    ─┐
  chest.glb      │ 5 slots, chacun skinné sur LE MÊME squelette (mêmes noms d'os,
  gloves.glb     │ même rest pose) → le runtime remappe les joints par nom
  legs.glb       │
  boots.glb     ─┘
  manifest.json couche `definition` : slots, masques de sous-mesh, liste d'os

Repère : Blender Z-up, le nain regarde +Y. L'export glTF (`export_yup`) convertit
(x,y,z) → (x,z,-y), donc +Y Blender = -Z glTF = forward Bevy. Rien à corriger côté
moteur.

Conventions honorées :
  - bmesh + recalc_face_normals systématique (from_pydata brut sort des faces
    inversées ; le StandardMaterial Bevy est single-sided → fatal en jeu)
  - export SANS animation (l'importeur glTF laisse la dernière action active et
    toute preview rendrait une pose aléatoire)
  - ≤ 4 influences par vertex (export_all_influences=False)

Usage :
  blender --background --factory-startup --python tools/blender/build_dwarf.py -- \
    --out assets/models/characters/dwarf
"""

import argparse
import json
import sys
from math import cos, pi, radians, sin, sqrt
from pathlib import Path

import bmesh
import bpy
from mathutils import Matrix, Vector

sys.path.insert(0, str(Path(__file__).resolve().parent))
import dwarf_anim  # noqa: E402  (dépend du sys.path ci-dessus)
import dwarf_texturing  # noqa: E402

# ============================================================================
# CONSTANTES TUNABLES — toute la silhouette du nain se règle ici
# ============================================================================

# -- proportions (mètres, Z-up) ---------------------------------------------
# Nain trapu ~4 têtes : jambes courtes, torse en tonneau, épaules très larges.
H_TOTAL = 1.45
Z_ANKLE = 0.11
Z_KNEE = 0.36
Z_HIP = 0.62
Z_WAIST = 0.75
Z_CHEST = 0.94
Z_SHOULDER = 1.01
Z_NECK = 1.06
Z_CHIN = 1.13
Z_HEAD_C = 1.26  # centre du crâne
Z_HEAD_TOP = 1.45

LEG_X = 0.115  # écartement des jambes
SHOULDER_X = 0.245  # demi-largeur d'épaule (pivot du bras)
ARM_UPPER = 0.215
ARM_FORE = 0.225
ARM_DROP_DEG = 62.0  # A-pose : bras construits en T puis rabattus

R_THIGH = 0.098
R_CALF = 0.086
R_ANKLE = 0.062
EYE_Z = 1.258  # mi-hauteur du crâne (1.070 → 1.446) — règle de proportion
EYE_X = 0.072  # règle des cinq yeux : l'écart entre les yeux = une largeur d'œil
EYE_R = 0.036
R_HEAD_U = 0.170  # demi-largeur crâne (X)
R_HEAD_V = 0.160  # demi-profondeur crâne (Y)
R_HEAD_W = 0.185  # demi-hauteur crâne (Z)
R_UPPERARM = 0.078
R_FOREARM = 0.070
R_WRIST = 0.056

FOOT_LEN = 0.235
FOOT_W = 0.135
FOOT_H = 0.095

RING_SEGMENTS = 16  # segments par anneau de tube — pilote le polycount
SUBSURF_BODY = 0  # 1 = corps plus lisse, ×4 tris

# -- palette cartoon (aplats, DA « Verre & Braise ») -------------------------
# nom -> (rgba, metallic, roughness). Les métaux sont volontairement mats : à
# roughness basse le spéculaire brûle la forme et l'aplat cartoon disparaît.
# Valeurs volontairement PROFONDES : un acier clair ne laisse aucune place à
# l'usure d'arête, et l'ensemble vire au pastel — le premier signal « jouet ».
PALETTE = {
    # Peau FROIDE gris-bleu (réf. Torin) : tout le punch de l'image vient du
    # contraste complémentaire peau froide / barbe et laiton chauds. En peau
    # chaude, le roux se fond dedans et le personnage s'aplatit.
    "skin": ((0.23, 0.27, 0.32, 1.0), 0.00, 0.70),
    "hair": ((0.56, 0.11, 0.03, 1.0), 0.00, 0.74),  # roux forge saturé
    "brass": ((0.48, 0.26, 0.08, 1.0), 0.85, 0.38),
    "glass": ((0.06, 0.09, 0.11, 1.0), 0.10, 0.18),
    "eye": ((0.05, 0.04, 0.06, 1.0), 0.00, 0.30),
    "mouth": ((0.09, 0.03, 0.035, 1.0), 0.00, 0.58),
    "eye_white": ((0.86, 0.83, 0.79, 1.0), 0.00, 0.50),
    "tunic": ((0.29, 0.09, 0.09, 1.0), 0.00, 0.78),
    "trouser": ((0.16, 0.14, 0.18, 1.0), 0.00, 0.80),
    "leather": ((0.21, 0.11, 0.06, 1.0), 0.00, 0.72),
    "leather_light": ((0.42, 0.26, 0.13, 1.0), 0.00, 0.68),  # sangles : contraste
    "fur": ((0.19, 0.14, 0.11, 1.0), 0.00, 0.90),
    "sole": ((0.09, 0.08, 0.07, 1.0), 0.00, 0.88),  # semelle : mate, jamais métallique
    "steel": ((0.30, 0.33, 0.39, 1.0), 0.78, 0.44),
    "steel_dark": ((0.12, 0.13, 0.17, 1.0), 0.78, 0.48),
    "gold": ((0.70, 0.48, 0.14, 1.0), 0.85, 0.36),
    "ember": ((1.00, 0.42, 0.10, 1.0), 0.00, 0.60),
}
EMISSIVE = {"ember": 2.5}  # au-delà la braise brûle en blanc et perd sa couleur

# -- squelette ---------------------------------------------------------------
# Hiérarchie et nommage calqués sur Mixamo (sans le préfixe `mixamorig:`, inutile
# en jeu) : un retarget d'anim Mixamo mappe sur la structure, pas sur le préfixe.
DROP = radians(ARM_DROP_DEG)
DROP_DIR = Vector((cos(DROP), 0.0, -sin(DROP)))
SHOULDER_P = Vector((SHOULDER_X, 0.0, Z_SHOULDER))
ELBOW_P = SHOULDER_P + DROP_DIR * ARM_UPPER
WRIST_P = ELBOW_P + DROP_DIR * ARM_FORE
HAND_END_P = WRIST_P + DROP_DIR * 0.105
WRIST_X = SHOULDER_X + ARM_UPPER + ARM_FORE  # abscisse du poignet en T-pose

# Bras construits en T-pose puis rabattus : la même matrice sert à la géométrie
# ET aux os des doigts, sinon les os ne tombent pas dans les doigts.
ARM_M = Matrix.Translation(SHOULDER_P) @ Matrix.Rotation(DROP, 4, "Y") @ Matrix.Translation(-SHOULDER_P)

# Disposition des doigts, source de vérité unique partagée par `g_hand` et le
# squelette. Ordre anatomique : l'index est le doigt VOISIN du pouce — la
# version précédente plaçait l'auriculaire à côté du pouce.
HAND_KNUCKLE = 0.078  # du poignet aux articulations
FINGERS = (
    ("Index", 0.035, 0.052),
    ("Middle", 0.012, 0.058),
    ("Ring", -0.011, 0.052),
    ("Pinky", -0.034, 0.042),
)
THUMB = ((0.020, 0.050, -0.012), (0.050, 0.070, -0.020), (0.072, 0.078, -0.030))


def _arm_pt(point, side_sign):
    """Point de bras en T-pose → position finale (A-pose, côté mirroré)."""
    vec = ARM_M @ Vector(point)
    return (side_sign * vec.x, vec.y, vec.z)

# nom -> (parent, head, tail, rayon d'enveloppe pour le skinning)
SKELETON = {
    "Hips": (None, (0, 0, Z_HIP), (0, 0, Z_WAIST), 0.22),
    "Spine": ("Hips", (0, 0, Z_WAIST), (0, 0, 0.855), 0.20),
    "Spine1": ("Spine", (0, 0, 0.855), (0, 0, Z_CHEST), 0.22),
    "Spine2": ("Spine1", (0, 0, Z_CHEST), (0, 0, Z_NECK), 0.24),
    "Neck": ("Spine2", (0, 0, Z_NECK), (0, 0, Z_CHIN), 0.12),
    "Head": ("Neck", (0, 0, Z_CHIN), (0, 0, Z_HEAD_TOP), 0.26),
    "HeadTop_End": ("Head", (0, 0, Z_HEAD_TOP), (0, 0, Z_HEAD_TOP + 0.05), 0.0),
}
for side, sx in (("Left", 1.0), ("Right", -1.0)):
    sh, el, wr, he = (
        Vector((sx * SHOULDER_P.x, SHOULDER_P.y, SHOULDER_P.z)),
        Vector((sx * ELBOW_P.x, ELBOW_P.y, ELBOW_P.z)),
        Vector((sx * WRIST_P.x, WRIST_P.y, WRIST_P.z)),
        Vector((sx * HAND_END_P.x, HAND_END_P.y, HAND_END_P.z)),
    )
    SKELETON[f"{side}Shoulder"] = ("Spine2", (sx * 0.05, 0, Z_SHOULDER), tuple(sh), 0.13)
    SKELETON[f"{side}Arm"] = (f"{side}Shoulder", tuple(sh), tuple(el), 0.13)
    SKELETON[f"{side}ForeArm"] = (f"{side}Arm", tuple(el), tuple(wr), 0.12)
    SKELETON[f"{side}Hand"] = (f"{side}ForeArm", tuple(wr), tuple(he), 0.11)
    SKELETON[f"{side}Hand_End"] = (f"{side}Hand", tuple(he), tuple(he + DROP_DIR * 0.03), 0.0)

    # OS DE TORSION : sans lui, une rotation du poignet vrille tout l'avant-bras
    # sur sa longueur (« papier de bonbon »). Il en absorbe la moitié.
    SKELETON[f"{side}ForeArmTwist"] = (
        f"{side}ForeArm",
        _arm_pt((SHOULDER_X + ARM_UPPER + ARM_FORE * 0.45, 0, Z_SHOULDER), sx),
        tuple(wr), 0.082,
    )

    # DOIGTS : 2 os par doigt, assez pour une main trapue et pour tenir une arme
    for fname, dy, length in FINGERS:
        x0 = WRIST_X + HAND_KNUCKLE
        p0 = _arm_pt((x0, dy, Z_SHOULDER + 0.004), sx)
        p1 = _arm_pt((x0 + length * 0.55, dy, Z_SHOULDER - 0.008), sx)
        p2 = _arm_pt((x0 + length, dy, Z_SHOULDER - 0.024), sx)
        SKELETON[f"{side}Hand{fname}1"] = (f"{side}Hand", p0, p1, 0.030)
        SKELETON[f"{side}Hand{fname}2"] = (f"{side}Hand{fname}1", p1, p2, 0.027)
    t0, t1, t2 = (_arm_pt((WRIST_X + dx, dy, Z_SHOULDER + dz), sx) for dx, dy, dz in THUMB)
    SKELETON[f"{side}HandThumb1"] = (f"{side}Hand", t0, t1, 0.034)
    SKELETON[f"{side}HandThumb2"] = (f"{side}HandThumb1", t1, t2, 0.030)

    lx = sx * LEG_X
    SKELETON[f"{side}UpLeg"] = ("Hips", (lx, 0, Z_HIP), (lx, 0, Z_KNEE), 0.16)
    SKELETON[f"{side}Leg"] = (f"{side}UpLeg", (lx, 0, Z_KNEE), (lx, 0, Z_ANKLE), 0.14)
    SKELETON[f"{side}Foot"] = (f"{side}Leg", (lx, 0, Z_ANKLE), (lx, 0.09, 0.030), 0.14)
    SKELETON[f"{side}ToeBase"] = (f"{side}Foot", (lx, 0.09, 0.030), (lx, 0.17, 0.025), 0.10)
    SKELETON[f"{side}Toe_End"] = (f"{side}ToeBase", (lx, 0.17, 0.025), (lx, 0.20, 0.025), 0.0)

# Mâchoire + chaîne de barbe : la barbe est L'élément d'un nain qui doit avoir du
# mouvement secondaire, et sans os de mâchoire il n'y a pas de bouche animable.
SKELETON["Jaw"] = ("Head", (0, 0.030, 1.150), (0, 0.140, 1.108), 0.105)
SKELETON["Beard1"] = ("Head", (0, 0.030, 1.150), (0, 0.040, 1.075), 0.115)
SKELETON["Beard2"] = ("Beard1", (0, 0.040, 1.075), (0, 0.044, 1.000), 0.105)
SKELETON["Beard3"] = ("Beard2", (0, 0.044, 1.000), (0, 0.040, 0.920), 0.085)

BOTH = lambda *names: [f"{s}{n}" for n in names for s in ("Left", "Right")]
FINGER_BONES = [
    f"{s}Hand{n}{i}"
    for s in ("Left", "Right")
    for n in ("Thumb", *(f[0] for f in FINGERS))
    for i in (1, 2)
]

# quels os peuvent influencer quel mesh (évite les fuites de poids)
SPINE = ["Hips", "Spine", "Spine1", "Spine2"]
INFLUENCES = {
    "head": ["Head", "Neck", "Jaw"],
    "hair": ["Head"],
    "goggles": ["Head"],
    "beard": ["Head", "Neck", "Jaw", "Beard1", "Beard2", "Beard3"],
    "torso": SPINE + ["Neck"] + BOTH("Shoulder", "Arm"),
    "hands": BOTH("ForeArm", "ForeArmTwist", "Hand") + FINGER_BONES,
    "pelvis": ["Hips", "Spine"] + BOTH("UpLeg"),
    "feet": BOTH("Leg", "Foot", "ToeBase"),
    "helmet": ["Head"],
    "chest": SPINE + ["Neck"] + BOTH("Shoulder", "Arm"),
    "gloves": BOTH("ForeArm", "ForeArmTwist", "Hand") + FINGER_BONES,
    "legs": ["Hips", "Spine"] + BOTH("UpLeg", "Leg"),
    "boots": BOTH("Leg", "Foot", "ToeBase"),
}

# slot -> sous-mesh du corps masqués quand il est équipé (modèle WoW)
HIDES = {
    "helmet": ["hair", "goggles"],  # la barbe reste : c'est un nain
    "chest": ["torso"],
    "gloves": ["hands"],
    "legs": ["pelvis"],
    "boots": ["feet"],
}
BODY_PARTS = ["head", "hair", "goggles", "beard", "torso", "hands", "pelvis", "feet"]
SLOTS = ["helmet", "chest", "gloves", "legs", "boots"]


# ============================================================================
# PRIMITIVES GÉOMÉTRIQUES — chaque helper renvoie (verts, faces)
# ============================================================================

AXES = {"X": (0, 1, 2), "Y": (1, 2, 0), "Z": (2, 0, 1)}


def g_tube(axis, sections, n=RING_SEGMENTS, cap_start=True, cap_end=True):
    """Tube lofté. `sections` = [(t, cu, cv, ru, rv)] ordonnées le long de `axis`.

    t = coordonnée sur l'axe, (cu, cv) = centre dans le plan, (ru, rv) = rayons.
    """
    ti, ui, vi = AXES[axis]
    verts, faces = [], []
    for t, cu, cv, ru, rv in sections:
        for k in range(n):
            a = 2.0 * pi * k / n
            p = [0.0, 0.0, 0.0]
            p[ti], p[ui], p[vi] = t, cu + ru * cos(a), cv + rv * sin(a)
            verts.append(tuple(p))
    for s in range(len(sections) - 1):
        b0, b1 = s * n, (s + 1) * n
        for k in range(n):
            k2 = (k + 1) % n
            faces.append((b0 + k, b0 + k2, b1 + k2, b1 + k))
    for cap, sec, base in ((cap_start, sections[0], 0), (cap_end, sections[-1], (len(sections) - 1) * n)):
        if not cap:
            continue
        t, cu, cv, _, _ = sec
        c = len(verts)
        p = [0.0, 0.0, 0.0]
        p[ti], p[ui], p[vi] = t, cu, cv
        verts.append(tuple(p))
        for k in range(n):
            faces.append((c, base + k, base + (k + 1) % n))
    return verts, faces


def g_planar_tube(axis, sections, n=28, sides=8, planar=0.5, cap_start=True, cap_end=True):
    """Tube à section POLYGONALE adoucie — donne des PLANS, pas une révolution.

    Principe Asaro : un visage est fait de plans qui accrochent la lumière
    différemment, et c'est la cassure entre eux qui crée la valeur. Une section
    circulaire ne produit aucune cassure, d'où l'aspect « plat » quel que soit
    le sculpt appliqué ensuite.

    Le rayon est ramené vers celui d'un polygone régulier à `sides` côtés :
    `planar=0` garde l'ellipse, `1` donne le polygone franc. La phase est calée
    pour qu'un plan soit CENTRÉ sur l'avant (+Y) — le plan du visage.
    """
    ti, ui, vi = AXES[axis]
    step = 2.0 * pi / sides
    phase = pi / 2.0 - step / 2.0
    apothem = cos(step / 2.0)
    verts, faces = [], []
    for t, cu, cv, ru, rv in sections:
        for k in range(n):
            a = 2.0 * pi * k / n
            local = ((a - phase) % step) - step / 2.0
            mod = 1.0 + planar * (apothem / cos(local) - 1.0)
            p = [0.0, 0.0, 0.0]
            p[ti], p[ui], p[vi] = t, cu + ru * mod * cos(a), cv + rv * mod * sin(a)
            verts.append(tuple(p))
    for s in range(len(sections) - 1):
        b0, b1 = s * n, (s + 1) * n
        for k in range(n):
            k2 = (k + 1) % n
            faces.append((b0 + k, b0 + k2, b1 + k2, b1 + k))
    for cap, sec, base in ((cap_start, sections[0], 0), (cap_end, sections[-1], (len(sections) - 1) * n)):
        if not cap:
            continue
        t, cu, cv, _, _ = sec
        centre = len(verts)
        p = [0.0, 0.0, 0.0]
        p[ti], p[ui], p[vi] = t, cu, cv
        verts.append(tuple(p))
        for k in range(n):
            faces.append((centre, base + k, base + (k + 1) % n))
    return verts, faces


def g_cloth(axis, sections, n=24, folds=8, depth=0.045, twist=0.55, cap_start=True, cap_end=True):
    """Tube d'ÉTOFFE : rayon modulé angulairement pour créer de vrais plis.

    Un cylindre lisse lit « plastique » quelle que soit sa texture — c'est le
    pli qui fait le tissu. La phase dérive avec la hauteur (`twist`) pour que
    les plis serpentent au lieu d'être des cannelures verticales rigides.

    Demande plus de segments qu'un tube ordinaire : sous ~20, le pli devient un
    polygone et non une ondulation.
    """
    ti, ui, vi = AXES[axis]
    t_first, t_last = sections[0][0], sections[-1][0]
    span = (t_last - t_first) or 1.0
    verts, faces = [], []
    for t, cu, cv, ru, rv in sections:
        f = (t - t_first) / span
        for k in range(n):
            a = 2.0 * pi * k / n
            mod = 1.0 + depth * sin(folds * a + twist * 2.0 * pi * f)
            p = [0.0, 0.0, 0.0]
            p[ti], p[ui], p[vi] = t, cu + ru * mod * cos(a), cv + rv * mod * sin(a)
            verts.append(tuple(p))
    for s in range(len(sections) - 1):
        b0, b1 = s * n, (s + 1) * n
        for k in range(n):
            k2 = (k + 1) % n
            faces.append((b0 + k, b0 + k2, b1 + k2, b1 + k))
    for cap, sec, base in ((cap_start, sections[0], 0), (cap_end, sections[-1], (len(sections) - 1) * n)):
        if not cap:
            continue
        t, cu, cv, _, _ = sec
        centre = len(verts)
        p = [0.0, 0.0, 0.0]
        p[ti], p[ui], p[vi] = t, cu, cv
        verts.append(tuple(p))
        for k in range(n):
            faces.append((centre, base + k, base + (k + 1) % n))
    return verts, faces


def g_ellipsoid(center, radii, n=RING_SEGMENTS, rings=9, zmin=None, zmax=None):
    """Ellipsoïde à pôles. `zmin`/`zmax` coupent (dôme de casque, paupière)."""
    cx, cy, cz = center
    rx, ry, rz = radii
    verts, faces, ring_base = [], [], []
    for i in range(1, rings):
        th = pi * i / rings
        z, s = cz + rz * cos(th), sin(th)
        ring_base.append(len(verts))
        for k in range(n):
            a = 2.0 * pi * k / n
            verts.append((cx + rx * s * cos(a), cy + ry * s * sin(a), z))
    top, bot = len(verts), len(verts) + 1
    verts.extend([(cx, cy, cz + rz), (cx, cy, cz - rz)])
    for r in range(len(ring_base) - 1):
        b0, b1 = ring_base[r], ring_base[r + 1]
        for k in range(n):
            k2 = (k + 1) % n
            faces.append((b0 + k, b0 + k2, b1 + k2, b1 + k))
    for k in range(n):
        faces.append((top, ring_base[0] + (k + 1) % n, ring_base[0] + k))
        faces.append((bot, ring_base[-1] + k, ring_base[-1] + (k + 1) % n))
    if zmin is not None or zmax is not None:
        verts, faces = _clip_z((verts, faces), zmin, zmax)
    return verts, faces


def _clip_z(g, zmin=None, zmax=None):
    """Jette les faces hors de la tranche [zmin, zmax], puis compacte les verts."""
    verts, faces = g
    kept = []
    for f in faces:
        z = sum(verts[i][2] for i in f) / len(f)
        if (zmin is None or z >= zmin) and (zmax is None or z <= zmax):
            kept.append(f)
    used = sorted({i for f in kept for i in f})
    remap = {old: new for new, old in enumerate(used)}
    return [verts[i] for i in used], [tuple(remap[i] for i in f) for f in kept]


def g_box(lo, hi):
    return (
        [
            (lo[0], lo[1], lo[2]), (hi[0], lo[1], lo[2]), (hi[0], hi[1], lo[2]), (lo[0], hi[1], lo[2]),
            (lo[0], lo[1], hi[2]), (hi[0], lo[1], hi[2]), (hi[0], hi[1], hi[2]), (lo[0], hi[1], hi[2]),
        ],
        [(0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)],
    )


def g_box_c(lo, hi, chamfer=0.015, segments=2):
    """Boîte chanfreinée — la lecture « hard-surface cartoon » vient de ce biseau."""
    bm = bmesh.new()
    verts, faces = g_box(lo, hi)
    bv = [bm.verts.new(v) for v in verts]
    bm.verts.index_update()
    for f in faces:
        bm.faces.new([bv[i] for i in f])
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces[:])
    bmesh.ops.bevel(
        bm, geom=bm.verts[:] + bm.edges[:], offset=chamfer, segments=segments,
        profile=0.5, affect="EDGES", clamp_overlap=True,
    )
    bm.verts.index_update()
    out_v = [tuple(v.co) for v in bm.verts]
    out_f = [tuple(v.index for v in f.verts) for f in bm.faces]
    bm.free()
    return out_v, out_f


def g_merge(*geoms):
    verts, faces = [], []
    for vs, fs in geoms:
        off = len(verts)
        verts.extend(vs)
        faces.extend(tuple(i + off for i in f) for f in fs)
    return verts, faces


def g_xform(g, matrix):
    return [tuple(matrix @ Vector(v)) for v in g[0]], g[1]


def g_mirror_x(g):
    """Miroir X + inversion du winding (sinon normales rentrantes = invisible en jeu)."""
    return [(-v[0], v[1], v[2]) for v in g[0]], [tuple(reversed(f)) for f in g[1]]


def g_arm(g):
    """Passe une géométrie de bras construite en T-pose vers la A-pose."""
    return g_xform(g, ARM_M)


def g_sided(builder):
    """Construit le côté gauche puis le miroite : une seule source de vérité."""
    left = builder()
    return g_merge(left, g_mirror_x(left))


def g_studs(axis, t, cu, cv, ru, rv, count, radius, phase=0.0):
    """Rivets répartis sur un anneau — le détail qui fait « pièce forgée ».

    Centre posé SUR la surface de la plaque : la demi-bille qui dépasse la croise
    en biais, jamais tangentiellement (cf. règle de tangence).
    """
    ti, ui, vi = AXES[axis]
    beads = []
    for i in range(count):
        a = phase + 2.0 * pi * i / count
        p = [0.0, 0.0, 0.0]
        p[ti], p[ui], p[vi] = t, cu + ru * cos(a), cv + rv * sin(a)
        beads.append(g_ellipsoid(tuple(p), (radius, radius, radius), n=6, rings=3))
    return g_merge(*beads)


def _temp_object(name, geom):
    mesh = bpy.data.meshes.new(name)
    mesh.from_pydata(geom[0], [], [list(f) for f in geom[1]])
    mesh.validate()
    mesh.update()
    obj = bpy.data.objects.new(name, mesh)
    bpy.context.scene.collection.objects.link(obj)
    return obj


# ============================================================================
# SCULPT PROGRAMMATIQUE
# Le mode sculpt de Blender exige un viewport (inaccessible en headless), mais
# une brosse n'est que de l'arithmétique sur des vertices : déplacer ceux d'une
# zone avec une atténuation. Écrites ici, elles donnent le seul outil capable de
# produire de l'ANATOMIE — pommette, sillon, arcade — là où l'empilement de
# primitives ne donne que des volumes.
# ============================================================================


def _falloff(distance, radius):
    if distance >= radius:
        return 0.0
    t = 1.0 - distance / radius
    return t * t * (3.0 - 2.0 * t)  # smoothstep : pas de cassure au bord


def _vertex_normals(geom):
    verts, faces = geom
    acc = [Vector((0.0, 0.0, 0.0)) for _ in verts]
    for face in faces:
        a, b, c = (Vector(verts[face[0]]), Vector(verts[face[1]]), Vector(verts[face[2]]))
        normal = (b - a).cross(c - a)
        for idx in face:
            acc[idx] += normal
    return [n.normalized() if n.length > 1e-9 else Vector((0.0, 0.0, 1.0)) for n in acc]


def g_subdivide(geom, levels=1):
    """Densifie avant sculpt : une brosse ne peut pas créer de détail plus fin
    que la maille sur laquelle elle s'applique."""
    obj = _temp_object("subdiv_src", geom)
    mod = obj.modifiers.new("subsurf", "SUBSURF")
    mod.levels = mod.render_levels = levels
    bpy.context.view_layer.update()
    deps = bpy.context.evaluated_depsgraph_get()
    baked = bpy.data.meshes.new_from_object(obj.evaluated_get(deps))
    out = (
        [tuple(v.co) for v in baked.vertices],
        [tuple(p.vertices) for p in baked.polygons],
    )
    bpy.data.meshes.remove(baked)
    mesh = obj.data
    bpy.data.objects.remove(obj, do_unlink=True)
    bpy.data.meshes.remove(mesh)
    return out


def s_grab(geom, center, radius, offset):
    """Brosse GRAB : tire une zone dans une direction (menton, masse d'une joue)."""
    origin, delta = Vector(center), Vector(offset)
    return [
        tuple(Vector(v) + delta * _falloff((Vector(v) - origin).length, radius))
        for v in geom[0]
    ], geom[1]


def s_inflate(geom, center, radius, amount):
    """Brosse INFLATE : gonfle (+) ou creuse (−) le long de la normale.

    C'est la brosse de la pommette, du renflement de biceps, du creux de tempe.
    """
    origin = Vector(center)
    out = []
    for vert, normal in zip(geom[0], _vertex_normals(geom)):
        pos = Vector(vert)
        out.append(tuple(pos + normal * amount * _falloff((pos - origin).length, radius)))
    return out, geom[1]


def s_crease(geom, start, end, radius, depth):
    """Brosse CREASE : creuse un sillon le long d'un segment.

    Ride du front, sillon nasogénien, pli de tissu — c'est ce qui fait lire
    « peau » et « étoffe » plutôt que « surface ».
    """
    pa, pb = Vector(start), Vector(end)
    out = []
    for vert, normal in zip(geom[0], _vertex_normals(geom)):
        pos = Vector(vert)
        out.append(tuple(pos - normal * depth * _falloff(_dist_to_segment(pos, pa, pb), radius)))
    return out, geom[1]


def g_union(*geoms):
    """Fond des coques qui s'interpénètrent en UN volume, par unions successives.

    Contrairement au remaillage voxel — essayé et abandonné, il rabote le nez et
    facette le crâne — l'union préserve exactement les formes : elle ne fait que
    supprimer les surfaces internes. C'est le passage de « coques qui se
    croisent » à « une surface ».
    """
    result = geoms[0]
    for geom in geoms[1:]:
        try:
            result = g_boolean(result, geom, operation="UNION")
        except Exception as exc:  # une union ratée ne doit pas casser le build
            print(f"[union] repli sur fusion simple : {exc}")
            result = g_merge(result, geom)
    return result


def g_relax(geom, factor=0.35, iterations=4):
    """Détend les vertices : adoucit les arêtes vives nées de l'union.

    C'est le congé qu'un sculpteur poserait à la jonction nez/joue. Sans lui,
    l'union laisse un pli net qui lit « collé » autant que l'interpénétration.
    """
    obj = _temp_object("relax_src", geom)
    mod = obj.modifiers.new("relax", "SMOOTH")
    mod.factor = factor
    mod.iterations = iterations
    bpy.context.view_layer.update()
    deps = bpy.context.evaluated_depsgraph_get()
    baked = bpy.data.meshes.new_from_object(obj.evaluated_get(deps))
    out = (
        [tuple(v.co) for v in baked.vertices],
        [tuple(p.vertices) for p in baked.polygons],
    )
    bpy.data.meshes.remove(baked)
    mesh = obj.data
    bpy.data.objects.remove(obj, do_unlink=True)
    bpy.data.meshes.remove(mesh)
    return out


def g_remesh(geom, voxel=0.005, ratio=0.18):
    """Fusionne un empilement de primitives en UNE surface continue.

    C'est le plafond de réalisme du procédural par primitives : tant que le nez,
    l'arcade et les oreilles sont des coques distinctes qui s'interpénètrent, il
    n'existe aucune surface continue et l'œil lit « pièces assemblées ». Le
    remaillage voxel les fond en un seul volume avec des raccords doux, façon
    sculpt ; la décimation ramène le polycount à un budget de jeu.

    À réserver aux formes ORGANIQUES : sur l'armure, le voxel arrondirait les
    arêtes vives et mangerait les rivets.
    """
    obj = _temp_object("remesh_src", geom)
    rem = obj.modifiers.new("remesh", "REMESH")
    rem.mode = "VOXEL"
    rem.voxel_size = voxel
    rem.use_smooth_shade = True
    dec = obj.modifiers.new("decimate", "DECIMATE")
    dec.decimate_type = "COLLAPSE"
    dec.ratio = ratio

    bpy.context.view_layer.update()
    deps = bpy.context.evaluated_depsgraph_get()
    baked = bpy.data.meshes.new_from_object(obj.evaluated_get(deps))
    out = (
        [tuple(v.co) for v in baked.vertices],
        [tuple(p.vertices) for p in baked.polygons],
    )
    bpy.data.meshes.remove(baked)
    mesh = obj.data
    bpy.data.objects.remove(obj, do_unlink=True)
    bpy.data.meshes.remove(mesh)
    return out


def g_boolean(base, tool, operation="DIFFERENCE"):
    """Booléen réel via le modificateur + depsgraph.

    C'est la seule façon de CREUSER : en empilant des primitives on ne fait
    qu'ajouter de la matière, jamais un vide. Les orbites en dépendent — un œil
    posé sur un crâne lisse est LE signal « jouet » n°1.
    """
    obj_a, obj_b = _temp_object("bool_a", base), _temp_object("bool_b", tool)
    mod = obj_a.modifiers.new("bool", "BOOLEAN")
    mod.object = obj_b
    mod.operation = operation
    mod.solver = "EXACT"
    bpy.context.view_layer.update()
    deps = bpy.context.evaluated_depsgraph_get()
    baked = bpy.data.meshes.new_from_object(obj_a.evaluated_get(deps))
    out = (
        [tuple(v.co) for v in baked.vertices],
        [tuple(p.vertices) for p in baked.polygons],
    )
    bpy.data.meshes.remove(baked)
    for obj in (obj_a, obj_b):
        mesh = obj.data
        bpy.data.objects.remove(obj, do_unlink=True)
        bpy.data.meshes.remove(mesh)
    return out


def g_fur_ring(t, cu, cv, ru, rv, count, lump, jitter=0.012, rows=2):
    """Couronne de fourrure : des MÈCHES fines, allongées, sur deux rangs décalés.

    Peu de grosses billes rondes lisent « cailloux » (rendu v8). Ce qui fait
    lire « poil » : beaucoup d'éléments, étirés dans l'axe de la retombée,
    de tailles très inégales, sur des rangs qui se recouvrent.

    Irrégularité par `sin` d'harmoniques non entières plutôt que par RNG — le
    build doit rester reproductible bit à bit.
    """
    tufts = []
    for row in range(rows):
        shrink = 1.0 - row * 0.09
        for i in range(count):
            a = 2.0 * pi * i / count + row * pi / count
            w = lump * (0.52 + 0.58 * sin(a * 3.7 + 1.1 + row * 2.1))
            x = cu + ru * shrink * cos(a)
            y = cv + rv * shrink * sin(a)
            z = t + jitter * sin(a * 5.3 + row) - row * lump * 0.85
            drop = lump * (1.05 + 0.5 * sin(a * 2.9 + row))
            # Touffe ARRONDIE et JOINTIVE : l'espacement est inférieur au
            # diamètre, donc les mèches se recouvrent et forment une bande —
            # c'est l'ombre ENTRE elles qui fait la fourrure. En billes isolées
            # ça lit « cailloux » (v9), en pointes ça lit « crocs » (v10).
            tufts.append(g_tube("Z", [
                (z + w * 0.70, x, y, w * 0.70, w * 0.70),
                (z + w * 0.10, x, y, w, w),
                (z - drop * 0.55, x * 1.03, y * 1.03, w * 0.82, w * 0.82),
                (z - drop, x * 1.05, y * 1.05, w * 0.42, w * 0.42),
            ], n=5))
    return g_merge(*tufts)


def g_ring_blocks(z_lo, z_hi, ru, rv, count, half_w, out, sink=0.030, phase=0.0, stagger=0.0):
    """Motif de créneaux en relief réparti sur un anneau elliptique.

    Boîtes NON chanfreinées volontairement : à cette taille le biseau est
    invisible, et l'arête vive donne le contraste franc qui survit au toon —
    un motif peint, lui, disparaîtrait dans la quantification.

    Le rayon est calculé sur l'ELLIPSE : à rayon constant, les blocs de côté
    s'enfoncent dans la plaque et ceux de devant flottent.
    """
    parts = []
    for i in range(count):
        a = 2.0 * pi * i / count + phase
        r = ru * rv / sqrt((rv * sin(a)) ** 2 + (ru * cos(a)) ** 2)
        dz = stagger if i % 2 else 0.0
        parts.append(g_xform(
            g_box((-half_w, r - sink, z_lo + dz), (half_w, r + out, z_hi + dz)),
            Matrix.Rotation(a, 4, "Z"),
        ))
    return g_merge(*parts)


def g_braid(z_top, z_bot, cx, cy_top, cy_bot, wind_r, strand_r, turns=2.4, steps=16, n=7):
    """Tresse RÉELLE : trois brins hélicoïdaux enroulés autour d'un axe.

    Des tubes droits côte à côte ne produisent jamais un tressage — une tresse
    est de la géométrie EN BRINS, même famille que la fourrure. Ici les brins
    s'enroulent pour de bon, et c'est le croisement qui fait la lecture.

    L'enroulement est aplati en Y (×0.55) : à section circulaire, la tresse
    s'enfonce dans la masse de la barbe et le tressage disparaît de face.
    """
    strands = []
    for k in range(3):
        phase = 2.0 * pi * k / 3.0
        sections = []
        for i in range(steps + 1):
            f = i / steps
            angle = phase + turns * 2.0 * pi * f
            taper = 1.0 - 0.45 * f
            sections.append((
                z_top + (z_bot - z_top) * f,
                cx + wind_r * taper * cos(angle),
                cy_top + (cy_bot - cy_top) * f + wind_r * taper * sin(angle) * 0.55,
                strand_r * taper,
                strand_r * taper,
            ))
        strands.append(g_tube("Z", sections, n=n))
    return g_merge(*strands)


def g_cog(center, radius, half_thick, teeth, tooth_w=0.0055):
    """Roue dentée face à +Y — motif signature de Torin, tressé dans la barbe."""
    cx, cy, cz = center
    parts = [g_tube("Y", [
        (cy - half_thick, cz, cx, radius, radius),
        (cy + half_thick, cz, cx, radius, radius),
    ], n=16)]
    for i in range(teeth):
        a = 2.0 * pi * i / teeth
        tooth = g_box(
            (radius - 0.006, -half_thick, -tooth_w), (radius + 0.010, half_thick, tooth_w)
        )
        parts.append(g_xform(
            tooth, Matrix.Translation(Vector((cx, cy, cz))) @ Matrix.Rotation(-a, 4, "Y")
        ))
    return g_merge(*parts)


def g_hand(x_wrist, pad=0.0):
    return g_merge(*g_hand_parts(x_wrist, pad))


def g_hand_parts(x_wrist, pad=0.0):
    """Main trapue : paume, rouleau de phalanges, 4 doigts de longueurs
    différentes, pouce opposé. Construite en T-pose (axe X).

    Renvoie les coques SÉPARÉMENT pour permettre une union booléenne — fusionnées
    d'office, elles ne peuvent plus être fondues en une surface continue.

    Partagée par le corps (`pad=0`) et par le gant (`pad>0`, coque gonflée) :
    une seule source de vérité garantit que le gant COUVRE la main qu'il masque.
    """
    parts = [
        g_box_c(
            (x_wrist + 0.008, -0.048 - pad, Z_SHOULDER - 0.040 - pad),
            (x_wrist + 0.078, 0.048 + pad, Z_SHOULDER + 0.040 + pad),
            0.018,
        ),
        g_tube("X", [
            (x_wrist + 0.060, 0.0, Z_SHOULDER, 0.050 + pad, 0.044 + pad),
            (x_wrist + 0.084, 0.0, Z_SHOULDER, 0.047 + pad, 0.041 + pad),
        ], n=10),
        g_tube("X", [  # pouce — mêmes points que les os `HandThumb*`
            (x_wrist + THUMB[0][0], THUMB[0][1], Z_SHOULDER + THUMB[0][2], 0.023 + pad, 0.023 + pad),
            (x_wrist + THUMB[1][0], THUMB[1][1], Z_SHOULDER + THUMB[1][2], 0.021 + pad, 0.021 + pad),
            (x_wrist + THUMB[2][0], THUMB[2][1], Z_SHOULDER + THUMB[2][2], 0.017 + pad, 0.017 + pad),
        ], n=8),
    ]
    x0 = x_wrist + HAND_KNUCKLE
    for _name, dy, length in FINGERS:
        parts.append(g_tube("X", [
            (x0 - 0.008, dy, Z_SHOULDER + 0.004, 0.019 + pad, 0.020 + pad),
            (x0 + length * 0.55, dy, Z_SHOULDER - 0.008, 0.018 + pad, 0.019 + pad),
            (x0 + length, dy, Z_SHOULDER - 0.024, 0.015 + pad, 0.016 + pad),
        ], n=8))
    return parts


def g_stud_line(start, end, count, radius):
    """Rivets alignés entre deux points (bordure de plaque, jointure)."""
    a, b = Vector(start), Vector(end)
    return g_merge(*[
        g_ellipsoid(tuple(a.lerp(b, i / max(1, count - 1))), (radius, radius, radius), n=6, rings=3)
        for i in range(count)
    ])


# ============================================================================
# SCÈNE : matériaux, objets, armature, skinning
# ============================================================================


def wipe_scene():
    for obj in list(bpy.data.objects):
        bpy.data.objects.remove(obj, do_unlink=True)
    for mesh in list(bpy.data.meshes):
        bpy.data.meshes.remove(mesh)
    for mat in list(bpy.data.materials):
        bpy.data.materials.remove(mat)


def build_materials():
    mats = {}
    for name, (rgba, metallic, roughness) in PALETTE.items():
        mat = bpy.data.materials.new(f"dwarf_{name}")
        mat.use_nodes = True
        # chercher par TYPE, pas par nom : le node s'appelle autrement selon la locale
        bsdf = next(n for n in mat.node_tree.nodes if n.type == "BSDF_PRINCIPLED")
        bsdf.inputs["Base Color"].default_value = rgba
        bsdf.inputs["Metallic"].default_value = metallic
        bsdf.inputs["Roughness"].default_value = roughness
        if name in EMISSIVE:
            bsdf.inputs["Emission Color"].default_value = rgba
            bsdf.inputs["Emission Strength"].default_value = EMISSIVE[name]
        mat.diffuse_color = rgba
        mats[name] = mat
    return mats


def make_object(name, chunks, chamfer=0.0, subsurf=0):
    """`chunks` = [(geom, material)] → un objet, un slot matériau par matériau."""
    mats, verts, faces, face_mat = [], [], [], []
    for geom, mat in chunks:
        if mat not in mats:
            mats.append(mat)
        idx, off = mats.index(mat), len(verts)
        verts.extend(geom[0])
        for f in geom[1]:
            faces.append(tuple(i + off for i in f))
            face_mat.append(idx)

    bm = bmesh.new()
    bv = [bm.verts.new(v) for v in verts]
    bm.verts.index_update()
    for f, mi in zip(faces, face_mat):
        try:
            bm.faces.new([bv[i] for i in f]).material_index = mi
        except ValueError:
            pass  # face doublon (géométries qui partagent des verts) — ignorée
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces[:])
    if chamfer > 0.0:
        bmesh.ops.bevel(
            bm, geom=bm.verts[:] + bm.edges[:], offset=chamfer, segments=2,
            profile=0.5, affect="EDGES", clamp_overlap=True,
        )
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    for mat in mats:
        mesh.materials.append(mat)
    for poly in mesh.polygons:
        poly.use_smooth = True
    mesh.validate()
    mesh.update()

    obj = bpy.data.objects.new(name, mesh)
    bpy.context.scene.collection.objects.link(obj)
    if subsurf > 0:
        apply_subsurf(obj, subsurf)
    return obj


def apply_subsurf(obj, levels):
    """Applique via le depsgraph : `modifier_apply` exige un contexte UI."""
    mod = obj.modifiers.new("subsurf", "SUBSURF")
    mod.levels = mod.render_levels = levels
    bpy.context.view_layer.update()
    deps = bpy.context.evaluated_depsgraph_get()
    baked = bpy.data.meshes.new_from_object(obj.evaluated_get(deps))
    obj.modifiers.clear()
    old = obj.data
    obj.data = baked
    bpy.data.meshes.remove(old)


def build_armature():
    arm_data = bpy.data.armatures.new("dwarf_rig")
    arm_obj = bpy.data.objects.new("dwarf_rig", arm_data)
    bpy.context.scene.collection.objects.link(arm_obj)
    bpy.context.view_layer.objects.active = arm_obj
    bpy.ops.object.mode_set(mode="EDIT")
    for name, (parent, head, tail, _) in SKELETON.items():
        bone = arm_data.edit_bones.new(name)
        bone.head, bone.tail = Vector(head), Vector(tail)
        if parent:
            bone.parent = arm_data.edit_bones[parent]
            bone.use_connect = bone.parent.tail == bone.head
    bpy.ops.object.mode_set(mode="OBJECT")
    return arm_obj


def _dist_to_segment(p, a, b):
    ab = b - a
    denom = ab.length_squared
    if denom < 1e-9:
        return (p - a).length
    t = max(0.0, min(1.0, (p - a).dot(ab) / denom))
    return (p - (a + ab * t)).length


def skin(obj, arm_obj, bone_names, max_influences=4):
    """Poids par enveloppe : w = (1 - d/rayon)², top 4, normalisé.

    Prévisible et sans échec, contrairement au bone-heat qui plante dès que des
    primitives s'interpénètrent — ce qui est le cas partout ici.
    """
    envelopes = [
        (name, Vector(SKELETON[name][1]), Vector(SKELETON[name][2]), SKELETON[name][3])
        for name in bone_names
        if SKELETON[name][3] > 0.0
    ]
    groups = {name: obj.vertex_groups.new(name=name) for name, _, _, _ in envelopes}

    for vert in obj.data.vertices:
        scored = []
        for name, head, tail, radius in envelopes:
            dist = _dist_to_segment(vert.co, head, tail)
            if dist < radius:
                scored.append((name, (1.0 - dist / radius) ** 2))
        if not scored:  # hors de toute enveloppe → rattaché à l'os le plus proche
            name = min(envelopes, key=lambda e: _dist_to_segment(vert.co, e[1], e[2]))[0]
            scored = [(name, 1.0)]
        scored.sort(key=lambda kv: kv[1], reverse=True)
        scored = scored[:max_influences]
        total = sum(w for _, w in scored)
        for name, weight in scored:
            groups[name].add([vert.index], weight / total, "REPLACE")

    obj.parent = arm_obj
    obj.matrix_parent_inverse = arm_obj.matrix_world.inverted()
    obj.modifiers.new("Armature", "ARMATURE").object = arm_obj


# ============================================================================
# CORPS — le « sous-vêtement » de base, façon modèle nu WoW (tunique + braies)
# ============================================================================


def sculpt_folds(geom, z_top, z_bot, ru, rv, count, radius, depth, phase=0.0, drift=0.35):
    """Plis d'étoffe : des SILLONS creusés, qui dérivent en descendant.

    Meilleur que la modulation de rayon de `g_cloth` : celle-ci produit des
    cannelures régulières, alors qu'un pli réel serpente et se referme. Les deux
    se cumulent — ondulation de fond + plis marqués par-dessus.
    """
    for i in range(count):
        a0 = phase + 2.0 * pi * i / count
        a1 = a0 + drift * (1.0 if i % 2 else -1.0)
        geom = s_crease(
            geom,
            (ru * cos(a0), rv * sin(a0), z_top),
            (ru * cos(a1), rv * sin(a1), z_bot),
            radius, depth,
        )
    return geom


def sculpt_wear(geom, ru, rv, z_lo, z_hi, count, radius, depth, seed=0.0):
    """Bosses et éraflures : une armure de forgeron ne sort pas neuve de la malle.

    Dispersion par suite du nombre d'or plutôt que par RNG — le build doit
    rester reproductible bit à bit.
    """
    for i in range(count):
        angle = 2.0 * pi * ((i * 0.61803 + seed) % 1.0)
        f = (i * 0.4370 + seed * 0.31) % 1.0
        geom = s_inflate(
            geom,
            (ru * cos(angle), rv * sin(angle), z_lo + (z_hi - z_lo) * f),
            radius, -depth,
        )
    return geom


def sculpt_face(face):
    """Passe de sculpt du visage, d'après la référence Torin.

    L'union donne une surface continue mais LISSE ; ce sont ces coups de brosse
    qui donnent l'anatomie — pommette saillante, orbite creusée, sillon
    nasogénien, tempe rentrée. C'est la différence entre un volume et un visage.

    Densifié d'abord : une brosse ne crée pas de détail plus fin que la maille.
    """
    face = g_subdivide(face, 1)

    # ORBITES creusées à la brosse : un bassin doux que le globe vient remplir.
    # Remplace le booléen, qui laissait un bord franc plus large que l'œil.
    for sx in (1.0, -1.0):
        face = s_inflate(face, (sx * EYE_X, 0.150, EYE_Z), 0.062, -0.0165)
    # ombre portée de l'arcade juste au-dessus des orbites
    face = s_crease(face, (-0.112, 0.118, EYE_Z + 0.040), (0.112, 0.118, EYE_Z + 0.040), 0.024, 0.0050)
    # le nez fondait au relax : on le repousse en avant
    face = s_grab(face, (0.0, 0.190, 1.226), 0.052, (0.0, 0.014, 0.0))
    # CASSURES DE PLANS (Asaro) : ce sont elles qui créent la valeur
    for sx in (1.0, -1.0):
        # arête front → tempe : sépare le plan frontal du plan latéral
        face = s_crease(face, (sx * 0.132, 0.086, 1.360), (sx * 0.156, 0.028, 1.286), 0.024, 0.0045)
        # arête pommette → mâchoire : sépare la joue avant de la joue latérale
        face = s_crease(face, (sx * 0.138, 0.078, 1.212), (sx * 0.112, 0.048, 1.116), 0.026, 0.0050)
        # bord de la boîte du museau (nez/lèvre supérieure)
        face = s_crease(face, (sx * 0.052, 0.150, 1.196), (sx * 0.058, 0.132, 1.152), 0.020, 0.0038)
    # glabelle : la ride verticale entre les sourcils, qui donne l'air buté
    face = s_crease(face, (0.0, 0.152, 1.298), (0.0, 0.146, 1.342), 0.014, 0.0040)
    # rides du front
    face = s_crease(face, (-0.100, 0.132, 1.362), (0.100, 0.132, 1.362), 0.015, 0.0032)
    face = s_crease(face, (-0.086, 0.124, 1.390), (0.086, 0.124, 1.390), 0.013, 0.0026)

    for sx in (1.0, -1.0):
        # pommette saillante, puis creux juste dessous : c'est le contraste des
        # deux qui fait lire l'os, pas la bosse seule
        face = s_inflate(face, (sx * 0.112, 0.108, 1.216), 0.055, 0.0090)
        face = s_inflate(face, (sx * 0.100, 0.116, 1.158), 0.046, -0.0060)
        # tempe rentrée
        face = s_inflate(face, (sx * 0.150, 0.020, 1.312), 0.050, -0.0070)
        # sillon nasogénien
        face = s_crease(
            face, (sx * 0.048, 0.180, 1.202), (sx * 0.084, 0.140, 1.142), 0.026, 0.0050
        )
    # menton poussé vers l'avant
    return s_grab(face, (0.0, 0.128, 1.092), 0.055, (0.0, 0.010, -0.004))


def build_body(m):
    parts = {}

    # -- tête : crâne, arcade, gros nez, oreilles, yeux ----------------------
    # Toute la lisibilité du visage tient à une bande libre entre le bord du
    # casque (haut) et la barbe (bas) : les yeux et le nez vivent dedans.
    # crâne construit en PLANS (menton, mâchoire, pommettes, tempes, front) et
    # non en œuf : c'est la structure qui distingue un visage d'une bille
    # La mâchoire et les pommettes sont SCULPTÉES DANS le profil du tube, pas
    # ajoutées en boules par-dessus : des masses rapportées sur un crâne lisse
    # ressortent en verrues au lieu de structurer le visage (rendu v8).
    # Section circulaire, volontairement : une section polygonale uniforme
    # (essayée, planar=0.55) facette TOUT le crâne et donne un sac froissé. Les
    # cassures de plan d'Asaro sont des lignes anatomiques PRÉCISES — elles sont
    # portées par les `s_crease` de `sculpt_face`, pas par la section du tube.
    # CRÂNE RESSERRÉ au-dessus de la ligne des yeux. La calotte précédente
    # restait large jusqu'au front puis fermait d'un coup : un dôme ballon qui
    # écrasait le visage dans le tiers inférieur. Ici elle se referme
    # progressivement, et le point le plus large redevient les POMMETTES —
    # c'est ce qui fait qu'on lit un visage plutôt qu'une tête d'œuf.
    skull = g_tube("Z", [
        (1.070, 0, 0.020, 0.104, 0.100),  # menton
        (1.104, 0, 0.014, 0.150, 0.144),  # mâchoire
        (1.150, 0, 0.008, 0.174, 0.162),  # bajoues
        (1.198, 0, 0.004, 0.180, 0.166),  # pommettes — le point le plus large
        (1.258, 0, -0.002, 0.174, 0.160),  # ligne des yeux
        (1.320, 0, -0.008, 0.168, 0.154),  # front
        (1.398, 0, -0.012, 0.134, 0.124),  # calotte — volontairement RONDE :
        (1.446, 0, -0.016, 0.060, 0.056),  # resserrée, elle rendait le crâne conique
    ])
    # ORBITES CREUSÉES : booléen sur le crâne SEUL, qui est un manifold propre.
    # Les masses rapportées sont fusionnées après, sinon le solveur patine sur
    # des coquilles qui s'interpénètrent.
    # PLUS DE BOOLÉEN D'ORBITE. Le creux booléen laissait un bord franc plus
    # large que le globe : trous visibles tout autour, invisibles en plan pied
    # mais criants en gros plan. L'orbite est désormais CREUSÉE À LA BROSSE dans
    # `sculpt_face` — un creux doux, sans arête, que le globe vient remplir.
    brow = g_tube("X", [
        (-0.152, 0.050, 1.312, 0.036, 0.024),
        (-0.074, 0.104, 1.328, 0.046, 0.030),
        (0.000, 0.094, 1.332, 0.042, 0.028),
        (0.074, 0.104, 1.328, 0.046, 0.030),
        (0.152, 0.050, 1.312, 0.036, 0.024),
    ])
    # Nez SURDIMENSIONNÉ à la construction : l'union puis le relax le rabotent
    # d'environ un tiers. Le dessiner à sa taille finale le fait disparaître.
    nose = g_tube("Z", [
        (1.166, 0, 0.158, 0.058, 0.072),
        (1.216, 0, 0.178, 0.062, 0.080),
        (1.264, 0, 0.138, 0.046, 0.060),
        (1.300, 0, 0.092, 0.030, 0.042),
    ])
    # oreilles POINTUES et écartées, orientées vers le haut-dehors (réf. Torin)
    ears = g_sided(lambda: g_tube("X", [
        (0.148, -0.012, 1.228, 0.054, 0.060),
        (0.196, 0.000, 1.270, 0.040, 0.046),
        (0.234, 0.010, 1.312, 0.015, 0.019),
    ]))
    # PAUPIÈRES EN CALOTTES qui ENVELOPPENT le globe : même centre, rayon
    # légèrement supérieur, découpées en haut et en bas. La supérieure mord le
    # haut de l'iris — un œil entièrement dégagé donne le regard fixe et mort.
    # à peine plus grandes que le globe (+5 mm) : à +9 mm elles lisaient comme
    # des casquettes rigides posées sur l'œil
    lids_up = g_sided(lambda: g_ellipsoid(
        (EYE_X, 0.118, EYE_Z), (EYE_R + 0.005, 0.032, EYE_R + 0.005),
        n=18, rings=10, zmin=EYE_Z + 0.015,
    ))
    lids_low = g_sided(lambda: g_ellipsoid(
        (EYE_X, 0.118, EYE_Z), (EYE_R + 0.005, 0.032, EYE_R + 0.005),
        n=18, rings=10, zmax=EYE_Z - 0.023,
    ))
    # SOURCILS broussailleux : c'est ce qui donne un regard. Sans eux le visage
    # n'est qu'une arcade pâle et deux points sombres perdus dedans (rendu v9).
    # sourcils POSÉS SUR l'arcade et non devant : en avant du front ils lisaient
    # comme deux tranches orange flottantes
    eyebrows = g_sided(lambda: g_tube("X", [
        (0.030, 0.116, EYE_Z + 0.044, 0.019, 0.012),
        (EYE_X + 0.004, 0.130, EYE_Z + 0.050, 0.024, 0.015),
        (0.118, 0.108, EYE_Z + 0.040, 0.017, 0.010),
    ]))
    # globe APLATI en profondeur : une sphère pleine bombe hors du visage et
    # accroche mal dès qu'on le regarde de trois quarts
    whites = g_sided(lambda: g_ellipsoid((EYE_X, 0.118, EYE_Z), (EYE_R, 0.028, EYE_R), n=16, rings=9))
    pupils = g_sided(lambda: g_ellipsoid(
        (EYE_X + 0.002, 0.134, EYE_Z - 0.002), (0.021, 0.018, 0.021), n=14, rings=8
    ))
    # AUCUN relax ici — et c'est délibéré. Le modificateur Smooth est un outil
    # GLOBAL : il adoucit bien les coutures de l'union, mais il aplatit du même
    # coup le nez, l'arcade et les pommettes, puis regomme les coups de brosse.
    # La subdivision Catmull-Clark adoucit les mêmes coutures SANS toucher aux
    # grandes formes — c'est elle qui doit faire ce travail.
    face = sculpt_face(g_union(skull, brow, nose, ears))
    parts["head"] = make_object(
        "head",
        [
            (g_merge(face, lids_up, lids_low), m["skin"]),
            (eyebrows, m["hair"]),
            (whites, m["eye_white"]),
            (pupils, m["eye"]),
        ],
        subsurf=SUBSURF_BODY,
    )

    # -- pilosité : CHAUVE sur le dessus, couronne à la nuque (réf. Torin) ----
    nape = g_ellipsoid((0, -0.106, 1.196), (0.144, 0.098, 0.130), rings=7)
    side_tufts = g_sided(lambda: g_ellipsoid((0.146, -0.038, 1.220), (0.050, 0.074, 0.074), rings=7))
    parts["hair"] = make_object(
        "hair", [(g_merge(nape, side_tufts), m["hair"])], subsurf=SUBSURF_BODY
    )

    # -- lunettes de forge relevées sur le front (masquées par le casque) -----
    strap = g_tube("Z", [
        (1.334, 0, -0.004, R_HEAD_U + 0.016, R_HEAD_V + 0.016),
        (1.372, 0, -0.004, R_HEAD_U + 0.014, R_HEAD_V + 0.014),
    ])
    # plus petites et bien moins profondes : à 10 cm de diamètre sur 5 cm de
    # profondeur, elles lisaient comme deux boules de laiton posées sur le front
    rims = g_sided(lambda: g_tube("Y", [
        (0.128, 1.353, 0.072, 0.040, 0.040),
        (0.164, 1.353, 0.072, 0.037, 0.037),
    ], n=16))
    lenses = g_sided(lambda: g_tube("Y", [
        (0.148, 1.353, 0.072, 0.031, 0.031),
        (0.158, 1.353, 0.072, 0.031, 0.031),
    ], n=16))
    parts["goggles"] = make_object(
        "goggles", [(g_merge(strap, rims), m["brass"]), (lenses, m["glass"])]
    )

    # -- barbe : la signature du nain — moustache séparée, taille en V, tresses
    # moustache : sausage HORIZONTAL sous le nez — en tube vertical elle fusionnait
    # avec la masse et le visage perdait son point d'accroche
    mustache = g_tube("X", [
        (-0.104, 0.070, 1.150, 0.034, 0.030),
        (-0.042, 0.106, 1.170, 0.046, 0.040),
        (0.042, 0.106, 1.170, 0.046, 0.040),
        (0.104, 0.070, 1.150, 0.034, 0.030),
    ])
    # masse : moins avancée (front à y≈0.155 au lieu de 0.208) et taillée en V,
    # sinon elle mange le plastron et lit « bavoir »
    # rallongée jusqu'à mi-poitrine (réf. Torin) : elle s'arrêtait au-dessus du
    # plastron et lisait « bavette » au lieu de « barbe de forgeron »
    # la masse s'arrête à mi-parcours et CÈDE LA PLACE aux tresses : en
    # descendant aussi bas qu'elles, elle les recouvrait entièrement et seules
    # les pointes dépassaient
    mane = g_tube("Z", [
        (1.160, 0, 0.014, 0.150, 0.136),
        (1.100, 0, 0.028, 0.172, 0.150),
        (1.040, 0, 0.036, 0.168, 0.144),
        (0.995, 0, 0.038, 0.140, 0.120),
        (0.968, 0, 0.036, 0.096, 0.084),
    ])
    # TROIS grosses tresses — une centrale, deux latérales — chacune sertie
    # d'un engrenage de laiton (réf. Torin). Les deux tresses fines à anneaux
    # d'or lisaient « breloque » ; ici c'est un attribut de forgeron.
    # BOUCHE : creusée au booléen DANS la masse de barbe, sous la moustache.
    # Sans découpe elle serait noyée dedans — le bas du visage est entièrement
    # couvert par la barbe à partir de z=1.160.
    # placée SOUS la moustache, dont le bas descend à z=1.124 : au-dessus, elle
    # était entièrement recouverte et le booléen ne servait à rien
    mane = g_boolean(mane, g_ellipsoid((0, 0.186, 1.094), (0.056, 0.044, 0.024), rings=8))
    mouth = g_ellipsoid((0, 0.136, 1.094), (0.050, 0.028, 0.019), n=16, rings=8)

    side_braids = g_sided(
        lambda: g_braid(1.040, 0.858, 0.100, 0.106, 0.130, 0.025, 0.021, turns=2.2)
    )
    mid_braid = g_braid(1.030, 0.820, 0.0, 0.130, 0.152, 0.030, 0.026, turns=2.6)
    # engrenages POSÉS SUR les tresses et nettement plus petits : centrés devant
    # la masse et surdimensionnés, ils masquaient toute la barbe
    # replacés sur le relief réel des nouvelles tresses, qui saillent davantage
    # posés PILE à la jonction masse/tresse : ils tiennent la tresse et masquent
    # la couture, exactement le rôle qu'ils ont sur la référence
    cogs = g_merge(
        g_cog((0.0, 0.190, 0.978), 0.040, 0.010, 10),
        g_cog((0.100, 0.158, 0.995), 0.027, 0.008, 8),
        g_cog((-0.100, 0.158, 0.995), 0.027, 0.008, 8),
    )
    # moyeu sombre au centre : c'est LE signe qui distingue un engrenage d'une
    # fleur. Sans lui, un disque à dents lit comme une corolle.
    hubs = g_merge(
        g_tube("Y", [(0.188, 0.978, 0.0, 0.015, 0.015), (0.206, 0.978, 0.0, 0.015, 0.015)], n=10),
        g_tube("Y", [(0.156, 0.995, 0.100, 0.010, 0.010), (0.172, 0.995, 0.100, 0.010, 0.010)], n=8),
        g_tube("Y", [(0.156, 0.995, -0.100, 0.010, 0.010), (0.172, 0.995, -0.100, 0.010, 0.010)], n=8),
    )
    parts["beard"] = make_object(
        "beard",
        [
            (g_merge(mane, mustache, side_braids, mid_braid), m["hair"]),
            (mouth, m["mouth"]),
            (cogs, m["brass"]),
            (hubs, m["steel_dark"]),
        ],
        subsurf=SUBSURF_BODY,
    )

    # -- torse : tonneau + cou + bras supérieurs (masqués par le plastron) ---
    barrel = g_cloth("Z", [
        (0.600, 0, 0, 0.180, 0.135),
        (Z_WAIST, 0, 0, 0.185, 0.135),
        (0.790, 0, 0, 0.205, 0.142),
        (0.860, 0, 0, 0.230, 0.150),
        (Z_CHEST, 0, 0, 0.255, 0.165),
        (Z_SHOULDER, 0, 0, 0.240, 0.150),
        (1.052, 0, 0, 0.150, 0.115),
    ], folds=9, depth=0.042)
    # ondulation de fond (g_cloth) + plis marqués creusés par-dessus
    barrel = sculpt_folds(g_subdivide(barrel, 1), 1.000, 0.620, 0.226, 0.152, 6, 0.030, 0.0055)
    neck = g_tube("Z", [(1.030, 0, 0, 0.082, 0.078), (1.135, 0, 0, 0.075, 0.072)])
    deltoid_one = g_ellipsoid((SHOULDER_X, 0, Z_SHOULDER), (0.092, 0.090, 0.088), rings=7)
    # Le bras supérieur se TERMINE EN FUSEAU au-delà du coude, pour finir enfoui
    # dans l'avant-bras qui l'englobe. Pas de boule de coude : une sphère de
    # rayon voisin d'un cylindre lui est tangente sur toute une zone → dents de
    # scie (rendu v3). Ici les deux surfaces se croisent en biais, net.
    upperarm_one = g_tube("X", [
        (SHOULDER_X - 0.030, 0, Z_SHOULDER, R_UPPERARM, R_UPPERARM),
        (SHOULDER_X + ARM_UPPER * 0.30, 0, Z_SHOULDER, 0.084, 0.080),  # biceps
        (SHOULDER_X + ARM_UPPER * 0.72, 0, Z_SHOULDER, 0.070, 0.068),
        (SHOULDER_X + ARM_UPPER + 0.045, 0, Z_SHOULDER, 0.056, 0.056),
    ])
    # épaule + bras fondus d'un côté, puis miroités : la couture deltoïde/biceps
    # était l'une des plus visibles du corps
    # sculpté en T-POSE (avant `g_arm`) : les coordonnées de brosse sont alors
    # celles du profil construit, pas celles du bras déjà rabattu
    arm_shape = g_subdivide(g_relax(g_union(deltoid_one, upperarm_one), 0.22, 2), 1)
    arm_shape = s_inflate(arm_shape, (SHOULDER_X + ARM_UPPER * 0.34, 0, Z_SHOULDER + 0.034), 0.068, 0.0105)
    arm_shape = s_inflate(arm_shape, (SHOULDER_X + ARM_UPPER * 0.36, 0, Z_SHOULDER - 0.036), 0.060, 0.0070)
    arm_shape = s_crease(
        arm_shape,
        (SHOULDER_X + 0.055, 0.052, Z_SHOULDER + 0.042),
        (SHOULDER_X + 0.115, 0.020, Z_SHOULDER + 0.014),
        0.026, 0.0042,  # insertion du deltoïde sur le biceps
    )
    arm_one = g_arm(arm_shape)
    arms = g_merge(arm_one, g_mirror_x(arm_one))
    parts["torso"] = make_object(
        "torso",
        [
            (barrel, m["tunic"]),
            (g_merge(neck, arms), m["skin"]),
        ],
        subsurf=SUBSURF_BODY,
    )

    # -- avant-bras + moufles (masqués par les gants) ------------------------
    def forearm_side():
        x0 = SHOULDER_X + ARM_UPPER
        # démarre fin (enfoui dans le bras supérieur) puis GROSSIT pour en sortir
        # franchement : le croisement des deux surfaces est transversal, donc net
        # galbe : renflement du brachio-radial puis effilement net au poignet.
        # Un tube de section constante lit « saucisse », jamais « bras ».
        fore = g_tube("X", [
            (x0 - 0.048, 0, Z_SHOULDER, 0.062, 0.062),
            (x0 - 0.005, 0, Z_SHOULDER, 0.076, 0.076),
            (x0 + ARM_FORE * 0.26, 0, Z_SHOULDER, 0.080, 0.078),
            (x0 + ARM_FORE * 0.62, 0, Z_SHOULDER, 0.066, 0.064),
            (x0 + ARM_FORE, 0, Z_SHOULDER, 0.052, 0.050),
        ])
        # avant-bras, paume et doigts FONDUS : les jointures se voyaient
        # (relax faible — les doigts sont fins et se rétracteraient)
        shape = g_subdivide(g_relax(g_union(fore, *g_hand_parts(x0 + ARM_FORE)), 0.16, 2), 1)
        shape = s_inflate(shape, (x0 + ARM_FORE * 0.26, 0, Z_SHOULDER + 0.024), 0.052, 0.0080)
        wrist = x0 + ARM_FORE
        for _name, dy, _length in FINGERS:  # bosses de phalanges sur le dos de la main
            shape = s_inflate(shape, (wrist + HAND_KNUCKLE - 0.004, dy, Z_SHOULDER + 0.032), 0.019, 0.0050)
        return g_arm(shape)

    parts["hands"] = make_object("hands", [(g_sided(forearm_side), m["skin"])], subsurf=SUBSURF_BODY)

    # -- bassin + cuisses (masqués par les jambières) ------------------------
    hips = g_cloth("Z", [
        (0.665, 0, 0, 0.184, 0.138),
        (0.580, 0, 0, 0.178, 0.134),
        (0.500, 0, 0, 0.166, 0.128),
    ], folds=7, depth=0.032)
    hips = sculpt_folds(g_subdivide(hips, 1), 0.650, 0.512, 0.176, 0.132, 5, 0.026, 0.0045, phase=0.4)
    thighs = g_sided(lambda: g_cloth("Z", [
        (0.606, LEG_X, 0.002, 0.104, 0.102),
        (0.520, LEG_X, 0.006, 0.106, 0.104),  # quadriceps
        (0.440, LEG_X, 0.004, 0.094, 0.094),
        (0.375, LEG_X, 0.000, 0.086, 0.086),
    ], folds=6, depth=0.030))
    parts["pelvis"] = make_object("pelvis", [(g_merge(hips, thighs), m["trouser"])], subsurf=SUBSURF_BODY)

    # -- genoux + tibias + pieds nus (masqués par les bottes) ----------------
    knees = g_sided(lambda: g_ellipsoid((LEG_X, 0, Z_KNEE), (0.090, 0.090, 0.078), rings=7))
    shins = g_sided(lambda: g_tube("Z", [
        (Z_KNEE + 0.010, LEG_X, 0.000, 0.086, 0.086),
        (0.290, LEG_X, -0.010, 0.094, 0.092),  # mollet, décalé vers l'arrière
        (0.200, LEG_X, 0.004, 0.076, 0.074),
        (Z_ANKLE, LEG_X, 0.010, 0.058, 0.056),
    ]))
    feet = g_sided(lambda: g_box_c(
        (LEG_X - FOOT_W / 2, -0.062, 0.0), (LEG_X + FOOT_W / 2, FOOT_LEN - 0.062, FOOT_H), 0.022
    ))
    parts["feet"] = make_object(
        "feet", [(g_merge(knees, shins), m["trouser"]), (feet, m["skin"])], subsurf=SUBSURF_BODY
    )
    return parts


# ============================================================================
# ARMURE — 5 slots, même repère de rest pose que le corps
# ============================================================================


def build_helmet(m):
    # Le bord passe AU-DESSUS des yeux (1.283) : sinon la bande dorée coupe le
    # regard en deux et le visage devient illisible.
    rim_z = Z_HEAD_C + 0.082
    dome = g_ellipsoid((0, 0, Z_HEAD_C + 0.008), (R_HEAD_U + 0.026, R_HEAD_V + 0.026, R_HEAD_W + 0.028), zmin=rim_z)
    band = g_tube("Z", [
        (rim_z - 0.030, 0, 0, R_HEAD_U + 0.034, R_HEAD_V + 0.034),
        (rim_z + 0.008, 0, 0, R_HEAD_U + 0.033, R_HEAD_V + 0.033),
    ])
    # le nasal passe DEVANT le nez (qui sort à y=0.238) et se recourbe vers le
    # dôme en montant — droit, il flotterait à 4 cm du casque en haut
    noseguard = g_tube("Z", [
        (1.196, 0, 0.236, 0.029, 0.020),
        (1.268, 0, 0.226, 0.031, 0.025),
        (1.320, 0, 0.192, 0.031, 0.029),
        (rim_z + 0.010, 0, 0.148, 0.031, 0.031),
    ])
    crest = g_box_c((-0.017, -0.120, Z_HEAD_TOP - 0.012), (0.017, 0.120, Z_HEAD_TOP + 0.048), 0.012)
    rivets = g_sided(lambda: g_ellipsoid((R_HEAD_U + 0.030, 0.052, rim_z - 0.018), (0.020, 0.020, 0.020), n=10, rings=6))
    vents = g_sided(lambda: g_box_c(
        (R_HEAD_U - 0.010, -0.086, rim_z + 0.038), (R_HEAD_U + 0.034, -0.028, rim_z + 0.062), 0.006
    ))
    band_studs = g_studs("Z", rim_z - 0.011, 0, 0, R_HEAD_U + 0.034, R_HEAD_V + 0.034, 10, 0.010)
    # frise gravée sur le dôme, juste au-dessus du bandeau
    dome_frieze = g_ring_blocks(1.356, 1.378, 0.173, 0.164, 12, 0.011, 0.008, sink=0.026)
    return make_object(
        "helmet",
        [
            (g_merge(dome, noseguard), m["steel"]),
            (g_merge(band, crest, rivets, dome_frieze), m["gold"]),
            (band_studs, m["steel_dark"]),
            (vents, m["ember"]),
        ],
    )


def _lame(z_bot, z_top, ru, rv, lip=0.008):
    """Une lame d'armure + son bord retourné.

    Le bord est ce qui donne l'ÉPAISSEUR : sans lui la plaque lit « coque de
    plastique », avec lui elle lit « tôle formée ». Il déborde de `lip` — assez
    pour croiser la lame en biais et pas l'affleurer.
    """
    plate = g_tube("Z", [(z_bot, 0, 0, ru, rv), (z_top, 0, 0, ru * 1.03, rv * 1.03)])
    rim = g_tube("Z", [
        (z_bot - 0.007, 0, 0, ru + lip, rv + lip * 0.72),
        (z_bot + 0.004, 0, 0, ru + lip, rv + lip * 0.72),
    ])
    return plate, rim


def build_chest(m):
    # Plastron EMPILÉ : un pectoral puis trois lames abdominales qui se
    # chevauchent en s'effilant vers la taille. Une coque continue d'un seul
    # tenant est le second signal « jouet » après le visage.
    pectoral = g_tube("Z", [
        (0.898, 0, 0, 0.258, 0.174),
        (Z_CHEST, 0, 0, 0.272, 0.182),
        (1.000, 0, 0, 0.262, 0.172),
        (Z_SHOULDER + 0.038, 0, 0, 0.162, 0.126),
    ])
    # bosses de martelage : un plastron de forgeron a servi
    pectoral = sculpt_wear(g_subdivide(pectoral, 1), 0.266, 0.176, 0.912, 1.012, 7, 0.038, 0.0045)
    pect_rim = g_tube("Z", [
        (0.891, 0, 0, 0.266, 0.181), (0.902, 0, 0, 0.266, 0.181),
    ])
    lame1, rim1 = _lame(0.848, 0.906, 0.252, 0.170)
    lame2, rim2 = _lame(0.796, 0.854, 0.238, 0.161)
    lame3, rim3 = _lame(0.742, 0.802, 0.222, 0.152)

    gorget = g_tube("Z", [(1.040, 0, 0, 0.104, 0.100), (1.096, 0, 0, 0.096, 0.092)])
    # col de fourrure : un nain WoW sans fourrure n'existe pas, et c'est le seul
    # élément SOUPLE de la panoplie — il casse le tout-dur
    collar = g_fur_ring(1.068, 0, 0, 0.180, 0.144, 26, 0.032)
    belt = g_tube("Z", [
        (Z_WAIST - 0.048, 0, 0, 0.206, 0.158),
        (Z_WAIST + 0.020, 0, 0, 0.204, 0.156),
    ])
    buckle = g_box_c((-0.052, 0.146, Z_WAIST - 0.040), (0.052, 0.176, Z_WAIST + 0.012), 0.012)

    # ASYMÉTRIE — une panoplie parfaitement symétrique lit « moule », pas
    # « équipement assemblé » : baudrier en diagonale + épaulière gauche en plus
    # sangle LARGE et PLATE (épaisseur 16 mm, largeur 42 mm) : en tube presque
    # rond elle lisait « bâton posé en travers » (rendu v8). Raccourcie aussi,
    # elle débordait dans le vide au-dessus de l'épaule.
    baldric = g_xform(
        g_tube("X", [
            (-0.168, 0.0, 0.0, 0.016, 0.042),
            (0.0, 0.014, 0.0, 0.018, 0.046),
            (0.168, 0.0, 0.0, 0.016, 0.042),
        ]),
        Matrix.Translation(Vector((-0.008, 0.180, 0.896))) @ Matrix.Rotation(radians(-38), 4, "Y"),
    )
    baldric_buckle = g_box_c((-0.030, 0.192, 0.856), (0.030, 0.212, 0.900), 0.008)

    def pauldron_lames(extra=False):
        lames = [
            g_ellipsoid((SHOULDER_X + 0.010, 0, Z_SHOULDER + 0.010), (0.112, 0.108, 0.076),
                        zmin=Z_SHOULDER - 0.020),
            g_ellipsoid((SHOULDER_X + 0.020, 0, Z_SHOULDER - 0.028), (0.126, 0.122, 0.082),
                        zmin=Z_SHOULDER - 0.062),
            g_ellipsoid((SHOULDER_X + 0.028, 0, Z_SHOULDER - 0.070), (0.136, 0.130, 0.086),
                        zmin=Z_SHOULDER - 0.104),
        ]
        if extra:
            lames.append(g_ellipsoid((SHOULDER_X + 0.036, 0, Z_SHOULDER - 0.112),
                                     (0.146, 0.138, 0.090), zmin=Z_SHOULDER - 0.148))
        return g_arm(g_merge(*lames))

    left = pauldron_lames(extra=True)
    pauldrons = g_merge(left, g_mirror_x(pauldron_lames(extra=False)))

    # MANCHES : le plastron masque `torso`, qui porte les bras supérieurs — sans
    # elles le nain équipé se retrouve manchot (bug vu au rendu v1)
    sleeves = g_sided(lambda: g_arm(g_tube("X", [
        (SHOULDER_X - 0.024, 0, Z_SHOULDER, R_UPPERARM + 0.026, R_UPPERARM + 0.026),
        (SHOULDER_X + ARM_UPPER * 0.30, 0, Z_SHOULDER, 0.096, 0.092),
        (SHOULDER_X + ARM_UPPER * 0.72, 0, Z_SHOULDER, 0.084, 0.082),
        (SHOULDER_X + ARM_UPPER + 0.034, 0, Z_SHOULDER, 0.078, 0.078),
    ])))
    forge_mark = g_box_c((-0.038, 0.178, Z_CHEST - 0.024), (0.038, 0.194, Z_CHEST + 0.014), 0.008)
    rib = g_tube("Z", [
        (0.905, 0, 0.158, 0.022, 0.022),
        (0.955, 0, 0.174, 0.026, 0.026),
        (1.002, 0, 0.156, 0.022, 0.022),
    ])
    studs = g_merge(
        g_studs("Z", 0.896, 0, 0, 0.266, 0.181, 12, 0.010),
        g_studs("Z", 0.850, 0, 0, 0.260, 0.177, 12, 0.010, phase=0.26),
        g_studs("Z", 0.744, 0, 0, 0.230, 0.159, 10, 0.009),
    )
    # FRISE gravée sur le pectoral : créneaux alternés encadrés de deux rails.
    # C'est ce qui fait lire « forgé par quelqu'un » plutôt que « généré ».
    frieze = g_ring_blocks(0.946, 0.978, 0.268, 0.179, 22, 0.013, 0.009, stagger=0.011)
    rails = g_merge(
        g_tube("Z", [(0.934, 0, 0, 0.2745, 0.1850), (0.944, 0, 0, 0.2740, 0.1845)]),
        g_tube("Z", [(0.986, 0, 0, 0.2680, 0.1790), (0.996, 0, 0, 0.2670, 0.1780)]),
    )
    belt_blocks = g_ring_blocks(0.716, 0.744, 0.208, 0.160, 18, 0.011, 0.007, sink=0.024)
    return make_object(
        "chest",
        [
            (g_merge(pectoral, lame1, lame2, lame3, pauldrons, rib), m["steel"]),
            (g_merge(gorget, studs, pect_rim, rim1, rim2, rim3, rails, belt_blocks), m["steel_dark"]),
            (g_merge(belt, sleeves), m["leather"]),
            (baldric, m["leather_light"]),
            (collar, m["fur"]),
            (g_merge(buckle, baldric_buckle, frieze), m["gold"]),
            (forge_mark, m["ember"]),
        ],
    )


def build_gloves(m):
    # GANTELET COMPLET coude→poignet : les gants masquent `hands`, qui porte les
    # avant-bras. Une simple manchette laissait le bras coupé en deux (rendu v1).
    def side():
        x0 = SHOULDER_X + ARM_UPPER
        # élargi : l'avant-bras du corps est passé au galbe (pic 0.080), une
        # coque à 0.078 le laissait transpercer le gantelet
        return g_arm(g_tube("X", [
            (x0 - 0.014, 0, Z_SHOULDER, 0.086, 0.086),
            (x0 + ARM_FORE * 0.30, 0, Z_SHOULDER, 0.090, 0.088),
            (x0 + ARM_FORE * 0.66, 0, Z_SHOULDER, 0.098, 0.096),
            (x0 + ARM_FORE * 0.86, 0, Z_SHOULDER, 0.076, 0.074),
            (x0 + ARM_FORE, 0, Z_SHOULDER, 0.068, 0.066),
        ]))

    def cuff_rim():
        x0 = SHOULDER_X + ARM_UPPER
        return g_arm(g_tube("X", [
            (x0 + ARM_FORE * 0.62, 0, Z_SHOULDER, 0.106, 0.104),
            (x0 + ARM_FORE * 0.70, 0, Z_SHOULDER, 0.104, 0.102),
        ]))

    def mitt():
        # MÊME générateur que la main nue, coque gonflée de 10 mm : le gant
        # couvre par construction ce qu'il masque, doigts compris
        return g_arm(g_hand(SHOULDER_X + ARM_UPPER + ARM_FORE, pad=0.010))

    def knuckles():
        x0 = SHOULDER_X + ARM_UPPER + ARM_FORE
        return g_arm(g_box_c(
            (x0 + 0.026, -0.050, Z_SHOULDER + 0.018), (x0 + 0.090, 0.048, Z_SHOULDER + 0.072), 0.012
        ))

    def knuckle_studs():
        x0 = SHOULDER_X + ARM_UPPER + ARM_FORE
        return g_arm(g_stud_line(
            (x0 + 0.040, 0.0, Z_SHOULDER + 0.068), (x0 + 0.080, 0.0, Z_SHOULDER + 0.068), 3, 0.009
        ))

    return make_object(
        "gloves",
        [
            (g_sided(mitt), m["leather"]),
            (g_sided(side), m["steel"]),
            (g_sided(knuckles), m["steel_dark"]),
            (g_merge(g_sided(cuff_rim), g_sided(knuckle_studs)), m["gold"]),
        ],
    )


def build_legs(m):
    tassets = g_cloth("Z", [
        (Z_HIP + 0.030, 0, 0, 0.200, 0.152),
        (0.560, 0, 0, 0.228, 0.174),
        (0.478, 0, 0, 0.238, 0.182),
    ], folds=10, depth=0.038)
    tassets = sculpt_folds(g_subdivide(tassets, 1), 0.642, 0.488, 0.220, 0.168, 7, 0.028, 0.0060)
    # nettement plus large que le bas des tassets (0.238) : à 4 mm d'écart les
    # deux surfaces étaient tangentes et z-fightaient (rendu v2)
    hem = g_tube("Z", [(0.480, 0, 0, 0.254, 0.198), (0.456, 0, 0, 0.250, 0.194)])
    # sommet sous l'ourlet de la jupe : plus haut/plus large, les plaques
    # transperçaient les tassets (petits triangles parasites au rendu v1)
    thigh_plates = g_sided(lambda: g_tube("Z", [
        (0.520, LEG_X, 0.004, 0.108, 0.106),
        (0.452, LEG_X, 0.006, 0.110, 0.108),
        (0.396, LEG_X, 0.004, 0.104, 0.102),
    ]))
    knee_caps = g_sided(lambda: g_ellipsoid((LEG_X, 0.012, Z_KNEE + 0.006), (0.098, 0.098, 0.084), rings=7))
    studs = g_sided(lambda: g_ellipsoid((LEG_X, 0.106, Z_KNEE + 0.010), (0.026, 0.026, 0.026), n=10, rings=6))
    # TASSETTES INDIVIDUELLES par-dessus la jupe de cuir : huit plaques
    # distinctes lisent « équipement assemblé », un cône continu lit « jupe ».
    # Rayon calculé sur l'ELLIPSE du buste, sinon les plaques de côté
    # s'enfoncent et celles de devant flottent.
    def tasset_parts():
        """Renvoie (plaques, cadres gravés, bossettes centrales)."""
        rx, ry = 0.248, 0.192
        plates, frames, bosses = [], [], []
        for i in range(8):
            a = 2.0 * pi * i / 8 + pi / 8
            r = rx * ry / sqrt((ry * sin(a)) ** 2 + (rx * cos(a)) ** 2)
            spin = Matrix.Rotation(a, 4, "Z")
            plates.append(g_xform(
                g_box_c((-0.054, r - 0.030, 0.466), (0.054, r + 0.014, 0.598), 0.010), spin
            ))
            # cadre en relief sur la face externe : quatre barres fines
            f0, f1 = r + 0.010, r + 0.021
            frames.append(g_xform(g_merge(
                g_box((-0.048, f0, 0.482), (-0.038, f1, 0.584)),
                g_box((0.038, f0, 0.482), (0.048, f1, 0.584)),
                g_box((-0.048, f0, 0.482), (0.048, f1, 0.492)),
                g_box((-0.048, f0, 0.574), (0.048, f1, 0.584)),
            ), spin))
            bosses.append(g_xform(
                g_ellipsoid((0.0, r + 0.008, 0.533), (0.019, 0.019, 0.019), n=8, rings=5), spin
            ))
        return g_merge(*plates), g_merge(*frames), g_merge(*bosses)

    tasset_plates, tasset_frames, tasset_bosses = tasset_parts()

    hem_studs = g_studs("Z", 0.468, 0, 0, 0.252, 0.196, 14, 0.010)
    thigh_straps = g_sided(lambda: g_tube("Z", [
        (0.446, LEG_X, 0.006, 0.120, 0.118),
        (0.470, LEG_X, 0.006, 0.120, 0.118),
    ]))
    return make_object(
        "legs",
        [
            (g_merge(tassets, thigh_straps), m["leather"]),
            (g_merge(thigh_plates, tasset_plates), m["steel"]),
            (g_merge(knee_caps, hem_studs, tasset_frames), m["steel_dark"]),
            (g_merge(hem, studs, tasset_bosses), m["gold"]),
        ],
    )


def build_boots(m):
    shells = g_sided(lambda: g_box_c(
        (LEG_X - FOOT_W / 2 - 0.016, -0.080, -0.004), (LEG_X + FOOT_W / 2 + 0.016, FOOT_LEN - 0.052, FOOT_H + 0.022), 0.026
    ))
    # ENGLOBE l'avant de la coque au lieu de l'affleurer à 6 mm : la coiffe
    # recouvre franchement le coin avant, croisement propre (rendu v3)
    toecaps = g_sided(lambda: g_box_c(
        (LEG_X - FOOT_W / 2 - 0.026, 0.116, -0.010), (LEG_X + FOOT_W / 2 + 0.026, FOOT_LEN - 0.038, FOOT_H - 0.004), 0.020
    ))
    # base resserrée : à r=0.086 le tibia affleurait exactement la coque de la
    # botte → intersection tangente en dents de scie (rendu v2)
    shins = g_sided(lambda: g_tube("Z", [
        (Z_ANKLE - 0.020, LEG_X, 0.004, 0.074, 0.074),
        (0.210, LEG_X, 0.008, 0.104, 0.102),
        (Z_KNEE - 0.030, LEG_X, 0.004, 0.108, 0.106),
    ]))
    cuffs = g_sided(lambda: g_tube("Z", [
        (Z_KNEE - 0.036, LEG_X, 0.004, 0.122, 0.120),
        (Z_KNEE - 0.008, LEG_X, 0.004, 0.118, 0.116),
    ]))
    straps = g_sided(lambda: g_tube("Z", [
        (0.176, LEG_X, 0.006, 0.116, 0.114), (0.196, LEG_X, 0.006, 0.114, 0.112),
    ]))
    # semelle : englobe franchement le bas de la coque (croisement transversal)
    soles = g_sided(lambda: g_box_c(
        (LEG_X - FOOT_W / 2 - 0.024, -0.090, -0.014), (LEG_X + FOOT_W / 2 + 0.024, FOOT_LEN - 0.044, 0.026), 0.012
    ))
    toe_studs = g_sided(lambda: g_stud_line(
        (LEG_X - 0.052, 0.150, FOOT_H - 0.010), (LEG_X + 0.052, 0.150, FOOT_H - 0.010), 4, 0.010
    ))
    # lame de cheville par-dessus la plaque de tibia : la superposition, encore
    ankle_lames = g_sided(lambda: g_tube("Z", [
        (0.128, LEG_X, 0.006, 0.100, 0.098),
        (0.196, LEG_X, 0.008, 0.112, 0.110),
    ]))
    ankle_rims = g_sided(lambda: g_tube("Z", [
        (0.120, LEG_X, 0.006, 0.108, 0.106),
        (0.132, LEG_X, 0.006, 0.108, 0.106),
    ]))
    fur_trim = g_sided(lambda: g_fur_ring(
        Z_KNEE - 0.012, LEG_X, 0.004, 0.122, 0.120, 16, 0.024, jitter=0.005
    ))
    return make_object(
        "boots",
        [
            (g_merge(shells, cuffs), m["leather"]),
            (g_merge(shins, ankle_lames), m["steel"]),
            (g_merge(toecaps, ankle_rims), m["steel_dark"]),
            (soles, m["sole"]),
            (fur_trim, m["fur"]),
            (g_merge(straps, toe_studs), m["gold"]),
        ],
    )


# ============================================================================
# EXPORT
# ============================================================================


def export_glb(objs, arm_obj, path, animations=False):
    """`animations` n'est vrai que pour le corps.

    Les pièces d'armure partagent le squelette au runtime : elles sont
    entraînées par lui. Y dupliquer les actions gonflerait les fichiers, et
    l'importeur glTF laisse la DERNIÈRE action importée active — toute preview
    d'un GLB animé rendrait alors une pose aléatoire.
    """
    for obj in bpy.data.objects:
        obj.select_set(False)
    for obj in list(objs) + [arm_obj]:
        obj.select_set(True)
    bpy.context.view_layer.objects.active = arm_obj
    bpy.ops.export_scene.gltf(
        filepath=str(path),
        export_format="GLB",
        use_selection=True,
        export_apply=False,
        export_yup=True,
        export_animations=animations,
        export_animation_mode="ACTIONS",
        export_skins=True,
        export_all_influences=False,  # ≤ 4 influences / vertex
        export_materials="EXPORT",
    )


def tri_count(obj):
    return sum(len(p.vertices) - 2 for p in obj.data.polygons)


def cli_args():
    if "--" not in sys.argv:
        raise SystemExit("Arguments attendus après `--`.")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--bake", action="store_true", help="cuire les cartes PBR procédurales")
    parser.add_argument("--no-anim", action="store_true", help="exporter sans les clips d'animation")
    parser.add_argument("--tex-body", type=int, default=1024)
    parser.add_argument("--tex-slot", type=int, default=512)
    return parser.parse_args(sys.argv[sys.argv.index("--") + 1 :])


def main():
    args = cli_args()
    out_dir = args.out.resolve()
    out_dir.mkdir(parents=True, exist_ok=True)

    wipe_scene()
    mats = build_materials()
    arm_obj = build_armature()

    body = build_body(mats)
    armor = {
        "helmet": build_helmet(mats),
        "chest": build_chest(mats),
        "gloves": build_gloves(mats),
        "legs": build_legs(mats),
        "boots": build_boots(mats),
    }
    for name, obj in list(body.items()) + list(armor.items()):
        skin(obj, arm_obj, INFLUENCES[name])

    if args.bake:
        body_objs = [body[p] for p in BODY_PARTS]
        armor_objs = [armor[s] for s in SLOTS]
        # Corps cuit SANS l'armure dans la scène : sinon l'ombre portée du
        # plastron resterait imprimée sur le torse nu.
        dwarf_texturing.bake_group(
            body_objs, [], out_dir, "body", PALETTE, set(EMISSIVE), args.tex_body
        )
        # Armure cuite AVEC le corps et le reste du set : le contact avec ce
        # qu'elle recouvre fait partie de son occlusion.
        for slot in SLOTS:
            dwarf_texturing.bake_group(
                [armor[slot]], body_objs + armor_objs, out_dir, slot,
                PALETTE, set(EMISSIVE), args.tex_slot,
            )

    clips = [] if args.no_anim else dwarf_anim.build_clips(arm_obj)

    export_glb([body[p] for p in BODY_PARTS], arm_obj, out_dir / "body.glb", animations=bool(clips))
    for slot in SLOTS:
        export_glb([armor[slot]], arm_obj, out_dir / f"{slot}.glb")

    manifest = {
        "_comment": "Généré par tools/blender/build_dwarf.py — ne pas éditer à la main.",
        "height_m": H_TOTAL,
        "forward": "-Z (glTF) — le nain regarde +Y en repère Blender",
        "skeleton": {
            "root": "Hips",
            "bone_count": len(SKELETON),
            "bones": list(SKELETON.keys()),
        },
        "body": {
            "glb": "body.glb",
            "parts": {p: {"tris": tri_count(body[p])} for p in BODY_PARTS},
        },
        # Les clips vivent UNIQUEMENT sur le corps : il porte le squelette que
        # les pièces d'armure suivent au runtime.
        "clips": {"glb": "body.glb", "fps": dwarf_anim.FPS, "names": clips},
        "slots": {
            s: {
                "glb": f"{s}.glb",
                "hides": HIDES[s],
                "bones": INFLUENCES[s],
                "tris": tri_count(armor[s]),
            }
            for s in SLOTS
        },
    }
    (out_dir / "manifest.json").write_text(json.dumps(manifest, indent=2, ensure_ascii=False), encoding="utf-8")

    body_tris = sum(tri_count(o) for o in body.values())
    armor_tris = sum(tri_count(o) for o in armor.values())
    print("=== DWARF BUILD ===")
    print(f"os          : {len(SKELETON)}")
    for part in BODY_PARTS:
        print(f"  body/{part:<8} {tri_count(body[part]):>6} tris")
    for slot in SLOTS:
        print(f"  slot/{slot:<8} {tri_count(armor[slot]):>6} tris")
    print(f"clips       : {clips or 'aucun'}")
    print(f"corps nu    : {body_tris} tris")
    print(f"set complet : {armor_tris} tris")
    print(f"équipé      : {body_tris + armor_tris} tris (avant masquage)")
    print(f"BUILD_OK -> {out_dir}")


if __name__ == "__main__":
    main()
