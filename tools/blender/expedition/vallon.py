"""Le Vallon — carte d'expédition fermée, verdoyante, authorée.

Spawn à l'ouest, chemin qui remonte tout le vallon, village au fond à l'est.
La carte est close par sa géométrie — une ceinture de falaises — jamais par des
murs invisibles : on doit VOIR où le monde s'arrête.

CE FICHIER EST LA COUCHE DEFINITION de la carte. Chaque nombre de `SPEC` est
dérivé d'une mesure (métriques joueur lues dans le code, dimensions des kits
relevées par les sondes 00/01/02, palette relue dans les glTF) et sa dérivation
est écrite à côté. Un nombre qu'on ne sait pas justifier est un nombre qu'on ne
saura pas re-régler.

CE QUI DONNE ENVIE D'EXPLORER — les quatre leviers, dans l'ordre de rendement :
  1. le RELIEF : une plaine plate ne se parcourt pas, elle se traverse ;
  2. la RÉVÉLATION : une crête cache le village jusqu'au col — le franchir
     est une récompense, pas une formalité ;
  3. le CONTRASTE DE DENSITÉ : des futaies serrées ET des clairières, jamais
     un semis uniforme (c'est ce qui rendait la passe précédente inerte) ;
  4. les POINTS D'APPEL : cascade, arbre-monument, ruines — on va vers ce
     qu'on voit de loin.

  python tools/blender/bmcp.py code tools/blender/expedition/vallon.py
"""

import json
import math
import os
import random
import re

import bpy
import mathutils
from mathutils import Vector

RACINE = r"C:\Users\Antoi\Desktop\Forgia Rewrite"
KIT_NATURE = os.path.join(RACINE, "assets", "models", "kenney", "nature")
KIT_VILLAGE = os.path.join(RACINE, "assets", "models", "kaykit", "medieval_hexagon")
DOSSIER_ANIMAUX = os.path.join(RACINE, "assets", "models", "characters", "animals")
KIT_CHATEAU = os.path.join(RACINE, "assets", "models", "environment", "castle_kit")
TEXTURES = os.path.join(RACINE, "assets", "textures-v1", "terrain")
SORTIE = os.path.join(RACINE, "assets", "models", "environment", "expedition")
GENOMES_ENNEMIS = os.path.join(RACINE, "assets", "genomes", "enemies")


def visions_archetypes():
    """Portee de vision de chaque archetype, LUE dans son genome.

    Recopier « 20,0 » dans cette SPEC en ferait une seconde source. Le jour ou
    `enemy_grunt.toml` bouge, la carte garderait l'ancienne valeur — et le
    controle de tir gratuit, qui compare precisement ces deux nombres, se
    mettrait a mentir dans le sens rassurant. C'est la classe de defaut n°1 du
    projet : une grandeur ecrite deux fois.

    Lecture par expression reguliere plutot que par `tomllib` : ces genomes
    vivent dans un format a blocs `[[genes]]` que Blender n'a pas a savoir
    parser entierement pour en tirer un seul nombre.
    """
    return _gene_archetypes("vision_range")


def pv_archetypes():
    """Points de vie de chaque archétype, LUS dans son génome.

    Même raison que les visions : c'est de ces nombres que la durée
    d'engagement se dérive, et les recopier ici les ferait diverger du jour où
    l'équilibrage bouge — en silence, puisque la carte continuerait de publier
    une durée d'apparence plausible.
    """
    return _gene_archetypes("max_hp")


def _gene_archetypes(suffixe):
    valeurs = {}
    if not os.path.isdir(GENOMES_ENNEMIS):
        return valeurs
    motif = re.compile(
        r"id\s*=\s*\"(\w+)_" + suffixe + r"\"(.*?)default\s*=\s*([0-9.]+)", re.S)
    for nom in sorted(os.listdir(GENOMES_ENNEMIS)):
        if not nom.endswith(".toml"):
            continue
        with open(os.path.join(GENOMES_ENNEMIS, nom), encoding="utf-8") as f:
            for m in motif.finditer(f.read()):
                valeurs[m.group(1)] = float(m.group(3))
    return valeurs


# ---------------------------------------------------------------------------
# SPEC — couche definition
# ---------------------------------------------------------------------------

SPEC = {
    # Métriques joueur LUES dans le code (player_movement.toml, arena_test.rs).
    # `oeil_m` : c'est LUI qui decide ce qu'est une couverture. Il n'y a pas
    # d'accroupissement dans Forgia, donc la taxonomie haute/basse ne transpose
    # pas : sous 1,70 m un bloc masque le corps sans masquer la vue, et ne sert
    # a rien (`map-design-patterns.md` §11).
    "joueur": {"rayon_m": 0.3, "saut_m": 1.174, "marche_ms": 6.5, "sprint_ms": 9.75,
               "oeil_m": 1.70},

    # Échelles dérivées (sondes 00 et 02) :
    #   nature  — tuile 1,0 → 4 m de chemin ; cliff_block → mur de 4 m ≫ saut
    #   village — home_A haut de 0,93 → chaumière de 5,6 m au faîte
    "echelle_nature": 4.0,
    "echelle_village": 6.0,

    # 280 x 200 m. Derive du RYTHME, pas du gout : ~45 m de marche libre entre
    # deux temps forts (6 temps -> 5 intervalles = 225 m) + 171 m occupes par
    # les temps forts eux-memes = ~400 m de chemin. Un chemin de 400 m dans une
    # boite de 240 m se replierait sur lui-meme.
    "demi_x": 140.0,
    "demi_y": 100.0,
    "pas_terrain": 1.6,          # maille fine : le relief mérite mieux que 2 m
    "graine": 20260813,

    # --- relief ---------------------------------------------------------
    # 14 m d'amplitude sur 240 m de large : assez pour que des crêtes barrent
    # la vue et qu'un col se mérite, pas assez pour transformer la marche en
    # escalade (la pente du chemin reste bornée à 12 %).
    # La ceinture est portée par LE TERRAIN, pas par un empilement de cubes :
    # 1 721 blocs identiques donnaient un gâteau à étages. 26 m de dénivelé sur
    # une bande courte = une paroi franche (≫ saut de 1,174 m), et son pied
    # ondule pour qu'aucune arête ne soit rectiligne.
    "rim_debut": 0.84,
    "rim_hauteur": 26.0,
    # Ondulation du PIED de la paroi. Relevee de 0,055 a 0,085 : a 5,5 % elle ne
    # rachetait pas une forme de boite, et maintenant qu'elle ondule une
    # superellipse (cf. `Relief.bord`) elle a de quoi se lire.
    "rim_ondulation": 0.085,
    # EXPOSANT DE LA SUPERELLIPSE qui donne sa forme a l'enceinte.
    # 2 = ellipse, l'infini = rectangle. La carte valait l'infini (une norme
    # `max()`), d'ou quatre parois droites et quatre angles vifs — vue de haut,
    # un plateau de maquette. 3,0 arrondit les angles en gardant 87 % de l'aire
    # du rectangle, et le chemin ne va jamais dans les coins.
    "bord_exposant": 3.0,
    # GORGE — l'entaille par ou la riviere traverse la ceinture. Sans elle, le
    # rempart barre le cours d'eau et la nappe escalade la paroi (defaut deja
    # mesure a +28,74 m).
    #   `demi_largeur` : le fond de gorge, plein pied. 11 m = la demi-largeur
    #     de la nappe (8) plus ses berges.
    #   `evasement`    : au-dela, la paroi remonte a pleine hauteur — ce sont
    #     les deux falaises qui encadrent le passage.
    #   `bouchon`      : fraction d'emprise a partir de laquelle l'entaille se
    #     REFERME. Sans lui, la gorge devient une sortie de carte : le joueur
    #     remonterait la riviere et sortirait du monde.
    #   `force`        : part du rempart REELLEMENT effacee au fond. 0,88 en
    #     laisse 12 % (~3 m) : ce sont les berges qui confinent le courant.
    #     A 1,00 le fond devenait une dalle plate au niveau de l'eau, et la
    #     nappe s'y etalait jusqu'a sa largeur maximale — un lac, pas un cours.
    # BOUCHAGE DE LA CEINTURE. Le rempart fait 17,9 m en mediane, mais des
    # creux descendent a 0,21 m — et par endroits le bord passe SOUS
    # l'interieur (-5,21 m mesure). Trois causes : les deux bouches de gorge,
    # et l'aplanissement du spawn qui ronge la paroi ouest.
    # On y pose les rochers du chateau (`SM_ENV_cliff_castle_01/02`), la meme
    # matiere que le Hall. Les positions sont MESUREES au moment de la
    # construction, pas listees a la main : si le trace ou le relief bouge,
    # le bouchage suit.
    "ceinture_bouchage": {
        # DEUX seuils, et il ne faut pas les confondre :
        #   `seuil_m` est VISUEL — en deca, la paroi parait basse et on bouche.
        #   L'invariant DUR, lui, est le saut du joueur : 1,174 m. Tout ce qui
        #   depasse ca est infranchissable, meme si ca reste laid.
        "seuil_m": 9.0,
        "sondes": 320,         # points de mesure sur le pourtour (~3 m de pas)
        "ecart_min_m": 5.5,    # deux rochers se chevauchent volontiers ici
        "assise": 0.94,
        "reference": 0.70,
        "echelle": [1.1, 1.9],
        "enfoncement_m": 0.5,  # a 1,0 m les rochers perdaient trop de hauteur
    },
    "gorge": {"demi_largeur": 11.0, "evasement": 17.0, "bouchon": 0.94,
              "force": 0.88},     # amplitude, en fraction d'emprise, du pied
    # PENTE MAXIMALE DE PLANTATION — une seule source, quatre consommateurs.
    #
    # Elle était écrite en dur dans les deux boucles d'arbres (34.0) et ABSENTE
    # des rochers et du sous-bois, qui ne testaient que le dégagement du chemin.
    # Mesuré : 362 props au-delà de 34°, dont des rochers posés sur des parois.
    #
    # 34° : c'est la pente au-delà de laquelle une motte ne tient pas. Les
    # éboulis (bande 12-40°) et les bouchons de ceinture en sont exemptés — se
    # tenir sur la paroi EST leur rôle, pas un défaut.
    "pente_plantation_deg": 34.0,
    # Au-delà de cette pente, le terrain n'est plus de l'herbe mais de la roche.
    # 36° = plantation + 2 : la roche affleure juste après qu'on cesse de planter,
    # sinon on verrait de l'herbe sur ce qui est déjà minéral.
    "pente_roche_deg": 36.0,
    # Altitude de bascule entre roche basse (chaude) et haute (claire), et
    # amplitude du bruit qui brouille cette limite. Le terrain va de -6 a
    # +15 m, la ceinture a +26 : 9 m place la bascule a mi-paroi.
    "roche_bascule_z": 9.0,
    "roche_bascule_flou": 3.5,
    # FONDUS ENTRE MATIERES. Un seuil net produit une frontiere lisse et
    # continue — la ligne de ciseaux qu'aucun terrain n'a. On brouille donc
    # chaque bascule par un bruit a DEUX octaves : une lobe large (~14 m) qui
    # fait divaguer la frontiere a l'echelle du paysage, et un grain fin
    # (~3 m) qui la dentelle. C'est ce qui s'exporte : le glTF ne transporte
    # qu'une texture de base par materiau, tout melange doit donc etre porte
    # par la REPARTITION des faces, pas par un shader.
    "fondu_pente_deg": 9.0,      # +/- sur le seuil roche/herbe
    "fondu_berge_m": 4.5,        # +/- sur la largeur de greve
    "fondu_lobe": 0.035,         # frequence de la lobe large
    "fondu_grain": 0.16,         # frequence du grain fin
    # Force du verdissement des vires peu pentues (recette « top projection »
    # du chateau). 0 = decoupe nette herbe/roche, 1 = roche entierement
    # moussue. Au-dela de ~0,5 la paroi redevient verte, le defaut d'origine.
    "mousse_force": 0.42,
    # Force des fondus de COULEUR aux frontieres de matiere. 0 = teinte
    # inchangee jusqu'a la bascule, 1 = la couleur voisine gagne tout.
    "fondu_couleur": 0.55,
    # Largeur de la bande de berge, mesuree depuis l'axe de l'eau.
    # 8,0 (demi-largeur) + 5 : le sable deborde l'eau, comme une greve.
    "berge_largeur": 10.0,
    "pierres_lit": 130,
    "collines_amplitude": 7.5,
    "collines_echelle": 0.011,
    "grain_amplitude": 1.1,
    "grain_echelle": 0.055,
    # 🚨 LES VUES PROTÉGÉES — ce que RIEN de haut n'a le droit de boucher.
    #
    # Le second levier de cette carte, écrit en tête de ce fichier, est la
    # RÉVÉLATION : « une crête cache le village jusqu'au col — le franchir est
    # une récompense, pas une formalité ». Or le semis ne gardait QUE le chemin
    # (`libre()` teste une distance à l'axe) ; aucune règle ne gardait une LIGNE
    # DE VUE. Mesuré le 2026-08-17 sur l'approche de la porte : **sept arbres de
    # 4,8 à 6,8 m**, dont trois à moins de 1,2 m de l'axe du regard, entre le
    # joueur et la porte. La récompense de 358 m de marche était un rideau de
    # troncs.
    #
    # Une vue se déclare donc par ses deux bouts et sa largeur. Elle interdit
    # tout ce qui dépasse l'œil du joueur : sous 1,70 m on voit par-dessus, donc
    # rien n'est bouché — c'est le même seuil qui définit une couverture, et
    # pour la même raison.
    #
    # Les demi-largeurs se dérivent de ce qu'on veut LIRE :
    #   - à l'arrivée, la porte fait 4,6 m de large : ±4 m la dégagent
    #     entièrement, plus la marge du corps du joueur ;
    #   - au départ du cône, on n'a besoin que de la largeur du chemin — au-delà
    #     on regarde de côté, et boucher les côtés est même souhaitable.
    "vues_protegees": [
        # La porte, vue de l'approche. 40 m : la distance à laquelle un ouvrage
        # de 8,5 m de haut se lit en entier sans lever la tête.
        {"nom": "porte", "portee_m": 40.0, "demi_pres": 3.0, "demi_loin": 6.0},
        # Le village depuis le col — LA révélation. Elle se prend depuis le col
        # et vise la place ; c'est le seul endroit d'où le village apparaît.
        {"nom": "revelation", "portee_m": 999.0, "demi_pres": 4.0, "demi_loin": 14.0},
    ],
    # LA crête de révélation : elle barre le vallon juste avant le village.
    "crete": {"x": 34.0, "epaisseur": 22.0, "hauteur": 11.0, "col_y": 18.0, "col_largeur": 26.0},
    # Le mamelon de l'arbre-monument : un point haut hors du chemin, donc un
    # détour qui se voit de loin et qui coûte (§4.2 map-design-intention).
    "mamelon": {"xy": [-6.0, -50.0], "rayon": 26.0, "hauteur": 9.0},

    # --- chemin ---------------------------------------------------------
    # 4 m de large : deux joueurs de front (Ø 0,6 m) avec du jeu. C'est cette
    # largeur qui a fixé l'échelle du kit — les deux ne peuvent pas diverger.
    "chemin_demi_largeur": 2.2,
    "chemin_raccord": 8.0,
    "chemin_pente_max": 0.12,
    "chemin": [
        [-124.0, -10.0], [-114.0, -28.0], [-98.0, -42.0], [-78.0, -48.0],
        [-58.0, -42.0], [-46.0, -28.0], [-42.0, -10.0], [-44.0, 8.0],
        [-36.0, 26.0], [-25.0, 38.0], [-6.0, 46.0], [14.0, 46.0],
        [32.0, 38.0], [44.0, 24.0], [50.0, 6.0], [52.0, -12.0],
        [62.0, -26.0], [78.0, -32.0], [90.0, -30.0], [90.0, 0.0],
    ],

    # --- rivière --------------------------------------------------------
    # Le trace s'arrete AVANT la ceinture. Il allait jusqu'au bord de carte
    # (+/-100) : comme la nappe suit le terrain, elle escaladait la falaise et
    # culminait a +28,74 m — une riviere qui monte un mur de 26 m. C'etait le
    # grand plan pale colle a la paroi.
    # Le trace rejoint les DEUX bords. Il s'arretait a +/-74 en plein pre, et
    # une riviere qui commence et finit au milieu d'un champ ne ressemble a
    # rien. Elle traverse desormais la ceinture par une gorge (cf. `gorge`).
    "riviere": [
        [-36.0, 94.0], [-34.0, 78.0], [-30.0, 58.0], [-26.0, 38.0],
        [-26.0, 12.0], [-31.0, -18.0], [-37.0, -50.0], [-41.0, -78.0],
        [-43.0, -94.0],
    ],
    # Elargie de 10 a 16 m et le lit RESSERRE (1,4 au lieu de 2,2 fois la
    # demi-largeur) : mesure a l'appui, la nappe etait bien 1,01 m au-dessus de
    # son lit — elle n'etait pas enfouie, elle etait ETROITE dans une cuvette
    # large et molle, donc illisible. Une riviere se voit a ses BERGES.
    "riviere_demi_largeur": 8.0,
    "riviere_evasement": 2.0,
    # L'eau ne se voit pas par sa couleur mais par son CONTRASTE et son reflet.
    # Rugosite 0,08 : quasi miroir, elle attrape le soleil et le ciel.
    "eau": {
        "teinte": "#186B8C",     # sombre ET saturee : elle doit trancher sur la greve
        "rugosite": 0.08,
        "metallique": 0.0,
        "alpha": 0.86,           # on devine les galets du fond
        "rides": "swamp",        # sa normale sert de clapot
        "rides_uv_m": 3.0,
        "rides_force": 0.45,
        # Longueur d'une tuile de texture le long du courant.
        "tuile_m": 6.0,
        # Vitesse de defilement de V, en tuiles par seconde. Le moteur
        # l'applique sur StandardMaterial::uv_transform.
        "courant_tuiles_par_s": 0.16,
    },
    # Tirant d'eau : le lit se creuse de tant SOUS la nappe.
    "riviere_profondeur": 1.7,
    # De combien la nappe est en contrebas de la berge. 0,7 m : l'eau remplit
    # son lit au lieu de couler au fond d'un fosse.
    "riviere_berge_libre": 0.7,
    # 8,0 (demi-largeur) x 1,35 (evasement) = 10,8 m de demi-chenal,
    # + 2,2 m de culee de chaque cote.
    "pont_demi_portee": 13.0,
    # Tirant d'air sous le tablier, au-dessus de la nappe.
    "pont_tirant_air": 2.2,
    # PONT DE PIERRE du chateau. `deck_local_z` = altitude du tablier DANS le
    # module (origine au pied, structure haute de 10,00 m) : c'est ce qui
    # permet de caler le passage a hauteur de chemin au lieu d'enterrer
    # l'ouvrage. Mesure locale : min Z -0,019, max Z 10,002.
    "pont_pierre": {
        "actif": False,   # remis au pont fabrique (planches + piles)
        "echelle": 1.0,          # 1.0 = monumental ; 0.5 = a l'echelle du sentier
        "deck_local_z": 10.002,
        "module": "SM_MOD_bridge_base_castle_LOD0",
        # Taille d'une tuile de pierre, en metres. Les modules du chateau
        # n'ont pas d'UV utilisables ici (exportes sans materiau) : on
        # projette donc en coordonnees generees.
        "uv_m": 2.5,
    },
    # Recalage lateral du tablier, RELEVE DANS LA SCENE apres qu'Antoine l'ait
    # ajuste a la main (`pont.loc.y = -0.746`) pour que les deux troncons de
    # chemin se rejoignent sans decalage. C'est une valeur constatee, pas
    # derivee : si la cause est trouvee un jour (la tangente prise sur 2
    # stations n'est peut-etre pas exactement l'axe du ruban au droit du
    # franchissement), elle doit disparaitre au profit du calcul.
    "pont_recalage_lateral_m": -0.746,
    # Planches en travers et leur jour. Le pas (0,32 m) donne l'echelle de
    # l'ouvrage : sans lui, un tablier lisse pourrait faire 3 m comme 30.
    "pont_planche_m": 0.26,
    "pont_jour_m": 0.06,
    # Culees de pierre aux deux abouts : elles cachent la couture entre le
    # chemin de terre et le tablier de bois, qui est la jonction la plus
    # visible de la carte.
    "pont_culees": 5,
    "cascade_xy": [-33.0, 94.0],

    # --- eclairage du chemin ---------------------------------------------
    # Des braseros jalonnent la route : la nuit tombe en approchant du village,
    # et le moteur les allumera PROGRESSIVEMENT selon l'avancee du joueur.
    #
    # Le choix de la piece est mesure, pas esthetique : la torche du kit fait
    # 1,13 m — sa flamme serait SOUS l'oeil du joueur (1,70 m) et l'eblouirait
    # au lieu d'eclairer. `Brazier_002` monte a 2,90 m, sa flamme culmine vers
    # 2,7 m. Son emprise de 2 m tient hors du couloir de marche.
    #
    # L'ecart de 19 m se derive de la portee voulue : une lumiere de 12 m de
    # rayon couvre 24 m de chemin ; espacer de 19 m garantit un recouvrement,
    # donc pas de trou noir entre deux feux.
    "lampes": {
        "piece": "environment/inferno/Brazier_002.glb",
        "ecart_m": 19.0,
        "lateral_m": 5.2,
        # Marge AU-DELA du degagement de 3,6 m : le brasero fait 2 m de
        # large, il lui faut son demi-rayon plus un jeu.
        "marge_m": 2.4,
        # Distance minimale a l'axe de la riviere. La nappe fait 8 m de
        # demi-largeur et son rivage va jusqu'a ~14 m : 17 m met le
        # brasero au sec meme la ou le lit s'evase.
        "degagement_eau_m": 17.0,
        "hauteur_flamme_m": 2.7,   # ou le moteur posera sa lumiere ponctuelle
        "portee_lumiere_m": 12.0,
        "echelle": 1.0,
    },

    # --- campements ennemis ---------------------------------------------
    # Ce sont des SALLES DE COMBAT, pas du decor.
    #
    # LE RAYON NE S'ECRIT PAS, IL SE DERIVE. La passe precedente declarait
    # `rayon: 12.0` dans le commentaire meme qui citait §2.2 (« ligne max <=
    # vision du grunt ») : 12 m de rayon font 24 m de ligne pour 20 m de vision,
    # soit 4 m de TIR GRATUIT sur les trois camps. Le moteur l'a detecte, l'a
    # ecrit en warn a chaque chargement et l'a epingle par un test — et rien ne
    # l'a corrige, parce que le nombre fautif etait un litteral.
    #
    #   rayon = min(vision des archetypes presents) / 2
    #
    # `archetypes` declare QUI apparait ici ; le nombre en decoule (voir la
    # derivation juste apres la SPEC). Le jour ou seul l'archer (vision 35 m)
    # peuple un camp, le camp s'agrandit tout seul.
    #
    # Effet de bord VOULU : le rayon retrecit de 12 a 10 m, donc les apparitions
    # aussi (elles sont exprimees en fraction du rayon). L'essaim ferme la
    # distance encore plus surement — §2.1 y gagne au lieu d'y perdre.
    "campements": {
        "fractions": [0.22, 0.52, 0.74],   # position le long du chemin
        "archetypes": ["grunt", "archer"],
        # LA SPEC DE COMBAT — `map-design-intention.md` §1 l'exige par salle, et
        # elle manquait entièrement : le manifeste portait des positions
        # d'apparition sans jamais dire QUI apparaît, en quelle quantité, contre
        # quel arsenal, ni quand la salle est finie. « `verrou_xyz` est une
        # position sans règle » tant que ces champs n'existent pas.
        #
        # Ce qui suit est un CHOIX D'AUTEUR assumé (la règle demande de le
        # déclarer, pas de le calculer) ; ce qui s'en DÉRIVE — les durées — est
        # calculé plus bas et publié à côté, pour que la spec se vérifie au
        # chargement au lieu de se croire.
        #
        # La composition monte le long du chemin : le premier camp apprend, le
        # dernier verrouille l'approche du village.
        "composition": [
            {"grunt": 5, "archer": 2},      # camp 1 — l'essaim, on découvre
            {"grunt": 6, "archer": 3},      # camp 2 — le tir entre en jeu
            {"grunt": 7, "archer": 4},      # camp 3 — dernier verrou
        ],
        # Ce que le joueur est censé porter à ce moment de la course. Sert à
        # dériver la durée d'engagement ; c'est le dps qui compte, pas le nom.
        # 168 dps = le fusil (`reference_weapon_stats_real_source_viewmodel_arena`).
        "arsenal_dps": 168.0,
        "condition_sortie": "tous morts",
        # `rayon`, `vision_min_m` et `apparition_rayon` sont DERIVES plus bas.
        "apparitions": 7,
        # Fractions du rayon, pas des metres : elles suivent le camp.
        "apparition_fraction": [0.50, 0.85],
        "abris": 6,                        # blocs >= 1,8 m : ils cassent la vue
        # Fractions du rayon, meme raison.
        "abri_fraction": [0.42, 0.92],
        # Demi-largeur de route qu'AUCUN prop de camp ne franchit.
        # 2,2 (chemin) + 1,4 de garde : mesure precedente, 57 objets
        # empietaient dont les 3 feux pile sur l'axe.
        "couloir_libre": 3.6,
    },

    # --- places ---------------------------------------------------------
    "clairiere_spawn": {"xy": [-124.0, -10.0], "rayon": 15.0},
    # `rayon_aplani` deborde le rempart : le sol montait 6,9 m AU-DESSUS du
    # seuil de la porte, qui etait donc enterree. On aplanit ce sur quoi on
    # batit, rempart compris — sinon la porte n'est pas une entree.
    "place_village": {"xy": [90.0, 0.0], "rayon": 31.0, "rayon_aplani": 52.0},

    # --- végétation -----------------------------------------------------
    # Futaies : le contraste densité/vide est ce qui fait une forêt. Un semis
    # régulier de même effectif se lit comme un verger.
    # Densites divisees par ~1,8 SUR UNE CARTE 1,5x PLUS GRANDE : mesure
    # precedente 99,6 props/1000 m², soit un objet tous les 10 m² — d'ou
    # « trop charge ». Cible ~40, ce qui laisse respirer sans clairsemer.
    "futaies": 30,
    "futaie_rayon": [16.0, 38.0],
    "arbres_total": 1150,
    "part_en_futaie": 0.82,      # le reste en isolés, pour lier les massifs
    "sousbois_total": 620,
    # Touffes d'herbe : petites et basses, elles habillent le sol sans
    # encombrer. Elles ne comptent pas comme « charge » — c'est
    # l'absence de tapis qui faisait paraitre le sol nu entre les arbres.
    "herbe_total": 3200,
    "rochers_total": 110,
    # Dégagement : rien de solide à moins de ça de l'axe du chemin.
    # 2,2 + 1,6 : le joueur (rayon 0,3) ne frotte jamais un tronc.
    # spawn-clearance.md — c'est le DÉCOR qui cède devant une position imposée.
    "degagement_chemin": 3.8,
    # La vegetation se tenait a 5,5 m de l'axe d'une riviere large de
    # 10 m : elle poussait donc DANS l'eau et masquait la berge.
    "degagement_riviere": 12.0,

    # --- faune ------------------------------------------------------------
    # Des zones, pas des positions : le moteur y fera deambuler ses betes. Le
    # milieu est choisi par des CRITERES DE TERRAIN, pas a la main — ainsi les
    # zones suivent le relief si le trace change.
    #
    # Regle commune a toutes : jamais sur le chemin (on ne bute pas dans un
    # cerf), jamais dans un campement (c'est une salle de combat), jamais dans
    # l'eau, jamais sur une pente infranchissable.
    "faune": {
        "milieux": {
            # (pente max, distance au chemin [min,max], distance a la riviere
            #  [min,max], distance au village [min,max], rayon de zone)
            "pre":       {"pente": 11.0, "chemin": [14.0, 55.0], "riviere": [22.0, 999.0],
                          "village": [50.0, 999.0], "rayon": 22.0},
            # UNE BERGE SE DEFINIT PAR SA HAUTEUR AU-DESSUS DE LA NAPPE, pas par
            # son eloignement en plan. Avec le seul critere `riviere: [11, 24]`,
            # la zone des manchots est sortie a z = 11,63 m pour un plan d'eau a
            # -2 m : cinq betes de rivage posees 13 m au-dessus de leur rivage,
            # sur un flanc de colline. Le critere etait 2D pour un milieu qui est
            # defini par une relation verticale.
            # Le profil de l'eau est deja calcule station par station — il suffit
            # de le lire (`relief.niveau_eau_en`), pas de le redecouvrir.
            # La bande EN PLAN s'elargit (11-24 -> 9-34) parce que ce n'est plus
            # elle qui definit le milieu : elle degrossit, et `hauteur_sur_nappe`
            # tranche. Mesure a l'appui, 2 000 des 4 000 tirages etaient rejetes
            # par cette seule borne, d'ou une zone de manchots sur deux.
            "berge":     {"pente": 16.0, "chemin": [10.0, 999.0], "riviere": [9.0, 34.0],
                          "village": [45.0, 999.0], "rayon": 16.0,
                          "hauteur_sur_nappe_m": [-0.5, 3.0],
                          # Une riviere est un couloir etroit : deux colonies
                          # separees de 42 m n'y tiennent pas deux fois. L'ecart
                          # se mesure ici le long du cours, pas en travers d'un pre.
                          "ecart": 16.0},
            "sous_bois": {"pente": 20.0, "chemin": [18.0, 70.0], "riviere": [20.0, 999.0],
                          "village": [55.0, 999.0], "rayon": 18.0},
            # LES POULES N'EXISTAIENT PAS. Deux zones demandees, zero posee, en
            # silence — et `chicken` restait declare dans `especes`, donc un role
            # sans instance (`map-design-intention.md` §5.1). Les compteurs de
            # rejet ont nomme la cause : 1 722 tirages perdus sur la couronne
            # [36, 52] et 1 164 sur la pente. La borne interieure reste a 36 m
            # (le rempart est a 29 m, une zone de rayon 15 le chevaucherait), la
            # borne exterieure s'ouvre, et la pente admise passe a 18 deg — une
            # basse-cour sur un pre en pente douce reste une basse-cour.
            # `ecart` propre au milieu : les 26 m d'ecart des TROUPEAUX n'ont pas
            # de sens pour des betes de basse-cour. Poules et chien vivent aux
            # memes abords — c'est meme ce qui rend le lieu habite. Meme
            # exception que le predateur sur la crete, et pour la meme raison :
            # une regle de troupeau appliquee a ce qui n'en est pas un ne laisse
            # aucun creneau (mesure : 388 tirages perdus sur ce seul critere).
            "abords":    {"pente": 18.0, "chemin": [8.0, 999.0], "riviere": [20.0, 999.0],
                          "village": [36.0, 70.0], "rayon": 15.0, "ecart": 10.0},
            # LA CRETE — le seul point haut INTERIEUR de la carte. Le milieu
            # « hauteurs » generique ne trouvait plus rien une fois les bords
            # resserres : tout le relief eleve etait la ceinture, exclue.
            # `sur_crete` contraint a la crete elle-meme, hors du col : une bete
            # dans le col se mettrait en travers de la revelation du village.
            "crete":     {"pente": 30.0, "chemin": [22.0, 999.0], "riviere": [25.0, 999.0],
                          "village": [55.0, 999.0], "rayon": 15.0, "sur_crete": True,
                          # Un predateur CHEVAUCHE le territoire de ses proies :
                          # lui imposer les 26 m d'ecart des troupeaux ne
                          # laissait aucun creneau sur la crete.
                          "ecart": 6.0},
        },
        # espece -> (milieu, nb de zones, effectif par zone, couleur de controle)
        "especes": {
            "deer":    ("pre", 2, 4, "#C8A05A"),
            "horse":   ("pre", 1, 3, "#8B6B4A"),
            "kitty":   ("sous_bois", 2, 2, "#D8CBB4"),
            "pinguin": ("berge", 2, 5, "#2E4B63"),
            "dog":     ("abords", 1, 3, "#A8763F"),
            "chicken": ("abords", 2, 6, "#E8D9B0"),
            "tiger":   ("crete", 1, 1, "#D2782E"),
        },
        # Deux zones ne se chevauchent pas : sinon les troupeaux se melangent
        # et la « zone » ne veut plus rien dire.
        "ecart_min": 26.0,
    },

    # --- couleurs -------------------------------------------------------
    # Le kit est nativement en pastel menthe (grass #2CD8B8, leafsGreen
    # #29C9AB). Aucune texture dedans → toute la couleur des props tient ici.
    # sRGB, converti en linéaire à l'application.
    # Quatre nuances par matiere minerale. Les blocs partagent leur mesh mais
    # plus leur couleur : c'est ce qui casse l'amas uniforme.
    "nuances_pierre": {
        "stone":     ["#9AA0A2", "#8A9296", "#A6A69C", "#7E888C"],
        "stoneDark": ["#6E7478", "#63696D", "#787C74", "#5A6165"],
        "dirt":      ["#9A8768", "#8B7A5E", "#A6957A", "#7E6F55"],
    },
    "palette": {
        # Desaturees d'environ un tiers : le kit sature tire vers le jouet.
        # On ne peut pas sortir du facette low-poly sans changer d'assets,
        # mais on peut sortir du plastique.
        "grass":        "#6E9450",
        "leafsGreen":   "#5F8447",
        "leafsDark":    "#3E5F33",
        "leafsFall":    "#B4783C",
        "dirt":         "#8B7B62",
        "dirtDark":     "#7A5B3C",
        "stone":        "#8E9088",
        "stoneDark":    "#686A64",
        "wood":         "#9A6838",
        "woodBark":     "#7C5230",
        "woodBarkDark": "#5F3D23",
        "woodDark":     "#4E3320",
        "woodBirch":    "#DED4BC",
        "woodInner":    "#C8AE86",
        "water":        "#42809E",
        "corn":         "#DCAE58",
        # Accents du kit ramenes vers des matieres : les tentes sortaient
        # en rose vif au milieu d'un vallon.
        "colorRed":     "#9E5245",
        "colorRedDark": "#7A3D34",
        "colorTan":     "#C0A57E",
        "colorYellow":  "#C9A24E",
        "colorPurple":  "#7B6E9C",
        "colorWhite":   "#D8D4C8",
        "_defaultMat":  "#B4B4AC",
    },
    # Sol texturé : jeux PBR de assets/textures-v1/terrain/.
    # Teinte de multiplication pour ramener le photoréalisme vers le stylisé du
    # kit — sans elle, sol photo + props facettés jurent.
    # SOLS — textures du chateau ramenees en 1K (`assets/textures/castle_1k/`,
    # 69,2 -> 5,3 Mo). Elles remplacent la bibliotheque `textures-v1` : meme
    # famille visuelle que le Hall, et l'albedo de la falaise n'existant pas a
    # la source, on lui donne celui de la pierre avec sa propre normale.
    # Les teintes restent claires : ces textures portent deja leur couleur,
    # les assombrir les salirait.
    "sols": {
        "terrain": {"jeu": "castle", "diff": "grass_bc.png", "norm": "grass_n.png",
                    "uv_m": 4.0, "teinte": "#CDE0A4"},
        "chemin":  {"jeu": "castle", "diff": "pavement_bc.png", "norm": "pavement_n.png",
                    "uv_m": 3.0, "teinte": "#DCD2BE"},
        # DEUX roches étagées en altitude, comme une vraie montagne : chaude et
        # terreuse au pied, claire et minérale en haut. Une roche unique donne
        # une paroi qui se répète sur 26 m de haut.
        #
        # Mesuré sur les 17 jeux (sonde 07) : `mossy_rock` sort à #AEAD9E,
        # luminance 0,414, saturation 0,200 — il est DÉJÀ gris-vert désaturé.
        # Le multiplier par une teinte grise (#94998E) le tirait au noir à
        # l'ombre : c'est le « gris noir vert » signalé. Les teintes ci-dessous
        # ÉCLAIRCISSENT (> 1 en clair) au lieu d'assombrir.
        "falaise": {"jeu": "castle", "diff": "stone_bc.png", "norm": "cliff_n.png",
                    "uv_m": 5.0, "teinte": "#D3CDBE"},
        # `tundra` : luminance 0,531 et contraste 0,075, le plus élevé du lot —
        # c'est ce contraste qui donne du relief à une paroi vue de loin.
        "falaise_haute": {"jeu": "castle", "diff": "stone_bc.png", "norm": "stone_n.png",
                          "uv_m": 6.5, "teinte": "#E8E6DE"},
        # Troisieme sol : la berge. Le terrain change de matiere en approchant
        # de l'eau — c'est ce qui fait qu'une riviere se LIT de loin, bien plus
        # que la couleur de sa nappe.
        "berge":   {"jeu": "castle", "diff": "ground_bc.png", "norm": "ground_n.png",
                    "uv_m": 3.0, "teinte": "#E2D3AC"},
    },
}

