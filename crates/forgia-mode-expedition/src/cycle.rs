//! cycle.rs — la nuit tombe avec la DISTANCE, pas avec le temps.
//!
//! « Plus je me rapproche du village, plus il fait nuit, et à la fin on voit
//! encore un peu mais c'est sombre. Pas mode horreur. »
//!
//! # Pourquoi la distance et pas une horloge
//!
//! Un cycle temporel punit celui qui explore et récompense celui qui court : la
//! même carte se joue en plein jour ou en pleine nuit selon le rythme du joueur,
//! et le level design ne peut plus rien promettre. En liant la nuit à
//! l'**abscisse curviligne sur le chemin autoré**, le trajet EST le cycle —
//! chacun voit la même lumière au même endroit, quel que soit son temps de jeu.
//!
//! C'est aussi ce qui rend la chose testable : aucune horloge, donc un résultat
//! reproductible à partir d'une seule position.
//!
//! # Ce que ce module fait, et ce qu'il ne fait pas
//!
//! Il **calcule** une progression et des grandeurs d'éclairage. Il ne touche à
//! aucune lumière — c'est `lighting.rs` qui applique. Séparation délibérée :
//! toute la partie qui peut être fausse est vérifiable sans moteur.

use bevy::prelude::*;
use serde::Deserialize;

const GENOME_PATH: &str = "assets/genomes/expedition_cycle.toml";

/// Réglages du cycle, lus depuis [`GENOME_PATH`]. Les défauts sont le reflet du
/// TOML — un test vérifie qu'ils ne divergent pas.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct CycleConfig {
    pub soleil_depart_deg: f32,
    pub soleil_arrivee_deg: f32,
    pub soleil_azimut_depart_deg: f32,
    pub soleil_azimut_arrivee_deg: f32,
    pub soleil_lux_depart: f32,
    pub soleil_lux_arrivee: f32,
    pub ambiante_depart_lux: f32,
    pub ambiante_arrivee_lux: f32,
    pub brouillard_debut_depart_m: f32,
    pub brouillard_fin_depart_m: f32,
    pub brouillard_debut_arrivee_m: f32,
    pub brouillard_fin_arrivee_m: f32,
    pub courbe_exposant: f32,
    /// Diamètre apparent du disque solaire, en degrés.
    ///
    /// Le vrai soleil couvre **0,53°** et se perd à l'écran — on le cherche.
    /// Le grossir est un choix de lisibilité, pas une erreur : c'est ce que
    /// fait tout jeu où le coucher de soleil est un moment de la partie.
    pub soleil_taille_deg: f32,
}

/// 3° : environ six fois le vrai, assez pour se voir descendre derrière les
/// crêtes sans occuper le ciel.
fn soleil_taille_par_defaut() -> f32 {
    3.0
}

impl Default for CycleConfig {
    fn default() -> Self {
        Self {
            soleil_depart_deg: 32.0,
            soleil_arrivee_deg: -8.0,
            soleil_azimut_depart_deg: 250.0,
            soleil_azimut_arrivee_deg: 298.0,
            soleil_lux_depart: 11000.0,
            soleil_lux_arrivee: 70.0,
            ambiante_depart_lux: 900.0,
            ambiante_arrivee_lux: 110.0,
            brouillard_debut_depart_m: 90.0,
            brouillard_fin_depart_m: 420.0,
            brouillard_debut_arrivee_m: 35.0,
            brouillard_fin_arrivee_m: 180.0,
            courbe_exposant: 1.15,
            soleil_taille_deg: soleil_taille_par_defaut(),
        }
    }
}

#[derive(Deserialize)]
struct CycleToml {
    cycle: CycleBloc,
}

