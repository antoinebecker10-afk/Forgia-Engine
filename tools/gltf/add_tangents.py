"""Ajoute l'attribut TANGENT aux primitives glTF qui portent une normal map.

Pourquoi ce script plutôt qu'un aller-retour Blender
----------------------------------------------------
Bevy ne peut pas appliquer une normal map sans repère tangent : il ignore la
carte et la surface perd tout son micro-relief. Les 46 cellules du Hall de Forgia
ont été exportées sans `TANGENT` (1138 primitives concernées sur 1138 → 100 %),
d'où l'aspect « texture absente » sur les grandes surfaces planes (plafonds, murs).

Réimporter puis réexporter dans Blender calculerait bien des tangentes Mikktspace,
mais ferait repasser toute la scène par une conversion de repère — précisément là
où ce pipeline a déjà déraillé (miroirs invisibles, frames décalés). Ce script
**n'ajoute qu'un attribut** : mêmes nœuds, mêmes noms, même ordre de fratrie,
mêmes transformations, mêmes matériaux, mêmes textures. Le seul delta est un
accesseur VEC4 de plus par primitive, ajouté en fin de `.bin`.

Base tangente : accumulation par triangle puis orthonormalisation de Gram-Schmidt
(méthode Lengyel), pas Mikktspace stricto sensu. Les deux ne diffèrent qu'aux
coutures d'UV, de façon imperceptible sur de la pierre et du plâtre. Le gain —
retrouver le relief sur 100 % du château — est sans commune mesure avec cet écart.

Usage :
    python tools/gltf/add_tangents.py <fichier.gltf | dossier> [--dry-run]
"""

from __future__ import annotations

import json
import math
import struct
import sys
from pathlib import Path

# (format struct, taille en octets) par componentType glTF.
COMPONENT_FORMATS = {
    5120: ("b", 1),
    5121: ("B", 1),
    5122: ("h", 2),
    5123: ("H", 2),
    5125: ("I", 4),
    5126: ("f", 4),
}
TYPE_COMPONENT_COUNT = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}
TRIANGLES_MODE = 4
# Sous ce déterminant, le triangle est dégénéré dans l'espace UV : il ne peut pas
# contribuer à une direction tangente.
UV_AREA_EPSILON = 1e-12
# Sous cette longueur, la tangente accumulée n'a pas de direction exploitable.
TANGENT_EPSILON = 1e-8


def read_accessor(gltf: dict, buffers: list[bytes], index: int) -> list[tuple]:
    """Lit un accesseur, en respectant un éventuel `byteStride` entrelacé."""
    accessor = gltf["accessors"][index]
    fmt, size = COMPONENT_FORMATS[accessor["componentType"]]
    count = accessor["count"]
    components = TYPE_COMPONENT_COUNT[accessor["type"]]
    view = gltf["bufferViews"][accessor["bufferView"]]
    data = buffers[view.get("buffer", 0)]
    base = view.get("byteOffset", 0) + accessor.get("byteOffset", 0)
    stride = view.get("byteStride") or size * components
    element = struct.Struct("<" + fmt * components)
    return [
        element.unpack_from(data, base + i * stride)
        for i in range(count)
    ]


def compute_tangents(
    positions: list[tuple],
    normals: list[tuple],
    uvs: list[tuple],
    indices: list[int],
) -> list[tuple]:
    vertex_count = len(positions)
    tan = [[0.0, 0.0, 0.0] for _ in range(vertex_count)]
    bitan = [[0.0, 0.0, 0.0] for _ in range(vertex_count)]

    for triangle in range(0, len(indices) - 2, 3):
        i0, i1, i2 = indices[triangle], indices[triangle + 1], indices[triangle + 2]
        p0, p1, p2 = positions[i0], positions[i1], positions[i2]
        w0, w1, w2 = uvs[i0], uvs[i1], uvs[i2]

        e1 = (p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2])
        e2 = (p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2])
        d1 = (w1[0] - w0[0], w1[1] - w0[1])
        d2 = (w2[0] - w0[0], w2[1] - w0[1])

        determinant = d1[0] * d2[1] - d2[0] * d1[1]
        if abs(determinant) < UV_AREA_EPSILON:
            continue
        f = 1.0 / determinant
        t = (
            f * (e1[0] * d2[1] - e2[0] * d1[1]),
            f * (e1[1] * d2[1] - e2[1] * d1[1]),
            f * (e1[2] * d2[1] - e2[2] * d1[1]),
        )
        b = (
            f * (e2[0] * d1[0] - e1[0] * d2[0]),
            f * (e2[1] * d1[0] - e1[1] * d2[0]),
            f * (e2[2] * d1[0] - e1[2] * d2[0]),
        )
        for i in (i0, i1, i2):
            tan[i][0] += t[0]
            tan[i][1] += t[1]
            tan[i][2] += t[2]
            bitan[i][0] += b[0]
            bitan[i][1] += b[1]
            bitan[i][2] += b[2]

    out = []
    for i in range(vertex_count):
        n = normals[i]
        t = tan[i]
        # Gram-Schmidt : la tangente doit être orthogonale à la normale du sommet.
        dot = n[0] * t[0] + n[1] * t[1] + n[2] * t[2]
        x, y, z = t[0] - n[0] * dot, t[1] - n[1] * dot, t[2] - n[2] * dot
        length = math.sqrt(x * x + y * y + z * z)
        if length < TANGENT_EPSILON:
            # Sommet sans UV exploitable : n'importe quelle tangente orthogonale
            # à la normale fait l'affaire, la normal map y est de toute façon
            # sans information.
            x, y, z = orthogonal_to(n)
            length = 1.0
        x, y, z = x / length, y / length, z / length
        # Handedness : signe de la bitangente reconstruite `cross(N, T) * w`.
        cx = n[1] * z - n[2] * y
        cy = n[2] * x - n[0] * z
        cz = n[0] * y - n[1] * x
        b = bitan[i]
        w = -1.0 if (cx * b[0] + cy * b[1] + cz * b[2]) < 0.0 else 1.0
        out.append((x, y, z, w))
    return out


