"""build_trooper.py — Importe le Sci-Fi Trooper et le DÉCOUPE en pièces équipables.

Le pack source (`D:/ressources externes/Perso/scifitroopermanv3.unitypackage`) livre
un personnage riggé Unreal (68 os) en 3 sous-mesh : un corps nu et l'armure. Ce
script en dérive la structure modulaire attendue par le jeu — corps + 5 slots —
en classant chaque ÎLOT de géométrie par l'os qui le porte. Aucun slot n'est
écrit à la main : la découpe est dérivée des poids de skinning.

Sortie `assets/models/characters/trooper/` : `body.glb` + 5 GLB de slot +
`manifest.json` (couche definition — le runtime ne hardcode rien).

Usage :
    blender -b --python tools/blender/build_trooper.py -- <src_assets_dir> <out_dir>

où `<src_assets_dir>` est l'arborescence `Assets/Sci-FI_Trooper_Man_v.3` extraite
du .unitypackage (cf. `tools/unity/unpack_unitypackage.ps1`).
"""

import bmesh
import bpy
import json
import math
import os
import sys
from collections import defaultdict

import mathutils
import numpy as np

# ── Découpe : os → slot ──────────────────────────────────────────────────────
# 5 slots, alignés sur ceux du nain (`assets/models/characters/dwarf/`) pour que
# l'UI d'équipement soit la même. `gloves` couvre avant-bras + mains : c'est le
# SEUL slot visible en run (le roguelite est FPS, on ne voit que le viewmodel).
BONE_SLOT = [
    ("helmet", ("head", "neck_")),
    ("gloves", ("hand_", "lowerarm", "forearm", "index_", "middle_", "ring_", "pinky_", "thumb_")),
    ("chest", ("spine_", "clavicle_", "upperarm")),
    ("boots", ("foot_", "ball_")),
    ("legs", ("pelvis", "thigh", "calf")),
]
SLOTS = [s for s, _ in BONE_SLOT]

# Sous-mesh source : le corps nu d'un côté, les deux couches d'armure de l'autre.
BODY_MESH = "Trooper_003"
ARMOR_MESHES = ("Trooper_000", "Trooper_002")

# Jeux de textures Unity : `bottom` habille le corps, `top` l'armure.
TEXSET = {"body": "bottom", "armor": "top"}

# Le pack livre du 4096². Les 5 pièces PARTAGENT le jeu de textures de l'armure :
# les embarquer dans chaque GLB donnait 210 Mo pour un personnage. On exporte donc
# en glTF+fichiers externes (une seule copie sur disque, un seul upload GPU), et
# on redescend à 2048² — en FPS on ne voit que les gants, le reste est un portrait
# de menu.
TEX_MAX = 2048


def slot_for_bone(bone_name):
    """Slot porteur d'un os, ou None si l'os n'appartient à aucun slot."""
    n = bone_name.lower()
    for slot, prefixes in BONE_SLOT:
        if any(p in n for p in prefixes):
            return slot
    return None


# ── Textures : Unity metallic/smoothness → ORM glTF ──────────────────────────


def _save(img, out_path, non_color):
    """Redimensionne à `TEX_MAX` et écrit la texture DANS le dossier de sortie.

    Toutes les images doivent vivre à côté des .gltf : l'export en fichiers
    externes les référence en chemin relatif, il ne les recopie pas.
    """
    # L'espace colorimétrique se fixe AVANT de toucher les pixels : le changer
    # relâche le buffer, et le faire après un `scale()` perd l'image redimensionnée.
    img.colorspace_settings.name = "Non-Color" if non_color else "sRGB"
    if not img.has_data:
        # `load()` est paresseux : `scale()` échoue tant que le buffer n'a pas
        # été touché. Lire un pixel suffit à le forcer.
        _ = img.pixels[0]
    if max(img.size) > TEX_MAX:
        w, h = img.size
        f = TEX_MAX / max(w, h)
        img.scale(int(w * f), int(h * f))
    img.filepath_raw = out_path
    img.file_format = "PNG"
    img.save()
    print(f"[tex] {img.size[0]}x{img.size[1]} -> {os.path.basename(out_path)}")
    return img


