"""Trouve les mèches de chaque bougie/bougeoir et écrit leurs positions locales.

Pourquoi
--------
Le Hall compte 98 bougies « objets » et 203 bougeoirs. Beaucoup de bougeoirs ont
leurs bougies **modelées dans le mesh** (une applique murale à deux bougies, un
chandelier à trois branches) : il n'existe alors aucun objet séparé sur lequel
accrocher une flamme.

Poser la flamme au sommet de la boîte englobante donne un seul point, au centre —
donc à côté des mèches, et une seule pour trois branches. Ce script détecte les
**vraies mèches** dans la géométrie et produit une table exploitable au runtime.

Méthode
-------
Une bougie est un cylindre de cire : son sommet est un petit disque de sommets à
la même hauteur. On garde les sommets de la tranche supérieure du mesh, on les
regroupe en X/Z, et chaque amas assez fourni devient un point de flamme.

Validation croisée : sur `SM_PROP_candle_castle_05`, la méthode trouve la mèche à
Y = 0,326 m ; le créateur place sa flamme à 0,360 m dans son prefab `_lit`. L'écart
(3,4 cm) est la hauteur de la flamme au-dessus de la cire — d'où `LIFT_M`.

Usage :
    python tools/gltf/extract_candle_mounts.py <dossier de cellules> <sortie.toml>
"""

from __future__ import annotations

import glob
import json
import os
import re
import struct
import sys
from pathlib import Path

# (format struct, taille) par componentType glTF.
COMPONENT_FORMATS = {5120: ("b", 1), 5121: ("B", 1), 5122: ("h", 2), 5123: ("H", 2),
                     5125: ("I", 4), 5126: ("f", 4)}
TYPE_COMPONENT_COUNT = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}
# Seuls les objets de la famille bougie nous intéressent.
NAME_FRAGMENT = "_candle"
# Tranche supérieure du mesh examinée, en fraction de sa hauteur.
TOP_SLICE = 0.08
# Rayon de regroupement en X/Z, puis distance de fusion finale. Le disque du
# sommet d'une bougie large s'étale sur ~4,6 cm : un seul passage à petit rayon
# la découpe en trois amas au même angle. La fusion finale les recolle, sans
# réunir deux bougies voisines d'un chandelier (les plus proches sont à ~13 cm).
CLUSTER_RADIUS_M = 0.04
MERGE_DISTANCE_M = 0.09
# Un amas plus maigre que ça est du bruit (un coin de métal, une vis).
MIN_CLUSTER_VERTICES = 8
# Hauteur de la flamme au-dessus de la cire — écart mesuré entre notre détection
# et la valeur authorée par le créateur.
LIFT_M = 0.034


def read_accessor(gltf: dict, blob: bytes, index: int) -> list[tuple]:
    accessor = gltf["accessors"][index]
    fmt, size = COMPONENT_FORMATS[accessor["componentType"]]
    components = TYPE_COMPONENT_COUNT[accessor["type"]]
    view = gltf["bufferViews"][accessor["bufferView"]]
    base = view.get("byteOffset", 0) + accessor.get("byteOffset", 0)
    stride = view.get("byteStride") or size * components
    element = struct.Struct("<" + fmt * components)
    return [element.unpack_from(blob, base + i * stride) for i in range(accessor["count"])]


