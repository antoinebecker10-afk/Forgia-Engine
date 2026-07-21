"""cartoonize_arms.py — story-661 : cartoonise la base CC0 Drillimpact (PSX FPS arms).

Pipeline reproductible (Blender 5.0 headless) :
  1. Ouvre le .blend source, rend un « avant » (POV + closeup main).
  2. Grossit les mains ×HAND_SCALE : verts du mesh (falloff par poids de skinning
     autour du poignet) + head/tail des edit_bones de la région main.
  3. Swap texture → variante gants, posterisée en aplats (quantization + saturation
     + tinte chaude « forge ») ; matériau flat (metallic 0, roughness 1).
  4. Exporte le GLB final (mesh + rig + 18 anims) et rend un « après ».

Usage :
  blender --background --python tools/blender/cartoonize_arms.py -- \
      <src.blend> <gloves.png> <out.glb> <render_dir>
"""

import math
import os
import sys
import tomllib

import bpy
import numpy as np
from mathutils import Matrix, Quaternion, Vector

# ── Tuning cartoon (constantes du pipeline asset, pas du gameplay) ──
HAND_SCALE = 1.6        # grossissement mains+doigts (cartoon « grosses mitaines »)
RENDER_W, RENDER_H = 900, 650
POSE_ACTION, POSE_FRAME = "guard_idle", 26  # mains levées → idéal pour juger le viewmodel

# Pose de préhension bakée comme rest par côté (clip du pack, frame « fermée »)
# + roulis autour de l'axe avant-bras (degrés, calibré via preview_ingame.py) :
# paume vers l'arme. Itérés offline (WYSIWYG), pas de paramètre runtime.
# Frame 10 de grab.* = prise en C (planche contact : f6=tendue, f10=C qui
# enveloppe un cylindre, f12+=poing fermé qui ne « tient » rien — retour user).
# La poignée doit passer DANS le C → ancres TOML qui chevauchent l'arme.
# ⚠️ grab.L et grab.R ne sont PAS synchrones (clips animés séparément) :
# R : f10=C, f11=serré. L : f10=doigts tendus (= « main déformée »), f12=poing.
# → planches contact par côté obligatoires (sheet_grab_L/R_f*.png).
GRIP_POSE = {"R": ("grab.R", 11), "L": ("grab.L", 11)}
# Roulis = rotation RIGIDE autour de l'avant-bras (0 déformation) : il fait le
# maximum du chemin vers l'axe cible, wrist_align n'absorbe que le résidu
# (>60° de flexion bakée = coude cassé, vu sur la gauche à 75°).
# Valeurs = suggestions read_grip_markers (axes des tubes MK_R/MK_L).
ROLL_DEG = {"R": -75.0, "L": -69.0}
# FLEXION DU POIGNET : le roulis seul ne peut pas rendre le tunnel des doigts
# vertical (l'avant-bras monte en diagonale ; le tunnel vit dans le plan ⊥).
# On plie la MAIN au poignet pour aligner le tunnel des doigts sur l'axe de la
# poignée (Bevy world). R = manche vertical (tube rouge), L = canon horizontal
# (tube bleu). None = pas de flexion.
WRIST_ALIGN_AXIS = {"R": (0.0, 1.0, 0.0), "L": (0.0, 0.0, 1.0)}
# Arme de référence pour la direction d'avant-bras in-game (genome + tuning).
CAL_WEAPON = "bourrasque"
_ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))

# Rampes toon 3 tons (ombre, base, lumière) par zone — sRGB 0-1.
# Peau cartoon pêche saturée ; gants = cuir de forgeron brun-orangé chaud.
# Gant : lumière == base (la bande claire créait des mouchetures « dalmatien »).
SKIN_RAMP = [(0.66, 0.38, 0.26), (0.85, 0.55, 0.38), (0.94, 0.70, 0.52)]
GLOVE_RAMP = [(0.32, 0.13, 0.05), (0.60, 0.30, 0.12), (0.60, 0.30, 0.12)]
# Masque gant = différence entre arms_gloves_01 et arms_01 (même layout UV) :
# là où les 2 textures divergent fortement = pixels du gant. Bien plus robuste
# qu'un seuil de luminance (les ombres de peau ne divergent pas).
GLOVE_DIFF_THRESHOLD = 0.12

