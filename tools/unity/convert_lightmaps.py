"""Convertit les lightmaps cuites du pack en textures chargeables par Bevy.

Pourquoi ce format et pas un autre
----------------------------------
Ses 11 atlas font 4096² en EXR, soit 400 Mo. Trois pistes ont été mesurées :

| Format | Disque (par atlas) | VRAM (par atlas) |
|---|---|---|
| `.hdr` Radiance 4096² | 22 Mo | **67 Mo** (Bevy le charge en `Rgba32Float`) |
| KTX2 RGB9E5 + zstd 4096² | 19 Mo | 67 Mo |
| **KTX2 RGB9E5 + zstd 2048²** | **6 Mo** | **16 Mo** |

Le `.hdr` est éliminé par la VRAM : 11 atlas en `Rgba32Float`, c'est 738 Mo de
mémoire vidéo pour la seule lumière cuite. RGB9E5 tient sur 4 octets par texel au
lieu de 16, à précision équivalente pour ce signal.

Pourquoi 2048 et pas 4096
-------------------------
Une lightmap est un signal **basse fréquence** : c'est du rebond diffus, pas de la
texture. Diviser la résolution par deux coûte peu visuellement et divise la
mémoire par quatre.

⚠️ **La contrepartie est le débord d'atlas.** Il cuit avec `m_Padding: 2`, soit
2 texels de marge entre deux pièces voisines à 4096. À 2048 il n'en reste **qu'un**,
ce qui suffit tout juste à un échantillonnage bilinéaire. D'où deux conséquences :

- `bicubic_sampling` doit rester **faux** côté runtime, sinon les pièces bavent
  l'une sur l'autre ;
- si un débord apparaît malgré tout, régénérer en 4096 (`--size 4096`) est un seul
  drapeau, pas une reprise du travail.

Sa plage de valeurs, mesurée sur l'atlas 0 : médiane 0,05, 99ᵉ centile 0,09,
maximum 27,5 — 0,04 % des texels dépassent 1,0. Un format 8 bits perdrait à la
fois la précision du bas (l'essentiel du signal) et les hautes lumières sous les
bougies. RGB9E5 tient les deux.

Le conteneur KTX2
-----------------
L'en-tête est écrit à la main. Le descripteur de format (DFD) est **repris tel quel**
de `assets/hdri/env-maps-v1/pisa_diffuse_rgb9e5_zstd.ktx2`, un fichier au même
format déjà présent dans le projet et issu des exemples Bevy : il est donc connu
pour être accepté. Bevy court-circuite de toute façon le DFD dès que `vkFormat` est
renseigné (`bevy_image/src/ktx2.rs`), mais reprendre un descripteur éprouvé évite
de parier là-dessus.

Usage :
    python tools/unity/convert_lightmaps.py <dossier EXR> <dossier de sortie> [--size 2048]
"""

from __future__ import annotations

import argparse
import os
import re
import struct
from compression import zstd
from pathlib import Path

# Doit être posé avant l'import d'OpenCV, qui lit la variable au chargement.
os.environ.setdefault("OPENCV_IO_ENABLE_OPENEXR", "1")

import cv2  # noqa: E402
import numpy as np  # noqa: E402

KTX2_IDENTIFIER = bytes([0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A])
# VK_FORMAT_E5B9G9R9_UFLOAT_PACK32 — trois mantisses de 9 bits, exposant partagé.
VK_FORMAT_E5B9G9R9_UFLOAT_PACK32 = 123
SUPERCOMPRESSION_ZSTD = 2
# Descripteur repris de la cubemap de référence du projet (même format).
REFERENCE_DFD = bytes.fromhex("000000000000000002000000")
ZSTD_LEVEL = 19
# Bornes du format : exposant sur 5 bits biaisé de 15, mantisses sur 9 bits.
RGB9E5_EXPONENT_BIAS = 15
RGB9E5_MANTISSA_BITS = 9


