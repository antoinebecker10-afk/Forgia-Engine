"""retarget_mixamo.py — Transfère des animations Mixamo sur le rig du Trooper.

Mixamo nomme ses os `mixamorig:Hips` (PascalCase préfixé) ; le Trooper suit la
convention **Unreal** (`pelvis`, `spine_01`, `upperarm_l`…). Ce script fait la
correspondance et transfère la pose, frame par frame.

🚨 On transfère la ROTATION, pas la position. Les deux squelettes n'ont ni les
mêmes longueurs d'os ni les mêmes proportions : recopier des positions
disloquerait le personnage. Seul le bassin transmet sa translation, mise à
l'échelle du rapport de taille — c'est elle qui porte le ballant vertical de la
marche.

🚨 Les os sont traités PARENT D'ABORD. `pose_bone.matrix` s'exprime en repère
armature : écrire un enfant avant son parent le ferait recalculer ensuite.

Usage :
    blender -b --python tools/blender/retarget_mixamo.py -- --selftest
    blender -b --python tools/blender/retarget_mixamo.py -- <trooper.gltf> <dossier_fbx> <sortie.gltf>
"""

import bpy
import math
import os
import sys

import mathutils

# ── Correspondance Mixamo → Unreal (trooper) ─────────────────────────────────
#
# Vérifiée contre les 68 os réels du trooper et la doc de `mixamo_converter`.
# Les os d'assistance du trooper (`*_twist_01_*`, `*_Muscle`) n'ont PAS
# d'équivalent Mixamo : ils restent au repos, ce qui est correct — les animer
# depuis un rig qui ne les produit pas ajouterait du bruit.

_CORE = {
    "Hips": "pelvis",
    "Spine": "spine_01",
    "Spine1": "spine_02",
    "Spine2": "spine_03",
    "Neck": "neck_01",
    "Head": "head",
}

_FINGERS = {"Thumb": "thumb", "Index": "index", "Middle": "middle", "Ring": "ring", "Pinky": "pinky"}


def build_bone_map():
    """`mixamorig:X` → nom d'os du trooper."""
    m = {f"mixamorig:{k}": v for k, v in _CORE.items()}
    for side, suffix in (("Left", "l"), ("Right", "r")):
        m[f"mixamorig:{side}Shoulder"] = f"clavicle_{suffix}"
        m[f"mixamorig:{side}Arm"] = f"upperarm_{suffix}"
        m[f"mixamorig:{side}ForeArm"] = f"lowerarm_{suffix}"
        m[f"mixamorig:{side}Hand"] = f"hand_{suffix}"
        m[f"mixamorig:{side}UpLeg"] = f"thigh_{suffix}"
        m[f"mixamorig:{side}Leg"] = f"calf_{suffix}"
        m[f"mixamorig:{side}Foot"] = f"foot_{suffix}"
        m[f"mixamorig:{side}ToeBase"] = f"ball_{suffix}"
        for mixamo_finger, ue_finger in _FINGERS.items():
            for i in (1, 2, 3):
                m[f"mixamorig:{side}Hand{mixamo_finger}{i}"] = f"{ue_finger}_0{i}_{suffix}"
    return m


BONE_MAP = build_bone_map()
FPS = 30


def _hierarchy_order(armature, names):
    """Os demandés, parents avant enfants."""
    ordered, seen = [], set()

    def visit(bone):
        if bone.name in seen:
            return
        seen.add(bone.name)
        if bone.parent:
            visit(bone.parent)
        if bone.name in names:
            ordered.append(bone.name)

    for n in names:
        if bone := armature.data.bones.get(n):
            visit(bone)
    return ordered


def _armature_of(objs):
    return next(o for o in objs if o.type == "ARMATURE")


