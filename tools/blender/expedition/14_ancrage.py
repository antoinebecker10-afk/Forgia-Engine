"""Cuit l'ancrage, le dégradé et la cavité dans les COULEURS DE SOMMET.

À lancer APRÈS `vallon.py`, AVANT `92_cellules.py` / `91_export.py`.

  python tools/blender/bmcp.py code tools/blender/expedition/vallon.py
  python tools/blender/bmcp.py code tools/blender/expedition/14_ancrage.py
  python tools/blender/bmcp.py code tools/blender/expedition/92_cellules.py

# Pourquoi ce script existe

Les props du Vallon sont des APLATS : 30 des 37 matériaux n'ont aucune texture,
héritage direct du kit low-poly. Un aplat n'a aucune variation de valeur, et
c'est ça — pas la palette, qui est bonne — qui donne l'impression d'autocollants
posés sur un sol. Les trois cuissons ci-dessous rendent la valeur qui manque,
sans un seul shader et sans toucher aux couleurs de l'auteur.

# 🚨 LE DÉFAUT QUE CE SCRIPT CORRIGE D'ABORD

Mesuré avant d'écrire une ligne : **zéro primitive des 48 cellules ne porte
`COLOR_0`**. Le terrain a pourtant son attribut `Col` (22 176 sommets), rempli
par `vallon.py` avec le brouillage de teinte aux frontières de matière — un
travail soigné qui n'a jamais atteint le jeu.

La cause n'est pas l'option d'export (`export_vertex_color="ACTIVE"` est bien
valide en Blender 4.5.10, vérifié) : c'est le `join()`. Sur 117 maillages, **3
seulement** portaient un attribut de couleur. Quand la fusion d'une cellule prend
pour objet actif un prop qui n'en a pas, l'attribut des autres est perdu, et
l'export n'a plus rien à écrire.

D'où l'étape 0 : **donner son `Col` à TOUT LE MONDE**, blanc par défaut. Blanc
est neutre — Bevy multiplie `COLOR_0` par `base_color`, donc 1,0 ne change rien.
C'est ce qui rend la fusion homogène, et l'attribut survit quel que soit l'objet
actif.

# Ce qui passe le glTF, et pourquoi on s'en contente

`COLOR_0` est l'un des rares canaux que le format transporte (avec la couleur de
base, le métallique/rugosité, la normale, l'émissif et l'alpha). Bevy le
multiplie par `base_color`. Il n'y a donc RIEN à câbler côté moteur, et ça se
compose avec le grain de `matiere.rs` au lieu de le concurrencer : lui module la
matière, celui-ci module la lumière.

# La règle qui interdit d'assombrir toute la carte

La teinte finale vaut `COLOR_0 × baseColorFactor`. Une cuisson centrée sur 0,5
assombrirait le décor de moitié. **Toutes les cuissons ci-dessous partent de 1,0
et ne font que DESCENDRE** — même piège que celui déjà payé dans `matiere.rs`,
où un octet 127 valait 0,21 en linéaire et assombrissait le décor de 79 %.
"""

import json
import math
import os

import bpy
from mathutils import Vector
from mathutils.kdtree import KDTree

RACINE = r"C:\Users\Antoi\Desktop\Forgia Rewrite"
MANIFESTE = os.path.join(RACINE, "assets", "models", "environment", "expedition",
                         "expedition_vallon.json")

# ---------------------------------------------------------------------------
# Réglages — chacun avec sa raison, aucun choisi au hasard
# ---------------------------------------------------------------------------

