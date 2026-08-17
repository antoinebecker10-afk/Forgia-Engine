"""Sculpte une tête de CHIEN PERSONNIFIÉ au style de l'avatar d'Expédition.

    python tools/blender/bmcp.py code tools/blender/personnage/32_tete_chien.py

POURQUOI DES MÉTABALLES ET PAS UN GÉNÉRATEUR PARAMÉTRIQUE

Un générateur procédural produit une forme *déduite* : elle est cohérente et
sans intention. Une tête de personnage vit de l'inverse — des masses posées à la
main qui se fondent. Les métaballes sont l'outil de blocage du sculpteur : on
pose des volumes (crâne, museau, bajoues, mâchoire), ils fusionnent en surface
molle, et c'est cette fusion qui donne le galbe cartoon. Le maillage n'est
qu'une conséquence, jamais la donnée d'entrée.

CE QUI EST MESURÉ, ET DONC PAS CHOISI (relevé par `30_probe_tete.py`)

    tête humaine   0,226 L × 0,274 P × 0,323 H m     11 620 triangles
    corps          1,802 m  →  rapport tête/corps 1/5,6 (déjà cartoon)
    regard         -Y monde (relevé sur les ORTEILS, pas supposé)
    nez            0,120 m devant le centre de tête
    matière        `Organik`, mate, tout lissé (11 620/11 620 faces)

La tête de chien vise la MÊME hauteur : un museau s'ajoute en profondeur, pas en
volume général. Un chien à grosse tête ne serait plus le même personnage, ce
serait une mascotte.

L'AJUSTEMENT EST DÉRIVÉ, JAMAIS TAPÉ

Le rayon d'influence d'une métaballe ne donne pas la taille de la bosse qu'elle
produit. Plutôt que de tâtonner, le rapport a été MESURÉ au banc
(`FACTEUR_VISIBLE`), chaque masse déclare ses DEMI-EXTENTS voulus, et le script
en déduit `radius`/`size`. Puis la tête entière est remise à l'échelle sur la
hauteur cible mesurée. Aucun nombre de forme n'est corrigé à l'œil — cf.
`feedback_ancrages_nommes_pas_ajustement_a_l_oeil`.

De même les yeux, la truffe et la ligne de gueule ne sont pas posés à des
coordonnées devinées : on LANCE UN RAYON sur la surface sculptée et on s'y
accroche. Retoucher une masse du crâne les déplace donc tout seuls.
"""

import json
import math
import os

import bpy
import bmesh
import mathutils
from mathutils import Matrix, Vector

# Rapport entre le rayon d'influence d'une métaballe et l'emprise qu'elle rend
# VRAIMENT, mesuré au banc sur 5 cas (boules 0,10 et 0,20 m, ellipsoïdes iso et
# aplatis) : constant à 4 décimales. Il vaut pour seuil 0,6 / raideur 2.
FACTEUR_VISIBLE = 0.5747

RACINE = r"C:\Users\Antoi\Desktop\Forgia Rewrite"
CORPS = os.path.join(RACINE, "assets", "models", "characters", "stylized",
                     "stylized_male_complet.glb")
BAC = (r"C:\Users\Antoi\AppData\Local\Temp\claude"
       r"\c--Users-Antoi-Desktop-Forgia-Rewrite"
       r"\2269ac95-4478-478f-b077-660d6c666db7\scratchpad")

# --- Repère local de la tête -------------------------------------------------
# x = droite du personnage · y = DEVANT (le regard) · z = haut.
# Toute la sculpture est écrite là-dedans, en mètres. L'objet porte ensuite la
# matrice qui l'amène sur le corps : la sculpture ignore où est le corps.

