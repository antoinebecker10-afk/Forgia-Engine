"""Réexporte un GLB en conservant sa scène et en écrivant l'attribut TANGENT.

Usage (Blender headless) :
  blender --background --python reexport_glb_with_tangents.py -- input.glb output.glb

L'export glTF de Blender calcule les tangentes Mikktspace lors de l'export pour
les meshes disposant d'UVs. Cela déplace ce coût hors du runtime Bevy.
"""

import bpy
import sys
from pathlib import Path


def args_after_separator() -> list[str]:
    if "--" not in sys.argv:
        raise SystemExit("Arguments attendus : input.glb output.glb")
    return sys.argv[sys.argv.index("--") + 1 :]


args = args_after_separator()
if len(args) != 2:
    raise SystemExit("Arguments attendus : input.glb output.glb")

source = Path(args[0]).resolve()
destination = Path(args[1]).resolve()
if source.suffix.lower() != ".glb":
    raise SystemExit(f"Source GLB attendue, reçu : {source}")
if source == destination:
    raise SystemExit("La destination doit être distincte de la source")

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=str(source))

# `export_tangents` force l'écriture de l'attribut glTF TANGENT : Bevy n'aura
# alors plus à appeler Mesh::generate_tangents au chargement.
bpy.ops.export_scene.gltf(
    filepath=str(destination),
    export_format="GLB",
    export_tangents=True,
    export_animations=True,
    export_lights=True,
    export_cameras=True,
    export_extras=True,
)
print(f"EXPORTED_WITH_TANGENTS {destination}")