argv = sys.argv[sys.argv.index("--") + 1 :]
SRC_BLEND, GLOVES_PNG, OUT_GLB, RENDER_DIR = argv[:4]
os.makedirs(RENDER_DIR, exist_ok=True)


def find_objects():
    mesh = next(o for o in bpy.data.objects if o.type == "MESH" and o.vertex_groups)
    arm = next(o for o in bpy.data.objects if o.type == "ARMATURE")
    return mesh, arm


def is_hand_bone(name: str, side: str) -> bool:
    """Région main = hand + palm.* + f_* (doigts) + thumb.* — PAS forearm."""
    if not name.endswith("." + side):
        return False
    base = name[: -(len(side) + 1)]
    return base == "hand" or base.startswith(("palm", "f_", "thumb"))


def set_pose(arm):
    act = bpy.data.actions.get(POSE_ACTION)
    if act is None:
        return
    if arm.animation_data is None:
        arm.animation_data_create()
    arm.animation_data.action = act
    # Blender 4.4+ : actions à slots — binder le premier slot si présent.
    if getattr(act, "slots", None):
        try:
            arm.animation_data.action_slot = act.slots[0]
        except Exception:
            pass
    bpy.context.scene.frame_set(POSE_FRAME)


def pick_engine(scene):
    for eng in ("BLENDER_EEVEE_NEXT", "BLENDER_EEVEE", "CYCLES"):
        try:
            scene.render.engine = eng
            if eng == "CYCLES":
                scene.cycles.samples = 48
                scene.cycles.device = "CPU"
            return eng
        except TypeError:
            continue
    return scene.render.engine


def render_views(mesh, arm, tag):
    scene = bpy.context.scene
    engine = pick_engine(scene)
    scene.render.resolution_x = RENDER_W
    scene.render.resolution_y = RENDER_H
    # AgX (défaut) désature fortement les albedos → preview mensongère vs Bevy.
    try:
        scene.view_settings.view_transform = "Standard"
    except TypeError:
        pass

    # Monde gris-bleu neutre (lisibilité silhouette).
    world = bpy.data.worlds.get("PrevWorld") or bpy.data.worlds.new("PrevWorld")
    world.use_nodes = True
    bg = world.node_tree.nodes.get("Background")
    if bg:
        bg.inputs[0].default_value = (0.18, 0.20, 0.26, 1.0)
        bg.inputs[1].default_value = 1.0
    scene.world = world

    # Lumières : key (sun) + fill, une seule fois.
    if "PrevSun" not in bpy.data.objects:
        sun = bpy.data.objects.new("PrevSun", bpy.data.lights.new("PrevSun", "SUN"))
        sun.data.energy = 2.8
        sun.rotation_euler = (0.9, 0.2, 0.6)
        scene.collection.objects.link(sun)
        fill = bpy.data.objects.new("PrevFill", bpy.data.lights.new("PrevFill", "AREA"))
        fill.data.energy = 60.0
        fill.data.size = 3.0
        fill.location = (-1.5, -1.5, 1.0)
        fill.rotation_euler = (1.1, 0.0, -0.8)
        scene.collection.objects.link(fill)

    # Positions POSÉES (pose.bones, pas data.bones = rest) → cadre la pose réelle.
    bpy.context.view_layer.update()
    pb = arm.pose.bones
    mw = arm.matrix_world
    sh_mid = mw @ ((pb["shoulder.L"].head + pb["shoulder.R"].head) / 2)
    hd_mid = mw @ ((pb["hand.L"].head + pb["hand.R"].head) / 2)
    fwd = (hd_mid - sh_mid).normalized()
    up = Vector((0, 0, 1))

    cam = bpy.data.objects.get("PrevCam")
    if cam is None:
        cam = bpy.data.objects.new("PrevCam", bpy.data.cameras.new("PrevCam"))
        scene.collection.objects.link(cam)
    cam.data.lens = 32
    scene.camera = cam

    hand_r = mw @ pb["hand.R"].head
    views = {
        # POV joueur : derrière/au-dessus des épaules, regard vers les mains.
        "pov": (sh_mid - fwd * 0.30 + up * 0.25, hd_mid),
        # Closeup main droite 3/4.
        "hand": (hand_r + Vector((0.30, 0, 0.22)) - fwd * 0.18, hand_r),
    }
    for name, (pos, target) in views.items():
        cam.location = pos
        cam.rotation_euler = (target - pos).to_track_quat("-Z", "Y").to_euler()
        scene.render.filepath = os.path.join(RENDER_DIR, f"{tag}_{name}.png")
        bpy.ops.render.render(write_still=True)
    print(f"[cartoonize] rendus '{tag}' OK (engine={engine})")