#[derive(Deserialize)]
struct CycleBloc {
    soleil_depart_deg: Option<f32>,
    soleil_arrivee_deg: Option<f32>,
    soleil_azimut_depart_deg: Option<f32>,
    soleil_azimut_arrivee_deg: Option<f32>,
    soleil_lux_depart: Option<f32>,
    soleil_lux_arrivee: Option<f32>,
    ambiante_depart_lux: Option<f32>,
    ambiante_arrivee_lux: Option<f32>,
    brouillard_debut_depart_m: Option<f32>,
    brouillard_fin_depart_m: Option<f32>,
    brouillard_debut_arrivee_m: Option<f32>,
    brouillard_fin_arrivee_m: Option<f32>,
    courbe_exposant: Option<f32>,
    soleil_taille_deg: Option<f32>,
}

impl CycleConfig {
    #[must_use]
    pub fn load_or_default() -> Self {
        match forgia_core::def_io::read_def_str(GENOME_PATH) {
            Ok(c) => Self::parse_toml(&c),
            Err(_) => Self::default(),
        }
    }

    #[must_use]
    pub fn parse_toml(contenu: &str) -> Self {
        let Ok(t) = toml::from_str::<CycleToml>(contenu) else {
            return Self::default();
        };
        let d = Self::default();
        let c = t.cycle;
        Self {
            soleil_depart_deg: c.soleil_depart_deg.unwrap_or(d.soleil_depart_deg),
            soleil_arrivee_deg: c.soleil_arrivee_deg.unwrap_or(d.soleil_arrivee_deg),
            soleil_azimut_depart_deg: c
                .soleil_azimut_depart_deg
                .unwrap_or(d.soleil_azimut_depart_deg),
            soleil_azimut_arrivee_deg: c
                .soleil_azimut_arrivee_deg
                .unwrap_or(d.soleil_azimut_arrivee_deg),
            soleil_lux_depart: c.soleil_lux_depart.unwrap_or(d.soleil_lux_depart),
            soleil_lux_arrivee: c.soleil_lux_arrivee.unwrap_or(d.soleil_lux_arrivee),
            ambiante_depart_lux: c.ambiante_depart_lux.unwrap_or(d.ambiante_depart_lux),
            // ★ Le PLANCHER de lisibilité. Borné en dur au-dessus de zéro : c'est
            // la seule garantie de « on voit encore un peu », et le seul rempart
            // contre le mode horreur écarté explicitement. Un génome mal réglé ne
            // doit pas pouvoir éteindre la carte.
            ambiante_arrivee_lux: c
                .ambiante_arrivee_lux
                .unwrap_or(d.ambiante_arrivee_lux)
                .max(60.0),
            brouillard_debut_depart_m: c
                .brouillard_debut_depart_m
                .unwrap_or(d.brouillard_debut_depart_m),
            brouillard_fin_depart_m: c
                .brouillard_fin_depart_m
                .unwrap_or(d.brouillard_fin_depart_m),
            brouillard_debut_arrivee_m: c
                .brouillard_debut_arrivee_m
                .unwrap_or(d.brouillard_debut_arrivee_m),
            brouillard_fin_arrivee_m: c
                .brouillard_fin_arrivee_m
                .unwrap_or(d.brouillard_fin_arrivee_m),
            // Sous 1 la nuit tomberait d'un coup au premier pas ; au-delà de 4
            // elle arriverait d'un bloc à la fin. Les deux se subissent au lieu
            // de se voir venir.
            courbe_exposant: c.courbe_exposant.unwrap_or(d.courbe_exposant).clamp(1.0, 4.0),
            // Borné : sous le vrai soleil (0,53°) on ne le verrait pas, et
            // au-delà de 15° il occuperait le ciel au lieu de s'y coucher.
            soleil_taille_deg: c
                .soleil_taille_deg
                .unwrap_or(d.soleil_taille_deg)
                .clamp(0.53, 15.0),
        }
    }
}

