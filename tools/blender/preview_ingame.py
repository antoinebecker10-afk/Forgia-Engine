"""preview_ingame.py — Scène de calibration WYSIWYG du viewmodel (story-661).

Reproduit EXACTEMENT ce que rend le jeu : parse `viewmodel_arena.toml` (offsets,
rotations, target_size de l'arme) + `fps_tuning.toml` (fractions bras, FOV
viewmodel), applique les mêmes maths que `position_hands` (forgia-viewmodel) et
l'autoscale AABB de l'arme, caméra à l'origine. Permet d'itérer le placement des
bras en rendus offline (~20 s) au lieu d'aller-retours in-game.

Usage :
  blender --background --python tools/blender/preview_ingame.py -- \
      <weapon_key> <out_dir>          # ex. bourrasque previews/
"""

import math
import os
import sys
import tomllib

import bpy
from mathutils import Matrix, Quaternion, Vector

argv = sys.argv[sys.argv.index("--") + 1 :]
WEAPON_KEY, OUT_DIR = argv[0], argv[1]
ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
os.makedirs(OUT_DIR, exist_ok=True)

# ── Données du jeu (source de vérité = les TOML, pas de duplication) ──
with open(os.path.join(ROOT, "assets/genomes/viewmodel_arena.toml"), "rb") as f:
    GENOME = tomllib.load(f)["weapons"][WEAPON_KEY]
with open(os.path.join(ROOT, "assets/genomes/fps_tuning.toml"), "rb") as f:
    FT = tomllib.load(f)
ARMS = FT["viewmodel_arms"]
FOV_DEG = FT["viewmodel_fov"]["fov_deg"]

WEAPON_GLB = os.path.join(ROOT, f"assets/models/weapons/forgia/{WEAPON_KEY}.glb")
ARM_R_GLB = os.path.join(ROOT, "assets/models/arms/fps_arm_R.glb")
ARM_L_GLB = os.path.join(ROOT, "assets/models/arms/fps_arm_L.glb")

# Bevy (x,y,z) → Blender (x,-z,y). Objets glTF importés = identité Bevy →
# placement Bevy M s'applique en Blender par W' = C·M·C⁻¹·W.
C4 = Matrix(((1, 0, 0, 0), (0, 0, -1, 0), (0, 1, 0, 0), (0, 0, 0, 1)))
C4_INV = C4.inverted()


def import_glb(path):
    """Importe et retourne les objets racine (sans parent) nouvellement créés.
    Reset toute pose/action : l'importeur glTF laisse la dernière animation
    active sur l'armature → rendrait une pose aléatoire (Bevy montre le rest)."""
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=path)
    new = [o for o in bpy.data.objects if o not in before]
    for o in new:
        if o.type == "ARMATURE":
            if o.animation_data:
                o.animation_data_clear()
            for pb in o.pose.bones:
                pb.matrix_basis = Matrix.Identity(4)
    return [o for o in new if o.parent is None or o.parent in before]


def apply_bevy_transform(roots, m_bevy):
    for r in roots:
        r.matrix_world = C4 @ m_bevy @ C4_INV @ r.matrix_world


def world_aabb_max_extent(roots):
    lo = Vector((1e9,) * 3)
    hi = Vector((-1e9,) * 3)
    for r in roots:
        for o in [r] + list(r.children_recursive):
            if o.type != "MESH":
                continue
            for corner in o.bound_box:
                w = o.matrix_world @ Vector(corner)
                lo = Vector(map(min, lo, w))
                hi = Vector(map(max, hi, w))
    return max(hi - lo)


def bevy_trs(translation, rotation, scale):
    return (
        Matrix.Translation(Vector(translation))
        @ rotation.to_matrix().to_4x4()
        @ Matrix.Diagonal(Vector((scale, scale, scale, 1.0)))
    )


def rot_arc_y_to(fwd):
    """Équivalent Quat::from_rotation_arc(Vec3::Y, fwd) de Bevy."""
    return Vector((0, 1, 0)).rotation_difference(Vector(fwd).normalized())


def genome_hipfire_rotation():
    rx = math.radians(GENOME["rotation_x_deg"])
    ry = math.radians(GENOME["rotation_y_deg"] + GENOME["hipfire_tilt_y_deg"])
    rz = math.radians(GENOME["rotation_z_deg"])
    return (
        Quaternion((1, 0, 0), rx) @ Quaternion((0, 1, 0), ry) @ Quaternion((0, 0, 1), rz)
    )