SPEC = {
    # ⚠️ 0,27 et non 0,323 : la cible porte sur le CRÂNE seul. Les 0,323 de la
    # tête humaine incluent une grosse masse de cheveux hérissés ; ici ce sont
    # les oreilles semi-dressées qui reprennent cette part de silhouette. Viser
    # 0,323 sur le crâne donnerait une tête de mascotte sur un corps de héros.
    "hauteur_crane_cible_m": 0.295,

    # Où l'os `Head` tombe DANS le crâne, en unités d'auteur — donc : devant
    # (+y) et sous (−z) le centre du crâne, ce qui est la place d'une nuque.
    # Ce n'est pas un nombre choisi : il vient de la position que la tête
    # humaine occupait, relevée puis exprimée dans le repère de l'os — moins
    # son biais latéral, qui n'était que sa coiffure asymétrique. Le x est donc
    # à zéro par construction, et le chien est centré sur son propre cou.
    #
    # 🚨 Signe : la pose vaut `articulation − R·k·ancre`. Écrit à l'envers, il
    # met le crâne SUR L'ÉPAULE — vu au rendu, pas deviné.
    "ancre_os": (0.0, 0.068, -0.073),
    # ⚠️ Était à 13 000 pour que le bord du masque crème soit moins grossier —
    # il était alors trié à la face. Depuis qu'il est COUPÉ géométriquement, sa
    # netteté ne dépend plus de la densité : la raison du surcoût a disparu, le
    # surcoût doit disparaître avec elle. Une valeur dont le motif est mort est
    # une dette qui ne se voit pas.
    "triangles_cible": 6000,
    "resolution_metaballe": 0.005,  # plancher dur de Blender, mesuré

    # Les masses du blocage. `a` = demi-extents voulus (x, y, z), `p` = centre.
    # Un chien PERSONNIFIÉ, c'est un crâne qui domine et un museau court : le
    # rapport profondeur/hauteur reste sous 1,15. Au-delà, la silhouette bascule
    # vers l'animal à quatre pattes — mesuré à 1,57 au premier jet, refusé.
    "masses": [
        {"nom": "crane",     "p": (0.000, -0.014, 0.025), "a": (0.108, 0.092, 0.112)},
        {"nom": "nuque",     "p": (0.000, -0.050, -0.030), "a": (0.086, 0.058, 0.072)},
        {"nom": "front",     "p": (0.000, 0.045, 0.060), "a": (0.088, 0.055, 0.062)},
        {"nom": "museau_bas", "p": (0.000, 0.058, -0.030), "a": (0.060, 0.055, 0.050)},
        {"nom": "museau",    "p": (0.000, 0.092, -0.040), "a": (0.046, 0.042, 0.040)},
        {"nom": "bout",      "p": (0.000, 0.112, -0.033), "a": (0.038, 0.028, 0.034)},
        {"nom": "joue_g",    "p": (-0.072, 0.030, -0.038), "a": (0.040, 0.052, 0.045)},
        {"nom": "joue_d",    "p": (0.072, 0.030, -0.038), "a": (0.040, 0.052, 0.045)},
        {"nom": "machoire",  "p": (0.000, 0.082, -0.075), "a": (0.048, 0.052, 0.030)},
        # Les arcades : deux bosses au-dessus des yeux. Elles donnent au regard
        # une assise ; sans elles les billes sont posées sur une sphère.
        {"nom": "arcade_g",  "p": (-0.050, 0.058, 0.062), "a": (0.030, 0.026, 0.020)},
        {"nom": "arcade_d",  "p": (0.050, 0.058, 0.062), "a": (0.030, 0.026, 0.020)},
        # 🚨 Masse NÉGATIVE — elle creuse au lieu de gonfler. C'est le « stop »,
        # la cassure entre le front et le museau : le trait qui sépare un chien
        # d'une boule à museau. Aucune masse positive ne peut le produire, on ne
        # fabrique pas un creux en ajoutant de la matière.
        {"nom": "stop", "negatif": True,
         "p": (0.000, 0.052, 0.020), "a": (0.052, 0.030, 0.026)},
    ],

    # Le cou. Sans lui la tête flotte au-dessus du col : la tête humaine
    # portait le cou dans son maillage, la masquer l'a emporté avec elle.
    #
    # 🚨 Son TRAJET ne s'écrit pas — il se lit sur les os `Head`, `Neck` et
    # `Spine2`. Première version écrite à la main : sommet à 137 mm de l'attache
    # osseuse (84 mm trop en arrière, 107 mm trop haut) et penché de 3° là où le
    # squelette penche de 23°. Le cou sortait de l'arrière du crâne. Seules les
    # ÉPAISSEURS restent déclarées : ce sont des choix de forme, pas des
    # positions, et rien dans le squelette ne les donne.
    "cou": {
        "enfoncement_crane": 0.060,   # remontée dans le crâne, le long de l'axe
        "descente_poitrine": 0.55,    # part du trajet Neck→Spine2 parcourue
        "cambrure": 0.012,            # léger arrondi vers l'avant, à mi-hauteur
        "demi_largeurs": [0.056, 0.047, 0.048, 0.062],
        "demi_epaisseurs": [0.052, 0.045, 0.046, 0.060],
        "sections": 12, "cotes": 12,
    },

    # Oreille : cinq points de contrôle POSÉS, pas une formule. Elle part du
    # haut du crâne, s'écarte, puis PEND le long de la joue avec la pointe
    # légèrement ramenée devant.
    #
    # 🚨 Première version semi-dressée : elle traversait le crâne par le dessus
    # et lisait « renard ». Le pli ne se décrète pas dans un commentaire, il se
    # voit au rendu — c'est pour ça que ce script rend ses vues lui-même.
    "oreille": {
        # Reculé de 12 mm par rapport au premier jet : l'attache tombait au
        # niveau de l'œil, l'oreille partait donc du visage et non du crâne.
        "points": [
            (0.076, -0.020, 0.092),
            (0.116, -0.008, 0.062),
            (0.126, 0.008, -0.004),
            (0.118, 0.036, -0.062),
            (0.102, 0.064, -0.098),
        ],
        # ⚠️ À 0,062 de demi-largeur l'oreille masquait TOUT le flanc du crâne :
        # de profil la tête devenait une masse lisse sans lecture. Anatomiquement
        # défendable (un beagle est ainsi), visuellement raté — et c'est le rendu
        # qui tranche, pas l'anatomie.
        "demi_largeurs": [0.034, 0.046, 0.048, 0.038, 0.010],
        "demi_epaisseurs": [0.015, 0.012, 0.010, 0.007, 0.004],
        "sections": 16,
        "cotes": 10,
        # 🎨 Une oreille n'est pas la copie de l'autre. La symétrie parfaite est
        # ce qui trahit le plus une tête générée ; quelques degrés d'écart
        # suffisent à la faire lire comme dessinée.
        "asymetrie_deg": 7.0,
        "asymetrie_pointe": 0.012,
        # La pointe de l'oreille prend la couleur de la selle, sur cette part
        # finale du balayage.
        "part_pointe_sombre": 0.34,
        # Le pavillon : le même chemin ramené vers le crâne, plus court.
        "pavillon": {"retrait": 0.011, "longueur": 0.86,
                     "part_largeur": 0.66, "part_epaisseur": 0.42},
    },

    # Les mèches ciselées — le trait de style de l'avatar, transposé. Chacune
    # est un chemin à pointe franche, aplati contre la surface qu'elle borde.
    "touffes": [
        # ⚠️ Une mèche est un ACCENT. Première version 2× trop large : en profil
        # les trois mèches de nuque se rejoignaient en une coque lisse — donc
        # exactement la surface ronde qu'elles devaient casser. Le geste s'était
        # annulé lui-même.
        {"nom": "sourcil", "miroir": True, "plat": (0.15, -0.30, 1.0),
         "points": [(0.028, 0.058, 0.092), (0.050, 0.056, 0.102),
                    (0.072, 0.042, 0.106), (0.090, 0.020, 0.100)],
         "demi_largeurs": [0.008, 0.011, 0.009, 0.002],
         "demi_epaisseurs": [0.006, 0.007, 0.005, 0.002],
         "sections": 8, "cotes": 6},
        {"nom": "nuque_haut", "miroir": True, "plat": (0.55, 0.0, 1.0),
         "points": [(0.026, -0.014, 0.116), (0.040, -0.056, 0.108),
                    (0.048, -0.092, 0.082), (0.046, -0.116, 0.048)],
         "demi_largeurs": [0.013, 0.016, 0.012, 0.002],
         "demi_epaisseurs": [0.008, 0.009, 0.007, 0.002],
         "sections": 10, "cotes": 6},
        {"nom": "nuque_bas", "miroir": False, "plat": (1.0, 0.0, 0.15),
         "points": [(0.0, -0.034, 0.070), (0.0, -0.080, 0.044),
                    (0.0, -0.106, 0.006), (0.0, -0.108, -0.032)],
         "demi_largeurs": [0.016, 0.019, 0.014, 0.002],
         "demi_epaisseurs": [0.009, 0.010, 0.007, 0.002],
         "sections": 12, "cotes": 8},
        {"nom": "bajoue", "miroir": True, "plat": (0.30, 0.30, 1.0),
         "points": [(0.062, 0.066, -0.058), (0.078, 0.036, -0.082),
                    (0.084, 0.002, -0.098), (0.080, -0.028, -0.102)],
         "demi_largeurs": [0.010, 0.013, 0.010, 0.002],
         "demi_epaisseurs": [0.007, 0.008, 0.006, 0.002],
         "sections": 10, "cotes": 8},
    ],

    "oeil": {
        # 🎨 +16 % et descendu : l'appeal d'un animal cartoon tient à la
        # NÉOTÉNIE — de gros yeux BAS sur le visage, un museau court, un grand
        # crâne. Le crâne et le museau y étaient ; les yeux, petits et hauts,
        # vieillissaient la tête.
        "rayon": 0.0400,
        # Le point de peau visé, et la direction depuis laquelle on le vise.
        # Un rayon lancé « vers l'origine » sortirait par le museau : mesuré au
        # premier jet, les yeux atterrissaient à y = 0,155, en plein sur la
        # truffe. Une ancre nommée coûte deux nombres et supprime la classe.
        "ancre": (0.052, 0.072, 0.028),
        "cap": (0.30, 1.0, 0.14),
        "saillie": 0.52,           # fraction de la bille qui dépasse du poil
        "cone_iris_deg": 33.0,
        "cone_limbe_deg": 38.0,     # l'anneau sombre, juste au-delà de l'iris
        "cone_pupille_deg": 14.0,
        "segments": 24,
        # Éclat blanc : deux degrés de plus de « vivant » pour une sphère de
        # 5 mm. Posé en haut-dedans, comme le fait tout portrait peint.
        # Deux reflets, comme tout portrait peint : un gros en haut-dedans et
        # un petit en bas-dehors. Un point unique fait œil de verre.
        "eclat_rayon": 0.0082,
        "eclat_cap": (-0.46, 1.0, 0.70),
        "eclat2_rayon": 0.0040,
        "eclat2_cap": (0.62, 1.0, -0.52),
        # Paupière : calotte de poil au-dessus de la bille, coupée par un plan.
        "paupiere_jeu": 1.06,      # rayon, en fraction de celui de l'œil
        # Hauteur du plan de coupe (sinus) — donc la TAILLE de la calotte :
        # rayon angulaire = 90° − asin(bord). 0,30 → 72,5°.
        #
        # 🚨 Deux régressions opposées sur ce seul nombre. À 0,12 la paupière
        # descendait sous la pupille, œil ouvert : chien endormi. À 0,20, même
        # défaut en plus discret — c'est ce qu'a produit la « correction » du
        # liseré de blanc. Le liseré ne venait PAS d'une calotte trop petite,
        # mais du dépassement de rotation qui poussait son pôle au-delà du
        # regard et faisait reculer son bord opposé. On règle donc la fermeture
        # côté rotation, et la taille reste celle qui va à l'œil OUVERT.
        "paupiere_bord": 0.30,
        "paupiere_incl_deg": 12.0,  # inclinaison vers l'extérieur = regard doux
    },

    "truffe": {"a": (0.030, 0.022, 0.024), "enfoncement": 0.010, "z": -0.008,
               "narine_a": (0.0068, 0.0060, 0.0092), "narine_ecart": 0.0138,
               "narine_avance": 0.86, "narine_hauteur": -0.0035},

    # Gueule : arc REMONTANT aux commissures. Une bande horizontale se lit comme
    # une sangle, pas comme une bouche — constaté au premier rendu.
    "gueule": {
        # Demi-angle de l'arc. À 64° la ligne sortait du museau crème et
        # continuait sur la joue — une bouche qui dépasse du visage.
        "ouverture_deg": 52.0,
        "pivot": (0.0, 0.020, -0.062),
        "remontee": 0.020,         # hauteur gagnée aux extrémités : le sourire
        # 🎨 Le ruban S'AFFINE aux commissures. D'épaisseur constante, il se
        # lisait comme un trait de feutre posé sur le museau.
        "demi_hauteur": 0.0052,
        "affinement": 0.72,        # part d'épaisseur perdue au bout de l'arc
        # Le philtrum : la rainure verticale de la truffe à la lèvre. C'est LE
        # détail qui fait lire un museau ; sans lui la truffe est un bouton.
        "philtrum_demi_largeur": 0.0034,
        "philtrum_haut": -0.006,
        "decollement": 0.0016,
        "echantillons": 25,
    },

    # Taches. Elles ne sont pas peintes : on affecte une seconde matière aux
    # faces qui tombent dans la zone. Même recette que l'ourlet ardent de la
    # cape (`26_cape_ardente.py`) — une texture ne serait pas transportée par
    # un maillage issu de métaballes, qui n'a aucune UV utile.
    # 🚨 Deux versions ont échoué avant celle-ci, pour la MÊME raison de fond :
    # trier des faces existantes ne peut pas produire un bord plus fin qu'une
    # face. D'abord deux inégalités (bord en dents de scie), puis une distance à
    # un point (contour rond, mais toujours en escalier à l'échelle du triangle,
    # et parfaitement visible au rendu). La coupe est donc GÉOMÉTRIQUE : des
    # sphères tranchent la surface, et le bord est exact par construction.
    # 🎨 Les marques. Chacune est une COUPE géométrique : des sphères qui
    # tranchent la surface, donc un bord exact. Elles sont maintenant BRUITÉES
    # — un bord parfaitement lisse se lisait comme une cagoule enfilée, pas
    # comme de la fourrure. Même recette que l'ourlet ardent de la cape.
    "marques": [
        {"nom": "creme", "matiere": "creme", "bruit": 0.09, "echelle_bruit": 7.0,
         "spheres": [((0.0, 0.086, -0.034), 0.070),
                     ((0.050, 0.068, 0.064), 0.028),
                     ((-0.050, 0.068, 0.064), 0.028)]},
        # La selle : la calotte sombre du crâne. C'est elle qui donne à la tête
        # sa valeur basse et qui la détache du buste clair.
        {"nom": "selle", "matiere": "selle", "bruit": 0.11, "echelle_bruit": 5.0,
         "spheres": [((0.0, -0.048, 0.052), 0.108)]},
        # Le cerne d'orbite : un anneau sombre autour de chaque œil. Il creuse
        # le regard sans toucher à la géométrie.
        {"nom": "cerne", "matiere": "cerne", "bruit": 0.07, "echelle_bruit": 11.0,
         "spheres": [((0.052, 0.070, 0.030), 0.046),
                     ((-0.052, 0.070, 0.030), 0.046)]},
    ],
    "segments_coupe": 32,
    # En dessous, un morceau détaché est un confetti, pas une marque.
    "ilot_min_faces": 24,

    "couleurs": {
        # Pris DANS la palette du personnage : le brun du poil est celui de ses
        # cheveux, l'iris celui de ses yeux, la crème celle de sa tunique.
        # 🎨 Une hiérarchie de VALEURS, pas une gamme de bruns.
        # Le premier jet mettait le poil (#AE7442) à la même valeur que la
        # tunique crème du corps : la tête et le buste se fondaient, et l'œil
        # ne savait pas où se poser — sur les rendus, l'écharpe attirait plus le
        # regard que le visage. Un personnage se lit par sa tête.
        # Le poil descend donc en valeur ET monte en saturation.
        "poil": "#8F5626",
        "selle": "#5E3418",        # la selle sur le crâne et le dessus d'oreille
        "creme": "#F4EADA",
        "pavillon": "#7A4230",
        "truffe": "#221B1C",
        "gueule": "#4A3730",
        "blanc_oeil": "#F6F2E8",
        # Trois tons d'iris au lieu d'un : un dégradé haut-bas (l'ombre de la
        # paupière tombe dessus) et un ANNEAU LIMBIQUE sombre au bord. C'est ce
        # cerne, plus que la couleur, qui fait qu'un œil accroche.
        "iris_haut": "#5A3418",
        "iris_bas": "#B07736",
        "limbe": "#1E1208",
        "pupille": "#0B0708",
        "cerne": "#4A2A14",        # le pourtour d'orbite, dans le poil
        # Matière technique : elle ne sort jamais du script. Elle sert à
        # étiqueter les faces du couteau pour les retirer à coup sûr.
        "marqueur": "#FF00FF",
    },

    # 🚨 La rugosité n'est PAS une constante de personnage. Tout était à 0,72 —
    # la valeur du poil — y compris les yeux et la truffe. Un œil à 0,72 ne
    # réfléchit rien : il lit comme du plastique poncé, et c'est très exactement
    # ce qui fait qu'un personnage a l'air mort. Une cornée est presque un
    # miroir, une truffe de chien est humide.
    "rugosite": 0.72,          # défaut : le poil
    "rugosites": {
        "blanc_oeil": 0.10,
        "iris": 0.10,
        "pupille": 0.08,
        "truffe": 0.26,        # humide, mais pas vernie
        "gueule": 0.45,        # lèvre : entre la peau et le mucus
        "pavillon": 0.62,      # l'intérieur d'oreille est moins duveteux
        "iris_haut": 0.10, "iris_bas": 0.10, "limbe": 0.12,
        "selle": 0.72, "cerne": 0.70,
    },
}


