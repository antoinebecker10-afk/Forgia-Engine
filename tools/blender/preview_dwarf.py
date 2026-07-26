"""Planche de contrôle du nain — et surtout, rendu FIDÈLE À CE QUE LE JEU AFFICHE.

Recharge les GLB exportés (donc ce que Bevy verra réellement, pas la scène
d'auteur), applique les masques de sous-mesh déclarés dans le manifest, et rend
trois instances côte à côte pour juger silhouette + lecture de l'armure.

Trois modes :
  (défaut)      trois vues, PBR brut
  --toon        + réplique de `assets/shaders/post_process/toon.wgsl` dans le
                compositeur, paramétrée par `roguelite_toon.toml`. En mode
                Roguelite le personnage passe par ce post-process : juger l'asset
                en PBR pur, c'est juger dans le mauvais moteur.
  --silhouette  aplat noir sur fond blanc à trois tailles apparentes
                (~5 m / 15 m / 50 m). Test de lisibilité utilisé en production :
                un personnage qui ne se reconnaît pas à sa silhouette a un
                problème de forme qu'aucune texture ne rattrapera.

`view_transform = Standard` : l'AgX par défaut délave les aplats cartoon.
Cycles et pas EEVEE : EEVEE headless crashe sans GPU/viewport.

Usage :
  blender --background --factory-startup --python tools/blender/preview_dwarf.py -- \
    --dir assets/models/characters/dwarf --out <png> [--toon] [--silhouette]
"""

import argparse
import json
import math
import sys
import tomllib
from pathlib import Path

import bpy
from mathutils import Euler, Matrix, Vector

sys.path.insert(0, str(Path(__file__).resolve().parent))
import dwarf_anim  # noqa: E402  (dépend du sys.path ci-dessus)

TOON_GENOME = Path("assets/genomes/roguelite/roguelite_toon.toml")

# Pose de présentation, en degrés (X, Y, Z) exprimés dans le repère de
# l'ARMATURE, pas dans le repère local de chaque os (cf. `bone_local()`).
# Convention : le nain regarde +Y, donc une rotation X POSITIVE ramène un membre
# pendant vers l'AVANT. Les valeurs négatives de la version précédente
# rabattaient les bras derrière le dos — bug signalé par Antoine.
#
# La pose n'est PAS exportée : le GLB garde une rest pose neutre, seul le rendu
# de contrôle est posé. Une A-pose lit toujours « asset », jamais
# « personnage » — mais figer une pose campée dans le bind compliquerait tout
# retarget d'animation.
POSE = {
    "Spine": (-4.0, 0.0, 0.0),
    "Spine1": (-3.0, 0.0, 2.0),
    "Spine2": (-2.0, 0.0, 0.0),
    "Neck": (4.0, 0.0, 0.0),
    "Head": (3.0, 0.0, -4.0),
    "LeftShoulder": (0.0, 0.0, -4.0),
    "LeftArm": (16.0, 0.0, -8.0),
    "LeftForeArm": (30.0, 0.0, 0.0),
    "LeftHand": (8.0, 0.0, 0.0),
    "RightShoulder": (0.0, 0.0, 4.0),
    "RightArm": (13.0, 0.0, 8.0),
    "RightForeArm": (25.0, 0.0, 0.0),
    "RightHand": (6.0, 0.0, 0.0),
    "LeftUpLeg": (9.0, 0.0, -3.0),
    "LeftLeg": (-14.0, 0.0, 0.0),
    "RightUpLeg": (-6.0, 0.0, 3.0),
    "RightLeg": (5.0, 0.0, 0.0),
}


def bone_local(pose_bone, angles_deg):
    """Convertit une rotation exprimée en repère ARMATURE vers le repère de l'os.

    Sans ça il faut deviner l'axe local de chaque os — et se tromper de signe
    envoie les bras derrière le dos. `matrix_local` mappe local → armature,
    donc la rotation locale équivalente est M⁻¹ · R · M.
    """
    rest = pose_bone.bone.matrix_local.to_3x3()
    world = Euler([math.radians(a) for a in angles_deg], "XYZ").to_matrix()
    return (rest.inverted() @ world @ rest).to_quaternion()


