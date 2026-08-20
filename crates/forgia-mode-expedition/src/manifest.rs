//! manifest.rs — le manifeste de gameplay d'une carte d'expédition.
//!
//! # Le repère, et pourquoi il occupe la moitié de ce fichier
//!
//! La carte est autorée sous Blender, qui travaille en **Z vers le haut**. Bevy
//! travaille en **Y vers le haut**. Les deux fichiers exportés ne disent pas la
//! même chose :
//!
//! | fichier | repère | déjà converti ? |
//! |---|---|---|
//! | `vallon_stream_cells.toml` (les cellules) | `bevy_y_up_meters` | ✅ oui |
//! | `expedition_vallon.json` (le gameplay) | `blender_z_up` | ❌ **non** |
//!
//! Le décor arrive donc déjà tourné, et **tout ce que ce fichier lit ne l'est
//! pas**. Un spawn, 943 colliders, 3 campements et 91 points de chemin qui
//! atterriraient dans le mauvais plan — et le symptôme serait « le joueur tombe
//! à l'infini » ou « les arbres n'ont pas de collision », deux diagnostics qui
//! n'évoquent pas un repère.
//!
//! La conversion est donc **une fonction nommée, avec ses tests**, jamais un
//! `Vec3::new(v[0], v[2], -v[1])` recopié à chaque site. C'est exactement la
//! classe de défaut n°1 du projet : une grandeur — ici une convention — écrite
//! plusieurs fois finit toujours par diverger sur un site.

use bevy::prelude::*;
use serde::Deserialize;

/// Bascule Blender (Z haut) → Bevy (Y haut). **La seule conversion du projet.**
///
/// `Bevy(x, y, z) = Blender(x, z, −y)` : on prend l'altitude de Blender comme
/// hauteur, et l'axe Y de Blender devient le Z de Bevy **avec un changement de
/// signe** — sans lui la carte serait en miroir, ce qui se voit… quand on
/// connaît déjà la carte.
#[must_use]
pub fn blender_to_bevy(v: [f32; 3]) -> Vec3 {
    Vec3::new(v[0], v[2], -v[1])
}

/// Idem pour un couple horizontal (plan XY de Blender → plan XZ de Bevy).
#[must_use]
pub fn blender_xy_to_bevy_xz(v: [f32; 2]) -> Vec2 {
    Vec2::new(v[0], -v[1])
}

/// Où le joueur commence, et vers quoi il regarde.
#[derive(Debug, Clone, Deserialize)]
pub struct SpawnDef {
    pub xyz: [f32; 3],
    /// Point VISÉ au départ, en plan. C'est un point, pas un angle : un cap
    /// déclaré diverge de la géométrie dès que le chemin bouge — la carte a déjà
    /// payé ça deux fois (le pont en biais, la porte enterrée).
    pub regard_xy: [f32; 2],
    pub dalle_demi: [f32; 3],
}

/// Un campement — et il porte **sa spec de combat**, ce qu'aucune arène n'a
/// jamais eu (`map-design-intention.md` §1).
#[derive(Debug, Clone, Deserialize)]
pub struct CampementDef {
    pub id: String,
    pub centre_xyz: [f32; 3],
    pub rayon_m: f32,
    /// Le verrou : franchir le campement ouvre la suite.
    pub verrou_xyz: [f32; 3],
    pub verrou_cap_rad: f32,
    /// Où les ennemis apparaissent. Déclaré par la carte, pas dérivé au runtime.
    pub apparitions_xyz: Vec<[f32; 3]>,
    /// Les abris — **des meubles de combat**, pas du décor.
    pub abris: Vec<AbriDef>,
    /// Quels archétypes peuplent ce campement. C'est d'eux que le rayon dérive.
    #[serde(default)]
    pub archetypes: Vec<String>,
    /// Plus longue ligne de vue mesurée dans le campement (m).
    pub ligne_max_m: f32,
    /// Portée de vision du grunt (m) — pour vérifier qu'elle couvre `ligne_max_m`.
    pub grunt_vision_m: f32,
    /// La plus petite vision parmi les archétypes présents. C'est ELLE qui borne
    /// `ligne_max_m` ; `grunt_vision_m` n'en est qu'un cas particulier.
    #[serde(default)]
    pub vision_min_m: Option<f32>,
    // ── La spec de combat (`map-design-intention.md` §1) ────────────────
    //
    // Elle manquait entièrement : le manifeste portait des points d'apparition
    // sans jamais dire QUI apparaît, en quelle quantité, contre quel arsenal,
    // ni quand la salle est finie. `verrou_xyz` était une position sans règle.
    //
    // Tout est `Option` : un manifeste cuit avant cet ajout reste lisible.
    /// Combien d'ennemis, par archétype.
    #[serde(default)]
    pub effectifs: Option<std::collections::BTreeMap<String, u32>>,
    /// Dégâts par seconde de l'arsenal attendu à ce moment de la course.
    #[serde(default)]
    pub arsenal_dps: Option<f32>,
    #[serde(default)]
    pub condition_sortie: Option<String>,
    /// Durée **dérivée** : total des pv sur le dps de l'arsenal.
    #[serde(default)]
    pub duree_tir_s: Option<f32>,
    /// Durée **dérivée** pour que le plus lent atteigne le joueur.
    #[serde(default)]
    pub duree_approche_s: Option<f32>,
    /// L'essaim arrive-t-il, ou meurt-il en chemin (`§2.1`) ?
    #[serde(default)]
    pub essaim_arrive: Option<bool>,
    /// Plancher **dérivé** : le temps de traverser la salle une fois.
    #[serde(default)]
    pub duree_plancher_s: Option<f32>,
}

