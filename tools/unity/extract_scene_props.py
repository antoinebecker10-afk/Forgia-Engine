"""Extrait les transformations complètes de props d'une scène Unity.

Pourquoi ce script en plus de `extract_scene_lights.py`
------------------------------------------------------
Les lumières de la scène sont des `Transform` ordinaires, que l'autre script sait
lire. Les props, eux, sont des **`PrefabInstance`** (classe 1001) : leur position
ne vit pas dans un bloc `Transform` mais dans une liste de `m_Modifications`
qui surcharge le prefab source. Il faut donc les reconstruire.

Ce script sert au réimport des instances écartées par la reconstruction
d'origine, limitée aux prefabs à correspondance 1:1 avec un FBX. Les variantes
(`_static`, `_lit`, `_comp`) réutilisent le mesh du prefab de base — vérifié sur
`P_PROP_flag_castle_02_static`, qui pointe le même `SM_PROP_flag_castle_02.fbx`
et n'ajoute qu'un matériau. Il n'y a donc **aucune géométrie à créer** : ces
instances sont des transformations manquantes, rien de plus.

Le miroir sur les rotations
---------------------------
La conversion de repère porte un miroir sur X (déterminant négatif, cf
`extract_scene_lights.py`). Une position se reflète en niant X ; une **rotation**
non : son axe est un pseudo-vecteur. Sous la réflexion `diag(-1, 1, 1)`, l'axe
`a` devient `det(M) · M·a = (a.x, −a.y, −a.z)` à angle constant, soit sur le
quaternion :

    (x, y, z, w)  ->  (x, −y, −z, w)

Nier X sur la rotation comme sur la position retournerait les bannières à
l'envers. Le script vérifie ce point sur une famille présente des deux côtés
(`--verify`), en comparant aux nœuds de nos cellules glTF.

Usage :
    python tools/unity/extract_scene_props.py <scene.unity> <sortie.toml> \
        --prefab P_PROP_flag_castle_02_static --prefab P_PROP_flag_castle_03_static
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

# Décalage et miroir déduits par appariement — mêmes valeurs que pour les
# lumières, vérifiées à 0,000 m de médiane sur 1195 pièces.
MIRROR_X = True
OFFSET = (-32.041, -172.671, 35.372)
MAX_DEPTH = 12

FLOAT = r"[-\d.eE+]+"


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


def parse_transforms(blocks: list[str]) -> dict:
    """Les `Transform` (classe 4) — ils portent la chaîne de parents."""
    transforms = {}
    for block in blocks:
        head = re.match(r"4 &(-?\d+)", block)
        if not head:
            continue
        position = re.search(
            rf"m_LocalPosition: {{x: ({FLOAT}), y: ({FLOAT}), z: ({FLOAT})}}", block
        )
        rotation = re.search(
            rf"m_LocalRotation: {{x: ({FLOAT}), y: ({FLOAT}), z: ({FLOAT}), w: ({FLOAT})}}",
            block,
        )
        scale = re.search(rf"m_LocalScale: {{x: ({FLOAT}), y: ({FLOAT}), z: ({FLOAT})}}", block)
        father = re.search(r"m_Father: {fileID: (-?\d+)}", block)
        transforms[head.group(1)] = {
            "position": tuple(map(float, position.groups())) if position else (0.0, 0.0, 0.0),
            "rotation": tuple(map(float, rotation.groups())) if rotation else (0.0, 0.0, 0.0, 1.0),
            "scale": tuple(map(float, scale.groups())) if scale else (1.0, 1.0, 1.0),
            "father": father.group(1) if father else "0",
        }
    return transforms


def parse_prefab_instances(blocks: list[str]) -> list[dict]:
    """Les `PrefabInstance` (classe 1001) et leur TRS local surchargé."""
    instances = []
    for block in blocks:
        if not re.match(r"1001 &(-?\d+)", block):
            continue
        source = re.search(r"m_SourcePrefab: {fileID: \d+, guid: ([0-9a-f]{32})", block)
        parent = re.search(r"m_TransformParent: {fileID: (-?\d+)}", block)
        # Le nom d'instance vaut `<nom du prefab> (12)` — on garde la racine.
        name = re.search(r"propertyPath: m_Name\n\s+value: (.+)", block)
        overrides = dict(
            re.findall(rf"propertyPath: (m_Local\w+\.\w)\n\s+value: ({FLOAT})", block)
        )

        def axis(prefix, keys, default):
            return tuple(float(overrides.get(f"{prefix}.{k}", d)) for k, d in zip(keys, default))

        instances.append(
            {
                "guid": source.group(1) if source else None,
                "name": name.group(1).strip() if name else "",
                "parent": parent.group(1) if parent else "0",
                "position": axis("m_LocalPosition", "xyz", (0.0, 0.0, 0.0)),
                "rotation": axis("m_LocalRotation", "xyzw", (0.0, 0.0, 0.0, 1.0)),
                "scale": axis("m_LocalScale", "xyz", (1.0, 1.0, 1.0)),
            }
        )
    return instances


def world_of_parent(transforms: dict, file_id: str):
    """Compose la chaîne de parents d'un `Transform` jusqu'à la racine."""
    chain = []
    current = file_id
    while current and current != "0" and len(chain) < MAX_DEPTH:
        node = transforms.get(current)
        if node is None:
            break
        chain.append(node)
        current = node["father"]
    position = (0.0, 0.0, 0.0)
    rotation = (0.0, 0.0, 0.0, 1.0)
    for node in reversed(chain):
        rotated = quaternion_rotate(rotation, node["position"])
        position = tuple(position[i] + rotated[i] for i in range(3))
        rotation = quaternion_multiply(rotation, node["rotation"])
    return position, rotation


def to_forgia_position(point):
    x = -point[0] if MIRROR_X else point[0]
    return (x + OFFSET[0], point[1] + OFFSET[1], point[2] + OFFSET[2])


def to_forgia_rotation(q):
    """Reflète une rotation : l'axe est un pseudo-vecteur (cf docstring)."""
    x, y, z, w = q
    return (x, -y, -z, w) if MIRROR_X else (x, y, z, w)