def retarget_frame(src_arm, dst_arm, pairs, hip_scale):
    """Recopie la pose du frame courant, source → cible.

    Le transfert se fait par **delta de rotation en repère armature** :
    `D = pose_source · repos_source⁻¹`, appliqué au repos de la cible. Cela rend
    le résultat indépendant des orientations d'axes locaux, qui diffèrent entre
    les deux rigs — c'est exactement le piège qui envoie les bras derrière le dos
    quand on devine l'axe.
    """
    for src_name, dst_name in pairs:
        src_pb = src_arm.pose.bones[src_name]
        dst_pb = dst_arm.pose.bones[dst_name]

        src_rest = src_pb.bone.matrix_local.to_3x3()
        delta = src_pb.matrix.to_3x3() @ src_rest.inverted()
        target = delta @ dst_pb.bone.matrix_local.to_3x3()

        keep = dst_pb.matrix.translation.copy()
        dst_pb.matrix = mathutils.Matrix.Translation(keep) @ target.to_4x4()
        dst_pb.rotation_mode = "QUATERNION"
        # 🚨 Écrire `pose_bone.matrix` ne réévalue pas la hiérarchie : sans ce
        # rafraîchissement, chaque enfant lit un parent PÉRIMÉ et l'erreur
        # s'accumule le long de la chaîne. Mesuré : 160° de dérive sur un os de
        # doigt (neuf niveaux sous le bassin) contre ~0 sur la colonne.
        bpy.context.view_layer.update()

    # Le bassin porte le ballant vertical. Sa translation est mise à l'échelle
    # du rapport de taille : recopier les centimètres d'un rig plus grand ferait
    # sautiller le personnage.
    src_hip = src_arm.pose.bones.get("mixamorig:Hips")
    dst_hip = dst_arm.pose.bones.get("pelvis")
    if src_hip and dst_hip:
        offset = src_hip.matrix.translation - src_hip.bone.matrix_local.translation
        dst_hip.location = (offset * hip_scale) @ dst_hip.bone.matrix_local.to_3x3()


def key_pose(dst_arm, names, frame):
    for n in names:
        pb = dst_arm.pose.bones[n]
        pb.keyframe_insert("rotation_quaternion", frame=frame)
    if hip := dst_arm.pose.bones.get("pelvis"):
        hip.keyframe_insert("location", frame=frame)


def armature_height(arm):
    """Hauteur du squelette, pour mettre le bassin à l'échelle."""
    zs = [b.head_local.z for b in arm.data.bones] + [b.tail_local.z for b in arm.data.bones]
    return max(zs) - min(zs) if zs else 1.0


def retarget_action(src_arm, dst_arm, clip_name):
    """Transfère l'action active de la source vers une nouvelle action cible."""
    pairs = []
    for src_name, dst_name in BONE_MAP.items():
        if src_name in src_arm.pose.bones and dst_name in dst_arm.pose.bones:
            pairs.append((src_name, dst_name))
    if not pairs:
        raise RuntimeError(
            "aucun os apparié — le FBX n'a pas de préfixe `mixamorig:` ? "
            f"exemples présents : {[b.name for b in src_arm.pose.bones][:5]}"
        )
    dst_names = [d for _, d in pairs]
    ordered = _hierarchy_order(dst_arm, set(dst_names))
    pairs = sorted(pairs, key=lambda p: ordered.index(p[1]))

    scale = armature_height(dst_arm) / max(armature_height(src_arm), 1e-4)
    action = bpy.data.actions.new(clip_name)
    dst_arm.animation_data_create()
    dst_arm.animation_data.action = action
    if getattr(action, "slots", None) and hasattr(dst_arm.animation_data, "action_slot"):
        dst_arm.animation_data.action_slot = action.slots[0] if action.slots else None

    src_action = src_arm.animation_data.action
    start, end = (int(v) for v in src_action.frame_range)
    for f in range(start, end + 1):
        bpy.context.scene.frame_set(f)
        retarget_frame(src_arm, dst_arm, pairs, scale)
        key_pose(dst_arm, ordered, f)
    print(f"[retarget] {clip_name!r} : {len(pairs)} os, frames {start}..{end}")
    return action


# ── Auto-test : aller-retour sur un clip connu ───────────────────────────────