/// Un abri de campement.
///
/// # Pourquoi il porte sa hauteur
///
/// Le manifeste ne donnait qu'un couple `[x, y]` : impossible de vérifier qu'un
/// abri abrite sans rouvrir Blender. Or un bloc n'est une couverture que s'il
/// dépasse l'œil du joueur (1,70 m) — il n'y a pas d'accroupissement dans
/// Forgia, donc sous cette hauteur il masque le corps sans masquer la vue
/// (`map-design-patterns.md` §11).
///
/// `casse_la_vue` est **dérivé** de `hauteur_m` côté Blender, jamais déclaré :
/// un bloc trop bas sort `false` et se voit, au lieu de se déguiser en abri.
#[derive(Debug, Clone, Deserialize)]
pub struct AbriDef {
    pub xyz: [f32; 3],
    pub rayon_m: f32,
    pub hauteur_m: f32,
    pub casse_la_vue: bool,
}

impl CampementDef {
    /// Un grunt voit-il d'un bout à l'autre de son campement ?
    ///
    /// Si non, il se fait tirer **sans pouvoir répondre ni réagir** — c'est
    /// `map-design-intention.md` §2.2, et le manifeste porte les deux nombres
    /// exprès pour que ça se vérifie au chargement au lieu de se découvrir en jeu.
    #[must_use]
    pub fn vision_couvre_la_ligne(&self) -> bool {
        self.grunt_vision_m >= self.ligne_max_m
    }
}

/// Un groupe d'animaux.
#[derive(Debug, Clone, Deserialize)]
pub struct FauneDef {
    pub espece: String,
    pub milieu: String,
    pub centre_xyz: [f32; 3],
    pub rayon_m: f32,
    pub effectif: u32,
}

/// La rivière : ce que le moteur doit en savoir pour la faire couler.
#[derive(Debug, Clone, Deserialize)]
pub struct EauDef {
    pub objet_glb: String,
    pub tuile_m: f32,
    pub courant_tuiles_par_s: f32,
    pub amont_xyz: [f32; 3],
    pub aval_xyz: [f32; 3],
    pub denivele_m: f32,
}

/// La porte du village, à deux battants.
#[derive(Debug, Clone, Deserialize)]
pub struct PorteDef {
    pub centre_xyz: [f32; 3],
    pub rayon_declenchement_m: f32,
    pub duree_ouverture_s: f32,
}