def cli_args():
    if "--" not in sys.argv:
        raise SystemExit("Arguments attendus après `--`.")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dir", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--samples", type=int, default=64)
    parser.add_argument("--res", nargs=2, type=int, default=[1680, 1120])
    parser.add_argument("--ortho", type=float, default=2.85)
    parser.add_argument("--no-pose", action="store_true", help="rendre en rest pose brute")
    parser.add_argument("--toon", action="store_true", help="appliquer le post-process du jeu")
    parser.add_argument("--silhouette", action="store_true", help="test de lisibilité à 3 distances")
    parser.add_argument("--clip", choices=sorted(dwarf_anim.CLIPS), help="planche des poses clés d'un cycle")
    return parser.parse_args(sys.argv[sys.argv.index("--") + 1 :])


# ---------------------------------------------------------------- chargement


def apply_pose(objs):
    for obj in objs:
        if obj.type != "ARMATURE":
            continue
        for name, angles in POSE.items():
            bone = obj.pose.bones.get(name)
            if bone is None:
                continue
            bone.rotation_mode = "QUATERNION"
            bone.rotation_quaternion = bone_local(bone, angles)


def spawn(glbs, offset, rot_z_deg, hidden_parts, scale=1.0):
    """Importe des GLB, supprime les sous-mesh masqués, pose l'ensemble.

    `offset` est un vecteur : la planche de cycle s'échelonne en Y (vue de
    profil) là où les autres s'échelonnent en X.
    """
    before = set(bpy.data.objects)
    for glb in glbs:
        bpy.ops.import_scene.gltf(filepath=str(glb.resolve()))
    fresh = [o for o in bpy.data.objects if o not in before]

    for obj in list(fresh):
        # l'importeur suffixe en cas de collision de nom (head.001) → base name
        if obj.type == "MESH" and obj.name.split(".")[0] in hidden_parts:
            fresh.remove(obj)
            bpy.data.objects.remove(obj, do_unlink=True)
        elif obj.type == "ARMATURE":
            # 🚨 l'importeur glTF laisse la DERNIÈRE action importée active :
            # sans ce reset, le GLB animé rendrait une pose arbitraire
            obj.animation_data_clear()
            for bone in obj.pose.bones:
                bone.matrix_basis.identity()

    placement = (
        Matrix.Translation(Vector(offset))
        @ Matrix.Rotation(math.radians(rot_z_deg), 4, "Z")
        @ Matrix.Scale(scale, 4)
    )
    for obj in fresh:
        if obj.parent is None:
            obj.matrix_world = placement @ obj.matrix_world
    return fresh


def load_manifest(root):
    manifest = json.loads((root / "manifest.json").read_text(encoding="utf-8"))
    body = root / manifest["body"]["glb"]
    slots = [root / s["glb"] for s in manifest["slots"].values()]
    hidden = {p for s in manifest["slots"].values() for p in s["hides"]}
    return body, slots, hidden


# ------------------------------------------------------- rendu fidèle au jeu


def toon_params():
    """Lit les défauts du génome — mêmes valeurs que celles servies en jeu.

    Rappel de l'état réel : `strength` a été bridé 1.0 → 0.6 et `edge_dark`
    0.15 → 0.0 le 2026-05-29 pour éviter un écran noir sur la scène Roguelite.
    Ce sont donc les valeurs à répliquer, pas les valeurs « pleines ».
    """
    path = Path.cwd() / TOON_GENOME
    if not path.exists():
        print(f"[preview] génome toon introuvable ({path}), valeurs de repli")
        return {"bands": 4.0, "strength": 0.6, "edge_dark": 0.0}
    genes = tomllib.loads(path.read_text(encoding="utf-8")).get("genes", [])
    by_id = {g["id"]: g.get("default") for g in genes}
    return {
        "bands": float(by_id.get("roguelite_toon_bands", 4.0)),
        "strength": float(by_id.get("roguelite_toon_strength", 0.6)),
        "edge_dark": float(by_id.get("roguelite_toon_edge_dark", 0.0)),
    }


