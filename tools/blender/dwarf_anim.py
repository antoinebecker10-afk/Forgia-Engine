"""Actions keyframées du nain — `idle` et `walk`, exportées dans body.glb.

Les poses clés sont écrites en degrés dans le repère de l'ARMATURE, jamais dans
le repère local des os : deviner l'axe local d'un os fait partir les bras
derrière le dos (bug vécu). `bone_local()` fait la conversion M⁻¹·R·M.

Les animations ne vivent QUE sur le corps. Les pièces d'armure partagent le même
squelette au runtime et sont donc entraînées par lui — leur mettre des actions
dupliquerait tout, et l'importeur glTF laisse la DERNIÈRE action importée active,
ce qui ferait rendre une pose aléatoire à chaque preview.

Cycles conçus « en place » (aucune avancée du root) : c'est le moteur qui déplace
le personnage, l'animation ne fait que le faire marcher sur lui-même.

Consommé par build_dwarf.py — pas d'usage direct en ligne de commande.
"""

import math

import bpy
from mathutils import Euler, Vector

FPS = 24

# Pose de repos servant de base à toutes les clés : chaque clé ne déclare que
# ses écarts. Repère armature, le nain regarde +Y → X positif = vers l'avant.
BASE = {
    "Spine": (-4.0, 0.0, 0.0),
    "Spine1": (-3.0, 0.0, 0.0),
    "Spine2": (-2.0, 0.0, 0.0),
    "Neck": (4.0, 0.0, 0.0),
    "Head": (2.0, 0.0, 0.0),
    "LeftShoulder": (0.0, 0.0, -4.0),
    "LeftArm": (14.0, 0.0, -8.0),
    "LeftForeArm": (28.0, 0.0, 0.0),
    "LeftHand": (8.0, 0.0, 0.0),
    "RightShoulder": (0.0, 0.0, 4.0),
    "RightArm": (14.0, 0.0, 8.0),
    "RightForeArm": (28.0, 0.0, 0.0),
    "RightHand": (8.0, 0.0, 0.0),
    "LeftUpLeg": (0.0, 0.0, -2.0),
    "LeftLeg": (-4.0, 0.0, 0.0),
    "RightUpLeg": (0.0, 0.0, 2.0),
    "RightLeg": (-4.0, 0.0, 0.0),
}

# --- idle : respiration + report de poids, 2 s ------------------------------
# Amplitudes volontairement faibles : un idle qui bouge trop lit « ivre ».
IDLE = [
    (1, {}, (0.0, 0.0, 0.000)),
    (13, {"Spine1": (-5.0, 0.0, 1.0), "Spine2": (-4.0, 0.0, 0.0),
          "Neck": (2.0, 0.0, 0.0), "Head": (0.0, 0.0, 3.0),
          "LeftShoulder": (0.0, 0.0, -6.0), "RightShoulder": (0.0, 0.0, 6.0),
          "LeftArm": (12.0, 0.0, -10.0), "RightArm": (12.0, 0.0, 10.0)},
     (0.0, 0.0, 0.008)),
    (25, {}, (0.0, 0.0, 0.000)),
    (37, {"Spine1": (-2.0, 0.0, -1.0), "Head": (3.0, 0.0, -3.0),
          "LeftArm": (16.0, 0.0, -7.0), "RightArm": (16.0, 0.0, 7.0)},
     (0.0, 0.0, -0.006)),
    (49, {}, (0.0, 0.0, 0.000)),
]

# --- walk : cycle 1 s, 4 poses clés + retour ---------------------------------
# Un nain marche lourd : pas courts, roulis d'épaules marqué, rebond du bassin
# deux fois par cycle (une fois par appui).
_CONTACT_L = {
    "LeftUpLeg": (24.0, 0.0, -2.0), "LeftLeg": (-10.0, 0.0, 0.0),
    "RightUpLeg": (-18.0, 0.0, 2.0), "RightLeg": (-26.0, 0.0, 0.0),
    "LeftArm": (-2.0, 0.0, -10.0), "LeftForeArm": (22.0, 0.0, 0.0),
    "RightArm": (30.0, 0.0, 10.0), "RightForeArm": (34.0, 0.0, 0.0),
    "Spine1": (-3.0, 0.0, -5.0), "Spine2": (-2.0, 0.0, 4.0),
    "Head": (2.0, 0.0, 3.0),
}
_PASS_L = {
    "LeftUpLeg": (4.0, 0.0, -2.0), "LeftLeg": (-8.0, 0.0, 0.0),
    "RightUpLeg": (-2.0, 0.0, 2.0), "RightLeg": (-42.0, 0.0, 0.0),
    "LeftArm": (6.0, 0.0, -9.0), "LeftForeArm": (26.0, 0.0, 0.0),
    "RightArm": (18.0, 0.0, 9.0), "RightForeArm": (30.0, 0.0, 0.0),
    "Spine1": (-3.0, 0.0, 0.0), "Spine2": (-2.0, 0.0, 0.0),
}