def orthogonal_to(n: tuple) -> tuple:
    """Un vecteur unitaire quelconque orthogonal à `n`."""
    if abs(n[0]) < 0.9:
        ax, ay, az = 1.0, 0.0, 0.0
    else:
        ax, ay, az = 0.0, 1.0, 0.0
    x = ay * n[2] - az * n[1]
    y = az * n[0] - ax * n[2]
    z = ax * n[1] - ay * n[0]
    length = math.sqrt(x * x + y * y + z * z) or 1.0
    return x / length, y / length, z / length


def process(path: Path, dry_run: bool = False) -> tuple[int, int, int]:
    """Retourne (primitives traitées, primitives déjà pourvues, octets ajoutés)."""
    gltf = json.loads(path.read_text(encoding="utf-8"))
    if len(gltf.get("buffers", [])) != 1:
        raise SystemExit(f"{path.name}: un seul buffer attendu (buffers séparés)")
    buffer_uri = gltf["buffers"][0].get("uri")
    if not buffer_uri:
        raise SystemExit(f"{path.name}: buffer embarqué non géré (attendu .bin séparé)")
    bin_path = path.parent / buffer_uri
    blob = bytearray(bin_path.read_bytes())
    materials = gltf.get("materials", [])

    # Deux primitives partageant les mêmes accesseurs partagent leur tangente.
    cache: dict[tuple, int] = {}
    treated = skipped = 0
    initial_size = len(blob)

    for mesh in gltf.get("meshes", []):
        for primitive in mesh.get("primitives", []):
            attributes = primitive.get("attributes", {})
            material_index = primitive.get("material")
            if material_index is None:
                continue
            if "normalTexture" not in materials[material_index]:
                continue
            if primitive.get("mode", TRIANGLES_MODE) != TRIANGLES_MODE:
                continue
            if "TANGENT" in attributes:
                skipped += 1
                continue
            if not {"POSITION", "NORMAL", "TEXCOORD_0"} <= attributes.keys():
                continue
            if "indices" not in primitive:
                continue

            key = (
                attributes["POSITION"],
                attributes["NORMAL"],
                attributes["TEXCOORD_0"],
                primitive["indices"],
            )
            if key in cache:
                attributes["TANGENT"] = cache[key]
                treated += 1
                continue

            positions = read_accessor(gltf, [blob], attributes["POSITION"])
            normals = read_accessor(gltf, [blob], attributes["NORMAL"])
            uvs = read_accessor(gltf, [blob], attributes["TEXCOORD_0"])
            indices = [i[0] for i in read_accessor(gltf, [blob], primitive["indices"])]
            tangents = compute_tangents(positions, normals, uvs, indices)

            if dry_run:
                treated += 1
                continue

            # Les accesseurs d'attributs doivent démarrer sur un multiple de 4.
            while len(blob) % 4:
                blob.append(0)
            offset = len(blob)
            packer = struct.Struct("<ffff")
            for tangent in tangents:
                blob += packer.pack(*tangent)

            gltf.setdefault("bufferViews", []).append(
                {
                    "buffer": 0,
                    "byteOffset": offset,
                    "byteLength": len(tangents) * 16,
                    "target": 34962,  # ARRAY_BUFFER
                }
            )
            gltf.setdefault("accessors", []).append(
                {
                    "bufferView": len(gltf["bufferViews"]) - 1,
                    "componentType": 5126,
                    "count": len(tangents),
                    "type": "VEC4",
                }
            )
            accessor_index = len(gltf["accessors"]) - 1
            attributes["TANGENT"] = accessor_index
            cache[key] = accessor_index
            treated += 1

    if dry_run:
        return treated, skipped, 0

    gltf["buffers"][0]["byteLength"] = len(blob)
    bin_path.write_bytes(bytes(blob))
    path.write_text(json.dumps(gltf, indent=2, ensure_ascii=False), encoding="utf-8")
    return treated, skipped, len(blob) - initial_size


def main() -> int:
    if len(sys.argv) < 2:
        print(__doc__)
        return 1
    target = Path(sys.argv[1])
    dry_run = "--dry-run" in sys.argv
    files = sorted(target.glob("*.gltf")) if target.is_dir() else [target]
    if not files:
        print(f"aucun .gltf sous {target}")
        return 1

    total_treated = total_skipped = total_bytes = 0
    for path in files:
        treated, skipped, added = process(path, dry_run)
        total_treated += treated
        total_skipped += skipped
        total_bytes += added
        print(f"  {path.name:34} +{treated:4} tangentes   (+{added / 1024:.0f} Ko)")

    verb = "à traiter" if dry_run else "traitées"
    print(
        f"\n{len(files)} fichier(s) · {total_treated} primitives {verb} · "
        f"{total_skipped} déjà pourvues · +{total_bytes / 1024 / 1024:.1f} Mo"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