# ---------------------------------------------------------------- utilitaires

def vider():
    """Vide la scène pour de bon.

    🚨 PAS `bpy.ops.object.select_all` : il ne sélectionne QUE ce qui est
    sélectionnable et visible. Tout objet masqué survit à la suppression — et
    ressort à l'import suivant sous un nom suffixé, pendant que le vrai objet
    attendu manque. Constaté deux fois : les icosphères de widget qui traînaient
    depuis le matin, puis l'armature de cape disparue de la scène alors que sa
    donnée subsistait, laissant `Cloak_low` sans parent et donc affichée à sa
    taille brute — cent fois trop grande.
    On supprime donc par la DONNÉE, où rien ne se cache."""
    for obj in list(bpy.data.objects):
        bpy.data.objects.remove(obj, do_unlink=True)
    for coll in (bpy.data.meshes, bpy.data.armatures, bpy.data.actions,
                 bpy.data.materials, bpy.data.images, bpy.data.collections,
                 bpy.data.cameras, bpy.data.lights, bpy.data.metaballs):
        for bloc in list(coll):
            try:
                coll.remove(bloc)
            except (RuntimeError, ReferenceError):
                pass


def cote_nom(cote):
    """`cote = +1` est le côté GAUCHE du personnage.

    Contre-intuitif, et assumé : c'est le prix du trièdre dextrogyre (voir le
    repère dans `main`). Un seul endroit décide, pour qu'aucun nom de pièce ne
    mente sur le côté qu'il occupe."""
    return "g" if cote > 0 else "d"


def srgb_lineaire(c):
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def hex_lineaire(code):
    code = code.lstrip("#")
    return tuple(srgb_lineaire(int(code[i:i + 2], 16) / 255.0)
                 for i in (0, 2, 4)) + (1.0,)


def matiere(nom, couleur_hex, rugosite=None):
    mat = bpy.data.materials.get(nom) or bpy.data.materials.new(nom)
    mat.use_nodes = True
    bsdf = next((n for n in mat.node_tree.nodes if n.type == "BSDF_PRINCIPLED"), None)
    if bsdf is None:
        return mat
    bsdf.inputs["Base Color"].default_value = hex_lineaire(couleur_hex)
    if "Roughness" in bsdf.inputs:
        bsdf.inputs["Roughness"].default_value = (
            SPEC["rugosite"] if rugosite is None else rugosite)
    if "Metallic" in bsdf.inputs:
        bsdf.inputs["Metallic"].default_value = 0.0
    return mat


def objet_depuis_bmesh(bm, nom, matrice, mat):
    mesh = bpy.data.meshes.new(nom)
    bm.to_mesh(mesh)
    bm.free()
    for poly in mesh.polygons:
        poly.use_smooth = True
    mesh.materials.append(mat)
    obj = bpy.data.objects.new(nom, mesh)
    obj.matrix_world = matrice
    bpy.context.scene.collection.objects.link(obj)
    return obj


def figer(obj):
    """Applique les modificateurs en relisant l'objet évalué.

    On passe par le depsgraph plutôt que par `modifier_apply` : l'opérateur
    exige un contexte de vue 3D, fragile à travers une socket."""
    dg = bpy.context.evaluated_depsgraph_get()
    mesh = bpy.data.meshes.new_from_object(obj.evaluated_get(dg))
    ancien = obj.data
    obj.modifiers.clear()
    obj.data = mesh
    try:
        bpy.data.meshes.remove(ancien)
    except (RuntimeError, ReferenceError):
        pass
    return obj


# ------------------------------------------------------------------ sculpture

def blocage(matrice):
    """Les masses fusionnées. C'est LE geste de sculpture : poser des volumes."""
    mb = bpy.data.metaballs.new("chien_masses")
    mb.resolution = SPEC["resolution_metaballe"]
    mb.render_resolution = SPEC["resolution_metaballe"]
    obj = bpy.data.objects.new("chien_masses", mb)
    bpy.context.scene.collection.objects.link(obj)
    obj.matrix_world = matrice

    # Loi MESURÉE au banc (5 cas, `probe_mball3`) et non déduite d'une doc :
    #
    #     demi-extent visible = FACTEUR · radius · size_axe
    #
    # avec FACTEUR = 0,5747 à seuil 0,6 / raideur 2. Le piège qu'elle corrige :
    # sur un ELLIPSOID, `size_*` DIVISE la distance, donc l'influence réelle
    # vaut radius × size. Un premier jet avec `size` en mètres a produit des
    # masses de 4 mm — invisibles à la résolution de grille, donc ZÉRO sommet
    # et pas la moindre erreur pour le dire.
    for masse in SPEC["masses"]:
        a = Vector(masse["a"])
        plus_grand = max(a)
        el = mb.elements.new(type="ELLIPSOID")
        el.co = Vector(masse["p"])
        el.radius = plus_grand / FACTEUR_VISIBLE
        el.size_x, el.size_y, el.size_z = (c / plus_grand for c in a)
        # Une masse négative CREUSE le champ au lieu de l'alimenter.
        el.use_negative = bool(masse.get("negatif"))
    return obj


