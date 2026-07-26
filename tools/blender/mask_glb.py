"""Post-process GLB : force alphaMode=MASK (alphaCutoff=0.4) sur TOUS les materiaux.
Blender 5.0 exporte la vegetation en BLEND (transparent) -> 21k cartes double-face
transparentes = le renderer Bevy meurt. MASK = alpha-cutout opaque = OK."""
import json, struct, sys

path = sys.argv[1]
data = open(path, "rb").read()
magic, version, length = struct.unpack_from("<III", data, 0)
assert magic == 0x46546C67, "pas un GLB"
off = 12
chunks = []
while off < length:
    clen, ctype = struct.unpack_from("<II", data, off)
    body = data[off + 8: off + 8 + clen]
    chunks.append([ctype, bytearray(body)])
    off += 8 + clen

for ci, (ctype, body) in enumerate(chunks):
    if ctype == 0x4E4F534A:  # JSON
        gltf = json.loads(body.decode("utf-8"))
        n = 0
        for m in gltf.get("materials", []):
            m["alphaMode"] = "MASK"
            m["alphaCutoff"] = 0.4
            m["doubleSided"] = True
            n += 1
        newj = json.dumps(gltf, separators=(",", ":")).encode("utf-8")
        while len(newj) % 4 != 0:
            newj += b" "
        chunks[ci][1] = bytearray(newj)
        print(f"MASK applique a {n} materiaux")
        break

out = bytearray(struct.pack("<III", magic, version, 0))
for ctype, body in chunks:
    out += struct.pack("<II", len(body), ctype) + body
struct.pack_into("<I", out, 8, len(out))
open(path, "wb").write(out)
print(f"REWROTE {path} ({len(out)//1024} KB)")
