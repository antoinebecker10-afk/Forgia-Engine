//! Le capteur d'éclairage du Hall — ce qui éclaire **vraiment**, pas ce qui est
//! déclaré.
//!
//! # Pourquoi ce module existe
//!
//! Sept sources éclairent le Hall — soleil, remplissage ciel, ambiante, éclairage
//! par image, 56 lumières du créateur, 96 bougies, lumière cuite — et trois
//! capteurs seulement les couvraient : les flammes, l'éclairage par image et les
//! lightmaps. Les **quatre leviers** du fichier de réglage (`[ambient]` et
//! `[creator_lights]` de `castle_hub_lighting.toml`) n'étaient couverts par rien.
//!
//! Sur « regarde l'éclairage », il n'y avait donc rien à lire — et l'audit du
//! 2026-08-21 a dû relire le code source pour établir un fait que le jeu aurait
//! dû publier lui-même : **une seule des sept sources porte des ombres**.
//!
//! # Deux grandeurs, jamais une seule
//!
//! Chaque réglage est publié DEUX fois : tel que **déclaré** dans le génome, et
//! tel qu'il existe sur **l'entité**. Publier seulement le TOML reviendrait à
//! relire son propre fichier de configuration — ça ne prouve rien. C'est
//! justement l'écart entre les deux (hot-reload qui n'atteint pas la lumière,
//! système qui ne tourne pas, entité despawnée) qui constitue le défaut, et il
//! devient ici une alerte au lieu d'une énigme.
//!
//! # Ce qu'il ne couvre pas
//!
//! Le détail des flammes (`forgia2_castle_flames.json`), de l'éclairage par
//! image (`forgia2_castle_envmap.json`) et de la lumière cuite
//! (`forgia2_castle_lightmaps.json`) reste chez eux : ce capteur-ci donne la vue
//! d'ensemble et l'écart déclaré/effectif, pas leur mécanique interne. Il ne
//! juge pas non plus l'esthétique — aucun chiffre ne dit si le Hall est beau.

use bevy::pbr::ScreenSpaceAmbientOcclusion;
use bevy::prelude::*;
use bevy::render::view::Msaa;
use forgia_core::constat;
use forgia_core::prelude::GameMode;

use crate::castle_flames::{CastleLighting, CreatorLight, CreatorLightCount, FlameStats};
use crate::castle_hub::{CastleFillLight, CastleKeyLight};

/// Cadence de publication. Alignée sur les trois autres capteurs du Hall pour
/// que le digest les lise dans la même fenêtre.
const PERIODE_S: f32 = 1.0;

/// Tolérance relative entre valeur déclarée et valeur portée par l'entité.
///
/// Pas zéro : l'aller-retour lux → `illuminance` passe par des `f32` et un
/// hot-reload peut être lu à une frame près. 1 % laisse passer le bruit
/// numérique et attrape tout écart réel — un facteur oublié, une unité fausse,
/// un système qui ne tourne pas se voient à des ordres de grandeur, jamais à 1 %.
const TOLERANCE_RELATIVE: f32 = 0.01;

pub struct CastleLightingSensorPlugin;

impl Plugin for CastleLightingSensorPlugin {
    fn build(&self, app: &mut App) {
        // 🚨 PAS de `run_if(in_state(CastleHub))`. Un capteur qui ne tourne pas
        // laisse son fichier figé sur sa dernière valeur, et une valeur figée se
        // lit comme une mesure fraîche. Il tourne toujours et publie `au_hall`.
        app.add_systems(Update, capteur_eclairage);
    }
}

/// Un écart déclaré/effectif, prêt à être raconté.
struct Ecart {
    quoi: &'static str,
    declare: f32,
    effectif: f32,
}

/// Compare une valeur du génome à celle portée par l'entité.
///
/// `effectif` vaut `None` quand l'entité n'existe pas — ce n'est PAS un écart
/// (hors du Hall, c'est l'état normal), c'est une absence, comptée à part.
fn ecart(quoi: &'static str, declare: f32, effectif: Option<f32>) -> Option<Ecart> {
    let effectif = effectif?;
    let reference = declare.abs().max(effectif.abs()).max(1.0);
    ((declare - effectif).abs() / reference > TOLERANCE_RELATIVE).then_some(Ecart {
        quoi,
        declare,
        effectif,
    })
}