def prepare_textures(tex_dir, out_dir, kind, texset):
    """Prépare les 4 cartes d'un jeu (albédo, normale, ORM, émissif)."""
    base = f"T_SciFiTrooperV3_{texset}"
    maps = {}
    for key, suffix, non_color in (
        ("albedo", "_a", False),
        ("normal", "_n", True),
        ("emissive", "_emissive", False),
    ):
        img = bpy.data.images.load(os.path.join(tex_dir, f"{base}{suffix}.png"))
        maps[key] = _save(img, os.path.join(out_dir, f"trooper_{kind}_{key}.png"), non_color)

    # Unity `_RGBA` (R=metallic, G=occlusion, A=smoothness) → ORM glTF
    # (R=occlusion, G=roughness, B=metallic). La rugosité est l'INVERSE du
    # smoothness — l'oublier donne un personnage en plastique verni.
    src = bpy.data.images.load(os.path.join(tex_dir, f"{base}_RGBA.png"))
    w, h = src.size
    px = np.empty(w * h * 4, dtype=np.float32)
    src.pixels.foreach_get(px)
    px = px.reshape(-1, 4)
    orm = np.empty_like(px)
    orm[:, 0] = px[:, 1]
    orm[:, 1] = 1.0 - px[:, 3]
    orm[:, 2] = px[:, 0]
    orm[:, 3] = 1.0
    dst = bpy.data.images.new(f"trooper_{kind}_orm", w, h, alpha=False, float_buffer=False)
    dst.pixels.foreach_set(orm.reshape(-1))
    bpy.data.images.remove(src)
    maps["orm"] = _save(dst, os.path.join(out_dir, f"trooper_{kind}_orm.png"), True)
    return maps


def build_material(name, maps):
    """Matériau Principled câblé pour l'exporteur glTF.

    L'ORM doit passer par un Separate Color : c'est le motif que l'exporteur
    reconnaît pour ré-empaqueter occlusion/roughness/metallic dans UNE texture.
    """
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    nt = mat.node_tree
    nt.nodes.clear()

    out = nt.nodes.new("ShaderNodeOutputMaterial")
    bsdf = nt.nodes.new("ShaderNodeBsdfPrincipled")
    nt.links.new(bsdf.outputs["BSDF"], out.inputs["Surface"])

    def tex(image):
        node = nt.nodes.new("ShaderNodeTexImage")
        node.image = image
        return node

    nt.links.new(tex(maps["albedo"]).outputs["Color"], bsdf.inputs["Base Color"])

    nrm = nt.nodes.new("ShaderNodeNormalMap")
    nt.links.new(tex(maps["normal"]).outputs["Color"], nrm.inputs["Color"])
    nt.links.new(nrm.outputs["Normal"], bsdf.inputs["Normal"])

    sep = nt.nodes.new("ShaderNodeSeparateColor")
    nt.links.new(tex(maps["orm"]).outputs["Color"], sep.inputs["Color"])
    nt.links.new(sep.outputs["Green"], bsdf.inputs["Roughness"])
    nt.links.new(sep.outputs["Blue"], bsdf.inputs["Metallic"])

    nt.links.new(tex(maps["emissive"]).outputs["Color"], bsdf.inputs["Emission Color"])
    bsdf.inputs["Emission Strength"].default_value = 1.0
    return mat


# ── Découpe géométrique ──────────────────────────────────────────────────────


def dominant_slot(obj):
    """Slot d'un îlot : celui dont les os portent le plus de poids total.

    On somme les poids RÉELS par os (pas un simple comptage de vertices) : une
    épaulière touche l'os du bras par quelques poids résiduels, seule la masse
    de poids dit à qui elle appartient vraiment.
    """
    totals = defaultdict(float)
    gname = {g.index: g.name for g in obj.vertex_groups}
    for v in obj.data.vertices:
        for g in v.groups:
            slot = slot_for_bone(gname.get(g.group, ""))
            if slot:
                totals[slot] += g.weight
    if not totals:
        return None, {}
    best = max(totals.items(), key=lambda kv: kv[1])[0]
    return best, dict(totals)