# ---------------------------------------------------------------------------
# Derivations — ce que le litteral ci-dessus ne peut pas porter
# ---------------------------------------------------------------------------
# Un dictionnaire litteral ne sait pas lire un fichier. Tout nombre dont la
# VERITE vit ailleurs (ici : les genomes d'ennemis) se calcule donc apres coup,
# jamais en le recopiant a la main dans la SPEC.

VISIONS = visions_archetypes()
PV = pv_archetypes()
OEIL_JOUEUR_M = SPEC["joueur"]["oeil_m"]

_camp = SPEC["campements"]
# `.get` avec repli : si un genome disparait, on prefere une carte trop PETITE
# (ligne courte, personne ne subit de tir gratuit) a une carte trop grande.
_camp["vision_min_m"] = min(
    (VISIONS.get(a, 20.0) for a in _camp["archetypes"]), default=20.0)
_camp["rayon"] = round(_camp["vision_min_m"] / 2.0, 2)
# Les distances d'apparition et d'abri suivent le rayon : declarees en metres,
# elles auraient survecu au retrecissement du camp et poseraient desormais des
# abris HORS de la clairiere qu'ils sont censes abriter.
_camp["apparition_rayon"] = [round(_camp["rayon"] * f, 2)
                             for f in _camp["apparition_fraction"]]
_camp["abri_rayon"] = [round(_camp["rayon"] * f, 2) for f in _camp["abri_fraction"]]
# Les DEUX bornes de hauteur d'un abri, dérivées de l'œil du joueur. Voir le
# commentaire de la boucle des abris : sous la première il ment, au-dessus de la
# seconde il cesse d'être une couverture pour devenir un mur.
_camp["abri_hauteur_m"] = [round(OEIL_JOUEUR_M + 0.25, 2),
                           round(OEIL_JOUEUR_M + 1.30, 2)]
# RAYON MAXIMAL D'UN ABRI — dérivé de l'anneau, pas choisi.
#
# Les `abris` se répartissent sur un anneau de rayon moyen `r_anneau`. L'écart
# d'arc entre deux voisins vaut donc `2·pi·r_anneau / n`. Un abri qui occupe le
# quart de cet arc laisse les trois autres quarts en passage : l'anneau reste
# une couverture qu'on contourne, pas une palissade.
# Mesuré au rendu ce qu'un abri non borné produit : deux blocs de plus de 4 m
# qui fermaient la clairière et mordaient sur le chemin.
_r_anneau = sum(_camp["abri_rayon"]) / 2.0
_camp["abri_rayon_max_m"] = round(
    (2.0 * math.pi * _r_anneau / _camp["abris"]) / 4.0, 2)

# ---------------------------------------------------------------------------
# Defauts — ce que la carte a RATE, et qui doit se voir
# ---------------------------------------------------------------------------
# Deux systemes de placement rataient une part de leur cible sans qu'aucune
# sortie ne le dise : 8 zones de faune sur 11 (les poules : zero sur deux) et
# 15 abris sur 18. Le manifeste rapportait l'obtenu sans la demande, ce qui se
# lit comme un succes.
#
# `map-design-patterns.md` §13 : « zero mesure n'est pas vert, c'est aveugle ».
# Un compteur a cible publie donc DEMANDE, OBTENU et REJETS PAR CAUSE — la
# cause est ce qui rend le defaut corrigeable, le compte seul ne dit pas ou
# desserrer.
#
# Effet recherche : `"defauts": []` devient une PREUVE. Aujourd'hui l'absence
# de la cle ne prouve rien du tout.

DEFAUTS = []

# ---------------------------------------------------------------------------
# Retouches d'auteur — la couche « exception »
# ---------------------------------------------------------------------------
# Ce fichier bâtit la carte par des RÈGLES. Mais une règle juste produit parfois
# un cas faux, et il ne faut ni tordre la règle pour un cas, ni renoncer à le
# corriger. C'est la quatrième couche du modèle à quatre étages
# (framework / definition / behaviour / **exception**) : un placement décidé à
# la main, pour cette pièce-là et pour aucune autre.
#
# Le trajet est donc :
#   1. `vallon.py` bâtit, puis écrit un INSTANTANÉ de toutes les poses ;
#   2. Antoine déplace ce qu'il veut dans Blender ;
#   3. `20_retouches.py` compare la scène à l'instantané et n'écrit QUE les
#      différences — donc rien s'il n'a rien bougé ;
#   4. la cuisson suivante les réapplique ici.
#
# 🚨 Une retouche PÉRIME. Les noms d'objets dépendent de l'ordre de création :
# si la graine ou le semis change, `Mesh tree_oak.042` ne désigne plus la même
# pièce, et réappliquer aveuglément déplacerait un arbre au hasard. Chaque
# retouche porte donc la position qu'elle a CORRIGÉE ; si la pièce reconstruite
# n'y est plus, la retouche est refusée et PUBLIÉE comme défaut — jamais
# appliquée à côté, jamais tue.
RETOUCHES_FICHIER = os.path.join(
    SORTIE, "vallon_retouches.json")
REFERENCE_FICHIER = os.path.join(
    SORTIE, "vallon_pose_reference.json")
# Tolérance de reconnaissance. 0,25 m : au-delà, ce n'est plus la même pièce
# posée par la même règle, c'est une autre pièce qui a hérité du nom.
RETOUCHE_TOLERANCE_M = 0.25


def noter_defaut(quoi, demande, obtenu, cause, rejets=None):
    """Consigne un manque. Ne consigne RIEN quand la cible est atteinte."""
    if obtenu >= demande:
        return
    DEFAUTS.append({
        "quoi": quoi,
        "demande": demande,
        "obtenu": obtenu,
        "manque": demande - obtenu,
        "gravite": "alerte" if obtenu == 0 else "avertissement",
        "cause": cause,
        "rejets": dict(sorted((rejets or {}).items(), key=lambda kv: -kv[1])),
    })

PALETTE_SOURCES = {
    "grass": "ground_grass.glb",
    "dirt": "ground_pathStraight.glb",
    "water": "ground_riverStraight.glb",
    "stone": "cliff_block_stone.glb",
    "wood": "bridge_wood.glb",
}

# LE TRONC, PAR SA MATIERE. Les pieces du kit portent `wood`, `woodBark`,
# `woodBarkDark`, `woodDark`, `woodBirch`, `woodInner` pour le bois et
# `leafsGreen/Dark/Fall` pour le feuillage. Restreindre la mesure d'emprise au
# prefixe `wood` donne le tronc ; sans ce filtre on mesure la jupe basse du
# houppier, et le rayon median passe de 0,2 a 0,72 m (mesure).
MATIERES_TRONC = ("wood",)

# CORRECTION D'ORIENTATION, PAR PIÈCE.
#
# Les GLB d'un même kit ne partagent pas tous la même convention d'« avant ».
# Relevé le 2026-08-17 : Antoine a fait pivoter de ~180° les DEUX exemplaires de
# `tent_detailedClosed` — et aucune des trois autres tentes, qu'il a laissées à
# 2-4° près (du bruit de manipulation). Ce n'est donc pas une règle de côté ni
# de camp : c'est ce modèle-là qui regarde à l'opposé des autres.
#
# La correction vit ici, appliquée à la pose, plutôt que dans chaque site
# d'appel : sinon la même rotation serait réécrite partout où la pièce sert, et
# le jour où une seule copie est corrigée, les autres restent à l'envers.
ORIENTATION_CORRIGEE = {
    "tent_detailedClosed.glb": math.pi,
}

# TAILLE RÉELLE DE CERTAINES PIÈCES, en mètres — et l'échelle s'en déduit.
#
# 🚨 Une échelle de kit est dérivée d'UNE famille et ne vaut que pour elle.
# `echelle_nature = 4.0` vient de la tuile de sol (1,0 → 4 m de chemin) ;
# `echelle_village = 6.0` du faîte d'une chaumière (0,93 → 5,6 m). Appliquées
# à des PLANTES, elles n'ont plus aucun sens : mesuré le 2026-08-17, le maïs
# du village sortait à **4,50 m de haut** et le blé à 1,93 m. Un champ de maïs
# de quatre mètres et demi écrase le village qu'il borde.
#
# C'est exactement la faute de `offset_base` : une grandeur juste pour une
# famille, appliquée à une autre. Le remède est le même — mesurer la pièce et
# DÉRIVER son échelle de la taille qu'elle doit avoir dans le monde.
TAILLES_REELLES = {
    "crops_cornStageC.glb": 2.20,     # un maïs adulte
    "crops_wheatStageB.glb": 1.10,    # un blé à maturité
    "crops_leafsStageB.glb": 0.80,    # un rang de légumes-feuilles
    "crop_pumpkin.glb": 0.45,         # une citrouille, posée au sol
}

# Essences (fichier, poids) — noms vérifiés sur kit_catalog.json.
FEUILLUS = [("tree_default.glb", 3), ("tree_oak.glb", 3), ("tree_detailed.glb", 3),
            ("tree_fat.glb", 2), ("tree_tall.glb", 2), ("tree_simple.glb", 2),
            ("tree_default_dark.glb", 2), ("tree_oak_dark.glb", 2),
            ("tree_detailed_dark.glb", 2), ("tree_thin.glb", 1), ("tree_small.glb", 1)]
CONIFERES = [("tree_pineDefaultA.glb", 3), ("tree_pineDefaultB.glb", 3),
             ("tree_pineTallA.glb", 3), ("tree_pineTallB.glb", 2),
             ("tree_pineTallC.glb", 2), ("tree_pineRoundA.glb", 2),
             ("tree_pineRoundD.glb", 2), ("tree_pineSmallA.glb", 1),
             ("tree_pineGroundA.glb", 1)]
AUTOMNE = [("tree_default_fall.glb", 3), ("tree_oak_fall.glb", 3),
           ("tree_detailed_fall.glb", 2), ("tree_fat_fall.glb", 2),
           ("tree_simple_fall.glb", 1), ("tree_tall_fall.glb", 1)]
SOUSBOIS = [("grass_large.glb", 5), ("grass_leafs.glb", 4), ("grass_leafsLarge.glb", 3),
            ("grass.glb", 3), ("plant_bush.glb", 3), ("plant_bushLarge.glb", 2),
            ("plant_bushDetailed.glb", 2), ("plant_bushSmall.glb", 2),
            ("plant_flatTall.glb", 2), ("plant_flatShort.glb", 2),
            ("flower_redA.glb", 1), ("flower_yellowB.glb", 1), ("flower_purpleC.glb", 1),
            ("mushroom_red.glb", 1), ("mushroom_tanGroup.glb", 1),
            ("mushroom_redGroup.glb", 1), ("rock_smallA.glb", 2), ("rock_smallD.glb", 2),
            ("stump_round.glb", 1), ("stump_old.glb", 1), ("log.glb", 1)]
# Les pièces `rock_*` portent le matériau `dirt` (beige) et les `stone_*` le
# gris : à parts égales, le décor virait au tas de sable. On pèse vers la
# pierre, et on garde quelques blocs terreux pour ne pas uniformiser.
ROCHERS = [("stone_largeA.glb", 3), ("stone_largeC.glb", 3), ("stone_largeE.glb", 2),
           ("stone_tallB.glb", 2), ("stone_tallF.glb", 2), ("stone_smallTopA.glb", 2),
           ("rock_largeA.glb", 1), ("rock_tallD.glb", 1)]

# LE VILLAGE SE DÉRIVE D'UNE TRAME, il ne se pose plus à la main.
#
# La version précédente était une table d'offsets écrits un par un, avec des
# caps arbitraires (45, −60, −10, −35, 15, 80, 8, −20 deg). Trois défauts, tous
# mesurés :
#   - écart MINIMAL de 5,62 m entre deux maisons hautes de 5,6 m : elles se
#     touchaient presque, et rien ne le garantissait dans un sens ou l'autre ;
#   - aucun bâtiment ne regardait quoi que ce soit — ni la porte, ni une rue ;
#   - le jour où le rayon de la place bouge, la table ne suit pas. C'est la
#     faute que ce fichier passe son temps à corriger : une grandeur écrite deux
#     fois (ici, la géométrie du village et sa description) finit par diverger.
#
# On déclare donc des RÔLES, et la trame place. Chaque rôle dit ce que la pièce
# fait dans le village, pas où elle est :
#   `centre` — sur la place, au bout de la rue ;
#   `fond`   — ferme la perspective en face de la porte (le clocher se voit du
#              seuil, c'est ce qui donne au village un « fond de scène ») ;
#   `place`  — encadre la place, façade vers elle ;
#   `rue`    — parcelle le long de la rue, façade vers la rue.
VILLAGE_TRAME = [
    ("buildings/red/building_well_red.gltf", "centre"),
    ("buildings/red/building_church_red.gltf", "fond"),
    ("buildings/red/building_tavern_red.gltf", "place"),
    ("buildings/red/building_market_red.gltf", "place"),
    ("buildings/red/building_home_A_red.gltf", "rue"),
    ("buildings/red/building_home_B_red.gltf", "rue"),
    ("buildings/red/building_home_A_red.gltf", "rue"),
    ("buildings/red/building_home_B_red.gltf", "rue"),
    ("buildings/red/building_home_A_red.gltf", "rue"),
    ("buildings/red/building_home_B_red.gltf", "rue"),
]

# ---------------------------------------------------------------------------
# Utilitaires
# ---------------------------------------------------------------------------


def srgb_lineaire(c):
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def hex_lineaire(code):
    code = code.lstrip("#")
    return tuple(srgb_lineaire(int(code[i:i + 2], 16) / 255.0) for i in (0, 2, 4)) + (1.0,)


def wipe():
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)
    for coll in (bpy.data.meshes, bpy.data.materials, bpy.data.images, bpy.data.collections):
        for block in list(coll):
            try:
                coll.remove(block)
            except (RuntimeError, ReferenceError):
                pass
    # Les proprietes custom de la SCENE ne partent pas avec les objets. Le
    # marqueur de `14_ancrage.py` survivait donc a la reconstruction, et la
    # passe suivante se croyait deja cuite : la carte sortait sans ancrage, sans
    # que rien ne le signale. C'est ici que « la scene est neuve » se declare —
    # dans le seul script qui a le droit de l'affirmer.
    for cle in list(bpy.context.scene.keys()):
        if cle.startswith("vallon_"):
            del bpy.context.scene[cle]


def collection(nom):
    coll = bpy.data.collections.get(nom)
    if coll is None:
        coll = bpy.data.collections.new(nom)
        bpy.context.scene.collection.children.link(coll)
    return coll


def lissage_doux(x, b0, b1):
    if b1 <= b0:
        return 0.0 if x < b0 else 1.0
    t = max(0.0, min(1.0, (x - b0) / (b1 - b0)))
    return t * t * (3.0 - 2.0 * t)


def distance_polyligne(x, y, points):
    meilleure = float("inf")
    for i in range(len(points) - 1):
        ax, ay = points[i]
        bx, by = points[i + 1]
        dx, dy = bx - ax, by - ay
        long2 = dx * dx + dy * dy
        if long2 <= 1e-9:
            continue
        t = max(0.0, min(1.0, ((x - ax) * dx + (y - ay) * dy) / long2))
        d = math.hypot(x - (ax + t * dx), y - (ay + t * dy))
        if d < meilleure:
            meilleure = d
    return meilleure


def densifier(points, pas=2.0, passes=3):
    """Chaikin puis rééchantillonnage à pas CONSTANT."""
    courbe = [tuple(p) for p in points]
    for _ in range(passes):
        neuf = [courbe[0]]
        for i in range(len(courbe) - 1):
            ax, ay = courbe[i]
            bx, by = courbe[i + 1]
            neuf.append((ax * 0.75 + bx * 0.25, ay * 0.75 + by * 0.25))
            neuf.append((ax * 0.25 + bx * 0.75, ay * 0.25 + by * 0.75))
        neuf.append(courbe[-1])
        courbe = neuf
    sortie = [courbe[0]]
    reste = pas
    for i in range(len(courbe) - 1):
        ax, ay = courbe[i]
        bx, by = courbe[i + 1]
        seg = math.hypot(bx - ax, by - ay)
        if seg <= 1e-9:
            continue
        pos = 0.0
        while pos + reste <= seg:
            pos += reste
            f = pos / seg
            sortie.append((ax + (bx - ax) * f, ay + (by - ay) * f))
            reste = pas
        reste -= seg - pos
    if math.dist(sortie[-1], courbe[-1]) > 1e-6:
        sortie.append(courbe[-1])
    return sortie


