"""Associe chaque pièce de notre château à sa zone dans les lightmaps du créateur.

Pourquoi
--------
Le rendu du pack repose sur des **lightmaps cuites** : deux rebonds de lumière et
une occlusion ambiante, précalculés dans 11 atlas de 4096². C'est ce qui manque à
notre Hall — sans rebond, tout ce qui n'est pas frappé directement par une source
tombe au plancher de l'ambiante, et monter les valeurs ne fait qu'élargir les
flaques sans remplir l'entre-deux.

Pour poser ces lightmaps, il faut savoir **quelle pièce lit quelle zone de quel
atlas**. Cette table n'est pas dans la scène : elle vit dans `LightingData.asset`,
un fichier binaire Unity. C'est ce qui bloquait le portage.

`UnityPy` sait le lire. Ce script en sort la table et la raccorde à nos cellules.

Chaîne de raccordement
----------------------
Le binaire identifie un renderer par `{targetObject, targetPrefab}` :

- `targetPrefab` = identifiant de l'instance de prefab **dans la scène** ;
- `targetObject` = identifiant du renderer **dans le prefab source**.

Il faut donc trois fichiers pour remonter jusqu'à un de nos nœuds :

1. `LightingData.asset` → `(targetObject, targetPrefab)` → index d'atlas + `lightmapST`
2. la scène → instance → prefab source + transformation monde
3. le prefab → quel renderer est le **LOD0** (nos cellules ne contiennent que lui)

Le LOD0 est le **premier** de `m_LODs` du `LODGroup`. Les prefabs sans `LODGroup`
n'ont qu'un seul renderer : c'est celui-là. Vérifié sur 400 instances — 331 avec
LODGroup, 69 sans, aucune ambiguïté.

Le raccord final se fait par **position monde**, comme pour les bannières : nos
cellules reproduisent ses positions à 0,0006 m.

`lightmapST` et Bevy
--------------------
Unity stocke `{x, y, z, w}` = (échelle U, échelle V, décalage U, décalage V). Le
composant `Lightmap` de Bevy veut un `uv_rect` allant de `(z, w)` à `(z+x, w+y)`.
C'est la même chose, sans conversion.

⚠️ **La convention verticale n'est pas vérifiable sans lancer le jeu.** Unity place
l'origine des textures en bas à gauche, glTF en haut à gauche. Si les lightmaps
sortent retournées, c'est ce seul point : le drapeau `flip_v` du runtime le corrige
sans régénérer cette table.

Usage :
    python tools/unity/extract_lightmap_table.py <dossier extrait> <cellules> <sortie.json>
"""

from __future__ import annotations

import json
import math
import re
import sys
from pathlib import Path

import UnityPy

# Repère déduit par appariement, vérifié à 0,000 m de médiane sur 1195 pièces.
MIRROR_X = True
OFFSET = (-32.041, -172.671, 35.372)
MAX_DEPTH = 12
FLOAT = r"[-\d.eE+]+"
# Deux pièces plus proches que ça sont considérées comme la même : nos cellules
# reproduisent ses positions à 0,0006 m, la marge est donc très large.
MATCH_TOLERANCE_M = 0.05


def quaternion_multiply(a, b):
    ax, ay, az, aw = a
    bx, by, bz, bw = b
    return (
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    )


def quaternion_rotate(q, v):
    x, y, z, w = q
    vx, vy, vz = v
    tx, ty, tz = 2 * (y * vz - z * vy), 2 * (z * vx - x * vz), 2 * (x * vy - y * vx)
    return (
        vx + w * tx + (y * tz - z * ty),
        vy + w * ty + (z * tx - x * tz),
        vz + w * tz + (x * ty - y * tx),
    )


