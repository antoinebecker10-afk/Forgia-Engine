"""Dump complet du matériau falaise du .blend créateur : quels textures (albedo,
normal), et surtout le montage de la PROJECTION HERBE PAR LE DESSUS (mix par
normale/Z, geometry normal, separate XYZ) — pour reproduire le look."""

import bpy


def dump_node(n, depth=0):
    pad = "  " * depth
    if n.type == "TEX_IMAGE":
        print(f"{pad}TEX_IMAGE: {n.image.name if n.image else '?'}")
    elif n.type == "MIX" or n.type == "MIX_RGB":
        print(f"{pad}MIX (fac driven par ->)")
    elif n.type in ("SEPARATE_XYZ", "NEW_GEOMETRY", "TEX_COORD", "NORMAL", "VECT_MATH", "MATH", "MAP_RANGE"):
        print(f"{pad}{n.type}")
    elif n.type == "BSDF_PRINCIPLED":
        print(f"{pad}PRINCIPLED")
    else:
        print(f"{pad}{n.type}")


def main():
    for mat in bpy.data.materials:
        if "cliff" not in mat.name.lower() and "ground" not in mat.name.lower():
            continue
        print(f"\n==== MATÉRIAU {mat.name} ====")
        if not mat.use_nodes:
            print("  pas de nodes"); continue
        nt = mat.node_tree
        print(f"  nodes: {[n.type for n in nt.nodes]}")
        print("  images:")
        for n in nt.nodes:
            if n.type == "TEX_IMAGE" and n.image:
                # trace : cette image alimente Base Color ? via un MIX ?
                outs = [l.to_node.type for l in nt.links if l.from_node == n]
                print(f"    {n.image.name}  -> alimente {outs}")
        # y a-t-il un mix piloté par la normale Z (top projection) ?
        for n in nt.nodes:
            if n.type in ("MIX", "MIX_RGB"):
                fac_src = [l.from_node.type for l in nt.links if l.to_node == n and l.to_socket.name in ("Fac", "Factor")]
                print(f"    MIX '{n.name}' fac piloté par: {fac_src}")
            if n.type in ("SEPARATE_XYZ", "NEW_GEOMETRY"):
                print(f"    >>> {n.type} présent = indice de PROJECTION PAR NORMALE (top projection)")


if __name__ == "__main__":
    main()