def split_into_slots(armature):
    """Sépare les mesh d'armure en îlots, classe chacun, refusionne par slot.

    Renvoie {slot: objet}. Un slot sans îlot n'apparaît pas — un slot vide est
    une alerte, pas un silence (cf. `map-design-intention.md` §5.1).
    """
    islands = []
    for name in ARMOR_MESHES:
        obj = bpy.data.objects.get(name)
        if obj is None:
            print(f"[warn] sous-mesh d'armure absent : {name}")
            continue
        bpy.ops.object.select_all(action="DESELECT")
        obj.select_set(True)
        bpy.context.view_layer.objects.active = obj
        before = set(bpy.data.objects)
        bpy.ops.object.mode_set(mode="EDIT")
        bpy.ops.mesh.separate(type="LOOSE")
        bpy.ops.object.mode_set(mode="OBJECT")
        islands.extend([obj] + [o for o in set(bpy.data.objects) - before])

    print(f"[split] {len(islands)} îlots de géométrie")

    by_slot = defaultdict(list)
    unassigned = []
    for isl in islands:
        slot, totals = dominant_slot(isl)
        if slot is None:
            unassigned.append(isl)
            continue
        by_slot[slot].append(isl)
    if unassigned:
        print(f"[warn] {len(unassigned)} îlots sans os dominant — ignorés")
        for o in unassigned:
            bpy.data.objects.remove(o, do_unlink=True)

    merged = {}
    for slot, objs in by_slot.items():
        bpy.ops.object.select_all(action="DESELECT")
        for o in objs:
            o.select_set(True)
        bpy.context.view_layer.objects.active = objs[0]
        if len(objs) > 1:
            bpy.ops.object.join()
        m = bpy.context.view_layer.objects.active
        m.name = f"trooper_{slot}"
        m.data.name = f"trooper_{slot}"
        tris = sum(len(p.vertices) - 2 for p in m.data.polygons)
        print(f"[split] {slot:8s} <- {len(objs):2d} îlots, {tris} tris")
        merged[slot] = m
    for slot in SLOTS:
        if slot not in merged:
            print(f"[ALERTE] slot déclaré SANS géométrie : {slot}")
    return merged


# ── Animations ───────────────────────────────────────────────────────────────
#
# 🚨 Le pack ne fournit AUCUNE animation utilisable : son unique clip
# « Take 001 » anime bien 69 os, mais avec des valeurs constantes — c'est une
# A-pose tenue 3,37 s (vérifié en rendant les frames 0 et 53, identiques). On
# construit donc `idle` et `walk` ici.
#
# 🚨 Les poses s'écrivent en repère ARMATURE, jamais en repère local d'os.
# Deviner l'axe local envoie les bras derrière le dos — piège déjà payé sur le
# nain. Conversion : `M⁻¹ · R · M` avec `M = bone.matrix_local.to_3x3()`.
#
# Repère : le personnage regarde −Y dans Blender ⇒ la flexion sagittale (jambes
# qui avancent, bras qui balancent) tourne autour de **X**.

FPS = 24


def _pose_rot(pbone, axis, angle):
    """Rotation d'un os exprimée dans le repère de l'ARMATURE."""
    m = pbone.bone.matrix_local.to_3x3()
    r = mathutils.Matrix.Rotation(angle, 3, axis)
    pbone.rotation_mode = "QUATERNION"
    pbone.rotation_quaternion = (m.inverted() @ r @ m).to_quaternion()


def _key(pbone, frame):
    pbone.keyframe_insert("rotation_quaternion", frame=frame)


def _bind_action(armature, action):
    """Assigne l'action. Blender 4.4+ : une action a des SLOTS, sans lequel les
    clés partent nulle part."""
    armature.animation_data_create()
    armature.animation_data.action = action
    slots = getattr(action, "slots", None)
    if slots and hasattr(armature.animation_data, "action_slot"):
        if armature.animation_data.action_slot is None:
            armature.animation_data.action_slot = slots[0]


def _clear_pose(armature):
    for pb in armature.pose.bones:
        pb.rotation_mode = "QUATERNION"
        pb.rotation_quaternion = mathutils.Quaternion()
        pb.location = mathutils.Vector()


def _bone(armature, name):
    return armature.pose.bones.get(name)


# Le pack est modélisé en A-pose (bras écartés ≈ 45°), ce qui lit « asset » et
# non « personnage ». Les deux clips partent donc bras BAISSÉS.
ARMS_DOWN_RAD = math.radians(38.0)


def _apply_arms_down(armature, frame):
    """Rentre les bras le long du corps. Le bras gauche descend en tournant
    dans un sens, le droit dans l'autre — d'où les signes opposés."""
    for side, sign in (("l", 1.0), ("r", -1.0)):
        if pb := _bone(armature, f"upperarm_{side}"):
            _pose_rot(pb, "Y", sign * ARMS_DOWN_RAD)
            _key(pb, frame)


