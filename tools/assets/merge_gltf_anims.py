"""Fusionne des clips d'animation glTF dans le GLB d'un personnage.

# Pourquoi pas Blender

Le projet a un pont Blender qui marche, mais l'import/export d'un GLB riggé y
coûte trois pièges déjà payés (`make_single_user` avant `join`, `keep_originals`
qui omet des images en silence, la ré-indexation des os). Ici on ne veut RIEN
transformer : on veut recopier des courbes d'animation dans un fichier dont le
squelette est déjà le bon. Une fusion glTF directe ne touche donc ni au maillage,
ni aux matériaux, ni aux images — elle ajoute des accesseurs et des animations.

# Le principe, et la seule chose qui peut mal tourner

Un clip glTF cible ses os **par index de nœud**. Ces index diffèrent d'un fichier
à l'autre. On remappe donc **par NOM** :

    canal.target.node (index dans le clip)  ->  nom  ->  index dans le corps

Un os présent dans le clip mais absent du corps est **signalé et ignoré** — le
laisser pointer sur un index arbitraire animerait le mauvais membre, en silence.
C'est le seul mode d'échec réel de cette opération, et il est nommé.

Usage :
    python tools/assets/merge_gltf_anims.py CORPS.glb ANIMS_DIR SORTIE.glb
"""

import json
import pathlib
import struct
import sys

GLB_MAGIC = b"glTF"
CHUNK_JSON = 0x4E4F534A
CHUNK_BIN = 0x004E4942


def lire_glb(chemin: pathlib.Path):
    """Rend (json, bin) d'un GLB, ou (json, b'') pour un .gltf + .bin externe."""
    if chemin.suffix.lower() == ".gltf":
        doc = json.loads(chemin.read_text(encoding="utf-8"))
        binaire = b""
        for buf in doc.get("buffers", []):
            uri = buf.get("uri")
            if uri and not uri.startswith("data:"):
                binaire += (chemin.parent / uri).read_bytes()
        return doc, binaire

    d = chemin.read_bytes()
    if d[:4] != GLB_MAGIC:
        raise ValueError(f"{chemin.name} n'est pas un GLB")
    doc, binaire = None, b""
    off = 12
    while off < len(d):
        (clen, ctype) = struct.unpack_from("<II", d, off)
        off += 8
        bloc = d[off : off + clen]
        if ctype == CHUNK_JSON:
            doc = json.loads(bloc.decode("utf-8"))
        elif ctype == CHUNK_BIN:
            binaire = bloc
        off += clen
    if doc is None:
        raise ValueError(f"{chemin.name} : pas de chunk JSON")
    return doc, binaire


def ecrire_glb(chemin: pathlib.Path, doc: dict, binaire: bytes):
    js = json.dumps(doc, separators=(",", ":")).encode("utf-8")
    js += b" " * ((4 - len(js) % 4) % 4)  # padding 4 octets, obligatoire
    bn = binaire + b"\x00" * ((4 - len(binaire) % 4) % 4)
    total = 12 + 8 + len(js) + (8 + len(bn) if bn else 0)
    out = bytearray()
    out += GLB_MAGIC + struct.pack("<II", 2, total)
    out += struct.pack("<II", len(js), CHUNK_JSON) + js
    if bn:
        out += struct.pack("<II", len(bn), CHUNK_BIN) + bn
    chemin.write_bytes(bytes(out))


def noms_des_noeuds(doc: dict) -> dict:
    """nom -> index. Un nom en double est un piège : on garde le premier et on
    le signale, parce que le remappage deviendrait ambigu."""
    table, doublons = {}, []
    for i, n in enumerate(doc.get("nodes", [])):
        nom = n.get("name")
        if not nom:
            continue
        if nom in table:
            doublons.append(nom)
        else:
            table[nom] = i
    if doublons:
        print(f"    ⚠ noms de noeuds en double, ignores : {sorted(set(doublons))[:5]}")
    return table


