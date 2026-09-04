#!/usr/bin/env python3
"""Photographie l'eclairage REEL du Hall depuis le monde vivant, via BRP.

    python tools/ai/lumiere_hall_brp.py            # resume + ecrit le JSON
    python tools/ai/lumiere_hall_brp.py --json     # le JSON sur la sortie standard

Pourquoi cet outil : les genomes DECLARENT des lumieres, le code les CONVERTIT
(facteur d'echelle Unity->lumens, budget d'allumage par distance, despawn hors
mode). Entre les deux il y a toute la place pour qu'une valeur n'arrive jamais.
Ce script ne lit aucun fichier de reglage : il demande au jeu ce qu'il porte.

Prerequis : le jeu tourne avec `dev-brp`, ET il est DANS le Hall — sinon les
lumieres du chateau n'existent pas. Le plus simple :

    FORGIA_BOOT_MODE=castle_hub cargo run -p forgia --features dev-brp

Sortie : target/analyse_lumiere/lumieres_vivantes.json
"""

from __future__ import annotations

import argparse
import json
import math
import sys
import urllib.error
import urllib.request
from pathlib import Path

BRP_URL = "http://127.0.0.1:15702/"
RACINE = Path(__file__).resolve().parents[2]
SORTIE = RACINE / "target" / "analyse_lumiere" / "lumieres_vivantes.json"

# Les chemins de types tels que Bevy 0.18 les enregistre. Un nom faux ne leve
# pas : la requete rend simplement zero entite — d'ou le controle de portee en
# fin de script, qui refuse un resultat vide au lieu de le presenter comme vert.
PONCTUELLE = "bevy_light::point_light::PointLight"
SPOT = "bevy_light::spot_light::SpotLight"
DIRECTIONNELLE = "bevy_light::directional_light::DirectionalLight"
AMBIANTE = "bevy_light::ambient_light::AmbientLight"
TRANSFORM = "bevy_transform::components::global_transform::GlobalTransform"
NOM = "bevy_ecs::name::Name"


