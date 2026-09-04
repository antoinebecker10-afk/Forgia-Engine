//! Le balancement de cape — **sans simulation**.
//!
//! # Pourquoi pas un solveur
//!
//! Une chaîne d'os-ressorts peut diverger, se plier en épingle et traverser le
//! personnage : trois défauts constatés à l'écran le 2026-08-21, dont un coude
//! de 89° jamais expliqué. Ici il n'y a pas de chaîne à faire diverger — la
//! cape reste une pièce rigide du costume, à laquelle on ajoute une inclinaison
//! CALCULÉE depuis ce que le personnage fait.
//!
//! C'est la technique historique de World of Warcraft, dont la cape suit le
//! corps avec un flottement discret : elle n'est pas simulée, elle est
//! **conduite**. Le rendu visé est celui-là — raide, mais vivante.
//!
//! # Ce qui entre, ce qui sort
//!
//! Deux entrées seulement : la vitesse horizontale (la cape traîne derrière
//! quand on court) et la vitesse de rotation du cap (elle part sur le côté
//! quand on tourne). Une inclinaison lissée en sort, répartie sur les maillons
//! avec des parts croissantes : la couture ne bouge presque pas, l'ourlet
//! bouge le plus.
//!
//! # Ce que ça ne peut pas faire
//!
//! Ni plier, ni s'emmêler, ni entrer dans le corps — il n'y a aucune intégration
//! et aucune contrainte à violer. En contrepartie, la cape ne réagit pas à ce
//! qui n'est pas dans ses deux entrées : pas de rebond après un saut, pas de
//! vent. Le jour où on voudra ça, ce sera des clés d'animation (voie WoW
//! moderne) ou un solveur réparé, pas un troisième mécanisme empilé ici.

use bevy::prelude::*;
use forgia_player::{Player, PlayerLocomotion};

/// La chaîne de cape et sa pose de liaison, capturées une fois à l'accrochage.
///
/// Les poses sont la référence ABSOLUE : chaque frame réécrit
/// `pose_de_liaison * inclinaison`, jamais `rotation_courante * quelque chose`.
/// Sans ça l'inclinaison s'accumulerait et la cape partirait en vrille — le
/// piège classique d'un pilotage par incréments.
#[derive(Resource, Default)]
pub struct ChaineCape {
    pub os: Vec<Entity>,
    pub parents: Vec<Entity>,
    pub poses: Vec<Quat>,
}

impl ChaineCape {
    pub fn oublier(&mut self) {
        self.os.clear();
        self.parents.clear();
        self.poses.clear();
    }

    #[must_use]
    pub fn vide(&self) -> bool {
        self.os.is_empty()
    }
}

/// L'inclinaison lissée, et de quoi mesurer la vitesse de rotation du cap.
#[derive(Resource, Default)]
pub struct EtatBalancement {
    /// `x` = vers l'arrière, `y` = sur le côté, en radians.
    pub tenu: Vec2,
    pub cap_precedent: Option<f32>,
    /// Publié au capteur : ce que la cape fait réellement, en degrés.
    pub incline_arriere_deg: f32,
    pub incline_cote_deg: f32,
}

/// Réglages du balancement. Couche **definition** : ça se juge à l'œil.
#[derive(Debug, Clone, Copy, serde::Deserialize)]
pub struct BalancementConfig {
    #[serde(default = "actif_defaut")]
    pub actif: bool,
    /// Inclinaison arrière à pleine vitesse, en degrés.
    #[serde(default = "angle_course_defaut")]
    pub angle_course_deg: f32,
    /// La vitesse à laquelle cette inclinaison est atteinte (m/s).
    #[serde(default = "vitesse_pleine_defaut")]
    pub vitesse_pleine_ms: f32,
    /// Inclinaison latérale à pleine vitesse de rotation, en degrés.
    #[serde(default = "angle_virage_defaut")]
    pub angle_virage_deg: f32,
    /// La vitesse de rotation à laquelle elle est atteinte (degrés/seconde).
    #[serde(default = "virage_plein_defaut")]
    pub virage_plein_deg_s: f32,
    /// Vitesse de rattrapage de l'inclinaison (par seconde). Petit = mou et
    /// tardif, grand = collé au mouvement, donc raide.
    #[serde(default = "reactivite_defaut")]
    pub reactivite: f32,
    /// Répartition sur les maillons. 1 = croissance régulière de la couture à
    /// l'ourlet ; > 1 = tout se joue en bas ; < 1 = la cape se courbe haut.
    #[serde(default = "courbure_defaut")]
    pub courbure: f32,
}