def detect_wicks(points: list[tuple]) -> list[tuple]:
    """Retourne les positions locales des mèches détectées."""
    if not points:
        return []
    ys = [p[1] for p in points]
    low, high = min(ys), max(ys)
    height = high - low
    if height <= 0:
        return []
    threshold = high - height * TOP_SLICE
    tops = [p for p in points if p[1] >= threshold]

    clusters: list[dict] = []
    for p in tops:
        for cluster in clusters:
            dx = p[0] - cluster["x"]
            dz = p[2] - cluster["z"]
            if dx * dx + dz * dz < CLUSTER_RADIUS_M * CLUSTER_RADIUS_M:
                # Moyenne glissante : le centre converge vers l'axe de la bougie.
                cluster["count"] += 1
                cluster["x"] += dx / cluster["count"]
                cluster["z"] += dz / cluster["count"]
                cluster["y"] = max(cluster["y"], p[1])
                break
        else:
            clusters.append({"x": p[0], "y": p[1], "z": p[2], "count": 1})

    wicks = [c for c in clusters if c["count"] >= MIN_CLUSTER_VERTICES]
    wicks.sort(key=lambda c: -c["count"])

    # Fusion : deux amas trop proches sont deux morceaux du même sommet de bougie.
    # On garde le plus fourni et on retient la hauteur la plus élevée des deux.
    merged: list[dict] = []
    for candidate in wicks:
        for kept in merged:
            dx = candidate["x"] - kept["x"]
            dz = candidate["z"] - kept["z"]
            if dx * dx + dz * dz < MERGE_DISTANCE_M * MERGE_DISTANCE_M:
                kept["y"] = max(kept["y"], candidate["y"])
                break
        else:
            merged.append(candidate)

    merged.sort(key=lambda c: (c["x"], c["z"]))
    return [(round(c["x"], 4), round(c["y"] + LIFT_M, 4), round(c["z"], 4)) for c in merged]


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__)
        return 1
    cells = Path(sys.argv[1])
    destination = Path(sys.argv[2])

    by_type: dict[str, list[tuple]] = {}
    for path in sorted(glob.glob(str(cells / "*.gltf"))):
        gltf = json.loads(Path(path).read_text(encoding="utf-8"))
        blob = (Path(path).parent / gltf["buffers"][0]["uri"]).read_bytes()
        mesh_names: dict[int, str] = {}
        for node in gltf.get("nodes", []):
            if "mesh" in node:
                name = re.sub(r"_LOD\d$", "", node.get("name", "").split(".")[0])
                mesh_names.setdefault(node["mesh"], name)
        for index, mesh in enumerate(gltf.get("meshes", [])):
            name = mesh_names.get(index, "")
            if NAME_FRAGMENT not in name or name in by_type:
                continue
            points: list[tuple] = []
            for primitive in mesh.get("primitives", []):
                position = primitive.get("attributes", {}).get("POSITION")
                if position is not None:
                    points += read_accessor(gltf, blob, position)
            wicks = detect_wicks(points)
            if wicks:
                by_type[name] = wicks

    lines = [
        "# Points de flamme par type de bougie ou bougeoir — positions LOCALES.",
        "# Généré par tools/gltf/extract_candle_mounts.py — ne pas éditer à la main.",
        "#",
        "# Beaucoup de bougeoirs ont leurs bougies modelées dans le mesh : il n'existe",
        "# aucun objet séparé où accrocher une flamme. Ces points sont les mèches,",
        "# détectées dans la géométrie (amas de sommets au sommet du mesh).",
        "#",
        f"# La flamme est posée {LIFT_M * 100:.1f} cm au-dessus de la cire — écart mesuré entre",
        "# notre détection et la hauteur authorée par le créateur dans ses prefabs `_lit`.",
        "#",
        "# Un type absent d'ici ne reçoit aucune flamme (bougeoir vide, objet sans mèche).",
        "",
    ]
    total = 0
    for name in sorted(by_type):
        wicks = by_type[name]
        total += len(wicks)
        lines.append("[[mount]]")
        lines.append(f'type = "{name}"')
        points = ", ".join("[%s]" % ", ".join(f"{c}" for c in w) for w in wicks)
        lines.append(f"points = [{points}]")
        lines.append("")
    destination.write_text("\n".join(lines), encoding="utf-8")
    print(f"{len(by_type)} types, {total} meches -> {destination}")
    for name in sorted(by_type):
        print(f"  {len(by_type[name]):2} meche(s)  {name}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
