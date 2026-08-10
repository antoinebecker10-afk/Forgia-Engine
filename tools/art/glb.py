"""Lecture d'un GLB : geometrie + texture de couleur, sans dependance externe.

Sert au pipeline « rendre le vrai modele, puis le reduire en pixel art » — celui
de Doom et Duke Nukem 3D, qui photographiaient des modeles 3D pour en faire des
sprites. Interet ici : le design obtenu est REELLEMENT celui de l'arme du jeu,
pas une interpretation dessinee de memoire.
"""

from __future__ import annotations

import io
import json
import struct
from dataclasses import dataclass

import numpy as np
from PIL import Image

_COMPONENT = {
    5120: ("i1", 1),
    5121: ("u1", 1),
    5122: ("i2", 2),
    5123: ("u2", 2),
    5125: ("u4", 4),
    5126: ("f4", 4),
}
_COUNT = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}


@dataclass
class Mesh:
    positions: np.ndarray  # (N, 3)
    normals: np.ndarray  # (N, 3)
    uvs: np.ndarray  # (N, 2)
    indices: np.ndarray  # (M, 3)
    base_color: Image.Image | None

    @property
    def bounds(self) -> tuple[np.ndarray, np.ndarray]:
        return self.positions.min(axis=0), self.positions.max(axis=0)


def _chunks(data: bytes) -> tuple[dict, bytes]:
    magic, _version, _length = struct.unpack("<III", data[:12])
    if magic != 0x46546C67:
        raise ValueError("ce n'est pas un GLB")
    offset = 12
    js: dict = {}
    binary = b""
    while offset < len(data):
        clen, ctype = struct.unpack("<II", data[offset : offset + 8])
        body = data[offset + 8 : offset + 8 + clen]
        if ctype == 0x4E4F534A:
            js = json.loads(body.decode("utf-8"))
        elif ctype == 0x004E4942:
            binary = body
        offset += 8 + clen + (-clen % 4)
    return js, binary


def _accessor(js: dict, binary: bytes, index: int) -> np.ndarray:
    acc = js["accessors"][index]
    dtype, size = _COMPONENT[acc["componentType"]]
    ncomp = _COUNT[acc["type"]]
    view = js["bufferViews"][acc["bufferView"]]
    start = view.get("byteOffset", 0) + acc.get("byteOffset", 0)
    stride = view.get("byteStride") or ncomp * size

    if stride == ncomp * size:
        raw = binary[start : start + acc["count"] * stride]
        return np.frombuffer(raw, dtype=f"<{dtype}").reshape(acc["count"], ncomp)

    # Buffer entrelace : on extrait colonne par colonne.
    out = np.empty((acc["count"], ncomp), dtype=f"<{dtype}")
    for i in range(acc["count"]):
        off = start + i * stride
        out[i] = np.frombuffer(binary[off : off + ncomp * size], dtype=f"<{dtype}")
    return out


def load(path: str) -> Mesh:
    js, binary = _chunks(open(path, "rb").read())
    prim = js["meshes"][0]["primitives"][0]
    attrs = prim["attributes"]

    positions = _accessor(js, binary, attrs["POSITION"]).astype(np.float32)
    normals = (
        _accessor(js, binary, attrs["NORMAL"]).astype(np.float32)
        if "NORMAL" in attrs
        else np.zeros_like(positions)
    )
    uvs = (
        _accessor(js, binary, attrs["TEXCOORD_0"]).astype(np.float32)
        if "TEXCOORD_0" in attrs
        else np.zeros((len(positions), 2), np.float32)
    )
    indices = _accessor(js, binary, prim["indices"]).astype(np.int64).reshape(-1, 3)

    base_color = None
    mat = js.get("materials", [{}])[prim.get("material", 0)]
    tex_index = mat.get("pbrMetallicRoughness", {}).get("baseColorTexture", {}).get("index")
    if tex_index is not None:
        source = js["textures"][tex_index]["source"]
        view = js["bufferViews"][js["images"][source]["bufferView"]]
        start = view.get("byteOffset", 0)
        blob = binary[start : start + view["byteLength"]]
        base_color = Image.open(io.BytesIO(blob)).convert("RGB")

    return Mesh(positions, normals, uvs, indices, base_color)
