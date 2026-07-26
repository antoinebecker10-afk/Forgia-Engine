"""read_grip_markers.py — Lit les repères-TUBES MK_R/MK_L ajustés à la main dans
`calibration_<arme>.blend` (produit par preview_ingame.py) et retraduit :
  - leurs POSITIONS → valeurs `[viewmodel_arms]` de fps_tuning.toml
  - leurs AXES → ROLL_DEG suggérés pour cartoonize_arms.py (le C de la main
    s'enroule autour de l'axe du tube ; axe local du cylindre = Z)

Boucle utilisateur : ouvrir le .blend → placer/tourner MK_R (rouge, manche) /
MK_L (bleu, canon) comme les poignées réelles → Ctrl+S → lancer ce script.

Usage :
  blender --background <calibration.blend> --python tools/blender/read_grip_markers.py -- <weapon_key>
"""

import math
import os
import sys
import tomllib

import bpy
from mathutils import Quaternion, Vector

argv = sys.argv[sys.argv.index("--") + 1 :]
WEAPON_KEY = argv[0]
ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))

with open(os.path.join(ROOT, "assets/genomes/viewmodel_arena.toml"), "rb") as f:
    GENOME = tomllib.load(f)["weapons"][WEAPON_KEY]
with open(os.path.join(ROOT, "assets/genomes/fps_tuning.toml"), "rb") as f:
    ARMS = tomllib.load(f)["viewmodel_arms"]


# Blender (x,y,z) → Bevy (x, z, -y) — inverse du mapping de preview_ingame.
def to_bevy(v):
    return Vector((v.x, v.z, -v.y))


def suggested_roll_deg(anchor_bevy, axis_bevy, mirror):
    """Roulis (bake cartoonize ROLL_DEG) pour que l'axe X local de la main
    (l'axe autour duquel les doigts s'enroulent en C) s'aligne sur l'axe du
    tube, étant donné l'orientation in-game `from_rotation_arc(Y, coude→poignet)`."""
    if mirror > 0.0:
        elbow_out = ARMS["grip_elbow_out"]
    else:
        elbow_out = ARMS["barrel_elbow_out"]
    elbow = anchor_bevy + Vector(
        (mirror * elbow_out, -ARMS["elbow_drop"], ARMS["elbow_back"])
    )
    fwd = (anchor_bevy - elbow).normalized()
    arc = Vector((0, 1, 0)).rotation_difference(fwd)
    v = arc.inverted() @ axis_bevy.normalized()
    # Rotation autour de Y : X=(1,0,0) → (cosθ, 0, -sinθ).
    theta = math.atan2(-v.z, v.x)
    return math.degrees(theta)


gun = Vector((GENOME["offset_x"], GENOME["offset_y"], GENOME["offset_z"]))
length = GENOME["target_size"]

out_toml = []
out_roll = []
for name, mirror, keys in (
    ("MK_R", 1.0, ("grip_x", "grip_drop", "grip_back", 1.0)),
    ("MK_L", -1.0, ("barrel_x", "barrel_drop", "barrel_fwd", -1.0)),
):
    mo = bpy.data.objects.get(name)
    if mo is None:
        print(f"ERREUR : {name} introuvable — scène de calibration invalide")
        sys.exit(1)
    pos = to_bevy(mo.matrix_world.translation)
    axis = to_bevy(mo.matrix_world.col[2].to_3d())  # axe local Z du cylindre
    off = pos - gun
    kx, ky, kz, sign = keys
    out_toml.append(f"{kx} = {off.x:.3f}")
    out_toml.append(f"{ky} = {off.y:.3f}")
    out_toml.append(f"{kz} = {sign * off.z / length:.3f}")
    side = "R" if mirror > 0 else "L"
    out_roll.append(f'"{side}": {suggested_roll_deg(pos, axis, mirror):.0f}.0')
    a = axis.normalized()
    print(f'AXIS {side} = ({a.x:.3f}, {a.y:.3f}, {a.z:.3f})  # Bevy world')

print(f"# Ancres PAR-ARME pour viewmodel_arena.toml [weapons.{WEAPON_KEY}] :")
r_off = to_bevy(mk_r.matrix_world.translation) - gun
l_off = to_bevy(mk_l.matrix_world.translation) - gun
print(f"grip_anchor = [{r_off.x:.3f}, {r_off.y:.3f}, {r_off.z:.3f}]")
print(f"barrel_anchor = [{l_off.x:.3f}, {l_off.y:.3f}, {l_off.z:.3f}]")
print("# (fallback fractions [viewmodel_arms], si besoin) :")
for line in out_toml:
    print(line)
print("# ROLL_DEG suggérés (cartoonize_arms.py, nécessite un rebake) :")
print("ROLL_DEG = {" + ", ".join(out_roll) + "}")
