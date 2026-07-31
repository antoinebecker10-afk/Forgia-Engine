#!/usr/bin/env python3
"""scan_assets.py — indexe TOUS les assets 3D en les MESURANT (story-673).

Pourquoi cet outil existe
-------------------------
Jusqu'ici la classification des assets se faisait sur le NOM DE FICHIER.
« RockBig_001.glb » était rangé dans les gros props parce qu'il s'appelle « Big ».
Aucune taille, aucun pivot, aucune orientation n'était connue. Conséquence
mesurée le 2026-07-31 : un bâtiment de 12 m s'est déclaré 1,92 m de rayon
d'emprise — les ennemis naissaient dedans, le joueur apparaissait dans un asset.

Une valeur de tuning n'est pas une mesure. Cet outil produit des mesures.

Ce qu'il lit vraiment
---------------------
glTF (.gltf) est du JSON ; GLB (.glb) est un conteneur binaire dont le PREMIER
chunk est ce même JSON. On n'a donc besoin ni de dépendance externe, ni de lire
les buffers : les accesseurs `POSITION` portent déjà leurs `min`/`max`, c'est-à-
dire la boîte englobante de chaque primitive. On compose ensuite les transformées
de la hiérarchie de nœuds pour obtenir l'AABB du modèle entier.

Corollaire pratique : pour un GLB de 185 Mo on ne lit que les premiers Ko.

Sortie
------
`assets/genomes/asset_registry.toml` — un enregistrement par fichier, versionné.
C'est de la DONNÉE MESURÉE : ne pas l'éditer à la main, le régénérer.

    python tools/gltf/scan_assets.py
    python tools/gltf/scan_assets.py --check   # échoue si le registre est périmé
"""

from __future__ import annotations

import argparse
import json
import math
import os
import struct
import sys
from typing import Any

ASSET_ROOT = "assets/models"
OUT_PATH = "assets/genomes/asset_registry.toml"

# ── Bandes de rôle, dérivées des métriques joueur mesurées ────────────────────
# Source : .claude/rules/map-design-patterns.md §11 (« sans accroupissement, la
# couverture est binaire ») — œil à 1,70 m, saut 1,174 m, pas de crouch.
JUMP_H = 1.174          # ≤ : franchissable au saut → traversée, pas couverture
EYE_H = 1.70            # entre les deux : masque le corps, PAS la vue → inutile
SIGHT_BREAK_H = 1.80    # ≥ : casse réellement la ligne de vue
LANDMARK_H = 8.0        # ≥ : point focal visible de loin

# ⚠️ Ces rôles valent pour la taille NATIVE. Le décor de Forgia recalibre chaque
# prop à une taille cible au runtime (`NeedsDecorCalibrate`) : le rôle réel en jeu
# se dérive de l'échelle FINALE, pas de celle-ci. Le registre dit ce que l'asset
# EST ; c'est au consommateur de dire ce qu'il en fait.


def read_gltf_json(path: str) -> dict[str, Any] | None:
    """Renvoie le document glTF, que le fichier soit .gltf (JSON) ou .glb."""
    try:
        if path.lower().endswith(".gltf"):
            with open(path, "rb") as f:
                return json.loads(f.read().decode("utf-8", errors="replace"))
        with open(path, "rb") as f:
            header = f.read(12)
            if len(header) < 12 or header[:4] != b"glTF":
                return None
            # GLB : suite de chunks (length u32, type u32, data). Le 1er est le JSON.
            chunk_header = f.read(8)
            if len(chunk_header) < 8:
                return None
            length, ctype = struct.unpack("<II", chunk_header)
            if ctype != 0x4E4F534A:  # 'JSON'
                return None
            return json.loads(f.read(length).decode("utf-8", errors="replace"))
    except Exception:
        return None


# ── Algèbre minimale (pas de dépendance) ─────────────────────────────────────

def mat_identity() -> list[float]:
    return [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]


def mat_mul(a: list[float], b: list[float]) -> list[float]:
    """Colonnes-majeur, convention glTF."""
    out = [0.0] * 16
    for c in range(4):
        for r in range(4):
            out[c * 4 + r] = sum(a[k * 4 + r] * b[c * 4 + k] for k in range(4))
    return out


def node_matrix(node: dict[str, Any]) -> list[float]:
    if "matrix" in node:
        return list(node["matrix"])
    t = node.get("translation", [0, 0, 0])
    r = node.get("rotation", [0, 0, 0, 1])  # quaternion xyzw
    s = node.get("scale", [1, 1, 1])
    x, y, z, w = r
    xx, yy, zz = x * x, y * y, z * z
    m = [
        (1 - 2 * (yy + zz)) * s[0], (2 * (x * y + z * w)) * s[0], (2 * (x * z - y * w)) * s[0], 0,
        (2 * (x * y - z * w)) * s[1], (1 - 2 * (xx + zz)) * s[1], (2 * (y * z + x * w)) * s[1], 0,
        (2 * (x * z + y * w)) * s[2], (2 * (y * z - x * w)) * s[2], (1 - 2 * (xx + yy)) * s[2], 0,
        t[0], t[1], t[2], 1,
    ]
    return m