fn actif_defaut() -> bool {
    true
}
fn angle_course_defaut() -> f32 {
    18.0
}
fn vitesse_pleine_defaut() -> f32 {
    9.75
}
fn angle_virage_defaut() -> f32 {
    12.0
}
fn virage_plein_defaut() -> f32 {
    180.0
}
fn reactivite_defaut() -> f32 {
    6.0
}
fn courbure_defaut() -> f32 {
    1.0
}

impl Default for BalancementConfig {
    fn default() -> Self {
        Self {
            actif: actif_defaut(),
            angle_course_deg: angle_course_defaut(),
            vitesse_pleine_ms: vitesse_pleine_defaut(),
            angle_virage_deg: angle_virage_defaut(),
            virage_plein_deg_s: virage_plein_defaut(),
            reactivite: reactivite_defaut(),
            courbure: courbure_defaut(),
        }
    }
}

/// La part d'inclinaison portée par chaque maillon. **Somme exactement 1** :
/// l'ourlet s'incline donc de l'angle demandé, ni plus ni moins, quel que soit
/// le nombre d'os. Sans normalisation, une cape à six os pencherait deux fois
/// plus qu'une à trois pour le même réglage.
#[must_use]
pub fn parts_du_balancement(nombre: usize, courbure: f32) -> Vec<f32> {
    if nombre == 0 {
        return Vec::new();
    }
    let courbure = courbure.clamp(0.1, 4.0);
    let brutes: Vec<f32> = (0..nombre)
        .map(|i| (((i + 1) as f32) / (nombre as f32)).powf(courbure))
        .collect();
    let somme: f32 = brutes.iter().sum();
    if somme <= f32::EPSILON {
        return vec![1.0 / nombre as f32; nombre];
    }
    brutes.into_iter().map(|p| p / somme).collect()
}

/// Ramène un écart d'angle dans `[-π, π]` — un cap qui passe par ±π ne fait pas
/// un demi-tour à 300 °/s dans le calcul.
#[must_use]
pub fn ecart_de_cap(courant: f32, precedent: f32) -> f32 {
    let deux_pi = std::f32::consts::TAU;
    let mut d = (courant - precedent) % deux_pi;
    if d > std::f32::consts::PI {
        d -= deux_pi;
    } else if d < -std::f32::consts::PI {
        d += deux_pi;
    }
    d
}