def en_maillage(source, nom):
    """Convertit la métaballe en maillage.

    ⚠️ `meshes.new_from_object` rend un maillage VIDE sur une métaballe : la
    tessellation vit sur l'objet « base » de la famille et le depsgraph ne la
    livre pas par ce chemin. Mesuré ici, pas supposé — premier essai à 0 sommet.
    L'opérateur reste donc la seule voie, avec son contexte forcé à la main.

    ⚠️ Et `convert` DÉTRUIT l'objet métaballe pour en rendre un neuf : garder la
    référence Python donne « StructRNA has been removed » au premier accès. On
    relit donc par le NOM, jamais par la variable — même piège que les widgets
    d'os dans `25_atelier_personnage.py`."""
    nom_source = source.name
    connus = {o.name for o in bpy.context.scene.objects}

    bpy.ops.object.select_all(action="DESELECT")
    source.select_set(True)
    bpy.context.view_layer.objects.active = source
    with bpy.context.temp_override(active_object=source, object=source,
                                   selected_editable_objects=[source]):
        bpy.ops.object.convert(target="MESH")

    obj = bpy.data.objects.get(nom_source)
    if obj is None or obj.type != "MESH":
        neufs = [o for o in bpy.context.scene.objects
                 if o.type == "MESH" and o.name not in connus]
        obj = neufs[0] if neufs else None
    if obj is None:
        return None
    obj.name = nom
    obj.data.name = nom
    return obj


def facteur_calibrage(obj, cible):
    """Le rapport entre la hauteur SCULPTÉE et la hauteur voulue. Ne transforme
    RIEN — c'est tout l'intérêt.

    🚨 Première version : elle mettait le maillage du crâne à l'échelle. Les
    oreilles, l'ancre des yeux et le masque crème restaient, eux, en unités
    d'auteur — donc une même grandeur vivait dans DEUX repères, et le crâne a
    avalé ses propres oreilles. Le calibrage part maintenant dans la matrice de
    pose : la sculpture entière reste dans un seul repère, et il n'y a plus
    d'endroit où se tromper.

    Uniforme, aussi : une mise à l'échelle par axe réécrirait en douce les
    proportions de SPEC, qui ne dirait alors plus la vérité."""
    zs = [v.co.z for v in obj.data.vertices]
    haut = max(zs) - min(zs)
    return 1.0 if haut < 1e-6 else cible / haut


def alleger(obj, cible_tri):
    """Ramène la densité à celle de la tête d'origine, puis adoucit.

    Les métaballes sortent un maillage fin et irrégulier ; le style de l'avatar
    est fait de grandes surfaces molles, pas de micro-relief."""
    obj.data.calc_loop_triangles()
    avant = len(obj.data.loop_triangles)
    if avant > cible_tri:
        dec = obj.modifiers.new("alleger", "DECIMATE")
        dec.ratio = cible_tri / avant
    lisse = obj.modifiers.new("adoucir", "SMOOTH")
    lisse.factor, lisse.iterations = 0.6, 2
    figer(obj)
    obj.data.calc_loop_triangles()
    for poly in obj.data.polygons:
        poly.use_smooth = True
    return avant, len(obj.data.loop_triangles)


def spline(points, t):
    """Catmull-Rom : la courbe PASSE par les points posés.

    Une Bézier les effleurerait, et le sculpteur ne retrouverait pas la forme
    qu'il a placée. Ici les cinq points de `SPEC` sont la sculpture ; tout le
    reste n'est que remplissage entre eux."""
    n = len(points) - 1
    seg = min(int(t * n), n - 1)
    u = t * n - seg
    p0 = points[max(0, seg - 1)]
    p1, p2 = points[seg], points[seg + 1]
    p3 = points[min(n, seg + 2)]
    return 0.5 * ((2.0 * p1) + (-p0 + p2) * u
                  + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * u * u
                  + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * u * u * u)


def entre(valeurs, t):
    """Interpolation linéaire d'un profil scalaire le long du même paramètre."""
    n = len(valeurs) - 1
    seg = min(int(t * n), n - 1)
    u = t * n - seg
    return valeurs[seg] * (1.0 - u) + valeurs[seg + 1] * u


def balayage(ctrl, demi_largeurs, demi_epaisseurs, sections, cotes,
             plat=Vector((1.0, 0.0, 0.0))):
    """Balaye une section elliptique le long du chemin posé.

    Le geste commun à l'oreille et à la mèche : une forme aplatie qui suit une
    courbe et s'affine vers la pointe. `plat` donne l'axe de l'aplatissement —
    ce qui distingue une oreille (plaquée sur le côté du crâne) d'une mèche
    (aplatie contre la surface qu'elle borde)."""
    n, k = sections, cotes
    bm = bmesh.new()
    anneaux = []

    for i in range(n + 1):
        t = i / n
        centre = spline(ctrl, t)
        demi_l = entre(demi_largeurs, t)
        demi_e = entre(demi_epaisseurs, t)

        # Section perpendiculaire au chemin : l'épaisseur sur `plat`, la largeur
        # sur ce qui reste une fois la tangente et `plat` posés.
        avance = spline(ctrl, min(1.0, t + 1.0 / (2 * n)))
        recule = spline(ctrl, max(0.0, t - 1.0 / (2 * n)))
        tangente = avance - recule
        tangente = (tangente.normalized() if tangente.length > 1e-6
                    else Vector((0.0, 0.0, -1.0)))
        e2 = tangente.cross(plat)
        e2 = e2.normalized() if e2.length > 1e-6 else Vector((0.0, 1.0, 0.0))
        e1 = e2.cross(tangente).normalized()

        anneau = []
        for j in range(k):
            a = 2.0 * math.pi * j / k
            p = centre + e1 * (demi_e * math.cos(a)) + e2 * (demi_l * math.sin(a))
            anneau.append(bm.verts.new(p))
        anneaux.append(anneau)

    bm.verts.ensure_lookup_table()
    for i in range(len(anneaux) - 1):
        for j in range(k):
            bm.faces.new((anneaux[i][j], anneaux[i][(j + 1) % k],
                          anneaux[i + 1][(j + 1) % k], anneaux[i + 1][j]))
    bm.faces.new(list(reversed(anneaux[0])))   # bouchon de base, noyé dans le crâne
    bm.faces.new(anneaux[-1])                  # bouchon de pointe
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    return bm


def oreille(cote, matrice, mats):
    """L'oreille, plus son pavillon : deux balayages, le second rentré dedans.

    Sans pavillon, l'oreille est une palette de bois. C'est l'intérieur plus
    sombre qui la fait lire comme une oreille, même de loin."""
    o = SPEC["oreille"]
    ctrl = [Vector((cote * p[0], p[1], p[2])) for p in o["points"]]
    bm = balayage(ctrl, o["demi_largeurs"], o["demi_epaisseurs"],
                  o["sections"], o["cotes"])
    pieces = [objet_depuis_bmesh(bm, f"chien_oreille_{cote_nom(cote)}", matrice,
                                 mats["poil"])]

    # Le pavillon : même chemin, décalé vers l'INTÉRIEUR (donc vers le crâne),
    # plus court et plus étroit. Il affleure la face interne de l'oreille.
    creux = o["pavillon"]
    ctrl_p = [p - Vector((cote * creux["retrait"], 0.0, 0.0)) for p in ctrl]
    bm_p = balayage([spline(ctrl_p, t * creux["longueur"])
                     for t in (0.0, 0.25, 0.5, 0.75, 1.0)],
                    [w * creux["part_largeur"] for w in o["demi_largeurs"]],
                    [e * creux["part_epaisseur"] for e in o["demi_epaisseurs"]],
                    o["sections"], o["cotes"])
    pieces.append(objet_depuis_bmesh(bm_p, f"chien_pavillon_{cote_nom(cote)}",
                                     matrice, mats["pavillon"]))
    return pieces


