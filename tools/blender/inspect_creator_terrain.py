"""Inspecte le .blend original du créateur pour comprendre son setup terrain :
objets, matériau du gazon (texture + échelle de tuilage via les nœuds Mapping),
couches UV, forme du mesh. Répond à « regarde comment le créateur a fait »."""

import bpy


def dump_material(mat):
    if not mat or not mat.use_nodes:
        print(f"    material {mat.name if mat else '?'}: pas de nodes")
        return
    for n in mat.node_tree.nodes:
        if n.type == "TEX_IMAGE":
            img = n.image
            print(f"    IMAGE: {img.name if img else '?'} {tuple(img.size) if img else ''} colorspace={img.colorspace_settings.name if img else ''}")
        if n.type == "MAPPING":
            sc = n.inputs.get("Scale")
            loc = n.inputs.get("Location")
            print(f"    MAPPING scale={tuple(round(v,3) for v in sc.default_value) if sc else '?'} (=tuilage)")
        if n.type == "TEX_COORD":
            print("    TEX_COORD present (UV/Generated/Object)")


def main():
    print("=== OBJETS ===")
    for o in bpy.data.objects:
        info = f"{o.type:8s} {o.name}"
        if o.type == "MESH":
            info += f"  verts={len(o.data.vertices)} polys={len(o.data.polygons)}"
            info += f"  uv_layers={[u.name for u in o.data.uv_layers]}"
            info += f"  mats={[m.name for m in o.data.materials if m]}"
        print(" ", info)

    print("\n=== MESHES ressemblant à un terrain/sol (gros, plats, 'terrain'/'ground'/'grass') ===")
    for o in bpy.data.objects:
        if o.type != "MESH":
            continue
        name = o.name.lower()
        big = len(o.data.vertices) > 2000
        keyworded = any(k in name for k in ("terrain", "ground", "grass", "sol", "land", "floor"))
        if big or keyworded:
            dims = o.dimensions
            print(f"  {o.name}: verts={len(o.data.vertices)} dims=({dims.x:.1f},{dims.y:.1f},{dims.z:.1f})")
            for m in o.data.materials:
                if m:
                    print(f"   material: {m.name}")
                    dump_material(m)

    print("\n=== LUMIÈRES ===")
    for o in bpy.data.objects:
        if o.type == "LIGHT":
            li = o.data
            print(f"  {o.name}: {li.type} energy={getattr(li,'energy','?')} color={tuple(round(c,2) for c in li.color)}")

    w = bpy.context.scene.world
    if w and w.use_nodes:
        for n in w.node_tree.nodes:
            if n.type == "BACKGROUND":
                col = n.inputs.get("Color")
                st = n.inputs.get("Strength")
                print(f"\n=== WORLD bg color={tuple(round(v,2) for v in col.default_value) if col else '?'} strength={st.default_value if st else '?'}")


if __name__ == "__main__":
    main()
