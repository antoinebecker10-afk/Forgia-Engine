"""Exporte un kit PCG Blender en GLB de rendu, LOD et collision proxy.

Usage :
  blender --background kit.blend --python tools/blender/export_pcg_kit.py -- \
    --asset-root assets/pcg/kits/castle_stone/1.0.0

Convention partagée avec catalog_pcg_kit.py :
  PCG_PIECE__<id> : collection d'une pièce
  PCG_ROOT__<id>  : Empty unique, origine locale stable
  PCG_SOCKET__*   : metadata uniquement, jamais exportée dans le rendu
  PCG_COLLISION__*: mesh proxy, exporté séparément sous collision/
  PCG_LOD<n>__*   : mesh LOD n, exporté séparément sous render/

Les objets sont copiés dans une collection temporaire avant export : le .blend
n'est jamais modifié ni sauvegardé. Le root est retiré de chaque transform afin
que la pièce exportée reste ancrée à son propre repère quelle que soit sa pose
dans une scène de prévisualisation.
"""

import argparse
import re
import sys
from pathlib import Path

import bpy


PIECE_PREFIX = "PCG_PIECE__"
ROOT_PREFIX = "PCG_ROOT__"
SOCKET_PREFIX = "PCG_SOCKET__"
COLLISION_PREFIX = "PCG_COLLISION__"
LOD_RE = re.compile(r"^PCG_LOD(\d+)__")


def args_after_separator():
    if "--" not in sys.argv:
        raise SystemExit("Arguments attendus après `--`; lance avec --help.")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--asset-root", required=True, type=Path)
    parser.add_argument("--dry-run", action="store_true")
    return parser.parse_args(sys.argv[sys.argv.index("--") + 1 :])


def fail(message):
    raise SystemExit(f"PCG_KIT_EXPORT_ERROR: {message}")


def objects_recursive(collection):
    objects = list(collection.objects)
    for child in collection.children:
        objects.extend(objects_recursive(child))
    return objects


def piece_collections():
    collections = [collection for collection in bpy.data.collections if collection.name.startswith(PIECE_PREFIX)]
    if not collections:
        fail(f"aucune collection `{PIECE_PREFIX}<id>` dans le fichier Blender courant")
    return sorted(collections, key=lambda collection: collection.name)


def piece_root(piece_id, objects):
    roots = [obj for obj in objects if obj.name == ROOT_PREFIX + piece_id]
    if len(roots) != 1 or roots[0].type != "EMPTY":
        fail(f"{piece_id}: exige exactement un Empty `{ROOT_PREFIX}{piece_id}`")
    return roots[0]


def is_metadata_object(obj):
    return obj.name.startswith(SOCKET_PREFIX) or obj.name.startswith(ROOT_PREFIX)


def lod_index(obj):
    match = LOD_RE.match(obj.name)
    return int(match.group(1)) if match else None


def copy_for_export(objects, root, stage):
    """Copies non destructives, rendues locales au PCG_ROOT."""
    root_inverse = root.matrix_world.inverted()
    copies = []
    for obj in objects:
        duplicate = obj.copy()
        if obj.data is not None:
            duplicate.data = obj.data.copy()
        duplicate.matrix_world = root_inverse @ obj.matrix_world
        stage.objects.link(duplicate)
        copies.append(duplicate)
    return copies


def export_glb(filepath, objects, root, dry_run):
    if not objects:
        return False
    if dry_run:
        print(f"PCG_KIT_EXPORT_DRY_RUN {filepath} objects={len(objects)}")
        return True
    filepath.parent.mkdir(parents=True, exist_ok=True)
    stage = bpy.data.collections.new("__PCG_EXPORT_STAGING__")
    bpy.context.scene.collection.children.link(stage)
    copies = copy_for_export(objects, root, stage)
    try:
        bpy.ops.object.select_all(action="DESELECT")
        for obj in copies:
            obj.select_set(True)
        bpy.context.view_layer.objects.active = copies[0]
        bpy.ops.export_scene.gltf(
            filepath=str(filepath),
            export_format="GLB",
            use_selection=True,
            export_apply=False,
            export_tangents=True,
            export_animations=False,
            export_lights=False,
            export_cameras=False,
            export_extras=True,
        )
        print(f"PCG_KIT_EXPORT_OK {filepath} objects={len(copies)}")
        return True
    finally:
        bpy.ops.object.select_all(action="DESELECT")
        for obj in copies:
            bpy.data.objects.remove(obj, do_unlink=True)
        bpy.data.collections.remove(stage)


def export_piece(collection, asset_root, dry_run):
    piece_id = collection.name.removeprefix(PIECE_PREFIX)
    if not piece_id:
        fail(f"{collection.name}: ID pièce vide")
    all_objects = objects_recursive(collection)
    root = piece_root(piece_id, all_objects)
    meshes = [obj for obj in all_objects if obj.type == "MESH"]
    collision = [obj for obj in meshes if obj.name.startswith(COLLISION_PREFIX)]
    lods = {}
    for obj in meshes:
        index = lod_index(obj)
        if index is not None:
            lods.setdefault(index, []).append(obj)
    render = [
        obj
        for obj in meshes
        if obj not in collision and lod_index(obj) is None and not is_metadata_object(obj)
    ]
    if not render:
        fail(f"{piece_id}: aucun mesh rendu (hors PCG_COLLISION/PCG_LOD)")
    exported = 0
    exported += export_glb(asset_root / "render" / f"{piece_id}.glb", render, root, dry_run)
    if collision:
        exported += export_glb(
            asset_root / "collision" / f"{piece_id}_proxy.glb", collision, root, dry_run
        )
    for index, objects in sorted(lods.items()):
        exported += export_glb(
            asset_root / "render" / f"{piece_id}_lod{index}.glb", objects, root, dry_run
        )
    return exported


def main():
    args = args_after_separator()
    total = 0
    pieces = piece_collections()
    for collection in pieces:
        total += export_piece(collection, args.asset_root, args.dry_run)
    print(f"PCG_KIT_EXPORT_SUMMARY pieces={len(pieces)} glbs={total} root={args.asset_root}")


if __name__ == "__main__":
    main()
