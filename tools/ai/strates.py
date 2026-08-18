#!/usr/bin/env python3
"""strates.py — Les liens entre les strates de The Spared, mesures sur la SORTIE.

Quatre controles, quatre classes de defaut deja payees dans ce projet :

  C1 plugins-orphelins  un plugin declare que `forgia-game::run` n'atteint jamais.
                        Il compile, il a des tests, il ne tourne pas. Personne ne
                        le voit manquer parce que rien ne dit qu'il devrait etre la.

  C2 genomes-morts      un TOML de `assets/genomes/` qu'aucun Rust ne nomme. Le
                        piege n'est pas le fichier, c'est qu'une REGLE finit par
                        le citer : `map-design-intention.md` a dimensionne des
                        salles entieres sur `enemies/enemy_grunt.toml`, que le
                        jeu n'a jamais lu.

  C3 strates-inversees  une crate PARTAGEE qui depend d'une crate de ZONE. Le
                        partage devient alors indissociable d'un mode : on ne
                        peut plus toucher la zone sans risquer tout le reste.

  C4 fuites-de-zone     un module public d'une zone importe par une autre crate.
                        La zone est devenue une bibliotheque sans changer de nom.

Portee declaree (ce controle NE regarde PAS) : la justesse de ce qui est branche,
les dependances tierces, le contenu des TOML, l'ordre des systemes. Il mesure des
LIENS, pas de la qualite. Un lien present peut etre mauvais.

    python tools/ai/strates.py            # rapport
    python tools/ai/strates.py --strict   # sortie 1 si regression vs ligne de base
    python tools/ai/strates.py --ecrire-baseline
"""
import collections
import os
import re
import subprocess
import sys

RACINE = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
BASELINE = os.path.join(RACINE, "docs", "audit", "strates-baseline.txt")

# Une ZONE = un endroit ou le joueur se tient. Le lobby en est un.
ZONES = {
    "forgia-mode-roguelite",
    "forgia-mode-expedition",
    "forgia-mode-fps-arena",
    "forgia-menu-hub",
    "forgia-rpg",
    "forgia-fps",
}
ASSEMBLAGE = {"forgia-game", "root"}


def git(*a):
    return subprocess.run(
        ["git", *a], cwd=RACINE, capture_output=True, text=True
    ).stdout.split()


def crate_de(f):
    return f.split("/")[1] if f.startswith("crates/") else "root"


def lire(f):
    with open(os.path.join(RACINE, f), encoding="utf-8", errors="replace") as h:
        return h.read()


def bloc(t, i):
    """Le texte entre l'accolade/parenthese ouvrante en i et sa fermante."""
    paires = {"{": "}", "(": ")"}
    ouv = t[i]
    fer = paires[ouv]
    p, j = 0, i
    while j < len(t):
        if t[j] == ouv:
            p += 1
        elif t[j] == fer:
            p -= 1
            if p == 0:
                return t[i:j]
        j += 1
    return t[i:]


def c1_plugins(src):
    decl = {}
    for f, t in src.items():
        for m in re.finditer(r"pub struct ([A-Za-z0-9_]*Plugin)\b", t):
            decl.setdefault(m.group(1), f)

    graphe = collections.defaultdict(set)
    for f, t in src.items():
        for m in re.finditer(r"impl\s+Plugin\s+for\s+([A-Za-z0-9_]+)", t):
            i = t.find("{", m.end())
            if i < 0:
                continue
            corps = re.sub(r"//[^\n]*", "", bloc(t, i))
            for am in re.finditer(r"add_plugins\s*\(", corps):
                for n in re.findall(
                    r"\b([A-Za-z0-9_]*Plugin)\b", bloc(corps, am.end() - 1)
                ):
                    if n in decl:
                        graphe[m.group(1)].add(n)

    # Racines : les add_plugins de l'assemblage, hors `impl Plugin`.
    racines = set()
    t = re.sub(r"//[^\n]*", "", src["crates/forgia-game/src/lib.rs"])
    for am in re.finditer(r"add_plugins\s*\(", t):
        for n in re.findall(r"\b([A-Za-z0-9_]*Plugin)\b", bloc(t, am.end() - 1)):
            if n in decl:
                racines.add(n)

    vus, pile = set(), list(racines)
    while pile:
        p = pile.pop()
        if p in vus:
            continue
        vus.add(p)
        pile.extend(graphe.get(p, ()))
    return decl, racines, vus


