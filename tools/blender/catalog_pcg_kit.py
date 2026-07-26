"""Catalogueur Blender headless pour les kits PCG Forgia.

Usage :
  blender --background kit.blend --python tools/blender/catalog_pcg_kit.py -- \
    --kit-id forgia.castle.stone@1.0.0 \
    --asset-root assets/pcg/kits/castle_stone/1.0.0 \
    --output assets/pcg/kits/castle_stone/1.0.0/kit.toml

Convention de scène (strictement validée) :
  PCG_PIECE__<piece-id>  collection d'une pièce exportable
  PCG_ROOT__<piece-id>   Empty unique : origine locale stable de la pièce
  PCG_SOCKET__<id>       Empty socket, enfant/direct ou indirect de la collection
  PCG_COLLISION__*       mesh proxy physique (optionnel, mais conseillé)
  PCG_LOD<n>__*          mesh de LOD (optionnel)

Chaque socket porte des custom properties :
  pcg_family, pcg_role (structural|portal|utility|decor),
  pcg_gender (male|female|neutral), pcg_aperture_shape,
  pcg_width_m/pcg_height_m/pcg_depth_m (rect) ou
  pcg_radius_m/pcg_length_m (cylinder), pcg_accepts (JSON optionnel).

Le script produit des coordonnées Forgia/Bevy (Y-up) depuis Blender par
(x, y, z) -> (x, z, -y), la conversion prouvée dans les outils existants.
Il ne modifie jamais le .blend : il échoue explicitement si le contrat est flou.
"""

import argparse
import json
import re
import sys
from pathlib import Path

import bpy
from mathutils import Vector


PIECE_PREFIX = "PCG_PIECE__"
ROOT_PREFIX = "PCG_ROOT__"
SOCKET_PREFIX = "PCG_SOCKET__"
COLLISION_PREFIX = "PCG_COLLISION__"
LOD_RE = re.compile(r"^PCG_LOD(\d+)__")
VALID_ROLES = {"structural", "portal", "utility", "decor"}
VALID_GENDERS = {"male", "female", "neutral"}
VALID_APERTURES = {"rect", "cylinder"}


def cli_args():
    if "--" not in sys.argv:
        raise SystemExit("Arguments attendus après `--`; lance avec --help pour la syntaxe.")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--kit-id", required=True)
    parser.add_argument("--asset-root", required=True, help="Racine relative assets/ du kit exporté")
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--license", default="internal")
    parser.add_argument("--dry-run", action="store_true", help="Écrit le TOML sur stdout, jamais sur disque")
    return parser.parse_args(sys.argv[sys.argv.index("--") + 1 :])


def fail(message):
    raise SystemExit(f"PCG_KIT_CATALOG_ERROR: {message}")


def toml_string(value):
    return json.dumps(str(value), ensure_ascii=False)


def forgia_vector(vector):
    """Blender local (x,y,z) -> Forgia local (x,z,-y), sans réflexion cachée."""
    return [round(vector.x, 6), round(vector.z, 6), round(-vector.y, 6)]


def array_toml(values):
    return "[" + ", ".join(f"{value:.6f}" for value in values) + "]"


def objects_recursive(collection):
    objects = list(collection.objects)
    for child in collection.children:
        objects.extend(objects_recursive(child))
    return objects


def property_string(obj, key):
    value = obj.get(key)
    if not isinstance(value, str) or not value.strip():
        fail(f"{obj.name}: custom property string obligatoire `{key}` absente")
    return value.strip()


def property_number(obj, key):
    value = obj.get(key)
    if not isinstance(value, (int, float)) or not float(value) > 0.0:
        fail(f"{obj.name}: custom property numérique > 0 obligatoire `{key}` absente")
    return float(value)


def optional_number(obj, key):
    value = obj.get(key)
    if value is None:
        return None
    if not isinstance(value, (int, float)) or not float(value) > 0.0:
        fail(f"{obj.name}: `{key}` doit être numérique > 0 quand présent")
    return float(value)


def clearance_toml(socket):
    """Optionnel : volume libre autour de l'interface (piège capsule/porte).

    Props : pcg_clearance_shape (box|cylinder), et selon la forme
    pcg_clearance_hx/hy/hz (box, demi-extents en axes Forgia) ou
    pcg_clearance_radius_m/pcg_clearance_length_m (cylinder)."""
    shape = socket.get("pcg_clearance_shape")
    if not isinstance(shape, str) or not shape.strip():
        return None
    shape = shape.strip()
    if shape == "box":
        half = [
            property_number(socket, "pcg_clearance_hx"),
            property_number(socket, "pcg_clearance_hy"),
            property_number(socket, "pcg_clearance_hz"),
        ]
        return 'clearance = { shape = "box", half_extents_m = %s }' % array_toml(half)
    if shape == "cylinder":
        return 'clearance = { shape = "cylinder", radius_m = %.6f, length_m = %.6f }' % (
            property_number(socket, "pcg_clearance_radius_m"),
            property_number(socket, "pcg_clearance_length_m"),
        )
    fail(f"{socket.name}: pcg_clearance_shape invalide `{shape}`")