def scale_hands(mesh, arm):
    """Grossit mains+doigts : verts (falloff par poids) + edit_bones, autour du poignet."""
    inv_mesh = mesh.matrix_world.inverted()
    pivots = {}
    for side in ("L", "R"):
        pivots[side] = inv_mesh @ (arm.matrix_world @ Vector(arm.data.bones["hand." + side].head_local))

    # Indices des vertex groups de la région main, par côté.
    hand_groups = {side: {g.index for g in mesh.vertex_groups if is_hand_bone(g.name, side)} for side in ("L", "R")}

    me = mesh.data
    for v in me.vertices:
        for side in ("L", "R"):
            w = sum(g.weight for g in v.groups if g.group in hand_groups[side])
            if w <= 0.0:
                continue
            t = min(w, 1.0)
            t = t * t * (3.0 - 2.0 * t)  # smoothstep → raccord doux au poignet
            factor = 1.0 + (HAND_SCALE - 1.0) * t
            v.co = pivots[side] + (v.co - pivots[side]) * factor

    # Squelette : positions des bones de la main autour du même pivot (armature-local).
    # Bones connectés : la tête suit automatiquement la queue du parent → ne poser
    # la tête que sur les bones NON connectés (sinon le snap Blender crée des
    # incohérences tête/queue → mesh déchiqueté sous pose).
    bpy.context.view_layer.objects.active = arm
    # PIÈGE (diagnostiqué story-661) : si le .blend a « X-Axis Mirror » actif,
    # chaque édition d'un bone .L est répliquée sur le .R → nos 2 passes L/R
    # composaient un ×S² (mesh déchiqueté). On coupe le mirror avant d'éditer.
    arm.data.use_mirror_x = False
    bpy.ops.object.mode_set(mode="EDIT")
    ebs = arm.data.edit_bones
    for side in ("L", "R"):
        pivot = Vector(ebs["hand." + side].head)
        targets = [eb for eb in ebs if is_hand_bone(eb.name, side)]
        new_pos = {
            eb.name: (pivot + (eb.head - pivot) * HAND_SCALE, pivot + (eb.tail - pivot) * HAND_SCALE)
            for eb in targets
        }
        for eb in targets:
            head, tail = new_pos[eb.name]
            if not eb.use_connect:
                eb.head = head
            eb.tail = tail
    bpy.ops.object.mode_set(mode="OBJECT")
    print(f"[cartoonize] mains x{HAND_SCALE} appliqué (mesh + rig)")


def _box_blur(chan2d, passes=2):
    """Box blur 3×3 (numpy, wrap) — dé-bruite avant classification en zones."""
    for _ in range(passes):
        acc = np.zeros_like(chan2d)
        for dy in (-1, 0, 1):
            for dx in (-1, 0, 1):
                acc += np.roll(np.roll(chan2d, dy, axis=0), dx, axis=1)
        chan2d = acc / 9.0
    return chan2d


def _read_rgba(img):
    w, h = img.size
    arr = np.empty(w * h * 4, dtype=np.float32)
    img.pixels.foreach_get(arr)
    return arr.reshape(h, w, 4)


