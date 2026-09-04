//! L'occlusion de contact du Hall — ce qui rend le rebond manquant.
//!
//! # Pourquoi ici, et pas ailleurs
//!
//! Depuis que la lumière cuite est coupée (`castle_hub_lighting.toml
//! [lightmaps] enabled = false`, 2026-07-26), le Hall n'a **plus aucune lumière
//! indirecte**. Ce qui n'est pas frappé en direct tombe sur l'ambiante, qui vaut
//! 400 et n'a pas de direction : les coins ne se creusent pas, rien ne touche le
//! sol, tout flotte sur un plancher de lumière uniforme.
//!
//! Le SSAO ne rend pas le rebond — il en rend la partie qui se voit le plus :
//! l'assombrissement de contact. Il n'agit d'ailleurs QUE sur la lumière
//! indirecte (ambiante 400 + éclairage par image 900), jamais sur le soleil ni
//! sur les 56 lumières du créateur. C'est exactement la portion qui est plate.
//!
//! # Le jumeau, et pourquoi il est éteint
//!
//! `forgia-mode-roguelite::render_quality` sait déjà attacher le SSAO, et le
//! génome roguelite le laisse à `0.0` : combiné au post-traitement toon de
//! l'arène, il BLANCHIT la scène (constaté en jeu le 2026-06-30, les maillages
//! sont dans le frustum mais ne se dessinent plus). Le Hall n'a pas ce
//! post-traitement — la raison de couper là-bas ne s'applique pas ici.
//!
//! Les deux sites répètent une règle : **SSAO et MSAA sont exclusifs** (sinon
//! `ERROR b0004` à chaque frame et un SSAO inopérant). Une règle recopiée est
//! une règle qui se désynchronise. Ici elle est rendue sûre par deux choses : la
//! coordination réelle est CENTRALE — `forgia-ui-lib::apply_msaa_to_cameras`
//! ignore les caméras qui portent un `ScreenSpaceAmbientOcclusion` et restaure
//! le MSAA de l'utilisateur dès qu'il est retiré — et le conflit est MESURÉ,
//! publié par `forgia2_castle_lighting.json`. Le jour où un troisième site
//! apparaît, c'est la crate fine qu'il faudra extraire.
//!
//! # Ce que ça coûte
//!
//! Le SSAO exige les pré-passes de profondeur et de normale (Bevy les ajoute par
//! `#[require]`) et force `Msaa::Off`. Il n'est supporté ni sur WebGL2 ni sur
//! WebGPU : le portage web devra le couper par la donnée, ce que le génome
//! permet déjà.

use bevy::pbr::{ScreenSpaceAmbientOcclusion, ScreenSpaceAmbientOcclusionQualityLevel};
use bevy::prelude::*;
use bevy::render::view::Msaa;
use forgia_core::prelude::{GameMode, GameSet};
use forgia_player::ViewmodelCamera;

use crate::castle_flames::CastleLighting;

/// Marqueur : cette caméra a reçu le SSAO du Hall, et devra le rendre en
/// sortant. Sans lui, on ne saurait pas quoi détacher — et un SSAO oublié
/// forcerait `Msaa::Off` pour le reste de la session.
#[derive(Component)]
struct CastleSsaoApplied;

/// 🚨 L'ÉCHANTILLONNAGE EST UNE PROPRIÉTÉ DE LA FENÊTRE, PAS DE LA CAMÉRA.
///
/// Mesuré en jeu le 2026-08-21, et ça a coûté une session de débogage : le SSAO
/// exige `Msaa::Off`, je l'avais posé sur la seule caméra 3D — et le Hall est
/// devenu NOIR, avec l'interface intacte par-dessus (« j'ai que les fps »).
///
/// La fenêtre porte deux caméras : la 3D (ordre 0) et `MenuCamera2d` (ordre 10,
/// egui, `clear_color: None`, `msaa_writeback: Auto`). Avec la 3D à 1
/// échantillon et l'interface à 4, celle qui passe en dernier réécrit la cible :
/// la 3D disparaît, l'interface survit. Aucune erreur, aucun avertissement —
/// les pipelines sont valides, c'est le composite qui est faux. C'est pour ça
/// que le journal était muet, et c'est la même famille que l'« arène blanchie »
/// du jumeau roguelite.
///
/// On mémorise donc l'échantillonnage d'origine de CHAQUE caméra de la fenêtre
/// pour le rendre à la sortie. `None` = elle n'en portait pas, il faudra
/// retirer le composant plutôt que d'en inventer un.
#[derive(Component)]
struct MsaaForceParSsao(Option<Msaa>);

pub struct CastleSsaoPlugin;

impl Plugin for CastleSsaoPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            appliquer_ssao
                .in_set(GameSet::Effects)
                .run_if(in_state(GameMode::CastleHub)),
        )
        .add_systems(OnExit(GameMode::CastleHub), retirer_ssao);
    }
}