/// L'inclinaison visée, avant lissage : arrière selon la vitesse, côté selon la
/// rotation du cap. Bornée aux angles du génome — une cape ne se retourne pas.
#[must_use]
pub fn inclinaison_visee(
    vitesse_ms: f32,
    rotation_rad_s: f32,
    cfg: &BalancementConfig,
) -> Vec2 {
    let part_vitesse = if cfg.vitesse_pleine_ms > f32::EPSILON {
        (vitesse_ms / cfg.vitesse_pleine_ms).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let plein_virage = cfg.virage_plein_deg_s.to_radians();
    let part_virage = if plein_virage > f32::EPSILON {
        (rotation_rad_s / plein_virage).clamp(-1.0, 1.0)
    } else {
        0.0
    };
    Vec2::new(
        part_vitesse * cfg.angle_course_deg.to_radians(),
        part_virage * cfg.angle_virage_deg.to_radians(),
    )
}

/// Incline la cape. Aucune intégration, aucune contrainte : à entrées égales,
/// sortie égale.
pub fn balancer_la_cape(
    cfg: Res<crate::cape::CapeConfig>,
    temps: Res<Time>,
    chaine: Res<ChaineCape>,
    mut etat: ResMut<EtatBalancement>,
    joueur: Query<&Player>,
    locomotion: Option<Res<PlayerLocomotion>>,
    q_gt: Query<&GlobalTransform>,
    mut q_tf: Query<&mut Transform>,
) {
    let reglages = cfg.balancement;
    if chaine.vide() || !reglages.actif {
        return;
    }
    let dt = temps.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let Ok(joueur) = joueur.single() else {
        return;
    };

    // Vitesse de rotation du cap, mesurée entre deux frames.
    let rotation_rad_s = match etat.cap_precedent {
        Some(precedent) => ecart_de_cap(joueur.yaw, precedent) / dt,
        None => 0.0,
    };
    etat.cap_precedent = Some(joueur.yaw);
    let vitesse = locomotion.map_or(0.0, |l| l.horizontal_speed);

    // Lissage exponentiel : c'est lui qui donne le retard et le retour au
    // calme. Exprimé PAR SECONDE, donc indépendant du nombre d'images.
    let visee = inclinaison_visee(vitesse, rotation_rad_s, &reglages);
    let rattrapage = 1.0 - (-reglages.reactivite.max(0.0) * dt).exp();
    let tenu = etat.tenu;
    etat.tenu = tenu + (visee - tenu) * rattrapage;
    etat.incline_arriere_deg = etat.tenu.x.to_degrees();
    etat.incline_cote_deg = etat.tenu.y.to_degrees();

    // Le repère du personnage, dérivé de son cap — la même convention que le
    // reste du mode (`cap = atan2(-x, -z)`).
    let avant = Vec3::new(-joueur.yaw.sin(), 0.0, -joueur.yaw.cos());
    let droite = Vec3::new(-avant.z, 0.0, avant.x);

    let parts = parts_du_balancement(chaine.os.len(), reglages.courbure);
    for (i, (&os, &pose)) in chaine.os.iter().zip(chaine.poses.iter()).enumerate() {
        let part = parts.get(i).copied().unwrap_or(0.0);
        // Signe : une inclinaison arrière positive doit emmener l'ourlet
        // DERRIÈRE le personnage. Vérifié par `l_inclinaison_arriere_va_bien_vers_l_arriere`.
        let monde = Quat::from_axis_angle(droite, -etat.tenu.x * part)
            * Quat::from_axis_angle(avant, etat.tenu.y * part);
        let repere = chaine
            .parents
            .get(i)
            .and_then(|&p| q_gt.get(p).ok())
            .map_or(Quat::IDENTITY, |gt| gt.rotation());
        let locale = repere.inverse() * monde * repere;
        if let Ok(mut tf) = q_tf.get_mut(os) {
            // Toujours depuis la pose de liaison, jamais depuis la précédente.
            tf.rotation = locale * pose;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_parts_somment_a_un_et_croissent_vers_l_ourlet() {
        for nombre in [1usize, 2, 5, 6, 12] {
            let parts = parts_du_balancement(nombre, 1.0);
            let somme: f32 = parts.iter().sum();
            assert!((somme - 1.0).abs() < 1e-5, "{nombre} os -> somme {somme}");
            for paire in parts.windows(2) {
                assert!(paire[1] >= paire[0], "l'ourlet doit bouger plus que la couture");
            }
        }
        // La couture bouge le moins, quelle que soit la courbure.
        for courbure in [0.5_f32, 1.0, 2.0, 3.0] {
            let parts = parts_du_balancement(6, courbure);
            assert!(parts[0] < parts[5]);
        }
    }

    #[test]
    fn l_inclinaison_est_bornee_par_le_genome() {
        let cfg = BalancementConfig::default();
        // Deux fois la vitesse pleine ne donne pas deux fois l'angle.
        let visee = inclinaison_visee(cfg.vitesse_pleine_ms * 2.0, 0.0, &cfg);
        assert!((visee.x - cfg.angle_course_deg.to_radians()).abs() < 1e-5);
        // Immobile : rien.
        assert!(inclinaison_visee(0.0, 0.0, &cfg).x.abs() < 1e-6);
        // Rotation dans un sens puis dans l'autre : angles opposés, bornés.
        let vif = cfg.virage_plein_deg_s.to_radians() * 5.0;
        let a = inclinaison_visee(0.0, vif, &cfg).y;
        let b = inclinaison_visee(0.0, -vif, &cfg).y;
        assert!((a + b).abs() < 1e-6);
        assert!((a.abs() - cfg.angle_virage_deg.to_radians()).abs() < 1e-5);
    }

    #[test]
    fn un_demi_tour_ne_compte_pas_comme_une_rotation_folle() {
        let pi = std::f32::consts::PI;
        // De +179° à −179° : 2° de rotation, pas 358.
        let d = ecart_de_cap(-pi + 0.017, pi - 0.017);
        assert!(d.abs() < 0.05, "ecart {d} rad");
    }

    /// Le signe : une inclinaison arrière doit emmener l'ourlet DERRIÈRE.
    #[test]
    fn l_inclinaison_arriere_va_bien_vers_l_arriere() {
        for cap in [0.0_f32, 1.0, -2.5, 3.0] {
            let avant = Vec3::new(-cap.sin(), 0.0, -cap.cos());
            let droite = Vec3::new(-avant.z, 0.0, avant.x);
            let inclinaison = 20.0_f32.to_radians();
            let ourlet = Quat::from_axis_angle(droite, -inclinaison) * Vec3::NEG_Y;
            assert!(
                ourlet.dot(avant) < -0.1,
                "cap {cap} : l'ourlet part vers l'avant ({ourlet})"
            );
            assert!(ourlet.y < 0.0, "l'ourlet doit rester sous la couture");
        }
    }
}