def toonify_image(gloves_img, skin_img):
    """Texture photo-peinte → aplats toon. Masque gant = diff gloves vs skin
    (même layout UV), puis rampe 3 tons par zone (seuils = percentiles intra-zone
    sur la luminance floutée)."""
    px = _read_rgba(gloves_img)
    skin_px = _read_rgba(skin_img)
    if skin_px.shape != px.shape:
        # Tailles différentes (ex. 512² vs 1024²) → upscale entier du plus petit.
        ry = px.shape[0] // skin_px.shape[0]
        rx = px.shape[1] // skin_px.shape[1]
        skin_px = np.repeat(np.repeat(skin_px, ry, axis=0), rx, axis=1)
    diff = np.abs(px[:, :, :3] - skin_px[:, :, :3]).mean(axis=2)
    glove = _box_blur(diff) > GLOVE_DIFF_THRESHOLD

    lum = _box_blur(px[:, :, :3] @ np.array([0.299, 0.587, 0.114], np.float32), passes=4)
    out = np.empty((px.shape[0], px.shape[1], 3), dtype=np.float32)
    # Aplats « base dominante » : la base couvre la zone ; l'ombre/la lumière ne
    # marquent que les extrêmes (percentiles par zone). Gant = 100 % plat (les
    # blobs d'ombre lisent comme de la saleté ; le facettage low-poly suffit).
    # (Banding 33/33/33 = camouflage marbré — testé, rejeté.)
    zones = ((glove, GLOVE_RAMP, 0, 100), (~glove, SKIN_RAMP, 12, 93))
    for mask, ramp, s_pct, l_pct in zones:
        if not mask.any():
            continue
        zone = lum[mask]
        band = np.ones(zone.shape, dtype=np.int64)
        if s_pct > 0:
            band[zone < np.percentile(zone, s_pct)] = 0
        if l_pct < 100:
            band[zone > np.percentile(zone, l_pct)] = 2
        out[mask] = np.array(ramp, dtype=np.float32)[band]

    px[:, :, :3] = out
    gloves_img.pixels.foreach_set(px.ravel())
    gloves_img.pack()  # embarque la version modifiée dans le GLB


def flat_material(mesh):
    """Texture gants posterisée + Principled flat (aplats, zéro reflet)."""
    gloves = bpy.data.images.load(GLOVES_PNG)
    skin_src = os.path.join(os.path.dirname(GLOVES_PNG), "arms_01.png")
    toonify_image(gloves, bpy.data.images.load(skin_src))
    for mat in mesh.data.materials:
        if not mat or not mat.use_nodes:
            continue
        for node in mat.node_tree.nodes:
            if node.type == "TEX_IMAGE":
                node.image = gloves
            elif node.type == "BSDF_PRINCIPLED":
                node.inputs["Metallic"].default_value = 0.0
                node.inputs["Roughness"].default_value = 1.0
    print("[cartoonize] matériau flat + texture gants posterisée")


def _ingame_forearm_dir(side):
    """Direction coude→poignet in-game (Bevy), répliquée de `position_hands`
    (fps_tuning [viewmodel_arms] + genome de l'arme de référence)."""
    with open(os.path.join(_ROOT, "assets/genomes/fps_tuning.toml"), "rb") as f:
        arms_t = tomllib.load(f)["viewmodel_arms"]
    with open(os.path.join(_ROOT, "assets/genomes/viewmodel_arena.toml"), "rb") as f:
        genome = tomllib.load(f)["weapons"][CAL_WEAPON]
    length = genome["target_size"]
    mirror = 1.0 if side == "R" else -1.0
    if mirror > 0.0:
        elbow_out = arms_t["grip_elbow_out"]
    else:
        elbow_out = arms_t["barrel_elbow_out"]
    # fwd = wrist - elbow ; l'ancre s'annule dans la différence.
    fwd = -Vector((mirror * elbow_out, -arms_t["elbow_drop"], arms_t["elbow_back"]))
    _ = length  # (les fractions d'ancre n'influent pas sur fwd)
    return fwd.normalized()