def objets_autores():
    """Les objets que la carte a POSÉS — ceux qu'on peut retoucher.

    Écarte les prototypes cachés, l'aperçu de la faune (des statues de contrôle
    qui ne sont pas cuites) et les instruments de vue.
    """
    ecartees = {"_proto", "_src", "faune_apercu", "faune_controle", "collisions"}
    for obj in bpy.data.objects:
        if obj.type != "MESH":
            continue
        if any(c.name in ecartees for c in obj.users_collection):
            continue
        yield obj


def pose_de(obj):
    """La pose d'un objet, arrondie. Cinq nombres suffisent : les pièces sont
    posées à l'échelle uniforme et ne pivotent qu'autour de la verticale (à
    l'inclinaison près, qui est du décor et pas du placement)."""
    return [round(obj.location.x, 3), round(obj.location.y, 3),
            round(obj.location.z, 3), round(obj.rotation_euler.z, 4),
            round(obj.scale.x, 4)]


def refuser_si_travail_non_capture():
    """🚨 NE JAMAIS EFFACER UN DÉPLACEMENT FAIT À LA MAIN ET PAS ENCORE CAPTURÉ.

    `main()` commence par `wipe()`. Si Antoine vient de déplacer des pièces dans
    Blender et qu'on recuit, son travail disparaît **sans un mot** — c'est
    arrivé le 2026-08-17 : il repositionnait les barrières, j'ai relancé la
    cuisson pour vérifier autre chose, et la capture suivante a rendu zéro.

    On compare donc la scène à l'instantané AVANT de toucher à quoi que ce soit.
    Toute différence non encore consignée dans `vallon_retouches.json` fait
    REFUSER la cuisson, avec la commande qui la sauve.

    Même doctrine que `92_cellules.py`, qui refuse d'écraser une carte déjà
    cuite : un cuiseur n'efface jamais en silence.

    L'échappatoire passe par une propriété de SCÈNE et non par une variable
    d'environnement : le script s'exécute DANS Blender, donc un `export` fait
    dans le terminal ne l'atteindrait jamais. Pour jeter volontairement les
    déplacements :

        bpy.context.scene["vallon_jeter_retouches"] = True

    Elle se consomme à l'usage — on ne la laisse pas armée, sinon le garde est
    désactivé pour toutes les cuissons suivantes sans que personne ne le sache.
    """
    if bpy.context.scene.get("vallon_jeter_retouches"):
        del bpy.context.scene["vallon_jeter_retouches"]
        return None
    if not os.path.exists(REFERENCE_FICHIER):
        return None                      # première cuisson : rien à protéger
    with open(REFERENCE_FICHIER, encoding="utf-8") as fh:
        reference = json.load(fh)
    connues = {}
    if os.path.exists(RETOUCHES_FICHIER):
        with open(RETOUCHES_FICHIER, encoding="utf-8") as fh:
            connues = json.load(fh)

    bougees = []
    for obj in objets_autores():
        avant = reference.get(obj.name)
        if avant is None:
            continue
        apres = pose_de(obj)
        if math.dist(apres[:3], avant[:3]) <= 0.01 and abs(apres[3] - avant[3]) <= 0.01:
            continue
        # Déjà consignée à cette position ? Alors elle est en sécurité.
        deja = connues.get(obj.name)
        if deja and math.dist(deja["apres"][:3], apres[:3]) <= 0.01:
            continue
        bougees.append({"objet": obj.name,
                        "ecart_m": round(math.dist(apres[:3], avant[:3]), 2)})
    return bougees or None


def ecrire_reference():
    """Écrit l'instantané des poses. C'est LUI que `20_retouches.py` compare."""
    ref = {obj.name: pose_de(obj) for obj in objets_autores()}
    with open(REFERENCE_FICHIER, "w", encoding="utf-8") as fh:
        json.dump(ref, fh, ensure_ascii=False, separators=(",", ":"))
    return len(ref)


def appliquer_retouches():
    """Réapplique les déplacements faits à la main, et REFUSE ceux qui ont périmé.

    Une retouche porte `avant` : la pose que la règle avait produite quand elle
    a été corrigée. Si la pièce reconstruite n'y est plus, c'est que le semis a
    changé sous elle — le nom désigne alors une AUTRE pièce, et l'appliquer
    déplacerait un objet au hasard sans que rien ne le dise.
    """
    if not os.path.exists(RETOUCHES_FICHIER):
        return {"fichier": "absent", "appliquees": 0}
    with open(RETOUCHES_FICHIER, encoding="utf-8") as fh:
        retouches = json.load(fh)

    par_nom = {obj.name: obj for obj in objets_autores()}
    appliquees, perimees, introuvables = 0, [], []
    for nom, r in sorted(retouches.items()):
        obj = par_nom.get(nom)
        if obj is None:
            introuvables.append(nom)
            continue
        avant, apres = r["avant"], r["apres"]
        ecart = math.dist(pose_de(obj)[:3], avant[:3])
        if ecart > RETOUCHE_TOLERANCE_M:
            perimees.append({"objet": nom, "ecart_m": round(ecart, 2)})
            continue
        obj.location = (apres[0], apres[1], apres[2])
        obj.rotation_euler = (obj.rotation_euler.x, obj.rotation_euler.y, apres[3])
        obj.scale = (apres[4], apres[4], apres[4])
        appliquees += 1

    if perimees or introuvables:
        noter_defaut(
            "retouches.perimees", len(retouches), appliquees,
            f"{len(perimees)} retouche(s) portent sur une piece qui a bouge de plus "
            f"de {RETOUCHE_TOLERANCE_M} m, {len(introuvables)} sur une piece disparue. "
            "Le semis a change sous elles : les refaire a la main, ou retirer leur "
            "ligne de vallon_retouches.json",
            {"perimees": len(perimees), "introuvables": len(introuvables)})
    return {"lues": len(retouches), "appliquees": appliquees,
            "perimees": perimees[:8], "introuvables": introuvables[:8]}


def dans_une_vue(x, y, vues):
    """Le point est-il dans un cône de vue protégé ?

    `vues` est une liste de `(depuis, vers, demi_pres, demi_loin, portee)` déjà
    résolue en coordonnées. Le cône s'évase de `demi_pres` à l'observateur
    jusqu'à `demi_loin` à la cible : une vue est un TRIANGLE, pas un couloir —
    ce qui est loin bouche plus large que ce qui est près.

    Ne dit rien de la hauteur : c'est à l'appelant de ne consulter cette
    fonction que pour les pièces qui dépassent l'œil du joueur.
    """
    for (ax, ay), (bx, by), demi_pres, demi_loin, portee in vues:
        dx, dy = bx - ax, by - ay
        n2 = dx * dx + dy * dy
        if n2 < 1e-9:
            continue
        t = ((x - ax) * dx + (y - ay) * dy) / n2
        if not 0.0 <= t <= 1.0:
            continue
        n = math.sqrt(n2)
        if t * n > portee:
            continue
        lat = abs(-(x - ax) * dy + (y - ay) * dx) / n
        if lat <= demi_pres + (demi_loin - demi_pres) * t:
            return True
    return False


def _tangente_chemin(relief, x, y):
    """Direction LOCALE du chemin au point le plus proche de (x, y).

    Sert aux pièces qui doivent suivre la route sans être posées dessus — une
    palissade de bord de route, par exemple. Prendre le cap du CENTRE du camp
    pour toute une palissade la fait diverger du chemin à ses extrémités, ce que
    les rotations faites à la main corrigeaient une par une.
    """
    pts = relief.chemin
    i = min(range(len(pts)), key=lambda k: (pts[k][0] - x) ** 2 + (pts[k][1] - y) ** 2)
    a = pts[max(0, i - 1)]
    b = pts[min(len(pts) - 1, i + 1)]
    dx, dy = b[0] - a[0], b[1] - a[1]
    n = math.hypot(dx, dy) or 1.0
    return dx / n, dy / n


def fusionner_solides(*kits):
    """Reunit les emprises solides de plusieurs kits, par famille.

    Chaque `Kit` ne connait QUE ce qu'il a pose lui-meme. Publier
    `nature.solides` seul aurait rate les 22 rochers de bouchage (poses par
    `kit_roches`) et les 16 braseros (`kit_lampe`) — soit exactement le genre
    d'oubli partiel que ce chantier corrige. On fusionne donc explicitement, et
    ajouter un kit sans l'inscrire ici se verra : sa famille sera vide.
    """
    total = {}
    for kit in kits:
        for famille, pieces in kit.solides.items():
            total.setdefault(famille, []).extend(pieces)
    return {f: total[f] for f in sorted(total)}


def hors_couloir(ecart, rayon, libre):
    """Ramene un ecart angulaire hors du couloir de marche, sans retirer au sort.

    Un objet pose a l'angle `ecart` et au rayon `rayon` autour d'un point du
    chemin est a `rayon x sin(ecart)` de l'axe. Pour qu'il n'empiete pas, il
    faut `|rayon x sin(ecart)| >= libre`.

    L'ancienne version tirait un angle puis JETAIT l'objet s'il tombait dans le
    couloir. Un tirage qui marche en moyenne et rate parfois produit un manque
    silencieux : trois abris sur dix-huit ont disparu ainsi, et le manifeste
    rapportait 4, 5 et 6 comme si c'etait le plan. On resout donc la contrainte
    — l'abri est deplace, jamais supprime.

    Rend l'ecart le plus PROCHE du voulu qui respecte la contrainte, pour que la
    repartition reste celle qu'on a dessinee.
    """
    if rayon <= 0.0:
        return ecart
    seuil = math.asin(max(-1.0, min(1.0, libre / rayon)))
    # Ramener dans (-pi, pi] : le probleme est symetrique par rapport a l'axe.
    ecart = (ecart + math.pi) % math.tau - math.pi
    signe = 1.0 if ecart >= 0.0 else -1.0
    a = abs(ecart)
    if a < seuil:
        a = seuil
    elif a > math.pi - seuil:
        a = math.pi - seuil
    return signe * a


def choisir(rng, table):
    total = sum(p for _, p in table)
    t = rng.uniform(0.0, total)
    for nom, poids in table:
        t -= poids
        if t <= 0.0:
            return nom
    return table[-1][0]


# ---------------------------------------------------------------------------
# Relief
# ---------------------------------------------------------------------------