def build_idle(armature):
    """Respiration lente, bras au repos. 2,5 s bouclées."""
    action = bpy.data.actions.new("idle")
    _bind_action(armature, action)
    _clear_pose(armature)
    frames = int(2.5 * FPS)
    for i in range(frames + 1):
        f = i + 1
        phase = math.sin(2.0 * math.pi * i / frames)
        _apply_arms_down(armature, f)
        # Le souffle se répartit sur deux vertèbres : sur une seule il lit
        # « hoquet ».
        for name, amp in (("spine_01", 1.1), ("spine_02", 0.8)):
            if pb := _bone(armature, name):
                _pose_rot(pb, "X", math.radians(amp) * phase)
                _key(pb, f)
        if pb := _bone(armature, "head"):
            _pose_rot(pb, "X", math.radians(-1.4) * phase)
            _key(pb, f)
    return action


def build_walk(armature):
    """Cycle de marche SUR PLACE, 1 s bouclée. Les bras contre-balancent les
    jambes (bras gauche avec jambe droite), sinon la marche lit « robot »."""
    action = bpy.data.actions.new("walk")
    _bind_action(armature, action)
    _clear_pose(armature)
    frames = FPS
    for i in range(frames + 1):
        f = i + 1
        t = 2.0 * math.pi * i / frames
        swing = math.sin(t)
        for side, sign in (("l", 1.0), ("r", -1.0)):
            leg = sign * swing
            if pb := _bone(armature, f"thigh_{side}"):
                _pose_rot(pb, "X", math.radians(26.0) * leg)
                _key(pb, f)
            # Le genou ne plie QUE vers l'arrière : on ne garde que la moitié
            # négative du balancement, sinon la jambe casse à l'envers.
            if pb := _bone(armature, f"calf_{side}"):
                bend = max(0.0, -leg)
                _pose_rot(pb, "X", math.radians(-42.0) * bend)
                _key(pb, f)
            if pb := _bone(armature, f"foot_{side}"):
                _pose_rot(pb, "X", math.radians(12.0) * leg)
                _key(pb, f)
            # Bras : opposé à la jambe du même côté.
            if pb := _bone(armature, f"upperarm_{side}"):
                m = pb.bone.matrix_local.to_3x3()
                down = mathutils.Matrix.Rotation(sign * ARMS_DOWN_RAD, 3, "Y")
                sw = mathutils.Matrix.Rotation(math.radians(-20.0) * leg, 3, "X")
                pb.rotation_mode = "QUATERNION"
                pb.rotation_quaternion = (m.inverted() @ (sw @ down) @ m).to_quaternion()
                _key(pb, f)
            if pb := _bone(armature, f"lowerarm_{side}"):
                _pose_rot(pb, "X", math.radians(-18.0))
                _key(pb, f)
        # Ballant du buste à DOUBLE fréquence (un pas par demi-cycle).
        if pb := _bone(armature, "spine_01"):
            _pose_rot(pb, "Z", math.radians(2.5) * math.sin(2.0 * t))
            _key(pb, f)
    return action


def build_clips(armature):
    """Construit les clips puis relâche la pose : sans ça la dernière action
    reste active et tout export/preview rend une pose arbitraire."""
    actions = [build_idle(armature), build_walk(armature)]
    if armature.animation_data:
        armature.animation_data.action = None
    _clear_pose(armature)
    print("[anim] clips construits :", ", ".join(a.name for a in actions))
    return actions


# ── Bras de viewmodel ────────────────────────────────────────────────────────
#
# Le viewmodel FPS n'a pas besoin d'un bras animé : il lui faut une main à un
# poignet, que le code place déjà en dérivant la position de l'arme équipée. On
# CUIT donc l'avant-bras + la main dans une pose de préhension et on exporte du
# statique — le mécanisme de placement par-arme reste intact.
#
# 🚨 Ne pas prendre `upperarm` : un viewmodel montre l'avant-bras, pas l'épaule.

VIEWMODEL_BONES = (
    "lowerarm",
    "hand_",
    "index_",
    "middle_",
    "ring_",
    "pinky_",
    "thumb_",
)

# Flexion des phalanges pour refermer la main sur une crosse. La 1re phalange
# plie le moins, la 2e le plus : c'est ce dégradé qui fait une main et non une
# pince.
GRIP_CURL_DEG = (38.0, 55.0, 45.0)
GRIP_THUMB_DEG = 26.0


