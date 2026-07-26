"""Author the grey-box `castle.stone` PCG kit (first vertical slice).

Builds five modular pieces as primitive box geometry — grande salle (landmark),
entrée, mur droit, angle, tour — following the PCG_* convention that
`catalog_pcg_kit.py` and `export_pcg_kit.py` consume, then saves a reproducible
`.blend` source. This never touches the monolithic reference castle GLB; it is
deterministic placeholder art whose only purpose is to prove the whole pipeline
(spec → kit → solver → SpatialPlan → streaming) end to end.

Conventions honoured (cf docs/architecture/schemas/kit-manifest.md):
  PCG_PIECE__<id>   collection
  PCG_ROOT__<id>    single Empty, identity at origin = stable local frame
  PCG_SOCKET__<id>  Empty, local +Y = socket forward, +Z = up (Blender Z-up)
  PCG_COLLISION__*  simple box proxy meshes (never a per-render-mesh TriMesh)
  PCG_LOD1__*       coarse box LOD

Everything is authored in Blender coordinates (Z-up); the export applies the
proven (x, z, -y) → Forgia conversion to meshes AND sockets consistently.

Usage:
  blender --background --factory-startup --python tools/blender/author_castle_stone_kit.py -- \
    --out assets/source/castle_stone_greybox.blend
"""

import argparse
import json
import sys

import bmesh
import bpy
from mathutils import Matrix, Vector


ROOT_PREFIX = "PCG_ROOT__"
SOCKET_PREFIX = "PCG_SOCKET__"
PIECE_PREFIX = "PCG_PIECE__"
COLLISION_PREFIX = "PCG_COLLISION__"


def cli_args():
    if "--" not in sys.argv:
        raise SystemExit("Arguments attendus après `--`.")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", required=True)
    return parser.parse_args(sys.argv[sys.argv.index("--") + 1 :])


def wipe_scene():
    for obj in list(bpy.data.objects):
        bpy.data.objects.remove(obj, do_unlink=True)
    for col in list(bpy.data.collections):
        bpy.data.collections.remove(col)


def make_box(name, lo, hi, collection):
    corners = [
        (lo[0], lo[1], lo[2]),
        (hi[0], lo[1], lo[2]),
        (hi[0], hi[1], lo[2]),
        (lo[0], hi[1], lo[2]),
        (lo[0], lo[1], hi[2]),
        (hi[0], lo[1], hi[2]),
        (hi[0], hi[1], hi[2]),
        (lo[0], hi[1], hi[2]),
    ]
    faces = [
        (0, 3, 2, 1),
        (4, 5, 6, 7),
        (0, 1, 5, 4),
        (1, 2, 6, 5),
        (2, 3, 7, 6),
        (3, 0, 4, 7),
    ]
    # bmesh + recalc : normales sortantes garanties. Le rendu de contrôle a
    # attrapé des faces inversées avec from_pydata brut — fatal en jeu, le
    # StandardMaterial Bevy est single-sided.
    mesh = bpy.data.meshes.new(name)
    bm = bmesh.new()
    bm_verts = [bm.verts.new(v) for v in corners]
    for face in faces:
        bm.faces.new([bm_verts[i] for i in face])
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces[:])
    bm.to_mesh(mesh)
    bm.free()
    mesh.update()
    obj = bpy.data.objects.new(name, mesh)
    collection.objects.link(obj)
    return obj


def make_root(piece_id, provides, collection):
    root = bpy.data.objects.new(ROOT_PREFIX + piece_id, None)
    root.empty_display_type = "PLAIN_AXES"
    collection.objects.link(root)
    root.matrix_world = Matrix.Identity(4)
    root["pcg_provides"] = json.dumps(provides)
    return root


