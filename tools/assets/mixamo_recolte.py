#!/usr/bin/env python3
"""Récolte des clips Mixamo nommés, via son API interne.

Ce script va chercher une **liste déclarée** de mouvements — pas le catalogue.
La distinction n'est pas cosmétique : les animations Mixamo sont libres d'usage,
mais l'aspiration de masse est dans une zone grise du contrat Adobe. Ici on
demande douze clips qu'on utilise vraiment, un par un, en respectant la limite
de débit.

# Ce que le script fait, et ce qu'il ne fait pas

1. Cherche chaque clip par son nom dans le catalogue.
2. Demande son export en FBX **sans skin** (on remappe par nom d'os — la peau ne
   sert à rien et triple le poids).
3. Attend que l'export soit prêt, puis le télécharge.
4. **Ne convertit pas** en glTF : c'est le travail de `--convertir`, qui appelle
   Blender en ligne de commande. Séparé parce qu'un téléchargement qui réussit
   et une conversion qui échoue sont deux pannes différentes, et qu'on veut
   savoir laquelle on a.

# Le jeton, et pourquoi il n'y a pas moyen de l'éviter

L'API exige un jeton Bearer de session. Il n'y a pas de clé d'application à
demander : Adobe n'expose pas cette API publiquement. On la prend donc là où le
navigateur la met.

    1. Se connecter sur mixamo.com
    2. F12 → onglet Réseau → lancer n'importe quelle recherche
    3. Ouvrir la requête `api/v1/products`
    4. Copier l'en-tête `Authorization`, SANS le mot « Bearer »

Le jeton expire (quelques heures). Ce n'est pas un défaut du script : c'est ce
qu'est un jeton de session. Quand il expire, l'API rend 401 et le script le dit
au lieu de télécharger des fichiers vides.

Le `character_id` se lit dans la même requête (n'importe quel personnage Mixamo
fait l'affaire : on ne garde que les courbes, remappées par nom d'os).

Les deux vont dans `tools/assets/mixamo.local.toml`, qui est **ignoré par git** —
un jeton de session dans un dépôt est une fuite, même sur un dépôt privé.

# Fragilité assumée

Ces points d'API ne sont pas documentés par Adobe : ils sont connus par
observation, et ils peuvent changer sans préavis. Le script **échoue fort** —
code de sortie non nul, message qui nomme l'étape — plutôt que de rendre un
dossier à moitié rempli. Un outil d'asset qui réussit à moitié en silence coûte
plus cher que pas d'outil.

Usage :
    python tools/assets/mixamo_recolte.py                 # télécharge les FBX
    python tools/assets/mixamo_recolte.py --clip slide    # un seul
    python tools/assets/mixamo_recolte.py --convertir     # FBX → GLB (Blender)
    python tools/assets/mixamo_recolte.py --lister        # ce qui est déclaré
"""

from __future__ import annotations

import argparse
import json
import struct
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:  # Python < 3.11
    print("ERREUR : Python 3.11+ requis (tomllib).", file=sys.stderr)
    raise SystemExit(2)

RACINE = Path(__file__).resolve().parents[2]
CONFIG = RACINE / "tools" / "assets" / "mixamo.local.toml"
MANIFESTE = RACINE / "tools" / "assets" / "mixamo_clips.toml"
SORTIE = RACINE / "assets" / "models" / "characters" / "stylized" / "anims"

API = "https://www.mixamo.com/api/v1"
# Clé publique de l'application web Mixamo. Elle n'authentifie rien — c'est le
# jeton Bearer qui le fait — mais l'API refuse les requêtes sans elle.
CLE_API = "mixamo2"

# Délai entre deux clips. L'API rend 429 quand on la presse, et un 429 mal géré
# fait re-télécharger le même clip en boucle jusqu'à expiration du quota (défaut
# connu des outils publics). Une seconde entre deux clips coûte douze secondes
# sur la liste entière : ce n'est pas cher payé.
PAUSE_ENTRE_CLIPS_S = 1.0
# Attente maximale pour un export. Mixamo met 5 à 20 s ; au-delà d'une minute
# c'est bloqué, et attendre en silence n'aide personne.
ATTENTE_EXPORT_MAX_S = 60.0


