"""Ouvre Blender (GUI) sur le nain : version équipée + version nue, côte à côte.

Charge les GLB **exportés** — donc exactement ce que Bevy chargera, pas la scène
d'auteur — et applique les masques de sous-mesh déclarés dans le manifest.

Le nain regarde +Y : la vue est donc initialisée sur BACK (caméra en +Y), sinon
la vue de face de Blender (Numpad 1, caméra en -Y) montrerait sa nuque.

Usage :
  blender --python tools/blender/open_dwarf.py -- --dir assets/models/characters/dwarf
"""

import json
import sys
from pathlib import Path

import bpy
from mathutils import Matrix, Vector

sys.path.insert(0, str(Path(__file__).resolve().parent))
import preview_dwarf  # noqa: E402  (dépend du sys.path ci-dessus)

DEFAULT_DIR = Path(__file__).resolve().parents[2] / "assets" / "models" / "characters" / "dwarf"


def target_dir():
    if "--" in sys.argv:
        rest = sys.argv[sys.argv.index("--") + 1 :]
        if "--dir" in rest:
            return Path(rest[rest.index("--dir") + 1]).resolve()
    return DEFAULT_DIR


def target_clip():
    if "--" in sys.argv:
        rest = sys.argv[sys.argv.index("--") + 1 :]
        if "--clip" in rest:
            return rest[rest.index("--clip") + 1]
    return "walk"


def apply_clip(objs, clip_name):
    """Assigne l'action à TOUTES les armatures de l'instance.

    Le corps et chaque pièce d'armure embarquent la même armature. Seul le
    corps porte les clips, mais les fcurves ciblent des os par NOM — les poser
    aussi sur les armatures d'armure les fait suivre. C'est la démonstration en
    petit de ce que fera le runtime : un squelette, N maillages.
    """
    action = next(
        (a for a in bpy.data.actions if a.name.split(".")[0] == clip_name), None
    )
    if action is None:
        print(f"[open_dwarf] clip '{clip_name}' introuvable dans le GLB")
        return None
    for obj in objs:
        if obj.type != "ARMATURE":
            continue
        if obj.animation_data is None:
            obj.animation_data_create()
        obj.animation_data.action = action
        slot = getattr(obj.animation_data, "action_slot", "absent")
        if slot is None and len(action.slots):
            try:
                obj.animation_data.action_slot = action.slots[0]
            except Exception as exc:  # API des slots encore mouvante
                print(f"[open_dwarf] slot non assigné : {exc}")
    return action


def setup_timeline(scene, action):
    scene.render.fps = 24
    if action is None:
        return
    start, end = action.frame_range
    scene.frame_start = int(start)
    # -1 : la dernière clé rejoue la première, la garder ferait un temps mort
    scene.frame_end = max(int(start), int(end) - 1)
    scene.frame_set(int(start))


def wipe():
    for obj in list(bpy.data.objects):
        bpy.data.objects.remove(obj, do_unlink=True)


def spawn(glbs, offset_x, hidden_parts, suffix):
    before = set(bpy.data.objects)
    for glb in glbs:
        bpy.ops.import_scene.gltf(filepath=str(glb))
    fresh = [o for o in bpy.data.objects if o not in before]

    for obj in list(fresh):
        # l'importeur suffixe en cas de collision (head.001) → comparer la base
        if obj.type == "MESH" and obj.name.split(".")[0] in hidden_parts:
            fresh.remove(obj)
            bpy.data.objects.remove(obj, do_unlink=True)

    for obj in fresh:
        if obj.parent is None:
            obj.matrix_world = Matrix.Translation(Vector((offset_x, 0.0, 0.0))) @ obj.matrix_world
        obj.name = f"{obj.name.split('.')[0]}__{suffix}"
    return fresh


def setup_realtime(scene):
    """EEVEE + compositeur toon DANS le viewport = le rendu du jeu, orbitable.

    L'aperçu matériau ne montre que du PBR : c'est précisément le piège dans
    lequel je suis tombé pendant onze itérations.
    """
    for engine in ("BLENDER_EEVEE_NEXT", "BLENDER_EEVEE"):
        try:
            scene.render.engine = engine
            break
        except TypeError:
            continue
    scene.view_settings.view_transform = "Standard"
    preview_dwarf.setup_lighting(scene)
    preview_dwarf.setup_toon_compositor(preview_dwarf.toon_params())


def setup_viewport():
    for window in bpy.context.window_manager.windows:
        for area in window.screen.areas:
            if area.type != "VIEW_3D":
                continue
            space = area.spaces[0]
            space.shading.type = "RENDERED"
            try:
                space.shading.use_compositor = "ALWAYS"
            except (AttributeError, TypeError) as exc:
                print(f"[open_dwarf] compositeur viewport indisponible : {exc}")
            space.clip_start = 0.01
            space.overlay.show_relationship_lines = False
            region = next((r for r in area.regions if r.type == "WINDOW"), None)
            if region is None:
                continue
            try:
                with bpy.context.temp_override(window=window, area=area, region=region):
                    bpy.ops.view3d.view_axis(type="BACK")  # face au nain
                    bpy.ops.view3d.view_all()
            except Exception as exc:  # cadrage cosmétique : ne doit jamais bloquer
                print(f"[open_dwarf] cadrage ignoré : {exc}")


def main():
    root = target_dir()
    manifest = json.loads((root / "manifest.json").read_text(encoding="utf-8"))
    body = root / manifest["body"]["glb"]
    slots = [root / s["glb"] for s in manifest["slots"].values()]
    hidden = {p for s in manifest["slots"].values() for p in s["hides"]}

    wipe()
    clip = target_clip()
    equipped = spawn([body] + slots, 0.0, hidden, "equipe")
    naked = spawn([body], 0.95, set(), "nu")

    action = apply_clip(equipped, clip)
    apply_clip(naked, clip)
    if action is None:  # GLB sans clips : au moins montrer une pose campée
        for fresh in (equipped, naked):
            preview_dwarf.apply_pose(fresh)

    scene = bpy.context.scene
    setup_realtime(scene)
    setup_timeline(scene, action)
    setup_viewport()

    if action is not None:
        try:
            bpy.ops.screen.animation_play()
        except Exception as exc:  # cosmétique : ne doit jamais bloquer
            print(f"[open_dwarf] lecture auto indisponible ({exc}) — Espace pour jouer")
        print(f"[open_dwarf] clip '{clip}' en lecture, frames {scene.frame_start}-{scene.frame_end}")
    print(f"[open_dwarf] chargé depuis {root}")
    print(f"[open_dwarf] masqués sur la version équipée : {sorted(hidden)}")


if __name__ == "__main__":
    main()