def main():
    strict = "--strict" in sys.argv
    ecrire = "--ecrire-baseline" in sys.argv

    rs = git("ls-files", "crates/**/*.rs", "src/*.rs")
    src = {f: lire(f) for f in rs}
    tout_le_rust = "\n".join(src.values())

    faits = []  # (classe, cle, detail)

    # -- C1 ---------------------------------------------------------------
    decl, racines, vus = c1_plugins(src)
    for p in sorted(set(decl) - vus):
        faits.append(("plugin-orphelin", p, decl[p]))

    # -- C2 ---------------------------------------------------------------
    genomes = [g for g in git("ls-files", "assets/genomes") if g.endswith(".toml")]
    for g in genomes:
        base = os.path.basename(g)
        if base in tout_le_rust or base[:-5] in tout_le_rust:
            continue
        faits.append(("genome-mort", g, ""))

    # -- C3 / C4 ----------------------------------------------------------
    deps = {}
    for c in sorted(os.listdir(os.path.join(RACINE, "crates"))):
        p = os.path.join(RACINE, "crates", c, "Cargo.toml")
        if not os.path.isfile(p):
            continue
        with open(p, encoding="utf-8", errors="replace") as h:
            txt = h.read()
        sect = txt[txt.find("[dependencies]"):] if "[dependencies]" in txt else ""
        deps[c] = set(re.findall(r"^(forgia-[a-z0-9-]+)\s*=", sect, re.M))

    for c, ds in sorted(deps.items()):
        if c in ZONES or c in ASSEMBLAGE:
            continue
        for d in sorted(ds & ZONES):
            faits.append(("strate-inversee", c + "|" + d, "partagee -> zone"))

    fuites = collections.defaultdict(set)
    for f, t in src.items():
        c = crate_de(f)
        # Sans ca, un chemin CITE dans un commentaire de documentation compte
        # comme un import : `forgia-combat` remontait ainsi une fuite qui
        # n'existe que dans une phrase. Mesurer la sortie, pas la prose.
        t = re.sub(r"//[^\n]*", "", t)
        for m in re.finditer(r"\bforgia_([a-z0-9_]+)::([a-z0-9_]+)", t):
            z = "forgia-" + m.group(1).replace("_", "-")
            if z in ZONES and z != c:
                fuites[z].add(m.group(2))
    for z, mods in sorted(fuites.items()):
        for mod in sorted(mods):
            faits.append(("fuite-de-zone", z + "::" + mod, ""))

    cles = {k + "|" + c for k, c, _ in faits}

    base = set()
    if os.path.isfile(BASELINE):
        with open(BASELINE, encoding="utf-8") as h:
            base = {l.strip() for l in h if l.strip() and not l.startswith("#")}

    if ecrire:
        os.makedirs(os.path.dirname(BASELINE), exist_ok=True)
        with open(BASELINE, "w", encoding="utf-8") as h:
            h.write(
                "# Ligne de base de `tools/ai/strates.py` — CE FICHIER NE DOIT QUE RETRECIR.\n"
                "# Chaque ligne est un lien casse ou absent, tolere parce qu'il preexiste.\n"
                "# Un lien NOUVEAU casse fait echouer --strict et n'a pas sa place ici.\n"
                "# Regenerer : python tools/ai/strates.py --ecrire-baseline\n"
            )
            for k in sorted(cles):
                h.write(k + "\n")
        print("[strates] ligne de base ecrite — %d entree(s)" % len(cles))
        return 0

    par_classe = collections.Counter(k for k, _, _ in faits)
    nouveaux = sorted(cles - base)
    repares = sorted(base - cles)

    titres = [
        ("plugin-orphelin", "C1 plugins declares que l'assemblage n'atteint jamais"),
        ("genome-mort", "C2 genomes sans aucun consommateur Rust"),
        ("strate-inversee", "C3 crates partagees qui dependent d'une zone"),
        ("fuite-de-zone", "C4 modules de zone importes du dehors"),
    ]
    for classe, titre in titres:
        lignes = [(c, d) for k, c, d in faits if k == classe]
        print("\n-- %s : %d" % (titre, len(lignes)))
        for c, d in lignes[:80]:
            marque = " " if (classe + "|" + c) in base else "+"
            print("  %s %s%s" % (marque, c, ("  — " + d) if d else ""))
        if len(lignes) > 80:
            print("    ... %d de plus" % (len(lignes) - 80))

    print(
        "\n[strates] %d plugin(s) declare(s) · %d racine(s) · %d atteint(s) "
        "· %d genome(s) · %d crate(s)"
        % (len(decl), len(racines), len(vus), len(genomes), len(deps))
    )
    print(
        "  portee : liens seulement — ni la justesse de ce qui est branche, ni les "
        "deps tierces, ni le contenu des TOML, ni l'ordre des systemes"
    )
    print("  %s" % dict(par_classe))
    if repares:
        print("  %d repare(s) — les retirer de la ligne de base :" % len(repares))
        for r in repares[:20]:
            print("    - " + r)
    if nouveaux:
        print("  %d NOUVEAU(X) hors ligne de base :" % len(nouveaux))
        for n in nouveaux[:20]:
            print("    + " + n)
        if strict:
            return 1
    else:
        print("  0 regression")
    return 0


if __name__ == "__main__":
    sys.exit(main())