/// Le manifeste complet, tel qu'exporté par `tools/blender/expedition/`.
///
/// Les champs absents ici (`mesures`, diagnostics) sont ignorés volontairement :
/// ce sont des traces de fabrication, pas des données de jeu.
/// Un brasier jalonnant le chemin.
///
/// `avancee` va de 0 au départ à 1 au village : c'est elle qu'on compare à la
/// progression du joueur pour allumer de proche en proche quand la nuit tombe.
/// Sans elle il faudrait recalculer une abscisse curviligne à l'exécution, et
/// elle divergerait du tracé dès que celui-ci bougerait.
#[derive(Debug, Clone, Deserialize)]
pub struct LampeDef {
    pub xyz: [f32; 3],
    /// Où poser la lumière : la flamme est à 2,7 m au-dessus du pied. Une
    /// lumière à l'origine du brasier éclairerait le sol, pas le chemin.
    pub flamme_xyz: [f32; 3],
    pub avancee: f32,
    pub portee_m: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExpeditionManifest {
    pub carte: String,
    pub emprise_m: [f32; 2],
    /// Doit valoir `blender_z_up` — sinon la conversion ci-dessous serait
    /// appliquée à des coordonnées déjà tournées, et la carte partirait de côté.
    pub repere: String,
    pub spawn: SpawnDef,
    pub village_xyz: [f32; 3],
    pub chemin_xyz: Vec<[f32; 3]>,
    pub eau: EauDef,
    pub porte_village: PorteDef,
    pub faune: Vec<FauneDef>,
    pub campements: Vec<CampementDef>,
    /// Les solides, **par famille** : `[x, y, z_base, hauteur, rayon]` en repère
    /// Blender.
    ///
    /// # Ce que ce champ remplace, et pourquoi
    ///
    /// Il s'appelait `colliders_cylindre_xyzr` et son commentaire annonçait
    /// « les troncs, rochers et murs ». Mesuré le 2026-08-17 : il ne contenait
    /// que **des troncs, et seulement 82 % d'entre eux** — le site qui publiait
    /// les proxys vivait à l'intérieur d'une des deux boucles de plantation.
    /// Traversaient donc le décor : 207 arbres isolés, 110 rochers, 260 éboulis,
    /// les 22 rochers qui bouchent la ceinture, les 16 braseros, et — le pire —
    /// **les 15 abris des trois salles de combat**, qui n'arrêtaient donc ni le
    /// joueur ni un tir.
    ///
    /// La famille n'est pas décorative : elle dit ce que la pièce fait au jeu,
    /// et elle rend le manque lisible — une famille vide se voit, un total qui
    /// baisse ne se voit pas.
    ///
    /// La hauteur est publiée parce qu'elle était **devinée** : le plugin posait
    /// 6,0 m pour tout le monde, y compris pour un éboulis de 80 cm.
    pub colliders_prop_xyzhr: std::collections::BTreeMap<String, Vec<[f32; 5]>>,
    /// Ce que la carte a **raté** à la cuisson. Vide = preuve, pas silence.
    ///
    /// Deux systèmes de placement manquaient une part de leur cible sans qu'aucune
    /// sortie ne le dise : 8 zones de faune sur 11 et 15 abris sur 18. Le
    /// manifeste rapportait l'obtenu sans la demande, ce qui se lit comme un
    /// succès (`map-design-patterns.md` §13).
    #[serde(default)]
    pub defauts: Vec<serde_json::Value>,
    /// Les brasiers du chemin. `default` : un manifeste cuit avant leur ajout
    /// reste lisible, il n'aura simplement pas d'éclairage.
    #[serde(default)]
    pub lampes: Vec<LampeDef>,
}

/// Ce qui peut clocher, **nommé** — un `Option` compilerait mais ne guiderait pas.
#[derive(Debug)]
pub enum ExpeditionManifestError {
    Json(String),
    /// Le repère n'est pas celui qu'on sait convertir. **Refuser plutôt que
    /// deviner** : appliquer la conversion à des coordonnées déjà tournées
    /// donnerait une carte cohérente mais fausse, le pire des cas.
    RepereInconnu(String),
    /// Une carte sans chemin ni campement n'est pas une expédition.
    Vide,
}

impl std::fmt::Display for ExpeditionManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(e) => write!(f, "JSON illisible : {e}"),
            Self::RepereInconnu(r) => write!(
                f,
                "repere « {r} » inconnu — attendu « blender_z_up ». Convertir des \
                 coordonnees deja tournees donnerait une carte coherente mais fausse"
            ),
            Self::Vide => write!(f, "ni chemin ni campement : ce n'est pas une expedition"),
        }
    }
}

/// Le seul repère que ce lecteur sait convertir.
pub const REPERE_ATTENDU: &str = "blender_z_up";

impl ExpeditionManifest {
    /// Lit un manifeste et **refuse** ce qu'il ne sait pas interpréter.
    pub fn parse(contenu: &str) -> Result<Self, ExpeditionManifestError> {
        let m: Self = serde_json::from_str(contenu)
            .map_err(|e| ExpeditionManifestError::Json(e.to_string()))?;
        if m.repere != REPERE_ATTENDU {
            return Err(ExpeditionManifestError::RepereInconnu(m.repere));
        }
        if m.chemin_xyz.is_empty() || m.campements.is_empty() {
            return Err(ExpeditionManifestError::Vide);
        }
        Ok(m)
    }

    /// Position de départ **au SOL**, en repère Bevy — l'altitude de la dalle,
    /// pas celle du joueur. Voir [`Self::spawn_player_origin`].
    #[must_use]
    pub fn spawn_bevy(&self) -> Vec3 {
        blender_to_bevy(self.spawn.xyz)
    }