def brp(methode: str, params: dict | None = None) -> dict:
    charge: dict = {"jsonrpc": "2.0", "id": 1, "method": methode}
    if params is not None:
        charge["params"] = params
    requete = urllib.request.Request(
        BRP_URL,
        data=json.dumps(charge).encode("utf-8"),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(requete, timeout=10) as reponse:
            corps = json.load(reponse)
    except (urllib.error.URLError, OSError) as erreur:
        raise SystemExit(
            "BRP injoignable sur 127.0.0.1:15702 — le jeu n'est pas lance, ou il "
            "l'est SANS la feature dev-brp.\n  (%s)" % erreur
        ) from erreur
    if "error" in corps:
        raise SystemExit("BRP %s : %s" % (methode, corps["error"]))
    return corps["result"]


def position(gt) -> list[float]:
    """La translation d'un `GlobalTransform`, quelle que soit sa forme serialisee.

    Bevy le serialise en affine 3x3 + translation. Selon la version le JSON rend
    un objet ou un tableau plat : on accepte les deux plutot que de parier.
    """
    if isinstance(gt, dict):
        for cle in ("translation", "3"):
            if cle in gt:
                v = gt[cle]
                if isinstance(v, dict):
                    return [v.get("x", 0.0), v.get("y", 0.0), v.get("z", 0.0)]
                if isinstance(v, list) and len(v) >= 3:
                    return [float(x) for x in v[:3]]
        # Forme { matrix3: {...}, translation: {...} } deja couverte ; sinon on
        # tente les 3 derniers scalaires d'une matrice aplatie.
        plat = [v for v in gt.values() if isinstance(v, (int, float))]
        if len(plat) >= 12:
            return [float(x) for x in plat[9:12]]
    if isinstance(gt, list) and len(gt) >= 12:
        return [float(x) for x in gt[9:12]]
    return [math.nan, math.nan, math.nan]


def interroger(composants: list[str], optionnels: list[str]) -> list[dict]:
    return brp(
        "world.query",
        {
            "data": {"components": composants, "option": optionnels},
            "filter": {"with": [composants[0]]},
        },
    )


def collecte() -> dict:
    lumieres = []

    for famille, type_ in (("ponctuelle", PONCTUELLE), ("spot", SPOT)):
        for e in interroger([type_], [TRANSFORM, NOM]):
            c = e.get("components", {})
            l = c.get(type_, {})
            lumieres.append(
                {
                    "famille": famille,
                    "entite": e.get("entity"),
                    "nom": (c.get(NOM) or {}).get("name")
                    if isinstance(c.get(NOM), dict)
                    else c.get(NOM),
                    "position": position(c.get(TRANSFORM)),
                    "intensite_lm": l.get("intensity"),
                    "portee_m": l.get("range"),
                    "rayon_m": l.get("radius"),
                    "couleur": l.get("color"),
                    "ombres": l.get("shadows_enabled"),
                    "angle_interieur_rad": l.get("inner_angle"),
                    "angle_exterieur_rad": l.get("outer_angle"),
                }
            )

    for e in interroger([DIRECTIONNELLE], [TRANSFORM, NOM]):
        c = e.get("components", {})
        l = c.get(DIRECTIONNELLE, {})
        lumieres.append(
            {
                "famille": "directionnelle",
                "entite": e.get("entity"),
                "nom": (c.get(NOM) or {}).get("name")
                if isinstance(c.get(NOM), dict)
                else c.get(NOM),
                "position": position(c.get(TRANSFORM)),
                "illuminance_lux": l.get("illuminance"),
                "couleur": l.get("color"),
                "ombres": l.get("shadows_enabled"),
            }
        )

    ambiantes = []
    for e in interroger([AMBIANTE], [NOM]):
        c = e.get("components", {})
        a = c.get(AMBIANTE, {})
        ambiantes.append(
            {
                "entite": e.get("entity"),
                "brightness": a.get("brightness"),
                "couleur": a.get("color"),
                "affects_lightmapped_meshes": a.get("affects_lightmapped_meshes"),
            }
        )

    return {"lumieres": lumieres, "ambiantes": ambiantes}


def resume(d: dict) -> int:
    par_famille: dict[str, list] = {}
    for l in d["lumieres"]:
        par_famille.setdefault(l["famille"], []).append(l)

    total = len(d["lumieres"])
    print("PORTEE : le monde ECS vivant, a l'instant de l'appel. Ce script ne lit")
    print("         AUCUN genome — si un chiffre diverge du TOML, c'est le TOML")
    print("         qui n'atteint pas la lumiere, et c'est precisement le defaut")
    print("         qu'on cherche. Non couvert : l'occlusion, la lumiere cuite")
    print("         (elle est dans les materiaux, pas dans des entites).")
    print()
    print("%-16s %6s  %s" % ("famille", "compte", "detail"))
    for fam in sorted(par_famille):
        ls = par_famille[fam]
        if fam == "directionnelle":
            for l in ls:
                print(
                    "%-16s %6d  %.0f lux, ombres=%s"
                    % (fam, 1, l.get("illuminance_lux") or 0, l.get("ombres"))
                )
        else:
            intens = [l["intensite_lm"] for l in ls if l.get("intensite_lm") is not None]
            portees = [l["portee_m"] for l in ls if l.get("portee_m") is not None]
            avec_ombres = sum(1 for l in ls if l.get("ombres"))
            print(
                "%-16s %6d  intensite %.0f..%.0f lm · portee %.2f..%.2f m · %d avec ombres"
                % (
                    fam,
                    len(ls),
                    min(intens) if intens else 0,
                    max(intens) if intens else 0,
                    min(portees) if portees else 0,
                    max(portees) if portees else 0,
                    avec_ombres,
                )
            )
    for a in d["ambiantes"]:
        print("%-16s %6d  brightness %.0f" % ("ambiante", 1, a.get("brightness") or 0))

    avec_ombres = sum(1 for l in d["lumieres"] if l.get("ombres"))
    print()
    print("SOURCES AVEC OMBRES : %d / %d" % (avec_ombres, total))

    SORTIE.parent.mkdir(parents=True, exist_ok=True)
    SORTIE.write_text(json.dumps(d, indent=1), encoding="utf-8")
    print("ECRIT %s" % SORTIE)

    # 🚨 Zero mesure n'est pas un feu vert. Si le jeu est au menu, les lumieres du
    # chateau n'existent pas et tout ce qui precede vaut zero : le dire fort.
    if total == 0:
        print()
        print("AVEUGLE : 0 lumiere trouvee. Le jeu est-il DANS le Hall ?")
        print("          FORGIA_BOOT_MODE=castle_hub cargo run -p forgia --features dev-brp")
        return 1
    return 0


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--json", action="store_true", help="le JSON brut sur stdout")
    args = p.parse_args()
    d = collecte()
    if args.json:
        print(json.dumps(d, indent=1))
        return 0
    return resume(d)


if __name__ == "__main__":
    sys.exit(main())
