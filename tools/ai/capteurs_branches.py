"""Chaque capteur a-t-il encore un producteur BRANCHÉ ?

# Le défaut que cet outil attrape

Un fichier de capteur qui reste sur le disque après que son producteur a été
débranché **se lit comme une mesure actuelle**. C'est le pire état possible d'un
instrument : il ne manque pas, il ment. Et il ment d'autant mieux qu'il a été
juste pendant des mois.

Le cas se produit sans faute de personne : on refactore un module, on déplace un
système, on oublie une ligne d'`add_systems` — le code compile, le jeu tourne, et
le JSON d'hier reste là.

# Les quatre états, et pourquoi il faut les distinguer

| état | ce qu'on voit | ce que ça veut dire |
|---|---|---|
| **BRANCHÉ** | producteur trouvé, système enregistré | rien à faire |
| **DÉBRANCHÉ** | producteur trouvé, système jamais dans un `add_systems` | 🚨 le fichier ment |
| **SANS PRODUCTEUR** | fichier sur disque, aucun appel trouvé dans le code | soit `fs::write` (bloque la frame), soit un producteur que ce script ne sait pas lire — VÉRIFIER avant de conclure |
| **MUET** | producteur branché, aucun fichier | jamais exécuté (mode jamais entré ?) |

Un outil qui les confondrait ne servirait à rien : « débranché » se corrige en
une ligne, « orphelin » se corrige en supprimant un fichier, et « muet » n'est
peut-être pas un défaut du tout.

Usage :
    python tools/ai/capteurs_branches.py           # rapport
    python tools/ai/capteurs_branches.py --strict  # sortie 1 si un DÉBRANCHÉ
"""

import argparse
import pathlib
import re
import sys

RACINE = pathlib.Path(__file__).resolve().parents[2]

# `enqueue("forgia2_x.json", …)` ou `enqueue(CHEMIN, …)` avec une const
APPEL = re.compile(r'enqueue\(\s*"([^"]+\.json)"')
APPEL_CONST = re.compile(r"enqueue\(\s*([A-Z_][A-Z0-9_]*)\s*,")
# 🚨 `forgia_core::constat` (2026-08-18) construit le nom depuis un ID :
# `.publier("animation", …)` ecrit `forgia2_animation.json`. Sans cette ligne,
# l'outil ne trouve plus le producteur et classe le capteur « hors pipeline » —
# c'est arrive le jour meme de la migration, sur les deux premiers migres.
# Une amelioration qui casse l'instrument qui la surveille est une regression.
APPEL_PUBLIER = re.compile(r'\.publier\(\s*"([a-z0-9_]+)"', re.S)
CONST_STR = re.compile(r'const\s+([A-Z_][A-Z0-9_]*)\s*:\s*&str\s*=\s*"([^"]+\.json)"')
DEBUT_FN = re.compile(r"^(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+([a-zA-Z0-9_]+)")