def setup_toon_compositor(params):
    """Réplique `toon.wgsl` : quantification de la luminance en N bandes.

    Le shader travaille sur la couleur ÉCRAN (déjà encodée pour l'affichage),
    le compositeur sur du linéaire scene-referred. D'où l'encadrement par deux
    nœuds Gamma : sans lui les bandes tomberaient au mauvais endroit et la
    preview mentirait autant qu'avant.
    """
    scene = bpy.context.scene
    scene.render.use_compositing = True

    # Blender 5.0 : le compositeur est un NODE GROUP (`scene.node_tree` a
    # disparu), et Composite/MixRGB/Math/Gamma « Compositor » ont été remplacés
    # par les nœuds shader unifiés + un NodeGroupOutput.
    tree = bpy.data.node_groups.new("dwarf_toon", "CompositorNodeTree")
    tree.interface.new_socket("Image", in_out="OUTPUT", socket_type="NodeSocketColor")
    scene.compositing_node_group = tree

    def sock(node, name, kind, out=False):
        """Mix/Math exposent plusieurs sockets homonymes, un par type."""
        for s in node.outputs if out else node.inputs:
            if s.name == name and s.type == kind:
                return s
        raise KeyError(f"{node.bl_idname}: socket {name}/{kind} introuvable")

    def math(op, a=None, b=None):
        node = tree.nodes.new("ShaderNodeMath")
        node.operation = op
        for idx, value in ((0, a), (1, b)):
            if value is None:
                continue
            if hasattr(value, "node"):
                tree.links.new(value, node.inputs[idx])
            else:
                node.inputs[idx].default_value = value
        return node.outputs["Value"]

    def mix_rgb(blend, factor, color_a, color_b):
        node = tree.nodes.new("ShaderNodeMix")
        node.data_type = "RGBA"
        node.blend_type = blend
        if hasattr(factor, "node"):
            tree.links.new(factor, sock(node, "Factor", "VALUE"))
        else:
            sock(node, "Factor", "VALUE").default_value = factor
        tree.links.new(color_a, sock(node, "A", "RGBA"))
        tree.links.new(color_b, sock(node, "B", "RGBA"))
        return sock(node, "Result", "RGBA", out=True)

    bands = max(params["bands"], 2.0)
    src = tree.nodes.new("CompositorNodeRLayers")

    encode = tree.nodes.new("ShaderNodeGamma")
    encode.inputs["Gamma"].default_value = 1.0 / 2.2
    tree.links.new(src.outputs["Image"], encode.inputs["Color"])
    screen = encode.outputs["Color"]

    bw = tree.nodes.new("CompositorNodeRGBToBW")
    tree.links.new(screen, bw.inputs["Image"])
    lum = bw.outputs["Val"]

    quantized = math("DIVIDE", math("FLOOR", math("MULTIPLY", lum, bands)), bands)
    ratio = math("DIVIDE", quantized, math("MAXIMUM", lum, 0.001))
    toon = mix_rgb("MULTIPLY", 1.0, screen, ratio)

    # assombrissement de la bande la plus basse (edge_dark)
    dark_fac = math("SUBTRACT", 1.0, math("MULTIPLY", math("LESS_THAN", quantized, 1.0 / bands), params["edge_dark"]))
    darkened = mix_rgb("MULTIPLY", 1.0, toon, dark_fac)

    blended = mix_rgb("MIX", params["strength"], screen, darkened)

    decode = tree.nodes.new("ShaderNodeGamma")
    decode.inputs["Gamma"].default_value = 2.2
    tree.links.new(blended, decode.inputs["Color"])

    out = tree.nodes.new("NodeGroupOutput")
    tree.links.new(decode.outputs["Color"], out.inputs[0])


# ------------------------------------------------------------------- scènes


def base_render_settings(args):
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    scene.cycles.samples = args.samples
    scene.cycles.device = "CPU"
    scene.render.resolution_x, scene.render.resolution_y = args.res
    scene.view_settings.view_transform = "Standard"  # AgX délave les aplats
    return scene