class Echec(Exception):
    """Panne nommée. Le message dit L'ÉTAPE, pas seulement le symptôme."""


# ─────────────────────────────────────────────────────────────────────────────
# Configuration
# ─────────────────────────────────────────────────────────────────────────────


def lire_config() -> tuple[str, str]:
    if not CONFIG.exists():
        raise Echec(
            f"{CONFIG.relative_to(RACINE)} absent. Le créer avec :\n\n"
            '  token = "<en-tête Authorization, sans le mot Bearer>"\n'
            '  character_id = "<vu dans la même requête>"\n\n'
            "Les deux se lisent dans F12 → Réseau → api/v1/products, "
            "une fois connecté sur mixamo.com."
        )
    d = tomllib.loads(CONFIG.read_text(encoding="utf-8"))
    jeton = str(d.get("token", "")).strip()
    perso = str(d.get("character_id", "")).strip()
    if not jeton or not perso:
        raise Echec(f"{CONFIG.name} : `token` et `character_id` sont tous deux requis.")
    if jeton.lower().startswith("bearer "):
        # Erreur de copier-coller assez fréquente pour valoir une correction
        # silencieuse plutôt qu'un 401 incompréhensible.
        jeton = jeton[7:].strip()
    return jeton, perso


def lire_manifeste(filtre: str | None) -> list[dict]:
    if not MANIFESTE.exists():
        raise Echec(f"{MANIFESTE.relative_to(RACINE)} absent — rien à récolter.")
    d = tomllib.loads(MANIFESTE.read_text(encoding="utf-8"))
    clips = d.get("clips", [])
    if not clips:
        raise Echec(f"{MANIFESTE.name} ne déclare aucun clip.")
    if filtre:
        clips = [c for c in clips if c.get("nom") == filtre]
        if not clips:
            raise Echec(f"aucun clip nommé « {filtre} » dans {MANIFESTE.name}.")
    return clips


# ─────────────────────────────────────────────────────────────────────────────
# API
# ─────────────────────────────────────────────────────────────────────────────


def appel(url: str, jeton: str, corps: dict | None = None, essais: int = 4) -> dict:
    """GET ou POST JSON, avec repli exponentiel sur 429.

    Un 429 non géré fait boucler les outils publics sur le même clip. On attend,
    on redemande, et on abandonne en le DISANT.
    """
    donnees = json.dumps(corps).encode() if corps is not None else None
    attente = 2.0
    for tentative in range(1, essais + 1):
        req = urllib.request.Request(url, data=donnees, method="POST" if donnees else "GET")
        req.add_header("Authorization", f"Bearer {jeton}")
        req.add_header("X-Api-Key", CLE_API)
        req.add_header("Accept", "application/json")
        if donnees:
            req.add_header("Content-Type", "application/json")
        try:
            with urllib.request.urlopen(req, timeout=30) as r:
                return json.loads(r.read().decode("utf-8"))
        except urllib.error.HTTPError as e:
            if e.code == 401:
                raise Echec(
                    "401 : jeton expiré ou invalide. Un jeton de session Mixamo "
                    "ne vit que quelques heures — le reprendre dans F12."
                ) from e
            if e.code == 429 and tentative < essais:
                print(f"    limite de débit atteinte, pause {attente:.0f}s…")
                time.sleep(attente)
                attente *= 2
                continue
            raise Echec(f"HTTP {e.code} sur {url}") from e
        except urllib.error.URLError as e:
            raise Echec(f"réseau injoignable : {e.reason}") from e
    raise Echec(f"abandon après {essais} tentatives sur {url}")