    /// Où poser l'ORIGINE du `Transform` du joueur pour que ses pieds reposent
    /// sur la dalle de départ.
    ///
    /// # Le défaut que cette fonction existe pour empêcher
    ///
    /// Mesuré en jeu le 2026-08-14 : le joueur posé directement à `spawn_bevy()`
    /// s'est retrouvé **un mètre sous le terrain**, parce que l'origine de son
    /// `Transform` est le CENTRE de sa capsule, pas ses pieds. Symptôme relevé —
    /// `grounded: true`, **20 contacts KCC**, et impossible de bouger. Ni le
    /// « grounded » ni le nombre de contacts n'évoquent une erreur d'altitude :
    /// on cherche un blocage de contrôleur, et on cherche longtemps.
    ///
    /// Deux hauteurs s'additionnent, et **aucune des deux ne se devine** :
    ///
    /// | terme | source | valeur |
    /// |---|---|---|
    /// | dessus de la dalle | `spawn.dalle_demi[2]`, le manifeste | 0,25 m |
    /// | pieds → centre de capsule | `PLAYER_FOOT_OFFSET_M`, `forgia-player` | 1,00 m |
    ///
    /// La seconde était un littéral enfoui dans `spawn_player` ; elle est
    /// désormais publique, pour que personne n'ait à la recopier.
    #[must_use]
    pub fn spawn_player_origin(&self) -> Vec3 {
        let sol = self.spawn_bevy();
        // `dalle_demi` est en repère Blender : son 3e terme est la demi-épaisseur
        // verticale, qui devient bien une hauteur en Bevy.
        let dessus_dalle = self.spawn.dalle_demi[2];
        sol + Vec3::Y * (dessus_dalle + forgia_player::PLAYER_FOOT_OFFSET_M)
    }

    /// Vers où il regarde au départ, **en repère Bevy**.
    #[must_use]
    pub fn regard_bevy(&self) -> Vec2 {
        blender_xy_to_bevy_xz(self.spawn.regard_xy)
    }

