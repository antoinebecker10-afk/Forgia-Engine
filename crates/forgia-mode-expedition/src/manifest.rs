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
    /// Les abris, en plan.
    pub abris_xy: Vec<[f32; 2]>,
    /// Plus longue ligne de vue mesurée dans le campement (m).
    pub ligne_max_m: f32,
    /// Portée de vision du grunt (m) — pour vérifier qu'elle couvre `ligne_max_m`.
    pub grunt_vision_m: f32,
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
    /// `[x, y, z, rayon]` en repère Blender. Les troncs, rochers et murs.
    pub colliders_cylindre_xyzr: Vec<[f32; 4]>,
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

    /// Les cylindres de collision, convertis : `(centre_bevy, rayon)`.
    ///
    /// La hauteur n'est pas dans le manifeste — l'appelant la choisit. Le `y`
    /// exporté est celui de la **base** du cylindre.
    pub fn colliders_bevy(&self) -> impl Iterator<Item = (Vec3, f32)> + '_ {
        self.colliders_cylindre_xyzr
            .iter()
            .map(|c| (blender_to_bevy([c[0], c[1], c[2]]), c[3]))
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
        assert_eq!(m.colliders_cylindre_xyzr.len(), 943);
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
            abris_xy: vec![[0.0; 2]],
            ligne_max_m: 18.0,
            grunt_vision_m: 20.0,
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
    fn le_vallon_donne_du_tir_gratuit_et_on_le_mesure() {
        // L'etat MESURE de la carte, grave pour qu'il ne se perde pas — et pour
        // que le jour ou elle est corrigee, ce test tombe et rappelle de le
        // mettre a jour. Un constat qui ne casse rien quand il devient faux
        // n'est pas un constat, c'est un commentaire.
        let camps = vallon().campements;
        let fautifs: Vec<(String, f32)> = camps
            .iter()
            .filter(|c| !c.vision_couvre_la_ligne())
            .map(|c| (c.id.clone(), c.ligne_max_m - c.grunt_vision_m))
            .collect();
        println!("TIR GRATUIT PAR CAMPEMENT : {fautifs:?}");
        assert_eq!(
            fautifs.len(),
            3,
            "l'etat mesure le 2026-08-14 etait 3 campements a 4 m de tir gratuit ; \
             si la carte a change, mettre ce test a jour au lieu de le contourner"
        );
        for (id, ecart) in &fautifs {
            assert!(
                (*ecart - 4.0).abs() < 0.01,
                "{id} : ecart de {ecart} m, mesure 4,0 m le 2026-08-14"
            );
        }
    }

    #[test]
    fn chaque_campement_a_de_quoi_faire_apparaitre_et_de_quoi_s_abriter() {
        // Un campement sans point d'apparition ne peuple rien ; sans abri, c'est
        // un stand de tir (map-design-patterns §11). Les deux silencieusement.
        for c in &vallon().campements {
            assert!(!c.apparitions_xyz.is_empty(), "{} sans apparition", c.id);
            assert!(!c.abris_xy.is_empty(), "{} sans abri", c.id);
        }
    }

    #[test]
    fn les_colliders_tiennent_dans_l_emprise_et_ont_un_rayon_reel() {
        // 943 cylindres : un seul mal converti se plante hors carte ou se
        // degenere en rayon nul, et le joueur traverse un arbre sans savoir
        // pourquoi.
        let m = vallon();
        let (demi_x, demi_z) = (m.emprise_m[0] * 0.5 + 5.0, m.emprise_m[1] * 0.5 + 5.0);
        let mut hors = 0;
        for (centre, rayon) in m.colliders_bevy() {
            assert!(rayon > 0.0, "collider de rayon nul");
            if centre.x.abs() > demi_x || centre.z.abs() > demi_z {
                hors += 1;
            }
        }
        assert_eq!(hors, 0, "{hors} colliders hors de l'emprise sur 943");
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
