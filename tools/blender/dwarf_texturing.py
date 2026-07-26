"""Cuisson PBR procédurale — trois cartes générées dans Blender, rien de téléchargé.

Par groupe (le corps entier, puis chaque pièce d'armure) on déplie un atlas UV
partagé, puis on cuit :

  <groupe>_basecolor.png  albédo × occlusion ambiante × usure d'arêtes × grain
  <groupe>_orm.png        G = rugosité modulée, B = métallicité (convention glTF)
  <groupe>_normal.png     relief tangent issu d'un bump procédural

Le principe : les signaux qui donnent l'impression d'un objet FABRIQUÉ sont
calculables, pas photographiés. L'occlusion ambiante creuse les interstices, la
« pointiness » (convexité) éclaircit et use les arêtes saillantes, un bruit
procédural casse l'uniformité du spéculaire et sculpte un grain — martelé sur
l'acier, granuleux sur le cuir, tissé sur l'étoffe.

Trois passes = trois reconstructions du node tree de chaque matériau, puis un
matériau final unique par groupe qui recâble les trois images dans un Principled.
Les matériaux ÉMISSIFS sont exclus de la fusion et conservés dans leur propre
slot, sinon la braise perdrait son glow.

Consommé par build_dwarf.py — pas d'usage direct en ligne de commande.
"""

from math import radians

import bpy

# Recette de surface par matériau (clé = nom sans le préfixe `dwarf_`).
#   scale/detail : bruit procédural  ·  bump : force du relief
#   grain        : amplitude de la variation d'albédo
#   wear         : usure des arêtes convexes  ·  wear_color : teinte de l'usure
#   ao           : plancher d'occlusion (plus bas = creux plus sombres)
#   rvar         : amplitude de variation de rugosité
SURFACE = {
    # Phase 1 : `bump` et `grain` BAISSENT, `rvar` MONTE. Règle de production
    # relevée sur les pipelines stylisés — pas de micro-relief ni de motif
    # textile sur le maillage, le détail va dans la rugosité où il reste lisible
    # de loin et ne se révèle qu'au spéculaire de près.
    "skin": dict(scale=30.0, detail=2.0, bump=0.05, grain=0.04, wear=0.00,
                 wear_color=(0.95, 0.78, 0.66), ao=0.38, rvar=0.08,
                 edge=0.10, cav=0.62, dust=0.02, lit=0.14),
    "hair": dict(scale=18.0, detail=4.0, bump=0.22, grain=0.10, wear=0.06,
                 wear_color=(0.88, 0.48, 0.22), ao=0.26, rvar=0.14,
                 edge=0.12, cav=0.44, dust=0.03),
    "eye": dict(scale=1.0, detail=0.0, bump=0.00, grain=0.00, wear=0.00,
                wear_color=(0.0, 0.0, 0.0), ao=0.90, rvar=0.00,
                edge=0.0, cav=0.88, dust=0.0, lit=0.05),
    "eye_white": dict(scale=1.0, detail=0.0, bump=0.00, grain=0.00, wear=0.00,
                      wear_color=(1.0, 1.0, 1.0), ao=0.88, rvar=0.00,
                      edge=0.0, cav=0.85, dust=0.0, lit=0.05),
    "tunic": dict(scale=80.0, detail=2.0, bump=0.08, grain=0.07, wear=0.05,
                  wear_color=(0.58, 0.30, 0.26), ao=0.30, rvar=0.16,
                  edge=0.12, cav=0.50, dust=0.08),
    "trouser": dict(scale=70.0, detail=2.0, bump=0.07, grain=0.06, wear=0.06,
                    wear_color=(0.40, 0.37, 0.42), ao=0.30, rvar=0.16,
                    edge=0.12, cav=0.50, dust=0.09),
    "leather": dict(scale=26.0, detail=4.0, bump=0.14, grain=0.09, wear=0.14,
                    wear_color=(0.56, 0.38, 0.24), ao=0.24, rvar=0.20,
                    edge=0.22, cav=0.42, dust=0.08),
    "leather_light": dict(scale=28.0, detail=4.0, bump=0.14, grain=0.09, wear=0.16,
                          wear_color=(0.66, 0.48, 0.28), ao=0.24, rvar=0.20,
                          edge=0.24, cav=0.42, dust=0.08),
    # fourrure : seule exception à la baisse de relief — le poil EST du relief.
    # Occlusion très profonde entre les touffes : c'est le contraste sombre à
    # leur base qui la fait lire comme de la fourrure.
    "fur": dict(scale=44.0, detail=6.0, bump=0.62, grain=0.16, wear=0.06,
                wear_color=(0.44, 0.36, 0.30), ao=0.14, rvar=0.10,
                edge=0.16, cav=0.20, dust=0.06),
    "sole": dict(scale=34.0, detail=3.0, bump=0.18, grain=0.07, wear=0.10,
                 wear_color=(0.26, 0.23, 0.21), ao=0.24, rvar=0.14,
                 edge=0.14, cav=0.40, dust=0.12),
    "steel": dict(scale=16.0, detail=4.0, bump=0.12, grain=0.05, wear=0.32,
                  wear_color=(0.80, 0.84, 0.88), ao=0.22, rvar=0.24,
                  edge=0.38, cav=0.34, dust=0.07),
    "steel_dark": dict(scale=16.0, detail=4.0, bump=0.12, grain=0.05, wear=0.28,
                       wear_color=(0.62, 0.66, 0.72), ao=0.22, rvar=0.24,
                       edge=0.32, cav=0.34, dust=0.07),
    "brass": dict(scale=18.0, detail=4.0, bump=0.11, grain=0.05, wear=0.30,
                  wear_color=(0.92, 0.74, 0.42), ao=0.22, rvar=0.22,
                  edge=0.36, cav=0.32, dust=0.06),
    "glass": dict(scale=6.0, detail=1.0, bump=0.00, grain=0.02, wear=0.10,
                  wear_color=(0.55, 0.66, 0.72), ao=0.50, rvar=0.04,
                  edge=0.30, cav=0.70, dust=0.02, lit=0.08),
    "gold": dict(scale=20.0, detail=3.0, bump=0.09, grain=0.04, wear=0.24,
                 wear_color=(0.96, 0.82, 0.46), ao=0.24, rvar=0.18,
                 edge=0.34, cav=0.36, dust=0.05),
}
DEFAULT = SURFACE["steel"]