def selftest(trooper_gltf):
    """Renomme une COPIE du rig du trooper aux noms Mixamo, lui applique le clip
    `walk`, puis le retargete en retour. Si la pose revient identique, le calcul
    de transfert est juste — sans avoir besoin d'un fichier Mixamo.
    """
    bpy.ops.wm.read_factory_settings(use_empty=True)
    for o in list(bpy.data.objects):
        bpy.data.objects.remove(o, do_unlink=True)
    bpy.ops.import_scene.gltf(filepath=trooper_gltf)

    dst = _armature_of(bpy.data.objects)
    ref_action = next(a for a in bpy.data.actions if "walk" in a.name)

    # Copie du rig, renommée à l'envers de la table : elle joue le rôle du FBX.
    src = dst.copy()
    src.data = dst.data.copy()
    bpy.context.scene.collection.objects.link(src)
    reverse = {v: k for k, v in BONE_MAP.items()}
    for b in src.data.bones:
        if b.name in reverse:
            b.name = reverse[b.name]
    src.animation_data_create()
    src.animation_data.action = ref_action
    if getattr(ref_action, "slots", None):
        src.animation_data.action_slot = ref_action.slots[0]

    # La cible repart du repos.
    dst.animation_data_clear()
    for pb in dst.pose.bones:
        pb.rotation_mode = "QUATERNION"
        pb.rotation_quaternion = mathutils.Quaternion()
        pb.location = mathutils.Vector()

    retarget_action(src, dst, "walk_retargeted")

    # Comparaison : à mi-clip, chaque os apparié doit retrouver son orientation.
    start, end = (int(v) for v in ref_action.frame_range)
    worst, worst_bone = 0.0, ""
    per_bone = {}
    for frame in (start, (start + end) // 2, end):
        bpy.context.scene.frame_set(frame)
        bpy.context.view_layer.update()
        for src_name, dst_name in BONE_MAP.items():
            if src_name not in src.pose.bones or dst_name not in dst.pose.bones:
                continue
            a = src.pose.bones[src_name].matrix.to_3x3().to_quaternion()
            b = dst.pose.bones[dst_name].matrix.to_3x3().to_quaternion()
            # 🚨 Un quaternion et son opposé décrivent la MÊME orientation
            # (double recouvrement) : `.angle` peut rendre 359° pour un écart
            # réel de 1°. On ramène dans [0°, 180°], sinon le test accuse à tort.
            raw = math.degrees(a.rotation_difference(b).angle) % 360.0
            diff = min(raw, 360.0 - raw)
            per_bone[dst_name] = max(per_bone.get(dst_name, 0.0), diff)
            if diff > worst:
                worst, worst_bone = diff, dst_name
    top = sorted(per_bone.items(), key=lambda kv: -kv[1])[:5]
    print("[selftest] pires os :", ", ".join(f"{n} {v:.2f}°" for n, v in top))
    print(f"[selftest] écart angulaire max {worst:.3f}° (os {worst_bone!r})")
    ok = worst < 0.5
    print("[selftest] RESULTAT :", "OK" if ok else "ECHEC")
    return ok


def main():
    argv = sys.argv[sys.argv.index("--") + 1:]
    trooper = os.path.join(
        "assets", "models", "characters", "trooper", "body.gltf"
    )
    if argv and argv[0] == "--selftest":
        if len(argv) > 1:
            trooper = argv[1]
        sys.exit(0 if selftest(trooper) else 1)

    trooper_gltf, fbx_dir, out_gltf = argv[0], argv[1], argv[2]
    bpy.ops.wm.read_factory_settings(use_empty=True)
    for o in list(bpy.data.objects):
        bpy.data.objects.remove(o, do_unlink=True)
    bpy.ops.import_scene.gltf(filepath=trooper_gltf)
    dst = _armature_of(bpy.data.objects)
    keep = set(bpy.data.objects)

    fbxs = sorted(f for f in os.listdir(fbx_dir) if f.lower().endswith(".fbx"))
    if not fbxs:
        print(f"[retarget] aucun .fbx dans {fbx_dir}")
        return
    for fbx in fbxs:
        bpy.ops.import_scene.fbx(filepath=os.path.join(fbx_dir, fbx))
        imported = [o for o in bpy.data.objects if o not in keep]
        src = _armature_of(imported)
        retarget_action(src, dst, os.path.splitext(fbx)[0])
        for o in imported:
            bpy.data.objects.remove(o, do_unlink=True)

    dst.animation_data.action = None
    bpy.ops.object.select_all(action="SELECT")
    bpy.context.view_layer.objects.active = dst
    bpy.ops.export_scene.gltf(
        filepath=out_gltf,
        export_format="GLTF_SEPARATE",
        use_selection=True,
        export_tangents=True,
        export_animations=True,
        export_skins=True,
        export_yup=True,
        export_keep_originals=True,
    )
    print(f"[retarget] {len(fbxs)} clips écrits dans {out_gltf}")


main()