def setup_lighting(scene, yaw_deg=0.0):
    """`yaw_deg` fait tourner tout le dispositif avec la caméra.

    Sans ça, une vue de profil hérite d'un éclairage calé pour la vue de face :
    le visage tombe dans l'ombre et la lumière de braise, censée détourer la
    silhouette, arrive de la mauvaise direction.
    """
    yaw = math.radians(yaw_deg)
    spin = Matrix.Rotation(yaw, 4, "Z")
    world = bpy.data.worlds.new("w")
    scene.world = world
    world.use_nodes = True
    for node in world.node_tree.nodes:
        if node.type == "BACKGROUND":
            node.inputs[0].default_value = (0.055, 0.048, 0.070, 1.0)  # aubergine
            node.inputs[1].default_value = 0.80

    key = bpy.data.objects.new("key", bpy.data.lights.new("key", "SUN"))
    key.data.energy = 2.6
    key.data.angle = math.radians(12.0)
    key.rotation_euler = (math.radians(58), 0.0, math.radians(212) + yaw)
    scene.collection.objects.link(key)

    rim = bpy.data.objects.new("rim", bpy.data.lights.new("rim", "AREA"))
    rim.data.energy = 150.0
    rim.data.size = 2.5
    rim.data.color = (1.0, 0.62, 0.30)  # braise, décolle du fond
    rim.location = spin @ Vector((0.4, -2.6, 1.9))
    rim.rotation_euler = (math.radians(122), 0.0, math.radians(8) + yaw)
    scene.collection.objects.link(rim)

    fill = bpy.data.objects.new("fill", bpy.data.lights.new("fill", "AREA"))
    fill.data.energy = 55.0
    fill.data.size = 3.0
    fill.location = spin @ Vector((-2.2, 2.4, 1.2))
    fill.rotation_euler = (math.radians(76), 0.0, math.radians(-138) + yaw)
    scene.collection.objects.link(fill)


def setup_camera(scene, ortho, center, center_z, side=False):
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    cam.data.type = "ORTHO"
    cam.data.ortho_scale = ortho
    if side:
        # profil : caméra en +X regardant -X. Le nain regarde +Y, donc il avance
        # vers la droite de l'image — seul angle où un cycle de marche se lit.
        cam.location = (6.0, center, center_z)
        cam.rotation_euler = (math.radians(90), 0.0, math.radians(90))
    else:
        # le nain regarde +Y : caméra placée en +Y, tournée vers -Y
        cam.location = (center, 6.0, center_z)
        cam.rotation_euler = (math.radians(90), 0.0, math.radians(180))
    scene.collection.objects.link(cam)
    scene.camera = cam


def build_showcase(args, body, slots, hidden):
    # la caméra regarde vers -Y : l'axe X est inversé à l'écran, d'où les
    # offsets à l'envers pour lire « nu | équipé face | équipé 3/4 »
    instances = [
        spawn([body], (0.88, 0, 0), 0.0, set()),
        spawn([body] + slots, (0, 0, 0), 0.0, set(hidden)),
        spawn([body] + slots, (-0.90, 0, 0), 148.0, set(hidden)),
    ]
    if not args.no_pose:
        for fresh in instances:
            apply_pose(fresh)
    scene = base_render_settings(args)
    setup_lighting(scene)
    setup_camera(scene, args.ortho, 0.0, 0.74)
    if args.toon:
        setup_toon_compositor(toon_params())