def chercher(terme: str, jeton: str, perso: str) -> dict:
    """Le produit dont le nom colle le mieux au terme cherché.

    On préfère une correspondance EXACTE (insensible à la casse) : « Jump » rend
    des dizaines de résultats, et prendre le premier donne un mouvement au
    hasard. Un terme qui ne colle exactement à rien est signalé avec les trois
    premiers résultats, pour qu'on corrige le manifeste au lieu de deviner.
    """
    q = urllib.parse.urlencode(
        {"page": 1, "limit": 48, "order": "", "type": "Motion,MotionPack", "query": terme}
    )
    d = appel(f"{API}/products?{q}", jeton)
    resultats = d.get("results", [])
    if not resultats:
        raise Echec(f"« {terme} » : aucun résultat dans le catalogue.")
    for r in resultats:
        if r.get("name", "").strip().lower() == terme.strip().lower():
            return r
    apercu = ", ".join(r.get("name", "?") for r in resultats[:3])
    raise Echec(
        f"« {terme} » : pas de correspondance exacte. Les plus proches : {apercu}. "
        f"Corriger `recherche` dans {MANIFESTE.name}."
    )


def exporter(produit: dict, jeton: str, perso: str, fps: int) -> str:
    """Demande l'export et rend l'URL du fichier prêt."""
    pid = produit["id"]
    detail = appel(f"{API}/products/{pid}?similar=0&character_id={perso}", jeton)
    gms = detail["details"]["gms_hash"]
    # Les paramètres arrivent en triplets [nom, defaut, ...] et l'API d'export
    # les attend aplatis en chaîne. C'est le seul endroit vraiment contre-intuitif
    # de cette API.
    gms["params"] = ",".join(str(p[1]) for p in gms.get("params", []))

    appel(
        f"{API}/animations/export",
        jeton,
        corps={
            "gms_hash": [gms],
            "preferences": {
                "format": "fbx7_2019",
                # Sans peau : on ne garde que les courbes, remappées par nom d'os
                # par `merge_gltf_anims.py`. Une peau embarquée serait un second
                # squelette à faire cohabiter — le piège déjà payé sur ce projet.
                "skin": "false",
                "fps": str(fps),
                "reducekf": "0",
            },
            "character_id": perso,
            "type": "Motion",
            "product_name": produit.get("name", "clip"),
        },
    )

    debut = time.time()
    while time.time() - debut < ATTENTE_EXPORT_MAX_S:
        m = appel(f"{API}/characters/{perso}/monitor", jeton)
        etat = m.get("status")
        if etat == "completed":
            url = m.get("job_result")
            if not url:
                raise Echec("export « completed » mais sans URL de résultat.")
            return url
        if etat == "failed":
            raise Echec(f"Mixamo a refusé l'export : {m.get('message', 'sans motif')}")
        time.sleep(1.5)
    raise Echec(f"export toujours pas prêt après {ATTENTE_EXPORT_MAX_S:.0f}s.")


def telecharger(url: str, cible: Path) -> int:
    cible.parent.mkdir(parents=True, exist_ok=True)
    with urllib.request.urlopen(url, timeout=120) as r:
        octets = r.read()
    # Un FBX de moins de 10 Ko n'est pas une animation : c'est une page d'erreur
    # ou un export vide. Mieux vaut le refuser que le laisser dans le dossier,
    # où il passerait pour un clip valide.
    if len(octets) < 10_000:
        raise Echec(f"fichier suspect ({len(octets)} octets) — export vide ?")
    cible.write_bytes(octets)
    return len(octets)


# ─────────────────────────────────────────────────────────────────────────────
# Conversion
# ─────────────────────────────────────────────────────────────────────────────

PREFIXE_MIXAMO = "mixamorig:"

# 🚨 LE défaut du 2026-08-17, et il ne se voyait qu'en jeu.
#
# Mixamo exporte ses FBX en **centimètres**. Le corps du jeu est riggé en
# **mètres**. Les courbes de translation arrivaient donc cent fois trop grandes :
# `LeftFoot` à 44,49 de son parent au lieu de 0,415. Ce n'est pas un os déplacé,
# c'est le squelette ENTIER qui s'étire d'un facteur cent — rapporté en jeu par
# « j'ai l'impression d'être immense ».
#
# Les courbes d'ÉCHELLE, elles, étaient à 1,0 : c'est ce qui rend le défaut
# trompeur. On cherche un facteur d'échelle, il n'y en a pas, et la vraie cause
# est dans les translations.
FACTEUR_CM_VERS_M = 0.01