def hand_transform(mirror):
    """Copie exacte de `position_hands` (hipfire, delta_rot = identité)."""
    gun = Vector((GENOME["offset_x"], GENOME["offset_y"], GENOME["offset_z"]))
    length = GENOME["target_size"]
    if mirror > 0.0:
        offset = Vector((ARMS["grip_x"], ARMS["grip_drop"], ARMS["grip_back"] * length))
        elbow_out = ARMS["grip_elbow_out"]
    else:
        offset = Vector((ARMS["barrel_x"], ARMS["barrel_drop"], -ARMS["barrel_fwd"] * length))
        elbow_out = ARMS["barrel_elbow_out"]
    wrist = gun + offset
    elbow = wrist + Vector((mirror * elbow_out, -ARMS["elbow_drop"], ARMS["elbow_back"]))
    rot = rot_arc_y_to(wrist - elbow)
    return bevy_trs(wrist, rot, ARMS.get("glb_scale", 1.0))


def main():
    bpy.ops.wm.read_factory_settings(use_empty=True)

    # Arme : import → autoscale AABB (comme auto_scale_viewmodel) → pose genome.
    weapon_roots = import_glb(WEAPON_GLB)
    extent = world_aabb_max_extent(weapon_roots)
    w_scale = GENOME["target_size"] / extent if extent > 1e-6 else 1.0
    gun = Vector((GENOME["offset_x"], GENOME["offset_y"], GENOME["offset_z"]))
    apply_bevy_transform(weapon_roots, bevy_trs(gun, genome_hipfire_rotation(), w_scale))
    print(f"[preview] weapon extent={extent:.3f} scale={w_scale:.4f}")

    # Bras : placement exact position_hands.
    arm_r = import_glb(ARM_R_GLB)
    apply_bevy_transform(arm_r, hand_transform(1.0))
    arm_l = import_glb(ARM_L_GLB)
    apply_bevy_transform(arm_l, hand_transform(-1.0))
    arm_roots = arm_r + arm_l

    # Repères-TUBES aux ancres calculées : l'utilisateur place/tourne le tube
    # comme la poignée réelle (position + AXE) ; `read_grip_markers.py` en déduit
    # l'ancre ET le roulis de la main (le C s'enroule autour de l'axe du tube).
    def marker(pos_bevy, rgba, name, upright):
        bpy.ops.mesh.primitive_cylinder_add(radius=0.022, depth=0.16)
        mo = bpy.context.active_object
        mo.name = name
        mo.location = (C4 @ Vector((*pos_bevy, 1.0)))[:3]
        if not upright:
            # Couché le long du canon (axe Bevy Z = Blender -Y).
            mo.rotation_euler = (math.pi / 2, 0, 0)
        mat = bpy.data.materials.new(name)
        mat.use_nodes = True
        try:
            mat.surface_render_method = "BLENDED"
        except (AttributeError, TypeError):
            pass
        # Par TYPE, pas par nom : le nom du node est localisé (UI française).
        for node in mat.node_tree.nodes:
            if node.type == "BSDF_PRINCIPLED":
                node.inputs["Base Color"].default_value = rgba
                node.inputs["Emission Color"].default_value = rgba
                node.inputs["Emission Strength"].default_value = 4.0
                node.inputs["Alpha"].default_value = 0.65
        mo.data.materials.append(mat)

    # Manche vertical (droite) = tube debout ; poignée avant (gauche) = tube
    # couché le long du canon. L'utilisateur ajuste position ET rotation.
    for mirror, col, tag, upright in (
        (1.0, (1, 0, 0, 1), "R", True),
        (-1.0, (0, 0.4, 1, 1), "L", False),
    ):
        anchor = hand_transform(mirror).to_translation()
        marker(tuple(anchor), col, f"MK_{tag}", upright)
        print(f"[preview] anchor {tag}: ({anchor.x:.3f}, {anchor.y:.3f}, {anchor.z:.3f})")

    # Caméra = FpsCamera Bevy : origine, -Z forward → Blender Rx(+90°), FOV vertical.
    scene = bpy.context.scene
    cam = bpy.data.objects.new("Cam", bpy.data.cameras.new("Cam"))
    scene.collection.objects.link(cam)
    scene.camera = cam
    cam.location = (0, 0, 0)
    cam.rotation_euler = (math.pi / 2, 0, 0)
    cam.data.sensor_fit = "VERTICAL"
    cam.data.angle_y = math.radians(FOV_DEG)
    cam.data.clip_start = 0.01

    sun = bpy.data.objects.new("Sun", bpy.data.lights.new("Sun", "SUN"))
    sun.data.energy = 3.0
    sun.rotation_euler = (1.0, 0.3, 0.4)
    scene.collection.objects.link(sun)
    fill = bpy.data.objects.new("Fill", bpy.data.lights.new("Fill", "AREA"))
    fill.data.energy = 80.0
    fill.data.size = 4.0
    fill.location = (0, -2, 1)
    fill.rotation_euler = (math.pi / 2, 0, 0)
    scene.collection.objects.link(fill)

    for eng in ("BLENDER_EEVEE_NEXT", "BLENDER_EEVEE", "CYCLES"):
        try:
            scene.render.engine = eng
            break
        except TypeError:
            continue
    scene.render.resolution_x, scene.render.resolution_y = 1280, 720
    try:
        scene.view_settings.view_transform = "Standard"
    except TypeError:
        pass

    # Vue joueur + vue de côté (debug placement, caméra décalée +X Bevy → +X bl).
    views = {
        "player": ((0, 0, 0), (math.pi / 2, 0, 0)),
        "side": ((1.4, 1.1, -0.35), None),  # Blender coords ; pointée par track
    }
    target_bl = C4 @ Vector((GENOME["offset_x"], GENOME["offset_y"], GENOME["offset_z"], 1.0))
    for name, (loc, rot) in views.items():
        cam.location = loc
        if rot is not None:
            cam.rotation_euler = rot
        else:
            d = Vector(target_bl[:3]) - Vector(loc)
            cam.rotation_euler = d.to_track_quat("-Z", "Y").to_euler()
        scene.render.filepath = os.path.join(OUT_DIR, f"ingame_{WEAPON_KEY}_{name}.png")
        bpy.ops.render.render(write_still=True)

    # Vue joueur FANTÔME : arme semi-transparente → contact mains↔poignées
    # lisible à travers le corps (debug placement uniquement).
    weapon_meshes = {
        o
        for r in weapon_roots
        for o in [r] + list(r.children_recursive)
        if o.type == "MESH"
    }
    for o in weapon_meshes:
        for slot in o.material_slots:
            mat = slot.material
            if not mat or not mat.use_nodes:
                continue
            # Blender 4.2+/5 EEVEE Next : surface_render_method (blend_method déprécié).
            try:
                mat.surface_render_method = "BLENDED"
            except (AttributeError, TypeError):
                pass
            for node in mat.node_tree.nodes:
                if node.type == "BSDF_PRINCIPLED":
                    node.inputs["Alpha"].default_value = 0.30
    cam.location = (0, 0, 0)
    cam.rotation_euler = (math.pi / 2, 0, 0)
    scene.render.filepath = os.path.join(OUT_DIR, f"ingame_{WEAPON_KEY}_ghost.png")
    bpy.ops.render.render(write_still=True)

    # Vue mains seules (arme masquée) : position exacte des mitaines + marqueurs.
    for o in weapon_meshes:
        o.hide_render = True
    scene.render.filepath = os.path.join(OUT_DIR, f"ingame_{WEAPON_KEY}_handsonly.png")
    bpy.ops.render.render(write_still=True)

    # Vue TUBES : arme translucide + tubes seuls (mains masquées — sinon les
    # tubes, situés aux paumes, sont cachés DANS les mains opaques). C'est la
    # vue de référence pour l'ajustement visuel des repères par l'utilisateur.
    for o in weapon_meshes:
        o.hide_render = False
    for r in arm_roots:
        for o in [r] + list(r.children_recursive):
            o.hide_render = True
    scene.render.filepath = os.path.join(OUT_DIR, f"ingame_{WEAPON_KEY}_tubes.png")
    bpy.ops.render.render(write_still=True)
    for r in arm_roots:
        for o in [r] + list(r.children_recursive):
            o.hide_render = False

    # Scène de calibration éditable : l'utilisateur déplace les sphères MK_R
    # (rouge, main crosse) / MK_L (bleue, main soutien) sur les poignées dans
    # Blender, sauvegarde, puis `read_grip_markers.py` retraduit en TOML.
    for o in weapon_meshes:
        o.hide_render = False
    blend_path = os.path.join(OUT_DIR, f"calibration_{WEAPON_KEY}.blend")
    bpy.ops.wm.save_as_mainfile(filepath=blend_path)
    print(f"[preview] calibration blend → {blend_path}")
    print("[preview] DONE")


main()
