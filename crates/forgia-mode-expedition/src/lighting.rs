//! lighting.rs — poser les lumières, et les faire suivre le cycle.
//!
//! # Ce fichier n'a aucune décision à prendre
//!
//! Toutes les grandeurs viennent de [`crate::cycle`], qui est pur et testé.
//! Ici on ne fait qu'**appliquer** : créer les lumières à l'entrée, les mettre à
//! jour selon la progression, tout retirer à la sortie. La séparation est
//! délibérée — ce qui peut être faux doit être vérifiable sans lancer le jeu.
//!
//! # Pourquoi l'Expédition doit apporter son propre éclairage
//!
//! Chaque mode du projet monte son rig (`arena_test`, `castle_hub`, `cyber_city`,
//! `rpg`). L'Expédition n'en avait aucun : elle héritait de ce qui traînait du
//! menu, d'où le « c'est très sombre » rapporté en jeu le 2026-08-14. Ce n'était
//! pas un réglage à ajuster, c'était une pièce absente.

use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use forgia_player::Player;

use crate::cycle::{etat_du_cycle, progression_sur_chemin, rotation_soleil, CycleConfig, EtatCycle};
use crate::plugin::{ActiveExpedition, ExpeditionMarker};

/// Le soleil de l'expédition.
#[derive(Component)]
pub struct ExpeditionSun;

/// La lumière de remplissage — le ciel, pas le soleil.
///
/// Sans elle, tout ce qui n'est pas frappé par le direct tombe au noir dès que
/// le soleil baisse, et le relief cesse de se lire. C'est elle qui tient la
/// promesse « on voit encore un peu », pas l'ambiante seule.
#[derive(Component)]
pub struct ExpeditionFill;

/// L'état courant, publié pour le capteur et pour qui voudra s'y accrocher
/// (musique, spawns nocturnes, comportement de la faune).
#[derive(Resource, Debug, Clone, Copy)]
pub struct CycleState {
    pub etat: EtatCycle,
    pub config: CycleConfig,
}

/// Teinte du soleil selon sa hauteur — **dérivée, pas choisie**.
///
/// Un soleil bas traverse plus d'atmosphère, donc rougit. Garder une lumière
/// blanche pendant qu'elle faiblit donnerait une image qui s'éteint au lieu
/// d'un soir qui tombe : c'est la couleur qui dit l'heure, autant que
/// l'intensité.
#[must_use]
pub fn couleur_soleil(elevation_deg: f32) -> Color {
    // 1 en plein jour (≥ 30°), 0 à l'horizon et en dessous.
    let hauteur = (elevation_deg / 30.0).clamp(0.0, 1.0);
    // Le rouge tient, le vert baisse un peu, le bleu s'effondre.
    Color::srgb(
        1.0,
        0.72 + 0.25 * hauteur,
        0.42 + 0.48 * hauteur,
    )
}

/// Teinte du remplissage : le ciel. Il bleuit à mesure que le direct rougit —
/// c'est ce contraste chaud/froid qui fait lire un crépuscule plutôt qu'une
/// simple baisse de gain.
#[must_use]
pub fn couleur_ciel(elevation_deg: f32) -> Color {
    let hauteur = (elevation_deg / 30.0).clamp(0.0, 1.0);
    Color::srgb(
        0.42 + 0.26 * hauteur,
        0.52 + 0.26 * hauteur,
        0.78 + 0.22 * hauteur,
    )
}

