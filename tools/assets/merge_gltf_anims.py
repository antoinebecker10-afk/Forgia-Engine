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


RAPPORT = pathlib.Path("tools/assets/rapport_clips.json")

# Une foulée fait ~1,5 m ; au-delà de quelques mètres, la racine emmène le
# personnage hors de sa capsule.
COURSE_MAX_M = 3.0
# Un os d'un rig humain dépasse rarement la demi-longueur de cuisse. Au-delà de
# ce RATIO au corps cible, le clip vient d'un autre personnage.
RATIO_OS_MAX = 1.25


def controler_la_sortie(sortie, doc, binaire, corps_path):
    """Mesure le CORPS LIVRÉ et écrit `rapport_clips.json`. Rend le rapport.

    🚨 Le cœur du cliquet, et la leçon de la nuit du 2026-08-17→18.

    Sept défauts ont traversé ce pipeline en passant chacun devant un contrôle
    qui affichait vert. À chaque fois, le contrôle regardait une **entrée** —
    un des 25 GLB convertis — alors que le défaut vivait dans la **sortie**, le
    corps fusionné :

      · les 9 clips en double n'existaient que dans le fichier produit ;
      · `death` gardait 0,89 m de dérive parce qu'il n'a pas de `.fbx` source
        et ne passait donc par aucune mesure de la conversion ;
      · les proportions d'os ne se voient qu'en comparant le clip AU CORPS,
        ce que la conversion, qui ne connaît pas le corps, ne peut pas faire.

    D'où la règle : **on mesure ce qu'on livre.** Et le résultat n'est pas
    imprimé — il est écrit dans un fichier qu'on peut citer, comparer d'une
    passe à l'autre, et faire relire par un cliquet. Un `print()` ne se compare
    à rien.
    """
    ref = lire_glb(corps_path)[0]
    ref_os = {
        n.get("name"): i for i, n in enumerate(ref.get("nodes", [])) if n.get("name")
    }

    def echantillons(document, blob, acc_idx):
        acc = document["accessors"][acc_idx]
        bv = document["bufferViews"][acc["bufferView"]]
        base = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
        return [
            struct.unpack_from("<fff", blob, base + i * 12) for i in range(acc["count"])
        ]

    par_nom = {n.get("name"): i for i, n in enumerate(doc.get("nodes", [])) if n.get("name")}
    racines = {i for n, i in par_nom.items() if n == "Hips"}

    clips, defauts = [], []
    vus = {}
    for anim in doc.get("animations", []):
        nom = anim.get("name", "?")
        vus[nom] = vus.get(nom, 0) + 1
        derive, os_touches = 0.0, set()
        for ch in anim.get("channels", []):
            cible = ch.get("target", {})
            noeud = cible.get("node")
            os_touches.add(noeud)
            if cible.get("path") == "translation" and noeud in racines:
                v = echantillons(doc, binaire, anim["samplers"][ch["sampler"]]["output"])
                if len(v) >= 2:
                    derive = max(
                        derive, max(abs(v[-1][k] - v[0][k]) for k in range(3))
                    )
        clips.append(
            {
                "nom": nom,
                "canaux": len(anim.get("channels", [])),
                "os_touches": len(os_touches),
                "derive_racine_m": round(derive, 4),
            }
        )
        if derive > COURSE_MAX_M:
            defauts.append(f"{nom} : la racine derive de {derive:.1f} m")
        if not anim.get("channels"):
            defauts.append(f"{nom} : clip VIDE, aucun canal")

    doubles = sorted(n for n, k in vus.items() if k > 1)
    if doubles:
        defauts.append(
            f"noms de clips en double dans le fichier livre : {doubles} — "
            f"au chargement, un seul survivra et l'autre sera invisible"
        )

    # Les os que le corps a et qu'AUCUN clip n'anime. Mesuré à l'envers de ce que
    # faisait le contrôle d'origine, qui listait les os du clip absents du corps —
    # l'autre moitié, celle qui ne dit rien des cheveux qui ne bougent plus.
    animes = {c["target"]["node"] for a in doc.get("animations", []) for c in a.get("channels", [])}
    jamais = sorted(n for n, i in par_nom.items() if i not in animes and n in ref_os)

    rapport = {
        "id": "clips_livres",
        "fichier": str(sortie),
        "clips": len(doc.get("animations", [])),
        "noms_distincts": len(vus),
        "os_du_corps": len(ref_os),
        "os_jamais_animes": jamais[:20],
        "pire_derive_m": round(max((c["derive_racine_m"] for c in clips), default=0.0), 4),
        "defauts": defauts,
        "detail": clips,
    }
    rapport["severity"] = "critical" if defauts else "ok"
    rapport["next_step"] = " · ".join(defauts) if defauts else ""
    RAPPORT.parent.mkdir(parents=True, exist_ok=True)
    RAPPORT.write_text(json.dumps(rapport, indent=1, ensure_ascii=False), encoding="utf-8")
    return rapport


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

    # 🚨 Les noms de clips DÉJÀ pris par le corps.
    #
    # Le contrôle existait pour les nœuds (`noms_des_noeuds`) et manquait pour
    # les clips, alors que la conséquence y est pire. Mesuré le 2026-08-17 : le
    # corps livré portait 43 animations pour 34 noms distincts — `idle`, `walk`,
    # `run`, `jump`, `death`, `swim`, `jog_backward`, `hit_react` et
    # `hammer_strike` en double. Le chargeur glTF de Bevy construit
    # `named_animations` par `insert` : le DERNIER gagne, sans un mot. Le génome
    # demandait `jump` et n'obtenait pas celui du corps.
    #
    # Et la fusion annonçait « 34 clips fusionnés, 0 ignoré » — un compte juste
    # sur ce qu'elle avait ajouté, faux sur ce qu'elle avait produit.
    noms_de_clips = {a.get("name") for a in doc["animations"] if a.get("name")}

    total_ok, total_ignores, total_doublons = 0, 0, 0
    for src_path in sorted(anims_dir.glob("*.glb")):
        src, src_bin = lire_glb(src_path)
        src_noms = {i: n.get("name") for i, n in enumerate(src.get("nodes", []))}
        if not src.get("animations"):
            print(f"  {src_path.name:22} AUCUNE animation — ignore")
            continue
        # Refusé AVANT de recopier quoi que ce soit : un clip masqué ferait
        # grossir le fichier de données que rien ne lira jamais.
        if src_path.stem in noms_de_clips:
            print(f"  {src_path.name:22} ⚠ le corps a DEJA un clip « "
                  f"{src_path.stem} » — refuse (le dernier masquerait l'autre "
                  f"en silence au chargement)")
            total_doublons += 1
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
            noms_de_clips.add(src_path.stem)
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
    rapport = controler_la_sortie(sortie_path, doc, binaire, corps_path)

    # Le compte porte sur le fichier PRODUIT, pas sur ce que la boucle a ajouté.
    # C'est la différence entre les deux qui avait échappé : « 34 fusionnés »
    # était vrai, et le fichier en contenait 43.
    noms = [a.get("name", "?") for a in doc["animations"]]
    distincts = sorted(set(noms))
    print(f"SORTIE {sortie_path.name} : {len(noms)} animations pour "
          f"{len(distincts)} noms distincts, {len(bytes(binaire))/1048576:.1f} Mo")
    print(f"  ajoutees {total_ok} · refusees en doublon {total_doublons} · "
          f"sans os commun {total_ignores}")
    if len(noms) != len(distincts):
        restants = sorted({n for n in noms if noms.count(n) > 1})
        print(f"  ⚠ DOUBLONS RESTANTS dans le corps de depart : {restants} — "
              f"au chargement, un seul survivra et l'autre sera invisible")
    print(f"  clips : {distincts}")
    # Le contrôle DIT ce qu'il a mesuré, réussite comprise — sinon on ne peut
    # pas distinguer « rien à signaler » de « rien n'a été regardé ».
    print(f"\nCONTROLE DE LA SORTIE -> {RAPPORT}")
    print(f"  {rapport['clips']} clip(s) mesure(s) · {rapport['os_du_corps']} os au corps "
          f"· pire derive racine {rapport['pire_derive_m']:.2f} m")
    if rapport["os_jamais_animes"]:
        print(f"  os du corps qu'AUCUN clip n'anime : {rapport['os_jamais_animes']}")
    for d in rapport["defauts"]:
        print(f"  ⚠ {d}")
    if rapport["defauts"]:
        print("  -> NE PAS LIVRER ce corps en l'etat.")
        return 1
    print("  aucun defaut mesure.")
    return 0


if __name__ == "__main__":
    if len(sys.argv) != 4:
        print(__doc__)
        sys.exit(1)
    # Le code de sortie porte le verdict du contrôle : un pipeline qui rend 0
    # quand il vient de mesurer un défaut ne peut pas être branché à un cliquet.
    sys.exit(fusionner(sys.argv[1], sys.argv[2], sys.argv[3]))