# Deux occlusions superposées : la large creuse les grands volumes (aisselles,
# sous la jupe, sous la barbe), la très courte pose les petits éléments sur leur
# support (chaque rivet gagne son ombre de contact). Avec la seule large, les
# rivets flottent ; avec la seule courte, le personnage reste plat.
AO_DISTANCE = 0.20  # portée large, à l'échelle d'un nain de 1.45 m
AO_CONTACT = 0.030  # portée de contact, à l'échelle d'un rivet
AO_CONTACT_FLOOR = 0.52
AO_SAMPLES = 16
WEAR_ROUGHEN = 0.18  # une arête usée diffuse plus
BUMP_DISTANCE = 0.009  # profondeur du relief : au-delà, tout lit « stuc »

CHAR_HEIGHT = 1.45  # borne haute du dégradé de lumière peinte
CAVITY_RANGE = (0.400, 0.497)  # pointiness CONCAVE — le creux, pas l'arête
UP_RANGE = (0.25, 1.0)  # à partir de quelle inclinaison une face « regarde le ciel »
DETAIL_NOISE_RATIO = 3.4  # le bruit fin ne sert QU'À la rugosité

# Réglages par défaut des signaux ajoutés en phase 1 ; une recette peut
# surcharger n'importe laquelle de ces clés.
SIGNAL_DEFAULTS = {
    "edge": 0.20,  # éclaircissement des arêtes convexes (« painted bevel »)
    "cav": 0.55,  # plancher de cavité : plus bas = creux plus sombres
    "dust": 0.05,  # poussière sur les faces tournées vers le haut
    "dust_color": (0.60, 0.56, 0.50),
    "lit": 0.16,  # amplitude du dégradé de lumière peinte dans l'albédo
}


# ---------------------------------------------------------------- utilitaires