def fonction_englobante(lignes, i):
    """Remonte jusqu'au `fn` de colonne 0 qui contient la ligne `i`."""
    for j in range(i, -1, -1):
        m = DEBUT_FN.match(lignes[j])
        if m:
            return m.group(1)
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--strict", action="store_true", help="sortie 1 si un capteur est debranche")
    args = ap.parse_args()

    sources = {}
    for p in (RACINE / "crates").rglob("*.rs"):
        try:
            sources[p] = p.read_text(encoding="utf-8", errors="replace")
        except Exception:
            pass
    corpus = "\n".join(sources.values())

    # 1. Les producteurs déclarés : chemin -> (fichier, fonction)
    producteurs = {}
    for p, texte in sources.items():
        lignes = texte.splitlines()
        consts = dict(CONST_STR.findall(texte))
        for i, l in enumerate(lignes):
            chemins = APPEL.findall(l)
            for nom_const in APPEL_CONST.findall(l):
                if nom_const in consts:
                    chemins.append(consts[nom_const])

            for c in chemins:
                fn = fonction_englobante(lignes, i)
                producteurs.setdefault(c, []).append((p.relative_to(RACINE), fn))

        # `.publier("animation", …)` ecrit `forgia2_animation.json`.
        #
        # 🚨 Deux pieges, tous deux rencontres le 2026-08-18 en ecrivant CETTE
        # ligne : l'appel tient souvent sur DEUX lignes (`.publier(` puis l'id),
        # donc un balayage ligne a ligne le rate ; et un exemple en commentaire
        # de doc se fait matcher comme un vrai producteur. On balaie donc le
        # texte entier, et on ecarte les lignes de commentaire.
        for m in APPEL_PUBLIER.finditer(texte):
            no = texte.count(chr(10), 0, m.start())
            if lignes[no].lstrip().startswith("//"):
                continue
            producteurs.setdefault(f"forgia2_{m.group(1)}.json", []).append(
                (p.relative_to(RACINE), fonction_englobante(lignes, no))
            )

    # 2. Le système est-il enregistré ? On cherche son nom dans un contexte
    #    d'enregistrement — `add_systems`, `add_plugins`, ou une référence nue
    #    dans un tuple de planification.
    #    🚨 Une fonction peut être branchée SANS être un système : `ecrire_releve`
    #    est appelée par `write_sensor_and_health`, qui lui est enregistré. Ne
    #    tester que la fonction englobante déclarait 15 capteurs débranchés dont
    #    la plupart écrivaient très bien — un instrument qui vise à côté fabrique
    #    un défaut au lieu d'en trouver un.
    #
    #    On suit donc la chaîne d'appel : est branché tout ce qui est enregistré,
    #    plus tout ce qu'un branché APPELLE, transitivement.
    #    Le signal est le plus SIMPLE possible, et c'est délibéré : une fonction
    #    est branchée si son nom apparaît quelque part **en dehors de sa propre
    #    définition**. Enregistrée dans un `add_systems`, appelée par un autre
    #    système, référencée dans un tuple — peu importe la forme, elle est
    #    citée.
    #
    #    🚨 Une version antérieure découpait les blocs `add_systems(...)` à
    #    l'expression régulière. Elle s'arrêtait au premier `)`, ratait tout
    #    `.in_set(GameSet::Sensors)`, et déclarait **dix capteurs débranchés qui
    #    marchaient tous**. Un instrument qui vise à côté ne rate pas un défaut :
    #    il en fabrique dix, et on va les corriger. Le signal grossier qui ne
    #    ment pas vaut mieux que l'analyse fine qui se trompe.
    references = {}
    for texte in sources.values():
        for nom in re.findall(r"[a-zA-Z_][a-zA-Z0-9_]*", texte):
            references[nom] = references.get(nom, 0) + 1
    # Chaque définition compte sa propre occurrence : on la retranche.
    definitions = {}
    for texte in sources.values():
        for m in DEBUT_FN.finditer(texte):
            pass
    for texte in sources.values():
        for l in texte.splitlines():
            m = DEBUT_FN.match(l)
            if m:
                definitions[m.group(1)] = definitions.get(m.group(1), 0) + 1

    def est_branche(fn):
        """Cité ailleurs que dans sa ou ses définitions."""
        return references.get(fn, 0) > definitions.get(fn, 0)

    enregistres = {fn for fn in definitions if est_branche(fn)}

    # 3. Les fichiers réellement présents
    sur_disque = {p.name for p in RACINE.glob("forgia*_*.json")}

    branches, debranches, muets = [], [], []
    for chemin, sites in sorted(producteurs.items()):
        fns = [fn for _, fn in sites if fn]
        vu = any(fn in enregistres for fn in fns)
        present = chemin in sur_disque
        if not vu:
            debranches.append((chemin, sites, present))
        elif not present:
            muets.append((chemin, sites))
        else:
            branches.append(chemin)

    connus = set(producteurs)
    orphelins = sorted(f for f in sur_disque if f not in connus)

    print(f"{len(producteurs)} producteurs declares · {len(sur_disque)} fichiers sur disque")
    print()
    print(f"  BRANCHES   {len(branches):3}  (producteur trouve, systeme enregistre)")
    print(f"  DEBRANCHES {len(debranches):3}  <- le fichier MENT s'il est encore la")
    print(f"  HORS-PIPE  {len(orphelins):3}  <- producteur introuvable : verifier avec `cargo run -p xtask -- capteur-gate --liste`")
    print(f"  MUETS      {len(muets):3}  (branche, mais jamais ecrit)")

    if debranches:
        print()
        print("=== DEBRANCHES ===")
        for c, sites, present in debranches:
            etat = "FICHIER PRESENT — il ment" if present else "pas de fichier"
            print(f"  {c}  [{etat}]")
            for f, fn in sites[:2]:
                print(f"      produit par {fn}()  dans {f}")

    if orphelins:
        print()
        print("=== SANS PRODUCTEUR TROUVE (verifier : fs::write, ou motif non reconnu ?) ===")
        for f in orphelins:
            p = RACINE / f
            print(f"  {f}  ({p.stat().st_size/1024:.1f} Ko)")

    if muets:
        print()
        print("=== MUETS (branches, jamais ecrits) ===")
        for c, sites in muets[:20]:
            print(f"  {c}  <- {sites[0][1]}()")

    if args.strict and debranches:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