#[allow(clippy::too_many_arguments)]
fn capteur_eclairage(
    time: Res<Time>,
    mode: Res<State<GameMode>>,
    reglages: Res<CastleLighting>,
    flammes: Res<FlameStats>,
    createur: Res<CreatorLightCount>,
    q_soleil: Query<&DirectionalLight, With<CastleKeyLight>>,
    q_fill: Query<&DirectionalLight, With<CastleFillLight>>,
    q_ambiante: Query<&AmbientLight>,
    q_createur_point: Query<&PointLight, With<CreatorLight>>,
    q_createur_spot: Query<&SpotLight, With<CreatorLight>>,
    q_ssao: Query<Option<&Msaa>, With<ScreenSpaceAmbientOcclusion>>,
    // TOUTES les cameras, 2D comprise : voir `msaa_distincts` plus bas.
    q_msaa: Query<Option<&Msaa>, With<Camera>>,
    mut prochain: Local<f32>,
) {
    let maintenant = time.elapsed_secs();
    if maintenant < *prochain {
        return;
    }
    *prochain = maintenant + PERIODE_S;

    let au_hall = matches!(mode.get(), GameMode::CastleHub);

    // ── Ce qui existe réellement dans le monde ────────────────────────────
    let soleil = q_soleil.iter().next();
    let fill = q_fill.iter().next();
    // L'ambiante est un composant porté par la caméra (cf `castle_hub`), pas une
    // ressource : c'est sur l'entité qu'il faut la lire.
    let ambiante = q_ambiante.iter().next();

    let createur_points = q_createur_point.iter().count();
    let createur_spots = q_createur_spot.iter().count();
    let createur_poses = createur_points + createur_spots;

    // ── Le fait structurel, rendu permanent ───────────────────────────────
    //
    // Sur toutes les lumières du Hall, combien portent des ombres ? La réponse
    // (une seule, le soleil) explique la platitude de l'intérieur : tout le
    // reste traverse la pierre. Ce chiffre doit vivre dans le capteur, pas dans
    // le souvenir d'un audit.
    let avec_ombres = usize::from(soleil.is_some_and(|l| l.shadows_enabled))
        + usize::from(fill.is_some_and(|l| l.shadows_enabled))
        + q_createur_point.iter().filter(|l| l.shadows_enabled).count()
        + q_createur_spot.iter().filter(|l| l.shadows_enabled).count();
    let sources_directes =
        usize::from(soleil.is_some()) + usize::from(fill.is_some()) + createur_poses;
    let sans_ombres = sources_directes.saturating_sub(avec_ombres);

    // L'échantillon : ce que ce verdict a réellement examiné. Hors du Hall il
    // vaut 0, et `Constat` dégrade alors tout `ok` en « AVEUGLE » — c'est le but.
    let echantillon = sources_directes + flammes.lit as usize + usize::from(ambiante.is_some());

    // ── SSAO : posé, et surtout OPÉRANT ───────────────────────────────────
    //
    // Son mode d'échec n'est pas l'absence, c'est la présence inerte : avec du
    // MSAA sur la même caméra, Bevy journalise `b0004` à chaque frame et le SSAO
    // ne rend RIEN. Un effet qui coûte sans se voir est pire qu'un effet absent,
    // parce qu'on le croit acquis. Compter les deux séparément.
    let ssao_pose = q_ssao.iter().count();
    // 🚨 LA MESURE QUI MANQUAIT LE 2026-08-21.
    //
    // Le Hall est devenu NOIR, interface intacte par-dessus, sans une seule
    // ligne d'erreur : la camera 3D etait a 1 echantillon et celle de l'interface
    // a 4, sur la MEME fenetre. Celle qui passe en dernier reecrit la cible.
    // Les pipelines etaient valides — rien ne pouvait le signaler. Ce compte,
    // lui, l'aurait dit en une lecture.
    let mut echantillonnages: Vec<u32> = q_msaa
        .iter()
        .map(|m| m.map_or(4, |m| m.samples()))
        .collect();
    echantillonnages.sort_unstable();
    echantillonnages.dedup();
    let msaa_distincts = echantillonnages.len();
    let ssao_conflit_msaa = q_ssao
        .iter()
        .filter(|m| m.is_some_and(|m| !matches!(m, Msaa::Off)))
        .count();

    // ── Déclaré contre effectif ───────────────────────────────────────────
    //
    // 🚨 Seulement AU HALL. Mesuré le 2026-08-21 : hors du Hall ce capteur
    // publiait `ecarts: 1` en comparant l'ambiante DÉCLARÉE du château (400) à
    // celle qui traînait sur la caméra d'un autre mode (871, puis 270 dans une
    // autre partie). Deux grandeurs qui ne parlent pas de la même chose : la
    // comparaison n'avait aucun sens, et un chiffre faux dans un capteur coûte
    // plus cher qu'une case vide. Une mesure porte sa condition.
    let ecarts: Vec<Ecart> = if au_hall {
        [
            ecart("soleil", reglages.key_lux, soleil.map(|l| l.illuminance)),
            ecart("fill", reglages.fill_lux, fill.map(|l| l.illuminance)),
            ecart(
                "ambiante",
                reglages.ambient_brightness,
                ambiante.map(|a| a.brightness),
            ),
        ]
        .into_iter()
        .flatten()
        .collect()
    } else {
        Vec::new()
    };

    // ── Le verdict ────────────────────────────────────────────────────────
    let verdict = if !au_hall {
        constat::info("hors du Hall — aucun eclairage de chateau a mesurer")
    } else if sources_directes == 0 {
        constat::critique("au Hall mais AUCUNE lumiere directe posee — la scene est noire ou eclairee par la seule ambiante")
            .remede("verifier que spawn_castle_hub a tourne (marqueurs CastleKeyLight/CastleFillLight) et que castle_hub_lighting.toml est lisible")
    } else if !ecarts.is_empty() {
        let details = ecarts
            .iter()
            .map(|e| format!("{} declare {:.0} mais porte {:.0}", e.quoi, e.declare, e.effectif))
            .collect::<Vec<_>>()
            .join(" ; ");
        constat::alerte(format!("le genome n'atteint pas la lumiere : {details}"))
            .remede("le hot-reload de castle_hub_lighting.toml ne repropage pas jusqu'a l'entite — verifier le systeme qui applique key_lux/fill_lux/ambient a la DirectionalLight et a la camera")
    } else if ssao_conflit_msaa > 0 {
        constat::alerte(format!(
            "{ssao_conflit_msaa} camera(s) portent le SSAO ET du MSAA — Bevy journalise b0004 a chaque frame et le SSAO ne rend RIEN"
        ))
        .remede("castle_ssao::appliquer_ssao doit poser Msaa::Off avec le SSAO ; verifier qu'un autre systeme ne repose pas le MSAA par-dessus (forgia-ui-lib::apply_msaa_to_cameras doit exclure les cameras a SSAO)")
    } else if msaa_distincts > 1 {
        constat::alerte(format!(
            "{msaa_distincts} echantillonnages MSAA differents sur la meme fenetre ({echantillonnages:?}) — la derniere camera reecrit la cible et efface les precedentes, SANS aucune erreur"
        ))
        .remede("l'echantillonnage est une propriete de la FENETRE : castle_ssao doit poser Msaa::Off sur toutes les cameras, interface comprise, pas seulement sur la 3D")
    } else if reglages.ssao_enabled && ssao_pose == 0 {
        constat::alerte("le SSAO est active dans le genome mais pose sur aucune camera")
            .remede("verifier castle_ssao::appliquer_ssao (la camera du Hall preexiste et peut porter ViewmodelCamera, qui est exclu)")
    } else if reglages.creator_lights_enabled && createur_poses == 0 {
        constat::alerte("les 56 lumieres du createur sont activees mais aucune n'est posee")
            .remede("verifier assets/genomes/castle_hub_creator_lights.toml (lisible ? parse ?) et sys_spawn_creator_lights")
    } else {
        constat::ok()
    };

    let charge = format!(
        r#""au_hall":{au_hall},"soleil_lux_declare":{:.0},"soleil_lux_effectif":{},"soleil_ombres":{},"fill_lux_declare":{:.0},"fill_lux_effectif":{},"fill_ombres":{},"ambiante_declaree":{:.0},"ambiante_effective":{},"createur_actives":{},"createur_echelle":{:.0},"createur_poses":{createur_poses},"createur_points":{createur_points},"createur_spots":{createur_spots},"flammes_posees":{},"flammes_allumees":{},"ibl_active":{},"ibl_intensite":{:.0},"lightmaps_actives":{},"lightmaps_exposure":{:.0},"cuisson_remplace_le_direct":{},"sources_directes":{sources_directes},"avec_ombres":{avec_ombres},"sans_ombres":{sans_ombres},"ssao_declare":{},"ssao_qualite":{},"ssao_epaisseur":{:.2},"ssao_pose":{ssao_pose},"ssao_conflit_msaa":{ssao_conflit_msaa},"msaa_distincts":{msaa_distincts},"msaa_valeurs":{echantillonnages:?},"ecarts":{}"#,
        reglages.key_lux,
        soleil.map_or_else(|| "null".to_string(), |l| format!("{:.0}", l.illuminance)),
        soleil.is_some_and(|l| l.shadows_enabled),
        reglages.fill_lux,
        fill.map_or_else(|| "null".to_string(), |l| format!("{:.0}", l.illuminance)),
        fill.is_some_and(|l| l.shadows_enabled),
        reglages.ambient_brightness,
        ambiante.map_or_else(|| "null".to_string(), |a| format!("{:.0}", a.brightness)),
        reglages.creator_lights_enabled,
        reglages.creator_light_scale,
        flammes.total,
        flammes.lit,
        reglages.env_enabled,
        reglages.env_intensity,
        reglages.lightmaps_enabled,
        reglages.lightmaps_exposure,
        // Le drapeau qui decide si l'ambiante et le soleil s'appliquent encore
        // aux pieces cuites. Il change le rendu du tout au tout, et rien ne le
        // publiait : un reglage muet est un reglage qu'on oublie.
        reglages.lightmaps_enabled && reglages.bake_includes_direct,
        reglages.ssao_enabled,
        reglages.ssao_quality,
        reglages.ssao_thickness,
        ecarts.len(),
    );

    // `createur.0` est le compte annoncé par le spawner ; `createur_poses` est
    // celui des entités trouvées. Les deux doivent coïncider — s'ils divergent,
    // c'est le spawner qui compte mal, et le capteur le montre côte à côte.
    let charge = format!(r#"{charge},"createur_annonces":{}"#, createur.0);

    verdict.echantillon(echantillon).publier(
        "castle_lighting",
        maintenant,
        &charge,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_absence_n_est_pas_un_ecart() {
        // Hors du Hall la lumière n'existe pas : ce n'est pas un défaut de
        // propagation, c'est qu'il n'y a rien. Confondre les deux ferait hurler
        // le capteur à chaque partie qui n'entre jamais au château.
        assert!(ecart("soleil", 12_000.0, None).is_none());
    }

    #[test]
    fn le_bruit_numerique_ne_declenche_rien() {
        assert!(ecart("soleil", 12_000.0, Some(12_050.0)).is_none());
    }

    #[test]
    fn un_facteur_oublie_se_voit() {
        // Le cas réel visé : une valeur qui n'a pas été repropagée après
        // hot-reload garde l'ancienne — souvent un ordre de grandeur d'écart.
        let e = ecart("fill", 600.0, Some(7000.0)).expect("7000 vs 600 doit alerter");
        assert_eq!(e.quoi, "fill");
    }

    #[test]
    fn zero_mesure_ne_peut_pas_etre_vert() {
        // La garantie que ce capteur hérite de `Constat` : hors du Hall, un
        // `ok()` sur zéro source se dégrade au lieu de mentir.
        let (severite, message) = constat::ok().echantillon(0).verdict();
        assert_eq!(severite, constat::Severite::Info);
        assert!(message.contains("AVEUGLE"));
    }
}