def build_silhouette(args, body, slots, hidden):
    """Trois tailles apparentes, aplat noir sur blanc.

    Ce n'est pas un rendu joli : c'est un GATE. Si les trois lectures ne sont
    pas identifiables, le problème est dans la forme, pas dans la matière.
    """
    # X est inversé à l'écran (caméra en +Y) → grand à gauche = +X le plus grand
    for offset_x, scale in ((0.86, 1.0), (-0.26, 1.0 / 3.0), (-0.80, 1.0 / 10.0)):
        fresh = spawn([body] + slots, (offset_x, 0, 0), 0.0, set(hidden), scale=scale)
        if not args.no_pose:
            apply_pose(fresh)

    scene = base_render_settings(args)
    scene.cycles.samples = max(8, args.samples // 4)  # un aplat n'a pas de bruit
    # `ortho_scale` cadre la PLUS GRANDE dimension : en 1680×700 la hauteur ne
    # faisait que 0.96 m et coupait un nain de 1.45 m. Format imposé ici.
    scene.render.resolution_x, scene.render.resolution_y = 1600, 900

    world = bpy.data.worlds.new("sil")
    scene.world = world
    world.use_nodes = True
    for node in world.node_tree.nodes:
        if node.type == "BACKGROUND":
            node.inputs[0].default_value = (1.0, 1.0, 1.0, 1.0)
            node.inputs[1].default_value = 1.0

    flat = bpy.data.materials.new("silhouette")
    flat.use_nodes = True
    flat.node_tree.nodes.clear()
    emit = flat.node_tree.nodes.new("ShaderNodeEmission")
    emit.inputs["Color"].default_value = (0.0, 0.0, 0.0, 1.0)
    out = flat.node_tree.nodes.new("ShaderNodeOutputMaterial")
    flat.node_tree.links.new(emit.outputs["Emission"], out.inputs["Surface"])
    bpy.context.view_layer.material_override = flat

    setup_camera(scene, 3.00, 0.0, 0.72)


def build_clipsheet(args, body, slots, hidden):
    """Les poses clés d'un cycle, côte à côte.

    Les clés sont rejouées depuis `dwarf_anim` plutôt que lues dans l'action
    importée : la frame est globale à la scène, on ne peut donc pas figer
    plusieurs instances à des instants différents dans un même rendu.
    """
    keys = dwarf_anim.CLIPS[args.clip][:-1]  # la dernière reboucle sur la première
    span = 3.6
    for i, (_frame, overrides, root_offset) in enumerate(keys):
        # vue de profil : les instances s'échelonnent en Y, et +Y va vers la
        # droite de l'image — donc l'ordre des clés est direct
        y = -span / 2 + i * (span / max(1, len(keys) - 1))
        pose = {**dwarf_anim.BASE, **overrides}
        for obj in spawn([body] + slots, (0, y, 0), 0.0, set(hidden)):
            if obj.type != "ARMATURE":
                continue
            for name, angles in pose.items():
                bone = obj.pose.bones.get(name)
                if bone is None:
                    continue
                bone.rotation_mode = "QUATERNION"
                bone.rotation_quaternion = dwarf_anim.bone_local(bone, angles)
            hips = obj.pose.bones.get("Hips")
            if hips is not None:
                hips.location = dwarf_anim.local_offset(hips, root_offset)

    scene = base_render_settings(args)
    scene.render.resolution_x, scene.render.resolution_y = 2000, 820
    setup_lighting(scene, yaw_deg=-90.0)  # la caméra passe de +Y à +X
    setup_camera(scene, 4.40, 0.0, 0.76, side=True)
    if args.toon:
        setup_toon_compositor(toon_params())


def main():
    args = cli_args()
    for obj in list(bpy.data.objects):
        bpy.data.objects.remove(obj, do_unlink=True)

    root = args.dir.resolve()
    body, slots, hidden = load_manifest(root)

    if args.silhouette:
        build_silhouette(args, body, slots, hidden)
    elif args.clip:
        build_clipsheet(args, body, slots, hidden)
    else:
        build_showcase(args, body, slots, hidden)

    args.out.parent.mkdir(parents=True, exist_ok=True)
    bpy.context.scene.render.filepath = str(args.out.resolve())
    bpy.ops.render.render(write_still=True)
    print(f"masqués quand équipé : {sorted(hidden)}")
    if args.toon:
        print(f"toon appliqué : {toon_params()}")
    print(f"PREVIEW_OK -> {bpy.context.scene.render.filepath}")


if __name__ == "__main__":
    main()
