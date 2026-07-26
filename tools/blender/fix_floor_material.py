"""Corrige les materiaux des cellules dont la BASE COLOR pointe sur une texture _PBR
(bug d'import : pas de _BC pour floor_05 -> PBR verte/bleue rendue en albedo =
dalle bleue + aqueduc arc-en-ciel). Fix : retirer baseColorTexture, mettre une
couleur pierre + metallic 0. Dry-run par defaut ; --apply pour ecrire (backup .bak)."""
import json, glob, sys, shutil
from pathlib import Path

CELLS = Path("assets/models/environment/castle/castle_stream_cells_grass")
APPLY = "--apply" in sys.argv
STONE = [0.50, 0.47, 0.42, 1.0]

def img_uri(g, tex_index):
    try:
        src = g["textures"][tex_index]["source"]
        return g["images"][src].get("uri", g["images"][src].get("name", ""))
    except Exception:
        return ""

total_fix = 0
cells_touched = 0
names = set()
for gp in sorted(glob.glob(str(CELLS / "*.gltf"))):
    g = json.loads(Path(gp).read_text(encoding="utf-8"))
    changed = False
    for m in g.get("materials", []):
        pbr = m.get("pbrMetallicRoughness", {})
        bct = pbr.get("baseColorTexture")
        if not bct:
            continue
        uri = img_uri(g, bct["index"])
        if "PBR" in uri.upper() and "_BC" not in uri.upper():
            # BUG : base color = texture PBR
            names.add((m.get("name", "?"), uri))
            del pbr["baseColorTexture"]
            pbr["baseColorFactor"] = STONE
            pbr["metallicFactor"] = 0.0
            m["pbrMetallicRoughness"] = pbr
            total_fix += 1
            changed = True
    if changed:
        cells_touched += 1
        if APPLY:
            bak = gp + ".bak"
            if not Path(bak).exists():
                shutil.copy2(gp, bak)
            Path(gp).write_text(json.dumps(g, separators=(",", ":")), encoding="utf-8")

print(f"{'APPLIQUE' if APPLY else 'DRY-RUN'} : {total_fix} materiaux corriges dans {cells_touched} cellules")
print("materiaux (nom -> texture base actuelle) :")
for nm, uri in sorted(names):
    print(f"  {nm} -> {uri}")