def _is_side(bone, side):
    """Côté d'un os, quelle que soit la convention de nommage du rig.

    🚨 Ce rig en mélange DEUX : `lowerarm_l` (suffixe) et `l_forearm_Muscle`
    (préfixe). Ne tester que le suffixe écarte tous les os musculaires — et donc
    la chair de l'avant-bras qu'ils portent, qui disparaît du viewmodel.
    """
    return bone.endswith(f"_{side}") or bone.startswith(f"{side}_")


def _dominant_bone(vertex, gname):
    """Nom de l'os qui porte le plus de poids sur ce vertex."""
    best, bw = "", 0.0
    for g in vertex.groups:
        if g.weight > bw:
            best, bw = gname.get(g.group, ""), g.weight
    return best


def pose_grip(armature):
    """Referme les doigts. Poses en repère ARMATURE, comme les clips."""
    for side in ("l", "r"):
        for finger in ("index", "middle", "ring", "pinky"):
            for i, deg in enumerate(GRIP_CURL_DEG, start=1):
                if pb := _bone(armature, f"{finger}_0{i}_{side}"):
                    _pose_rot(pb, "X", math.radians(deg))
        for i in (1, 2, 3):
            if pb := _bone(armature, f"thumb_0{i}_{side}"):
                _pose_rot(pb, "X", math.radians(GRIP_THUMB_DEG))


def _isolate_side(src, side):
    """Copie de `src` réduite à l'avant-bras + la main du côté demandé.

    Rend `None` si rien ne subsiste : une pièce qui ne couvre pas les mains (le
    casque, les bottes) ne doit pas produire un objet vide.
    """
    dup = src.copy()
    dup.data = src.data.copy()
    bpy.context.scene.collection.objects.link(dup)
    gname = {g.index: g.name for g in dup.vertex_groups}
    drop = set()
    for v in dup.data.vertices:
        bone = _dominant_bone(v, gname)
        if not (any(p in bone for p in VIEWMODEL_BONES) and _is_side(bone, side)):
            drop.add(v.index)
    if len(drop) == len(dup.data.vertices):
        bpy.data.objects.remove(dup, do_unlink=True)
        return None

    # 🚨 Découpe par bmesh, pas par opérateur. `bpy.ops.mesh.delete` dépend du
    # MODE de sélection actif (vertex/arête/face) : selon le maillage il emporte
    # tout ou rien, en silence. Ici la géométrie supprimée est désignée
    # explicitement.
    bm = bmesh.new()
    bm.from_mesh(dup.data)
    bm.verts.ensure_lookup_table()
    bmesh.ops.delete(bm, geom=[bm.verts[i] for i in drop], context="VERTS")
    bm.to_mesh(dup.data)
    bm.free()
    tris = sum(len(p.vertices) - 2 for p in dup.data.polygons)
    print(f"[viewmodel] {src.name} côté {side} : {tris} tris isolés")
    if tris == 0:
        bpy.data.objects.remove(dup, do_unlink=True)
        return None
    return dup


def _normalize_viewmodel_arm(obj, armature, side):
    """Met le bras dans la convention attendue par le viewmodel.

    🚨 Découpé tel quel, le maillage garde ses coordonnées DANS le personnage :
    son origine est à plus d'un mètre de lui, et le code — qui place l'objet au
    poignet — envoie la main au loin. Mesuré : origine à 1,25 m, taille 0,25 m
    contre 2,0 pour l'asset de référence.

    Deux corrections, dérivées du squelette : poignet à l'ORIGINE, et avant-bras
    sur l'axe **+Z de Blender** (qui devient +Y en glTF, l'axe que
    `position_hands` aligne du coude vers le poignet).

    🚨 **Aucune mise à l'échelle.** `ViewmodelArmsTuning::glb_scale` documente la
    convention : « mesh baked en mètres réels ; 1.0 = taille physique ». Le bras
    découpé fait 0,25 m, ce qui EST sa taille physique. Le normaliser sur
    l'emprise de l'ancien asset (±1) le rendait 8× trop gros — celui-ci suit une
    autre convention interne, ce n'est pas une référence de taille.
    """
    bones = armature.data.bones
    wrist_b, elbow_b = bones.get(f"hand_{side}"), bones.get(f"lowerarm_{side}")
    if not (wrist_b and elbow_b):
        print(f"[ALERTE] bras {side} : os poignet/coude introuvables, pas de normalisation")
        return
    # 🚨 Les vertices vivent en repère OBJET, la position des os se calcule en
    # repère MONDE. Translater les uns par les autres laisse le maillage à côté.
    # On cuit d'abord la transformation de l'objet dans sa donnée : les deux
    # repères coïncident ensuite.
    obj.data.transform(obj.matrix_world)
    obj.matrix_world = mathutils.Matrix.Identity(4)

    m = armature.matrix_world
    wrist = m @ wrist_b.head_local
    elbow = m @ elbow_b.head_local
    axis = (wrist - elbow).normalized()

    to_origin = mathutils.Matrix.Translation(-wrist)
    align = axis.rotation_difference(mathutils.Vector((0.0, 0.0, 1.0))).to_matrix().to_4x4()
    obj.data.transform(align @ to_origin)
    obj.matrix_world = mathutils.Matrix.Identity(4)