/// Où en est le joueur sur le chemin, dans `[0, 1]`.
///
/// **Abscisse curviligne**, pas distance à vol d'oiseau : on projette la position
/// sur chaque segment du chemin, on garde le plus proche, et on rend la fraction
/// de longueur déjà parcourue.
///
/// # Pourquoi pas la distance au village
///
/// Le chemin serpente sur 359,8 m pour 214 m à vol d'oiseau. Mesurer « distance
/// au village / distance totale » ferait donc bondir la nuit à chaque virage qui
/// rapproche géographiquement sans faire avancer, et reculer le jour dans les
/// lacets. La projection sur le tracé suit ce que le joueur PARCOURT.
///
/// Un joueur qui quitte le chemin garde la progression du point le plus proche —
/// c'est le comportement voulu : s'écarter pour contourner un campement ne doit
/// ni avancer ni reculer l'heure.
#[must_use]
pub fn progression_sur_chemin(pos: Vec3, chemin: &[Vec3]) -> f32 {
    if chemin.len() < 2 {
        return 0.0;
    }
    // Longueurs cumulées, calculées à la volée : le chemin fait 91 points, donc
    // deux passes coûtent moins qu'un cache à invalider.
    let total: f32 = chemin.windows(2).map(|s| s[0].distance(s[1])).sum();
    if total <= f32::EPSILON {
        return 0.0;
    }
    let p = Vec2::new(pos.x, pos.z);
    let mut parcouru = 0.0_f32;
    let mut meilleur_d2 = f32::MAX;
    let mut meilleur_s = 0.0_f32;
    for seg in chemin.windows(2) {
        let (a, b) = (Vec2::new(seg[0].x, seg[0].z), Vec2::new(seg[1].x, seg[1].z));
        let ab = b - a;
        let len = ab.length();
        if len > f32::EPSILON {
            // Projection bornée au segment — sans le clamp, un joueur loin
            // devant verrait sa progression extrapolée au-delà du chemin.
            let t = ((p - a).dot(ab) / (len * len)).clamp(0.0, 1.0);
            let d2 = (a + ab * t).distance_squared(p);
            if d2 < meilleur_d2 {
                meilleur_d2 = d2;
                meilleur_s = parcouru + t * len;
            }
        }
        parcouru += len;
    }
    (meilleur_s / total).clamp(0.0, 1.0)
}

/// La progression **courbée** — ce qui pilote réellement la lumière.
///
/// Sans courbe, la lumière baisse dès le premier pas et se lit comme un fondu
/// d'écran. Avec un exposant > 1, la nuit tombe tard et vite, comme un vrai
/// crépuscule.
#[must_use]
pub fn courbe(progression: f32, exposant: f32) -> f32 {
    progression.clamp(0.0, 1.0).powf(exposant.max(0.01))
}

/// L'état d'éclairage à un instant donné. Tout est **dérivé** de la progression.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EtatCycle {
    /// Progression brute sur le chemin, `[0, 1]`.
    pub progression: f32,
    pub soleil_elevation_deg: f32,
    pub soleil_azimut_deg: f32,
    pub soleil_lux: f32,
    pub ambiante_lux: f32,
    pub brouillard_debut_m: f32,
    pub brouillard_fin_m: f32,
}