def transform_point(m: list[float], p: tuple[float, float, float]) -> tuple[float, float, float]:
    x, y, z = p
    return (
        m[0] * x + m[4] * y + m[8] * z + m[12],
        m[1] * x + m[5] * y + m[9] * z + m[13],
        m[2] * x + m[6] * y + m[10] * z + m[14],
    )


def measure(doc: dict[str, Any]) -> dict[str, Any] | None:
    """AABB monde + compteurs, en composant la hiérarchie de nœuds."""
    nodes = doc.get("nodes", [])
    meshes = doc.get("meshes", [])
    accessors = doc.get("accessors", [])
    if not nodes or not meshes:
        return None

    lo = [math.inf] * 3
    hi = [-math.inf] * 3
    tris = 0
    prims = 0
    mesh_hits = 0

    scenes = doc.get("scenes", [])
    scene_idx = doc.get("scene", 0)
    roots = (
        scenes[scene_idx].get("nodes", [])
        if 0 <= scene_idx < len(scenes)
        else list(range(len(nodes)))
    )

    stack = [(i, mat_identity()) for i in roots]
    seen = 0
    while stack:
        seen += 1
        if seen > 100_000:  # garde-fou anti-cycle
            break
        idx, parent = stack.pop()
        if not (0 <= idx < len(nodes)):
            continue
        node = nodes[idx]
        world = mat_mul(parent, node_matrix(node))
        mi = node.get("mesh")
        if mi is not None and 0 <= mi < len(meshes):
            mesh_hits += 1
            for prim in meshes[mi].get("primitives", []):
                prims += 1
                ii = prim.get("indices")
                if ii is not None and 0 <= ii < len(accessors):
                    tris += accessors[ii].get("count", 0) // 3
                pa = prim.get("attributes", {}).get("POSITION")
                if pa is None or not (0 <= pa < len(accessors)):
                    continue
                acc = accessors[pa]
                amin, amax = acc.get("min"), acc.get("max")
                if not amin or not amax or len(amin) < 3:
                    continue
                # Les 8 coins transformés : une AABB tournée reste correcte.
                for cx in (amin[0], amax[0]):
                    for cy in (amin[1], amax[1]):
                        for cz in (amin[2], amax[2]):
                            wp = transform_point(world, (cx, cy, cz))
                            for k in range(3):
                                lo[k] = min(lo[k], wp[k])
                                hi[k] = max(hi[k], wp[k])
        for child in node.get("children", []):
            stack.append((child, world))

    if any(math.isinf(v) for v in lo + hi):
        return None
    return {
        "min": [round(v, 4) for v in lo],
        "max": [round(v, 4) for v in hi],
        "size": [round(hi[k] - lo[k], 4) for k in range(3)],
        "triangles": tris,
        "primitives": prims,
        "meshes_used": mesh_hits,
        "materials": len(doc.get("materials", [])),
        "skinned": bool(doc.get("skins")),
        "animations": len(doc.get("animations", [])),
    }


# ── Classification : ce qui se DÉRIVE d'une mesure, et ce qui vient du chemin ──

def role_from_height(h: float) -> str:
    """Le rôle se dérive de la HAUTEUR (map-design-patterns §11), pas du nom."""
    if h >= LANDMARK_H:
        return "landmark"
    if h >= SIGHT_BREAK_H:
        return "cover_high"
    if h > JUMP_H:
        return "cover_useless"  # masque le corps, pas la vue — ne sert à rien
    return "traversal"          # franchissable au saut


def pivot_kind(lo: list[float], hi: list[float]) -> str:
    """Où est l'origine par rapport à la boîte ? Décide si un prop s'enterre."""
    h = hi[1] - lo[1]
    if h <= 1e-4:
        return "flat"
    r = (0.0 - lo[1]) / h  # position de y=0 dans la boîte
    if r < 0.15:
        return "base"       # origine au sol → poser tel quel
    if r > 0.85:
        return "top"
    if 0.35 < r < 0.65:
        return "center"     # origine au centre → remonter de h/2
    return "offset"


def kit_of(asset_rel: str) -> str:
    """`asset_rel` est relatif à `assets/` : « models/kaykit/dungeon/wall.glb »."""
    p = asset_rel.split("/")
    if len(p) > 2 and p[1] == "kaykit":
        return "kaykit_" + p[2]
    if len(p) > 2 and p[1] == "environment":
        return "env_" + p[2]
    return p[1] if len(p) > 1 else "?"


def nature_of(rel: str, size: list[float]) -> str:
    """Nature grossière — dérivée du chemin ET de la forme. Surchargeable."""
    n = rel.lower()
    w, h, d = size
    flat = h < 0.35 * max(w, d, 1e-3)
    if "/characters/" in n or "skeleton" in n:
        return "character"
    if "/weapons/" in n or "/arms/" in n:
        return "weapon"
    if "wall" in n or "fence" in n:
        return "wall"
    if "floor" in n or "tile" in n or (flat and max(w, d) > 2.0):
        return "floor"
    if "building" in n or "tower" in n or "castle" in n or "church" in n:
        return "building"
    if "tree" in n or "hill" in n or "mountain" in n or "rock" in n or "crag" in n:
        return "nature"
    if "brazier" in n or "candle" in n or "lamp" in n or "torch" in n:
        return "light"
    return "prop"