SPEC = {
    # --- 1. ancrage au sol ---------------------------------------------
    # Un objet posé sans contact flotte. Une ombre portée coûte à l'exécution ;
    # un assombrissement du SOL sous l'objet ne coûte rien et fait le même
    # travail perceptif — c'est la vieille recette du « contact shadow » peint.
    # 0,68 : assez pour lire le contact, pas assez pour faire une flaque noire.
    "ancrage_force": 0.68,
    # Débord au-delà du rayon de l'objet. 1,8 m : l'ombre d'un tronc au sol est
    # plus large que le tronc, sinon on voit un cerne net au lieu d'un contact.
    "ancrage_marge_m": 1.8,
    # Au-delà du rayon de l'objet, l'assombrissement s'éteint en douceur.
    # Un seuil net produirait un disque, donc un défaut de plus.
    #
    # 🚨 PLANCHER — mesuré, pas prudentiel. La première cuisson MULTIPLIAIT les
    # contributions : en futaie, six arbres couvrent le même sommet de terrain
    # et 0,68⁶ vaut 0,10. Le contrôle a rendu `valeur_min: 0,084`, soit un sol
    # de sous-bois à 8 % de luminosité — un trou noir, pas une ombre.
    # Deux corrections : on retient la contribution la PLUS SOMBRE au lieu de
    # les composer (deux ombres qui se recouvrent n'assombrissent pas deux
    # fois), et on borne le résultat.
    "ancrage_plancher": 0.55,

    # --- 2. dégradé de hauteur ------------------------------------------
    # Un arbre en aplat n'a aucun volume. Sombre au pied du houppier, clair au
    # sommet : c'est ce que fait la lumière du ciel, et ça donne du volume sans
    # un polygone de plus.
    # 0,62 au pied — plus bas, les troncs virent au noir en sous-bois.
    "gradient_bas": 0.62,
    # Les pièces BASSES n'ont pas de dégradé : sur 40 cm de haut il n'y a pas de
    # « haut » et de « bas », seulement du bruit. Au-delà de ce seuil seulement.
    "gradient_hauteur_min_m": 1.2,

    # --- 3. cavité --------------------------------------------------------
    # Occlusion approchée par la DENSITÉ DE VOISINS : un sommet entouré de
    # beaucoup de géométrie proche est enfoui (creux de rocher, intérieur d'un
    # houppier), un sommet isolé est exposé. C'est grossier, et c'est très
    # au-dessus de rien sur des pièces facettées.
    # Une AO par lancer de rayons demanderait Cycles, un dépliage UV et des
    # minutes par pièce, pour un gain invisible sur des aplats à 200 faces.
    "cavite_min": 0.78,
    # Rayon de voisinage, en fraction de la diagonale de la pièce. 0,16 : assez
    # large pour lire un creux, assez court pour ne pas voir toute la pièce.
    "cavite_rayon_frac": 0.16,
    # Densité au-delà de laquelle on considère le sommet complètement enfoui,
    # en fraction du nombre de sommets de la pièce.
    "cavite_sature_frac": 0.14,
}

# Ces objets ne reçoivent AUCUNE cuisson. L'eau porte son propre matériau
# (quasi miroir, rugosité 0,08) : l'assombrir tuerait le reflet qui la rend
# lisible, et une nappe n'a ni cavité ni contact au sol.
JAMAIS = {"riviere", "vallon_eau"}
# Les collections qui ne sortent pas de Blender : inutile de les cuire.
IGNORE_COLL = {"faune_controle", "faune_apercu", "_proto", "_src"}

BLANC = (1.0, 1.0, 1.0, 1.0)

# Matières qui ondulent au vent. Le kit les distingue déjà : `leafs*` pour le
# feuillage, `grass*` / `plant*` / `flower*` pour le sous-bois. Le bois et la
# pierre restent rigides — un tronc qui ondule est pire que pas de vent.
MATIERES_SOUPLES = ("leafs", "grass", "plant", "flower")

# 🚨 LE CANAL DE RAIDEUR : L'ALPHA DE `COLOR_0`, ET IL EST BIEN LIBRE.
#
# J'avais conclu l'inverse hier — « Bevy multiplie base_color par COLOR_0 alpha
# comprise, donc y loger un masque rendrait le feuillage translucide » — et
# j'allais faire cuire un second jeu d'UV pour rien.
#
# La source dit le contraire, noir sur blanc
# (`bevy_pbr-0.18.1/src/render/pbr_functions.wgsl:100-103`) :
#
#     if alpha_mode == ...ALPHA_MODE_OPAQUE {
#         // NOTE: If rendering as opaque, alpha should be ignored so set to 1.0
#         color.a = 1.0;
#     }
#
# Les matériaux du kit sont opaques (un glTF sans `alphaMode` vaut OPAQUE).
# Leur alpha n'est donc jamais lue par le rendu : le canal est disponible, il
# est déjà exporté (COLOR_0 est en VEC4), et il ne coûte pas un octet de plus.
#
# 0 = rigide (le pied, le tronc), 1 = libre (la pointe des feuilles).
RAIDEUR_RIGIDE = 0.0


