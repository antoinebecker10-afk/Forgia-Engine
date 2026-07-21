"""Inspecte un GLB : objets, meshes (polycount), armatures (bones), animations, matériaux."""
import bpy
import sys

argv = sys.argv[sys.argv.index("--") + 1 :]
path = argv[0]

# Scène vierge
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=path)

print("=== GLB INSPECT ===")
print(f"file: {path}")

meshes = [o for o in bpy.data.objects if o.type == "MESH"]
arms = [o for o in bpy.data.objects if o.type == "ARMATURE"]

print(f"\n-- OBJECTS ({len(bpy.data.objects)}) --")
for o in bpy.data.objects:
    dims = tuple(round(d, 3) for d in o.dimensions)
    print(f"  [{o.type}] {o.name}  dims={dims}")

print(f"\n-- MESHES ({len(meshes)}) --")
total_tris = 0
for o in meshes:
    m = o.data
    tris = sum(len(p.vertices) - 2 for p in m.polygons)
    total_tris += tris
    has_skin = any(mod.type == "ARMATURE" for mod in o.modifiers) or bool(o.vertex_groups)
    print(f"  {o.name}: {len(m.vertices)} verts, {tris} tris, uv={len(m.uv_layers)}, skinned={has_skin}, vgroups={len(o.vertex_groups)}")
print(f"  TOTAL tris: {total_tris}")

print(f"\n-- ARMATURES ({len(arms)}) --")
for o in arms:
    bones = o.data.bones
    print(f"  {o.name}: {len(bones)} bones")
    for b in list(bones)[:40]:
        print(f"    - {b.name}")

print(f"\n-- ANIMATIONS ({len(bpy.data.actions)}) --")
for a in bpy.data.actions:
    print(f"  {a.name}: frames {a.frame_range[0]:.0f}-{a.frame_range[1]:.0f}")

print(f"\n-- MATERIALS ({len(bpy.data.materials)}) --")
for mat in bpy.data.materials:
    print(f"  {mat.name}")

print(f"\n-- IMAGES ({len(bpy.data.images)}) --")
for img in bpy.data.images:
    print(f"  {img.name}: {img.size[0]}x{img.size[1]}")
print("=== END ===")