def read_lighting_data(path: Path) -> dict[int, list[tuple[int, int, dict]]]:
    """Instance de prefab -> liste de `(renderer, index d'atlas, lightmapST)`."""
    env = UnityPy.load(str(path))
    for obj in env.objects:
        tree = obj.read_typetree()
        if "m_LightmappedRendererData" not in tree:
            continue
        table: dict[int, list] = {}
        for ident, entry in zip(
            tree["m_LightmappedRendererDataIDs"], tree["m_LightmappedRendererData"]
        ):
            table.setdefault(ident["targetPrefab"], []).append(
                (ident["targetObject"], entry["lightmapIndex"], entry["lightmapST"])
            )
        return table
    raise SystemExit("LightingData.asset ne contient pas de table de renderers")


def read_prefab_lod0(directory: Path) -> dict[str, int]:
    """Nom de prefab -> identifiant du renderer LOD0."""
    lod0 = {}
    for path in sorted(directory.glob("*.prefab")):
        text = path.read_text(encoding="utf-8", errors="replace")
        marker = text.find("m_LODs:")
        if marker >= 0:
            renderers = re.findall(r"renderer: {fileID: (-?\d+)}", text[marker:])
            if renderers:
                lod0[path.stem] = int(renderers[0])
                continue
        # Pas de LODGroup : le prefab n'a qu'un renderer, c'est celui-là.
        stripped = re.findall(r"^--- !u!23 &(-?\d+)", text, re.M)
        if len(stripped) == 1:
            lod0[path.stem] = int(stripped[0])
    return lod0


def parse_scene(text: str):
    """Retourne (transforms par fileID, instances de prefab)."""
    transforms, instances = {}, []
    for block in re.split(r"^--- !u!", text, flags=re.M):
        head = re.match(r"(\d+) &(-?\d+)", block)
        if not head:
            continue
        class_id, file_id = head.group(1), head.group(2)
        if class_id == "4":
            position = re.search(
                rf"m_LocalPosition: {{x: ({FLOAT}), y: ({FLOAT}), z: ({FLOAT})}}", block
            )
            rotation = re.search(
                rf"m_LocalRotation: {{x: ({FLOAT}), y: ({FLOAT}), z: ({FLOAT}), w: ({FLOAT})}}",
                block,
            )
            father = re.search(r"m_Father: {fileID: (-?\d+)}", block)
            transforms[file_id] = {
                "position": tuple(map(float, position.groups())) if position else (0.0,) * 3,
                "rotation": tuple(map(float, rotation.groups()))
                if rotation
                else (0.0, 0.0, 0.0, 1.0),
                "father": father.group(1) if father else "0",
            }
        elif class_id == "1001":
            source = re.search(r"m_SourcePrefab: {fileID: \d+, guid: ([0-9a-f]{32})", block)
            parent = re.search(r"m_TransformParent: {fileID: (-?\d+)}", block)
            overrides = dict(
                re.findall(rf"propertyPath: (m_Local\w+\.\w)\n\s+value: ({FLOAT})", block)
            )

            def axis(prefix, keys, default):
                return tuple(
                    float(overrides.get(f"{prefix}.{k}", d)) for k, d in zip(keys, default)
                )

            instances.append(
                {
                    "file_id": int(file_id),
                    "guid": source.group(1) if source else None,
                    "parent": parent.group(1) if parent else "0",
                    "position": axis("m_LocalPosition", "xyz", (0.0, 0.0, 0.0)),
                    "rotation": axis("m_LocalRotation", "xyzw", (0.0, 0.0, 0.0, 1.0)),
                }
            )
    return transforms, instances


def world_of_parent(transforms: dict, file_id: str):
    chain, current = [], file_id
    while current and current != "0" and len(chain) < MAX_DEPTH:
        node = transforms.get(current)
        if node is None:
            break
        chain.append(node)
        current = node["father"]
    position, rotation = (0.0, 0.0, 0.0), (0.0, 0.0, 0.0, 1.0)
    for node in reversed(chain):
        rotated = quaternion_rotate(rotation, node["position"])
        position = tuple(position[i] + rotated[i] for i in range(3))
        rotation = quaternion_multiply(rotation, node["rotation"])
    return position, rotation


def to_forgia(point):
    x = -point[0] if MIRROR_X else point[0]
    return (x + OFFSET[0], point[1] + OFFSET[1], point[2] + OFFSET[2])


