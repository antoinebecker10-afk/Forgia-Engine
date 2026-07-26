"""Rehausse la heightmap du terrain gazon en plateau sous l'emprise du château.

Le gazon est drapé sur les rochers → il suit la pente naturelle et laisse un VIDE
sous les dalles plates du sol intérieur (Y≈36,5). Ce script remonte les sommets
situés sous l'emprise du château jusqu'à un plateau `--target-y` (coordonnée LOCALE
du GLB, l'align runtime -2,70 s'applique ensuite), avec un fondu doux `--blend`
vers le terrain naturel aux bords → une motte, pas une falaise. Ne fait que
MONTER (jamais descendre) : les zones déjà hautes restent.

Repère : le GLB est Y-up jeu. Import Blender → Z-up : jeu (x,y,z) devient
blender (x, -z, y). Donc hauteur = Blender Z ; emprise jeu X→Blender X,
jeu Z→Blender -Y. `transform_apply` d'abord pour que les sommets = monde.

Usage :
  blender --background --factory-startup --python tools/blender/raise_castle_terrain.py -- \
    --in  assets/models/environment/castle/castle_terrain.glb \
    --out assets/models/environment/castle/castle_terrain.glb \
    --core-min-x -175 --core-max-x 150 --core-min-z -155 --core-max-z 225 \
    --blend 26 --target-y 39.0
"""

import argparse
import sys
from pathlib import Path

import bpy


def cli():
    if "--" not in sys.argv:
        raise SystemExit("Arguments attendus après `--`.")
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--in", dest="src", required=True, type=Path)
    p.add_argument("--out", dest="dst", required=True, type=Path)
    p.add_argument("--core-min-x", type=float, required=True)
    p.add_argument("--core-max-x", type=float, required=True)
    p.add_argument("--core-min-z", type=float, required=True)  # jeu Z
    p.add_argument("--core-max-z", type=float, required=True)
    p.add_argument("--blend", type=float, default=26.0)
    p.add_argument("--target-y", type=float, required=True)  # hauteur LOCALE GLB
    return p.parse_args(sys.argv[sys.argv.index("--") + 1 :])


def smoothstep(t):
    t = max(0.0, min(1.0, t))
    return t * t * (3.0 - 2.0 * t)


def wipe():
    for obj in list(bpy.data.objects):
        bpy.data.objects.remove(obj, do_unlink=True)


def main():
    args = cli()
    wipe()
    bpy.ops.import_scene.gltf(filepath=str(args.src.resolve()))

    meshes = [o for o in bpy.data.objects if o.type == "MESH"]
    if not meshes:
        raise SystemExit("RAISE_TERRAIN_ERROR: aucun mesh importé")

    # Sommets = coordonnées monde (Z-up Blender).
    bpy.ops.object.select_all(action="SELECT")
    bpy.context.view_layer.objects.active = meshes[0]
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)

    # Emprise en Blender : X jeu = X blender ; Z jeu = -Y blender.
    core_x = (args.core_min_x, args.core_max_x)
    core_by = (-args.core_max_z, -args.core_min_z)  # jeu Z → blender Y (inversé)
    cx = 0.5 * (core_x[0] + core_x[1])
    hx = 0.5 * (core_x[1] - core_x[0])
    cy = 0.5 * (core_by[0] + core_by[1])
    hy = 0.5 * (core_by[1] - core_by[0])
    target_z = args.target_y  # hauteur GLB = Blender Z
    blend = max(args.blend, 1e-3)

    raised_total = 0
    y_max_before = -1e9
    y_max_after = -1e9
    for obj in meshes:
        mesh = obj.data
        for v in mesh.vertices:
            y_max_before = max(y_max_before, v.co.z)
            # distance HORS du rectangle core (0 si dedans).
            dx = max(0.0, abs(v.co.x - cx) - hx)
            dy = max(0.0, abs(v.co.y - cy) - hy)
            dist = (dx * dx + dy * dy) ** 0.5
            factor = smoothstep(1.0 - dist / blend)
            if factor > 0.0 and v.co.z < target_z:
                lift = factor * (target_z - v.co.z)
                v.co.z += lift
                raised_total += 1
            y_max_after = max(y_max_after, v.co.z)
        mesh.update()
        mesh.calc_normals_split() if hasattr(mesh, "calc_normals_split") else None

    bpy.ops.object.select_all(action="SELECT")
    bpy.context.view_layer.objects.active = meshes[0]
    bpy.ops.export_scene.gltf(
        filepath=str(args.dst.resolve()),
        export_format="GLB",
        use_selection=True,
        export_yup=True,
        export_apply=False,
        export_normals=True,
        export_texcoords=True,
        export_materials="EXPORT",
        export_animations=False,
    )
    print(
        f"RAISE_TERRAIN_OK meshes={len(meshes)} raised_verts={raised_total} "
        f"local_z_max {y_max_before:.2f}->{y_max_after:.2f} out={args.dst}"
    )


if __name__ == "__main__":
    main()