def _sock(node, name, kind, out=False):
    """Socket par (nom, type).

    Les nodes modernes (Mix, MapRange) exposent PLUSIEURS sockets homonymes, un
    par type de donnée — `node.inputs["A"]` renvoie alors le mauvais. Filtrer sur
    le type est la seule façon stable entre versions de Blender.
    """
    for sock in node.outputs if out else node.inputs:
        if sock.name == name and sock.type == kind:
            return sock
    raise KeyError(f"{node.bl_idname}: socket {name}/{kind} introuvable")


def _recipe(mat):
    return SURFACE.get(mat.name.removeprefix("dwarf_"), DEFAULT)


def _occlusion(nt, distance, floor):
    """Une occlusion ambiante ramenée dans [floor, 1] — 1 = dégagé, floor = creux."""
    ao = nt.nodes.new("ShaderNodeAmbientOcclusion")
    ao.samples = AO_SAMPLES
    ao.inside = False
    ao.only_local = False
    ao.inputs["Distance"].default_value = distance
    remap = nt.nodes.new("ShaderNodeMapRange")
    _sock(remap, "From Min", "VALUE").default_value = 0.0
    _sock(remap, "From Max", "VALUE").default_value = 1.0
    _sock(remap, "To Min", "VALUE").default_value = floor
    _sock(remap, "To Max", "VALUE").default_value = 1.0
    nt.links.new(ao.outputs["AO"], _sock(remap, "Value", "VALUE"))
    return _sock(remap, "Result", "VALUE", out=True)


def _v(rec, key):
    return rec.get(key, SIGNAL_DEFAULTS[key])


def _remap(nt, value, lo, hi, out_lo, out_hi):
    node = nt.nodes.new("ShaderNodeMapRange")
    _sock(node, "From Min", "VALUE").default_value = lo
    _sock(node, "From Max", "VALUE").default_value = hi
    _sock(node, "To Min", "VALUE").default_value = out_lo
    _sock(node, "To Max", "VALUE").default_value = out_hi
    nt.links.new(value, _sock(node, "Value", "VALUE"))
    return _sock(node, "Result", "VALUE", out=True)


def _mul(nt, a, b):
    node = nt.nodes.new("ShaderNodeMath")
    node.operation = "MULTIPLY"
    for idx, val in ((0, a), (1, b)):
        if hasattr(val, "node"):
            nt.links.new(val, node.inputs[idx])
        else:
            node.inputs[idx].default_value = val
    return node.outputs["Value"]


def _signals(nt, rec, want_ao):
    """Les signaux procéduraux partagés par les trois passes.

    Phase 1 ajoute trois lectures que je n'exploitais pas :
      - `cavity`  : pointiness CONCAVE (je ne lisais que le côté convexe)
      - `height`  : dégradé vertical, pour PEINDRE la lumière dans l'albédo
      - `up`      : orientation au ciel, pour l'encrassement directionnel
    Plus un second bruit, plus fin, réservé à la rugosité.
    """
    coord = nt.nodes.new("ShaderNodeTexCoord")
    geom = nt.nodes.new("ShaderNodeNewGeometry")

    noise = nt.nodes.new("ShaderNodeTexNoise")
    noise.inputs["Scale"].default_value = rec["scale"]
    noise.inputs["Detail"].default_value = rec["detail"]
    nt.links.new(coord.outputs["Object"], noise.inputs["Vector"])

    fine = nt.nodes.new("ShaderNodeTexNoise")
    fine.inputs["Scale"].default_value = rec["scale"] * DETAIL_NOISE_RATIO
    fine.inputs["Detail"].default_value = 2.0
    nt.links.new(coord.outputs["Object"], fine.inputs["Vector"])

    ao_out = None
    if want_ao:
        ao_out = _mul(
            nt,
            _occlusion(nt, AO_DISTANCE, rec["ao"]),
            _occlusion(nt, AO_CONTACT, AO_CONTACT_FLOOR),
        )

    convex = _remap(nt, geom.outputs["Pointiness"], 0.495, 0.62, 0.0, 1.0)
    cavity = _remap(nt, geom.outputs["Pointiness"], *CAVITY_RANGE, _v(rec, "cav"), 1.0)

    # dégradé de lumière peinte : les objets sont construits en coordonnées
    # monde, donc l'axe Z objet EST la hauteur du personnage
    sep_pos = nt.nodes.new("ShaderNodeSeparateXYZ")
    nt.links.new(coord.outputs["Object"], sep_pos.inputs["Vector"])
    # Dégradé ASYMÉTRIQUE : on éclaircit franchement le haut, on assombrit à
    # peine le bas. Symétrique, il empile son assombrissement sur l'écrasement
    # des basses lumières du post-process toon et les jambes tombent dans le noir.
    lit = _v(rec, "lit")
    height = _remap(nt, sep_pos.outputs["Z"], 0.0, CHAR_HEIGHT, 1.0 - lit * 0.45, 1.0 + lit)

    sep_nrm = nt.nodes.new("ShaderNodeSeparateXYZ")
    nt.links.new(geom.outputs["Normal"], sep_nrm.inputs["Vector"])
    up = _remap(nt, sep_nrm.outputs["Z"], *UP_RANGE, 0.0, 1.0)

    return {
        "noise": noise.outputs["Fac"],
        "fine": fine.outputs["Fac"],
        "ao": ao_out,
        "convex": convex,
        "cavity": cavity,
        "height": height,
        "up": up,
    }