def make_socket(piece_id, socket, collection):
    # Blender object names are globally unique, so two pieces sharing a socket id
    # (`west`, `east`) would be renamed `west.001`. Keep the object name unique per
    # piece and carry the real socket id in an explicit `pcg_socket_id` property.
    empty = bpy.data.objects.new(f"{SOCKET_PREFIX}{piece_id}_{socket['id']}", None)
    empty.empty_display_type = "ARROWS"
    collection.objects.link(empty)
    empty["pcg_socket_id"] = socket["id"]
    # Basis: local +Y = forward, +Z = world up; +X = Y × Z (right-handed).
    y = Vector(socket["forward"]).normalized()
    z = Vector((0.0, 0.0, 1.0))
    x = y.cross(z).normalized()
    loc = socket["loc"]
    empty.matrix_world = Matrix(
        (
            (x.x, y.x, z.x, loc[0]),
            (x.y, y.y, z.y, loc[1]),
            (x.z, y.z, z.z, loc[2]),
            (0.0, 0.0, 0.0, 1.0),
        )
    )
    empty["pcg_family"] = socket["family"]
    empty["pcg_role"] = socket["role"]
    empty["pcg_gender"] = socket["gender"]
    empty["pcg_aperture_shape"] = "rect"
    empty["pcg_width_m"] = socket["aperture"][0]
    empty["pcg_height_m"] = socket["aperture"][1]
    empty["pcg_depth_m"] = socket["aperture"][2]
    empty["pcg_accepts"] = json.dumps(socket["accepts"])
    clearance = socket.get("clearance")
    if clearance is not None:
        empty["pcg_clearance_shape"] = "box"
        empty["pcg_clearance_hx"] = clearance[0]
        empty["pcg_clearance_hy"] = clearance[1]
        empty["pcg_clearance_hz"] = clearance[2]
    return empty


def build_piece(piece):
    collection = bpy.data.collections.new(PIECE_PREFIX + piece["id"])
    bpy.context.scene.collection.children.link(collection)
    make_root(piece["id"], piece["provides"], collection)
    for index, (lo, hi) in enumerate(piece["render"]):
        make_box(f"{piece['id']}_render_{index}", lo, hi, collection)
    for index, (lo, hi) in enumerate(piece["collision"]):
        make_box(f"{COLLISION_PREFIX}{piece['id']}_{index}", lo, hi, collection)
    for index, (lo, hi) in enumerate(piece.get("lod1", [])):
        make_box(f"PCG_LOD1__{piece['id']}_{index}", lo, hi, collection)
    for socket in piece["sockets"]:
        make_socket(piece["id"], socket, collection)


STONE = ["wall.load_bearing", "theme.medieval_stone"]
WALL_ACCEPT = [{"family": "arch.wall"}]

