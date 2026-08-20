#!/usr/bin/env python3
"""verifier_corps.py — controle les corps d'animation LIVRES, sans rien rebatir.

# Pourquoi ce fichier existe

Sept defauts ont traverse le pipeline d'animation dans la nuit du 2026-08-17 au
18. Chacun est passe devant un controle qui affichait vert, et chaque fois pour
la meme raison : **le controle regardait une entree, le defaut vivait dans la
sortie.**

  · les 9 clips en double n'existaient QUE dans le corps fusionne ;
  · `death` gardait 0,89 m de derive parce qu'il n'a pas de `.fbx` source et
    ne passait donc par aucune mesure de la conversion ;
  · le facteur cent venait des unites de Mixamo, et le controle de la longueur
    d'os est passe au vert pendant que la course de la racine restait en
    centimetres — deux grandeurs, une seule mesuree.

Ce script mesure le fichier qu'on livre au jeu. Il ne convertit rien, ne fusionne
rien, n'a besoin ni de Blender ni des sources FBX : il ouvre le GLB final et
regarde ce qu'il contient reellement. C'est ce qui le rend executable en CI.

# Ce qu'il ne fait pas

Il ne juge pas la QUALITE d'une animation — ni sa duree, ni son allure, ni si
elle correspond au geste attendu. Il attrape des defauts de structure, qui sont
ceux qui coutent le plus cher parce qu'ils ne se voient pas a la lecture du code.

Usage
-----
    python tools/assets/verifier_corps.py                  # tous les corps connus
    python tools/assets/verifier_corps.py chemin/vers.glb   # un fichier precis
"""

from __future__ import annotations

import json
import pathlib
import struct
import sys

# Une foulee fait ~1,5 m. Au-dela, la racine emmene le personnage hors de sa
# capsule : c'est le « je ne me vois plus nulle part quand je saute » du 17/08.
COURSE_MAX_M = 3.0

# Le plus long os d'un squelette humain est la cuisse, ~0,45 m. Deux metres
# laissent la marge d'un personnage plus grand sans jamais atteindre les
# dizaines que produit un fichier reste en centimetres.
OS_MAX_M = 2.0

# L'os qui porte le deplacement d'ensemble. Le SEUL dont la translation ait le
# droit de varier au cours d'un clip.
OS_RACINE = "Hips"

# Ecart tolere entre la translation animee d'un os et sa translation au repos.
# 1 mm : une difference plus grande n'est plus du bruit de quantification, c'est
# une longueur d'os que le clip impose.
TOLERANCE_OS_M = 0.001

# Les corps que le jeu charge vraiment. Une liste explicite plutot qu'un glob :
# le dossier contient aussi des bases de travail et des variantes, qu'on ne
# livre pas et dont les defauts ne bloquent personne.
CORPS_LIVRES = [
    "assets/models/characters/stylized/stylized_male_fusil.glb",
]


def lire_glb(chemin: pathlib.Path) -> tuple[dict, bytes]:
    d = chemin.read_bytes()
    if d[:4] != b"glTF":
        raise ValueError(f"{chemin} n'est pas un GLB")
    lg_json, = struct.unpack_from("<I", d, 12)
    doc = json.loads(d[20 : 20 + lg_json].decode("utf-8"))
    dep = 20 + lg_json
    lg_bin, = struct.unpack_from("<I", d, dep)
    return doc, d[dep + 8 : dep + 8 + lg_bin]


def vec3(doc: dict, blob: bytes, acc_idx: int) -> list[tuple[float, float, float]]:
    acc = doc["accessors"][acc_idx]
    bv = doc["bufferViews"][acc["bufferView"]]
    base = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
    return [struct.unpack_from("<fff", blob, base + i * 12) for i in range(acc["count"])]