def couleurs_de(mesh):
    """Rend l'attribut `Col` du maillage, en le CRÉANT blanc s'il manque.

    Blanc et non noir : Bevy multiplie, donc 1,0 est le neutre. Un attribut
    initialisé à 0 rendrait la pièce noire — et ce serait mis sur le compte de
    l'éclairage, pas de ce script.
    """
    att = mesh.color_attributes.get("Col")
    if att is None:
        att = mesh.color_attributes.new(name="Col", type="FLOAT_COLOR", domain="POINT")
        for d in att.data:
            d.color = BLANC
    # Il doit être l'attribut ACTIF et celui DE RENDU : l'export en « ACTIVE »
    # ne regarde que celui-là.
    idx = mesh.color_attributes.find("Col")
    if idx >= 0:
        mesh.color_attributes.active_color_index = idx
        mesh.color_attributes.render_color_index = idx
    return att


def multiplier(att, indice, facteur):
    """N'écrase jamais : module. Le terrain porte déjà sa variation de teinte,
    calculée par `vallon.py` — la remplacer perdrait un travail qui est juste."""
    c = att.data[indice].color
    att.data[indice].color = (c[0] * facteur, c[1] * facteur, c[2] * facteur, c[3])


def objets_a_cuire():
    for obj in bpy.data.objects:
        if obj.type != "MESH" or obj.name.split(".")[0] in JAMAIS:
            continue
        if any(c.name in IGNORE_COLL for c in obj.users_collection):
            continue
        yield obj


MARQUEUR = "vallon_ancrage_cuit"