# Au-delà de quoi on décide qu'un fichier est en centimètres.
#
# La plus longue translation d'os d'un squelette humain est la cuisse, ~0,45 m.
# Un demi-mètre laisse donc de la marge pour un personnage plus grand, sans
# jamais atteindre les dizaines que produit un fichier en centimètres. On MESURE
# et on décide : le convertisseur reste idempotent — le relancer ne divise pas
# deux fois — et il marchera si Mixamo change un jour d'unité.
SEUIL_OS_M = 2.0

# La SECONDE borne, sur une grandeur DIFFÉRENTE : de combien la racine se
# déplace pendant le clip.
#
# 🚨 Elles ne se déduisent pas l'une de l'autre, et c'est tout le piège du
# 2026-08-17. `transform_apply` a ramené la cuisse de 44,49 m à 0,45 — la borne
# ci-dessus est passée au vert sur les 25 clips — pendant que les hanches
# gardaient leur course en centimètres : 158 m sur `rifle_walk`, 682 m sur
# `slide`. Le personnage était à la bonne taille ET projeté hors de l'écran, ce
# qui s'est rapporté « je ne me vois plus nulle part quand je saute ».
#
# Une seule mesure ne pouvait pas attraper les deux : elles vivent dans deux
# endroits du fichier (le repos de l'armature / les keyframes de l'action), et
# corriger l'une laissait l'autre avec un défaut différent et tout aussi
# crédible. D'où deux bornes explicites, et non un contrôle « la taille est
# bonne ».
#
# 3 m : une foulée fait ~1,5 m et un saut chargé peut porter plus loin ; la
# décade reste hors d'atteinte sans un facteur cent.
COURSE_MAX_M = 3.0