class Relief:
    """Champ de hauteur, en couches — l'ordre EST le design."""

    def __init__(self, spec):
        self.s = spec
        # Coordonnée de bruit modeste : une graine brute (2×10⁷) sort du
        # domaine utile d'un Perlin et rend une constante.
        self.zb = (spec["graine"] % 811) * 0.37
        self.chemin = densifier(spec["chemin"], 2.0)
        self.riviere = densifier(spec["riviere"], 2.0)
        # OU LE CHEMIN COUPE LA RIVIERE. Une constante `pont_xy` figee dans la
        # SPEC decrivait la meme grandeur que ce croisement : les deux ont
        # diverge des que le trace a change, et le pont s'est retrouve en biais,
        # a cote de la route. Une seule source, derivee.
        self.pont_idx = min(range(len(self.chemin)),
                            key=lambda i: distance_polyligne(self.chemin[i][0],
                                                             self.chemin[i][1], self.riviere))
        self.pont_xy = self.chemin[self.pont_idx]

        self._profil = None
        # L'eau AVANT le chemin : le lit se creuse sous une nappe plane, et le
        # profil du chemin devra ensuite savoir de combien enjamber.
        self._preparer_profil_riviere()
        self._preparer_profil()
        # Ou le chemin coupe l'anneau du rempart : c'est la porte. Calculee ici
        # pour que le champ de hauteur ET le semis vegetal la connaissent tous
        # les deux, au lieu de la redecouvrir chacun de son cote.
        v = spec["place_village"]
        r_mur = v["rayon"] - 2.0
        self.porte_xy = min(
            self.chemin,
            key=lambda pt: abs(math.hypot(pt[0] - v["xy"][0], pt[1] - v["xy"][1]) - r_mur))

    def _collines(self, x, y):
        e = self.s["collines_echelle"]
        v = mathutils.noise.noise(Vector((x * e, y * e, self.zb)))
        v += 0.5 * mathutils.noise.noise(Vector((x * e * 2.3, y * e * 2.3, self.zb + 11.0)))
        h = v * self.s["collines_amplitude"]
        g = self.s["grain_echelle"]
        h += mathutils.noise.noise(Vector((x * g, y * g, self.zb + 23.0))) * self.s["grain_amplitude"]
        return h

    def _gorge(self, x, y):
        """Part du rempart supprimee, la ou la riviere le traverse.

        Rend 1 au fond de la gorge (rempart efface, l'eau passe) et 0 au-dela
        de son evasement (rempart intact : ce sont les falaises qui bordent le
        passage). Se referme pres du bord pour que l'entaille ne devienne pas
        une porte de sortie.
        """
        g = self.s["gorge"]
        d = distance_polyligne(x, y, self.riviere)
        ouvert = 1.0 - lissage_doux(d, g["demi_largeur"], g["evasement"])
        bouchon = 1.0 - lissage_doux(self.bord(x, y), g["bouchon"], 1.0)
        return ouvert * bouchon

    def bord(self, x, y):
        """Distance normalisée au bord de la carte. 0 au centre, 1 sur l'enceinte.

        # 🚨 CETTE FONCTION EXISTE POUR NE PLUS RENDRE UN RECTANGLE

        Elle valait `max(|x|/dx, |y|/dy)` — la norme de **Tchebychev**, dont les
        lignes de niveau sont des RECTANGLES. Toute la ceinture en héritait :
        quatre parois droites et quatre angles vifs, ce qui donne à la carte vue
        de haut son allure de plateau de maquette. Le commentaire d'origine
        disait vouloir éviter exactement ça, et faisait onduler un rectangle au
        lieu de ne pas en produire un — une ondulation de 5 % ne rachète pas une
        forme fausse.

        La norme superelliptique corrige la FORME :

            t = ( (|x|/dx)^p + (|y|/dy)^p ) ^ (1/p)

        `p = 2` donne une ellipse, `p → ∞` redonne le rectangle. `p = 3` arrondit
        les angles tout en gardant l'essentiel de l'aire utile : c'est le premier
        exposant qui casse la lecture « boîte » sans rogner le jouable — mesuré à
        l'angle, l'emprise passe de 100 % à 87 % du rectangle, et le chemin, qui
        ne va jamais dans les coins, n'en perd rien.
        """
        p = self.s["bord_exposant"]
        return ((abs(x) / self.s["demi_x"]) ** p
                + (abs(y) / self.s["demi_y"]) ** p) ** (1.0 / p)

    def _cuvette(self, x, y):
        """La paroi du vallon. Sa forme n'est plus une boîte (cf. `bord`), et son
        PIED ondule — les deux sont nécessaires : une paroi ronde parfaitement
        régulière se lit comme un cirque de béton."""
        t = self.bord(x, y)
        ang = math.atan2(y, x)
        sinuosite = mathutils.noise.noise(
            Vector((math.cos(ang) * 2.4, math.sin(ang) * 2.4, self.zb + 3.0))
        ) * self.s["rim_ondulation"]
        debut = self.s["rim_debut"] + sinuosite
        # Puissance 1,6 : le pied reste doux, le haut se redresse — un profil
        # linéaire donne un talus régulier, pas une paroi.
        montee = (lissage_doux(t, debut, 1.0) ** 1.6) * self.s["rim_hauteur"]
        # LE REVERS. Passé la crête (`t > 1`), la montée sature et le terrain
        # devient PLAT — un plateau à 26 m de haut, dont la pente nulle le fait
        # même rendre en herbe. Avec l'ancienne norme rectangulaire il tombait
        # pile sur le bord du maillage et ne se voyait pas ; la superellipse
        # laisse de la place aux quatre coins (`t` y monte à 1,26) et le plateau
        # est apparu, vu de haut, comme une table verte autour du vallon.
        #
        # On lui donne donc son versant extérieur. La pente se DÉRIVE du seuil
        # de roche : au-delà de `pente_roche_deg`, le terrain rend en minéral —
        # un revers plus doux redeviendrait de la prairie perchée, ce qu'on
        # vient précisément de corriger. `+4°` de marge pour que le rendu ne
        # soit pas au seuil.
        pente = math.tan(math.radians(self.s["pente_roche_deg"] + 4.0))
        revers = max(0.0, t - 1.0) * self.s["demi_x"] * pente
        return (montee - revers) * (1.0 - self._gorge(x, y) * self.s["gorge"]["force"])

    def _crete(self, x, y):
        """La crête qui cache le village, percée d'un col là où passe le chemin."""
        c = self.s["crete"]
        dos = 1.0 - lissage_doux(abs(x - c["x"]), 0.0, c["epaisseur"])
        col = 1.0 - lissage_doux(abs(y - c["col_y"]), c["col_largeur"] * 0.35, c["col_largeur"])
        return dos * c["hauteur"] * (1.0 - col * 0.92)

    def _mamelon(self, x, y):
        m = self.s["mamelon"]
        d = math.hypot(x - m["xy"][0], y - m["xy"][1])
        return (1.0 - lissage_doux(d, m["rayon"] * 0.25, m["rayon"])) * m["hauteur"]

    def _base(self, x, y, creuser=True):
        h = self._cuvette(x, y) + self._collines(x, y) + self._crete(x, y) + self._mamelon(x, y)
        d = distance_polyligne(x, y, self.riviere)
        if not creuser:
            d = 1e9
        # Le lit se creuse SOUS LA NAPPE, jamais sous le terrain local. Creuser
        # une profondeur fixe depuis le sol donne un chenal qui suit les bosses
        # du relief pendant que la nappe, elle, reste plane : mesure, 37 des
        # 77 stations avaient leur eau enterrée jusqu'à 2,46 m sous la berge.
        # Une seule formule de poids, partagée avec le diagnostic — deux
        # formules pour la même grandeur, c'est la faute qui a coûté cette passe.
        poids_lit = self.poids_lit_en(d)
        if poids_lit > 0.0:
            lit = self.niveau_eau_en(x, y) - self.s["riviere_profondeur"]
            h = h * (1.0 - poids_lit) + lit * poids_lit
        for cle in ("clairiere_spawn", "place_village"):
            z = self.s[cle]
            cx, cy = z["xy"]
            # Le village aplanit AU-DELÀ de son rempart : on ne pose pas une
            # porte sur une pente. `rayon_aplani` était déclaré mais jamais lu.
            r_plat = z.get("rayon_aplani", z["rayon"])
            poids = 1.0 - lissage_doux(math.hypot(x - cx, y - cy), r_plat * 0.60, r_plat)
            if poids > 0.0:
                cible = self._cuvette(cx, cy) + self._collines(cx, cy) + self._crete(cx, cy)
                h = h * (1.0 - poids) + cible * poids
        return h

    def _preparer_profil_riviere(self):
        """Niveau de l'eau le long du cours, en descente stricte.

        Une nappe d'eau est HORIZONTALE : son altitude ne peut pas suivre les
        bosses du terrain, ni differer d'une rive a l'autre. La version
        precedente calculait la hauteur sommet par sommet depuis le sol local —
        la nappe gondolait et se vrillait en travers, ce qui est physiquement
        impossible et se voit immediatement d'en haut (elle n'attrape jamais le
        soleil sous le meme angle, donc elle ne brille jamais).

        On calcule donc UNE altitude par station, puis on interdit toute remontee
        vers l'aval : une riviere ne remonte pas.
        """
        berges = [self._base(x, y, creuser=False) for (x, y) in self.riviere]
        fen = 9
        lisse = []
        for i in range(len(berges)):
            a, b = max(0, i - fen // 2), min(len(berges), i + fen // 2 + 1)
            lisse.append(sum(berges[a:b]) / (b - a))
        marge = self.s["riviere_berge_libre"]
        niveau = [b - marge for b in lisse]
        # Descente stricte de l'amont (index 0, la cascade) vers l'aval.
        for i in range(1, len(niveau)):
            niveau[i] = min(niveau[i], niveau[i - 1])
        self._niveau_eau = niveau

    def poids_lit_en(self, distance):
        """Part du creusement appliquee a cette distance de l'axe."""
        return 1.0 - lissage_doux(
            distance, self.s["riviere_demi_largeur"] * 1.15,
            self.s["riviere_demi_largeur"] * self.s["riviere_evasement"])

    def niveau_eau_en(self, x, y):
        meilleure, idx = float("inf"), 0
        for i, (px, py) in enumerate(self.riviere):
            d = (x - px) ** 2 + (y - py) ** 2
            if d < meilleure:
                meilleure, idx = d, i
        return self._niveau_eau[idx]

    def _preparer_profil(self):
        # `creuser=False` : le profil se cale sur la BERGE, pas sur le fond.
        brut = [self._base(x, y, creuser=False) for (x, y) in self.chemin]
        fen = 11
        lisse = []
        for i in range(len(brut)):
            a, b = max(0, i - fen // 2), min(len(brut), i + fen // 2 + 1)
            lisse.append(sum(brut[a:b]) / (b - a))
        dz = 2.0 * self.s["chemin_pente_max"]
        for i in range(1, len(lisse)):
            lisse[i] = max(lisse[i - 1] - dz, min(lisse[i - 1] + dz, lisse[i]))
        for i in range(len(lisse) - 2, -1, -1):
            lisse[i] = max(lisse[i + 1] - dz, min(lisse[i + 1] + dz, lisse[i]))

        # LE PONT EST DE NIVEAU. Sur sa portee, le chemin est force a UNE seule
        # altitude. Sans ca, le tablier — pose a une hauteur unique — colle a un
        # about et flotte a l'autre : c'est le pont penche, infranchissable.
        px, py = self.pont_xy
        portee = self.s["pont_demi_portee"]
        plancher = self.niveau_eau_en(px, py) + self.s["pont_tirant_air"]
        sur_pont = [i for i, (x, y) in enumerate(self.chemin)
                    if math.hypot(x - px, y - py) <= portee + 1.0]
        for i in sur_pont:
            lisse[i] = plancher
        # On RELEVE ensuite les abords pour tenir la pente, sans jamais
        # redescendre le tablier : un second clamp l'aurait ecrase.
        if sur_pont:
            for i in range(min(sur_pont) - 1, -1, -1):
                lisse[i] = max(lisse[i], lisse[i + 1] - dz)
            for i in range(max(sur_pont) + 1, len(lisse)):
                lisse[i] = max(lisse[i], lisse[i - 1] - dz)
        self._profil = lisse

    def profil_en(self, x, y):
        meilleure, idx = float("inf"), 0
        for i, (px, py) in enumerate(self.chemin):
            d = (x - px) ** 2 + (y - py) ** 2
            if d < meilleure:
                meilleure, idx = d, i
        return self._profil[idx], math.sqrt(meilleure)

    def hauteur(self, x, y):
        h = self._base(x, y)
        hc, d = self.profil_en(x, y)
        poids = 1.0 - lissage_doux(d, self.s["chemin_demi_largeur"],
                                   self.s["chemin_demi_largeur"] + self.s["chemin_raccord"])
        # Le lit de la rivière n'est JAMAIS comblé par le nivellement du chemin :
        # c'est ce qui laisse une brèche à enjamber, donc un pont qui sert.
        dr = distance_polyligne(x, y, self.riviere)
        poids *= lissage_doux(dr, self.s["riviere_demi_largeur"] * 0.5,
                              self.s["riviere_demi_largeur"] * 1.5)
        return h * (1.0 - poids) + hc * poids

    def pente(self, x, y, pas=1.5):
        """Pente locale en degrés — sert à ne rien planter sur une falaise."""
        h = self.hauteur(x, y)
        dx = self.hauteur(x + pas, y) - h
        dy = self.hauteur(x, y + pas) - h
        return math.degrees(math.atan(math.hypot(dx, dy) / pas))


# ---------------------------------------------------------------------------
# Matériaux
# ---------------------------------------------------------------------------


def emprunter_palette():
    palette, cache = {}, collection("_src")
    for cible, fichier in PALETTE_SOURCES.items():
        chemin = os.path.join(KIT_NATURE, fichier)
        if not os.path.exists(chemin):
            continue
        avant = set(bpy.data.objects)
        bpy.ops.import_scene.gltf(filepath=chemin)
        for obj in [o for o in bpy.data.objects if o not in avant]:
            if obj.type == "MESH":
                for slot in obj.material_slots:
                    if slot.material and slot.material.name.split(".")[0] == cible:
                        palette.setdefault(cible, slot.material)
            for c in list(obj.users_collection):
                c.objects.unlink(obj)
            cache.objects.link(obj)
            obj.hide_viewport = obj.hide_render = True
    return palette


def materiau_sol(nom, cfg):
    """Matériau texturé PBR (diffuse + normale), teinté vers le stylisé.

    La teinte n'est pas cosmétique : un sol photoréaliste sous des props
    facettés jure. On multiplie la photo par une couleur du monde pour les
    ramener dans la même famille. L'exporteur glTF sait écrire ce motif
    (image × facteur) en baseColorTexture + baseColorFactor.
    """
    mat = bpy.data.materials.new(nom)
    mat.use_nodes = True
    nodes, links = mat.node_tree.nodes, mat.node_tree.links
    bsdf = next(n for n in nodes if n.type == "BSDF_PRINCIPLED")

    # Deux bibliotheques : celle du chateau (fichiers nommes, deja en 1K) et
    # l'ancienne `textures-v1` (un dossier par jeu, diff/normal.jpg).
    if cfg["jeu"] == "castle":
        dossier = os.path.join(RACINE, "assets", "textures", "castle_1k")
        diff = os.path.join(dossier, cfg["diff"])
        norm = os.path.join(dossier, cfg["norm"])
    else:
        dossier = os.path.join(TEXTURES, cfg["jeu"])
        diff = os.path.join(dossier, "diff.jpg")
        norm = os.path.join(dossier, "normal.jpg")

    # PAS de noeud Mapping. Les UV du terrain encodent DEJA `metres / uv_m`
    # (cf. `batir_terrain` : `co.x / uv_herbe`) : un Mapping qui divise encore
    # par `uv_m` etirait la texture d'un facteur uv_m. Et un `str.replace` sans
    # limite de compte avait supprime le lien de la diffuse mais pas celui de
    # la normale — les deux ne tournaient donc meme plus a la meme echelle.
    # Bonus : sans Mapping, l'echelle est portee par les UV, donc elle
    # S'EXPORTE en glTF. Un noeud procedural, non.

    if os.path.exists(diff):
        tex = nodes.new("ShaderNodeTexImage")
        tex.image = bpy.data.images.load(diff, check_existing=True)
        mix = nodes.new("ShaderNodeMix")
        mix.data_type = "RGBA"
        mix.blend_type = "MULTIPLY"
        mix.inputs["Factor"].default_value = 1.0
        links.new(tex.outputs["Color"], mix.inputs[6])
        mix.inputs[7].default_value = hex_lineaire(cfg["teinte"])
        links.new(mix.outputs[2], bsdf.inputs["Base Color"])
    else:
        bsdf.inputs["Base Color"].default_value = hex_lineaire(cfg["teinte"])

    if os.path.exists(norm):
        tn = nodes.new("ShaderNodeTexImage")
        tn.image = bpy.data.images.load(norm, check_existing=True)
        tn.image.colorspace_settings.name = "Non-Color"
        nmap = nodes.new("ShaderNodeNormalMap")
        nmap.inputs["Strength"].default_value = 0.8
        links.new(tn.outputs["Color"], nmap.inputs["Color"])
        links.new(nmap.outputs["Normal"], bsdf.inputs["Normal"])

    bsdf.inputs["Roughness"].default_value = 0.95
    bsdf.inputs["Metallic"].default_value = 0.0
    return mat


def materiau_eau():
    """Eau exportable : ce qui traverse le glTF, et rien d'autre.

    Un shader procedural Blender (vagues, refraction) ne franchit PAS l'export
    glTF : seuls passent couleur de base, metallique/rugosite, normale, alpha.
    On fait donc une eau qui tient dans ce contrat — sinon on livre une belle
    riviere qui ne sort jamais de Blender.

    Vue de haut, ce qui fait qu'on VOIT de l'eau, ce n'est pas sa teinte : c'est
    (1) le contraste avec la berge, (2) le reflet — une nappe mate de la couleur
    du ciel s'efface, une nappe sombre et lisse renvoie le soleil, (3) les rides,
    qui donnent l'echelle. Les trois sont exportables.
    """
    mat = bpy.data.materials.new("eau_riviere")
    mat.use_nodes = True
    nodes, links = mat.node_tree.nodes, mat.node_tree.links
    bsdf = next(n for n in nodes if n.type == "BSDF_PRINCIPLED")
    cfg = SPEC["eau"]
    bsdf.inputs["Base Color"].default_value = hex_lineaire(cfg["teinte"])
    bsdf.inputs["Roughness"].default_value = cfg["rugosite"]
    bsdf.inputs["Metallic"].default_value = cfg["metallique"]
    if "Alpha" in bsdf.inputs:
        bsdf.inputs["Alpha"].default_value = cfg["alpha"]
    mat.blend_method = "BLEND"

    normale = os.path.join(TEXTURES, cfg["rides"], "normal.jpg")
    if os.path.exists(normale):
        coords = nodes.new("ShaderNodeTexCoord")
        mapping = nodes.new("ShaderNodeMapping")
        e = 1.0 / cfg["rides_uv_m"]
        mapping.inputs["Scale"].default_value = (e, e, e)
        links.new(coords.outputs["Object"], mapping.inputs["Vector"])
        tex = nodes.new("ShaderNodeTexImage")
        tex.image = bpy.data.images.load(normale, check_existing=True)
        tex.image.colorspace_settings.name = "Non-Color"
        nmap = nodes.new("ShaderNodeNormalMap")
        nmap.inputs["Strength"].default_value = cfg["rides_force"]
        links.new(tex.outputs["Color"], nmap.inputs["Color"])
        links.new(nmap.outputs["Normal"], bsdf.inputs["Normal"])
    return mat


def unifier_materiaux():
    """Fusionne grass/grass.001/… et applique la palette verdoyante.

    Sans cette passe, 1 500 imports produisent 1 500 copies de « grass » : la
    fusion finale ne fusionne plus rien et les draw calls explosent. C'est LE
    geste qui rend le détour par Blender payant.
    """
    canon = {}
    for mat in sorted(bpy.data.materials, key=lambda m: m.name):
        canon.setdefault(mat.name.split(".")[0], mat)
    for mesh in bpy.data.meshes:
        for i, mat in enumerate(mesh.materials):
            if mat is not None:
                mesh.materials[i] = canon[mat.name.split(".")[0]]
    teintes = SPEC["palette"]
    applique = 0
    for base, mat in canon.items():
        if not mat.use_nodes:
            continue
        bsdf = next((n for n in mat.node_tree.nodes if n.type == "BSDF_PRINCIPLED"), None)
        if bsdf is None or bsdf.inputs["Base Color"].is_linked:
            continue
        if base in teintes:
            couleur = hex_lineaire(teintes[base])
            bsdf.inputs["Base Color"].default_value = couleur
            # `diffuse_color` est la couleur d'affichage SOLIDE du viewport, et
            # elle est indépendante du shader. Sans cette ligne, Blender montre
            # encore le turquoise d'origine du kit pendant que les rendus
            # sortent verts : on travaille alors sur une scène qui ment.
            mat.diffuse_color = couleur
            applique += 1
        bsdf.inputs["Roughness"].default_value = 0.92
        bsdf.inputs["Metallic"].default_value = 0.0
        mat.roughness = 0.92
    return canon, applique


# ---------------------------------------------------------------------------
# Maillages générés
# ---------------------------------------------------------------------------


def creer_mesh(nom, verts, faces, materiau, uvs, coll, couleurs=None):
    mesh = bpy.data.meshes.new(nom)
    mesh.from_pydata(verts, [], faces)
    mesh.update()
    if materiau is not None:
        mesh.materials.append(materiau)
    couche = mesh.uv_layers.new(name="UVMap")
    for poly in mesh.polygons:
        for li in poly.loop_indices:
            couche.data[li].uv = uvs(mesh.vertices[mesh.loops[li].vertex_index].co)
    if couleurs is not None:
        att = mesh.color_attributes.new(name="Col", type="FLOAT_COLOR", domain="POINT")
        for i, vert in enumerate(mesh.vertices):
            att.data[i].color = couleurs(vert.co)
    obj = bpy.data.objects.new(nom, mesh)
    coll.objects.link(obj)
    return obj


def batir_terrain(relief, mat_herbe, mat_roche, mat_berge, mat_roche_haute, coll):
    """Nappe de sol à DEUX matériaux, choisis par la pente.

    C'est le geste qui remplace 1 721 cubes de falaise : au-delà de
    `pente_roche_deg`, la face n'est plus de l'herbe mais de la roche. La
    ceinture, les flancs de la crête et les berges deviennent rocheux tout
    seuls, sans une seule pièce posée à la main — et la transition suit la
    forme réelle du terrain au lieu de la contredire.

    Les UV suivent la même logique : une face verticale texturée en projection
    horizontale étire la roche en traînées ; on la projette donc sur sa propre
    verticale.
    """
    pas = SPEC["pas_terrain"]
    nx = int(SPEC["demi_x"] * 2 / pas)
    ny = int(SPEC["demi_y"] * 2 / pas)
    verts, faces = [], []
    for j in range(ny + 1):
        for i in range(nx + 1):
            x = -SPEC["demi_x"] + i * pas
            y = -SPEC["demi_y"] + j * pas
            verts.append((x, y, relief.hauteur(x, y)))
    for j in range(ny):
        for i in range(nx):
            a = j * (nx + 1) + i
            faces.append((a, a + 1, a + nx + 2, a + nx + 1))

    mesh = bpy.data.meshes.new("terrain")
    mesh.from_pydata(verts, [], faces)
    mesh.update()
    mesh.materials.append(mat_herbe)
    mesh.materials.append(mat_roche)
    mesh.materials.append(mat_berge)
    mesh.materials.append(mat_roche_haute)

    uv_herbe = SPEC["sols"]["terrain"]["uv_m"]
    uv_roche = SPEC["sols"]["falaise"]["uv_m"]
    uv_berge = SPEC["sols"]["berge"]["uv_m"]
    uv_haute = SPEC["sols"]["falaise_haute"]["uv_m"]
    # Altitude de bascule entre les deux roches, brouillée par du bruit : une
    # limite horizontale nette se lit comme une ligne de niveau peinte.
    bascule = SPEC["roche_bascule_z"]
    flou = SPEC["roche_bascule_flou"]
    # On compare des ANGLES, pas des cosinus : brouiller un seuil en cosinus
    # donnerait un fondu d'amplitude variable selon la pente.
    zb_f = relief.zb + 71.0

    def brouillage(c):
        """Ecart aleatoire mais CONTINU applique aux seuils, en [-1, 1]."""
        lobe = mathutils.noise.noise(Vector((c.x * SPEC["fondu_lobe"],
                                             c.y * SPEC["fondu_lobe"], zb_f)))
        grain = mathutils.noise.noise(Vector((c.x * SPEC["fondu_grain"],
                                              c.y * SPEC["fondu_grain"], zb_f + 5.0)))
        return max(-1.0, min(1.0, lobe + 0.45 * grain))
    couche = mesh.uv_layers.new(name="UVMap")
    roche = 0
    berge = 0
    haut = 0
    for poly in mesh.polygons:
        n = poly.normal
        c = poly.center
        b = brouillage(c)
        pente_face = math.degrees(math.acos(max(-1.0, min(1.0, n.z))))
        est_roche = pente_face > SPEC["pente_roche_deg"] + b * SPEC["fondu_pente_deg"]
        if not est_roche:
            # Le sol change de MATIERE en approchant de l'eau : greve sableuse.
            # C'est ce qui signale une riviere de loin — bien plus surement que
            # la couleur de sa nappe, qu'on ne voit qu'au bord.
            if distance_polyligne(c.x, c.y, relief.riviere) <                     SPEC["berge_largeur"] + b * SPEC["fondu_berge_m"]:
                poly.material_index = 2
                berge += 1
                for li in poly.loop_indices:
                    co = mesh.vertices[mesh.loops[li].vertex_index].co
                    couche.data[li].uv = (co.x / uv_berge, co.y / uv_berge)
                continue
        if est_roche:
            # Étagement : roche chaude au pied, roche claire en altitude. La
            # limite ondule (bruit) — une bascule à altitude fixe se lit comme
            # une ligne de niveau peinte au cordeau.
            limite = bascule + mathutils.noise.noise(
                Vector((c.x * 0.02, c.y * 0.02, relief.zb + 61.0))) * flou
            haute = c.z > limite
            poly.material_index = 3 if haute else 1
            uvm = uv_haute if haute else uv_roche
            if haute:
                haut += 1
            else:
                roche += 1
            # Projection sur l'axe DOMINANT de la face (triplanaire pauvre).
            # La version precedente projetait toujours sur la perpendiculaire
            # horizontale de la normale : mesure, la densite partait de 11 a
            # 24,7 m par tuile — soit un facteur 2,2 d'etirement visible.
            if abs(n.x) >= abs(n.y):
                for li in poly.loop_indices:
                    co = mesh.vertices[mesh.loops[li].vertex_index].co
                    couche.data[li].uv = (co.y / uvm, co.z / uvm)
            else:
                for li in poly.loop_indices:
                    co = mesh.vertices[mesh.loops[li].vertex_index].co
                    couche.data[li].uv = (co.x / uvm, co.z / uvm)
        else:
            for li in poly.loop_indices:
                co = mesh.vertices[mesh.loops[li].vertex_index].co
                couche.data[li].uv = (co.x / uv_herbe, co.y / uv_herbe)

    zb = relief.zb
    att = mesh.color_attributes.new(name="Col", type="FLOAT_COLOR", domain="POINT")
    # Pente locale lue sur la GRILLE (voisins immediats) plutot que par trois
    # appels au champ de hauteur : meme resultat, 60 000 evaluations en moins.
    hauteurs = [v.co.z for v in mesh.vertices]

    def pente_grille(idx):
        i_, j_ = idx % (nx + 1), idx // (nx + 1)
        gx = (hauteurs[idx + 1] if i_ < nx else hauteurs[idx]) -              (hauteurs[idx - 1] if i_ > 0 else hauteurs[idx])
        gy = (hauteurs[idx + nx + 1] if j_ < ny else hauteurs[idx]) -              (hauteurs[idx - nx - 1] if j_ > 0 else hauteurs[idx])
        return math.degrees(math.atan(math.hypot(gx, gy) / (2.0 * pas)))

    for i, vert in enumerate(mesh.vertices):
        co = vert.co
        # Variation lente : casse la répétition de la texture et zone le vallon
        # (creux humides et sombres, hauteurs plus sèches).
        v = mathutils.noise.noise(Vector((co.x * 0.008, co.y * 0.008, zb + 41.0)))
        sec = lissage_doux(co.z, 2.0, 14.0)
        # BANDE DE TRANSITION, recette du chateau : de l'herbe s'accroche aux
        # faces dont la normale pointe vers le haut. Ici on l'obtient en
        # verdissant la couleur de sommet la ou la pente est faible — la
        # frontiere herbe/roche cesse d'etre une decoupe nette a 36 deg et
        # devient un degrade de mousse sur les vires.
        pente_v = pente_grille(i)
        # MOUSSE : de l'herbe s'accroche aux vires peu pentues (recette du
        # chateau, « top projection »).
        mousse = (1.0 - lissage_doux(pente_v,
                                     SPEC["pente_roche_deg"] - 14.0,
                                     SPEC["pente_roche_deg"] + 4.0)) * SPEC["mousse_force"]
        # FONDU VERS LA ROCHE : l'herbe se DESATURE en approchant des pentes,
        # avant meme que le materiau ne bascule. L'oeil suit un degrade au lieu
        # de buter sur une frontiere — c'est ce qui fait qu'on ne la voit plus.
        vers_roche = lissage_doux(pente_v, SPEC["pente_roche_deg"] - 20.0,
                                  SPEC["pente_roche_deg"] + 2.0) * SPEC["fondu_couleur"]
        # FONDU VERS LA GREVE : meme principe au bord de l'eau, ou la coupure
        # herbe/sable est la plus regardee de la carte.
        d_eau = distance_polyligne(co.x, co.y, relief.riviere)
        vers_sable = (1.0 - lissage_doux(d_eau, SPEC["berge_largeur"],
                                         SPEC["berge_largeur"] + SPEC["fondu_berge_m"] * 2.0))             * SPEC["fondu_couleur"]
        # En altitude on ECLAIRCIT : c'est la ou la roche prend le jour.
        # L'ancienne courbe assombrissait le bleu en hauteur, ce qui verdissait
        # la paroi — l'exact defaut signale.
        r = 0.88 + 0.14 * v + 0.20 * sec - 0.26 * mousse
        g = 0.92 + 0.12 * v + 0.18 * sec + 0.06 * mousse
        bl = 0.86 + 0.12 * v + 0.20 * sec - 0.30 * mousse
        # Vers la roche : on tire vers un gris neutre (desaturation).
        r = r * (1.0 - vers_roche) + 0.86 * vers_roche
        g = g * (1.0 - vers_roche) + 0.86 * vers_roche
        bl = bl * (1.0 - vers_roche) + 0.84 * vers_roche
        # Vers la greve : on tire vers un beige chaud.
        r = r * (1.0 - vers_sable) + 1.00 * vers_sable
        g = g * (1.0 - vers_sable) + 0.94 * vers_sable
        bl = bl * (1.0 - vers_sable) + 0.78 * vers_sable
        att.data[i].color = (r, g, bl, 1.0)

    obj = bpy.data.objects.new("terrain", mesh)
    coll.objects.link(obj)
    mesh.shade_smooth()
    return obj, roche, haut, berge, len(mesh.polygons)


def batir_ruban(nom, relief, points, demi_l, dz, mat, coll, suivre=False, uv_m=6.0,
                trou=None, niveau=None, uv_flux=False, demi_variable=None,
                colonnes=1, couleurs=None):
    """Bâtit une bande le long d'une polyligne.

    # `colonnes` — ce qu'il débloque, et pourquoi il manquait

    Le ruban ne savait produire que DEUX sommets par station : une bande d'un
    seul quad de large. Mesuré sur la rivière — 97 stations × 2 colonnes, aucun
    sommet intérieur. Or tout ce qui fait qu'une eau se lit comme de l'eau se
    porte par des sommets INTÉRIEURS : une écume qui ne mord que les rives, une
    teinte qui s'assombrit vers le chenal, un courant plus vif au milieu.
    Sans eux, on ne peut peindre que « toute la nappe » ou « rien ».

    `colonnes=1` reproduit exactement l'ancien comportement — les deux rives et
    rien entre elles.

    # `couleurs` — la fonction qui reçoit la position TRANSVERSALE

    Elle est appelée avec `(co, t, s)` où `t ∈ [0, 1]` traverse la largeur (0 =
    rive gauche, 1 = rive droite) et `s ∈ [0, 1]` suit le cours. C'est `t` qui
    permet une écume de rive : `creer_mesh` ne donne que la position monde, d'où
    il est impossible de savoir de quel côté de la bande on se trouve.
    """
    verts, faces = [], []
    longueur = 0.0
    abscisses = []
    par_colonne = colonnes + 1
    infos = []            # (t, s) par sommet, pour les UV et les couleurs
    for i, (x, y) in enumerate(points):
        if i == 0:
            dx, dy = points[1][0] - x, points[1][1] - y
        elif i == len(points) - 1:
            dx, dy = x - points[-2][0], y - points[-2][1]
        else:
            dx = points[i + 1][0] - points[i - 1][0]
            dy = points[i + 1][1] - points[i - 1][1]
        n = math.hypot(dx, dy) or 1.0
        if i > 0:
            longueur += math.dist(points[i], points[i - 1])
        abscisses.append(longueur)
        # Le RIVAGE est la ou le plan d'eau coupe le sol, pas a une largeur
        # fixe. Un ruban de largeur constante laisse du lit a sec d'un cote
        # et deborde de l'autre : la ligne d'eau devient une droite
        # geometrique, ce qu'aucune riviere ne fait.
        # On releve donc la demi-largeur des DEUX cotes, puis on interpole entre
        # elles pour les colonnes interieures — sinon un chenal asymetrique
        # verrait ses sommets interieurs se tasser du cote le plus etroit.
        larges = {}
        for signe in (-1.0, 1.0):
            larges[signe] = (demi_l if demi_variable is None
                             else demi_variable(x, y, -dy / n * signe, dx / n * signe))
        for k in range(par_colonne):
            t = k / colonnes                      # 0 = rive gauche, 1 = droite
            lat = t * 2.0 - 1.0                   # -1 .. +1
            large = larges[-1.0] if lat < 0.0 else larges[1.0]
            ox, oy = -dy / n * large, dx / n * large
            px, py = x + ox * lat, y + oy * lat
            if niveau is not None:
                # Meme altitude pour TOUTE la largeur : une nappe ne se vrille pas.
                z = niveau(x, y) + dz
            else:
                z = (relief.profil_en(x, y)[0] if suivre else relief.hauteur(px, py)) + dz
            verts.append((px, py, z))
            infos.append((t, 0.0))
    total = longueur or 1.0
    for i in range(len(points)):
        for k in range(par_colonne):
            t, _ = infos[i * par_colonne + k]
            infos[i * par_colonne + k] = (t, abscisses[i] / total)

    for i in range(len(points) - 1):
        if trou is not None:
            (tx, ty), rayon = trou
            # Une brèche enjambée par un pont ne doit pas AUSSI être couverte
            # par le ruban : celui-ci suit le profil de berge, donc il traverse
            # le vide en bande suspendue et double le tablier.
            if (math.hypot(points[i][0] - tx, points[i][1] - ty) < rayon
                    and math.hypot(points[i + 1][0] - tx, points[i + 1][1] - ty) < rayon):
                continue
        for k in range(colonnes):
            a = i * par_colonne + k
            b = (i + 1) * par_colonne + k
            faces.append((a, a + 1, b + 1, b))

    if uv_flux:
        # UV alignees sur le COURANT : U traverse la largeur (0..1), V suit
        # l'abscisse curviligne. La direction d'ecoulement est ainsi encodee
        # dans la geometrie elle-meme — le moteur n'a qu'a faire defiler V dans
        # le temps pour donner le courant, sans flow map a peindre ni
        # simulation. Une UV planaire (x/m, y/m) rendrait ce defilement faux
        # des que la riviere tourne : elle coulerait en travers.
        uv_par_sommet = {}
        for i in range(len(points)):
            v = abscisses[i] / uv_m
            for k in range(par_colonne):
                uv_par_sommet[i * par_colonne + k] = (k / colonnes, v)
        mesh = bpy.data.meshes.new(nom)
        mesh.from_pydata(verts, [], faces)
        mesh.update()
        if mat is not None:
            mesh.materials.append(mat)
        couche = mesh.uv_layers.new(name="UVMap")
        for poly in mesh.polygons:
            for li in poly.loop_indices:
                couche.data[li].uv = uv_par_sommet.get(mesh.loops[li].vertex_index, (0.0, 0.0))
        if couleurs is not None:
            att = mesh.color_attributes.new(name="Col", type="FLOAT_COLOR", domain="POINT")
            for idx, vert in enumerate(mesh.vertices):
                t, s = infos[idx]
                att.data[idx].color = couleurs(vert.co, t, s)
        obj = bpy.data.objects.new(nom, mesh)
        coll.objects.link(obj)
        return obj

    obj = creer_mesh(nom, verts, faces, mat,
                     lambda co: (co.x / uv_m, co.y / uv_m), coll)
    if couleurs is not None:
        mesh = obj.data
        att = mesh.color_attributes.get("Col") or mesh.color_attributes.new(
            name="Col", type="FLOAT_COLOR", domain="POINT")
        for idx, vert in enumerate(mesh.vertices):
            t, s = infos[idx]
            att.data[idx].color = couleurs(vert.co, t, s)
    return obj


def materiau_pierre_chateau():
    """Pierre du chateau, en 1K. Les modules sont exportes SANS materiau (avec,
    le GLB re-embarquait 37,5 Mo de textures 2K/4K pour 15 000 triangles) : on
    la reapplique ici depuis `assets/textures/castle_1k/`."""
    mat = bpy.data.materials.new("pierre_chateau")
    mat.use_nodes = True
    nodes, links = mat.node_tree.nodes, mat.node_tree.links
    bsdf = next(n for n in nodes if n.type == "BSDF_PRINCIPLED")
    base = os.path.join(RACINE, "assets", "textures", "castle_1k")
    # ON UTILISE LES UV DU MODULE. Le chateau a deplie ces pieces pour ses
    # propres textures de brique, et l'export les a conservees (la geometrie
    # part meme quand les materiaux ne partent pas). Une projection
    # `Generated` — normalisee sur la boite englobante — etirait au contraire
    # la tuile sur tout l'ouvrage : la pierre ne se voyait plus.
    # Bonus : des UV reelles s'exportent en glTF, une projection procedurale non.
    for fichier, entree, non_couleur in (("stone_bc.png", "Base Color", False),
                                         ("stone_n.png", "Normal", True)):
        chemin = os.path.join(base, fichier)
        if not os.path.exists(chemin):
            continue
        tex = nodes.new("ShaderNodeTexImage")
        tex.image = bpy.data.images.load(chemin, check_existing=True)
        if non_couleur:
            tex.image.colorspace_settings.name = "Non-Color"
        if entree == "Normal":
            nmap = nodes.new("ShaderNodeNormalMap")
            nmap.inputs["Strength"].default_value = 0.9
            links.new(tex.outputs["Color"], nmap.inputs["Color"])
            links.new(nmap.outputs["Normal"], bsdf.inputs["Normal"])
        else:
            links.new(tex.outputs["Color"], bsdf.inputs[entree])
    bsdf.inputs["Roughness"].default_value = 0.88
    bsdf.inputs["Metallic"].default_value = 0.0
    return mat


def batir_pont_pierre(relief, coll, kit):
    """Pose le module de pont du chateau, oriente sur le chemin.

    Un seul module pour l'instant : sa grammaire d'assemblage n'est pas
    validee (le tablier de base monte a 10,00 m, l'extension a 8,01 — ils ne
    s'aboutent pas en un tablier plat), et poser six extensions sur une
    hypothese non verifiee produirait un ouvrage faux qu'il faudrait defaire.
    On regarde d'abord ce que donne la piece maitresse.
    """
    cfg = SPEC["pont_pierre"]
    px, py = relief.pont_xy
    a = relief.chemin[max(0, relief.pont_idx - 1)]
    b = relief.chemin[min(len(relief.chemin) - 1, relief.pont_idx + 1)]
    dx, dy = b[0] - a[0], b[1] - a[1]
    n = math.hypot(dx, dy) or 1.0
    ux, uy = dx / n, dy / n
    # Le grand axe du module est son +Y local : on l'aligne sur le chemin.
    rot = math.atan2(uy, ux) - math.pi / 2.0
    ech = cfg["echelle"]
    # Le tablier doit arriver a hauteur de chemin : on descend l'origine
    # (qui est au PIED de l'ouvrage) de la hauteur du tablier.
    z = relief.profil_en(px, py)[0] - cfg["deck_local_z"] * ech
    poses = kit.poser(cfg["module"] + ".glb", px, py, z, coll, rot_z=rot, echelle=ech)
    pierre = materiau_pierre_chateau()
    for obj in poses:
        obj.data.materials.clear()
        obj.data.materials.append(pierre)
    return poses


def batir_pont(relief, palette, coll):
    """Tablier + parapets. Fabriqué, pas assemblé : les pièces `bridge_*` sont
    des tuiles de grille et rien ne garantit qu'une file tombe pile sur une
    brèche dont la largeur est dictée par le creusement."""
    px, py = relief.pont_xy
    portee = SPEC["pont_demi_portee"]
    proche = relief.pont_idx
    # Tangente locale sur 2 stations (4 m) : sur 6, la corde s'ecarte du trace
    # la ou il tourne, et le tablier part de travers.
    a = relief.chemin[max(0, proche - 1)]
    b = relief.chemin[min(len(relief.chemin) - 1, proche + 1)]
    dx, dy = b[0] - a[0], b[1] - a[1]
    n = math.hypot(dx, dy) or 1.0
    ux, uy, vx, vy = dx / n, dy / n, -dy / n, dx / n
    # Le tablier se cale sur le PLUS HAUT des deux : le profil du chemin, ou
    # la nappe d'eau majoree d'un tirant d'air. Au seul profil, le nivellement
    # etant coupe au droit de la riviere, le pont plongeait dans le courant.
    # Le tablier prend EXACTEMENT l'altitude du chemin, mise de niveau sur la
    # portee. Une formule independante ici, c'etait « une grandeur ecrite deux
    # fois » — et les deux divergeaient.
    # Recalage transversal constate en scene (cf. SPEC).
    rec = SPEC["pont_recalage_lateral_m"]
    px, py = px + vx * rec, py + vy * rec
    z = relief.profil_en(px, py)[0] + 0.10   # reference des piles
    demi = SPEC["chemin_demi_largeur"] + 0.5

    verts, faces = [], []

    def pt(t, lat, haut):
        """Point du pont. Son altitude est LUE SOUS LUI dans le profil du
        chemin, planche par planche.

        Poser tout le tablier a une altitude unique, c'etait lire le profil en
        UN point pendant que le chemin le lit station par station : les deux
        divergeaient, et le pont dominait la route de 1,07 m d'un cote sans
        l'atteindre de l'autre. En le faisant lire au meme endroit que le
        chemin, la jonction est exacte par construction — le tablier epouse
        meme les rampes d'acces au lieu de flotter au-dessus.
        """
        wx = px + ux * t + vx * lat
        wy = py + uy * t + vy * lat
        return (wx, wy, relief.profil_en(wx, wy)[0] + 0.10 + haut)

    def boite(t0, t1, l0, l1, h0, h1):
        """Ajoute un pavé dans le repère du pont. Six faces, pas un plan."""
        d = len(verts)
        for h in (h0, h1):
            for t in (t0, t1):
                for lat in (l0, l1):
                    verts.append(pt(t, lat, h))
        faces.extend([(d, d + 1, d + 3, d + 2), (d + 4, d + 6, d + 7, d + 5),
                      (d, d + 2, d + 6, d + 4), (d + 1, d + 5, d + 7, d + 3),
                      (d, d + 4, d + 5, d + 1), (d + 2, d + 3, d + 7, d + 6)])

    # PLANCHES individuelles en travers, avec leur jour. Un tablier d'un seul
    # tenant se lit comme une feuille de carton posée sur le vide : ce sont les
    # joints qui disent « ouvrage de bois » et qui donnent l'échelle du pas.
    largeur_planche = SPEC["pont_planche_m"]
    jour = SPEC["pont_jour_m"]
    pas_planche = largeur_planche + jour
    nb = max(4, int((2.0 * portee) / pas_planche))
    depart = -portee + (2.0 * portee - nb * pas_planche + jour) * 0.5
    for k in range(nb):
        t0 = depart + k * pas_planche
        boite(t0, t0 + largeur_planche, -demi, demi, -0.14, 0.0)

    # Deux longrines sous les planches : elles portent, et surtout elles
    # ferment le dessous, qui serait sinon une grille de planches flottantes.
    for cote in (-1.0, 1.0):
        lat = demi * 0.72 * cote
        boite(-portee, portee, lat - 0.22, lat + 0.22, -0.55, -0.14)

    # Piles dans le lit : sans appui visible, un pont ne tient sur rien.
    for t in (-portee * 0.45, portee * 0.45):
        for cote in (-1.0, 1.0):
            lat = demi * 0.72 * cote
            # Profondeur relative au tablier local : la pile plonge toujours
            # sous le lit, quelle que soit la rampe au-dessus d'elle.
            boite(t - 0.35, t + 0.35, lat - 0.3, lat + 0.3, -7.0, -0.5)

    # Garde-corps : poteaux + lisse. Une simple plaque verticale ne se lit pas
    # comme un garde-corps, et masque la rivière qu'on traverse.
    pas_poteau = (2.0 * portee) / max(2, int(2.0 * portee / 2.6))
    for cote in (-1.0, 1.0):
        lat = demi * cote
        k = 0
        while True:
            t = -portee + k * pas_poteau
            if t > portee + 1e-6:
                break
            boite(t - 0.11, t + 0.11, lat - 0.11, lat + 0.11, 0.0, 1.05)
            k += 1
        boite(-portee, portee, lat - 0.09, lat + 0.09, 0.9, 1.05)
    return creer_mesh("pont", verts, faces, palette.get("wood"),
                      lambda co: (co.x * 0.2, co.y * 0.2), coll)


# ---------------------------------------------------------------------------
# Kits
# ---------------------------------------------------------------------------


class Kit:
    """Charge un kit glTF et pose ses pièces sur le terrain.

    # 🚨 `offset_base` a disparu, et voici la mesure qui l'a tué

    Il valait 0,05 pour le kit nature, et on le croyait inoffensif : le
    catalogue du kit dit que 299 pièces sur 329 ont `z_min = −0,05`, donc
    `z_sol + 0,05·s` semblait poser le point le plus bas pile au sol.

    **C'est faux, et c'est mesurable.** Le catalogue décrit le GLB SUR LE
    DISQUE. L'importateur glTF de Blender, lui, remonte ce décalage sur la
    transformation du NŒUD — et `Kit.poser` crée un objet neuf à partir du seul
    datablock, donc il la jette. Relevé sur les prototypes réellement en scène :
    `z_min = 0,0` sur 46 des 49 maillages, et **aucun** à −0,05.

    `offset_base` ne compensait donc plus rien : c'était une lévitation pure de
    `0,05 × échelle`. Mesuré sur 1 645 pièces posées, par tranche d'échelle :

        ×3 → +0,062 m · ×4 → +0,144 m · ×5 → +0,173 m

    soit `0,05·s` à la mesure près, et un résidu MÉDIAN de −0,007 m une fois la
    compensation retirée.

    L'assise se MESURE donc désormais par pièce, sur le datablock qui sera
    réellement instancié. Un scalaire par kit ne pouvait pas être juste : le
    décalage origine→base est une propriété de chaque GLB, pas d'un dossier.

    # Ce que ce changement rend enfin honnête

    Les enfoncements volontaires (`−0,3`, `−0,4`, `enfoncement_m`) étaient
    amputés de `0,05·s` : un éboulis « enfoncé de 0,40 m » l'était en réalité de
    0,20. Ils valent maintenant ce qu'ils annoncent.
    """

    def __init__(self, racine, echelle, filtre_nom=False):
        self.racine, self.echelle = racine, echelle
        # `filtre_nom` : ne garder que les meshes dont le nom derive du fichier.
        # L'importateur glTF de Blender FABRIQUE une « Icosphere » comme forme
        # d'affichage des os d'un squelette — elle n'est pas dans le GLB (verifie
        # sur le JSON brut : meshes=['deer.001'] seul). Sans ce filtre, une boule
        # de 2 m se pose a cote de chaque bete.
        self.filtre_nom = filtre_nom
        self._proto, self._cache = {}, collection("_proto")
        self.manquants = []
        self.poses = 0
        # Les emprises solides, telles qu'elles seront publiees au manifeste.
        # Elles se relevent AU MOMENT DE LA POSE et non apres instanciation :
        # `91_export.py` fusionne et detruit les objets, il ne resterait plus
        # rien a mesurer ensuite (meme raison que `spawn-clearance.md` §3, ou
        # les emprises se publient au plan).
        self.solides = {}

    def assise(self, fichier):
        """Altitude du point le plus BAS de la pièce, en local et à l'échelle 1.

        C'est la seule grandeur qui permet de poser une pièce SUR le sol sans
        rien supposer : `z_objet = z_sol − assise × échelle` place le point le
        plus bas exactement à `z_sol`, quelle que soit la pièce et quelle que
        soit l'échelle.

        Rend 0,0 pour une pièce introuvable : mieux vaut une pièce posée à plat
        qu'une exception au milieu d'un semis de 3 200 touffes.
        """
        bas = None
        for data in self.prototype(fichier):
            zs = [v.co.z for v in data.vertices]
            if zs:
                m = min(zs)
                bas = m if bas is None else min(bas, m)
        return bas if bas is not None else 0.0

    def echelle_pour(self, fichier, defaut):
        """L'échelle qui donne à cette pièce sa taille RÉELLE, si on la connaît.

        Rend `defaut` pour tout ce qui n'a pas de taille déclarée : on ne
        prétend pas connaître la hauteur vraie de chaque caillou du kit, on
        corrige seulement là où un symptôme l'a nommé.
        """
        cible = TAILLES_REELLES.get(os.path.basename(fichier))
        if cible is None:
            return defaut
        h = self.hauteur_locale(fichier)
        return (cible / h) if h > 1e-6 else defaut

    def rayon_local(self, fichier):
        """Rayon au sol de la pièce, en local et à l'échelle 1.

        Pendant de `hauteur_locale` : sans lui on ne peut contraindre QUE la
        hauteur, et l'échelle étant uniforme, une pièce basse qu'on agrandit
        pour atteindre une hauteur devient large d'autant. C'est exactement le
        défaut que ça a produit — des abris de 2,5 m de haut et 4 m de large,
        qui ont transformé l'anneau de couverture en muraille.
        """
        rayon = 0.0
        for data in self.prototype(fichier):
            for v in data.vertices:
                rayon = max(rayon, math.hypot(v.co.x, v.co.y))
        return rayon

    def hauteur_locale(self, fichier):
        """Hauteur d'une piece AVANT mise a l'echelle. Sert a calculer l'echelle
        qu'il faut pour qu'elle atteigne une hauteur voulue — au lieu de tirer
        une echelle au hasard puis de constater que l'abri n'abrite pas."""
        haut = 0.0
        for data in self.prototype(fichier):
            zs = [v.co.z for v in data.vertices]
            if zs:
                haut = max(haut, max(zs) - min(zs))
        return haut

    @staticmethod
    def _emprise(data, s, bande_m, matieres=None):
        """Emprise au sol et hauteur d'une piece PLACEE, mesurees sur ses sommets.

        # Pourquoi la tranche basse, et pas l'AABB entiere

        Le houppier d'un pin deborde de 2 a 3 m alors qu'on ne heurte que son
        tronc. Prendre l'emprise complete rendrait la foret infranchissable —
        on aurait « corrige » l'absence de collision en supprimant la marche.
        On mesure donc le rayon sur la seule tranche que le joueur traverse,
        de la base jusqu'a `bande_m`.

        # Pourquoi c'est une mesure et pas un coefficient

        Le rayon precedent valait `0,055 x echelle` : un nombre de reglage
        attrape au passage. `spawn-clearance.md` §4 nomme exactement cette
        faute — « une valeur de tuning n'est pas une mesure » — et c'est elle
        qui avait declare un batiment de 12 m avec 1,92 m de rayon, donc des
        mobs naissant dedans.

        Le rayon est pris depuis l'ORIGINE de la piece (hypot x,y), pas depuis
        le centre de son AABB : le collider sera pose a `obj.location`, et une
        mesure prise ailleurs decrirait un cylindre qui n'est pas celui-la.
        C'est aussi ce qui la rend invariante par rotation autour de Z, donc
        juste quel que soit le `rot_z` tire au hasard.
        """
        sommets = data.vertices
        if not sommets:
            return 0.0, 0.0
        z0 = min(v.co.z for v in sommets)
        z1 = max(v.co.z for v in sommets)
        haut = (z1 - z0) * s
        # `bande_m` est un metre MONDE : le ramener en local avant de comparer.
        plafond = z0 + min(bande_m / s, z1 - z0)

        # QUELS SOMMETS COMPTENT. Sans filtre, la tranche basse d'un arbre
        # attrape le bas du houppier autant que le tronc : mesure du premier
        # jet, rayon MEDIAN de 0,72 m et 27 % des arbres au-dela de 1 m, la ou
        # le tronc fait 0,2 m. La futaie serait devenue un labyrinthe — on
        # aurait « corrige » l'absence de collision en supprimant la marche.
        #
        # Ce qu'on heurte dans un arbre, c'est son TRONC ; on pousse a travers
        # les branches. Et le kit distingue deja les deux PAR LEUR MATIERE
        # (`wood*` contre `leafs*`). Le filtre est donc une lecture du modele,
        # pas un facteur de correction invente pour retomber sur ses pieds.
        indices = None
        if matieres:
            noms = [m.name.split(".")[0] if m else "" for m in data.materials]
            gardes = {i for i, n in enumerate(noms)
                      if any(n.startswith(p) for p in matieres)}
            if gardes:
                indices = set()
                for poly in data.polygons:
                    if poly.material_index in gardes:
                        indices.update(poly.vertices)
            # Aucune matiere reconnue (un buisson tout en feuillage) : on
            # retombe sur la piece entiere. Rendre 0 en ferait un fantome.

        rayon = 0.0
        for i, v in enumerate(sommets):
            if v.co.z > plafond:
                continue
            if indices is not None and i not in indices:
                continue
            rayon = max(rayon, math.hypot(v.co.x, v.co.y))
        return rayon * s, haut

    def prototype(self, fichier):
        if fichier in self._proto:
            return self._proto[fichier]
        chemin = os.path.join(self.racine, fichier)
        if not os.path.exists(chemin):
            self.manquants.append(fichier)
            self._proto[fichier] = []
            return []
        avant = set(bpy.data.objects)
        bpy.ops.import_scene.gltf(filepath=chemin)
        neufs = [o for o in bpy.data.objects if o not in avant]
        datas = [o.data for o in neufs if o.type == "MESH"]
        if self.filtre_nom:
            racine_nom = os.path.splitext(os.path.basename(fichier))[0]
            datas = [d for d in datas if d.name.split(".")[0] == racine_nom]
        for obj in neufs:
            for c in list(obj.users_collection):
                c.objects.unlink(obj)
            self._cache.objects.link(obj)
            obj.hide_viewport = obj.hide_render = True
        self._proto[fichier] = datas
        return datas

    def poser(self, fichier, x, y, z_sol, coll, rot_z=0.0, echelle=None, inclinaison=0.0,
              famille=None, bande_m=1.8, matieres=None, hauteur_min_m=0.6):
        """Pose une piece. `famille` non nul = elle est SOLIDE, on publie son emprise.

        La famille n'est pas une etiquette decorative : c'est elle qui dit au
        moteur ce que la piece fait au jeu. Un `abri` doit casser une ligne de
        vue, un `arbre` doit juste se contourner — et le prochain audit doit
        pouvoir verifier que chacun tient son contrat sans rouvrir Blender
        (`map-design-intention.md` §5.1 : le nom est un contrat).
        """
        datas = self.prototype(fichier)
        if not datas:
            return 0
        s = echelle if echelle is not None else self.echelle
        # La convention d'« avant » de CE modèle (cf. `ORIENTATION_CORRIGEE`).
        rot_z += ORIENTATION_CORRIGEE.get(os.path.basename(fichier), 0.0)
        # L'assise MESURÉE de cette pièce-ci. `z_sol` reste ce que l'appelant a
        # demandé — y compris les enfoncements volontaires qu'il y a déjà
        # soustraits — et l'assise ne fait que rattraper le décalage propre au
        # GLB. Les deux ne se mélangent pas : l'un est une intention, l'autre
        # une propriété du fichier.
        assise = self.assise(fichier) * s
        crees = []
        for data in datas:
            # On garde le NOM DU MESH source, pas celui du fichier : c'est lui
            # qui distingue `wall_straight_gate_door_left` du dormant. Sans ca,
            # les battants deviennent indiscernables et rien ne peut les animer.
            obj = bpy.data.objects.new(
                data.name or os.path.splitext(os.path.basename(fichier))[0], data)
            obj.location = (x, y, z_sol - assise)
            obj.rotation_euler = (inclinaison, 0.0, rot_z)
            obj.scale = (s, s, s)
            coll.objects.link(obj)
            # L'ETIQUETTE RESTE SUR L'OBJET. Sans elle, la famille n'existe que
            # dans la memoire de ce script : une passe suivante (calcul
            # d'enveloppes, controle visuel) devrait la redeviner par le nom du
            # fichier, ce qui redonnerait deux sources pour une meme verite.
            if famille is not None:
                obj["famille"] = famille
                if matieres:
                    obj["matieres_emprise"] = ",".join(matieres)
            crees.append(obj)
        self.poses += 1
        if famille is not None and crees:
            # Une piece peut compter plusieurs maillages (un tronc + son
            # feuillage). On garde le rayon le plus large et la hauteur la plus
            # haute : sous-estimer l'emprise coute un joueur qui apparait
            # dedans, la surestimer coute un trou dans le decor. Les deux
            # erreurs n'ont pas le meme prix (`spawn-clearance.md` §4).
            rayon = hauteur = 0.0
            for data in datas:
                r, h = self._emprise(data, s, bande_m, matieres)
                rayon, hauteur = max(rayon, r), max(hauteur, h)
            # Deux refus, et le second n'est pas une precaution :
            #
            # - un rayon degenere donne un cylindre que le moteur accepte et qui
            #   n'arrete rien — pire qu'absent, parce qu'il se compte comme
            #   present (`map-design-patterns.md` §13) ;
            # - une piece plus basse que `hauteur_min_m` est du decor de SOL. La
            #   mesure l'a montre : un `campfire_logs` fait 0,25 m. Un cylindre
            #   dessus est un muret invisible a hauteur de cheville, sur lequel
            #   on trebuche sans jamais voir pourquoi. Le joueur saute 1,174 m :
            #   ce qui est bas se franchit, ca ne se contourne pas.
            if rayon > 0.01 and hauteur >= hauteur_min_m:
                self.solides.setdefault(famille, []).append(
                    [round(x, 2), round(y, 2), round(z_sol, 2),
                     round(hauteur, 2), round(rayon, 2)])
        return crees


# ---------------------------------------------------------------------------
# Passe principale
# ---------------------------------------------------------------------------


def main():
    # AVANT TOUT : ne pas effacer un travail manuel non capturé (cf. la
    # fonction, et le jour où c'est arrivé).
    non_capturees = refuser_si_travail_non_capture()
    if non_capturees:
        print("RESULT: " + json.dumps({
            "refus": "des pieces ont ete deplacees a la main et ne sont pas capturees",
            "combien": len(non_capturees),
            "exemples": non_capturees[:10],
            "remede": "python tools/blender/bmcp.py code "
                      "tools/blender/expedition/20_retouches.py",
            "forcer": "bpy.context.scene[\"vallon_jeter_retouches\"] = True "
                      "(jette les deplacements, ne vaut que pour UNE cuisson)",
        }, ensure_ascii=False))
        return

    wipe()
    os.makedirs(SORTIE, exist_ok=True)
    rng = random.Random(SPEC["graine"])
    palette = emprunter_palette()
    relief = Relief(SPEC)

    mat_terrain = materiau_sol("sol_terrain", SPEC["sols"]["terrain"])
    mat_chemin = materiau_sol("sol_chemin", SPEC["sols"]["chemin"])
    mat_roche = materiau_sol("sol_falaise", SPEC["sols"]["falaise"])
    mat_berge = materiau_sol("sol_berge", SPEC["sols"]["berge"])
    mat_roche_haute = materiau_sol("sol_falaise_haute", SPEC["sols"]["falaise_haute"])

    # Centres des campements : pris SUR le chemin, donc infranchissables sans
    # les traverser. C'est ce qui en fait des verrous et pas des detours.
    # Un campement ne doit pas coloniser un lieu qui a deja un role : le pont,
    # le col (la revelation), les places. Le premier jet en avait pose un SUR
    # le pont et un SUR le col. On glisse le long du chemin jusqu'a etre au
    # clair, au lieu de retoucher les fractions a la main.
    interdits = [
        (relief.pont_xy[0], relief.pont_xy[1], 24.0),
        # Le col garde un halo court : le 3e campement doit pouvoir en
        # tenir l'approche — c'est le dernier verrou avant la revelation.
        (SPEC["crete"]["x"], SPEC["crete"]["col_y"], 14.0),
        (SPEC["clairiere_spawn"]["xy"][0], SPEC["clairiere_spawn"]["xy"][1], 26.0),
        (SPEC["place_village"]["xy"][0], SPEC["place_village"]["xy"][1], 40.0),
    ]

    def degage(i):
        px, py = relief.chemin[i]
        return all(math.hypot(px - ix, py - iy) > r for ix, iy, r in interdits)

    camps = []
    pris = []
    for frac in SPEC["campements"]["fractions"]:
        depart = max(1, min(len(relief.chemin) - 2, int(frac * len(relief.chemin))))
        idx = None
        for saut in range(0, len(relief.chemin)):
            for cand in (depart + saut, depart - saut):
                if not 1 <= cand <= len(relief.chemin) - 2:
                    continue
                if not degage(cand):
                    continue
                # Deux camps colles ne font qu'un combat trop long.
                if any(abs(cand - autre) < 15 for autre in pris):
                    continue
                idx = cand
                break
            if idx is not None:
                break
        if idx is None:
            continue
        pris.append(idx)
        cx, cy = relief.chemin[idx]
        ax, ay = relief.chemin[idx - 1]
        bx, by = relief.chemin[idx + 1]
        dx, dy = bx - ax, by - ay
        n = math.hypot(dx, dy) or 1.0
        camps.append({"xy": (cx, cy), "avance": (dx / n, dy / n), "idx": idx})

    def ecarter_du_chemin(x, y, marge):
        """Pousse un point perpendiculairement au chemin jusqu'a le degager.

        La statue du col tombait EN PLEIN sur la route. Recopier la position
        qu'Antoine lui a donnee a la main ne reglerait que ce cas-la : c'est la
        GARDE qui manquait, celle qu'ont deja les props de campement.
        """
        for _ in range(14):
            if distance_polyligne(x, y, relief.chemin) >= marge:
                return x, y
            # direction d'eloignement : le gradient de la distance au chemin
            eps = 0.5
            d0 = distance_polyligne(x, y, relief.chemin)
            gx = distance_polyligne(x + eps, y, relief.chemin) - d0
            gy = distance_polyligne(x, y + eps, relief.chemin) - d0
            n = math.hypot(gx, gy) or 1.0
            x += gx / n * 1.5
            y += gy / n * 1.5
        return x, y

    c_sol = collection("sol")
    c_falaise = collection("falaises")
    c_foret = collection("foret")
    c_village = collection("village")
    c_reperes = collection("reperes")

    _, faces_roche, faces_haut, faces_berge, faces_total = batir_terrain(
        relief, mat_terrain, mat_roche, mat_berge, mat_roche_haute, c_sol)
    # LA ROUTE RESPIRE. Elle etait d'une demi-largeur RIGOUREUSEMENT constante
    # sur 358 m : deux droites paralleles, ce qu'aucun chemin de terre battue
    # n'est. La variation se derive de deux bornes reelles, pas d'un gout :
    #   - borne basse : la largeur qui laisse passer deux joueurs de front
    #     (2 x 0,6 m de diametre + du jeu) — c'est deja la valeur declaree ;
    #   - borne haute : l'anneau ou l'herbe est autorisee a pousser
    #     (`chemin_demi_largeur + 0.3`, cf. le semis d'herbe). Au-dela, le pave
    #     passerait SOUS des touffes, et la route aurait de l'herbe dessus.
    # Le bruit est lent (une periode d'environ 40 m) : une largeur qui varierait
    # station par station donnerait un bord dentele, pas un chemin.
    demi_bas = SPEC["chemin_demi_largeur"]
    demi_haut = SPEC["chemin_demi_largeur"] + 0.3

    def largeur_chemin(x, y, _nx, _ny):
        v = mathutils.noise.noise(Vector((x * 0.025, y * 0.025, relief.zb + 7.0)))
        return demi_bas + (demi_haut - demi_bas) * (v * 0.5 + 0.5)

    # LE FONDU DE LISIERE. Le ruban n'avait AUCUN attribut de couleur : sa
    # rencontre avec l'herbe etait une arete franche, la seule de la carte que
    # rien ne brouillait. COLOR_0 atteint desormais le jeu (mesure : 632
    # primitives sur 633), donc la bordure peut s'assombrir et se salir vers
    # l'exterieur, ce qui fond le pave dans la terre au lieu de l'y poser.
    #
    # `t` traverse la largeur : 0 et 1 sont les bords, 0,5 est l'axe. On ne
    # touche donc QUE les bords, et l'axe reste a 1,0 — sinon toute la route
    # s'assombrit et on croit a un probleme d'eclairage.
    def couleur_chemin(_co, t, _s):
        bord = abs(t - 0.5) * 2.0                 # 0 au centre, 1 aux bords
        # Puissance 2,5 : le salissement se concentre sur le dernier tiers.
        # Lineaire, il grisait le milieu de la chaussee.
        f = 1.0 - SPEC["fondu_couleur"] * 0.55 * (bord ** 2.5)
        return (f, f, f, 1.0)

    batir_ruban("chemin", relief, relief.chemin, SPEC["chemin_demi_largeur"], 0.08,
                mat_chemin, c_sol, suivre=True, uv_m=SPEC["sols"]["chemin"]["uv_m"],
                trou=(relief.pont_xy, SPEC["pont_demi_portee"] - 1.5),
                demi_variable=largeur_chemin,
                # 4 colonnes : il en faut au moins 3 pour qu'un bord puisse
                # s'assombrir sans entrainer l'axe. 4 donne un centre franc et
                # deux bordures, pour 3 quads par station au lieu d'un.
                colonnes=4, couleurs=couleur_chemin)
    # L'eau se pose 1,1 m au-dessus du LIT, pas sous lui : `hauteur()` contient
    # déjà le creusement de la rivière. Soustraire une seconde fois la
    # profondeur enfouissait la nappe sous son propre lit — rivière invisible.
    # L'eau est un objet SEPARE : le moteur doit pouvoir animer son UV sans
    # toucher au sol, et la fusion de la collection `sol` la lui retirerait.
    c_eau = collection("eau")

    def rivage(x, y, nx, ny):
        """Ou le plan d'eau rencontre le sol, de ce cote-ci.

        On marche vers la rive par petits pas jusqu'a sortir de l'eau. C'est la
        definition physique d'un rivage, et c'est ce qui lui donne sa ligne
        irreguliere : une demi-largeur constante produit une droite.
        """
        niv = relief.niveau_eau_en(x, y)
        mini = SPEC["riviere_demi_largeur"] * 0.30
        maxi = SPEC["riviere_demi_largeur"] * 1.70
        # 🚨 AUX BOUCHES DE GORGE, LE COURS SE RESSERRE.
        #
        # Mesuré : le fond de gorge est PLAT (sol constant à 2,25 m en amont,
        # −5,72 en aval) et la nappe flotte 1,4 à 1,7 m au-dessus. Le rivage,
        # qui marche jusqu'à ce que le sol remonte, ne rencontrait donc rien —
        # l'eau s'étalait jusqu'à sa borne, et la rivière se lisait comme une
        # mare fermée par un mur de rochers.
        #
        # Une rivière qui traverse une gorge fait l'inverse : elle se resserre.
        # La borne haute se pince donc à l'approche du bord, et la borne basse
        # avec elle — sinon on obtiendrait un chenal étroit dans une nappe large.
        # `bouchon` (0,94) est déjà la fraction d'emprise où l'entaille se
        # referme : on reprend la MÊME grandeur, pour que les deux ne divergent
        # jamais.
        t_bord = max(abs(x) / SPEC["demi_x"], abs(y) / SPEC["demi_y"])
        pince = 1.0 - 0.62 * lissage_doux(t_bord, SPEC["gorge"]["bouchon"] - 0.16,
                                          SPEC["gorge"]["bouchon"] + 0.04)
        mini *= pince
        maxi *= pince
        d = mini
        while d < maxi:
            if relief.hauteur(x + nx * d, y + ny * d) >= niv:
                break
            d += 0.4
        # On mord de quelques centimetres dans la berge : sans ce recouvrement,
        # un lisere de sol nu apparait a chaque approximation de maillage.
        return d + 0.35

    # L'ECUME DE RIVE, et la profondeur — ni l'une ni l'autre n'etaient
    # POSSIBLES jusqu'ici. Mesure du 2026-08-17 : la nappe faisait 97 stations
    # x 2 colonnes, un seul quad de large, donc AUCUN sommet interieur. On ne
    # pouvait peindre que « toute la nappe » ou « rien ».
    #
    # `t` traverse la largeur : 0 et 1 sont les rives, 0,5 le chenal.
    #   - vers les rives, on ECLAIRCIT : l'eau peu profonde laisse voir le fond
    #     clair et l'agitation blanchit — c'est ce qui dessine un rivage.
    #   - vers le chenal, on ASSOMBRIT : plus d'eau au-dessus du fond.
    # Bevy multiplie COLOR_0 par la couleur de base, donc tout part de 1,0 et
    # seule la part sombre descend ; l'ecume, elle, ne peut pas depasser 1,0 —
    # c'est le materiau qui porte sa clarte, pas ce canal.
    def couleur_eau(_co, t, _s):
        bord = abs(t - 0.5) * 2.0                 # 0 au chenal, 1 aux rives
        # Le chenal descend a 0,62 : l'eau profonde est nettement plus sombre
        # que sa lisiere, et c'est ce contraste qui donne le volume du lit.
        profond = 0.62 + 0.38 * (bord ** 1.6)
        return (profond, profond, profond, 1.0)

    batir_ruban("riviere", relief, relief.riviere, SPEC["riviere_demi_largeur"],
                0.0, materiau_eau(), c_eau, uv_m=SPEC["eau"]["tuile_m"],
                niveau=relief.niveau_eau_en, uv_flux=True, demi_variable=rivage,
                # 6 colonnes sur 16 m de large = un sommet tous les ~2,7 m.
                # Derive de ce qu'on veut LIRE : une bande d'ecume credible fait
                # 1 a 3 m au bord d'un cours de cette taille, il faut donc au
                # moins deux sommets pour la porter de chaque cote, plus deux
                # pour le chenal. 6 est le premier compte pair qui le permet.
                colonnes=6, couleurs=couleur_eau)
    if SPEC["pont_pierre"]["actif"]:
        # Pas de `filtre_nom` ici : il n'existe que pour écarter l'icosphère que
        # l'importateur glTF fabrique face à un GLB À SQUELETTE. Le pont n'en a
        # pas, et le filtre rejetait alors le module lui-même (0 pièce posée).
        kit_chateau = Kit(KIT_CHATEAU, SPEC["pont_pierre"]["echelle"])
        pont_pieces = batir_pont_pierre(relief, c_sol, kit_chateau)
    else:
        # `batir_pont` rend UN objet (pas une liste) : on compte 1, sinon
        # l'objet Blender partait tel quel dans le rapport JSON.
        batir_pont(relief, palette, c_sol)
        pont_pieces = 1

    nature = Kit(KIT_NATURE, SPEC["echelle_nature"])
    village = Kit(KIT_VILLAGE, SPEC["echelle_village"])

    # -- bouchage des creux de la ceinture ---------------------------------
    ROCHES_CHATEAU = ["SM_ENV_cliff_castle_01_LOD0.glb", "SM_ENV_cliff_castle_02_LOD0.glb"]
    bch = SPEC["ceinture_bouchage"]
    kit_roches = Kit(KIT_CHATEAU, 1.0)
    # Le MEME materiau que la paroi qu'ils prolongent : ces rochers bouchent la
    # ceinture, ils doivent en etre la continuation, pas une piece rapportee.
    # `materiau_pierre_chateau` (pierre taillee du pont) les rendait blancs —
    # leurs UV sont depliees pour la texture de FALAISE, pas pour l'appareil.
    mat_roche_chateau = mat_roche
    roches_bouchon = 0
    poses_roches = []
    creux_trouves = 0
    for k in range(bch["sondes"]):
        ang = math.tau * k / bch["sondes"]
        ex, ey = math.cos(ang), math.sin(ang)
        m = max(abs(ex) / SPEC["demi_x"], abs(ey) / SPEC["demi_y"])
        bx, by = ex / m * bch["assise"], ey / m * bch["assise"]
        ix, iy = ex / m * bch["reference"], ey / m * bch["reference"]
        if relief.hauteur(bx, by) - relief.hauteur(ix, iy) >= bch["seuil_m"]:
            continue
        creux_trouves += 1
        if any(math.hypot(bx - rx, by - ry) < bch["ecart_min_m"] for rx, ry in poses_roches):
            continue
        poses_roches.append((bx, by))
        # Le rocher s'enfonce d'un metre : pose en surface, il flotterait sur
        # la moindre irregularite du maillage.
        objets = kit_roches.poser(
            ROCHES_CHATEAU[k % len(ROCHES_CHATEAU)], bx, by,
            relief.hauteur(bx, by) - bch["enfoncement_m"], c_falaise,
            rot_z=rng.uniform(0.0, math.tau),
            echelle=rng.uniform(*bch["echelle"]), famille="bouchon")
        for o in objets:
            o.data.materials.clear()
            o.data.materials.append(mat_roche_chateau)
        roches_bouchon += len(objets)

    # -- éboulis au pied des parois ---------------------------------------
    # La falaise elle-même est maintenant du TERRAIN (faces au-delà de 36° en
    # matériau roche). Il reste à casser la ligne où la paroi rencontre le sol :
    # une rencontre nette se voit, un éboulis la fait oublier.
    eboulis = 0
    essais = 0
    while eboulis < 260 and essais < 4000:
        essais += 1
        x = rng.uniform(-SPEC["demi_x"] + 3, SPEC["demi_x"] - 3)
        y = rng.uniform(-SPEC["demi_y"] + 3, SPEC["demi_y"] - 3)
        pente = relief.pente(x, y)
        # La bande utile : le pied de paroi, là où ça commence à se redresser.
        if not (12.0 < pente < 40.0):
            continue
        if distance_polyligne(x, y, relief.chemin) < SPEC["degagement_chemin"]:
            continue
        nature.poser(choisir(rng, ROCHERS), x, y, relief.hauteur(x, y) - 0.4, c_falaise,
                     rot_z=rng.uniform(0.0, math.tau),
                     echelle=SPEC["echelle_nature"] * rng.uniform(0.5, 1.4),
                     inclinaison=rng.uniform(-0.12, 0.12), famille="eboulis")
        eboulis += 1
    noter_defaut("falaises.eboulis", 260, eboulis,
                 "bande de pied de paroi 12-40 deg, hors degagement du chemin")

    # -- zones interdites --------------------------------------------------
    place, spawn = SPEC["place_village"], SPEC["clairiere_spawn"]

    # LES DEUX VUES PROTÉGÉES, résolues en coordonnées. Elles se déduisent de la
    # géométrie (la porte, le col, la place) et non d'un couple écrit à la main :
    # le jour où le tracé bouge, les cônes suivent.
    _col = (SPEC["crete"]["x"], SPEC["crete"]["col_y"])
    _approche = min(relief.chemin,
                    key=lambda p: abs(math.hypot(p[0] - relief.porte_xy[0],
                                                 p[1] - relief.porte_xy[1]) - 40.0))
    VUES = []
    for v in SPEC["vues_protegees"]:
        depuis, vers = (_approche, relief.porte_xy) if v["nom"] == "porte" \
            else (_col, tuple(place["xy"]))
        VUES.append((depuis, vers, v["demi_pres"], v["demi_loin"], v["portee_m"]))

    def libre(x, y, marge, hauteur=99.0):
        if abs(x) > SPEC["demi_x"] - 15.0 or abs(y) > SPEC["demi_y"] - 15.0:
            return False
        # 🚨 NE PAS BOUCHER UNE RÉVÉLATION. Seules les pièces qui dépassent
        # l'œil comptent : sous 1,70 m on voit par-dessus, donc rien n'est caché.
        if hauteur > OEIL_JOUEUR_M and dans_une_vue(x, y, VUES):
            return False
        if distance_polyligne(x, y, relief.chemin) < marge:
            return False
        if distance_polyligne(x, y, relief.riviere) < SPEC["degagement_riviere"]:
            return False
        if math.hypot(x - place["xy"][0], y - place["xy"][1]) < place["rayon"] + 4.0:
            return False
        if math.hypot(x - spawn["xy"][0], y - spawn["xy"][1]) < spawn["rayon"]:
            return False
        # Un arbre plante dans l'axe de la porte la cache et bloque le passage.
        if math.hypot(x - relief.porte_xy[0], y - relief.porte_xy[1]) < 13.0:
            return False
        # Un campement se lit depuis son seuil : une futaie dedans rendrait le
        # combat illisible (map-design-intention.md §3.3).
        for camp in camps:
            if math.hypot(x - camp["xy"][0], y - camp["xy"][1]) < SPEC["campements"]["rayon"]:
                return False
        return True

    def essence(x, y, h):
        """L'essence se déduit du MILIEU, pas d'une coordonnée.

        Elle valait `y > 34 ou h > 7` pour les conifères et `x > 46` pour
        l'automne : **deux frontières rectilignes** tracées dans le peuplement,
        que rien ne brouillait. Le sol de cette carte se donne pourtant beaucoup
        de mal pour éviter exactement ça (`fondu_lobe`, `fondu_grain`) — une
        bascule nette produit une ligne de ciseaux qu'aucune nature n'a.

        Trois critères, tous relevés sur place, tous brouillés :

        1. **L'eau d'abord.** Une ripisylve borde un cours d'eau : des feuillus,
           jamais de conifère. C'était le manque le plus criant — la rivière
           traversait des pinèdes.
        2. **L'altitude et la pente.** Le conifère tient là où le feuillu ne
           tient plus. La pente compte autant que la hauteur : un versant raide
           à mi-altitude est déjà un milieu de conifère.
        3. **L'automne aux abords du village**, en frange et non en bloc.
        """
        # Brouillage à deux octaves, même recette que les frontières de matière
        # du terrain : une lobe large qui fait divaguer la limite à l'échelle du
        # paysage, un grain fin qui la dentelle.
        b = (mathutils.noise.noise(Vector((x * SPEC["fondu_lobe"],
                                           y * SPEC["fondu_lobe"], relief.zb + 41.0)))
             + 0.5 * mathutils.noise.noise(Vector((x * SPEC["fondu_grain"],
                                                   y * SPEC["fondu_grain"],
                                                   relief.zb + 53.0))))

        # 1. la ripisylve. 22 m : au-delà de la grève (10 m) et de son
        #    dégagement (12 m), donc la première bande vraiment plantable.
        if distance_polyligne(x, y, relief.riviere) < 22.0 + b * 9.0:
            return FEUILLUS

        # 2. le conifère prend l'altitude ET la pente. Le seuil de hauteur
        #    reste 7 m (mi-relief : le terrain va de −6 à +15) ; celui de pente
        #    est 22°, sous la limite de plantation de 34° — le feuillu lâche
        #    avant que rien ne pousse.
        if h > 7.0 + b * 4.0 or relief.pente(x, y) > 22.0 + b * 7.0:
            return CONIFERES

        # 3. l'automne en approche du village, en frange dégradée : la
        #    probabilité monte avec la proximité au lieu de basculer sur une
        #    ligne. `place` est le centre du village, `rayon_aplani` son emprise.
        d_vil = math.hypot(x - place["xy"][0], y - place["xy"][1])
        part = 1.0 - lissage_doux(d_vil, place["rayon_aplani"] * 0.8,
                                  place["rayon_aplani"] * 2.6)
        if part > 0.05 and rng.random() < part * 0.75:
            return AUTOMNE
        return FEUILLUS

    # -- futaies -----------------------------------------------------------
    centres = []
    while len(centres) < SPEC["futaies"]:
        x = rng.uniform(-SPEC["demi_x"] + 18, SPEC["demi_x"] - 18)
        y = rng.uniform(-SPEC["demi_y"] + 18, SPEC["demi_y"] - 18)
        if libre(x, y, SPEC["degagement_chemin"] + 6.0):
            centres.append((x, y, rng.uniform(*SPEC["futaie_rayon"])))

    arbres = 0
    cible_futaie = int(SPEC["arbres_total"] * SPEC["part_en_futaie"])
    essais = 0
    while arbres < cible_futaie and essais < cible_futaie * 8:
        essais += 1
        cx, cy, r = centres[rng.randrange(len(centres))]
        # Tirage en racine : dense au cœur, effiloché sur les bords — un massif
        # à bord net se lit comme une haie.
        rad = r * math.sqrt(rng.random())
        ang = rng.uniform(0.0, math.tau)
        x, y = cx + math.cos(ang) * rad, cy + math.sin(ang) * rad
        if not libre(x, y, SPEC["degagement_chemin"] + 1.5):
            continue
        if relief.pente(x, y) > SPEC["pente_plantation_deg"]:
            continue
        h = relief.hauteur(x, y)
        ech = SPEC["echelle_nature"] * rng.uniform(0.78, 1.35)
        # Un tronc ne se collisionne pas en TriMesh : 1 500 arbres feraient
        # 300 000 triangles de collision pour un cylindre chacun. Le moteur
        # instancie ces proxys — meme doctrine que la dalle deterministe du Hall.
        # `famille` remplace l'ancien `troncs.append` : la publication se fait
        # DANS `poser`, donc aucune boucle ne peut plus l'oublier. C'est ce qui
        # est arrive a la seconde boucle ci-dessous — 207 arbres isoles sans la
        # moindre collision, et l'oubli ne levait rien.
        nature.poser(choisir(rng, essence(x, y, h)), x, y, h, c_foret,
                     rot_z=rng.uniform(0.0, math.tau), echelle=ech,
                     inclinaison=rng.uniform(-0.035, 0.035), famille="arbre",
                     matieres=MATIERES_TRONC)
        arbres += 1

    essais = 0
    while arbres < SPEC["arbres_total"] and essais < SPEC["arbres_total"] * 8:
        essais += 1
        x = rng.uniform(-SPEC["demi_x"], SPEC["demi_x"])
        y = rng.uniform(-SPEC["demi_y"], SPEC["demi_y"])
        if not libre(x, y, SPEC["degagement_chemin"] + 1.5) or relief.pente(x, y) > SPEC["pente_plantation_deg"]:
            continue
        h = relief.hauteur(x, y)
        nature.poser(choisir(rng, essence(x, y, h)), x, y, h, c_foret,
                     rot_z=rng.uniform(0.0, math.tau),
                     echelle=SPEC["echelle_nature"] * rng.uniform(0.8, 1.3),
                     inclinaison=rng.uniform(-0.035, 0.035), famille="arbre",
                     matieres=MATIERES_TRONC)
        arbres += 1

    noter_defaut("foret.arbres", SPEC["arbres_total"], arbres,
                 "semis contraint : degagement du chemin 3,8 m, pente max 34 deg")

    # -- sous-bois : il a le droit de border le chemin, il ne bloque pas ----
    sousbois, essais = 0, 0
    rejets_sousbois = {}
    while sousbois < SPEC["sousbois_total"] and essais < SPEC["sousbois_total"] * 6:
        essais += 1
        x = rng.uniform(-SPEC["demi_x"], SPEC["demi_x"])
        y = rng.uniform(-SPEC["demi_y"], SPEC["demi_y"])
        # `hauteur=1.5` : le sous-bois reste sous l'oeil du joueur, il ne peut
        # donc boucher aucune vue — l'exempter des cones evite de creer des
        # clairieres vides la ou on voulait seulement voir loin.
        if not libre(x, y, SPEC["chemin_demi_largeur"] + 0.7, hauteur=1.5):
            continue
        # Le sous-bois ne testait PAS la pente : des buissons poussaient sur des
        # parois. Meme source que les arbres — un buisson ne tient pas mieux
        # qu'un arbre sur un talus.
        if relief.pente(x, y) > SPEC["pente_plantation_deg"]:
            rejets_sousbois["pente"] = rejets_sousbois.get("pente", 0) + 1
            continue
        nature.poser(choisir(rng, SOUSBOIS), x, y, relief.hauteur(x, y), c_foret,
                     rot_z=rng.uniform(0.0, math.tau),
                     echelle=SPEC["echelle_nature"] * rng.uniform(0.65, 1.25))
        sousbois += 1

    # Tapis d'herbe : petites pieces uniquement, autorisees jusqu'au bord du
    # chemin. Elles ne bloquent rien et suppriment l'effet « sol nu entre les
    # arbres » qui trahit la carte generee.
    HERBES = [("grass.glb", 5), ("grass_large.glb", 4), ("grass_leafs.glb", 4),
              ("grass_leafsLarge.glb", 3), ("plant_flatShort.glb", 2)]
    herbe, essais = 0, 0
    while herbe < SPEC["herbe_total"] and essais < SPEC["herbe_total"] * 4:
        essais += 1
        x = rng.uniform(-SPEC["demi_x"], SPEC["demi_x"])
        y = rng.uniform(-SPEC["demi_y"], SPEC["demi_y"])
        if abs(x) > SPEC["demi_x"] - 15.0 or abs(y) > SPEC["demi_y"] - 15.0:
            continue
        if relief.pente(x, y) > 40.0:
            continue
        # Pas d'herbe dans l'eau, mais elle a le droit de border la greve.
        if distance_polyligne(x, y, relief.riviere) < SPEC["riviere_demi_largeur"] + 1.0:
            continue
        if distance_polyligne(x, y, relief.chemin) < SPEC["chemin_demi_largeur"] + 0.3:
            continue
        nature.poser(choisir(rng, HERBES), x, y, relief.hauteur(x, y), c_foret,
                     rot_z=rng.uniform(0.0, math.tau),
                     echelle=SPEC["echelle_nature"] * rng.uniform(0.45, 0.95))
        herbe += 1

    # Pierres DANS le lit : une nappe d'eau nue se lit comme une bache bleue.
    # Ce sont les cailloux qui donnent le fond et l'echelle.
    pierres, essais = 0, 0
    while pierres < SPEC["pierres_lit"] and essais < SPEC["pierres_lit"] * 8:
        essais += 1
        idx = rng.randrange(len(relief.riviere))
        bx0, by0 = relief.riviere[idx]
        lat = rng.uniform(-SPEC["riviere_demi_largeur"] * 1.15,
                          SPEC["riviere_demi_largeur"] * 1.15)
        ang = rng.uniform(0.0, math.tau)
        px = bx0 + math.cos(ang) * abs(lat) * (1.0 if lat > 0 else -1.0)
        py = by0 + math.sin(ang) * abs(lat) * 0.35
        if distance_polyligne(px, py, relief.chemin) < 5.0:
            continue
        nature.poser(choisir(rng, [("stone_smallFlatA.glb", 3), ("stone_smallFlatB.glb", 3),
                                   ("stone_smallA.glb", 2), ("rock_smallFlatC.glb", 2),
                                   ("stone_largeB.glb", 1)]),
                     px, py, relief.niveau_eau_en(px, py) - 0.55, c_foret,
                     rot_z=rng.uniform(0.0, math.tau),
                     echelle=SPEC["echelle_nature"] * rng.uniform(0.4, 1.1))
        pierres += 1

    rochers, essais = 0, 0
    rejets_rochers = {}
    while rochers < SPEC["rochers_total"] and essais < SPEC["rochers_total"] * 8:
        essais += 1
        x = rng.uniform(-SPEC["demi_x"], SPEC["demi_x"])
        y = rng.uniform(-SPEC["demi_y"], SPEC["demi_y"])
        if not libre(x, y, SPEC["degagement_chemin"] + 2.0):
            continue
        # Idem : un rocher POSE (par opposition a un eboulis, qui s'accroche a
        # la paroi par definition) ne tient pas au-dela de la pente de plantation.
        if relief.pente(x, y) > SPEC["pente_plantation_deg"]:
            rejets_rochers["pente"] = rejets_rochers.get("pente", 0) + 1
            continue
        nature.poser(choisir(rng, ROCHERS), x, y, relief.hauteur(x, y), c_foret,
                     rot_z=rng.uniform(0.0, math.tau),
                     echelle=SPEC["echelle_nature"] * rng.uniform(0.55, 1.15),
                     famille="rocher")
        rochers += 1
    noter_defaut("foret.rochers", SPEC["rochers_total"], rochers,
                 "hors degagement du chemin (3,8 + 2,0 m), hors zones interdites, "
                 f"pente <= {SPEC['pente_plantation_deg']:.0f} deg", rejets_rochers)

    # -- culees du pont : la couture chemin/tablier ------------------------
    bpx, bpy_ = relief.pont_xy
    _proche = relief.pont_idx
    _a = relief.chemin[max(0, _proche - 1)]
    _b = relief.chemin[min(len(relief.chemin) - 1, _proche + 1)]
    _dx, _dy = _b[0] - _a[0], _b[1] - _a[1]
    _n = math.hypot(_dx, _dy) or 1.0
    _ux, _uy = _dx / _n, _dy / _n
    _lx, _ly = -_uy, _ux
    culees = 0
    for signe in (-1.0, 1.0):
        for k in range(SPEC["pont_culees"]):
            # 🚨 LES CULEES NE MONTENT PLUS SUR LA CHAUSSEE.
            # Elles s'etalaient de -4,34 a +4,34 m, donc a cheval sur l'axe :
            # mesure du 2026-08-17, trois pierres de 76 a 90 cm de haut a 0,10 /
            # 1,43 / 1,96 m du milieu de la route — et sans collider, puisque je
            # les avais ecartees comme « decor de sol ». On les voyait, on
            # marchait dedans.
            # Elles bordent maintenant le tablier au lieu de le traverser : la
            # borne basse est la demi-largeur du chemin, la haute son debord.
            bord = SPEC["chemin_demi_largeur"] + 0.5
            etal = 1.9
            f = k / max(1, SPEC["pont_culees"] - 1)          # 0..1
            lat = (bord + (etal - 0.5) * f) * (1.0 if k % 2 == 0 else -1.0)
            longi = signe * (SPEC["pont_demi_portee"] + 1.2)
            cxp = bpx + _ux * longi + _lx * lat
            cyp = bpy_ + _uy * longi + _ly * lat
            # 🚨 LE REPERE DU PONT EST DROIT, LE CHEMIN NE L'EST PAS.
            # `_ux/_uy` est la tangente AU PONT ; avancer de 14,2 m dessus sort
            # du tracé dès qu'il tourne. Mesuré : des culées posées à 2,7 m de
            # « l'axe » se retrouvaient à **0,24 m** du chemin réel, hautes de
            # 90 cm et sans collider — on marchait dedans.
            # C'est la troisième fois aujourd'hui que ce piège se referme (les
            # abris, la palissade, ces culées) : un repère local est valable AU
            # POINT où on le prend, jamais à quinze mètres de là. La garde se
            # verifie donc toujours contre la polyligne.
            cxp, cyp = ecarter_du_chemin(cxp, cyp, SPEC["chemin_demi_largeur"] + 0.5)
            culees += len(nature.poser(
                choisir(rng, [("stone_largeA.glb", 2), ("stone_largeC.glb", 2),
                              ("stone_smallTopA.glb", 1)]),
                cxp, cyp, relief.hauteur(cxp, cyp) - 0.3, c_reperes,
                rot_z=rng.uniform(0.0, math.tau),
                echelle=SPEC["echelle_nature"] * rng.uniform(0.7, 1.15)))

    # -- points d'appel ----------------------------------------------------
    # 1. L'arbre-monument sur son mamelon : visible du départ, hors du chemin.
    mx, my = SPEC["mamelon"]["xy"]
    nature.poser("tree_detailed.glb", mx, my, relief.hauteur(mx, my), c_reperes,
                 echelle=SPEC["echelle_nature"] * 4.2, famille="arbre",
                 matieres=MATIERES_TRONC)
    for i in range(7):
        ang = math.tau * i / 7.0
        rx, ry = mx + math.cos(ang) * 11.0, my + math.sin(ang) * 11.0
        nature.poser("statue_column.glb" if i % 2 else "statue_columnDamaged.glb",
                     rx, ry, relief.hauteur(rx, ry), c_reperes,
                     rot_z=ang, echelle=SPEC["echelle_nature"] * 1.25, famille="repere")

    # 2. CASCADE RETIRÉE — elle n'a plus de paroi d'où tomber.
    #
    # Elle s'adossait à des pièces `cliff_rock` (dalles de 4 × 0,67 × 4 m)
    # censées figurer sa falaise. Mais en raccourcissant la rivière pour qu'elle
    # cesse d'escalader la ceinture (sa nappe culminait à +28,74 m), sa tête
    # s'est retrouvée en terrain PLAT : les dalles se dressaient seules au
    # milieu du pré. Elles apparaissaient dans plusieurs rendus comme trois
    # panneaux beiges flottants, et j'ai mis plusieurs passes à les identifier.
    #
    # Pour la remettre il faut d'ABORD une paroi : prolonger la tête de rivière
    # jusqu'au pied du rempart (|y| > 84) et l'y adosser. Tant que ce n'est pas
    # fait, mieux vaut pas de cascade qu'une cascade sans falaise.

    # 3. Le belvédère du col : on souffle, et le village apparaît en contrebas.
    col_x, col_y = SPEC["crete"]["x"], SPEC["crete"]["col_y"]
    for dx, dy in ((-13.0, 9.0), (-11.0, -8.0), (12.0, 10.0), (13.0, -7.0)):
        ox2, oy2 = ecarter_du_chemin(col_x + dx, col_y + dy,
                                     SPEC["degagement_chemin"] + 2.0)
        nature.poser("statue_obelisk.glb", ox2, oy2, relief.hauteur(ox2, oy2), c_reperes,
                     rot_z=rng.uniform(0, math.tau), echelle=SPEC["echelle_nature"] * 1.5,
                     famille="repere")
    rx, ry = ecarter_du_chemin(col_x, col_y + 15.0, SPEC["degagement_chemin"] + 3.0)
    nature.poser("statue_ring.glb", rx, ry, relief.hauteur(rx, ry), c_reperes,
                 echelle=SPEC["echelle_nature"] * 2.0, famille="repere")

    # -- clairière de départ -----------------------------------------------
    # Les pieces BASSES restent traversables : `campfire_stones` est un cercle
    # de galets a ras de terre et `stump_round` une souche sous le saut de
    # 1,174 m. Leur donner un cylindre de 0,5 m de haut ferait trebucher le
    # joueur sur du decor de sol, dans les six premieres secondes de la partie.
    sx, sy = spawn["xy"]
    nature.poser("campfire_stones.glb", sx + 3.0, sy - 2.0, relief.hauteur(sx + 3.0, sy - 2.0), c_reperes)
    for nom, dx, dy in (("tent_detailedOpen.glb", -4.5, 3.5), ("tent_smallOpen.glb", -2.0, -5.5),
                        ("tent_smallClosed.glb", 5.5, 3.5)):
        nature.poser(nom, sx + dx, sy + dy, relief.hauteur(sx + dx, sy + dy), c_reperes,
                     rot_z=math.atan2(-dy, -dx), famille="camp")
    # Les souches s'ecartent du chemin : l'une d'elles etait a 1,55 m de l'axe,
    # haute de 83 cm et sans collider — le PREMIER objet que le joueur rencontre,
    # et il le traverse. `ecarter_du_chemin` la pousse sur le gradient.
    for dx, dy in ((1.5, 3.0), (-3.0, -2.0), (4.0, -4.0)):
        spx, spy = ecarter_du_chemin(sx + dx, sy + dy,
                                     SPEC["chemin_demi_largeur"] + 1.4)
        nature.poser("stump_round.glb", spx, spy, relief.hauteur(spx, spy), c_reperes)
    # LE PANNEAU REGARDE CELUI QUI ARRIVE. Son cap était écrit −70° : un angle
    # qui ne désigne rien, et qui pointait à côté dès que le tracé bougeait. Un
    # panneau se lit de face — on le tourne donc vers le chemin, en prenant la
    # perpendiculaire à sa tangente locale.
    spx, spy = sx + 10.0, sy + 1.5
    _tx, _ty = _tangente_chemin(relief, spx, spy)
    nature.poser("sign.glb", spx, spy, relief.hauteur(spx, spy), c_reperes,
                 rot_z=math.atan2(-_tx, _ty))
    nature.poser("log_stack.glb", sx - 7.0, sy + 6.0, relief.hauteur(sx - 7.0, sy + 6.0), c_reperes,
                 famille="camp")

    # -- village -----------------------------------------------------------
    vx, vy = place["xy"]
    rayon_mur = place["rayon"] - 2.0
    pas_ang = 12.0 / rayon_mur          # wall_straight = 2,0 × 6 = 12 m
    # L'angle de la porte se CALCULE : c'est celui du point ou le chemin coupe
    # l'anneau du rempart. Le fixer a pi (« plein ouest ») supposait que le
    # chemin arrivait par l'ouest — il arrivait par le sud-est, et la porte se
    # retrouvait sur un talus, 6,9 m sous le sol.
    # Il se calcule DESORMAIS AVANT les batiments, parce que c'est de lui que
    # toute la trame decoule : la rue part de la porte.
    croisement = min(
        relief.chemin,
        key=lambda pt: abs(math.hypot(pt[0] - vx, pt[1] - vy) - rayon_mur))
    angle_porte = math.atan2(croisement[1] - vy, croisement[0] - vx)

    # LA TRAME. Un repere local du village : `ur` remonte la rue depuis la
    # porte vers le centre, `lr` la traverse.
    urx, ury = -math.cos(angle_porte), -math.sin(angle_porte)
    lrx, lry = -ury, urx
    # Demi-largeur de la rue. Derivee du chemin qui y aboutit : une rue de
    # village est un peu plus large que la route de campagne qu'elle prolonge,
    # sinon la porte se lit comme un goulot.
    rue_demi = SPEC["chemin_demi_largeur"] + 1.0
    # Jour entre deux parcelles voisines : une venelle ou un joueur passe.
    venelle = SPEC["joueur"]["rayon_m"] * 2.0 + 1.4

    def emprise_bati(fichier):
        """Demi-emprise au sol de la piece, MESUREE. C'est elle qui espace les
        parcelles — un ecart ecrit a la main ne suit pas le kit."""
        return village.rayon_local(fichier) * SPEC["echelle_village"]

    parcelles = []            # (fichier, px, py, cap, rayon) — on POSE a la fin

    def poser_bati(fichier, longi, lat, cap):
        """Enregistre une parcelle en repere de rue. `cap` = azimut de la FACADE.

        On ne pose pas tout de suite : les quatre roles sont calcules
        independamment, donc rien ne garantit qu'ils ne se recouvrent pas.
        Mesure du premier jet : le marche chevauchait une maison de 6,3 m. Les
        positions passent donc par une relaxation avant d'exister.
        """
        px = vx + urx * longi + lrx * lat
        py = vy + ury * longi + lry * lat
        parcelles.append([fichier, px, py, cap, emprise_bati(fichier)])
        return 1

    batis = 0
    # Les rangees de la rue avancent depuis la porte vers la place, cote par
    # cote. L'abscisse de depart laisse la porte degagee ; chaque parcelle
    # avance de sa propre emprise plus une venelle — donc deux maisons ne se
    # touchent JAMAIS, quelle que soit la piece.
    longi_cote = {-1.0: -rayon_mur + 10.0, 1.0: -rayon_mur + 10.0}
    cote = 1.0
    for fichier, role in VILLAGE_TRAME:
        r = emprise_bati(fichier)
        if role == "centre":
            # Le puits est SUR la place, au bout de la rue. Pas de facade.
            batis += poser_bati(fichier, 0.0, 0.0, rng.uniform(0.0, math.tau))
        elif role == "fond":
            # Le clocher ferme la perspective : il est en FACE de la porte, et
            # il la REGARDE. C'est lui qu'on voit depuis le seuil.
            batis += poser_bati(fichier, rayon_mur - r - 4.0, 0.0,
                                math.atan2(-ury, -urx))
        elif role == "place":
            # De part et d'autre de la place, facades tournees vers elle.
            lat = (rue_demi + r + 2.0) * cote
            batis += poser_bati(fichier, r * 0.5, lat,
                                math.atan2(-lry * cote, -lrx * cote))
            cote = -cote
        else:  # "rue"
            lat = (rue_demi + r) * cote
            longi = longi_cote[cote] + r
            longi_cote[cote] = longi + r + venelle
            # La facade regarde la rue : sa normale pointe vers l'axe.
            batis += poser_bati(fichier, longi, lat,
                                math.atan2(-lry * cote, -lrx * cote))
            cote = -cote

    # RELAXATION — deux parcelles ne se recouvrent pas, et le rempart les tient.
    #
    # Chaque role calcule sa position sans connaitre les autres. On resout donc
    # les recouvrements en ecartant les paires fautives le long de leur axe,
    # puis on ramene tout le monde dans l'enceinte. Douze passes suffisent (la
    # correction decroit vite) ; ce qui resterait est PUBLIE, pas masque.
    recouvrement_final = 0.0
    for _ in range(12):
        pire = 0.0
        for i in range(len(parcelles)):
            for j in range(i + 1, len(parcelles)):
                a, b = parcelles[i], parcelles[j]
                dx, dy = b[1] - a[1], b[2] - a[2]
                d = math.hypot(dx, dy) or 1e-6
                voulu = a[4] + b[4] + venelle
                if d >= voulu:
                    continue
                pire = max(pire, voulu - d)
                pousse = (voulu - d) * 0.5
                ux_, uy_ = dx / d, dy / d
                a[1] -= ux_ * pousse
                a[2] -= uy_ * pousse
                b[1] += ux_ * pousse
                b[2] += uy_ * pousse
        # Puis on rentre tout le monde : une parcelle ne sort jamais du rempart.
        # Le clamp precedent utilisait 11 m pour TOUT le monde (la demi-emprise
        # du plus gros) et tassait donc les maisons les unes sur les autres.
        for p in parcelles:
            d = math.hypot(p[1] - vx, p[2] - vy)
            utile = rayon_mur - p[4] - 1.0
            if d > utile and d > 1e-6:
                p[1] = vx + (p[1] - vx) * utile / d
                p[2] = vy + (p[2] - vy) * utile / d
        recouvrement_final = pire
        if pire < 0.05:
            break

    batis = 0
    for fichier, px, py, cap, _r in parcelles:
        # PAS de `famille` : les bâtiments sont dans la collection `village`,
        # donc déjà fusionnés dans le TriMesh de collision (`91_export.py`). Y
        # ajouter un cylindre les collisionnerait DEUX fois — et un cylindre
        # autour d'une église en approxime très mal la forme.
        batis += len(village.poser(fichier, px, py, relief.hauteur(px, py),
                                   c_village, rot_z=cap))
    if recouvrement_final >= 0.05:
        noter_defaut("village.parcelles_disjointes", len(parcelles),
                     len(parcelles) - 1,
                     f"il reste {recouvrement_final:.2f} m de recouvrement apres "
                     "relaxation : l'enceinte est trop petite pour ces emprises. "
                     "Remede : agrandir place_village.rayon, ou retirer un bati")
    c_portes = collection("portes")
    # Le tour COMPLET : 9 pieces ne couvraient que 189,7 deg, soit un trou de
    # 170,3 deg — une enceinte a moitie batie n'enceint rien. Le nombre se
    # derive de la circonference : 2*pi*r / 12 m par piece.
    n_murs = max(8, int(round(2.0 * math.pi * rayon_mur / 12.0)))
    pas_ang = 2.0 * math.pi / n_murs
    murs = 0
    porte_manifeste = None
    for k in range(n_murs):
        ang = angle_porte + k * pas_ang
        mx2, my2 = vx + math.cos(ang) * rayon_mur, vy + math.sin(ang) * rayon_mur
        sol_mur = relief.hauteur(mx2, my2)
        piece = "walls/wall_straight_gate.gltf" if k == 0 else "walls/wall_straight.gltf"
        objets = village.poser(piece, mx2, my2, sol_mur, c_village,
                               rot_z=ang + math.pi / 2)
        murs += len(objets)
        if k == 0:
            # Les battants partent dans leur PROPRE collection : ils ne doivent
            # pas etre fusionnes avec le rempart, sinon plus rien ne peut les
            # faire pivoter a l'approche du joueur.
            battants = []
            for obj in objets:
                if "door" in obj.name.lower():
                    for c in list(obj.users_collection):
                        c.objects.unlink(obj)
                    cote = "gauche" if "left" in obj.name.lower() else "droite"
                    obj.name = f"porte_village_{cote}"
                    c_portes.objects.link(obj)
                    battants.append({
                        "nom": obj.name,
                        "cote": cote,
                        # Le gond est le bord exterieur du battant : c'est lui
                        # l'axe de rotation, pas le centre de l'objet.
                        "pivot_xyz": [round(obj.location.x, 2), round(obj.location.y, 2),
                                      round(obj.location.z, 2)],
                        "sens_ouverture_rad": round(
                            (1.0 if cote == "gauche" else -1.0) * math.radians(95.0), 4),
                    })
            porte_manifeste = {
                "centre_xyz": [round(mx2, 2), round(my2, 2), round(sol_mur, 2)],
                "cap_rad": round(ang + math.pi / 2, 4),
                # Rayon de declenchement : a 6,5 m/s, 7 m laissent ~1,1 s pour
                # que l'animation s'acheve avant qu'on touche le battant.
                "rayon_declenchement_m": 7.0,
                "duree_ouverture_s": 0.9,
                "battants": battants,
            }
    champs = 0
    for i in range(8):
        for j in range(6):
            ccx, ccy = vx - 10.0 + i * 3.4, vy - 30.0 + j * 3.0
            if math.hypot(ccx - vx, ccy - vy) > place["rayon"] - 1.0:
                continue
            # Le chemin traverse le village jusqu'au puits : un champ de ble
            # planté dessus, c'est 11 pieces en travers de la route (mesure).
            if distance_polyligne(ccx, ccy, relief.chemin) < 4.5:
                continue
            _cult = choisir(rng, [("crops_wheatStageB.glb", 3), ("crops_cornStageC.glb", 2),
                                  ("crops_leafsStageB.glb", 2), ("crop_pumpkin.glb", 1)])
            # L'echelle vient de la TAILLE REELLE de la plante, pas de celle du
            # kit : `echelle_nature * 0,9` donnait un mais de 4,50 m.
            nature.poser(_cult, ccx, ccy, relief.hauteur(ccx, ccy), c_village,
                         echelle=nature.echelle_pour(_cult, SPEC["echelle_nature"] * 0.9))
            champs += 1
    for i in range(14):
        ang = math.tau * i / 14.0
        fx, fy = vx + math.cos(ang) * (place["rayon"] - 8.0), vy + math.sin(ang) * (place["rayon"] - 8.0)
        if abs(math.cos(ang) + 1.0) < 0.35:      # on ne clôt pas la porte
            continue
        nature.poser("fence_simple.glb", fx, fy, relief.hauteur(fx, fy), c_village,
                     rot_z=ang + math.pi / 2, echelle=SPEC["echelle_nature"] * 1.1)

    # -- eclairage du chemin -----------------------------------------------
    c_lampes = collection("lampes")
    lcfg = SPEC["lampes"]
    kit_lampe = Kit(os.path.join(RACINE, "assets", "models"), lcfg["echelle"])
    lampes_manifeste = []
    longueur_chemin = sum(math.dist(relief.chemin[i], relief.chemin[i + 1])
                          for i in range(len(relief.chemin) - 1))
    parcouru = 0.0
    prochaine = lcfg["ecart_m"] * 0.5
    cote = 1.0
    for i in range(len(relief.chemin) - 1):
        a, b = relief.chemin[i], relief.chemin[i + 1]
        seg = math.dist(a, b)
        if parcouru + seg < prochaine:
            parcouru += seg
            continue
        while parcouru + seg >= prochaine:
            t = (prochaine - parcouru) / max(1e-6, seg)
            x = a[0] + (b[0] - a[0]) * t
            y = a[1] + (b[1] - a[1]) * t
            dx, dy = b[0] - a[0], b[1] - a[1]
            n = math.hypot(dx, dy) or 1.0
            # Alternance gauche/droite : deux files paralleles donneraient un
            # couloir d'aeroport, une alternance donne un chemin balise.
            # ON CHOISIT LE COTE. Le cote alterne est un souhait, pas une
            # obligation : la ou le chemin longe la riviere, il jette le
            # brasero a l'eau. Mesure : 4 braseros dans le lit, 3 carrement
            # SOUS la nappe (jusqu'a 1,30 m dessous).
            candidat = None
            for essai in (cote, -cote):
                lx = -dy / n * lcfg["lateral_m"] * essai
                ly = dx / n * lcfg["lateral_m"] * essai
                cx2, cy2 = x + lx, y + ly
                # La garde du chemin d'abord : dans un virage, le decalage
                # lateral seul ramene la piece vers la route.
                cx2, cy2 = ecarter_du_chemin(cx2, cy2,
                                             SPEC["degagement_chemin"] + lcfg["marge_m"])
                z2 = relief.hauteur(cx2, cy2)
                d_eau = distance_polyligne(cx2, cy2, relief.riviere)
                # Deux refus : trop pres du lit, ou sous la nappe. Le second
                # attrape les cas que le premier laisse passer quand la
                # riviere s'elargit.
                if d_eau < lcfg["degagement_eau_m"]:
                    continue
                if z2 < relief.niveau_eau_en(cx2, cy2) + 0.3:
                    continue
                candidat = (cx2, cy2, z2, essai)
                break
            if candidat is None:
                # Aucune rive ne convient : on saute cette station. Un trou
                # d'eclairage se voit moins qu'un brasero noye.
                prochaine += lcfg["ecart_m"]
                continue
            px, py, sol, cote = candidat[0], candidat[1], candidat[2], candidat[3]
            kit_lampe.poser(lcfg["piece"], px, py, sol, c_lampes,
                            rot_z=rng.uniform(0.0, math.tau), famille="brasero")
            lampes_manifeste.append({
                "xyz": [round(px, 2), round(py, 2), round(sol, 2)],
                "flamme_xyz": [round(px, 2), round(py, 2),
                               round(sol + lcfg["hauteur_flamme_m"], 2)],
                # 0 au depart, 1 au village : c'est cette valeur que le moteur
                # compare a l'avancee du joueur pour allumer de proche en proche.
                "avancee": round(prochaine / max(1e-6, longueur_chemin), 4),
                "portee_m": lcfg["portee_lumiere_m"],
            })
            cote = -cote
            prochaine += lcfg["ecart_m"]
        parcouru += seg

    # -- campements ennemis, verrous du chemin -----------------------------
    c_camps = collection("campements")
    cfg = SPEC["campements"]
    camps_manifeste = []
    for numero, camp in enumerate(camps, start=1):
        cx, cy = camp["xy"]
        ux, uy = camp["avance"]
        base = math.atan2(uy, ux)
        sol_camp = relief.profil_en(cx, cy)[0]

        # Repere local du camp : `avance` le long du chemin, `lat` en travers.
        # Tout se pose en (longitudinal, lateral) — c'est ce qui permet de
        # GARANTIR que la route reste libre, au lieu de l'esperer.
        lx, ly = -uy, ux
        LIBRE = SPEC["campements"]["couloir_libre"]

        rejets_camp = {}

        # La composition de CE camp, et les deux durées qui s'en déduisent.
        # `numero` part à 1 ; la liste est indexée depuis 0. Un camp au-delà de
        # la liste reprend la dernière composition plutôt que de lever — mieux
        # vaut un camp peuplé comme son voisin qu'une carte qui ne cuit pas.
        composition = cfg["composition"][min(numero - 1, len(cfg["composition"]) - 1)]
        pv_total = sum(n * PV.get(a, 30.0) for a, n in composition.items())
        # Le plus LENT décide de l'arrivée de l'essaim, pas le plus rapide.
        vitesses = {"grunt": 9.0, "archer": 5.5, "elite": 5.0}
        v_lente = min(vitesses.get(a, 5.0) for a in composition)
        melee = 3.0
        duree_approche = max(0.0, (cfg["apparition_rayon"][1] - melee)) / v_lente
        # PLANCHER DE DURÉE — dérivé, pas choisi. Un engagement doit au moins
        # laisser le temps de traverser la salle une fois : sous cette durée, le
        # combat est fini avant qu'on ait pu se repositionner, donc la salle et
        # ses six abris n'ont servi à rien.
        #   plancher = diametre / vitesse de marche
        duree_plancher = (cfg["rayon"] * 2.0) / SPEC["joueur"]["marche_ms"]
        duree_tir = pv_total / cfg["arsenal_dps"]
        if duree_tir < duree_plancher:
            noter_defaut(
                f"camp_{numero}.duree_engagement",
                # On compare des dixièmes de seconde : le compteur les porte.
                round(duree_plancher * 10), round(duree_tir * 10),
                f"{duree_tir:.1f} s d'engagement pour un plancher de "
                f"{duree_plancher:.1f} s (diametre / vitesse de marche). "
                f"{sum(composition.values())} ennemis a {cfg['arsenal_dps']:.0f} dps "
                "meurent avant qu'on ait traverse la salle. Remede : des VAGUES "
                "— le compte simultane est plafonne par la lisibilite, pas la duree")

        def poser_camp(fichier, longi, lat, famille="camp", **kw):
            """Pose en repere du camp. Refuse tout ce qui empieterait sur la route.

            Le refus se COMPTE : rendre 0 en silence est exactement ce qui a fait
            disparaitre trois abris sans que rien ne le signale.
            """
            if abs(lat) < LIBRE:
                rejets_camp["couloir"] = rejets_camp.get("couloir", 0) + 1
                return 0
            px = cx + ux * longi + lx * lat
            py = cy + uy * longi + ly * lat
            return len(nature.poser(fichier, px, py, relief.hauteur(px, py), c_camps,
                                    famille=famille, **kw))

        # Barricade : elle ENCADRE la route au lieu de la murer. Les pieces font
        # 1,0 x 4,6 m ; centrees a +/-4,6 elles laissent une trouee de 4,6 m,
        # soit exactement la largeur du chemin. C'est un poste de controle
        # qu'on franchit, pas un mur — le moteur y posera son verrou quand le
        # camp sera actif, et le retirera une fois nettoye.
        bx, by = cx + ux * 8.0, cy + uy * 8.0
        largeur_piece = SPEC["echelle_nature"] * 1.15
        # 🚨 LA BARRICADE LONGE LA ROUTE — elle ne la traverse pas.
        #
        # Elle sortait perpendiculairement dans le pré : trois pièces par côté à
        # 5,9 / 10,5 / 15,1 m de l'axe, alignées en travers. Vu en scène, ça ne
        # ressemblait à rien — un mur planté au milieu d'un champ, qui ne borde
        # rien et n'enferme rien.
        #
        # Antoine les a repositionnées à la main le 2026-08-17, et la mesure de
        # son geste est sans ambiguïté : les trois pièces sont passées de
        # (5,5 / 10,1 / 14,7 m de l'axe) à (5,4 / 5,4 / 5,8 m), c'est-à-dire
        # toutes à la MÊME distance du bord, mais échelonnées sur une vingtaine
        # de mètres LE LONG du chemin, avec de petites rotations suivant sa
        # courbe.
        #
        # C'est une palissade de bord de route : elle marque le territoire du
        # camp en le bordant. Le rôle de VERROU, lui, n'a jamais été porté par
        # cette géométrie — il est dans `verrou_xyz` du manifeste.
        #
        # La longueur se dérive du camp : `2 x rayon` de couverture le long de
        # la route, soit trois pièces par côté à la largeur du kit.
        # LES POSITIONS SE DÉRIVENT, et dans le BON SENS.
        #
        # Elles étaient écrites (±5,4 / ±10,0 / ±14,6) et la mesure les condamne :
        # une pièce de 4,6 m centrée à 5,4 a son bord intérieur à **3,1 m**,
        # donc elle mord de 0,5 m dans la garde de `couloir_libre` (3,6 m).
        #
        # 🚨 Le correctif proposé par l'analyse partait de `chemin_demi_largeur`
        # (2,2) et donnait 4,5 — soit un bord intérieur à 2,2 m, la garde
        # ENTIÈREMENT supprimée, 2,8 fois pire qu'aujourd'hui. La barricade est
        # un prop de camp : sa borne est `couloir_libre`, pas la largeur du
        # pavé. Le bord intérieur doit donc être À `LIBRE`, ce qui place le
        # centre de la première pièce à `LIBRE + largeur/2` — c'est-à-dire PLUS
        # LOIN qu'aujourd'hui, pas plus près.
        #
        # Les suivantes se suivent d'une largeur de pièce : jointives, sans
        # recouvrement ni jour, ce qu'aucune liste écrite à la main ne garantit.
        # 🚨 LA BARRICADE EST UNE CHICANE. C'est le seul prop du camp qui a le
        # droit d'entrer dans le couloir de marche — parce que son rôle est
        # justement de le contraindre.
        #
        # Antoine l'a repositionnée à la main le 2026-08-17, et le relevé est
        # sans ambiguïté : les pièces ALTERNENT d'un bord à l'autre en avançant
        # (latéral +1,1 / −5,5 / −4,5 / +3,2 / −2,6 sur 15 m). On slalome. Deux
        # lignes parallèles, elles, ne contraignaient rien — on passait droit au
        # milieu, et le « poste de contrôle qu'on franchit » du commentaire
        # d'origine n'a jamais existé sur le terrain.
        #
        # Ce qu'elle doit garantir n'est donc PAS une garde latérale, mais la
        # part de chaussée qu'elle laisse libre. Et cette part se dérive :
        #
        #   chaque pièce n'empiète que d'UN QUART de la largeur de la route.
        #
        # C'est-à-dire, en résolvant `R − (L − W/2) = R/2` :
        #
        #   latéral = largeur_piece/2 + demi_route/2 = 2,30 + 1,10 = 3,40 m
        #
        # 🚨 J'avais d'abord dérivé 1,30 m, en raisonnant sur le passage
        # résiduel (un corps + un corps de jeu). Antoine a repoussé la pièce
        # centrale de −1,3 à **−3,5 m** : ma chicane mordait la moitié de la
        # chaussée, on ne slalomait plus, on se cognait. Le quart reproduit son
        # geste à 10 cm près.
        #
        # La leçon vaut plus que le nombre : une contrainte se pose sur ce qui
        # RESTE LIBRE, pas sur ce qu'on ajoute. Formulée à l'envers, elle se
        # règle à l'envers.
        lat_chicane = largeur_piece / 2.0 + SPEC["chemin_demi_largeur"] / 2.0
        # Et des pièces de FLANC, hors du couloir : elles ferment le camp de
        # part et d'autre du poste. Antoine en a gardé deux — la chicane sans
        # elles se lit comme trois planches perdues sur une route.
        lat_flanc = LIBRE + largeur_piece / 2.0
        pieces = []
        for k in range(3):                       # la chicane, sur la route
            pieces.append((8.0 + (k - 1) * largeur_piece,
                           lat_chicane * (1.0 if k % 2 == 0 else -1.0)))
        for k in range(2):                       # les flancs, hors couloir
            pieces.append((8.0 + (k - 0.5) * largeur_piece * 1.6,
                           -lat_flanc if k == 0 else lat_flanc))
        for longi, lat in pieces:
                px = cx + ux * longi + lx * lat
                py = cy + uy * longi + ly * lat
                # LE CAP SUIT LA COURBE DE LA ROUTE, pas l'axe du camp. Une
                # palissade qui longe un chemin sinueux et garde un cap unique
                # s'en écarte à ses extrémités — c'est ce que les rotations
                # manuelles (jusqu'à 16°) corrigeaient.
                cap = math.atan2(*_tangente_chemin(relief, px, py)[::-1])
                nature.poser("fence_planksDouble.glb", px, py,
                             relief.hauteur(px, py), c_camps,
                             rot_z=cap, echelle=largeur_piece, famille="camp")

        # Vie du camp, TOUTE de cote : on doit lire « quelqu'un habite ici »
        # sans avoir a enjamber leur feu.
        poser_camp("campfire_logs.glb", 0.0, 6.5)
        for fichier, longi, lat in (("tent_detailedOpen.glb", -3.0, 8.5),
                                    ("tent_detailedClosed.glb", 4.0, 9.5),
                                    ("tent_detailedClosed.glb", -5.0, -8.0),
                                    ("tent_smallClosed.glb", 2.0, -9.0)):
            poser_camp(fichier, longi, lat, rot_z=base + (math.pi if lat > 0 else 0.0))
        poser_camp("log_stack.glb", 6.0, -7.0, rot_z=base)
        poser_camp("log_stack.glb", -7.0, 9.0, rot_z=base)
        poser_camp("statue_block.glb", -8.0, -6.5,
                   echelle=SPEC["echelle_nature"] * 1.2)

        # ABRIS — ce ne sont pas des rochers, ce sont les MEUBLES DU COMBAT.
        #
        # Seuls des blocs >= 1,8 m cassent la ligne de vue : l'oeil du joueur est
        # a 1,70 m et il n'y a PAS d'accroupissement dans Forgia
        # (`map-design-patterns.md` §11). Un abri plus bas masque le corps sans
        # masquer la vue — il ne sert donc a rien, et il faut le SAVOIR.
        #
        # Deux defauts corriges ici :
        #
        # 1. Trois abris sur dix-huit disparaissaient. L'angle etait tire
        #    regulierement puis l'abri ETAIT JETE s'il tombait dans le couloir de
        #    marche. Un tirage qui marche en moyenne et rate parfois est
        #    exactement ce qui produit un manque silencieux. On RESOUT desormais
        #    la contrainte au lieu de retirer (cf. `hors_couloir`) : les six sont
        #    poses, tous.
        # 2. Leur hauteur n'etait ni mesuree ni publiee — le manifeste ne
        #    donnait que `[x, y]`. Impossible de verifier qu'un abri abrite sans
        #    rouvrir Blender. Elle est desormais relevee sur la piece placee, et
        #    `casse_la_vue` s'en DEDUIT (`map-design-intention.md` §5.1 : le nom
        #    est un contrat, donc il se verifie).
        abris = []
        for k in range(cfg["abris"]):
            r = max(rng.uniform(*cfg["abri_rayon"]), LIBRE + 0.4)
            ang = base + hors_couloir(math.tau * (k + 0.5) / cfg["abris"], r, LIBRE)
            ax2, ay2 = cx + math.cos(ang) * r, cy + math.sin(ang) * r
            # 🚨 LA GARDE SE VERIFIE SUR LE CHEMIN REEL, PAS SUR L'AXE DU CAMP.
            #
            # `hors_couloir` resout la contrainte contre l'axe DROIT du camp.
            # Mais le chemin COURBE : une piece conforme a cet axe peut se
            # retrouver plus pres du trace reel. Releve par Antoine a l'oeil le
            # 2026-08-17 — un abri a 3,36 m du chemin pour une garde de 3,60 m,
            # qu'il a pousse a 4,26 m.
            #
            # On mesure donc contre la polyligne, et on ecarte en suivant le
            # gradient (`ecarter_du_chemin` fait exactement ca). Un controle qui
            # verifie une approximation ne verifie pas l'invariant.
            ax2, ay2 = ecarter_du_chemin(ax2, ay2, LIBRE)
            # LA PIÈCE SE CHOISIT SUR SES PROPORTIONS, pas au hasard.
            #
            # Contraindre la seule hauteur ne suffit pas : l'échelle est
            # uniforme, donc agrandir une pièce basse pour atteindre 2,5 m la
            # rend large d'autant. Vu en rendu — deux blocs qui barraient toute
            # la clairière et débordaient sur le chemin.
            #
            # On teste donc chaque candidate à l'échelle qu'imposerait la
            # hauteur voulue, et on garde la première assez ÉTROITE. À défaut,
            # la moins large — jamais rien, sinon l'abri disparaît en silence.
            candidates = [("stone_tallB.glb", 2), ("stone_tallF.glb", 2),
                          ("stone_largeC.glb", 1)]
            h_voulue = rng.uniform(*cfg["abri_hauteur_m"])
            r_max = cfg["abri_rayon_max_m"]
            piece, meilleur_r = None, None
            for essai in range(6):
                cand = choisir(rng, candidates)
                h_loc = nature.hauteur_locale(cand)
                if h_loc <= 1e-6:
                    continue
                r_monde = nature.rayon_local(cand) * (h_voulue / h_loc)
                if meilleur_r is None or r_monde < meilleur_r[1]:
                    meilleur_r = (cand, r_monde)
                if r_monde <= r_max:
                    piece = cand
                    break
            if piece is None:
                piece = meilleur_r[0] if meilleur_r else candidates[0][0]
                rejets_camp["abri_trop_large"] = rejets_camp.get("abri_trop_large", 0) + 1
            # LA HAUTEUR D'UN ABRI EST BORNEE DES DEUX COTES.
            #
            # La passe precedente ne posait qu'un PLANCHER : `ech = max(ech, ...)`
            # ne faisait que relever, jamais redescendre. Resultat mesure —
            # abris de 1,95 a 4,87 m, et vu en rendu : des blocs qui ecrasent la
            # clairiere et la rendent illisible depuis son seuil (§3.3).
            #
            # Une couverture se definit par DEUX bornes, et les deux se derivent
            # de l'oeil du joueur :
            #   - plancher : oeil + 0,25 — en dessous elle masque le corps sans
            #     masquer la vue, donc elle ment (§11) ;
            #   - plafond : oeil + 1,30 — au-dela on ne voit plus par-dessus
            #     depuis un point legerement haut, la salle cesse de se lire, et
            #     l'abri devient un mur.
            # L'echelle DECOULE de la hauteur voulue au lieu d'etre tiree puis
            # rattrapee : c'est la hauteur qui est la grandeur de design.
            # L'ECHELLE SE CALCULE, elle ne se tire pas. Un abri existe pour
            # depasser l'oeil : sa hauteur est sa fonction, pas un effet de bord
            # du hasard. Le premier jet tirait dans [1,0 ; 1,4] et sortait deux
            # blocs a 1,52 et 1,77 m — sous les 1,80 m requis, donc deux fausses
            # couvertures que seul le controle `casse_la_vue` a revelees.
            # On mesure la piece, et on impose l'echelle minimale qui tient le
            # contrat. `+0,25` de marge : l'abri se pose sur un sol irregulier,
            # et le joueur peut le regarder depuis un point legerement haut.
            h_locale = nature.hauteur_locale(piece)
            ech = (h_voulue / h_locale) if h_locale > 1e-6 else SPEC["echelle_nature"]
            nature.poser(piece, ax2, ay2, relief.hauteur(ax2, ay2), c_camps,
                         rot_z=rng.uniform(0.0, math.tau), echelle=ech,
                         famille="abri")
            # L'emprise vient d'etre mesuree par `poser` : on la relit plutot que
            # de la recalculer, sinon les deux formules divergeront un jour.
            pose = nature.solides.get("abri", [])
            if not pose:
                continue
            _, _, _, haut, ray = pose[-1]
            abris.append({
                "xyz": [round(ax2, 2), round(ay2, 2), round(relief.hauteur(ax2, ay2), 2)],
                "rayon_m": ray,
                "hauteur_m": haut,
                # DERIVE de la hauteur mesuree, jamais declare : un abri qui ne
                # casse pas la vue doit sortir `false` et se voir, pas se
                # deguiser en couverture.
                "casse_la_vue": haut >= OEIL_JOUEUR_M + 0.1,
            })
        noter_defaut(f"camp_{numero}.abris", cfg["abris"], len(abris),
                     "contrainte de couloir resolue par l'angle, plus par rejet",
                     dict(rejets_camp))
        aveugles = [a for a in abris if not a["casse_la_vue"]]
        if aveugles:
            noter_defaut(f"camp_{numero}.abris_qui_abritent", len(abris),
                         len(abris) - len(aveugles),
                         f"{len(aveugles)} bloc(s) sous {OEIL_JOUEUR_M + 0.1:.1f} m : "
                         "ils masquent le corps, pas la vue")

        # Apparitions : reparties devant et sur les flancs (+/-110 deg), jamais
        # dans le dos du joueur au contact — une arrivee invisible se lit comme
        # une triche (map-design-intention.md §2.4).
        apparitions = []
        n_app = cfg["apparitions"]
        for k in range(n_app):
            ang = base + math.radians(-110.0 + 220.0 * k / max(1, n_app - 1))
            r = rng.uniform(*cfg["apparition_rayon"])
            sxp, syp = cx + math.cos(ang) * r, cy + math.sin(ang) * r
            apparitions.append([round(sxp, 2), round(syp, 2),
                                round(relief.hauteur(sxp, syp) + 0.2, 2)])

        camps_manifeste.append({
            "id": f"camp_{numero}",
            "centre_xyz": [round(cx, 2), round(cy, 2), round(sol_camp, 2)],
            "rayon_m": cfg["rayon"],
            "verrou_xyz": [round(bx, 2), round(by, 2), round(relief.hauteur(bx, by), 2)],
            "verrou_cap_rad": round(base, 4),
            "apparitions_xyz": apparitions,
            # `abris_xy` (des couples muets) remplace par des abris qui portent
            # leur contrat : emprise, hauteur, et le verdict qui en decoule.
            "abris": abris,
            "archetypes": cfg["archetypes"],
            # --- la spec de combat, §1 -----------------------------------
            "role": "combat",
            "effectifs": composition,
            "arsenal_dps": cfg["arsenal_dps"],
            "condition_sortie": cfg["condition_sortie"],
            # DÉRIVÉES, pas déclarées — c'est ce qui permet de vérifier la spec
            # au chargement au lieu de la croire sur parole.
            #   duree_tir  = somme(effectif x pv) / dps de l'arsenal
            #   duree_approche = (rayon d'apparition le plus lointain
            #                     - portee de melee) / vitesse du plus lent
            # Si l'approche dépasse le tir, l'essaim MEURT EN CHEMIN et la salle
            # se joue toute seule (§2.1) : c'est exactement ce que ces deux
            # nombres, mis côte à côte, rendent visible.
            "duree_tir_s": round(pv_total / cfg["arsenal_dps"], 1),
            "duree_approche_s": round(duree_approche, 1),
            "essaim_arrive": duree_approche <= pv_total / cfg["arsenal_dps"],
            "duree_plancher_s": round(duree_plancher, 1),
            # Ligne max = diametre de la clairiere. Elle DERIVE maintenant du
            # meme rayon que la vision a fixe : l'invariant « ligne <= vision »
            # est vrai par construction, il n'a plus a etre espere.
            "ligne_max_m": round(cfg["rayon"] * 2.0, 1),
            # Nom conserve pour le lecteur moteur, mais la valeur est LUE dans
            # `enemy_grunt.toml` au lieu d'etre recopiee.
            "grunt_vision_m": round(VISIONS.get("grunt", 20.0), 1),
            "vision_min_m": round(cfg["vision_min_m"], 1),
        })

    # -- zones de faune ----------------------------------------------------
    c_faune = collection("faune_controle")
    faune_manifeste = []
    zones_posees = []
    for espece, (milieu, nb_zones, effectif, couleur) in SPEC["faune"]["especes"].items():
        crit = SPEC["faune"]["milieux"][milieu]
        poses = 0
        essais = 0
        # Chaque `continue` ci-dessous compte sa cause. Sans ca, « 0 zone sur 2 »
        # ne dit pas OU desserrer — et c'est precisement ce qui a laisse les
        # poules absentes de la carte sans que personne ne s'en apercoive.
        rejets = {}

        def refuser(cause):
            rejets[cause] = rejets.get(cause, 0) + 1
            return True

        while poses < nb_zones and essais < 4000:
            essais += 1
            # La borne porte sur le BORD de la zone, pas sur son centre : avec
            # une marge fixe, un disque de 20 m de rayon debordait sur la
            # ceinture rocheuse — un troupeau a moitie dans la falaise.
            # Sol jouable = |coord| < demi x rim_debut.
            bx = SPEC["demi_x"] * SPEC["rim_debut"] - crit["rayon"] - 4.0
            by = SPEC["demi_y"] * SPEC["rim_debut"] - crit["rayon"] - 4.0
            x = rng.uniform(-bx, bx)
            y = rng.uniform(-by, by)
            if relief.pente(x, y) > crit["pente"] and refuser("pente"):
                continue
            if crit.get("sur_crete"):
                cr = SPEC["crete"]
                # Sur le dos de la crete, et a l'ecart du col.
                if abs(x - cr["x"]) > cr["epaisseur"] * 0.75 and refuser("hors_crete"):
                    continue
                if abs(y - cr["col_y"]) < cr["col_largeur"] * 0.9 and refuser("dans_le_col"):
                    continue
            d_ch = distance_polyligne(x, y, relief.chemin)
            if not (crit["chemin"][0] <= d_ch <= crit["chemin"][1]) and refuser("chemin"):
                continue
            d_ri = distance_polyligne(x, y, relief.riviere)
            if not (crit["riviere"][0] <= d_ri <= crit["riviere"][1]) and refuser("riviere"):
                continue
            # UNE BERGE EST DEFINIE PAR SA HAUTEUR SUR L'EAU, pas par sa distance
            # en plan. Le profil de la nappe est deja calcule station par
            # station : on le LIT. Sans ce critere, les manchots sont sortis a
            # 13,7 m au-dessus de leur rivage, sur un flanc de colline.
            if "hauteur_sur_nappe_m" in crit:
                sur_nappe = relief.hauteur(x, y) - relief.niveau_eau_en(x, y)
                bas, haut = crit["hauteur_sur_nappe_m"]
                if not (bas <= sur_nappe <= haut) and refuser("hauteur_sur_nappe"):
                    continue
            d_vi = math.hypot(x - place["xy"][0], y - place["xy"][1])
            if not (crit["village"][0] <= d_vi <= crit["village"][1]) and refuser("village"):
                continue
            if any(math.hypot(x - cp["xy"][0], y - cp["xy"][1])
                   < SPEC["campements"]["rayon"] + crit["rayon"]
                   for cp in camps) and refuser("campement"):
                continue
            ecart = crit.get("ecart", SPEC["faune"]["ecart_min"])
            if any(math.hypot(x - zx, y - zy) < ecart + crit["rayon"]
                   for zx, zy, _ in zones_posees) and refuser("ecart_entre_zones"):
                continue
            zones_posees.append((x, y, crit["rayon"]))
            faune_manifeste.append({
                "espece": espece, "milieu": milieu,
                "centre_xyz": [round(x, 2), round(y, 2), round(relief.hauteur(x, y), 2)],
                "rayon_m": crit["rayon"], "effectif": effectif,
            })
            # (Les disques de controle ont ete retires : ils avaient servi a
            # juger la repartition, ils masquaient ensuite ce qu'ils reperaient.
            # Les zones restent dans le manifeste, ou le moteur les lit.)
            poses += 1

        noter_defaut(f"faune.{espece}", nb_zones, poses,
                     f"milieu « {milieu} » : aucun creneau ne satisfait tous ses "
                     f"criteres apres {essais} tirages", rejets)

    # -- apercu de la faune ------------------------------------------------
    # Les betes sont posees pour JUGER l'echelle et la repartition, et pour
    # rien d'autre : en jeu c'est le moteur qui les fera apparaitre et
    # deambuler. Cette collection est donc ecartee de la cuisson, comme les
    # disques de controle — une bete cuite dans la carte serait une statue.
    c_apercu = collection("faune_apercu")
    kit_animaux = Kit(DOSSIER_ANIMAUX, 1.0, filtre_nom=True)  # deja en metres
    betes = 0
    for zone in faune_manifeste:
        cx, cy = zone["centre_xyz"][0], zone["centre_xyz"][1]
        r = zone["rayon_m"]
        for k in range(zone["effectif"]):
            # Tirage en racine : reparti sur toute la surface, pas agglutine
            # au centre.
            rad = r * 0.85 * math.sqrt(rng.random())
            ang = rng.uniform(0.0, math.tau)
            bx, by = cx + math.cos(ang) * rad, cy + math.sin(ang) * rad
            betes += len(kit_animaux.poser(
                f"{zone['espece']}.glb", bx, by, relief.hauteur(bx, by), c_apercu,
                rot_z=rng.uniform(0.0, math.tau)))

    canon, applique = unifier_materiaux()

    # VARIATION DES PIERRES. Tous les blocs partagent un unique materiau
    # `stone` : d'ou ces amas d'un bleu-gris parfaitement uniforme. On decline
    # 4 nuances et on les affecte PAR OBJET (`link = "OBJECT"`), ce qui laisse
    # les meshes partages intacts — sinon il faudrait dupliquer 500 meshes.
    variantes = {}
    for base, nuances in SPEC["nuances_pierre"].items():
        mere = canon.get(base)
        if mere is None:
            continue
        lot = []
        for k, teinte in enumerate(nuances):
            v = mere.copy()
            v.name = f"{base}_v{k}"
            bsdf = next((n for n in v.node_tree.nodes if n.type == "BSDF_PRINCIPLED"), None)
            if bsdf is not None and not bsdf.inputs["Base Color"].is_linked:
                couleur = hex_lineaire(teinte)
                bsdf.inputs["Base Color"].default_value = couleur
                v.diffuse_color = couleur
            lot.append(v)
        variantes[base] = lot

    # ⚠️ LES BRASEROS RESTENT SOMBRES, ET C'EST UN CONSTAT, PAS UN OUBLI.
    #
    # Jugés à l'œil sur les rendus du 2026-08-17, ils sortent quasi noirs sur un
    # décor pastel — le seul objet de la carte qu'on ne distingue pas de son
    # ombre. J'ai tenté de relever leur facteur de couleur de base : la mesure
    # dit que ce facteur est LIÉ À UNE TEXTURE, donc il n'est pas lu. Leur
    # noirceur est peinte, pas déclarée.
    #
    # La corriger demande soit de retoucher la texture du kit, soit d'insérer un
    # nœud de mélange — que l'exportateur glTF ne transporterait pas. Ce n'est
    # donc pas un réglage, c'est un chantier d'asset. Écrit ici pour que
    # personne ne le retente en croyant à un oubli.

    nuances_posees = 0
    for obj in bpy.data.objects:
        if obj.type != "MESH" or not obj.material_slots:
            continue
        for slot in obj.material_slots:
            if slot.material is None:
                continue
            lot = variantes.get(slot.material.name.split(".")[0])
            if not lot:
                continue
            slot.link = "OBJECT"
            slot.material = lot[(len(obj.name) * 7 + sum(ord(c) for c in obj.name)) % len(lot)]
            nuances_posees += 1
    bpy.context.view_layer.update()

    longueur = sum(math.dist(relief.chemin[i], relief.chemin[i + 1])
                   for i in range(len(relief.chemin) - 1))
    hauteurs = [relief.hauteur(x, y)
                for x in range(-110, 111, 10) for y in range(-70, 71, 10)]
    rapport = {
        "chemin_m": round(longueur, 1),
        "marche_s": round(longueur / SPEC["joueur"]["marche_ms"], 1),
        "profil_min_max": [round(min(relief._profil), 2), round(max(relief._profil), 2)],
        "terrain_min_max": [round(min(hauteurs), 2), round(max(hauteurs), 2)],
        "faces_roche_haut_berge_total": [faces_roche, faces_haut, faces_berge, faces_total],
        "eboulis": eboulis, "culees_pont": culees, "lampes": len(lampes_manifeste),
        "ceinture_creux": creux_trouves, "roches_bouchon": roches_bouchon,
        "pont_pieces": len(pont_pieces) if isinstance(pont_pieces, list) else int(pont_pieces),
        "arbres": arbres, "sousbois": sousbois, "rochers": rochers,
        "herbe": herbe, "pierres_lit": pierres,
        "batiments": batis, "murs": murs, "champs": champs,
        "objets": len(bpy.data.objects),
        "materiaux": len(canon), "teintes_appliquees": applique,
        "nuances_pierre": nuances_posees,
        "betes_apercu": betes,
        "zones_faune": [(f["espece"], f["centre_xyz"][:2], f["effectif"]) for f in faune_manifeste],
        "kit_manquant": sorted(set(nature.manquants + village.manquants)),
    }
    # -- manifeste pour le moteur -----------------------------------------
    # Le GLB porte la forme ; ce fichier porte les POINTS. Un spawn ou un trace
    # relus dans la geometrie seraient des devinettes : ici ils sont ecrits par
    # celui qui les a places.
    sx0, sy0 = spawn["xy"]
    manifeste = {
        "carte": "expedition_vallon",
        "emprise_m": [SPEC["demi_x"] * 2, SPEC["demi_y"] * 2],
        "echelle_glb": 1.0,
        # Blender est Z-up, Bevy Y-up : l'export glTF convertit deja. Les points
        # ci-dessous sont donnes en repere BLENDER (x, y=profondeur, z=hauteur).
        # Cote Bevy : (x, z, -y).
        "repere": "blender_z_up",
        "spawn": {
            "xyz": [sx0, sy0, round(relief.hauteur(sx0, sy0) + 0.2, 3)],
            "regard_xy": [relief.chemin[6][0], relief.chemin[6][1]],
            # Dalle deterministe sous le spawn : un point critique ne depend
            # jamais d'un TriMesh (lecon castle_hub.rs:80-93).
            "dalle_demi": [6.0, 6.0, 0.25],
        },
        "village_xyz": [place["xy"][0], place["xy"][1],
                        round(relief.hauteur(*place["xy"]), 3)],
        "pont_xyz": [round(relief.pont_xy[0], 2), round(relief.pont_xy[1], 2),
                     round(relief.profil_en(*relief.pont_xy)[0], 3)],
        "arbre_monument_xyz": [SPEC["mamelon"]["xy"][0], SPEC["mamelon"]["xy"][1],
                               round(relief.hauteur(*SPEC["mamelon"]["xy"]), 3)],
        "chemin_xyz": [[round(x, 2), round(y, 2), round(relief.profil_en(x, y)[0], 2)]
                       for (x, y) in relief.chemin[::2]],
        "eau": {
            "objet_glb": "vallon_eau",
            # UV alignees sur le courant : U = largeur (0..1), V = abscisse le
            # long du cours / tuile_m. Le moteur donne le courant en faisant
            # defiler V dans le temps (StandardMaterial::uv_transform), sans
            # flow map ni simulation — la direction est dans la geometrie.
            "uv": "U=largeur, V=courant",
            "tuile_m": SPEC["eau"]["tuile_m"],
            "courant_tuiles_par_s": SPEC["eau"]["courant_tuiles_par_s"],
            "sens": "V croissant = vers l'aval",
            "amont_xyz": [round(relief.riviere[0][0], 2), round(relief.riviere[0][1], 2),
                          round(relief._niveau_eau[0], 2)],
            "aval_xyz": [round(relief.riviere[-1][0], 2), round(relief.riviere[-1][1], 2),
                         round(relief._niveau_eau[-1], 2)],
            "denivele_m": round(relief._niveau_eau[0] - relief._niveau_eau[-1], 2),
        },
        "lampes": lampes_manifeste,
        "porte_village": porte_manifeste,
        "faune": faune_manifeste,
        "campements": camps_manifeste,
        # LES SOLIDES, PAR FAMILLE — `[x, y, z_base, hauteur, rayon]`.
        #
        # Remplace `colliders_cylindre_xyzr`, qui ne portait que les troncs des
        # futaies : 943 cylindres pour une carte qui compte aussi 207 arbres
        # isoles, 110 rochers, 260 eboulis, 22 rochers de bouchage, 15 abris et
        # 16 braseros — tous traversables. Le champ s'appelait pourtant « troncs,
        # rochers et murs » cote moteur.
        #
        # La famille n'est pas decorative : elle dit au moteur ce que la piece
        # fait au jeu (un `abri` doit casser une vue, un `arbre` se contourne),
        # et elle rend le manque LISIBLE — une famille vide se voit.
        #
        # La hauteur est publiee parce qu'elle etait devinee : `plugin.rs` posait
        # 6,0 m en dur pour tout le monde, y compris pour un eboulis de 80 cm.
        "colliders_prop_xyzhr": fusionner_solides(nature, kit_roches, kit_lampe),
        # Ce que la carte a RATE. `[]` est une preuve, l'absence de cle n'en
        # est pas une (`map-design-patterns.md` §13).
        "defauts": DEFAUTS,
        "mesures": rapport,
    }
    # --- diagnostic riviere, station par station -------------------------
    # « Elle s'enterre » ne se corrige pas ; « station 47, tirant -2,41,
    # poids_lit 0,38, chemin a 6 m » se corrige.
    stations = []
    for i, (rx, ry) in enumerate(relief.riviere):
        d_axe = distance_polyligne(rx, ry, relief.riviere)
        h = relief.hauteur(rx, ry)
        niv = relief._niveau_eau[i]
        stations.append({
            "i": i,
            "xy": [round(rx, 1), round(ry, 1)],
            "niveau_eau": round(niv, 2),
            "terrain": round(h, 2),
            "tirant": round(niv - h, 2),
            "base_creuse": round(relief._base(rx, ry), 2),
            "base_nue": round(relief._base(rx, ry, creuser=False), 2),
            "poids_lit": round(relief.poids_lit_en(d_axe), 3),
            "d_chemin": round(distance_polyligne(rx, ry, relief.chemin), 1),
        })
    enterrees = [st for st in stations if st["tirant"] < 0.0]
    diag = {
        "stations": len(stations),
        "enterrees": len(enterrees),
        "tirant_min": min(st["tirant"] for st in stations),
        "tirant_median": sorted(st["tirant"] for st in stations)[len(stations) // 2],
        "pires": sorted(enterrees, key=lambda st: st["tirant"])[:12],
        "toutes": stations,
    }
    with open(os.path.join(SORTIE, "vallon_riviere_diag.json"), "w", encoding="utf-8") as fh:
        json.dump(diag, fh, ensure_ascii=False, indent=1)
    rapport["riviere_enterrees"] = f"{len(enterrees)}/{len(stations)}"
    rapport["riviere_tirant_min"] = diag["tirant_min"]

    with open(os.path.join(SORTIE, "expedition_vallon.json"), "w", encoding="utf-8") as fh:
        json.dump(manifeste, fh, ensure_ascii=False, indent=1)
    # -- retouches d'auteur, et l'instantané qui les rend possibles ---------
    rapport["retouches"] = appliquer_retouches()
    ecrire_reference()

    solides = manifeste["colliders_prop_xyzhr"]
    rapport["solides_par_famille"] = {f: len(v) for f, v in solides.items()}
    rapport["solides_total"] = sum(len(v) for v in solides.values())
    # L'ETENDUE DES RAYONS MESURES, par famille. C'est le controle de la
    # nouvelle derivation : un rayon d'arbre qui monterait a 2-3 m voudrait dire
    # qu'on mesure le houppier et non le tronc, et la foret serait
    # infranchissable. Le voir ici coute une ligne ; le decouvrir en jeu coute
    # une session.
    rapport["rayons_min_max"] = {
        f: [round(min(p[4] for p in v), 2), round(max(p[4] for p in v), 2)]
        for f, v in solides.items() if v
    }
    rapport["hauteurs_min_max"] = {
        f: [round(min(p[3] for p in v), 2), round(max(p[3] for p in v), 2)]
        for f, v in solides.items() if v
    }
    rapport["defauts"] = DEFAUTS
    rapport["campements"] = [c["centre_xyz"] for c in camps_manifeste]
    rapport["rayon_campement_derive"] = SPEC["campements"]["rayon"]
    rapport["visions_lues"] = VISIONS
    print("RESULT: " + json.dumps(rapport, ensure_ascii=False))


main()