def _reset(mat):
    nt = mat.node_tree
    nt.nodes.clear()
    return nt


def _emit_out(nt, color_socket):
    emit = nt.nodes.new("ShaderNodeEmission")
    emit.inputs["Strength"].default_value = 1.0
    nt.links.new(color_socket, emit.inputs["Color"])
    out = nt.nodes.new("ShaderNodeOutputMaterial")
    nt.links.new(emit.outputs["Emission"], out.inputs["Surface"])


# ------------------------------------------------------------------- 3 passes


def _mix_rgb(nt, factor, color_a, color_b):
    node = nt.nodes.new("ShaderNodeMix")
    node.data_type = "RGBA"
    nt.links.new(factor, _sock(node, "Factor", "VALUE"))
    for name, value in (("A", color_a), ("B", color_b)):
        if hasattr(value, "node"):
            nt.links.new(value, _sock(node, name, "RGBA"))
        else:
            _sock(node, name, "RGBA").default_value = value
    return _sock(node, "Result", "RGBA", out=True)


def _pass_basecolor(mat, palette_entry):
    rec = _recipe(mat)
    rgba, _, _ = palette_entry
    nt = _reset(mat)
    sig = _signals(nt, rec, want_ao=True)

    # UNE seule chaîne scalaire d'ombrage, appliquée ensuite à la couleur :
    # occlusion × cavité × lumière peinte × éclat d'arête × grain.
    shade = _mul(nt, sig["ao"], sig["cavity"])
    shade = _mul(nt, shade, sig["height"])

    # « painted bevel » : les arêtes convexes s'ÉCLAIRCISSENT (1 + edge·convex).
    # C'est un effet de lumière, distinct de l'usure qui, elle, change la teinte.
    edge = nt.nodes.new("ShaderNodeMath")
    edge.operation = "MULTIPLY_ADD"
    nt.links.new(sig["convex"], edge.inputs[0])
    edge.inputs[1].default_value = _v(rec, "edge")
    edge.inputs[2].default_value = 1.0
    shade = _mul(nt, shade, edge.outputs["Value"])
    shade = _mul(nt, shade, _remap(nt, sig["noise"], 0.0, 1.0, 1.0 - rec["grain"], 1.0 + rec["grain"]))

    shaded = nt.nodes.new("ShaderNodeVectorMath")
    shaded.operation = "MULTIPLY"
    shaded.inputs[0].default_value = rgba[:3]
    nt.links.new(shade, shaded.inputs[1])  # float -> vector : diffusion implicite

    worn = _mix_rgb(
        nt, _mul(nt, sig["convex"], rec["wear"]),
        shaded.outputs["Vector"], (*rec["wear_color"], 1.0),
    )
    # encrassement DIRECTIONNEL : la poussière se dépose sur ce qui regarde le ciel
    dusted = _mix_rgb(
        nt, _mul(nt, sig["up"], _v(rec, "dust")),
        worn, (*_v(rec, "dust_color"), 1.0),
    )
    _emit_out(nt, dusted)


