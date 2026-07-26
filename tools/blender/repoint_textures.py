"""Branche de VRAIES textures pierre sur les materiaux muraille/aqueduc (au lieu
d'une couleur plate). Ajoute l'image+texture dans chaque .gltf si absente, pointe
baseColorTexture dessus, facteur -> blanc, metallic 0. Backup .bak deja fait."""
import json, glob, shutil
from pathlib import Path

CELLS = Path("assets/models/environment/castle/castle_stream_cells_grass")
# materiau -> texture BC reelle a utiliser
MAP = {
    "M_MOD_wall_castle": "textures/T_MOD_wall_bricks_castle_01_BC.png",
    "M_MOD_wall_curved_castle": "textures/T_MOD_wall_bricks_castle_01_BC.png",
    "M_MOD_wall_gate_castle": "textures/T_MOD_wall_bricks_castle_01_BC.png",
    "M_MOD_floor_castle_04": "textures/T_MOD_floor_castle_01_BC.png",
}

def ensure_image(g, uri):
    g.setdefault("images", [])
    for i, im in enumerate(g["images"]):
        if im.get("uri") == uri:
            return i
    g["images"].append({"uri": uri, "name": Path(uri).stem})
    return len(g["images"]) - 1

def ensure_sampler(g):
    g.setdefault("samplers", [])
    if not g["samplers"]:
        g["samplers"].append({"magFilter": 9729, "minFilter": 9987, "wrapS": 10497, "wrapT": 10497})
    return 0

def ensure_texture(g, img_idx):
    g.setdefault("textures", [])
    for i, t in enumerate(g["textures"]):
        if t.get("source") == img_idx:
            return i
    g["textures"].append({"sampler": ensure_sampler(g), "source": img_idx})
    return len(g["textures"]) - 1

n = cells = 0
for gp in sorted(glob.glob(str(CELLS / "*.gltf"))):
    g = json.loads(Path(gp).read_text(encoding="utf-8"))
    ch = False
    for m in g.get("materials", []):
        tgt = MAP.get(m.get("name"))
        if not tgt:
            continue
        img = ensure_image(g, tgt)
        tex = ensure_texture(g, img)
        pbr = m.setdefault("pbrMetallicRoughness", {})
        pbr["baseColorTexture"] = {"index": tex}
        pbr["baseColorFactor"] = [1, 1, 1, 1]
        pbr["metallicFactor"] = 0.0
        n += 1
        ch = True
    if ch:
        cells += 1
        bak = gp + ".bak"
        if not Path(bak).exists():
            shutil.copy2(gp, bak)
        Path(gp).write_text(json.dumps(g, separators=(",", ":")), encoding="utf-8")
print(f"re-pointe: {n} materiaux (vraie texture pierre) sur {cells} cellules")