def fusionner(corps_path, anims_dir, sortie_path):
    corps_path = pathlib.Path(corps_path)
    anims_dir = pathlib.Path(anims_dir)
    sortie_path = pathlib.Path(sortie_path)

    doc, binaire = lire_glb(corps_path)
    cible = noms_des_noeuds(doc)
    print(f"CORPS {corps_path.name} : {len(doc.get('nodes', []))} noeuds, "
          f"{len(doc.get('animations', []))} animation(s) au depart")

    doc.setdefault("animations", [])
    doc.setdefault("accessors", [])
    doc.setdefault("bufferViews", [])
    doc.setdefault("buffers", [{"byteLength": 0}])
    binaire = bytearray(binaire)

    total_ok, total_ignores = 0, 0
    for src_path in sorted(anims_dir.glob("*.glb")):
        src, src_bin = lire_glb(src_path)
        src_noms = {i: n.get("name") for i, n in enumerate(src.get("nodes", []))}
        if not src.get("animations"):
            print(f"  {src_path.name:22} AUCUNE animation — ignore")
            continue

        # Décalages : tout ce qu'on copie est renuméroté à la suite de l'existant.
        off_bv = len(doc["bufferViews"])
        off_acc = len(doc["accessors"])
        base_octets = len(binaire)
        if base_octets % 4:
            binaire += b"\x00" * (4 - base_octets % 4)
            base_octets = len(binaire)

        # 1. Les données brutes.
        binaire += src_bin

        # 2. Les vues, décalées dans le buffer fusionné.
        for bv in src.get("bufferViews", []):
            neuf = dict(bv)
            neuf["buffer"] = 0
            neuf["byteOffset"] = bv.get("byteOffset", 0) + base_octets
            doc["bufferViews"].append(neuf)

        # 3. Les accesseurs, pointant vers les vues décalées.
        for acc in src.get("accessors", []):
            neuf = dict(acc)
            if "bufferView" in neuf:
                neuf["bufferView"] += off_bv
            doc["accessors"].append(neuf)

        # 4. Les animations, avec remappage des cibles PAR NOM.
        for anim in src["animations"]:
            canaux, manquants = [], set()
            for ch in anim.get("channels", []):
                idx_src = ch.get("target", {}).get("node")
                nom = src_noms.get(idx_src)
                if nom is None or nom not in cible:
                    manquants.add(nom or f"#{idx_src}")
                    continue
                neuf = {
                    "sampler": ch["sampler"],
                    "target": {"node": cible[nom], "path": ch["target"]["path"]},
                }
                canaux.append(neuf)
            if not canaux:
                print(f"  {src_path.name:22} AUCUN os en commun — clip ignore")
                total_ignores += 1
                continue
            samplers = []
            for s in anim.get("samplers", []):
                samplers.append(
                    {
                        "input": s["input"] + off_acc,
                        "output": s["output"] + off_acc,
                        "interpolation": s.get("interpolation", "LINEAR"),
                    }
                )
            # Le nom du CLIP est celui du FICHIER : les clips exportes par Mixamo
            # s'appellent tous « Armature|mixamo.com|Layer0 », donc ils se
            # masqueraient les uns les autres dans `named_animations`.
            doc["animations"].append(
                {"name": src_path.stem, "channels": canaux, "samplers": samplers}
            )
            total_ok += 1
            note = f"  ({len(manquants)} os absents du corps)" if manquants else ""
            print(f"  {src_path.name:22} -> clip « {src_path.stem} », "
                  f"{len(canaux)} canaux{note}")
            if manquants:
                print(f"      absents : {sorted(manquants)[:6]}")

    doc["buffers"] = [{"byteLength": len(binaire)}]
    sortie_path.parent.mkdir(parents=True, exist_ok=True)
    ecrire_glb(sortie_path, doc, bytes(binaire))
    print()
    print(f"SORTIE {sortie_path.name} : {total_ok} clips fusionnes, "
          f"{total_ignores} ignores, {len(bytes(binaire))/1048576:.1f} Mo")
    print(f"  clips : {[a['name'] for a in doc['animations']]}")


if __name__ == "__main__":
    if len(sys.argv) != 4:
        print(__doc__)
        sys.exit(1)
    fusionner(sys.argv[1], sys.argv[2], sys.argv[3])