def cou(matrice, mat, arm):
    """Le cou : un balayage conique posé SUR l'axe osseux, jamais à côté.

    ⚠️ Il n'était pas là parce que personne ne l'avait retiré : le cou vivait
    DANS le maillage `SM_Head`. Masquer la tête humaine l'a donc emporté avec
    elle, en silence. Une pièce absente parce qu'elle appartenait à une autre
    est la faute la plus discrète d'un remplacement.

    Le trajet vient des os `Head` → `Neck` → `Spine2`, ramenés dans le repère
    de la tête. C'est le squelette qui sait où passe un cou et de combien il
    penche ; l'écrire à la main revient à déclarer une deuxième fois une
    grandeur que le corps porte déjà."""
    c = SPEC["cou"]
    inv = matrice.inverted()

    def local(nom):
        bone = arm.pose.bones.get(nom)
        return None if bone is None else inv @ (arm.matrix_world @ bone.matrix.translation)

    tete, base, poitrine = local("Head"), local("Neck"), local("Spine2")
    if tete is None or base is None:
        return None
    axe = (tete - base).normalized()
    haut = tete + axe * c["enfoncement_crane"]
    bas = (base + (poitrine - base) * c["descente_poitrine"]
           if poitrine is not None else base - axe * 0.12)

    # Quatre points sur le segment, avec une légère cambrure vers l'avant au
    # milieu — une nuque parfaitement droite se lit comme un tuyau.
    ctrl = []
    for i, t in enumerate((0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0)):
        p = haut.lerp(bas, t)
        p.y += c["cambrure"] * math.sin(math.pi * t)
        ctrl.append(p)

    bm = balayage(ctrl, c["demi_largeurs"], c["demi_epaisseurs"],
                  c["sections"], c["cotes"])
    return objet_depuis_bmesh(bm, "chien_cou", matrice, mat)


def touffes(matrice, mat):
    """Les mèches ciselées.

    C'est LE trait de style de l'avatar : ses cheveux ne sont pas une masse
    lisse mais des mèches à pointe franche. Une tête de chien entièrement ronde
    peut être jolie et ne PAS appartenir au même jeu. Ces mèches sont ce qui la
    raccroche — sourcils, nuque, bajoues."""
    faites = []
    for touffe in SPEC["touffes"]:
        for cote in ((-1, 1) if touffe["miroir"] else (1,)):
            ctrl = [Vector((cote * p[0], p[1], p[2])) for p in touffe["points"]]
            bm = balayage(ctrl, touffe["demi_largeurs"], touffe["demi_epaisseurs"],
                          touffe["sections"], touffe["cotes"],
                          plat=Vector(touffe["plat"]).normalized())
            suffixe = f"_{cote_nom(cote)}" if touffe["miroir"] else ""
            faites.append(objet_depuis_bmesh(
                bm, f"chien_meche_{touffe['nom']}{suffixe}", matrice, mat))
    return faites


def surface(tete, origine, direction):
    """Le point de peau visé, et sa normale. Un rayon, pas une coordonnée."""
    ok, pos, nor, _ = tete.ray_cast(origine, direction)
    return (pos, nor) if ok else (None, None)