def export_viewmodel_arms(body, gloves, armature, out_dir):
    """Cuit avant-bras + main (corps ET gant) dans la pose de préhension et
    exporte un fichier STATIQUE par côté.

    Statique volontairement : le viewmodel dérive déjà la position des poignets
    de l'arme équipée, il n'a ni squelette ni animation à piloter.
    """
    pose_grip(armature)
    bpy.context.view_layer.update()

    for side in ("l", "r"):
        parts = [p for p in (_isolate_side(body, side), _isolate_side(gloves, side)) if p]
        if not parts:
            print(f"[ALERTE] bras {side} : aucune géométrie isolée")
            continue
        # Cuire la pose : appliquer le modificateur d'armature fige les doigts
        # refermés dans le maillage, ce qui permet d'exporter sans squelette.
        for p in parts:
            bpy.ops.object.select_all(action="DESELECT")
            p.select_set(True)
            bpy.context.view_layer.objects.active = p
            for mod in list(p.modifiers):
                if mod.type == "ARMATURE":
                    bpy.ops.object.modifier_apply(modifier=mod.name)
        bpy.ops.object.select_all(action="DESELECT")
        for p in parts:
            p.select_set(True)
        bpy.context.view_layer.objects.active = parts[0]
        if len(parts) > 1:
            bpy.ops.object.join()
        arm_obj = bpy.context.view_layer.objects.active
        arm_obj.name = f"trooper_viewmodel_arm_{side}"
        arm_obj.data.name = arm_obj.name
        _normalize_viewmodel_arm(arm_obj, armature, side)
        tris = sum(len(p.vertices) - 2 for p in arm_obj.data.polygons)

        path = os.path.join(out_dir, f"viewmodel_arm_{side}.gltf")
        bpy.ops.object.select_all(action="DESELECT")
        arm_obj.select_set(True)
        bpy.context.view_layer.objects.active = arm_obj
        bpy.ops.export_scene.gltf(
            filepath=path,
            export_format="GLTF_SEPARATE",
            use_selection=True,
            export_tangents=True,
            export_animations=False,
            export_skins=False,
            export_yup=True,
            export_keep_originals=True,
        )
        print(f"[viewmodel] bras {side} : {tris} tris -> {os.path.basename(path)}")
        bpy.data.objects.remove(arm_obj, do_unlink=True)
    _clear_pose(armature)


def export_part(objs, armature, path, with_anim):
    """Exporte une sélection + l'armature en glTF à fichiers externes.

    `export_keep_originals` fait référencer les PNG déjà écrits dans le dossier
    au lieu de les ré-encoder dans chaque fichier — c'est ce qui fait passer les
    5 pièces de 33 Mo chacune à quelques centaines de Ko. Tangentes ON : sans
    elles, Bevy ne peut pas appliquer les normal maps.
    """
    bpy.ops.object.select_all(action="DESELECT")
    for o in objs:
        o.select_set(True)
    armature.select_set(True)
    bpy.context.view_layer.objects.active = armature
    bpy.ops.export_scene.gltf(
        filepath=path,
        export_format="GLTF_SEPARATE",
        use_selection=True,
        export_tangents=True,
        export_animations=with_anim,
        export_skins=True,
        export_yup=True,
        export_keep_originals=True,
    )
    base = os.path.splitext(path)[0]
    total = sum(
        os.path.getsize(f) for f in (f"{base}.gltf", f"{base}.bin") if os.path.exists(f)
    )
    print(f"[gltf] {os.path.basename(base)}  ({total / 1024:.0f} KB + textures partagées)")