def piece_bounds(meshes, root_inverse):
    """AABB solide de la pièce (axes Forgia), depuis les meshes de rendu."""
    if not meshes:
        return None
    mn = [float("inf")] * 3
    mx = [float("-inf")] * 3
    for obj in meshes:
        for corner in obj.bound_box:
            local = root_inverse @ (obj.matrix_world @ Vector(corner))
            fv = forgia_vector(local)
            for i in range(3):
                mn[i] = min(mn[i], fv[i])
                mx[i] = max(mx[i], fv[i])
    return (mn, mx)


def parse_accepts(obj):
    raw = obj.get("pcg_accepts", "[]")
    if not isinstance(raw, str):
        fail(f"{obj.name}: `pcg_accepts` doit être une chaîne JSON")
    try:
        rules = json.loads(raw)
    except json.JSONDecodeError as error:
        fail(f"{obj.name}: `pcg_accepts` JSON invalide: {error}")
    if not isinstance(rules, list) or not rules:
        fail(f"{obj.name}: `pcg_accepts` doit contenir au moins une règle")
    normalized = []
    for rule in rules:
        if not isinstance(rule, dict) or not isinstance(rule.get("family"), str):
            fail(f"{obj.name}: chaque règle pcg_accepts exige `family`")
        gender = rule.get("gender")
        if gender is not None and gender not in VALID_GENDERS:
            fail(f"{obj.name}: gender de pcg_accepts invalide `{gender}`")
        normalized.append({"family": rule["family"], "gender": gender})
    return normalized


def socket_id_of(socket):
    """Real socket id: the explicit `pcg_socket_id` prop wins; otherwise the name
    minus the prefix, with Blender's `.NNN` duplicate suffix stripped so pieces
    can legitimately share socket ids (`west`, `east`) without a global clash."""
    explicit = socket.get("pcg_socket_id")
    if isinstance(explicit, str) and explicit.strip():
        return explicit.strip()
    return re.sub(r"\.\d{3}$", "", socket.name.removeprefix(SOCKET_PREFIX))


def socket_toml(socket, root_inverse):
    socket_id = socket_id_of(socket)
    if not socket_id:
        fail(f"{socket.name}: ID socket vide")
    family = property_string(socket, "pcg_family")
    role = property_string(socket, "pcg_role")
    gender = property_string(socket, "pcg_gender")
    shape = property_string(socket, "pcg_aperture_shape")
    if role not in VALID_ROLES:
        fail(f"{socket.name}: pcg_role invalide `{role}`")
    if gender not in VALID_GENDERS:
        fail(f"{socket.name}: pcg_gender invalide `{gender}`")
    if shape not in VALID_APERTURES:
        fail(f"{socket.name}: pcg_aperture_shape invalide `{shape}`")

    local = root_inverse @ socket.matrix_world
    origin = forgia_vector(local.translation)
    forward = forgia_vector(local.col[1].to_3d())  # +Y Blender = forward de socket
    up = forgia_vector(local.col[2].to_3d())       # +Z Blender = up de socket
    if Vector(forward).length < 0.999 or Vector(up).length < 0.999:
        fail(f"{socket.name}: frame socket dégénérée")

    aperture = [f'shape = {toml_string(shape)}']
    if shape == "rect":
        aperture.extend(
            [
                f"width_m = {property_number(socket, 'pcg_width_m'):.6f}",
                f"height_m = {property_number(socket, 'pcg_height_m'):.6f}",
                f"depth_m = {property_number(socket, 'pcg_depth_m'):.6f}",
            ]
        )
    else:
        aperture.extend(
            [
                f"radius_m = {property_number(socket, 'pcg_radius_m'):.6f}",
                f"length_m = {property_number(socket, 'pcg_length_m'):.6f}",
            ]
        )
    accepts = parse_accepts(socket)

    lines = ["[[pieces.sockets]]"]
    lines.append(f"id = {toml_string(socket_id)}")
    lines.append(f"family = {toml_string(family)}")
    lines.append(f"role = {toml_string(role)}")
    lines.append(f"gender = {toml_string(gender)}")
    lines.append(
        "frame = { origin_m = %s, forward = %s, up = %s }"
        % (array_toml(origin), array_toml(forward), array_toml(up))
    )
    lines.append("aperture = { " + ", ".join(aperture) + " }")
    clearance = clearance_toml(socket)
    if clearance is not None:
        lines.append(clearance)
    lines.append(
        "accepts = ["
        + ", ".join(
            "{ family = %s%s }"
            % (
                toml_string(rule["family"]),
                "" if rule["gender"] is None else f", gender = {toml_string(rule['gender'])}",
            )
            for rule in accepts
        )
        + "]"
    )
    return lines