def verifier(chemin: pathlib.Path) -> list[str]:
    """Rend la liste des defauts. Liste vide = rien de mesure d'anormal."""
    doc, blob = lire_glb(chemin)
    defauts: list[str] = []

    noeuds = {i: n.get("name") for i, n in enumerate(doc.get("nodes", []))}
    racines = {i for i, n in noeuds.items() if n == "Hips"}

    # 1. Les homonymes. Le chargeur glTF de Bevy construit `named_animations`
    #    par `insert` : le dernier gagne, sans un mot. Un genome qui demande
    #    « jump » obtient alors un clip qu'il n'a pas choisi.
    noms = [a.get("name", "?") for a in doc.get("animations", [])]
    doubles = sorted({n for n in noms if noms.count(n) > 1})
    if doubles:
        defauts.append(
            f"{len(noms)} animations pour {len(set(noms))} noms — homonymes {doubles} : "
            f"au chargement un seul survit, l'autre est invisible"
        )

    # 2. La derive de racine, clip par clip.
    for anim in doc.get("animations", []):
        nom = anim.get("name", "?")
        if not anim.get("channels"):
            defauts.append(f"« {nom} » : clip VIDE, aucun canal")
            continue
        for ch in anim["channels"]:
            c = ch.get("target", {})
            if c.get("path") != "translation" or c.get("node") not in racines:
                continue
            v = vec3(doc, blob, anim["samplers"][ch["sampler"]]["output"])
            if len(v) < 2:
                continue
            derive = max(abs(v[-1][k] - v[0][k]) for k in range(3))
            if derive > COURSE_MAX_M:
                defauts.append(
                    f"« {nom} » : la racine derive de {derive:.1f} m — le personnage "
                    f"decolle de sa capsule (le controleur le deplace deja)"
                )

    # 3. L'echelle des os — deux causes possibles, qu'il faut SEPARER.
    #
    # La MEDIANE distingue : elle bouge si TOUT le squelette est a la mauvaise
    # echelle (unites), elle ne bouge pas si quelques os sont egares (rig).
    # Confondre les deux envoie chercher une unite dans un convertisseur quand
    # le defaut est un rig — un capteur qui accuse le mauvais coupable coute
    # plus cher qu'un capteur muet.
    #
    # 🚨 La longueur d'un os est sa translation MULTIPLIEE par l'echelle de ses
    # parents. Lire la translation brute est faux, et le 2026-08-18 ca m'a fait
    # annoncer un rig casse sur un rig sain.
    #
    # Mesure : `root.001`, le parent des six os de cape, porte une echelle de
    # 0,01. Les translations de ses enfants sont donc ecrites dans un espace
    # x100 — `cloak_01` a 139,483 mesure 1,395 m en vrai, soit exactement la
    # retombee d'une cape depuis l'epaule. Le brut criait au geant ; le reel ne
    # depasse 2 m nulle part.
    #
    # C'est la meme faute que ce fichier existe pour attraper, commise dans son
    # propre code : une metrique dont la PORTEE est fausse, enoncee avec
    # assurance. Une grandeur ne se lit jamais hors du repere ou elle vit.
    parent = {}
    for i, n in enumerate(doc.get("nodes", [])):
        for c in n.get("children", []):
            parent[c] = i

    def echelle_des_parents(i: int) -> float:
        e, j = 1.0, parent.get(i)
        while j is not None:
            s = doc["nodes"][j].get("scale", [1.0, 1.0, 1.0])
            e *= (abs(s[0]) * abs(s[1]) * abs(s[2])) ** (1 / 3)
            j = parent.get(j)
        return e

    longueurs = []
    for i, n in enumerate(doc.get("nodes", [])):
        t = n.get("translation")
        if t:
            brut = (t[0] ** 2 + t[1] ** 2 + t[2] ** 2) ** 0.5
            longueurs.append((brut * echelle_des_parents(i), n.get("name", "?")))
    if longueurs:
        tri = sorted(v for v, _ in longueurs)
        mediane = tri[len(tri) // 2]
        egares = sorted((v, n) for v, n in longueurs if v > OS_MAX_M)
        if mediane > OS_MAX_M:
            defauts.append(
                f"mediane des os a {mediane:.1f} m — TOUT le squelette est a la "
                f"mauvaise echelle (unites en centimetres ?), le personnage sera un geant"
            )
        elif egares:
            noms = ", ".join(f"{n} ({v:.1f} m)" for v, n in egares[-6:])
            defauts.append(
                f"{len(egares)} os a plus de {OS_MAX_M:.0f} m de leur parent, echelle "
                f"des parents comprise, alors que la mediane est saine ({mediane:.2f} m) "
                f"— rig casse, pas un probleme d'unites : {noms}"
            )

    # 4. Un clip ne doit JAMAIS changer la longueur d'un os.
    #
    # 🚨 Le defaut le plus couteux du 2026-08-18, et il etait invisible a la
    # lecture : les 25 clips Mixamo portaient une translation sur CHAQUE os.
    #
    # Un clip capture declare, image par image, OU se trouve chaque os. Recopier
    # ces positions sur un autre squelette lui impose les proportions du premier.
    # Mesure dans Blender sur le corps livre : l'os `Head` etirait de **63 %** —
    # le rig Vanguard n'a pas la tete d'un personnage stylise. Le personnage
    # etait visiblement deforme, et aucun controle ne le disait.
    #
    # La regle du metier : on transfere les ROTATIONS, jamais les translations,
    # sauf la racine dont la translation est un DEPLACEMENT et non une
    # proportion. Ici on verifie le resultat : toute translation d'os non-racine
    # qui s'ecarte de sa valeur au repos etire cet os.
    rest = {
        i: n.get("translation", [0.0, 0.0, 0.0]) for i, n in enumerate(doc.get("nodes", []))
    }
    racines = {i for i, n in noeuds.items() if n == OS_RACINE}
    etires = {}
    for anim in doc.get("animations", []):
        for ch in anim.get("channels", []):
            c = ch.get("target", {})
            n_idx = c.get("node")
            if c.get("path") != "translation" or n_idx in racines:
                continue
            v = vec3(doc, blob, anim["samplers"][ch["sampler"]]["output"])
            r = rest.get(n_idx, [0.0, 0.0, 0.0])
            pire = max(
                (max(abs(p[k] - r[k]) for k in range(3)) for p in v), default=0.0
            )
            if pire > TOLERANCE_OS_M:
                cle = anim.get("name", "?")
                garde = etires.get(cle)
                if garde is None or pire > garde[1]:
                    etires[cle] = (noeuds.get(n_idx, "?"), pire)
    if etires:
        pire_clip = max(etires.items(), key=lambda kv: kv[1][1])
        defauts.append(
            f"{len(etires)} clip(s) imposent une longueur d'os : pire = "
            f"« {pire_clip[0]} » sur l'os {pire_clip[1][0]} ({pire_clip[1][1] * 100:.1f} cm "
            f"d'ecart au repos) — retargeting qui copie les TRANSLATIONS au lieu "
            f"des seules rotations, le personnage sera deforme"
        )

    return defauts


# Les defauts deja connus au moment ou ce controle a ete ecrit. Meme dispositif
# que `xtask plugin-gate` et `xtask deps-mortes` : on interdit les NOUVEAUX, on
# publie les anciens. Un controle qui echoue sur tout l'existant se desactive
# dans la semaine, et laisse le projet sans controle du tout.
BASELINE = pathlib.Path("docs/audit/corps-anim-baseline.txt")


def cle(corps: pathlib.Path, defaut: str) -> str:
    """Un defaut se reconnait a son corps et a ses PREMIERS mots — pas a sa
    phrase entiere, dont les nombres bougent d'une passe a l'autre."""
    return f"{corps.name}|{' '.join(defaut.split()[:4])}"


def main() -> int:
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    ecrire = "--ecrire-baseline" in sys.argv
    cibles = [pathlib.Path(a) for a in args] or [pathlib.Path(c) for c in CORPS_LIVRES]

    connus = set()
    if BASELINE.exists():
        connus = {
            l.strip()
            for l in BASELINE.read_text(encoding="utf-8").splitlines()
            if l.strip() and not l.startswith("#")
        }

    total, absents, nouveaux, tolerees, vus = 0, 0, 0, 0, set()
    for c in cibles:
        if not c.exists():
            # Un corps absent n'est pas un succes silencieux : on le DIT. Un
            # controle qui n'a rien mesure n'est pas vert, il est aveugle.
            print(f"  ABSENT {c} — rien n'a ete mesure")
            absents += 1
            continue
        total += 1
        defauts = verifier(c)
        doc, _ = lire_glb(c)
        print(
            f"  {c.name} — {len(doc.get('animations', []))} clip(s), "
            f"{len(doc.get('nodes', []))} noeuds"
        )
        for d in defauts:
            k = cle(c, d)
            vus.add(k)
            if k in connus:
                tolerees += 1
                print(f"    · connu : {d}")
            else:
                nouveaux += 1
                print(f"    - NOUVEAU : {d}")
        if not defauts:
            print("    aucun defaut de structure")

    if ecrire:
        BASELINE.parent.mkdir(parents=True, exist_ok=True)
        BASELINE.write_text(
            "# Defauts de structure connus des corps d'animation livres.\n"
            "# CE FICHIER NE DOIT QUE RETRECIR.\n"
            "# Regenerer : python tools/assets/verifier_corps.py --ecrire-baseline\n"
            + "\n".join(sorted(vus))
            + "\n",
            encoding="utf-8",
        )
        print(f"[verifier-corps] ligne de base ecrite : {BASELINE} ({len(vus)} entrees)")

    reparees = connus - vus
    for r in sorted(reparees):
        print(f"  · REPARE {r} — retirer de {BASELINE}")
    print(
        f"[verifier-corps] {total} corps mesure(s), {absents} absent(s), "
        f"{tolerees} defaut(s) connu(s), {len(reparees)} repare(s), {nouveaux} NOUVEAU(X)"
    )
    if absents and total == 0:
        print("[verifier-corps] AVEUGLE — aucun corps mesure")
        return 1
    return 1 if nouveaux else 0


if __name__ == "__main__":
    sys.exit(main())