# 🚨 Le renommage des os est la ligne la plus importante de ce script.
#
# Mixamo exporte ses os préfixés `mixamorig:` — `mixamorig:RightArm`. Le corps du
# jeu, lui, dit `RightArm` : les neuf clips déjà en place viennent d'une chaîne
# qui avait retiré le préfixe. Or `merge_gltf_anims.py` remappe **par nom** et
# n'a aucune gestion de préfixe.
#
# Sans ce renommage, la fusion trouve zéro correspondance et produit un corps
# avec vingt-cinq clips qui n'animent RIEN. Elle ne plante pas : elle signale des
# os manquants dans un log, et le personnage reste en pose de repos. Vérifié
# avant de fusionner, le 2026-08-17 — c'est exactement la panne silencieuse que
# ce dépôt paie le plus cher.
#
# Blender met à jour les chemins de données des courbes quand on renomme un os :
# le renommage suffit, il n'y a pas de retouche d'animation à faire derrière.
BLENDER_SCRIPT = f"""
import bpy, sys
entree, sortie = sys.argv[-2], sys.argv[-1]
bpy.ops.wm.read_factory_settings(use_empty=True)
# 🚨 `global_scale` À L'IMPORT, et pas une correction des courbes après coup.
#
# Mixamo exporte en centimètres. Ma première correction scalait les fcurves
# `.location` — et n'a RIEN changé, parce que ce qui part dans le glTF est
# `repos + animation` : les 44,49 relevés étaient la LONGUEUR AU REPOS de l'os,
# pas son déplacement. C'est l'armature entière qui était en centimètres.
#
# La leçon : corriger la grandeur qu'on mesure, pas celle qu'on suppose. Ici on
# ramène tout l'import à l'échelle, ce qui couvre repos ET animation.
# 🚨 `automatic_bone_orientation` DOIT rester a False.
#
# A True, Blender REORIENTE chaque os par une heuristique — il le fait pointer
# vers son enfant — au lieu de preserver le repere declare dans le FBX. Le rig
# importe ne parle alors plus la meme langue que le corps du jeu, et une
# rotation copiee de l'un a l'autre ne veut plus rien dire.
#
# Mesure du 2026-08-18, corps livre contre clip Mixamo : ecart median des
# orientations au repos **66 degres** hors mains (pieds 110,9 · cuisses 102,3 ·
# epaules 85,6), et l'erreur s'accumule le long de la chaine — les aretes du
# maillage s'etiraient de **x13,9** autour de la main droite. Les doigts, eux,
# concordaient a 0,7 deg : ce sont bien les os reorientes qui cassaient tout.
#
# On ne veut RIEN transformer ici : on veut des courbes d'animation exprimees
# dans le repere du corps. Toute commodite d'affichage qui reecrit ce repere est
# une corruption de la donnee.
bpy.ops.import_scene.fbx(filepath=entree, automatic_bone_orientation=False)

# `global_scale` a l'import est IGNORE ici (mesure : os toujours a 44,49 m).
# On passe donc par l'application de transformation, qui n'a pas d'ambiguite :
# elle cuit l'echelle dans les longueurs d'os au repos ET dans les courbes de
# translation de l'action. C'est la seule facon d'atteindre les DEUX.
for obj in bpy.data.objects:
    if obj.type != 'ARMATURE':
        continue
    bpy.ops.object.select_all(action='DESELECT')
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    obj.scale = ({FACTEUR_CM_VERS_M},) * 3
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)

# 🚨 ET les courbes de translation, SEPAREMENT.
#
# `transform_apply` cuit l'echelle dans les longueurs d'os au repos — mesure :
# la cuisse est bien passee de 44,49 a 0,45 m — mais il NE TOUCHE PAS aux
# keyframes de l'action. Les hanches gardaient donc leur deplacement en
# centimetres : 158 m d'excursion sur `rifle_walk`, 682 m sur `slide`. Le
# personnage etait a la bonne taille et projete hors de l'ecran.
#
# Deux grandeurs, deux corrections. Ma premiere version ne faisait que celle-ci,
# la deuxieme que celle du dessus : chacune reparait une moitie et laissait
# l'autre, ce qui donnait a chaque fois un defaut different et credible.
for act in bpy.data.actions:
    for fc in act.fcurves:
        if not fc.data_path.endswith('.location'):
            continue
        for kp in fc.keyframe_points:
            kp.co[1] *= {FACTEUR_CM_VERS_M}
            kp.handle_left[1] *= {FACTEUR_CM_VERS_M}
            kp.handle_right[1] *= {FACTEUR_CM_VERS_M}

renommes = 0
for obj in bpy.data.objects:
    if obj.type != 'ARMATURE':
        continue
    for os_ in obj.data.bones:
        if os_.name.startswith("{PREFIXE_MIXAMO}"):
            os_.name = os_.name[len("{PREFIXE_MIXAMO}"):]
            renommes += 1
print("OS_RENOMMES=%d" % renommes)

# ── La MESURE qui compte : la taille du squelette au repos ──────────────────
# C'est elle qui part dans le glTF, et c'est elle qui faisait un geant. Le plus
# long os d'un humain est la cuisse, ~0,45 m. Si on lit des dizaines, l'import
# n'a pas converti les unites et le clip etirera le personnage.
pire = 0.0
for obj in bpy.data.objects:
    if obj.type != 'ARMATURE':
        continue
    for os_ in obj.data.bones:
        pire = max(pire, (os_.tail_local - os_.head_local).length)
print("OS_LE_PLUS_LONG=%.3f" % pire)

# La SECONDE grandeur, celle qui projette le personnage hors de l'ecran : de
# combien la racine se deplace pendant le clip. Une foulee fait ~1,5 m ; au-dela
# de quelques metres, la courbe est dans une autre unite.
course = 0.0
for act in bpy.data.actions:
    for fc in act.fcurves:
        if not fc.data_path.endswith('.location'):
            continue
        vals = [kp.co[1] for kp in fc.keyframe_points]
        if vals:
            course = max(course, max(vals) - min(vals))
print("COURSE_RACINE=%.3f" % course)

bpy.ops.export_scene.gltf(filepath=sortie, export_format='GLB',
                          export_animations=True, export_skins=False,
                          export_materials='NONE')
"""


# Le nom de l'os racine dans la hiérarchie Mixamo, une fois le préfixe retiré.
# C'est lui — et lui seul — qui porte le déplacement d'ensemble du personnage.
OS_RACINE = "Hips"