def _mirror(pose):
    """Symétrise une pose gauche/droite : X et Y gardés, Z inversé."""
    swap = {"Left": "Right", "Right": "Left"}
    out = {}
    for name, (rx, ry, rz) in pose.items():
        side = name[:5] if name[:5] in swap else name[:4] if name[:4] in swap else None
        if side:
            out[swap[side] + name[len(side):]] = (rx, ry, -rz)
        else:
            out[name] = (rx, -ry, -rz)
    return out


WALK = [
    (1, _CONTACT_L, (0.0, 0.0, -0.018)),
    (7, _PASS_L, (0.0, 0.0, 0.012)),
    (13, _mirror(_CONTACT_L), (0.0, 0.0, -0.018)),
    (19, _mirror(_PASS_L), (0.0, 0.0, 0.012)),
    (25, _CONTACT_L, (0.0, 0.0, -0.018)),
]

CLIPS = {"idle": IDLE, "walk": WALK}


def bone_local(pose_bone, angles_deg):
    """Rotation exprimée en repère ARMATURE → repère local de l'os."""
    rest = pose_bone.bone.matrix_local.to_3x3()
    world = Euler([math.radians(a) for a in angles_deg], "XYZ").to_matrix()
    return (rest.inverted() @ world @ rest).to_quaternion()


def local_offset(pose_bone, world_vec):
    """Translation monde → repère local de l'os (le root ne pointe pas vers +Z)."""
    return pose_bone.bone.matrix_local.to_3x3().inverted() @ Vector(world_vec)


def _bind_action(arm_obj, name):
    """Crée l'action et l'assigne — en gérant les « slots » de Blender 4.4+."""
    if arm_obj.animation_data is None:
        arm_obj.animation_data_create()
    action = bpy.data.actions.new(name)
    arm_obj.animation_data.action = action
    # `action_slot` n'existe que sur les versions à actions calquées ; s'il est
    # exposé et vide, l'assigner explicitement sinon les clés partent nulle part.
    if hasattr(arm_obj.animation_data, "action_slot") and arm_obj.animation_data.action_slot is None:
        try:
            arm_obj.animation_data.action_slot = action.slots.new(id_type="OBJECT", name=name)
        except Exception as exc:  # API en mouvement : ne pas casser le build
            print(f"[anim] slot non assigné pour {name} : {exc}")
    return action


def build_clips(arm_obj):
    """Écrit toutes les actions sur l'armature et renvoie leurs noms."""
    bpy.context.scene.render.fps = FPS
    root = arm_obj.pose.bones.get("Hips")
    built = []

    for name, keys in CLIPS.items():
        _bind_action(arm_obj, name)
        for frame, overrides, root_offset in keys:
            pose = {**BASE, **overrides}
            for bone_name, angles in pose.items():
                bone = arm_obj.pose.bones.get(bone_name)
                if bone is None:
                    continue
                bone.rotation_mode = "QUATERNION"
                bone.rotation_quaternion = bone_local(bone, angles)
                bone.keyframe_insert("rotation_quaternion", frame=frame)
            if root is not None:
                root.location = local_offset(root, root_offset)
                root.keyframe_insert("location", frame=frame)
        built.append(name)

    # L'armature ne doit pas rester avec une action active : sinon toute preview
    # du GLB exporté rendrait la dernière pose au lieu de la rest pose.
    arm_obj.animation_data.action = None
    for bone in arm_obj.pose.bones:
        bone.matrix_basis.identity()
    return built