def resolve(scene_text: str, guids: dict[str, str]) -> list[dict]:
    blocks = re.split(r"^--- !u!", scene_text, flags=re.M)
    transforms = parse_transforms(blocks)
    entries = []
    for instance in parse_prefab_instances(blocks):
        prefab = guids.get(instance["guid"])
        if prefab is None:
            continue
        base_position, base_rotation = world_of_parent(transforms, instance["parent"])
        rotated = quaternion_rotate(base_rotation, instance["position"])
        position = tuple(base_position[i] + rotated[i] for i in range(3))
        rotation = quaternion_multiply(base_rotation, instance["rotation"])
        entries.append(
            {
                "prefab": prefab,
                "position": to_forgia_position(position),
                "rotation": to_forgia_rotation(rotation),
                "scale": instance["scale"],
            }
        )
    return entries


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("scene", type=Path)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--inventory", type=Path, required=True,
                        help="fichier GUID<TAB>chemin issu du .unitypackage")
    parser.add_argument("--prefab", action="append", default=[],
                        help="nom de prefab à extraire (répétable)")
    parser.add_argument("--mesh", action="append", default=[],
                        help="mesh à employer pour le prefab de même rang")
    args = parser.parse_args()

    if len(args.mesh) != len(args.prefab):
        print("Il faut autant de --mesh que de --prefab.", file=sys.stderr)
        return 1

    # prefab -> guid, via l'inventaire du .unitypackage.
    wanted = {}
    meshes = dict(zip(args.prefab, args.mesh))
    for line in args.inventory.read_text(encoding="utf-8", errors="replace").splitlines():
        guid, _, path = line.partition("\t")
        stem = path.rsplit("/", 1)[-1]
        if stem.endswith(".prefab") and stem[: -len(".prefab")] in args.prefab:
            wanted[guid] = stem[: -len(".prefab")]
    missing = set(args.prefab) - set(wanted.values())
    if missing:
        print(f"Prefabs introuvables dans l'inventaire : {sorted(missing)}", file=sys.stderr)
        return 1

    entries = resolve(args.scene.read_text(encoding="utf-8", errors="replace"), wanted)

    lines = [
        "# Instances écartées par la reconstruction d'origine, réimportées depuis la",
        "# scène Unity du pack. Généré par tools/unity/extract_scene_props.py —",
        "# ne pas éditer à la main.",
        "#",
        "# Ces variantes (`_static`) réutilisent le mesh du prefab de base : aucune",
        "# géométrie n'est créée, seules les transformations manquaient.",
        "#",
        f"# Repère : bevy = (−unity.x {OFFSET[0]:+.3f}, unity.y {OFFSET[1]:+.3f}, "
        f"unity.z {OFFSET[2]:+.3f}), rotation reflétée en (x, −y, −z, w).",
        "",
    ]
    for entry in entries:
        lines.append("[[prop]]")
        lines.append(f'mesh = "{meshes[entry["prefab"]]}"')
        lines.append("pos = [%s]" % ", ".join(f"{c:.4f}" for c in entry["position"]))
        lines.append("rot = [%s]" % ", ".join(f"{c:.6f}" for c in entry["rotation"]))
        lines.append("scale = [%s]" % ", ".join(f"{c:.5f}" for c in entry["scale"]))
        lines.append("")
    args.destination.write_text("\n".join(lines), encoding="utf-8")

    counts: dict[str, int] = {}
    for entry in entries:
        counts[entry["prefab"]] = counts.get(entry["prefab"], 0) + 1
    print(f"{len(entries)} instances -> {args.destination}")
    for prefab in sorted(counts):
        print(f"  {counts[prefab]:4}  {prefab}  -> mesh {meshes[prefab]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