def wrist_align(mesh, arm, side):
    """Flexion du poignet bakée : rotation de la région MAIN (verts pondérés +
    bones) autour de l'origine paume pour que le tunnel des doigts (axe X·m
    après roulis) s'aligne in-game sur l'axe du manche (WRIST_ALIGN_AXIS)."""
    axis = WRIST_ALIGN_AXIS[side]
    if axis is None:
        return
    m = 1.0 if side == "R" else -1.0
    fwd = _ingame_forearm_dir(side)
    arc = Vector((0, 1, 0)).rotation_difference(fwd)
    handle_ent = arc.inverted() @ Vector(axis)  # axe manche en espace entité
    tunnel_ent = Quaternion((0, 1, 0), math.radians(ROLL_DEG[side])) @ Vector((m, 0, 0))
    # Un axe de cylindre n'a pas de sens : prendre l'orientation la plus proche.
    if tunnel_ent.dot(handle_ent) < 0.0:
        handle_ent = -handle_ent
    qfix_ent = tunnel_ent.rotation_difference(handle_ent)  # Bevy entité
    angle = math.degrees(qfix_ent.angle)
    if angle < 3.0:
        return
    # Bevy → Blender : conjugaison par C (x,y,z)→(x,-z,y).
    c3 = Matrix(((1, 0, 0), (0, 0, -1), (0, 1, 0)))
    q_bl = (c3 @ qfix_ent.to_matrix() @ c3.inverted()).to_quaternion()

    # Verts : rotation pondérée par le poids RELATIF main/(main+reste) autour de
    # l'origine tunnel. ⚠️ PAS le poids absolu : les poids du pack ne sont pas
    # normalisés (bout de doigt à 0,7) → rotation partielle → doigts « dévrillés »
    # (main déformée, retour user 2026-07-20). Relatif : doigts=1 (rigides),
    # seule la vraie zone partagée du poignet plie.
    hand_groups = {g.index for g in mesh.vertex_groups if is_hand_bone(g.name, side)}
    ident = Quaternion()
    for v in mesh.data.vertices:
        w_hand = sum(g.weight for g in v.groups if g.group in hand_groups)
        if w_hand <= 0.0:
            continue
        w_other = sum(g.weight for g in v.groups if g.group not in hand_groups)
        t = w_hand / (w_hand + w_other) if (w_hand + w_other) > 1e-6 else 1.0
        t = t * t * (3.0 - 2.0 * t)
        v.co = ident.slerp(q_bl, t) @ v.co

    # Bones main : rotation pleine autour de l'origine (cohérent verts w=1).
    bpy.context.view_layer.objects.active = arm
    arm.data.use_mirror_x = False
    bpy.ops.object.mode_set(mode="EDIT")
    ebs = arm.data.edit_bones
    targets = [eb for eb in ebs if is_hand_bone(eb.name, side)]
    new_pos = {eb.name: (q_bl @ eb.head, q_bl @ eb.tail) for eb in targets}
    for eb in targets:
        head, tail = new_pos[eb.name]
        if not eb.use_connect:
            eb.head = head
        eb.tail = tail
    bpy.ops.object.mode_set(mode="OBJECT")
    print(f"[cartoonize] wrist_align {side}: {angle:.0f}° (tunnel → axe manche)")


def export_glb(mesh, arm, path, animations=True):
    bpy.ops.object.select_all(action="DESELECT")
    mesh.select_set(True)
    arm.select_set(True)
    bpy.context.view_layer.objects.active = arm
    bpy.ops.export_scene.gltf(
        filepath=path,
        export_format="GLB",
        use_selection=True,
        export_animations=animations,
        export_skins=True,
        export_yup=True,
    )
    print(f"[cartoonize] export → {path}")