def index_cells(directory: Path):
    """Nos nœuds, groupés par famille : famille -> [(cellule, nom, position)]."""
    by_family: dict[str, list] = {}
    for path in sorted(directory.glob("*.gltf")):
        gltf = json.loads(path.read_text(encoding="utf-8"))
        for node in gltf.get("nodes", []):
            name = node.get("name", "")
            if "mesh" not in node or "_LOD" not in name:
                continue
            family = name.split("_LOD")[0]
            by_family.setdefault(family, []).append(
                (path.stem, name, tuple(node.get("translation", (0.0, 0.0, 0.0))))
            )
    return by_family


def main() -> int:
    if len(sys.argv) != 4:
        print(__doc__)
        return 1
    extracted, cells_dir, destination = (Path(a) for a in sys.argv[1:4])

    lighting = read_lighting_data(extracted / "LightingData.asset")
    lod0 = read_prefab_lod0(extracted.parent / "prefabs")
    scene_path = next(extracted.glob("*.unity"))
    transforms, instances = parse_scene(scene_path.read_text(encoding="utf-8", errors="replace"))
    by_family = index_cells(cells_dir)

    names = {}
    for line in (extracted.parent / "urp_inventory.txt").read_text(
        encoding="utf-8", errors="replace"
    ).splitlines():
        guid, _, path = line.partition("\t")
        stem = path.strip().rsplit("/", 1)[-1]
        if stem.endswith(".prefab"):
            names[guid] = stem[: -len(".prefab")]

    entries, stats = {}, {"sans_lod0": 0, "non_cuite": 0, "sans_famille": 0, "sans_voisin": 0}
    for instance in instances:
        prefab = names.get(instance["guid"])
        baked = lighting.get(instance["file_id"])
        if not baked:
            # Pièce non cuite : mobile (drapeau animé) ou hors du lot statique.
            stats["non_cuite"] += 1
            continue
        if len(baked) == 1:
            # Un seul renderer cuit : aucune ambiguïté, quel que soit le prefab.
            # C'est le cas du sol (818 instances), dont le prefab n'expose pas de
            # `LODGroup` — s'appuyer sur le LOD0 seul l'aurait écarté.
            _, atlas, st = baked[0]
        else:
            target = lod0.get(prefab) if prefab else None
            found = next((e for e in baked if e[0] == target), None) if target else None
            if found is None:
                stats["sans_lod0"] += 1
                continue
            _, atlas, st = found

        base_position, base_rotation = world_of_parent(transforms, instance["parent"])
        rotated = quaternion_rotate(base_rotation, instance["position"])
        position = to_forgia(tuple(base_position[i] + rotated[i] for i in range(3)))

        family = "SM_" + prefab[2:] if prefab.startswith("P_") else prefab
        candidates = by_family.get(family)
        if not candidates:
            stats["sans_famille"] += 1
            continue
        cell, node, distance = min(
            ((c, n, math.dist(position, p)) for c, n, p in candidates), key=lambda t: t[2]
        )
        if distance > MATCH_TOLERANCE_M:
            stats["sans_voisin"] += 1
            continue
        entries.setdefault(cell, {})[node] = {
            "atlas": atlas,
            "uv": [
                round(st["z"], 6),
                round(st["w"], 6),
                round(st["z"] + st["x"], 6),
                round(st["w"] + st["y"], 6),
            ],
        }

    matched = sum(len(v) for v in entries.values())
    payload = {
        "comment": (
            "Zone de lightmap par pièce du Hall. Généré par "
            "tools/unity/extract_lightmap_table.py — ne pas éditer à la main. "
            "uv = [min_u, min_v, max_u, max_v], directement le uv_rect de Bevy."
        ),
        "atlas_count": 11,
        "cells": entries,
    }
    destination.write_text(json.dumps(payload, indent=1, sort_keys=True), encoding="utf-8")

    print(f"{matched} pieces raccordees, reparties sur {len(entries)} cellules")
    for key, value in stats.items():
        print(f"   {key:14} : {value}")
    print(f"  -> {destination}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