def scan(root: str) -> list[dict[str, Any]]:
    out = []
    for dirpath, _, files in os.walk(root):
        for fn in sorted(files):
            if not fn.lower().endswith((".glb", ".gltf")):
                continue
            full = os.path.join(dirpath, fn)
            rel = os.path.relpath(full, ".").replace(os.sep, "/")
            asset_rel = rel[len("assets/"):] if rel.startswith("assets/") else rel
            rec: dict[str, Any] = {
                "path": asset_rel,
                "kit": kit_of(asset_rel),
                "bytes": os.path.getsize(full),
            }
            doc = read_gltf_json(full)
            if doc is None:
                rec["measured"] = False
                rec["error"] = "glTF illisible"
                out.append(rec)
                continue
            m = measure(doc)
            if m is None:
                rec["measured"] = False
                rec["error"] = "aucune AABB exploitable"
                out.append(rec)
                continue
            rec["measured"] = True
            rec.update(m)
            w, h, d = m["size"]
            rec["height_m"] = round(h, 3)
            rec["footprint_radius_m"] = round(0.5 * math.hypot(w, d), 3)
            rec["pivot"] = pivot_kind(m["min"], m["max"])
            rec["role_native"] = role_from_height(h)
            rec["nature"] = nature_of(asset_rel, m["size"])
            out.append(rec)
    return out


def to_toml(records: list[dict[str, Any]]) -> str:
    ok = [r for r in records if r.get("measured")]
    ko = [r for r in records if not r.get("measured")]
    lines = [
        "# asset_registry.toml — GÉNÉRÉ, ne pas éditer à la main (story-673).",
        "#",
        "# Produit par `python tools/gltf/scan_assets.py` en LISANT chaque fichier",
        "# glTF/GLB : les accesseurs POSITION portent leur min/max, composés avec la",
        "# hiérarchie de nœuds → AABB réelle. Aucune valeur n'est devinée.",
        "#",
        "# ⚠️ `role_native` vaut pour la taille NATIVE. Le décor recalibre les props à",
        "# une taille cible au runtime : le rôle EN JEU se dérive de l'échelle finale.",
        "# Le registre dit ce que l'asset EST, pas ce qu'on en fait.",
        "#",
        f"# {len(ok)} mesurés / {len(records)} fichiers.",
        "",
        "[meta]",
        "version = 1",
        f"total_files = {len(records)}",
        f"measured = {len(ok)}",
        f"unmeasured = {len(ko)}",
        "",
    ]
    for r in sorted(records, key=lambda x: x["path"]):
        lines.append("[[assets]]")
        lines.append(f'path = "{r["path"]}"')
        lines.append(f'kit = "{r["kit"]}"')
        lines.append(f"bytes = {r['bytes']}")
        lines.append(f"measured = {str(r.get('measured', False)).lower()}")
        if not r.get("measured"):
            lines.append(f'error = "{r.get("error", "?")}"')
            lines.append("")
            continue
        s = r["size"]
        lines.append(f"size_m = [{s[0]}, {s[1]}, {s[2]}]")
        lines.append(f"height_m = {r['height_m']}")
        lines.append(f"footprint_radius_m = {r['footprint_radius_m']}")
        lines.append(f'pivot = "{r["pivot"]}"')
        lines.append(f'role_native = "{r["role_native"]}"')
        lines.append(f'nature = "{r["nature"]}"')
        lines.append(f"triangles = {r['triangles']}")
        lines.append(f"materials = {r['materials']}")
        lines.append(f"skinned = {str(r['skinned']).lower()}")
        lines.append(f"animations = {r['animations']}")
        lines.append("")
    return "\n".join(lines)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="échoue si le registre est périmé")
    args = ap.parse_args()

    if not os.path.isdir(ASSET_ROOT):
        print(f"ERREUR : {ASSET_ROOT} introuvable (lancer depuis la racine du repo)")
        return 2

    records = scan(ASSET_ROOT)
    content = to_toml(records)

    if args.check:
        old = ""
        if os.path.exists(OUT_PATH):
            with open(OUT_PATH, encoding="utf-8") as f:
                old = f.read()
        if old != content:
            print("ERREUR : asset_registry.toml est PERIME - relancer scan_assets.py")
            return 1
        print(f"asset_registry.toml a jour ({len(records)} fichiers)")
        return 0

    with open(OUT_PATH, "w", encoding="utf-8") as f:
        f.write(content)

    ok = [r for r in records if r.get("measured")]
    print(f"{len(ok)}/{len(records)} assets mesures -> {OUT_PATH}")
    for r in records:
        if not r.get("measured"):
            print(f"  NON MESURE  {r['path']} : {r.get('error')}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