def _pass_orm(mat, palette_entry):
    rec = _recipe(mat)
    _, metallic, roughness = palette_entry
    nt = _reset(mat)
    sig = _signals(nt, rec, want_ao=False)

    # Le détail fin ne vit QUE dans la rugosité (bruit `fine`, jamais utilisé par
    # l'albédo) : lisible de loin, il ne se révèle qu'au spéculaire de près.
    rough = _remap(
        nt, sig["fine"], 0.0, 1.0,
        max(0.04, roughness - rec["rvar"]), min(1.0, roughness + rec["rvar"]),
    )
    worn_rough = nt.nodes.new("ShaderNodeMath")
    worn_rough.operation = "MULTIPLY_ADD"
    worn_rough.inputs[1].default_value = rec["wear"] * WEAR_ROUGHEN
    worn_rough.use_clamp = True
    nt.links.new(sig["convex"], worn_rough.inputs[0])
    nt.links.new(rough, worn_rough.inputs[2])

    pack = nt.nodes.new("ShaderNodeCombineXYZ")  # X=0, Y=rugosité, Z=métallicité
    pack.inputs["X"].default_value = 0.0
    pack.inputs["Z"].default_value = metallic
    nt.links.new(worn_rough.outputs["Value"], pack.inputs["Y"])
    _emit_out(nt, pack.outputs["Vector"])


def _pass_normal(mat, _palette_entry):
    rec = _recipe(mat)
    nt = _reset(mat)
    sig = _signals(nt, rec, want_ao=False)

    bump = nt.nodes.new("ShaderNodeBump")
    bump.inputs["Strength"].default_value = rec["bump"]
    bump.inputs["Distance"].default_value = BUMP_DISTANCE
    nt.links.new(sig["noise"], bump.inputs["Height"])

    diffuse = nt.nodes.new("ShaderNodeBsdfDiffuse")
    nt.links.new(bump.outputs["Normal"], diffuse.inputs["Normal"])
    out = nt.nodes.new("ShaderNodeOutputMaterial")
    nt.links.new(diffuse.outputs["BSDF"], out.inputs["Surface"])


PASSES = (
    # nom          builder          type de bake  échantillons  couleur de fond   data
    ("basecolor", _pass_basecolor, "EMIT", 48, (0.0, 0.0, 0.0, 1.0), False),
    ("orm", _pass_orm, "EMIT", 4, (0.0, 0.5, 0.0, 1.0), True),
    ("normal", _pass_normal, "NORMAL", 4, (0.5, 0.5, 1.0, 1.0), True),
)


# --------------------------------------------------------------- orchestration


def _select(objs, active=None):
    for obj in bpy.data.objects:
        obj.select_set(False)
    for obj in objs:
        obj.select_set(True)
    bpy.context.view_layer.objects.active = active or objs[0]


def unwrap(objs):
    """Atlas UV PARTAGÉ : le dépliage multi-objets range toutes les îles dans le
    même 0..1, ce qui permet ensuite une seule texture pour tout le groupe."""
    for obj in objs:
        if not obj.data.uv_layers:
            obj.data.uv_layers.new(name="UVMap")
    _select(objs)
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.uv.smart_project(angle_limit=radians(66.0), island_margin=0.012)
    bpy.ops.object.mode_set(mode="OBJECT")