/// Pose le rig au chargement de la carte.
pub fn setup_expedition_lighting(mut commands: Commands) {
    let config = CycleConfig::load_or_default();
    let etat = etat_du_cycle(0.0, &config);

    commands.spawn((
        Name::new("ExpeditionSun"),
        ExpeditionMarker,
        ExpeditionSun,
        DirectionalLight {
            color: couleur_soleil(etat.soleil_elevation_deg),
            illuminance: etat.soleil_lux,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(rotation_soleil(
            etat.soleil_elevation_deg,
            etat.soleil_azimut_deg,
        )),
    ));

    commands.spawn((
        Name::new("ExpeditionFill"),
        ExpeditionMarker,
        ExpeditionFill,
        DirectionalLight {
            color: couleur_ciel(etat.soleil_elevation_deg),
            // Le remplissage vaut une fraction du direct, sans ombres : il
            // rattrape les faces à l'ombre au lieu d'en créer de nouvelles.
            illuminance: etat.ambiante_lux,
            shadows_enabled: false,
            ..default()
        },
        // Vient d'en haut et légèrement à l'opposé du soleil.
        Transform::from_rotation(rotation_soleil(70.0, etat.soleil_azimut_deg + 180.0)),
    ));

    // ⚠️ `AmbientLight` est un composant de CAMERA en Bevy 0.18, pas une
    // Resource comme en 0.15. Il est donc posé par `update_expedition_ambiance`,
    // en même temps que le brouillard — les deux vivent au même endroit et se
    // retirent ensemble à la sortie.
    commands.insert_resource(CycleState { etat, config });

    info!(
        "[expedition-lumiere] rig pose — soleil {:.0}° a {:.0} lux, ambiante {:.0} lux, \
         brouillard {:.0}-{:.0} m",
        etat.soleil_elevation_deg, etat.soleil_lux, etat.ambiante_lux,
        etat.brouillard_debut_m, etat.brouillard_fin_m
    );
}

/// Fait descendre le soleil à mesure que le joueur avance.
#[allow(clippy::type_complexity)]
pub fn update_expedition_cycle(
    active: Option<Res<ActiveExpedition>>,
    mut state: Option<ResMut<CycleState>>,
    q_player: Query<&Transform, With<Player>>,
    mut q_sun: Query<
        (&mut DirectionalLight, &mut Transform),
        (With<ExpeditionSun>, Without<Player>, Without<ExpeditionFill>),
    >,
    mut q_fill: Query<
        (&mut DirectionalLight, &mut Transform),
        (With<ExpeditionFill>, Without<Player>, Without<ExpeditionSun>),
    >,
    mut chemin: Local<Vec<Vec3>>,
) {
    let (Some(active), Some(state)) = (active, state.as_mut()) else {
        return;
    };
    let Ok(joueur) = q_player.single() else {
        return;
    };
    // Le chemin est immuable pendant la partie : on le convertit UNE fois.
    // 91 points reconvertis à chaque frame, ce serait 91 allocations par frame
    // sur un chemin qui ne bouge jamais (`scalability.md` — 0 alloc hot path).
    if chemin.is_empty() {
        chemin.extend(active.gameplay.chemin_bevy());
    }

    let progression = progression_sur_chemin(joueur.translation, &chemin);
    let etat = etat_du_cycle(progression, &state.config);
    state.etat = etat;

    let teinte_soleil = couleur_soleil(etat.soleil_elevation_deg);
    let teinte_ciel = couleur_ciel(etat.soleil_elevation_deg);

    if let Ok((mut light, mut xf)) = q_sun.single_mut() {
        light.illuminance = etat.soleil_lux;
        light.color = teinte_soleil;
        *xf = Transform::from_rotation(rotation_soleil(
            etat.soleil_elevation_deg,
            etat.soleil_azimut_deg,
        ));
    }
    if let Ok((mut light, mut xf)) = q_fill.single_mut() {
        light.illuminance = etat.ambiante_lux;
        light.color = teinte_ciel;
        *xf = Transform::from_rotation(rotation_soleil(70.0, etat.soleil_azimut_deg + 180.0));
    }
}

/// Accroche l'ambiante ET le brouillard à la caméra, et les fait suivre la nuit.
///
/// Les deux sont des composants de **CAMÉRA** en Bevy 0.18 — `AmbientLight` l'est
/// devenu (il était une `Resource` en 0.15), `DistanceFog` l'a toujours été. Ils
/// doivent donc être posés sur chaque `Camera3d` et **retirés à la sortie**,
/// sinon l'Expédition laisse sa nuit et sa brume dans l'arène et le Hall.
#[allow(clippy::type_complexity)]
pub fn update_expedition_ambiance(
    mut commands: Commands,
    state: Option<Res<CycleState>>,
    mut q_cam: Query<
        (Entity, Option<&mut DistanceFog>, Option<&mut AmbientLight>),
        With<Camera3d>,
    >,
) {
    let Some(state) = state else { return };
    let e = state.etat;
    let teinte = couleur_ciel(e.soleil_elevation_deg);
    // Reconstruit dans la boucle : `FogFalloff` n'est pas `Copy`, et le sortir
    // le ferait deplacer a la premiere camera — donc rien pour les suivantes.
    for (entity, fog, ambient) in &mut q_cam {
        let falloff = FogFalloff::Linear {
            start: e.brouillard_debut_m,
            end: e.brouillard_fin_m,
        };
        match fog {
            Some(mut f) => {
                f.color = teinte;
                f.falloff = falloff;
            }
            None => {
                commands.entity(entity).insert(DistanceFog {
                    color: teinte,
                    // Le halo autour du soleil : c'est ce qui donne les couchers
                    // de soleil lisibles plutôt qu'un voile uniforme.
                    directional_light_color: couleur_soleil(e.soleil_elevation_deg),
                    directional_light_exponent: 24.0,
                    falloff,
                });
            }
        }
        match ambient {
            Some(mut a) => {
                a.color = teinte;
                a.brightness = e.ambiante_lux;
            }
            None => {
                commands.entity(entity).insert(AmbientLight {
                    color: teinte,
                    brightness: e.ambiante_lux,
                    ..default()
                });
            }
        }
    }
}

/// Retire tout à la sortie. Le brouillard vit sur la caméra, qui SURVIT au mode :
/// l'oublier laisserait l'Expédition brumeuse dans l'arène et le Hall.
pub fn teardown_expedition_lighting(
    mut commands: Commands,
    q_cam: Query<Entity, With<Camera3d>>,
) {
    for cam in &q_cam {
        commands.entity(cam).remove::<(DistanceFog, AmbientLight)>();
    }
    commands.remove_resource::<CycleState>();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luma(c: Color) -> f32 {
        let s = c.to_srgba();
        0.2126 * s.red + 0.7152 * s.green + 0.0722 * s.blue
    }

    #[test]
    fn le_soleil_rougit_en_descendant() {
        // Garder une lumiere blanche pendant qu'elle faiblit donne une image qui
        // s'ETEINT, pas un soir qui tombe. La couleur dit l'heure autant que
        // l'intensite.
        let midi = couleur_soleil(35.0).to_srgba();
        let couchant = couleur_soleil(2.0).to_srgba();
        assert!(
            couchant.blue < midi.blue - 0.2,
            "le bleu doit s'effondrer : {} -> {}",
            midi.blue,
            couchant.blue
        );
        assert!(
            (couchant.red - midi.red).abs() < 0.05,
            "le rouge doit tenir"
        );
    }

    #[test]
    fn le_ciel_bleuit_quand_le_soleil_rougit() {
        // C'est ce CONTRASTE chaud/froid qui fait lire un crepuscule. Sans lui on
        // ne voit qu'une baisse de gain.
        let bas = couleur_soleil(2.0).to_srgba();
        let ciel = couleur_ciel(2.0).to_srgba();
        assert!(
            ciel.blue > bas.blue,
            "le ciel ({}) doit etre plus froid que le direct ({})",
            ciel.blue,
            bas.blue
        );
    }

    #[test]
    fn les_teintes_ne_s_assombrissent_jamais_jusqu_au_noir() {
        // « Pas mode horreur » : meme sous l'horizon, les COULEURS restent
        // lisibles. C'est l'intensite qui baisse, pas la teinte qui s'eteint —
        // sinon on obtient du gris boueux au lieu d'une nuit bleue.
        for elev in [-20.0_f32, -8.0, 0.0, 15.0, 40.0] {
            assert!(luma(couleur_soleil(elev)) > 0.5, "soleil noir a {elev}°");
            assert!(luma(couleur_ciel(elev)) > 0.4, "ciel noir a {elev}°");
        }
    }

    #[test]
    fn les_teintes_sont_bornees_meme_pour_une_elevation_absurde() {
        // Un genome mal regle ne doit pas produire une couleur hors [0,1], qui
        // donnerait un rendu sature ou negatif selon le pipeline.
        for elev in [-999.0_f32, 999.0] {
            for c in [couleur_soleil(elev), couleur_ciel(elev)] {
                let s = c.to_srgba();
                for v in [s.red, s.green, s.blue] {
                    assert!((0.0..=1.0).contains(&v), "composante {v} hors bornes");
                }
            }
        }
    }
}