def main():
    rapport = {}

    # UN CUISEUR NE CUIT PAS DEUX FOIS. Ce script MULTIPLIE dans un attribut
    # existant : le relancer sur une scène déjà cuite composerait une seconde
    # fois (0,62² = 0,38 au pied des arbres) et le décor s'assombrirait
    # « tout seul », sans erreur, à chaque exécution. Le symptôme — « c'est
    # devenu sombre » — n'évoque pas une double cuisson.
    if bpy.context.scene.get(MARQUEUR):
        print("RESULT: " + json.dumps({
            "refus": "scene deja cuite",
            "remede": "relancer vallon.py pour repartir d'une scene neuve",
        }, ensure_ascii=False))
        return

    # -- étape 0 : l'attribut pour TOUS ------------------------------------
    # Sans ça, le `join()` des cellules le perd et l'export n'écrit rien.
    meshes = {}
    for obj in objets_a_cuire():
        meshes[obj.data.name] = obj.data
    for mesh in meshes.values():
        couleurs_de(mesh)
    rapport["meshes_avec_col"] = len(meshes)

    # -- étape 1 : dégradé de hauteur + cavité, SUR LES PROTOTYPES ---------
    # Les 1 616 props partagent une soixantaine de datablocks : cuire le
    # datablock donne la variation à toutes ses instances pour le prix d'une.
    # C'est ce qui rend cette passe tenable — la cuire par instance
    # demanderait de rendre 6 000 maillages mono-utilisateurs, donc d'exploser
    # la mémoire ET de perdre l'instanciation.
    gradients = cavites = 0
    for nom, mesh in meshes.items():
        if nom.startswith(("terrain", "chemin", "riviere", "pont")):
            continue          # le sol se traite à l'étape 2
        sommets = mesh.vertices
        if len(sommets) < 4:
            continue
        att = couleurs_de(mesh)

        zs = [v.co.z for v in sommets]
        z0, z1 = min(zs), max(zs)
        haut = z1 - z0

        bornes = [(min(v.co[a] for v in sommets), max(v.co[a] for v in sommets))
                  for a in range(3)]
        diagonale = math.sqrt(sum((h - b) ** 2 for b, h in bornes)) or 1.0

        # Cavité : un KD-tree par pièce. Les pièces du kit font 100 à 3 000
        # sommets, donc c'est instantané — et O(n log n) au lieu du O(n²) d'une
        # comparaison naïve, qui aurait mis des minutes sur les grosses pièces.
        kd = KDTree(len(sommets))
        for i, v in enumerate(sommets):
            kd.insert(v.co, i)
        kd.balance()
        rayon = diagonale * SPEC["cavite_rayon_frac"]
        sature = max(3.0, len(sommets) * SPEC["cavite_sature_frac"])

        for i, v in enumerate(sommets):
            f = 1.0
            if haut > SPEC["gradient_hauteur_min_m"]:
                t = (v.co.z - z0) / haut
                f *= SPEC["gradient_bas"] + (1.0 - SPEC["gradient_bas"]) * t
                gradients += 1
            voisins = len(kd.find_range(v.co, rayon))
            occ = min(1.0, voisins / sature)
            f *= 1.0 - (1.0 - SPEC["cavite_min"]) * occ
            cavites += 1
            multiplier(att, i, f)

    # -- étape 1 bis : la RAIDEUR au vent, dans l'alpha ------------------
    # Le vent est du travail moteur (aucun shader ne passe le glTF), mais son
    # MASQUE se cuit ici — et il ne coûte rien : l'alpha de `COLOR_0` est déjà
    # exportée et jamais lue en rendu opaque (cf. l'en-tête).
    #
    # La raideur se dérive de la position du sommet DANS SA PIÈCE : le pied ne
    # bouge pas, la pointe bouge le plus. Au carré, parce qu'une tige fléchit en
    # console — le déplacement croît plus vite que la hauteur, il n'est pas
    # proportionnel.
    #
    # Elle ne s'applique qu'aux matières souples : un tronc ou un rocher qui
    # ondule est bien pire que pas de vent du tout.
    souples = rigides = 0
    for nom, mesh in meshes.items():
        att = mesh.color_attributes.get("Col")
        if att is None or not mesh.vertices:
            continue
        noms = [m.name.split(".")[0] if m else "" for m in mesh.materials]
        idx_souples = {i for i, n in enumerate(noms)
                       if any(n.startswith(p) for p in MATIERES_SOUPLES)}
        sommets_souples = set()
        for poly in mesh.polygons:
            if poly.material_index in idx_souples:
                sommets_souples.update(poly.vertices)
        zs = [v.co.z for v in mesh.vertices]
        z0, z1 = min(zs), max(zs)
        haut = (z1 - z0) or 1.0
        for i, v in enumerate(mesh.vertices):
            c = att.data[i].color
            if i in sommets_souples:
                t = (v.co.z - z0) / haut
                att.data[i].color = (c[0], c[1], c[2], min(1.0, t * t))
                souples += 1
            else:
                att.data[i].color = (c[0], c[1], c[2], RAIDEUR_RIGIDE)
                rigides += 1
    rapport["raideur"] = {"sommets_souples": souples, "sommets_rigides": rigides}

    rapport["sommets_gradient"] = gradients
    rapport["sommets_cavite"] = cavites

    # -- étape 2 : ancrage au sol -----------------------------------------
    # Le contact fait plus pour l'intégration d'un objet qu'une ombre portée, et
    # il est gratuit à l'exécution. Les positions viennent du MANIFESTE que
    # `vallon.py` vient d'écrire : une seule source, déjà mesurée. Les relire
    # dans la scène serait une seconde mesure de la même grandeur.
    #
    # LE CHEMIN AUSSI, et c'est un correctif à part entière. Il en était exclu
    # alors qu'il court sur 358 m au milieu du décor : les arbres de bordure
    # posaient leur contact sur l'herbe et s'arrêtaient NET au bord du pavé, ce
    # qui SOULIGNAIT la lisière au lieu de la fondre. Deux axes d'analyse ont
    # proposé ce correctif le même jour sur deux sites différents ; c'est
    # celui-ci qui est le bon — l'étape 1 saute le chemin délibérément (cavité
    # et dégradé de hauteur n'ont aucun sens sur une nappe), l'étape 2 est
    # faite pour lui.
    solides = {}
    if os.path.exists(MANIFESTE):
        with open(MANIFESTE, encoding="utf-8") as fh:
            solides = json.load(fh).get("colliders_prop_xyzhr", {})

    def ancrer(obj):
        """Assombrit les sommets d'une nappe sous les pièces solides posées dessus."""
        if obj is None or not solides:
            return None
        att = couleurs_de(obj.data)
        avant = min(d.color[0] for d in att.data)
        mw = obj.matrix_world
        verts = obj.data.vertices
        kd = KDTree(len(verts))
        for i, v in enumerate(verts):
            pv = mw @ v.co
            kd.insert(Vector((pv.x, pv.y, 0.0)), i)
        kd.balance()

        # On COLLECTE d'abord la contribution la plus sombre par sommet, on
        # applique ensuite — une seule fois. Appliquer dans la boucle
        # composerait les recouvrements, et c'est ce qui avait produit un sol de
        # sous-bois à 8 % de luminosité.
        le_plus_sombre = {}
        poses = 0
        for _famille, pieces in solides.items():
            for x, y, _z, _h, r in pieces:
                poses += 1
                portee = r + SPEC["ancrage_marge_m"]
                for _co, idx, dist in kd.find_range(Vector((x, y, 0.0)), portee):
                    # Plein assombrissement sous la pièce, extinction douce
                    # jusqu'au bord : un seuil net dessinerait un disque, donc
                    # un défaut de plus au lieu d'un contact.
                    t = 0.0 if dist <= r else (dist - r) / max(1e-6, portee - r)
                    f = SPEC["ancrage_force"] + (1.0 - SPEC["ancrage_force"]) * (t * t)
                    if f < le_plus_sombre.get(idx, 1.0):
                        le_plus_sombre[idx] = f
        for idx, f in le_plus_sombre.items():
            multiplier(att, idx, max(SPEC["ancrage_plancher"], f))
        return {
            "sommets": len(verts),
            "assombris": len(le_plus_sombre),
            "part_pct": round(100.0 * len(le_plus_sombre) / max(1, len(verts)), 1),
            "min_avant": round(avant, 3),
        }

    rapport["ancrage"] = {
        "props": sum(len(v) for v in solides.values()),
        "recouvrements_absorbes": True,
        "nappes": {nom: ancrer(bpy.data.objects.get(nom))
                   for nom in ("terrain", "chemin")},
    }
    n_terr = rapport["ancrage"]["nappes"].get("terrain")
    rapport["terrain_min_avant"] = n_terr["min_avant"] if n_terr else 1.0

    # -- contrôle : aucune couleur ne doit avoir sombré -------------------
    # Un plancher trop bas ne se voit pas sur un rendu de jour et transforme les
    # sous-bois en trous noirs la nuit. On MESURE le minimum atteint au lieu de
    # supposer que les facteurs se sont bien composés.
    mini = 1.0
    somme = n = 0
    for mesh in meshes.values():
        att = mesh.color_attributes.get("Col")
        if att is None:
            continue
        for d in att.data:
            mini = min(mini, d.color[0])
            somme += d.color[0]
            n += 1
    rapport["valeur_min"] = round(mini, 3)
    rapport["valeur_moyenne"] = round(somme / max(1, n), 3)
    rapport["sommets_total"] = n
    # Le plancher théorique : gradient au pied x cavite pleine. Si le minimum
    # mesuré descend nettement en dessous, c'est qu'une contribution se compose
    # quelque part — exactement le defaut corrige a l'etape 2.
    rapport["plancher_theorique"] = round(min(
        SPEC["gradient_bas"] * SPEC["cavite_min"],
        SPEC["ancrage_plancher"] * rapport.get("terrain_min_avant", 1.0)), 3)
    rapport["coherent"] = mini >= rapport["plancher_theorique"] - 0.01

    bpy.context.scene[MARQUEUR] = True
    print("RESULT: " + json.dumps(rapport, ensure_ascii=False))


main()