def sur_place(glb: Path) -> float:
    """Retire la DÉRIVE de la racine, sur les trois axes. Rend la pire retirée.

    🚨 Pourquoi c'est obligatoire, et pas un réglage.

    Dans Forgia, l'avatar est un enfant visuel de la capsule que déplace le
    contrôleur. Un clip qui fait AUSSI avancer la racine met les deux en
    concurrence : le personnage dérive de sa propre capsule pendant toute la
    durée du clip, puis y revient d'un coup à la boucle suivante. Mesuré ici
    avant correction : `slide` emmenait la racine à 6,82 m, `rifle_start_run` à
    3,18 m — un glissement qui décolle littéralement le personnage du joueur.

    Mixamo propose une case « In Place » à l'export ; s'y fier voudrait dire que
    la justesse du résultat dépend d'une case cochée à la main sur un site
    externe, clip par clip. On la retire ici, systématiquement : le résultat ne
    dépend plus de ce qui a été demandé au téléchargement.

    🚨 On ne raisonne PAS par axe, et c'est le cœur de la leçon.

    Première version : « j'aplatis X et Z, je garde Y qui est la verticale ».
    Mesuré ensuite : la courbe Y de `slide` monte de 0,01 à 6,84 de façon
    STRICTEMENT CROISSANTE sur ses 47 images — c'est le déplacement vers
    l'avant, rangé dans Y par la chaîne d'export. J'avais donc supprimé le
    balancement du bassin et gardé la translation, exactement l'inverse du but,
    et le contrôle disait « 0,74 m retiré » en toute bonne foi.

    La convention d'axes de ces fichiers dépend de l'orientation du nœud parent,
    qui n'est pas la même selon la provenance du clip — `jump` a bien son
    déplacement dans Z quand `slide` l'a dans Y. Une règle qui nomme un axe est
    donc fausse pour une partie du lot, sans jamais le dire.

    Ce qui distingue le déplacement de l'animation n'est pas SON AXE, c'est sa
    FORME : une translation s'accumule, un balancement revient à son point de
    départ. On retire donc la rampe qui joint la première image à la dernière,
    sur les trois axes. Le cycle de marche, dont le début et la fin coïncident,
    n'est pas touché ; le saut, qui redescend, non plus. Cette règle ne connaît
    aucun axe, donc aucune convention ne peut la prendre en défaut.
    """
    d = bytearray(glb.read_bytes())
    lg_json, = struct.unpack_from("<I", d, 12)
    j = json.loads(d[20 : 20 + lg_json].decode("utf-8"))
    dep_bin = 20 + lg_json + 8

    racines = {
        i for i, n in enumerate(j.get("nodes", [])) if n.get("name") == OS_RACINE
    }
    if not racines:
        return 0.0

    pire = 0.0
    for anim in j.get("animations", []):
        for canal in anim.get("channels", []):
            cible = canal.get("target", {})
            if cible.get("path") != "translation" or cible.get("node") not in racines:
                continue
            acc = j["accessors"][anim["samplers"][canal["sampler"]]["output"]]
            bv = j["bufferViews"][acc["bufferView"]]
            base = dep_bin + bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
            n = acc["count"]
            vals = [struct.unpack_from("<fff", d, base + i * 12) for i in range(n)]
            if n < 2:
                continue
            # La dérive = ce qui sépare la dernière image de la première. On la
            # retire progressivement, pour ne pas créer de saut au milieu.
            derive = [vals[-1][k] - vals[0][k] for k in range(3)]
            pire = max(pire, max(abs(x) for x in derive))
            corr = []
            for i, v in enumerate(vals):
                t = i / (n - 1)
                corr.append(tuple(v[k] - derive[k] * t for k in range(3)))
            for i, v in enumerate(corr):
                struct.pack_into("<fff", d, base + i * 12, *v)
            # Les bornes déclarées doivent suivre la donnée : un accesseur dont
            # le min/max ment est un piège pour tout consommateur qui s'en sert
            # sans relire les octets.
            if "min" in acc and "max" in acc:
                acc["min"] = [min(v[k] for v in corr) for k in range(3)]
                acc["max"] = [max(v[k] for v in corr) for k in range(3)]

    # Le JSON a pu changer de longueur (min/max réécrits) : on reconstruit le
    # conteneur au lieu de parier que la taille est restée la même.
    neuf = json.dumps(j, separators=(",", ":")).encode("utf-8")
    neuf += b" " * (-len(neuf) % 4)
    binaire = bytes(d[dep_bin:])
    total = 12 + 8 + len(neuf) + 8 + len(binaire)
    out = bytearray()
    out += b"glTF" + struct.pack("<II", 2, total)
    out += struct.pack("<II", len(neuf), 0x4E4F534A) + neuf
    out += struct.pack("<II", len(binaire), 0x004E4942) + binaire
    glb.write_bytes(bytes(out))
    return pire