def pack_rgb9e5(rgb: np.ndarray) -> bytes:
    """Encode un tableau RGB flottant en E5B9G9R9, un mot de 32 bits par texel."""
    values = np.maximum(rgb.astype(np.float32), 0.0)
    brightest = np.maximum(values.max(axis=2), 1e-9)
    exponent = np.clip(
        np.ceil(np.log2(brightest)) + RGB9E5_EXPONENT_BIAS, 0, 31
    ).astype(np.uint32)
    scale = np.exp2(
        exponent.astype(np.float32) - RGB9E5_EXPONENT_BIAS - RGB9E5_MANTISSA_BITS
    )
    mantissa = np.clip(np.round(values / scale[..., None]), 0, 511).astype(np.uint32)
    packed = (
        mantissa[:, :, 0]
        | (mantissa[:, :, 1] << 9)
        | (mantissa[:, :, 2] << 18)
        | (exponent << 27)
    )
    return packed.astype("<u4").tobytes()


def write_ktx2(destination: Path, width: int, height: int, payload: bytes) -> None:
    """Écrit un KTX2 mono-niveau, mono-couche, supercompressé en zstd."""
    compressed = zstd.compress(payload, level=ZSTD_LEVEL)
    header = KTX2_IDENTIFIER + struct.pack(
        "<9I",
        VK_FORMAT_E5B9G9R9_UFLOAT_PACK32,
        1,  # typeSize — comme le fichier de référence
        width,
        height,
        1,  # pixelDepth
        1,  # layerCount — >1 ferait une texture tableau, pas une 2D
        1,  # faceCount — 6 en ferait une cubemap
        1,  # levelCount
        SUPERCOMPRESSION_ZSTD,
    )
    level_index_size = 24
    dfd_offset = len(header) + 16 + 16 + level_index_size
    index = struct.pack("<4I", dfd_offset, len(REFERENCE_DFD), 0, 0) + struct.pack("<2Q", 0, 0)
    level_offset = dfd_offset + len(REFERENCE_DFD)
    level = struct.pack("<3Q", level_offset, len(compressed), len(payload))
    destination.write_bytes(header + index + level + REFERENCE_DFD + compressed)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="dossier des Lightmap-N_comp_light.exr")
    parser.add_argument("destination", type=Path)
    parser.add_argument("--size", type=int, default=2048, help="côté de sortie (défaut 2048)")
    args = parser.parse_args()

    paths = sorted(
        args.source.glob("Lightmap-*_comp_light.exr"),
        key=lambda p: int(re.search(r"Lightmap-(\d+)", p.name).group(1)),
    )
    if not paths:
        print(f"Aucune lightmap dans {args.source}")
        return 1
    args.destination.mkdir(parents=True, exist_ok=True)

    total = 0
    for path in paths:
        atlas = int(re.search(r"Lightmap-(\d+)", path.name).group(1))
        image = cv2.imread(str(path), cv2.IMREAD_UNCHANGED)
        if image is None:
            print(f"  illisible : {path.name}")
            continue
        # OpenCV rend du BGR ; l'alpha, s'il existe, ne porte pas d'irradiance.
        rgb = image[:, :, :3][:, :, ::-1].astype(np.float32)
        source_size = rgb.shape[0]
        if source_size != args.size:
            # INTER_AREA moyenne les texels source : c'est le bon filtre pour
            # réduire un signal diffus sans créer d'aliasing.
            rgb = cv2.resize(rgb, (args.size, args.size), interpolation=cv2.INTER_AREA)
        out = args.destination / f"lightmap_{atlas}.ktx2"
        write_ktx2(out, args.size, args.size, pack_rgb9e5(np.ascontiguousarray(rgb)))
        size_mb = out.stat().st_size / 1e6
        total += size_mb
        print(f"  atlas {atlas:2} : {source_size}² -> {args.size}²  {size_mb:5.1f} Mo")

    print(f"{len(paths)} atlas convertis, {total:.0f} Mo au total -> {args.destination}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