# All coordinates in Blender space (Z-up, metres). Pieces are authored at their
# own origin; the solver places them, the exporter anchors each to its ROOT.
PIECES = [
    {
        "id": "mur_droit",
        "provides": STONE,
        "render": [((-2.0, -0.25, 0.0), (2.0, 0.25, 4.0))],
        "collision": [((-2.0, -0.25, 0.0), (2.0, 0.25, 4.0))],
        "lod1": [((-2.0, -0.25, 0.0), (2.0, 0.25, 4.0))],
        "sockets": [
            {
                "id": "west", "family": "arch.wall", "role": "structural", "gender": "neutral",
                "loc": (-2.0, 0.0, 2.0), "forward": (-1.0, 0.0, 0.0),
                "aperture": (0.5, 4.0, 0.5), "accepts": WALL_ACCEPT,
            },
            {
                "id": "east", "family": "arch.wall", "role": "structural", "gender": "neutral",
                "loc": (2.0, 0.0, 2.0), "forward": (1.0, 0.0, 0.0),
                "aperture": (0.5, 4.0, 0.5), "accepts": WALL_ACCEPT,
            },
        ],
    },
    {
        "id": "angle",
        "provides": STONE,
        "render": [((-0.25, -0.25, 0.0), (0.25, 0.25, 4.0))],
        "collision": [((-0.25, -0.25, 0.0), (0.25, 0.25, 4.0))],
        "lod1": [((-0.25, -0.25, 0.0), (0.25, 0.25, 4.0))],
        "sockets": [
            {
                "id": "west", "family": "arch.wall", "role": "structural", "gender": "neutral",
                "loc": (-0.25, 0.0, 2.0), "forward": (-1.0, 0.0, 0.0),
                "aperture": (0.5, 4.0, 0.5), "accepts": WALL_ACCEPT,
            },
            {
                "id": "south", "family": "arch.wall", "role": "structural", "gender": "neutral",
                "loc": (0.0, -0.25, 2.0), "forward": (0.0, -1.0, 0.0),
                "aperture": (0.5, 4.0, 0.5), "accepts": WALL_ACCEPT,
            },
        ],
    },
    {
        "id": "tour",
        "provides": ["landmark.high", "theme.medieval_stone"],
        "render": [((-2.0, -2.0, 0.0), (2.0, 2.0, 12.0))],
        "collision": [((-2.0, -2.0, 0.0), (2.0, 2.0, 12.0))],
        "lod1": [((-2.0, -2.0, 0.0), (2.0, 2.0, 12.0))],
        "sockets": [
            {
                # Faces +X so a wall's free west(-X) tip can seat the tower: the
                # V1 solver only binds sockets whose raw forwards are opposed.
                "id": "attach", "family": "arch.wall", "role": "structural", "gender": "neutral",
                "loc": (2.0, 0.0, 2.0), "forward": (1.0, 0.0, 0.0),
                "aperture": (0.5, 4.0, 0.5), "accepts": WALL_ACCEPT,
            },
        ],
    },
    {
        "id": "entree",
        "provides": ["portal.door", "theme.medieval_stone", "nav.walkable"],
        "render": [
            ((-2.0, -0.25, 0.0), (-0.8, 0.25, 4.0)),  # left post
            ((0.8, -0.25, 0.0), (2.0, 0.25, 4.0)),     # right post
            ((-0.8, -0.25, 3.0), (0.8, 0.25, 4.0)),    # lintel → 1.6m × 3.0m doorway
        ],
        "collision": [
            ((-2.0, -0.25, 0.0), (-0.8, 0.25, 4.0)),
            ((0.8, -0.25, 0.0), (2.0, 0.25, 4.0)),
            ((-0.8, -0.25, 3.0), (0.8, 0.25, 4.0)),
        ],
        "lod1": [((-2.0, -0.25, 0.0), (2.0, 0.25, 4.0))],
        "sockets": [
            {
                "id": "west", "family": "arch.wall", "role": "structural", "gender": "neutral",
                "loc": (-2.0, 0.0, 2.0), "forward": (-1.0, 0.0, 0.0),
                "aperture": (0.5, 4.0, 0.5), "accepts": WALL_ACCEPT,
            },
            {
                "id": "east", "family": "arch.wall", "role": "structural", "gender": "neutral",
                "loc": (2.0, 0.0, 2.0), "forward": (1.0, 0.0, 0.0),
                "aperture": (0.5, 4.0, 0.5), "accepts": WALL_ACCEPT,
            },
            {
                # Walkable portal binding into the great hall (+Y toward hall).
                "id": "door_out", "family": "portal.door", "role": "portal", "gender": "male",
                "loc": (0.0, 0.25, 1.5), "forward": (0.0, 1.0, 0.0),
                "aperture": (1.6, 3.0, 0.5), "clearance": (0.9, 1.5, 1.2),
                "accepts": [{"family": "portal.door", "gender": "female"}],
            },
        ],
    },
    {
        "id": "great_hall",
        "provides": ["space.hall", "landmark.high", "social.safe", "nav.walkable"],
        "render": [
            ((-8.0, -6.0, 0.0), (8.0, 6.0, 0.4)),   # floor
            ((-8.0, 5.6, 0.0), (8.0, 6.0, 8.0)),    # back wall (+Y)
            ((-8.0, -6.0, 0.0), (-7.6, 6.0, 8.0)),  # left wall (-X)
            ((7.6, -6.0, 0.0), (8.0, 6.0, 8.0)),    # right wall (+X)
        ],
        "collision": [
            ((-8.0, -6.0, 0.0), (8.0, 6.0, 0.4)),
            ((-8.0, 5.6, 0.0), (8.0, 6.0, 8.0)),
            ((-8.0, -6.0, 0.0), (-7.6, 6.0, 8.0)),
            ((7.6, -6.0, 0.0), (8.0, 6.0, 8.0)),
        ],
        "lod1": [((-8.0, -6.0, 0.0), (8.0, 6.0, 8.0))],
        "sockets": [
            {
                # Front open side (-Y) receives the entrée doorway.
                "id": "entry", "family": "portal.door", "role": "portal", "gender": "female",
                "loc": (0.0, -6.0, 1.5), "forward": (0.0, -1.0, 0.0),
                "aperture": (1.6, 3.0, 0.5), "clearance": (0.9, 1.5, 1.5),
                "accepts": [{"family": "portal.door", "gender": "male"}],
            },
        ],
    },
]


def main():
    args = cli_args()
    wipe_scene()
    for piece in PIECES:
        build_piece(piece)
    bpy.ops.wm.save_as_mainfile(filepath=args.out)
    print(f"PCG_KIT_AUTHOR_OK pieces={len(PIECES)} out={args.out}")


if __name__ == "__main__":
    main()