def collection_piece(collection, asset_root):
    piece_id = collection.name.removeprefix(PIECE_PREFIX)
    if not piece_id:
        fail(f"{collection.name}: ID piece vide")
    objects = objects_recursive(collection)
    roots = [obj for obj in objects if obj.name == ROOT_PREFIX + piece_id]
    if len(roots) != 1:
        fail(f"{collection.name}: exige exactement un Empty `{ROOT_PREFIX}{piece_id}`")
    root = roots[0]
    if root.type != "EMPTY":
        fail(f"{root.name}: PCG_ROOT doit être un Empty")
    root_inverse = root.matrix_world.inverted()
    sockets = sorted((obj for obj in objects if obj.name.startswith(SOCKET_PREFIX)), key=lambda obj: obj.name)
    if not sockets:
        fail(f"{collection.name}: aucune socket `{SOCKET_PREFIX}*`")
    socket_ids = [socket_id_of(obj) for obj in sockets]
    if len(socket_ids) != len(set(socket_ids)):
        fail(f"{collection.name}: IDs socket dupliqués")

    # Chemins d'asset relatifs à `assets/` : le kit.toml est consommé par le
    # AssetServer Bevy dont la racine EST `assets/` (sinon double préfixe).
    asset_root_rel = asset_root[len("assets/") :] if asset_root.startswith("assets/") else asset_root
    asset = root.get("pcg_asset", f"{asset_root_rel}/render/{piece_id}.glb#Scene0")
    collision_meshes = [obj for obj in objects if obj.name.startswith(COLLISION_PREFIX)]
    render_meshes = [
        obj
        for obj in objects
        if obj.type == "MESH"
        and not obj.name.startswith(COLLISION_PREFIX)
        and LOD_RE.match(obj.name) is None
    ]
    lod_indices = sorted(
        {int(match.group(1)) for obj in objects if (match := LOD_RE.match(obj.name)) is not None}
    )
    lines = ["[[pieces]]"]
    lines.append(f"id = {toml_string(piece_id)}")
    lines.append(f"asset = {toml_string(asset)}")
    bounds = piece_bounds(render_meshes, root_inverse)
    if bounds is not None:
        lines.append(
            "bounds_m = { min = %s, max = %s }" % (array_toml(bounds[0]), array_toml(bounds[1]))
        )
    provides = root.get("pcg_provides", "[]")
    try:
        provides = json.loads(provides) if isinstance(provides, str) else provides
    except json.JSONDecodeError as error:
        fail(f"{root.name}: pcg_provides JSON invalide: {error}")
    if not isinstance(provides, list) or not all(isinstance(value, str) for value in provides):
        fail(f"{root.name}: pcg_provides doit être un tableau JSON de chaînes")
    lines.append("provides = [" + ", ".join(toml_string(value) for value in provides) + "]")
    if collision_meshes:
        lines.append(
            "collision = { kind = \"proxy_mesh\", asset = %s }"
            % toml_string(f"{asset_root_rel}/collision/{piece_id}_proxy.glb#Scene0")
        )
    if lod_indices:
        lods = [
            "{ asset = %s, max_distance_m = %.1f }"
            % (toml_string(f"{asset_root_rel}/render/{piece_id}_lod{index}.glb#Scene0"), 30.0 * (index + 1))
            for index in lod_indices
        ]
        lines.append("lods = [" + ", ".join(lods) + "]")
    for socket in sockets:
        lines.extend(socket_toml(socket, root_inverse))
    return lines


def main():
    args = cli_args()
    pieces = sorted(
        (collection for collection in bpy.data.collections if collection.name.startswith(PIECE_PREFIX)),
        key=lambda collection: collection.name,
    )
    if not pieces:
        fail(f"aucune collection `{PIECE_PREFIX}<id>` dans le fichier Blender courant")
    ids = [collection.name.removeprefix(PIECE_PREFIX) for collection in pieces]
    if len(ids) != len(set(ids)):
        fail("IDs de pièces dupliqués")

    lines = [
        "# Generated by tools/blender/catalog_pcg_kit.py — do not hand-edit generated fields.",
        'schema_version = "forgia.kit-manifest/v1"',
        f"id = {toml_string(args.kit_id)}",
        f"license = {toml_string(args.license)}",
        'source = { tool = "Blender 5.0", coordinate_system = "forgia_y_up_meters" }',
        "",
    ]
    for collection in pieces:
        lines.extend(collection_piece(collection, args.asset_root.rstrip("/")))
        lines.append("")
    output = "\n".join(lines)
    if args.dry_run:
        print(output)
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(output, encoding="utf-8", newline="\n")
        print(f"PCG_KIT_CATALOG_OK pieces={len(pieces)} output={args.output}")


if __name__ == "__main__":
    main()