def main():
    argv = sys.argv[sys.argv.index("--") + 1:]
    src_dir, out_dir = argv[0], argv[1]
    tex_dir = os.path.join(src_dir, "Textures")
    fbx = os.path.join(src_dir, "Meshes", "SK_SciFiTrooperManV3.fbx")
    os.makedirs(out_dir, exist_ok=True)

    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.fbx(filepath=fbx)

    armature = next(o for o in bpy.data.objects if o.type == "ARMATURE")
    # Les empties IK d'Unreal ne servent qu'au rig d'anim : ils alourdissent
    # chaque GLB de slot pour rien.
    for o in [o for o in bpy.data.objects if o.type == "EMPTY"]:
        bpy.data.objects.remove(o, do_unlink=True)

    # 🚨 L'import FBX crée des images aux chemins de texture du pack, sans données
    # (il ne les résout pas). `images.load()` renverrait ces coquilles au lieu de
    # lire le fichier, et toute écriture échoue sur « pas de données d'image ».
    # On reconstruit matériaux et textures de zéro : on purge ce que l'import laisse.
    for img in list(bpy.data.images):
        bpy.data.images.remove(img)
    for mat in list(bpy.data.materials):
        bpy.data.materials.remove(mat)
    # L'unique clip du pack (« Take 001 ») est une A-pose figée : 69 os animés à
    # valeurs constantes. L'exporter donnerait un clip qui ne fait rien et qu'on
    # confondrait avec une vraie animation.
    for act in list(bpy.data.actions):
        bpy.data.actions.remove(act)

    maps = {k: prepare_textures(tex_dir, out_dir, k, ts) for k, ts in TEXSET.items()}
    mat_body = build_material("trooper_body", maps["body"])
    mat_armor = build_material("trooper_armor", maps["armor"])

    body = bpy.data.objects[BODY_MESH]
    body.name = "trooper_body"
    # Renommer aussi la DONNÉE : le glTF exporte le nom du maillage, pas celui de
    # l'objet — sans ça le manifeste annonce `trooper_body` et le fichier dit
    # `Maillage.001`.
    body.data.name = "trooper_body"
    body.data.materials.clear()
    body.data.materials.append(mat_body)

    slots = split_into_slots(armature)
    for obj in slots.values():
        obj.data.materials.clear()
        obj.data.materials.append(mat_armor)

    # 🚨 Les clips ne vont QUE dans le corps. Les pièces partagent son squelette
    # au runtime (leurs joints sont rebranchés par nom sur les os du corps), donc
    # une animation embarquée dans une pièce ne pourrait que la désynchroniser.
    # Ne pas l'exporter rend la dislocation impossible au lieu de la surveiller.
    build_clips(armature)
    export_part([body], armature, os.path.join(out_dir, "body.gltf"), with_anim=True)
    manifest = {
        "id": "trooper",
        "source": "Sci-Fi Trooper Man v3 (unitypackage)",
        "rig": {"kind": "unreal", "bones": len(armature.data.bones), "root": "pelvis"},
        "body": {"file": "body.gltf", "mesh": "trooper_body"},
        "slots": {},
    }
    for slot, obj in slots.items():
        export_part([obj], armature, os.path.join(out_dir, f"{slot}.gltf"), with_anim=False)
        tris = sum(len(p.vertices) - 2 for p in obj.data.polygons)
        manifest["slots"][slot] = {
            "file": f"{slot}.gltf",
            "mesh": obj.name,
            "tris": tris,
            # L'armure se pose PAR-DESSUS le corps (combinaison lisse) : aucune
            # pièce n'a besoin de masquer un sous-mesh, contrairement au nain.
            "hides": [],
        }

    # Bras de viewmodel, dérivés du MÊME corps : ce qu'on voit en FPS est
    # littéralement le personnage qu'on voit à la 3ᵉ personne.
    export_viewmodel_arms(body, slots.get("gloves"), armature, out_dir)
    manifest["viewmodel"] = {
        "left": "viewmodel_arm_l.gltf",
        "right": "viewmodel_arm_r.gltf",
        "note": "statique, pose de préhension cuite — le placement des poignets reste au code",
    }

    mpath = os.path.join(out_dir, "manifest.json")
    with open(mpath, "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=2, ensure_ascii=False)
    print(f"[manifest] {mpath}")
    print("[ok] slots exportés :", ", ".join(sorted(slots)))


main()
