"""Extrait les lumières d'une scène Unity et les convertit dans le repère Forgia.

Contexte
--------
Le Hall de Forgia est reconstruit depuis le pack « FANTASTIC Highlands Castle ».
La reconstruction n'a gardé que les objets **porteurs de mesh** : les 57 lumières
placées à la main par le créateur, ses 31 sondes de réflexion et ses systèmes de
particules sont tombés en silence. Ce script récupère les lumières.

La conversion de repère n'est pas devinée, elle est **déduite** puis vérifiée :

1. On résout les positions **monde** (Unity) de pièces présentes des deux côtés,
   en remontant la chaîne de parents (`m_Father`).
2. On les apparie avec les positions des mêmes pièces dans nos cellules glTF.
3. On ajuste. Résultat obtenu le 2026-07-26, écart 0,0000 m sur 3 pièces uniques
   puis **écart médian 0,000 m sur 1 195 pièces** de 7 types :

       bevy = ( −unity.x − 32,041 , unity.y − 172,671 , unity.z + 35,372 )

   Le déterminant est **négatif** : le château porte un miroir sur X, ce qui
   confirme indépendamment ce qui avait été constaté sur le terrain.

Direction des spots : une lumière Unity pointe vers son **+Z local**. On applique
au vecteur avant le même miroir que sur les positions, et on oriente la lumière
Bevy avec `looking_to` (qui aligne son −Z).

Usage :
    python tools/unity/extract_scene_lights.py <scene.unity> <sortie.toml>

La scène s'obtient depuis le `.unitypackage` (tar gzip de dossiers GUID, chacun
contenant `asset` + `pathname`) — voir `docs/audits/audit-2026-07-26-diff-pack-unity-source.md`.
"""

from __future__ import annotations

import math
import re
import sys
from pathlib import Path

# Décalage et miroir déduits par appariement (cf docstring). En mètres.
MIRROR_X = True
OFFSET = (-32.041, -172.671, 35.372)
# Profondeur maximale de remontée de hiérarchie (garde-fou anti-cycle).
MAX_DEPTH = 12
UNITY_LIGHT_TYPE = {"0": "spot", "1": "directional", "2": "point"}


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


def parse_scene(text: str):
    """Retourne (transforms par fileID, liste des lumières)."""
    transforms = {}
    lights = []
    for block in re.split(r"^--- !u!", text, flags=re.M):
        head = re.match(r"(\d+) &(-?\d+)", block)
        if not head:
            continue
        class_id = head.group(1)
        if class_id == "4":  # Transform
            game_object = re.search(r"m_GameObject: \{fileID: (-?\d+)\}", block)
            position = re.search(
                r"m_LocalPosition: \{x: ([-\d.eE]+), y: ([-\d.eE]+), z: ([-\d.eE]+)\}", block
            )
            rotation = re.search(
                r"m_LocalRotation: \{x: ([-\d.eE]+), y: ([-\d.eE]+), z: ([-\d.eE]+), w: ([-\d.eE]+)\}",
                block,
            )
            father = re.search(r"m_Father: \{fileID: (-?\d+)\}", block)
            transforms[head.group(2)] = {
                "game_object": game_object.group(1) if game_object else None,
                "position": tuple(map(float, position.groups())) if position else (0.0, 0.0, 0.0),
                "rotation": tuple(map(float, rotation.groups())) if rotation else (0.0, 0.0, 0.0, 1.0),
                "father": father.group(1) if father else "0",
            }
        elif class_id == "108":  # Light
            def field(key, default=None):
                found = re.search(rf"^\s+{key}: (.+)$", block, re.M)
                return found.group(1).strip() if found else default

            color = re.search(r"m_Color: \{r: ([-\d.eE]+), g: ([-\d.eE]+), b: ([-\d.eE]+)", block)
            game_object = re.search(r"m_GameObject: \{fileID: (-?\d+)\}", block)
            lights.append(
                {
                    "game_object": game_object.group(1) if game_object else None,
                    "kind": UNITY_LIGHT_TYPE.get(field("m_Type"), field("m_Type")),
                    "intensity": float(field("m_Intensity", 0)),
                    "range": float(field("m_Range", 0)),
                    "outer": float(field("m_SpotAngle", 30)),
                    "inner": float(field("m_InnerSpotAngle", 21)),
                    "color": tuple(map(float, color.groups())) if color else (1.0, 1.0, 1.0),
                }
            )
    return transforms, lights


def world_transform(transforms, file_id):
    """Compose la chaîne de parents jusqu'à la racine de scène."""
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
        position = (position[0] + rotated[0], position[1] + rotated[1], position[2] + rotated[2])
        rotation = quaternion_multiply(rotation, node["rotation"])
    return position, rotation


def to_forgia(point):
    x = -point[0] if MIRROR_X else point[0]
    return (x + OFFSET[0], point[1] + OFFSET[1], point[2] + OFFSET[2])


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__)
        return 1
    scene = Path(sys.argv[1])
    destination = Path(sys.argv[2])
    transforms, lights = parse_scene(scene.read_text(encoding="utf-8", errors="replace"))
    by_game_object = {t["game_object"]: fid for fid, t in transforms.items() if t["game_object"]}

    entries = []
    skipped_directional = 0
    for light in lights:
        if light["kind"] == "directional":
            # Nos deux directionnelles sont réglées pour l'extérieur, qui rend
            # bien : on ne les remplace pas ici (cf `sun_scale` côté jeu).
            skipped_directional += 1
            continue
        file_id = by_game_object.get(light["game_object"])
        if not file_id:
            continue
        position, rotation = world_transform(transforms, file_id)
        forward = quaternion_rotate(rotation, (0.0, 0.0, 1.0))
        direction = (-forward[0] if MIRROR_X else forward[0], forward[1], forward[2])
        norm = math.sqrt(sum(c * c for c in direction)) or 1.0
        entries.append(
            {
                **light,
                "position": to_forgia(position),
                "direction": tuple(c / norm for c in direction),
            }
        )

    lines = [
        "# Lumières du créateur du pack, portées 1:1 depuis sa scène Unity.",
        "# Généré par tools/unity/extract_scene_lights.py — ne pas éditer à la main.",
        f"# Conversion vérifiée : bevy = (−unity.x {OFFSET[0]:+.3f}, unity.y {OFFSET[1]:+.3f}, unity.z {OFFSET[2]:+.3f}).",
        f"# {skipped_directional} directionnelle(s) volontairement non reprise(s).",
        "",
    ]
    for e in entries:
        lines.append("[[light]]")
        lines.append(f'kind = "{e["kind"]}"')
        lines.append("pos = [%s]" % ", ".join(f"{c:.3f}" for c in e["position"]))
        if e["kind"] == "spot":
            lines.append("dir = [%s]" % ", ".join(f"{c:.4f}" for c in e["direction"]))
            lines.append(f"outer_deg = {e['outer']}")
            lines.append(f"inner_deg = {e['inner']}")
        lines.append(f"intensity = {e['intensity']}")
        lines.append(f"range = {e['range']:.3f}")
        lines.append("color = [%s]" % ", ".join(f"{c:.4f}" for c in e["color"]))
        lines.append("")
    destination.write_text("\n".join(lines), encoding="utf-8")
    point = sum(1 for e in entries if e["kind"] == "point")
    spot = sum(1 for e in entries if e["kind"] == "spot")
    # ASCII : la console Windows est en cp1252, une flèche la ferait planter.
    print(f"{len(entries)} lumieres ecrites ({point} point, {spot} spot) -> {destination}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