def isolate_side(mesh, arm, keep):
    """Supprime l'autre bras ET l'épaule/bras supérieur du côté gardé (viewmodel
    FPS classique = avant-bras seulement ; sinon le coude plié « casse » à
    l'écran). Verts par poids dominant, puis bones. Root conservé."""
    import bmesh

    other = "R" if keep == "L" else "L"

    def group_set(pred):
        return {g.index for g in mesh.vertex_groups if pred(g.name)}

    # Coupe au COUDE (retour user 2026-07-20 : « il manque les bras ») : on garde
    # l'avant-bras, on supprime épaule + bras supérieur (leur flexion en rest
    # créait le coude cassé à l'écran).
    drop_groups = group_set(lambda n: n.endswith("." + other)) | group_set(
        lambda n: n in (f"shoulder.{keep}", f"upper_arm.{keep}")
    )
    keep_groups = group_set(lambda n: n.endswith("." + keep)) - drop_groups
    bm = bmesh.new()
    bm.from_mesh(mesh.data)
    layer = bm.verts.layers.deform.active
    doomed = []
    for v in bm.verts:
        dw = v[layer]
        wo = sum(w for gi, w in dw.items() if gi in drop_groups)
        wk = sum(w for gi, w in dw.items() if gi in keep_groups)
        if wo > wk:
            doomed.append(v)
    bmesh.ops.delete(bm, geom=doomed, context="VERTS")
    bm.to_mesh(mesh.data)
    bm.free()

    bpy.context.view_layer.objects.active = arm
    arm.data.use_mirror_x = False  # même piège que scale_hands
    bpy.ops.object.mode_set(mode="EDIT")
    for eb in list(arm.data.edit_bones):
        if eb.name.endswith("." + other) or eb.name in (
            f"shoulder.{keep}",
            f"upper_arm.{keep}",
        ):
            arm.data.edit_bones.remove(eb)
    bpy.ops.object.mode_set(mode="OBJECT")


def bake_pose_as_rest(mesh, arm, action_name, frame, side):
    """Fige la pose MAIN de `action@frame` comme nouvelle pose de repos.
    Seuls les bones de la région main (hand/palm/doigts/pouce) gardent la pose —
    épaule/coude/avant-bras reset identité (sinon la flexion de bras de l'anim
    est bakée et l'avant-bras part en travers — vu itération 2).
    ⚠️ invalide les autres clips (autorisé : les GLB par-côté sont statiques,
    Inc.2 ré-exportera depuis le rest d'origine)."""
    act = bpy.data.actions.get(action_name)
    if act is None:
        print(f"[cartoonize] WARN pose '{action_name}' absente — rest inchangé")
        return
    if arm.animation_data is None:
        arm.animation_data_create()
    arm.animation_data.action = act
    if getattr(act, "slots", None):
        try:
            arm.animation_data.action_slot = act.slots[0]
        except Exception:
            pass
    bpy.context.scene.frame_set(frame)
    bpy.context.view_layer.update()

    # Ne garder que la pose des bones MAIN, puis figer.
    saved = {pb.name: pb.matrix_basis.copy() for pb in arm.pose.bones}
    arm.animation_data.action = None
    for pb in arm.pose.bones:
        if is_hand_bone(pb.name, side):
            pb.matrix_basis = saved[pb.name]
        else:
            pb.matrix_basis = Matrix.Identity(4)
    bpy.context.view_layer.update()

    # Capture le mesh déformé par la pose filtrée, puis applique-la comme rest.
    dg = bpy.context.evaluated_depsgraph_get()
    mesh.data = bpy.data.meshes.new_from_object(mesh.evaluated_get(dg))
    bpy.context.view_layer.objects.active = arm
    bpy.ops.object.mode_set(mode="POSE")
    bpy.ops.pose.armature_apply(selected=False)
    bpy.ops.object.mode_set(mode="OBJECT")