def convertir(dossier: Path, blender: str) -> int:
    """FBX → GLB, un par un, en signalant chaque échec.

    Séparé du téléchargement : un export réussi et une conversion ratée sont
    deux pannes distinctes, et les confondre ferait chercher au mauvais endroit.
    """
    fbx = sorted(dossier.glob("*.fbx"))
    if not fbx:
        raise Echec(f"aucun .fbx dans {dossier} — rien à convertir.")
    script = dossier / "_conv.py"
    script.write_text(BLENDER_SCRIPT, encoding="utf-8")
    faits = 0
    try:
        for f in fbx:
            glb = f.with_suffix(".glb")
            r = subprocess.run(
                [blender, "--background", "--factory-startup", "--python", str(script),
                 "--", str(f), str(glb)],
                capture_output=True, text=True, timeout=300,
            )
            if r.returncode != 0 or not glb.exists():
                print(f"  ✗ {f.name} : Blender a échoué\n{r.stderr[-400:]}", file=sys.stderr)
                continue
            # Zéro os renommé sur un clip Mixamo = le préfixe n'était pas là où
            # on croit, donc la fusion sera muette. On le DIT au lieu de compter
            # une conversion réussie : c'est le seul moment où l'information
            # existe encore.
            renommes, plus_long, course = 0, 0.0, 0.0
            for ligne in r.stdout.splitlines():
                if ligne.startswith("OS_RENOMMES="):
                    renommes = int(ligne.split("=", 1)[1])
                elif ligne.startswith("OS_LE_PLUS_LONG="):
                    plus_long = float(ligne.split("=", 1)[1])
                elif ligne.startswith("COURSE_RACINE="):
                    course = float(ligne.split("=", 1)[1])
            # La dérive de la racine est retirée ICI, avant d'être jugée. Le
            # contrôleur déplace le personnage, le clip l'anime.
            derive = sur_place(glb)
            # Et la borne porte sur ce qui RESTE, jamais sur l'état d'avant.
            #
            # 🚨 Une garde qui crie sur un défaut réparé trois lignes plus bas
            # apprend à ignorer les gardes. `slide` affichait « NE PAS
            # FUSIONNER » alors que sa dérive venait d'être supprimée — un
            # capteur qui a raison sur le passé et tort sur le fichier livré.
            # `course` reste utile comme mesure d'entrée, pas comme verdict.
            restant = max(0.0, course - derive)
            if restant > COURSE_MAX_M:
                print(
                    f"  ⚠ {glb.name} : il reste {restant:.1f} m de course racine "
                    f"après correction — le personnage sortira de l'écran. "
                    f"NE PAS FUSIONNER.",
                    file=sys.stderr,
                )
            # 🚨 LE contrôle qui aurait évité « j'ai l'impression d'être
            # immense ». Le plus long os d'un humain est la cuisse, ~0,45 m.
            # Au-delà de deux mètres, le squelette est dans une autre unité et
            # le clip étirera le personnage d'autant. Mesuré ici, où
            # l'information existe — pas déduit en jeu, trois heures plus tard.
            if plus_long > SEUIL_OS_M:
                print(
                    f"  ⚠ {glb.name} : os le plus long {plus_long:.2f} m — le "
                    f"squelette n'est pas à l'échelle du corps du jeu. Ce clip "
                    f"fera un géant. NE PAS FUSIONNER.",
                    file=sys.stderr,
                )
            if renommes == 0:
                print(
                    f"  ⚠ {glb.name} : aucun os préfixé « {PREFIXE_MIXAMO} » — "
                    "soit la source est déjà propre, soit le remappage de la "
                    "fusion ne trouvera rien. Vérifier avant de fusionner.",
                    file=sys.stderr,
                )
            print(
                f"  ✓ {glb.name} ({renommes} os renommés, "
                f"os le plus long {plus_long:.2f} m, "
                f"dérive racine retirée {derive:.2f} m)"
            )
            faits += 1
    finally:
        script.unlink(missing_ok=True)
    if faits == 0:
        raise Echec("aucune conversion réussie — vérifier le chemin de Blender.")
    return faits