def oeil(cote, tete, matrice, mats):
    """Bille posée SUR la surface sculptée, zonée par l'angle au regard.

    Trois matières sur une même sphère : blanc, iris, pupille. Le découpage se
    fait sur l'angle entre la normale de face et le cap du regard — donc il
    survit à tout changement de forme, contrairement à une UV peinte.

    🚨 La sphère est TOURNÉE pour que son pôle regarde. Sans ça, le cône de
    l'iris coupe la grille UV en biais et la pupille sort CARRÉE — c'est
    exactement ce qu'a montré le premier rendu. Pôle aligné, les zones sont des
    anneaux concentriques et le bord est rond par construction."""
    o = SPEC["oeil"]
    cap = Vector((cote * o["cap"][0], o["cap"][1], o["cap"][2])).normalized()
    ancre = Vector((cote * o["ancre"][0], o["ancre"][1], o["ancre"][2]))
    pos, nor = surface(tete, ancre + cap * 0.35, -cap)
    if pos is None:
        return [], None
    centre = pos - nor.normalized() * (o["rayon"] * (1.0 - o["saillie"]))
    vers_cap = Vector((0.0, 0.0, 1.0)).rotation_difference(cap).to_matrix()

    bm = bmesh.new()
    bmesh.ops.create_uvsphere(bm, u_segments=o["segments"],
                              v_segments=o["segments"] // 2, radius=o["rayon"])
    bmesh.ops.rotate(bm, cent=Vector((0, 0, 0)), matrix=vers_cap, verts=bm.verts)
    bmesh.ops.translate(bm, verts=bm.verts, vec=centre)

    # 🎨 CINQ zones, du centre vers le bord : pupille · iris bas · iris haut ·
    # limbe · blanc.
    #
    # Le partage haut/bas ne suit PAS le rayon mais la verticale : c'est l'ombre
    # que la paupière projette sur l'iris, et elle vaut à elle seule la moitié
    # de la profondeur d'un regard. Le limbe — l'anneau sombre au bord de
    # l'iris — fait le reste : c'est lui, plus que la couleur, qui fait qu'un
    # œil accroche au lieu de flotter.
    #
    # 🚨 Les slots sont NOMMÉS, jamais comptés. Deux ajouts successifs de
    # matières ont décalé les index en silence, et la pupille se peignait avec
    # la couleur de l'iris haut. Un index littéral ne survit pas à une
    # insertion.
    ordre = ("blanc_oeil", "iris_bas", "iris_haut", "limbe", "pupille")
    obj = objet_depuis_bmesh(bm, f"chien_oeil_{cote_nom(cote)}", matrice,
                             mats[ordre[0]])
    for cle in ordre[1:]:
        obj.data.materials.append(mats[cle])
    slot = {cle: i for i, cle in enumerate(ordre)}

    seuil_pup = math.cos(math.radians(o["cone_pupille_deg"]))
    seuil_iris = math.cos(math.radians(o["cone_iris_deg"]))
    seuil_limbe = math.cos(math.radians(o["cone_limbe_deg"]))
    haut_monde = Vector((0.0, 0.0, 1.0))
    for poly in obj.data.polygons:
        direction = (poly.center - centre).normalized()
        d = direction.dot(cap)
        if d > seuil_pup:
            poly.material_index = slot["pupille"]
        elif d > seuil_iris:
            poly.material_index = (slot["iris_haut"]
                                   if direction.dot(haut_monde) > 0.0
                                   else slot["iris_bas"])
        elif d > seuil_limbe:
            poly.material_index = slot["limbe"]
        else:
            poly.material_index = slot["blanc_oeil"]

    # 🎨 DEUX reflets, comme tout portrait peint : un gros en haut-dedans, un
    # petit en bas-dehors. Un point unique donne l'œil de verre — c'est le
    # second, minuscule et à l'opposé, qui suggère un environnement autour.
    #
    # 🚨 L'enfoncement (0,85 du rayon) n'est pas cosmétique : à 0,45 la bille
    # dépassait de 1,1 mm alors que la paupière ne dégage que 2,1 mm de la
    # cornée — deux points blancs perçaient donc les paupières FERMÉES. Une
    # pièce qui affleure doit tenir sous celle qui passera devant.
    eclats = []
    for cle_cap, cle_rayon, suffixe in (("eclat_cap", "eclat_rayon", ""),
                                        ("eclat2_cap", "eclat2_rayon", "b")):
        e = Vector((cote * o[cle_cap][0], o[cle_cap][1],
                    o[cle_cap][2])).normalized()
        bmi = bmesh.new()
        bmesh.ops.create_uvsphere(bmi, u_segments=8, v_segments=4,
                                  radius=o[cle_rayon])
        bmesh.ops.translate(bmi, verts=bmi.verts,
                            vec=centre + e * (o["rayon"] - o[cle_rayon] * 0.85))
        eclats.append(objet_depuis_bmesh(
            bmi, f"chien_eclat{suffixe}_{cote_nom(cote)}", matrice,
            mats["blanc_oeil"]))

    # La paupière : une calotte de poil légèrement plus grande que la bille,
    # coupée par un plan. Sans elle l'œil est un globe POSÉ sur la joue — le
    # défaut « yeux de peluche » du premier jet. Son bord incliné vers
    # l'extérieur fait le regard doux ; incliné dans l'autre sens, il ferait
    # méchant, avec exactement le même maillage.
    haut = (Vector((0.0, 0.0, 1.0)) - cap * cap.z).normalized()
    incline = Matrix.Rotation(math.radians(cote * o["paupiere_incl_deg"]), 3, cap)
    haut = incline @ haut
    bmp = bmesh.new()
    bmesh.ops.create_uvsphere(bmp, u_segments=o["segments"],
                              v_segments=o["segments"] // 2,
                              radius=o["rayon"] * o["paupiere_jeu"])
    a_virer = [f for f in bmp.faces
               if f.calc_center_median().normalized().dot(haut) < o["paupiere_bord"]]
    bmesh.ops.delete(bmp, geom=a_virer, context="FACES")
    bmesh.ops.translate(bmp, verts=bmp.verts, vec=centre)
    paupiere = objet_depuis_bmesh(bmp, f"chien_paupiere_{cote_nom(cote)}", matrice,
                                  mats["poil"])
    return [obj, paupiere] + eclats, centre


def truffe(tete, matrice, mats):
    """Au bout du museau, trouvée au rayon — pas à une profondeur devinée.

    Avec ses deux narines : deux billes sombres à peine enfoncées. Une truffe
    lisse se lit comme un bouton de manteau."""
    t = SPEC["truffe"]
    pos, nor = surface(tete, Vector((0.0, 0.45, t["z"])), Vector((0.0, -1.0, 0.0)))
    if pos is None:
        return []
    centre = pos - nor.normalized() * t["enfoncement"]
    bm = bmesh.new()
    bmesh.ops.create_uvsphere(bm, u_segments=18, v_segments=10, radius=1.0)
    bmesh.ops.scale(bm, verts=bm.verts, vec=Vector(t["a"]))
    bmesh.ops.translate(bm, verts=bm.verts, vec=centre)
    pieces = [objet_depuis_bmesh(bm, "chien_truffe", matrice, mats["truffe"])]

    for cote in (-1, 1):
        bn = bmesh.new()
        bmesh.ops.create_uvsphere(bn, u_segments=10, v_segments=6, radius=1.0)
        bmesh.ops.scale(bn, verts=bn.verts, vec=Vector(t["narine_a"]))
        bmesh.ops.translate(bn, verts=bn.verts, vec=centre + Vector((
            cote * t["narine_ecart"], t["a"][1] * t["narine_avance"],
            t["narine_hauteur"])))
        pieces.append(objet_depuis_bmesh(
            bn, f"chien_narine_{cote_nom(cote)}", matrice,
            mats["gueule"]))
    return pieces


def gueule(tete, matrice, mat):
    """Ruban sombre plaqué sur le museau, échantillonné au rayon.

    Sans ligne de gueule, une tête de chien lisse ne lit pas comme un visage :
    c'est le trait qui donne l'expression."""
    g = SPEC["gueule"]
    pivot = Vector(g["pivot"])
    points = []
    for i in range(g["echantillons"]):
        part = 2.0 * i / (g["echantillons"] - 1) - 1.0     # -1 … +1
        a = math.radians(g["ouverture_deg"]) * part
        # Le sourire : les commissures remontent en carré de l'écart au centre.
        hauteur = g["remontee"] * part * part
        cap = Vector((math.sin(a), math.cos(a), 0.0))
        pos, nor = surface(tete, pivot + Vector((0, 0, hauteur)) + cap * 0.4, -cap)
        if pos is not None:
            points.append((pos + nor.normalized() * g["decollement"],
                           nor.normalized()))
    if len(points) < 2:
        return None

    bm = bmesh.new()
    hauts, bas = [], []
    for i, (pos, _) in enumerate(points):
        # 🎨 Le ruban S'AFFINE vers les commissures. D'épaisseur constante il se
        # lisait comme un trait de feutre posé sur le museau ; une bouche
        # dessinée est toujours plus épaisse au milieu.
        part = abs(2.0 * i / max(1, len(points) - 1) - 1.0)
        demi = g["demi_hauteur"] * (1.0 - g["affinement"] * part * part)
        hauts.append(bm.verts.new(pos + Vector((0, 0, demi))))
        bas.append(bm.verts.new(pos - Vector((0, 0, demi))))
    for i in range(len(points) - 1):
        bm.faces.new((hauts[i], hauts[i + 1], bas[i + 1], bas[i]))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    return objet_depuis_bmesh(bm, "chien_gueule", matrice, mat)


def taches(tete, mats):
    """Les marques du pelage — par COUPES successives, pas par tri de faces.

    🎨 Trois marques au lieu d'une : le masque crème du museau, la SELLE sombre
    du crâne, et le CERNE d'orbite. C'est la selle qui fait le travail lourd :
    elle descend la valeur du haut de la tête et détache enfin le visage du
    buste clair, là où un fauve uniforme s'y fondait.

    🚨 Les sphères de coupe sont BRUITÉES. Un bord géométriquement parfait se
    lisait comme une cagoule enfilée sur le crâne — trop propre pour de la
    fourrure. Le bruit rend la frontière irrégulière, exactement comme l'ourlet
    ardent de la cape (`26_cape_ardente.py`)."""
    produits, retires, confettis = [], 0, 0
    for marque in SPEC["marques"]:
        zones = []
        for centre, rayon in marque["spheres"]:
            zones.append((Vector(centre), rayon))

        couteau = None
        for i, (centre, rayon) in enumerate(zones):
            bm = bmesh.new()
            seg = SPEC["segments_coupe"]
            bmesh.ops.create_uvsphere(bm, u_segments=seg, v_segments=seg // 2,
                                      radius=rayon)
            # Le bruit déplace chaque sommet le long de sa normale : la sphère
            # reste une sphère à grande échelle et devient dentelée de près.
            for v in bm.verts:
                n = v.co.normalized()
                bruit = mathutils.noise.noise(n * marque["echelle_bruit"]
                                              + Vector((i * 7.3, 0.0, 0.0)))
                v.co = n * (rayon * (1.0 + bruit * marque["bruit"]))
            bmesh.ops.translate(bm, verts=bm.verts, vec=centre)
            piece = objet_depuis_bmesh(bm, f"couteau_{marque['nom']}_{i}",
                                       tete.matrix_world, mats["marqueur"])
            if couteau is None:
                couteau = piece
                continue
            mod = couteau.modifiers.new("union", "BOOLEAN")
            mod.operation, mod.object, mod.solver = "UNION", piece, "EXACT"
            figer(couteau)
            bpy.data.objects.remove(piece, do_unlink=True)

        tranche = tete.copy()
        tranche.data = tete.data.copy()
        tranche.name = tranche.data.name = f"chien_marque_{marque['nom']}"
        bpy.context.scene.collection.objects.link(tranche)

        for obj, operation in ((tranche, "INTERSECT"), (tete, "DIFFERENCE")):
            mod = obj.modifiers.new("marque", "BOOLEAN")
            mod.operation, mod.object, mod.solver = operation, couteau, "EXACT"
            # 🚨 `TRANSFER` et non `INDEX` (le défaut). En mode INDEX, les faces
            # venues du couteau reprennent un INDICE de slot de la cible : la
            # matière marqueur n'arrive jamais, et le repérage exact des parois
            # ne repère rien — 0 face retirée, 24 000 triangles au lieu de
            # 10 000. C'est ce réglage, pas la logique, qui portait la promesse.
            mod.material_mode = "TRANSFER"
            figer(obj)

        # 🚨 Les parois de coupe se reconnaissent à leur MATIÈRE, pas à leur
        # géométrie. La version précédente les cherchait par distance au centre
        # de la sphère ; sur une sphère BRUITÉE ce rayon n'est plus une
        # constante, la tolérance a dû s'élargir, et elle a fini par manger de
        # vraies faces du crâne — la tête est sortie déchiquetée, criblée de
        # trous blancs.
        #
        # Le booléen, lui, sait exactement d'où vient chaque face : celles du
        # couteau héritent de SA matière. Un marqueur unique les désigne donc
        # sans la moindre heuristique.
        retires += retirer_parois_marquees(tranche, mats["marqueur"])
        retires += retirer_parois_marquees(tete, mats["marqueur"])
        confettis += retirer_ilots(tranche, SPEC["ilot_min_faces"])
        confettis += retirer_ilots(tete, SPEC["ilot_min_faces"])
        bpy.data.objects.remove(couteau, do_unlink=True)

        tranche.data.materials.clear()
        tranche.data.materials.append(mats[marque["matiere"]])
        for poly in tranche.data.polygons:
            poly.use_smooth = True
        produits.append(tranche)

    for poly in tete.data.polygons:
        poly.use_smooth = True
    return produits, retires, confettis


def retirer_ilots(obj, min_faces):
    """Supprime les morceaux DÉTACHÉS trop petits pour signifier quelque chose.

    🚨 Une coupe bruitée ne produit pas qu'une frontière irrégulière : là où le
    bruit fait plonger la sphère sous la surface, elle détache des CONFETTIS —
    des îlots de trois ou quatre faces. Vus au rendu, ce sont des éclats blancs
    posés au hasard sur le front, et rien dans la géométrie ne les distingue
    d'une marque voulue sinon leur taille. On les compte, donc, et on jette les
    plus petits."""
    bm = bmesh.new()
    bm.from_mesh(obj.data)
    bm.faces.ensure_lookup_table()

    vus, a_virer = set(), []
    for depart in bm.faces:
        if depart.index in vus:
            continue
        groupe, pile = [], [depart]
        vus.add(depart.index)
        while pile:
            f = pile.pop()
            groupe.append(f)
            for arete in f.edges:
                for voisin in arete.link_faces:
                    if voisin.index not in vus:
                        vus.add(voisin.index)
                        pile.append(voisin)
        if len(groupe) < min_faces:
            a_virer += groupe

    if a_virer:
        bmesh.ops.delete(bm, geom=a_virer, context="FACES")
    bm.to_mesh(obj.data)
    bm.free()
    return len(a_virer)


def retirer_parois_marquees(obj, marqueur):
    """Supprime les faces issues du COUTEAU, reconnues à leur matière.

    Exact par construction : le booléen affecte au résultat la matière de
    l'opérande dont vient chaque face. Aucun seuil, aucune tolérance, donc rien
    à régler et rien qui dérape quand la forme du couteau change."""
    slots = [i for i, s in enumerate(obj.data.materials) if s == marqueur]
    if not slots:
        return 0
    bm = bmesh.new()
    bm.from_mesh(obj.data)
    bm.faces.ensure_lookup_table()
    a_virer = [f for f in bm.faces if f.material_index in slots]
    if a_virer:
        bmesh.ops.delete(bm, geom=a_virer, context="FACES")
    bm.to_mesh(obj.data)
    bm.free()
    for i in sorted(slots, reverse=True):
        obj.data.materials.pop(index=i)
    return len(a_virer)


def retirer_parois(obj, spheres, tol_rel=0.04, radial_min=0.80):
    """Supprime les faces qui appartiennent à une sphère de coupe."""
    bm = bmesh.new()
    bm.from_mesh(obj.data)
    bm.faces.ensure_lookup_table()
    a_virer = []
    for face in bm.faces:
        centre_f = face.calc_center_median()
        for centre, rayon in spheres:
            vers = centre_f - centre
            d = vers.length
            if abs(d - rayon) > rayon * tol_rel or d < 1e-9:
                continue
            if abs(face.normal.dot(vers / d)) > radial_min:
                a_virer.append(face)
                break
    if a_virer:
        bmesh.ops.delete(bm, geom=a_virer, context="FACES")
    bm.to_mesh(obj.data)
    bm.free()
    return len(a_virer)


# --------------------------------------------------------------------- rendu

def rendre(nom, cible, distance, azimut_deg, elevation_deg, taille=640, lens=45.0,
           devant=None):
    """Une vue orbitale. `devant` = le cap du personnage.

    ⚠️ Sans lui, l'azimut se compte depuis l'axe du MONDE : la vue « de face »
    n'est alors de face que si le personnage regarde pile −Y. Le nôtre regarde
    à −9,9°, et la vue de face montrait donc un trois-quarts. Une caméra
    « de face » qui ne l'est pas fait juger de travers une forme correcte."""
    # ⚠️ L'OBJET caméra se nomme `vue_…` et non `nom` : les vues s'appellent
    # `chien_face`, `chien_profil`… comme les pièces sculptées. Un script qui
    # ramasse les objets par préfixe attrapait donc des caméras et cherchait
    # leurs sommets. Deux familles, deux préfixes.
    cam_data = bpy.data.cameras.new(f"vue_{nom}")
    cam_data.lens = lens
    cam = bpy.data.objects.new(f"vue_{nom}", cam_data)
    bpy.context.scene.collection.objects.link(cam)
    a, e = math.radians(azimut_deg), math.radians(elevation_deg)
    f = (devant or Vector((0.0, -1.0, 0.0))).normalized()
    vertical = Vector((0.0, 0.0, 1.0))
    lateral = vertical.cross(f).normalized()
    cam.location = cible + (lateral * math.sin(a) * math.cos(e)
                            + f * math.cos(a) * math.cos(e)
                            + vertical * math.sin(e)) * distance
    cam.rotation_euler = (cible - cam.location).normalized().to_track_quat("-Z", "Y").to_euler()
    bpy.context.scene.camera = cam
    bpy.context.scene.render.resolution_x = taille
    bpy.context.scene.render.resolution_y = taille
    # 🚨 Le format se DÉCLARE. Hérité de la scène, il peut être un format
    # d'ANIMATION (la démo le laisse sur MPEG4) et Blender refuse alors d'écrire
    # une image fixe : « Impossible d'écrire un fichier unique avec un format
    # d'animation sélectionné ». Un script qui rend ne suppose jamais l'état
    # dans lequel le précédent l'a laissé.
    bpy.context.scene.render.image_settings.file_format = "PNG"
    chemin = os.path.join(BAC, f"{nom}.png")
    bpy.context.scene.render.filepath = chemin
    bpy.ops.render.render(write_still=True)
    return chemin


def decor(centre):
    monde = bpy.data.worlds.new("neutre")
    monde.use_nodes = True
    fond = next((n for n in monde.node_tree.nodes if n.type == "BACKGROUND"), None)
    if fond is not None:
        fond.inputs[0].default_value = (0.35, 0.36, 0.4, 1)
        fond.inputs[1].default_value = 1.4
    bpy.context.scene.world = monde
    key = bpy.data.lights.new("key", type="AREA")
    key.energy, key.size = 95.0, 1.2
    obj = bpy.data.objects.new("key", key)
    obj.location = centre + Vector((0.7, -0.9, 0.8))
    obj.rotation_euler = (centre - obj.location).normalized().to_track_quat("-Z", "Y").to_euler()
    bpy.context.scene.collection.objects.link(obj)
    moteurs = [e.identifier for e in
               bpy.types.RenderSettings.bl_rna.properties["engine"].enum_items]
    bpy.context.scene.render.engine = ("BLENDER_EEVEE_NEXT" if "BLENDER_EEVEE_NEXT"
                                       in moteurs else "BLENDER_EEVEE")


# ---------------------------------------------------------------------- main

def main():
    vider()
    bpy.ops.import_scene.gltf(filepath=CORPS)

    # ————————————————————————————————————————————————————————————————
    # LE REPÈRE VIENT DE L'OS `Head`. Trois erreurs mesurées ont mené ici,
    # toutes de la même famille — prendre une grandeur là où elle est commode
    # plutôt que là où elle fait foi :
    #
    #  1. le CAP venait des ORTEILS. Dans une pose de repos les pieds sont
    #     écartés : 7,5° de lacet d'écart avec l'os de la tête. La tête était
    #     tournée par rapport au corps.
    #  2. l'INCLINAISON était nulle. L'os penche de 12,3° vers l'avant — le
    #     corps humain, lui, la suivait, puisqu'il y est pesé.
    #  3. la POSITION venait du centre de la boîte de `SM_Head`, boîte gonflée
    #     par une chevelure ASYMÉTRIQUE : 18,5 mm de biais latéral, hérités
    #     d'une coiffure que le chien n'a pas.
    #
    # Et un quatrième défaut, muet celui-là : (droite, devant, haut) est un
    # trièdre LÉVOGYRE. La matrice avait donc un déterminant NÉGATIF — un objet
    # miroir, que l'export glTF aurait retourné. On prend x = GAUCHE pour la
    # rendre dextrogyre. La sculpture étant symétrique, seul le suffixe des
    # noms change.
    # ————————————————————————————————————————————————————————————————
    arm = next((o for o in bpy.data.objects if o.type == "ARMATURE"), None)

    # 🚨 ON SCULPTE AU REPOS, jamais dans la pose.
    #
    # Le GLB arrive POSÉ : l'os `Head` est à 116 mm de sa position de repos.
    # Sculpter contre la pose puis peser la peau revient à déclarer que ces
    # coordonnées SONT le repos — le modificateur d'armature applique alors une
    # seconde fois l'écart pose↔repos. Mesuré : 86 à 140 mm de dérive, la tête
    # partait dans l'épaule dès qu'on liait.
    #
    # Effet de bord assumé : les rendus de ce script montrent le corps en
    # T-pose. C'est le prix de la justesse, et c'est de toute façon la posture
    # dans laquelle on relie une peau à un squelette. `36_rig_chien.py` remet la
    # pose une fois la liaison faite.
    arm.data.pose_position = "REST"
    bpy.context.view_layer.update()

    articulation = (arm.matrix_world @ arm.pose.bones["Head"].matrix).translation.copy()

    # LE CAP vient des ÉPAULES. Pas des pieds (écartés au repos : 4,7° de lacet
    # d'erreur), pas de l'os `Head` non plus — celui-ci penche de 12,3°, et
    # copier son inclinaison faisait PLONGER le museau alors que la tête humaine
    # est de niveau. Le maillage humain a été modelé droit puis pesé sur un os
    # penché : pour la peau seul compte l'écart figé au bind, pour l'œil c'est
    # le niveau qui compte. La ligne d'épaules, elle, ne ment ni sur le lacet ni
    # sur le niveau.
    #
    # 🚨 Et les épaules donnent l'AXE, pas le SENS : `LeftArm`/`RightArm` sont
    # nommés du point de vue du personnage, ce qui ne dit rien du côté du monde
    # où ils tombent. Pris tel quel, le cap sortait à 172° — la tête regardait
    # derrière. Ce sont les orteils qui tranchent le sens : grossiers en lacet,
    # mais sans ambiguïté sur l'avant. Deux sources, chacune pour ce qu'elle
    # sait ; aucune des deux ne suffit seule.
    haut = Vector((0.0, 0.0, 1.0))
    lateral = ((arm.matrix_world @ arm.pose.bones["RightArm"].matrix).translation
               - (arm.matrix_world @ arm.pose.bones["LeftArm"].matrix).translation)
    lateral.z = 0.0
    devant = lateral.normalized().cross(haut).normalized()

    grossier = Vector((0.0, 0.0, 0.0))
    for cheville, orteil in (("LeftFoot", "LeftToeBase"), ("RightFoot", "RightToeBase")):
        a, b = arm.pose.bones.get(cheville), arm.pose.bones.get(orteil)
        if a and b:
            d = ((arm.matrix_world @ b.matrix).translation
                 - (arm.matrix_world @ a.matrix).translation)
            d.z = 0.0
            if d.length > 1e-4:
                grossier += d.normalized()
    if grossier.length > 1e-6 and devant.dot(grossier.normalized()) < 0.0:
        devant = -devant
    gauche = devant.cross(haut).normalized()

    rot = Matrix(((gauche.x, devant.x, haut.x, 0.0),
                  (gauche.y, devant.y, haut.y, 0.0),
                  (gauche.z, devant.z, haut.z, 0.0),
                  (0.0, 0.0, 0.0, 1.0)))

    # LA POSITION reprend celle qu'occupait la tête humaine — validée à l'œil —
    # sauf son abscisse, prise sur l'os. Les 18,5 mm de biais latéral du centre
    # de boîte n'étaient que sa coiffure asymétrique, et le chien n'a pas cette
    # coiffure.
    humaine = bpy.data.objects["SM_Head"]
    pts_h = [humaine.matrix_world @ v.co for v in humaine.data.vertices]
    P = Vector((articulation.x,
                sum(p.y for p in pts_h) / len(pts_h),
                (min(p.z for p in pts_h) + max(p.z for p in pts_h)) / 2.0))

    mats = {cle: matiere(f"Chien_{cle}", val, SPEC["rugosites"].get(cle))
            for cle, val in SPEC["couleurs"].items()}

    # 1. le blocage, 2. le maillage, 3. le calibrage — qui va dans la POSE,
    #    pas dans le maillage, 4. l'allègement.
    #    Les métaballes vivent en coordonnées locales : la matrice provisoire
    #    ne sert qu'à les poser quelque part, le maillage n'en dépend pas.
    masses = blocage(Matrix.Translation(P) @ rot)
    tete = en_maillage(masses, "chien_tete")
    if len(tete.data.vertices) == 0:
        print("RESULT: " + json.dumps({"erreur": "métaballes non tessellées"}))
        return
    facteur = facteur_calibrage(tete, SPEC["hauteur_crane_cible_m"])
    matrice = (Matrix.Translation(P) @ rot
               @ Matrix.Diagonal((facteur, facteur, facteur, 1.0)))
    tete.matrix_world = matrice
    tri_avant, tri_apres = alleger(tete, SPEC["triangles_cible"])
    tete.data.materials.append(mats["poil"])

    # 5. les pièces accrochées à la surface — chacune trouvée au rayon, dans le
    #    MÊME repère d'auteur que les masses.
    pieces = [p for p in (cou(matrice, mats["poil"], arm),) if p]
    for cote in (-1, 1):
        pieces += oreille(cote, matrice, mats)
    pieces += touffes(matrice, mats["poil"])
    yeux = {}
    for cote in (-1, 1):
        objs, centre = oeil(cote, tete, matrice, mats)
        pieces += objs
        if centre is not None:
            yeux[cote_nom(cote)] = [round(v, 4) for v in centre]
    nez = truffe(tete, matrice, mats)
    bouche = gueule(tete, matrice, mats["gueule"])
    pieces += nez + ([bouche] if bouche else [])
    marques, parois_retirees, confettis = taches(tete, mats)
    pieces += marques

    # 6. mesures de contrôle : ce que la sculpture fait VRAIMENT.
    def emprise(objets):
        """En mètres MONDE : le calibrage vit dans la matrice, donc mesurer en
        coordonnées locales rendrait les dimensions d'avant calibrage."""
        vs = [o.matrix_world @ v.co for o in objets for v in o.data.vertices]
        return [round(max(v[i] for v in vs) - min(v[i] for v in vs), 4)
                for i in range(3)]

    # La tête est en DEUX morceaux depuis la coupe : le poil et le masque. Les
    # mesurer séparément rendrait une emprise fausse.
    crane = [tete] + marques
    dims = emprise(crane)
    silhouette = emprise(crane + [p for p in pieces if "oreille" in p.name])
    tete.data.calc_loop_triangles()
    total_tri = len(tete.data.loop_triangles)
    for p in pieces:
        p.data.calc_loop_triangles()
        total_tri += len(p.data.loop_triangles)

    # 7. rendu : le chien SUR le corps, seul juge du style.
    #    La cible de cadrage se MESURE sur la sculpture finie — la tête n'est
    #    plus posée à un centre connu d'avance, elle est ancrée sur un os.
    humaine = bpy.data.objects.get("SM_Head")
    if humaine is not None:
        humaine.hide_render = True
    sommets_crane = [o.matrix_world @ v.co for o in crane for v in o.data.vertices]
    cible_tete = Vector((
        sum(p.x for p in sommets_crane) / len(sommets_crane),
        sum(p.y for p in sommets_crane) / len(sommets_crane),
        (min(p.z for p in sommets_crane) + max(p.z for p in sommets_crane)) / 2.0))

    tous = [o.matrix_world @ v.co for o in bpy.data.objects
            if o.type == "MESH" and not o.name.startswith("Icosph")
            for v in o.data.vertices]
    z_bas, z_haut = min(p.z for p in tous), max(p.z for p in tous)
    decor(cible_tete)
    vues = {"chien_corps": rendre("chien_corps", Vector((0, 0, (z_bas + z_haut) / 2)),
                                  (z_haut - z_bas) * 1.35, 25, 4, devant=devant)}
    for nom, az, el in (("chien_face", 0, 3), ("chien_trois_quarts", 40, 8),
                        ("chien_profil", 90, 3)):
        vues[nom] = rendre(nom, cible_tete, 0.62, az, el, lens=42.0, devant=devant)

    print("RESULT: " + json.dumps({
        "facteur_echelle": round(facteur, 4),
        # Contrôles d'alignement.
        #  · le déterminant DOIT être positif — négatif = objet MIROIR, que
        #    l'export glTF retournerait ;
        #  · l'articulation doit tomber devant (+y) et sous (−z) le centre du
        #    crâne : c'est la place d'une nuque. Positive en z voudrait dire que
        #    le cou sort du sommet du crâne ;
        #  · le lacet des épaules dit de combien le cap corrigé s'écarte de
        #    l'axe de modélisation.
        "determinant_pose": round(matrice.to_3x3().determinant(), 5),
        "articulation_en_local": [round(v, 4)
                                  for v in (matrice.inverted() @ articulation)],
        "lacet_epaules_deg": round(math.degrees(
            devant.xy.angle(Vector((0.0, -1.0)))), 2),
        "dims_crane_m": dims,
        "dims_silhouette_m": silhouette,
        # Contrôle qui ne peut pas passer à vide : si les oreilles n'élargissent
        # pas la silhouette, c'est qu'elles sont DANS le crâne.
        "oreilles_debordent_mm": round((silhouette[0] - dims[0]) * 1000, 1),
        "dims_humaine_m": [0.2263, 0.2737, 0.3229],
        # Le seul rapport qui décide « personnifié » contre « quadrupède ».
        "profondeur_sur_hauteur": round(dims[1] / dims[2], 3),
        "triangles": {"metaballes": tri_avant, "tete_allegee": tri_apres,
                      "total_avec_pieces": total_tri, "humaine": 11620},
        "parois_de_coupe_retirees": parois_retirees,
        "confettis_retires": confettis,
        "yeux_locaux": yeux,
        "truffe": len(nez),
        "gueule": bouche is not None,
        "pieces": sorted(p.name for p in pieces),
        "vues": vues,
    }, ensure_ascii=False))


main()