def normalize_arm(mesh, arm, side):
    """Bake la convention viewmodel des poings procéduraux (espace Bevy/glTF :
    poignet à l'origine, avant-bras vers -Y, doigts +Y, paume +Z, pouce ±X).
    En axes Blender (glTF export : X→X, Y→-Z, Z→Y) : axe coude→poignet → +Z_bl,
    pouce → ±X_bl, paume → -Y_bl."""
    from mathutils import Matrix

    m = 1.0 if side == "R" else -1.0
    bones = arm.data.bones
    wrist = Vector(bones["hand." + side].head_local)
    elbow = Vector(bones["forearm." + side].head_local)
    y_s = (wrist - elbow).normalized()
    t = Vector(bones["thumb.01." + side].head_local) - wrist
    x_s = (t - t.dot(y_s) * y_s).normalized()
    z_s = x_s.cross(y_s)
    x_t = Vector((m, 0.0, 0.0))
    y_t = Vector((0.0, 0.0, 1.0))
    z_t = x_t.cross(y_t)
    src = Matrix((x_s, y_s, z_s)).transposed()   # colonnes = base source
    dst = Matrix((x_t, y_t, z_t)).transposed()
    rot = (dst @ src.inverted()).to_4x4()
    # Roulis calibré autour de l'axe avant-bras (= +Z Blender après alignement).
    roll = Matrix.Rotation(math.radians(ROLL_DEG[side]), 4, "Z")
    # Origine = CENTRE DU TUNNEL DES DOIGTS (pas la paume ! diagnostic
    # check_tunnel 2026-07-20 : avec l'origine paume, le tube passe derrière le
    # dos de la main et les doigts se referment à côté). En pose de prise, le
    # tunnel ≈ moyenne des milieux [phalange 01 → phalange 03] des 4 doigts —
    # c'est LÀ que la poignée doit passer.
    fingers = ("f_index", "f_middle", "f_ring", "f_pinky")
    pts = []
    for f in fingers:
        b1 = bones.get(f"{f}.01.{side}")
        b3 = bones.get(f"{f}.03.{side}")
        if b1 and b3:
            pts.append((Vector(b1.head_local) + Vector(b3.head_local)) / 2.0)
    tunnel = sum(pts, Vector((0, 0, 0))) / len(pts)
    xform = roll @ rot @ Matrix.Translation(-tunnel)

    # Le parent mesh→armature ne sert qu'à l'organisation (le skinning passe par
    # le modifier) ; on le retire en gardant le transform pour appliquer le même
    # transform world aux deux objets.
    bpy.ops.object.select_all(action="DESELECT")
    mesh.select_set(True)
    bpy.context.view_layer.objects.active = mesh
    bpy.ops.object.parent_clear(type="CLEAR_KEEP_TRANSFORM")
    for o in (mesh, arm):
        o.matrix_world = xform @ o.matrix_world
    bpy.ops.object.select_all(action="DESELECT")
    mesh.select_set(True)
    arm.select_set(True)
    bpy.context.view_layer.objects.active = arm
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)


def main():
    bpy.ops.wm.open_mainfile(filepath=SRC_BLEND)
    mesh, arm = find_objects()
    # Le .blend contient des objets de démo/rig hors export (knife_dummy parenté
    # au bone handIK.R, cubes de scène, custom shapes WGT-*) → masqués au rendu.
    for o in bpy.data.objects:
        if o not in (mesh, arm):
            o.hide_render = True
    set_pose(arm)
    render_views(mesh, arm, "before")

    scale_hands(mesh, arm)
    flat_material(mesh)
    export_glb(mesh, arm, OUT_GLB)
    render_views(mesh, arm, "after")

    # Rendu bind pose forcée (sanity : isole un défaut de rig/skinning d'un
    # défaut d'interaction avec l'animation). pose_position, PAS action=None
    # (vider l'action ne reset pas la pose évaluée).
    arm.data.pose_position = "REST"
    render_views(mesh, arm, "after_rest")
    arm.data.pose_position = "POSE"

    # Exports par-côté pour le viewmodel in-game (story-661 Inc.1) : chaque main
    # est posée indépendamment sur l'arme par `position_hands` → 1 GLB par bras,
    # normalisé à la convention des poings procéduraux.
    work = os.path.join(RENDER_DIR, "_work_cartoon.blend")
    bpy.ops.wm.save_as_mainfile(filepath=work)
    out_dir = os.path.dirname(OUT_GLB)
    for side in ("L", "R"):
        bpy.ops.wm.open_mainfile(filepath=work)
        mesh, arm = find_objects()
        isolate_side(mesh, arm, side)
        bake_pose_as_rest(mesh, arm, *GRIP_POSE[side], side)
        normalize_arm(mesh, arm, side)
        wrist_align(mesh, arm, side)
        # SANS animations : le re-rest (pose grip bakée) les invalide, et un GLB
        # avec anims fait mentir les previews Blender (l'importeur laisse la
        # dernière action active → pose aléatoire ; Bevy lui n'auto-joue rien).
        export_glb(mesh, arm, os.path.join(out_dir, f"fps_arm_{side}.glb"), animations=False)
        print(
            "[cartoonize] side %s: dims=%s verts=%d"
            % (side, tuple(round(d, 3) for d in mesh.dimensions), len(mesh.data.vertices))
        )
    os.remove(work)
    print("[cartoonize] DONE")


main()