# ─────────────────────────────────────────────────────────────────────────────


def main() -> int:
    p = argparse.ArgumentParser(description="Récolte les clips Mixamo déclarés.")
    p.add_argument("--clip", help="ne traiter qu'un clip (son `nom` au manifeste)")
    p.add_argument("--convertir", action="store_true", help="FBX → GLB via Blender")
    p.add_argument("--lister", action="store_true", help="afficher ce qui est déclaré")
    p.add_argument(
        "--chercher",
        help="explorer le catalogue sur un terme, sans rien télécharger. "
        "Sert quand un titre du manifeste ne colle pas : on voit ce qui existe "
        "vraiment au lieu de deviner le nom exact.",
    )
    p.add_argument("--blender", default="blender", help="chemin de l'exécutable Blender")
    p.add_argument("--fps", type=int, default=30)
    a = p.parse_args()

    try:
        if a.lister:
            for c in lire_manifeste(None):
                etat = "✓" if (SORTIE / f"{c['nom']}.glb").exists() else " "
                print(f"  [{etat}] {c['nom']:18s} ← « {c['recherche']} »")
            return 0

        if a.chercher:
            jeton, perso = lire_config()
            q = urllib.parse.urlencode(
                {"page": 1, "limit": 48, "order": "",
                 "type": "Motion,MotionPack", "query": a.chercher}
            )
            d = appel(f"{API}/products?{q}", jeton)
            res = d.get("results", [])
            print(f"{len(res)} résultat(s) pour « {a.chercher} » :")
            for r in res:
                print(f"    {r.get('name','?')}")
            if not res:
                print("  (rien — le terme est trop précis ou mal orthographié)")
            return 0

        if a.convertir:
            n = convertir(SORTIE, a.blender)
            print(f"\n{n} clip(s) converti(s) en GLB dans {SORTIE.relative_to(RACINE)}")
            print("Fusion : python tools/assets/merge_gltf_anims.py CORPS.glb "
                  f"{SORTIE.relative_to(RACINE)} SORTIE.glb")
            return 0

        jeton, perso = lire_config()
        clips = lire_manifeste(a.clip)
        obtenus, rates = 0, []
        for c in clips:
            nom, terme = c["nom"], c["recherche"]
            cible = SORTIE / f"{nom}.fbx"
            if cible.exists() or (SORTIE / f"{nom}.glb").exists():
                print(f"  = {nom} déjà là, ignoré")
                continue
            print(f"  → {nom} (« {terme} »)")
            try:
                produit = chercher(terme, jeton, perso)
                url = exporter(produit, jeton, perso, a.fps)
                ko = telecharger(url, cible) // 1024
                print(f"    ✓ {cible.name} ({ko} Ko)")
                obtenus += 1
            except Echec as e:
                print(f"    ✗ {e}", file=sys.stderr)
                rates.append(nom)
            time.sleep(PAUSE_ENTRE_CLIPS_S)

        print(f"\n{obtenus} clip(s) récolté(s), {len(rates)} échec(s)")
        if rates:
            print("échecs : " + ", ".join(rates), file=sys.stderr)
        # Zéro obtenu ET zéro déjà là = panne, pas succès. Un script d'asset qui
        # rend 0 en silence laisse croire que le dossier est complet.
        if obtenus == 0 and rates:
            return 1
        if obtenus:
            print("Convertir ensuite : python tools/assets/mixamo_recolte.py --convertir")
        return 1 if rates else 0

    except Echec as e:
        print(f"\nÉCHEC : {e}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