def bake_group(objs, occluders, out_dir, group, palette, emissive, res):
    """Déplie, cuit les 3 cartes, puis fusionne les slots en un matériau PBR.

    `occluders` = objets visibles aux rayons d'occlusion en plus du groupe. On
    NE met pas l'armure quand on cuit le corps : sinon l'ombre du plastron
    resterait imprimée sur le torse nu.
    """
    tex_dir = out_dir / "textures"
    tex_dir.mkdir(parents=True, exist_ok=True)
    unwrap(objs)

    for obj in bpy.data.objects:
        if obj.type == "MESH":
            obj.hide_render = obj not in objs and obj not in occluders

    slots = []  # (matériau, entrée de palette) uniques du groupe, émissifs inclus
    for obj in objs:
        for slot in obj.material_slots:
            key = slot.material.name.removeprefix("dwarf_")
            if all(slot.material is not m for m, _ in slots):
                slots.append((slot.material, palette[key]))

    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    scene.cycles.device = "CPU"
    scene.render.bake.margin = 8
    scene.render.bake.use_clear = False

    images = {}
    for name, builder, bake_type, samples, fill, is_data in PASSES:
        image = bpy.data.images.new(f"{group}_{name}", res, res, alpha=False, is_data=is_data)
        image.generated_color = fill
        if is_data:
            image.colorspace_settings.name = "Non-Color"
        images[name] = image

        for mat, entry in slots:
            key = mat.name.removeprefix("dwarf_")
            if key in emissive:
                # exclu de la fusion (il garde son glow) mais le bake EXIGE une
                # cible image active dans CHAQUE matériau de l'objet
                node = mat.node_tree.nodes.new("ShaderNodeTexImage")
            else:
                builder(mat, entry)
                node = mat.node_tree.nodes.new("ShaderNodeTexImage")
            node.image = image
            node.name = "__bake_target"
            mat.node_tree.nodes.active = node

        scene.cycles.samples = samples
        _select(objs)
        bpy.ops.object.bake(type=bake_type, use_clear=False)

        for mat, _ in slots:
            node = mat.node_tree.nodes.get("__bake_target")
            if node:
                mat.node_tree.nodes.remove(node)

        path = tex_dir / f"{group}_{name}.png"
        image.filepath_raw = str(path)
        image.file_format = "PNG"
        image.save()

    baked = _final_material(f"dwarf_{group}_baked", images)
    for obj in objs:
        _reassign(obj, baked, emissive)

    for obj in bpy.data.objects:
        obj.hide_render = False
    return images


def _final_material(name, images):
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    nt = mat.node_tree
    nt.nodes.clear()
    out = nt.nodes.new("ShaderNodeOutputMaterial")
    bsdf = nt.nodes.new("ShaderNodeBsdfPrincipled")
    nt.links.new(bsdf.outputs["BSDF"], out.inputs["Surface"])

    tex_bc = nt.nodes.new("ShaderNodeTexImage")
    tex_bc.image = images["basecolor"]
    nt.links.new(tex_bc.outputs["Color"], bsdf.inputs["Base Color"])

    # rugosité et métallicité tirées de la MÊME image via Separate Color : c'est
    # le motif que l'exporteur glTF reconnaît pour sortir un metallicRoughness
    tex_orm = nt.nodes.new("ShaderNodeTexImage")
    tex_orm.image = images["orm"]
    sep = nt.nodes.new("ShaderNodeSeparateColor")
    nt.links.new(tex_orm.outputs["Color"], sep.inputs["Color"])
    nt.links.new(sep.outputs["Green"], bsdf.inputs["Roughness"])
    nt.links.new(sep.outputs["Blue"], bsdf.inputs["Metallic"])

    tex_n = nt.nodes.new("ShaderNodeTexImage")
    tex_n.image = images["normal"]
    nmap = nt.nodes.new("ShaderNodeNormalMap")
    nt.links.new(tex_n.outputs["Color"], nmap.inputs["Color"])
    nt.links.new(nmap.outputs["Normal"], bsdf.inputs["Normal"])
    return mat


def _reassign(obj, baked, emissive):
    """Slot 0 = matériau cuit, slots suivants = émissifs préservés."""
    old_names = [s.material.name.removeprefix("dwarf_") if s.material else "" for s in obj.material_slots]
    old_index = [poly.material_index for poly in obj.data.polygons]

    kept = []
    for name in old_names:
        if name in emissive and name not in kept:
            kept.append(name)

    obj.data.materials.clear()
    obj.data.materials.append(baked)
    for name in kept:
        obj.data.materials.append(bpy.data.materials[f"dwarf_{name}"])

    remap = {}
    for i, name in enumerate(old_names):
        remap[i] = 1 + kept.index(name) if name in kept else 0
    for poly, idx in zip(obj.data.polygons, old_index):
        poly.material_index = remap.get(idx, 0)
