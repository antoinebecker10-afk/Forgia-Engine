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

# ---------------------------------------------------------------------------
# SPEC — couche definition
# ---------------------------------------------------------------------------

SPEC = {
    # Métriques joueur LUES dans le code (player_movement.toml, arena_test.rs).
    "joueur": {"rayon_m": 0.3, "saut_m": 1.174, "marche_ms": 6.5, "sprint_ms": 9.75},

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
    "rim_ondulation": 0.055,
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
    # Au-delà de cette pente, le terrain n'est plus de l'herbe mais de la roche.
    # 36° : au-dessus des 34° où l'on cesse de planter, sous les 50° montables.
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
    # Ce sont des SALLES DE COMBAT, pas du decor. Leurs dimensions se derivent
    # des archetypes (assets/genomes/enemies/) :
    #   grunt  30 pv, 9,0 m/s, vision 20 m, melee 3 m
    #   archer 45 pv, tir 15 m, vision 35 m
    # -> ligne max <= vision du grunt (20 m), sinon on lui tire dessus sans
    #    qu'il puisse voir ni repondre (map-design-intention.md §2.2).
    # -> rayon 12 m : les apparitions a 6-10 m ferment la distance en ~2 s a
    #    9 m/s, donc l'essaim ARRIVE au lieu de mourir en chemin (§2.1).
    "campements": {
        "fractions": [0.22, 0.52, 0.74],   # position le long du chemin
        "rayon": 12.0,
        "apparitions": 7,
        "apparition_rayon": [6.0, 10.0],
        "abris": 6,                        # blocs >= 1,8 m : ils cassent la vue
        "abri_rayon": [5.0, 11.0],
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
            "berge":     {"pente": 16.0, "chemin": [10.0, 999.0], "riviere": [11.0, 24.0],
                          "village": [45.0, 999.0], "rayon": 16.0},
            "sous_bois": {"pente": 20.0, "chemin": [18.0, 70.0], "riviere": [20.0, 999.0],
                          "village": [55.0, 999.0], "rayon": 18.0},
            "abords":    {"pente": 14.0, "chemin": [8.0, 999.0], "riviere": [20.0, 999.0],
                          "village": [36.0, 52.0], "rayon": 15.0},
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

PALETTE_SOURCES = {
    "grass": "ground_grass.glb",
    "dirt": "ground_pathStraight.glb",
    "water": "ground_riverStraight.glb",
    "stone": "cliff_block_stone.glb",
    "wood": "bridge_wood.glb",
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

VILLAGE_BATIS = [
    ("buildings/red/building_well_red.gltf", (0.0, 0.0), 0.0),
    ("buildings/red/building_church_red.gltf", (13.0, 17.0), -20.0),
    ("buildings/red/building_tavern_red.gltf", (-14.0, 12.0), 25.0),
    ("buildings/red/building_market_red.gltf", (3.0, -14.0), 8.0),
    ("buildings/red/building_home_A_red.gltf", (-17.0, -10.0), 45.0),
    ("buildings/red/building_home_B_red.gltf", (17.0, -6.0), -35.0),
    ("buildings/red/building_home_A_red.gltf", (23.0, 7.0), -60.0),
    ("buildings/red/building_home_B_red.gltf", (-8.0, 23.0), 15.0),
    ("buildings/red/building_home_A_red.gltf", (9.0, 25.0), -10.0),
    ("buildings/red/building_home_B_red.gltf", (-22.0, 2.0), 80.0),
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
        t = max(abs(x) / self.s["demi_x"], abs(y) / self.s["demi_y"])
        bouchon = 1.0 - lissage_doux(t, g["bouchon"], 1.0)
        return ouvert * bouchon

    def _cuvette(self, x, y):
        """La paroi du vallon. Son PIED ondule : une falaise dont la base est
        un rectangle parfait se lit comme le bord d'une maquette."""
        t = max(abs(x) / self.s["demi_x"], abs(y) / self.s["demi_y"])
        ang = math.atan2(y, x)
        sinuosite = mathutils.noise.noise(
            Vector((math.cos(ang) * 2.4, math.sin(ang) * 2.4, self.zb + 3.0))
        ) * self.s["rim_ondulation"]
        debut = self.s["rim_debut"] + sinuosite
        # Puissance 1,6 : le pied reste doux, le haut se redresse — un profil
        # linéaire donne un talus régulier, pas une paroi.
        montee = (lissage_doux(t, debut, 1.0) ** 1.6) * self.s["rim_hauteur"]
        return montee * (1.0 - self._gorge(x, y) * self.s["gorge"]["force"])

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
                trou=None, niveau=None, uv_flux=False, demi_variable=None):
    verts, faces = [], []
    longueur = 0.0
    abscisses = []
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
        for signe in (-1.0, 1.0):
            # Le RIVAGE est la ou le plan d'eau coupe le sol, pas a une largeur
            # fixe. Un ruban de largeur constante laisse du lit a sec d'un cote
            # et deborde de l'autre : la ligne d'eau devient une droite
            # geometrique, ce qu'aucune riviere ne fait.
            large = (demi_l if demi_variable is None
                     else demi_variable(x, y, -dy / n * signe, dx / n * signe))
            ox, oy = -dy / n * large, dx / n * large
            px, py = x + ox * signe, y + oy * signe
            if niveau is not None:
                # Meme altitude pour les DEUX rives : une nappe ne se vrille pas.
                z = niveau(x, y) + dz
            else:
                z = (relief.profil_en(x, y)[0] if suivre else relief.hauteur(px, py)) + dz
            verts.append((px, py, z))
    for i in range(len(points) - 1):
        if trou is not None:
            (tx, ty), rayon = trou
            # Une brèche enjambée par un pont ne doit pas AUSSI être couverte
            # par le ruban : celui-ci suit le profil de berge, donc il traverse
            # le vide en bande suspendue et double le tablier.
            if (math.hypot(points[i][0] - tx, points[i][1] - ty) < rayon
                    and math.hypot(points[i + 1][0] - tx, points[i + 1][1] - ty) < rayon):
                continue
        a = i * 2
        faces.append((a, a + 1, a + 3, a + 2))
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
            uv_par_sommet[i * 2] = (0.0, v)
            uv_par_sommet[i * 2 + 1] = (1.0, v)
        mesh = bpy.data.meshes.new(nom)
        mesh.from_pydata(verts, [], faces)
        mesh.update()
        if mat is not None:
            mesh.materials.append(mat)
        couche = mesh.uv_layers.new(name="UVMap")
        for poly in mesh.polygons:
            for li in poly.loop_indices:
                couche.data[li].uv = uv_par_sommet.get(mesh.loops[li].vertex_index, (0.0, 0.0))
        obj = bpy.data.objects.new(nom, mesh)
        coll.objects.link(obj)
        return obj
    return creer_mesh(nom, verts, faces, mat,
                      lambda co: (co.x / uv_m, co.y / uv_m), coll)


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
    def __init__(self, racine, echelle, offset_base, filtre_nom=False):
        self.racine, self.echelle, self.offset_base = racine, echelle, offset_base
        # `filtre_nom` : ne garder que les meshes dont le nom derive du fichier.
        # L'importateur glTF de Blender FABRIQUE une « Icosphere » comme forme
        # d'affichage des os d'un squelette — elle n'est pas dans le GLB (verifie
        # sur le JSON brut : meshes=['deer.001'] seul). Sans ce filtre, une boule
        # de 2 m se pose a cote de chaque bete.
        self.filtre_nom = filtre_nom
        self._proto, self._cache = {}, collection("_proto")
        self.manquants = []
        self.poses = 0

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

    def poser(self, fichier, x, y, z_sol, coll, rot_z=0.0, echelle=None, inclinaison=0.0):
        datas = self.prototype(fichier)
        if not datas:
            return 0
        s = echelle if echelle is not None else self.echelle
        crees = []
        for data in datas:
            # On garde le NOM DU MESH source, pas celui du fichier : c'est lui
            # qui distingue `wall_straight_gate_door_left` du dormant. Sans ca,
            # les battants deviennent indiscernables et rien ne peut les animer.
            obj = bpy.data.objects.new(
                data.name or os.path.splitext(os.path.basename(fichier))[0], data)
            obj.location = (x, y, z_sol + self.offset_base * s)
            obj.rotation_euler = (inclinaison, 0.0, rot_z)
            obj.scale = (s, s, s)
            coll.objects.link(obj)
            crees.append(obj)
        self.poses += 1
        return crees


# ---------------------------------------------------------------------------
# Passe principale
# ---------------------------------------------------------------------------


def main():
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
    batir_ruban("chemin", relief, relief.chemin, SPEC["chemin_demi_largeur"], 0.08,
                mat_chemin, c_sol, suivre=True, uv_m=SPEC["sols"]["chemin"]["uv_m"],
                trou=(relief.pont_xy, SPEC["pont_demi_portee"] - 1.5))
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
        d = mini
        while d < maxi:
            if relief.hauteur(x + nx * d, y + ny * d) >= niv:
                break
            d += 0.4
        # On mord de quelques centimetres dans la berge : sans ce recouvrement,
        # un lisere de sol nu apparait a chaque approximation de maillage.
        return d + 0.35

    batir_ruban("riviere", relief, relief.riviere, SPEC["riviere_demi_largeur"],
                0.0, materiau_eau(), c_eau, uv_m=SPEC["eau"]["tuile_m"],
                niveau=relief.niveau_eau_en, uv_flux=True, demi_variable=rivage)
    if SPEC["pont_pierre"]["actif"]:
        # Pas de `filtre_nom` ici : il n'existe que pour écarter l'icosphère que
        # l'importateur glTF fabrique face à un GLB À SQUELETTE. Le pont n'en a
        # pas, et le filtre rejetait alors le module lui-même (0 pièce posée).
        kit_chateau = Kit(KIT_CHATEAU, SPEC["pont_pierre"]["echelle"], 0.0)
        pont_pieces = batir_pont_pierre(relief, c_sol, kit_chateau)
    else:
        # `batir_pont` rend UN objet (pas une liste) : on compte 1, sinon
        # l'objet Blender partait tel quel dans le rapport JSON.
        batir_pont(relief, palette, c_sol)
        pont_pieces = 1

    nature = Kit(KIT_NATURE, SPEC["echelle_nature"], 0.05)
    village = Kit(KIT_VILLAGE, SPEC["echelle_village"], 0.0)

    # -- bouchage des creux de la ceinture ---------------------------------
    ROCHES_CHATEAU = ["SM_ENV_cliff_castle_01_LOD0.glb", "SM_ENV_cliff_castle_02_LOD0.glb"]
    bch = SPEC["ceinture_bouchage"]
    kit_roches = Kit(KIT_CHATEAU, 1.0, 0.0)
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
            echelle=rng.uniform(*bch["echelle"]))
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
                     inclinaison=rng.uniform(-0.12, 0.12))
        eboulis += 1

    # -- zones interdites --------------------------------------------------
    place, spawn = SPEC["place_village"], SPEC["clairiere_spawn"]

    def libre(x, y, marge):
        if abs(x) > SPEC["demi_x"] - 15.0 or abs(y) > SPEC["demi_y"] - 15.0:
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
        # L'étagement raconte le trajet : conifères au nord et en altitude,
        # feuillus au centre, frange d'automne en approche du village.
        if y > 34.0 or h > 7.0:
            return CONIFERES
        if x > 46.0 and rng.random() < 0.5:
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
    troncs = []          # proxys cylindriques pour le moteur
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
        if relief.pente(x, y) > 34.0:
            continue
        h = relief.hauteur(x, y)
        ech = SPEC["echelle_nature"] * rng.uniform(0.78, 1.35)
        nature.poser(choisir(rng, essence(x, y, h)), x, y, h, c_foret,
                     rot_z=rng.uniform(0.0, math.tau), echelle=ech,
                     inclinaison=rng.uniform(-0.035, 0.035))
        # Un tronc ne se collisionne pas en TriMesh : 1 500 arbres feraient
        # 300 000 triangles de collision pour un cylindre chacun. Le moteur
        # instancie ces proxys — meme doctrine que la dalle deterministe du Hall.
        troncs.append([round(x, 2), round(y, 2), round(h, 2), round(0.055 * ech, 2)])
        arbres += 1

    essais = 0
    while arbres < SPEC["arbres_total"] and essais < SPEC["arbres_total"] * 8:
        essais += 1
        x = rng.uniform(-SPEC["demi_x"], SPEC["demi_x"])
        y = rng.uniform(-SPEC["demi_y"], SPEC["demi_y"])
        if not libre(x, y, SPEC["degagement_chemin"] + 1.5) or relief.pente(x, y) > 34.0:
            continue
        h = relief.hauteur(x, y)
        nature.poser(choisir(rng, essence(x, y, h)), x, y, h, c_foret,
                     rot_z=rng.uniform(0.0, math.tau),
                     echelle=SPEC["echelle_nature"] * rng.uniform(0.8, 1.3),
                     inclinaison=rng.uniform(-0.035, 0.035))
        arbres += 1

    # -- sous-bois : il a le droit de border le chemin, il ne bloque pas ----
    sousbois, essais = 0, 0
    while sousbois < SPEC["sousbois_total"] and essais < SPEC["sousbois_total"] * 6:
        essais += 1
        x = rng.uniform(-SPEC["demi_x"], SPEC["demi_x"])
        y = rng.uniform(-SPEC["demi_y"], SPEC["demi_y"])
        if not libre(x, y, SPEC["chemin_demi_largeur"] + 0.7):
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
    while rochers < SPEC["rochers_total"] and essais < SPEC["rochers_total"] * 8:
        essais += 1
        x = rng.uniform(-SPEC["demi_x"], SPEC["demi_x"])
        y = rng.uniform(-SPEC["demi_y"], SPEC["demi_y"])
        if not libre(x, y, SPEC["degagement_chemin"] + 2.0):
            continue
        nature.poser(choisir(rng, ROCHERS), x, y, relief.hauteur(x, y), c_foret,
                     rot_z=rng.uniform(0.0, math.tau),
                     echelle=SPEC["echelle_nature"] * rng.uniform(0.55, 1.15))
        rochers += 1

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
            lat = (-1.4 + 2.8 * k / max(1, SPEC["pont_culees"] - 1)) * (SPEC["chemin_demi_largeur"] + 0.9)
            longi = signe * (SPEC["pont_demi_portee"] + 1.2)
            cxp = bpx + _ux * longi + _lx * lat
            cyp = bpy_ + _uy * longi + _ly * lat
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
                 echelle=SPEC["echelle_nature"] * 4.2)
    for i in range(7):
        ang = math.tau * i / 7.0
        rx, ry = mx + math.cos(ang) * 11.0, my + math.sin(ang) * 11.0
        nature.poser("statue_column.glb" if i % 2 else "statue_columnDamaged.glb",
                     rx, ry, relief.hauteur(rx, ry), c_reperes,
                     rot_z=ang, echelle=SPEC["echelle_nature"] * 1.25)

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
                     rot_z=rng.uniform(0, math.tau), echelle=SPEC["echelle_nature"] * 1.5)
    rx, ry = ecarter_du_chemin(col_x, col_y + 15.0, SPEC["degagement_chemin"] + 3.0)
    nature.poser("statue_ring.glb", rx, ry, relief.hauteur(rx, ry), c_reperes,
                 echelle=SPEC["echelle_nature"] * 2.0)

    # -- clairière de départ -----------------------------------------------
    sx, sy = spawn["xy"]
    nature.poser("campfire_stones.glb", sx + 3.0, sy - 2.0, relief.hauteur(sx + 3.0, sy - 2.0), c_reperes)
    for nom, dx, dy in (("tent_detailedOpen.glb", -4.5, 3.5), ("tent_smallOpen.glb", -2.0, -5.5),
                        ("tent_smallClosed.glb", 5.5, 3.5)):
        nature.poser(nom, sx + dx, sy + dy, relief.hauteur(sx + dx, sy + dy), c_reperes,
                     rot_z=math.atan2(-dy, -dx))
    for dx, dy in ((1.5, 3.0), (-3.0, -2.0), (4.0, -4.0)):
        nature.poser("stump_round.glb", sx + dx, sy + dy, relief.hauteur(sx + dx, sy + dy), c_reperes)
    nature.poser("sign.glb", sx + 10.0, sy + 1.5, relief.hauteur(sx + 10.0, sy + 1.5), c_reperes,
                 rot_z=math.radians(-70.0))
    nature.poser("log_stack.glb", sx - 7.0, sy + 6.0, relief.hauteur(sx - 7.0, sy + 6.0), c_reperes)

    # -- village -----------------------------------------------------------
    vx, vy = place["xy"]
    batis = 0
    for fichier, (dx, dy), rot in VILLAGE_BATIS:
        # Un batiment sur la route se traverse mal. On le decale lateralement
        # jusqu'a degager le corridor, plutot que de retoucher la table a la
        # main a chaque fois que le trace bouge.
        # Degager la route SANS sortir de l'enceinte : le decalage radial
        # poussait le marche dans un pan de mur (recouvrement mesure : 75 %).
        # Rayon utile = rempart moins la demi-emprise du plus gros batiment
        # (marche : 1,80 x 6 = 10,8 m -> 6 m de marge).
        rayon_utile = (place["rayon"] - 2.0) - 11.0
        for _ in range(14):
            trop_pres = distance_polyligne(vx + dx, vy + dy, relief.chemin) < 7.0
            if not trop_pres:
                break
            norme = math.hypot(dx, dy) or 1.0
            dx += dx / norme * 2.0
            dy += dy / norme * 2.0
        rayon = math.hypot(dx, dy)
        if rayon > rayon_utile and rayon > 1e-6:
            dx *= rayon_utile / rayon
            dy *= rayon_utile / rayon
        batis += len(village.poser(fichier, vx + dx, vy + dy,
                                   relief.hauteur(vx + dx, vy + dy),
                                   c_village, rot_z=math.radians(rot)))
    rayon_mur = place["rayon"] - 2.0
    pas_ang = 12.0 / rayon_mur          # wall_straight = 2,0 × 6 = 12 m
    # L'angle de la porte se CALCULE : c'est celui du point ou le chemin coupe
    # l'anneau du rempart. Le fixer a pi (« plein ouest ») supposait que le
    # chemin arrivait par l'ouest — il arrivait par le sud-est, et la porte se
    # retrouvait sur un talus, 6,9 m sous le sol.
    croisement = min(
        relief.chemin,
        key=lambda pt: abs(math.hypot(pt[0] - vx, pt[1] - vy) - rayon_mur))
    angle_porte = math.atan2(croisement[1] - vy, croisement[0] - vx)
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
            nature.poser(choisir(rng, [("crops_wheatStageB.glb", 3), ("crops_cornStageC.glb", 2),
                                       ("crops_leafsStageB.glb", 2), ("crop_pumpkin.glb", 1)]),
                         ccx, ccy, relief.hauteur(ccx, ccy), c_village,
                         echelle=SPEC["echelle_nature"] * 0.9)
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
    kit_lampe = Kit(os.path.join(RACINE, "assets", "models"), lcfg["echelle"], 0.0)
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
                            rot_z=rng.uniform(0.0, math.tau))
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

        def poser_camp(fichier, longi, lat, **kw):
            """Pose en repere du camp. Refuse tout ce qui empieterait sur la route."""
            if abs(lat) < LIBRE:
                return 0
            px = cx + ux * longi + lx * lat
            py = cy + uy * longi + ly * lat
            return len(nature.poser(fichier, px, py, relief.hauteur(px, py), c_camps, **kw))

        # Barricade : elle ENCADRE la route au lieu de la murer. Les pieces font
        # 1,0 x 4,6 m ; centrees a +/-4,6 elles laissent une trouee de 4,6 m,
        # soit exactement la largeur du chemin. C'est un poste de controle
        # qu'on franchit, pas un mur — le moteur y posera son verrou quand le
        # camp sera actif, et le retirera une fois nettoye.
        bx, by = cx + ux * 8.0, cy + uy * 8.0
        largeur_piece = SPEC["echelle_nature"] * 1.15
        for pas_lat in (-14.6, -10.0, -5.4, 5.4, 10.0, 14.6):
            px, py = bx + lx * pas_lat, by + ly * pas_lat
            nature.poser("fence_planksDouble.glb", px, py, relief.hauteur(px, py), c_camps,
                         rot_z=base, echelle=largeur_piece)

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

        # Abris : seuls des blocs >= 1,8 m cassent la ligne de vue (l'oeil du
        # joueur est a 1,70 m et il n'y a PAS d'accroupissement dans Forgia —
        # map-design-patterns.md §11). Un abri plus bas ne sert donc a rien.
        abris = []
        for k in range(cfg["abris"]):
            ang = base + math.tau * (k + 0.5) / cfg["abris"]
            r = rng.uniform(*cfg["abri_rayon"])
            ax2, ay2 = cx + math.cos(ang) * r, cy + math.sin(ang) * r
            # Un abri sur la route est un obstacle, pas un abri.
            if abs((ax2 - cx) * lx + (ay2 - cy) * ly) < LIBRE:
                continue
            nature.poser(choisir(rng, [("stone_tallB.glb", 2), ("stone_tallF.glb", 2),
                                       ("stone_largeC.glb", 1)]),
                         ax2, ay2, relief.hauteur(ax2, ay2), c_camps,
                         rot_z=rng.uniform(0.0, math.tau),
                         echelle=SPEC["echelle_nature"] * rng.uniform(1.0, 1.4))
            abris.append([round(ax2, 2), round(ay2, 2)])

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
            "abris_xy": abris,
            # Ligne max mesuree = diametre de la clairiere. A comparer a la
            # vision du grunt (20 m) : au-dela, il se fait tirer sans repondre.
            "ligne_max_m": round(cfg["rayon"] * 2.0, 1),
            "grunt_vision_m": 20.0,
        })

    # -- zones de faune ----------------------------------------------------
    c_faune = collection("faune_controle")
    faune_manifeste = []
    zones_posees = []
    for espece, (milieu, nb_zones, effectif, couleur) in SPEC["faune"]["especes"].items():
        crit = SPEC["faune"]["milieux"][milieu]
        poses = 0
        essais = 0
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
            if relief.pente(x, y) > crit["pente"]:
                continue
            if crit.get("sur_crete"):
                cr = SPEC["crete"]
                # Sur le dos de la crete, et a l'ecart du col.
                if abs(x - cr["x"]) > cr["epaisseur"] * 0.75:
                    continue
                if abs(y - cr["col_y"]) < cr["col_largeur"] * 0.9:
                    continue
            d_ch = distance_polyligne(x, y, relief.chemin)
            if not (crit["chemin"][0] <= d_ch <= crit["chemin"][1]):
                continue
            d_ri = distance_polyligne(x, y, relief.riviere)
            if not (crit["riviere"][0] <= d_ri <= crit["riviere"][1]):
                continue
            d_vi = math.hypot(x - place["xy"][0], y - place["xy"][1])
            if not (crit["village"][0] <= d_vi <= crit["village"][1]):
                continue
            if any(math.hypot(x - cp["xy"][0], y - cp["xy"][1])
                   < SPEC["campements"]["rayon"] + crit["rayon"] for cp in camps):
                continue
            ecart = crit.get("ecart", SPEC["faune"]["ecart_min"])
            if any(math.hypot(x - zx, y - zy) < ecart + crit["rayon"]
                   for zx, zy, _ in zones_posees):
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

    # -- apercu de la faune ------------------------------------------------
    # Les betes sont posees pour JUGER l'echelle et la repartition, et pour
    # rien d'autre : en jeu c'est le moteur qui les fera apparaitre et
    # deambuler. Cette collection est donc ecartee de la cuisson, comme les
    # disques de controle — une bete cuite dans la carte serait une statue.
    c_apercu = collection("faune_apercu")
    kit_animaux = Kit(DOSSIER_ANIMAUX, 1.0, 0.0, filtre_nom=True)  # deja en metres
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
        "colliders_cylindre_xyzr": troncs,
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
    rapport["proxys_troncs"] = len(troncs)
    rapport["campements"] = [c["centre_xyz"] for c in camps_manifeste]
    print("RESULT: " + json.dumps(rapport, ensure_ascii=False))


main()
