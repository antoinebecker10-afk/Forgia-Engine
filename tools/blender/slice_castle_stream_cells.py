"""Cook spatial stream cells for the Highlands Castle.

The visual castle is deliberately kept as its source-of-truth GLB. This cooker
only derives runtime cells; it never mutates the source. Each mesh is assigned
to the cell containing its world-space AABB center. Large boundary meshes are
therefore owned once (no duplicate draw cost); a future HLOD pass can replace
them by cross-cell impostors.

Run from the repository root:
  & "C:\Program Files\Blender Foundation\Blender 5.0\blender.exe" --background \
    --factory-startup --python tools/blender/slice_castle_stream_cells.py

Outputs:
  assets/models/environment/castle/castle_stream_cells_textured/cell_x*_z*_render.gltf
  assets/models/environment/castle/castle_stream_cells_textured/cell_x*_z*_render.bin
  assets/models/environment/castle/castle_stream_cells_textured/textures/*
  assets/models/environment/castle/castle_stream_cells_textured/castle_stream_cells.toml

Cells deliberately use ``GLTF_SEPARATE`` rather than GLB. A GLB embeds every
texture it references, which multiplied the original 48 MB castle into 580 MB
when it was spatially sliced. Separate glTFs instead point to the one shared
``textures/`` library. KTX2 compression is an optional post-process of that
library; it is not required to cook or run the map.
"""

from collections import defaultdict
from pathlib import Path

import bpy
from mathutils import Vector


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "assets/models/environment/castle/castle_highlands.glb"
# Version texturée : laissée distincte de l'export expérimental blanc afin de
# pouvoir comparer/valider le poids avant de retirer un artefact précédent.
OUTPUT = ROOT / "assets/models/environment/castle/castle_stream_cells_textured"
CELL_SIZE_M = 32.0


def clear_scene() -> None:
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)


def world_bounds(obj: bpy.types.Object):
    points = [obj.matrix_world @ Vector(corner) for corner in obj.bound_box]
    return (
        min(point.x for point in points),
        max(point.x for point in points),
        min(point.y for point in points),
        max(point.y for point in points),
        min(point.z for point in points),
        max(point.z for point in points),
    )


def cell_of(bounds):
    min_x, max_x, min_y, max_y, _min_z, _max_z = bounds
    # Blender is Z-up while the exported glTF is Bevy Y-up:
    # Bevy (x, z) = Blender (x, -y).
    center_x = (min_x + max_x) * 0.5
    center_bevy_z = -((min_y + max_y) * 0.5)
    return (int(center_x // CELL_SIZE_M), int(center_bevy_z // CELL_SIZE_M))


def export_cell(cell, objects, bounds) -> None:
    cell_x, cell_z = cell
    bpy.ops.object.select_all(action="DESELECT")
    for obj in objects:
        obj.select_set(True)
    bpy.context.view_layer.objects.active = objects[0]
    filepath = OUTPUT / f"cell_x{cell_x}_z{cell_z}_render.gltf"
    bpy.ops.export_scene.gltf(
        filepath=str(filepath),
        # Do not use GLB here: it re-embeds each material texture into every
        # cell. All exports share OUTPUT/textures by their source image name.
        export_format="GLTF_SEPARATE",
        export_texture_dir="textures",
        # Les images du GLB source sont embarquées, donc elles n'ont pas de
        # fichier source réutilisable. `True` les omet silencieusement et rend
        # les pierres blanches. `False` les extrait vers la bibliothèque
        # commune `textures/` ; les exports suivants réemploient ces noms.
        export_keep_originals=False,
        use_selection=True,
        export_apply=False,
        export_yup=True,
    )
    min_x, max_x, min_y, max_y, min_z, max_z = bounds
    # Convert Blender bounds to Bevy bounds, reversing Y into Z.
    bevy_min = (min_x, min_z, -max_y)
    bevy_max = (max_x, max_z, -min_y)
    return filepath, bevy_min, bevy_max


def main() -> None:
    if not SOURCE.is_file():
        raise RuntimeError(f"missing source GLB: {SOURCE}")
    # A cooker must never silently delete an existing generated map. This also
    # prevents a bad invocation from erasing a known-good streamed build.
    if OUTPUT.exists():
        raise RuntimeError(
            f"refusing to overwrite existing output: {OUTPUT}. "
            "Move it aside or remove it explicitly after inspecting it."
        )
    OUTPUT.mkdir(parents=True)

    clear_scene()
    bpy.ops.import_scene.gltf(filepath=str(SOURCE))
    bpy.context.view_layer.update()

    cells = defaultdict(list)
    cell_bounds = {}
    for obj in bpy.context.scene.objects:
        if obj.type != "MESH" or obj.hide_render:
            continue
        bounds = world_bounds(obj)
        cell = cell_of(bounds)
        cells[cell].append(obj)
        prior = cell_bounds.get(cell)
        if prior is None:
            cell_bounds[cell] = list(bounds)
        else:
            for index in (0, 2, 4):
                prior[index] = min(prior[index], bounds[index])
            for index in (1, 3, 5):
                prior[index] = max(prior[index], bounds[index])

    lines = [
        "# Generated by tools/blender/slice_castle_stream_cells.py — do not hand edit.",
        "schema_version = 1",
        f"cell_size_m = {CELL_SIZE_M:.1f}",
        "coordinate_system = \"bevy_y_up_meters\"",
        "asset_format = \"gltf_separate_shared_texture_library\"",
        "",
    ]
    total_objects = 0
    for cell in sorted(cells):
        objects = cells[cell]
        filepath, bevy_min, bevy_max = export_cell(cell, objects, cell_bounds[cell])
        relative = filepath.relative_to(ROOT / "assets").as_posix()
        lines.extend(
            [
                "[[cells]]",
                f'id = "cell_x{cell[0]}_z{cell[1]}"',
                f'render = "{relative}#Scene0"',
                "bounds_min_m = [%.3f, %.3f, %.3f]" % bevy_min,
                "bounds_max_m = [%.3f, %.3f, %.3f]" % bevy_max,
                f"source_meshes = {len(objects)}",
                "",
            ]
        )
        total_objects += len(objects)

    manifest = OUTPUT / "castle_stream_cells.toml"
    manifest.write_text("\n".join(lines), encoding="utf-8")
    print(
        "CASTLE_STREAM_CELLS_BUILT "
        f"cells={len(cells)} meshes={total_objects} manifest={manifest}"
    )


if __name__ == "__main__":
    main()