    /// Le chemin, converti. 91 points sur le Vallon.
    pub fn chemin_bevy(&self) -> impl Iterator<Item = Vec3> + '_ {
        self.chemin_xyz.iter().copied().map(blender_to_bevy)
    }

    /// Longueur du chemin (m) — **mesurée sur les points**, jamais recopiée du
    /// champ `mesures` du manifeste. Deux sources pour une même grandeur
    /// divergent toujours ; celle-ci se recalcule.
    #[must_use]
    pub fn longueur_chemin_m(&self) -> f32 {
        self.chemin_bevy()
            .collect::<Vec<_>>()
            .windows(2)
            .map(|p| p[0].distance(p[1]))
            .sum()
    }

    /// Les cylindres de collision, convertis : `(famille, base_bevy, hauteur, rayon)`.
    ///
    /// Le `y` rendu est celui de la **base** du cylindre — Rapier veut son
    /// centre, c'est à l'appelant de remonter d'une demi-hauteur. On ne le fait
    /// pas ici parce que ce module ne connaît pas la convention du moteur
    /// physique, et qu'une conversion faite au mauvais endroit est exactement ce
    /// qui a enterré le joueur d'un mètre le 2026-08-14.
    pub fn colliders_bevy(&self) -> impl Iterator<Item = (&str, Vec3, f32, f32)> + '_ {
        self.colliders_prop_xyzhr.iter().flat_map(|(famille, pieces)| {
            pieces.iter().map(move |c| {
                (
                    famille.as_str(),
                    blender_to_bevy([c[0], c[1], c[2]]),
                    c[3],
                    c[4],
                )
            })
        })
    }

    /// Combien de solides, toutes familles confondues.
    #[must_use]
    pub fn nb_colliders(&self) -> usize {
        self.colliders_prop_xyzhr.values().map(Vec::len).sum()
    }

    /// Les abris qui ne cassent pas la vue, campement par campement.
    ///
    /// Un abri sous l'œil du joueur masque le corps sans masquer la vue : il se
    /// lit comme une couverture et n'en est pas une. C'est un défaut de CARTE —
    /// il se signale, il ne se corrige pas ici.
    pub fn abris_qui_n_abritent_pas(&self) -> impl Iterator<Item = (&str, &AbriDef)> + '_ {
        self.campements.iter().flat_map(|c| {
            c.abris
                .iter()
                .filter(|a| !a.casse_la_vue)
                .map(move |a| (c.id.as_str(), a))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALLON: &str =
        include_str!("../../../assets/models/environment/expedition/expedition_vallon.json");

    fn vallon() -> ExpeditionManifest {
        ExpeditionManifest::parse(VALLON).expect("le manifeste du Vallon doit se lire")
    }

    // ── La conversion de repère ─────────────────────────────────────────

    #[test]
    fn la_conversion_prend_l_altitude_de_blender_comme_hauteur() {
        // Blender Z = altitude. Si on prenait Y, toute la carte serait couchee.
        assert_eq!(blender_to_bevy([1.0, 2.0, 3.0]).y, 3.0);
    }

    #[test]
    fn la_conversion_inverse_le_signe_du_second_axe() {
        // LE detail qui rend une carte MIROIR si on l'oublie — et un miroir se
        // voit seulement quand on connait deja la carte, donc trop tard.
        assert_eq!(blender_to_bevy([1.0, 2.0, 3.0]).z, -2.0);
        assert_eq!(blender_xy_to_bevy_xz([1.0, 2.0]).y, -2.0);
    }

    #[test]
    fn la_conversion_preserve_les_distances() {
        // Une rotation-reflexion conserve les longueurs. Si ce test tombe, la
        // conversion deforme la carte au lieu de la tourner — et toutes les
        // mesures derivees (358,7 m de chemin, 55 s de marche) seraient fausses.
        let a: [f32; 3] = [12.0, -30.0, 4.0];
        let b: [f32; 3] = [-45.0, 18.0, -2.0];
        let d_blender = (a[0] - b[0]).hypot(a[1] - b[1]).hypot(a[2] - b[2]);
        let d_bevy = blender_to_bevy(a).distance(blender_to_bevy(b));
        assert!((d_blender - d_bevy).abs() < 1.0e-4, "{d_blender} vs {d_bevy}");
    }

    #[test]
    fn les_deux_conversions_sont_d_accord_sur_le_plan() {
        // `blender_to_bevy` et `blender_xy_to_bevy_xz` decrivent la MEME
        // convention. Les laisser diverger produirait un chemin juste et des
        // abris en miroir, sur la meme carte.
        let p3 = blender_to_bevy([7.0, -13.0, 99.0]);
        let p2 = blender_xy_to_bevy_xz([7.0, -13.0]);
        assert_eq!((p3.x, p3.z), (p2.x, p2.y));
    }

    // ── LE MANIFESTE RÉEL ───────────────────────────────────────────────

    #[test]
    fn le_manifeste_du_vallon_se_lit() {
        let m = vallon();
        assert_eq!(m.carte, "expedition_vallon");
        assert_eq!(m.emprise_m, [280.0, 200.0]);
        assert_eq!(m.chemin_xyz.len(), 91);
        assert_eq!(m.campements.len(), 3);
        // 1 616 solides le 2026-08-17, contre 943 avant : les 207 arbres isoles,
        // les rochers, les eboulis, les bouchons de ceinture, les braseros et
        // les abris ont cesse de se traverser.
        assert!(
            m.nb_colliders() > 1_500,
            "{} solides : le contrat de collision a regresse",
            m.nb_colliders()
        );
        // Et les familles attendues sont TOUTES peuplees. Un total qui tient
        // pendant qu'une famille se vide ne se verrait pas.
        for famille in ["abri", "arbre", "bouchon", "brasero", "eboulis", "repere", "rocher"] {
            let n = m.colliders_prop_xyzhr.get(famille).map_or(0, Vec::len);
            assert!(n > 0, "famille « {famille} » vide : plus rien ne s'y collisionne");
        }
    }

    #[test]
    fn la_carte_ne_rate_aucune_production() {
        // `defauts` porte deux choses tres differentes, et les confondre ferait
        // taire la plus grave :
        //
        //   - un defaut de PRODUCTION : la cuisson n'a pas pose ce qu'elle
        //     declarait (zones de faune introuvables, abris jetes par la
        //     contrainte de couloir, rochers refuses par la pente). Ceux-la
        //     doivent etre a ZERO — avant ce champ, 3 zones et 3 abris
        //     manquaient sans que rien ne le dise ;
        //   - un defaut de CONCEPTION : la carte a pose exactement ce qu'elle
        //     voulait, et ce qu'elle voulait ne tient pas son propre contrat.
        //     Voir le test suivant.
        let d = vallon().defauts;
        let production: Vec<_> = d
            .iter()
            .filter(|v| {
                v.get("quoi")
                    .and_then(|q| q.as_str())
                    .is_some_and(|q| !q.contains("duree_engagement"))
            })
            .collect();
        assert!(
            production.is_empty(),
            "{} defaut(s) de production : {production:?}",
            production.len()
        );
    }

    #[test]
    fn les_campements_se_jouent_en_moins_de_temps_qu_il_n_en_faut_pour_les_traverser() {
        // # Un constat grave, pas un commentaire
        //
        // La spec de combat (§1) a ete ajoutee le 2026-08-17, et son premier
        // effet a ete de rendre visible ceci : les trois campements durent
        // **1,4 / 1,9 / 2,3 s** pour un plancher derive de 3,1 s (le diametre
        // de la salle divise par la vitesse de marche). Le combat est fini
        // avant qu'on ait pu se repositionner — les six abris de chaque camp
        // n'ont donc servi a rien.
        //
        // Ce n'est pas un bug de cuisson : la carte pose exactement ce qu'elle
        // declare. C'est la DECLARATION qui est trop maigre, et le remede
        // (des vagues) demande un consommateur moteur qui n'existe pas encore.
        //
        // Le jour ou les vagues arrivent, ce test tombe et rappelle de le
        // mettre a jour. Un constat qui ne casse rien quand il devient faux
        // n'est pas un constat.
        let d = vallon().defauts;
        let courts = d
            .iter()
            .filter(|v| {
                v.get("quoi")
                    .and_then(|q| q.as_str())
                    .is_some_and(|q| q.contains("duree_engagement"))
            })
            .count();
        assert_eq!(
            courts,
            3,
            "etat mesure le 2026-08-17 : les 3 campements sous leur plancher de duree"
        );
        // Et la spec elle-meme porte les nombres, pour qu'on n'ait pas a
        // rouvrir Blender pour savoir de combien on est loin du compte.
        for c in &vallon().campements {
            assert!(c.duree_tir_s.is_some(), "{} sans duree derivee", c.id);
            assert!(c.effectifs.is_some(), "{} sans effectifs declares", c.id);
        }
    }

    #[test]
    fn un_repere_inconnu_se_refuse_au_lieu_de_se_deviner() {
        // Appliquer la conversion a des coordonnees DEJA tournees donnerait une
        // carte coherente mais fausse — le pire des cas, parce qu'elle se joue
        // sans erreur.
        let deja_tourne = VALLON.replace("\"blender_z_up\"", "\"bevy_y_up_meters\"");
        match ExpeditionManifest::parse(&deja_tourne) {
            Err(ExpeditionManifestError::RepereInconnu(r)) => assert_eq!(r, "bevy_y_up_meters"),
            autre => panic!("attendu un refus de repere, obtenu {autre:?}"),
        }
    }

    #[test]
    fn le_spawn_tombe_dans_l_emprise_de_la_carte() {
        // Un spawn hors carte, c'est le joueur qui tombe a l'infini — un
        // symptome qui n'evoque PAS un probleme de repere. Ce test attrape
        // l'inversion d'axes la ou elle fait le plus mal.
        let m = vallon();
        let s = m.spawn_bevy();
        let (demi_x, demi_z) = (m.emprise_m[0] * 0.5, m.emprise_m[1] * 0.5);
        assert!(
            s.x.abs() <= demi_x && s.z.abs() <= demi_z,
            "spawn ({:.1}, {:.1}) hors de l'emprise {demi_x} x {demi_z}",
            s.x,
            s.z
        );
        // Et il est a une altitude plausible (terrain mesure : -5,84 a 14,98 m).
        assert!(
            (-10.0..=20.0).contains(&s.y),
            "altitude de spawn {:.2} m invraisemblable",
            s.y
        );
    }

    #[test]
    fn la_longueur_du_chemin_recalculee_confirme_la_mesure_de_blender() {
        // Blender annonce 358,7 m dans son bloc `mesures`. On NE LE LIT PAS —
        // on recalcule sur les points. Deux sources pour une meme grandeur
        // divergent toujours ; ici l'accord des deux valide la conversion sur
        // 91 points d'un coup.
        let l = vallon().longueur_chemin_m();
        assert!(
            (l - 358.7).abs() < 2.0,
            "chemin recalcule a {l:.1} m, Blender annonce 358,7"
        );
    }

    #[test]
    fn le_joueur_se_pose_sur_la_dalle_et_pas_dedans() {
        // LE test du defaut du 2026-08-14. L'origine du Transform du joueur est
        // le CENTRE de sa capsule : le poser a l'altitude du sol l'enterre d'un
        // metre. Releve en jeu : grounded true, 20 contacts KCC, immobile.
        let m = vallon();
        let sol = m.spawn_bevy();
        let joueur = m.spawn_player_origin();
        let ecart = joueur.y - sol.y;
        let attendu = m.spawn.dalle_demi[2] + forgia_player::PLAYER_FOOT_OFFSET_M;
        assert!(
            (ecart - attendu).abs() < 1.0e-4,
            "ecart {ecart:.3} m au lieu de {attendu:.3} (dalle + demi-capsule)"
        );
        // Et surtout : les PIEDS doivent etre AU-DESSUS du sol, jamais dedans.
        let pieds = joueur.y - forgia_player::PLAYER_FOOT_OFFSET_M;
        assert!(
            pieds > sol.y,
            "pieds a {pieds:.2} m pour un sol a {:.2} : le joueur est enterre",
            sol.y
        );
        // Le XZ ne bouge pas — on ne corrige qu'une altitude.
        assert!((joueur.x - sol.x).abs() < 1.0e-6 && (joueur.z - sol.z).abs() < 1.0e-6);
    }

    #[test]
    fn le_chemin_relie_le_spawn_au_village() {
        // Verifie les DEUX bouts : une conversion en miroir ferait finir le
        // chemin a l'oppose du village sans changer sa longueur.
        let m = vallon();
        let pts: Vec<Vec3> = m.chemin_bevy().collect();
        let depart = pts[0].distance(m.spawn_bevy());
        let arrivee = pts[pts.len() - 1].distance(blender_to_bevy(m.village_xyz));
        assert!(depart < 5.0, "le chemin demarre a {depart:.1} m du spawn");
        assert!(arrivee < 10.0, "le chemin finit a {arrivee:.1} m du village");
    }

    #[test]
    fn les_campements_jalonnent_le_chemin() {
        // Trois verrous sur 358 m : chacun doit etre PRES du chemin, sinon le
        // joueur les contourne sans les voir et l'expedition n'a plus de rythme.
        let m = vallon();
        let pts: Vec<Vec3> = m.chemin_bevy().collect();
        for c in &m.campements {
            let centre = blender_to_bevy(c.centre_xyz);
            let d = pts
                .iter()
                .map(|p| p.distance(centre))
                .fold(f32::MAX, f32::min);
            assert!(
                d <= c.rayon_m * 2.0,
                "{} est a {d:.1} m du chemin (rayon {})",
                c.id,
                c.rayon_m
            );
        }
    }

    #[test]
    fn le_detecteur_de_tir_gratuit_fonctionne() {
        // `map-design-intention.md` §2.2 : un ennemi dont la vision est
        // inferieure a la plus longue ligne de sa salle se fait tirer SANS
        // pouvoir repondre ni reagir.
        //
        // # Pourquoi ce test verifie le DETECTEUR et pas la carte
        //
        // Mesure du 2026-08-14 sur Le Vallon : les TROIS campements annoncent
        // `ligne_max 24 m` pour `vision_grunt 20 m`, soit **4 m de tir gratuit**
        // chacun. C'est un defaut de CARTE, pas de code — il se corrige sous
        // Blender (casser la ligne de vue) ou au genome (elargir la vision), et
        // ni l'un ni l'autre n'est une decision de ce fichier.
        //
        // Faire echouer la compilation dessus bloquerait le build sur une donnee
        // qu'il ne peut pas corriger ; abaisser le seuil pour repasser au vert
        // serait pire encore (`map-design-patterns.md` §13 : « ne jamais regagner
        // le vert en abaissant le seuil »). Le constat vit donc dans le LOG de
        // chargement, ou il se voit a chaque partie, et ce test garantit
        // seulement que le detecteur sait le voir.
        let large = CampementDef {
            id: "large".into(),
            centre_xyz: [0.0; 3],
            rayon_m: 12.0,
            verrou_xyz: [0.0; 3],
            verrou_cap_rad: 0.0,
            apparitions_xyz: vec![[0.0; 3]],
            abris: vec![AbriDef {
                xyz: [0.0; 3],
                rayon_m: 1.0,
                hauteur_m: 2.4,
                casse_la_vue: true,
            }],
            archetypes: vec!["grunt".into()],
            ligne_max_m: 18.0,
            grunt_vision_m: 20.0,
            vision_min_m: Some(20.0),
            effectifs: None,
            arsenal_dps: None,
            condition_sortie: None,
            duree_tir_s: None,
            duree_approche_s: None,
            essaim_arrive: None,
            duree_plancher_s: None,
        };
        assert!(large.vision_couvre_la_ligne(), "18 m de ligne, 20 m de vision");

        let etroit = CampementDef {
            ligne_max_m: 24.0,
            ..large
        };
        assert!(
            !etroit.vision_couvre_la_ligne(),
            "24 m de ligne pour 20 m de vision doit etre DETECTE"
        );
    }

    #[test]
    fn le_vallon_ne_donne_plus_de_tir_gratuit() {
        // # L'histoire de ce test, parce qu'elle est la lecon
        //
        // Il affirmait l'inverse : « 3 campements a 4 m de tir gratuit », grave
        // exprès pour tomber le jour ou la carte serait corrigee. Il vient de
        // tomber — c'etait sa fonction, et c'est pour ca qu'un constat doit
        // etre un test et pas un commentaire.
        //
        // La cause etait un LITTERAL : `rayon: 12.0` ecrit dans le commentaire
        // meme qui citait §2.2 (« ligne max <= vision du grunt »). 12 m de rayon
        // font 24 m de ligne pour 20 m de vision. Le rayon se DERIVE desormais
        // de la vision des archetypes presents (rayon = min(vision) / 2 = 10 m),
        // donc l'invariant est vrai par construction et non plus par vigilance.
        let camps = vallon().campements;
        let fautifs: Vec<(String, f32)> = camps
            .iter()
            .filter(|c| !c.vision_couvre_la_ligne())
            .map(|c| (c.id.clone(), c.ligne_max_m - c.grunt_vision_m))
            .collect();
        assert!(
            fautifs.is_empty(),
            "tir gratuit revenu sur {fautifs:?} — la derivation du rayon a saute"
        );
        // Et on verifie la derivation elle-meme, pas seulement son resultat :
        // une carte dont tous les camps feraient 1 m de rayon passerait le test
        // ci-dessus sans rien valoir.
        for c in &camps {
            if let Some(v) = c.vision_min_m {
                assert!(
                    (c.ligne_max_m - v).abs() < 0.51,
                    "{} : ligne {} m pour une vision min de {v} m — le rayon ne \
                     derive plus de la vision, il a ete ecrit",
                    c.id,
                    c.ligne_max_m
                );
            }
        }
    }

    #[test]
    fn chaque_campement_a_de_quoi_faire_apparaitre_et_de_quoi_s_abriter() {
        // Un campement sans point d'apparition ne peuple rien ; sans abri, c'est
        // un stand de tir (map-design-patterns §11). Les deux silencieusement.
        for c in &vallon().campements {
            assert!(!c.apparitions_xyz.is_empty(), "{} sans apparition", c.id);
            assert!(!c.abris.is_empty(), "{} sans abri", c.id);
        }
    }

    #[test]
    fn tous_les_abris_cassent_vraiment_la_ligne_de_vue() {
        // L'œil du joueur est a 1,70 m et il n'y a PAS d'accroupissement : un
        // bloc plus bas masque le corps sans masquer la vue. Le premier jet
        // tirait l'echelle au hasard et sortait des blocs a 1,52 m — des
        // couvertures qui n'en sont pas, et que rien ne distinguait des vraies.
        let m = vallon();
        let menteurs: Vec<_> = m
            .abris_qui_n_abritent_pas()
            .map(|(camp, a)| format!("{camp} @ {:.1} m", a.hauteur_m))
            .collect();
        assert!(
            menteurs.is_empty(),
            "abris qui n'abritent pas : {menteurs:?} — sous l'oeil du joueur"
        );
        // Et il y en a assez pour que la salle ait un jeu de couverture.
        for c in &m.campements {
            assert!(c.abris.len() >= 4, "{} n'a que {} abris", c.id, c.abris.len());
        }
    }

    #[test]
    fn les_colliders_tiennent_dans_l_emprise_et_ont_une_emprise_reelle() {
        // Un seul mal converti se plante hors carte ou se degenere en rayon nul,
        // et le joueur traverse un arbre sans savoir pourquoi. Un cylindre de
        // rayon ou de hauteur nulle est PIRE qu'absent : il se compte comme
        // present (`map-design-patterns.md` §13).
        let m = vallon();
        let (demi_x, demi_z) = (m.emprise_m[0] * 0.5 + 5.0, m.emprise_m[1] * 0.5 + 5.0);
        let mut hors = 0;
        for (famille, base, hauteur, rayon) in m.colliders_bevy() {
            assert!(rayon > 0.0, "{famille} : collider de rayon nul");
            assert!(hauteur > 0.0, "{famille} : collider de hauteur nulle");
            if base.x.abs() > demi_x || base.z.abs() > demi_z {
                hors += 1;
            }
        }
        assert_eq!(hors, 0, "{hors} colliders hors de l'emprise");
    }

    #[test]
    fn aucun_solide_ne_se_franchit_au_saut() {
        // Le joueur saute 1,174 m. Un cylindre plus court se franchit par le
        // dessus, et le symptome est « les collisions ne marchent pas ».
        // La hauteur vient desormais du manifeste : avant, le plugin posait
        // 6,0 m pour TOUT LE MONDE — genereux pour un arbre, mensonger pour un
        // eboulis de 80 cm, qui devenait un mur de 6 m invisible.
        //
        // Le seuil de solidite cote Blender est 0,60 m : sous le saut, donc
        // franchissable — c'est VOULU, on ne veut pas trebucher sur du decor
        // de sol. Ce test verifie seulement qu'aucune hauteur n'est absurde.
        let m = vallon();
        for (famille, _base, hauteur, _r) in m.colliders_bevy() {
            assert!(
                (0.5..=30.0).contains(&hauteur),
                "{famille} : hauteur {hauteur:.2} m invraisemblable"
            );
        }
    }

    #[test]
    fn la_riviere_descend_vraiment_vers_l_aval() {
        // L'amont doit etre PLUS HAUT que l'aval. Une conversion qui melange les
        // axes ferait remonter la riviere — et une riviere qui remonte ne se
        // voit qu'en la suivant sur 190 m.
        let m = vallon();
        let amont = blender_to_bevy(m.eau.amont_xyz);
        let aval = blender_to_bevy(m.eau.aval_xyz);
        assert!(
            amont.y > aval.y,
            "amont a {:.2} m, aval a {:.2} m : la riviere remonte",
            amont.y,
            aval.y
        );
        assert!(
            ((amont.y - aval.y) - m.eau.denivele_m).abs() < 0.1,
            "denivele recalcule {:.2} m, annonce {:.2}",
            amont.y - aval.y,
            m.eau.denivele_m
        );
    }
}