impl EtatCycle {
    /// Il fait nuit quand le soleil est passé sous l'horizon.
    #[must_use]
    pub fn est_nuit(&self) -> bool {
        self.soleil_elevation_deg < 0.0
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Calcule tout l'éclairage depuis la seule progression. **Pur.**
#[must_use]
pub fn etat_du_cycle(progression: f32, cfg: &CycleConfig) -> EtatCycle {
    let p = progression.clamp(0.0, 1.0);
    let t = courbe(p, cfg.courbe_exposant);
    EtatCycle {
        progression: p,
        soleil_elevation_deg: lerp(cfg.soleil_depart_deg, cfg.soleil_arrivee_deg, t),
        soleil_azimut_deg: lerp(
            cfg.soleil_azimut_depart_deg,
            cfg.soleil_azimut_arrivee_deg,
            t,
        ),
        soleil_lux: lerp(cfg.soleil_lux_depart, cfg.soleil_lux_arrivee, t),
        ambiante_lux: lerp(cfg.ambiante_depart_lux, cfg.ambiante_arrivee_lux, t),
        brouillard_debut_m: lerp(
            cfg.brouillard_debut_depart_m,
            cfg.brouillard_debut_arrivee_m,
            t,
        ),
        brouillard_fin_m: lerp(cfg.brouillard_fin_depart_m, cfg.brouillard_fin_arrivee_m, t),
    }
}

/// Direction vers laquelle pointe le soleil, depuis élévation et azimut.
///
/// Une `DirectionalLight` de Bevy éclaire **dans son axe −Z local**. On construit
/// donc la rotation qui amène cet axe sur la direction descendante voulue.
#[must_use]
pub fn rotation_soleil(elevation_deg: f32, azimut_deg: f32) -> Quat {
    Quat::from_euler(
        EulerRot::YXZ,
        azimut_deg.to_radians(),
        -elevation_deg.to_radians(),
        0.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chemin_droit() -> Vec<Vec3> {
        (0..=10).map(|i| Vec3::new(i as f32 * 10.0, 0.0, 0.0)).collect()
    }

    // ── Le génome ───────────────────────────────────────────────────────

    #[test]
    fn le_repli_reflete_exactement_le_toml() {
        // Le piège payé le 2026-08-13 : quand le TOML et le défaut Rust
        // divergent, le repli (fichier absent, wasm) devient différent du jeu
        // normal, et le défaut ne se voit qu'en production.
        let toml = include_str!("../../../assets/genomes/expedition_cycle.toml");
        assert_eq!(CycleConfig::parse_toml(toml), CycleConfig::default());
    }

    #[test]
    fn le_plancher_de_lisibilite_ne_peut_pas_etre_eteint() {
        // « Pas mode horreur » : un génome mal réglé ne doit pas pouvoir rendre
        // la carte noire. C'est la SEULE garantie de « on voit encore un peu ».
        let c = CycleConfig::parse_toml("[cycle]\nambiante_arrivee_lux = 0.0\n");
        assert!(c.ambiante_arrivee_lux >= 60.0, "la nuit peut etre totale");
    }

    #[test]
    fn la_courbe_reste_dans_une_bande_jouable() {
        // Sous 1 la nuit tombe au premier pas ; au-dela de 4 elle arrive d'un
        // bloc. Les deux se subissent au lieu de se voir venir.
        assert!(CycleConfig::parse_toml("[cycle]\ncourbe_exposant = 0.1\n").courbe_exposant >= 1.0);
        assert!(CycleConfig::parse_toml("[cycle]\ncourbe_exposant = 99.0\n").courbe_exposant <= 4.0);
    }

    // ── La progression ──────────────────────────────────────────────────

    #[test]
    fn au_depart_la_progression_est_nulle_et_a_l_arrivee_elle_vaut_un() {
        let ch = chemin_droit();
        assert!(progression_sur_chemin(ch[0], &ch).abs() < 1.0e-4);
        assert!((progression_sur_chemin(ch[ch.len() - 1], &ch) - 1.0).abs() < 1.0e-4);
    }

    #[test]
    fn a_mi_chemin_la_progression_vaut_un_demi() {
        let ch = chemin_droit();
        let p = progression_sur_chemin(Vec3::new(50.0, 0.0, 0.0), &ch);
        assert!((p - 0.5).abs() < 1.0e-3, "progression {p}");
    }

    #[test]
    fn s_ecarter_du_chemin_ne_change_pas_l_heure() {
        // Comportement VOULU : contourner un campement ne doit ni avancer ni
        // reculer la nuit. On garde la progression du point le plus proche.
        let ch = chemin_droit();
        let sur = progression_sur_chemin(Vec3::new(50.0, 0.0, 0.0), &ch);
        let a_cote = progression_sur_chemin(Vec3::new(50.0, 0.0, 25.0), &ch);
        assert!((sur - a_cote).abs() < 1.0e-3, "{sur} vs {a_cote}");
    }

    #[test]
    fn la_progression_ne_depasse_jamais_les_bornes() {
        // Sans le clamp de projection, un joueur loin devant verrait sa
        // progression EXTRAPOLEE au-dela du chemin — donc une nuit plus noire
        // que la nuit.
        let ch = chemin_droit();
        for p in [
            Vec3::new(-500.0, 0.0, 0.0),
            Vec3::new(9999.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, -800.0),
        ] {
            let v = progression_sur_chemin(p, &ch);
            assert!((0.0..=1.0).contains(&v), "progression {v} hors bornes");
        }
    }

    #[test]
    fn un_chemin_degenere_ne_panique_pas() {
        assert_eq!(progression_sur_chemin(Vec3::ZERO, &[]), 0.0);
        assert_eq!(progression_sur_chemin(Vec3::ZERO, &[Vec3::ZERO]), 0.0);
        assert_eq!(
            progression_sur_chemin(Vec3::ZERO, &[Vec3::ZERO, Vec3::ZERO]),
            0.0
        );
    }

    #[test]
    fn l_abscisse_curviligne_suit_le_trace_pas_le_vol_d_oiseau() {
        // Un chemin en L : le coude est a mi-parcours EN DISTANCE PARCOURUE,
        // alors qu'a vol d'oiseau il est bien plus loin de l'arrivee que du
        // depart. Mesurer « distance au village » ferait donc bondir la nuit
        // dans les virages.
        let l = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(100.0, 0.0, 0.0),
            Vec3::new(100.0, 0.0, 100.0),
        ];
        let coude = progression_sur_chemin(Vec3::new(100.0, 0.0, 0.0), &l);
        assert!((coude - 0.5).abs() < 1.0e-3, "au coude : {coude}");
    }

    // ── Le cycle lui-même ───────────────────────────────────────────────

    #[test]
    fn au_depart_il_fait_jour_et_a_l_arrivee_il_fait_nuit() {
        let c = CycleConfig::default();
        let depart = etat_du_cycle(0.0, &c);
        let arrivee = etat_du_cycle(1.0, &c);
        assert!(!depart.est_nuit(), "le depart doit etre de jour");
        assert!(arrivee.est_nuit(), "l'arrivee doit etre de nuit");
        assert!(depart.soleil_elevation_deg > 0.0);
        assert!(arrivee.soleil_elevation_deg < 0.0);
    }

    #[test]
    fn la_lumiere_ne_fait_que_baisser() {
        // Une remontee, meme locale, se lirait comme un bug d'eclairage. On
        // balaie tout le trajet.
        let c = CycleConfig::default();
        let mut precedent = f32::MAX;
        for i in 0..=100 {
            let e = etat_du_cycle(i as f32 / 100.0, &c);
            let total = e.soleil_lux + e.ambiante_lux;
            assert!(
                total <= precedent + 1.0e-3,
                "la lumiere remonte a {}% : {total} apres {precedent}",
                i
            );
            precedent = total;
        }
    }

    #[test]
    fn on_voit_encore_a_l_arrivee() {
        // LE critere de la demande : « a la fin on voit encore un peu mais c'est
        // sombre ». Pas de noir absolu.
        let e = etat_du_cycle(1.0, &CycleConfig::default());
        assert!(e.ambiante_lux >= 60.0, "ambiante {} : c'est le noir", e.ambiante_lux);
        assert!(
            e.ambiante_lux < 500.0,
            "ambiante {} : ce n'est plus la nuit",
            e.ambiante_lux
        );
    }

    #[test]
    fn le_soir_est_deja_engage_a_mi_chemin() {
        // 🚨 CE TEST A ÉTÉ RETOURNÉ le 2026-08-14. Il exigeait l'inverse —
        // « il doit RESTER l'essentiel de la lumière à mi-chemin » — parce que la
        // demande d'origine était « plus je me rapproche du village, plus il fait
        // nuit ». La demande a changé : « faire débuter la nuit plus tôt ».
        //
        // Un test qui garde une intention périmée est pire qu'aucun test : il
        // s'oppose au travail en cours en ayant l'air d'avoir raison. On le
        // retourne, on ne le contourne pas.
        let c = CycleConfig::default();
        let depart = etat_du_cycle(0.0, &c).ambiante_lux;
        let milieu = etat_du_cycle(0.5, &c).ambiante_lux;
        let fait = 1.0 - (milieu - c.ambiante_arrivee_lux) / (depart - c.ambiante_arrivee_lux);
        println!("A MI-CHEMIN {:.0}% de l'assombrissement est fait", fait * 100.0);
        // L'exposant étant borné à [1 ; 4] par `parse_toml`, la part faite à
        // mi-chemin vaut au plus 50 % (linéaire) et descend vers 6 % à 4. Le
        // seuil est donc dans la zone que la borne laisse ouverte : il ATTRAPE
        // vraiment un retour à la courbe lente (1,8 donnait 29 %).
        assert!(
            fait > 0.40,
            "a mi-chemin seulement {:.0}% de l'assombrissement est fait : la nuit \
             attend encore l'approche du village, alors qu'elle doit s'installer \
             des la premiere moitie du trajet",
            fait * 100.0
        );
    }

    /// Le garde-fou opposé, qui n'a pas changé : la nuit ne doit pas tomber
    /// **dès le premier pas**. Il ne se lit plus dans le test ci-dessus depuis
    /// qu'il a été retourné, donc il est écrit ici — sinon la borne basse de
    /// l'exposant ne serait plus gardée par rien.
    #[test]
    fn la_lumiere_ne_chute_pas_des_les_premiers_metres() {
        let c = CycleConfig::default();
        let depart = etat_du_cycle(0.0, &c).ambiante_lux;
        let dixieme = etat_du_cycle(0.1, &c).ambiante_lux;
        let fait = 1.0 - (dixieme - c.ambiante_arrivee_lux) / (depart - c.ambiante_arrivee_lux);
        assert!(
            fait < 0.20,
            "au dixieme du trajet {:.0}% de l'assombrissement est deja fait : ca se \
             lit comme un fondu d'ecran, pas comme un coucher de soleil",
            fait * 100.0
        );
    }

    #[test]
    fn le_soleil_tourne_il_ne_descend_pas_tout_droit() {
        // Sans rotation d'azimut, les ombres s'allongent sans jamais pivoter —
        // ca ne se lit pas comme un coucher de soleil.
        let c = CycleConfig::default();
        let a = etat_du_cycle(0.0, &c).soleil_azimut_deg;
        let b = etat_du_cycle(1.0, &c).soleil_azimut_deg;
        assert!((b - a).abs() > 20.0, "l'azimut ne bouge que de {:.0}°", b - a);
    }

    #[test]
    fn le_brouillard_se_resserre_avec_la_nuit() {
        // C'est lui qui fait sentir l'enfermement SANS baisser la lumiere —
        // la difference entre « le soir tombe » et « on n'y voit plus rien ».
        let c = CycleConfig::default();
        let depart = etat_du_cycle(0.0, &c);
        let arrivee = etat_du_cycle(1.0, &c);
        assert!(arrivee.brouillard_fin_m < depart.brouillard_fin_m);
        assert!(arrivee.brouillard_debut_m < depart.brouillard_debut_m);
        // Et il reste toujours de la marge entre debut et fin, sinon le fondu
        // devient un mur opaque.
        assert!(arrivee.brouillard_fin_m > arrivee.brouillard_debut_m * 1.5);
    }

    #[test]
    fn le_soleil_pointe_bien_vers_le_bas_quand_il_est_haut() {
        // Une rotation inversee eclairerait le monde par en dessous : tout serait
        // a contre-jour, et le symptome ne dirait pas « rotation ».
        let q = rotation_soleil(45.0, 0.0);
        let dir = q * Vec3::NEG_Z;
        assert!(dir.y < 0.0, "le soleil eclaire vers le haut : dir.y = {}", dir.y);
    }
}