fn qualite(niveau: u8) -> ScreenSpaceAmbientOcclusionQualityLevel {
    match niveau {
        0 => ScreenSpaceAmbientOcclusionQualityLevel::Low,
        1 => ScreenSpaceAmbientOcclusionQualityLevel::Medium,
        3 => ScreenSpaceAmbientOcclusionQualityLevel::Ultra,
        // `parse_toml` borne déjà à 0..=3, donc ce bras ne reçoit que 2. Le
        // laisser nommer High plutôt que « tout le reste » évite qu'une valeur
        // hors table passe pour un réglage choisi.
        _ => ScreenSpaceAmbientOcclusionQualityLevel::High,
    }
}

/// Attache le SSAO aux caméras du Hall, et le retire si la donnée l'éteint.
///
/// Tourne à chaque frame plutôt qu'une fois à l'entrée : le génome est
/// rechargeable à chaud, et la caméra du Hall n'est pas créée par le Hall — elle
/// préexiste, donc un simple `OnEnter` raterait celles qui arrivent après.
/// Le travail réel est nul quand rien ne change : les requêtes sont filtrées par
/// le marqueur, donc chaque caméra n'est touchée qu'une fois.
fn appliquer_ssao(
    mut commands: Commands,
    reglages: Res<CastleLighting>,
    q_sans: Query<Entity, (With<Camera3d>, Without<CastleSsaoApplied>, Without<ViewmodelCamera>)>,
    q_avec: Query<Entity, With<CastleSsaoApplied>>,
    // TOUTES les caméras, 2D comprise : l'échantillonnage se règle par fenêtre.
    q_cameras: Query<(Entity, Option<&Msaa>), (With<Camera>, Without<MsaaForceParSsao>)>,
    q_msaa_forcee: Query<(Entity, &MsaaForceParSsao)>,
) {
    if reglages.ssao_enabled {
        for cam in &q_sans {
            commands.entity(cam).insert((
                ScreenSpaceAmbientOcclusion {
                    quality_level: qualite(reglages.ssao_quality),
                    constant_object_thickness: reglages.ssao_thickness,
                },
                CastleSsaoApplied,
            ));
        }
        // 🚨 Sur TOUTE la fenêtre, interface comprise. Ne le poser que sur la
        // caméra 3D rendait le Hall noir : voir `MsaaForceParSsao`.
        for (cam, msaa) in &q_cameras {
            commands
                .entity(cam)
                .insert((Msaa::Off, MsaaForceParSsao(msaa.copied())));
        }
    } else {
        // La donnée a éteint le SSAO en cours de session : tout rendre sur-le-
        // champ, sans attendre la sortie du Hall. C'est ce qui rend le réglage
        // jugeable à chaud, alt-tab compris.
        for cam in &q_avec {
            detacher(&mut commands, cam);
        }
        rendre_msaa(&mut commands, &q_msaa_forcee);
    }
}

fn retirer_ssao(
    mut commands: Commands,
    q: Query<Entity, With<CastleSsaoApplied>>,
    q_msaa_forcee: Query<(Entity, &MsaaForceParSsao)>,
) {
    for cam in &q {
        detacher(&mut commands, cam);
    }
    rendre_msaa(&mut commands, &q_msaa_forcee);
}

/// Rend à chaque caméra l'échantillonnage qu'elle avait avant le SSAO.
///
/// Rendre, pas deviner : une caméra qui n'en portait pas doit se retrouver SANS
/// composant, sinon on lui impose un réglage qu'elle n'avait jamais choisi et
/// le défaut du moteur ne s'applique plus jamais.
fn rendre_msaa(commands: &mut Commands, q: &Query<(Entity, &MsaaForceParSsao)>) {
    for (cam, avant) in q {
        let mut e = commands.entity(cam);
        match avant.0 {
            Some(msaa) => {
                e.insert(msaa);
            }
            None => {
                e.remove::<Msaa>();
            }
        }
        e.remove::<MsaaForceParSsao>();
    }
}

/// Retire le SSAO d'une caméra. L'échantillonnage, lui, est rendu par
/// `rendre_msaa` — il concerne toute la fenêtre, pas cette seule caméra.
///
/// On ne retire PAS non plus `DepthPrepass`/`NormalPrepass`, que le `#[require]`
/// de Bevy a ajoutés : ils survivent à la sortie du Hall. C'est délibéré et
/// aligné sur `render_quality::sys_detach_ssao`, qui fait de même — d'autres
/// effets peuvent s'appuyer dessus, et les arracher casserait plus que le peu
/// qu'on économiserait.
fn detacher(commands: &mut Commands, cam: Entity) {
    commands
        .entity(cam)
        .remove::<(ScreenSpaceAmbientOcclusion, CastleSsaoApplied)>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_table_de_qualite_couvre_le_genome() {
        // Le génome documente « 0 Low · 1 Medium · 2 High · 3 Ultra ». Si la
        // table et le commentaire divergent, un créateur règle 3 et obtient High
        // sans que rien ne le dise.
        assert_eq!(qualite(0), ScreenSpaceAmbientOcclusionQualityLevel::Low);
        assert_eq!(qualite(1), ScreenSpaceAmbientOcclusionQualityLevel::Medium);
        assert_eq!(qualite(2), ScreenSpaceAmbientOcclusionQualityLevel::High);
        assert_eq!(qualite(3), ScreenSpaceAmbientOcclusionQualityLevel::Ultra);
    }
}
